#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
"""gsp-surface.py: the GSP-facing surface of the vendored NVIDIA open kernel driver,
crossed with what the Caraxes device (crates/vfio-user-nvgpu) answers.

Caraxes plays the GSP-RM role towards a stock kernel RM in GSP-client mode. This script
derives, from source only, everything that kernel RM can send to GSP, and marks what the
device handles today and with which provenance (decision D9). The result is the work list
for making the device complete without trace-driven bring-up
(docs/caraxes/gsp-surface.md).

Inputs (all read only):
  an open-gpu-kernel-modules tree: --ogkm DIR, else $CARAXES_OGKM, else the vendored
  vendor/open-gpu-kernel-modules (610.57.04); the driver version is its version.mk
    src/nvidia/inc/kernel/vgpu/rpc_global_enums.h   RPC functions (X) and events (E)
    src/nvidia/generated/g_rpc-structures.h           versioned payload structs
    src/nvidia/generated/g_*_nvoc.c                   exported control tables
                                                      (methodId, flags, class)
    src/nvidia/inc/kernel/rmapi/control.h             RMCTRL_FLAGS_* bit values
    src/nvidia/src/kernel/rmapi/resource_list.h       RS_ENTRY class table
                                                      (RS_FLAGS_ALLOC_RPC_TO_*)
    src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c        events the CPU side dispatches
    src/common/sdk/nvidia/inc/ctrl, .../class         control and class numbers
    call sites in src/nvidia/src, src/nvidia/arch, src/nvidia-modeset, src/common
      (displayport, modeset, unix), kernel-open/nvidia-drm, kernel-open/nvidia-uvm
  crates/vfio-user-nvgpu/src (dispatcher modules, ver/v610*, tests stripped)
  matrix/traces/<version>/control/*.jsonl (optional: native user-level control traces,
    used only as a secondary "seen on the real card" signal)

What counts as "routed to GSP":
  (a) every exported control with RMCTRL_FLAGS_ROUTE_TO_PHYSICAL (0x40): on a GSP client
      rmresControl_Prologue forwards it as GSP_RM_CONTROL (resource.c:254-296);
  (b) every control the kernel RM sends itself, through NV_RM_RPC_CONTROL or through
      GPU_GET_PHYSICAL_RMAPI(pGpu)->Control (heuristic: the physical RM API pointer is
      taken in the 80 lines before the call); these are mostly the INTERNAL (0x80)
      controls, which also carry 0x40;
  (c) every class whose RS_ENTRY carries RS_FLAGS_ALLOC_RPC_TO_PHYS_RM (NVBIT(6)), which
      reaches GSP as GSP_RM_ALLOC, plus the classes that RPC from their constructors.

How the crate side is read (heuristic, see --explain):
  named constants CMD_*/FN_*/EV_* and hex literals from src/ver/v610.rs and
  src/ver/v610/*.rs; `match` arms in ctrl.rs, cuda.rs, nvml.rs, uvm.rs, kgr.rs,
  display/mod.rs and boot.rs (tests cut off at #[cfg(test)]); the tables FORWARD,
  LEGACY, BINAPI (host), ACKNOWLEDGED (echo) and MIRROR_FORWARDED (host mirror, only
  when a mirror exists); and the range rule for class 0x90E7 (nvml.rs). The provenance
  of an arm is read from the D9 label strings in its body (and one level of local helper
  functions): "host ..." is host-forwarded, "guest ..." guest-derived, a NOT_SUPPORTED
  answer without a success path is refused, an NV_OK answer that returns the request
  unchanged or says "acknowledged"/"echo" is echo, anything else ("synthesized",
  "card", "profile", a chip-table label) is synthesized. The display fallback arm
  (`c if is_display_cmd(c)`, label "unanswered") does not count as handled.
  The two table rules of chipctl.rs count as handled too: the category E families of the
  tree's src/ver/v<major>/surface.rs that chips/tu106/chip.toml [features] names absent
  (refused, or synthesized for its `empty_ok` controls), and the entries of
  chips/tu106/controls.toml (synthesized; open rows are not answered). The category D rule
  table of dmodel.rs counts as handled for its REFUSE, DENY and ACK rows that stay in D and
  for the controls it names MODELLED (src/ver/v<major>/dmodel.rs, caraxes/tools/gsp-dtable.py,
  guard "D table"); its families of D rows moved to E count like the E families. Every
  routed control carries its heuristic category as `class_category` in the JSON, handled or
  not (caraxes/tools/gsp-tables.py).

Categories for unhandled controls (name and flag heuristics, see classify()):
  A chip data        answerable from the chip/board tables (static getters, CACHEABLE)
  B host-forwardable public getter of dynamic state, same struct on host and guest
  C guest state      answer derives from objects the guest allocated
  D device model     needs engine behaviour (scheduling, ctx switch, link, clocks, ...)
  E never expected   INTERNAL without a kernel-RM call site, vGPU/plugin/SR-IOV only,
                     test-only, or a class a stock client on this chip never allocates

Usage:
  python3 caraxes/tools/gsp-surface.py                   markdown summary on stdout
  python3 caraxes/tools/gsp-surface.py --full            plus the per-control lists
  python3 caraxes/tools/gsp-surface.py --json out.json   raw inventory as JSON
  python3 caraxes/tools/gsp-surface.py --explain         how each handled id was read
  python3 caraxes/tools/gsp-surface.py --ogkm DIR        against another driver tree
                                                         (or CARAXES_OGKM=DIR)

Python 3.8 or later, standard library only. Runs in a few seconds.
"""

import argparse
import collections
import glob
import json
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
CRATE = os.path.join(ROOT, "crates", "vfio-user-nvgpu", "src")
# The driver tree (VENDOR, NV, DRIVER, TRACES, CALLER_DIRS) is set by select_tree():
# --ogkm DIR, else $CARAXES_OGKM, else the vendored tree.
DEFAULT_VENDOR = os.path.join(ROOT, "vendor", "open-gpu-kernel-modules")

# RMCTRL_FLAGS_* (src/nvidia/inc/kernel/rmapi/control.h:167-344); re-read from the header
# at run time, these are only the names used below.
F_PRIVILEGED = 0x4
F_NON_PRIVILEGED = 0x8
F_ROUTE_TO_PHYSICAL = 0x40
F_INTERNAL = 0x80
F_ROUTE_TO_VGPU_HOST = 0x200
F_CACHEABLE = 0x400
F_CPU_PLUGIN_FOR_SRIOV = 0x4000
F_CPU_PLUGIN_FOR_LEGACY = 0x8000
F_GSP_PLUGIN_FOR_VGPU_GSP = 0x10000
F_CACHEABLE_BY_INPUT = 0x20000
F_PHYSICAL_IMPLEMENTED_ON_VGPU_GUEST = 0x40000
F_RM_TEST_ONLY_CODE = 0x100000

# RS_FLAGS_* (src/nvidia/src/kernel/rmapi/resource_desc_flags.h:28-60).
RS_ALLOC_RPC_TO_VGPU_HOST = 1 << 4
RS_ALLOC_RPC_TO_PHYS_RM = 1 << 6
RS_INTERNAL_ONLY = 1 << 7

# CPU->GSP functions the kernel RM sends in GSP-client mode, and the events its
# dispatcher accepts, from docs/caraxes/gsp-rpc-protocol.md section 3 (a source survey
# with file:line anchors). "sent": always or regularly sent; "maybe": the macro has no
# FW-client guard, reachable on some paths.
GSP_CLIENT_SENDS = {
    "GSP_SET_SYSTEM_INFO": "sent", "SET_REGISTRY": "sent",
    "SET_GUEST_SYSTEM_INFO": "sent", "GET_GSP_STATIC_INFO": "sent",
    "INIT_GSP_TRACE_CRASH_BUFFER": "sent (regkey)", "GSP_RM_ALLOC": "sent",
    "GSP_RM_CONTROL": "sent", "FREE": "sent", "DUP_OBJECT": "sent",
    "CONTINUATION_RECORD": "sent (large RPCs)", "UNLOADING_GUEST_DRIVER": "sent (unload)",
    "UPDATE_BAR_PDE": "sent", "ALLOC_MEMORY": "sent (memRegisterWithGsp)",
    "DUMP_PROTOBUF_COMPONENT": "sent (debug dump)", "ECC_NOTIFIER_WRITE_ACK": "sent (RC path)",
    "MAP_MEMORY_DMA": "maybe", "UNMAP_MEMORY_DMA": "maybe", "IDLE_CHANNELS": "maybe",
    "INVALIDATE_TLB": "maybe", "GPU_EXEC_REG_OPS": "maybe",
}

# The same for a 615 GSP client on TU106 (docs/caraxes/gsp-plan/rpc-functions.md sections 2
# and 4.3): GSP_SET_SYSTEM_INFO, SET_REGISTRY, GET_GSP_STATIC_INFO and SET_GUEST_SYSTEM_INFO are
# replaced by GMCAPI_CMD_GSP_INIT, UNLOADING_GUEST_DRIVER by GMCAPI_CMD_GSP_SUSPEND, and
# INVALIDATE_TLB has no GSP-client path; ALLOC_MEMORY is sent at every boot.
GSP_CLIENT_SENDS_615 = {
    "GSP_RM_CONTROL": "sent", "GSP_RM_ALLOC": "sent", "FREE": "sent",
    "CONTINUATION_RECORD": "sent (large RPCs)", "UPDATE_BAR_PDE": "sent",
    "INIT_GSP_TRACE_CRASH_BUFFER": "sent (regkey)", "ALLOC_MEMORY": "sent (event import)",
    "DUP_OBJECT": "sent", "IDLE_CHANNELS": "maybe", "GPU_EXEC_REG_OPS": "maybe",
    "DUMP_PROTOBUF_COMPONENT": "sent (debug dump)", "ECC_NOTIFIER_WRITE_ACK": "sent (RC path)",
    "MAP_MEMORY_DMA": "maybe", "UNMAP_MEMORY_DMA": "maybe",
}
# Crate modules whose FN_/EV_ constants the boot.rs dispatch uses by path.
CRATE_RPC_MODULES = ["gsprpc.rs", "gspmem.rs", "events.rs"]

