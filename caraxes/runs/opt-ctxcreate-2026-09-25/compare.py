#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""The rows of docs/caraxes/ctx-create.md per run directory of this record: the census guest
medians (census-table.md) and bench-cu init/churn (v2/init-*.txt, v2/churn-*.txt), medians
with the range over the reps, plus the device's other VMs at start.

Usage: compare.py RUN [RUN ...]   (directory names under this record)
"""
import os
import re
import statistics
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CENSUS = ["cuInit", "cuCtxCreate", "cuCtxDestroy", "cuMemAlloc_1M", "cuMemAlloc_64M",
          "cuMemFree_1M", "cuMemFree_64M"]


def census(d):
    out = {}
    p = os.path.join(d, "census-table.md")
    if not os.path.exists(p):
        return out
    for line in open(p):
        c = [x.strip() for x in line.split("|")]
        if len(c) > 6 and c[2] in CENSUS:
            out[c[2]] = float(c[6])
    return out


def bench(d):
    vals = {}
    v2 = os.path.join(d, "v2")
    if not os.path.isdir(v2):
        return vals
    for f in sorted(os.listdir(v2)):
        for line in open(os.path.join(v2, f)):
            if line.startswith("INIT"):
                for k in ("cuinit_ms", "ctxcreate_ms", "ctxdestroy_ms"):
                    m = re.search(k + r"=([\d.]+)", line)
                    vals.setdefault(k, []).append(float(m.group(1)))
            elif line.startswith("CHURN"):
                mib = re.search(r"block_mib=(\d+)", line).group(1)
                for k in ("alloc_med_us", "free_med_us"):
                    m = re.search(k + r"=([\d.]+)", line)
                    vals.setdefault(f"{k.split('_')[0]} {mib} MiB us", []).append(float(m.group(1)))
    return vals


def fmt(v):
    if not v:
        return "-"
    return f"{statistics.median(v):.1f} [{min(v):.1f}-{max(v):.1f}]"


def main():
    runs = sys.argv[1:]
    rows = {}
    others = {}
    for r in runs:
        d = os.path.join(HERE, r)
        for k, v in census(d).items():
            rows.setdefault(f"census {k}, us", {})[r] = f"{v:.0f}"
        for k, v in bench(d).items():
            rows.setdefault(f"bench {k}", {})[r] = fmt(v)
        p = os.path.join(d, "other-vms-at-start.txt")
        if os.path.exists(p):
            others[r] = sum(1 for line in open(p) if "vfio-user-nvgpu --socket" in line and "/vm3/" not in line)
    print("| row | " + " | ".join(runs) + " |")
    print("|---|" + "---|" * len(runs))
    for k in sorted(rows):
        print(f"| {k} | " + " | ".join(rows[k].get(r, "-") for r in runs) + " |")
    print("| other device processes at start | " + " | ".join(str(others.get(r, "-")) for r in runs) + " |")


if __name__ == "__main__":
    main()
