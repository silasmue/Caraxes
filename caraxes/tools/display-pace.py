#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Per-frame display timeline from device traces (docs/caraxes/display-pacing.md).

Usage: display-pace.py TRACE [TRACE...]   (plain, .gz or .xz device traces, us timestamps)

Per trace window: the tick fit (period, latch lateness after its tick), ticks between
latches, whether a flip latched at the first tick after its kick, and per frame the
latch -> next UPDATE kick time split into
  - latch -> start of the pre-kick run of trapped BAR2 flush reads / MMU invalidates,
  - that run (flush reads and invalidates less than 3 ms apart, ending at the kick),
  - run end -> kick,
plus the flush-read spacing inside runs (the device's service time per flush read).
"""
import bisect
import collections
import gzip
import lzma
import re
import sys

KICK = re.compile(r"display channel 0xc57e/\d+ kick \d+: PUT \S+, \d+ method\(s\), (\d+) UPDATE")
LATCH = re.compile(r"display: window \d+ latch \d+ at tick (\d+)")


def opener(p):
    if p.endswith(".xz"):
        return lzma.open(p, "rt", errors="replace")
    if p.endswith(".gz"):
        return gzip.open(p, "rt", errors="replace")
    return open(p, errors="replace")


def pct(v, q):
    v = sorted(v)
    return v[min(len(v) - 1, int(q * len(v)))] if v else float("nan")


def dist(v):
    return f"p50 {pct(v, .5):.1f} / p90 {pct(v, .9):.1f} / p99 {pct(v, .99):.1f} (n={len(v)})"


def load(path):
    kicks, latches, acc = [], [], []
    for line in opener(path):
        parts = line.split(None, 1)
        if len(parts) < 2 or not parts[0].isdigit():
            continue
        t, body = int(parts[0]) / 1000.0, parts[1]
        if body.startswith("R bar3 0x00fea000"):
            acc.append((t, "f"))
        elif body.startswith("W bar0 0x00b830b0"):
            acc.append((t, "I"))
        elif (m := KICK.search(body)) and int(m.group(1)) > 0:
            kicks.append(t)
        elif m := LATCH.search(body):
            latches.append((t, int(m.group(1))))
    return kicks, latches, acc


def report(path):
    kicks, latches, acc = load(path)
    if len(latches) < 3:
        print(f"== {path}: {len(latches)} latch(es), nothing to fit")
        return
    n = len(latches)
    mk = sum(k for _, k in latches) / n
    mt = sum(t for t, _ in latches) / n
    per = sum((k - mk) * (t - mt) for t, k in latches) / sum((k - mk) ** 2 for _, k in latches)
    t0 = min(t - k * per for t, k in latches)
    span = (max(kicks[-1], latches[-1][0]) - min(kicks[0], latches[0][0])) / 1000
    print(f"== {path}")
    print(f"  {len(kicks)} UPDATE kicks in {span:.1f} s = {len(kicks) / span:.1f}/s; tick period {per:.3f} ms")
    print(f"  latch after its tick ms: {dist([t - (t0 + k * per) for t, k in latches])}")
    gaps = collections.Counter(latches[i + 1][1] - latches[i][1] for i in range(n - 1))
    print(f"  ticks between latches: {dict(sorted(gaps.items()))}")
    lt = [t for t, _ in latches]
    late = collections.Counter()
    for k in kicks:
        j = bisect.bisect_right(lt, k)
        if j < n:
            late[latches[j][1] - (int((k - t0) // per) + 1)] += 1
    print(f"  latch tick minus first tick after the kick: {dict(sorted(late.items()))}")
    at = [a[0] for a in acc]
    l2k, l2run, run, end2k, nacc = [], [], [], [], []
    for i, (t, _) in enumerate(latches):
        j = bisect.bisect_right(kicks, t)
        if j >= len(kicks) or (i + 1 < n and kicks[j] > latches[i + 1][0]):
            continue
        k = kicks[j]
        l2k.append(k - t)
        e = bisect.bisect_left(at, k) - 1
        if e < 0 or at[e] < t:
            l2run.append(k - t)
            run.append(0.0)
            end2k.append(0.0)
            nacc.append(0)
            continue
        s = e
        while s > 0 and at[s] - at[s - 1] < 3.0 and at[s - 1] > t:
            s -= 1
        l2run.append(at[s] - t)
        run.append(at[e] - at[s])
        end2k.append(k - at[e])
        nacc.append(e - s + 1)
    print(f"  latch -> next kick ms: {dist(l2k)}")
    print(f"    latch -> pre-kick flush/invalidate run: {dist(l2run)}")
    print(f"    the run: {dist(run)}; accesses {dist(nacc)}")
    print(f"    run end -> kick: {dist(end2k)}")
    fl = [t for t, w in acc if w == "f"]
    sp = [fl[i + 1] - fl[i] for i in range(len(fl) - 1) if fl[i + 1] - fl[i] < 3.0]
    print(f"  flush reads {len(fl) / span:.0f}/s, spacing inside runs ms: {dist(sp)}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    for p in sys.argv[1:]:
        report(p)