# nova-core (drm-rust-next 896ed0833627, gsp/fw.rs MsgFunction and gsp/commands.rs
# `const FUNCTION`): the functions it sends and the events it waits for; the other
# entries of MsgFunction are only decoded. docs/caraxes/nova.md sections 1.6 and 2.1.
NOVA_SENDS = {"GSP_SET_SYSTEM_INFO", "SET_REGISTRY", "GET_GSP_STATIC_INFO",
              "UNLOADING_GUEST_DRIVER", "CONTINUATION_RECORD"}
NOVA_WAITS = {"GSP_INIT_DONE", "GSP_RUN_CPU_SEQUENCER"}
NOVA_KNOWS = {"ALLOC_CHANNEL_DMA", "ALLOC_CTX_DMA", "ALLOC_DEVICE", "ALLOC_MEMORY",
              "ALLOC_OBJECT", "ALLOC_ROOT", "BIND_CTX_DMA", "CONTINUATION_RECORD", "FREE",
              "GET_GSP_STATIC_INFO", "GET_STATIC_INFO", "GSP_INIT_POST_OBJGPU",
              "GSP_RM_CONTROL", "GSP_SET_SYSTEM_INFO", "LOG", "MAP_MEMORY", "NOP",
              "SET_GUEST_SYSTEM_INFO", "SET_REGISTRY", "UNLOADING_GUEST_DRIVER",
              "GSP_INIT_DONE", "GSP_LOCKDOWN_NOTICE", "GSP_POST_NOCAT_RECORD",
              "GSP_RUN_CPU_SEQUENCER", "MMU_FAULT_QUEUED", "OS_ERROR_LOG", "POST_EVENT",
              "RC_TRIGGERED", "UCODE_LIBOS_PRINT"}


def rel(p):
    r = os.path.relpath(p, ROOT)
    # a tree outside the repository (CARAXES_OGKM) is cited by its absolute path
    return os.path.abspath(p) if r.startswith("..") else r


def tree_version(vendor):
    """NVIDIA_VERSION from the tree's version.mk, or None."""
    try:
        t = read(os.path.join(vendor, "version.mk"))
    except OSError:
        return None
    m = re.search(r"^NVIDIA_VERSION\s*=\s*(\S+)", t, re.M)
    return m.group(1) if m else None


def select_tree(vendor):
    """Point every vendor-side path at the tree `vendor`."""
    global VENDOR, NV, DRIVER, TRACES, CALLER_DIRS
    VENDOR = vendor
    NV = os.path.join(VENDOR, "src", "nvidia")
    DRIVER = tree_version(VENDOR) or "unknown"
    TRACES = os.path.join(ROOT, "matrix", "traces", DRIVER, "control")
    CALLER_DIRS = {
        "kernel-rm": [os.path.join(NV, "src"), os.path.join(NV, "arch")],
        "display": [os.path.join(VENDOR, "src", "nvidia-modeset"),
                    os.path.join(VENDOR, "src", "common", "modeset"),
                    os.path.join(VENDOR, "src", "common", "displayport"),
                    os.path.join(VENDOR, "src", "common", "unix"),
                    os.path.join(VENDOR, "kernel-open", "nvidia-drm"),
                    os.path.join(VENDOR, "kernel-open", "nvidia-modeset")],
        "uvm": [os.path.join(VENDOR, "kernel-open", "nvidia-uvm")],
    }


def read(p):
    with open(p, encoding="utf-8", errors="replace") as f:
        return f.read()


def hexint(s):
    return int(s.replace("_", "").rstrip("uU"), 0)


# ---------------------------------------------------------------------------------------
# Vendor side.

def parse_rmctrl_flags():
    """RMCTRL_FLAGS_* name -> bit, from control.h."""
    p = os.path.join(NV, "inc", "kernel", "rmapi", "control.h")
    out = {}
    for m in re.finditer(r"#define RMCTRL_FLAGS_(\w+)\s+(0x[0-9a-fA-F]+)", read(p)):
        v = int(m.group(2), 16)
        if v:
            out[m.group(1)] = v
    return out


def parse_sdk_defines():
    """Control numbers (NVxxxx_CTRL_CMD_*) and class numbers from the SDK headers."""
    sdk = os.path.join(VENDOR, "src", "common", "sdk", "nvidia", "inc")
    ctrl_by_name, ctrl_by_val = {}, collections.defaultdict(list)
    rx = re.compile(r"#define\s+(NV\w*?_CTRL_CMD_\w+)\s+\((0x[0-9a-fA-F]+)U?\)")
    for p in glob.glob(os.path.join(sdk, "ctrl", "**", "*.h"), recursive=True):
        for m in rx.finditer(read(p)):
            name, val = m.group(1), int(m.group(2), 16)
            if name not in ctrl_by_name:
                ctrl_by_name[name] = val
                ctrl_by_val[val].append(name)
    # Older headers name some commands without _CMD_ (NV906F_CTRL_GET_CLASS_ENGINEID).
    rx2 = re.compile(r"#define\s+(NV\w*?_CTRL_\w+)\s+\((0x[0-9a-fA-F]{6,})U?\)")
    for p in glob.glob(os.path.join(sdk, "ctrl", "**", "*.h"), recursive=True):
        for m in rx2.finditer(read(p)):
            name, val = m.group(1), int(m.group(2), 16)
            if val not in ctrl_by_val and val >> 16:
                ctrl_by_name.setdefault(name, val)
                ctrl_by_val[val].append(name)
    classes = {}
    rxc = re.compile(r"#define\s+(\w+)\s+\((0x[0-9a-fA-F]+)U?\)\s*/\* finn: Evaluated from \"\w+_ALLOC")
    rxc2 = re.compile(r"#define\s+(\w+)\s+\(?(0x[0-9a-fA-F]+)U?\)?\s*(?:/[*/].*)?$", re.M)
    for p in glob.glob(os.path.join(sdk, "class", "*.h")):
        t = read(p)
        for m in rxc.finditer(t):
            classes.setdefault(m.group(1), int(m.group(2), 16))
        base = os.path.basename(p)
        for m in rxc2.finditer(t):
            # cl<id>.h defines its class under the same number; keep plain class ids only.
            v = int(m.group(2), 16)
            if v <= 0xFFFF and base.startswith("cl") and base[2:6].lower() == "%04x" % v:
                classes.setdefault(m.group(1), v)
    for m in re.finditer(r"#define\s+(NV01_\w+)\s+\((0x[0-9a-fA-F]+)\)", read(os.path.join(sdk, "nvos.h"))):
        classes.setdefault(m.group(1), int(m.group(2), 16))
    return ctrl_by_name, ctrl_by_val, classes


def best_name(val, by_val, func):
    names = by_val.get(val, [])
    if names:
        # Prefer names without a _V2-less duplicate ambiguity: shortest, then sorted.
        return sorted(names, key=lambda n: (len(n), n))[0]
    return func or ("0x%08x" % val)


def parse_export_tables(by_val):
    """Every exported control of every NVOC class (g_*_nvoc.c)."""
    rx = re.compile(
        r"/\*flags=\*/\s+(0x[0-9a-fA-F]+)u,\s*"
        r"/\*accessRight=\*/(0x[0-9a-fA-F]+)u,\s*"
        r"/\*methodId=\*/\s+(0x[0-9a-fA-F]+)u,\s*"
        r"/\*paramSize=\*/\s+([^\n]+?),\s*"
        r"/\*pClassInfo=\*/\s+&\(__nvoc_class_def_(\w+)\.classInfo\),?"
        r"(?:\s*#if NV_PRINTF_STRINGS_ALLOWED\s*/\*func=\*/\s+\"(\w+)\")?")
    out = []
    for p in sorted(glob.glob(os.path.join(NV, "generated", "g_*_nvoc.c"))):
        t = read(p)
        if "__nvoc_exported_method_def_" not in t:
            continue
        for m in rx.finditer(t):
            line = t.count("\n", 0, m.start(3)) + 1
            flags = int(m.group(1), 16)
            mid = int(m.group(3), 16)
            psize = m.group(4).strip()
            ps = re.match(r"sizeof\((\w+)\)", psize)
            out.append({
                "id": mid, "flags": flags, "access": int(m.group(2), 16),
                "params": ps.group(1) if ps else psize, "nvoc_class": m.group(5),
                "func": m.group(6) or "", "name": best_name(mid, by_val, m.group(6)),
                "src": "%s:%d" % (rel(p), line),
            })
    return out


def balanced_call(text, start):
    """The text of the call whose '(' is at or after `start`, and the top-level args."""
    i = text.index("(", start)
    depth, args, cur = 0, [], []
    for j in range(i, min(len(text), i + 4000)):
        c = text[j]
        if c == "(":
            depth += 1
            if depth == 1:
                continue
        elif c == ")":
            depth -= 1
            if depth == 0:
                args.append("".join(cur).strip())
                return text[i:j + 1], args
        if c == "," and depth == 1:
            args.append("".join(cur).strip())
            cur = []
        else:
            cur.append(c)
    return text[i:], args


