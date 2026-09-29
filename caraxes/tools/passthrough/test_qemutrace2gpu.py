#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Unit tests for qemutrace2gpu.py on a synthetic QEMU 11.1 log (no card, no VM).
Run: python3 -m unittest -v caraxes/tools/passthrough/test_qemutrace2gpu.py"""
import contextlib
import io
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import qemutrace2gpu as q  # noqa: E402

# The shapes QEMU 11.1.1 prints (hw/vfio/trace-events, log backend, -msg timestamp=on).
SAMPLE = """\
2026-10-03T12:00:00.000100Z vfio_region_read  (0000:2d:00.0:region0+0x0, 4) = 0x166000a1
2026-10-03T12:00:00.000150Z vfio_region_read  (0000:2d:00.1:region0+0x0, 4) = 0x10de10f9
2026-10-03T12:00:00.000200Z vfio_region_read  (0000:2d:00.0:region0+0x9410, 4) = 0x1b
2026-10-03T12:00:00.000210Z vfio_region_read  (0000:2d:00.0:region0+0x9400, 4) = 0x4d704ae0
2026-10-03T12:00:00.000300Z vfio_region_write  (0000:2d:00.0:region0+0x680000, 0x10, 4)
2026-10-03T12:00:00.000400Z vfio_region_read  (0000:2d:00.0:region0+0x640000, 8) = 0xff00000f0f
2026-10-03T12:00:00.000500Z vfio_region_read  (0000:2d:00.0:region0+0x88000, 4) = 0xdeadbeef
2026-10-03T12:00:00.000501Z vfio_pci_read_config  (0000:2d:00.0, @0x0, len=0x4) 0x1f0210de
2026-10-03T12:00:00.000502Z vfio_quirk_generic_mirror_read 0000:2d:00.0 vfio-nvidia-bar0-88000-mirror-quirk 0x0: 0x1f0210de
2026-10-03T12:00:00.000600Z vfio_pci_write_config  (0000:2d:00.0, @0x704, 0x0, len=0x4)
2026-10-03T12:00:00.000601Z vfio_quirk_generic_mirror_write 0000:2d:00.0 vfio-nvidia-bar0-88000-mirror-quirk 0x704: 0x0
2026-10-03T12:00:00.000700Z vfio_msi_interrupt  (0000:2d:00.0) vector 0 0xfee00000/0x4021
2026-10-03T12:00:00.000800Z vfio_region_write  (0000:2d:00.0:region1+0x1000, 0x1234, 4)
2026-10-03T12:00:00.000900Z vfio_region_read  (0000:2d:00.0:region0+0x100000, 4) = 0x5
2026-10-03T12:00:00.017400Z vfio_msi_interrupt  (0000:2d:00.0) vector 0 0xfee00000/0x4021
2026-10-03T12:00:00.017500Z vfio_pci_reset  (0000:2d:00.0)
2026-10-03T12:00:00.017600Z vfio_region_read  (0000:2d:00.0:region0+0x616330, 4) = 0x45d
"""

EXPECTED_ALL = [
    "           0 R bar0 0x00000000 4  0x166000a1   NV_PMC_BOOT_0",
    "         100 R bar0 0x00009410 4  0x1b         NV_PTIMER_TIME",
    "         110 R bar0 0x00009400 4  0x4d704ae0   NV_PTIMER_TIME",
    "         200 W bar0 0x00680000 4  0x10",
    "         300 R bar0 0x00640000 8  0xff00000f0f",
    "         402 R bar0 0x00088000 4  0x1f0210de   (cfg mirror, hw=0xdeadbeef)",
    "         501 W bar0 0x00088704 4  0x0          NV_XVE_CYA_2 (cfg mirror)",
    "         600 E msi vector 0 addr 0xfee00000 data 0x4021",
    "         700 W bar1 0x00001000 4  0x1234",
    "         800 R bar0 0x00100000 4  0x5",
    "       17300 E msi vector 0 addr 0xfee00000 data 0x4021",
    "       17400 E qemu vfio_pci_reset (0000:2d:00.0)",
    "       17500 R bar0 0x00616330 4  0x45d",
]

# A few lines of a Caraxes device trace: the names the converter borrows.
GPU_TRACE_NAMES = """\
   118823165 R bar0 0x00009410 4  0x1b         NV_PTIMER_TIME
   118823171 R bar0 0x00009400 4  0x4d704ae0   NV_PTIMER_TIME
      123605 E boot: display: auto dump on
    78836525 W bar0 0x00690000 4  0x0
