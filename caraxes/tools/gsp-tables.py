#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
"""gsp-tables.py: the per-version control tables of the device, generated from the GSP
surface inventory (caraxes/tools/gsp-surface.py --json) of one open-gpu-kernel-modules tree.

Writes crates/vfio-user-nvgpu/src/ver/v<major>/surface.rs with
  ROUTED       every control that reaches GSP: id, name, the inventory's heuristic category
               (A to E) and whether a crate handler other than the two table rules of
               chipctl.rs answers it;
  E_FAMILIES   the category E controls sorted into feature families, each with the SDK
               header blocks that define them (file:first-last line in the tree);
  E_INVENTORY  the number of category E controls not answered otherwise (the count the
               family table must cover), A_INVENTORY the same for category A.

Which families a chip lacks is chip data (chips/<chip>/chip.toml [features]); the ids are
driver-version data (decision D14), hence one file per driver version.

Usage:
  python3 caraxes/tools/gsp-tables.py --ogkm DIR [--out FILE] [--date YYYY-MM-DD]

Python 3.11 or later, standard library only.
"""

import argparse
import collections
import datetime
import glob
import json
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
SURFACE = os.path.join(ROOT, "caraxes", "tools", "gsp-surface.py")
F_RM_TEST_ONLY_CODE = 0x100000

# (family, what it is, predicate over (name, flags, class, reason)). First match wins; a
# category E control no family takes stops the generator.
FAMILIES = [
    ("rm_test", "RM self-tests, fuzzers and error injection (RMCTRL_FLAGS_RM_TEST_ONLY_CODE, "
                "ECC and ctxsw-ucode injection); not compiled into a production GSP-RM",
     lambda n, f, c, r: bool(f & F_RM_TEST_ONLY_CODE) or re.search(
         r"ECC_INJECT|INJECTION_SUPPORTED|INJECT_CTXSW", n) and "NVLINK" not in n),
    ("nvlink", "NVLink links, counters, NVLE and the INTERNAL_NVLINK set (KernelNvlink)",
     lambda n, f, c, r: "NVLINK" in n),
    ("hshub", "the high-speed hub behind NVLink and C2C (Ampere and later)",
     lambda n, f, c, r: "HSHUB" in n),
    ("fabric", "NVLink fabric: FLA ranges, fabric probe and fabric-manager sync, memory fabric",
     lambda n, f, c, r: re.search(r"_FLA_|FABRIC", n) or c.startswith("MemoryFabric")),
    ("c2c", "chip-to-chip links (Grace Hopper and later)",
     lambda n, f, c, r: "C2C" in n),
    ("gsync", "the G-SYNC (framelock) board",
     lambda n, f, c, r: "GSYNC" in n),
    ("vgpu", "vGPU host and SR-IOV virtual functions (the PF is not in SR-IOV mode)",
     lambda n, f, c, r: re.search(r"VGPU|_VF_|VMMU|NVENC_SW_SESSION|_PF_BAR1", n)),
    ("mig", "MIG and SMC partitioning (GPU and compute instances)",
     lambda n, f, c, r: re.search(r"MIG|EXEC_PARTITIONS|GPU_INSTANCE", n)),
    ("cc", "Confidential Computing (keys, secure channels, security policy)",
     lambda n, f, c, r: re.search(r"CONF_COMPUTE|_KMB$|_CRYPTO_", n)),
    ("spdm", "SPDM attestation (Hopper and later)",
     lambda n, f, c, r: "SPDM" in n),
    ("fsp", "the FSP (firmware security processor, Hopper and later)",
     lambda n, f, c, r: "_FSP_" in n),
    ("ccu", "the clock-counter unit and its profiler streams (Hopper and later)",
     lambda n, f, c, r: "CCU" in n),
    ("internal_uncalled", "INTERNAL controls without a kernel-RM call site in this tree",
     lambda n, f, c, r: r.startswith("internal, no kernel-RM call site")),
]
RULE_GUARDS = ("E family", "A table", "D table")


def load_inventory(ogkm):
    with tempfile.TemporaryDirectory() as d:
        out = os.path.join(d, "inv.json")
        subprocess.run([sys.executable, SURFACE, "--ogkm", ogkm, "--json", out],
                       check=True, stdout=subprocess.DEVNULL)
        with open(out) as f:
            return json.load(f)


def defines(ogkm):
    """Control name -> (header path relative to sdk/nvidia/inc/ctrl, line)."""
    base = os.path.join(ogkm, "src", "common", "sdk", "nvidia", "inc", "ctrl")
    out = {}
    rx = re.compile(r"#define\s+(NV\w+)\s+\(0x[0-9a-fA-F]+U?\)")
    for p in sorted(glob.glob(os.path.join(base, "**", "*.h"), recursive=True)):
        with open(p, encoding="utf-8", errors="replace") as f:
            for i, line in enumerate(f, 1):
                m = rx.match(line)
                if m:
                    out.setdefault(m.group(1), (os.path.relpath(p, base), i))
    return out


