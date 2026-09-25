#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
"""Rank the rows of a vfio-user-nvgpu profile (CARAXES_PROFILE=1, src/profile.rs) by cost.

The device dumps its profile as TSV on SIGUSR2 (<prefix>.<n>.tsv) and at exit
(<prefix>.exit.tsv). A workload's profile is the dump after it minus the dump before it
(--base); a guest process's is its client scope (--scope client:0x...).

Cost is count times mean latency, that is the row's total time:
  - trapped accesses: the guest-seen time (the time since the previous trapped access ended,
    when under 2 ms: rpc-cost.py's attribution), --cost device for the device's own time;
  - controls, allocations, RPCs, host RM calls, shadow calls, window work: device wall time.

Every row is joined with what explains it:
  - controls with caraxes/abi/gsp-615.71.09/controls.tsv (name, category, provenance,
    answered_by, privilege, route) and the device's own answer label;
  - allocations with classes.tsv, RPCs and events with rpc.tsv and events.tsv;
  - accesses with rpc-cost.py's access class and the module that answers it;
  - the handler module (a file under crates/vfio-user-nvgpu/src).

The ABI tables default to gsp-615.71.09; CARAXES_PROFILE_ABI=gsp-610.57.04 joins with the 610
guest's tables instead.

Usage:
  profile-rank.py DUMP [--base DUMP] [--scope device|clients|client:0x..] [--kind K,...]
                  [--top N] [--cost guest|device] [--tsv] [--json OUT]
                  [--seen-in DUMP[:BASE] ...]   mark controls that none of these sent
  profile-rank.py DUMP --clients     list the client lifetimes in the dump
"""
import argparse
import collections
import importlib.util
import json
import lzma
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
ABI = os.path.join(ROOT, "caraxes", "abi", os.environ.get("CARAXES_PROFILE_ABI", "gsp-615.71.09"))
SRC = "crates/vfio-user-nvgpu/src"

COLS = ("scope", "from_us", "to_us", "kind", "key", "name", "count", "total_ns", "max_ns",
        "gap_ns", "gaps", "units", "status", "label")
NUMS = ("count", "total_ns", "max_ns", "gap_ns", "gaps", "units")

# The module that answers, per answered_by prefix of controls.tsv (gsp-surface.py's reading).
MODULE_OF_ANSWER = {
    "ctrl": "ctrl.rs", "public": "public.rs", "display": "display/mod.rs", "nvml": "nvml.rs",
    "uvm": "uvm.rs", "kgr": "kgr.rs", "cuda": "cuda.rs",
}
# The module that answers a trapped access, per rpc-cost.py access class.
MODULE_OF_CLASS = {
    "bar2-write": "fb.rs (BAR2 page tables), bar2area.rs with the memfd",
    "bar2-flush-read": "fb.rs, bar2area.rs (the memfd sync point)",
    "bar2-read (not the flush page)": "fb.rs",
    "bar1-write": "fb.rs, bar1pool.rs",
    "bar1-read": "fb.rs, bar1pool.rs",
    "mmu-invalidate": "invalidate.rs, shadow.rs (plans)",
    "gsp-queue-doorbell": "boot.rs (service_queues), msgq.rs",
    "gsp-falcon": "falcon.rs, gspboot.rs",
    "usermode-doorbell": "channel.rs, tokens.rs, mirror.rs",
    "usermode page, other": "channel.rs (TIME_0/1, usermode page)",
    "pmc-intr-en (RM lock)": "bar0.rs, intr.rs",
    "pmc page 0, other": "bar0.rs",
    "vf-intr-tree": "intr.rs",
    "pcfg-mirror (intr re-arm)": "bar0.rs, config.rs",
    "pci-config": "config.rs",
    "display-intr": "display/mod.rs",
    "bar0-other": "bar0.rs, boot.rs",
    "rom": "device.rs (ROM region)",
}