SRC_EXT = (".c", ".cpp", ".h")


def iter_sources(dirs):
    for d in dirs:
        for dp, _, fs in os.walk(d):
            for f in fs:
                if f.endswith(SRC_EXT):
                    yield os.path.join(dp, f)


def parse_call_sites(ctrl_by_name):
    """Per control name: reference counts per component, explicit kernel-RM sends."""
    refs = collections.defaultdict(collections.Counter)
    ref_where = collections.defaultdict(list)
    explicit = collections.defaultdict(list)     # name -> ["path:line (how)"]
    dynamic = collections.defaultdict(list)      # function -> ["path:line (how)"]
    rx_name = re.compile(r"\b(NV\w*?_CTRL_CMD_\w+)\b")
    for comp, dirs in CALLER_DIRS.items():
        for p in iter_sources(dirs):
            t = read(p)
            if "_CTRL_CMD_" not in t:
                continue
            lines = t.split("\n")
            vgpu = os.sep + "vgpu" + os.sep in p    # vGPU-guest dispatch, not a caller
            for ln, line in enumerate(lines, 1):
                if "#define" in line or vgpu:
                    continue
                for m in rx_name.finditer(line):
                    n = m.group(1)
                    if n in ctrl_by_name:
                        refs[n][comp] += 1
                        if len(ref_where[n]) < 6:
                            ref_where[n].append("%s:%d" % (rel(p), ln))
            if comp != "kernel-rm" or os.sep + "vgpu" + os.sep in p:
                continue            # src/kernel/vgpu: the vGPU-guest RPC helpers
            for m in re.finditer(r"NV_RM_RPC_CONTROL\s*\(|->Control\s*\(", t):
                if t[max(0, m.start() - 20):m.start()].rstrip().endswith("#define"):
                    continue
                call, args = balanced_call(t, m.start())
                line = t.count("\n", 0, m.start()) + 1
                conds = enclosing_conditions(t, m.start())
                gate = " && ".join(conds)
                if re.search(r"IS_VIRTUAL|[Vv]gpu|VGPU", gate) and not re.search(
                        r"IS_GSP_CLIENT|IS_FW_CLIENT", gate):
                    continue        # a vGPU-guest path, not a GSP client's
                if re.search(r"!\s*IS_(GSP|FW)_CLIENT", gate):
                    continue
                if m.group(0).startswith("NV_RM_RPC_CONTROL"):
                    how, cmdarg = "NV_RM_RPC_CONTROL", args[3] if len(args) > 3 else ""
                else:
                    back = "\n".join(lines[max(0, line - 80):line])
                    cut = back.rfind("\n}")
                    back = back[cut:] if cut >= 0 else back
                    if "GPU_GET_PHYSICAL_RMAPI" not in back:
                        continue
                    how, cmdarg = "physical RM API", args[3] if len(args) > 3 else ""
                names = [n for n in rx_name.findall(cmdarg) if n in ctrl_by_name]
                if not names and re.search(r"(\.|->)cmd\b", cmdarg):
                    # rmCtrlParams.cmd = NV..._CTRL_CMD_...; a few lines above the call.
                    back = "\n".join(lines[max(0, line - 30):line])
                    names = [n for n in re.findall(r"cmd\s*=\s*(NV\w*?_CTRL_CMD_\w+)", back)
                             if n in ctrl_by_name]
                for n in names:
                    explicit[n].append("%s:%d (%s)" % (rel(p), line, how))
                if not names and re.search(r"\bcmd\b|->cmd", cmdarg):
                    # A _KERNEL/_IMPL body that forwards its own command (for example
                    # kchannelCtrlCmdStopChannel_IMPL): keyed by the enclosing function.
                    fn = enclosing_function(lines, line)
                    if fn:
                        dynamic[re.sub(r"_(IMPL|KERNEL)$", "", fn)].append(
                            "%s:%d (%s, own cmd)" % (rel(p), line, how))
    return refs, ref_where, explicit, dynamic


def enclosing_conditions(t, pos):
    """The conditions of the if-blocks around `pos` (walking out through unmatched '{'
    up to the function body), as source text."""
    conds = []
    depth = 0
    i = pos
    while i > 0 and len(conds) < 8:
        i -= 1
        c = t[i]
        if c == "}":
            depth += 1
        elif c == "{":
            if depth:
                depth -= 1
                continue
            head = t[max(0, i - 400):i].rstrip()
            if head.endswith(")"):
                # Find the matching '(' of the header.
                d, j = 0, len(head) - 1
                while j >= 0:
                    if head[j] == ")":
                        d += 1
                    elif head[j] == "(":
                        d -= 1
                        if d == 0:
                            break
                    j -= 1
                kw = re.search(r"(\w+)\s*$", head[:j])
                if kw and kw.group(1) == "if":
                    conds.append(head[j:])
                elif kw and kw.group(1) not in ("while", "for", "switch"):
                    break               # a function header: stop
            elif head.endswith("else"):
                conds.append("else")
    return conds


C_NOT_NAMES = {"if", "for", "while", "switch", "return", "static", "NV_STATUS", "void",
               "NvBool", "NvU32", "NvU64", "NvU8", "NvS32", "else", "do", "case", "sizeof"}


def enclosing_function(lines, line):
    for k in range(line - 1, max(0, line - 1500), -1):
        l = lines[k - 1]
        m = re.match(r"^([A-Za-z_]\w*)\s*(\(.*)?$", l)
        if not m or m.group(1) in C_NOT_NAMES:
            continue
        nxt = lines[k].strip() if k < len(lines) else ""
        if m.group(2) or nxt.startswith("("):
            return m.group(1)
    return None


def parse_rpc_enums():
    p = os.path.join(NV, "inc", "kernel", "vgpu", "rpc_global_enums.h")
    t = read(p)
    funcs, events = [], []
    for ln, line in enumerate(t.split("\n"), 1):
        m = re.match(r"\s*X\(\s*(\w+)\s*,\s*(\w+)\s*,\s*(\d+)\s*\)\s*(//\s*(.*))?", line)
        if m and m.group(2) != "NUM_FUNCTIONS":
            funcs.append({"num": int(m.group(3)), "name": m.group(2), "unit": m.group(1),
                          "note": (m.group(5) or "").strip(), "src": "%s:%d" % (rel(p), ln)})
        m = re.match(r"\s*E\(\s*(\w+)\s*,\s*(0x[0-9a-fA-F]+)\s*\)", line)
        if m and m.group(1) not in ("FIRST_EVENT", "NUM_EVENTS"):
            events.append({"num": int(m.group(2), 16), "name": m.group(1),
                           "src": "%s:%d" % (rel(p), ln)})
    structs = collections.defaultdict(list)
    ps = os.path.join(NV, "generated", "g_rpc-structures.h")
    for m in re.finditer(r"^\} (rpc_(\w+?)_v([0-9A-Fa-f]{2}_[0-9A-Fa-f]{2}));", read(ps), re.M):
        structs[m.group(2).upper()].append(m.group(1))
    for f in funcs + events:
        f["structs"] = structs.get(f["name"], [])
    # Events the CPU side dispatches (_kgspProcessRpcEvent, kernel_gsp.c).
    pk = os.path.join(NV, "src", "kernel", "gpu", "gsp", "kernel_gsp.c")
    tk = read(pk)
    i = tk.find("\n_kgspProcessRpcEvent\n")
    j = tk.find("\n}\n", i)
    handled = set(re.findall(r"case NV_VGPU_MSG_EVENT_(\w+)", tk[i:j]))
    first = tk.count("\n", 0, i) + 2
    for e in events:
        e["cpu_dispatch"] = e["name"] in handled
    return funcs, events, "%s:%d" % (rel(pk), first)


def parse_resource_list(classes):
    p = os.path.join(NV, "src", "kernel", "rmapi", "resource_list.h")
    t = read(p)
    fl = {}
    pf = os.path.join(NV, "src", "kernel", "rmapi", "resource_desc_flags.h")
    for m in re.finditer(r"#define (RS_FLAGS_\w+)\s+NVBIT\((\d+)\)", read(pf)):
        fl[m.group(1)] = 1 << int(m.group(2))
    for m in re.finditer(r"#define (RS_FLAGS_\w+)\s+\(([^)]*)\)", read(pf)):
        v = 0
        for n in re.findall(r"RS_FLAGS_\w+", m.group(2)):
            v |= fl.get(n, 0)
        fl[m.group(1)] = v
    out = []
    for m in re.finditer(r"RS_ENTRY\((.*?)\n\)", t, re.S):
        body = m.group(1)
        ext = re.search(r"External Class\s*\*/\s*(\w+)", body).group(1)
        intern = re.search(r"Internal Class\s*\*/\s*(\w+)", body).group(1)
        fm = re.search(r"Flags\s*\*/\s*(.*?),\s*/\*", body, re.S)
        v = 0
        names = re.findall(r"RS_FLAGS_\w+", fm.group(1)) if fm else []
        for n in names:
            v |= fl.get(n, 0)
        line = t.count("\n", 0, m.start()) + 1
        out.append({"class": ext, "id": classes.get(ext), "internal": intern, "flags": v,
                    "rpc_to_phys": bool(v & RS_ALLOC_RPC_TO_PHYS_RM),
                    "rpc_to_vgpu_host": bool(v & RS_ALLOC_RPC_TO_VGPU_HOST),
                    "internal_only": bool(v & RS_INTERNAL_ONLY),
                    "src": "%s:%d" % (rel(p), line)})
    return out


