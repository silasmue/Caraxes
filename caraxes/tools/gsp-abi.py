#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
"""gsp-abi.py: the GSP ABI of one open-gpu-kernel-modules tree as a diffable snapshot, and
the delta between two snapshots (docs/caraxes/gsp-abi-615.md).

615.71.09 is the recorded baseline (decision D20, caraxes/abi/gsp-615.71.09/); a later driver
version is handled as the delta of its snapshot against that baseline.

  snapshot --ogkm DIR --out caraxes/abi/gsp-<version>/ [--tables FILE]
      Write the snapshot of the tree DIR: MANIFEST, controls.tsv, rpc.tsv, events.tsv,
      classes.tsv, boot.tsv, layouts.tsv, constants.tsv, enums.tsv, bitfields.tsv. With
      --tables also the human-readable tables (the same as `render`).
  diff BASE NEW [--strict]
      Print the delta report. BASE and NEW are snapshot directories or driver trees (a
      directory with version.mk is snapshotted to a temporary directory first). Exit 1 when
      a struct the device implements changed size or field offsets (see `implemented()`),
      or, with --strict, on any difference of the driver side (the check.sh drift step);
      0 otherwise.
  render SNAPSHOT [--out FILE] [--check FILE]
      The snapshot as markdown tables; --check compares with FILE and exits 1 if it differs.

Sources. The inventory (controls, RPC functions, events, classes and the device's answer
today) is caraxes/tools/gsp-surface.py, imported, not copied; its crate reading decides the
provenance column. Struct sizes and layouts are measured by the compiler: the control and
class parameter structs against the tree's SDK headers (cc for sizeof, clang's record-layout
dump for the layout hash), the RPC, event, transport and boot structs with the minimal flag
set of caraxes/tools/gsp-abi-sizes.sh (`--print-flags`), whose own output gives the field
offsets of the boot structs and the derived constants; caraxes/tools/ctrl-layouts.py gives
the field offsets of the control structs the device lays out. Nothing is typed by hand.

The driver-side columns depend only on the tree; the device-side columns (provenance,
device, answered_by) depend on crates/vfio-user-nvgpu at the time of the snapshot. `diff`
reports the two apart, and --strict fails only on the driver side.

Needs python 3.11, cc and clang. Reads only; no network, no GPU.
"""

import argparse
import collections
import contextlib
import glob
import hashlib
import importlib.util
import io
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
TOOLS = os.path.join(ROOT, "caraxes", "tools")
CRATE = os.path.join(ROOT, "crates", "vfio-user-nvgpu")

FILES = ["controls.tsv", "rpc.tsv", "events.tsv", "classes.tsv", "boot.tsv", "layouts.tsv",
         "constants.tsv", "enums.tsv", "bitfields.tsv"]

# Columns per file; the key column first. DEVICE columns are the crate's reading, SRC columns
# are file:line citations; neither is part of the driver-side comparison.
COLUMNS = {
    "controls.tsv": ["id", "name", "nvoc_class", "params", "size", "layout", "route",
                     "route_why", "flags", "privilege", "category", "provenance",
                     "answered_by", "src"],
    "rpc.tsv": ["num", "name", "struct", "size", "layout", "versions", "gsp_client",
                "device", "src"],
    "events.tsv": ["num", "name", "struct", "size", "layout", "cpu_dispatch", "device", "src"],
    "classes.tsv": ["name", "id", "internal", "route", "reaches_gsp", "alloc_params", "size",
                    "layout", "tu106", "device", "src"],
    "boot.tsv": ["name", "kind", "size", "layout", "version", "stable", "device", "src"],
    "layouts.tsv": ["key", "struct", "path", "offset", "size", "origin"],
    "constants.tsv": ["name", "value", "origin"],
    "enums.tsv": ["key", "enum", "enumerator", "value"],
    "bitfields.tsv": ["name", "bits", "src"],
}
DEVICE_COLS = {"provenance", "answered_by", "device"}
SRC_COLS = {"src"}

DESCRIPTIONS = {
    "controls.tsv": "one row per exported control id (and per control the kernel RM sends "
                    "to GSP without an export entry)",
    "rpc.tsv": "every RPC function of rpc_global_enums.h",
    "events.tsv": "every RPC event (GSP to CPU) of rpc_global_enums.h",
    "classes.tsv": "every RS_ENTRY of resource_list.h",
    "boot.tsv": "the NV_ABI_STABLE types and the transport and boot structs the device "
                "implements",
    "layouts.tsv": "field offsets of the boot structs (gsp-abi-sizes.sh) and of the control "
                   "structs the device lays out (ctrl-layouts.py)",
    "constants.tsv": "derived transport constants (gsp-abi-sizes.sh) and version constants",
    "enums.tsv": "the enumerators of the NV_ABI_STABLE enums",
    "bitfields.tsv": "the DRF bit ranges of the control headers (flag-word layouts)",
}


def load_module(name, filename):
    sys.dont_write_bytecode = True   # no __pycache__ next to the tools
    spec = importlib.util.spec_from_file_location(name, os.path.join(TOOLS, filename))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def read(p):
    with open(p, encoding="utf-8", errors="replace") as f:
        return f.read()


