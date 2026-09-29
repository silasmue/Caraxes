#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""R4 raster and flip probe (docs/caraxes/experiments.md R4, measurements M1 and M2).

The Windows KMD on GSP-RM gets its scan line through an RPC (SYSTEM_GET_SCANLINE), which GSP
answers from inside the card: no BAR access shows it. This probe reads the display registers
itself while the guest runs, in a tight loop, and logs every change with the scan line and
frame count of that moment:
  RG_DPCA(h)             0x616330+0x800h  LINE_CNT 15:0, FRM_CNT 31:16 (the clock of every line)
  EVT_STAT_HEAD_TIMING(h) 0x611800+4h     LAST_DATA bit 1, VBLANK bit 2 (latched, W1C by the ISR)
  window w PUT / GET     0x690000+0x1000w, +4 (the channel's USERD, NVC57E_PUT/GET)
  --set full adds: window-immediate PUT/GET 0x690000+0x1000(32+w), core PUT/GET 0x680000/4,
                   RM_INTR_STAT_HEAD_TIMING(h) 0x611C00+4h
Reads only (4-byte preads), no writes. It borrows QEMU's own vfio device fd with pidfd_getfd
(so root: yama ptrace_scope=1), so the reads bypass QEMU: they are neither trapped nor traced
(pass B's QEMU log stays clean) and cost a few microseconds (QMP `xp` costs ~350 us).

  sudo python3 pt-probe.py capture [--pid VM_PID | --run RUN_DIR] [--seconds 30]
                                   [--set flip|full] [--head H] [--window W] -o PROBE.tsv
  python3 pt-probe.py readout PROBE.tsv [--flips 40]

PROBE.tsv: "# key value" header lines (start ISO time, vtotal, head, window, BOOT_0), then
"t_us  frame  line  reg  old  new" per change (t_us since the start; frame 16-bit as read).
readout prints: the LAST_DATA and VBLANK onset lines (M1b), the ISR's clear latency, per flip
the UPDATE's line, the GET values while it is pending and the line and blank it latched at
(M2a), and the latch deadline bracket from all flips (M1a).
"""
import argparse
import collections
import ctypes
import fcntl
import os
import statistics
import struct
import sys
import time

TU106_ID = 0x1f0210de            # config dword 0: device << 16 | vendor
VFIO_DEVICE_GET_REGION_INFO = 0x3B6C   # _IO(';', 100 + 8)
BAR0, CONFIG = 0, 7
SYS_PIDFD_GETFD = 438


def regs_for(head, window, full):
    r = [('evt_stat', 0x611800 + 4 * head),
         ('win_put', 0x690000 + 0x1000 * window), ('win_get', 0x690004 + 0x1000 * window)]
    if full:
        r += [('winim_put', 0x690000 + 0x1000 * (32 + window)),
              ('winim_get', 0x690004 + 0x1000 * (32 + window)),
              ('core_put', 0x680000), ('core_get', 0x680004),
              ('rm_intr_stat', 0x611C00 + 4 * head)]
    return r


def dpca(head):
    return 0x616330 + 0x800 * head


# -- capture ---------------------------------------------------------------------------------
def pidfd_getfd(pid, fd):
    libc = ctypes.CDLL(None, use_errno=True)
    pfd = os.pidfd_open(pid)
    try:
        r = libc.syscall(SYS_PIDFD_GETFD, pfd, fd, 0)
        if r < 0:
            e = ctypes.get_errno()
            raise OSError(e, f'pidfd_getfd({pid}, {fd}): {os.strerror(e)} (run as root)')
        return r
    finally:
        os.close(pfd)


def region_offset(fd, index):
    """The region's file offset (VFIO_DEVICE_GET_REGION_INFO; vfio-pci's index << 40 else)."""
    buf = bytearray(struct.pack('=IIIIQQ', 32, 0, index, 0, 0, 0))
    try:
        fcntl.ioctl(fd, VFIO_DEVICE_GET_REGION_INFO, buf)
    except OSError:
        return index << 40
    return struct.unpack('=IIIIQQ', buf)[5]


def rd32(fd, off):
    return int.from_bytes(os.pread(fd, 4, off), 'little')


def open_device(pid, want=TU106_ID, is_candidate=None):
    """Borrow the target's vfio device fd whose config dword 0 is `want`."""
    is_candidate = is_candidate or (lambda t: t == 'anon_inode:[vfio-device]'
                                    or t.startswith('/dev/vfio/devices/'))
    for n in sorted(os.listdir(f'/proc/{pid}/fd'), key=int):
        try:
            target = os.readlink(f'/proc/{pid}/fd/{n}')
        except OSError:
            continue
        if not is_candidate(target):
            continue
        fd = pidfd_getfd(pid, int(n))
        if rd32(fd, region_offset(fd, CONFIG)) == want:
            return fd
        os.close(fd)
    raise SystemExit(f'no vfio device fd with id {want:#010x} in pid {pid}')


def raster(read, off, seconds=0.05):
    """(vtotal, line at which FRM_CNT increments) from a short full-speed sample."""
    top, inc, prev = 0, collections.Counter(), None
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        v = read(off)
        top = max(top, v & 0xffff)
        if prev is not None and (v >> 16) != (prev >> 16):
            inc[v & 0xffff] += 1
        prev = v
    return top + 1, (inc.most_common(1)[0][0] if inc else -1)


def active_head(read, heads=4):
    a = [read(dpca(h)) for h in range(heads)]
    time.sleep(0.002)
    b = [read(dpca(h)) for h in range(heads)]
    moving = [h for h in range(heads) if a[h] != b[h]]
    return moving[0] if moving else 0


def capture_loop(read, regs, dpca_off, seconds, now_us=None):
    """Change events [(t_us, frame, line, name, old, new)] and the round count."""
    now_us = now_us or (lambda: time.monotonic_ns() // 1000)
    t_start = now_us()
    end = t_start + int(seconds * 1e6)
    prev = {name: read(off) for name, off in regs}
    d = read(dpca_off)
    prev_frame = d >> 16
    events, rounds = [], 0
    while True:
        d = read(dpca_off)
        vals = [(name, read(off)) for name, off in regs]
        t = now_us()
        rounds += 1
        frame, line = d >> 16, d & 0xffff
        if frame != prev_frame:
            events.append((t - t_start, frame, line, 'frame', prev_frame, frame))
            prev_frame = frame
        for name, v in vals:
            if v != prev[name]:
                events.append((t - t_start, frame, line, name, prev[name], v))
                prev[name] = v
        if t >= end:
            return events, rounds, t - t_start


def run_pid(run):
    """The VM's pid (QEMU or Cloud Hypervisor) of a pt-vm.sh run folder (default: $PT_DIR/current)."""
    if run is None:
        top = os.environ.get('PT_DIR', '/mnt/vmstore/caraxes/r4')
        with open(os.path.join(top, 'current')) as f:
            run = os.path.join(top, 'runs', f.read().strip())
    with open(os.path.join(run, 'vm.pid')) as f:
        return int(f.read())


def iso_now():
    t = time.time()
    return time.strftime('%Y-%m-%dT%H:%M:%S', time.gmtime(t)) + f'.{int(t % 1 * 1e6):06d}Z'


def capture(a):
    pid = a.pid if a.pid is not None else run_pid(a.run)
    fd = open_device(pid)
    base = region_offset(fd, BAR0)
    read = lambda off: rd32(fd, base + off)  # noqa: E731
    boot0 = read(0)
    head = a.head if a.head is not None else active_head(read)
    window = a.window if a.window is not None else 2 * head
    vtotal, step_line = raster(read, dpca(head))
    regs = regs_for(head, window, a.set == 'full')
    print(f'probe: pid {pid}, BOOT_0 {boot0:#010x}, head {head} (vtotal {vtotal}, frame count '
          f'steps at line {step_line}), window {window}, {len(regs) + 1} registers, {a.seconds} s',
          file=sys.stderr)
    start = iso_now()
    events, rounds, span = capture_loop(read, regs, dpca(head), a.seconds)
    os.close(fd)
    with open(a.out, 'w') as f:
        for k, v in (('start', start), ('pid', pid), ('boot0', f'{boot0:#010x}'), ('head', head),
                     ('window', window), ('vtotal', vtotal), ('frame_step_line', step_line),
                     ('regs', ' '.join(f'{n}={o:#x}' for n, o in [('dpca', dpca(head))] + regs)),
                     ('rounds', rounds), ('span_us', span),
                     ('round_us', f'{span / max(rounds, 1):.2f}')):
            f.write(f'# {k} {v}\n')
        for t, fr, ln, name, old, new in events:
            f.write(f'{t}\t{fr}\t{ln}\t{name}\t{old:#x}\t{new:#x}\n')
    print(f'probe: {rounds} rounds in {span} us ({span / max(rounds, 1):.2f} us each), '
          f'{len(events)} changes -> {a.out} (t=0 is {start})', file=sys.stderr)
    return 0


# -- readout ---------------------------------------------------------------------------------
def load(path):
    meta, ev = {}, []
    with open(path) as f:
        for line in f:
            if line.startswith('#'):
                k, _, v = line[1:].strip().partition(' ')
                meta[k] = v
            elif line.strip():
                t, fr, ln, name, old, new = line.split('\t')
                ev.append((int(t), int(fr), int(ln), name, int(old, 16), int(new, 16)))
    return meta, ev


def unwrap(ev):
    """16-bit frame counts -> monotonic ones."""
    out, base, last = [], 0, None
    for t, fr, ln, name, old, new in ev:
        if last is not None and fr < last - 32768:
            base += 65536
        last = fr
        out.append((t, fr + base, ln, name, old, new))
    return out


def stats(xs):
    if not xs:
        return 'none'
    xs = sorted(xs)
    q = lambda p: xs[min(len(xs) - 1, int(p * len(xs)))]  # noqa: E731
    return f'n={len(xs)} min={xs[0]} p10={q(0.1)} median={q(0.5)} p90={q(0.9)} max={xs[-1]}'


def readout(path, max_flips=40, out=sys.stdout):
    meta, ev = load(path)
    ev = unwrap(ev)
    if not ev:
        out.write(f'probe {path}: no changes recorded\n')
        return dict(vtotal=0, latch_line=None, ok=[], held=[], flips=[])
    vt = int(meta.get('vtotal', '0')) or (max(e[2] for e in ev) + 1)
    p = lambda fr, ln: fr * vt + ln  # noqa: E731
    w = out.write
    w(f"probe {path}: start {meta.get('start')}, head {meta.get('head')}, window {meta.get('window')}, "
      f"vtotal {vt}, frame count steps at line {meta.get('frame_step_line')}, "
      f"{meta.get('round_us')} us per round\n")
    # M1b: event onsets and the ISR's clear
    for bit, label in ((1, 'LAST_DATA'), (2, 'VBLANK')):
        on, clear, t_set = [], [], None
        for t, fr, ln, name, old, new in ev:
            if name != 'evt_stat':
                continue
            if not old >> bit & 1 and new >> bit & 1:
                on.append(ln)
                t_set = t
            elif old >> bit & 1 and not new >> bit & 1 and t_set is not None:
                clear.append(t - t_set)
                t_set = None
        w(f'{label} set at line: {stats(on)}\n')
        w(f'{label} cleared after (us, the ISR): {stats(clear)}\n')
    # M2a and M1a: every window PUT change, followed to GET == PUT (or to the next PUT change)
    gets = [(t, fr, ln, new) for t, fr, ln, name, old, new in ev if name == 'win_get']
    puts = [(t, fr, ln, new) for t, fr, ln, name, old, new in ev if name == 'win_put']
    flips = []
    slack = 2 * float(meta.get('round_us', '0') or 0)   # the reads of one round are not atomic
    for i, (t, fr, ln, put) in enumerate(puts):
        t_next = puts[i + 1][0] if i + 1 < len(puts) else None
        seen, latch = [], None
        for gt, gfr, gln, gv in gets:
            if gt < t - slack:
                continue
            if t_next is not None and gt > t_next:
                break
            if gv == put:
                latch = (gt, gfr, gln)
                break
            if gv not in seen:
                seen.append(gv)
        flips.append(dict(t=t, fr=fr, ln=ln, put=put, pending=seen, latch=latch,
                          superseded=latch is None and t_next is not None))
    waited = [f for f in flips if f['latch'] and f['latch'][0] - f['t'] > 100]
    lm = collections.Counter(f['latch'][2] for f in waited).most_common(1)
    lm = lm[0][0] if lm else None
    w(f'window PUT changes: {len(flips)}; GET reached PUT after >100 us: {len(waited)}; '
      f"superseded by the next PUT: {sum(f['superseded'] for f in flips)}; "
      f'latch line (mode of the waited ones): {lm}\n')
    ok, held = [], []
    if lm is not None:
        for f in flips:
            if not f['latch']:
                continue
            pp, pl = p(f['fr'], f['ln']), p(f['latch'][1], f['latch'][2])
            f['blanks'] = (pl - lm) // vt - (pp - lm) // vt
            f['d'] = (lm - f['ln']) % vt or vt
            if f['latch'][0] - f['t'] > 100:
                if f['blanks'] == 1:
                    ok.append(f['d'])
                elif f['blanks'] >= 2:
                    held.append(f['d'])
    w(f"UPDATE lines before the latch line, latched at the coming blank: {stats(ok)}\n")
    w(f"UPDATE lines before the latch line, held to a later blank: {stats(held)}\n")
    if ok and held:
        w(f'latch deadline F: an UPDATE {min(ok)} lines before the latch still made it, '
          f'{max(held)} lines before did not -> F in ({max(held)}, {min(ok)}]'
          + (' (overlap: not a sharp line)' if max(held) >= min(ok) else '') + '\n')
    w('flips (t_us, frame:line of the PUT, lines to the latch line, GET values while pending, '
      'latched at frame:line after us, blanks crossed):\n')
    for f in flips[:max_flips]:
        la = f['latch']
        w(f"  {f['t']:>10} {f['fr']}:{f['ln']:<5} d={f.get('d', '-'):<5} pending GET "
          f"{','.join(hex(g) for g in f['pending']) or '-':<24} "
          + (f"-> {la[1]}:{la[2]} after {la[0] - f['t']} us, blanks {f.get('blanks', '-')}" if la
             else '-> superseded' if f['superseded'] else '-> never')
          + '\n')
    return dict(vtotal=vt, latch_line=lm, ok=ok, held=held, flips=flips)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    sub = ap.add_subparsers(dest='cmd', required=True)
    c = sub.add_parser('capture')
    c.add_argument('--pid', type=int)
    c.add_argument('--run', help='pt-vm.sh run folder (default: $PT_DIR/current)')
    c.add_argument('--seconds', type=float, default=30)
    c.add_argument('--set', choices=('flip', 'full'), default='flip')
    c.add_argument('--head', type=int)
    c.add_argument('--window', type=int)
    c.add_argument('-o', '--out', required=True)
    r = sub.add_parser('readout')
    r.add_argument('probe')
    r.add_argument('--flips', type=int, default=40)
    a = ap.parse_args(argv)
    if a.cmd == 'capture':
        return capture(a)
    readout(a.probe, a.flips)
    return 0


if __name__ == '__main__':
    sys.exit(main())
