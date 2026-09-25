#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""After each latch of window 6: when the guest's first interrupt service ended (the MSI
re-arm write, trapped NV_PCFG 0x88704 or its eventfd), and when the compositor's first
doorbell came.
Usage: isr.py <device-trace.txt.xz> (docs/caraxes/intr-page.md §11.4).
"""
import bisect, lzma, re, sys
L = re.compile(r"display: window (\d+) latch \d+ at tick (\d+): surface (\S+)")
lat, rearm, db = [], [], []
for ln in lzma.open(sys.argv[1], "rt", errors="replace"):
    p = ln.split(None, 6)
    if len(p) < 3 or not p[0].isdigit():
        continue
    ts = int(p[0])
    if p[1] == "E":
        m = L.search(ln)
        if m and m.group(3) != "none" and m.group(1) == "6":
            lat.append(ts)
        elif "msi re-arm" in ln:
            rearm.append(ts)
        elif "doorbell: guest" in ln:
            db.append(ts)
    elif p[1] == "W" and p[3] == "0x00088704":
        rearm.append(ts)
lat = lat[-700:]
d1, d2, n = [], [], []
for a, b in zip(lat, lat[1:]):
    i = bisect.bisect_left(rearm, a)
    if i < len(rearm) and rearm[i] < b:
        d1.append((rearm[i] - a) / 1000)
    j = bisect.bisect_left(db, a)
    if j < len(db) and db[j] < b:
        d2.append((db[j] - a) / 1000)
    n.append(bisect.bisect_left(rearm, b) - i)
q = lambda v, f: sorted(v)[int(len(v) * f)] if v else float("nan")
print(f"first ISR end after the latch (ms): p50 {q(d1,.5):.3f} p90 {q(d1,.9):.3f} p99 {q(d1,.99):.3f}; ISRs per frame p50 {q(n,.5)}; first doorbell after the latch p50 {q(d2,.5):.2f} p90 {q(d2,.9):.2f}")
