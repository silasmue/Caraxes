#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""sweep-list.py: the control list of the lab kit's read-only sweep (caraxes/tools/lab-sweep.rs,
docs/caraxes/gsp-plan/lab-kit.md), for one open-gpu-kernel-modules tree.

The sweep issues every getter of categories A, B and E (the E families of
crates/vfio-user-nvgpu/src/ver/v<major>/surface.rs and the D rows moved to E in dmodel.rs) once, on
the matching object of a private RM client on the card: root client (0000), device (0080),
subdevice (2080), display common (0073), ZBC (9096), InfoROM (90E7) and the diagnostic subdevice
(208F, allocated only when the RM allows it). This script decides which controls go in and with
which parameter size; it runs at home, in `lab-kit.sh pack`, never on the lab machine.

Selection, in this order (every rule errs towards leaving a control out):
  1. routed to GSP in the gsp-surface.py inventory of the tree, with category A, B or E, or a
     member of a feature family (docs/caraxes/gsp-plan/e-families.md), or one of the few
     static-info getters of `EXTRA` the boot model of a new chip needs (category "boot");
  2. not RMCTRL_FLAGS_INTERNAL (user space cannot send those; the RM's refusal is the same on
     every card) and not RM_TEST_ONLY_CODE;
  3. a getter by name: GET, QUERY, IS, READ, CHECK or DESCRIBE, or a name ending in _INFO, _CAPS,
     _STATUS or _COUNTERS, and no verb that changes state (SET, CLEAR, UPDATE, MAP, ...); the
     same rule as e-families.md section 1;
  4. an interface class the sweep can allocate (list above);
  5. a parameter struct the compiler can measure against the tree's SDK headers.

Output: a TSV, one row per control:
  id  name  iface  size  category  family  access  tmpl  ptr
