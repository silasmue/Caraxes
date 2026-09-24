#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
"""ctrl-layouts.py: compiler-measured layouts of the control parameter structs the chip
data tables name (crates/vfio-user-nvgpu/chips/*/controls.toml, `params`, open rows
excepted) and of the category D controls whose layout the device must know: the two whose
layout changes between 610 and 615 and the ones with a hand-written model
(docs/caraxes/gsp-plan/d-controls.tsv; `MODELLED` of caraxes/tools/gsp-dtable.py), for one
open-gpu-kernel-modules tree. A D struct the tree does not define (a control new in a later
version) is left out of that tree's table.

Writes crates/vfio-user-nvgpu/src/ver/v<major>/ctrl_layouts.rs: per struct its size and
every scalar leaf as a path (`entries[].nv2080EngineType`) with offset, size and the
stride and count of each array level. Nothing is typed by hand: clang's record-layout
dump names the fields (`-Xclang -fdump-record-layouts`), and a C program compiled
against the tree's SDK headers measures them with offsetof/sizeof. Where crates/nvrm-sys
binds the same struct, the file also asserts at compile time that its size and the
offsets of its top-level fields equal the nvrm-sys binding, so the two generated ABIs
cannot drift apart (decision D14; the handler is chipctl.rs).

Usage:
  python3 caraxes/tools/ctrl-layouts.py --ogkm DIR [--out FILE]

Needs clang and a C compiler (cc); python 3.11 or later, standard library only.
"""

import argparse
import glob
import importlib.util
import os
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
CRATE = os.path.join(ROOT, "crates", "vfio-user-nvgpu")
NVRM_SYS = os.path.join(ROOT, "crates", "nvrm-sys", "src")
SCALAR_SIZES = (1, 2, 4, 8)


def wanted_structs():
    """`params` of every entry (not open) of every chip's controls.toml."""
    names = set()
    for p in sorted(glob.glob(os.path.join(CRATE, "chips", "*", "controls.toml"))):
        with open(p, "rb") as f:
            for e in tomllib.load(f).get("control", []):
                if "open" not in e:
                    names.add(e["params"])
    return sorted(names)


def d_structs():
    """The parameter structs of the D controls with a layout change or a model, and of the
    getters that answer from a model (`MODEL_STRUCTS` of gsp-dtable.py)."""
    spec = importlib.util.spec_from_file_location(
        "gsp_dtable", os.path.join(ROOT, "caraxes", "tools", "gsp-dtable.py"))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return sorted({r["params"] for r in m.read_tsv()
                   if r["params"] and (r["layout_change"] or r["name"] in m.MODELLED
                                       or r["name"] in m.MODELLED_HERE)}
                  | set(m.MODEL_STRUCTS))


def header_of(sdk, name):
    rx = re.compile(r"}\s*%s\s*;" % re.escape(name))
    for p in sorted(glob.glob(os.path.join(sdk, "ctrl", "**", "*.h"), recursive=True)):
        with open(p, encoding="utf-8", errors="replace") as f:
            if rx.search(f.read()):
                return os.path.relpath(p, sdk)
    return None