# Classes that reach GSP from their own constructors on a GSP client
# (gsp-rpc-protocol.md section 5, rpc.h NV_RM_RPC_ALLOC_* with the FW-client branch):
# device.c:186, subdevice.c:124, kernel_channel.c:2761, kernel_channel_group_api.c:429,
# event_api.c:162, vaspace_api.c:263, kern_perfbuffer.c:60, kern_profiler_v2.c:508-787,
# nvencsession.c:69, p2p_api.c:657, nvfbc_session.c:86, virt_mem_range.c:127,
# mem_fabric.c:404 (all under src/nvidia/src/kernel). Some are conditional.
CTOR_RPC_INTERNAL = {"RmClientResource", "Device", "Subdevice", "KernelChannel",
                     "EventApi", "KernelChannelGroupApi", "VaSpaceApi", "PerfBuffer",
                     "ProfilerDev", "ProfilerCtx", "NvencSession", "P2PApi",
                     "NvfbcSession", "VirtualMemoryRange", "MemoryFabric"}


# ---------------------------------------------------------------------------------------
# Native traces (secondary signal).

def parse_traces():
    seen = collections.defaultdict(set)
    for p in sorted(glob.glob(os.path.join(TRACES, "*.jsonl"))):
        probe = os.path.basename(p)[:-6]
        with open(p, encoding="utf-8", errors="replace") as f:
            for line in f:
                if '"nr":"0x2a"' not in line:
                    continue
                try:
                    d = json.loads(line)
                except ValueError:
                    continue
                if d.get("t") == "ioctl" and d.get("sub"):
                    try:
                        seen[int(d["sub"], 16)].add(probe)
                    except ValueError:
                        pass
    return seen


# ---------------------------------------------------------------------------------------
# Crate side.

DISPATCH = [  # module, relative path, order in Dispatcher::control (ctrl.rs)
    ("display", "display/mod.rs"), ("kgr", "kgr.rs"), ("cuda", "cuda.rs"),
    ("uvm", "uvm.rs"), ("public", "public.rs"), ("nvml", "nvml.rs"), ("ctrl", "ctrl.rs"),
]
# The rule kinds of the GSP plan's table (src/ver/v610/public.rs `PUBLIC`, `PUBLIC_C`,
# answered by public.rs): provenance per `Rule::` variant, and per `Dev::` answer where it
# differs (a device NOT_SUPPORTED is a refusal).
PUBLIC_RULES = {
    "Filtered": ("host-forwarded", "host (forwarded, filtered)"),
    "Device": ("synthesized", "synthesized (device-owned answer)"),
    "Refuse": ("refused", "refused (table rule)"),
    "Guest": ("guest-derived", "guest (the guest object's record or its own mirror)"),
}
PUBLIC_REFUSING_DEV = {"NoEcc"}
TABLES = {  # table name -> (provenance, label)
    "FORWARD": ("host-forwarded", "host (forwarded as-is, public control)"),
    "LEGACY": ("host-forwarded", "host (GSS legacy, forwarded opaquely)"),
    "BINAPI": ("host-forwarded", "host (BINAPI object, forwarded opaquely)"),
    "ACKNOWLEDGED": ("echo", "synthesized (NV_OK echo, nothing acted on)"),
    "MIRROR_FORWARDED": ("host-forwarded", "host (mirror object; only with a mirror)"),
}
RANGE_RULES = [  # (class prefix, provenance, label, where)
    (0x90E7, "refused", "synthesized (NOT_SUPPORTED, as the host card answers; no BBX)",
     "nvml.rs GF100_SUBDEVICE_INFOROM range"),
]


def strip_tests(t):
    i = t.find("\n#[cfg(test)]")
    return t if i < 0 else t[:i]


def crate_constants():
    consts = {}
    files = [os.path.join(CRATE, "ver", "v610.rs")] + sorted(
        glob.glob(os.path.join(CRATE, "ver", "v610", "*.rs")))
    for p in files + [os.path.join(CRATE, m) for _, m in DISPATCH] + [
            os.path.join(CRATE, "boot.rs"), os.path.join(CRATE, "rpc.rs")] + [
            os.path.join(CRATE, m) for m in CRATE_RPC_MODULES]:
        for m in re.finditer(r"\bconst (\w+): u32 = (0x[0-9A-Fa-f_]+|\d+);", read(p)):
            consts.setdefault(m.group(1), hexint(m.group(2)))
    return consts, files


def indent(s):
    return len(s) - len(s.lstrip(" "))


def fn_bodies(lines):
    out = {}
    for i, l in enumerate(lines):
        m = re.match(r"\s*(?:pub(?:\([^)]*\))? )?fn (\w+)", l)
        if not m:
            continue
        d = indent(l)
        for k in range(i + 1, len(lines)):
            if lines[k].strip() == "}" and indent(lines[k]) == d:
                out[m.group(1)] = "\n".join(lines[i:k + 1])
                break
    return out


def str_consts(t):
    return {m.group(1): m.group(2)
            for m in re.finditer(r"const (\w+): &str =\s*\"((?:[^\"\\]|\\.)*)\"", t)}


LABEL_PREFIXES = ("host", "synthesized", "guest", "card", "profile", "chip", "board",
                  "unanswered", "captured")
TRANSPARENT = {"ok", "answer", "answer_with", "fail", "forward", "not_supported", "into",
               "to_vec", "push", "format", "get32", "put32", "put64", "get64", "len",
               "min", "max", "truncate", "Some", "Ok", "Err", "vec", "new", "unwrap_or",
               "copied", "iter", "find"}


def classify_arm(body, helpers, sconsts):
    text = body
    for name in set(re.findall(r"\b(?:self\.)?(\w+)\(", body)):
        if name in helpers and name not in TRANSPARENT:
            text += "\n" + helpers[name]
    labels = [s for s in re.findall(r"\"((?:[^\"\\]|\\.)*)\"", text)
              if s.lower().startswith(LABEL_PREFIXES)]
    for c, v in sconsts.items():
        if re.search(r"\b%s\b" % c, text):
            labels.append(v)
    if re.search(r"&[\w.]*label\b", body):
        labels.append("chip/board table label")
    low = [l.lower() for l in labels]
    host = any(l.startswith("host") for l in low) or re.search(
        r"\bforward(_or)?\(|forward_control\(|host\.control\(", text)
    if low and all(l.startswith("unanswered") for l in low):
        return None, labels
    if host:
        return "host-forwarded", labels
    if any(l.startswith("guest") for l in low):
        return "guest-derived", labels
    refused = re.search(r"NV_ERR_NOT_SUPPORTED|not_supported\(", body)
    success = re.search(r"Answer::ok\(|\bok\(|\banswer\(|answer_with\(\s*NV_OK|status: NV_OK",
                        body)
    if refused and not success:
        return "refused", labels
    unchanged = ("params.to_vec()" in body or re.search(r"\bok\(\s*b\s*,", body)) and not \
        re.search(r"put(32|64)\(|b\[[^\]]+\]\s*[|&+]?=|vec!\[0u8|copy_from_slice", body)
    if any("echo" in l or "acknowledged" in l for l in low) or (
            unchanged and any(l.startswith("synthesized") for l in low)):
        return "echo", labels
    return "synthesized", labels