def other_handled(c):
    return any(not h.get("guard", "").startswith(RULE_GUARDS) for h in c["handlers"])


def family_of(c):
    n, cls = c["name"].upper(), c["nvoc_class"]
    for name, _, pred in FAMILIES:
        if pred(n, c["flags"], cls, c.get("class_reason", "")):
            return name
    return None


def rust_id(v):
    return "0x%04X_%04X" % (v >> 16, v & 0xFFFF)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--ogkm", required=True, help="open-gpu-kernel-modules tree")
    ap.add_argument("--out", help="output file (default src/ver/v<major>/surface.rs)")
    ap.add_argument("--date", default=datetime.date.today().isoformat())
    a = ap.parse_args()
    ogkm = os.path.abspath(a.ogkm)
    inv = load_inventory(ogkm)
    driver = inv["driver"]
    major = driver.split(".")[0]
    out = a.out or os.path.join(ROOT, "crates", "vfio-user-nvgpu", "src", "ver",
                                "v%s" % major, "surface.rs")
    defs = defines(ogkm)
    routed = [c for c in inv["controls"] if c["routed"]]
    fams = collections.OrderedDict((n, []) for n, _, _ in FAMILIES)
    counts = collections.Counter()
    for c in routed:
        if other_handled(c):
            continue
        counts[c["class_category"]] += 1
        if c["class_category"] != "E":
            continue
        fam = family_of(c)
        if fam is None:
            sys.exit("no family for category E control %s %#x (%s)" % (
                c["name"], c["id"], c.get("class_reason")))
        fams[fam].append(c)

    w = []
    w.append("// SPDX-License-Identifier: MIT")
    w.append("// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>")
    w.append("// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR")
    w.append("//! The GSP control surface of driver %s as the device's tables: generated by" % driver)
    w.append("//! `caraxes/tools/gsp-tables.py --ogkm <tree>` from the inventory of")
    w.append("//! `caraxes/tools/gsp-surface.py --json` (docs/caraxes/gsp-surface.md) on %s." % a.date)
    w.append("//! Do not edit; rerun the generator. Header paths are relative to")
    w.append("//! `src/common/sdk/nvidia/inc/ctrl/` of open-gpu-kernel-modules %s." % driver)
    w.append("")
    w.append("use crate::ver::{Family, Routed};")
    w.append("")
    w.append("/// The driver version the tables were generated from.")
    w.append('pub const SURFACE_DRIVER: &str = "%s";' % driver)
    w.append("/// Category E controls (never expected from a stock client on this chip) that no")
    w.append("/// other handler answers: the number `E_FAMILIES` must cover.")
    w.append("pub const E_INVENTORY: usize = %d;" % counts["E"])
    w.append("/// Category A controls (pure chip data) that no other handler answers.")
    w.append("pub const A_INVENTORY: usize = %d;" % counts["A"])
    w.append("")
    w.append("/// The category E controls by feature family. A chip names the families it lacks in")
    w.append("/// `chips/<chip>/chip.toml` `[features]`; chipctl.rs refuses their controls.")
    w.append("pub const E_FAMILIES: &[Family] = &[")
    for name, what, _ in FAMILIES:
        cs = sorted(fams[name], key=lambda c: c["id"])
        if not cs:
            continue
        blocks = collections.defaultdict(list)
        for c in cs:
            hdr = defs.get(c["name"])
            if hdr:
                blocks[hdr[0]].append(hdr[1])
        w.append("    Family {")
        w.append('        name: "%s",' % name)
        w.append('        what: "%s",' % what)
        w.append("        headers: &[")
        for h in sorted(blocks):
            ls = blocks[h]
            w.append('            "%s:%d-%d",' % (h, min(ls), max(ls)) if min(ls) != max(ls)
                     else '            "%s:%d",' % (h, ls[0]))
        w.append("        ],")
        w.append("        controls: &[")
        for c in cs:
            w.append('            (%s, "%s"),' % (rust_id(c["id"]), c["name"]))
        w.append("        ],")
        w.append("    },")
    w.append("];")
    w.append("")
    w.append("/// Every control that reaches GSP: its inventory category and whether a handler")
    w.append("/// other than the E and A rules of chipctl.rs answers it (`other`).")
    w.append("pub const ROUTED: &[Routed] = &[")
    for c in sorted(routed, key=lambda c: c["id"]):
        w.append("    Routed { id: %s, name: \"%s\", category: '%s', other: %s }," % (
            rust_id(c["id"]), c["name"], c["class_category"],
            "true" if other_handled(c) else "false"))
    w.append("];")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "w") as f:
        f.write("\n".join(w) + "\n")
    print("%s: %d routed, E %d in %d families (%s), A %d" % (
        os.path.relpath(out, ROOT), len(routed), counts["E"],
        sum(1 for v in fams.values() if v),
        ", ".join("%s %d" % (k, len(v)) for k, v in fams.items() if v), counts["A"]))


if __name__ == "__main__":
    main()
