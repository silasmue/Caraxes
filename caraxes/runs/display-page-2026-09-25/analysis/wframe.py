#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Per frame of a compositor window: previous latch lateness, repaint start (first GPPut after
the latch), GPU+wait (last GPPut to kick), and whether the next tick was missed.
Usage: wframe.py <device-trace.txt.xz> (docs/caraxes/intr-page.md §11.4).
"""
import bisect, lzma, re, statistics, sys
L = re.compile(r"display: window (\d+) latch \d+ at tick (\d+): surface (\S+)")
lat, kicks, gpput, msis = [], [], [], []
for ln in lzma.open(sys.argv[1], "rt", errors="replace"):
    p = ln.split(None, 6)
    if len(p) < 3 or not p[0].isdigit():
        continue
    ts = int(p[0])
    if p[1] == "E":
        m = L.search(ln)
        if m and m.group(3) != "none" and m.group(1) == "6":
            lat.append((ts, int(m.group(2))))
        elif "doorbell: guest" in ln:
            gpput.append(ts)
        elif "host notifier" in ln:
            msis.append(ts)
    elif p[1] == "W" and p[2] == "bar0" and p[3] == "0x00696000":
        kicks.append(ts)
run = [lat[-1]]
for a in reversed(lat[:-1]):
    if a[1] < run[-1][1]:
        run.append(a)
    else:
        break
run.reverse()
per = 1e6 / 60
base = min(ts - t * per for ts, t in run)
rows = []
for (ts, t), (ts2, t2) in zip(run, run[1:]):
    late = ts - (base + t * per)
    # work of the frame latched at t2: doorbells between ts and the kick before ts2
    k = kicks[bisect.bisect_left(kicks, ts2) - 1]
    if k < ts:
        continue
    d = [g for g in gpput[bisect.bisect_left(gpput, ts):bisect.bisect_left(gpput, k)]]
    if not d:
        continue
    n_msi = len(msis[bisect.bisect_left(msis, ts):bisect.bisect_left(msis, k)])
    rows.append(dict(late=late / 1000, start=(d[0] - (base + t * per)) / 1000, gpu=(k - d[-1]) / 1000,
                     kick=(k - (base + t * per)) / 1000, miss=t2 - t > 1, msi=n_msi))
for key in ("late", "start", "gpu", "kick", "msi"):
    ok = [r[key] for r in rows if not r["miss"]]
    bad = [r[key] for r in rows if r["miss"]]
    q = lambda v, f: sorted(v)[int(len(v) * f)] if v else float("nan")
    print(f"{key:6} on time: p50 {q(ok, .5):7.2f} p90 {q(ok, .9):7.2f} | before a missed tick: n {len(bad)} p50 {q(bad, .5):7.2f} p90 {q(bad, .9):7.2f}")