"""
TABLE_NAMES = "0x00000000 NV_PMC_BOOT_0\n0x88704 NV_XVE_CYA_2\n"


def write(path, text):
    with open(path, 'w') as f:
        f.write(text)


def read_lines(path):
    with open(path) as f:
        return f.read().splitlines()


def run(lines, **kw):
    conv = q.Converter(**kw)
    out = []
    for line in lines.splitlines(keepends=True):
        out.extend(conv.feed(line))
    out.extend(conv.finish())
    return out, conv


def names():
    with tempfile.TemporaryDirectory() as d:
        a, b = os.path.join(d, 'gpu-trace.log'), os.path.join(d, 'table.txt')
        write(a, GPU_TRACE_NAMES)
        write(b, TABLE_NAMES)
        n = q.load_names(a)
        n.update(q.load_names(b))
        return n


class Format(unittest.TestCase):
    def test_access_line_matches_the_device_trace(self):
        # C73-lf12/gpu-trace.log: "   118823165 R bar0 0x00009410 4  0x1b         NV_PTIMER_TIME"
        self.assertEqual(q.fmt_access(118823165, 'R', 'bar0', 0x9410, 4, 0x1b, 'NV_PTIMER_TIME'),
                         "   118823165 R bar0 0x00009410 4  0x1b         NV_PTIMER_TIME")
        # unnamed: trailing blanks trimmed, as trace.rs does
        self.assertEqual(q.fmt_access(78836525, 'W', 'bar0', 0x690000, 4, 0, ''),
                         "    78836525 W bar0 0x00690000 4  0x0")

    def test_event_line(self):
        self.assertEqual(q.fmt_event(212651, 'set_irqs index=0'), "      212651 E set_irqs index=0")


class Convert(unittest.TestCase):
    def test_whole_sample(self):
        out, conv = run(SAMPLE, names=names())
        self.assertEqual(out, EXPECTED_ALL)
        self.assertTrue(conv.stamped)
        self.assertEqual(conv.stats['unparsed'], 0)
        self.assertEqual(conv.stats['msi'], 2)

    def test_other_function_is_dropped_and_all_keeps_it(self):
        out, _ = run(SAMPLE)
        self.assertFalse(any('0x10de10f9' in o for o in out))
        out, _ = run(SAMPLE, device='all')
        self.assertTrue(any('0x10de10f9' in o for o in out))

    def test_flip_preset_keeps_display_ptimer_and_mirror_only(self):
        out, _ = run(SAMPLE, ranges=q.PRESETS['flip'])
        offs = [o.split()[3] for o in out if o.split()[1] in 'RW']
        self.assertEqual(offs, ['0x00009410', '0x00009400', '0x00680000', '0x00640000',
                                '0x00088000', '0x00088704', '0x00616330'])
        self.assertIn("         600 E msi vector 0 addr 0xfee00000 data 0x4021", out)

    def test_region_filter(self):
        out, _ = run(SAMPLE, regions=['bar1'], events=False)
        self.assertEqual(out, ["         700 W bar1 0x00001000 4  0x1234"])

    def test_time_base_is_shared_by_filtered_extracts(self):
        full, _ = run(SAMPLE)
        disp, _ = run(SAMPLE, ranges=q.PRESETS['display'], events=False)
        self.assertIn(disp[-1], full)

    def test_explicit_t0_and_window(self):
        t0 = q.parse_iso('2026-10-03T12:00:00.017000Z')
        out, conv = run(SAMPLE, t0=t0, t_from=0, t_to=550)
        self.assertEqual(out, ["         400 E msi vector 0 addr 0xfee00000 data 0x4021",
                               "         500 E qemu vfio_pci_reset (0000:2d:00.0)"])
        self.assertEqual(len(conv.msi_times), 1)

    def test_msi_rate(self):
        _, conv = run(SAMPLE)
        self.assertIn('MSIs: 2 in the window, 59.9/s', conv.summary())

    def test_markers_are_merged_in_time_order(self):
        marks = [(q.parse_iso('2026-10-03T12:00:00.000250Z'), 'dwm restart'),
                 (q.parse_iso('2026-10-03T12:00:01.000000Z'), 'end')]
        out, _ = run(SAMPLE, markers=marks)
        self.assertEqual(out[3], "         150 E mark dwm restart")
        self.assertEqual(out[-1], "      999900 E mark end")

    def test_old_prefix_and_unstamped_logs(self):
        old = ("4242@1790000000.000010:vfio_region_read  (0000:2d:00.0:region0+0x9410, 4) = 0x1\n"
               "4242@1790000000.000030:vfio_region_read  (0000:2d:00.0:region0+0x9410, 4) = 0x2\n")
        out, _ = run(old)
        self.assertEqual([o.split()[0] for o in out], ['0', '20'])
        bare = ("vfio_region_read  (0000:2d:00.0:region0+0x9410, 4) = 0x1\n"
                "vfio_region_read  (0000:2d:00.0:region0+0x9414, 4) = 0x2\n")
        out, conv = run(bare)
        self.assertEqual([o.split()[0] for o in out], ['1', '2'])
        self.assertIn('no timestamps', conv.summary())

    def test_foreign_and_broken_lines(self):
        junk = ("2026-10-03T12:00:00.000001Z vm_state_notify running 1 reason 9 (running)\n"
                "2026-10-03T12:00:00.000002Z vfio_region_read  (0000:2d:00.0:region0+0xzz, 4) = 0x1\n"
                "qemu-system-x86_64: terminating on signal 15\n")
        out, conv = run(junk)
        self.assertEqual(out, [])
        self.assertEqual(conv.stats['unparsed'], 1)

    def test_cli_round_trip(self):
        with tempfile.TemporaryDirectory() as d:
            log, res, mk = (os.path.join(d, n) for n in ('qemu-trace.log', 'out.log', 'markers.txt'))
            write(log, SAMPLE)
            write(mk, '2026-10-03T12:00:00.000250Z dwm restart\n')
            err = io.StringIO()
            with contextlib.redirect_stderr(err):
                rc = q.main([log, '-o', res, '--preset', 'ptimer', '--markers', mk, '--no-events'])
            self.assertEqual(rc, 0)
            self.assertEqual(read_lines(res),
                             ["         100 R bar0 0x00009410 4  0x1b",
                              "         110 R bar0 0x00009400 4  0x4d704ae0"])
            self.assertIn('2 accesses', err.getvalue())


if __name__ == '__main__':
    unittest.main()