def crate_handlers(consts):
    """id -> list of handler dicts {module, provenance, labels, guard, src}."""
    handlers = collections.defaultdict(list)
    fn_handlers = collections.defaultdict(list)
    rx_id = re.compile(r"(?:\b(?:v|d|rpc|self)::)?\b([A-Z][A-Z0-9_]+)\b|\b(0x[0-9A-Fa-f_]{6,})\b")
    files = [(m, os.path.join(CRATE, p)) for m, p in DISPATCH] + [
        ("boot", os.path.join(CRATE, "boot.rs"))]
    # The FORWARD, LEGACY and BINAPI tables live in src/ver and are consulted by nvml.rs.
    files += [("nvml", p) for p in [os.path.join(CRATE, "ver", "v610.rs")] + sorted(
        glob.glob(os.path.join(CRATE, "ver", "v610", "*.rs")))]
    for module, path in files:
        t = strip_tests(read(path))
        lines = t.split("\n")
        helpers = fn_bodies(lines)
        sconsts = str_consts(t)
        # Tables.
        for m in re.finditer(r"const (\w+): &\[[^=]*?\]\s*=\s*&\[(.*?)\];", t, re.S):
            name = m.group(1)
            if name not in TABLES:
                continue
            prov, label = TABLES[name]
            line = t.count("\n", 0, m.start()) + 1
            body = re.sub(r"//[^\n]*", "", m.group(2))
            body = re.sub(r"\"(?:[^\"\\]|\\.)*\"", "", body)
            for tok in re.findall(r"\b(CMD_\w+|0x[0-9A-Fa-f_]{6,})\b", body):
                val = hexint(tok) if tok.startswith("0x") else consts.get(tok)
                if val is not None:
                    handlers[val].append({"module": module, "provenance": prov,
                                          "labels": [label], "guard": name,
                                          "src": "%s:%d" % (rel(path), line)})
        # The GSP plan's table (src/ver/v610/public.rs): one `row(id, name, size, Rule::X(..))`
        # or `PublicControl { id: .., rule: Rule::X(..) }` per control.
        if path.endswith(os.path.join("ver", "v610", "public.rs")):
            rx_row = re.compile(r"(?:row\(\s*|id:\s*)(0x[0-9A-Fa-f_]+)\b.*?Rule::(\w+)\((?:Dev::(\w+))?",
                                re.S)
            for m in rx_row.finditer(t):
                prov, label = PUBLIC_RULES[m.group(2)]
                if m.group(3) in PUBLIC_REFUSING_DEV:
                    prov = "refused"
                line = t.count("\n", 0, m.start()) + 1
                handlers[hexint(m.group(1))].append({
                    "module": "public", "provenance": prov, "labels": [label],
                    "guard": "public table", "src": "%s:%d" % (rel(path), line)})
        # Match arms.
        if "/ver/" in path:
            continue
        for i, l in enumerate(lines):
            if "=>" not in l or l.strip().startswith("//"):
                continue
            pat = [l.split("=>")[0]]
            j = i - 1
            while pat[0].strip().startswith("|") and j >= 0:
                pat.insert(0, lines[j])
                j -= 1
            first = j + 1
            ptext = " ".join(pat)
            guard = ""
            if " if " in ptext:
                ptext, guard = ptext.split(" if ", 1)
            ids = []
            for a, b in rx_id.findall(ptext):
                if a and (a.startswith("CMD_") or a.startswith("FN_") or a.startswith("EV_")) \
                        and a in consts:
                    ids.append((a, consts[a]))
                elif b:
                    ids.append((b, hexint(b)))
            if not ids:
                continue
            ai = indent(lines[first])
            body = [l.split("=>", 1)[1]]
            for k in range(i + 1, len(lines)):
                s = lines[k].strip()
                if not s:
                    continue
                if indent(lines[k]) < ai or (indent(lines[k]) == ai and
                                             not s.startswith(("}", ")", "]"))):
                    break
                body.append(lines[k])
            btext = "\n".join(body)
            src = "%s:%d" % (rel(path), first + 1)
            if module == "boot":
                for name, val in ids:
                    if name.startswith("FN_") or name.startswith("EV_"):
                        consumed = btext.strip().startswith("None") or (
                            re.search(r"^\s*None\s*$", btext, re.M) and "Some(" not in btext)
                        how = "consumed (no reply)" if consumed else "answered"
                        fn_handlers[name[3:]].append({"how": how, "src": src})
                continue
            prov, labels = classify_arm(btext, helpers, sconsts)
            if prov is None:
                continue
            for name, val in ids:
                if name.startswith("CMD_") or name.startswith("0x"):
                    handlers[val].append({"module": module, "provenance": prov,
                                          "labels": labels[:3], "guard": guard.strip(),
                                          "src": src})
    # Functions the queue code compares against (continuation records).
    for f in ("boot.rs", "msgq.rs"):
        t = strip_tests(read(os.path.join(CRATE, f)))
        for m in re.finditer(r"function == (?:rpc::|crate::ver::(?:active|common)::|v::)?(FN_\w+)", t):
            line = t.count("\n", 0, m.start()) + 1
            fn_handlers[m.group(1)[3:]].append({
                "how": "handled in the queue code",
                "src": "%s:%d" % (rel(os.path.join(CRATE, f)), line)})
    # Events the device sends.
    boot = read(os.path.join(CRATE, "boot.rs"))
    ev_names = {v: k for k, v in consts.items() if k.startswith("EV_")}
    # Events queued as Unsolicited::Event (events.rs, sent by the drain in boot.rs).
    for m in re.finditer(r"Unsolicited::Event\(\s*(?:crate::events::)?(EV_\w+)",
                         strip_tests(boot)):
        val = consts.get(m.group(1))
        if val is not None:
            line = boot.count("\n", 0, m.start()) + 1
            fn_handlers["EVENT:%#x" % val].append({"how": "sent by the device",
                                                   "src": "%s:%d" % (rel(os.path.join(CRATE, "boot.rs")), line)})
    for m in re.finditer(r"RpcHeader::event\(\s*(?:rpc::|v::)?(\w+)", boot):
        val = consts.get(m.group(1))
        if val is not None:
            line = boot.count("\n", 0, m.start()) + 1
            fn_handlers["EVENT:%#x" % val].append({"how": "sent by the device",
                                                   "src": "%s:%d" % (rel(os.path.join(CRATE, "boot.rs")), line)})
    # The events sent when the queues link (ver/mod.rs `link_events`, sent by boot.rs
    # `link_queues`): a 610 guest's (`common::EV_*`); older guests' (v570) are not the
    # snapshot's version.
    verm = read(os.path.join(CRATE, "ver", "mod.rs"))
    body = re.search(r"pub fn link_events\(.*?\n}\n", verm, re.S)
    for m in re.finditer(r"(?:active|common)::(EV_\w+)", body.group(0) if body else ""):
        val = consts.get(m.group(1))
        if val is not None:
            line = verm.count("\n", 0, body.start() + m.start()) + 1
            fn_handlers["EVENT:%#x" % val].append({"how": "sent by the device",
                                                   "src": "%s:%d" % (rel(os.path.join(CRATE, "ver", "mod.rs")), line)})
    return handlers, fn_handlers


CHIP_DIR = os.path.join(ROOT, "crates", "vfio-user-nvgpu", "chips", "tu106")
E_RULE_LABEL = "refused (feature absent on this chip)"


D_RULE_LABELS = {"Refuse": ("refused", "refused (feature absent on this board)"),
                 "Deny": ("refused", "refused (isolation)"),
                 "Ack": ("echo", "synthesized (acknowledged, no effect)")}
E_ACK_LABEL = "synthesized (acknowledged, no effect; feature absent on this chip)"


def dmodel_file():
    """The generated D rule table of the tree's driver version (src/ver/v<major>/dmodel.rs,
    written by caraxes/tools/gsp-dtable.py), or None."""
    p = os.path.join(CRATE, "ver", "v%s" % DRIVER.split(".")[0], "dmodel.rs")
    return p if os.path.exists(p) else None


def surface_file():
    """The generated surface table of the tree's driver version (src/ver/v<major>/surface.rs,
    written by caraxes/tools/gsp-tables.py), or None."""
    p = os.path.join(CRATE, "ver", "v%s" % DRIVER.split(".")[0], "surface.rs")
    return p if os.path.exists(p) else None


def chip_rule_handlers(ctrl_by_name):
    """The two table-driven rules of chipctl.rs: the category E refusal per feature family
    (the families of the version's surface.rs that chips/tu106/chip.toml [features] names
    absent) and the category A chip-data table (chips/tu106/controls.toml). Both run in
    Dispatcher::control after nvml.rs and before the match of ctrl.rs."""
    import tomllib
    out = collections.defaultdict(list)
    rule_src = "crates/vfio-user-nvgpu/src/chipctl.rs"
    chip_p = os.path.join(CHIP_DIR, "chip.toml")
    feats = tomllib.loads(read(chip_p)).get("features", {})
    absent = set(feats.get("absent", []))
    empty_ok = {e["name"]: e["label"] for e in feats.get("empty_ok", [])}
    dp = dmodel_file()
    e_ack = set()
    d_names = {}
    if dp:
        dtext = read(dp)
        ack_block = dtext.split("pub const D_E_ACK: &[u32] = &[", 1)[-1].split("];", 1)[0]
        e_ack = {hexint(v) for v in re.findall(r"\b(0x[0-9A-Fa-f_]+)\b", ack_block)}
        dsrc = read(os.path.join(CRATE, "dmodel.rs"))
        modelled = re.findall(r'"(\w+)"', dsrc.split(
            "pub const MODELLED: &[&str] = &[", 1)[-1].split("];", 1)[0])
        # The models of round 2 (dmodel.rs, rc.rs, zbc.rs) answer their rows whatever the
        # category the plan moves them to (MODELLED_HERE), and the getters that read their
        # state (MODEL_GETTERS, categories A and B).
        def const_list(const):
            head = "pub const %s: &[&str] = &[" % const
            return re.findall(r'"(\w+)"', dsrc.split(head, 1)[-1].split("];", 1)[0]) \
                if head in dsrc else []
        here = const_list("MODELLED_HERE")
        for g in const_list("MODEL_GETTERS"):
            cid = ctrl_by_name.get(g)
            if cid is not None:
                out[cid].append({"module": "ctrl", "provenance": "guest-derived",
                                     "labels": ["guest (device model, dmodel.rs)"],
                                     "guard": "D table",
                                     "src": "crates/vfio-user-nvgpu/src/dmodel.rs"})
        rx_rule = re.compile(r'DRule \{\s*id: (0x[0-9A-Fa-f_]+),\s*name: "(\w+)",.*?'
                             r'answer: DAnswer::(\w+),.*?reclass: \'(.)\'', re.S)
        for m in rx_rule.finditer(dtext):
            # Controls the inventory names by handler (no #define, e.g. 0x20800A3E).
            d_names[m.group(2)] = hexint(m.group(1))
            kind, reclass = m.group(3), m.group(4)
            if m.group(2) in here:
                out[hexint(m.group(1))].append({"module": "ctrl", "provenance": "guest-derived",
                                                "labels": ["guest (%s model, dmodel.rs)" % kind.lower()],
                                                "guard": "D table",
                                                "src": "crates/vfio-user-nvgpu/src/dmodel.rs"})
                continue
            if reclass != "-":
                continue
            if kind in D_RULE_LABELS:
                prov, label = D_RULE_LABELS[kind]
            elif m.group(2) in modelled:
                prov, label = "synthesized", "synthesized (%s model, display/mod.rs)" % kind.lower()
            else:
                continue
            out[hexint(m.group(1))].append({"module": "ctrl", "provenance": prov,
                                            "labels": [label], "guard": "D table",
                                            "src": "crates/vfio-user-nvgpu/src/dmodel.rs"})
    for sp in [f for f in (surface_file(), dp) if f]:
        rx = re.compile(r'Family \{\s*name: "(\w+)",.*?controls: &\[(.*?)\],\s*\}', re.S)
        for m in rx.finditer(read(sp)):
            if m.group(1) not in absent:
                continue
            for val, name in re.findall(r'\((0x[0-9A-Fa-f_]+), "(\w+)"\)', m.group(2)):
                if hexint(val) in e_ack:
                    h = {"module": "ctrl", "provenance": "echo", "labels": [E_ACK_LABEL],
                         "guard": "E family %s (acknowledged)" % m.group(1), "src": rule_src}
                elif name in empty_ok:
                    h = {"module": "ctrl", "provenance": "synthesized",
                         "labels": [empty_ok[name]], "src": rel(chip_p),
                         "guard": "E family %s (NV_OK, empty)" % m.group(1)}
                else:
                    h = {"module": "ctrl", "provenance": "refused", "labels": [E_RULE_LABEL],
                         "guard": "E family %s" % m.group(1), "src": rule_src}
                out[hexint(val)].append(h)
    ctl_p = os.path.join(CHIP_DIR, "controls.toml")
    if os.path.exists(ctl_p):
        for e in tomllib.loads(read(ctl_p)).get("control", []):
            if "open" in e:
                continue
            val = ctrl_by_name.get(e["name"], d_names.get(e["name"]))
            if val is None or DRIVER.split(".")[0] not in e.get("abi", ["610", "615"]):
                continue
            labels = [c["label"] for c in e.get("case", []) if "label" in c] or [e["label"]]
            refused = e.get("status", "OK") != "OK"
            out[val].append({"module": "ctrl", "provenance": "refused" if refused else
                             "synthesized", "labels": labels[:3], "guard": "A table",
                             "src": rel(ctl_p)})
    return out


