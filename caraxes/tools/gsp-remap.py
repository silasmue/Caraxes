#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
"""gsp-remap.py: the field-by-field copy table that turns a struct the device builds in the
emulated version's layout into another driver version's layout, from the compiler-measured
layouts.tsv of two gsp-abi.py snapshots (docs/caraxes/gsp-abi-615.md). Nothing is typed by
hand: every offset and size comes from the two snapshots.

  gsp-remap.py FROM_SNAPSHOT TO_SNAPSHOT STRUCT [--elem PATH=ELEMSTRUCT ...] --out FILE.rs

The table lists (from offset, to offset, length, field) for every top-level field present in
both layouts; a field whose size or inner layout differs is split into its leaves (copying
the shorter length), and an array of structs named by --elem is copied element by element with
the element struct's own leaves (the stride may differ). Fields present on one side only are
listed in the generated file's comments: dropped (FROM only) or left zero (TO only).

Used for GspStaticConfigInfo 610.57.04 -> 570.144, the reply nova-core reads
(docs/caraxes/nova-guest.md):
  gsp-remap.py caraxes/abi/gsp-610.57.04 caraxes/abi/gsp-570.144 GspStaticConfigInfo \\
      --elem fbRegionInfoParams.fbRegion=NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO \\
      --out crates/vfio-user-nvgpu/src/ver/v570/static_info.rs
and, both ways, for the RM alloc parameters of a 570.144 guest (nouveau's r570 path) whose
layout differs from 610.57.04's (docs/research/open-render-2026-09-29.md):
  gsp-remap.py caraxes/abi/gsp-570.144 caraxes/abi/gsp-610.57.04 NV_CHANNEL_ALLOC_PARAMS \\
      --out crates/vfio-user-nvgpu/src/ver/v570/channel_up.rs
  gsp-remap.py caraxes/abi/gsp-610.57.04 caraxes/abi/gsp-570.144 NV_CHANNEL_ALLOC_PARAMS \\
      --out crates/vfio-user-nvgpu/src/ver/v570/channel_down.rs
  (the same for NV_VASPACE_ALLOCATION_PARAMETERS into vaspace_up.rs and vaspace_down.rs)
"""

import argparse
import csv
import os
import re
import sys


def load(snapshot):
    """layouts.tsv -> {struct: {path: (offset, size)}} and the MANIFEST's driver version."""
    rows = {}
    path = os.path.join(snapshot, "layouts.tsv")
    with open(path) as f:
        lines = [l for l in f if not l.startswith("#")]
    for r in csv.DictReader(lines, delimiter="\t"):
        size = 0 if r["size"] == "flex" else int(r["size"])  # a flexible array member
        rows.setdefault(r["struct"], {})[r["path"]] = (int(r["offset"]), size)
    version = "?"
    with open(os.path.join(snapshot, "MANIFEST")) as f:
        for line in f:
            if line.startswith("driver:"):
                version = line.split(":", 1)[1].strip()
                break
    return rows, version


def alloc_classes(snapshot, struct):
    """{(class id, name)} of the classes.tsv rows whose alloc_params is `struct`."""
    path = os.path.join(snapshot, "classes.tsv")
    if not os.path.exists(path):
        return set()
    with open(path) as f:
        lines = [l for l in f if not l.startswith("#")]
    return {(int(r["id"], 16), r["name"]) for r in csv.DictReader(lines, delimiter="\t")
            if r["alloc_params"] == struct}


def children(fields, prefix):
    """The direct children of `prefix` (its paths one level deeper)."""
    depth = prefix.count(".") + 1 if prefix else 0
    out = []
    for p in fields:
        if p == "(sizeof)":
            continue
        if prefix and not p.startswith(prefix + "."):
            continue
        if p.count(".") == depth:
            out.append(p)
    return out


def same_inner(a, b, prefix):
    """True when every leaf below `prefix` has the same relative offset and size."""
    pa = {p: v for p, v in a.items() if p.startswith(prefix + ".")}
    pb = {p: v for p, v in b.items() if p.startswith(prefix + ".")}
    if set(pa) != set(pb):
        return False
    base_a, base_b = a[prefix][0], b[prefix][0]
    return all(pa[p][0] - base_a == pb[p][0] - base_b and pa[p][1] == pb[p][1] for p in pa)


