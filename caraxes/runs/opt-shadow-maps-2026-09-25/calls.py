#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
"""The shadow's host RM calls per census operation (docs/caraxes/memory-model.md §7).

Reads a device trace of census-cu processes (plain or .xz) and splits it at census-cu's marker
writes (NV_PBUS_SW_SCRATCH(30), BAR0 0x1478, as census-table.py does). Per operation, divided
by its repetitions:
  - the shadow plans applied in the window, their worker apply time, and the RM calls of their
    sync lines by kind ("RM calls N: map a (x us), unmap b (y us), object alloc c, ...; regions
    reused R, pooled P", shadow.rs RmCalls::line);
  - the incremental teardowns that finished in the window (their own RM calls, the last
    "RM calls" of the "freed:" line).
Every plan and teardown is counted in the window in which its line appears (a plan's line is
written when the worker has applied it). Lines outside every window are listed at the end.
A trace of a binary before these counters has none: then only the "mapped in N call(s)" of
the sync lines is counted, as maps. The second table lists every window of cuCtxCreate and
cuCtxDestroy on its own.

Usage: calls.py TRACE [--names census.txt]
"""
import argparse
import collections
import lzma
import re

ACCESS = re.compile(r"^\s*(\d+) ([RW]) (\S+)\s+(0x[0-9a-f]+) (\d+)\s+(\S+)")
SYNC = re.compile(r"shadow: sync .*?mapped in (\d+) call\(s\).*?apply (\d+) us")
CALLS = re.compile(r"RM calls (\d+): (.*?); regions reused (\d+), pooled (\d+)")
PAIR = re.compile(r"([a-z][a-z ]*?) (\d+) \((\d+) us\)")
OPNAME = re.compile(r"OP name=(\S+) id=(\d+) reps=(\d+)")
KINDS = ["map", "unmap", "object alloc", "object free", "invalidate", "gather alloc", "gather free"]
CTX_OPS = (3, 4)


def opener(path):
    return lzma.open(path, "rt", errors="replace") if path.endswith(".xz") else open(path, errors="replace")


class Acc:
    def __init__(self):
        self.reps = 0
        self.plans = 0
        self.apply_us = 0
        self.n = collections.Counter()
        self.us = collections.Counter()
        self.reused = 0
        self.pooled = 0
        self.teardowns = 0
        self.td_n = collections.Counter()
        self.td_us = collections.Counter()
        self.legacy_maps = 0
        self.has_calls = False

    def calls(self):
        return sum(self.n.values()) if self.has_calls else self.legacy_maps

    def account(self, line):
        s = SYNC.search(line)
        if s:
            self.plans += 1
            self.apply_us += int(s.group(2))
            c = CALLS.search(line)
            if c:
                self.has_calls = True
                add(c.group(2), self.n, self.us)
                self.reused += int(c.group(3))
                self.pooled += int(c.group(4))
            else:
                self.legacy_maps += int(s.group(1))
            return
        if "shadow: VAS of guest root" in line and "freed:" in line:
            self.teardowns += 1
            cs = list(CALLS.finditer(line))
            if cs:
                self.has_calls = True
                add(cs[-1].group(2), self.td_n, self.td_us)


def add(pairs, n, us):
    for m in PAIR.finditer(pairs):
        n[m.group(1)] += int(m.group(2))
        us[m.group(1)] += int(m.group(3))


def read_names(path):
    names = {}
    if path:
        with open(path, errors="replace") as f:
            for ln in f:
                m = OPNAME.search(ln)
                if m:
                    names[int(m.group(2))] = m.group(1)
    return names


def scan(trace):
    """Per op id an Acc, the Acc of everything outside the windows, and every window."""
    ops = collections.defaultdict(Acc)
    outside = Acc()
    cur, t_start, win = None, 0, None
    windows = []  # (op, rep, window us, Acc)
    with opener(trace) as f:
        for line in f:
            m = ACCESS.match(line)
            if m:
                if m.group(3) == "bar0" and m.group(2) == "W" and int(m.group(4), 16) == 0x1478:
                    v = int(m.group(6), 16)
                    if v >> 24 == 0xCE:
                        op, end = (v >> 12) & 0xFFF, v & 1
                        if not end:
                            cur, t_start, win = op, int(m.group(1)), Acc()
                        elif cur == op:
                            ops[op].reps += 1
                            windows.append((op, ops[op].reps, int(m.group(1)) - t_start, win))
                            cur, win = None, None
                continue
            if cur is None:
                outside.account(line)
            else:
                ops[cur].account(line)
                win.account(line)
    return ops, outside, windows


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("trace")
    ap.add_argument("--names", default="")
    a = ap.parse_args()
    names = read_names(a.names)
    ops, outside, windows = scan(a.trace)

    print("# Shadow host RM calls per census operation\n")
    print(f"Trace: `{a.trace.rsplit('/', 1)[-1]}`. Per repetition; plans are the shadow sync lines in the window, "
          "teardowns the incremental VAS teardowns that finished in it.\n")
    hdr = "| operation | reps | plans | apply ms | " + " | ".join(f"{k}: n (ms)" for k in KINDS) + \
          " | RM calls | regions reused / pooled | teardowns: calls (ms) |"
    print(hdr)
    print("|" + "---|" * (hdr.count("|") - 1))

    def row(label, o, n):
        n = max(n, 1)
        if o.has_calls:
            cells = [f"{o.n[k] / n:.1f} ({o.us[k] / n / 1000:.1f})" for k in KINDS]
            td = f"{sum(o.td_n.values()) / n:.1f} ({sum(o.td_us.values()) / n / 1000:.1f})" if o.teardowns else "-"
            ru = f"{o.reused / n:.1f} / {o.pooled / n:.1f}"
        else:
            cells = [f"{o.legacy_maps / n:.1f} (-)"] + ["-"] * 4
            td, ru = ("yes" if o.teardowns else "-"), "-"
        print(f"| {label} | {o.reps} | {o.plans / n:.1f} | {o.apply_us / n / 1000:.1f} | " + " | ".join(cells) +
              f" | {o.calls() / n:.1f} | {ru} | {td} |")

    for op in sorted(ops):
        o = ops[op]
        if o.plans == 0 and o.teardowns == 0:
            continue
        row(names.get(op, f"op {op}"), o, o.reps)
    if outside.plans or outside.teardowns:
        row("outside every window (total)", outside, 1)

    print("\n## Every context window\n")
    print("| operation | rep | window ms | plans | apply ms | maps | unmaps | object alloc / free | invalidates | RM calls | teardowns |")
    print("|---|---|---|---|---|---|---|---|---|---|---|")
    for op, rep, us, w in windows:
        if op not in CTX_OPS:
            continue
        maps = w.n["map"] if w.has_calls else w.legacy_maps
        cell = (lambda k: str(w.n[k])) if w.has_calls else (lambda k: "-")
        print(f"| {names.get(op, f'op {op}')} | {rep} | {us / 1000:.1f} | {w.plans} | {w.apply_us / 1000:.1f} | {maps} | "
              f"{cell('unmap')} | {cell('object alloc')} / {cell('object free')} | {cell('invalidate')} | {w.calls()} | {w.teardowns} |")


if __name__ == "__main__":
    main()
