#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""The trap census per CUDA operation (docs/caraxes/optimization-backlog.md).

Reads a vfio-user-nvgpu device trace (plain or .xz, access tracing on) of census-cu runs.
census-cu brackets every repetition of an operation with two writes to NV_PBUS_SW_SCRATCH(30)
(BAR0 0x1478, value 0xce000000 | op << 12 | rep << 1 | end). Everything the trace holds
between the two markers of one repetition belongs to that repetition: the trapped accesses
(also those another vCPU makes meanwhile, such as interrupt servicing), the GSP RPCs, the
shadow plans and the other device events. Per operation, divided by the repetitions:

  1. trapped accesses by class (the classes of rpc-cost.py), with the time the guest spent
     on them (the gap to the previous access line, as rpc-cost.py attributes it);
  2. trapped accesses by BAR, page and register (name as the trace gives it);
  3. which of them an existing window would take (--windows: what the configuration had on)
     and what stays;
  4. device work: GSP RPCs (count, device time rx to tx, how many made a host RM call),
     events that name a host RM call ("[host"), shadow plans (RM map and unmap calls, walk and
     worker apply time), forwarded doorbells, doorbell eventfd rings, MSIs, MMU invalidates.

The marker writes themselves are not counted. The window time is the device's time from the
start marker to the end marker.

Usage: census-table.py TRACE [--names OPS.txt] [--json OUT] [--detail N] [--ops id,id] [--pool-mib 64]
  OPS.txt: census-cu stdout ("OP name=... id=... reps=..." lines) for the names and the guest
  wall times; several files may be given comma-separated.