def primary(hs):
    """The handler that answers first (Dispatcher::control order); the mirror table is
    conditional on a mirror and yields to the unconditional handler."""
    order = {m: i for i, (m, _) in enumerate(DISPATCH)}
    uncond = [h for h in hs if h["guard"] != "MIRROR_FORWARDED"] or hs
    return sorted(uncond, key=lambda h: order.get(h["module"], 99))[0]


def crate_classes(consts):
    """Classes the crate names (ver constants <= 0xFFFF that are not offsets) and the
    chip's channel classes."""
    named = set()
    t = read(os.path.join(CRATE, "ver", "v610.rs"))
    for m in re.finditer(r"pub const ((?:NV|GF|GT|GK|GM|GP|GV|TU|FERMI|KEPLER|MAXWELL|PASCAL|VOLTA|TURING|AMPERE|UVM|ACCESS)\w*): u32 = (0x[0-9A-Fa-f_]+);", t):
        named.add(hexint(m.group(2)))
    for p in glob.glob(os.path.join(CRATE, "ver", "v610", "*.rs")):
        for m in re.finditer(r"pub const (NV\w+): u32 = (0x[0-9A-Fa-f_]{4,6});", read(p)):
            named.add(hexint(m.group(2)))
    for p in glob.glob(os.path.join(ROOT, "crates", "vfio-user-nvgpu", "chips", "*", "chip.toml")):
        sec = read(p).split("[channel]", 1)
        if len(sec) == 2:
            for m in re.finditer(r"=\s*(0x[0-9A-Fa-f]{4})\b", sec[1].split("\n[", 1)[0]):
                named.add(int(m.group(1), 16))
    return named


# ---------------------------------------------------------------------------------------
# Classification of unhandled controls.

GUEST_OBJECT_CLASSES = {
    "KernelChannel", "KernelChannelGroupApi", "VaSpaceApi", "Memory", "MemoryFabric",
    "ContextDma", "Event", "EventBuffer", "KernelGraphicsContext", "KernelCtxShareApi",
    "DispChannelPio", "DispChannelDma", "DispSfUser", "UvmChannelRetainer",
    "ChannelDescendant", "KernelGraphicsObject", "KernelCeContext",
    "MemoryMapper", "SyncpointMemory", "VideoMemory", "SystemMemory", "OsDescMemory",
    "RegisterMemory", "ConsoleMemory", "NoDeviceMemory", "PhysicalMemory", "VirtualMemory",
    "MemoryList", "FlaMemory", "HostVgpuDeviceApi", "UserModeApi", "MmuFaultBuffer",
    "AccessCounterBuffer", "SemaphoreSurface", "GpuUserSharedData", "UvmSwObject",
}
NEVER_CLASSES = {
    "VgpuConfigApi", "KernelHostVgpuDeviceApi", "VgpuApi", "KernelVgpuMgr",
    "KernelSMDebuggerSessionInternal", "LockStressObject", "LockTestRelaxedDupObject",
    "ConfidentialComputeApi", "KernelCcuApi", "GpuManagementApi", "Spdm",
    "NvencSession", "NvfbcSession", "ComputeInstanceSubscription", "GPUInstanceSubscription",
    "MIGConfigSession", "MIGMonitorSession", "MpsApi", "MemoryMulticastFabric",
    "ImexSessionApi", "KernelNvlinkApi", "NvlinkApi", "GpuMgmtApi", "Hdacodec",
    "P2PApi", "ThirdPartyP2P", "MemoryFabric", "MemoryFabricImportV2",
}
RX_NEVER = re.compile(r"VGPU|SRIOV|_VF_|PLUGIN|_GFID|HOST_VGPU|NVLINK|NVSWITCH|_C2C_|"
                      r"CONF_COMPUTE|_CC_|SPDM|FSP|_MIG_|EXEC_PARTITION|GPU_INSTANCE|"
                      r"COMPUTE_INSTANCE|_SMC_|FABRIC|IMEX|CCU|_NVENC_SW_SESSION|_NVFBC_|_FLA_|"
                      r"_TEST(_|$)|VMMU|_KMB$|_LOCK_STRESS|_DEBUG_ONLY|_RUSD_|HSHUB|GSYNC|_NVD_|"
                      r"_EMULATION|_SIM_|_MODS_|_ECC_INJECT|INJECT_|_ROOT_PORT|_MNOC")
RX_MODEL = re.compile(r"EXEC|REGOPS|REG_OPS|CTXSW|PREEMPT|SCHEDULE|INVALIDATE|FLUSH|RESET|"
                      r"MODESET|LINK_TRAIN|_LT$|AUXCH|I2C_(ACCESS|TRANSACTION|INDEXED)|"
                      r"SUSPEND|RESUME|SINGLE_STEP|_WRITE|READ_(MEMORY|VIRTUAL)|_START|_STOP|"
                      r"TRIGGER|_PMA_|HWPM|_PM_|PERFMON|BIND|PROMOTE|_EVICT|_IDLE|_SET_|"
                      r"_CTRL$|_ENABLE|_DISABLE|_ALLOC|_FREE|_MAP|_UNMAP|SEMAPHORE|_ARM|"
                      r"_WAIT|_KICK|BOOST|CLK_|_VOLT|_POWER|_FAN|_TIMER_SCHEDULE|"
                      r"_PREPARE|_UPDATE|_SERVICE|_POST_|_PRE_|_CHANNEL_PUSHBUFFER|_RESERVE|"
                      r"_RELEASE|_REGISTER|_UNREGISTER|_LOAD|_UNLOAD|_CONFIG_STREAM|_CLEAR")
RX_STATIC = re.compile(r"_STATIC|_GET_(INFO|CAPS|NUM|COUNT|MASK|LIST|CONFIG|SIZE|NAME|"
                       r"VERSION|PROPERTIES|TABLE|LIMITS|SUPPORTED|ID|IDS|TYPE|BASE|"
                       r"ENGINES|CLASSLIST|CLASS|ATTRIBUTES|ARCH|SKU|FEATURES|DESC|MAP|"
                       r"SHAPE|LAYOUT|FLOORSWEEP|FS_|GPC|TPC|ROP|ZCULL|PPC|SM_)|"
                       r"_GET_\w*(INFO|CAPS|MASK|NUM_\w+|LIST|CONFIG|SIZE|VERSION|"
                       r"PROPERTIES|TABLE|LIMITS|SUPPORTED|MAP|VECTORS|ARCH)(_V\d+)?$")
DISPLAY_CLASSES = {"DispCommon", "NvDispApi", "DispChannelPio", "DispChannelDma",
                   "DispSwObj", "DispSwObject", "DispSfUser", "DispCapabilities"}
RX_DYNAMIC = re.compile(r"STATUS|COUNTER|ACCOUNTING|PIDS|UTIL|SAMPLE|TEMP|READING|CURRENT|_STATE|EVENTS?$|"
                        r"ERROR|_ECC|FAULT|USAGE|STATS|PSTATE|_CLK|_PERF|THERMAL|_POWER|"
                        r"ACTIVE|BW_|BANDWIDTH|HISTORY|TIMESTAMP|_TIME|RATE$|_LOAD")


