#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""(caraxes/runs/trap-census-2026-09-25/workload-rates.py with two rows more: all trapped
accesses, and the MSI re-arms that came through the ioeventfd.) Counts per workload window (MODE/{rl,ppo,short-auto}-trace.txt.xz of run.sh, the rl and ppo
traces in CENSUS_TRACE_STORE): submissions
(trapped doorbells and token-pool eventfd rings), MSIs, the trapped classes that stay with the
windows on, and the same per second of window. The window is the whole bench-run.sh process,
so it includes one context creation and destruction (their BAR2, GSP and invalidate traffic);
the "steady" columns leave out the classes that belong to the context lifetime (BAR2 writes
and flush reads, GSP queue and falcon registers). Output: markdown on stdout.
"""
import collections
import lzma
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
# The rl and ppo traces (47 MB) live beside the VM state, not in git; short-auto stays here.
STORE = os.environ.get("CENSUS_TRACE_STORE", os.path.join("/mnt/vmstore/caraxes", os.path.basename(HERE)))
ACCESS = re.compile(r"^\s*(\d+) ([RW]) (\S+)\s+(0x[0-9a-f]+) ")
KEYS = ["submissions (trapped doorbells + token-pool eventfd rings)", "MSIs", "BAR1 writes", "intr pages (0x0, 0xb81000)",
        "MSI re-arm NV_PCFG 0x88704", "PCI config reads", "MMU invalidate", "BAR2 writes", "BAR2 flush reads",
        "GSP queue + falcon", "other", "all trapped accesses", "MSI re-arm through the eventfd (writes)"]


def count(path):
    c = collections.Counter()
    t0 = t1 = None
    with lzma.open(path, "rt", errors="replace") as f:
        for ln in f:
            m = ACCESS.match(ln)
            if m:
                c[KEYS[11]] += 1
                t = int(m.group(1))
                t0 = t if t0 is None else t0
                t1 = t
                r, rw, off = m.group(3), m.group(2), int(m.group(4), 16)
                if r == "bar0" and off == 0xBB0090:
                    c[KEYS[0]] += 1
                elif r == "bar1":
                    c[KEYS[2]] += 1
                elif r == "bar0" and (off & ~0xFFF) in (0, 0xB81000):
                    c[KEYS[3]] += 1
                elif r == "bar0" and off == 0x88704:
                    c[KEYS[4]] += 1
                elif r == "cfg":
                    c[KEYS[5]] += 1
                elif r == "bar0" and (off & ~0xFFF) == 0xB83000:
                    c[KEYS[6]] += 1
                elif r == "bar3" and rw == "W":
                    c[KEYS[7]] += 1
                elif r == "bar3":
                    c[KEYS[8]] += 1
                elif r == "bar0" and (off & ~0xFFF) == 0x110000:
                    c[KEYS[9]] += 1
                else:
                    c[KEYS[10]] += 1
                continue
            if " E " in ln[:18]:
                if "(eventfd) -> chid" in ln:
                    c[KEYS[0]] += 1
                elif "MSI [host]" in ln:
                    c[KEYS[1]] += 1
                elif "msi re-arm (eventfd): " in ln:
                    c[KEYS[12]] += int(ln.split("msi re-arm (eventfd): ")[1].split()[0])
    return c, ((t1 - t0) / 1e6 if t0 is not None else 0.0)


def main():
    modes = sys.argv[1:] or ["bare", "bare-2", "off", "off-2", "on-nopool"]
    for w in ("rl", "ppo", "short-auto"):
        rows = {}
        for m in modes:
            p = os.path.join(HERE, m, f"{w}-trace.txt.xz")
            if not os.path.exists(p):
                p = os.path.join(STORE, m, f"{w}-trace.txt.xz")
            if os.path.exists(p):
                rows[m] = count(p)
        if not rows:
            continue
        print(f"### {w}\n")
        print("| count (whole process window) | " + " | ".join(f"{m} ({s:.1f} s)" for m, (_, s) in rows.items()) + " |")
        print("|" + "---|" * (len(rows) + 1))
        for k in KEYS:
            print(f"| {k} | " + " | ".join(f"{c.get(k, 0)} ({c.get(k, 0) / s:.0f}/s)" for c, s in rows.values()) + " |")
        print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
