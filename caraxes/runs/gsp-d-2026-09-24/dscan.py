#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
"""dscan.py: the category D controls in a device trace window (docs/caraxes/gsp-plan/
d-device-model.md §8, Q1 and Q3): every `ctrl 0x...` answer line whose id is a row of
d-controls.tsv, with the plan's answer and the label the device gave, the watched ids of
Q3 (0x2080110B FIFO_DISABLE_CHANNELS, 0x20800177 GPU_REPORT_NON_REPLAYABLE_FAULT) and
profiler allocations, and the count of `unanswered` answers of any control.

Usage: dscan.py <trace file, plain or .xz> [...]
"""
import collections
import lzma
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
TSV = os.path.join(ROOT, "docs", "caraxes", "gsp-plan", "d-controls.tsv")
WATCH = {0x2080110B: "FIFO_DISABLE_CHANNELS", 0x20800177: "GPU_REPORT_NON_REPLAYABLE_FAULT"}


def rows():
    head, out = None, {}
    for line in open(TSV, encoding="utf-8"):
        if line.startswith("#"):
            continue
        cols = line.rstrip("\n").split("\t")
        if head is None:
            head = cols
            continue
        r = dict(zip(head, cols))
        out[int(r["id"], 16)] = r
    return out


def lines(path):
    op = lzma.open if path.endswith(".xz") else open
    with op(path, "rt", encoding="utf-8", errors="replace") as f:
        yield from f


def main():
    d = rows()
    rx = re.compile(r"ctrl (0x[0-9a-f]+): (.*) -> status (0x[0-9a-f]+) \[(.*)\]\s*$")
    rx_alloc = re.compile(r"alloc \S+ (0x[0-9a-f]+) object|GSP_RM_ALLOC.*hClass=(0x[0-9a-f]+)")
    for path in sys.argv[1:]:
        seen = collections.Counter()
        labels = {}
        unanswered = collections.Counter()
        watch = []
        prof = []
        total = 0
        for line in lines(path):
            m = rx.search(line)
            if m:
                total += 1
                cmd = int(m.group(1), 16)
                if m.group(4).startswith("unanswered"):
                    unanswered[cmd] += 1
                if cmd in d:
                    seen[cmd] += 1
                    labels[cmd] = (m.group(3), m.group(4))
                if cmd in WATCH:
                    watch.append(line.strip()[:300])
                continue
            a = rx_alloc.search(line)
            if a:
                cls = int(a.group(1) or a.group(2), 16)
                if cls in (0x90CC, 0xB0CC, 0xB1CC, 0xB2CC):
                    prof.append(line.strip()[:300])
        print("== %s: %d control answers, %d distinct D ids, %d unanswered answers (%d ids)" % (
            path, total, len(seen), sum(unanswered.values()), len(unanswered)))
        for cmd, n in sorted(seen.items()):
            r = d[cmd]
            st, lab = labels[cmd]
            print("D %#010x %-55s x%-3d plan %-28s -> status %s [%s]" % (
                cmd, r["name"], n, r["answer"].split(" (")[0][:28], st, lab))
        for cmd, n in sorted(unanswered.items()):
            print("unanswered %#010x x%d%s" % (cmd, n, " (D)" if cmd in d else ""))
        for w in watch:
            print("watch:", w)
        for p in prof:
            print("profiler alloc:", p)
        if not watch:
            print("watch: none of %s" % ", ".join("%#x %s" % kv for kv in WATCH.items()))


if __name__ == "__main__":
    main()
