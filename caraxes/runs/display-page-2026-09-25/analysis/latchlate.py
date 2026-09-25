#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Per compositor window: how late each latch of window 6 ran against the tick grid, and
how long before the next tick the display-channel kick came (on-time and skipped frames).
Usage: latchlate.py <device-trace.txt.xz>... (docs/caraxes/intr-page.md §11.4)."""
import lzma, re, sys, statistics
L = re.compile(r"display: window (\d+) latch \d+ at tick (\d+)(?: \(immediate\))?: surface (\S+)")
for path in sys.argv[1:]:
    pts = []
    kicks = []
    gpput = []
    for ln in lzma.open(path, "rt", errors="replace"):
        p = ln.split(None, 6)
        if len(p) < 3 or not p[0].isdigit(): continue
        ts = int(p[0])
        if p[1] == "E":
            m = L.search(ln)
            if m and m.group(3) != "none" and m.group(1) == "6":
                pts.append((ts, int(m.group(2))))
        elif p[1] == "W" and p[2] == "bar0" and p[3] == "0x00696000":
            kicks.append(ts)
        elif p[1] == "W" and p[2] == "bar1" and p[3].endswith("8c"):
            gpput.append(ts)
    # tick period 16666.667 us; fit offset on the minimum lateness among consecutive ticks
    # ticks restart at modesets; use last contiguous run
    run = [pts[-1]]
    for a in reversed(pts[:-1]):
        if a[1] < run[-1][1]:
            run.append(a)
        else:
            break
    run.reverse()
    per = 1e6 / 60.0
    base = min(ts - t * per for ts, t in run)
    late = [ts - t * per - base for ts, t in run]
    q = sorted(late)
    print(path.split('/')[-3], path.split('/')[-2], "latches", len(run), "late us p50 %.0f p90 %.0f p99 %.0f max %.0f" % (q[len(q)//2], q[int(len(q)*.9)], q[int(len(q)*.99)], q[-1]),
          "over 1 ms:", sum(1 for x in late if x > 1000))
    # kick -> nominal next vblank margin
    import bisect
    margins = []
    misses = 0
    for (ts, t), (ts2, t2) in zip(run, run[1:]):
        # the kick for the frame latched at t2: last kick before ts2
        i = bisect.bisect_left(kicks, ts2) - 1
        if i < 0: continue
        k = kicks[i]
        # nominal vblank times
        nv = base + t2 * per
        margins.append((t2 - t, (base + (t + 1) * per) - k))
    skipped = [m for d, m in margins if d > 1]
    ok = [m for d, m in margins if d == 1]
    print("   frames", len(margins), "skipped ticks", len(skipped), "kick before the next tick (us): on-time p10 %.0f p50 %.0f; skipped p50 %.0f" % (sorted(ok)[len(ok)//10], statistics.median(ok), statistics.median(skipped) if skipped else float('nan')))
