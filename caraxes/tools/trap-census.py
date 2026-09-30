#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Trap census from vfio-user-nvgpu profile dumps (CARAXES_PROFILE=1, src/profile.rs).

A workload is the dump after it minus the dump before it (--base); its wall time is the
difference of the dumps' "device time" headers. Every trapped access row is put into a
trap source (finer than rpc-cost.py's classes: the display registers, PTIMER, PRAMIN and
the falcons get their own), and every window row (fast path) is listed with its rate.

Per source:
  count, count/s,
  device us/trap (the device's wall time from the vfio-user message to its answer),
  guest-seen us/trap (profile.rs gap_ns/gaps: time since the previous trapped access ended,
  when under 2 ms; an upper bound of the vCPU's stall when accesses come back to back),
  device share (device ns / wall ns, in % of one core),
  vCPU share (guest-seen ns / wall ns, in % of one vCPU: trapped accesses are served one at
  a time over the vfio-user socket, so this is also the socket's busy share).

Usage:
  trap-census.py DUMP [--base DUMP] [--name NAME] [--top N] [--regs N] [--json OUT]
                  [--scope client:0x.. [--minus-scope client:0x..]]
A client scope is a time window without SIGUSR2 dumps: one guest RM client's lifetime, or the
difference of two clients that close together (the window between their opens).
"""
import argparse
import collections
import json
import lzma
import os
import re
import sys

COLS = ("scope", "from_us", "to_us", "kind", "key", "name", "count", "total_ns", "max_ns",
        "gap_ns", "gaps", "units", "status", "label")
NUMS = ("count", "total_ns", "max_ns", "gap_ns", "gaps", "units")
HEAD = re.compile(r"device time (\d+) us")

# BAR2 (vfio region 3) flush page (fb.rs FLUSH_PAGE, the memfd's trapped sync point).
FLUSH_PAGE = 0xFEA000


def open_any(path):
    if not os.path.exists(path):
        for ext in (".xz",):
            if os.path.exists(path + ext):
                path += ext
                break
    if path.endswith(".xz"):
        return lzma.open(path, "rt")
    if path.endswith(".zst"):
        import subprocess
        p = subprocess.Popen(["zstd", "-dc", path], stdout=subprocess.PIPE, text=True)
        return p.stdout
    return open(path)


def read_dump(path, scope="device"):
    """Rows of one scope; the wall time is the dump's device time for the device scope and
    the scope's own lifetime (from_us, to_us) for a client scope (profile.rs `Lifetime`:
    the device rows at its free minus those at its allocation)."""
    rows = {}
    us = None
    span = None
    with open_any(path) as f:
        for line in f:
            if line.startswith("#"):
                m = HEAD.search(line)
                if m:
                    us = int(m.group(1))
                continue
            p = line.rstrip("\n").split("\t")
            if p[0] != scope:
                continue
            r = dict(zip(COLS, p))
            for k in NUMS:
                r[k] = int(r[k])
            rows[(r["kind"], r["key"])] = r
            span = (int(r["from_us"]), int(r["to_us"]))
    if scope != "device":
        us = span[1] - span[0] if span else None
    return rows, us


def minus(rows, base):
    out = {}
    for k, r in rows.items():
        b = base.get(k)
        if b is None:
            out[k] = r
            continue
        d = dict(r)
        for n in NUMS:
            if n == "max_ns":
                continue
            d[n] = max(0, r[n] - b[n])
        if d["count"] or d["units"]:
            out[k] = d
    return out


def source(region, rw, page, reg, label):
    """The trap source of one access row."""
    if region == "bar3":
        if rw == "W":
            return "BAR2 write (page tables, inst blocks)"
        if page == FLUSH_PAGE:
            return "BAR2 flush-page read (memfd sync)"
        return "BAR2 read"
    if region == "bar1":
        return "BAR1 " + ("write" if rw == "W" else "read") + " (outside pool)"
    if region == "cfg":
        return "PCI config " + ("write" if rw == "W" else "read")
    if region == "rom":
        return "ROM read"
    if region != "bar0":
        return region
    off = reg if reg is not None else page
    if page == 0:
        if off in (0x160, 0x180):
            return "PMC_INTR_EN set/clear (RM lock)"
        return "PMC page 0, other"
    if page == 0x9000:
        return "PTIMER (0x9000)"
    if page == 0xB81000:
        return "VF interrupt tree (0xb81000)"
    if off in (0xB830A0, 0xB830A4, 0xB830B0):
        return "MMU invalidate (0xb830a0..b0)"
    if page == 0xB83000:
        return "VF MMU page, other"
    if off == 0xBB0090:
        return "usermode doorbell (0xbb0090)"
    if off in (0xBB0080, 0xBB0084):
        return "usermode TIME_0/1 (0xbb0080)"
    if page == 0xBB0000:
        return "usermode page, other"
    if page == 0x110000:
        if off in (0x110C00, 0x110C04):
            return "GSP queue head/tail (0x110c00)"
        return "GSP falcon, other (0x110000)"
    if page == 0x840000:
        return "SEC2 falcon (0x840000)"
    if page == 0x88000:
        if off == 0x88704:
            return "PCFG mirror 0x88704 (MSI re-arm)"
        return "PCFG mirror, other"
    if page == 0x611000:
        return "display FE intr page (0x611000)"
    if 0x680000 <= page < 0x688000:
        if rw == "W" and off == 0x680000:
            return "display core PUT (0x680000)"
        if rw == "R" and 0x680220 <= off < 0x680260:
            return "display core scanline/blank PIO (0x680220)"
        return "display core USERD, other"
    if 0x688000 <= page < 0x690000:
        return "display core ARMED (0x688000)"
    if 0x690000 <= page < 0x6D8000:
        kind = "window" if page < 0x6B0000 else "window-imm"
        within = off & 0xFFF
        if rw == "W" and within == 0:
            return f"display {kind} PUT (0x{0x690000 if kind == 'window' else 0x6B0000:x}+)"
        if rw == "R" and within >= 0x800:
            return f"display {kind} ARMED reads"
        if rw == "R" and within == 4:
            return f"display {kind} GET reads"
        return f"display {kind} USERD, other"
    if 0x6D8000 <= page < 0x6E0000:
        return "display cursor USERD (0x6d8000)"
    if 0x610000 <= page < 0x680000:
        return "display PDISP, other"
    if 0x700000 <= page < 0x800000:
        return "PRAMIN window (0x700000)"
    if 0x300000 <= page < 0x400000:
        return "PROM window (0x300000)"
    if 0x100000 <= page < 0x101000:
        return "PFB (0x100000)"
    return "BAR0 other"


def parse_key(key):
    region, rw, page, reg = key.split(":")
    page = int(page, 16)
    reg = None if reg == "-" else int(reg, 16)
    return region, rw, page, reg


def census(rows, wall_us):
    wall_s = wall_us / 1e6
    src = collections.OrderedDict()
    regs = collections.Counter()
    regns = collections.Counter()
    for (kind, key), r in rows.items():
        if kind != "access" or not r["count"]:
            continue
        region, rw, page, reg = parse_key(key)
        s = source(region, rw, page, reg, r["label"])
        a = src.setdefault(s, collections.Counter())
        for n in ("count", "total_ns", "gap_ns", "gaps"):
            a[n] += r[n]
        rk = f"{region}:{rw}:{(reg if reg is not None else page):#x}"
        if r["label"] not in ("", "-"):
            rk += " " + r["label"]
        regs[rk] += r["count"]
        regns[rk] += r["total_ns"]
    # The trapped accesses the routes serve without the device mutex (profile.rs `routed`):
    # window rows, counts and device nanoseconds; no guest-seen time.
    for (kind, key), r in rows.items():
        if kind != "window" or not key.startswith("routed trap: ") or not r["count"]:
            continue
        what = key[len("routed trap: "):]
        ns = rows.get(("window", "routed trap ns: " + what), {}).get("count", 0)
        a = src.setdefault(f"routed: {what[:-2]} ({what[-1]})", collections.Counter())
        a["count"] += r["count"]
        a["total_ns"] += ns
        regs[f"routed {what}"] += r["count"]
        regns[f"routed {what}"] += ns
    out = []
    tot = collections.Counter()
    for s, a in src.items():
        for n in a:
            tot[n] += a[n]
        out.append(row(s, a, wall_s))
    out.sort(key=lambda x: -x["count"])
    windows = []
    for (kind, key), r in rows.items():
        if kind != "window" or not (r["count"] or r["units"]) or key.startswith("routed trap"):
            continue
        windows.append({
            "window": key, "count": r["count"], "per_s": r["count"] / wall_s,
            "units": r["units"], "units_per_s": r["units"] / wall_s,
            "dev_us": r["total_ns"] / r["count"] / 1e3 if r["count"] else 0.0,
            "dev_share_pct": 100.0 * r["total_ns"] / (wall_s * 1e9),
        })
    windows.sort(key=lambda x: -max(x["count"], x["units"]))
    return {
        "wall_s": wall_s,
        "total": row("all trapped accesses", tot, wall_s),
        "sources": out,
        "regs": [(k, v, v / wall_s, regns[k] / v / 1e3) for k, v in regs.most_common()],
        "windows": windows,
    }


def row(name, a, wall_s):
    c = a["count"]
    return {
        "source": name,
        "count": c,
        "per_s": c / wall_s if wall_s else 0.0,
        "dev_us": a["total_ns"] / c / 1e3 if c else 0.0,
        "seen_us": a["gap_ns"] / a["gaps"] / 1e3 if a["gaps"] else 0.0,
        "dev_share_pct": 100.0 * a["total_ns"] / (wall_s * 1e9) if wall_s else 0.0,
        "vcpu_share_pct": 100.0 * a["gap_ns"] / (wall_s * 1e9) if wall_s else 0.0,
    }


def fmt(res, name, top, nregs):
    o = []
    t = res["total"]
    o.append(f"## {name}: {res['wall_s']:.1f} s, {t['count']} traps, {t['per_s']:.0f}/s, "
             f"device {t['dev_share_pct']:.1f} % of a core, guest-seen {t['vcpu_share_pct']:.1f} % of a vCPU")
    o.append("")
    o.append("| source | count | /s | dev us | seen us | dev % | vCPU % |")
    o.append("|---|---:|---:|---:|---:|---:|---:|")
    for r in res["sources"][:top]:
        o.append(f"| {r['source']} | {r['count']} | {r['per_s']:.1f} | {r['dev_us']:.1f} | "
                 f"{r['seen_us']:.1f} | {r['dev_share_pct']:.2f} | {r['vcpu_share_pct']:.2f} |")
    if nregs:
        o.append("")
        o.append("| register | count | /s | dev us |")
        o.append("|---|---:|---:|---:|")
        for k, c, ps, us in res["regs"][:nregs]:
            o.append(f"| {k} | {c} | {ps:.1f} | {us:.1f} |")
    o.append("")
    o.append("| window (fast path) | count | /s | units/s | dev us | dev % |")
    o.append("|---|---:|---:|---:|---:|---:|")
    for w in res["windows"][:top]:
        o.append(f"| {w['window']} | {w['count']} | {w['per_s']:.1f} | {w['units_per_s']:.1f} | "
                 f"{w['dev_us']:.1f} | {w['dev_share_pct']:.2f} |")
    return "\n".join(o)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dump")
    ap.add_argument("--base")
    ap.add_argument("--scope", default="device",
                    help="a client scope (client:0x...) instead of the device: its lifetime's rows")
    ap.add_argument("--minus-scope",
                    help="a later client scope of the same dump that outlives --scope's end by "
                         "little: the rows between the two scopes' starts")
    ap.add_argument("--name")
    ap.add_argument("--top", type=int, default=30)
    ap.add_argument("--regs", type=int, default=25)
    ap.add_argument("--json")
    args = ap.parse_args()
    rows, us = read_dump(args.dump, args.scope)
    if us is None:
        sys.exit(f"{args.dump}: no device time header or no scope {args.scope}")
    if args.minus_scope:
        later, lus = read_dump(args.dump, args.minus_scope)
        rows = minus(rows, later)
        us -= lus
    if args.base:
        base, bus = read_dump(args.base)
        rows = minus(rows, base)
        us -= bus
    res = census(rows, us)
    name = args.name or args.dump
    print(fmt(res, name, args.top, args.regs))
    if args.json:
        res["name"] = name
        with open(args.json, "w") as f:
            json.dump(res, f, indent=1)


if __name__ == "__main__":
    main()
