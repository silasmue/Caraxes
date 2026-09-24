#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""compare.py: the per-frame trap tables of the 615 series next to series C of the display page
(docs/caraxes/display-615.md §5): every row of trap-table.txt (trap-table.py --pages) with its
per-frame value in both, 615 minus 610 last. Usage: compare.py <615 session dir> [C dir]"""
import os
import re
import sys

here = os.path.dirname(os.path.abspath(__file__))
s615 = sys.argv[1]
c610 = sys.argv[2] if len(sys.argv) > 2 else os.path.join(here, "..", "display-page-2026-09-24", "C-display-page")


def rows(path):
    out, head = {}, ""
    for line in open(path):
        if line.startswith("window"):
            head = line.strip()
            continue
        m = re.match(r"^(.*?)\s+(\d+)\s+([\d.]+)\s*$", line.rstrip())
        if m and m.group(1).strip() != "what":
            out.setdefault(m.group(1).rstrip(), float(m.group(3)))
    return head, out


for comp in ("traps-mutter", "traps-weston"):
    h6, a = rows(os.path.join(s615, comp, "trap-table.txt"))
    h0, b = rows(os.path.join(c610, comp, "trap-table.txt"))
    print(f"== {comp}\n615: {h6}\n610 C: {h0}")
    print(f"{'row':66} {'615':>8} {'610 C':>8} {'diff':>8}")
    for k in list(a) + [k for k in b if k not in a]:
        x, y = a.get(k, 0.0), b.get(k, 0.0)
        if max(x, y) >= 0.05:
            print(f"{k[:66]:66} {x:8.2f} {y:8.2f} {x - y:+8.2f}")
    print()
