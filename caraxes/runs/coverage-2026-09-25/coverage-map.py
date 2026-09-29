#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""The coverage map from one `run.sh map` directory (docs/caraxes/coverage-map.md).

Reads RUN/marks.tsv and the dumps RUN/profile/dev.<n>.tsv; workload n is dump n minus dump
n-1 (profile-rank.py `minus`). Writes to stdout, in markdown:
  (a) per workload: the top rows by cost and the controls it sent that no gate sent;
  (b) the union: controls the device answers (controls.tsv provenance) touched by any
      workload and by none, by provenance; RPC functions, events, classes, host call kinds
      and windows exercised;
  (c) candidates: rows with a high cost outside the benchmark workloads.
--json OUT keeps the numbers.
Several RUN directories combine: their workloads side by side (a label already taken gets the
run's name as prefix), the union over all their exit dumps.
Usage: coverage-map.py RUN... [--top 8] [--json OUT]
"""
import argparse
import collections
import importlib.util
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
spec = importlib.util.spec_from_file_location("profile_rank", os.path.join(ROOT, "caraxes", "tools", "profile-rank.py"))
pr = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pr)

GATES = ("gate-nvml", "gate-cuda")
BENCH = ("bench-init", "bench-short-auto", "bench-rl", "bench-ppo")
ANSWERED = ("synthesized", "host-forwarded", "guest-derived", "echo", "refused")


def load(run):
    marks = []
    with open(os.path.join(run, "marks.tsv")) as f:
        for line in f:
            n, label, *_ = line.rstrip("\n").split("\t")
            marks.append((int(n), label))
    dumps = {}
    for n, _ in marks:
        p = os.path.join(run, "profile", f"dev.{n}.tsv")
        if os.path.exists(p) or os.path.exists(p + ".xz"):
            dumps[n] = pr.read_dump(p)[0]
    work = collections.OrderedDict()
    prev = None
    for n, label in marks:
        if n not in dumps:
            prev = None
            continue
        rows = dumps[n] if prev is None else pr.minus(dumps[n], dumps[prev])
        if prev is None:
            rows = collections.OrderedDict((k, v) for k, v in rows.items() if k[0] == "device")
        work[label] = rows
        prev = n
    exit_path = os.path.join(run, "profile", "dev.exit.tsv")
    have_exit = os.path.exists(exit_path) or os.path.exists(exit_path + ".xz")
    total = pr.read_dump(exit_path)[0] if have_exit else dumps[max(dumps)]
    return work, total


def keys_of(rows, kind):
    return {k[2] for k in rows if k[1] == kind and k[0] == "device"}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("run", nargs="+")
    ap.add_argument("--top", type=int, default=8)
    ap.add_argument("--json")
    a = ap.parse_args()
    work, totals = collections.OrderedDict(), []
    for run in a.run:
        w, t = load(run)
        for label, rows in w.items():
            if label in ("boot", "setup") and label in work:
                continue
            work[label if label not in work else f"{os.path.basename(run.rstrip('/'))}/{label}"] = rows
        totals.append(t)
    total = collections.OrderedDict()
    for t in totals:
        for k, r in t.items():
            if k[0] != "device":
                continue
            if k in total:
                total[k] = dict(total[k], count=total[k]["count"] + r["count"], units=total[k]["units"] + r["units"])
            else:
                total[k] = r
    j = pr.Joiner()
    out = {"workloads": {}}

    gate_ctl = set()
    for g in GATES:
        gate_ctl |= keys_of(work.get(g, {}), "control")

    print("## (a) Per workload\n")
    print("Cost: trapped accesses by guest-seen time, the rest by device time (profile-rank.py). "
          "\"New controls\": sent in this workload and in neither gate's window.\n")
    print("| workload | rows | cost ms | controls (ids) | new controls | RPCs | trapped accesses | host RM calls | top rows by cost |")
    print("|---|---|---|---|---|---|---|---|---|")
    for label, rows in work.items():
        rk = pr.ranked(list(rows.values()), j, "guest")
        ctl = keys_of(rows, "control")
        new = sorted(ctl - gate_ctl) if label not in GATES else []
        cnt = collections.Counter()
        for r in rk:
            cnt[r["kind"]] += r["count"]
        top = "; ".join(f"{r['kind']} {short(r)} {r['count']}x {r['cost_ns'] / 1e6:.1f} ms" for r in rk[:4])
        total_ms = sum(r["cost_ns"] for r in rk) / 1e6
        print(f"| {label} | {len(rk)} | {total_ms:.0f} | {sum(1 for r in rk if r['kind'] == 'control')} | "
              f"{len(new)} | {cnt['rpc']} | {cnt['access']} | {cnt['host']} | {top} |")
        out["workloads"][label] = {
            "rows": len(rk), "cost_ms": total_ms, "new_controls": new,
            "top": [{k: r[k] for k in ("kind", "key", "jname", "count", "cost_ns", "mean_ns", "module", "provenance", "label")}
                    for r in rk[:a.top]],
            "kinds": dict(cnt),
        }
    print()
    print("### New controls per workload (not sent in either gate's window)\n")
    for label, w in out["workloads"].items():
        if not w["new_controls"]:
            continue
        names = []
        for c in w["new_controls"]:
            ctl = j.controls.get(int(c, 16), {})
            names.append(f"{c} {ctl.get('name', '(not in controls.tsv)')} [{ctl.get('provenance', '-')}]")
        print(f"- **{label}** ({len(names)}): " + "; ".join(names))
    print()

    print(f"### Top {a.top} rows per workload\n")
    for label, w in out["workloads"].items():
        print(f"**{label}**\n")
        print("| # | kind | key | name | count | cost ms | mean us | module | label |\n|---|---|---|---|---|---|---|---|---|")
        for i, r in enumerate(w["top"], 1):
            print(f"| {i} | {r['kind']} | {r['key']} | {r['jname']} | {r['count']} | {r['cost_ns'] / 1e6:.2f} | "
                  f"{r['mean_ns'] / 1000:.1f} | {r['module']} | {r['label']} |")
        print()

    # (b) The union over the whole device lifetime (the exit dump holds boot and teardown too).
    seen_ctl = {int(k, 16) for k in keys_of(total, "control")}
    print("## (b) The union\n")
    print(f"Controls of controls.tsv ({os.path.basename(pr.ABI)}) that reach GSP, by the device's provenance, and "
          "whether any workload of this run (boot and teardown included) sent them:\n")
    print("| provenance | answered by the device | touched | untouched |\n|---|---|---|---|")
    by_prov = collections.defaultdict(lambda: [0, 0, []])
    for cid, c in j.controls.items():
        if c.get("route") != "gsp":
            continue
        p = c.get("provenance", "-")
        if p == "-":
            p = "(not handled, no provenance)"
        e = by_prov[p]
        if cid in seen_ctl:
            e[0] += 1
        else:
            e[1] += 1
            e[2].append(cid)
    for p in list(ANSWERED) + ["unhandled", "(not handled, no provenance)"]:
        if p in by_prov:
            t, u, _ = by_prov[p]
            print(f"| {p} | {'yes' if p in ANSWERED else 'no'} | {t} | {u} |")
    outside = sorted(c for c in seen_ctl if c not in j.controls)
    print(f"\nControls seen that controls.tsv does not list (GSS legacy, BINAPI): {len(outside)}: "
          + ", ".join(f"{c:#010x}" for c in outside) + "\n")
    out["union"] = {p: {"touched": v[0], "untouched": v[1]} for p, v in by_prov.items()}
    out["untouched"] = {p: [f"{c:#010x}" for c in v[2]] for p, v in by_prov.items()}
    out["seen_outside_tsv"] = [f"{c:#010x}" for c in outside]
    for kind, table in (("rpc", j.rpcs), ("event", j.events), ("alloc", j.classes)):
        seen = keys_of(total, kind)
        names = []
        for k in sorted(seen, key=lambda x: int(x, 16)):
            row = table.get(int(k, 16), {})
            names.append(f"{row.get('name', k)}")
        print(f"- {kind}: {len(seen)} exercised: {', '.join(names)}")
        out[f"{kind}_seen"] = sorted(seen)
    for kind in ("host", "shadow", "window"):
        seen = sorted({(k[2], r["name"]) for k, r in total.items() if k[1] == kind and k[0] == "device" and (r["count"] or r["units"])})
        print(f"- {kind}: {len(seen)} exercised: " + ", ".join(f"{n} {k}" if kind == "host" else k for k, n in seen))
        out[f"{kind}_seen"] = [k for k, _ in seen]
    print()

    # (c) Candidates: per (kind, key), the largest cost in a non-benchmark workload, with the
    # benchmark workloads' cost of the same row beside it.
    print("## (c) Candidates outside the benchmarks\n")
    cand = {}
    bench_cost = collections.Counter()
    for label, rows in work.items():
        for r in pr.ranked(list(rows.values()), j, "guest"):
            key = (r["kind"], r["key"])
            if label in BENCH:
                bench_cost[key] += r["cost_ns"]
                continue
            if label in ("boot", "setup"):
                continue
            c = cand.setdefault(key, dict(r, workloads=collections.Counter(), cost_all=0, count_all=0))
            c["workloads"][label] += r["cost_ns"]
            c["cost_all"] += r["cost_ns"]
            c["count_all"] += r["count"]
    ranked = sorted(cand.values(), key=lambda r: -r["cost_all"])
    print("\"In the benchmarks\" is the same row's cost in bench-init, short-auto, rl and ppo together; "
          "a row with 0 there is a path the benchmarks do not reach at all.\n")
    print("| # | kind | key | name | count | cost ms (all non-benchmark workloads) | mean us | in the benchmarks, ms | top workloads | module | label |")
    print("|---|---|---|---|---|---|---|---|---|---|---|")
    out["candidates"] = []
    for i, r in enumerate(ranked[:40], 1):
        key = (r["kind"], r["key"])
        tw = ", ".join(f"{w} {v / 1e6:.0f}" for w, v in r["workloads"].most_common(3))
        mean = r["cost_all"] / r["count_all"] / 1000 if r["count_all"] else 0
        print(f"| {i} | {r['kind']} | {r['key']} | {r['jname']} | {r['count_all']} | {r['cost_all'] / 1e6:.1f} | {mean:.1f} | "
              f"{bench_cost[key] / 1e6:.1f} | {tw} | {r['module']} | {r['label']} |")
        out["candidates"].append({"kind": r["kind"], "key": r["key"], "name": r["jname"], "count": r["count_all"],
                                  "cost_ms": r["cost_all"] / 1e6, "bench_ms": bench_cost[key] / 1e6,
                                  "workloads": dict(r["workloads"]), "module": r["module"]})
    if a.json:
        with open(a.json, "w") as f:
            json.dump(out, f, indent=1, default=list)
    return 0


def short(r):
    if r["kind"] == "access":
        return f"{r['key']} ({r['category']})"
    if r["jname"] not in ("-", ""):
        return f"{r['jname']}"
    return r["key"]


if __name__ == "__main__":
    sys.exit(main())
