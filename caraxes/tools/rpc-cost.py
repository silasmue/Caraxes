#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Where the time of a guest RM operation goes (docs/caraxes/alloc-path.md).

Reads a vfio-user-nvgpu device trace (plain or .xz, access tracing on) and attributes the
guest's time to what it trapped on. Every trace line carries the microseconds since device
start at which the device finished handling it (trace.rs `line`), so the cost of a trapped
access, as the guest sees it, is the time since the previous access line: the guest's own
work in between, the KVM exit, the vfio-user round trip and the device's handling. A gap
longer than --idle-ms is idle time and is not attributed.

Sections:
  1. per access class (BAR2 page-table writes, the BAR2 flush read, the MMU invalidate,
     the RM lock's interrupt-enable toggles on BAR0 page 0 and the VF interrupt tree,
     the GSP queue, doorbells, ...): count, time, median and p90 per access;
  2. per GSP RPC function (and per class / control command): count, device time from the
     rx line to the tx reply line, and how many of them made a host RM call (an event with
     "[host]" between rx and tx);
  3. the MMU invalidates, per root, and the shadow plans the worker applied (RM map and
     unmap calls, walk and apply time, time queued): the device work that is off the
     guest's critical path unless a doorbell waits for it (mapping.md §4);
  4. with --gap-ms: the window split into segments where operation activity (BAR2, the
     MMU invalidate, the GSP queue, doorbells, BAR1 writes; not the interrupt servicing,
     which host notifier events from other clients' work cause at any time) pauses for
     the gap. bench-cu churn with BENCH_CU_PHASE_GAP_MS set sleeps between its phases, so
     one `bench-cu churn 1` process is six segments, labelled cyclically with
     --phases ctx:0,alloc1:64,free1:64,alloc64:8,free64:8,destroy:0 (a label with N 0 is
     not printed); per label the cost per operation by class and the projection of 5;
  5. a projection: what stays trapped with CARAXES_INTR_PAGE=1 (BAR0 page 0 and the VF
     interrupt tree become memory, intr-page.md) and CARAXES_BAR2_MEMFD=1 (BAR2 except the
     flush page becomes memory, bar2-memfd.md), at the measured per-class cost.

Usage: rpc-cost.py TRACE [--from US] [--to US] [--idle-ms 2] [--gap-ms MS]
                         [--phases LABEL:N,...] [--skip K] [--ops N]
                         [--flush-page 0xfea000] [--json OUT]
"""
import argparse
import collections
import json
import lzma
import re
import statistics
import sys

ACCESS = re.compile(r"^\s*(\d+) ([RW]) (\S+)\s+(0x[0-9a-f]+) (\d+)\s+(\S+)\s*(.*)$")
EVENT = re.compile(r"^\s*(\d+) E (.*)$")
RX = re.compile(r"rx seq \d+ (\S+) \(\d+\) sequence (\d+)")
RX_CLASS = re.compile(r"hClass=(0x[0-9a-f]+)")
RX_CMD = re.compile(r"cmd=(0x[0-9a-f]+)")
TX = re.compile(r"tx seq \d+ reply (\S+) sequence (\d+) result (\S+)")
INVAL = re.compile(r"mmu invalidate: pdb (0x[0-9a-f]+) flags (0x[0-9a-f]+)")
SYNC = re.compile(
    r"shadow: sync (\d+) leaves walked.*?-> (\d+) unmapped, (\d+) mapped in (\d+) call\(s\)"
    r".*?walk (\d+) us, apply (\d+) us(?: after (\d+) us in the worker's queue)?, sync (\d+) us"
)
# The shadow's host RM calls by kind (shadow.rs RmCalls::line, from 2026-09-25): on every sync
# line, and on a VAS teardown line (the last one there is the teardown's own).
RMCALLS = re.compile(r"RM calls (\d+): (.*?); regions reused (\d+), pooled (\d+)")
RMPAIR = re.compile(r"([a-z][a-z ]*?) (\d+) \((\d+) us\)")
RMCALL_HIST = collections.OrderedDict()


def rm_calls(text):
    """Add the RM calls of a sync or teardown line to RMCALL_HIST (kind -> [calls, us])."""
    found = list(RMCALLS.finditer(text))
    if not found:
        return
    for m in RMPAIR.finditer(found[-1].group(2)):
        h = RMCALL_HIST.setdefault(m.group(1), [0, 0])
        h[0] += int(m.group(2))
        h[1] += int(m.group(3))

# What an allocation, a free or a submission traps on (the rest is interrupt servicing).
ACTIVITY = {
    "bar2-write", "bar2-flush-read", "bar2-read (not the flush page)", "mmu-invalidate",
    "gsp-queue-doorbell", "usermode-doorbell", "bar1-write",
}

# Classes the existing switches turn into memory (no trap).
INTR_PAGE = {"pmc-intr-en (RM lock)", "pmc page 0, other", "vf-intr-tree"}
BAR2_MEMFD = {"bar2-write", "bar2-read (not the flush page)"}


def classify(region, rw, off, name, flush_page):
    if region == "bar3":
        if rw == "W":
            return "bar2-write"
        if off & ~0xFFF == flush_page:
            return "bar2-flush-read"
        return "bar2-read (not the flush page)"
    if region == "bar1":
        return "bar1-" + ("write" if rw == "W" else "read")
    if region == "cfg":
        return "pci-config"
    if region != "bar0":
        return region
    page = off & ~0xFFF
    if off in (0x160, 0x180):
        return "pmc-intr-en (RM lock)"
    if page == 0:
        return "pmc page 0, other"
    if page == 0xB81000:
        return "vf-intr-tree"
    if "MMU_INVALIDATE" in name:
        return "mmu-invalidate"
    if off == 0xBB0090:
        return "usermode-doorbell"
    if page == 0xBB0000:
        return "usermode page, other"
    if "QUEUE_HEAD" in name:
        return "gsp-queue-doorbell"
    if "PGSP" in name:
        return "gsp-falcon"
    if page == 0x88000:
        return "pcfg-mirror (intr re-arm)"
    if page == 0x611000:
        return "display-intr"
    return "bar0-other"


def num(n, ops):
    return f"{n}" if ops == 1 else f"{n / ops:.1f}"


def pct(xs, p):
    if not xs:
        return 0.0
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round(p * (len(xs) - 1))))]


def open_trace(path):
    if path == "-":
        return sys.stdin
    if path.endswith(".xz"):
        return lzma.open(path, "rt", errors="replace")
    return open(path, errors="replace")


class Stats:
    def __init__(self):
        self.n = collections.Counter()
        self.us = collections.defaultdict(list)

    def add(self, cls, dt):
        self.n[cls] += 1
        if dt is not None:
            self.us[cls].append(dt)

    def total_us(self, cls=None):
        if cls is None:
            return sum(sum(v) for v in self.us.values())
        return sum(self.us.get(cls, []))


def parse(args):
    flush_page = int(args.flush_page, 16) & ~0xFFF
    idle = args.idle_ms * 1000
    gap = args.gap_ms * 1000 if args.gap_ms else None
    whole = Stats()
    segments = []  # (start, end, Stats)
    cur = None
    last_access = None
    last_activity = None
    rpcs = {}  # sequence -> [func, key, t_rx, host_calls]
    rpc_done = collections.defaultdict(list)  # key -> [(dev_us, host_calls, result)]
    open_seq = []
    invals = collections.Counter()
    syncs = []
    # Episodes: from one MMU invalidate of a channel VA space (not HUBTLB_ONLY, which the
    # BAR1/BAR2 invalidates set) to the next; one guest RM mapping change each.
    episodes = []  # (duration us, Counter)
    ep = None
    t_first = t_last = None
    with open_trace(args.trace) as f:
        for line in f:
            m = ACCESS.match(line)
            if m:
                t = int(m.group(1))
                if args.start is not None and t < args.start:
                    continue
                if args.end is not None and t > args.end:
                    break
                t_first = t if t_first is None else t_first
                t_last = t
                region, rw, off = m.group(3), m.group(2), int(m.group(4), 16)
                cls = classify(region, rw, off, m.group(7), flush_page)
                dt = None if last_access is None else t - last_access
                if dt is not None and dt > idle:
                    dt = None
                if gap is not None and cls in ACTIVITY:
                    if cur is None or last_activity is None or t - last_activity >= gap:
                        cur = [t, t, Stats()]
                        segments.append(cur)
                    last_activity = t
                last_access = t
                whole.add(cls, dt)
                if ep is not None:
                    ep[1][cls] += 1
                if cur is not None:
                    cur[1] = t
                    cur[2].add(cls, dt)
                continue
            m = EVENT.match(line)
            if not m:
                continue
            t, text = int(m.group(1)), m.group(2)
            if args.start is not None and t < args.start:
                continue
            if args.end is not None and t > args.end:
                break
            if "rx seq" in text:
                r = RX.search(text)
                if r:
                    func, seq = r.group(1), r.group(2)
                    key = func
                    c = RX_CLASS.search(text)
                    d = RX_CMD.search(text)
                    if func == "GSP_RM_ALLOC" and c:
                        key = f"GSP_RM_ALLOC class {c.group(1)}"
                    elif func == "GSP_RM_CONTROL" and d:
                        key = f"GSP_RM_CONTROL cmd {d.group(1)}"
                    rpcs[seq] = [func, key, t, 0]
                    open_seq.append(seq)
                continue
            if "tx seq" in text:
                r = TX.search(text)
                if r and r.group(2) in rpcs:
                    func, key, t0, host = rpcs.pop(r.group(2))
                    if r.group(2) in open_seq:
                        open_seq.remove(r.group(2))
                    rpc_done[key].append((t - t0, host, r.group(3)))
                continue
            if "[host]" in text or "forwarded to the h" in text:
                for s in open_seq:
                    rpcs[s][3] += 1
            if ep is not None and "host notifier" in text:
                ep[1]["(host notifier events)"] += 1
            r = INVAL.search(text)
            if r:
                invals[(r.group(1), r.group(2))] += 1
                if int(r.group(2), 16) & 0x4 == 0:
                    if ep is not None:
                        episodes.append((t - ep[0], ep[1]))
                    ep = (t, collections.Counter())
            r = SYNC.search(text)
            if r:
                syncs.append(tuple(int(x) if x is not None else 0 for x in r.groups()))
                rm_calls(text)
            elif "shadow: VAS of guest root" in text and "freed:" in text:
                rm_calls(text)
    return whole, segments, rpc_done, invals, syncs, episodes, (t_first, t_last)


EP_COLS = ("bar2-write", "bar2-flush-read", "mmu-invalidate", "pmc-intr-en (RM lock)", "vf-intr-tree",
           "(host notifier events)")


def episode_table(episodes, top=14):
    groups = collections.defaultdict(list)
    for dur, c in episodes:
        rest = sum(v for k, v in c.items() if k not in EP_COLS)
        groups[tuple(c.get(k, 0) for k in EP_COLS) + (rest,)].append(dur)
    rows = ["| BAR2 writes | flush reads | invalidate accesses | RM lock toggles (page 0) | VF interrupt tree | host events | other | episodes | median us | min us |",
            "|---|---|---|---|---|---|---|---|---|---|"]
    for sig, durs in sorted(groups.items(), key=lambda kv: -len(kv[1]))[:top]:
        rows.append("| " + " | ".join(str(x) for x in sig) + f" | {len(durs)} | {statistics.median(durs):.0f} | {min(durs)} |")
    return "\n".join(rows)


def class_table(st, ops=1, title="per access class"):
    total = st.total_us() or 1
    rows = [f"| class ({title}) | accesses{' per op' if ops != 1 else ''} | time{' per op' if ops != 1 else ''}, us | share | median us | p90 us |",
            "|---|---|---|---|---|---|"]
    for cls, n in sorted(st.n.items(), key=lambda kv: -st.total_us(kv[0])):
        us = st.us.get(cls, [])
        rows.append(
            f"| {cls} | {num(n, ops)} | {sum(us) / ops:.0f} | {100 * sum(us) / total:.1f} % | "
            f"{statistics.median(us) if us else 0:.1f} | {pct(us, 0.9):.1f} |"
        )
    rows.append(f"| all | {num(sum(st.n.values()), ops)} | {total / ops:.0f} | 100 % | | |")
    return "\n".join(rows)


def projection(st, ops=1):
    med = {c: (statistics.median(v) if v else 0.0) for c, v in st.us.items()}
    base = st.total_us()
    out = ["| configuration | trapped accesses per op | time per op, us |", "|---|---|---|"]
    for label, gone in (
        ("as traced", set()),
        ("+ CARAXES_INTR_PAGE=1", INTR_PAGE),
        ("+ CARAXES_BAR2_MEMFD=1", BAR2_MEMFD),
        ("+ both", INTR_PAGE | BAR2_MEMFD),
    ):
        n = sum(v for c, v in st.n.items() if c not in gone)
        us = base - sum(st.total_us(c) for c in gone)
        out.append(f"| {label} | {num(n, ops)} | {us / ops:.0f} |")
    out.append("")
    out.append("Median per access by class, us: " + ", ".join(f"{c} {m:.1f}" for c, m in sorted(med.items())))
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("trace")
    ap.add_argument("--from", dest="start", type=int)
    ap.add_argument("--to", dest="end", type=int)
    ap.add_argument("--idle-ms", type=float, default=2.0)
    ap.add_argument("--gap-ms", type=float)
    ap.add_argument("--phases", help="LABEL:N,... assigned cyclically to the segments of --gap-ms")
    ap.add_argument("--skip", type=int, default=0, help="leading segments to leave out")
    ap.add_argument("--flush-page", default="0xfea000")
    ap.add_argument("--ops", type=float, default=1.0, help="divide the whole-window numbers by this")
    ap.add_argument("--json")
    args = ap.parse_args()

    whole, segments, rpc_done, invals, syncs, episodes, (t0, t1) = parse(args)
    out = {"window_us": [t0, t1]}
    print(f"# rpc-cost: {args.trace}\n")
    if t0 is None:
        print("no access lines in the window (access tracing off? CARAXES_TRACE_ACCESSES=1)")
        return 1
    print(f"Window {t0} .. {t1} us ({(t1 - t0) / 1000:.1f} ms), {sum(whole.n.values())} trapped accesses, "
          f"{whole.total_us() / 1000:.1f} ms attributed (gaps over {args.idle_ms} ms are idle).\n")
    print("## 1. Trapped accesses\n")
    print(class_table(whole, args.ops))
    print()

    print("## 2. GSP RPCs (device time from rx to the tx reply)\n")
    if rpc_done:
        print("| RPC | count | device us median | p90 | max | with a host RM call |")
        print("|---|---|---|---|---|---|")
        for key, v in sorted(rpc_done.items(), key=lambda kv: -sum(x[0] for x in kv[1])):
            us = [x[0] for x in v]
            print(f"| {key} | {len(v)} | {statistics.median(us):.0f} | {pct(us, 0.9):.0f} | {max(us)} | "
                  f"{sum(1 for x in v if x[1] > 0)} |")
    else:
        print("none in the window")
    print()

    print("## 3. MMU invalidates and shadow plans\n")
    if invals:
        print("| pdb | flags | count |\n|---|---|---|")
        for (pdb, fl), n in invals.most_common():
            print(f"| {pdb} | {fl} | {n} |")
    else:
        print("no MMU invalidate in the window")
    if syncs:
        cols = list(zip(*syncs))
        print(f"\n{len(syncs)} plan(s) applied by the worker: {sum(cols[1])} leaves unmapped, "
              f"{sum(cols[2])} mapped in {sum(cols[3])} RM map call(s); walk median {statistics.median(cols[4]):.0f} us "
              f"(server thread), apply median {statistics.median(cols[5]):.0f} us p90 {pct(cols[5], 0.9):.0f} us "
              f"total {sum(cols[5]) / 1000:.1f} ms (worker), queued median {statistics.median(cols[6]):.0f} us.")
    if RMCALL_HIST:
        print("\nThe shadow's host RM calls by kind (plans and VAS teardowns in the window):\n")
        print("| kind | calls | ms | us per call |\n|---|---|---|---|")
        for k, (n, us) in RMCALL_HIST.items():
            print(f"| {k} | {n} | {us / 1000:.1f} | {us / n if n else 0:.0f} |")
        print(f"| total | {sum(v[0] for v in RMCALL_HIST.values())} | "
              f"{sum(v[1] for v in RMCALL_HIST.values()) / 1000:.1f} | - |")
    print()
    if episodes:
        print("Episodes from one channel-VAS invalidate to the next (one guest mapping change each), "
              "by what they trapped on, most frequent first:\n")
        print(episode_table(episodes))
        print()

    if segments:
        print(f"## 4. Segments split at idle gaps of {args.gap_ms} ms or more\n")
        labels = []
        if args.phases:
            for part in args.phases.split(","):
                name, n = part.split(":")
                labels.append((name, float(n)))
        per = collections.OrderedDict()
        print("Segments: " + ", ".join(f"{(s1 - s0) / 1000:.1f} ms" for s0, s1, _ in segments[:40])
              + (" ..." if len(segments) > 40 else "") + "\n")
        for i, (s0, s1, st) in enumerate(segments[args.skip:]):
            if labels:
                name, n = labels[i % len(labels)]
            else:
                name, n = f"segment {i}", 1.0
            agg = per.setdefault(name, [Stats(), 0.0, 0])
            for c, v in st.n.items():
                agg[0].n[c] += v
            for c, v in st.us.items():
                agg[0].us[c].extend(v)
            agg[1] += n
            agg[2] += 1
        for name, (st, ops, nseg) in per.items():
            if ops == 0:
                continue
            print(f"### {name}: {nseg} segment(s), {ops:.0f} operation(s)\n")
            print(class_table(st, ops, name))
            print()
            print(projection(st, ops))
            print()
        out["segments"] = {k: {"ops": v[1], "accesses": dict(v[0].n), "us": v[0].total_us()} for k, v in per.items()}
    else:
        print("## 5. Projection with the existing switches (whole window)\n")
        print(projection(whole, args.ops))
        print()

    if args.json:
        out["classes"] = {c: {"n": n, "us": whole.total_us(c)} for c, n in whole.n.items()}
        out["rpcs"] = {k: [x[0] for x in v] for k, v in rpc_done.items()}
        with open(args.json, "w") as f:
            json.dump(out, f, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
