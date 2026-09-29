#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""caraxes/runs/trap-census-2026-09-25/summarize.py with this directory's `on` (the fixed BAR2
memfd beside the BAR1 pool) next to the census's `off` and `on-nopool`.

The census tables of docs/caraxes/optimization-backlog.md from this directory's runs.

Reads MODE/census.json (caraxes/tools/census-table.py) for the modes bare, off and on, and
MODE/census.txt (census-cu's OP lines) for the guest and native wall times. Every marked
window also carries the background traps of the guest (interrupt servicing of host notifier
events that other clients' work on the card causes, the guest's PCI config polling); the idle
windows (op 1, 100 ms of sleep) give their rate per microsecond in each mode, and the "net"
column takes rate x window length off. Output: markdown on stdout.
"""
import collections
import json
import os
import re
import statistics
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
# mode -> run directories pooled into it (a second pass is MODE-2)
# Here: the census's off and on-nopool beside this directory's on (the fixed memfd with the pool).
C = "../trap-census-2026-09-25/"
MODES = {"off": [C + "off", C + "off-2"], "on-nopool": [C + "on-nopool"], "on": ["on"]}
NATIVE = [C + "native", C + "native-2"]
OPNAME = re.compile(r"OP name=(\S+) id=(\d+) reps=(\d+) per_rep=(\d+) med_us=([\d.]+)")
ORDER = [2, 3, 4, 10, 11, 12, 13, 14, 15, 16, 17, 20, 21, 30, 31, 32, 33, 34, 35, 36, 37, 40, 41, 50, 51, 1]
LABEL = {
    1: "idle, 100 ms (background)", 2: "cuInit", 3: "cuCtxCreate", 4: "cuCtxDestroy",
    10: "cuMemAlloc 1 MiB", 11: "cuMemFree 1 MiB", 12: "cuMemAlloc 64 MiB", 13: "cuMemFree 64 MiB",
    14: "cuMemAllocHost 1 MiB", 15: "cuMemFreeHost 1 MiB", 16: "cuMemHostRegister 1 MiB",
    17: "cuMemHostUnregister 1 MiB", 20: "launch + sync, single (after 30 ms idle)",
    21: "launch + sync, 1000 back to back (per launch)", 30: "H2D 1 MiB pinned", 31: "D2H 1 MiB pinned",
    32: "D2D 1 MiB", 33: "H2D 16 MiB pinned", 34: "D2H 16 MiB pinned", 35: "D2D 16 MiB",
    36: "H2D 1 MiB pageable", 37: "D2H 1 MiB pageable", 40: "cuStreamSynchronize (idle stream)",
    41: "cuEventRecord + cuEventSynchronize", 50: "cuModuleLoad (4.6 KiB cubin)", 51: "cuModuleUnload",
}
# classes a window takes (rpc-cost.py classes), for the "covered" column of the bare mode
COVER = {
    "intr": {"pmc-intr-en (RM lock)", "pmc page 0, other", "vf-intr-tree"},
    "bar2memfd": {"bar2-write", "bar2-read (not the flush page)"},
    "tokens": {"usermode-doorbell"},
    "bar1pool": {"bar1-write", "bar1-read"},
}


def load_times(path):
    t = collections.defaultdict(list)
    if not os.path.exists(path):
        return t
    for ln in open(path, errors="replace"):
        m = OPNAME.search(ln)
        if m:
            t[int(m.group(2))].append(float(m.group(5)))
    return t


def main():
    data, times, perrun = {}, {}, {}
    for m, dirs in MODES.items():
        runs = []
        for d in dirs:
            p = os.path.join(HERE, d, "census.json")
            if os.path.exists(p):
                runs.append((d, {int(k): v for k, v in json.load(open(p)).items()}))
                perrun[d] = load_times(os.path.join(HERE, d, "census.txt"))
        if not runs:
            continue
        merged = {}
        for op in {op for _, r in runs for op in r}:
            have = [r[op] for _, r in runs if op in r]
            n = sum(o["reps"] for o in have)
            o = dict(have[0])
            o["reps"] = n
            o["traps_per_rep"] = sum(x["traps_per_rep"] * x["reps"] for x in have) / n
            o["window_us_med"] = statistics.median([x["window_us_med"] for x in have])
            for key in ("classes", "events"):
                acc = collections.Counter()
                for x in have:
                    for c, v in x[key].items():
                        acc[c] += v * x["reps"]
                o[key] = {c: v / n for c, v in acc.items()}
            for key in ("syncs", "sync_calls", "sync_apply_us", "host_events"):
                o[key] = sum(x[key] * x["reps"] for x in have) / n
            rp = collections.defaultdict(lambda: [0.0, 0.0, 0.0])
            for x in have:
                for k, v in x["rpcs"].items():
                    for i in range(3):
                        rp[k][i] += v[i] * x["reps"]
            o["rpcs"] = {k: [v[0] / n, v[1] / n, v[2] / n] for k, v in rp.items()}
            merged[op] = o
        data[m] = merged
        times[m] = collections.defaultdict(list)
        for d, _ in runs:
            for op, v in perrun[d].items():
                times[m][op].extend(v)
    native = collections.defaultdict(list)
    for d in NATIVE:
        for op, v in load_times(os.path.join(HERE, d, "census.txt")).items():
            native[op].extend(v)
    bg = {}
    for m, d in data.items():
        idle = d.get(1)
        bg[m] = idle["traps_per_rep"] / idle["window_us_med"] if idle else 0.0

    def med(xs):
        return statistics.median(xs) if xs else None

    def fmt(x, nd=0):
        if x is None:
            return "-"
        return f"{x:.{nd}f}" if abs(x) < 1e5 else f"{x:.0f}"

    print("## Census table\n")
    print("Traps per operation, gross and net of the background (idle windows: "
          + ", ".join(f"{m} {bg[m] * 1e3:.2f} per ms" for m in data) + "). Wall time is the guest's median "
          "(census-cu, both ops runs pooled), native the host's median over the same probe.\n")
    hdr = "| operation | " + " | ".join(f"traps {m} (net)" for m in data) + " | covered in bare by: intr / memfd / tokens / pool | " \
          + " | ".join(f"wall {m}, us" for m in data) + " | native, us | off / native |"
    print(hdr)
    print("|" + "---|" * (hdr.count("|") - 1))
    for op in ORDER:
        if not any(op in d for d in data.values()):
            continue
        cells = []
        for m, d in data.items():
            o = d.get(op)
            if not o:
                cells.append("-")
                continue
            per = o["traps_per_rep"]
            win = o["window_us_med"]
            if op == 21:
                per, win = per / 1000, win / 1000
            net = max(0.0, per - bg[m] * win) if op != 1 else per
            cells.append(f"{fmt(per, 1)} ({fmt(net, 1)})")
        cov = "-"
        if "bare" in data and op in data["bare"]:
            c = data["bare"][op]["classes"]
            div = 1000 if op == 21 else 1
            cov = " / ".join(fmt(sum(c.get(k, 0) for k in COVER[w]) / div, 1) for w in ("intr", "bar2memfd", "tokens", "bar1pool"))
        walls = []
        for m in data:
            walls.append(fmt(med(times[m].get(op, [])), 1))
        nat = med(native.get(op, []))
        on = med(times.get("off", {}).get(op, [])) if "off" in times else None
        ratio = f"{on / nat:.1f}" if on and nat else "-"
        print(f"| {LABEL.get(op, op)} | " + " | ".join(cells) + f" | {cov} | " + " | ".join(walls)
              + f" | {fmt(nat, 1)} | {ratio} |")
    print()

    # What stays trapped in the on mode, by class, per operation (gross).
    for mode in [x for x in ("off", "on-nopool") if x in data]:
        print(f"## What stays trapped in mode {mode}, per operation, by class (gross)\n")
        classes = collections.Counter()
        for op in ORDER:
            o = data[mode].get(op)
            if o:
                for c, v in o["classes"].items():
                    classes[c] += v
        cols = [c for c, _ in classes.most_common(9)]
        print("| operation | " + " | ".join(cols) + " | GSP RPCs | shadow plans (RM map calls, apply us) | host doorbells | MSIs |")
        print("|" + "---|" * (len(cols) + 5))
        for op in ORDER:
            o = data[mode].get(op)
            if not o:
                continue
            div = 1000 if op == 21 else 1
            ev = o["events"]
            rp = sum(v[0] for v in o["rpcs"].values())
            print(f"| {LABEL.get(op, op)} | " + " | ".join(fmt(o['classes'].get(c, 0) / div, 1) for c in cols)
                  + f" | {fmt(rp / div, 1)} | {fmt(o['syncs'] / div, 1)} ({fmt(o['sync_calls'] / div, 1)}, {fmt(o['sync_apply_us'] / div)})"
                  + f" | {fmt(ev.get('host doorbells written (forwarded)', 0) / div, 1)}"
                  + f" | {fmt(ev.get('host notifier events -> MSI', 0) / div, 1)} |")
        print()
    # Run to run: guest medians per run directory for the operations that share the GR engine.
    print("## Run to run (guest median per run, us)\n")
    dirs = [d for d in perrun]
    print("| operation | " + " | ".join(dirs) + " | " + " | ".join(NATIVE) + " |")
    print("|" + "---|" * (len(dirs) + len(NATIVE) + 1))
    for op in (20, 21, 30, 32, 33, 35, 41, 50, 10, 3):
        cells = [fmt(med(perrun[d].get(op, [])), 1) for d in dirs]
        cells += [fmt(med(load_times(os.path.join(HERE, d, "census.txt")).get(op, [])), 1) for d in NATIVE]
        print(f"| {LABEL[op]} | " + " | ".join(cells) + " |")
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
