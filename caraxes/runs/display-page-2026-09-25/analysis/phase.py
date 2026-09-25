#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# docs/caraxes/intr-page.md §11.7 (tab.py, run from caraxes/runs/) and §11.8 (the others,
# with device-trace.txt.xz files as arguments).
import bisect, lzma, sys
# Per display kick: last doorbell before it -> first flush read of the PTE batch before the kick
# (the wait for the rendering and the compositor's flip decision), with the host notifier MSIs in
# between; and the batch itself (first flush read -> kick).
q = lambda v, f: sorted(v)[int(len(v) * f)] if v else float("nan")
for path in sys.argv[1:]:
    db, fl, kicks, ev = [], [], [], []
    for ln in lzma.open(path, "rt", errors="replace"):
        p = ln.split(None, 6)
        if len(p) < 4 or not p[0].isdigit():
            continue
        ts = int(p[0])
        if p[1] == "E":
            if "doorbell: guest" in ln:
                db.append(ts)
            elif "host notifier" in ln and "MSI [host]" in ln:
                ev.append(ts)
        elif p[2] == "bar3" and p[3] == "0x00fea000":
            fl.append(ts)
        elif p[2] == "bar0" and p[3] == "0x00696000" and p[1] == "W":
            kicks.append(ts)
    wait, batch, gap = [], [], []
    for k in kicks:
        i = bisect.bisect_left(db, k) - 1
        if i < 0:
            continue
        d = db[i]
        j = bisect.bisect_left(fl, d)
        if j >= len(fl) or fl[j] > k:
            continue
        wait.append((fl[j] - d) / 1000)
        batch.append((k - fl[j]) / 1000)
        # the last host notifier MSI before the batch: the render-done event
        e = bisect.bisect_left(ev, fl[j]) - 1
        if e >= 0 and ev[e] > d:
            gap.append((fl[j] - ev[e]) / 1000)
    f = lambda v: f"n {len(v)} p50 {q(v,.5):6.2f} p90 {q(v,.9):6.2f}"
    print(path.split("/")[0], "| doorbell->batch", f(wait), "| last MSI->batch", f(gap), "| batch->kick", f(batch))
