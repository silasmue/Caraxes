#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""compare.py BEFORE AFTER: the device-controls and gate rows of two phases of run.sh.

Prints the controls whose (status, label) changed, the non-zero answers only in AFTER
(new refusals), and the cuda table rows side by side."""
import glob
import os
import re
import sys


def controls(d):
    out = {}
    for line in open(os.path.join(d, "device-controls.txt")):
        m = re.match(r"\s*(\d+) (0x[0-9a-f]{8}) (0x[0-9a-f]+) \[(.*)\]$", line)
        if m:
            out.setdefault(m.group(2), set()).add((m.group(3), m.group(4)))
    return out


def table(d):
    rows = {}
    for p in glob.glob(os.path.join(d, "gate-cuda-*", "table.txt")):
        for line in open(p):
            if line.startswith(("#", "TEST")):
                continue
            f = line.split()
            if len(f) >= 2:
                rows[f[0]] = f[1]
    return rows


def nvml(d):
    t = open(os.path.join(d, "gate-nvml.txt")).read()
    m = re.search(r'"verdict":"(\w+)","reason":"([^"]*)"', t)
    return m.groups() if m else ("?", "?")


b, a = sys.argv[1], sys.argv[2]
cb, ca = controls(b), controls(a)
print(f"# {os.path.basename(b)} -> {os.path.basename(a)}")
print("\n## Controls whose answer changed (status, label)")
for cmd in sorted(set(cb) | set(ca)):
    if cb.get(cmd) != ca.get(cmd):
        print(f"- {cmd}: {sorted(cb.get(cmd, []))} -> {sorted(ca.get(cmd, []))}")
print("\n## Non-zero answers only after (new refusals)")
new = [
    (cmd, st, lab)
    for cmd, s in ca.items()
    for st, lab in s
    if st != "0x0" and (st, lab) not in cb.get(cmd, set())
]
for cmd, st, lab in sorted(new):
    print(f"- {cmd} {st} [{lab}]")
if not new:
    print("- none")
print("\n## nvml gate")
print(f"- before: {nvml(b)}\n- after: {nvml(a)}")
print("\n## cuda gate rows")
tb, ta = table(b), table(a)
for t in sorted(set(tb) | set(ta)):
    print(f"- {t}: {tb.get(t, '-')} -> {ta.get(t, '-')}")
