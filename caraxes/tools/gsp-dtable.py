#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""gsp-dtable.py: the category D rule table of the device (docs/caraxes/gsp-plan/d-device-model.md
§2 and roadmap R0), generated per driver version from the plan's per-control table
(docs/caraxes/gsp-plan/d-controls.tsv) and the GSP surface inventory of one
open-gpu-kernel-modules tree (caraxes/tools/gsp-surface.py --json).

Writes crates/vfio-user-nvgpu/src/ver/v<major>/dmodel.rs with
  D_RULES          every category D control of the tree that no other handler answers: id,
                   name, group (§6), the plan's answer kind (REFUSE, DENY, ACK, TABLE, MODEL,
                   MIRROR), the model name of a MODEL row, the NV status of a REFUSE or DENY
                   row, the category it moves to (A, C, E or none), rank and the size of its
                   parameter struct in this version;
  D_FAMILIES       the 102 rows that belong to E as feature families (the names of
                   `FAMILIES` below), refused by chipctl.rs where chips/<chip>/chip.toml
                   [features] names the family absent;
  D_E_ACK          the ids of those whose plan answer is ACK: an absent family answers them
                   NV_OK with the request echoed (the kernel asserts or ignores the status);
  PROFILER_CLASSES the profiler classes whose allocation the device refuses (6.16);
  D_ENUMS          the SDK enum values the modelled controls read or write (`ENUMS` below,
                   the low bit of the `BITFIELDS`) and the NV statuses the device looks up by
                   name (`STATUSES`).

Ids come from the inventory (a TSV id that disagrees stops the generator), sizes from the
TSV (compiler-measured by the plan, §9), statuses from the tree's nvstatuscodes.h, classes
and enum values from the tree's SDK headers. Nothing here is typed as a number.

Usage:
  python3 caraxes/tools/gsp-dtable.py --ogkm DIR [--out FILE] [--date YYYY-MM-DD]

