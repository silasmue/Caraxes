#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""What the guest control sweep reached, and what stays untested (coverage-map.md §3).

Joins extra/work/ctlsweep-table-out.txt (the guest's status per swept control) with the
device's profile of the ctlsweep window (dumps of extra/, the controls that arrived as
GSP_RM_CONTROL) and with the union of map/ and extra/ (combined-coverage.json). Prints:
  1. the sweep: sent, reached the device, answered by the guest kernel before GSP, by status;
  2. the device-answered controls still untouched, by provenance and by how a client could
     reach them: user space (non-priv, priv as root), an object the sweep does not make, or
     the guest kernel only (internal, kernel): the latter get unit tests, not probes.
Usage: sweep-reach.py (from this directory; CARAXES_PROFILE_ABI as for coverage-map.py)
"""
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
ANSWERED = ("synthesized", "host-forwarded", "guest-derived", "echo", "refused")
SWEEP_OBJECTS = {"RmClientResource", "Device", "Subdevice", "DispCommon", "DiagApi", "I2cApi", "ZbcApi"}


def main():
    j = pr.Joiner()
    sent = {}
    for line in open(os.path.join(HERE, "extra", "work", "ctlsweep-table-out.txt")):
        p = line.rstrip("\n").split("\t")
        if len(p) >= 7 and p[0].startswith("0x"):
            sent[int(p[0], 16)] = p
    marks = [l.rstrip("\n").split("\t") for l in open(os.path.join(HERE, "extra", "marks.tsv"))]
    n = {m[1]: int(m[0]) for m in marks}
    d = lambda k: pr.read_dump(os.path.join(HERE, "extra", "profile", f"dev.{k}.tsv"))[0]
    win = pr.minus(d(n["ctlsweep"]), d(n["ctlsweep"] - 1))
    arrived = {int(k[2], 16): r for k, r in win.items() if k[1] == "control"}
    issued = {c: p for c, p in sent.items() if p[6] != "no object"}
    print("## The sweep\n")
    print(f"{len(sent)} controls in the table, {len(issued)} issued ({len(sent) - len(issued)} without their object: "
          "NV04_DISPLAY_COMMON is refused on vm2, which has no display engine).\n")
    by = collections.Counter()
    for c, p in issued.items():
        where = "reached the device" if c in arrived else "answered by the guest kernel RM"
        by[(where, p[5], p[6])] += 1
    print("| path | provenance | guest status | controls |\n|---|---|---|---|")
    for (where, prov, st), v in sorted(by.items()):
        print(f"| {where} | {prov} | {st} | {v} |")
    print(f"\nReached the device: {sum(1 for c in issued if c in arrived)} of {len(issued)}; "
          f"the device's answers there by label: "
          + ", ".join(f"{k} {v}" for k, v in collections.Counter(r['label'] for r in arrived.values()).most_common()) + "\n")

    comb = json.load(open(os.path.join(HERE, "combined-coverage.json")))
    untouched = {int(x, 16) for p in ANSWERED for x in comb["untouched"].get(p, [])}
    print("## Device-answered controls no workload and no sweep reached\n")
    print("| provenance | untouched | user space, non-priv | user space as root (priv) | object the sweep lacks | guest kernel only (internal, kernel) |")
    print("|---|---|---|---|---|---|")
    fam = collections.Counter()
    for prov in ANSWERED:
        ids = [c for c in untouched if j.controls.get(c, {}).get("provenance") == prov]
        cls = collections.Counter()
        for c in ids:
            row = j.controls[c]
            priv, obj = row["privilege"], row["nvoc_class"]
            if priv in ("internal", "kernel"):
                k = "kernel"
            elif obj not in SWEEP_OBJECTS:
                k = "object"
            else:
                k = priv
            cls[k] += 1
            fam[(prov, row.get("answered_by", "-"), k)] += 1
        print(f"| {prov} | {len(ids)} | {cls['non-priv']} | {cls['priv']} | {cls['object']} | {cls['kernel']} |")
    print("\nBy the handler (controls.tsv answered_by), largest groups first:\n")
    print("| provenance | answered_by | reach | controls |\n|---|---|---|---|")
    for (prov, ab, k), v in fam.most_common(25):
        print(f"| {prov} | {ab} | {k} | {v} |")
    return 0


if __name__ == "__main__":
    sys.exit(main())