def record_dump(sdk, headers, names):
    """Record name -> [(field name, type text)] of its direct fields, from clang."""
    src = "#include <nvtypes.h>\n" + "".join("#include <%s>\n" % h for h in headers)
    src += "int caraxes_sizes[] = {%s};\n" % ", ".join("sizeof(%s)" % n for n in names)
    with tempfile.TemporaryDirectory() as d:
        c = os.path.join(d, "l.c")
        with open(c, "w") as f:
            f.write(src)
        out = subprocess.run(["clang", "-fsyntax-only", "-Xclang", "-fdump-record-layouts",
                              "-DNV_LINUX", "-std=gnu11", "-I", sdk, c],
                             check=True, capture_output=True, text=True).stdout
    records = {}
    for block in out.split("*** Dumping AST Record Layout")[1:]:
        lines = [l for l in block.split("\n") if "|" in l]
        head = lines[0].split("|", 1)[1].strip()
        m = re.match(r"(?:struct|union) (\w+)$", head)
        if not m:
            continue
        fields = []
        # An anonymous struct member (`struct { ... } value;`) has no record of its own in
        # the dump: its direct members follow it one level deeper and become the record
        # "<outer>::<member>".
        anon = None
        for l in lines[1:]:
            # "   OFF |   TYPE NAME": one space after the bar, two per nesting level.
            body = l.split("|", 1)[1][1:]
            depth = (len(body) - len(body.lstrip(" "))) // 2
            if body.strip().startswith("["):
                continue
            tm = re.match(r"(.+?)\s+(\w+)$", body.strip())
            if not tm:
                continue
            if depth == 1:
                anon = None
                ty = tm.group(1)
                if "(unnamed at " in ty and "[" not in ty.rsplit(")", 1)[-1]:
                    anon = "%s::%s" % (m.group(1), tm.group(2))
                    ty = anon
                    records.setdefault(anon, [])
                fields.append((tm.group(2), ty))
            elif depth == 2 and anon is not None:
                records[anon].append((tm.group(2), tm.group(1)))
        records.setdefault(m.group(1), fields)
    return records


def leaves(records, rec, prefix_c, prefix_path, dims):
    """(C member expression, path with [] per array level, [C expr of each array]) leaves."""
    for name, ty in records.get(rec, []):
        base = re.sub(r"\[\d+\]", "", ty).replace("struct ", "").strip()
        ndim = len(re.findall(r"\[\d+\]", ty))
        c_expr = prefix_c + name
        arrays = []
        for _ in range(ndim):
            arrays.append(c_expr)
            c_expr += "[0]"
        path = prefix_path + name + "[]" * ndim
        if base in records:
            yield from leaves(records, base, c_expr + ".", path + ".", dims + arrays)
        else:
            yield c_expr, path, dims + arrays


