#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
"""The per-frame trap table of a compositor (docs/caraxes/display-e.md, series F).

Reads a device trace window (plain or .xz) of vfio-user-nvgpu with access tracing on and
counts, over a 10 s window of steady frames, what the guest still traps per presented frame:
BAR0 writes (usermode doorbell, MMU invalidate, interrupt enables, the rest) and reads,
BAR1 writes (USERD GPPut vs other: GPFIFO entries, push data), BAR2 writes, and the device
work the frames cost (forwarded doorbells, host events and MSIs, shadow plans with their
server-thread walk and worker apply time).

The window ends 1 s before the last latch of a scanout surface in the trace (the client
stops, the compositor idles) and is --secs long. A presented frame is one latch of a
window with a surface (display: window N latch ... surface fb ...).

Usage: trap-table.py <device-trace.txt[.xz]> [--secs 10] [--json out.json] [--pages] [--regs]

--pages adds the BAR0 accesses per 4 KiB page and per frame (docs/caraxes/intr-page.md §1:
with CARAXES_INTR_PAGE=1 pages 0x000000 and 0xb81000 are memory and drop out).

--regs adds every trapped register (BAR, offset, read or write) per frame with its name, and
the frame-time spread of the main window's latches (docs/caraxes/intr-page.md §11).
"""
import argparse
import collections
import json
import lzma
import re
import statistics
import sys

LATCH = re.compile(r"display: window (\d+) latch \d+ at tick \d+: surface (\S+)")
SYNC = re.compile(r"shadow: sync .*?walk (\d+) us, apply (\d+) us(?: after (\d+) us in the worker's queue)?, sync (\d+) us")


def lines(path):
    op = lzma.open if path.endswith(".xz") else open
    with op(path, "rt", errors="replace") as f:
        yield from f


