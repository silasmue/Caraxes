#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
"""Per-function execution counts of the device crate from two LLVM coverage profiles.

The device of `run.sh map` is built with -C instrument-coverage (target/cov-instr); its
profile holds what the real workloads executed. The crate's unit tests, built the same way
(target/cov-test), give what the tests execute. For every function of
crates/vfio-user-nvgpu/src (test modules left out) this prints whether workloads, tests,
both or neither reached it, and a per-file summary.

Usage: llvm-functions.py WORKLOAD.json TESTS.json [--tsv OUT] > summary.md
where each JSON is `llvm-cov export -format=text -skip-expansions BIN -instr-profile=P`.
"""
import argparse
import collections
import json
import re
import sys

SRC = "crates/vfio-user-nvgpu/src/"


def demangle(sym):
    """A readable path for a Rust symbol: v0 (_R...) identifiers joined by ::, crate hashes
    dropped (the tests and the device binary are separate crate instances), and legacy
    (_ZN ... 17h<hash>E) symbols unpacked."""
    if sym.startswith("_R"):
        body = re.sub(r"Cs[0-9A-Za-z]+_", "C", sym)
        idents, i = [], 0
        while i < len(body):
            m = re.match(r"(\d+)_?", body[i:])
            if m and body[i - 1:i] not in ("B",) and m.group(1) != "0":
                n = int(m.group(1))
                start = i + len(m.group(0))
                ident = body[start:start + n]
                if re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", ident):
                    idents.append(ident)
                    i = start + n
                    continue
            i += 1
        return "::".join(idents) or body
    m = re.match(r"^_ZN(.*)E$", sym)
    if not m:
        return sym
    s, parts = m.group(1), []
    while s and s[0].isdigit():
        n = re.match(r"^(\d+)", s).group(1)
        s = s[len(n):]
        parts.append(s[:int(n)])
        s = s[int(n):]
    if parts and re.match(r"^h[0-9a-f]{16}$", parts[-1]):
        parts.pop()
    out = "::".join(parts)
    for a, b in (("$LT$", "<"), ("$GT$", ">"), ("$u20$", " "), ("$C$", ","), ("..", "::"), ("$RF$", "&"),
                 ("$u7b$", "{"), ("$u7d$", "}"), ("$BP$", "*"), ("$u27$", "'"), ("$u5b$", "["), ("$u5d$", "]")):
        out = out.replace(a, b)
    return out


def functions(path):
    """(file, function) -> (count, first line) for the device crate's source files."""
    d = json.load(open(path))
    out = {}
    for f in d["data"][0]["functions"]:
        files = [x for x in f["filenames"] if SRC in x]
        if not files:
            continue
        rel = files[0][files[0].index(SRC) + len(SRC):]
        name = demangle(f["name"])
        # Back-references leave fragments before the path: start at the file's module.
        mod = rel[:-3].split("/")[-1]
        toks = name.split("::")
        if mod in toks:
            name = "::".join(toks[toks.index(mod):])
        if "tests" in name.split("::"):
            continue
        line = f["regions"][0][0] if f["regions"] else 0
        key = (rel, name)
        c, l0 = out.get(key, (0, line))
        out[key] = (c + f["count"], min(l0, line))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("workload")
    ap.add_argument("tests")
    ap.add_argument("--tsv")
    a = ap.parse_args()
    w, t = functions(a.workload), functions(a.tests)
    keys = sorted(set(w) | set(t))
    per = collections.defaultdict(collections.Counter)
    rows = []
    for k in keys:
        wc, line = w.get(k, (0, 0))
        tc, tl = t.get(k, (0, 0))
        cls = ("both" if wc and tc else "workload only" if wc else "tests only" if tc else "neither")
        per[k[0]][cls] += 1
        rows.append((k[0], line or tl, k[1], wc, tc, cls))
    if a.tsv:
        with open(a.tsv, "w") as f:
            f.write("# SPDX-License-Identifier: MIT\nfile\tline\tfunction\tworkload_count\ttest_count\treached_by\n")
            for r in rows:
                f.write("\t".join(str(x) for x in r) + "\n")
    tot = collections.Counter()
    for c in per.values():
        tot.update(c)
    print("| file | functions | both | workload only | tests only | neither |\n|---|---|---|---|---|---|")
    for fname, c in sorted(per.items(), key=lambda kv: -(kv[1]["neither"] + kv[1]["tests only"])):
        print(f"| {fname} | {sum(c.values())} | {c['both']} | {c['workload only']} | {c['tests only']} | {c['neither']} |")
    print(f"| all | {sum(tot.values())} | {tot['both']} | {tot['workload only']} | {tot['tests only']} | {tot['neither']} |")
    return 0


if __name__ == "__main__":
    sys.exit(main())
