#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Whole-run trap rates from a device exit summary (gpu.log: "bar0: N reads, M writes", ...,
"bar0 trapped accesses by offset (busiest first ...)"). These counters are the locked path's:
the lock-free fast reads (PTIMER, usermode TIME) are not in them (the profile has them).
Usage: exit-summary.py DURATIONS.txt DIR... (DURATIONS: "<dir> <us>" lines)"""
import re
import sys

dur = {}
for line in open(sys.argv[1]):
    p = line.split()
    if len(p) == 2 and p[1].isdigit():
        dur[p[0]] = int(p[1]) / 1e6
REG = re.compile(r"^(bar\d|cfg|rom): (\d+) reads, (\d+) writes")
OFF = re.compile(r"(0x[0-9a-f]+) ([RW])(?: [^:,]*)?:(\d+)")
for d in sys.argv[2:]:
    name = d.rstrip("/").split("/")[-1]
    s = dur.get(name)
    if not s:
        continue
    regions, offs = {}, []
    for line in open(f"{d}/gpu.log", errors="replace"):
        m = REG.match(line)
        if m:
            regions[m.group(1)] = (int(m.group(2)), int(m.group(3)))
        elif line.startswith("bar0 trapped accesses by offset"):
            offs = [(o, rw, int(n)) for o, rw, n in OFF.findall(line.split("):", 1)[1])]
    tot = sum(r + w for r, w in regions.values())
    print(f"## {name}: {s:.0f} s, {tot} locked-path traps, {tot / s:.0f}/s")
    print("  " + ", ".join(f"{k} R {r / s:.0f}/s W {w / s:.0f}/s" for k, (r, w) in regions.items()))
    print("  bar0 top: " + ", ".join(f"{o} {rw} {n / s:.1f}/s" for o, rw, n in offs[:16]))
