#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# docs/caraxes/intr-page.md §11.7 (tab.py, run from caraxes/runs/) and §11.8 (the others,
# with device-trace.txt.xz files as arguments).
import lzma, sys
# time from each "invalidate page: trigger" event (traced after the device's invalidate) to the
# guest's next trapped access (the guest saw TRIGGER clear), and the PTE batch length: first
# flush read of a batch to the kick
for path in sys.argv[1:]:
    gaps, batch = [], []
    pend = None
    first_flush = None
    last = 0
    for ln in lzma.open(path, "rt", errors="replace"):
        p = ln.split(None, 6)
        if len(p) < 4 or not p[0].isdigit():
            continue
        ts = int(p[0])
        if p[1] == "E" and "invalidate page: trigger" in ln:
            pend = ts
        elif p[1] in ("R", "W") and p[2] in ("bar3", "bar0") and not p[3].startswith("0x00bb008"):
            if pend is not None:
                gaps.append((ts - pend) / 1000)
                pend = None
            if p[2] == "bar3" and p[3] == "0x00fea000":
                if first_flush is None or ts - last > 5000:
                    first_flush = ts
                last = ts
            if p[2] == "bar0" and p[3] == "0x00696000" and first_flush is not None:
                batch.append((ts - first_flush) / 1000)
                first_flush = None
    q = lambda v, f: sorted(v)[int(len(v) * f)] if v else float("nan")
    print(path.split("/")[0], f"trigger->next access n {len(gaps)} p50 {q(gaps,.5):.3f} p90 {q(gaps,.9):.3f} p99 {q(gaps,.99):.3f} ms;",
          f"PTE batch->kick n {len(batch)} p50 {q(batch,.5):.3f} p90 {q(batch,.9):.3f} p99 {q(batch,.99):.3f} ms")