def classify(c, has_callsite):
    """One of A B C D E with a short reason."""
    n = c["name"].upper()
    f = c["flags"]
    cls = c["nvoc_class"].replace(" (not exported)", "")
    if f & F_RM_TEST_ONLY_CODE:
        return "E", "test-only flag"
    if cls in NEVER_CLASSES or RX_NEVER.search(n):
        return "E", "vGPU/MIG/NVLink/CC/test scope"
    if (f & F_INTERNAL) and not has_callsite:
        return "E", "internal, no kernel-RM call site"
    if cls in GUEST_OBJECT_CLASSES and not re.search(r"EXEC|REGOPS|CTXSW|SUSPEND|RESUME|"
                                                     r"SINGLE_STEP|DEBUG", n):
        return "C", "control on a guest object"
    if re.search(r"_GET_|_QUERY_|_IS_", n) and not re.search(r"_SET_|_EXEC", n):
        if (f & (F_CACHEABLE | F_CACHEABLE_BY_INPUT)) or (RX_STATIC.search(n) and not
                                                          RX_DYNAMIC.search(n)):
            return "A", "static getter" + (" (CACHEABLE)" if f & F_CACHEABLE else "")
        if cls in DISPLAY_CLASSES:
            # The host's display is not the guest's: live display state comes from the
            # device's own connector and sink model, never from the host.
            return "D", "display live state (connector/sink model)"
        if not (f & F_INTERNAL):
            return "B", "public getter of live state"
        return "A" if not RX_DYNAMIC.search(n) else "D", "internal getter"
    if RX_MODEL.search(n):
        return "D", "acts on engines or hardware state"
    if f & (F_CACHEABLE | F_CACHEABLE_BY_INPUT):
        return "A", "CACHEABLE"
    return "D", "no getter shape"


CAT_NAMES = {
    "A": "pure chip data",
    "B": "host-forwardable as-is",
    "C": "guest-state derived",
    "D": "needs device modelling",
    "E": "never expected from a stock client",
}


def priv(f):
    if f & F_INTERNAL:
        return "internal"
    if f & F_NON_PRIVILEGED:
        return "non-priv"
    if f & F_PRIVILEGED:
        return "priv"
    return "kernel"


# ---------------------------------------------------------------------------------------

IFACE_OWNER = {0x0000: "RmClientResource", 0x0073: "DispCommon", 0x0080: "Device",
               0x2080: "Subdevice", 0xA080: "VgpuConfigApi"}


def build():
    flags_by_name = parse_rmctrl_flags()
    ctrl_by_name, ctrl_by_val, classes = parse_sdk_defines()
    exports = parse_export_tables(ctrl_by_val)
    refs, ref_where, explicit, dynamic = parse_call_sites(ctrl_by_name)
    funcs, events, dispatch_src = parse_rpc_enums()
    rlist = parse_resource_list(classes)
    traces = parse_traces()
    consts, _ = crate_constants()
    handlers, fn_handlers = crate_handlers(consts)
    crate_cls = crate_classes(consts)
    rule_handlers = chip_rule_handlers(ctrl_by_name)

    # Controls: one row per (methodId), merged over classes that export the same id.
    by_id = {}
    for e in exports:
        r = by_id.setdefault(e["id"], dict(e, classes=[], routed=False, why=[]))
        r["classes"].append(e["nvoc_class"])
        if e["flags"] & F_ROUTE_TO_PHYSICAL:
            r["routed"] = True
            r["flags"] |= e["flags"]
    for r in by_id.values():
        if r["routed"]:
            r["why"].append("ROUTE_TO_PHYSICAL")
    # Controls whose kernel implementation forwards its own command to GSP.
    for e in exports:
        sites = dynamic.get(e["func"])
        if sites:
            r = by_id[e["id"]]
            r.setdefault("explicit", [])
            r["explicit"] = r["explicit"] + sites
            if not r["routed"]:
                r["routed"] = True
                r["why"].append("kernel impl forwards its own cmd")
    # Explicit sends of controls without the flag, or not exported at all.
    for name, sites in explicit.items():
        val = ctrl_by_name[name]
        r = by_id.get(val)
        if r is None:
            owner = IFACE_OWNER.get(val >> 16, "%04x" % (val >> 16)) + " (not exported)"
            r = by_id[val] = {"id": val, "flags": 0, "access": 0, "params": "",
                              "nvoc_class": owner, "func": "", "name": name,
                              "src": sites[0], "classes": [owner],
                              "routed": False, "why": []}
        r["explicit"] = sites
        if not r["routed"]:
            r["routed"] = True
            r["why"].append("explicit kernel-RM send")
    rows = []
    for val, r in sorted(by_id.items()):
        names = ctrl_by_val.get(val, [r["name"]])
        cs = collections.Counter()
        where = []
        for n in names:
            cs.update(refs.get(n, {}))
            where += ref_where.get(n, [])
        hs = list(handlers.get(val, [])) + list(rule_handlers.get(val, []))
        for pre, prov, label, src in RANGE_RULES:
            if val >> 16 == pre:
                hs.append({"module": "nvml", "provenance": prov, "labels": [label],
                           "guard": "range", "src": src})
        r["callsites"] = dict(cs)
        r["callsite_examples"] = where[:6]
        r["explicit"] = r.get("explicit") or explicit.get(r["name"], [])
        r["trace_probes"] = sorted(traces.get(val, []))
        r["handlers"] = hs
        r["handled"] = bool(hs)
        r["provenance"] = primary(hs)["provenance"] if hs else ""
        r["privilege"] = priv(r["flags"])
        r["interface"] = "%04x" % (val >> 16)
        kernel_site = bool(r["explicit"]) or cs.get("kernel-rm", 0) > 0 or \
            cs.get("display", 0) > 0 or cs.get("uvm", 0) > 0
        if r["routed"]:
            # The heuristic category of every routed control, handled or not: the table
            # generator (gsp-tables.py) selects the E families and the A candidates by it.
            r["class_category"], r["class_reason"] = classify(r, kernel_site)
        if r["routed"] and not r["handled"]:
            r["category"], r["reason"] = r["class_category"], r["class_reason"]
        rows.append(r)

    # Handled ids that the vendor tables do not export (legacy GSS numbers, ...).
    extra = sorted(v for v in handlers if v not in by_id and v >= 0x10000)

    for f in funcs:
        major = DRIVER.split(".")[0]
        sends = GSP_CLIENT_SENDS_615 if major.isdigit() and int(major) >= 615 else GSP_CLIENT_SENDS
        f["gsp_client"] = sends.get(f["name"], "")
        f["nova"] = "sends" if f["name"] in NOVA_SENDS else (
            "decodes" if f["name"] in NOVA_KNOWS else "")
        hs = fn_handlers.get(f["name"], [])
        f["device"] = hs[0]["how"] if hs else ""
        f["device_src"] = hs[0]["src"] if hs else ""
    for e in events:
        hs = fn_handlers.get("EVENT:%#x" % e["num"], [])
        e["nova"] = "waits" if e["name"] in NOVA_WAITS else (
            "decodes" if e["name"] in NOVA_KNOWS else "")
        e["device"] = hs[0]["how"] if hs else ""
        e["device_src"] = hs[0]["src"] if hs else ""

    for c in rlist:
        c["reaches_gsp"] = c["rpc_to_phys"] or c["internal"] in CTOR_RPC_INTERNAL
        c["crate"] = "named" if c["id"] is not None and c["id"] in crate_cls else (
            "acknowledged (generic alloc echo)" if c["reaches_gsp"] else "")

    return {
        "driver": DRIVER,
        "sources": {"vendor": rel(VENDOR), "crate": rel(CRATE),
                    "event_dispatch": dispatch_src},
        "rmctrl_flags": flags_by_name,
        "controls": rows,
        "handled_not_exported": ["0x%08x" % v for v in extra],
        "handled_extra_detail": {("0x%08x" % v): handlers[v] for v in extra},
        "rpc_functions": funcs,
        "rpc_events": events,
        "classes": rlist,
    }


# ---------------------------------------------------------------------------------------
# Markdown.

