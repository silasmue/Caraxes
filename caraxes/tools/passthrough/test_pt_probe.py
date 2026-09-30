#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Unit tests for pt-probe.py against a simulated card (no card, no VM, no root).
Run: python3 -m unittest -v caraxes/tools/passthrough/test_pt_probe.py"""
import importlib.util
import io
import os
import subprocess
import sys
import tempfile
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('pt_probe', os.path.join(HERE, 'pt-probe.py'))
pp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pp)

VT, FRAME_US = 1125, 1e6 / 60          # 1920x1080@60, CEA timing: 1125 lines
LT = FRAME_US / VT                     # 14.815 us per line
LATCH, FETCH = 1081, 85                # latch at BLANK_START+1, fetch ends 85 lines earlier
HEAD, WINDOW = 0, 0


class FakeCard:
    """A raster, EVT_STAT with a 30 us ISR, and one window channel whose UPDATE waits for the
    blank: before line LATCH-FETCH it latches at the coming blank, later it is held a frame."""

    def __init__(self, flips):
        self.t = 0.0
        self.put, self.flips = 0x100, []
        for fr, ln in flips:
            t_w = (fr * VT + ln) * LT + 1
            nxt = fr * VT + LATCH if ln < LATCH - FETCH else (fr + 1) * VT + LATCH
            if ln >= LATCH:
                nxt = (fr + 1) * VT + LATCH
            self.put += 0x40
            self.flips.append((t_w, nxt * LT, self.put))

    def read(self, off):
        self.t += 3.0
        pos = int(self.t / LT)
        fr, ln = divmod(pos, VT)
        fstart = fr * FRAME_US
        if off == pp.dpca(HEAD):
            return (fr & 0xffff) << 16 | ln
        if off == 0x611800 + 4 * HEAD:
            v = 0
            if 0 <= self.t - (fstart + (LATCH - FETCH) * LT) < 30:
                v |= 2
            if 0 <= self.t - (fstart + LATCH * LT) < 30:
                v |= 4
            return v
        put, get = 0x100, 0x100
        for t_w, t_l, p in self.flips:
            if self.t >= t_w:
                put = p
                get = p if self.t >= t_l else p - 4
        if off == 0x690000 + 0x1000 * WINDOW:
            return put
        if off == 0x690004 + 0x1000 * WINDOW:
            return get
        return 0

    def now_us(self):
        return int(self.t)


def simulate(flips, frames=12):
    card = FakeCard(flips)
    regs = pp.regs_for(HEAD, WINDOW, False)
    events, rounds, span = pp.capture_loop(card.read, regs, pp.dpca(HEAD), frames * FRAME_US / 1e6,
                                           now_us=card.now_us)
    return events, rounds, span


def write_probe(path, events):
    with open(path, 'w') as f:
        f.write(f'# start 2026-10-03T12:00:00.000000Z\n# head {HEAD}\n# window {WINDOW}\n'
                f'# vtotal {VT}\n# frame_step_line 0\n# round_us 12.00\n')
        for t, fr, ln, name, old, new in events:
            f.write(f'{t}\t{fr}\t{ln}\t{name}\t{old:#x}\t{new:#x}\n')


FLIPS = [(1, 500), (2, 900), (3, 990), (5, 1000), (7, 1050), (9, 100)]


class Capture(unittest.TestCase):
    def test_changes_carry_the_line_of_the_moment(self):
        events, rounds, _ = simulate(FLIPS)
        self.assertGreater(rounds, 10000)
        puts = [(fr, ln) for t, fr, ln, name, old, new in events if name == 'win_put']
        self.assertEqual(len(puts), len(FLIPS))
        for (fr, ln), (efr, eln) in zip(FLIPS, puts):
            self.assertEqual(fr, efr)
            self.assertLessEqual(abs(ln - eln), 1)
        frames = [e for e in events if e[3] == 'frame']
        self.assertTrue(all(e[2] <= 1 for e in frames))

    def test_raster_and_head(self):
        card = FakeCard([])
        vt, step = pp.raster(card.read, pp.dpca(HEAD), seconds=0)
        self.assertEqual(vt, 1)   # zero-length sample: one read, no frame step
        card = FakeCard([])
        top, seen = 0, set()
        for _ in range(20000):
            v = card.read(pp.dpca(HEAD))
            top = max(top, v & 0xffff)
            seen.add(v >> 16)
        self.assertEqual(top + 1, VT)


class Readout(unittest.TestCase):
    def setUp(self):
        events, _, _ = simulate(FLIPS)
        self.dir = tempfile.TemporaryDirectory()
        self.path = os.path.join(self.dir.name, 'probe.tsv')
        write_probe(self.path, events)
        self.out = io.StringIO()
        self.r = pp.readout(self.path, out=self.out)
        self.text = self.out.getvalue()

    def tearDown(self):
        self.dir.cleanup()

    def test_latch_line_and_deadline_bracket(self):
        self.assertIn(self.r['latch_line'], (LATCH - 1, LATCH, LATCH + 1))  # one round is about one line
        self.assertTrue(80 <= max(self.r['held']) <= 83, self.r['held'])
        self.assertTrue(89 <= min(self.r['ok']) <= 92, self.r['ok'])
        self.assertEqual(len(self.r['ok']), 4)
        self.assertEqual(len(self.r['held']), 2)
        self.assertIn('latch deadline F:', self.text)

    def test_pending_get_stops_before_the_update(self):
        held = [f for f in self.r['flips'] if f.get('blanks') == 2]
        self.assertTrue(held)
        for f in held:
            self.assertEqual(f['pending'], [f['put'] - 4])

    def test_last_data_onset_and_isr_clear(self):
        line = [ln for ln in self.text.splitlines() if ln.startswith('LAST_DATA set at line')][0]
        med = int(line.split('median=')[1].split()[0])
        self.assertIn(med, (LATCH - FETCH, LATCH - FETCH + 1))
        clear = [ln for ln in self.text.splitlines() if ln.startswith('LAST_DATA cleared')][0]
        self.assertTrue(24 <= int(clear.split('median=')[1].split()[0]) <= 36, clear)


class Wrap(unittest.TestCase):
    def test_frame_counter_wraps(self):
        ev = [(0, 65535, 10, 'frame', 65534, 65535), (1, 0, 0, 'frame', 65535, 0), (2, 1, 0, 'frame', 0, 1)]
        self.assertEqual([e[1] for e in pp.unwrap(ev)], [65535, 65536, 65537])


class BorrowedFd(unittest.TestCase):
    """pidfd_getfd and region offsets on a child's sparse file (a descendant: no root needed)."""

    def test_open_device_reads_config_and_bar0(self):
        with tempfile.TemporaryDirectory() as d:
            f = os.path.join(d, 'fake-vfio-device')
            with open(f, 'wb') as h:
                h.seek(pp.CONFIG << 40)
                h.write(pp.TU106_ID.to_bytes(4, 'little'))
                h.seek(pp.dpca(0))
                h.write((0x0007045d).to_bytes(4, 'little'))
            child = subprocess.Popen([sys.executable, '-c',
                                      f'import time; h = open({f!r}, "rb"); time.sleep(30)'])
            try:
                for _ in range(100):
                    links = [os.readlink(f'/proc/{child.pid}/fd/{n}') for n in os.listdir(f'/proc/{child.pid}/fd')]
                    if f in links:
                        break
                    time.sleep(0.05)
                fd = pp.open_device(child.pid, is_candidate=lambda t: t == f)
                try:
                    self.assertEqual(pp.region_offset(fd, pp.BAR0), 0)
                    self.assertEqual(pp.rd32(fd, pp.region_offset(fd, pp.BAR0) + pp.dpca(0)), 0x0007045d)
                finally:
                    os.close(fd)
            finally:
                child.kill()
                child.wait()


if __name__ == '__main__':
    unittest.main()