def load_rpc_cost():
    spec = importlib.util.spec_from_file_location("rpc_cost", os.path.join(HERE, "rpc-cost.py"))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def read_tsv_table(path, key):
    out = {}
    if not os.path.exists(path):
        return out
    with open(path) as f:
        header = None
        for line in f:
            if line.startswith("#"):
                continue
            parts = line.rstrip("\n").split("\t")
            if header is None:
                header = parts
                continue
            row = dict(zip(header, parts))
            try:
                out[int(row[key], 0)] = row
            except (KeyError, ValueError):
                pass
    return out


def read_dump(path):
    """Rows by (scope, kind, key) and the header comment."""
    rows = collections.OrderedDict()
    head = ""
    if not os.path.exists(path) and os.path.exists(path + ".xz"):
        path += ".xz"
    with (lzma.open(path, "rt") if path.endswith(".xz") else open(path)) as f:
        for line in f:
            if line.startswith("#"):
                head += line
                continue
            parts = line.rstrip("\n").split("\t")
            if parts[0] == "scope":
                continue
            r = dict(zip(COLS, parts))
            for k in NUMS:
                r[k] = int(r[k])
            for k in ("from_us", "to_us"):
                r[k] = int(r[k])
            rows[(r["scope"], r["kind"], r["key"])] = r
    return rows, head


def status_map(s):
    if s in ("", "-"):
        return {}
    out = {}
    for part in s.split(","):
        k, v = part.split("=")
        out[k] = int(v)
    return out


def status_str(m):
    return ",".join(f"{k}={v}" for k, v in m.items() if v > 0) or "-"


def minus(rows, base):
    """Device-scope rows of `rows` minus those of `base` (a workload between two dumps)."""
    out = collections.OrderedDict()
    for (scope, kind, key), r in rows.items():
        if scope != "device":
            continue
        b = base.get((scope, kind, key))
        d = dict(r)
        if b is not None:
            for k in NUMS:
                if k != "max_ns":
                    d[k] = r[k] - b[k]
            sm, bm = status_map(r["status"]), status_map(b["status"])
            d["status"] = status_str({k: v - bm.get(k, 0) for k, v in sm.items()})
            if kind == "window" and d["count"] == 0 and d["units"] == 0:
                continue
        if d["count"] <= 0 and d["units"] <= 0:
            continue
        out[(scope, kind, key)] = d
    return out


def select(rows, scope):
    if scope == "device":
        return [r for (s, _, _), r in rows.items() if s == "device"]
    if scope == "clients":
        agg = collections.OrderedDict()
        for (s, kind, key), r in rows.items():
            if not s.startswith("client:"):
                continue
            a = agg.setdefault((kind, key), dict(r, scope="clients"))
            if a is not r:
                for k in NUMS:
                    a[k] = a[k] + r[k] if k != "max_ns" else max(a[k], r[k])
        return list(agg.values())
    return [r for (s, _, _), r in rows.items() if s == scope or s == scope + ":open"]