def classify_bar0_write(off, name):
    if "DOORBELL" in name or off == 0xBB0090:
        return "doorbell (usermode NOTIFY_CHANNEL_PENDING)"
    if "MMU_INVALIDATE" in name:
        return "MMU invalidate (VF PRIV_MMU_INVALIDATE, 3 writes each)"
    if "INTR" in name:
        return "interrupt enable/clear (VF CPU_INTR)"
    if off in (0x160, 0x180):
        return "0x160/0x180 (latched)"
    if name == "NV_PCFG":
        return "PCFG mirror 0x88704"
    return f"other {off:#x} {name}".strip()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("trace")
    ap.add_argument("--secs", type=float, default=10.0)
    ap.add_argument("--json")
    ap.add_argument("--pages", action="store_true", help="BAR0 accesses per 4 KiB page")
    ap.add_argument("--regs", action="store_true", help="every trapped register per frame")
    a = ap.parse_args()

    rows = []
    last_latch = None
    for ln in lines(a.trace):
        p = ln.split(None, 6)
        if len(p) < 3 or not p[0].isdigit():
            continue
        ts = int(p[0])
        rows.append((ts, ln))
        if p[1] == "E":
            m = LATCH.search(ln)
            if m and m.group(2) != "none":
                last_latch = ts
    if last_latch is None:
        sys.exit("no scanout latch in the trace")
    end = last_latch - 1_000_000
    start = end - int(a.secs * 1e6)

    frames = collections.Counter()
    latch_ts = collections.defaultdict(list)
    regs = collections.Counter()
    reg_name = {}
    w0 = collections.Counter()
    r0 = collections.Counter()
    w1 = collections.Counter()
    w1_bytes = 0
    w2 = collections.Counter()
    r1 = 0
    pages = collections.Counter()
    r2 = 0
    ev = collections.Counter()
    walk, apply_, queued = [], [], []
    for ts, ln in rows:
        if ts < start or ts >= end:
            continue
        p = ln.split(None, 6)
        kind = p[1]
        if kind == "E":
            m = LATCH.search(ln)
            if m and m.group(2) != "none":
                frames[int(m.group(1))] += 1
                latch_ts[int(m.group(1))].append(ts)
            if "doorbell: guest" in ln:
                ev["forwarded doorbells (host doorbell rung)"] += 1
            elif "event: host notifier" in ln:
                ev["host notifier events" + (" -> MSI" if ", MSI" in ln else " (no MSI)")] += 1
            elif " msi: " in ln:
                ev["MSIs signalled"] += 1
            elif "mmu invalidate:" in ln:
                ev["guest MMU invalidates seen"] += 1
            elif "display channel" in ln:
                ev["display channel kicks"] += 1
            m = SYNC.search(ln)
            if m:
                walk.append(int(m.group(1)))
                apply_.append(int(m.group(2)))
                if m.group(3):
                    queued.append(int(m.group(3)))
            continue
        if len(p) < 5:
            continue
        region, off, size = p[2], int(p[3], 16), int(p[4])
        name = p[6].split()[0] if len(p) > 6 and p[6].strip() else ""
        # p[5] is the value; p[6] the register name / fb annotation
        name = ln.split(None, 6)[6].strip() if len(p) > 6 else ""
        key = (region, kind, off)
        regs[key] += 1
        if key not in reg_name:
            reg_name[key] = name.split("(")[0].strip()
        if region == "bar0":
            pages[(off & ~0xFFF, kind)] += 1
        if kind == "W" and region == "bar0":
            w0[classify_bar0_write(off, name)] += 1
        elif kind == "R" and region == "bar0":
            r0[name.split("(")[0] or f"{off:#x}"] += 1
        elif kind == "W" and region == "bar1":
            w1["USERD GPPut (offset 0x8c)" if (off & 0x1FF) == 0x8C else "other (GPFIFO entries, push data)"] += 1
            w1_bytes += size
        elif kind == "R" and region == "bar1":
            r1 += 1
        elif kind == "W" and region == "bar3":
            w2[f"page {off & ~0xFFF:#x}"] += 1
        elif kind == "R" and region == "bar3":
            r2 += 1

    nf = max(frames.values()) if frames else 0
    main_win = max(frames, key=frames.get) if frames else None

    def per(n):
        return f"{n / nf:.2f}" if nf else "-"

    out = {"window_us": [start, end], "frames": dict(frames), "main_window": main_win, "rows": []}
    print(f"window {a.secs:.0f} s [{start}, {end}) us; presented frames (latches with a surface): "
          + ", ".join(f"window {k}: {v}" for k, v in sorted(frames.items()))
          + f"; per-frame base: window {main_win} = {nf} ({nf / a.secs:.2f} Hz)")
    print(f"{'what':62} {'count':>8} {'per frame':>10}")

    def row(what, n):
        print(f"{what:62} {n:>8} {per(n):>10}")
        out["rows"].append({"what": what, "count": n, "per_frame": (n / nf if nf else None)})

    row("BAR0 writes, all", sum(w0.values()))
    for k, v in w0.most_common(8):
        row("  " + k, v)
    row("BAR0 reads, all", sum(r0.values()))
    for k, v in r0.most_common(5):
        row("  " + k, v)
    row("BAR1 writes, all", sum(w1.values()))
    for k, v in w1.most_common():
        row("  " + k, v)
    row("BAR1 reads", r1)
    row("BAR2 writes, all", sum(w2.values()))
    for k, v in w2.most_common(3):
        row("  " + k, v)
    row("BAR2 reads", r2)
    traps = sum(w0.values()) + sum(r0.values()) + sum(w1.values()) + r1 + sum(w2.values()) + r2
    row("all trapped accesses", traps)
    for k, v in sorted(ev.items()):
        row(k, v)
    row("shadow plans applied (sync lines)", len(walk))
    if a.pages:
        for (pg, kind), n in sorted(pages.items(), key=lambda x: -x[1])[:12]:
            row(f"BAR0 page {pg:#08x} {'reads' if kind == 'R' else 'writes'}", n)
    if a.regs:
        bars = {"bar0": "BAR0", "bar1": "BAR1", "bar3": "BAR2"}
        print("per register (BAR, access, offset, name):")
        for (region, kind, off), n in regs.most_common(40):
            what = f"  {bars.get(region, region)} {kind} {off:#010x} {reg_name[(region, kind, off)][:34]}"
            row(what, n)
        ts = latch_ts.get(main_win, [])
        if len(ts) > 2:
            d = [(b - x) / 1000 for x, b in zip(ts, ts[1:])]
            print(f"latch intervals of window {main_win}: {len(d)}, mean {statistics.mean(d):.3f} ms, "
                  f"sd {statistics.pstdev(d):.3f} ms, min {min(d):.3f} ms, max {max(d):.3f} ms, "
                  f"over 20 ms {sum(1 for x in d if x > 20)}")
            out["latch_interval_ms"] = d
    if walk:
        tot_w, tot_a = sum(walk), sum(apply_)
        print(f"  server-thread walk per plan: p50 {statistics.median(walk)} us, max {max(walk)} us, "
              f"sum {tot_w} us ({tot_w / nf:.0f} us per frame)" if nf else "")
        print(f"  worker apply per plan: p50 {statistics.median(apply_)} us, max {max(apply_)} us, "
              f"sum {tot_a} us ({tot_a / nf:.0f} us per frame)" if nf else "")
        if queued:
            print(f"  queued before apply: p50 {statistics.median(queued)} us, max {max(queued)} us")
        out.update(walk_us=walk, apply_us=apply_, queued_us=queued)
    if a.json:
        with open(a.json, "w") as f:
            json.dump(out, f, indent=1)


if __name__ == "__main__":
    main()
