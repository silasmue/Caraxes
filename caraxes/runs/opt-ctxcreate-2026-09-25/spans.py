#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""The three spans of each cuCtxCreate of a census trace (docs/caraxes/ctx-create.md §5).

Per repetition of cuCtxCreate (census-cu op 3), times from its start marker:
  - setup: to the first usermode doorbell (the guest's own work: RPCs, BAR2 flush reads,
    MMU invalidates);
  - rings: from the first to the last first-doorbell (the 20 host channel builds, with the
    first plan's wait);
  - tail: from the last doorbell to the end marker (libcuda's allocations after the rings).
Prints one row per run: the median and the minimum of each span and of the window, ms.

Usage: spans.py RUNDIR...   (each with census-trace.txt.xz)
"""
import lzma
import os
import re
import statistics
import sys

ACC = re.compile(r"^\s*(\d+) ([RW]) (\S+)\s+(0x[0-9a-f]+) (\d+)\s+(\S+)")


def reps(path):
    cur = None
    with lzma.open(path, "rt", errors="replace") as f:
        for line in f:
            m = ACC.match(line)
            if not m:
                continue
            t, reg, off = int(m.group(1)), m.group(3), int(m.group(4), 16)
            if reg == "bar0" and off == 0x1478 and m.group(2) == "W":
                v = int(m.group(6), 16)
                if v >> 24 != 0xCE or (v >> 12) & 0xFFF != 3:
                    continue
                if not v & 1:
                    cur = {"t0": t, "first": None, "last": None}
                elif cur:
                    cur["end"] = t
                    if cur["first"] is not None:
                        yield cur
                    cur = None
                continue
            if cur and reg == "bar0" and off == 0xBB0090 and m.group(2) == "W":
                cur["first"] = cur["first"] or t
                cur["last"] = t


def main():
    print("| run | window, median (min) | setup | rings | tail |")
    print("|---|---|---|---|---|")
    for d in sys.argv[1:]:
        rs = list(reps(os.path.join(d, "census-trace.txt.xz")))
        cols = {
            "window": [(r["end"] - r["t0"]) / 1000 for r in rs],
            "setup": [(r["first"] - r["t0"]) / 1000 for r in rs],
            "rings": [(r["last"] - r["first"]) / 1000 for r in rs],
            "tail": [(r["end"] - r["last"]) / 1000 for r in rs],
        }
        cell = lambda v: f"{statistics.median(v):.0f} ({min(v):.0f})"
        print(f"| {os.path.basename(d.rstrip('/'))} | " + " | ".join(cell(cols[k]) for k in cols) + " |")


if __name__ == "__main__":
    main()