Python 3.11 or later, standard library only.
"""

import argparse
import collections
import datetime
import importlib.util
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
TSV = os.path.join(ROOT, "docs", "caraxes", "gsp-plan", "d-controls.tsv")
PLAN = "docs/caraxes/gsp-plan/d-device-model.md"


def _tables():
    spec = importlib.util.spec_from_file_location(
        "gsp_tables", os.path.join(ROOT, "caraxes", "tools", "gsp-tables.py"))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


# The §6 groups (the TSV's `group` column).
GROUPS = {
    "fifo": "6.1 FIFO, channels and TSGs",
    "gr": "6.2 GR, contexts and ZBC",
    "mem": "6.3 memory, caches and page tables",
    "bus": "6.4 bus, BAR and P2P",
    "disp": "6.5 display",
    "perf": "6.6 perf, clocks and power",
    "therm": "6.7 thermal and fan",
    "ecc": "6.8 ECC, RAS and RC recovery",
    "eng": "6.9 engines",
    "events": "6.10 events, interrupts and firmware tracing",
    "vgpu": "6.11 vGPU, SR-IOV and MIG internals",
    "nvlink": "6.12 NVLink, C2C and SLI bridge",
    "sec": "6.13 security, fuses and protected memory",
    "timer": "6.14 timers",
    "gpio": "6.15 GPIO and I2C",
    "prof": "6.16 profiling and SM debugger",
    "other": "6.17 other (GPU accounting)",
}

# The feature families of the rows that belong to E (§7 "Nobody on this chip"; the reason per
# control is the TSV's `reclass` column). Every E row must be named exactly once.
FAMILIES = [
    ("frl", "HDMI FRL (HDMI 2.1 fixed-rate link; the Turing C5 display HAL has none)", [
        "NV0073_CTRL_CMD_SPECIFIC_SET_HDMI_FRL_CONFIG",
        "NV0073_CTRL_CMD_SPECIFIC_SET_HDMI_FRL_FLUSH_MODE",
        "NV0073_CTRL_CMD_SPECIFIC_GET_HDMI_FRL_CAPACITY_COMPUTATION",
        "NV0073_CTRL_CMD_FRL_CONFIG_MACRO_PAD"]),
    ("dp2x", "DisplayPort 2.x (UHBR link training, lane and level tables, USB-C cable id)", [
        "NV0073_CTRL_CMD_DP2X_LINK_TRAINING_CTRL",
        "NV0073_CTRL_CMD_DP2X_GET_LANE_DATA",
        "NV0073_CTRL_CMD_DP2X_SET_LANE_DATA",
        "NV0073_CTRL_CMD_DP2X_SET_LEVEL_INFO_TABLE_DATA",
        "NV0073_CTRL_CMD_DP2X_GET_LEVEL_INFO_TABLE_DATA",
        "NV0073_CTRL_CMD_DP_GET_CABLEID_INFO_FROM_MACRO"]),
    ("display_later", "display features of later display generations (shared generic "
     "packets of the C8 HAL and later, the NV Display 5.0 SOR flush mode)", [
        "NV0073_CTRL_CMD_SPECIFIC_SET_SHARED_GENERIC_PACKET",
        "NV0073_CTRL_CMD_SPECIFIC_ACQUIRE_SHARED_GENERIC_PACKET",
        "NV0073_CTRL_CMD_SPECIFIC_RELEASE_SHARED_GENERIC_PACKET",
        "NVC370_CTRL_CMD_SET_SOR_FLUSH_MODE"]),
    ("display_embedded", "Tegra and automotive display and LVDS (DSI, serializer timings, "
     "safety region RAM and interrupts, LVDS SOR modes)", [
        "NV0073_CTRL_CMD_DFP_GET_DSI_MODE_TIMING",
        "NV0073_CTRL_CMD_DFP_GET_FIXED_MODE_TIMING",
        "NV0073_CTRL_CMD_SYSTEM_SET_REGION_RAM_RECTANGLES",
        "NV0073_CTRL_CMD_SYSTEM_CONFIGURE_SAFETY_INTERRUPTS",
        "NV5070_CTRL_CMD_GET_SOR_OP_MODE",
        "NV5070_CTRL_CMD_SET_SOR_OP_MODE"]),
    ("notebook_power", "notebook power: the platform request handler (ACPI GPS, NVPCF), "
     "Dynamic Boost, AUX power and AC/battery states, RTD3/GCX", [
        "NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_DEPENDENCY_CHECK",
        "NV2080_CTRL_CMD_INTERNAL_PMGR_UNSET_DYNAMIC_BOOST_LIMIT",
        "NV2080_CTRL_CMD_INTERNAL_PERF_GET_AUX_POWER_STATE",
        "NV2080_CTRL_CMD_INTERNAL_SMBPBI_PFM_REQ_HNDLR_CAP_UPDATE",
        "NV2080_CTRL_CMD_INTERNAL_PMGR_PFM_REQ_HNDLR_STATE_LOAD_SYNC",
        "NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_GET_PM1_STATE",
        "NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_SET_PM1_STATE",
        "NV2080_CTRL_CMD_INTERNAL_PMGR_PFM_REQ_HNDLR_UPDATE_EDPP_LIMIT",
        "NV2080_CTRL_CMD_INTERNAL_PMGR_PFM_REQ_HNDLR_CONFIGURE_TGP_MODE",
        "NV2080_CTRL_CMD_INTERNAL_PMGR_PFM_REQ_HNDLR_CONFIGURE_TURBO_V2",
        "NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_GET_VPSTATE_INFO",
        "NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_GET_VPSTATE_MAPPING",
        "NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_SET_VPSTATE",
        "NV2080_CTRL_CMD_PERF_SET_POWERSTATE",
        "NV2080_CTRL_CMD_PERF_SET_AUX_POWER_STATE",
        "NV2080_CTRL_CMD_INTERNAL_GCX_ENTRY_PREREQUISITE",
        "NV2080_CTRL_CMD_INTERNAL_THERM_PFM_REQ_HNDLR_STATE_INIT_SYNC",
        "NV2080_CTRL_CMD_INTERNAL_THERM_PFM_REQ_HNDLR_UPDATE_TGPU_LIMIT"]),
    ("ecc", "ECC configuration, error reset and kill-pointer injection (no GeForce board "
     "of TU106 or GB206 has ECC)", [
        "NV2080_CTRL_CMD_GPU_SET_ECC_CONFIGURATION",
        "NV2080_CTRL_CMD_GPU_RESET_ECC_ERROR_STATUS",
        "NV208F_CTRL_CMD_FB_ECC_SET_KILL_PTR",
        "NV208F_CTRL_CMD_FB_ECC_SET_WRITE_KILL"]),
    ("row_remap", "DRAM row and bank remapping (Ampere and later)", [
        "NV208F_CTRL_CMD_FB_CLEAR_REMAPPED_ROWS",
        "NV208F_CTRL_CMD_FB_CLEAR_REMAPPED_BANKS"]),
    ("mig_partitioning", "MIG partitioning internals (CTS ids, partitionable memory, "
     "runlist promotion, the partitioning mode)", [
        "NV2080_CTRL_CMD_INTERNAL_GPU_CHECK_CTS_ID_VALID",
        "NV2080_CTRL_CMD_INTERNAL_MEMSYS_SET_PARTITIONABLE_MEM",
        "NV2080_CTRL_CMD_INTERNAL_FIFO_PROMOTE_RUNLIST_BUFFERS",
        "NV2080_CTRL_CMD_INTERNAL_MIGMGR_SET_PARTITIONING_MODE"]),
    ("nvlink_sli", "NVLink peers and fabric recovery, the SLI bridge, its GPIOs and SLI "
     "boost sync (two GPUs and a bridge or NVSwitch)", [
        "NV2080_CTRL_CMD_INTERNAL_DETECT_HS_VIDEO_BRIDGE",
        "NV2080_CTRL_CMD_INTERNAL_DISP_SET_SLI_LINK_GPIO_SW_CONTROL",
        "NV2080_CTRL_CMD_INTERNAL_GPIO_PROGRAM_DIRECTION",
        "NV2080_CTRL_CMD_INTERNAL_GPIO_PROGRAM_OUTPUT",
        "NV2080_CTRL_CMD_INTERNAL_GPIO_READ_INPUT",
        "NV2080_CTRL_CMD_INTERNAL_GPIO_ACTIVATE_HW_FUNCTION",
        "NV2080_CTRL_CMD_INTERNAL_PERF_GPU_BOOST_SYNC_SET_CONTROL",
        "NV2080_CTRL_CMD_INTERNAL_PERF_GPU_BOOST_SYNC_SET_LIMITS",
        "NV2080_CTRL_CMD_INTERNAL_MEMSYS_SET_PEER_ATS_CONFIG",
        "NV2080_CTRL_CMD_BUS_SET_P2P_MAPPING",
        "NV2080_CTRL_CMD_BUS_UNSET_P2P_MAPPING",
        "NV0080_CTRL_CMD_INTERNAL_FIFO_RC_AND_PERMANENTLY_DISABLE_CHANNELS",
        "NV2080_CTRL_CMD_INTERNAL_RECOVER_ALL_COMPUTE_CONTEXTS"]),
    ("ampere_later", "Ampere-and-later features Turing lacks (the faulted-bit clear path, "
     "raw compression mode for FBSR, the vidmem access-bit buffer)", [
        "NV2080_CTRL_CMD_FIFO_CLEAR_FAULTED_BIT",
        "NV2080_CTRL_CMD_INTERNAL_MEMSYS_PROGRAM_RAW_COMPRESSION_MODE",
        "NVC763_CTRL_CMD_VIDMEM_ACCESS_BIT_DUMP"]),
    ("ga10x_pcie_ro", "PCIe relaxed ordering control, set only by the GA100-GA107 bus HAL", [
        "NV2080_CTRL_CMD_INTERNAL_BIF_SET_PCIE_RO"]),
    ("hopper_platform", "Hopper and Grace platform features (the coherent C2C probe filter, "
     "Confidential Computing memory transfers)", [
        "NV2080_CTRL_CMD_INTERNAL_MEMSYS_CLEAN_LTC_PROBE_FILTER",
        "NV2080_CTRL_CMD_INTERNAL_MEMMGR_MEMORY_TRANSFER_WITH_GSP"]),
    ("blackwell", "Blackwell features (GSP-lite CMC engines, DRAM encryption, MMU GCC debug "
     "mode, HES)", [
        "NV2080_CTRL_CMD_INTERNAL_SEND_CMC_LIBOS_BUFFER_INFO",
        "NV2080_CTRL_CMD_FB_SET_DRAM_ENCRYPTION_CONFIGURATION",
        "NV83DE_CTRL_CMD_DEBUG_SET_MODE_MMU_GCC_DEBUG",
        "NVB0CC_CTRL_CMD_RESERVE_HES",
        "NVB0CC_CTRL_CMD_RELEASE_HES"]),
    ("vgpu_paths", "paths only a vGPU host or a vGPU guest takes (host scheduler, plugin P2P "
     "caps, PMA streams over RPC, accounting clears)", [
        "NV2080_CTRL_CMD_FIFO_OBJSCHED_SET_STATE",
        "NV2080_CTRL_CMD_INTERNAL_SET_P2P_CAPS",
        "NV2080_CTRL_CMD_INTERNAL_REMOVE_P2P_CAPS",
        "NVB0CC_CTRL_CMD_ALLOC_PMA_STREAM",
        "NVB0CC_CTRL_CMD_FREE_PMA_STREAM",
        "NVB0CC_CTRL_CMD_INTERNAL_QUIESCE_PMA_CHANNEL",
        "NV0000_CTRL_CMD_GPUACCT_CLEAR_ACCOUNTING_DATA"]),
    ("windows_only", "Windows-only paths (DRR and blit-tearing notifications, system-memory "
     "access disable, non-preemptable debugger contexts)", [
        "NV0073_CTRL_CMD_SYSTEM_NOTIFY_DRR_MSCG_WAR",
        "NV5070_CTRL_CMD_SET_MEMPOOL_WAR_FOR_BLIT_TEARING",
        "NV2080_CTRL_CMD_INTERNAL_BIF_DISABLE_SYSTEM_MEMORY_ACCESS",
        "NV83DE_CTRL_CMD_DEBUG_SET_NON_PREEMPTABLE_DEBUGGER_CONTEXT",
        "NV83DE_CTRL_CMD_DEBUG_SET_RECOVERY_SUPPRESSION_TIMEOUT"]),
    ("legacy_uncalled", "verification-only, superseded and uncalled paths (VERIF ONLY "
     "channel properties, the deferred API, legacy contexts, WDDM GfxP pools, MODS and "
     "instrumented-ucode controls, compressible ZBC references D16 keeps away)", [
        "NV0080_CTRL_CMD_FIFO_SET_CHANNEL_PROPERTIES",
        "NV2080_CTRL_CMD_SET_GPFIFO",
        "NV2080_CTRL_CMD_FIFO_UPDATE_CHANNEL_INFO",
        "NV5080_CTRL_CMD_DEFERRED_API",
        "NV5080_CTRL_CMD_REMOVE_API",
        "NV5080_CTRL_CMD_DEFERRED_API_INTERNAL",
        "NV0080_CTRL_CMD_INTERNAL_MEMSYS_SET_ZBC_REFERENCED",
        "NV2080_CTRL_CMD_GPU_INITIALIZE_CTX",
        "NV2080_CTRL_CMD_GR_GFX_POOL_INITIALIZE",
        "NV2080_CTRL_CMD_GR_GFX_POOL_ADD_SLOTS",
        "NV2080_CTRL_CMD_GR_GFX_POOL_REMOVE_SLOTS",
        "NV0080_CTRL_DMA_SET_DEFAULT_VASPACE",
        "NV208F_CTRL_CMD_FB_CTRL_GPU_CACHE",
        "NV2080_CTRL_CMD_SET_RC_WATCHDOG_INFO",
        "NV2080_CTRL_CMD_INTERNAL_UCODE_INSTRUMENTATION_GET_STATE",
        "NV2080_CTRL_CMD_INTERNAL_UCODE_INSTRUMENTATION_SET_STATE",
        "NV2080_CTRL_CMD_INTERNAL_PERF_PERFMON_CLIENT_RESERVATION_CHECK",
        "NV2080_CTRL_CMD_INTERNAL_PERF_PERFMON_CLIENT_RESERVATION_SET"]),
]

# DENY statuses the plan names per group rather than per row (the TSV note names the others).
DENY_STATUS = {
    # 6.1 design: the host-global rule answers NV_ERR_INSUFFICIENT_PERMISSIONS
    # (d-device-model.md:256).
    "NV0080_CTRL_CMD_FIFO_START_RUNLIST": "NV_ERR_INSUFFICIENT_PERMISSIONS",
    "NV2080_CTRL_CMD_FIFO_CONFIG_CTXSW_TIMEOUT": "NV_ERR_INSUFFICIENT_PERMISSIONS",
    # 6.3 design: the DRAM setters (d-device-model.md:345).
    "NV2080_CTRL_CMD_FB_SET_RRD": "NV_ERR_INSUFFICIENT_PERMISSIONS",
    "NV2080_CTRL_CMD_FB_SET_READ_LIMIT": "NV_ERR_INSUFFICIENT_PERMISSIONS",
    "NV2080_CTRL_CMD_FB_SET_WRITE_LIMIT": "NV_ERR_INSUFFICIENT_PERMISSIONS",
}

# The profiler classes the device refuses to allocate (6.16 design, d-device-model.md:1190).
PROFILER_CLASSES = ["GF100_PROFILER", "MAXWELL_PROFILER", "MAXWELL_PROFILER_CONTEXT",
                    "MAXWELL_PROFILER_DEVICE"]

# The controls with a hand-written model (R1, display/mod.rs); their parameter structs are
# laid out by caraxes/tools/ctrl-layouts.py. (TSV answer MODEL:<name> or ACK.)
MODELLED = ["NV0073_CTRL_CMD_SPECIFIC_HDCP_CTRL", "NV0073_CTRL_CMD_DP_CONFIGURE_FEC",
            "NV5070_CTRL_CMD_GET_RG_STATUS"]

# The controls the device models of round 2 answer (R2, the open MODEL rows, the ZBC table:
# dmodel.rs `MODELLED_HERE`, rc.rs, zbc.rs), outside the display; their parameter structs are
# laid out by ctrl-layouts.py like those of MODELLED.
MODELLED_HERE = [
    # R2: the CUDA error and teardown paths, nvidia-smi admin, the BAR0 mmap policy.
    "NV2080_CTRL_CMD_FIFO_DISABLE_CHANNELS",
    "NV2080_CTRL_CMD_GPU_REPORT_NON_REPLAYABLE_FAULT",
    "NV2080_CTRL_CMD_INTERNAL_RC_WATCHDOG_TIMEOUT",
    "NV2080_CTRL_CMD_INTERNAL_LOG_OOB_XID",
    "NV2080_CTRL_CMD_RC_SET_CLEAN_ERROR_HISTORY",
    "NV2080_CTRL_CMD_GPU_SET_COMPUTE_MODE_RULES",
    "NV0000_CTRL_CMD_GPUACCT_SET_ACCOUNTING_STATE",
    "NV2080_CTRL_CMD_GPU_VALIDATE_MEM_MAP_REQUEST",
    # The ZBC table (6.2, R3 pulled forward).
    "NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR",
    "NV9096_CTRL_CMD_SET_ZBC_DEPTH_CLEAR",
    "NV9096_CTRL_CMD_SET_ZBC_CLEAR_TABLE",
    "NV9096_CTRL_CMD_SET_ZBC_STENCIL_CLEAR",
    # The other open MODEL rows outside the display.
    "NV2080_CTRL_CMD_INTERNAL_GR_SET_FECS_TRACE_HW_ENABLE",
    "NV2080_CTRL_CMD_INTERNAL_GR_SET_FECS_TRACE_RD_OFFSET",
    "NV2080_CTRL_CMD_INTERNAL_GR_SET_FECS_TRACE_WR_OFFSET",
    "NV0080_CTRL_CMD_DMA_FLUSH",
    "NV2080_CTRL_CMD_INTERNAL_MEMSYS_FLUSH_L2_ALL_RAMS_AND_CACHES",
    "NV2080_CTRL_CMD_INTERNAL_FBSR_INIT",
    "NV2080_CTRL_CMD_PERF_RATED_TDP_SET_CONTROL",
    "NV83DE_CTRL_CMD_DEBUG_SET_MODE_MMU_DEBUG",
]

# Parameter structs laid out beside those of the rows: the getters that answer from the
# models (not D rows themselves: the ZBC getters of categories A and B, the MMU debug getter
# of B), and the structs the FECS setters name through a typedef (ctrl2080internal.h:614,
# :658, :662), which ctrl-layouts.py does not follow.
MODEL_STRUCTS = ["NV9096_CTRL_GET_ZBC_CLEAR_TABLE_PARAMS",
                 "NV9096_CTRL_GET_ZBC_CLEAR_TABLE_ENTRY_PARAMS",
                 "NV83DE_CTRL_DEBUG_GET_MODE_MMU_DEBUG_PARAMS",
                 "NV2080_CTRL_INTERNAL_GR_FECS_TRACE_HW_ENABLE_PARAMS",
                 "NV2080_CTRL_INTERNAL_GR_FECS_TRACE_OFFSET_PARAMS"]

# SDK enum values the models use: every `#define` whose name matches (header, regex).
ENUMS = [
    ("ctrl/ctrl0073/ctrl0073specific.h", r"NV0073_CTRL_SPECIFIC_HDCP_CTRL_CMD_\w+"),
    ("ctrl/ctrl0073/ctrl0073specific.h", r"NV0073_CTRL_SPECIFIC_HDCP_CTRL_ERR_\w+_YES"),
    ("ctrl/ctrl5070/ctrl5070rg.h", r"NV5070_CTRL_CMD_GET_RG_STATUS_\w+_(?:NO|YES)"),
    # Round 2 (dmodel.rs, rc.rs, zbc.rs).
    ("ctrl/ctrl9096.h", r"NV9096_CTRL_ZBC_CLEAR_OBJECT_TYPE_\w+"),
    ("ctrl/ctrl2080/ctrl2080gpu.h", r"NV2080_CTRL_GPU_COMPUTE_MODE_RULES_\w+"),
    ("ctrl/ctrl0000/ctrl0000gpuacct.h", r"NV0000_CTRL_GPU_ACCOUNTING_STATE_\w+"),
    ("ctrl/ctrl83de/ctrl83dedebug.h", r"NV83DE_CTRL_CMD_DEBUG_[GS]ET_MODE_MMU_DEBUG_\w+"),
    ("nverror.h", r"ROBUST_CHANNEL_(?:FIFO_ERROR_MMU_ERR_FLT|PREEMPTIVE_REMOVAL)"),
]

# SDK bit fields the models test (`#define NAME hi:lo`): the value is the low bit.
BITFIELDS = [
    ("ctrl/ctrl0080/ctrl0080dma.h", r"NV0080_CTRL_DMA_FLUSH_TARGET_UNIT_(?:L2|COMPTAG|FB)"),
]

# The NV statuses the device code looks up by name (nvstatuscodes.h).
STATUSES = ["NV_OK", "NV_ERR_NOT_SUPPORTED", "NV_ERR_INVALID_ARGUMENT",
            "NV_ERR_INSUFFICIENT_PERMISSIONS", "NV_ERR_INSUFFICIENT_RESOURCES",
            "NV_ERR_PROTECTION_FAULT", "NV_ERR_INVALID_CHANNEL"]

ANSWER_KIND = {"REFUSE": "Refuse", "DENY": "Deny", "ACK": "Ack", "TABLE": "Table",
               "MODEL": "Model", "MIRROR": "Mirror"}


def read_tsv():
    rows, head = [], None
    with open(TSV, encoding="utf-8") as f:
        for line in f:
            if line.startswith("#"):
                continue
            cols = line.rstrip("\n").split("\t")
            if head is None:
                head = cols
                continue
            rows.append(dict(zip(head, cols)))
    return rows


def status_codes(sdk):
    out = {}
    with open(os.path.join(sdk, "nvstatuscodes.h")) as f:
        for m in re.finditer(r"NV_STATUS_CODE\((NV_\w+),\s*(0x[0-9A-Fa-f]+)", f.read()):
            out[m.group(1)] = int(m.group(2), 16)
    return out


def class_ids(sdk, names):
    out = {}
    rx = re.compile(r"#define\s+(\w+)\s+\((0x[0-9A-Fa-f]+)U?\)")
    for fn in sorted(os.listdir(os.path.join(sdk, "class"))):
        with open(os.path.join(sdk, "class", fn), encoding="utf-8", errors="replace") as f:
            for m in rx.finditer(f.read()):
                if m.group(1) in names:
                    out.setdefault(m.group(1), (int(m.group(2), 16), "class/%s" % fn))
    missing = [n for n in names if n not in out]
    if missing:
        sys.exit("no class define for %s" % ", ".join(missing))
    return out


def enum_values(sdk):
    out = []
    for hdr, rx in ENUMS:
        with open(os.path.join(sdk, hdr), encoding="utf-8") as f:
            for m in re.finditer(r"#define\s+(%s)\s+\(?(0x[0-9A-Fa-f]+|\d+)U?\)?\s*$" % rx,
                                 f.read(), re.M):
                out.append((m.group(1), int(m.group(2), 0)))
    for hdr, rx in BITFIELDS:
        with open(os.path.join(sdk, hdr), encoding="utf-8") as f:
            for m in re.finditer(r"#define\s+(%s)\s+(\d+):(\d+)\s*$" % rx, f.read(), re.M):
                out.append((m.group(1), int(m.group(3))))
    return out


def rule_status(r, codes):
    a = r["answer"]
    kind = a.split(" ")[0].split(":")[0]
    named = re.findall(r"NV_ERR_\w+", a)
    if kind == "REFUSE":
        st = named[0] if named else "NV_ERR_NOT_SUPPORTED"
    elif kind == "DENY":
        named = named or re.findall(r"NV_ERR_\w+", r["note"])
        st = DENY_STATUS.get(r["name"]) or (named[0] if named else "NV_ERR_NOT_SUPPORTED")
    elif kind == "ACK":
        st = "NV_OK"
    else:
        return "0", None
    return "0x%X" % codes[st], st


def rust_str(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--ogkm", required=True, help="open-gpu-kernel-modules tree")
    ap.add_argument("--out", help="output file (default src/ver/v<major>/dmodel.rs)")
    ap.add_argument("--date", default=datetime.date.today().isoformat())
    a = ap.parse_args()
    gt = _tables()
    ogkm = os.path.abspath(a.ogkm)
    sdk = os.path.join(ogkm, "src", "common", "sdk", "nvidia", "inc")
    inv = gt.load_inventory(ogkm)
    driver = inv["driver"]
    major = driver.split(".")[0]
    out = a.out or os.path.join(ROOT, "crates", "vfio-user-nvgpu", "src", "ver",
                                "v%s" % major, "dmodel.rs")
    by_name = {c["name"]: c for c in inv["controls"] if c["routed"]}
    codes = status_codes(sdk)
    size_col = "size_%s" % major
    rows, skipped = [], []
    for r in read_tsv():
        c = by_name.get(r["name"])
        if c is None:
            if r[size_col] != "-":
                sys.exit("%s is in d-controls.tsv but not routed in %s" % (r["name"], driver))
            continue
        if c["id"] != int(r["id"], 16):
            sys.exit("%s: TSV id %s, inventory %#x" % (r["name"], r["id"], c["id"]))
        if c["class_category"] != "D":
            sys.exit("%s: category %s in the %s inventory" % (r["name"], c["class_category"],
                                                             driver))
        if gt.other_handled(c):
            skipped.append(r["name"])
            continue
        rows.append(r)
    fam_of = {}
    for fam, _, names in FAMILIES:
        for n in names:
            if n in fam_of:
                sys.exit("%s in two families" % n)
            fam_of[n] = fam
    e_rows = [r for r in rows if r["reclass"].startswith("E:")]
    for r in e_rows:
        if r["name"] not in fam_of:
            sys.exit("E row %s is in no family" % r["name"])
    names_e = {r["name"] for r in e_rows}
    stray = [n for n in fam_of if n not in names_e and n in by_name
             and n not in skipped]
    if stray:
        sys.exit("family names that are no E row of d-controls.tsv: %s" % ", ".join(stray))
    classes = class_ids(sdk, PROFILER_CLASSES)
    enums = enum_values(sdk)

    w = ["// SPDX-License-Identifier: MIT",
         "// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>",
         "//! The category D rule table of driver %s (%s §2, roadmap R0): generated by" % (driver, PLAN),
         "//! `caraxes/tools/gsp-dtable.py --ogkm <tree>` from docs/caraxes/gsp-plan/d-controls.tsv and",
         "//! the inventory of `caraxes/tools/gsp-surface.py --json` on %s. Do not edit; rerun" % a.date,
         "//! the generator. Header paths are relative to `src/common/sdk/nvidia/inc/` of",
         "//! open-gpu-kernel-modules %s." % driver,
         "",
         "use crate::ver::{DAnswer, DRule, Family};",
         "",
         "/// The category D controls of the inventory no other handler answers: the number",
         "/// `D_RULES` covers.",
         "pub const D_INVENTORY: usize = %d;" % len(rows),
         "",
         "/// One rule per category D control, in the TSV's order (§6 groups).",
         "pub const D_RULES: &[DRule] = &["]
    for r in rows:
        kind = r["answer"].split(" ")[0]
        base = kind.split(":")[0]
        model = kind.split(":", 1)[1] if ":" in kind else ""
        st, st_name = rule_status(r, codes)
        why = re.search(r"\((.*)\)", r["answer"])
        why = why.group(1) if why and base in ("REFUSE", "TABLE") else ""
        reclass = r["reclass"][:1] if r["reclass"] else "-"
        w.append("    DRule {")
        w.append("        id: %s," % gt.rust_id(int(r["id"], 16)))
        w.append('        name: "%s",' % r["name"])
        w.append('        group: "%s",' % r["group"])
        w.append("        answer: DAnswer::%s," % ANSWER_KIND[base])
        w.append('        model: "%s",' % model)
        w.append("        status: %s,%s" % (st, " // %s" % st_name if st_name else ""))
        w.append("        reclass: '%s'," % reclass)
        w.append("        rank: %s," % r["rank"])
        w.append("        size: %s," % r[size_col])
        w.append('        why: "%s",' % rust_str(why))
        w.append("    },")
    w.append("];")
    w.append("")
    w.append("/// The rows that belong to category E, by feature family. A chip names each family")
    w.append("/// absent or present in `chips/<chip>/chip.toml` `[features]`; chipctl.rs refuses")
    w.append("/// the controls of absent families.")
    w.append("pub const D_FAMILIES: &[Family] = &[")
    for fam, what, names in FAMILIES:
        cs = [r for r in e_rows if fam_of[r["name"]] == fam]
        if not cs:
            continue
        cs.sort(key=lambda r: int(r["id"], 16))
        blocks = collections.defaultdict(list)
        for r in cs:
            d = r["define_%s" % major]
            if d:
                h, line = d.rsplit(":", 1)
                blocks[h].append(int(line))
        w.append("    Family {")
        w.append('        name: "%s",' % fam)
        w.append('        what: "%s",' % rust_str(what))
        w.append("        headers: &[")
        for h in sorted(blocks):
            ls = blocks[h]
            w.append('            "%s:%d-%d",' % (h, min(ls), max(ls)) if min(ls) != max(ls)
                     else '            "%s:%d",' % (h, ls[0]))
        w.append("        ],")
        w.append("        controls: &[")
        for r in cs:
            w.append('            (%s, "%s"),' % (gt.rust_id(int(r["id"], 16)), r["name"]))
        w.append("        ],")
        w.append("    },")
    w.append("];")
    w.append("")
    w.append("/// E rows the plan answers ACK: the kernel asserts on a failure or ignores the status,")
    w.append("/// so an absent family acknowledges them (NV_OK, request echoed) instead of refusing.")
    w.append("pub const D_E_ACK: &[u32] = &[")
    for r in e_rows:
        if r["answer"].startswith("ACK"):
            w.append("    %s, // %s" % (gt.rust_id(int(r["id"], 16)), r["name"]))
    w.append("];")
    w.append("")
    w.append("/// The profiler classes whose GSP_RM_ALLOC the device answers")
    w.append("/// NV_ERR_INSUFFICIENT_PERMISSIONS (6.16).")
    w.append("pub const PROFILER_CLASSES: &[(u32, &str)] = &[")
    for n in PROFILER_CLASSES:
        v, h = classes[n]
        w.append('    (0x%04X, "%s"), // %s' % (v, n, h))
    w.append("];")
    w.append("")
    w.append("/// SDK enum values of the modelled controls and the NV statuses the rules use (name,")
    w.append("/// value; nvstatuscodes.h).")
    w.append("pub const D_ENUMS: &[(&str, u32)] = &[")
    for n, v in enums + [(s, codes[s]) for s in STATUSES]:
        w.append('    ("%s", 0x%X),' % (n, v))
    w.append("];")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "w") as f:
        f.write("\n".join(w) + "\n")
    cnt = collections.Counter(r["answer"].split(" ")[0].split(":")[0] for r in rows)
    rc = collections.Counter(r["reclass"][:1] or "-" for r in rows)
    print("%s: D %d (%s), reclass %s, %d families, %d E ACK%s" % (
        os.path.relpath(out, ROOT), len(rows),
        ", ".join("%s %d" % kv for kv in sorted(cnt.items())),
        ", ".join("%s %d" % kv for kv in sorted(rc.items())),
        sum(1 for f, _, _ in FAMILIES if any(fam_of[r["name"]] == f for r in e_rows)),
        sum(1 for r in e_rows if r["answer"].startswith("ACK")),
        "; answered elsewhere now: " + ", ".join(skipped) if skipped else ""))


if __name__ == "__main__":
    main()