def md(inv, full=False, explain=False):
    out = []
    w = out.append
    rows = inv["controls"]
    routed = [r for r in rows if r["routed"]]
    handled = [r for r in routed if r["handled"]]
    unhandled = [r for r in routed if not r["handled"]]
    prov = collections.Counter(r["provenance"] for r in handled)
    cats = collections.Counter(r["category"] for r in unhandled)
    funcs, events = inv["rpc_functions"], inv["rpc_events"]
    cls = inv["classes"]
    gsp_cls = [c for c in cls if c["reaches_gsp"]]

    w("# GSP surface of driver %s against the Caraxes device\n" % inv["driver"])
    w("Generated by `caraxes/tools/gsp-surface.py` from `%s` and `%s`.\n" %
      (inv["sources"]["vendor"], inv["sources"]["crate"]))
    w("## Totals\n")
    w("| Item | Count |")
    w("|---|---|")
    w("| Exported controls (all NVOC classes, unique ids) | %d |" % len(rows))
    w("| Controls that reach GSP (ROUTE_TO_PHYSICAL or explicit kernel-RM send) | %d |" % len(routed))
    w("| of which INTERNAL (0x80) | %d |" % sum(1 for r in routed if r["flags"] & F_INTERNAL))
    w("| of which sent explicitly by kernel RM | %d |" % sum(1 for r in routed if r["explicit"]))
    w("| Routed controls the device handles | %d |" % len(handled))
    w("| Routed controls unhandled | %d |" % len(unhandled))
    w("| Handled ids outside the export tables (GSS legacy and others) | %d |" %
      len(inv["handled_not_exported"]))
    w("| RPC functions (rpc_global_enums.h) | %d |" % len(funcs))
    w("| of which sent by a GSP-client kernel RM (always or maybe) | %d |" %
      sum(1 for f in funcs if f["gsp_client"]))
    w("| of which the device handles | %d |" % sum(1 for f in funcs if f["device"]))
    w("| RPC events | %d |" % len(events))
    w("| of which the CPU side dispatches | %d |" % sum(1 for e in events if e["cpu_dispatch"]))
    w("| of which the device sends | %d |" % sum(1 for e in events if e["device"]))
    w("| Classes in resource_list.h | %d |" % len(cls))
    w("| of which reach GSP on alloc | %d |" % len(gsp_cls))
    w("| of which the crate names | %d |" % sum(1 for c in gsp_cls if c["crate"] == "named"))
    w("")
    w("## Handled routed controls by provenance\n")
    w("| Provenance | Controls |")
    w("|---|---|")
    for p in ("host-forwarded", "synthesized", "guest-derived", "echo", "refused"):
        w("| %s | %d |" % (p, prov.get(p, 0)))
    xp = collections.Counter(h[0]["provenance"] for h in inv["handled_extra_detail"].values())
    w("")
    w("Outside the export tables the device also handles %d ids (%s): the GSS-legacy "
      "THERMAL/CLK/PERF/PMGR numbers and the NV2081 BINAPI commands, which have no "
      "NVOC export entry." % (len(inv["handled_not_exported"]),
                              ", ".join("%s %d" % kv for kv in sorted(xp.items()))))
    w("")
    w("## Unhandled routed controls by category\n")
    w("| Category | Controls | with a kernel/modeset/UVM call site | seen in a native trace |")
    w("|---|---|---|---|")
    for k in "ABCDE":
        sel = [r for r in unhandled if r["category"] == k]
        w("| %s %s | %d | %d | %d |" % (k, CAT_NAMES[k], len(sel),
                                        sum(1 for r in sel if r["callsites"] or r["explicit"]),
                                        sum(1 for r in sel if r["trace_probes"])))
    w("")
    w("## Per resource class (NVOC class of the exporting object)\n")
    w("| NVOC class | Routed | INTERNAL | Handled | host | synth | guest | echo | refused | Unhandled | A | B | C | D | E |")
    w("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    per = collections.defaultdict(list)
    for r in routed:
        per[r["nvoc_class"]].append(r)
    for k in sorted(per, key=lambda k: (-len(per[k]), k)):
        rs = per[k]
        h = [r for r in rs if r["handled"]]
        u = [r for r in rs if not r["handled"]]
        pc = collections.Counter(r["provenance"] for r in h)
        cc = collections.Counter(r["category"] for r in u)
        w("| %s | %d | %d | %d | %d | %d | %d | %d | %d | %d | %d | %d | %d | %d | %d |" % (
            k, len(rs), sum(1 for r in rs if r["flags"] & F_INTERNAL), len(h),
            pc["host-forwarded"], pc["synthesized"], pc["guest-derived"], pc["echo"],
            pc["refused"], len(u), cc["A"], cc["B"], cc["C"], cc["D"], cc["E"]))
    w("")
    w("## RPC functions\n")
    w("Only functions a GSP-client kernel RM sends, nova-core uses, or the device handles; "
      "the other %d are vGPU-guest (IS_VIRTUAL) or deprecated numbers.\n" %
      sum(1 for f in funcs if not (f["gsp_client"] or f["nova"] or f["device"])))
    w("| # | Function | Struct versions | GSP client | nova-core | Device |")
    w("|---|---|---|---|---|---|")
    for f in funcs:
        if not (f["gsp_client"] or f["nova"] or f["device"]):
            continue
        w("| %d | %s | %s | %s | %s | %s |" % (
            f["num"], f["name"], ", ".join(s.split("_v")[-1] and "v" + s.split("_v")[-1]
                                           for s in f["structs"]) or "-",
            f["gsp_client"] or "-", f["nova"] or "-",
            ("%s (`%s`)" % (f["device"], f["device_src"])) if f["device"] else (
                "**unhandled**" if (f["gsp_client"] or f["nova"] == "sends") else
                "not sent to GSP by either client")))
    w("")
    w("## RPC events (GSP to CPU)\n")
    w("| Event | Name | CPU dispatches | nova-core | Device sends |")
    w("|---|---|---|---|---|")
    for e in events:
        w("| %#x | %s | %s | %s | %s |" % (e["num"], e["name"],
                                         "yes" if e["cpu_dispatch"] else "no",
                                         e["nova"] or "-", "yes" if e["device"] else "no"))
    w("")
    w("## Classes that reach GSP on allocation\n")
    w("One row per internal class. The device acknowledges every GSP_RM_ALLOC (ctrl.rs "
      "`Dispatcher::alloc`, the parameters echoed); \"named\" means the crate models or "
      "records the class by number.\n")
    w("| Internal class | Route | Classes named by the crate | Other classes (acknowledged only) |")
    w("|---|---|---|---|")
    grp = collections.OrderedDict()
    for c in gsp_cls:
        grp.setdefault(c["internal"], []).append(c)
    for k, cs in grp.items():
        c0 = cs[0]
        route = "RPC_TO_ALL" if (c0["rpc_to_phys"] and c0["rpc_to_vgpu_host"]) else (
            "RPC_TO_PHYS_RM" if c0["rpc_to_phys"] else "constructor RPC")
        fmt = lambda c: "%s %s" % (c["class"], "%#06x" % c["id"] if c["id"] is not None else "")
        named = [fmt(c) for c in cs if c["crate"] == "named"]
        other = [fmt(c) for c in cs if c["crate"] != "named"]
        w("| %s | %s | %s | %s |" % (k, route, ", ".join(named) or "-",
                                     ", ".join(other) or "-"))
    w("")
    if full:
        w("## Handled routed controls\n")
        w("| Id | Name | Class | Priv | Provenance | Where |")
        w("|---|---|---|---|---|---|")
        for r in handled:
            p = primary(r["handlers"])
            w("| %#010x | %s | %s | %s | %s | `%s` |" % (r["id"], r["name"], r["nvoc_class"],
                                                        r["privilege"], r["provenance"], p["src"]))
        w("")
        for k in "ABCDE":
            w("## Unhandled, category %s: %s\n" % (k, CAT_NAMES[k]))
            w("| Id | Name | Class | Priv | Flags | Call sites (krm/disp/uvm) | Traces | Reason |")
            w("|---|---|---|---|---|---|---|---|")
            for r in unhandled:
                if r["category"] != k:
                    continue
                cs = r["callsites"]
                w("| %#010x | %s | %s | %s | %#x | %d/%d/%d%s | %d | %s |" % (
                    r["id"], r["name"], r["nvoc_class"], r["privilege"], r["flags"],
                    cs.get("kernel-rm", 0), cs.get("display", 0), cs.get("uvm", 0),
                    " explicit" if r["explicit"] else "", len(r["trace_probes"]), r["reason"]))
            w("")
    if explain:
        w("## How each handled id was read\n")
        for r in handled:
            for h in r["handlers"]:
                w("- %#010x %s: %s via %s (%s) guard `%s` labels %s" % (
                    r["id"], r["name"], h["provenance"], h["module"], h["src"], h["guard"],
                    h["labels"]))
        for v in inv["handled_not_exported"]:
            for h in inv["handled_extra_detail"][v]:
                w("- %s (not exported): %s via %s (%s)" % (v, h["provenance"], h["module"],
                                                          h["src"]))
        w("")
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--json", help="write the raw inventory to this file")
    ap.add_argument("--full", action="store_true", help="per-control lists in the markdown")
    ap.add_argument("--explain", action="store_true", help="list how each handled id was read")
    ap.add_argument("--top", type=int, default=0,
                    help="print the N unhandled controls with most call sites and trace hits")
    ap.add_argument("--ogkm", metavar="DIR",
                    help="open-gpu-kernel-modules tree (default: $CARAXES_OGKM, else "
                         "vendor/open-gpu-kernel-modules)")
    a = ap.parse_args()
    select_tree(os.path.abspath(a.ogkm or os.environ.get("CARAXES_OGKM") or DEFAULT_VENDOR))
    if not os.path.isdir(NV):
        sys.exit("vendor tree not found: %s" % VENDOR)
    inv = build()
    if a.json:
        with open(a.json, "w") as f:
            json.dump(inv, f, indent=1, default=list)
    print(md(inv, a.full, a.explain))
    if a.top:
        un = [r for r in inv["controls"] if r["routed"] and not r["handled"]
              and r["category"] != "E"]

        def score(r):
            cs = r["callsites"]
            return (len(r["trace_probes"]) * 3 + cs.get("display", 0) * 2 +
                    cs.get("uvm", 0) * 2 + cs.get("kernel-rm", 0) + 2 * len(r["explicit"]))
        print("\n## Top %d unhandled by call sites and native traces\n" % a.top)
        print("| Id | Name | Cat | krm/disp/uvm | explicit | trace probes |")
        print("|---|---|---|---|---|---|")
        for r in sorted(un, key=score, reverse=True)[:a.top]:
            cs = r["callsites"]
            print("| %#010x | %s | %s | %d/%d/%d | %d | %s |" % (
                r["id"], r["name"], r["category"], cs.get("kernel-rm", 0),
                cs.get("display", 0), cs.get("uvm", 0), len(r["explicit"]),
                ",".join(r["trace_probes"][:6])))


if __name__ == "__main__":
    main()
