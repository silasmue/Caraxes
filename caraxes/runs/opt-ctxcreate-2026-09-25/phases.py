#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Where the time of a context creation and destruction goes (docs/caraxes/ctx-create.md).

Reads a census device trace (plain or .xz, census-cu's markers, as census-table.py) and, per
repetition of cuCtxCreate (op 3) and cuCtxDestroy (op 4), splits the device's window into:
  - trapped: the gaps before trapped access lines up to --idle-ms (the guest's time on its
    traps, as rpc-cost.py attributes it), by class;
  - first doorbells: the device's handling of a shadow channel's first doorbell (the host
    build: plan and apply wait, TSG, channel, engine objects, schedule and token, doorbell),
    from the "shadow: chid N built in" line (builds from 2026-09-25 on); a trace without
    those lines gets the gap before the doorbell line instead (the build and the guest's
    work before the ring);
  - other gaps above --idle-ms: time without a trap, the guest's own work or its wait for
    the GPU (reported with the line that ended the gap);
and adds the device's background work in the window: shadow plans applied by the worker
(apply and gather time) and RPCs.

Usage: phases.py TRACE [--names census.txt] [--idle-ms 2]
"""
import argparse
import collections
import lzma
import re
import statistics

ACCESS = re.compile(r"^\s*(\d+) ([RW]) (\S+)\s+(0x[0-9a-f]+) (\d+)\s+(\S+)\s*(.*)$")
EVENT = re.compile(r"^\s*(\d+) E (.*)$")
BUILT = re.compile(r"shadow: chid (\d+) built in (\d+) us: plan and apply (\d+) us, TSG (\d+) us, channel (\d+) us, objects (\d+) us, schedule and token (\d+) us, doorbell (\d+) us")
SYNC = re.compile(r"shadow: sync .*?apply (\d+) us.*?gather alloc (\d+) \((\d+) us\)")
RX = re.compile(r"rx seq \d+ (\S+) \(")
FREED = re.compile(r"host channel \S+ (?:freed|retired) in (\d+) us")
MARK = 0x1478
PHASES = ["plan and apply", "TSG", "channel", "objects", "schedule and token", "doorbell"]


def klass(region, off, name):
    off = int(off, 16)
    if region == "bar0" and off == 0xBB0090:
        return "usermode-doorbell"
    if region == "bar0" and (off & ~0xFFF) == 0xB83000:
        return "mmu-invalidate"
    if region == "bar0" and (off & ~0xFFF) == 0x110000:
        return "gsp"
    if region == "bar3" and off == 0xFEA000:
        return "bar2-flush-read"
    if region.startswith("pci") or region == "config":
        return "pci-config"
    return f"{region}-other"


def windows(path):
    op = rep = None
    lines = []
    opener = lzma.open if path.endswith(".xz") else open
    with opener(path, "rt", errors="replace") as f:
        for line in f:
            m = ACCESS.match(line)
            if m and m.group(3) == "bar0" and int(m.group(4), 16) == MARK and m.group(2) == "W":
                v = int(m.group(6), 16)
                if v >> 24 != 0xCE:
                    continue
                o, r, end = (v >> 12) & 0xFFF, (v >> 1) & 0x7FF, v & 1
                if not end:
                    op, rep, lines = o, r, [line]
                elif op == o and rep == r:
                    lines.append(line)
                    yield op, rep, lines
                    op = None
                continue
            if op is not None:
                lines.append(line)


def analyse(lines, idle_us):
    t0 = int(lines[0].split()[0])
    t1 = int(lines[-1].split()[0])
    out = {"window": t1 - t0, "trapped": collections.Counter(), "builds": [0] * 6, "nbuilds": 0,
           "doorbell_gaps": 0, "ndoorbell_gaps": 0, "gaps": collections.Counter(), "gap_total": 0,
           "apply": 0, "gather": 0, "ngather": 0, "rpcs": 0, "chfree": 0, "nchfree": 0}
    prev = t0
    pending_db = None  # (gap) of a doorbell line whose build line may follow
    for i, line in enumerate(lines[1:-1], 1):
        m = ACCESS.match(line)
        if m:
            t = int(m.group(1))
            gap = t - prev
            prev = t
            k = klass(m.group(3), m.group(4), m.group(7))
            if gap <= idle_us:
                out["trapped"][k] += gap
            elif k == "usermode-doorbell":
                pending_db = gap
                # A build line follows within the next events if this was a first doorbell.
                built = None
                for nxt in lines[i + 1:i + 40]:
                    b = BUILT.search(nxt)
                    if b:
                        built = b
                        break
                    if ACCESS.match(nxt):
                        break
                if built:
                    ph = [int(built.group(j)) for j in range(3, 9)]
                    out["builds"] = [a + b for a, b in zip(out["builds"], ph)]
                    out["nbuilds"] += 1
                    rest = gap - int(built.group(2))
                    if rest > 0:
                        out["gaps"]["before a first doorbell (guest)"] += rest
                        out["gap_total"] += rest
                else:
                    out["doorbell_gaps"] += gap
                    out["ndoorbell_gaps"] += 1
            else:
                out["gaps"][f"before {k}"] += gap
                out["gap_total"] += gap
            continue
        e = EVENT.match(line)
        if not e:
            continue
        text = e.group(2)
        s = SYNC.search(text)
        if s:
            out["apply"] += int(s.group(1))
            out["gather"] += int(s.group(3))
            out["ngather"] += int(s.group(2))
        if RX.search(text):
            out["rpcs"] += 1
        f = FREED.search(text)
        if f:
            out["chfree"] += int(f.group(1))
            out["nchfree"] += 1
    # Builds found by a doorbell whose gap stayed under idle: count them too.
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("trace")
    ap.add_argument("--names")
    ap.add_argument("--idle-ms", type=float, default=2.0)
    a = ap.parse_args()
    idle = int(a.idle_ms * 1000)
    per = collections.defaultdict(list)
    for op, rep, lines in windows(a.trace):
        if op in (3, 4):
            per[op].append(analyse(lines, idle))
    name = {3: "cuCtxCreate", 4: "cuCtxDestroy"}
    print(f"# phases: {a.trace}\n")
    print("Medians over the repetitions, ms. Trapped: gaps up to the idle bound before a trapped access;"
          " builds: the device's first-doorbell handling; untrapped: the other gaps; worker: background.\n")
    for op, rs in sorted(per.items()):
        med = lambda f: statistics.median(f(r) for r in rs) / 1000
        print(f"## {name[op]} ({len(rs)} repetitions)\n")
        print("| part | ms |\n|---|---|")
        print(f"| window | {med(lambda r: r['window']):.1f} |")
        print(f"| trapped, all classes | {med(lambda r: sum(r['trapped'].values())):.1f} |")
        classes = sorted({k for r in rs for k in r["trapped"]})
        for k in classes:
            print(f"| trapped: {k} | {med(lambda r: r['trapped'][k]):.1f} |")
        print(f"| first-doorbell builds ({statistics.median(r['nbuilds'] for r in rs):.0f}) | {med(lambda r: sum(r['builds'])):.1f} |")
        for j, p in enumerate(PHASES):
            print(f"| build: {p} | {med(lambda r: r['builds'][j]):.1f} |")
        print(f"| doorbell gaps without a build line ({statistics.median(r['ndoorbell_gaps'] for r in rs):.0f}) | {med(lambda r: r['doorbell_gaps']):.1f} |")
        print(f"| untrapped gaps above {a.idle_ms} ms | {med(lambda r: r['gap_total']):.1f} |")
        gk = sorted({k for r in rs for k in r["gaps"]})
        for k in gk:
            print(f"| untrapped: {k} | {med(lambda r: r['gaps'][k]):.1f} |")
        print(f"| worker apply (background) | {med(lambda r: r['apply']):.1f} |")
        print(f"| of it gather allocs ({statistics.median(r['ngather'] for r in rs):.0f}) | {med(lambda r: r['gather']):.1f} |")
        print(f"| host channel frees or retires in RPC handling ({statistics.median(r['nchfree'] for r in rs):.0f}) | {med(lambda r: r['chfree']):.1f} |")
        print(f"| RPCs (count) | {statistics.median(r['rpcs'] for r in rs):.0f} |")
        print()


if __name__ == "__main__":
    main()
