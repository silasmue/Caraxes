#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""The census's ranked table: trap sources across workloads (trap-census.py --json outputs).

Per source and workload: traps per second; the vCPU share as modelled (rate x (RTT + device
time per trap), RTT the measured round trip of a trapped access without device work) and as
the profile's guest-seen upper bound; the device share. Sources are ranked by the largest
modelled vCPU share over the workloads given.
Usage: rank.py [--rtt US] NAME=FILE.json ...
"""
import argparse
import json

ap = argparse.ArgumentParser()
ap.add_argument("--rtt", type=float, default=25.0)
ap.add_argument("--top", type=int, default=16)
ap.add_argument("--md", action="store_true")
ap.add_argument("runs", nargs="+")
a = ap.parse_args()
runs = []
for spec in a.runs:
    name, path = spec.split("=", 1)
    with open(path) as f:
        runs.append((name, json.load(f)))
src = {}
for name, r in runs:
    for s in r["sources"]:
        d = src.setdefault(s["source"], {})
        model = s["per_s"] * (a.rtt + s["dev_us"]) / 1e4  # percent of one vCPU
        d[name] = (s["per_s"], s["dev_us"], model, s["vcpu_share_pct"], s["dev_share_pct"])
order = sorted(src, key=lambda k: -max(v[2] for v in src[k].values()))
names = [n for n, _ in runs]
print(f"RTT {a.rtt} us; cell: traps/s | dev us | vCPU % modelled (seen) | device %")
print("| source | " + " | ".join(names) + " |")
print("|---|" + "---|" * len(names))
for k in order[: a.top]:
    cells = []
    for n in names:
        v = src[k].get(n)
        cells.append("-" if v is None or v[0] < 0.05 else f"{v[0]:.0f}/s, {v[1]:.0f} us, {v[2]:.1f} ({v[3]:.1f}) %, dev {v[4]:.1f} %")
    print(f"| {k} | " + " | ".join(cells) + " |")
tot = []
for n, r in runs:
    t = r["total"]
    model = sum(s["per_s"] * (a.rtt + s["dev_us"]) / 1e4 for s in r["sources"])
    tot.append(f"{t['per_s']:.0f}/s, vCPU {model:.1f} ({t['vcpu_share_pct']:.1f}) %, dev {t['dev_share_pct']:.1f} %")
print("| all | " + " | ".join(tot) + " |")