"""
import argparse
import collections
import importlib.util
import json
import os
import re
import statistics
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
_spec = importlib.util.spec_from_file_location("rpccost", os.path.join(HERE, "rpc-cost.py"))
rpccost = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(rpccost)

ACCESS = rpccost.ACCESS
EVENT = rpccost.EVENT
RX, RX_CLASS, RX_CMD, TX = rpccost.RX, rpccost.RX_CLASS, rpccost.RX_CMD, rpccost.TX
SYNC = rpccost.SYNC
MARK_OFF = 0x1478
POOL = [64 << 20]  # the BAR1 pool's size (--pool-mib)
OPNAME = re.compile(r"OP name=(\S+) id=(\d+) reps=(\d+) per_rep=(\d+) med_us=([\d.]+) min_us=([\d.]+) max_us=([\d.]+)")

# What each existing window takes (in the configuration that has it on):
#   intr page: BAR0 page 0 and the VF interrupt tree page 0xb81000 (intr-page.md §2);
#   display page: BAR0 0x611000 (intr-page.md §10);
#   token pool: the usermode doorbell 0xbb0090 after a channel's first ring (doorbell-token-pool.md);
#   BAR1 pool / P21 area: BAR1 [0, pool) (bar1-pool.md), 64 MiB by default (--pool-mib);
#   BAR2 memfd: BAR2 except the flush page 0xfea000 (bar2-memfd.md §3).
WINDOWS = {
    "intr": lambda r, off: r == "bar0" and (off & ~0xFFF) in (0x0, 0xB81000),
    "display": lambda r, off: r == "bar0" and (off & ~0xFFF) == 0x611000,
    "tokens": lambda r, off: r == "bar0" and off == 0xBB0090,
    "bar1pool": lambda r, off: r == "bar1" and off < POOL[0],
    "bar2memfd": lambda r, off: r == "bar3" and (off & ~0xFFF) != 0xFEA000,
}


def norm_event(text):
    t = re.sub(r"^boot: ", "", text)
    t = re.sub(r"0x[0-9a-f]+", "X", t)
    t = re.sub(r"\d+", "N", t)
    return t[:90]


class Op:
    def __init__(self):
        self.reps = 0
        self.win_us = []
        self.cls = collections.Counter()
        self.cls_us = collections.defaultdict(float)
        self.reg = collections.Counter()
        self.rpc = collections.defaultdict(list)  # key -> [(dev_us, host)]
        self.ev = collections.Counter()
        self.host_ev = collections.Counter()
        self.sync = []  # (unmapped, mapped, calls, walk, apply)
        self.window_cov = collections.Counter()
        self.total = 0


def region_name(r):
    return {"bar0": "BAR0", "bar1": "BAR1", "bar3": "BAR2", "cfg": "PCI config"}.get(r, r)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("trace")
    ap.add_argument("--names", default="")
    ap.add_argument("--json")
    ap.add_argument("--detail", type=int, default=12, help="registers listed per operation")
    ap.add_argument("--ops", default="", help="only these op ids")
    ap.add_argument("--flush-page", default="0xfea000")
    ap.add_argument("--pool-mib", type=int, default=64, help="BAR1 pool size (CARAXES_BAR1_POOL_MIB)")
    a = ap.parse_args()
    POOL[0] = a.pool_mib << 20
    flush_page = int(a.flush_page, 16) & ~0xFFF

    names, guest = {}, collections.defaultdict(list)
    for fn in [x for x in a.names.split(",") if x]:
        with open(fn, errors="replace") as f:
            for ln in f:
                m = OPNAME.search(ln)
                if m:
                    names[int(m.group(2))] = m.group(1)
                    guest[int(m.group(2))].append((float(m.group(5)), int(m.group(3)), int(m.group(4))))
    only = {int(x) for x in a.ops.split(",") if x}

    ops = collections.defaultdict(Op)
    cur = None  # (op, rep, t0)
    last_t = None
    rpcs, open_seq = {}, []
    with rpccost.open_trace(a.trace) as f:
        for line in f:
            m = ACCESS.match(line)
            if m:
                t = int(m.group(1))
                region, rw, off = m.group(3), m.group(2), int(m.group(4), 16)
                if region == "bar0" and rw == "W" and off == MARK_OFF:
                    v = int(m.group(6), 16)
                    if v >> 24 == 0xCE:
                        op, rep, end = (v >> 12) & 0xFFF, (v >> 1) & 0x7FF, v & 1
                        if not end:
                            cur = (op, rep, t)
                            last_t = t
                            rpcs.clear()
                            open_seq.clear()
                        elif cur and cur[0] == op and cur[1] == rep:
                            o = ops[op]
                            o.reps += 1
                            o.win_us.append(t - cur[2])
                            cur = None
                        continue
                if cur is None or (only and cur[0] not in only):
                    continue
                o = ops[cur[0]]
                cls = rpccost.classify(region, rw, off, m.group(7), flush_page)
                o.cls[cls] += 1
                o.total += 1
                dt = t - last_t if last_t is not None else 0
                if dt <= 2000:
                    o.cls_us[cls] += dt
                last_t = t
                name = (m.group(7).split() or [""])[0]
                if region == "bar0":
                    key = (region_name(region), f"{off & ~0xFFF:#08x}", rw, f"{off:#08x} {name}")
                elif region == "bar3":
                    key = (region_name(region), "flush page" if (off & ~0xFFF) == flush_page else "other pages", rw, "")
                elif region == "bar1":
                    key = (region_name(region), f"[0, {a.pool_mib} MiB)" if off < POOL[0] else f"above {a.pool_mib} MiB", rw,
                           "USERD GPPut" if (off & 0x1FF) == 0x8C else "")
                else:
                    key = (region_name(region), "", rw, f"{off:#x}")
                o.reg[key] += 1
                for w, f_ in WINDOWS.items():
                    if f_(region, off):
                        o.window_cov[w] += 1
                continue
            m = EVENT.match(line)
            if not m or cur is None or (only and cur[0] not in only):
                continue
            t, text = int(m.group(1)), m.group(2)
            o = ops[cur[0]]
            if "rx seq" in text:
                r = RX.search(text)
                if r:
                    func, seq = r.group(1), r.group(2)
                    key = func
                    c, d = RX_CLASS.search(text), RX_CMD.search(text)
                    if func == "GSP_RM_ALLOC" and c:
                        key = f"GSP_RM_ALLOC class {c.group(1)}"
                    elif func == "GSP_RM_CONTROL" and d:
                        key = f"GSP_RM_CONTROL cmd {d.group(1)}"
                    rpcs[seq] = [key, t, 0]
                    open_seq.append(seq)
                continue
            if "tx seq" in text:
                r = TX.search(text)
                if r and r.group(2) in rpcs:
                    key, t0, host = rpcs.pop(r.group(2))
                    if r.group(2) in open_seq:
                        open_seq.remove(r.group(2))
                    o.rpc[key].append((t - t0, host))
                continue
            if "[host" in text:
                for s in open_seq:
                    rpcs[s][2] += 1
                o.host_ev[norm_event(text)] += 1
            s = SYNC.search(text)
            if s:
                g = [int(x) if x is not None else 0 for x in s.groups()]
                o.sync.append((g[1], g[2], g[3], g[4], g[5]))
            if "doorbell token" in text and "(eventfd)" in text:
                o.ev["doorbell eventfd rings (token pool)"] += 1
            elif "doorbell: guest chid" in text:
                o.ev["host doorbells written (forwarded)"] += 1
            elif "host notifier" in text:
                o.ev["host notifier events" + (" -> MSI" if ", MSI" in text else " (no MSI)")] += 1
            elif "mmu invalidate:" in text:
                o.ev["guest MMU invalidates"] += 1
            elif "bar1 pool" in text or "bar1pool" in text:
                o.ev["BAR1 pool events"] += 1
            else:
                o.ev["other events"] += 1

    out = {}
    print(f"# census-table: {a.trace}\n")
    print("| op | name | reps | trapped per rep | window per rep, us (device) | guest med, us |")
    print("|---|---|---|---|---|---|")
    for op in sorted(ops):
        o = ops[op]
        if not o.reps:
            continue
        g = guest.get(op)
        gm = statistics.median([x[0] for x in g]) if g else None
        print(f"| {op} | {names.get(op, '?')} | {o.reps} | {o.total / o.reps:.1f} | "
              f"{statistics.median(o.win_us):.0f} | {'' if gm is None else f'{gm:.1f}'} |")
    print()
    for op in sorted(ops):
        o = ops[op]
        if not o.reps:
            continue
        n = o.reps
        print(f"## op {op} {names.get(op, '?')}: {n} repetition(s), {o.total / n:.1f} trapped accesses per repetition, "
              f"window median {statistics.median(o.win_us):.0f} us\n")
        if o.cls:
            print("| class | per rep | us per rep |\n|---|---|---|")
            for c, v in sorted(o.cls.items(), key=lambda kv: -kv[1]):
                print(f"| {c} | {v / n:.1f} | {o.cls_us[c] / n:.0f} |")
            print()
            print("| BAR | page | R/W | register | per rep |\n|---|---|---|---|---|")
            for (bar, page, rw, reg), v in o.reg.most_common(a.detail):
                print(f"| {bar} | {page} | {rw} | {reg} | {v / n:.2f} |")
            rest = sum(o.reg.values()) - sum(v for _, v in o.reg.most_common(a.detail))
            if rest:
                print(f"| (rest) | | | | {rest / n:.2f} |")
            print()
            cov = ", ".join(f"{w} {v / n:.1f}" for w, v in sorted(o.window_cov.items())) or "none"
            print(f"On pages an existing window serves (trapped here, so that window was off or the client did not map it): {cov}\n")
        dev = []
        if o.rpc:
            nr = sum(len(v) for v in o.rpc.values())
            us = sum(x[0] for v in o.rpc.values() for x in v)
            hr = sum(1 for v in o.rpc.values() for x in v if x[1])
            dev.append(f"GSP RPCs {nr / n:.1f} per rep ({us / n:.0f} us device time, {hr / n:.1f} with a host RM call): "
                       + "; ".join(f"{k} x{len(v) / n:.1f}" for k, v in sorted(o.rpc.items(), key=lambda kv: -len(kv[1]))[:8]))
        if o.sync:
            cols = list(zip(*o.sync))
            dev.append(f"shadow plans {len(o.sync) / n:.1f} per rep: {sum(cols[1]) / n:.0f} leaves mapped and {sum(cols[0]) / n:.0f} unmapped in "
                       f"{sum(cols[2]) / n:.1f} RM map call(s), walk {sum(cols[3]) / n:.0f} us, worker apply {sum(cols[4]) / n:.0f} us per rep")
        if o.host_ev:
            dev.append(f"events naming a host RM call {sum(o.host_ev.values()) / n:.1f} per rep: "
                       + "; ".join(f"{k} x{v / n:.1f}" for k, v in o.host_ev.most_common(6)))
        if o.ev:
            dev.append("device events per rep: " + "; ".join(f"{k} {v / n:.1f}" for k, v in sorted(o.ev.items())))
        for d in dev:
            print(f"- {d}")
        print()
        out[op] = {
            "name": names.get(op), "reps": n, "traps_per_rep": o.total / n,
            "window_us_med": statistics.median(o.win_us),
            "guest_us_med": (statistics.median([x[0] for x in guest[op]]) if guest.get(op) else None),
            "classes": {c: v / n for c, v in o.cls.items()},
            "class_us": {c: v / n for c, v in o.cls_us.items()},
            "registers": {" ".join(k): v / n for k, v in o.reg.items()},
            "windows": {w: v / n for w, v in o.window_cov.items()},
            "rpcs": {k: [len(v) / n, sum(x[0] for x in v) / n, sum(1 for x in v if x[1]) / n] for k, v in o.rpc.items()},
            "host_events": sum(o.host_ev.values()) / n,
            "syncs": len(o.sync) / n,
            "sync_calls": (sum(x[2] for x in o.sync) / n) if o.sync else 0,
            "sync_apply_us": (sum(x[4] for x in o.sync) / n) if o.sync else 0,
            "events": {k: v / n for k, v in o.ev.items()},
        }
    if a.json:
        with open(a.json, "w") as f:
            json.dump(out, f, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
