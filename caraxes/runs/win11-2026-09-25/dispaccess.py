#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Summarise the display block's BAR0 accesses in a device trace taken with
CARAXES_TRACE_ACCESSES=bar0:<lo>-<hi>: per offset the reads and writes, the values seen,
and the register name from the 615 dev_disp.h headers (v03_00, v04_01) where one matches.
Usage: dispaccess.py <gpu-trace.log> [from_us] [to_us]"""
import collections, re, sys

H = '/mnt/vmstore/caraxes/ogkm-615.71.09/src/common/inc/swref/published/disp/'
names = {}
for v in ('v03_00', 'v04_01', 'v04_00'):
    try:
        for line in open(H + v + '/dev_disp.h', errors='replace'):
            m = re.match(r'#define (NV_[A-Z0-9_]+)(\(i\))?\s+\((0x[0-9A-Fa-f]+)(\+\(i\)\*(\d+|0x[0-9A-Fa-f]+))?\)', line)
            m2 = re.match(r'#define (NV_[A-Z0-9_]+)\s+(0x00[0-9A-Fa-f]{6})\s', line)
            if m:
                base = int(m.group(3), 16)
                step = int(m.group(5), 0) if m.group(5) else 0
                for i in range(8 if step else 1):
                    names.setdefault(base + i * step, m.group(1) + (f'({i})' if step else ''))
            elif m2:
                names.setdefault(int(m2.group(2), 16), m2.group(1))
    except FileNotFoundError:
        pass

path = sys.argv[1]
t0 = int(sys.argv[2]) if len(sys.argv) > 2 else 0
t1 = int(sys.argv[3]) if len(sys.argv) > 3 else 1 << 62
acc = collections.defaultdict(lambda: [0, 0, collections.Counter(), collections.Counter()])
for line in open(path, errors='replace'):
    f = line.split()
    if len(f) < 6 or f[2] != 'bar0' or f[1] not in ('R', 'W'):
        continue
    t = int(f[0])
    if not t0 <= t < t1:
        continue
    off = int(f[3], 16)
    a = acc[off]
    if f[1] == 'R':
        a[0] += 1
        a[2][f[5]] += 1
    else:
        a[1] += 1
        a[3][f[5]] += 1
for off in sorted(acc):
    r, w, rv, wv = acc[off]
    top = lambda c: ' '.join(f'{v}x{n}' for v, n in c.most_common(4))
    print(f'{off:#08x} {names.get(off, ""):<48} R{r:<7} W{w:<6} reads[{top(rv)}] writes[{top(wv)}]')