def sha256_file(p):
    with open(p, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def short_hash(text):
    return hashlib.sha256(text.encode()).hexdigest()[:12]


def tree_version(ogkm):
    m = re.search(r"^NVIDIA_VERSION\s*=\s*(\S+)", read(os.path.join(ogkm, "version.mk")), re.M)
    if not m:
        sys.exit("%s: no NVIDIA_VERSION in version.mk" % ogkm)
    return m.group(1)


def run(cmd, **kw):
    r = subprocess.run(cmd, capture_output=True, text=True, **kw)
    if r.returncode != 0:
        sys.exit("%s failed (%d):\n%s" % (" ".join(cmd[:3]), r.returncode, r.stderr[-4000:]))
    return r


# ---------------------------------------------------------------------------------------
# Compiler measurements.

ANON = re.compile(r"\((?:unnamed|anonymous)(?: struct| union)? at [^)]*\)")


def parse_record_dump(out):
    """clang -fdump-record-layouts output -> {raw record name: (size, [body lines])}."""
    recs = {}
    for block in out.split("*** Dumping AST Record Layout")[1:]:
        lines = [l for l in block.split("\n") if "|" in l]
        if not lines:
            continue
        head = lines[0].split("|", 1)[1].strip()
        name = re.sub(r"^(struct|union) ", "", head)
        body, size = [], None
        for l in lines[1:]:
            off, rest = l.split("|", 1)
            m = re.match(r"\s*\[sizeof=(\d+)", rest)
            if m:
                size = int(m.group(1))
                continue
            body.append((off.strip(), rest.rstrip()))
        recs.setdefault(name, (size, body))
    return recs


def layout_signature(recs, name, memo, stack=()):
    """A text that changes when any offset, type or field name of `name` changes, including
    those inside array elements of record type; paths and line numbers of anonymous records
    are left out."""
    if name in memo:
        return memo[name]
    if name not in recs or name in stack:
        return ANON.sub("(anon)", name)
    size, body = recs[name]
    out = ["sizeof=%s" % size]
    for off, rest in body:
        out.append("%s|%s" % (off, ANON.sub("(anon)", rest)))
        m = re.match(r"\s*(?:struct |union )?(.+?)\[[\d\]\[]*\]\s+\w+$", rest)
        if m and m.group(1).strip() in recs:
            out.append("{" + layout_signature(recs, m.group(1).strip(), memo,
                                              stack + (name,)) + "}")
    memo[name] = "\n".join(out)
    return memo[name]


def measure(names, prelude, flags, workdir, tag, clang_extra=()):
    """sizeof and layout hash of every type in `names`, compiled after `prelude`.
    Names the compiler rejects are dropped one round at a time; they come back as None."""
    names = sorted(set(names))
    bad = set()
    for _ in range(8):
        ok = [n for n in names if n not in bad]
        src = prelude + "\nint printf(const char *, ...);\n"
        src += "unsigned long caraxes_sizes[] = {%s};\n" % ", ".join(
            "sizeof(%s)" % n for n in ok)
        src += "int main(void) {\n" + "".join(
            '    printf("%s %%lu\\n", (unsigned long)sizeof(%s));\n' % (n, n) for n in ok)
        src += "    return 0;\n}\n"
        c = os.path.join(workdir, tag + ".c")
        with open(c, "w") as f:
            f.write(src)
        r = subprocess.run(["cc", "-std=gnu11", "-O0"] + flags + ["-o", c[:-2], c],
                           capture_output=True, text=True)
        if r.returncode == 0:
            break
        q, e = "[\u2018']", "[\u2019']"
        found = set(re.findall(q + r"(\w+)" + e + r" undeclared|unknown type name " + q + r"(\w+)" + e +
                               r"|incomplete type " + q + r"(?:struct )?(\w+)" + e, r.stderr))
        found = {x for t in found for x in t if x} & set(ok)
        if not found:
            sys.exit("%s: compile failed:\n%s" % (tag, r.stderr[:4000]))
        bad |= found
    else:
        sys.exit("%s: compile still failing" % tag)
    sizes = {}
    for line in run([c[:-2]]).stdout.split("\n"):
        p = line.split()
        if len(p) == 2:
            sizes[p[0]] = int(p[1])
    dump = run(["clang", "-std=gnu11", "-fsyntax-only", "-Xclang", "-fdump-record-layouts",
                "-Wno-everything"] + list(clang_extra) + flags + [c]).stdout
    recs = parse_record_dump(dump)
    memo = {}
    out = {}
    for n in names:
        if n in bad:
            out[n] = (None, None)
        else:
            sig = layout_signature(recs, n, memo) if n in recs else "sizeof=%d" % sizes[n]
            out[n] = (sizes[n], short_hash(sig))
    return out


def sdk_prelude(sdk):
    hs = ["nvos.h"] + sorted(os.path.relpath(p, sdk) for d in ("ctrl", "class", "alloc")
                             for p in glob.glob(os.path.join(sdk, d, "**", "*.h"),
                                                recursive=True))
    return "#include <nvtypes.h>\n" + "".join("#include <%s>\n" % h for h in hs)


def finn_params(sdk):
    """Control name -> the params struct its FINN id is evaluated from."""
    rx = re.compile(r"#define\s+(NV\w+)\s+\(0x[0-9a-fA-F]+\)\s*/\* finn: Evaluated from "
                    r"\"[^\"]*\|\s*(\w+)_MESSAGE_ID\"")
    out = {}
    for p in glob.glob(os.path.join(sdk, "ctrl", "**", "*.h"), recursive=True):
        for m in rx.finditer(read(p)):
            out.setdefault(m.group(1), m.group(2))
    return out


def gmcapi_macros(ogkm):
    """The object-like NVGMC_*/GMCAPI_* macros of the GMCAPI headers that are not bit ranges
    (NVKV keys, GMCAPI command and category numbers), with the header they come from (the
    *_unstable.h ones are the unstable keys); their values are read by the compiler."""
    out = {}
    d = os.path.join(ogkm, "src", "nvidia", "interface", "gmcapi")
    rx = re.compile(r"^[ \t]*#define[ \t]+((?:NVGMC|GMCAPI)_\w+)[ \t]+([^\n]*)$", re.M)
    for p in sorted(glob.glob(os.path.join(d, "*.h"))):
        for m in rx.finditer(read(p)):
            v = re.sub(r"/\*.*?\*/|//.*", "", m.group(2)).strip()
            if not v or re.fullmatch(r"\(?\d+\s*:\s*\d+\)?", v) or '"' in v:
                continue
            out.setdefault(m.group(1), os.path.basename(p))
    return out


def sdk_defined_types(sdk):
    names = set()
    for p in glob.glob(os.path.join(sdk, "**", "*.h"), recursive=True):
        t = read(p)
        names.update(re.findall(r"}\s*(\w+)\s*;", t))
        names.update(re.findall(r"\btypedef\s+(?:struct\s+|union\s+)?\w+\s+(\w+)\s*;", t))
    return names


INTERNAL_HEADERS = [  # the translation unit of gsp-abi-sizes.c, plus the GMCAPI headers
    ("nvtypes.h", ""), ("gpu/gsp/kernel_gsp.h", ""), ("vgpu/rpc.h", ""),
    ("g_rpc-structures.h", "RPC_STRUCTURES RPC_GENERIC_UNION"),
    ("g_rpc-message-header.h", "RPC_MESSAGE_STRUCTURES RPC_MESSAGE_GENERIC_UNION"),
    ("gpu/gsp/message_queue_priv.h", ""), ("msgq/msgq_priv.h", ""),
    ("gmcapi/gmcapi_base.h", ""), ("gmcapi/gmcapi_engine_types.h", ""),
    ("gmcapi/gmcapi_gsp.h", ""), ("gmcapi/gmcapi_vgpu.h", ""), ("mctp_format.h", ""),
    ("nvdm_format.h", ""),
    ("gpu/gsp/gsp_static_config.h", ""), ("gpu/gsp/gsp_init_args.h", ""),
    ("gsp/gsp_fw_wpr_meta.h", ""), ("gsp/gspifpub.h", ""), ("libos_init_args.h", ""),
    ("vgpu/vgpu_version.h", ""),
]


def internal_flags(ogkm):
    out = run([os.path.join(TOOLS, "gsp-abi-sizes.sh"), "--print-flags", ogkm]).stdout
    return [l for l in out.split("\n") if l]


def internal_prelude(flags):
    dirs = [f[2:] for f in flags if f.startswith("-I")]
    src = []
    for h, defs in INTERNAL_HEADERS:
        if not any(os.path.exists(os.path.join(d, h)) for d in dirs):
            continue
        for d in defs.split():
            src.append("#define %s" % d)
        src.append("#include \"%s\"" % h)
        for d in defs.split():
            src.append("#undef %s" % d)
    return "\n".join(src) + "\n"


def macro_values(names, prelude, flags, workdir):
    """Integer values of the macros and enumerators in `names` that the prelude defines."""
    src = prelude + "\nint printf(const char *, ...);\nint main(void) {\n"
    for n in names:
        src += ("#ifdef %s\n    printf(\"%s %%llu\\n\", (unsigned long long)(%s));\n#endif\n"
                % (n, n, n))
    src += "    return 0;\n}\n"
    c = os.path.join(workdir, "macros.c")
    with open(c, "w") as f:
        f.write(src)
    run(["cc", "-std=gnu11", "-O0"] + flags + ["-o", c[:-2], c])
    out = {}
    for line in run([c[:-2]]).stdout.split("\n"):
        p = line.split()
        if len(p) == 2:
            out[p[0]] = int(p[1])
    return out


def enum_values(names, prelude, flags, workdir):
    src = prelude + "\nint printf(const char *, ...);\nint main(void) {\n"
    for n in names:
        src += "    printf(\"%s %%lld\\n\", (long long)(%s));\n" % (n, n)
    src += "    return 0;\n}\n"
    c = os.path.join(workdir, "enums.c")
    with open(c, "w") as f:
        f.write(src)
    run(["cc", "-std=gnu11", "-O0"] + flags + ["-o", c[:-2], c])
    return {p[0]: int(p[1]) for p in (l.split() for l in run([c[:-2]]).stdout.split("\n"))
            if len(p) == 2}


# ---------------------------------------------------------------------------------------
# Tree-side text sources.

def find_definition(ogkm, dirs, name):
    """file:line (tree-relative) of the typedef or enum that defines `name`."""
    n = re.escape(name)
    rx = re.compile(r"(?:^[ \t]*(?:typedef[ \t]+)?(?:struct|union|enum)[ \t]+(?:NV_ABI_STABLE[ \t]+)?%s\b(?![ \t]*[*;])"
                    r"|^[ \t]*\}[ \t]*%s[ \t]*;|^[ \t]*typedef[ \t]+\w+[ \t]+%s[ \t]*;)" % (n, n, n), re.M)
    for d in dirs:
        for p in sorted(glob.glob(os.path.join(ogkm, d, "**", "*.h"), recursive=True)):
            t = read(p)
            m = rx.search(t)
            if m:
                return "%s:%d" % (os.path.relpath(p, ogkm), t.count("\n", 0, m.start()) + 1)
    return "-"


def stable_carriers(ogkm):
    """(name, kind, file:line) of every type tagged NV_ABI_STABLE in the tree."""
    out = []
    rx = re.compile(r"^[ \t]*(?:typedef[ \t]+)?(struct|enum|union)[ \t]+NV_ABI_STABLE[ \t]+(\w+)", re.M)
    for d in ("src", "kernel-open"):
        for p in sorted(glob.glob(os.path.join(ogkm, d, "**", "*.h"), recursive=True)):
            t = read(p)
            if "NV_ABI_STABLE" not in t:
                continue
            for m in rx.finditer(t):
                out.append((m.group(2), "enum" if m.group(1) == "enum" else "struct",
                            "%s:%d" % (os.path.relpath(p, ogkm), t.count("\n", 0, m.start()) + 1)))
    return out


def enum_names(ogkm, src):
    """Enumerator names of the enum defined at `src` (file:line)."""
    path, line = src.rsplit(":", 1)
    t = read(os.path.join(ogkm, path)).split("\n")[int(line) - 1:]
    body = "\n".join(t)
    body = body[body.index("{") + 1:body.index("}")]
    body = re.sub(r"/\*.*?\*/|//[^\n]*", "", body, flags=re.S)
    names = [m.group(1) for m in re.finditer(r"^\s*(\w+)\s*(?:=|,|$)", body, re.M)]
    # An X-macro table included into the enum (GMCAPI_COMMANDS: gmcapi_table.h, each entry
    # `/* symbolic_id = */ NAME`, pasted as GMCAPI_CMD_ ## symbolic_id, gmcapi_base.h:82-84).
    pre = re.search(r"(\w+)\s*##\s*symbolic_id", body)
    for inc in re.findall(r'#include\s+"([^"]+)"', body):
        hits = glob.glob(os.path.join(ogkm, "src", "**", inc), recursive=True)
        if pre and hits:
            names += [pre.group(1) + s for s in
                      re.findall(r"symbolic_id\s*=\s*\*/\s*(\w+)", read(hits[0]))]
    return names


def bitfields(sdk, ogkm):
    rx = re.compile(r"^[ \t]*#define[ \t]+((?:NV|GMCAPI_)\w+)[ \t]+\(?(\d+)[ \t]*:[ \t]*(\d+)\)?[ \t]*(?:$|/)", re.M)
    out = {}
    paths = sorted(glob.glob(os.path.join(sdk, "ctrl", "**", "*.h"), recursive=True))
    paths += sorted(glob.glob(os.path.join(ogkm, "src", "nvidia", "interface", "gmcapi", "*.h")))
    for p in paths:
        t = read(p)
        for m in rx.finditer(t):
            out.setdefault(m.group(1), ("%s:%s" % (m.group(2), m.group(3)),
                                        "%s:%d" % (os.path.relpath(p, ogkm),
                                                   t.count("\n", 0, m.start()) + 1)))
    return out


def class_list_tu106(ogkm):
    """Class names of the TU106 class database (g_gpu_class_list.c: the generic list and the
    TU106 no-engine and engine lists), as chips/tu106/chip.toml [classes] cites them."""
    p = os.path.join(ogkm, "src", "nvidia", "generated", "g_gpu_class_list.c")
    t = read(p)
    names = set()
    for fn in ("gpuGetGenericClassList_IMPL", "gpuGetNoEngClassList_TU106",
               "gpuGetEngClassDescriptorList_TU106"):
        i = t.find("\n" + fn + "(")
        if i < 0:
            continue
        body = t[i:t.find("\n}\n", i)]
        body = body[body.index("{", body.index("[]")):]
        names.update(re.findall(r"\b([A-Z][A-Z0-9_]+)\b", body))
    names.discard("ENG_GR")
    return {n for n in names if not n.startswith("ENG_")}


def alloc_params(ogkm):
    """resource_list.h line -> the alloc param struct of that RS_ENTRY."""
    p = os.path.join(ogkm, "src", "nvidia", "src", "kernel", "rmapi", "resource_list.h")
    t = read(p)
    out = {}
    for m in re.finditer(r"RS_ENTRY\((.*?)\n\)", t, re.S):
        line = t.count("\n", 0, m.start()) + 1
        pm = re.search(r"Alloc Param Info\s*\*/\s*(RS_REQUIRED|RS_OPTIONAL|RS_NONE)"
                       r"\s*(?:\(\s*(\w+)\s*\))?", m.group(1))
        out[line] = pm.group(2) if pm and pm.group(2) else "-"
    return out


# ---------------------------------------------------------------------------------------
# Snapshot.

# The version constants that belong to a boot or transport struct (printed when defined).
VERSION_OF = {
    "msgqTxHeader": ["MSGQ_VERSION_MAJOR", "MSGQ_VERSION_MINOR", "MSGQ_VERSION"],
    "GSP_MSG_QUEUE_ELEMENT": ["MCTP_MAGIC", "NVDM_TYPE_RM_RPC"],
    "rpc_message_header_v": ["NV_VGPU_MSG_HEADER_VERSION_MAJOR_TOT",
                             "NV_VGPU_MSG_HEADER_VERSION_MINOR_TOT", "NV_VGPU_MSG_SIGNATURE_VALID"],
    "GSP_ARGUMENTS_CACHED": ["GSP_ARGUMENTS_MAGIC_VALUE"],
    "GspFwWprMeta": ["GSP_FW_WPR_META_REVISION", "GSP_FW_WPR_META_MAGIC"],
    "GMCAPI_HEADER": ["NVDM_TYPE_GMCAPI"],
}
# Types the device reads or writes besides those gsp-abi-sizes.sh lays out, and the
# NV_ABI_STABLE carriers of 615, measured in every tree that defines them.
BOOT_EXTRA = ["GSP_ACR_BOOT_GSP_RM_PARAMS", "GSP_RM_PARAMS", "GSP_SPDM_PARAMS",
              "GSP_RM_MEM_PARAMS", "GmcApiGspSuspendParams", "GmcApiGspResumeDoneParams"]
# Boot and transport types the device does not implement on TU106, and why.
NOT_DEVICE = {
    "GSP_MSG_QUEUE_ENCRYPTION_TAG": "no (Confidential Computing)",
    "GSP_FMC_INIT_PARAMS": "no (GSP-FMC boot, Hopper and later)",
    "GSP_FMC_BOOT_PARAMS": "no (GSP-FMC boot, Hopper and later)",
    "GSP_ACR_BOOT_GSP_RM_PARAMS": "no (GSP-FMC boot, Hopper and later)",
    "GSP_RM_PARAMS": "no (GSP-FMC boot, Hopper and later)",
    "GSP_SPDM_PARAMS": "no (GSP-FMC boot, Hopper and later)",
    "GSP_RM_MEM_PARAMS": "no (GSP-FMC boot, Hopper and later)",
    "GMCAPI_VGPU_PLUGIN_EVENT": "no (vGPU host)",
    "NV2080_CTRL_CMD_FB_GET_FB_REGION_FB_REGION_INFO": "yes (inside GspStaticConfigInfo)",
}
INTERNAL_DIRS = ["src/nvidia/inc", "src/nvidia/arch", "src/nvidia/interface",
                 "src/nvidia/generated", "src/common"]


def parse_sizes_md(md):
    """gsp-abi-sizes.sh output -> ({struct: (size, [(field, offset, size)])}, {const: value})."""
    structs, consts = {}, {}
    cur = None
    in_consts = False
    for line in md.split("\n"):
        m = re.match(r"#### `(\w+)`: sizeof (\d+)", line)
        if m:
            cur = m.group(1)
            structs[cur] = (int(m.group(2)), [])
            continue
        if line.startswith("### "):
            cur = None
            in_consts = "Derived constants" in line
            continue
        if cur:
            m = re.match(r"\| (\d+) \| 0x[0-9a-f]+ \| (\d+|flex|\.\.\.) \| `([^`]+)`", line)
            if m:
                structs[cur][1].append((m.group(3), int(m.group(1)), m.group(2)))
        elif in_consts:
            m = re.match(r"\| `([^`]+)` \| (\d+) \| (0x[0-9a-f]+) \|", line)
            if m:
                consts[m.group(1)] = int(m.group(2))
    return structs, consts


def parse_ctrl_layouts_rs(rs):
    out = {}
    for m in re.finditer(r'CtrlLayout \{\s*name: "(\w+)",.*?size: (\d+),\s*fields: &\[(.*?)\],\s*\}',
                         rs, re.S):
        fields = []
        for f in re.finditer(r'CtrlField \{ path: "([^"]+)", offset: (\d+), size: (\d+), dims: &\[([^\]]*)\] \}',
                             m.group(3)):
            dims = "".join("[%sx%s]" % d for d in re.findall(r"\((\d+), (\d+)\)", f.group(4)))
            fields.append((f.group(1) + dims, int(f.group(2)), f.group(3)))
        out[m.group(1)] = (int(m.group(2)), fields)
    return out


def ctrl_layouts_of(ogkm, workdir):
    out = os.path.join(workdir, "ctrl_layouts.rs")
    r = subprocess.run([sys.executable, os.path.join(TOOLS, "ctrl-layouts.py"), "--ogkm", ogkm,
                        "--out", out, "--skip-missing"], capture_output=True, text=True)
    if r.returncode != 0:
        print("warning: ctrl-layouts.py failed for this tree, no control layouts: %s"
              % (r.stderr.strip() or r.stdout.strip())[-300:], file=sys.stderr)
        return {}
    return parse_ctrl_layouts_rs(read(out))


def version_major(driver):
    m = driver.split(".")[0]
    return int(m) if m.isdigit() else 0


def fmt_size(s):
    return "-" if s is None else str(s)


def snapshot(ogkm, outdir, tables=None):
    ogkm = os.path.abspath(ogkm)
    driver = tree_version(ogkm)
    sdk = os.path.join(ogkm, "src", "common", "sdk", "nvidia", "inc")
    surface = load_module("gsp_surface", "gsp-surface.py")
    surface.select_tree(ogkm)
    inv = surface.build()
    vprefix = surface.rel(ogkm).rstrip("/") + "/"

    def tsrc(s):
        return s.replace(vprefix, "").replace(ogkm + "/", "") if s else "-"

    work = tempfile.mkdtemp(prefix="gsp-abi.")
    try:
        # Control and class parameter structs against the SDK headers.
        rlp = alloc_params(ogkm)
        want = set()
        defined = sdk_defined_types(sdk)
        finn = finn_params(sdk)
        for r in inv["controls"]:
            if not r["params"] and "(not exported)" in r["nvoc_class"]:
                # No export entry: the struct FINN evaluated the id from, else the SDK's
                # naming convention (NV2080_CTRL_CMD_X -> NV2080_CTRL_X_PARAMS).
                guess = finn.get(r["name"]) or r["name"].replace("_CTRL_CMD_", "_CTRL_", 1) + "_PARAMS"
                r["params"] = guess if guess in defined else ""
            if re.fullmatch(r"[A-Za-z_]\w*", r["params"] or ""):
                want.add(r["params"])
        for c in inv["classes"]:
            line = int(c["src"].rsplit(":", 1)[1])
            c["alloc_params"] = rlp.get(line, "-")
            if c["alloc_params"] != "-":
                want.add(c["alloc_params"])
        sdk_meas = measure([n for n in want if n in defined], sdk_prelude(sdk),
                           ["-DNV_LINUX", "-I", sdk], work, "sdk")
        for n in want - set(sdk_meas):
            sdk_meas[n] = (None, None)

        # RPC, event, transport and boot structs with the flags of gsp-abi-sizes.sh.
        iflags = internal_flags(ogkm)
        ipre = internal_prelude(iflags)
        rpc_structs = set()
        for f in inv["rpc_functions"] + inv["rpc_events"]:
            rpc_structs.update(f["structs"])
        sizes_md = run([os.path.join(TOOLS, "gsp-abi-sizes.sh"), ogkm]).stdout
        boot_layouts, derived = parse_sizes_md(sizes_md)
        carriers = stable_carriers(ogkm)
        boot_names = list(boot_layouts) + [n for n, k, _ in carriers if k == "struct"]
        boot_names += BOOT_EXTRA
        # Under clang nvport takes its C11 atomics header, which expects <stdatomic.h>
        # included first (nvport/atomic.h:75-80, :396-397); the layouts do not depend on it.
        int_meas = measure(list(rpc_structs) + boot_names, ipre, iflags, work, "internal",
                           ["-include", "stdatomic.h"])

        vnames = sorted({v for vs in VERSION_OF.values() for v in vs} |
                        {"VGX_MAJOR_VERSION_NUMBER", "VGX_MINOR_VERSION_NUMBER"})
        gmc = gmcapi_macros(ogkm)
        macros = macro_values(vnames + sorted(gmc), ipre, iflags, work)
        enum_rows = []
        stable_enums = [(n, s) for n, k, s in carriers if k == "enum"]
        enumerators = {n: enum_names(ogkm, s) for n, s in stable_enums}
        evals = enum_values([e for es in enumerators.values() for e in es], ipre, iflags, work) \
            if enumerators else {}
        ctrl_lay = ctrl_layouts_of(ogkm, work)
    finally:
        shutil.rmtree(work, ignore_errors=True)

    rows = collections.OrderedDict((f, []) for f in FILES)

    # controls.tsv
    for r in sorted(inv["controls"], key=lambda r: r["id"]):
        p = r["params"] if re.fullmatch(r"[A-Za-z_]\w*", r["params"] or "") else "-"
        size, lay = sdk_meas.get(p, (0 if p == "-" else None, "-" if p == "-" else None))
        if r["handled"]:
            prov = r["provenance"]
            h = surface.primary(r["handlers"])
            by = h["module"] + (":" + h["guard"] if h.get("guard") else "")
        else:
            prov = "unhandled" if r["routed"] else "-"
            by = "-"
        rows["controls.tsv"].append([
            "0x%08X" % r["id"], r["name"], ",".join(dict.fromkeys(r["classes"])), p,
            fmt_size(size), lay or "-", "gsp" if r["routed"] else "kernel",
            ",".join(r["why"]) or "-", "0x%x" % r["flags"], r["privilege"],
            r.get("class_category", "-") if r["routed"] else "-", prov, by, tsrc(r["src"])])

    # rpc.tsv and events.tsv
    def vkey(s):
        return tuple(int(x, 16) for x in re.search(r"_v([0-9A-Fa-f]{2})_([0-9A-Fa-f]{2})$", s).groups())

    for f in inv["rpc_functions"]:
        st = max(f["structs"], key=vkey) if f["structs"] else "-"
        size, lay = int_meas.get(st, (None, None)) if st != "-" else (None, None)
        rows["rpc.tsv"].append([
            str(f["num"]), f["name"], st, fmt_size(size), lay or "-",
            ",".join(sorted((re.search(r"_(v\w\w_\w\w)$", s).group(1) for s in f["structs"]),
                            key=lambda v: (int(v[1:3], 16), int(v[4:6], 16)))) or "-",
            f["gsp_client"] or "-", f["device"] or "-", tsrc(f["src"])])
    for e in inv["rpc_events"]:
        st = max(e["structs"], key=vkey) if e["structs"] else "-"
        size, lay = int_meas.get(st, (None, None)) if st != "-" else (None, None)
        rows["events.tsv"].append([
            "0x%04X" % e["num"], e["name"], st, fmt_size(size), lay or "-",
            "yes" if e["cpu_dispatch"] else "no", "sent" if e["device"] else "-",
            tsrc(e["src"])])

    # classes.tsv
    chip = tomllib.loads(read(os.path.join(CRATE, "chips", "tu106", "chip.toml")))
    chip_db = set(chip.get("classes", {}).get("ids", []))
    tu106 = class_list_tu106(ogkm)
    dm = os.path.join(CRATE, "src", "ver", "v%d" % version_major(driver), "dmodel.rs")
    profiler = set()
    if os.path.exists(dm):
        blk = read(dm).split("pub const PROFILER_CLASSES", 1)[-1].split("];", 1)[0]
        profiler = {int(v, 16) for v in re.findall(r"\((0x[0-9A-Fa-f]+),", blk)}
    for c in sorted(inv["classes"], key=lambda c: c["class"]):
        if c["rpc_to_phys"] and c["rpc_to_vgpu_host"]:
            route = "RPC_TO_ALL"
        elif c["rpc_to_phys"]:
            route = "RPC_TO_PHYS_RM"
        elif c["internal"] in surface.CTOR_RPC_INTERNAL:
            route = "constructor RPC"
        elif c["rpc_to_vgpu_host"]:
            route = "RPC_TO_VGPU_HOST"
        else:
            route = "kernel"
        cid = c["id"]
        if not c["reaches_gsp"]:
            dev = "-"
        elif cid in profiler:
            dev = "refused (profiling, D table)"
        elif cid is not None and cid not in chip_db:
            dev = "refused (not in the chip's class database)"
        elif c["crate"] == "named":
            dev = "named"
        else:
            dev = "acknowledged"
        ap = c["alloc_params"]
        size, lay = sdk_meas.get(ap, (None, None)) if ap != "-" else (None, None)
        rows["classes.tsv"].append([
            c["class"], "0x%04X" % cid if cid is not None else "-", c["internal"], route,
            "yes" if c["reaches_gsp"] else "no", ap, fmt_size(size), lay or "-",
            "yes" if c["class"] in tu106 else "no", dev, tsrc(c["src"])])

    # boot.tsv, layouts.tsv, constants.tsv, enums.tsv
    carrier_src = {n: s for n, _, s in carriers}
    seen = set()
    for n in boot_names:
        if n in seen:
            continue
        seen.add(n)
        size, lay = int_meas.get(n, (None, None))
        if size is None:
            continue
        ver = ",".join("%s=0x%x" % (v, macros[v]) for v in VERSION_OF.get(n, []) if v in macros)
        m = re.search(r"_(v[0-9A-Fa-f]{2}_[0-9A-Fa-f]{2})$", n)
        if m:
            ver = m.group(1)
        src = carrier_src.get(n) or find_definition(ogkm, INTERNAL_DIRS, n)
        rows["boot.tsv"].append([n, "struct", str(size), lay, ver or "-",
                                 "yes" if n in carrier_src else "no",
                                 NOT_DEVICE.get(n, "yes"), src])
    for n, s in stable_enums:
        es = enumerators[n]
        sig = ",".join("%s=%d" % (e, evals[e]) for e in es if e in evals)
        rows["boot.tsv"].append([n, "enum", "-", short_hash(sig), "-", "yes",
                                 NOT_DEVICE.get(n, "yes"), s])
        for e in es:
            if e in evals:
                rows["enums.tsv"].append(["%s.%s" % (n, e), n, e, str(evals[e])])
    for n, (size, fields) in boot_layouts.items():
        rows["layouts.tsv"].append(["%s.(sizeof)" % n, n, "(sizeof)", "0", str(size),
                                    "gsp-abi-sizes.sh"])
        for fld, off, sz in fields:
            rows["layouts.tsv"].append(["%s.%s" % (n, fld), n, fld, str(off), sz,
                                        "gsp-abi-sizes.sh"])
    for n, (size, fields) in sorted(ctrl_lay.items()):
        rows["layouts.tsv"].append(["%s.(sizeof)" % n, n, "(sizeof)", "0", str(size),
                                    "ctrl-layouts.py"])
        for fld, off, sz in fields:
            rows["layouts.tsv"].append(["%s.%s" % (n, fld), n, fld, str(off), sz,
                                        "ctrl-layouts.py"])
    for k, v in derived.items():
        rows["constants.tsv"].append([k, "0x%x" % v, "gsp-abi-sizes.sh"])
    for k in vnames:
        if k in macros and k not in derived:
            rows["constants.tsv"].append([k, "0x%x" % macros[k], "header macro"])
    for k, h in sorted(gmc.items()):
        if k in macros and k not in derived and k not in vnames:
            rows["constants.tsv"].append([k, "0x%x" % macros[k], h])

    # bitfields.tsv
    for n, (bits, src) in sorted(bitfields(sdk, ogkm).items()):
        rows["bitfields.tsv"].append([n, bits, src])

    os.makedirs(outdir, exist_ok=True)
    for f, rs in rows.items():
        write_tsv(os.path.join(outdir, f), f, driver, rs)
    write_manifest(outdir, ogkm, driver, rows, inv)
    if tables:
        write_text(tables, render(outdir))
    counts = summary_counts(load_snapshot(outdir))
    print("%s: driver %s, %s" % (outdir, driver, counts_line(counts)), file=sys.stderr)


def write_tsv(path, fname, driver, rows):
    head = ["# SPDX-License-Identifier: MIT",
            "# %s of open-gpu-kernel-modules %s: %s." % (fname, driver, DESCRIPTIONS[fname]),
            "# Generated by caraxes/tools/gsp-abi.py snapshot; do not edit, rerun it "
            "(docs/caraxes/gsp-abi-615.md)."]
    with open(path, "w") as f:
        f.write("\n".join(head) + "\n")
        f.write("\t".join(COLUMNS[fname]) + "\n")
        for r in rows:
            f.write("\t".join(str(x) for x in r) + "\n")


def write_text(path, text):
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with open(path, "w") as f:
        f.write(text)


def git_head():
    try:
        head = run(["git", "rev-parse", "HEAD"], cwd=ROOT).stdout.strip()
        dirty = run(["git", "status", "--porcelain", "--untracked-files=no", "--", "crates",
                     "caraxes/tools"],
                    cwd=ROOT).stdout.strip()
        return head + (" (with uncommitted changes under crates/ or caraxes/tools/)" if dirty else "")
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def tool_version(cmd):
    try:
        return run([cmd, "--version"]).stdout.split("\n")[0]
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def write_manifest(outdir, ogkm, driver, rows, inv):
    lines = [
        "# SPDX-License-Identifier: MIT",
        "# The GSP ABI snapshot of one open-gpu-kernel-modules tree (docs/caraxes/gsp-abi-615.md).",
        "# Generated by caraxes/tools/gsp-abi.py snapshot; do not edit, rerun it.",
        "driver: %s" % driver,
        "version_mk_sha256: %s" % sha256_file(os.path.join(ogkm, "version.mk")),
        "repository_commit: %s" % git_head(),
        "generator: python3 caraxes/tools/gsp-abi.py snapshot --ogkm <the %s tree> "
        "--out caraxes/abi/gsp-%s/" % (driver, driver),
        "compilers: %s; %s" % (tool_version("cc"), tool_version("clang")),
    ]
    for f in FILES:
        lines.append("file: %s rows %d sha256 %s" % (
            f, len(rows[f]), sha256_file(os.path.join(outdir, f))))
    write_text(os.path.join(outdir, "MANIFEST"), "\n".join(lines) + "\n")


# ---------------------------------------------------------------------------------------
# Loading and counting.

def load_tsv(path):
    rows = []
    cols = None
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if line.startswith("#") or not line:
                continue
            parts = line.split("\t")
            if cols is None:
                cols = parts
                continue
            rows.append(dict(zip(cols, parts)))
    return rows


def load_snapshot(d):
    snap = {"dir": d, "manifest": {}}
    for line in read(os.path.join(d, "MANIFEST")).split("\n"):
        if line and not line.startswith("#") and ": " in line:
            k, v = line.split(": ", 1)
            snap["manifest"].setdefault(k, v)
    for f in FILES:
        snap[f] = load_tsv(os.path.join(d, f))
    return snap


PROVENANCES = ["host-forwarded", "synthesized", "guest-derived", "echo", "refused"]


def summary_counts(s):
    c = collections.OrderedDict()
    ctl = s["controls.tsv"]
    routed = [r for r in ctl if r["route"] == "gsp"]
    c["controls"] = len(ctl)
    c["routed"] = len(routed)
    c["internal"] = sum(1 for r in routed if r["privilege"] == "internal")
    c["handled"] = sum(1 for r in routed if r["provenance"] not in ("unhandled", "-"))
    for p in PROVENANCES:
        c[p] = sum(1 for r in routed if r["provenance"] == p)
    c["unhandled"] = sum(1 for r in routed if r["provenance"] == "unhandled")
    for k in "ABCDE":
        c["cat_" + k] = sum(1 for r in routed if r["category"] == k)
        c["unhandled_" + k] = sum(1 for r in routed if r["category"] == k
                                  and r["provenance"] == "unhandled")
    rpc = s["rpc.tsv"]
    c["rpc"] = len(rpc)
    c["rpc_gsp_client"] = sum(1 for r in rpc if r["gsp_client"] != "-")
    c["rpc_device"] = sum(1 for r in rpc if r["device"] != "-")
    c["rpc_client_device"] = sum(1 for r in rpc if r["device"] != "-" and r["gsp_client"] != "-")
    ev = s["events.tsv"]
    c["events"] = len(ev)
    c["events_dispatched"] = sum(1 for r in ev if r["cpu_dispatch"] == "yes")
    c["events_device"] = sum(1 for r in ev if r["device"] != "-")
    cl = s["classes.tsv"]
    c["classes"] = len(cl)
    c["classes_gsp"] = sum(1 for r in cl if r["reaches_gsp"] == "yes")
    c["classes_gsp_tu106"] = sum(1 for r in cl if r["reaches_gsp"] == "yes" and r["tu106"] == "yes")
    for k in ("named", "acknowledged"):
        c["classes_" + k] = sum(1 for r in cl if r["device"] == k)
    c["classes_refused"] = sum(1 for r in cl if r["device"].startswith("refused"))
    b = s["boot.tsv"]
    c["boot"] = len(b)
    c["boot_stable"] = sum(1 for r in b if r["stable"] == "yes")
    return c


def counts_line(c):
    return ("%(controls)d controls, %(routed)d routed (%(handled)d handled, %(unhandled)d "
            "unhandled), %(rpc)d RPC functions (%(rpc_gsp_client)d sent by a GSP client, "
            "%(rpc_device)d handled), %(events)d events (%(events_dispatched)d dispatched, "
            "%(events_device)d sent by the device), %(classes)d classes (%(classes_gsp)d reach "
            "GSP), %(boot)d boot types (%(boot_stable)d NV_ABI_STABLE)" % c)


# ---------------------------------------------------------------------------------------
# Diff.

def keyed(rows, key):
    return collections.OrderedDict((r[key], r) for r in rows)


def implemented(row, fname):
    """Does the device implement this struct (read or write it by layout)? A control it
    answers other than by refusal, an RPC function or event it handles, a class it names,
    a boot type it uses, a laid-out struct."""
    if fname == "controls.tsv":
        return row["provenance"] not in ("unhandled", "-", "refused")
    if fname in ("rpc.tsv", "events.tsv"):
        return row["device"] != "-"
    if fname == "classes.tsv":
        return row["device"] in ("named", "acknowledged")
    if fname == "boot.tsv":
        return row["device"].startswith("yes")
    return True


class Report:
    def __init__(self):
        self.sections = collections.OrderedDict()
        self.failing = []
        self.driver_changes = 0
        self.device_changes = 0

    def add(self, section, text, fail=False, device=False):
        self.sections.setdefault(section, []).append(text)
        if device:
            self.device_changes += 1
        else:
            self.driver_changes += 1
        if fail:
            self.failing.append("%s: %s" % (section, text))


def diff_snapshots(a, b):
    rep = Report()

    # Controls.
    A, B = keyed(a["controls.tsv"], "id"), keyed(b["controls.tsv"], "id")
    names_a = {r["name"]: k for k, r in A.items()}
    names_b = {r["name"]: k for k, r in B.items()}
    for k in B:
        if k not in A:
            r = B[k]
            moved = names_a.get(r["name"])
            note = " (moved from %s)" % moved if moved else ""
            rep.add("Controls added", "%s %s%s, %s, %s bytes, %s, %s" % (
                k, r["name"], note, r["route"], r["size"], r["privilege"], r["nvoc_class"]))
    for k in A:
        if k not in B:
            r = A[k]
            moved = names_b.get(r["name"])
            note = " (moved to %s)" % moved if moved else ""
            rep.add("Controls removed", "%s %s%s, %s, %s bytes%s" % (
                k, r["name"], note, r["route"], r["size"],
                ", the device answered it (%s)" % r["provenance"] if implemented(r, "controls.tsv") else ""))
    for k in A:
        if k not in B:
            continue
        ra, rb = A[k], B[k]
        impl = implemented(ra, "controls.tsv") or implemented(rb, "controls.tsv")
        dev = " [device: %s]" % ra["provenance"] if ra["provenance"] not in ("-", "unhandled") else ""
        if ra["name"] != rb["name"]:
            rep.add("Control ids reused or renamed", "%s: %s -> %s (params %s %s -> %s %s)%s" % (
                k, ra["name"], rb["name"], ra["params"], ra["size"], rb["params"], rb["size"], dev),
                fail=impl and ra["params"] != rb["params"])
        if ra["size"] != rb["size"]:
            rep.add("Control params resized", "%s %s: %s %s -> %s bytes (%s)%s" % (
                k, rb["name"], rb["params"], ra["size"], rb["size"], rb["route"], dev), fail=impl)
        elif ra["params"] != rb["params"] and ra["name"] == rb["name"]:
            rep.add("Control params struct renamed", "%s %s: %s -> %s, %s bytes%s" % (
                k, rb["name"], ra["params"], rb["params"], rb["size"], dev))
        elif ra["layout"] != rb["layout"] and ra["params"] == rb["params"]:
            rep.add("Control params layout changed at the same size", "%s %s: %s, %s bytes%s" % (
                k, rb["name"], rb["params"], rb["size"], dev))
        if ra["privilege"] != rb["privilege"]:
            rep.add("Control privilege changed", "%s %s: %s -> %s (flags %s -> %s)%s" % (
                k, rb["name"], ra["privilege"], rb["privilege"], ra["flags"], rb["flags"], dev))
        if ra["route"] != rb["route"]:
            rep.add("Control routing changed", "%s %s: %s -> %s (%s -> %s)%s" % (
                k, rb["name"], ra["route"], rb["route"], ra["route_why"], rb["route_why"], dev))
        other = int(ra["flags"], 16) ^ int(rb["flags"], 16)
        if other & ~0xCC:  # beyond PRIVILEGED, NON_PRIVILEGED, ROUTE_TO_PHYSICAL, INTERNAL
            rep.add("Control flags changed", "%s %s: %s -> %s" % (k, rb["name"], ra["flags"], rb["flags"]))
        if ra["category"] != rb["category"] and ra["route"] == rb["route"] == "gsp":
            rep.add("Control category changed (heuristic)", "%s %s: %s -> %s" % (
                k, rb["name"], ra["category"], rb["category"]))
        if ra["provenance"] != rb["provenance"]:
            rep.add("Device answer changed", "%s %s: %s -> %s" % (
                k, rb["name"], ra["provenance"], rb["provenance"]), device=True)

    # RPC functions and events.
    for fname, what, key in (("rpc.tsv", "RPC functions", "num"), ("events.tsv", "Events", "num")):
        A, B = keyed(a[fname], key), keyed(b[fname], key)
        for k in B:
            if k not in A:
                r = B[k]
                rep.add("%s added" % what, "%s %s, %s %s bytes" % (k, r["name"], r["struct"], r["size"]))
        for k in A:
            if k not in B:
                r = A[k]
                rep.add("%s removed" % what, "%s %s" % (k, r["name"]), fail=implemented(r, fname))
        for k in A:
            if k not in B:
                continue
            ra, rb = A[k], B[k]
            impl = implemented(ra, fname) or implemented(rb, fname)
            if ra["name"] != rb["name"]:
                rep.add("%s renamed" % what, "%s: %s -> %s" % (k, ra["name"], rb["name"]), fail=impl)
            if ra["struct"] != rb["struct"]:
                rep.add("%s payload version changed" % what, "%s %s: %s %s -> %s %s bytes" % (
                    k, rb["name"], ra["struct"], ra["size"], rb["struct"], rb["size"]),
                    fail=impl and ra["size"] != rb["size"])
            elif ra["size"] != rb["size"]:
                rep.add("%s payload resized" % what, "%s %s: %s %s -> %s bytes" % (
                    k, rb["name"], rb["struct"], ra["size"], rb["size"]), fail=impl)
            elif ra["layout"] != rb["layout"]:
                rep.add("%s payload layout changed at the same size" % what, "%s %s: %s" % (
                    k, rb["name"], rb["struct"]), fail=impl)
            if fname == "rpc.tsv":
                if ra["versions"] != rb["versions"]:
                    rep.add("RPC struct versions changed", "%s %s: %s -> %s" % (
                        k, rb["name"], ra["versions"], rb["versions"]))
                if ra["gsp_client"] != rb["gsp_client"]:
                    rep.add("RPC functions a GSP client sends", "%s %s: %s -> %s" % (
                        k, rb["name"], ra["gsp_client"], rb["gsp_client"]))
            else:
                if ra["cpu_dispatch"] != rb["cpu_dispatch"]:
                    rep.add("Events the CPU side dispatches", "%s %s: %s -> %s" % (
                        k, rb["name"], ra["cpu_dispatch"], rb["cpu_dispatch"]))
            if ra["device"] != rb["device"]:
                rep.add("Device answer changed", "%s %s %s: %s -> %s" % (
                    what, k, rb["name"], ra["device"], rb["device"]), device=True)

    # Classes.
    A, B = keyed(a["classes.tsv"], "name"), keyed(b["classes.tsv"], "name")
    for k in B:
        if k not in A:
            r = B[k]
            rep.add("Classes added", "%s %s (%s, %s, reaches GSP %s, alloc %s %s bytes)" % (
                r["id"], k, r["internal"], r["route"], r["reaches_gsp"], r["alloc_params"], r["size"]))
    for k in A:
        if k not in B:
            r = A[k]
            rep.add("Classes removed", "%s %s" % (r["id"], k), fail=implemented(r, "classes.tsv"))
    for k in A:
        if k not in B:
            continue
        ra, rb = A[k], B[k]
        impl = implemented(ra, "classes.tsv") or implemented(rb, "classes.tsv")
        if ra["id"] != rb["id"]:
            rep.add("Class ids changed", "%s: %s -> %s" % (k, ra["id"], rb["id"]), fail=impl)
        if ra["reaches_gsp"] != rb["reaches_gsp"] or ra["route"] != rb["route"]:
            rep.add("Class routing changed", "%s %s: %s -> %s" % (rb["id"], k, ra["route"], rb["route"]))
        if ra["alloc_params"] != rb["alloc_params"]:
            rep.add("Class alloc params struct changed", "%s %s: %s -> %s" % (
                rb["id"], k, ra["alloc_params"], rb["alloc_params"]))
        if ra["size"] != rb["size"]:
            rep.add("Class alloc params resized", "%s %s: %s %s -> %s bytes [device: %s]" % (
                rb["id"], k, rb["alloc_params"], ra["size"], rb["size"], rb["device"]), fail=impl)
        elif ra["layout"] != rb["layout"]:
            rep.add("Class alloc params layout changed at the same size", "%s %s: %s" % (
                rb["id"], k, rb["alloc_params"]))
        if ra["tu106"] != rb["tu106"]:
            rep.add("Class database of TU106 changed", "%s %s: %s -> %s" % (rb["id"], k, ra["tu106"], rb["tu106"]))
        if ra["device"] != rb["device"]:
            rep.add("Device answer changed", "class %s %s: %s -> %s" % (
                rb["id"], k, ra["device"], rb["device"]), device=True)

    # Boot and transport types.
    A, B = keyed(a["boot.tsv"], "name"), keyed(b["boot.tsv"], "name")
    for k in B:
        if k not in A:
            r = B[k]
            rep.add("Boot and transport types added", "%s (%s, %s bytes, stable %s)" % (k, r["kind"], r["size"], r["stable"]))
    for k in A:
        if k not in B:
            rep.add("Boot and transport types removed", k, fail=implemented(A[k], "boot.tsv"))
    for k in A:
        if k not in B:
            continue
        ra, rb = A[k], B[k]
        impl = implemented(ra, "boot.tsv") or implemented(rb, "boot.tsv")
        if ra["size"] != rb["size"]:
            rep.add("Boot and transport types resized", "%s: %s -> %s bytes (stable %s -> %s)" % (
                k, ra["size"], rb["size"], ra["stable"], rb["stable"]), fail=impl)
        elif ra["layout"] != rb["layout"]:
            rep.add("Boot and transport types changed at the same size", "%s (%s)" % (k, rb["kind"]),
                    fail=impl and ra["stable"] == "yes")
        if ra["stable"] != rb["stable"]:
            rep.add("NV_ABI_STABLE tag changed", "%s: %s -> %s" % (k, ra["stable"], rb["stable"]))
        if ra["version"] != rb["version"]:
            rep.add("Version constants changed", "%s: %s -> %s" % (k, ra["version"], rb["version"]),
                    fail=impl and not re.fullmatch(r"v\w\w_\w\w", ra["version"]))

    # Field offsets of the laid-out structs.
    A, B = keyed(a["layouts.tsv"], "key"), keyed(b["layouts.tsv"], "key")
    sa = {r["struct"] for r in A.values()}
    sb = {r["struct"] for r in B.values()}
    per = collections.defaultdict(list)
    for k, r in A.items():
        if r["struct"] not in sb:
            continue
        if k not in B:
            per[r["struct"]].append("%s removed (was offset %s)" % (r["path"], r["offset"]))
        elif (r["offset"], r["size"]) != (B[k]["offset"], B[k]["size"]):
            per[r["struct"]].append("%s offset %s size %s -> offset %s size %s" % (
                r["path"], r["offset"], r["size"], B[k]["offset"], B[k]["size"]))
    for k, r in B.items():
        if r["struct"] in sa and k not in A:
            per[r["struct"]].append("%s added at offset %s" % (r["path"], r["offset"]))
    used = set()
    for snap in (a, b):
        used |= {r["name"] for r in snap["boot.tsv"] if implemented(r, "boot.tsv")}
        used |= {r["params"] for r in snap["controls.tsv"] if implemented(r, "controls.tsv")}
    for s, items in sorted(per.items()):
        shown = items[:6] + (["... %d more" % (len(items) - 6)] if len(items) > 6 else [])
        rep.add("Field offsets changed (laid-out structs)", "%s: %s%s" % (
            s, "; ".join(shown), "" if s in used else " [not used by the device]"), fail=s in used)
    for s in sorted(sb - sa):
        rep.add("Laid-out structs added", s)
    for s in sorted(sa - sb):
        rep.add("Laid-out structs no longer laid out", s)

    # Constants, enums, bitfields.
    A, B = keyed(a["constants.tsv"], "name"), keyed(b["constants.tsv"], "name")
    added = collections.defaultdict(list)
    for k in B:
        if k not in A:
            added[B[k]["origin"]].append("%s=%s" % (k, B[k]["value"]))
    for o, items in sorted(added.items()):
        rep.add("Constants added", "%d from %s: %s" % (
            len(items), o, ", ".join(items[:10]) + (" ..." if len(items) > 10 else "")))
    for k in A:
        if k not in B:
            rep.add("Constants removed", "%s (was %s, %s)" % (k, A[k]["value"], A[k]["origin"]))
        elif A[k]["value"] != B[k]["value"]:
            rep.add("Constants changed", "%s: %s -> %s (%s)" % (k, A[k]["value"], B[k]["value"], B[k]["origin"]),
                    fail=B[k]["origin"].startswith("gmcapi_") and "unstable" not in B[k]["origin"])
    A, B = keyed(a["enums.tsv"], "key"), keyed(b["enums.tsv"], "key")
    ea = {r["enum"] for r in A.values()}
    eb = {r["enum"] for r in B.values()}
    for e in sorted(eb - ea):
        n = [r for r in B.values() if r["enum"] == e]
        rep.add("Stable enums added", "%s: %d enumerators" % (e, len(n)))
    for e in sorted(ea - eb):
        rep.add("Stable enums removed", e, fail=True)
    for k in sorted(set(A) | set(B)):
        e = (A.get(k) or B.get(k))["enum"]
        if e not in ea or e not in eb:
            continue
        va = A[k]["value"] if k in A else "none"
        vb = B[k]["value"] if k in B else "none"
        if va != vb:
            rep.add("Stable enumerators changed", "%s: %s -> %s" % (k, va, vb),
                    fail=k in A and k in B)
    A, B = keyed(a["bitfields.tsv"], "name"), keyed(b["bitfields.tsv"], "name")
    added = [k for k in B if k not in A]
    removed = [k for k in A if k not in B]
    for k in A:
        if k in B and A[k]["bits"] != B[k]["bits"]:
            rep.add("Flag-word bit ranges changed", "%s: %s -> %s (%s)" % (
                k, A[k]["bits"], B[k]["bits"], B[k]["src"]))
    if added:
        rep.add("Flag-word bit ranges added", "%d names: %s" % (len(added), ", ".join(added[:8]) + (" ..." if len(added) > 8 else "")))
    if removed:
        rep.add("Flag-word bit ranges removed", "%d names: %s" % (len(removed), ", ".join(removed[:8]) + (" ..." if len(removed) > 8 else "")))
    return rep


def print_report(rep, a, b, strict):
    da, db = a["manifest"].get("driver", "?"), b["manifest"].get("driver", "?")
    print("# GSP ABI delta %s -> %s\n" % (da, db))
    print("BASE %s (%s)" % (a["dir"], a["manifest"].get("version_mk_sha256", "?")[:16]))
    print("NEW  %s (%s)\n" % (b["dir"], b["manifest"].get("version_mk_sha256", "?")[:16]))
    driver = [(s, t) for s, t in rep.sections.items() if s != "Device answer changed"]
    for s, items in driver:
        print("## %s (%d)\n" % (s, len(items)))
        for t in items:
            print("- " + t)
        print()
    dev = rep.sections.get("Device answer changed", [])
    if dev:
        print("## Device answer changed (%d; crate side, not a driver change)\n" % len(dev))
        for t in dev:
            print("- " + t)
        print()
    if rep.failing:
        print("## Changes to what the device implements (%d)\n" % len(rep.failing))
        for t in rep.failing:
            print("- " + t)
        print()
    ca, cb = summary_counts(a), summary_counts(b)
    print("Summary: %s -> %s: %d driver-side changes in %d groups, %d affecting what the device "
          "implements, %d device-side changes; routed controls %d -> %d, RPC functions %d -> %d, "
          "events %d -> %d, classes %d -> %d." % (
              da, db, rep.driver_changes, len(driver), len(rep.failing), rep.device_changes,
              ca["routed"], cb["routed"], ca["rpc"], cb["rpc"], ca["events"], cb["events"],
              ca["classes"], cb["classes"]))
    if strict:
        return 1 if rep.driver_changes else 0
    return 1 if rep.failing else 0


def resolve(path, work, tag):
    """A snapshot directory, or a driver tree snapshotted into `work`."""
    if os.path.exists(os.path.join(path, "MANIFEST")):
        return path
    if os.path.exists(os.path.join(path, "version.mk")):
        out = os.path.join(work, tag)
        snapshot(path, out)
        return out
    sys.exit("%s: neither a snapshot (MANIFEST) nor a driver tree (version.mk)" % path)


# ---------------------------------------------------------------------------------------
# Render.

LEGEND = """Legend. **route**: `gsp` when the control reaches GSP-RM (ROUTE_TO_PHYSICAL, or sent by
the kernel RM itself), `kernel` when the kernel RM answers it. **privilege**: `non-priv`,
`priv`, `internal` (kernel only) or `kernel` (no privilege flag). **cat**: the heuristic
category of a routed control (A chip data, B host-forwardable, C guest-state derived, D device
model, E never expected from a stock client; docs/caraxes/gsp-surface.md). **device**: how
crates/vfio-user-nvgpu answers today: `host-forwarded`, `synthesized`, `guest-derived`,
`echo` (NV_OK, request returned), `refused` (a named refusal), `unhandled`, `-` (not routed).
Sizes are bytes, measured by the compiler; `-` means no parameters or not measurable. The
device columns are those of the snapshot's repository commit (MANIFEST)."""


def md_escape(s):
    return s.replace("|", "\\|")


def class_order(rows):
    per = collections.defaultdict(list)
    for r in rows:
        per[r["nvoc_class"].split(",")[0]].append(r)
    routed = {k: sum(1 for r in v if r["route"] == "gsp") for k, v in per.items()}
    return sorted(per, key=lambda k: (-routed[k], -len(per[k]), k)), per


def render(d):
    s = load_snapshot(d)
    m = s["manifest"]
    c = summary_counts(s)
    drv = m.get("driver", "?")
    rel = os.path.relpath(os.path.abspath(d), ROOT)
    out = []
    w = out.append
    w("<!-- SPDX-License-Identifier: MIT -->")
    w("<!-- Generated by `python3 caraxes/tools/gsp-abi.py render %s` from the snapshot of the "
      "same directory; do not edit, rerun it (tools/check.sh compares). -->" % rel)
    w("# GSP ABI %s: the tables\n" % drv)
    w("The human-readable form of the snapshot `%s/` (driver %s, version.mk sha256 `%s`, "
      "repository commit `%s`). The baseline document is "
      "[gsp-abi-615.md](gsp-abi-615.md).\n" % (rel, drv, m.get("version_mk_sha256", "?")[:16],
                                               m.get("repository_commit", "?").split()[0][:12]))
    w(LEGEND + "\n")
    w("Totals: %s.\n" % counts_line(c))
    w("## Contents\n")
    order, per = class_order(s["controls.tsv"])
    w("1. Controls, by NVOC class: " + ", ".join(
        "[%s](#%s)" % (k, "controls-" + re.sub(r"[^a-z0-9]+", "-", k.lower()).strip("-"))
        for k in order))
    w("2. [RPC functions](#rpc-functions)")
    w("3. [Events](#events)")
    w("4. [Classes](#classes)")
    w("5. [Boot and transport types](#boot-and-transport-types)\n")
    w("## Controls\n")
    w("| NVOC class | controls | routed | handled | unhandled |")
    w("|---|---:|---:|---:|---:|")
    for k in order:
        rs = per[k]
        rt = [r for r in rs if r["route"] == "gsp"]
        w("| %s | %d | %d | %d | %d |" % (
            k, len(rs), len(rt), sum(1 for r in rt if r["provenance"] not in ("unhandled", "-")),
            sum(1 for r in rt if r["provenance"] == "unhandled")))
    w("")
    for k in order:
        w("### Controls: %s\n" % k)
        w("| id | name | params struct | size | route | privilege | cat | device |")
        w("|---|---|---|---:|---|---|---|---|")
        for r in per[k]:
            w("| `%s` | `%s` | %s | %s | %s | %s | %s | %s |" % (
                r["id"], r["name"], "`%s`" % r["params"] if r["params"] != "-" else "-",
                r["size"], r["route"], r["privilege"], r["category"], r["provenance"]))
        w("")
    w("## RPC functions\n")
    w("| # | function | payload struct | size | struct versions | GSP client | device |")
    w("|---:|---|---|---:|---|---|---|")
    for r in s["rpc.tsv"]:
        w("| %s | `%s` | %s | %s | %s | %s | %s |" % (
            r["num"], r["name"], "`%s`" % r["struct"] if r["struct"] != "-" else "-", r["size"],
            r["versions"], r["gsp_client"], r["device"]))
    w("")
    w("## Events\n")
    w("| # | event | payload struct | size | CPU dispatches | device |")
    w("|---|---|---|---:|---|---|")
    for r in s["events.tsv"]:
        w("| %s | `%s` | %s | %s | %s | %s |" % (
            r["num"], r["name"], "`%s`" % r["struct"] if r["struct"] != "-" else "-", r["size"],
            r["cpu_dispatch"], r["device"]))
    w("")
    w("## Classes\n")
    w("| id | class | internal class | route | reaches GSP | alloc params | size | on TU106 | device |")
    w("|---|---|---|---|---|---|---:|---|---|")
    for r in sorted(s["classes.tsv"], key=lambda r: (r["id"] == "-", r["id"], r["name"])):
        w("| %s | `%s` | %s | %s | %s | %s | %s | %s | %s |" % (
            r["id"], r["name"], r["internal"], r["route"], r["reaches_gsp"],
            "`%s`" % r["alloc_params"] if r["alloc_params"] != "-" else "-", r["size"],
            r["tu106"], r["device"]))
    w("")
    w("## Boot and transport types\n")
    w("| type | kind | size | version | NV_ABI_STABLE | device | defined at |")
    w("|---|---|---:|---|---|---|---|")
    for r in s["boot.tsv"]:
        w("| `%s` | %s | %s | %s | %s | %s | `%s` |" % (
            r["name"], r["kind"], r["size"], md_escape(r["version"]).replace(",", ", "),
            r["stable"], r["device"], r["src"]))
    w("")
    return "\n".join(out) + "\n"


# ---------------------------------------------------------------------------------------

def main():
    os.environ["PYTHONDONTWRITEBYTECODE"] = "1"   # also for ctrl-layouts.py, run as a child
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("snapshot", help="write the snapshot of a driver tree")
    p.add_argument("--ogkm", required=True, help="open-gpu-kernel-modules tree")
    p.add_argument("--out", required=True, help="snapshot directory (caraxes/abi/gsp-<version>/)")
    p.add_argument("--tables", help="also write the markdown tables to this file")
    p = sub.add_parser("diff", help="the delta report between two snapshots or trees")
    p.add_argument("base")
    p.add_argument("new")
    p.add_argument("--strict", action="store_true",
                   help="exit 1 on any driver-side difference (drift check)")
    p.add_argument("--out", help="also write the report to this file (markdown, SPDX header)")
    p = sub.add_parser("render", help="the snapshot as markdown tables")
    p.add_argument("snapshot")
    p.add_argument("--out", help="write to this file (default stdout)")
    p.add_argument("--check", metavar="FILE", help="exit 1 if FILE differs from the rendering")
    a = ap.parse_args()
    if a.cmd == "snapshot":
        snapshot(a.ogkm, a.out, a.tables)
        return 0
    if a.cmd == "render":
        text = render(a.snapshot)
        if a.check:
            if not os.path.exists(a.check) or read(a.check) != text:
                print("%s is not the rendering of %s; rerun `gsp-abi.py render %s --out %s`"
                      % (a.check, a.snapshot, a.snapshot, a.check))
                return 1
            print("%s: up to date with %s" % (a.check, a.snapshot))
            return 0
        if a.out:
            write_text(a.out, text)
        else:
            sys.stdout.write(text)
        return 0
    work = tempfile.mkdtemp(prefix="gsp-abi-diff.")
    try:
        base = load_snapshot(resolve(a.base, work, "base"))
        new = load_snapshot(resolve(a.new, work, "new"))
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = print_report(diff_snapshots(base, new), base, new, a.strict)
        sys.stdout.write(buf.getvalue())
        if a.out:
            write_text(a.out, "<!-- SPDX-License-Identifier: MIT -->\n<!-- Generated by "
                       "`python3 caraxes/tools/gsp-abi.py diff %s %s`; do not edit, rerun it. -->\n"
                       % (a.base, a.new) + buf.getvalue())
        return rc
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