def nvrm_sys_path(name, version_mod):
    for f, path in ((os.path.join(NVRM_SYS, version_mod + ".rs"), "nvrm_sys::%s::%s" % (version_mod, name)),
                    (os.path.join(NVRM_SYS, "stable.rs"), "nvrm_sys::%s" % name)):
        with open(f, encoding="utf-8") as fh:
            if re.search(r"pub struct %s \{" % re.escape(name), fh.read()):
                return path
    return None


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--ogkm", required=True)
    ap.add_argument("--out")
    a = ap.parse_args()
    ogkm = os.path.abspath(a.ogkm)
    with open(os.path.join(ogkm, "version.mk")) as f:
        driver = re.search(r"^NVIDIA_VERSION\s*=\s*(\S+)", f.read(), re.M).group(1)
    major = driver.split(".")[0]
    sdk = os.path.join(ogkm, "src", "common", "sdk", "nvidia", "inc")
    names = wanted_structs()
    headers = {}
    for n in names:
        h = header_of(sdk, n)
        if h is None:
            sys.exit("%s: no header of %s defines it" % (driver, n))
        headers[n] = h
    for n in d_structs():
        h = header_of(sdk, n)
        if h is not None and n not in headers:
            headers[n] = h
            names.append(n)
    names.sort()
    hs = sorted(set(headers.values()))
    records = record_dump(sdk, hs, names)
    prog = ["#include <stdio.h>", "#include <stddef.h>", "#include <nvtypes.h>"]
    prog += ["#include <%s>" % h for h in hs]
    prog.append("int main(void) {")
    per = {}
    for n in names:
        if n not in records:
            sys.exit("%s: clang dumped no layout for %s" % (driver, n))
        prog.append('    printf("S %s %%zu\\n", sizeof(%s));' % (n, n))
        for top, _ in records[n]:
            prog.append('    printf("T %s %s %%zu\\n", offsetof(%s, %s));' % (n, top, n, top))
        per[n] = list(leaves(records, n, "", "", []))
        for i, (c_expr, path, arrays) in enumerate(per[n]):
            prog.append('    printf("F %s %d %%zu %%zu\\n", offsetof(%s, %s), sizeof(((%s *)0)->%s));'
                        % (n, i, n, c_expr, n, c_expr))
            for k, arr in enumerate(arrays):
                prog.append('    printf("D %s %d %d %%zu %%zu\\n", sizeof(((%s *)0)->%s[0]), '
                            'sizeof(((%s *)0)->%s) / sizeof(((%s *)0)->%s[0]));'
                            % (n, i, k, n, arr, n, arr, n, arr))
    prog.append("    return 0;\n}")
    with tempfile.TemporaryDirectory() as d:
        c, exe = os.path.join(d, "m.c"), os.path.join(d, "m")
        with open(c, "w") as f:
            f.write("\n".join(prog) + "\n")
        subprocess.run(["cc", "-std=gnu11", "-DNV_LINUX", "-I", sdk, "-o", exe, c], check=True)
        out = subprocess.run([exe], check=True, capture_output=True, text=True).stdout
    size, field, dim, tops = {}, {}, {}, {}
    for l in out.split("\n"):
        p = l.split()
        if not p:
            continue
        if p[0] == "S":
            size[p[1]] = int(p[2])
        elif p[0] == "T":
            tops.setdefault(p[1], []).append((p[2], int(p[3])))
        elif p[0] == "F":
            field[(p[1], int(p[2]))] = (int(p[3]), int(p[4]))
        elif p[0] == "D":
            dim[(p[1], int(p[2]), int(p[3]))] = (int(p[4]), int(p[5]))

    w = ["// SPDX-License-Identifier: MIT",
         "// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>",
         "// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR",
         "//! Layouts of the control parameter structs the chip tables name (chips/*/controls.toml)",
         "//! and of the category D controls with a layout change or a model (d-controls.tsv),",
         "//! measured by the compiler against the SDK headers of open-gpu-kernel-modules %s:" % driver,
         "//! generated by `caraxes/tools/ctrl-layouts.py --ogkm <tree>`. Do not edit; rerun it.",
         "//! Where nvrm-sys binds a struct, the assertions at the end pin these numbers to it.",
         "",
         "use crate::ver::{CtrlField, CtrlLayout};",
         "",
         "pub const CTRL_LAYOUTS: &[CtrlLayout] = &["]
    for n in names:
        w.append("    CtrlLayout {")
        w.append('        name: "%s",' % n)
        w.append('        header: "%s",' % os.path.relpath(os.path.join(sdk, headers[n]), os.path.join(sdk, "ctrl")))
        w.append("        size: %d," % size[n])
        w.append("        fields: &[")
        for i, (_, path, arrays) in enumerate(per[n]):
            off, sz = field[(n, i)]
            if sz not in SCALAR_SIZES:
                continue
            dims = ", ".join("(%d, %d)" % dim[(n, i, k)] for k in range(len(arrays)))
            w.append('            CtrlField { path: "%s", offset: %d, size: %d, dims: &[%s] },'
                     % (path, off, sz, dims))
        w.append("        ],")
        w.append("    },")
    w.append("];")
    checks = []
    for n in names:
        rs = nvrm_sys_path(n, "v" + major)
        if not rs:
            continue
        checks.append("    assert!(size_of::<%s>() == %d);" % (rs, size[n]))
        for top, off in tops.get(n, []):
            checks.append("    assert!(offset_of!(%s, %s) == %d);" % (rs, top, off))
    if checks:
        w += ["", "// The same structs as nvrm-sys binds them (sizes and top-level offsets).",
              "use core::mem::{offset_of, size_of};",
              "const _: () = {"] + checks + ["};"]
    out_p = a.out or os.path.join(CRATE, "src", "ver", "v%s" % major, "ctrl_layouts.rs")
    os.makedirs(os.path.dirname(out_p), exist_ok=True)
    with open(out_p, "w") as f:
        f.write("\n".join(w) + "\n")
    print("%s: %d structs, %d of them checked against nvrm-sys" % (
        os.path.relpath(out_p, ROOT), len(names),
        sum(1 for n in names if nvrm_sys_path(n, "v" + major))))


if __name__ == "__main__":
    main()
