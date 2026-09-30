#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""QEMU vfio trace -> the device's gpu-trace.log format (docs/caraxes/experiments.md R4).

Turns the log of a real-card run (QEMU 11.1, log trace backend, `-msg timestamp=on`,
`-D <file>`, R4 pass B) into the line format of crates/vfio-user-nvgpu/src/trace.rs, so a
real-card capture and a Caraxes run can be diffed and read with the same tools
(CH-build/dispaccess.py, caraxes/tools/display-pace.py).

  qemutrace2gpu.py LOG [-o OUT] [--device 0000:2d:00.0|all] [--names GPU_TRACE_OR_TABLE]
                   [--preset flip|display|ptimer|usermode|intr|pmc|cfgmirror|prom ...]
                   [--range LO-HI ...] [--region bar0 ...] [--t0 first|ISO]
                   [--from US] [--to US] [--markers FILE] [--no-events] [--no-summary]

Input (hw/vfio/trace-events of QEMU 11.1.1; the prefix "2026-10-03T12:00:00.123456Z " comes
from -msg timestamp=on, the older "PID@SEC.USEC:" prefix is accepted too; .gz/.xz/.zst read):
  vfio_region_read  (0000:2d:00.0:region0+0x9410, 4) = 0x1b
  vfio_region_write  (0000:2d:00.0:region0+0x680000, 0x10, 4)
  vfio_pci_read_config  (0000:2d:00.0, @0x4, len=0x2) 0x7
  vfio_pci_write_config  (0000:2d:00.0, @0x704, 0x0, len=0x4)
  vfio_quirk_generic_mirror_read 0000:2d:00.0 vfio-nvidia-bar0-88000-mirror-quirk 0x0: 0x1f0210de
  vfio_quirk_generic_mirror_write 0000:2d:00.0 vfio-nvidia-bar0-88000-mirror-quirk 0x704: 0x0
  vfio_rom_read  (0000:2d:00.0, 0x0, 0x4) = 0xaa55
  vfio_msi_interrupt  (0000:2d:00.0) vector 0 0xfee00000/0x4021
  any other vfio_* event naming the device -> "E qemu <event> <args>"

Output: "{t:>12} R|W {region:<4} {offset:#010x} {size:<2} {value:<12} {name}" and
"{t:>12} E <text>", t in microseconds since --t0 (default: the device's first line in the
log, before any filter, so several extracts of one log share one time base). Region names are
the VFIO region indices (bar0..bar5, rom, cfg, vga), as in the device's trace.