def remap(a, b, prefix, elems, ea, eb, out, dropped, zeroed, base_a=0, base_b=0, label=""):
    for p in sorted(set(children(a, prefix)) | set(children(b, prefix)),
                    key=lambda p: (b.get(p, a.get(p))[0])):
        name = label + p
        if p not in b:
            dropped.append(name)
            continue
        if p not in a:
            zeroed.append(name)
            continue
        (oa, sa), (ob, sb) = a[p], b[p]
        if p in elems:
            es = elems[p]
            fa, fb = ea[es], eb[es]
            stride_a, stride_b = fa["(sizeof)"][1], fb["(sizeof)"][1]
            n = min(sa // stride_a, sb // stride_b)
            for i in range(n):
                remap(fa, fb, "", {}, ea, eb, out, dropped if i == 0 else [], zeroed if i == 0 else [],
                      base_a + oa + i * stride_a, base_b + ob + i * stride_b, f"{name}[{i}].")
            continue
        if sa == sb and same_inner(a, b, p):
            out.append((base_a + oa, base_b + ob, sa, name))
        elif children(a, p) or children(b, p):
            remap(a, b, p, elems, ea, eb, out, dropped, zeroed, base_a, base_b, label)
        else:
            out.append((base_a + oa, base_b + ob, min(sa, sb), name))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("frm")
    ap.add_argument("to")
    ap.add_argument("struct")
    ap.add_argument("--elem", action="append", default=[], help="PATH=ELEMSTRUCT")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    ra, va = load(a.frm)
    rb, vb = load(a.to)
    if a.struct not in ra or a.struct not in rb:
        sys.exit(f"{a.struct} missing in one of the snapshots")
    elems = dict(e.split("=", 1) for e in a.elem)
    out, dropped, zeroed = [], [], []
    remap(ra[a.struct], rb[a.struct], "", elems, ra, rb, out, dropped, zeroed)
    # Merge adjacent runs that are contiguous on both sides.
    merged = []
    for fa, fb, n, name in sorted(out, key=lambda t: t[1]):
        if merged and merged[-1][0] + merged[-1][2] == fa and merged[-1][1] + merged[-1][2] == fb:
            pa, pb, pn, pname = merged[-1]
            merged[-1] = (pa, pb, pn + n, pname if "," in pname or pname.endswith("..") else pname + " ..")
        else:
            merged.append((fa, fb, n, name))
    size_a, size_b = ra[a.struct]["(sizeof)"][1], rb[a.struct]["(sizeof)"][1]
    cmd = "caraxes/tools/gsp-remap.py " + " ".join(sys.argv[1:])
    lines = [
        "// SPDX-License-Identifier: MIT",
        f"// Generated by {cmd}; do not edit, rerun it.",
        f"//! `{a.struct}` from the {va} layout ({size_a} bytes) into the {vb} layout",
        f"//! ({size_b} bytes): (from offset, to offset, length, first field), from the",
        "//! compiler-measured layouts.tsv of the two gsp-abi.py snapshots.",
        "",
        f"/// sizeof({a.struct}) in {vb}.",
        f"pub const SIZE: usize = {size_b};",
        f"/// sizeof({a.struct}) in {va}, the layout the table reads.",
        f"pub const FROM_SIZE: usize = {size_a};",
        "",
        "/// (from offset, to offset, length, first field of the run).",
        "pub const COPY: &[(usize, usize, usize, &str)] = &[",
    ]
    for fa, fb, n, name in merged:
        lines.append(f'    ({fa}, {fb}, {n}, "{name}"),')
    lines.append("];")
    lines.append("")
    lines.append(f"/// Fields of {va} that {vb} does not have (dropped).")
    lines.append("pub const DROPPED: &[&str] = &[")
    lines += [f'    "{d}",' for d in dropped]
    lines.append("];")
    lines.append(f"/// Fields of {vb} that {va} does not have (left zero).")
    lines.append("pub const ZEROED: &[&str] = &[")
    lines += [f'    "{z}",' for z in zeroed]
    lines.append("];")
    # The classes whose RM alloc parameters are this struct (classes.tsv of either snapshot),
    # so a caller can tell which allocations the table applies to.
    classes = sorted(alloc_classes(a.frm, a.struct) | alloc_classes(a.to, a.struct))
    if classes:
        lines.append("")
        lines.append(f"/// The classes allocated with `{a.struct}` (classes.tsv of {va} and {vb}).")
        lines.append("pub const CLASSES: &[u32] = &[")
        lines += [f"    0x{cid:04X}, // {name}" for cid, name in classes]
        lines.append("];")
    # The offsets a reader of the TO layout needs, as constants (top-level fields).
    lines.append("")
    lines.append(f"/// Offsets of the top-level fields in {vb}.")
    lines.append("pub mod at {")
    for p in children(rb[a.struct], ""):
        ident = re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "_", p).upper()
        lines.append(f"    pub const {ident}: usize = {rb[a.struct][p][0]};")
    lines.append("}")
    os.makedirs(os.path.dirname(a.out) or ".", exist_ok=True)
    with open(a.out, "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"{a.out}: {len(merged)} runs, {len(dropped)} dropped, {len(zeroed)} zeroed; {size_a} -> {size_b} bytes")


if __name__ == "__main__":
    main()