class Joiner:
    def __init__(self):
        self.controls = read_tsv_table(os.path.join(ABI, "controls.tsv"), "id")
        self.classes = read_tsv_table(os.path.join(ABI, "classes.tsv"), "id")
        self.rpcs = read_tsv_table(os.path.join(ABI, "rpc.tsv"), "num")
        self.events = read_tsv_table(os.path.join(ABI, "events.tsv"), "num")
        self.rc = load_rpc_cost()

    def access_class(self, key, label):
        region, rw, page, reg = key.split(":")
        off = int(page, 16) if reg == "-" else int(reg, 16)
        return self.rc.classify(region, rw, off, label or "", 0xFEA000)

    def explain(self, r):
        """(name, category, provenance, module, where) of a row."""
        kind, key, label = r["kind"], r["key"], r["label"]
        if kind == "control":
            c = self.controls.get(int(key, 16))
            if c is None:
                # The GSS-legacy and BINAPI ids lie outside the export tables (gsp-surface.md §2).
                return ("(not in controls.tsv: GSS legacy or BINAPI)", "-", "-",
                        module_of_label(label), "-")
            ab = c.get("answered_by", "-")
            mod = MODULE_OF_ANSWER.get(ab.split(":")[0], "-") if ab != "-" else module_of_label(label)
            return (c["name"], c.get("category", "-"), c.get("provenance", "-"), mod,
                    f"answered_by {ab}; {c.get('privilege', '-')}, route {c.get('route', '-')}")
        if kind == "alloc":
            c = self.classes.get(int(key, 16))
            return (c["name"] if c else "-", "-", c.get("device", "-") if c else "-",
                    module_of_label(label), "classes.tsv" if c else "-")
        if kind == "rpc":
            c = self.rpcs.get(int(key, 16))
            return (c["name"] if c else r["name"], "-", c.get("device", "-") if c else "-",
                    "boot.rs (service_queues), gsprpc.rs", "-")
        if kind == "event":
            c = self.events.get(int(key, 16))
            return (c["name"] if c else "-", "-", c.get("device", "-") if c else "-", "events.rs", "-")
        if kind == "access":
            cls = self.access_class(key, label)
            return (label or "-", cls, "-", MODULE_OF_CLASS.get(cls, "bar0.rs"), "-")
        if kind == "host":
            nr, k = key.split(":")
            what = r["name"]
            if what == "RM_CONTROL":
                c = self.controls.get(int(k, 16))
                what += " " + (c["name"] if c else k)
            elif what == "RM_ALLOC":
                c = self.classes.get(int(k, 16))
                what += " " + (c["name"] if c else k)
            return (what, "-", "-", "hostrm.rs, hostmem.rs, mirror.rs, shadow.rs", "-")
        if kind == "shadow":
            return (key, "-", "-", "shadow.rs (worker)", "-")
        return (key, "-", "-", window_module(key), "-")


def module_of_label(label):
    for part in (label or "").replace("(", " ").replace(")", " ").replace(",", " ").split():
        if part.endswith(".rs"):
            return part
    return "-"


def window_module(name):
    for pre, mod in (("intr page", "intr_page.rs"), ("display page", "display/fe_page.rs"),
                     ("token pool", "tokens.rs, poll.rs"), ("bar2 memfd", "bar2area.rs"),
                     ("bar1 pool", "bar1pool.rs"), ("interrupts", "intr.rs"),
                     ("doorbell eventfd", "device.rs, poll.rs"), ("msi re-arm", "device.rs, intr_page.rs")):
        if name.startswith(pre):
            return mod
    return "-"


def cost(r, how):
    if r["kind"] == "access" and how == "guest":
        return r["gap_ns"]
    return r["total_ns"]


def ranked(rows, joiner, how, kinds=None):
    out = []
    for r in rows:
        if kinds and r["kind"] not in kinds:
            continue
        name, cat, prov, mod, where = joiner.explain(r)
        c = cost(r, how)
        mean = c / r["count"] if r["count"] else 0
        out.append(dict(r, cost_ns=c, mean_ns=mean, jname=name, category=cat, provenance=prov,
                        module=mod, where=where))
    out.sort(key=lambda x: (-x["cost_ns"], -x["count"]))
    return out


def controls_of(path, base=None):
    rows, _ = read_dump(path)
    if base:
        rows = minus(rows, read_dump(base)[0])
    return {int(k[2], 16) for k in rows if k[1] == "control" and k[0] == "device"}


