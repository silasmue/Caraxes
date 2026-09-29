#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Before against after for the rows of backlog item 2 (docs/caraxes/optimization-backlog.md).

Per row: the guest's median (census-cu's OP lines; cuInit is the median of its three
processes), the shadow's plans, worker apply time and host RM calls per repetition (calls.py
on the census trace), for each run directory given, and native from the trap census.

Usage: compare.py DIR... (each with census.txt and census-trace.txt[.xz])
"""
import os
import re
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import calls  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
NATIVE = [os.path.join(HERE, "..", "trap-census-2026-09-25", d, "census.txt") for d in ("native", "native-2")]
ROWS = [
    ("cuInit", "cuInit", 2),
    ("cuCtxCreate", "cuCtxCreate", 3),
    ("cuCtxDestroy", "cuCtxDestroy", 4),
    ("cuMemAlloc 64 MiB", "cuMemAlloc_64M", 12),
    ("cuMemFree 64 MiB", "cuMemFree_64M", 13),
    ("cuMemAllocHost 1 MiB", "cuMemHostAlloc_1M", 14),
    ("cuMemFreeHost 1 MiB", "cuMemFreeHost_1M", 15),
    ("cuMemHostRegister 1 MiB", "cuMemHostRegister_1M", 16),
    ("cuMemHostUnregister 1 MiB", "cuMemHostUnregister_1M", 17),
]
OP = re.compile(r"OP name=(\S+) id=(\d+) reps=(\d+) per_rep=\d+ med_us=([\d.]+)")


def medians(path):
    got = {}
    with open(path, errors="replace") as f:
        for ln in f:
            m = OP.search(ln)
            if m:
                got.setdefault(m.group(1), []).append(float(m.group(4)))
    return {k: statistics.median(v) for k, v in got.items()}


def main():
    dirs = sys.argv[1:]
    runs = []
    for d in dirs:
        tr = os.path.join(d, "census-trace.txt")
        if not os.path.exists(tr):
            tr += ".xz"
        ops, _, windows = calls.scan(tr)
        runs.append((os.path.basename(d.rstrip("/")), medians(os.path.join(d, "census.txt")), ops, windows))
    nat = {}
    for p in NATIVE:
        if os.path.exists(p):
            for k, v in medians(p).items():
                nat.setdefault(k, []).append(v)
    nat = {k: statistics.median(v) for k, v in nat.items()}

    names = [r[0] for r in runs]
    print("| operation | " + " | ".join(f"guest median {n}, us" for n in names) + " | native, us | " +
          " | ".join(f"{n}: plans, apply ms, RM calls per rep" for n in names) + " |")
    print("|---|" + "---|" * (2 * len(names) + 1))
    for label, key, op in ROWS:
        cells = [f"{r[1][key]:.0f}" if key in r[1] else "-" for r in runs]
        work = []
        for r in runs:
            o = r[2].get(op)
            if o is None or o.reps == 0:
                work.append("-")
            else:
                n = o.reps
                work.append(f"{o.plans / n:.1f}, {o.apply_us / n / 1000:.1f}, {o.calls() / n:.0f}")
        print(f"| {label} | " + " | ".join(cells) + f" | {nat.get(key, float('nan')):.0f} | " + " | ".join(work) + " |")

    print("\nHost RM calls by kind per cuCtxCreate and cuCtxDestroy (sum over the plans in the window, "
          "and the teardowns that finished in it), per repetition, count (ms):\n")
    print("| run | operation | " + " | ".join(calls.KINDS) + " | total | teardown calls |")
    print("|---|---|" + "---|" * (len(calls.KINDS) + 2))
    for name, _, ops, _ in runs:
        for op in (3, 4):
            o = ops.get(op)
            if o is None or not o.reps:
                continue
            n = o.reps
            cells = [f"{o.n[k] / n:.1f} ({o.us[k] / n / 1000:.1f})" for k in calls.KINDS]
            td = sum(o.td_n.values()) / n
            print(f"| {name} | {'cuCtxCreate' if op == 3 else 'cuCtxDestroy'} | " + " | ".join(cells) +
                  f" | {o.calls() / n:.1f} | {td:.1f} |")

    print("\nEvery context window (ms: window, worker apply; RM calls):\n")
    print("| run | " + " | ".join(f"create {i} | destroy {i}" for i in range(1, 6)) + " |")
    print("|---|" + "---|" * 10)
    for name, _, _, windows in runs:
        cells = []
        for op, rep, us, w in windows:
            if op in (3, 4):
                cells.append(f"{us / 1000:.0f}, {w.apply_us / 1000:.0f}; {w.calls()}")
        print(f"| {name} | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    main()