A guest access to QEMU's NVIDIA config mirror (BAR0 0x88000-0x88fff) is traced by QEMU as a
discarded hardware read, a config-space access and the mirror event; it becomes one BAR0 line
with the value the guest saw, "(cfg mirror, hw=0x..)" in the name column.
A summary (counts per region, MSIs per second, time span) goes to stderr.
"""
import argparse
import calendar
import collections
import gzip
import io
import lzma
import re
import subprocess
import sys
import time

REGIONS = ['bar0', 'bar1', 'bar2', 'bar3', 'bar4', 'bar5', 'rom', 'cfg', 'vga']
PRESETS = {
    'display': [(0x610000, 0x6fffff)],
    'ptimer': [(0x9000, 0x9fff)],
    'usermode': [(0x810000, 0x81ffff)],
    'intr': [(0xb80000, 0xbbffff)],
    'pmc': [(0x0, 0xfff)],
    'cfgmirror': [(0x88000, 0x88fff)],
    'prom': [(0x300000, 0x3fffff)],
}
PRESETS['flip'] = (PRESETS['display'] + PRESETS['ptimer'] + PRESETS['usermode']
                   + PRESETS['intr'] + PRESETS['cfgmirror'])
MIRRORS = {'88000': 0x88000, '1800': 0x1800}

TS_ISO = re.compile(r'(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d)(?:\.(\d{1,9}))?Z\s+')
TS_OLD = re.compile(r'\d+@(\d+)\.(\d{6}):')
EVENT = re.compile(r'(vfio_[a-z0-9_]+)\s*(.*)$')
HEX = r'0x([0-9a-fA-F]+)'
P_REGION_R = re.compile(r'\((.+?):region(\d+)\+' + HEX + r', (\d+)\) = ' + HEX)
P_REGION_W = re.compile(r'\((.+?):region(\d+)\+' + HEX + r', ' + HEX + r', (\d+)\)')
P_CFG_R = re.compile(r'\(([^,]+), @' + HEX + r', len=' + HEX + r'\) ' + HEX)
P_CFG_W = re.compile(r'\(([^,]+), @' + HEX + r', ' + HEX + r', len=' + HEX + r'\)')
P_MIRROR = re.compile(r'(\S+) (\S+) ' + HEX + r': ' + HEX)
P_ROM_R = re.compile(r'\(([^,]+), ' + HEX + r', ' + HEX + r'\) = ' + HEX)
P_MSI = re.compile(r'\(([^)]+)\) vector (\d+) ' + HEX + r'/' + HEX)
P_GPU_LINE = re.compile(r'\s*\d+ ([RW]) (\S+)\s+0x([0-9a-fA-F]+) +\d+ +\S+ *(.*)$')
P_TABLE = re.compile(r'\s*0x([0-9a-fA-F]+)\s+(\S.*?)\s*$')


def fmt_access(t, d, region, offset, size, value, note=''):
    """One access line, byte for byte as trace.rs `Trace::access` writes it."""
    return f"{t:>12} {d} {region:<4} {offset:#010x} {size:<2} {f'{value:#x}':<12} {note}".rstrip()


def fmt_event(t, text):
    return f"{t:>12} E {text}"


class Clock:
    """ISO8601 / PID@SEC.USEC prefixes -> integer microseconds since the epoch."""

    def __init__(self):
        self._sec_cache = (None, 0)

    def split(self, line):
        """(epoch_us or None, rest of the line)."""
        m = TS_ISO.match(line)
        if m:
            day, frac = m.group(1), m.group(2) or ''
            if self._sec_cache[0] != day:
                self._sec_cache = (day, calendar.timegm(time.strptime(day, '%Y-%m-%dT%H:%M:%S')))
            us = int((frac + '000000')[:6])
            return self._sec_cache[1] * 1_000_000 + us, line[m.end():]
        m = TS_OLD.match(line)
        if m:
            return int(m.group(1)) * 1_000_000 + int(m.group(2)), line[m.end():]
        return None, line


def parse_iso(s):
    t, rest = Clock().split(s.strip() + ' ')
    if t is None:
        raise ValueError(f'not an ISO time like 2026-10-03T12:00:00.123456Z: {s!r}')
    return t


def load_names(path):
    """(region, offset) -> name, from a device gpu-trace.log or an "0xOFFSET NAME" table."""
    names = {}
    with open_any(path) as f:
        for line in f:
            m = P_GPU_LINE.match(line)
            if m:
                if m.group(4).strip():
                    names.setdefault((m.group(2), int(m.group(3), 16)), m.group(4).strip())
                continue
            m = P_TABLE.match(line)
            if m:
                names.setdefault(('bar0', int(m.group(1), 16)), m.group(2))
    return names


def open_any(path):
    if path == '-':
        return io.TextIOWrapper(sys.stdin.buffer, errors='replace')
    if path.endswith('.gz'):
        return gzip.open(path, 'rt', errors='replace')
    if path.endswith('.xz'):
        return lzma.open(path, 'rt', errors='replace')
    if path.endswith('.zst'):
        p = subprocess.Popen(['zstd', '-dcq', path], stdout=subprocess.PIPE)
        return io.TextIOWrapper(p.stdout, errors='replace')
    return open(path, errors='replace')


class Converter:
    def __init__(self, device='0000:2d:00.0', names=None, ranges=None, regions=None, t0=None,
                 t_from=None, t_to=None, markers=(), events=True):
        self.device = device
        self.names = names or {}
        self.ranges = ranges or []
        self.regions = set(regions) if regions else None
        self.t0 = t0
        self.t_from, self.t_to = t_from, t_to
        self.markers = collections.deque(sorted(markers))
        self.events = events
        self.clock = Clock()
        self.pending = collections.deque()
        self.stamped = None
        self.lineno = 0
        self.stats = collections.Counter()
        self.msi_times = []
        self.first_t = self.last_t = None

    # -- input -----------------------------------------------------------------------------
    def feed(self, line):
        """Parse one QEMU log line; returns the output lines that became final."""
        self.lineno += 1
        self.stats['lines'] += 1
        t, rest = self.clock.split(line)
        if self.stamped is None:
            self.stamped = t is not None
        if t is None:
            t = self.lineno
        m = EVENT.match(rest)
        if not m:
            return []
        ev, args = m.group(1), m.group(2).rstrip('\n')
        rec = self._parse(t, ev, args)
        if rec is None:
            return []
        if self.t0 is None:
            self.t0 = t
        self.stats['device'] += 1
        out = []
        self.pending.append(rec)
        while len(self.pending) > 8:
            out.extend(self._emit(self.pending.popleft()))
        return out

    def finish(self):
        out = []
        while self.pending:
            out.extend(self._emit(self.pending.popleft()))
        while self.markers:
            t, text = self.markers.popleft()
            out.extend(self._keep_event(t, f'mark {text}'))
        return out

    def _mine(self, dev):
        return self.device == 'all' or dev == self.device

    def _parse(self, t, ev, args):
        """A record dict, or None (not this device's, or not parsed)."""
        if ev == 'vfio_region_read' or ev == 'vfio_region_write':
            m = (P_REGION_R if ev == 'vfio_region_read' else P_REGION_W).match(args)
            if not m:
                self.stats['unparsed'] += 1
                return None
            if not self._mine(m.group(1)):
                return None
            idx, off = int(m.group(2)), int(m.group(3), 16)
            if ev == 'vfio_region_read':
                size, val = int(m.group(4)), int(m.group(5), 16)
            else:
                val, size = int(m.group(4), 16), int(m.group(5))
            region = REGIONS[idx] if idx < len(REGIONS) else f'r{idx}'
            return dict(t=t, d='R' if ev == 'vfio_region_read' else 'W', region=region, off=off,
                        size=size, val=val, note=None)
        if ev in ('vfio_pci_read_config', 'vfio_pci_write_config'):
            m = (P_CFG_R if ev == 'vfio_pci_read_config' else P_CFG_W).match(args)
            if not m:
                self.stats['unparsed'] += 1
                return None
            if not self._mine(m.group(1)):
                return None
            off = int(m.group(2), 16)
            if ev == 'vfio_pci_read_config':
                size, val = int(m.group(3), 16), int(m.group(4), 16)
            else:
                val, size = int(m.group(3), 16), int(m.group(4), 16)
            return dict(t=t, d='R' if ev == 'vfio_pci_read_config' else 'W', region='cfg', off=off,
                        size=size, val=val, note=None)
        if ev in ('vfio_quirk_generic_mirror_read', 'vfio_quirk_generic_mirror_write'):
            m = P_MIRROR.match(args)
            if not m:
                self.stats['unparsed'] += 1
                return None
            if not self._mine(m.group(1)):
                return None
            base = next((b for k, b in MIRRORS.items() if f'-{k}-' in m.group(2)), None)
            if base is None:
                return dict(t=t, event=f'qemu {ev} {args}')
            return self._merge_mirror(t, ev.endswith('_read'), base, int(m.group(3), 16),
                                      int(m.group(4), 16))
        if ev == 'vfio_rom_read':
            m = P_ROM_R.match(args)
            if not m or not self._mine(m.group(1)):
                return None
            return dict(t=t, d='R', region='rom', off=int(m.group(2), 16), size=int(m.group(3), 16),
                        val=int(m.group(4), 16), note=None)
        if ev == 'vfio_msi_interrupt':
            m = P_MSI.match(args)
            if not m or not self._mine(m.group(1)):
                return None
            return dict(t=t, event=f'msi vector {m.group(2)} addr 0x{m.group(3)} data 0x{m.group(4)}',
                        msi=True)
        if self.device == 'all' or self.device in args:
            return dict(t=t, event=f'qemu {ev} {args}'.rstrip())
        return None

    def _merge_mirror(self, t, is_read, base, cfg_off, val):
        """Fold QEMU's discarded hardware read and its config access into one BAR0 line."""
        size, hw = 4, None
        for rec in reversed(self.pending):
            if rec.get('region') == 'cfg' and rec['off'] == cfg_off and rec['d'] == ('R' if is_read else 'W'):
                size = rec['size']
                self.pending.remove(rec)
                break
        if is_read:
            for rec in reversed(self.pending):
                if rec.get('region') == 'bar0' and rec['d'] == 'R' and rec['off'] == base + cfg_off:
                    size, hw = rec['size'], rec['val']
                    self.pending.remove(rec)
                    break
        note = 'cfg mirror' + (f', hw={hw:#x}' if hw is not None else '')
        return dict(t=t, d='R' if is_read else 'W', region='bar0', off=base + cfg_off, size=size,
                    val=val, note=note)

    # -- output ----------------------------------------------------------------------------
    def _rel(self, t):
        return t - self.t0 if self.stamped else t

    def _in_window(self, rel):
        return (self.t_from is None or rel >= self.t_from) and (self.t_to is None or rel <= self.t_to)

    def _keep_event(self, t, text):
        rel = self._rel(t)
        if not self.events or not self._in_window(rel):
            return []
        self.stats['events'] += 1
        return [fmt_event(rel, text)]

    def _emit(self, rec):
        out = []
        while self.markers and self.markers[0][0] <= rec['t']:
            mt, text = self.markers.popleft()
            out.extend(self._keep_event(mt, f'mark {text}'))
        rel = self._rel(rec['t'])
        if 'event' in rec:
            if rec.get('msi'):
                self.stats['msi'] += 1
                if self._in_window(rel):
                    self.msi_times.append(rel)
            out.extend(self._keep_event(rec['t'], rec['event']))
            return out
        self.stats[f"{rec['d']} {rec['region']}"] += 1
        if not self._in_window(rel):
            return out
        if self.regions is not None and rec['region'] not in self.regions:
            return out
        if self.ranges and rec['region'] == 'bar0' and not any(lo <= rec['off'] <= hi for lo, hi in self.ranges):
            return out
        if self.ranges and self.regions is None and rec['region'] != 'bar0':
            return out
        name = self.names.get((rec['region'], rec['off']), '')
        if rec['note']:
            name = f"{name} ({rec['note']})" if name else f"({rec['note']})"
        self.stats['kept'] += 1
        self.first_t = rel if self.first_t is None else self.first_t
        self.last_t = rel
        out.append(fmt_access(rel, rec['d'], rec['region'], rec['off'], rec['size'], rec['val'], name))
        return out

    def summary(self):
        s = self.stats
        lines = [f"qemutrace2gpu: {s['lines']} log lines, {s['device']} device records, "
                 f"{s['kept']} accesses and {s['events']} events written, {s['unparsed']} unparsed"]
        if not self.stamped:
            lines.append('  WARNING: no timestamps in the log (QEMU without -msg timestamp=on); '
                         't is the line number')
        per = sorted((k, v) for k, v in s.items() if k[:2] in ('R ', 'W '))
        if per:
            lines.append('  all accesses: ' + ', '.join(f'{k} {v}' for k, v in per))
        if self.first_t is not None:
            lines.append(f'  kept span: {self.first_t} .. {self.last_t} us')
        if self.msi_times:
            span = max(self.msi_times[-1] - self.msi_times[0], 1)
            rate = (len(self.msi_times) - 1) * 1e6 / span if len(self.msi_times) > 1 else 0.0
            lines.append(f'  MSIs: {len(self.msi_times)} in the window, {rate:.1f}/s')
        return '\n'.join(lines)


def parse_range(s):
    lo, hi = s.split('-', 1)
    return int(lo, 16), int(hi, 16)


def load_markers(path):
    out = []
    with open(path) as f:
        lines = f.readlines()
    for line in lines:
        if line.strip():
            stamp, _, text = line.strip().partition(' ')
            out.append((parse_iso(stamp), text))
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('log', help='QEMU -D log (or -, .gz, .xz, .zst)')
    ap.add_argument('-o', '--out', help='output file (default stdout)')
    ap.add_argument('--device', default='0000:2d:00.0', help="VFIO device name or 'all'")
    ap.add_argument('--names', help='gpu-trace.log of a Caraxes run, or an "0xOFFSET NAME" table')
    ap.add_argument('--preset', action='append', default=[], choices=sorted(PRESETS))
    ap.add_argument('--range', action='append', default=[], type=parse_range, help='BAR0 LO-HI, hex')
    ap.add_argument('--region', action='append', choices=REGIONS)
    ap.add_argument('--t0', default='first', help="'first' or an ISO time (2026-10-03T12:00:00.123456Z)")
    ap.add_argument('--from', dest='t_from', type=int, help='first microsecond kept (after --t0)')
    ap.add_argument('--to', dest='t_to', type=int, help='last microsecond kept')
    ap.add_argument('--markers', help='pt-vm.sh markers.txt ("ISO text" per line)')
    ap.add_argument('--no-events', action='store_true')
    ap.add_argument('--no-summary', action='store_true')
    a = ap.parse_args(argv)
    ranges = list(a.range)
    for p in a.preset:
        ranges.extend(PRESETS[p])
    conv = Converter(device=a.device, names=load_names(a.names) if a.names else None, ranges=ranges,
                     regions=a.region, t0=None if a.t0 == 'first' else parse_iso(a.t0),
                     t_from=a.t_from, t_to=a.t_to,
                     markers=load_markers(a.markers) if a.markers else (), events=not a.no_events)
    out = open(a.out, 'w') if a.out else sys.stdout
    with open_any(a.log) as f:
        for line in f:
            for o in conv.feed(line):
                out.write(o + '\n')
    for o in conv.finish():
        out.write(o + '\n')
    if a.out:
        out.close()
    if not a.no_summary:
        print(conv.summary(), file=sys.stderr)
    return 0


if __name__ == '__main__':
    sys.exit(main())