tmpl is `plain` (one call with a zeroed buffer), `perindex:N` (the FB/BUS/GPU/GR GET_INFO_V2 list
controls: one call per index 0..N-1 with infoListSize 1, so one invalid index does not fail the
rest) or `perdisplay` (a 0073 control whose struct starts with subDeviceInstance and displayId: one
call per display of SYSTEM_GET_SUPPORTED's displayMask). ptr is 1 when the struct holds an NvP64
(the zeroed pointer goes in with a zero count; the RM checks both).

Usage:
  python3 caraxes/tools/sweep-list.py --ogkm DIR [--out FILE]

Needs cc and python 3.8 or later (standard library only); reads the tree and the crate, writes
FILE (default: stdout). No network, no GPU.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
CRATE_VER = os.path.join(ROOT, "crates", "vfio-user-nvgpu", "src", "ver")

GET_RE = re.compile(r"_(GET|QUERY|IS|READ|CHECK|DESCRIBE)(_|$)|_INFO$|_CAPS(_V2)?$|_STATUS$|_COUNTERS$")
SET_RE = re.compile(
    r"_(SET|CLEAR|RESET|INJECT|ENABLE|DISABLE|WRITE|UPDATE|ALLOC|FREE|MAP|UNMAP|FLUSH|TRIGGER|START|"
    r"STOP|PROGRAM|CONFIGURE|RESERVE|RELEASE|LOCK|SAVE|SEND|INIT|INITIALIZE|CREATE|DELETE|EXPORT|"
    r"IMPORT|MARK|UNMARK|PROMOTE|DERIVE|ROTATE|BIND|RECOVER|POST|CYCLE|SETUP|TOGGLE|SUSPEND|RESUME|"
    r"INVALIDATE|ATTACH|APPLY|OPTIMIZE|ACQUIRE|SYNC|REPLAY|FORCE|RC|DETECT|ACTIVATE|BOOTLOAD|"
    r"SHUTDOWN|ADD|ENUMERATE|SCRUB|CALLBACK|EOM|PRE|TRAIN|PROCESS|HANDLE|REMOVE|PARTITION|"
    r"DEPENDENCY|INBAND|NOTIFY|SPDM|REGISTER|UNREGISTER|EXECUTE|RUN|ARM|CANCEL|LOAD|UNLOAD|"
    r"ALLOCATE|DESTROY|WAIT|KILL|PURGE|RESTORE|EVICT|ISSUE)(_|$)")
# class -> the object the sweep allocates for it (lab-sweep.rs `Objects`)
IFACES = {"0000": "client", "0080": "device", "2080": "subdevice", "0073": "disp",
          "9096": "zbc", "90e7": "inforom", "208f": "diag"}
FLAG_INTERNAL, FLAG_TEST = 0x80, 0x100000
# the list controls answered one index at a time: (control, INDEX_MAX macro, MAX_LIST_SIZE macro)
PERINDEX = {
    "NV2080_CTRL_CMD_FB_GET_INFO_V2": ("NV2080_CTRL_FB_INFO_INDEX_MAX", "NV2080_CTRL_FB_INFO_MAX_LIST_SIZE"),
    "NV2080_CTRL_CMD_BUS_GET_INFO_V2": ("NV2080_CTRL_BUS_INFO_INDEX_MAX", "NV2080_CTRL_BUS_INFO_MAX_LIST_SIZE"),
    "NV2080_CTRL_CMD_GPU_GET_INFO_V2": (None, "NV2080_CTRL_GPU_INFO_MAX_LIST_SIZE"),
    "NV2080_CTRL_CMD_GR_GET_INFO_V2": ("NV2080_CTRL_GR_INFO_INDEX_MAX", "NV2080_CTRL_GR_INFO_MAX_SIZE"),
}
# Getters the kernel answers from GSP-RM's static info (not routed, so not in A, B or E) that the
# boot model of a new chip needs (docs/caraxes/blackwell-gb206.md 2.3, gsp-boot-blackwell.md 6):
# taken in with the category "boot".
EXTRA = {
    "NV2080_CTRL_CMD_MC_GET_ARCH_INFO", "NV2080_CTRL_CMD_GPU_GET_ENGINES_V2",
    "NV2080_CTRL_CMD_GPU_GET_PES_INFO", "NV2080_CTRL_CMD_GR_GET_INFO_V2",
    "NV2080_CTRL_CMD_FB_GET_FB_REGION_INFO", "NV2080_CTRL_CMD_CE_GET_CAPS_V2",
}


def is_getter(name):
    n = name.split("_CTRL_", 1)[1] if "_CTRL_" in name else name
    n = "_" + (n[4:] if n.startswith("CMD_") else n)
    return bool(GET_RE.search(n)) and not SET_RE.search(n)


def families(major):
    """id -> family name, from the generated E and D family tables of the crate."""
    out = {}
    for f, const in (("surface.rs", "E_FAMILIES"), ("dmodel.rs", "D_FAMILIES")):
        p = os.path.join(CRATE_VER, "v%s" % major, f)
        if not os.path.exists(p):
            sys.exit("sweep-list: %s missing (no generated tables for major %s)" % (p, major))
        text = open(p).read()
        start = text.index("pub const %s" % const)
        end = text.index("\n];", start)
        fam = None
        for line in text[start:end].splitlines():
            m = re.match(r'\s+name: "(\w+)",', line)
            if m:
                fam = m.group(1)
            m = re.match(r'\s+\((0x[0-9A-F_]+), "(\w+)"\),', line)
            if m and fam:
                out[int(m.group(1).replace("_", ""), 16)] = fam
    return out


def struct_index(sdk):
    """struct name -> (header relative to sdk, body text), from every ctrl header."""
    idx = {}
    for d, _, files in os.walk(os.path.join(sdk, "ctrl")):
        for fn in files:
            if not fn.endswith(".h"):
                continue
            p = os.path.join(d, fn)
            text = open(p, errors="replace").read()
            for m in re.finditer(r"typedef struct (\w+) \{(.*?)\} (\w+);", text, re.S):
                idx[m.group(3)] = (os.path.relpath(p, sdk), m.group(2))
    return idx


def measure(sdk, headers, names, macros):
    """sizeof of each struct and the value of each macro, compiled against the SDK headers.
    A header that does not compile on its own is dropped with its structs."""
    def build(hs, ns, ms):
        prog = ["#include <stdio.h>", "#include <nvtypes.h>"] + ["#include <%s>" % h for h in hs]
        prog.append("int main(void) {")
        prog += ['    printf("S %s %%zu\\n", sizeof(%s));' % (n, n) for n in ns]
        prog += ['    printf("M %s %%lu\\n", (unsigned long)(%s));' % (m, m) for m in ms]
        prog.append("    return 0;\n}")
        with tempfile.TemporaryDirectory() as t:
            c, exe = os.path.join(t, "m.c"), os.path.join(t, "m")
            open(c, "w").write("\n".join(prog) + "\n")
            r = subprocess.run(["cc", "-std=gnu11", "-DNV_LINUX", "-w", "-I", sdk, "-o", exe, c],
                               capture_output=True, text=True)
            if r.returncode != 0:
                return None
            return subprocess.run([exe], capture_output=True, text=True, check=True).stdout
    good = [h for h in sorted(set(headers.values())) if build([h], [], []) is not None]
    ns = [n for n in names if headers[n] in good]
    ms = [m for m in macros if m]
    out = build(good, ns, ms)
    if out is None:
        sys.exit("sweep-list: the measuring program does not compile against %s" % sdk)
    sizes, vals = {}, {}
    for line in out.splitlines():
        k, n, v = line.split()
        (sizes if k == "S" else vals)[n] = int(v)
    return sizes, vals, sorted(set(headers.values()) - set(good))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--ogkm", required=True, help="open-gpu-kernel-modules tree")
    ap.add_argument("--out", help="output TSV (default stdout)")
    a = ap.parse_args()
    tree = os.path.abspath(a.ogkm)
    ver = re.search(r"NVIDIA_VERSION\s*=\s*(\S+)", open(os.path.join(tree, "version.mk")).read()).group(1)
    major = ver.split(".")[0]
    sdk = os.path.join(tree, "src", "common", "sdk", "nvidia", "inc")
    with tempfile.TemporaryDirectory() as t:
        js = os.path.join(t, "inv.json")
        subprocess.run([sys.executable, os.path.join(ROOT, "caraxes", "tools", "gsp-surface.py"),
                        "--ogkm", tree, "--json", js], check=True, stdout=subprocess.DEVNULL)
        inv = json.load(open(js))
    fams = families(major)
    idx = struct_index(sdk)
    picked, skipped = [], {"category": 0, "internal": 0, "setter": 0, "iface": 0, "struct": 0}
    for c in inv["controls"]:
        cat = c.get("class_category") or ""
        fam = fams.get(c["id"], "")
        if c["name"] in EXTRA:
            c = dict(c, class_category="boot")
        elif not c["routed"] or not (cat in ("A", "B", "E") or fam):
            skipped["category"] += 1
            continue
        if c["flags"] & (FLAG_INTERNAL | FLAG_TEST):
            skipped["internal"] += 1
            continue
        if not is_getter(c["name"]):
            skipped["setter"] += 1
            continue
        if c["interface"] not in IFACES:
            skipped["iface"] += 1
            continue
        if c["params"] not in idx:
            skipped["struct"] += 1
            continue
        picked.append(c)
    headers = {c["params"]: idx[c["params"]][0] for c in picked}
    macros = sorted({m for v in PERINDEX.values() for m in v if m})
    sizes, vals, dropped = measure(sdk, headers, sorted(headers), macros)
    rows = []
    for c in picked:
        n = c["params"]
        if n not in sizes:
            skipped["struct"] += 1
            continue
        body = idx[n][1]
        tmpl = "plain"
        if c["name"] in PERINDEX:
            imax, lmax = PERINDEX[c["name"]]
            cnt = vals[lmax] if imax is None else min(vals[imax] + 1, vals[lmax])
            tmpl = "perindex:%d" % cnt
        elif c["interface"] == "0073" and re.match(r"\s*NvU32\s+subDeviceInstance;\s*NvU32\s+displayId;", body):
            tmpl = "perdisplay"
        ptr = "1" if "NvP64" in body else "0"
        access = "priv" if c["privilege"] == "priv" else "user"
        rows.append((c["id"], c["name"], c["interface"], sizes[n], c.get("class_category") or "-",
                     fams.get(c["id"], "-"), access, tmpl, ptr))
    rows.sort()
    out = open(a.out, "w") if a.out else sys.stdout
    out.write("# SPDX-License-Identifier: MIT\n")
    out.write("# lab-sweep control list for driver %s, made by caraxes/tools/sweep-list.py\n" % ver)
    out.write("# %d controls; left out: %s; headers that do not compile alone: %s\n" % (
        len(rows), ", ".join("%s %d" % kv for kv in skipped.items()), " ".join(dropped) or "none"))
    out.write("# id\tname\tiface\tsize\tcategory\tfamily\taccess\ttmpl\tptr\n")
    for r in rows:
        out.write("%#010x\t%s\t%s\t%d\t%s\t%s\t%s\t%s\t%s\n" % r)
    if a.out:
        out.close()
        print("sweep-list: %d controls for %s -> %s" % (len(rows), ver, a.out), file=sys.stderr)


if __name__ == "__main__":
    main()
