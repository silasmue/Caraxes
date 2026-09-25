#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# docs/caraxes/intr-page.md §11.7 (tab.py, run from caraxes/runs/) and §11.8 (the others,
# with device-trace.txt.xz files as arguments).
import lzma, sys
# inside a PTE batch: (a) the last flush read before a trigger to the trigger event (the reactor
# noticing TRIGGER plus the device's invalidate with the owed sync); (b) the trigger event to the
# guest's next flush read or kick (after_bar0_write, TRIGGER clear, the guest's poll); (c) the
# flush-read spacing
q = lambda v, f: sorted(v)[int(len(v) * f)] if v else float("nan")
for path in sys.argv[1:]:
    a, b, c = [], [], []
    last_acc = None
    trig = None
    for ln in lzma.open(path, "rt", errors="replace"):
        p = ln.split(None, 6)
        if len(p) < 4 or not p[0].isdigit():
            continue
        ts = int(p[0])
        if p[1] == "E" and "invalidate page: trigger" in ln:
            if last_acc is not None and ts - last_acc < 5000:
                a.append((ts - last_acc) / 1000)
            trig = ts
        elif (p[2] == "bar3" and p[3] == "0x00fea000") or (p[2] == "bar0" and p[3] == "0x00696000"):
            if trig is not None and ts - trig < 5000:
                b.append((ts - trig) / 1000)
            elif last_acc is not None and ts - last_acc < 5000 and trig is None:
                c.append((ts - last_acc) / 1000)
            trig = None
            last_acc = ts
    f = lambda v: f"n {len(v)} p50 {q(v,.5):.3f} p90 {q(v,.9):.3f} p99 {q(v,.99):.3f}"
    print(path.split("/")[0], "| flush->trigger", f(a), "| trigger->next", f(b), "| flush spacing", f(c))