def fmt_us(ns):
    return f"{ns / 1000:.1f}"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dump")
    ap.add_argument("--base", help="subtract this earlier dump (device scope)")
    ap.add_argument("--scope", default="device")
    ap.add_argument("--kind", help="comma-separated kinds: access,control,alloc,rpc,event,host,shadow,window")
    ap.add_argument("--top", type=int, default=30)
    ap.add_argument("--cost", choices=("guest", "device"), default="guest")
    ap.add_argument("--tsv", action="store_true", help="all rows as TSV instead of the markdown top")
    ap.add_argument("--json")
    ap.add_argument("--seen-in", nargs="*", default=[],
                    help="dumps (DUMP or DUMP:BASE): mark controls none of them sent")
    ap.add_argument("--clients", action="store_true", help="list the client lifetimes")
    a = ap.parse_args()

    rows, head = read_dump(a.dump)
    if a.clients:
        per = collections.OrderedDict()
        for (s, kind, key), r in rows.items():
            if s.startswith("client:"):
                p = per.setdefault(s, [r["from_us"], r["to_us"], 0, 0, 0])
                p[2] += r["count"] if kind == "control" else 0
                p[3] += r["count"] if kind == "access" else 0
                p[4] += r["count"] if kind == "rpc" else 0
        print("| client | from s | to s | controls | trapped accesses | RPCs |\n|---|---|---|---|---|---|")
        for s, (f, t, c, acc, rp) in per.items():
            print(f"| {s} | {f / 1e6:.1f} | {t / 1e6:.1f} | {c} | {acc} | {rp} |")
        return 0
    if a.base:
        rows = minus(rows, read_dump(a.base)[0])
    sel = select(rows, a.scope)
    j = Joiner()
    kinds = set(a.kind.split(",")) if a.kind else None
    rk = ranked(sel, j, a.cost, kinds)
    seen = set()
    for spec in a.seen_in:
        p, _, b = spec.partition(":")
        seen |= controls_of(p, b or None)
    if a.tsv:
        cols = ("kind", "key", "jname", "count", "cost_ns", "mean_ns", "total_ns", "gap_ns", "max_ns",
                "units", "status", "label", "category", "provenance", "module", "where")
        print("\t".join(cols) + ("\tnot_in_seen" if a.seen_in else ""))
        for r in rk:
            extra = ""
            if a.seen_in:
                extra = "\t" + ("1" if r["kind"] == "control" and int(r["key"], 16) not in seen else "0")
            print("\t".join(str(r[c]) if not isinstance(r[c], float) else f"{r[c]:.0f}" for c in cols) + extra)
    else:
        total = sum(r["cost_ns"] for r in rk) or 1
        print(f"# profile-rank: {a.dump}" + (f" minus {a.base}" if a.base else "") + f", scope {a.scope}, cost {a.cost}\n")
        print(f"{len(rk)} rows, {total / 1e6:.1f} ms in all.\n")
        print("| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |")
        print("|---|---|---|---|---|---|---|---|---|---|---|---|---|")
        for i, r in enumerate(rk[:a.top], 1):
            mark = " (no reference sent it)" if a.seen_in and r["kind"] == "control" and int(r["key"], 16) not in seen else ""
            print(f"| {i} | {r['kind']} | {r['key']} | {r['jname']}{mark} | {r['count']} | {r['cost_ns'] / 1e6:.2f} | "
                  f"{r['mean_ns'] / 1000:.1f} | {fmt_us(r['max_ns'])} | {r['status']} | {r['category']} | "
                  f"{r['provenance']} | {r['module']} | {r['label']} |")
        by_kind = collections.Counter()
        n_kind = collections.Counter()
        for r in rk:
            by_kind[r["kind"]] += r["cost_ns"]
            n_kind[r["kind"]] += r["count"]
        print("\n| kind | rows | count | cost ms |\n|---|---|---|---|")
        for k, v in by_kind.most_common():
            print(f"| {k} | {sum(1 for r in rk if r['kind'] == k)} | {n_kind[k]} | {v / 1e6:.2f} |")
        if a.seen_in:
            new = sorted({int(r["key"], 16) for r in rk if r["kind"] == "control"} - seen)
            print(f"\nControls sent here and in none of the {len(a.seen_in)} reference profile(s): {len(new)}")
            for c in new:
                ctl = j.controls.get(c, {})
                print(f"- {c:#010x} {ctl.get('name', '(not in controls.tsv)')} "
                      f"[{ctl.get('category', '-')}, {ctl.get('provenance', '-')}]")
    if a.json:
        with open(a.json, "w") as f:
            json.dump(rk, f, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
