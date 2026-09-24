<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
<!-- Research by a read-only subagent on 2026-09-24 (P23, category D of gsp-surface.md §3), with five read-only helper agents for the per-group reading. Numbers from caraxes/tools/gsp-surface.py at worktree HEAD 61e0b6a against the 610.57.04 and 615.71.09 trees. No crate file was changed. -->
# Category D: the controls that need device modelling (615.71.09)

[gsp-surface.md](../gsp-surface.md) §3 sorts the unhandled GSP-bound controls into five
categories; D, "needs device modelling", is the largest and the one without a generic recipe.
This document is the plan for it: every D control sorted into a subsystem group, per group the
state a device model needs, what the real GSP-RM does, who calls it and when, the truth source,
the isolation risk and a design with an estimate; then the roadmap by client (CUDA, the Wayland
display with NVENC, Vulkan games, nobody), the controls that belong to another category, and the
open questions that need a measurement on the card. The per-control table with all columns is
[d-controls.tsv](d-controls.tsv) (one row per control, tab-separated, same order as here).

## Summary

- **No workload has reached D so far.** Across every Caraxes run log (610 and 615 guests: boot,
  the CUDA and nvml gates, the display gates with GNOME, Weston and Vulkan), exactly one D control
  appears: `NV0073_CTRL_CMD_SPECIFIC_HDCP_CTRL` 0x00730282, sent by the DP library on a
  hot-unplug and answered NOT_SUPPORTED
  (`caraxes/runs/display-cd-2026-09-24/d11-unplug/device-trace.txt:16`). Across the 79 native
  traces of the real card with 610 user space (CUDA, cuda-gdb, OpenCL, GL, GLES, EGL, Vulkan,
  NVENC, NVDEC, NVML), exactly one appears: `NV83DE_CTRL_CMD_DEBUG_SET_MODE_MMU_DEBUG` 0x83DE0307
  from cuda-gdb (`matrix/traces/610.57.04/cuda-gdb.jsonl:2384`, status 0). So D is not a wall for
  the Linux CUDA client or the Wayland desktop that run today. It is the surface of error and
  teardown paths, suspend, tools, the next display features (the 615 DP library, DSC at 4K144,
  VRR) and platform features this board does not have.
- **308 controls** (305 in 610.57.04, 3 new in 615.71.09). Two change layout between the two
  versions (§4). 279 have no kernel-side body in the open tree, so the real behaviour is inside
  the closed GSP-RM firmware. For the silicon, the only oracle is a read-only query to the host
  RM, which runs the same 615.71.09 on the same TU106.
- **Answers.** 200 REFUSE (feature absent), 37 DENY (refused for isolation), 21 ACK, 42 MODEL, 3
  MIRROR, 5 TABLE; 4 of the REFUSE rows carry the status the header names (3 of them
  NV_ERR_INVALID_ARGUMENT). **Nothing in D should be forwarded to the
  host as it is.** Many of the setters are NON_PRIVILEGED on the host too, and the suspend-path
  FIFO controls would stop every channel on the physical GPU, the desktop included.
- **Reclassification.** 126 of the 308 belong to other categories: 102 to E (never sent to a
  TU106 desktop board in GSP-client mode), 17 to C (derived from the guest's channel and RC
  state), 7 to A (static data, among them 0x20800A3E, §3). That leaves 182 in D.
- **Work.** About 23 to 41 engineer-days for everything (§5). The first pass costs about 3 days:
  the generic rule table, which answers about 260 controls with a labelled REFUSE/DENY/ACK, plus
  the two display controls a 615 guest sends on every plug and unplug. After that, work follows the
  clients: CUDA error paths, Vulkan-game ZBC, suspend, tools (§7).

## 1. Scope and numbers

`python3 caraxes/tools/gsp-surface.py --ogkm <tree> --json out.json` at HEAD 61e0b6a:

| Item | 610.57.04 | 615.71.09 | Union |
|---|---:|---:|---:|
| Routed controls unhandled | 653 | 669 | |
| of which D | 305 | 308 | 308 |
| D with a kernel, modeset or UVM call site | 155 | 157 | |
| D seen in a native trace of the real card (79 probes, 610 user space) | 1 | 1 | 1 |
| D seen in any Caraxes run log (610 and 615 guests) | | | 1 |

The union is 308: all 305 D controls of 610 are D in 615 as well, and three are new in 615
(`NV0073_CTRL_CMD_SPECIFIC_HDCP_KEY_DECRYPTION` 0x007302B2, `NV0073_CTRL_CMD_DP_SET_WAR_FLAGS`
0x00731391, `NV2080_CTRL_CMD_INTERNAL_PERF_CLIENT_PERF_MODES_SET_CONTROL` 0x20800B20). The 306 of
gsp-surface.md was the crate of that morning; since then `GR_CTXSW_ZCULL_BIND` and a few others
got arms (ctrl.rs:1155), so the count moves with the crate. Rerun the script before
implementing a group.

By privilege (`RMCTRL_FLAGS_*`, 615 `src/nvidia/inc/kernel/rmapi/control.h:175-236`): 112 user
(NON_PRIVILEGED), 100 admin (PRIVILEGED), 64 INTERNAL (only the kernel RM sends them), 32 kernel
(KERNEL_PRIVILEGED). Admin in a guest means root in the guest, which the device must treat as
untrusted: a PRIVILEGED control is not safer than a user one.

**Where the real behaviour lives.** 279 of the 308 have no kernel-side body in the open tree:
a ROUTE_TO_PHYSICAL control without PHYSICAL_IMPLEMENTED_ON_VGPU_GUEST gets a NULL handler
in its NVOC export entry (`NVOC_EXPORTED_METHOD_DISABLED_BY_FLAG`, 615
`src/nvidia/inc/kernel/rmapi/control.h:156-158`; for example `GPU_SET_POWER`,
`src/nvidia/generated/g_subdevice_nvoc.c:4128-4131`) and `rmresControl_Prologue`
(615 `src/nvidia/src/kernel/rmapi/resource.c:254-296`) sends the control to GSP as
`GSP_RM_CONTROL`. The physical implementation is in the closed GSP-RM firmware. The 29 with a
body are kernel-side wrappers that forward (`FIFO_DISABLE_CHANNELS`, 615
`src/nvidia/src/kernel/gpu/fifo/kernel_fifo_ctrl.c:744`; `GR_CTXSW_PM_MODE`,
`gpu/gr/kernel_graphics.c:3984`) or do kernel work first (`DMA_FLUSH`, `gpu/mem_mgr/dma.c:663`;
the timed semaphore, `gpu/timed_semaphore.c:272`). So "what GSP-RM does" below is inferred from
the SDK header comment, the kernel-side HAL code that programs the same hardware, and what the
caller does with the answer; the groups say where. The one oracle for the closed side is the
host: it runs the same 615.71.09 on the same TU106 with the real GSP-RM, so a read-only control
the device issues as a host RM client on its own host objects returns the real answer
(`forward_or`, ctrl.rs:1213-1233). Setters on host objects act on the GPU the host desktop uses.

**Who sends them.** In all Caraxes run logs (`caraxes/runs/*/device-trace.txt`, 610 and 615
guests; init, the CUDA and nvml gates, the display gates with GNOME, Weston and Vulkan) exactly
one D control appears: `NV0073_CTRL_CMD_SPECIFIC_HDCP_CTRL` on a hot-unplug
(`caraxes/runs/display-cd-2026-09-24/d11-unplug/device-trace.txt:16`, answered NOT_SUPPORTED by
the display fallback). In the 79 native traces of the real card
(`matrix/traces/610.57.04/**/*.jsonl`: CUDA core, launch, JIT, managed, hostreg, torch, cuda-gdb,
OpenCL, GL, GLES, EGL on GBM, Wayland, X, Vulkan enum, offscreen and ray tracing, NVENC, NVDEC,
NVML, rm-direct) exactly one appears: `NV83DE_CTRL_CMD_DEBUG_SET_MODE_MMU_DEBUG` from cuda-gdb
(`cuda-gdb.jsonl:2384`, status 0). Today every D control is answered NV_ERR_NOT_SUPPORTED with
the label `unanswered` (ctrl.rs:1170-1175). D is therefore not on the path of any workload the
project has run; it is the surface of error paths, suspend, tools, other connectors and
platform features. That sets the order of the roadmap (§4).

**The output test does not decide much.** gsp-surface.md §3 D rule 1 proposes to acknowledge
setters whose parameters have no output fields. Read from the SDK comment blocks of the 615
headers: 91 D controls mark output fields, 41 mark only inputs, 19 have no parameters, 156 carry
no in/out marking at all. The mechanical rule covers 60; the rest were decided by reading, in the
`Answer` column below.

## 2. Answer rules

Every D control gets one of seven answers. The column `Answer` of the group tables uses these
words; the model names refer to the group sections of §6. No control ended up HOST-RO as its
answer: host reads appear only as one-off measurements (§8) and in the existing L2 flush.

| Answer | Meaning | Label (D9) |
|---|---|---|
| `ACK` | NV_OK, request echoed, a note naming the control; only where no caller reads an output or depends on an effect | `synthesized (acknowledged, no effect)` |
| `REFUSE` | NV_ERR_NOT_SUPPORTED (or the status the real RM gives when the feature is absent), feature absent on this emulated board | `refused (feature absent on this board)` |
| `DENY` | refused although a client may want it, because the effect would reach shared host hardware | `refused (isolation)` |
| `MODEL:<name>` | answered from a device model's state | `guest (<model>)` or `synthesized (<model>)` |
| `MIRROR` | forwarded to the host mirror object of the guest's own channel or TSG, after the ownership check | `host (mirror object)` |
| `HOST-RO` | read-only query of the host RM as a client, filtered | `host (read-only, filtered)` |
| `TABLE` | static chip or board data (belongs to category A) | the table's label |

Placement: one module `crates/vfio-user-nvgpu/src/dmodel.rs` (new) holding the per-control rule
table (id to answer kind, generated per driver version like `ver/v615.rs`, D14) and the
dispatcher hook before the `unanswered` arm of ctrl.rs; the models themselves go to the modules
named per group. A unit test walks the table against the inventory JSON so a new driver version
that adds a D control fails the test instead of falling through to `unanswered`.

## 3. 0x20800A3E belongs to A

The inventory lists `subdeviceCtrlCmdGrStaticGetFecsTraceDefines`, id 0x20800A3E, as D ("no
getter shape": it has no named `#define`, so the heuristic sees only the handler name). It is the
legacy, non-KGR form of the static FECS trace getter:

- export entry: flags 0x10040 (ROUTE_TO_PHYSICAL | GSP_PLUGIN_FOR_VGPU_GSP), methodId 0x20800a3e,
  parameters `NV2080_CTRL_INTERNAL_STATIC_GR_GET_FECS_TRACE_DEFINES_PARAMS` (615
  `src/nvidia/generated/g_subdevice_nvoc.c:6412-6416`); the id is only the message id 0x3E of that
  struct (615 `ctrl2080/ctrl2080internal.h:726`);
- the payload is per-engine static hardware data, the FECS record size and the timestamp tag masks
  and shifts (`ctrl2080internal.h:695-730`), 615 and 610 of equal size;
- the kernel RM never sends 0x20800A3E; it sends the KGR form 0x20800A3F
  (`ctrl2080internal.h:732`) from `kgraphicsLoadStaticInfo_KERNEL` (615
  `src/nvidia/src/kernel/gpu/gr/kernel_graphics.c:1486-1489`; a refusal only leaves `pFecsTraceDefines` NULL, `:1492-1502`), which the device already answers
  NOT_SUPPORTED on purpose (`crates/vfio-user-nvgpu/src/kgr.rs:452`, FECS tracing off).

So it is category A with the same answer as 0x20800A3F: one arm for both ids in kgr.rs, the same
chip-table values once FECS tracing is modelled, NOT_SUPPORTED until then. The group tables below
still list it under GR, answered REFUSE like 0x20800A3F today.

## 4. Layout changes between 610.57.04 and 615.71.09

`sizeof` of every D parameter struct compiled against each tree's SDK headers
(`cc -std=gnu11 -DNV_LINUX`, all headers under `src/common/sdk/nvidia/inc/ctrl/`), plus a
comparison of the comment-stripped struct bodies to catch equal-size layout changes. Two D
controls change layout between 610 and 615:

| Control | Id | 610 | 615 | Change |
|---|---|---:|---:|---|
| `NV2080_CTRL_CMD_INTERNAL_CONTROL_GSP_TRACE` | 0x208001E3 | 32 | 32 | `NvU16 targetTask` inserted before `flag`, which moves from offset 24 to 26 (610 `ctrl2080internal.h:4054-4060`, 615 `ctrl2080internal.h:4053-4060`) |
| `NVB0CC_CTRL_CMD_INTERNAL_PERMISSIONS_INIT` | 0xB0CC0203 | 7 | 6 | `bAdminProfilingPermitted` removed, every later flag moves up one byte (610 and 615 `ctrlb0cc/ctrlb0ccinternal.h:127`) |

The comparison sees only the top-level struct text; a changed nested type of equal size would
not show. The per-control sizes are in the group tables below and in d-controls.tsv.

Neither change matters for the answers planned here, because both controls are refused and
neither parameter block is read or written: `INTERNAL_CONTROL_GSP_TRACE` is REFUSE (GSP trace
buffers are a tool feature), and `INTERNAL_PERMISSIONS_INIT` is DENY as the backstop of the
profiling refusal (6.16). They still belong in the generated `ver/v615` module (D14), so that a
later model does not read `flag` at the 610 offset. The three controls that are new in 615 need
their ids in `ver/v615` only:

| Control | Id | Bytes (615) | Answer |
|---|---|---:|---|
| `NV0073_CTRL_CMD_DP_SET_WAR_FLAGS` | 0x00731391 | 16 | ACK. The 615 DP library sends it on every plug after the OUI handshake and on every unplug (`src/common/displayport/src/dp_connectorimpl.cpp:8048`, `:8475`), so it arrives with the first 615 guest that runs the DP path |
| `NV0073_CTRL_CMD_SPECIFIC_HDCP_KEY_DECRYPTION` | 0x007302B2 | 8 | REFUSE (no HDCP) |
| `NV2080_CTRL_CMD_INTERNAL_PERF_CLIENT_PERF_MODES_SET_CONTROL` | 0x20800B20 | 32 | REFUSE (workload power profiles, not offered on this board) |

The layout changes of handled controls between the two versions (FIFO device info
table, SYSTEM_GET_ACTIVE, the KGR floorsweeping and SM order getters, the 0x20800A36 id reuse)
are outside D and are listed in [gsp-surface.md](../gsp-surface.md), section 615.71.09.

## 5. The groups at a glance

Estimates are engineer-days for everything the group section proposes, including tests and gate runs,
excluding the event emitter of gsp-surface.md §4 (2 to 3 days) and the category C RESET_CHANNEL work,
which several groups depend on. Rank: 1 = on the CUDA or Wayland/KMS-with-NVENC path now or at the next
milestone; 2 = a plausible stock client (Vulkan games, NVENC, suspend, error paths, nvidia-smi admin);
3 = tools only; 4 = a platform feature absent on this board; 5 = never on this chip.

| # | Group | Controls | Answers | Moves to A / C / E | Estimate (days) | Rank | Decided by | Client |
|---|---|---:|---|---|---|---|---|---|
| 6.1 | FIFO, channels and TSGs | 16 | 8 REFUSE, 4 DENY, 2 MODEL, 2 MIRROR | 0 / 4 / 8 | 3 to 5 | 2 | `FIFO_DISABLE_CHANNELS` (UVM teardown with live channels), `FIFO_DISABLE_USERMODE_CHANNELS` (suspend) | CUDA errors, suspend |
| 6.2 | GR, contexts and ZBC | 18 | 7 REFUSE, 1 ACK, 9 MODEL, 1 MIRROR | 1 / 3 / 5 | 4 to 6 | 2 | `SET_ZBC_*` (clear colours outside the default table), `GPU_SET_COMPUTE_MODE_RULES` | Vulkan games, CUDA admin |
| 6.3 | Memory, caches and page tables | 13 | 5 REFUSE, 3 DENY, 1 ACK, 4 MODEL | 1 / 0 / 5 | 2 to 4 | 2 | `INTERNAL_FBSR_INIT` (suspend), `DMA_FLUSH` | suspend |
| 6.4 | Bus, BAR and P2P | 24 | 10 REFUSE, 12 DENY, 2 MODEL | 0 / 2 / 6 | 1.5 to 3 | 2 | `GPU_VALIDATE_MEM_MAP_REQUEST` (non-root BAR0 mmap) | any (policy) |
| 6.5 | Display | 107 | 74 REFUSE, 11 ACK, 17 MODEL, 5 TABLE | 5 / 2 / 22 | 3.5 to 5.5 | 1 | `SPECIFIC_HDCP_CTRL` (every unplug), `DP_SET_WAR_FLAGS` (615, every plug and unplug), `DP_CONFIGURE_FEC` (DSC) | Wayland/KMS |
| 6.6 | Perf, clocks and power | 21 | 15 REFUSE, 1 DENY, 3 ACK, 2 MODEL | 0 / 0 / 18 | 1 to 2 | 3 | `PERF_RATED_TDP_SET_CONTROL` (clock locking by profilers) | tools |
| 6.7 | Thermal and fan | 2 | 2 REFUSE | 0 / 0 / 2 | 0.1 | 4 | both notebook platform-request controls | nobody |
| 6.8 | ECC, RAS and RC recovery | 15 | 11 REFUSE, 1 DENY, 2 ACK, 1 MODEL | 0 / 2 / 9 | 1.5 to 4 | 2 | `INTERNAL_RC_WATCHDOG_TIMEOUT`, `INTERNAL_LOG_OOB_XID` | CUDA errors, display errors |
| 6.9 | Engines CE/NVENC/NVDEC/OFA | 1 | 1 ACK | 0 / 0 / 0 | 0.25 | 2 | `PERF_NOTIFY_VIDEOEVENT` | NVENC (Sunshine) |
| 6.10 | Events, interrupts and firmware tracing | 7 | 6 REFUSE, 1 MODEL | 0 / 1 / 2 | 2 to 4 | 2 | `GPU_REPORT_NON_REPLAYABLE_FAULT` | CUDA errors |
| 6.11 | vGPU, SR-IOV and MIG internals | 8 | 7 REFUSE, 1 MODEL | 0 / 3 / 5 | 1 to 1.5 | 2 | `INTERNAL_FIFO_TOGGLE_ACTIVE_CHANNEL_SCHEDULING` (suspend); the rest rank 5 | suspend |
| 6.12 | NVLink, C2C and SLI bridge | 3 | 3 REFUSE | 0 / 0 / 3 | 0.1 to 0.25 | 5 | none (two GPUs and a bridge needed) | nobody |
| 6.13 | Security, fuses and protected memory | 4 | 3 REFUSE, 1 DENY | 0 / 0 / 1 | 0.25 to 0.5 | 4 | `FB_SETUP_VPR_REGION` | nobody |
| 6.14 | Timers | 2 | 2 MODEL | 0 / 0 / 0 | 2 to 3 | 2 (low confidence) | `NV9074_CTRL_CMD_RELEASE`; no trace shows the class | unknown, measure first |
| 6.15 | GPIO and I2C | 12 | 12 REFUSE | 0 / 0 / 4 | 0.5 to 1 | 2 | `I2C_READ_REG`/`WRITE_REG`, `GPU_SET_ILLUM` (stock tools) | tools (ddcutil, LED) |
| 6.16 | Profiling and SM debugger | 53 | 36 REFUSE, 15 DENY, 1 ACK, 1 MODEL | 0 / 0 / 11 | 2 to 3 | 3 | `DEBUG_SET_MODE_MMU_DEBUG` (cuda-gdb), profiler allocations (Nsight, CUPTI) | tools |
| 6.17 | Other (GPU accounting) | 2 | 1 REFUSE, 1 ACK | 0 / 0 / 1 | 0.5 to 1 | 2 | `GPUACCT_SET_ACCOUNTING_STATE` (nvidia-smi -am) | CUDA admin |
| | **Total** | **308** | 200 REFUSE, 37 DENY, 21 ACK, 42 MODEL, 3 MIRROR, 5 TABLE | 7 / 17 / 102 | **about 25 to 44** | | | |

The sum overlaps in one place: the RC reflection to the guest is counted in 6.10 (events) and
is the same work item as gr-completion.md item g and the watchdog recovery of 6.8; counted once
the total is about 23 to 41 days. gsp-surface.md §7 estimated 20 to 35 days for D.

## 6. The groups

Each group has its control table (generated from the inventory, the compiled sizes and the
per-control reading; `Bytes 610/615` is `sizeof` of the parameter struct, `-` where the control
does not exist in that version; `Priv`: user = NON_PRIVILEGED, admin = PRIVILEGED, internal = INTERNAL,
kernel = KERNEL_PRIVILEGED), then the analysis. Citations without a tree prefix are in the 615 tree;
`ctrl/...` is `src/common/sdk/nvidia/inc/ctrl/...`. The columns `client`, `gsp`, `risk` and
`reclass` per control are in d-controls.tsv.

### 6.1 FIFO, channels and TSGs (16)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV0080_CTRL_CMD_FIFO_SET_CHANNEL_PROPERTIES` | 0x0080170F | 16/16 | user | REFUSE | 5 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV0080_CTRL_CMD_FIFO_STOP_RUNLIST` | 0x00801711 | 4/4 | admin | DENY | 3 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV0080_CTRL_CMD_FIFO_START_RUNLIST` | 0x00801712 | 4/4 | admin | DENY | 3 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS` | 0x00801714 | 16396/16396 | user | MIRROR | 3 | guest state | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces; kernel body src/nvidia/src/kernel/gpu/fifo/kernel_fifo_ctrl.c:131-186 takes the FIFO lock and forwards at :166-174 |
| `NV2080_CTRL_CMD_SET_GPFIFO` | 0x20801102 | 24/24 | user | REFUSE | 5 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV2080_CTRL_CMD_FIFO_DISABLE_CHANNELS` | 0x2080110B | 536/536 | user | MIRROR | 2 | guest state | src/nvidia/src/kernel/rmapi/nv_gpu_ops.c:1068-1073 nvGpuOpsDisableVaSpaceChannels and :1126-1131 Enable, from nvGpuOpsSetPageDirectory :9087 and nvGpuOpsUnsetPageDirectory :9193 when the VA space still has channels; kernel body src/nvidia/src/kernel/gpu/fifo/kernel_fifo_ctrl.c:744-780 forwards at :763-772 |
| `NV2080_CTRL_CMD_FIFO_CONFIG_CTXSW_TIMEOUT` | 0x20801110 | 8/8 | admin | DENY | 3 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV2080_CTRL_CMD_FIFO_CLEAR_FAULTED_BIT` | 0x20801113 | 12/12 | admin | REFUSE | 5 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces; Turing UVM clears faulted channels with a host method, not this control (kernel-open/nvidia-uvm/uvm_turing.c:69-71, uvm_hal.c:173-175) |
| `NV2080_CTRL_CMD_FIFO_RUNLIST_SET_SCHED_POLICY` | 0x20801115 | 8/8 | user | DENY | 3 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV2080_CTRL_CMD_FIFO_UPDATE_CHANNEL_INFO` | 0x20801116 | 32/32 | admin | REFUSE | 5 | none | src/nvidia/src/kernel/gpu/deferred_api.c:349-375 (deferred-API V2 registers the new USERD with GSP) and :479-480 (deferred SW-method executor); kernel body src/nvidia/src/kernel/gpu/fifo/kernel_fifo_ctrl.c:550-627 forwards at :592-601 |
| `NV2080_CTRL_CMD_FIFO_DISABLE_USERMODE_CHANNELS` | 0x20801117 | 1/1 | kernel | MODEL:channel-registry | 2 | guest state | src/nvidia/arch/nvalloc/unix/src/osapi.c:2867-2895 rm_stop_user_channels and :2897-2924 rm_restart_user_channels, from kernel-open/nvidia/nv.c:4737 nv_preempt_user_channels (suspend) and :4687 nv_restore_user_channels (resume) when vidmem is preserved |
| `NV2080_CTRL_CMD_FIFO_OBJSCHED_SET_STATE` | 0x20801121 | 24/24 | user | REFUSE | 5 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV208F_CTRL_CMD_FIFO_CHECK_ENGINE_CONTEXT` | 0x208F0401 | 12/12 | user | MODEL:channel-registry | 3 | guest state | src/nvidia/src/kernel/gpu/subdevice/subdevice_diag.c:79-91 (the only DiagApi control a vGPU guest may send, besides VAS block details); no sender in the tree |
| `NV5080_CTRL_CMD_DEFERRED_API` | 0x50800101 | 584/584 | user | REFUSE | 5 | none | src/nvidia/src/kernel/gpu/deferred_api.c:281-313 defapiCtrlCmdDeferredApi (routed, ROUTE_TO_PHYSICAL 0x50048) |
| `NV5080_CTRL_CMD_REMOVE_API` | 0x50800102 | 4/4 | user | REFUSE | 5 | none | src/nvidia/src/kernel/gpu/deferred_api.c:413-441 defapiCtrlCmdRemoveApi |
| `NV5080_CTRL_CMD_DEFERRED_API_INTERNAL` | 0x50800104 | 584/584 | internal | REFUSE | 5 | none | src/nvidia/src/kernel/gpu/deferred_api.c:388-394, the kernel forwarding a client DEFERRED_API_V2 |

**Subfamilies.** (1) Channel scheduling on guest channel sets: `FIFO_DISABLE_CHANNELS` 0x2080110B, `FIFO_DISABLE_USERMODE_CHANNELS` 0x20801117, `FIFO_IDLE_CHANNELS` 0x00801714. (2) Host-global scheduler knobs: `STOP_RUNLIST`/`START_RUNLIST` 0x00801711/12, `FIFO_CONFIG_CTXSW_TIMEOUT` 0x20801110, `FIFO_RUNLIST_SET_SCHED_POLICY` 0x20801115, `FIFO_OBJSCHED_SET_STATE` 0x20801121 (vGPU scheduler). (3) Channel rewiring and verification: `SET_CHANNEL_PROPERTIES` 0x0080170F (VERIF ONLY), `SET_GPFIFO` 0x20801102, `FIFO_UPDATE_CHANNEL_INFO` 0x20801116, `FIFO_CLEAR_FAULTED_BIT` 0x20801113. (4) Deferred API (class NV50_DEFERRED_API 0x5080): `DEFERRED_API` 0x50800101, `REMOVE_API` 0x50800102, `DEFERRED_API_INTERNAL` 0x50800104. (5) Diag: `NV208F FIFO_CHECK_ENGINE_CONTEXT` 0x208F0401.

**State a device model needs.** Per guest channel (existing `channel.rs` Registry, keyed by (hClient, hObject), ChID, TSG, VA space): a `disabled` flag and a `usermode` flag (kernel-internal channels vs client channels); per device a `scheduling_disabled` flag for suspend. Invariant: a disabled channel's doorbell is latched but not executed (software executor) or not rung on the host (mirror). Lifetime: per channel object, dropped at free; the device flag is cleared by the resume call and does not survive a guest driver reload (the reload re-inits FIFO). The host-global knobs need no state: they are denied.

**What GSP-RM does.** All closed firmware. `DISABLE_CHANNELS` disables scheduling of the listed channels, by default also preempting them, optionally rewinding GP_PUT to GP_GET (`ctrl2080fifo.h:198-233`); the kernel only checks that `pRunlistPreemptEvent` comes from a kernel client and forwards (`gpu/fifo/kernel_fifo_ctrl.c:744-780`, forward at :763-772). `DISABLE_USERMODE_CHANNELS` does the same for every usermode channel of the GPU (`ctrl2080fifo.h:729-739`). `IDLE_CHANNELS` deschedules and waits for the listed channels (`ctrl0080fifo.h:367-390`; kernel takes the FIFO lock and forwards, `kernel_fifo_ctrl.c:131-186`). STOP/START_RUNLIST act on a whole engine runlist (`ctrl0080fifo.h:292-329`), SET_SCHED_POLICY on all runlists with client-refcounted locking, privileged only (`ctrl2080fifo.h:599-672`), CONFIG_CTXSW_TIMEOUT on the engine watchdog (`:466-478`). UPDATE_CHANNEL_INFO moves a channel to a new USERD/GPFIFO (`:693-714`; kernel `kernel_fifo_ctrl.c:550-627` forwards at :592-601, then rebuilds the USERD memdesc). The deferred API stores a control to run when a SW method fires on the channel (`ctrl5080.h:64-85`); the executor is `gpu/deferred_api.c:444-560`, and V2 forwards as DEFERRED_API_INTERNAL (:388-394).

**Who calls it and when.** `DISABLE_CHANNELS`: UVM, `rmapi/nv_gpu_ops.c:1068-1073` (disable) and :1126-1131 (enable) from `nvGpuOpsSetPageDirectory` :9087 and `nvGpuOpsUnsetPageDirectory` :9193 when the VA space still has channels. It never appeared in a device run: the CUDA gates free channels before UVM unregisters the VA space, so it is the path of a CUDA process that dies with live channels. `DISABLE_USERMODE_CHANNELS`: `src/nvidia/arch/nvalloc/unix/src/osapi.c:2867-2924`, from `kernel-open/nvidia/nv.c:4737` (suspend) and :4687 (resume) when video memory is preserved. `CHECK_ENGINE_CONTEXT` is the only 208F control a vGPU guest may send (`gpu/subdevice/subdevice_diag.c:79-91`), no sender in the tree. UPDATE_CHANNEL_INFO: deferred API only (`deferred_api.c:349-375`, :479-480). The class 0x5080 is allocated in none of the 79 native traces. All others: no caller in kernel-open, nvidia-modeset, src/common or UVM; user space unknown (closed), none in the traces. Turing UVM clears faulted channels with a host method (`kernel-open/nvidia-uvm/uvm_turing.c:69-71`), not with CLEAR_FAULTED_BIT.

**Truth source.** Guest state for (1) and CHECK_ENGINE_CONTEXT; none for the rest (denied or refused).

**Isolation risk.** The FIFO is the most dangerous group to forward blindly. `DISABLE_USERMODE_CHANNELS` or `TOGGLE_ACTIVE_CHANNEL_SCHEDULING` forwarded as-is to the device's host subdevice would stop every usermode channel on the physical GPU, the host compositor included. `DISABLE_CHANNELS` carries (hClient, hChannel) pairs in the guest's handle space; passed through, a collision with a host handle of the device's own client could disable a channel the guest does not own. STOP_RUNLIST, SCHED_POLICY and CTXSW_TIMEOUT change runlists and watchdogs the mirror channels share with the desktop (`mirror.rs:1-30`: mirror channels are host channels on the host runlists).

**Design proposal.** In `channel.rs`: `Registry::set_disabled(chids, bool)` and a device-level `scheduling_disabled`; the doorbell paths (`ctrl.rs:183-232`) check both. In `mirror.rs`: `disable_channels(chids, disable, preempt)` that issues DISABLE_CHANNELS on the *device's own* host client with the mirror handles, and `preempt_tsg` (exists, :490-494) for IDLE_CHANNELS. A new `fifo.rs` holds the dispatch: rule "guest channel set" (0x2080110B, 0x20801117, 0x00801714, 0x208F0401) resolves every handle through the registry and rejects unknown ones with NV_ERR_INVALID_CHANNEL; rule "host-global" (0x00801711/12, 0x20801110, 0x20801115) answers NV_ERR_INSUFFICIENT_PERMISSIONS; rule "absent" answers NOT_SUPPORTED for the E set. Tests: unit tests with the generated ABI (`ver::v615` offsets of `NV2080_CTRL_FIFO_DISABLE_CHANNELS_PARAMS`, 536 bytes) that a foreign handle is rejected and that no host control is issued for a software channel; gate run: kill -9 of a running cuda-core in the guest, then a new CUDA process must run.

**Estimate.** 3 to 5 engineer-days: registry flags and doorbell gating (1), mirror disable/preempt (1), suspend pair with vgpu's TOGGLE (0.5), deny/refuse table (0.5), tests and the kill gate (1 to 2).

**Client likelihood.** Group rank 2. Decided by `DISABLE_CHANNELS` (UVM teardown with live channels, the error path of every CUDA crash) and `DISABLE_USERMODE_CHANNELS` (suspend with preserved vidmem).

**Roadmap.** Linux CUDA client (crash and kill robustness); suspend/resume after that. Depends on the mirror (exists) and on the RC path of the events group for the kill case.

**Reclassification.** C (4): 0x00801714, 0x2080110B, 0x20801117, 0x208F0401 (all resolve through the channel registry). E (8): 0x0080170F (VERIF ONLY), 0x20801102 (legacy, no caller), 0x20801113 (Ampere SW-method/register path; Turing uses the method), 0x20801116 and 0x50800101/02/04 (deferred API, no Linux client), 0x20801121 (vGPU scheduler). Stay D (4): the host-global knobs, answered DENY.

**Measurements.** (a) Guest: start cuda-core, kill -9 it mid-kernel, read the device trace for 0x2080110B and its channel list (no host access needed; software mode first). (b) Guest: `echo mem > /sys/power/state` with `NVreg_PreserveVideoMemoryAllocations=1`, pure emulation: records the order of 0x20801117, 0x20800AC3, 0x20800AC2.

### 6.2 GR, contexts and ZBC (18)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV0080_CTRL_CMD_INTERNAL_MEMSYS_SET_ZBC_REFERENCED` | 0x0080200A | 1/1 | user | ACK | 5 | guest state | src/nvidia/src/kernel/gpu/mem_mgr/arch/maxwell/mem_mgr_gm107.c:556-557 (first ZBC-kind surface of a device) and :623-624 (last freed), via memmgrSetZbcReferenced :266-320; kernel body src/nvidia/src/kernel/gpu/mem_sys/kern_mem_sys_ctrl.c:1303-1334 forwards at :1318-1331 |
| `NV2080_CTRL_CMD_GPU_INITIALIZE_CTX` | 0x2080012D | 56/56 | admin | REFUSE | 5 | none | src/nvidia/src/kernel/gpu/deferred_api.c:467-468, the deferred-API SW-method executor only |
| `NV2080_CTRL_CMD_GPU_SET_COMPUTE_MODE_RULES` | 0x20800130 | 8/8 | admin | MODEL:compute-mode | 2 | guest state | src/nvidia/src/kernel/gpu/subdevice/subdevice_ctrl_gpu_kernel.c:2047-2129: forwarded at :2059-2075, then the kernel stores the rule itself (:2097-2121) and enforces it at compute reservation (:2200); user caller nvidia-smi -c (closed) |
| `NV2080_CTRL_CMD_INTERNAL_GR_SET_FECS_TRACE_HW_ENABLE` | 0x20800A37 | 24/24 | internal | MODEL:fecs-trace | 3 | pure emulation | src/nvidia/src/kernel/gpu/gr/fecs_event_list.c:1580-1588 fecsBufferReset (enable) and :1633-1638 fecsBufferDisableHw; reset only runs when the FECS buffer is mapped (:1515-1516) and a ctxsw-log consumer exists (src/nvidia/src/kernel/gpu/gr/kernel_graphics.c:362-366, :1837-1838) |
| `NV2080_CTRL_CMD_INTERNAL_GR_SET_FECS_TRACE_RD_OFFSET` | 0x20800A39 | 24/24 | internal | MODEL:fecs-trace | 3 | pure emulation | src/nvidia/src/kernel/gpu/gr/fecs_event_list.c:1567-1576 fecsBufferReset |
| `NV2080_CTRL_CMD_INTERNAL_GR_SET_FECS_TRACE_WR_OFFSET` | 0x20800A3A | 24/24 | internal | MODEL:fecs-trace | 3 | pure emulation | src/nvidia/src/kernel/gpu/gr/fecs_event_list.c:1556-1565 fecsBufferReset |
| `subdeviceCtrlCmdGrStaticGetFecsTraceDefines` | 0x20800A3E | 256/256 | kernel | REFUSE | 5 | chip table | no caller: the kernel sends the K twin 0x20800A3F (src/nvidia/src/kernel/gpu/gr/kernel_graphics.c:1486-1505), which kgr.rs:452-458 already refuses, NOT_SUPPORTED being tolerated (:1503-1505) |
| `NV2080_CTRL_CMD_GR_CTXSW_ZCULL_MODE` | 0x20801205 | 16/16 | user | MODEL:channel-registry | 2 | guest state | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces; the bind twin GR_CTXSW_ZCULL_BIND is in every GL/EGL/Vulkan trace and recorded by ctrl.rs:1155-1169 |
| `NV2080_CTRL_CMD_GR_SET_GPC_TILE_MAP` | 0x2080120A | 152/152 | user | REFUSE | 3 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND` | 0x20801211 | 112/112 | user | MIRROR | 2 | guest state | src/nvidia/src/kernel/gpu/deferred_api.c:540-541 (deferred-API executor); direct callers are user space (closed), none in the native traces |
| `NV2080_CTRL_CMD_GR_GFX_POOL_INITIALIZE` | 0x20801220 | 16/16 | kernel | REFUSE | 5 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV2080_CTRL_CMD_GR_GFX_POOL_ADD_SLOTS` | 0x20801221 | 272/272 | kernel | REFUSE | 5 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV2080_CTRL_CMD_GR_GFX_POOL_REMOVE_SLOTS` | 0x20801222 | 276/276 | kernel | REFUSE | 5 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV2080_CTRL_CMD_GR_CTXSW_SETUP_BIND` | 0x2080123A | 16/16 | user | REFUSE | 3 | host RO | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces; GR_CTX_BUFFER_SETUP is a GfxP buffer kind (src/nvidia/src/kernel/gpu/gr/kernel_graphics.c:160) |
| `NV9096_CTRL_CMD_SET_ZBC_COLOR_CLEAR` | 0x90960101 | 40/40 | user | MODEL:zbc-table | 2 | guest state | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces (the same traces call GET_ZBC_CLEAR_TABLE_SIZE/ENTRY 0x90960106/7 72 and 1080 times on GF100_ZBC_CLEAR) |
| `NV9096_CTRL_CMD_SET_ZBC_DEPTH_CLEAR` | 0x90960102 | 12/12 | user | MODEL:zbc-table | 2 | guest state | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV9096_CTRL_CMD_SET_ZBC_CLEAR_TABLE` | 0x90960104 | 60/60 | user | MODEL:zbc-table | 3 | guest state | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV9096_CTRL_CMD_SET_ZBC_STENCIL_CLEAR` | 0x90960105 | 12/12 | user | MODEL:zbc-table | 2 | guest state | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |

**Subfamilies.** (1) ZBC table: `NV9096 SET_ZBC_COLOR_CLEAR`/`DEPTH`/`CLEAR_TABLE`/`STENCIL` 0x90960101/02/04/05 and `NV0080 INTERNAL_MEMSYS_SET_ZBC_REFERENCED` 0x0080200A. (2) Per-channel context binds: `GR_CTXSW_ZCULL_MODE` 0x20801205, `GR_CTXSW_PREEMPTION_BIND` 0x20801211, `GR_CTXSW_SETUP_BIND` 0x2080123A, `GPU_INITIALIZE_CTX` 0x2080012D. (3) FECS context-switch trace: `INTERNAL_GR_SET_FECS_TRACE_HW_ENABLE`/`RD_OFFSET`/`WR_OFFSET` 0x20800A37/39/3A, `STATIC_GR_GET_FECS_TRACE_DEFINES` 0x20800A3E. (4) Policy: `GPU_SET_COMPUTE_MODE_RULES` 0x20800130. (5) Global GR configuration: `GR_SET_GPC_TILE_MAP` 0x2080120A. (6) WDDM GfxP pool: `GR_GFX_POOL_INITIALIZE`/`ADD_SLOTS`/`REMOVE_SLOTS` 0x20801220/21/22.

**State a device model needs.** ZBC: per device a table of color (colorFB[4], colorDS[4], format), depth (value, format) and stencil (value, format) entries with the index range the size getter reports; seeded from the host's default entries, guest additions appended; survives while the device runs (the real table persists across clients until unreferenced). Compute mode: one u32 per device. FECS: per GR engine an enable flag and rd/wr offsets. Context binds: per guest channel the zcull mode and the preemption buffer VAs and modes (the registry already records `ZCULL_BIND`, `ctrl.rs:1152-1169`).

**What GSP-RM does.** Closed firmware throughout. SET_ZBC_* add an entry to the L2 table and, before GA10x, the DS table (`ctrl9096.h:84-112`, :144-162, :322-338; deprecated indexed form :250-294), failing with NVOS_STATUS_INSUFFICIENT_RESOURCES when full. SET_ZBC_REFERENCED tells physical RM whether ZBC-kind surfaces exist so it may flush the table (`ctrl0080internal.h:118-126`). ZCULL_MODE selects GLOBAL/NO_CTXSW/SEPARATE_BUFFER (`ctrl2080gr.h:417-452`); PREEMPTION_BIND sets GfxP/CILP modes and binds preemption buffers at client VAs (`:874-916`); SETUP_BIND binds the Setup buffer, NOT_SUPPORTED where the channel has none (`:664-681`). INITIALIZE_CTX is the legacy virtual-context init (`ctrl2080gpu.h:1011-1046`). SET_COMPUTE_MODE_RULES records the rule that compute object allocation enforces (`ctrl2080gpu.h:1217-1276`); the kernel forwards and then stores and enforces it itself (`gpu/subdevice/subdevice_ctrl_gpu_kernel.c:2059-2121`, check at :2200). FECS trace controls program the FECS logging (`ctrl2080internal.h:587-637`). GPC_TILE_MAP loads the GR tile map (`ctrl2080gr.h:693-706`). GFX_POOL_* manage a GfxP pool for kernel callers (`:1310-1409`).

**Who calls it and when.** SET_ZBC_REFERENCED: `gpu/mem_mgr/arch/maxwell/mem_mgr_gm107.c:556-557` on the first ZBC-kind allocation of a device and :623-624 on the last free. On Turing a ZBC kind is a compressible kind (`gpu/mem_mgr/arch/turing/mem_mgr_tu102_base.c:52-53`), so under D16 (no compression offered, memory-model.md section 6) the stock guest never sends it. FECS SETs: `gpu/gr/fecs_event_list.c:1556-1588` (reset) and :1633-1638 (disable), only when the FECS buffer is mapped (:1515-1516) and a ctxsw-log consumer is bound (`gpu/gr/kernel_graphics.c:362-366`, :1837-1838); the device reports FECS_EVENT size 0 (`kgr.rs:499-510`), so nothing is mapped. The kernel reads trace defines through 0x20800A3F (`kernel_graphics.c:1486-1505`, NOT_SUPPORTED tolerated), which `kgr.rs:452-458` refuses; 0x20800A3E is its non-K export alias (`src/nvidia/generated/g_subdevice_nvoc.c:6407-6420`) without a caller. PREEMPTION_BIND and INITIALIZE_CTX are in the deferred-API executor (`gpu/deferred_api.c:540-541`, :467-468). ZBC setters, ZCULL_MODE, PREEMPTION_BIND, SETUP_BIND, GPC_TILE_MAP: user space (closed); the native GL, EGL and Vulkan traces allocate GF100_ZBC_CLEAR 10 times and call GET_ZBC_CLEAR_TABLE_SIZE/ENTRY 72/1080 times but never a setter. SET_COMPUTE_MODE_RULES: nvidia-smi -c (closed). GFX_POOL_*: kernel-privileged, no Linux kernel caller.

**Truth source.** ZBC defaults: host RO (GET_ZBC_CLEAR_TABLE_SIZE/ENTRY on a device-owned GF100_ZBC_CLEAR object), additions guest state. Compute mode, binds: guest state. FECS: pure emulation. Trace defines: chip table.

**Isolation risk.** The host ZBC table is global and shared with the desktop's UMD; forwarding SET_ZBC_* would consume host slots (the desktop then misses fast clears) and let the guest influence clear values of host surfaces that reference those slots. The guest gains nothing, since ZBC needs compressible kinds that D16 never lets reach the host. SET_COMPUTE_MODE_RULES forwarded to the host subdevice would set the host GPU's compute mode (PROHIBITED would break host CUDA). FECS enable on the host would log context switches of host processes. GPC_TILE_MAP rewrites global GR configuration. PREEMPTION_BIND on a mirror takes guest VAs in the guest VA space the mirror shares: safe once the D17 filter validates the mapping.

**Design proposal.** New `zbc.rs`: `ZbcTable { color, depth, stencil, index_size }`, seeded at the first GF100_ZBC_CLEAR allocation from one host RO sweep (label `host (read once)`), setters append locally (`guest (recorded; never on the host)`), and the GET_ZBC_CLEAR_TABLE_SIZE/ENTRY rows (category A/B in gsp-surface.md section 6 item 4) are answered from the same table so a guest reads back what it set. SET_ZBC_REFERENCED: ACK plus a D16 alarm note (its arrival means a compressible kind was allocated). Compute mode: a field on the dispatcher, NV_OK for the four legal rules, NV_ERR_INVALID_ARGUMENT otherwise. FECS: three fields in `kgr.rs` next to the GET_FECS_TRACE_HW_ENABLE arm (`kgr.rs:477-489`) so GET reflects SET. ZCULL_MODE: record like ZCULL_BIND. PREEMPTION_BIND: record for software channels, add to `MIRROR_FORWARDED` (`ctrl.rs:130-136`) for mirrored ones after the VA check. 0x20800A3E: same arm as 0x20800A3F. SETUP_BIND, GPC_TILE_MAP, INITIALIZE_CTX, GFX_POOL_*: NOT_SUPPORTED. Tests: unit tests of the ZBC table (seed, add, full, readback through the 0x90960107 layout from the generated ABI), compute-mode round trip with QUERY_COMPUTE_MODE_RULES; gate: vk-offscreen and gl-render on the device with the ZBC trace, plus nvidia-smi -c 3 followed by two CUDA processes.

**Estimate.** 4 to 6 engineer-days: ZBC model with host seed and GET rows (2 to 3), compute mode (0.5), binds record and mirror (1), FECS fields (0.5), tests and gates (1).

**Client likelihood.** Group rank 2. Decided by the ZBC setters (a Vulkan or GL game with a clear color outside the default table) and `GPU_SET_COMPUTE_MODE_RULES` (nvidia-smi admin).

**Roadmap.** Vulkan games (ZBC, PREEMPTION_BIND); Linux CUDA client (compute mode, admin only); Wayland/KMS indirectly through the GL/EGL compositor's clears. Depends on the mirror (exists), the D17 filter for PREEMPTION_BIND, and category A for the GET_ZBC rows.

**Reclassification.** A (1): 0x20800A3E (static chip data, alias of 0x20800A3F). C (3): 0x20801205, 0x20801211, 0x2080123A (per guest channel). E (5): 0x0080200A (only for compressible kinds, which D16 excludes on Turing), 0x2080012D (legacy, deferred API only), 0x20801220/21/22 (kernel-only, no Linux kernel caller). Stay D (9): the ZBC setters, compute mode, the three FECS setters, GPC_TILE_MAP.

**Measurements.** (a) Host RO, one sweep: GET_ZBC_CLEAR_TABLE_SIZE for color, depth and stencil and GET_ZBC_CLEAR_TABLE_ENTRY for every index on a GF100_ZBC_CLEAR object of the device's own client; gives index_size and the default entries. (b) Existing data: the init log of `kgr.rs` `context_buffers_info` (host GR_GET_ENGINE_CONTEXT_PROPERTIES for ids 0..0x19) tells whether TU106 has a Setup buffer and the GfxP buffer sizes; decides SETUP_BIND. (c) Guest: vk-rt and a game with nonstandard clear colors on the device, count 0x9096010x in the device trace.

### 6.3 Memory, caches and page tables (13)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV0080_CTRL_CMD_DMA_FLUSH` | 0x00801805 | 4/4 | user | MODEL:l2-flush | 3 | host RO | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces; the kernel body src/nvidia/src/kernel/gpu/mem_mgr/dma.c:663-726 (610 :659) runs only on a vGPU guest (flag PHYSICAL_IMPLEMENTED_ON_VGPU_GUEST 0x40000), a GSP client routes it |
| `NV0080_CTRL_DMA_SET_DEFAULT_VASPACE` | 0x00801812 | 4/4 | kernel | REFUSE | 5 | guest state | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces; kernel body src/nvidia/src/kernel/gpu/mem_mgr/dma.c:827-858 (610 :823) sets the default VAS then RPCs at :843-855 |
| `NV2080_CTRL_CMD_INTERNAL_MEMSYS_FLUSH_L2_ALL_RAMS_AND_CACHES` | 0x20800A6D | 0/0 | internal | MODEL:l2-flush | 3 | host RO | src/nvidia/src/kernel/gpu/mem_sys/kern_mem_sys.c:1210-1221 kmemsysSendFlushL2AllRamsAndCaches, from kmemsysCacheOp_GM200 FB_CACHE_DIRTY_ALL + EVICT (src/nvidia/src/kernel/gpu/mem_sys/arch/maxwell/kern_mem_sys_gm200.c:175-180); no caller names FB_CACHE_DIRTY_ALL (only the enum, src/nvidia/generated/g_kern_mem_sys_nvoc.h:80) |
| `NV2080_CTRL_CMD_INTERNAL_MEMSYS_PROGRAM_RAW_COMPRESSION_MODE` | 0x20800A6F | 1/1 | internal | REFUSE | 5 | none | src/nvidia/src/kernel/gpu/mem_mgr/arch/ampere/fbsr_ga100.c:29-45; TU106 gets the stub fbsrSendMemsysProgramRawCompressionMode_ac1694 (src/nvidia/generated/g_fbsr_nvoc.c:202-211) |
| `NV2080_CTRL_CMD_INTERNAL_MEMSYS_CLEAN_LTC_PROBE_FILTER` | 0x20800AB0 | 0/0 | internal | REFUSE | 5 | none | src/nvidia/src/kernel/gpu/mem_sys/kern_mem_sys.c:1224-1237 kmemsysCleanLTCProbeFilter |
| `NV2080_CTRL_CMD_INTERNAL_FBSR_INIT` | 0x20800AC2 | 24/24 | internal | MODEL:suspend | 2 | pure emulation | src/nvidia/src/kernel/gpu/mem_mgr/arch/maxwell/fbsr_gm107.c:104-129 _fbsrInitGsp, called at :511-514 when a save begins on a GSP client |
| `NV2080_CTRL_CMD_INTERNAL_MEMMGR_MEMORY_TRANSFER_WITH_GSP` | 0x20800AFA | 96/96 | internal | REFUSE | 5 | none | src/nvidia/src/kernel/gpu/mem_mgr/mem_utils.c:310-317 and :472-479 (memcpy/memset/read/write with GSP), chosen when BAR access is blocked (:120-124) |
| `NV2080_CTRL_CMD_GSP_FB_STATS_GET` | 0x2080132A | 4848/4848 | internal | MODEL:fb-stats | 3 | pure emulation | src/nvidia/src/kernel/gpu/mem_mgr/mem_mgr_ctrl.c:878-893 (kernel FB_STATS_GET 0x2080132f asks GSP for its part); user caller of FB_STATS_GET unknown, none in the traces |
| `NV2080_CTRL_CMD_FB_CBC_OP` | 0x20801337 | 4/4 | admin | ACK | 3 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV2080_CTRL_CMD_FB_SET_RRD` | 0x2080133E | 4/4 | admin | DENY | 3 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV2080_CTRL_CMD_FB_SET_READ_LIMIT` | 0x2080133F | 1/1 | admin | DENY | 3 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV2080_CTRL_CMD_FB_SET_WRITE_LIMIT` | 0x20801340 | 1/1 | admin | DENY | 3 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV208F_CTRL_CMD_FB_CTRL_GPU_CACHE` | 0x208F0506 | 28/28 | user | REFUSE | 5 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |

**Subfamilies.** (1) Cache operations: `NV0080 DMA_FLUSH` 0x00801805, `INTERNAL_MEMSYS_FLUSH_L2_ALL_RAMS_AND_CACHES` 0x20800A6D, `FB_CBC_OP` 0x20801337, `INTERNAL_MEMSYS_CLEAN_LTC_PROBE_FILTER` 0x20800AB0, `NV208F FB_CTRL_GPU_CACHE` 0x208F0506. (2) Compression: `INTERNAL_MEMSYS_PROGRAM_RAW_COMPRESSION_MODE` 0x20800A6F (and CBC_OP above). (3) Suspend and GSP memory service: `INTERNAL_FBSR_INIT` 0x20800AC2, `INTERNAL_MEMMGR_MEMORY_TRANSFER_WITH_GSP` 0x20800AFA. (4) FB accounting: `GSP_FB_STATS_GET` 0x2080132A. (5) DRAM tuning: `FB_SET_RRD` 0x2080133E, `FB_SET_READ_LIMIT`/`WRITE_LIMIT` 0x2080133F/40. (6) VA spaces: `NV0080 DMA_SET_DEFAULT_VASPACE` 0x00801812.

**State a device model needs.** Almost none. The flush path is stateless (it already exists, `ctrl.rs:674-680` `host_flush_gpu_cache`). FBSR: the sysmem memlist handle, `bEnteringGcoffState` and the suspend metadata address, per device, cleared at resume. FB stats: derived from the device's own FB region table (fb.rs), static per profile.

**What GSP-RM does.** Closed firmware; the kernel twins show the intent. DMA_FLUSH maps targetUnit bits to an L2 writeback (dirty), sysmem or peer L2 invalidate, comptag clean and a sysmembar (`gpu/mem_mgr/dma.c:680-723`, header `ctrl0080dma.h:248-274`); the body runs only on a vGPU guest, a GSP client routes it (flag PHYSICAL_IMPLEMENTED_ON_VGPU_GUEST in 0x50048). FLUSH_L2_ALL_RAMS is the ELPG flush of all L2 RAMs (`ctrl2080internal.h:2156-2161`), reached by `kmemsysCacheOp_GM200` with FB_CACHE_DIRTY_ALL/EVICT (`gpu/mem_sys/arch/maxwell/kern_mem_sys_gm200.c:175-180`). CBC_OP cleans or invalidates the comptag cache (`ctrl2080fb.h:1499-1511`). PROGRAM_RAW_COMPRESSION_MODE switches raw/legacy compression around FB save (`ctrl2080internal.h:2372-2376`); only the GA100 HAL sends it (`gpu/mem_mgr/arch/ampere/fbsr_ga100.c:29-45`), TU106 gets a stub (`src/nvidia/generated/g_fbsr_nvoc.c:202-211`). FBSR_INIT makes GSP-RM save its own FB regions to the given sysmem at suspend (`ctrl2080internal.h:2906-2923`). MEMORY_TRANSFER_WITH_GSP copies or sets memory for CPU-RM when BAR access is blocked (`ctrl2080internal.h:3820-3830`; chosen at `gpu/mem_mgr/mem_utils.c:120-124`). GSP_FB_STATS_GET returns reserved sizes, heap free and a per-owner table (kernel twin `gpu/mem_mgr/mem_mgr_ctrl.c:920-957`). SET_RRD/READ_LIMIT/WRITE_LIMIT program DRAM timing and the FBPA arbiter (`ctrl2080fb.h:1775-1839`), next to PATCH_PBR_FOR_MINING. FB_CTRL_GPU_CACHE exists only with NV_VERIF_FEATURES (`ctrl208ffb.h:91-138`).

**Who calls it and when.** FBSR_INIT: `gpu/mem_mgr/arch/maxwell/fbsr_gm107.c:104-129`, called at :511-514 when a save begins on a GSP client (suspend). FLUSH_L2_ALL_RAMS: `gpu/mem_sys/kern_mem_sys.c:1210-1221`, but no caller names FB_CACHE_DIRTY_ALL (only the enum, `src/nvidia/generated/g_kern_mem_sys_nvoc.h:80`). CLEAN_LTC_PROBE_FILTER: `kern_mem_sys.c:1224-1237`, coherent C2C systems. MEMORY_TRANSFER_WITH_GSP: `mem_utils.c:310-317`, :472-479. GSP_FB_STATS_GET: `mem_mgr_ctrl.c:878-893` inside the kernel's FB_STATS_GET 0x2080132f, whose own caller is unknown (not in the traces). DMA_FLUSH, CBC_OP, SET_RRD and the limits: no caller in the tree, none in the traces. SET_DEFAULT_VASPACE: kernel-privileged (0x1c000), body `dma.c:827-858`, no kernel client calls it.

**Truth source.** Host RO for the flushes (the physical L2 is shared and real); pure emulation for FBSR and FB stats; none for the rest.

**Isolation risk.** D16 is the centre. CBC_OP INVALIDATE forwarded would drop the comptag cache of the desktop's compressed surfaces; PROGRAM_RAW_COMPRESSION_MODE would switch the host's compression mode; both must never reach the host, and CBC_OP answers NV_OK because the device reports no compression, so there is nothing to clean. SET_RRD and the limits change DRAM timing of the physical card under the desktop. MEMORY_TRANSFER_WITH_GSP takes raw physical addresses: forwarded it would be a DMA engine on host memory. FB stats from the host would leak the host GSP heap usage and allocation owners. Whole-L2 flushes cost the desktop bandwidth only.

**Design proposal.** In `ctrl.rs`: DMA_FLUSH and FLUSH_L2_ALL_RAMS route to `host_flush_gpu_cache` (writeback, invalidate, evict per bit; COMPTAG bit ignored; FB bit like BUS_FLUSH_WITH_SYSMEMBAR at `ctrl.rs:1050`), label `host (flush on the device subdevice)`. CBC_OP: ACK with label `synthesized (no compression, D16; never forwarded)`. A small `suspend.rs` (or arms in `ctrl.rs`) records FBSR_INIT and pairs with the fifo/vgpu suspend controls. FB stats: synthesized from the fb.rs region table. DENY rule for 0x2080133E/3F/40 (NV_ERR_INSUFFICIENT_PERMISSIONS), refusal rule for the E set, with PROGRAM_RAW_COMPRESSION_MODE and SET_ZBC_REFERENCED also raising a D16 alarm. Tests: unit test that no host call is made for CBC_OP, the compression and DRAM rows (a HostRm fake that fails on any call); the flush mapping test extends the existing one at `ctrl.rs:1399-1431`; gate: the suspend run of the fifo group.

**Estimate.** 2 to 4 engineer-days: flush mapping (0.5), CBC/deny/refuse rules with the no-host-call test (0.5), FBSR record (0.5, the suspend gate belongs to fifo), FB stats (0.5), slack for the suspend gate findings (0 to 2).

**Client likelihood.** Group rank 2. Decided by `INTERNAL_FBSR_INIT` (every suspend of a guest with GSP) and `DMA_FLUSH` (a plausible UMD flush, unobserved).

**Roadmap.** Suspend/resume of the guest (none of the three milestones needs it); the flushes serve any client. Depends on fb.rs (FB layout) and the existing host flush.

**Reclassification.** A (1): 0x2080132A (device FB layout, static). E (5): 0x20800A6F (Ampere+ FBSR; TU106 stub), 0x20800AB0 (C2C probe filter), 0x20800AFA (BAR access blocked only under Confidential Computing), 0x208F0506 (verification builds), 0x00801812 (kernel-privileged, no kernel caller). Stay D (7): DMA_FLUSH, FLUSH_L2_ALL_RAMS, CBC_OP, FBSR_INIT and the three DRAM setters (DENY).

**Measurements.** (a) Guest suspend with preserved vidmem in pure emulation: does 0x20800AC2 arrive and what memlist size; does the resume path need anything back from GSP. (b) None on the host: every host-facing row here is a write the device must not issue, or the existing flush.

### 6.4 Bus, BAR and P2P (24)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV0080_CTRL_CMD_BIF_ASPM_CYA_UPDATE` | 0x00800105 | 2/2 | kernel | DENY | 3 | pure emulation | no caller in the tree |
| `NV2080_CTRL_CMD_GPU_VALIDATE_MEM_MAP_REQUEST` | 0x20800198 | 24/24 | kernel | MODEL:bar0-map-policy (new) | 2 | guest state | 615 src/nvidia/arch/nvalloc/unix/src/osapi.c:2273 (RmValidateMmapRequest, from rm_create_mmap_context at :2588) on every non-admin mmap of a BAR0 range; the kernel body subdevice_ctrl_gpu_kernel.c:2918-2995 answers PTIMER (RO), usermode (RW) and PMC (RO) itself and forwards only other BAR0 ranges (:2974-2982); 610 :2915 and osapi.c:2199/:2536 |
| `NV2080_CTRL_CMD_INTERNAL_BUS_SETUP_P2P_MAILBOX_LOCAL` | 0x20800A71 | 48/48 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/gpu/bus/arch/maxwell/kern_bus_gm200.c:124 (kbusSetupMailboxes_GM200, from the PCIe mailbox P2P mapping at :565), only when a PCIe P2P mapping between two GPUs (or a loopback) is created |
| `NV2080_CTRL_CMD_INTERNAL_BUS_SETUP_P2P_MAILBOX_REMOTE` | 0x20800A72 | 40/40 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/gpu/bus/arch/maxwell/kern_bus_gm200.c:140, same path as the local half |
| `NV2080_CTRL_CMD_INTERNAL_BUS_DESTROY_P2P_MAILBOX` | 0x20800A73 | 8/8 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/gpu/bus/kern_bus.c:649 (kbusDestroyMailbox, from kern_bus_gm200.c:724), teardown of a PCIe mailbox P2P mapping |
| `NV2080_CTRL_CMD_INTERNAL_SET_P2P_CAPS` | 0x20800AB5 | 1284/1284 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/gpu.c:3432 (_gpuPropagateP2PCapsToAllGpus), called at :3627 only under IS_VGPU_GSP_PLUGIN_OFFLOAD_ENABLED && IS_GSP_CLIENT (vGPU host) |
| `NV2080_CTRL_CMD_INTERNAL_REMOVE_P2P_CAPS` | 0x20800AB6 | 132/132 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/gpu.c:3320 (_gpuRemoveP2pCapsFromPeerGpus, called at :4091 under IS_VGPU_GSP_PLUGIN_OFFLOAD_ENABLED) and :3480 (error unwind of the propagate) |
| `NV2080_CTRL_CMD_INTERNAL_BIF_SET_PCIE_RO` | 0x20800AB9 | 1/1 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/bif/kernel_bif.c:615 (_kbifSetPcieRelaxedOrdering, from kbifStatePostLoad :650) only when PDB_PROP_KBIF_PCIE_RELAXED_ORDERING_SET_IN_EMULATED_CONFIG_SPACE is set; the setter is GA10x only (generated/g_kernel_bif_nvoc.c:822-836), a stub on TU106 |
| `NV2080_CTRL_CMD_INTERNAL_BIF_DISABLE_SYSTEM_MEMORY_ACCESS` | 0x20800ADB | 1/1 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/bif/arch/maxwell/kernel_bif_gm107.c:1046, guarded by NV_ASSERT_OR_RETURN(RMCFG_FEATURE_PLATFORM_WINDOWS) at :1043 |
| `NV2080_CTRL_CMD_BUS_SET_PCIE_LINK_WIDTH` | 0x20801804 | 8/8 | admin | DENY | 3 | pure emulation | no caller in the tree |
| `NV2080_CTRL_CMD_BUS_SET_PCIE_SPEED` | 0x20801805 | 4/4 | admin | DENY | 2 | pure emulation | no kernel caller; listed in rmapiutilSkipErrorMessageForUnsupportedVgpuGuestControl (615 src/nvidia/src/kernel/rmapi/rmapi_utils.c:229), so stock clients send it and vGPU guests refuse it |
| `NV2080_CTRL_CMD_BUS_SERVICE_GPU_MULTIFUNC_STATE` | 0x20801812 | 8/8 | user | DENY | 3 | pure emulation | no caller in the tree |
| `NV2080_CTRL_CMD_BUS_CLEAR_PEX_COUNTERS` | 0x20801814 | 4/4 | user | DENY | 2 | pure emulation | no kernel caller; in the vGPU-guest skip list (615 rmapi_utils.c:213) |
| `NV2080_CTRL_CMD_BUS_FREEZE_PEX_COUNTERS` | 0x20801815 | 8/8 | user | DENY | 3 | pure emulation | no caller in the tree |
| `NV2080_CTRL_CMD_BUS_SET_PCIE_LTR_LATENCY` | 0x20801818 | 8/8 | user | DENY | 2 | pure emulation | no kernel caller; in the vGPU-guest skip list (615 rmapi_utils.c:228) |
| `NV2080_CTRL_CMD_BUS_CLEAR_PEX_UTIL_COUNTERS` | 0x20801820 | 4/4 | user | DENY | 3 | pure emulation | no caller in the tree |
| `NV2080_CTRL_CMD_BUS_CONTROL_PUBLIC_ASPM_BITS` | 0x20801824 | 1/1 | user | DENY | 3 | pure emulation | no caller in the tree |
| `NV2080_CTRL_CMD_BUS_SET_EOM_PARAMETERS` | 0x20801826 | 3/3 | user | DENY | 3 | pure emulation | no caller in the tree |
| `NV2080_CTRL_CMD_BUS_SET_P2P_MAPPING` | 0x2080182E | 32/32 | kernel | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/bus/arch/pascal/kern_bus_gp100.c:169 (_kbusCreateNvlinkPeerMapping, NVLink peers only, CONNECTION_TYPE_NVLINK at :162) |
| `NV2080_CTRL_CMD_BUS_UNSET_P2P_MAPPING` | 0x2080182F | 32/32 | kernel | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/bus/arch/pascal/kern_bus_gp100.c:546, NVLink peer removal under IS_VGPU_GSP_PLUGIN_OFFLOAD_ENABLED (:536) |
| `NV208F_CTRL_CMD_BIF_PBI_WRITE_COMMAND` | 0x208F0701 | 12/12 | user | DENY | 3 | pure emulation | no caller in the tree |
| `NV208F_CTRL_CMD_BIF_CONFIG_REG_READ` | 0x208F0702 | 8/8 | user | REFUSE | 3 | pure emulation | no caller in the tree |
| `NV208F_CTRL_CMD_BIF_CONFIG_REG_WRITE` | 0x208F0703 | 8/8 | user | DENY | 3 | pure emulation | no caller in the tree |
| `NV208F_CTRL_CMD_BIF_INFO` | 0x208F0704 | 8/8 | user | MODEL:config-space (exists: config.rs) | 3 | pure emulation | no caller in the tree |

**Subfamilies.** (1) BAR0 user-mapping validation: `GPU_VALIDATE_MEM_MAP_REQUEST`. (2) Physical-link setters: `BUS_SET_PCIE_LINK_WIDTH`, `BUS_SET_PCIE_SPEED`, `BUS_SET_PCIE_LTR_LATENCY`, `BUS_CONTROL_PUBLIC_ASPM_BITS`, `BUS_SET_EOM_PARAMETERS`, `NV0080 BIF_ASPM_CYA_UPDATE`. (3) PEX counter control: `BUS_CLEAR_PEX_COUNTERS`, `BUS_FREEZE_PEX_COUNTERS`, `BUS_CLEAR_PEX_UTIL_COUNTERS`. (4) Multi-function power: `BUS_SERVICE_GPU_MULTIFUNC_STATE`. (5) NV208F BIF diag: `BIF_PBI_WRITE_COMMAND`, `BIF_CONFIG_REG_READ`, `BIF_CONFIG_REG_WRITE`, `BIF_INFO`. (6) PCIe P2P mailbox (kernel-internal): `INTERNAL_BUS_SETUP_P2P_MAILBOX_LOCAL`, `_REMOTE`, `INTERNAL_BUS_DESTROY_P2P_MAILBOX`. (7) Things that never fire on TU106/Linux: `INTERNAL_SET_P2P_CAPS`, `INTERNAL_REMOVE_P2P_CAPS` (vGPU host), `INTERNAL_BIF_SET_PCIE_RO` (GA10x), `INTERNAL_BIF_DISABLE_SYSTEM_MEMORY_ACCESS` (Windows), `BUS_SET_P2P_MAPPING` and `BUS_UNSET_P2P_MAPPING` (NVLink).

**State a device model needs.** Only two pieces. First, the guest BAR0 base, which comes from the device's own config space (`config.rs`), plus a per-device allow-list of BAR0 windows that non-root guest processes may map. The list is empty today, it is per device, and it survives a guest driver reload because it depends only on the chip's register map. Second, `BIF_INFO` reads the ASPM Control bits of Link Control in the emulated PCIe capability. That capability is copied from the card and is writable under PCI rules (`config.rs:1-13`), so it is per device and lives as long as the config space. Nothing else needs state. The link setters and counter controls get a fixed refusal. Also note that the emulated header type is 0x80 (multi-function, copied from `caraxes/reference/rtx2070/config-space.txt` line `00:`), but only function 0 exists.

**What GSP-RM does.** All 24 controls are closed on the physical side. `GPU_VALIDATE_MEM_MAP_REQUEST` has a kernel body (615 `src/nvidia/src/kernel/gpu/subdevice/subdevice_ctrl_gpu_kernel.c:2918-2995`; 610 `:2915`). It returns READ_WRITE up front (`:2934`). Inside BAR0 it grants PTIMER read-only (`tmrGetTimerBar0MapInfo_PTIMER`, `timer/timer_ptimer.c:184-185`), the usermode page read-write (`kfifoGetUsermodeMapInfo_GV100`, `fifo/arch/volta/kernel_fifo_gv100.c:155-173`) and PMC read-only (`mc/arch/maxwell/kernel_mc_gm107.c:50-51`). Every other BAR0 range goes to GSP when `IS_FW_CLIENT` (`:2974-2982`), and without GSP it returns `NV_ERR_PROTECTION_FAULT` (`:2985`). A range inside the FB aperture is always OK (`:2988-2992`). The params go to GSP unchanged: `start -= gpuPhysAddr` (`:2939`) changes only a local copy, so GSP receives the absolute bus address. Inferred from this: GSP-RM approves BAR0 windows of objects that only it tracks. The link setters retrain or reprogram the physical XVE/PMU (`ctrl2080bus.h:700-749`, `:1030-1041`, `:1091-1135`; `ctrl0080bif.h:84-99`). The PEX clear/freeze controls act on the XVE counters (`ctrl2080bus.h:950-985`, `:1072-1089`). The PBI controls drive the config-space post box (`ctrl208fbif.h:34-125`). The mailbox controls program P2P windows and the write-mailbox tag (`ctrl2080internal.h:1160-1263`).

**Who calls it and when.** `VALIDATE_MEM_MAP` is called from `RmValidateMmapRequest` (615 `src/nvidia/arch/nvalloc/unix/src/osapi.c:2251-2280`, control at `:2273`), which `rm_create_mmap_context` calls for every BAR mapping (`:2588`; 610 `osapi.c:2199`, `:2536`). Administrators skip the check entirely (`:2262-2266`). The kernel enforces the returned protection at fault time (`kernel-open/nvidia/nv-mmap.c:155`, `:758`). CUDA's mappings (usermode doorbell, PTIMER) are decided locally, which is why no run log shows the control. The mailbox controls come from `kern_bus_gm200.c:124/:140` and `kern_bus.c:649`, only when a PCIe P2P mapping is created or destroyed. SET/REMOVE_P2P_CAPS come from `gpu.c:3432/:3320/:3480`, only under `IS_VGPU_GSP_PLUGIN_OFFLOAD_ENABLED` (`gpu.c:3623`, `:4089`). SET_PCIE_RO comes from `kernel_bif.c:615`, gated by a property that only the GA10x HAL sets (`generated/g_kernel_bif_nvoc.c:822-836`). DISABLE_SYSTEM_MEMORY_ACCESS is called from `kernel_bif_gm107.c:1046` behind a Windows assert (`:1043`). SET/UNSET_P2P_MAPPING are NVLink-only (`kern_bus_gp100.c:162`, `:536-546`). The public link and counter controls have no caller in the tree. `SET_PCIE_SPEED`, `SET_PCIE_LTR_LATENCY` and `CLEAR_PEX_COUNTERS` are in `rmapiutilSkipErrorMessageForUnsupportedVgpuGuestControl` (`src/nvidia/src/kernel/rmapi/rmapi_utils.c:213`, `:228-229`). So closed stock clients (nvml class) do send them, and a vGPU guest answers NOT_SUPPORTED without an error print. There are no hits in the 79 native traces and none in the Caraxes runs.

**Truth source.** `VALIDATE_MEM_MAP` comes from guest state: the device's own BAR0 base and register map. The real GSP-RM answer cannot be read as an unprivileged host client, because the control is kernel-privileged (flags 0x110, no NON_PRIVILEGED). `BIF_INFO` comes from the emulated config space. The link, counter, PBI and multi-function controls are pure emulation, and the answer is a refusal. The E members have no truth source (none).

**Isolation risk.** The host-side risk is in forwarding. The following controls are non-privileged, so the device's unprivileged host client could forward them to the physical card: `SET_PCIE_LTR_LATENCY`, `CONTROL_PUBLIC_ASPM_BITS`, `SET_EOM_PARAMETERS`, `CLEAR/FREEZE_PEX_COUNTERS`, `CLEAR_PEX_UTIL_COUNTERS`, `SERVICE_GPU_MULTIFUNC_STATE`, and the NV208F PBI writes (NV20_SUBDEVICE_DIAG allocates non-privileged, `rmapi/resource_list.h:864-870`). Forwarded, they would change ASPM or LTR on the link under the host desktop, power the host HDA function up or down, or zero counters the host reads. The PEX case is concrete: `BUS_GET_PEX_COUNTERS` and `BUS_GET_PEX_UTIL_COUNTERS` are already forwarded (`ver/v610.rs:936-937`), so a forwarded clear would show on the host. The privileged link setters (`SET_PCIE_LINK_WIDTH`, `SET_PCIE_SPEED`) would retrain the physical link and must never be forwarded. `VALIDATE_MEM_MAP` does not reach the host: guest BAR0 is trapped or backed by a device memfd (`device.rs:261-320`, intr-page.md section 2). A wrong approval, however, lets a non-root guest process write emulated BAR0 registers that the guest kernel owns: the `NV_PCFG` config mirror (`bar0.rs`, WriteEffect), the falcon/GSP queue registers, and the interrupt page. That escalates privilege inside the guest. The host stays behind D17/D18 validation.

**Design proposal.** Put the model in a new `crates/vfio-user-nvgpu/src/bus.rs`, called from the ctrl.rs dispatcher before the `unanswered` fallback (`ctrl.rs:1170-1175`). Four rules:
1. `DENY` table: subfamilies 2, 3 and 4, plus `PBI_WRITE_COMMAND` and `CONFIG_REG_WRITE`. They answer NV_ERR_NOT_SUPPORTED with the label `refused (isolation: acts on the physical link/board)`. NOT_SUPPORTED is what a vGPU guest gives and what stock clients tolerate (rmapi_utils.c list).
2. `REFUSE` for the mailbox trio and `CONFIG_REG_READ`, with the label `refused (feature absent on this board: single GPU, no PCIe P2P)`. This matches `INTERNAL_GET_PCIE_P2P_CAPS` (`ctrl.rs:1103-1110`).
3. `bar0-map-policy`: compute `off = addressStart - bar0_base(config)`. Outside `[0, 16 MiB)` or on overflow, return PROTECTION_FAULT. Inside, return PROTECTION_FAULT unless the range lies fully in an allow-list entry, and the allow-list starts empty. The label is `guest (BAR0 map policy)` and the note carries the offset.
4. `BIF_INFO` returns index 0 or 1 from LnkCtl bits 0 and 1 of the emulated PCIe capability, and INVALID_ARGUMENT for other indices.

The six E members go into the chip file's E-family refusal (gsp-surface.md section 3 E).

Tests:
- Unit tests against the generated ABI (`ver/v615.rs` sizes 24/8/…) for every control in the table.
- A guard test that no command of this slice appears in `FORWARD`, the GSS legacy list, or `MIRROR_FORWARDED`.
- bar0-map-policy edge cases: range straddling BAR0 end, length 0, address below BAR0, 64-bit wrap.
- Gate run: the guest mmap probe (Measurements 1).

**Estimate.** 1.5 to 3 engineer-days: bus.rs with the tables and labels (0.5), bar0-map-policy with tests (0.5 to 1), BIF_INFO from config.rs (0.25), guard test over the forward lists (0.25), guest mmap probe and gate run (0.5 to 1).

**Client likelihood.** The group ranks 2. `GPU_VALIDATE_MEM_MAP_REQUEST` decides it: any non-root mmap of an unusual BAR0 range by a GL/Vulkan/CUDA/tool process. `SET_PCIE_SPEED` and `CLEAR_PEX_COUNTERS` also rank 2, because they are in the vGPU-guest skip list and stock clients therefore send them. Nothing in the group is on the rank-1 path today, since CUDA's own maps are answered by the kernel.

**Roadmap.** Linux CUDA client: already safe, because NOT_SUPPORTED fails the mmap just as PROTECTION_FAULT would. The policy mainly makes the status honest and the decision explicit. Wayland/KMS with NVENC: no dependency. Vulkan games: the same mmap path as CUDA. The DENY table should land before anyone widens `FORWARD`. It depends on nothing, and `BIF_INFO` depends only on config.rs.

**Reclassification.** 8 controls:
- C (2): `GPU_VALIDATE_MEM_MAP_REQUEST`, which depends only on the guest BAR0 base and the device register map, and `NV208F BIF_INFO`, which is emulated LnkCtl ASPM bits.
- E (6):
  - `INTERNAL_SET_P2P_CAPS` and `INTERNAL_REMOVE_P2P_CAPS`: vGPU host with GSP plugin offload only.
  - `INTERNAL_BIF_SET_PCIE_RO`: the gating HAL is GA10x-only.
  - `INTERNAL_BIF_DISABLE_SYSTEM_MEMORY_ACCESS`: Windows only.
  - `BUS_SET_P2P_MAPPING` and `BUS_UNSET_P2P_MAPPING`: NVLink peers only.

**Measurements.**
1. Guest mmap probe (guest only, no host access). A non-root program in the 615 guest opens `/dev/nvidia0` and mmaps four ranges of the guest BAR0: PTIMER (0x9000), the usermode page (`NV_REG_BASE_USERMODE`), PMC (0x0) and one other page (for example 0x100000). It then repeats as root. Expected: the device trace shows 0x20800198 only for the fourth non-root map, with an `addressStart` equal to the guest BAR0 GPA plus the offset, and the mmap fails. Root sees no control at all. This confirms the address convention and the root bypass.
2. What the real GSP-RM approves beyond PTIMER, usermode and PMC. This can only be measured by an unprivileged mmap of a host `/dev/nvidia0` BAR0 range, unmapped without being touched. It touches host GPU state, so it needs the operator's go-ahead. It is optional: the policy stays deny-by-default whatever the result.
3. Stock-client sweep in the guest: `nvidia-smi -q`, `nvidia-smi -q -d PCIE`, `nvidia-settings -q all` on the Wayland session. Record the device trace for any control of this slice to settle the rank-2 claims for `SET_PCIE_SPEED`, `SET_PCIE_LTR_LATENCY` and `CLEAR_PEX_COUNTERS`.
4. Consistency check, from files only: LnkCtl ASPM bits in `config-space.txt` versus the host-forwarded link fields that nvidia-smi shows in the guest. Makes sure `BIF_INFO` and the forwarded telemetry do not contradict each other.

### 6.5 Display (107)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV0073_CTRL_CMD_SYSTEM_GET_HOTPLUG_STATE` | 0x0073010A | 12/12 | user | MODEL:display-connector (exists: the per-connector pending plug/unplug flags; add a hotplugAfterEdid mask, not cleared on read) | 3 | pure emulation | no caller in the tree (NVKMS uses SYSTEM_GET_HOTPLUG_UNPLUG_STATE, 615 src/nvidia-modeset/src/nvkms-event.c:56-110) |
| `NV0073_CTRL_CMD_SYSTEM_VALIDATE_SRM` | 0x00730118 | 3600/3600 | admin | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SYSTEM_GET_SRM_STATUS` | 0x00730119 | 8/8 | admin | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SYSTEM_HDCP_REVOCATION_CHECK` | 0x0073011B | 3596/3596 | admin | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SYSTEM_UPDATE_SRM` | 0x0073011C | 3596/3596 | admin | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SYSTEM_CLEAR_ELV_BLOCK` | 0x0073012E | 8/8 | kernel | ACK | 2 | pure emulation | 615 src/nvidia-modeset/src/nvkms-vrr.c:628 ClearElvBlock, from SetStallLockOneDisp (nvkms-vrr.c:726) when a head leaves VRR at flip time; only with VRR enabled (SetVrrActivePriv returns early otherwise, nvkms-vrr.c:784-786) |
| `NV0073_CTRL_CMD_SYSTEM_ARM_LIGHTWEIGHT_SUPERVISOR` | 0x0073012F | 20/20 | admin | ACK | 2 | pure emulation | 615 src/nvidia-modeset/src/nvkms-evo.c:553 nvEvoArmLightweightSupervisor, from SetStallLockOneDisp (nvkms-vrr.c:724, :748) around every VRR on/off switch |
| `NV0073_CTRL_CMD_SYSTEM_CONFIG_VRR_PSTATE_SWITCH` | 0x00730134 | 24/24 | admin | ACK | 2 | pure emulation | 615 src/nvidia-modeset/src/nvkms-vrr.c:672 ConfigVrrPstateSwitch, from RmEnableVrr/RmDisableVrr (nvkms-vrr.c:487, :394) and SetStallLockOneDisp (:698, :762); returns early without a VRR head (:642-646); failure only logged (:675-679) |
| `NV0073_CTRL_CMD_SYSTEM_SET_HOTPLUG_EVENT_CONFIG` | 0x00730145 | 16/16 | admin | MODEL:display-connector (new: per-display hotplug event enable mask gating the POST_EVENT HOTPLUG the connector model raises) | 3 | pure emulation | no caller in the tree |
| `NV0073_CTRL_CMD_SYSTEM_RECORD_CHANNEL_REGS` | 0x0073014A | 16/16 | kernel | REFUSE | 3 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SYSTEM_CHECK_SIDEBAND_I2C_SUPPORT` | 0x0073014B | 8/8 | kernel | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SYSTEM_CHECK_SIDEBAND_SR_SUPPORT` | 0x0073014C | 12/12 | kernel | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SYSTEM_NOTIFY_DRR_MSCG_WAR` | 0x00730159 | 12/12 | user | ACK | 5 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SPECIFIC_FAKE_DEVICE` | 0x00730243 | 16/16 | admin | REFUSE | 3 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SPECIFIC_SET_HDMI_ENABLE` | 0x00730273 | 12/12 | admin | REFUSE | 4 | none | 615 src/nvidia-modeset/src/nvkms-hdmi.c:400 HdmiSendEnable, from nvDpyUpdateHdmiPreModesetEvo (nvkms-hdmi.c:1418-1427) for HDMI dpys only |
| `NV0073_CTRL_CMD_SPECIFIC_CTRL_HDMI` | 0x00730274 | 12/12 | admin | REFUSE | 4 | none | 615 src/common/modeset/hdmipacket/nvhdmipkt_9171.c:611 and nvhdmipkt_C871.c:196 (HDMI packet write path) |
| `NV0073_CTRL_CMD_SPECIFIC_SET_HDMI_AUDIO_MUTESTREAM` | 0x00730275 | 12/12 | admin | REFUSE | 4 | none | 615 src/nvidia-modeset/src/nvkms-hdmi.c:835 SetHdmiAudioMute (HDMI audio) |
| `NV0073_CTRL_CMD_SPECIFIC_GET_HDCP_DIAGNOSTICS` | 0x00730281 | 12/12 | admin | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SPECIFIC_HDCP_CTRL` | 0x00730282 | 2608/2608 | admin | MODEL:display-connector (new per-cmd handler: DISABLE_AUTHENTICATION and ABORT_AUTHENTICATION return NV_OK with err 0; READ_TOPOLOGY returns isHdcpCapable 0 and a zero topology; RENEGOTIATE and VALIDATE_LINK return NV_OK with err UNSUCCESSFUL) | 1 | pure emulation | 615 src/common/displayport/src/dp_connectorimpl.cpp:8491 (610: :8001) configureHDCPDisableAuthentication (dp_evoadapter.cpp:443) on every SST unplug in notifyLongPulseInternal; READ_TOPOLOGY from 615 src/nvidia-modeset/src/nvkms-rm.c:1628 on HDCP KsvOk/RepComplete events (nvkms-event.c:304-308) for nvidia-drm; RENEGOTIATE/VALIDATE_LINK only for an HDCP-capable sink (dp_connectorimpl.cpp:2939, :5771) |
| `NV0073_CTRL_CMD_SPECIFIC_GET_ACPI_DOD_DISPLAY_PORT_ATTACHMENT` | 0x00730285 | 12/12 | admin | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SPECIFIC_SET_OD_PACKET_CTRL` | 0x00730289 | 16/16 | kernel | REFUSE | 4 | none | 615 src/nvidia-modeset/src/nvkms-evo1.c:269 nvEvo1DisableHdmiInfoFrame (HDMI vendor infoframe, nvkms-hdmi.c:602); 615 src/common/modeset/hdmipacket/nvhdmipkt_0073.c:78 |
| `NV0073_CTRL_CMD_SPECIFIC_HDCP_KSVLIST_VALIDATE` | 0x0073028D | 12/12 | admin | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SPECIFIC_HDCP_UPDATE` | 0x0073028E | 12/12 | admin | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SPECIFIC_SET_BACKLIGHT_BRIGHTNESS` | 0x00730292 | 16/16 | kernel | REFUSE | 4 | none | 615 src/nvidia-modeset/src/nvkms.c:7185 nvKmsSetBacklight (nvidia-drm backlight device) and nvkms-attributes.c:75; registered only for internal panels (SYSTEM_GET_INTERNAL_DISPLAYS answers none) |
| `NV0073_CTRL_CMD_SPECIFIC_SET_HDMI_SINK_CAPS` | 0x00730293 | 12/12 | admin | REFUSE | 4 | none | 615 src/nvidia-modeset/src/nvkms-hdmi.c:374 nvSendHdmiCapsToRm on dpy connect/disconnect; returns early for DP-library connectors (nvkms-hdmi.c:320-322) |
| `NV0073_CTRL_CMD_SPECIFIC_SET_HDMI_FRL_CONFIG` | 0x0073029A | 16/16 | admin | REFUSE | 5 | none | 615 src/common/modeset/hdmipacket/nvhdmipkt_C671.c:194 SetFRLLinkRate |
| `NV0073_CTRL_CMD_SPECIFIC_SET_HDMI_FRL_FLUSH_MODE` | 0x0073029B | 12/12 | admin | REFUSE | 5 | none | 615 src/common/modeset/hdmipacket/nvhdmipkt_C671.c:231 SetFRLFlushMode |
| `NV0073_CTRL_CMD_SPECIFIC_GET_REGIONAL_CRCS` | 0x007302A0 | 48/48 | admin | REFUSE | 3 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SPECIFIC_GET_HDMI_SCDC_DATA` | 0x007302A6 | 12/12 | admin | REFUSE | 4 | none | 615 src/nvidia-modeset/src/nvkms-hdmi.c:2743 GetForceMaxFrlRateType (HDMI 2.1 FRL rate test of an HDMI sink) |
| `NV0073_CTRL_CMD_SPECIFIC_GET_HDMI_FRL_CAPACITY_COMPUTATION` | 0x007302A8 | 132/132 | admin | REFUSE | 5 | none | 615 src/common/modeset/hdmipacket/nvhdmipkt_C671.c:639, :704, :824 (FRL/DSC capacity for the C671 packet class) |
| `NV0073_CTRL_CMD_SPECIFIC_SET_SHARED_GENERIC_PACKET` | 0x007302A9 | 56/56 | admin | REFUSE | 5 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SPECIFIC_ACQUIRE_SHARED_GENERIC_PACKET` | 0x007302AA | 12/12 | admin | REFUSE | 5 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SPECIFIC_RELEASE_SHARED_GENERIC_PACKET` | 0x007302AB | 12/12 | admin | REFUSE | 5 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SPECIFIC_DEFAULT_ADAPTIVESYNC_DISPLAY` | 0x007302AE | 6/6 | kernel | TABLE | 4 | host RO | 615 src/nvidia-modeset/src/nvkms-dpy.c:3556 nvDpyIsAdaptiveSyncDefaultlisted, only from nvDpyUpdateHdmiVRRCaps (nvkms-hdmi.c:1447) for HDMI 2.1 VRR EDIDs |
| `NV0073_CTRL_CMD_INTERNAL_DFP_GET_DISP_MUX_STATUS` | 0x00730404 | 12/12 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/gpu/disp/disp_common_ctrl_acpi.c:1057 in dispcmnCtrlCmdDfpGetDispMuxStatus_IMPL (:1011), the kernel body of the public mux-status control |
| `NV0073_CTRL_CMD_INTERNAL_DFP_SWITCH_DISP_MUX` | 0x00730460 | 20/20 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/gpu/disp/disp_common_ctrl_acpi.c:926 in dispcmnCtrlCmdDfpSwitchDispMux_IMPL (:874) |
| `NV0073_CTRL_CMD_FRL_CONFIG_MACRO_PAD` | 0x00730502 | 24/24 | admin | REFUSE | 5 | none | no caller in the tree |
| `NV0073_CTRL_CMD_DFP_GET_SPREAD_SPECTRUM` | 0x0073114C | 8/8 | admin | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_DFP_GET_LCD_GPIO_PIN_NUM` | 0x00731154 | 16/16 | admin | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_DFP_CONFIG_TWO_HEAD_ONE_OR` | 0x00731156 | 20/20 | admin | REFUSE | 4 | none | no caller in the tree; NVKMS programs 2Heads1OR with core-channel methods (HEAD_SET_RG_MERGE, 615 src/nvidia-modeset/src/nvkms-evo3.c:7492) |
| `NV0073_CTRL_CMD_DFP_DSC_CRC_CONTROL` | 0x00731157 | 20/20 | admin | REFUSE | 3 | none | 615 src/common/displayport/src/dp_evoadapter.cpp:2198 dscCrcTransaction, from DeviceImpl (dp_deviceimpl.cpp:3204): DSC CRC test |
| `NV0073_CTRL_CMD_DFP_INIT_MUX_DATA` | 0x00731158 | 16/16 | kernel | REFUSE | 4 | none | 615 src/nvidia-modeset/src/nvkms-rm.c:5673 MuxInit, only for DYNAMIC_MUX_CAPABLE dpys (DFP_GET_INFO answers 0) |
| `NV0073_CTRL_CMD_DFP_RUN_PRE_DISP_MUX_OPERATIONS` | 0x00731161 | 24/24 | kernel | REFUSE | 4 | none | 615 src/nvidia-modeset/src/nvkms-rm.c:5748 nvRmMuxPre (dynamic mux switch) |
| `NV0073_CTRL_CMD_DFP_RUN_POST_DISP_MUX_OPERATIONS` | 0x00731162 | 24/24 | kernel | REFUSE | 4 | none | 615 src/nvidia-modeset/src/nvkms-rm.c:5846 nvRmMuxPost (dynamic mux switch) |
| `NV0073_CTRL_CMD_DFP_GET_DSI_MODE_TIMING` | 0x00731166 | 248/248 | admin | REFUSE | 5 | none | 615 src/nvidia-modeset/src/nvkms-dpy.c:535 ReadDSITimingsFromResman (DSI connectors) |
| `NV0073_CTRL_CMD_DFP_GET_FIXED_MODE_TIMING` | 0x00731172 | 72/72 | admin | REFUSE | 5 | none | 615 src/nvidia-modeset/src/nvkms-dpy.c:660 ReadDPSerializerTimings (DP serializer connectors) |
| `NV0073_CTRL_CMD_DFP_ENTER_DISPLAY_POWER_GATING` | 0x00731174 | 24/24 | kernel | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_DFP_EXIT_DISPLAY_POWER_GATING` | 0x00731175 | 20/20 | kernel | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_DFP_EDP_DRIVER_UNLOAD` | 0x00731176 | 8/8 | user | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SYSTEM_SET_REGION_RAM_RECTANGLES` | 0x00731177 | 168/168 | admin | REFUSE | 5 | none | no caller in the tree |
| `NV0073_CTRL_CMD_SYSTEM_CONFIGURE_SAFETY_INTERRUPTS` | 0x00731178 | 52/52 | admin | REFUSE | 5 | none | no caller in the tree |
| `NV0073_CTRL_CMD_DP_GET_LANE_DATA` | 0x00731345 | 44/44 | admin | MODEL:dp-sink (exists: the trained link; return the trained lane count and drive/pre-emphasis level 0) | 3 | pure emulation | 615 src/common/displayport/src/dp_evoadapter.cpp:2006 getDpLaneData via ConnectorImpl::getDpLaneData (dp_connectorimpl.cpp:9328), a DP-library API for test clients |
| `NV0073_CTRL_CMD_DP_SET_LANE_DATA` | 0x00731346 | 44/44 | admin | ACK | 3 | pure emulation | 615 src/common/displayport/src/dp_evoadapter.cpp:2061 setDpLaneData via ConnectorImpl::setDpLaneData (dp_connectorimpl.cpp:9333) |
| `NV0073_CTRL_CMD_DP_SET_TESTPATTERN` | 0x00731347 | 56/56 | admin | MODEL:dp-sink (new: the recorded test pattern per connector) | 3 | pure emulation | 615 src/common/displayport/src/dp_evoadapter.cpp:1011 physicalLayerSetTestPattern from handlePhyPatternRequest (dp_connectorimpl.cpp:9487) and notifyDetachBegin (:4404), only when the sink's DPCD PHY_TEST_PATTERN is non-zero; :1993 setDpTestPattern (API) |
| `NV0073_CTRL_CMD_DP_GET_TESTPATTERN` | 0x00731348 | 48/48 | admin | MODEL:dp-sink (new: returns the recorded pattern, default none) | 3 | pure emulation | 615 src/common/displayport/src/dp_evoadapter.cpp:1972 getDpTestPattern via ConnectorImpl (dp_connectorimpl.cpp:9318) |
| `NV0073_CTRL_CMD_DP_SET_PREEMPHASIS_DRIVECURRENT_POSTCURSOR2_DATA` | 0x00731351 | 1032/1032 | admin | ACK | 3 | pure emulation | no caller in the tree |
| `NV0073_CTRL_CMD_DP_GET_PREEMPHASIS_DRIVECURRENT_POSTCURSOR2_DATA` | 0x00731352 | 1032/1032 | admin | TABLE | 3 | chip table | no caller in the tree |
| `NV0073_CTRL_CMD_DP_ASSR_CTRL` | 0x0073135A | 20/20 | admin | REFUSE | 4 | none | 615 src/common/displayport/src/dp_evoadapter.cpp:589 disableAlternateScramblerReset, only for eDP (dp_connectorimpl.cpp:4569-4576) |
| `NV0073_CTRL_CMD_DP_TOPOLOGY_ALLOCATE_DISPLAYID` | 0x0073135B | 24/24 | admin | REFUSE | 4 | none | 615 src/nvidia-modeset/src/nvkms-rm.c:2375 nvRmAllocDisplayId for MST dpys only (non-MST returns the static id, nvkms-rm.c:2366-2369); 615 src/common/displayport/src/dp_evoadapter.cpp:2098 allocDisplayId |
| `NV0073_CTRL_CMD_DP_TOPOLOGY_FREE_DISPLAYID` | 0x0073135C | 8/8 | admin | REFUSE | 4 | none | 615 src/nvidia-modeset/src/nvkms-rm.c:2455 nvRmFreeDisplayId; 615 src/common/displayport/src/dp_evoadapter.cpp:2114 (MST) |
| `NV0073_CTRL_CMD_DP_GET_EDP_DATA` | 0x00731361 | 12/12 | admin | REFUSE (NV_ERR_INVALID_ARGUMENT: not an eDP display) | 4 | none | 615 src/common/displayport/src/dp_evoadapter.cpp:242 getEdpPowerData, only when isEDP() (dp_connectorimpl.cpp:4532, dp_deviceimpl.cpp:1265); 615 src/nvidia/src/kernel/gpu/disp/disp_common_kern_ctrl_minimal.c:288 inside DP_GENERATE_FAKE_INTERRUPT (test) |
| `NV0073_CTRL_CMD_DP_SET_RATE_GOV` | 0x00731363 | 16/16 | admin | REFUSE | 4 | none | 615 src/common/displayport/src/dp_evoadapter.cpp:1957 controlRateGoverning, only on MST links (dp_connectorimpl.cpp:7389-7420 inside linkUseMultistream, :7519 after the SST early return at :7473, :7631 beforeAddStreamMST) |
| `NV0073_CTRL_CMD_DP_SET_ECF` | 0x00731366 | 24/24 | admin | REFUSE | 4 | none | 615 src/common/displayport/src/dp_evoadapter.cpp:387 configureAndTriggerECF, from HDCP/MST paths (dp_connectorimpl.cpp:2857, :2928, :4624; dp_groupimpl.cpp:200, :523, :687) |
| `NV0073_CTRL_CMD_DP_SEND_ACT` | 0x00731367 | 8/8 | admin | REFUSE | 4 | none | 615 src/common/displayport/src/dp_evoadapter.cpp:348 triggerACT, MST only (afterAddStream returns early for SST, dp_connectorimpl.cpp:7473) |
| `NV0073_CTRL_CMD_DP_CONFIG_RAD_SCRATCH_REG` | 0x0073136C | 28/28 | admin | ACK | 2 | pure emulation | 615 src/nvidia-modeset/src/dp/nvdp-connector.cpp:960 nvDPPause when skipConsoleRestore is set and a head is attached (teardown after NVKMS restored the console, nvkms-console-restore.c:984, nvkms-evo.c:5602); 615 src/common/displayport/src/dp_evoadapter.cpp:780 (MST VBIOS scratch, dp_groupimpl.cpp:784) |
| `NV0073_CTRL_CMD_DP_CONFIG_SINGLE_HEAD_MULTI_STREAM` | 0x0073136E | 24/24 | admin | REFUSE | 4 | none | 615 src/common/displayport/src/dp_evoadapter.cpp:719 configureSingleHeadMultiStreamMode, from dp_connectorimpl.cpp:3746 (single-head dual SST/MST) |
| `NV0073_CTRL_CMD_DP_SET_TRIGGER_SELECT` | 0x0073136F | 16/16 | admin | REFUSE | 4 | none | 615 src/common/displayport/src/dp_evoadapter.cpp:2125 configureTriggerSelect, MST paths only (dp_connectorimpl.cpp:4323, :4431, :7062) |
| `NV0073_CTRL_CMD_DP_SET_TRIGGER_ALL` | 0x00731370 | 12/12 | admin | REFUSE | 4 | none | 615 src/common/displayport/src/dp_evoadapter.cpp:2138 configureTriggerAll (dp_connectorimpl.cpp:7175, single-head MST) |
| `NV0073_CTRL_CMD_DP_GET_AUXLOGGER_BUFFER_DATA` | 0x00731373 | 2412/2412 | admin | REFUSE | 3 | none | 615 src/nvidia-modeset/src/nvkms-rm.c:5022 nvRmQueryDpAuxLog, an NVKMS disp attribute (nvkms-framelock.c:2030); drained at teardown only once enabled (nvkms-rm.c:2205-2208) |
| `NV0073_CTRL_CMD_DP_CONFIG_INDEXED_LINK_RATES` | 0x00731377 | 44/44 | admin | REFUSE | 4 | none | 615 src/common/displayport/src/dp_evoadapter.cpp:2247 configureLinkRateTable, only for sinks with indexed link rates (dp_connectorimpl.cpp:926-937); the sink model does not claim them |
| `NV0073_CTRL_CMD_DP_CONFIGURE_FEC` | 0x0073137A | 12/12 | admin | MODEL:dp-sink (exists: link config; set FEC enabled so DP_GET_LINK_CONFIG reports bFECEnabled 1) | 1 | pure emulation | 615 src/common/displayport/src/dp_evoadapter.cpp:2299 configureFec, from dp_connectorimpl.cpp:6912 after link training when activeLinkConfig.bEnableFEC (DSC modes) |
| `NV0073_CTRL_CMD_DP_CONFIG_MACRO_PAD` | 0x0073137B | 36/36 | admin | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_DP_AUXCH_I2C_TRANSFER_CTRL` | 0x0073137C | 276/276 | kernel | MODEL:display-connector (exists: I2C-over-AUX in display/aux.rs, EDID at 0x50; DDC/CI would be new) | 2 | pure emulation | 615 src/nvidia/arch/nvalloc/unix/src/osapi.c:4001 RmDpAuxI2CTransfer (:3958) for the Linux /dev/i2c adapters on DP connectors (ddcutil), created only if SPECIFIC_GET_I2C_PORTID reports a DDC port (the device answers none) |
| `NV0073_CTRL_CMD_DP_ENABLE_VRR` | 0x0073137D | 16/16 | admin | REFUSE | 4 | none | 615 src/common/displayport/src/dp_evoadapter.cpp:225 vrrRunEnablementStage, from VrrEnablement (dp_vrr.cpp:68-148): the G-SYNC monitor handshake |
| `NV0073_CTRL_CMD_DP_GET_GENERIC_INFOFRAME` | 0x0073137E | 52/52 | admin | MODEL:display-engine (new: the SDP the guest programmed for the head) | 3 | guest state | no caller in the tree |
| `NV0073_CTRL_CMD_DP_GET_MSA_ATTRIBUTES` | 0x0073137F | 36/36 | admin | MODEL:display-engine (new: MSA fields from the guest's head raster and SET_MSA_PROPERTIES_V2) | 3 | guest state | no caller in the tree |
| `NV0073_CTRL_CMD_DP_AUXCH_OD_CTRL` | 0x00731380 | 16/16 | admin | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_DP2X_LINK_TRAINING_CTRL` | 0x00731383 | 24/24 | admin | REFUSE | 5 | none | 615 src/common/displayport/src/dp_evoadapter2x.cpp:742-917, :1196 EvoMainLink2x::train/resetDPRXLink; EvoMainLink2x exists only when DP_GET_CAPS reports DP2.0 (dp_evoadapter.cpp:2159-2167) |
| `NV0073_CTRL_CMD_DP2X_GET_LANE_DATA` | 0x00731384 | 44/44 | admin | REFUSE | 5 | none | 615 src/common/displayport/src/dp_evoadapter2x.cpp:1322 getDp2xLaneData |
| `NV0073_CTRL_CMD_DP2X_SET_LANE_DATA` | 0x00731385 | 44/44 | admin | REFUSE | 5 | none | 615 src/common/displayport/src/dp_evoadapter2x.cpp:1346 setDp2xLaneData |
| `NV0073_CTRL_CMD_DP_AUXCH_VBL_CTRL` | 0x00731386 | 12/12 | kernel | REFUSE | 4 | none | no caller in the tree |
| `NV0073_CTRL_CMD_DP_SET_LEVEL_INFO_TABLE_DATA` | 0x00731387 | 272/272 | admin | ACK | 3 | pure emulation | no caller in the tree |
| `NV0073_CTRL_CMD_DP_GET_LEVEL_INFO_TABLE_DATA` | 0x00731388 | 272/272 | admin | TABLE | 3 | chip table | no caller in the tree |
| `NV0073_CTRL_CMD_DP2X_SET_LEVEL_INFO_TABLE_DATA` | 0x00731389 | 272/272 | admin | REFUSE | 5 | none | no caller in the tree |
| `NV0073_CTRL_CMD_DP2X_GET_LEVEL_INFO_TABLE_DATA` | 0x0073138A | 272/272 | admin | REFUSE | 5 | none | no caller in the tree |
| `NV0073_CTRL_CMD_DP_GET_CABLEID_INFO_FROM_MACRO` | 0x0073138D | 20/20 | admin | REFUSE | 5 | none | 615 src/common/displayport/src/dp_evoadapter2x.cpp:1487 getUSBCCableIDInfo; the DP 1.x EvoMainLink returns false without a call (615 src/common/displayport/inc/dp_evoadapter.h:381) |
| `NV0073_CTRL_CMD_DP_SET_WAR_FLAGS` (615 only) | 0x00731391 | -/16 | admin | ACK | 1 | pure emulation | 615 src/common/displayport/src/dp_evoadapter.cpp:363 setDpWarFlag, from dp_connectorimpl.cpp:8048 (every plug, after the OUI handshake) and :8475 (every unplug); TRUE only for a Realtek DP-tunneling sink (dp_wardatabase.cpp:130-139) |
| `NV2080_CTRL_CMD_INTERNAL_DISP_PINSETS_TO_LOCKPINS` | 0x20800ADC | 24/24 | internal | TABLE (no pinsets: bMasterScanLock and bSlaveScanLock FALSE) | 4 | chip table | 615 src/nvidia/src/kernel/gpu/disp/arch/v02/kern_disp_0207.c:100 kdispGetRgScanLock_v02_01, for NV5070 GET_RG_CONNECTED_LOCKPIN_STATELESS (disp_object_kern_ctrl_minimal.c:48), sent by NVKMS GetRasterLockPin (615 src/nvidia-modeset/src/nvkms-evo.c:8075) for cross-GPU raster lock |
| `NV2080_CTRL_CMD_LPWR_DIFR_PREFETCH_RESPONSE` | 0x20802802 | 4/4 | kernel | REFUSE | 4 | none | 615 src/nvidia-modeset/src/nvkms-difr.c:387 nvDIFRSendPrefetchResponse; never reached while LPWR_DIFR_CTRL SUPPORT_STATUS is refused (display/mod.rs:976-981, nvkms-difr.c:196-205) |
| `NV5070_CTRL_CMD_GET_PINSET_COUNT` | 0x50700115 | 8/8 | user | TABLE (pinsetCount 0) | 4 | chip table | no caller in the tree |
| `NV5070_CTRL_CMD_GET_PINSET_PEER` | 0x50700116 | 16/16 | user | REFUSE (NV_ERR_INVALID_ARGUMENT: no such pinset) | 4 | chip table | no caller in the tree |
| `NV5070_CTRL_CMD_IMP_SET_GET_PARAMETER` | 0x50700118 | 24/24 | user | MODEL:display-imp (new: parameter store; IMP_ENABLE reads TRUE, ASR/MSCG overrides recorded without effect) | 3 | pure emulation | no caller in the tree |
| `NV5070_CTRL_CMD_SET_MEMPOOL_WAR_FOR_BLIT_TEARING` | 0x50700119 | 8/8 | user | ACK | 5 | none | no caller in the tree |
| `NV5070_CTRL_CMD_GET_RG_STATUS` | 0x50700202 | 20/20 | admin | MODEL:display-engine (exists: the per-head clock; scanLocked/flipLocked YES for heads of a merge pair, NO otherwise) | 2 | pure emulation | 615 src/nvidia-modeset/src/nvkms-evo.c:9105 EvoWaitForRasterLock, from nvEvoEnableMergeModePostModeset (nvkms-evo.c:9714: 2Heads1OR when a DSC mode exceeds the head clock cap; supportsMergeMode TRUE in the C5 HAL, nvkms-evo3.c:7949) and cross-GPU fliplock (nvkms-evo.c:1863) |
| `NV5070_CTRL_CMD_GET_RG_UNDERFLOW_PROP` | 0x50700203 | 24/24 | user | MODEL:display-engine (new: per-head underflow properties, never underflows) | 3 | pure emulation | no caller in the tree |
| `NV5070_CTRL_CMD_SET_RG_UNDERFLOW_PROP` | 0x50700204 | 24/24 | user | MODEL:display-engine (new: stores the per-head underflow properties) | 3 | pure emulation | no caller in the tree |
| `NV5070_CTRL_CMD_GET_RG_FLIPLOCK_PROP` | 0x50700205 | 16/16 | user | REFUSE | 4 | none | no caller in the tree |
| `NV5070_CTRL_CMD_SET_RG_FLIPLOCK_PROP` | 0x50700206 | 16/16 | kernel | REFUSE | 4 | none | 615 src/nvidia-modeset/src/nvkms-evo.c:8722 nvSetSwapBarrierNotifyEvo, from nvkms-framelock.c:1377, :1388 (swap barrier with a framelock board) |
| `NV5070_CTRL_CMD_GET_PINSET_LOCKPINS` | 0x5070020B | 16/16 | user | REFUSE (NV_ERR_INVALID_ARGUMENT: no such pinset) | 4 | chip table | no caller in the tree |
| `NV5070_CTRL_CMD_GET_RG_SCAN_LINE` | 0x5070020C | 16/16 | user | MODEL:display-engine (exists: Engine::scanline, as for NV0073 SYSTEM_GET_SCANLINE) | 3 | pure emulation | no caller in the tree |
| `NV5070_CTRL_CMD_GET_FRAMELOCK_HEADER_LOCKPINS` | 0x5070020D | 16/16 | kernel | REFUSE (NV_ERR_NOT_SUPPORTED) | 4 | none | 615 src/nvidia-modeset/src/nvkms-evo.c:5670 QueryFrameLockHeaderPins via nvEvoGetPinForSignal, only in framelock/SLI lock states (nvkms-hw-states.c:337-1108; the single-GPU head lock uses internal pins without it, :304-335) and cross-GPU fliplock (nvkms-evo.c:1856-1858) |
| `NV5070_CTRL_CMD_GET_SOR_OP_MODE` | 0x50700422 | 76/76 | admin | REFUSE | 5 | none | no caller in the tree |
| `NV5070_CTRL_CMD_SET_SOR_OP_MODE` | 0x50700423 | 76/76 | admin | REFUSE | 5 | none | no caller in the tree |
| `NV9072_CTRL_CMD_NOTIFY_ON_VBLANK` | 0x90720101 | 8/8 | user | MODEL:display-engine (new: per-DispSw pending vblank notify, fired on the head's next vblank tick as a SEMAPHORE_SCHEDULE_CALLBACK event) | 2 | guest state | no caller in the tree; closed user space: the GL/EGL driver allocates GF100_DISP_SW per head (docs/caraxes/display-e.md:77, :96) |
| `NVC370_CTRL_CMD_SET_SOR_FLUSH_MODE` | 0xC3700401 | 20/20 | admin | REFUSE | 5 | none | 615 src/common/displayport/src/dp_evoadapter2x.cpp:1103 setFlushMode, :1148 clearFlushMode (EvoMainLink2x only) |
| `NVC370_CTRL_CMD_SET_FORCE_MODESWITCH_FLAGS_OVERRIDES` | 0xC3700602 | 16/16 | user | ACK | 3 | pure emulation | no caller in the tree |
| `NVC372_CTRL_CMD_IS_MODE_POSSIBLE_OR_SETTINGS` | 0xC3720102 | 220/220 | user | MODEL:display-imp (exists: is_mode_possible in display/mod.rs; extend to per-OR pixel clocks) | 4 | pure emulation | no caller in the tree |

Paths: `615 ctrl/...` is `615 src/common/sdk/nvidia/inc/ctrl/...`; `DPL/` is `615 src/common/displayport/src/`; `NVKMS/` is `615 src/nvidia-modeset/src/`; `display/` is `crates/vfio-user-nvgpu/src/display/`. Every control here routes to GSP (the kernel RM has no body for any of them) and today hits the display fallback `unanswered` NOT_SUPPORTED (`display/mod.rs:982-987`; the two NV2080 ones and NV9072 fall through to `ctrl.rs:1170-1175`). The display model is fully synthetic and forwards nothing (`display/mod.rs:17-18`); none of these 107 should ever be forwarded.

**Subfamilies.** Eight subfamilies, each with its own subsection at the end of this section (names, answers and ranks there; the full table comes from the JSONL): DP link and lane (32), HDMI/FRL/SCDC (13), HDCP/SRM (8), DFP/mux/eDP/backlight/power gating (24), system/hotplug/supervisor/VRR (11), NV5070 RG/lockpin/SOR (13), IMP/C370/C372 (5), vblank sw object (1).

**State a device model needs.** Almost none beyond what `display/` has. Per connector (`Connector` in `display/mod.rs`, lives as long as the device, survives guest driver reload): an HDCP state that is permanently "not capable, not authenticated" (so `HDCP_CTRL` can answer per `cmd`), the last test pattern and lane overrides (tools only), the FEC-enabled bit of the trained link (`DP_CONFIGURE_FEC` feeds `DP_GET_LINK_CONFIG.bFECEnabled`), a hotplug-event enable mask, and a "hotplug after last EDID read" mask that is not cleared on read (unlike the existing `pending_plug/unplug` pair behind `INTERNAL_GET_HOTPLUG_UNPLUG_STATE`). Per head (`engine.rs`, reset when the guest frees its core channel): merge-pair membership for `GET_RG_STATUS`, underflow properties, and a list of pending vblank notifications keyed by the guest's `GF100_DISP_SW` object (hClient, hObject, head, the semaphore/notifier target the method data selects), dropped when the object is freed. Per device: an IMP parameter store (`IMP_SET_GET_PARAMETER`), persistent until the guest driver unloads (header, `615 ctrl/ctrl5070/ctrl5070chnc.h:976`). Invariants: the answer tables are per chip/board (D14: board says DP SST only, no HDMI, no MST, no panel, no pinsets), and every answer carries a D9 label (`board`, `synthesized`, `guest`, `sink`).

**What GSP-RM does.** All 107 are closed firmware; the physical bodies are not in the open tree, so the behaviour is read from the header comments (cited per control in the JSONL) and from the callers. Four families have enough open context to pin down:

- `SPECIFIC_HDCP_CTRL` drives the SOR's HDCP 1.x/2.x state machine by `cmd` (`615 ctrl/ctrl0073/ctrl0073specific.h:911-1228`): RENEGOTIATE, DISABLE_AUTHENTICATION, ABORT_AUTHENTICATION with a reason, VALIDATE_LINK (returns `vP`, `aN`), READ_TOPOLOGY (capability, repeater, KSV list). The errors come back in `err` bits, not only in the status.
- The VRR trio (`CLEAR_ELV_BLOCK`, `ARM_LIGHTWEIGHT_SUPERVISOR`, `CONFIG_VRR_PSTATE_SWITCH`) configures the physical supervisor and the memory-clock switch inside the stretched vblank (`615 ctrl/ctrl0073/ctrl0073system.h:1548-1677`); on real hardware the last one changes pstate behaviour, a host-wide effect.
- `NV9072_CTRL_CMD_NOTIFY_ON_VBLANK` is the out-of-band form of the `NV9072_NOTIFY_ON_VBLANK` method (`615 ctrl/ctrl9072.h:57-77`). GSP-RM registers a one-shot vblank callback on the DispSw's head. At vblank the release reaches the CPU RM as the `SEMAPHORE_SCHEDULE_CALLBACK` event (`615 src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:1633-1635`). The kernel then calls `dispswReleaseSemaphoreAndNotifierFill` (`kernel_gsp.c:1237-1262`, `615 src/nvidia/src/kernel/disp/disp_sw.c:129-181`).
- `GET_RG_STATUS` reports the RG scan-lock/flip-lock bits (`615 ctrl/ctrl5070/ctrl5070rg.h:37-55`).

`NVC370_CTRL_CMD_SET_SOR_FLUSH_MODE` is declared "NV Display 5.0+ (GB20X+ and T264+)" (`615 ctrl/ctrlc370/ctrlc370or.h:36-73`), so GSP-RM on TU106 refuses it (inferred).

**Who calls it and when.** Only three controls sit on the path the display goal takes:

1. `SPECIFIC_HDCP_CTRL` (DISABLE_AUTHENTICATION) goes out on every SST unplug from `notifyLongPulseInternal` (`DPL/dp_connectorimpl.cpp:8491`, 610 `:8001`, via `dp_evoadapter.cpp:443`). It is the one D control seen in a Caraxes run (`caraxes/runs/display-cd-2026-09-24/d11-unplug/device-trace.txt:16-17`). The DP library only logs the failure (`dp_evoadapter.cpp:444-447`).
2. `DP_SET_WAR_FLAGS` exists only in 615 (`615 ctrl/ctrl0073/ctrl0073dp.h:3773`). It goes out on every plug after the OUI handshake (`dp_connectorimpl.cpp:8048`) and on every unplug (`:8475`), so the first 615 guest that runs the DP path will send it at boot.
3. `DP_CONFIGURE_FEC` goes out after link training whenever FEC is on, which DSC requires (`dp_connectorimpl.cpp:6898-6913`); this is milestone G, 4K144 (`docs/caraxes/display-scaling.md:417`).

Conditional and nearer the edge:

- `GET_RG_STATUS` comes from `nvEvoEnableMergeModePostModeset` (`NVKMS/nvkms-evo.c:9714`) when a DSC mode exceeds the head clock cap and 2Heads1OR is used. `supportsMergeMode` is TRUE on the C5 HAL (`NVKMS/nvkms-evo3.c:7949`), so the device's `HEAD_CLK_CAP` decides.
- The VRR trio comes from `SetStallLockOneDisp` (`NVKMS/nvkms-vrr.c:680-767`) and `RmEnable/DisableVrr`, but only once a VRR dpy exists (`nvkms-vrr.c:783-786`).
- `CONFIG_RAD_SCRATCH_REG` comes from `nvDPPause` at teardown after NVKMS restored the console (`615 src/nvidia-modeset/src/dp/nvdp-connector.cpp:938-973`).
- `DP_AUXCH_I2C_TRANSFER_CTRL` comes from nvidia.ko's `/dev/i2c` adapters (`615 src/nvidia/arch/nvalloc/unix/src/osapi.c:3958-4001`), which the device's `ddcPortId` 0 keeps from existing.
- `NV9072 NOTIFY_ON_VBLANK` would come from closed GL/EGL user space, which allocates `GF100_DISP_SW` per head (`docs/caraxes/display-e.md:77, 96`). No trace shows it: none of the 79 native GL/EGL/Vulkan traces contain it.

Everything else has either no caller in the open tree or a caller gated by a feature the board does not have:

- HDMI: `nvSendHdmiCapsToRm` returns for DP-library connectors, `nvkms-hdmi.c:320-322`.
- MST: `nvRmAllocDisplayId`, `nvkms-rm.c:2366-2369`; SST returns early in `afterAddStream`, `dp_connectorimpl.cpp:7473`.
- eDP: `isEDP()` guards at `dp_connectorimpl.cpp:4532, 4569`.
- Mux: `MuxInit` needs `DYNAMIC_MUX_CAPABLE`, `nvkms-rm.c:5673`.
- DP 2.x: `EvoMainLink2x` exists only if `DP_GET_CAPS` claims DP2.0, `dp_evoadapter.cpp:2159-2167`.
- Framelock/SLI: `nvkms-hw-states.c:337-1108`. The single-GPU head lock uses internal pins without a control, `:304-335`.
- DIFR: `nvDIFRAllocate` gives up when `LPWR_DIFR_CTRL` is refused, `nvkms-difr.c:196-205`, which `display/mod.rs:976-981` already does.

No kernel init, RC or suspend path sends any of them. The only kernel-internal senders are the ACPI mux bodies (`615 src/nvidia/src/kernel/gpu/disp/disp_common_ctrl_acpi.c:926, 1057`), the cross-GPU scan-lock path for `INTERNAL_DISP_PINSETS_TO_LOCKPINS` (`arch/v02/kern_disp_0207.c:100`) and a test path for `DP_GET_EDP_DATA` (`disp_common_kern_ctrl_minimal.c:288`).

**Truth source.** Pure emulation for everything the display goal needs: the connector, link and head are synthetic (D15), so the right answer is the one consistent with the device's own DP caps, DPCD and head clock. Chip/board tables cover the pinset, drive-table and adaptive-sync-list getters. Guest state covers the NV9072 notify targets and the MSA and infoframe readbacks. Host RO is possible in principle for `SPECIFIC_DEFAULT_ADAPTIVESYNC_DISPLAY`, a lookup keyed by EDID IDs, but the synthetic EDID makes the answer FALSE anyway.

**Isolation risk.** The risk lies entirely in forwarding. None of these may reach the host RM:

- `HDCP_CTRL` would act on the HDCP state of the host desktop's monitor.
- `CONFIG_VRR_PSTATE_SWITCH` and `IMP_SET_GET_PARAMETER` would change the host GPU's mclk switching and ASR/MSCG policy.
- `DFP_*_DISP_MUX*`, `ENTER/EXIT_DISPLAY_POWER_GATING`, `DP/FRL_CONFIG_MACRO_PAD`, `SET_SOR_OP_MODE` and `CONFIG_RAD_SCRATCH_REG` act on physical display hardware and VBIOS scratch registers the host scans out from.
- `LPWR_DIFR_PREFETCH_RESPONSE` touches host low-power state.
- `RECORD_CHANNEL_REGS` would leak the host's display register state.

The display module already forwards nothing, so the rule is "stay local". The one new outward-facing piece, the NV9072 notify, must build its `SEMAPHORE_SCHEDULE_CALLBACK` only from the guest's own hClient, hEvent, GPUVA and VA space (these go back to the guest kernel, never to the host) and must be bound to a synthetic head.

**Design proposal.** Everything lives in the existing `display/mod.rs` `control()` match (plus `engine.rs` for per-head state); no new file. Four generic rules cover the group:

1. **Absent-feature refusal (74 controls).** A per-board table of NV0073/NV5070/NVC370 command ids answered `fail(NV_ERR_NOT_SUPPORTED, BOARD, "<feature>: absent on this board")`: HDMI, FRL, shared generic packets, HDCP/SRM except `HDCP_CTRL`, eDP/panel/backlight, mux, power gating, MST, DP 2.x, LVDS, framelock/pinsets, Tegra/automotive and debug. Where the header names a different status, the table carries it: `NV_ERR_INVALID_ARGUMENT` for `DP_GET_EDP_DATA` on a non-eDP display and for `GET_PINSET_PEER`/`GET_PINSET_LOCKPINS`. This replaces `unanswered` with a D9 label that says why. Extend `is_display_cmd` (`display/mod.rs:1535-1537`) to the NV2080 display range and NV9072 so those stop falling through to `ctrl.rs`.
2. **No-output setters (11 ACK).** `DP_SET_WAR_FLAGS`, the VRR trio, `CONFIG_RAD_SCRATCH_REG`, lane/level/pre-emphasis overrides, `SET_FORCE_MODESWITCH_FLAGS_OVERRIDES`, plus the two Windows-only notifications (`NOTIFY_DRR_MSCG_WAR`, `SET_MEMPOOL_WAR_FOR_BLIT_TEARING`). Each is logged with its fields.
3. **Readbacks from existing models (about 10).** `HDCP_CTRL` per `cmd` (DISABLE/ABORT: OK, err 0; READ_TOPOLOGY: zeros; RENEGOTIATE/VALIDATE_LINK: OK with `err` UNSUCCESSFUL). `DP_CONFIGURE_FEC` sets the link's FEC bit. `GET_RG_STATUS` answers scan-locked for heads in a merge pair. `GET_RG_SCAN_LINE` reuses `Engine::scanline`. Lane data and test pattern come from the DP sink. `GET_HOTPLUG_STATE`, `SET_HOTPLUG_EVENT_CONFIG` and `DP_AUXCH_I2C_TRANSFER_CTRL` go through `aux.rs`. `IS_MODE_POSSIBLE_OR_SETTINGS` extends `is_mode_possible`. The static answers are `TABLE`: pinset count 0, `INTERNAL_DISP_PINSETS_TO_LOCKPINS` all FALSE, the drive tables, and adaptive-sync FALSE.
4. **New small state (4).** `NOTIFY_ON_VBLANK` queues a notify on the head's clock. At the next vblank tick, `engine.rs` asks the POST_EVENT path for `SEMAPHORE_SCHEDULE_CALLBACK` (0x100b), through the `emit_event` helper that gsp-surface.md section 4 proposes. It also covers the underflow and IMP parameter stores and the MSA/infoframe readbacks, which come from the guest-programmed head state.

Tests:

- Unit tests in `display/tests.rs` against the generated ABI modules (`ver/v610/display.rs` plus the v615 counterpart; `DP_SET_WAR_FLAGS` needs its id added to the v615 module only). They should assert one answer per id, the status and the label, and that no control in the group returns `unanswered`.
- An `HDCP_CTRL` test per `cmd` value.
- An NV9072 test: queue, tick, and exactly one event with the guest's handles.
- Gate runs: a 615 guest DP plug/unplug via the control file (`connector_command`), GNOME and Weston on vm0 as in display-cd, and later the milestone-G 4K144 DSC gate.

**Estimate.** 3.5 to 5.5 engineer-days:

- 0.5 for the refusal and ACK tables with status overrides and labels.
- 0.5 for `HDCP_CTRL` per cmd, `DP_SET_WAR_FLAGS` in v615 and `DP_CONFIGURE_FEC` into the link config.
- 1 for the rule-3 readbacks and the TABLE answers.
- 1 to 2 for NV9072 vblank notify with event emission. This assumes the generic `emit_event` helper exists; if it does not, add gsp-surface.md's 2 to 3 days for events.
- 0.5 to 1.5 for unit tests and the three gate runs.

**Client likelihood.** Group rank 1. Two controls decide it:

- `SPECIFIC_HDCP_CTRL`, already hit on every unplug and currently failing.
- `DP_SET_WAR_FLAGS`, which every 615 guest sends on every plug and unplug.

`DP_CONFIGURE_FEC` joins them at 4K144. Rank 2: `GET_RG_STATUS` (2Heads1OR), the VRR trio, `CONFIG_RAD_SCRATCH_REG` (teardown), `DP_AUXCH_I2C_TRANSFER_CTRL` (ddcutil) and NV9072 (GL). The remaining 97 are tools (22), board-absent features (53) or never on this chip (22).

**Roadmap.** Wayland/KMS display with NVENC streaming: `HDCP_CTRL`, `DP_SET_WAR_FLAGS` (at the 615 guest image move, D19) and `DP_CONFIGURE_FEC` (milestone G). Vulkan games: NV9072 if GL/Vulkan vsync turns out to use it, the VRR trio once VRR is offered (optional, display-scaling.md:359), and `GET_RG_STATUS` if 4K144 lands on 2Heads1OR. Linux CUDA client: none. Nobody on this chip: the 22 E controls.

Dependencies:

- the v615 display ABI module;
- the POST_EVENT/`emit_event` work (gsp-surface.md section 4) for NV9072;
- DSC caps and the DPCD FEC model (display-scaling.md milestone G);
- the per-head clock in `engine.rs`, which exists.

**Reclassification.** 29 of 107 move:

- **E (22), never on TU106:**
  - FRL: `SET_HDMI_FRL_CONFIG`, `SET_HDMI_FRL_FLUSH_MODE`, `GET_HDMI_FRL_CAPACITY_COMPUTATION`, `FRL_CONFIG_MACRO_PAD`. The C5 HAL has `supportsHDMIFRL` FALSE, `nvkms-evo3.c:7945`.
  - Shared generic packets (3). These belong to the C8 HAL; Turing sends SDPs through `nvEvo1SendDpInfoFrameSdp`, `nvkms-evo3.c:7927`.
  - DP 2.x: `DP2X_LINK_TRAINING_CTRL`, `DP2X_GET/SET_LANE_DATA`, `DP2X_SET/GET_LEVEL_INFO_TABLE_DATA`, `DP_GET_CABLEID_INFO_FROM_MACRO`.
  - NVC370 `SET_SOR_FLUSH_MODE` (NV Display 5.0+).
  - DSI and DP-serializer timing (Tegra).
  - Safety region RAM and interrupts (automotive).
  - LVDS `GET/SET_SOR_OP_MODE`.
  - Windows-only notifications `NOTIFY_DRR_MSCG_WAR` and `SET_MEMPOOL_WAR_FOR_BLIT_TEARING`.
- **A (5), static board data:** `GET_PINSET_COUNT` (0), `INTERNAL_DISP_PINSETS_TO_LOCKPINS` (no pins), `DP_GET_PREEMPHASIS_DRIVECURRENT_POSTCURSOR2_DATA` and `DP_GET_LEVEL_INFO_TABLE_DATA` (VBIOS drive tables), `SPECIFIC_DEFAULT_ADAPTIVESYNC_DISPLAY` (firmware list keyed by EDID IDs).
- **C (2), readbacks of guest-programmed head state:** `DP_GET_GENERIC_INFOFRAME`, `DP_GET_MSA_ATTRIBUTES`.
- **B: none.** Forwarding any display control would act on the host's scanout.

**Measurements.** All of these are guest-side except the last, which reads the host and needs the operator's go-ahead first.

1. **615 guest, DP plug/unplug** through the control file with device tracing. Expect `0x731391` once per plug and once per unplug, and `0x730282` once per unplug. With the new answers, expect no new D ids and no "setDpWarFlag failed" or "configureHDCPDisableAuthentication failed" lines, where DP-library prints reach dmesg.
2. **4K144 with DSC on one head**, run twice: once with `HEAD_CLK_CAP` above the mode's pixel clock, once below. Expect `DP_CONFIGURE_FEC` in both. Expect `GET_RG_STATUS` (merge mode, `nvkms-evo.c:9714`) only in the second, and the mode must come up.
3. **Two heads with identical modes under GNOME.** Confirm that neither `GET_RG_STATUS` nor `GET_FRAMELOCK_HEADER_LOCKPINS` appears: the opportunistic head raster lock, `nvkms-evo.c:1735-1811`, uses internal pins.
4. **GL/EGL and Vulkan clients with vsync** (eglprobe with swap interval 1, a Vulkan FIFO swapchain under Weston). Record whether `GF100_DISP_SW` is allocated and whether `0x90720101` or NV9072 methods on a channel appear; this decides NV9072's rank.
5. **Guest nvidia-drm unload with the fbdev console, then suspend/resume.** Check whether `DP_CONFIG_RAD_SCRATCH_REG` appears at teardown (`nvdp-connector.cpp:938-973`).
6. **Host RO, read-only and only after the operator's OK.** As a client on the device's own `NV04_DISPLAY_COMMON` and subdevice objects:
   - `SPECIFIC_DEFAULT_ADAPTIVESYNC_DISPLAY` with the synthetic EDID's manufacturer and product ID (expect FALSE);
   - `LPWR_DIFR_CTRL` with `SUPPORT_STATUS` (whether TU106 GSP-RM supports DIFR at all, which decides between rank 4 and E for `DIFR_PREFETCH_RESPONSE`).

**DP link and lane (32).**

Lane/drive/level tables and test patterns (`DP_GET/SET_LANE_DATA`, `DP_SET/GET_TESTPATTERN`, `*_PREEMPHASIS_*`, `DP_SET/GET_LEVEL_INFO_TABLE_DATA`, `DFP_DSC_CRC_CONTROL`, `DP_GET_AUXLOGGER_BUFFER_DATA`); MST (`TOPOLOGY_ALLOCATE/FREE_DISPLAYID`, `SET_RATE_GOV`, `SET_ECF`, `SEND_ACT`, `CONFIG_SINGLE_HEAD_MULTI_STREAM`, `SET_TRIGGER_SELECT/ALL`, `CONFIG_RAD_SCRATCH_REG`); SST link features (`DP_CONFIGURE_FEC`, `DP_CONFIG_INDEXED_LINK_RATES`, `DP_SET_WAR_FLAGS`, `DP_CONFIG_MACRO_PAD`, `DP_AUXCH_I2C_TRANSFER_CTRL`, `DP_GET_GENERIC_INFOFRAME`, `DP_GET_MSA_ATTRIBUTES`); DP 2.x (`DP2X_*` 5, `DP_GET_CABLEID_INFO_FROM_MACRO`).

Answers: `DP_SET_WAR_FLAGS` ACK (rank 1, 615 only); `DP_CONFIGURE_FEC` MODEL:dp-sink (rank 1 at 4K144 DSC); `DP_CONFIG_RAD_SCRATCH_REG` ACK (rank 2, teardown); `DP_AUXCH_I2C_TRANSFER_CTRL` MODEL:display-connector (rank 2, unreachable while `ddcPortId` is 0); lane/test-pattern/drive-table controls MODEL:dp-sink, ACK or TABLE (rank 3, tools); MST, eDP-indexed rates and macro pad REFUSE (rank 4); DP 2.x and cable ID REFUSE (rank 5, E); infoframe and MSA readbacks MODEL:display-engine (rank 3, C).

**HDMI/FRL/SCDC (13).**

`SET_HDMI_ENABLE`, `CTRL_HDMI`, `SET_HDMI_AUDIO_MUTESTREAM`, `SET_OD_PACKET_CTRL`, `SET_HDMI_SINK_CAPS`, `GET_HDMI_SCDC_DATA`, FRL (`SET_HDMI_FRL_CONFIG`, `SET_HDMI_FRL_FLUSH_MODE`, `GET_HDMI_FRL_CAPACITY_COMPUTATION`, `FRL_CONFIG_MACRO_PAD`), shared generic packets (`SET/ACQUIRE/RELEASE_SHARED_GENERIC_PACKET`).

Answers: all REFUSE. HDMI enable, packet, audio, sink caps and SCDC are rank 4 (no HDMI connector on the board, D15). FRL (4) and shared generic packets (3) are rank 5, E.

**HDCP/SRM (8).**

`SPECIFIC_HDCP_CTRL`, `GET_HDCP_DIAGNOSTICS`, `HDCP_KSVLIST_VALIDATE`, `HDCP_UPDATE`, `SYSTEM_VALIDATE_SRM`, `GET_SRM_STATUS`, `HDCP_REVOCATION_CHECK`, `UPDATE_SRM`.

Answers: `SPECIFIC_HDCP_CTRL` MODEL:display-connector per `cmd` (rank 1, every unplug); the other seven REFUSE (rank 4, no HDCP on the emulated connector).

**DFP/mux/eDP/backlight/power gating (24).**

Mux (`INTERNAL_DFP_GET_DISP_MUX_STATUS`, `INTERNAL_DFP_SWITCH_DISP_MUX`, `DFP_INIT_MUX_DATA`, `DFP_RUN_PRE/POST_DISP_MUX_OPERATIONS`, `SYSTEM_CHECK_SIDEBAND_I2C/SR_SUPPORT`); panel/eDP (`SET_BACKLIGHT_BRIGHTNESS`, `DP_GET_EDP_DATA`, `DP_ASSR_CTRL`, `DP_AUXCH_OD_CTRL`, `DP_AUXCH_VBL_CTRL`, `DFP_EDP_DRIVER_UNLOAD`, `DFP_GET_LCD_GPIO_PIN_NUM`, `DFP_GET_SPREAD_SPECTRUM`, `GET_REGIONAL_CRCS`, `GET_ACPI_DOD_DISPLAY_PORT_ATTACHMENT`); power gating (`DFP_ENTER/EXIT_DISPLAY_POWER_GATING`); Tegra/automotive (`DFP_GET_DSI_MODE_TIMING`, `DFP_GET_FIXED_MODE_TIMING`, `SYSTEM_SET_REGION_RAM_RECTANGLES`, `SYSTEM_CONFIGURE_SAFETY_INTERRUPTS`); `DFP_CONFIG_TWO_HEAD_ONE_OR`.

Answers: REFUSE throughout (`DP_GET_EDP_DATA` with NV_ERR_INVALID_ARGUMENT). Rank 4 for panel, mux, eDP and power gating; rank 3 for `DFP_DSC_CRC_CONTROL` and `GET_REGIONAL_CRCS`; rank 5, E, for DSI, the DP serializer and the safety region RAM and interrupts.

**System/hotplug/supervisor/VRR (11).**

`SYSTEM_GET_HOTPLUG_STATE`, `SET_HOTPLUG_EVENT_CONFIG`; VRR (`CLEAR_ELV_BLOCK`, `ARM_LIGHTWEIGHT_SUPERVISOR`, `CONFIG_VRR_PSTATE_SWITCH`, `SPECIFIC_DEFAULT_ADAPTIVESYNC_DISPLAY`, `DP_ENABLE_VRR`); `NOTIFY_DRR_MSCG_WAR`, `LPWR_DIFR_PREFETCH_RESPONSE`; debug (`RECORD_CHANNEL_REGS`, `SPECIFIC_FAKE_DEVICE`).

Answers: the VRR trio ACK (rank 2, optional DP adaptive sync); hotplug state and event config MODEL:display-connector (rank 3); `DEFAULT_ADAPTIVESYNC_DISPLAY` TABLE FALSE (rank 4, A); `DP_ENABLE_VRR` (G-SYNC module) and `DIFR_PREFETCH_RESPONSE` REFUSE (rank 4); debug controls REFUSE (rank 3); `NOTIFY_DRR_MSCG_WAR` ACK (rank 5, E).

**NV5070 RG/lockpin/SOR (13).**

`GET_RG_STATUS`, `GET_RG_SCAN_LINE`, `GET/SET_RG_UNDERFLOW_PROP`, `GET/SET_RG_FLIPLOCK_PROP`, `GET_FRAMELOCK_HEADER_LOCKPINS`, pinsets (`GET_PINSET_COUNT`, `GET_PINSET_PEER`, `GET_PINSET_LOCKPINS`, NV2080 `INTERNAL_DISP_PINSETS_TO_LOCKPINS`), LVDS (`GET/SET_SOR_OP_MODE`).

Answers: `GET_RG_STATUS` MODEL:display-engine (rank 2, 2Heads1OR); `GET_RG_SCAN_LINE` and the underflow properties MODEL:display-engine (rank 3); pinset getters TABLE or REFUSE with INVALID_ARGUMENT (rank 4, two A); fliplock and framelock-header REFUSE (rank 4); LVDS SOR op mode REFUSE (rank 5, E).

**IMP/C370/C372 (5).**

NV5070 `IMP_SET_GET_PARAMETER`, `SET_MEMPOOL_WAR_FOR_BLIT_TEARING`; NVC370 `SET_SOR_FLUSH_MODE`, `SET_FORCE_MODESWITCH_FLAGS_OVERRIDES`; NVC372 `IS_MODE_POSSIBLE_OR_SETTINGS`.

Answers: `IMP_SET_GET_PARAMETER` MODEL:display-imp (rank 3); `SET_FORCE_MODESWITCH_FLAGS_OVERRIDES` ACK (rank 3); `IS_MODE_POSSIBLE_OR_SETTINGS` MODEL:display-imp (rank 4, non-DP ORs only); `SET_MEMPOOL_WAR_FOR_BLIT_TEARING` ACK (rank 5, E); NVC370 `SET_SOR_FLUSH_MODE` REFUSE (rank 5, E).

**vblank sw object (1)

NV9072 `NOTIFY_ON_VBLANK`.

Answer: MODEL:display-engine, new. A pending notify per DispSw fires at the next vblank tick of its head as a `SEMAPHORE_SCHEDULE_CALLBACK` event. Rank 2 (closed GL/EGL user space); whether any client uses the control rather than the channel method is measurement 4.

### 6.6 Perf, clocks and power (21)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV2080_CTRL_CMD_GPU_SET_POWER` | 0x20800112 | 12/12 | user | DENY | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs |
| `NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_DEPENDENCY_CHECK` | 0x20800A45 | 0/0 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/platform/platform_request_handler.c:116-121 (pfmreqhndlrInitGpu, NV_ASSERT_OK) from pfmreqhndlrStateInit, from kperfStateInitLocked (gpu/perf/kern_perf.c:71); sent only after pfmreqhndlrInitGpu finds the ACPI GPS/GPS_2X _DSM on the GPU (615 src/nvidia/src/kernel/platform/platform_request_handler.c:96-104); a QEMU guest has no such _DSM, so never sent |
| `NV2080_CTRL_CMD_INTERNAL_PMGR_UNSET_DYNAMIC_BOOST_LIMIT` | 0x20800A7B | 0/0 | internal | ACK | 4 | none | 615 src/nvidia/src/kernel/gpu/subdevice/subdevice.c:141-160 (subdeviceUnsetDynamicBoostLimit) from subdevicePreDestruct :168, only if bUpdateTGP was set by NV2080_CTRL_CMD_OS_UNIX_UPDATE_TGP_STATUS from nvidia-powerd (arch/nvalloc/unix/src/subdevice_ctrl_os_unix.c:89-95) |
| `NV2080_CTRL_CMD_INTERNAL_PERF_GPU_BOOST_SYNC_SET_CONTROL` | 0x20800A7E | 1/1 | internal | REFUSE | 4 | chip table | 615 src/nvidia/src/kernel/gpu/perf/kern_perf_gpuboostsync.c:88-106 (kperfGpuBoostSyncActivate) from kperfGpuBoostSyncStateUpdate :344/:374 on SYNC_GPU_BOOST_GROUP_CREATE/DESTROY (power/gpu_boost_mgr.c:430,484), only if GPU_BOOST_SYNC_GET_INFO reported bSliGpuBoostSyncEnable; the device answers that with zeros (crates/vfio-user-nvgpu/src/ctrl.rs:1120) |
| `NV2080_CTRL_CMD_INTERNAL_PERF_GPU_BOOST_SYNC_SET_LIMITS` | 0x20800A7F | 16/16 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/gpu/perf/kern_perf_gpuboostsync.c:252-262 (kperfDoSyncGpuBoostLimits) from the GSP event PERF_GPU_BOOST_SYNC_LIMITS_CALLBACK (gpu/gsp/kernel_gsp.c:977), only while a boost group is active |
| `NV2080_CTRL_CMD_INTERNAL_PERF_GET_AUX_POWER_STATE` | 0x20800A81 | 4/4 | internal | MODEL:perf-power-source | 4 | pure emulation | 615 src/nvidia/src/kernel/gpu/perf/kern_perf_pwr.c:155-196 (_kperfSendPostPowerStateCallback) after SET_POWERSTATE/SET_AUX_POWER_STATE (:231, :281), only if SBIOS registered the POSTPOWERSTATE ACPI callback (platform/acpi_common.c:902,960); none in a QEMU guest |
| `NV2080_CTRL_CMD_INTERNAL_SMBPBI_PFM_REQ_HNDLR_CAP_UPDATE` | 0x20800ACB | 2/2 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/platform/platform_request_handler.c:436-449 (pfmreqhndlrStateInit); sent only after pfmreqhndlrInitGpu finds the ACPI GPS/GPS_2X _DSM on the GPU (615 src/nvidia/src/kernel/platform/platform_request_handler.c:96-104); a QEMU guest has no such _DSM, so never sent |
| `NV2080_CTRL_CMD_INTERNAL_PMGR_PFM_REQ_HNDLR_STATE_LOAD_SYNC` | 0x20800ACC | 16/16 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/platform/platform_request_handler.c:495-507 (pfmreqhndlrStateInit, PMGR load sync); sent only after pfmreqhndlrInitGpu finds the ACPI GPS/GPS_2X _DSM on the GPU (615 src/nvidia/src/kernel/platform/platform_request_handler.c:96-104); a QEMU guest has no such _DSM, so never sent |
| `NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_GET_PM1_STATE` | 0x20800ACE | 2/2 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/platform/platform_request_handler.c:759-782 (_pfmreqhndlrCheckAndGetPM1ForcedOffState at state init) and platform_request_handler_ctrl.c:1570-1575 (pfmreqhndlrHandleCheckPM1Available); sent only after pfmreqhndlrInitGpu finds the ACPI GPS/GPS_2X _DSM on the GPU (615 src/nvidia/src/kernel/platform/platform_request_handler.c:96-104); a QEMU guest has no such _DSM, so never sent |
| `NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_SET_PM1_STATE` | 0x20800ACF | 1/1 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/platform/platform_request_handler_ctrl.c:2069-2090 (_pfmreqhndlrEnablePM1) on an SBIOS PM1 request; sent only after pfmreqhndlrInitGpu finds the ACPI GPS/GPS_2X _DSM on the GPU (615 src/nvidia/src/kernel/platform/platform_request_handler.c:96-104); a QEMU guest has no such _DSM, so never sent |
| `NV2080_CTRL_CMD_INTERNAL_PMGR_PFM_REQ_HNDLR_UPDATE_EDPP_LIMIT` | 0x20800AD0 | 8/8 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/platform/platform_request_handler_ctrl.c:1613-1618 (pfmreqhndlrHandleEdppeakLimitUpdate) and :1670-1675 (pfmreqhndlrHandlePlatformEdppLimitUpdate) on SBIOS EDPp requests; sent only after pfmreqhndlrInitGpu finds the ACPI GPS/GPS_2X _DSM on the GPU (615 src/nvidia/src/kernel/platform/platform_request_handler.c:96-104); a QEMU guest has no such _DSM, so never sent |
| `NV2080_CTRL_CMD_INTERNAL_PMGR_PFM_REQ_HNDLR_CONFIGURE_TGP_MODE` | 0x20800AD2 | 1/1 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/platform/platform_request_handler_ctrl.c:1818-1840 (pfmreqhndlrHandleUserConfigurableTgpMode); sent only after pfmreqhndlrInitGpu finds the ACPI GPS/GPS_2X _DSM on the GPU (615 src/nvidia/src/kernel/platform/platform_request_handler.c:96-104); a QEMU guest has no such _DSM, so never sent |
| `NV2080_CTRL_CMD_INTERNAL_PMGR_PFM_REQ_HNDLR_CONFIGURE_TURBO_V2` | 0x20800AD3 | 4/4 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/platform/platform_request_handler_ctrl.c:2770-2780 (_pfmreqhndlrCallPshareStatus, NVPCF turbo v2); sent only after pfmreqhndlrInitGpu finds the ACPI GPS/GPS_2X _DSM on the GPU (615 src/nvidia/src/kernel/platform/platform_request_handler.c:96-104); a QEMU guest has no such _DSM, so never sent |
| `NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_GET_VPSTATE_INFO` | 0x20800AD4 | 8/8 | internal | REFUSE | 4 | host RO | 615 src/nvidia/src/kernel/platform/platform_request_handler.c:472-491 (pfmreqhndlrStateInit, vPstate cache); sent only after pfmreqhndlrInitGpu finds the ACPI GPS/GPS_2X _DSM on the GPU (615 src/nvidia/src/kernel/platform/platform_request_handler.c:96-104); a QEMU guest has no such _DSM, so never sent |
| `NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_GET_VPSTATE_MAPPING` | 0x20800AD5 | 8/8 | internal | REFUSE | 4 | host RO | 615 src/nvidia/src/kernel/platform/platform_request_handler.c:270-283 (pfmreqhndlrPcontrol) on an SBIOS P-control request; sent only after pfmreqhndlrInitGpu finds the ACPI GPS/GPS_2X _DSM on the GPU (615 src/nvidia/src/kernel/platform/platform_request_handler.c:96-104); a QEMU guest has no such _DSM, so never sent |
| `NV2080_CTRL_CMD_INTERNAL_PERF_PFM_REQ_HNDLR_SET_VPSTATE` | 0x20800AD6 | 4/4 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/platform/platform_request_handler.c:308-320 (pfmreqhndlrPcontrol); sent only after pfmreqhndlrInitGpu finds the ACPI GPS/GPS_2X _DSM on the GPU (615 src/nvidia/src/kernel/platform/platform_request_handler.c:96-104); a QEMU guest has no such _DSM, so never sent |
| `NV2080_CTRL_CMD_INTERNAL_PERF_CLIENT_PERF_MODES_SET_CONTROL` (615 only) | 0x20800B20 | -/32 | internal | REFUSE | 4 | host RO | 615 src/nvidia/src/kernel/gpu/perf/wpps_config_session.c:88-103 (NVD0CD_CTRL_CMD_SET_PROFILES on a WPPS_CONFIG_SESSION 0xd0cd allocated with the wpps-access capability, arch/nvalloc/unix/src/os.c:5430); absent in 610 |
| `NV2080_CTRL_CMD_PERF_SET_POWERSTATE` | 0x2080205B | 4/4 | user | ACK | 4 | pure emulation | 615 src/nvidia/arch/nvalloc/unix/src/osapi.c:1310-1325 (RmPowerSourceChangeEvent; 610 :1285) from RmInitAdapter only if b_mobile_config_enabled (osinit.c:2343-2355, set from GPU_GET_INFO_V2 MOBILE_CONFIG at :678), on resume only if IsMobile (dynamic-power.c:2373-2381), and on ACPI _PSR notifications (kernel-open/nvidia/nv-acpi.c:129-144); kernel wrapper kern_perf_pwr.c:240-292 forwards to physical |
| `NV2080_CTRL_CMD_PERF_RATED_TDP_SET_CONTROL` | 0x2080206F | 12/12 | user | MODEL:rated-tdp-requests | 3 | guest state | 615 src/nvidia/src/kernel/gpu/perf/kern_perf_pwr.c:54-148 (subdeviceCtrlCmdPerfRatedTdpSetControl_KERNEL; 610 :53-110): checks admin/profiling rights for TURBO_BOOST, then NV_RM_RPC_CONTROL with the caller's handles; no in-tree caller, user space closed (profilers locking clocks, app profiles) |
| `NV2080_CTRL_CMD_PERF_SET_AUX_POWER_STATE` | 0x20802092 | 4/4 | user | ACK | 4 | pure emulation | 615 src/nvidia/arch/nvalloc/unix/src/osapi.c:1370-1422 (RmHandleDNotifierEvent; 610 :1371) on ACPI D-notifier events D1-D5 (osapi.c:2187-2192); kernel wrapper kern_perf_pwr.c:202-234 forwards, NV_ASSERTs on failure (:228), then notifies AUX_POWER_EVENT |
| `NV2080_CTRL_CMD_INTERNAL_GCX_ENTRY_PREREQUISITE` | 0x2080A7D7 | 2/2 | internal | REFUSE | 4 | none | 615 src/nvidia/arch/nvalloc/unix/src/dynamic-power.c:1401-1427 (RmCheckForGcxSupportOnCurrentState; 610 :1398) from RmCanEnterGcxUnderGpuLock :323 during RTD3 idle checks; RTD3 stays off unless static info sets bIsGc6Rtd3Allowed/bIsGcOffRtd3Allowed (gpu/gsp/kernel_gsp.c:7435-7440, dynamic-power.c:2056-2085), which the device sends as 0 (docs/caraxes/gsp-abi-610.md:759) |

**Subfamilies.** 21 controls. (1) Platform request handler (PRH, notebook SBIOS power arbitration over ACPI GPS/NVPCF), 11: INTERNAL_PERF_PFM_REQ_HNDLR_DEPENDENCY_CHECK, _GET_PM1_STATE, _SET_PM1_STATE, _GET_VPSTATE_INFO, _GET_VPSTATE_MAPPING, _SET_VPSTATE, INTERNAL_SMBPBI_PFM_REQ_HNDLR_CAP_UPDATE, INTERNAL_PMGR_PFM_REQ_HNDLR_STATE_LOAD_SYNC, _UPDATE_EDPP_LIMIT, _CONFIGURE_TGP_MODE, _CONFIGURE_TURBO_V2. (2) Power-source and aux-power (AC/DC, ACPI D-notifiers), 3: PERF_SET_POWERSTATE, PERF_SET_AUX_POWER_STATE, INTERNAL_PERF_GET_AUX_POWER_STATE. (3) SLI GPU boost sync, 2: INTERNAL_PERF_GPU_BOOST_SYNC_SET_CONTROL, _SET_LIMITS. (4) Notebook runtime power, 2: INTERNAL_PMGR_UNSET_DYNAMIC_BOOST_LIMIT (nvidia-powerd), INTERNAL_GCX_ENTRY_PREREQUISITE (RTD3 GC6/GCOFF). (5) Client clock/profile requests, 2: PERF_RATED_TDP_SET_CONTROL, INTERNAL_PERF_CLIENT_PERF_MODES_SET_CONTROL (WPPS, 615 only). (6) Legacy setter, 1: GPU_SET_POWER.

**State a device model needs.** Almost none. `perf-power-source`: one u32 power source (AC/battery) and one u32 aux state (P0..P4), per device, reset on guest driver reload (the kernel re-sends at RmInitAdapter only on a mobile config, `615 src/nvidia/arch/nvalloc/unix/src/osinit.c:2343-2355`). `rated-tdp-requests`: 5 client slots of {action, vPstateType} (`ctrl2080perf.h:212-255`), per device, cleared at guest driver reload; the invariant is that nothing in it is ever applied to the host. No per-client lifetime is needed: RATED_TDP slots are per arbitration client, not per RM client.

**What GSP-RM does.** All 21 are ROUTE_TO_PHYSICAL with the physical body in closed firmware. From the headers: PRH controls sync the PMGR/THERM/PERF PMU state with SBIOS requests (limits, PM1 boost, VPstates, cTGP, EDPp; `ctrl2080internal.h:3029-3440`); SET_POWERSTATE switches the perf policy between AC and battery (`ctrl2080perf.h:152-166`, 610 :148-162); SET_AUX_POWER_STATE forces a perf cap from the platform Dx level (`ctrl2080perf.h:175-189`); RATED_TDP arbitrates boost above the rated TDP point, FORCE_LOCK "used for achieving stable clocks required for profiling" (`ctrl2080perf.h:284-287`); GCX_ENTRY_PREREQUISITE reports "P8 and engine idle" for RTD3 (`ctrl2080internal.h:3386-3398`); boost sync applies common pstate/gpcclk limits across an SLI group (`ctrl2080internal.h:1711-1746`). Three have a kernel-side wrapper that validates and forwards: `kern_perf_pwr.c:54-148` (RATED_TDP, admin/profiling check, `NV_RM_RPC_CONTROL` with the caller's handles), `:202-234` (aux power, NV_ASSERT on failure at :228), `:240-292` (power source, emits NV2080_NOTIFIERS_POWER_EVENT itself at :289).

**Who calls it and when.** Kernel RM, all gated on notebook platform facts the device does not present. PRH: `kperfStateInitLocked` (`gpu/perf/kern_perf.c:71`) calls `pfmreqhndlrStateInit`, which stops unless `testIfDsmSubFunctionEnabled(... GPS_2X/GPS ...)` succeeds (`platform/platform_request_handler.c:96-104`); a QEMU guest has no GPS _DSM on the GPU. SET_POWERSTATE: at init only if `b_mobile_config_enabled` (`osinit.c:678`, `:2343`), on resume only if `IsMobile` (`dynamic-power.c:2373-2381`), and on ACPI _PSR notifications (`kernel-open/nvidia/nv-acpi.c:129-144`). SET_AUX_POWER_STATE: ACPI D1-D5 notifications (`osapi.c:2187-2192`). GET_AUX_POWER_STATE: only when SBIOS registered the POSTPOWERSTATE callback (`platform/acpi_common.c:960`). GCX: RTD3 idle checks (`dynamic-power.c:1401-1427`), off because static info `bIsGc6Rtd3Allowed/bIsGcOffRtd3Allowed` (`gpu/gsp/kernel_gsp.c:7435-7440`) is 0 on a desktop card (`docs/caraxes/gsp-abi-610.md:759`). Boost sync: `kern_perf_gpuboostsync.c:88-106/252-262`, only with `bSliGpuBoostSyncEnable`, which the device answers as zero (`crates/vfio-user-nvgpu/src/ctrl.rs:1120`). Dynamic boost: subdevice pre-destruct after nvidia-powerd set `bUpdateTGP` (`gpu/subdevice/subdevice.c:141-168`, `arch/nvalloc/unix/src/subdevice_ctrl_os_unix.c:89-95`). User space: RATED_TDP (profilers, app profiles), WPPS (`gpu/perf/wpps_config_session.c:88-103`, wpps-access capability), GPU_SET_POWER: closed/unknown. None appears in the 79 native traces or any Caraxes run. Suspend/resume itself goes through GMCAPI_GSP_SUSPEND/RESUME_DONE (`gmcapi.rs` CMD_GSP_SUSPEND), not through these controls.

**Truth source.** None for 16 (feature absent). Host RO would tell whether RATED_TDP and WPPS are supported on TU106 (the GET counterparts); the setters themselves are pure emulation.

**Isolation risk.** Every setter here changes host clocks, power limits, thermal targets or perf policy if forwarded; the host desktop shares that GPU. A wrong ACK is harmless; a forward is not. RATED_TDP FORCE_LOCK would pin host clocks for the whole host; GPU_SET_POWER could drop the host GPU's power level. Rule: no control of this group is ever forwarded.

**Design proposal.** New `crates/vfio-user-nvgpu/src/power.rs`: (a) a const table `(id, Answer kind, status, label)` consumed by `ctrl.rs` before the `unanswered` fallback (`ctrl.rs:1170-1175`): PRH, boost sync, GCX, WPPS -> REFUSE NV_ERR_NOT_SUPPORTED labelled "absent on this board"; GPU_SET_POWER -> DENY; SET_POWERSTATE, SET_AUX_POWER_STATE, UNSET_DYNAMIC_BOOST_LIMIT -> ACK "guest (recorded; not applied)" (ACK because the kernel asserts or ignores status); (b) `PerfPowerSource` answering GET_AUX_POWER_STATE; (c) `RatedTdp` slots answering SET_CONTROL and GET_CONTROL 0x2080206e consistently. Tests: unit tests against `ver/v610.rs`/`ver/v615.rs` ids and param sizes (4, 12, 32 bytes); a test that no id in the table reaches `forward_or`; 0x20800B20 only in the 615 table. Gate: existing boot gates for 610 and 615 guests unchanged; one ncu run for RATED_TDP.

**Estimate.** 1 to 2 engineer-days: table and dispatch 0.5, two small models 0.5 to 1, tests 0.5.

**Client likelihood.** Group rank 3, set by PERF_RATED_TDP_SET_CONTROL (profilers locking clocks, possibly GL/Vulkan app profiles). Everything else is rank 4 (notebook, SLI, RTD3).

**Roadmap.** Nobody on this chip for 18 of 21; RATED_TDP belongs to CUDA tooling (ncu) after the Linux CUDA client milestone. Independent of other groups; pairs with the NVML host-forward design (`docs/caraxes/nvml.md:227-236`), which already keeps clocks and power as shared facts read-only.

**Reclassification.** E (18): all 11 PRH controls, SET_POWERSTATE, SET_AUX_POWER_STATE, GET_AUX_POWER_STATE (notebook ACPI/SBIOS), both boost-sync controls (SLI), UNSET_DYNAMIC_BOOST_LIMIT (nvidia-powerd), GCX_ENTRY_PREREQUISITE (RTD3). Stay D: RATED_TDP_SET_CONTROL (small model), CLIENT_PERF_MODES_SET_CONTROL (pending measurement), GPU_SET_POWER (DENY).

**Measurements.** (1) Host RO: NV2080_CTRL_CMD_PERF_RATED_TDP_GET_CONTROL 0x2080206e (`ctrl2080perf.h:403`) for each of the 5 client slots on the device's host subdevice: supported on TU106 or NOT_SUPPORTED, and the default slot values; decides MODEL vs REFUSE for 0x2080206F. (2) Guest: `ncu --clock-control base` and `--clock-control none` on a small kernel; look for 0x2080206F in device-trace.txt and ncu's reaction to NV_ERR_NOT_SUPPORTED. (3) Guest 615: check `/proc/driver/nvidia/capabilities/wpps-access` and run nvidia-smi's workload power profile query; look for a 0xd0cd alloc and 0x20800B20. Host: the same NVML read-only query under the matrix trace tooling to see whether GSP-RM on TU106 reports any profile. (4) Host RO: GPU_GET_INFO_V2 with MOBILE_CONFIG_ENABLED to confirm the desktop value the device must mirror.

### 6.7 Thermal and fan (2)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV2080_CTRL_CMD_INTERNAL_THERM_PFM_REQ_HNDLR_STATE_INIT_SYNC` | 0x20800ACD | 16/16 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/platform/platform_request_handler.c:510-523 (pfmreqhndlrStateInit, GPS_2X only); sent only after pfmreqhndlrInitGpu finds the ACPI GPS/GPS_2X _DSM on the GPU (615 src/nvidia/src/kernel/platform/platform_request_handler.c:96-104); a QEMU guest has no such _DSM, so never sent |
| `NV2080_CTRL_CMD_INTERNAL_THERM_PFM_REQ_HNDLR_UPDATE_TGPU_LIMIT` | 0x20800AD1 | 4/4 | internal | REFUSE | 4 | none | 615 src/nvidia/src/kernel/platform/platform_request_handler.c:589-600 (pfmreqhndlrStateLoad), :838-848 (THERM post-init work item) and platform_request_handler_ctrl.c:2157-2205 (_pfmreqhndlrUpdateTgpuLimit); sent only after pfmreqhndlrInitGpu finds the ACPI GPS/GPS_2X _DSM on the GPU (615 src/nvidia/src/kernel/platform/platform_request_handler.c:96-104); a QEMU guest has no such _DSM, so never sent |

**Subfamilies.** 2 controls, both platform request handler syncs: INTERNAL_THERM_PFM_REQ_HNDLR_STATE_INIT_SYNC and INTERNAL_THERM_PFM_REQ_HNDLR_UPDATE_TGPU_LIMIT. The thermal getters are not here: they are host-forwarded GSS-legacy ids (`docs/caraxes/gsp-surface.md:142`, `docs/caraxes/nvml.md:72-73`).

**State a device model needs.** None.

**What GSP-RM does.** Closed firmware. STATE_INIT_SYNC syncs PRH init with the THERM PMU (`ctrl2080internal.h:3133-3145`, 610 :3136-3148); UPDATE_TGPU_LIMIT sets "Target Temperature Set from SBIOS" (`ctrl2080internal.h:3240-3253`).

**Who calls it and when.** Kernel RM only through the PRH: STATE_INIT_SYNC in `pfmreqhndlrStateInit` for GPS_2X (`platform/platform_request_handler.c:510-523`); UPDATE_TGPU_LIMIT in `pfmreqhndlrStateLoad` (:589-600), the THERM post-init work item (:838-848) and `_pfmreqhndlrUpdateTgpuLimit` (`platform_request_handler_ctrl.c:2157-2205`). All need the ACPI GPS _DSM (`platform_request_handler.c:96-104`), which a QEMU guest lacks. No user-space caller, no trace hit.

**Truth source.** None.

**Isolation risk.** Forwarding UPDATE_TGPU_LIMIT would set the host GPU's thermal target, which throttles the host desktop. Refusing is harmless.

**Design proposal.** Two rows in the `power.rs` REFUSE table (NV_ERR_NOT_SUPPORTED, label "absent on this board"). A unit test on both ids.

**Estimate.** 0.1 engineer-days, included in the perf table work.

**Client likelihood.** Rank 4 (notebook platform feature); neither control decides more.

**Roadmap.** Nobody on this chip. No dependencies.

**Reclassification.** E (2): both are notebook PRH controls, never sent on a desktop board or under QEMU ACPI.

**Measurements.** Optional: in a guest, confirm that no GPU _DSM with the GPS GUID exists (`acpidump` of the guest, or NVRM PRH messages at ResmanDebugLevel), which makes the whole PRH family unreachable. No host read is needed.

### 6.8 ECC, RAS and RC recovery (15)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV0080_CTRL_CMD_INTERNAL_FIFO_RC_AND_PERMANENTLY_DISABLE_CHANNELS` | 0x00802008 | 804/804 | kernel | REFUSE | 5 | none | 615 src/nvidia/src/kernel/compute/imex_session_api.c:53-69 (_performRcAndDisableChannels), called at :216/:224 when an IMEX session drops importer clients (multi-node NVLink fabric memory) |
| `NV2080_CTRL_CMD_GPU_SET_ECC_CONFIGURATION` | 0x20800134 | 4/4 | admin | REFUSE | 4 | host RO | no caller in the tree; nvidia-smi -e would send it only after an ECC-supported query, which reads N/A on this board (docs/caraxes/nvml.md:58,61) |
| `NV2080_CTRL_CMD_GPU_RESET_ECC_ERROR_STATUS` | 0x20800136 | 8/8 | admin | REFUSE | 4 | host RO | no caller in the tree; nvidia-smi -p 0/1 after an ECC-supported query |
| `NV2080_CTRL_CMD_INTERNAL_RECOVER_ALL_COMPUTE_CONTEXTS` | 0x20800A4A | 0/0 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/compute/fm_session_api.c:60-65 (_clearOutstandingComputeChannels), from _clearFmState :93 when nvidia-fabricmanager goes away with NV000F_FLAGS_CHANNEL_RECOVERY |
| `NV2080_CTRL_CMD_INTERNAL_LOG_OOB_XID` | 0x20800A56 | 92/92 | internal | ACK | 2 | pure emulation | 615 src/nvidia/src/kernel/gpu/gpu.c:7555-7614 (gpuLogOobXidMessage_KERNEL; 610 send at gpu.c:7699) from nvErrorLog2 (src/kernel/os/os_init.c:550) for every Xid the guest kernel logs itself: Xid 154 recovery action (gpu.c:7386), Xid 10 vblank timeout (gpu/rc/kernel_rc_watchdog_callback.c:447); not for GSP-reported Xids (kernel_gsp.c:883 passes oobLogging false) nor GSP_ERROR/RPC_TIMEOUT/ECC/fallen-off-bus (gpu.c:7584-7597) |
| `NV2080_CTRL_CMD_INTERNAL_RC_WATCHDOG_TIMEOUT` | 0x20800A6A | 0/0 | internal | MODEL:channel-registry | 2 | guest state | 615 src/nvidia/src/kernel/gpu/rc/kernel_rc_watchdog_callback.c:357-374 (krcWatchdogRecovery_KERNEL; 610 :373) from krcWatchdog :290 when the watchdog channel's notifier stays unwritten past timeoutSecs; today never, the device boots the guest with RmRcWatchdog=0 (crates/vfio-user-nvgpu/src/gmcapi.rs:790) |
| `NV2080_CTRL_CMD_RC_SET_CLEAN_ERROR_HISTORY` | 0x20802207 | 0/0 | admin | ACK | 3 | guest state | no caller in the tree; user space closed |
| `NV2080_CTRL_CMD_SET_RC_RECOVERY` | 0x2080220D | 4/4 | admin | REFUSE | 3 | none | no caller in the tree; user space closed (debug tools disable RC recovery to keep a faulted context) |
| `NV2080_CTRL_CMD_SET_RC_INFO` | 0x20802211 | 8/8 | admin | REFUSE | 3 | none | no caller in the tree; user space closed |
| `NV2080_CTRL_CMD_SET_RC_WATCHDOG_INFO` | 0x20802214 | 24/24 | admin | REFUSE | 3 | none | no caller in the tree; the header says 'supported only on MODS' |
| `NV208F_CTRL_CMD_FB_ECC_SET_KILL_PTR` | 0x208F050E | 24/24 | user | REFUSE | 3 | host RO | no caller in the tree; diag class NV20_SUBDEVICE_DIAG (field diag/MODS) |
| `NV208F_CTRL_CMD_FB_ECC_SET_WRITE_KILL` | 0x208F0511 | 16/16 | user | REFUSE | 3 | host RO | no caller in the tree; diag class |
| `NV208F_CTRL_CMD_FB_CLEAR_REMAPPED_ROWS` | 0x208F0515 | 8/8 | admin | REFUSE | 3 | host RO | no caller in the tree; diag class |
| `NV208F_CTRL_CMD_FB_CLEAR_REMAPPED_BANKS` | 0x208F051B | 8/8 | admin | REFUSE | 3 | host RO | no caller in the tree; diag class |
| `NV208F_CTRL_CMD_GPU_SET_RAM_SVOP_VALUES` | 0x208F1102 | 16/16 | user | DENY | 3 | none | no caller in the tree; diag class |

**Subfamilies.** 15 controls. (1) ECC config, 2: GPU_SET_ECC_CONFIGURATION, GPU_RESET_ECC_ERROR_STATUS. (2) NV208F diag memory fault injection and InfoROM repair, 4: FB_ECC_SET_KILL_PTR, FB_ECC_SET_WRITE_KILL, FB_CLEAR_REMAPPED_ROWS, FB_CLEAR_REMAPPED_BANKS. (3) NV208F SRAM timing, 1: GPU_SET_RAM_SVOP_VALUES. (4) RC policy setters, 4: SET_RC_RECOVERY, SET_RC_INFO, SET_RC_WATCHDOG_INFO, RC_SET_CLEAN_ERROR_HISTORY. (5) Kernel RC/Xid internals, 2: INTERNAL_RC_WATCHDOG_TIMEOUT, INTERNAL_LOG_OOB_XID. (6) Fabric recovery, 2: INTERNAL_RECOVER_ALL_COMPUTE_CONTEXTS (fabric manager), NV0080 INTERNAL_FIFO_RC_AND_PERMANENTLY_DISABLE_CHANNELS (IMEX).

**State a device model needs.** Only the RC side. Watchdog recovery reuses the channel registry and mirror (`crates/vfio-user-nvgpu/src/mirror.rs`, `channel.rs`): per guest channel its last GPGet and whether its host mirror saw an RC on the host error notifier (`docs/caraxes/execution.md:114-118`). An RC journal (per device, cleared by RC_SET_CLEAN_ERROR_HISTORY and on guest driver reload) becomes needed once the device emits RC_TRIGGERED/OS_ERROR_LOG (`docs/caraxes/gsp-surface.md:459-469`). LOG_OOB_XID needs no state beyond the device log.

**What GSP-RM does.** Closed firmware for all 15 (no kernel-side body). RC_WATCHDOG_TIMEOUT: "Invoke RC recovery after watchdog timeout is hit" (`ctrl2080internal.h:1660-1665`, 610 :1667-1672); inferred: find the hung channel/engine, RC it, report RC_TRIGGERED and the Xid. LOG_OOB_XID: "Log an XID message to OOB" (`ctrl2080internal.h:4722-4741`, 610 :4698-4717). SET_RC_RECOVERY/SET_RC_INFO toggle robust-channel recovery and break-on-RC (`ctrl2080rc.h:242-258`, `:342-367`); SET_RC_WATCHDOG_INFO is "supported only on MODS" (`ctrl2080rc.h:389-405`). ECC and remap controls write the InfoROM or DRAM ECC logic (`ctrl2080gpu.h:1350-1397`, `ctrl208f/ctrl208ffb.h:320-351, 434-459, 611-629, 792-810`); on a GeForce TU106 they are NOT_SUPPORTED, as the host already shows for the ECC queries (`docs/caraxes/nvml.md:58,61`). SVOP programs SRAM timing (`ctrl208f/ctrl208fgpu.h:68-89`).

**Who calls it and when.** RC_WATCHDOG_TIMEOUT: `krcWatchdogRecovery_KERNEL` (`gpu/rc/kernel_rc_watchdog_callback.c:357-374`, 610 :373) from `krcWatchdog` :290 once the watchdog channel's notifier stays unwritten past timeoutSecs; today the guest boots with `RmRcWatchdog=0` (`crates/vfio-user-nvgpu/src/gmcapi.rs:790`), which the CUDA plan wants to lift (`docs/caraxes/cuda-path.md:357-358`). LOG_OOB_XID: `gpuLogOobXidMessage_KERNEL` (`gpu/gpu.c:7555-7614`, 610 send at :7699) from `nvErrorLog2` (`kernel/os/os_init.c:550`) for every Xid the guest kernel raises itself, such as Xid 154 on a recovery-action change (`gpu.c:7386`) and Xid 10 on a vblank timeout (`kernel_rc_watchdog_callback.c:447`); Xids that GSP reports via OS_ERROR_LOG are logged with oobLogging false (`gpu/gsp/kernel_gsp.c:883`), and GSP/RPC-timeout Xids are excluded (`gpu.c:7584-7597`). Fabric: `compute/fm_session_api.c:60-65` (fabric manager exit), `compute/imex_session_api.c:53-69` (IMEX). RC policy, ECC, NV208F: no caller in the tree; user space closed (nvidia-smi -e/-p, field diag, MODS, debug tools). No trace or run hit for any of the 15.

**Truth source.** Host RO for the ECC/remap family (NOT_SUPPORTED on this board, already measured through NVML). Guest state for the watchdog and the RC journal. None for the rest.

**Isolation risk.** The highest in this slice. Forwarded, RECOVER_ALL_COMPUTE_CONTEXTS RCs the host desktop's compute work; SET_RC_RECOVERY/SET_RC_INFO disable host-wide RC, so a hung host channel would never recover; NV208F kill pointers and write-kill inject ECC faults at host physical addresses; CLEAR_REMAPPED_* and SET_ECC_CONFIGURATION write the host InfoROM; SVOP changes SRAM timing of the host silicon. A watchdog recovery that reset an engine instead of the guest's mirror channels would kill host work. LOG_OOB_XID forwarded would put guest-controlled text into the host board's OOB log.

**Design proposal.** New `crates/vfio-user-nvgpu/src/rc.rs`: (a) REFUSE/DENY rows for ECC, NV208F, SET_RC_RECOVERY, SET_RC_INFO, SET_RC_WATCHDOG_INFO and the two fabric controls, in the same table as `power.rs`; (b) LOG_OOB_XID -> ACK plus a sanitized device-log line "guest xid N: text"; (c) RC_SET_CLEAN_ERROR_HISTORY -> ACK now, later clears the journal; (d) RC_WATCHDOG_TIMEOUT -> walk the channel registry, pick guest channels with a stalled GPGet or a host RC note, reset each on its host mirror through the RESET_CHANNEL path of category C, emit RC_TRIGGERED/OS_ERROR_LOG through the event helper proposed in `gsp-surface.md` §4. The status is ignored by the caller, so (d) can start as ACK plus log. Tests: unit tests for ids and sizes (LOG_OOB_XID 92 bytes, NV0080 804 bytes) against the generated ABI; a test that the watchdog routine never touches a channel that is not the guest's mirror; gate: the CUDA gate and a GNOME display gate with `RmRcWatchdog` removed from the guest registry.

**Estimate.** 1.5 to 4 engineer-days: table and LOG_OOB_XID 0.5; RC journal 0.5 to 1; watchdog recovery 0.5 to 2.5 on top of the RC_TRIGGERED emitter (2 to 3 days, counted in gsp-surface.md §4) and RESET_CHANNEL (category C).

**Client likelihood.** Group rank 2, set by INTERNAL_RC_WATCHDOG_TIMEOUT and INTERNAL_LOG_OOB_XID (kernel RC/error paths once the watchdog runs and the guest raises its own Xids).

**Roadmap.** Linux CUDA client (watchdog re-enable, RC path after a channel fault) and Wayland/KMS display (Xid 10 vblank timeout goes through LOG_OOB_XID once the watchdog runs with display on). Depends on the RC_TRIGGERED/OS_ERROR_LOG events (`gsp-surface.md` §4) and on RESET_CHANNEL (category C, `gsp-surface.md:542`). ECC, NV208F and fabric: nobody on this chip.

**Reclassification.** E (9): SET_ECC_CONFIGURATION, RESET_ECC_ERROR_STATUS, FB_ECC_SET_KILL_PTR, FB_ECC_SET_WRITE_KILL, FB_CLEAR_REMAPPED_ROWS, FB_CLEAR_REMAPPED_BANKS (no ECC/remapping on TU106), SET_RC_WATCHDOG_INFO (MODS only), RECOVER_ALL_COMPUTE_CONTEXTS (fabric manager), NV0080 RC_AND_PERMANENTLY_DISABLE_CHANNELS (IMEX). C (2): INTERNAL_RC_WATCHDOG_TIMEOUT and RC_SET_CLEAN_ERROR_HISTORY (derived from guest channel/RC state). Stay D: LOG_OOB_XID (ACK), SET_RC_RECOVERY, SET_RC_INFO (REFUSE), SET_RAM_SVOP_VALUES (DENY).

**Measurements.** (1) Guest with `RmRcWatchdog` removed from the registry: boot, idle 60 s, then the CUDA gate; check that the watchdog channel's GPFIFO entries execute through the device (notifier written each interval) and that no 0x20800A6A appears; then stall the device's executor for that channel for longer than the watchdog timeout and check that 0x20800A6A arrives and what the guest logs next. (2) Same with display on (GNOME on the device): watch for Xid 10 and 0x20800A56 if the display engine's vblank counter stops. (3) Host RO: NV2080_CTRL_CMD_GET_RC_RECOVERY 0x2080220e and GET_RC_INFO 0x20802212 on the device's host subdevice, to learn the defaults a guest GET should see. (4) Host RO, already known from NVML: GPU_QUERY_ECC_STATUS and INFOROM_ECC_SUPPORT are 0x56, so no new ECC measurement is needed.

### 6.9 Engines CE/NVENC/NVDEC/OFA (1)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV2080_CTRL_CMD_PERF_NOTIFY_VIDEOEVENT` | 0x2080205D | 4/4 | user | ACK | 2 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces (not in the nvenc.jsonl or nvdec.jsonl traces) |

**Subfamilies.** One control: `PERF_NOTIFY_VIDEOEVENT` 0x2080205D, a video-driver hint to the perf policy.

**Why category D holds almost no engine controls.** The engine context classes export no controls at all: `src/nvidia/generated/g_kernel_ce_context_nvoc.c`, `g_kernel_nvenc_ctx_nvoc.c`, `g_kernel_nvdec_ctx_nvoc.c`, `g_kernel_ofa_ctx_nvoc.c`, `g_kernel_nvjpg_ctx_nvoc.c` have zero method ids. CE, NVENC, NVDEC, NVJPG and OFA work is methods on engine objects in channels (the allocations route to GSP, s615.txt:214-218: KernelCeContext, NvdecContext, NvjpgContext, OfaContext, MsencContext), which the device handles as channel work (mirror.rs `add_engine_object`, the software CE in channel.rs), not as controls. The remaining engine controls of the 615 surface landed elsewhere: CE capability and mapping controls are handled (s615.txt:366-373: CE_GET_CE_PCE_MASK, GET_PHYSICAL_CAPS, GET_ALL_PHYSICAL_CAPS host-forwarded in uvm.rs; UPDATE_CLASS_DB, GET_FAULT_METHOD_BUFFER_SIZE, GET_PCE_CONFIG_FOR_LCE_TYPE, IS_DECOMP_LCE_ENABLED synthesized; UPDATE_PCE_LCE_MAPPINGS_V2 echoed); the capability getters are category A (s615.txt:420-422 NVENC/NVDEC/NVJPG_GET_CAPS_V2, :428 GET_FIRST_ASYNC_CE_IDX, :457-459 CE_GET_LCE_SHIM_INFO, GET_HUB_PCE_MASK_V2, GET_DECOMP_LCE_MASK); MIG and vGPU ones are E (s615.txt:899, :905 NVENC SW session stats/info, :1002-1004 CE HUB mask and MIG PCE/LCE, :1071 vGPU encoder capacity). The NVENC session controls on NVENC_SW_SESSION 0xa0bc (`src/nvidia/generated/g_nvencsession_nvoc.c`, 0xa0bc0101/02) are kernel-side and not routed, and `GET_NVENC_SW_SESSION_STATS` is sent to GSP only on a vGPU GSP host (`gpu/subdevice/subdevice_ctrl_gpu_kernel.c:1036-1051`); nvidia-smi's call in `nvml.jsonl` is answered by the guest kernel. Turing has no OFA and no NVJPG engine, so those classes never allocate on TU106.

**State a device model needs.** A per-device record of active stream events (count by type), for logging only.

**What GSP-RM does.** Closed firmware; the header says the video driver notifies RM of HD/4K stream and OFA start/stop, which the perf policy uses to raise clocks (`ctrl2080perf.h:776-810`).

**Who calls it and when.** No caller in the tree; user space (NVDEC/NVENC libraries, closed); not in `nvenc.jsonl` or `nvdec.jsonl` of the native traces.

**Truth source.** None.

**Isolation risk.** Forwarded, it would raise host clocks and power for the desktop's whole GPU on a guest's request.

**Design proposal.** ACK arm in `ctrl.rs` (or in the acknowledge rule of gsp-surface.md section 3 D, rule 1): NV_OK, request echoed, note naming the event, label `synthesized (acknowledged, no effect)`. Test: generated-ABI size 4 and echo.

**Estimate.** 0.25 engineer-days.

**Client likelihood.** Rank 2: NVENC/NVDEC libraries may send it; Sunshine encoding is the path that matters.

**Roadmap.** Wayland/KMS display with NVENC streaming (Sunshine). No dependency.

**Reclassification.** None; it stays D under the acknowledge rule.

**Measurements.** Guest: run Sunshine (or `ffmpeg -c:v h264_nvenc`) on the device and check whether 0x2080205D appears in the device trace; if it never does, the row is dormant.

### 6.10 Events, interrupts and firmware tracing (7)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV2080_CTRL_CMD_GPU_REPORT_NON_REPLAYABLE_FAULT` | 0x20800177 | 32/32 | kernel | MODEL:channel-registry | 2 | guest state | kernel-open/nvidia-uvm/uvm_gpu_non_replayable_faults.c:525-537 kill_channel_delayed via src/nvidia/src/kernel/rmapi/nv_gpu_ops.c:11441-11472 (610 :11395); kernel body src/nvidia/src/kernel/gpu/subdevice/subdevice_ctrl_gpu_kernel.c:3813-3831 (610 :3703) runs only on a vGPU guest, a GSP client routes it (0x40040) |
| `NV2080_CTRL_CMD_INTERNAL_CONTROL_GSP_TRACE` | 0x208001E3 | 32/32 (layout changed) | internal | REFUSE | 3 | none | src/nvidia/src/kernel/gpu/gsp/kernel_gsp_trace_rats.c:304-306 gspRatsAddBindpoint (start) and :377-382 gspRatsRemoveBindpoint (stop), from the event-buffer bind src/nvidia/src/kernel/gpu/subdevice/subdevice_ctrl_event_kernel.c:321-345 |
| `NV2080_CTRL_CMD_INTERNAL_UCODE_INSTRUMENTATION_GET_STATE` | 0x20800B12 | 16/16 | internal | REFUSE | 3 | none | src/nvidia/src/kernel/diagnostics/ucode_instrumentation_ctrl.c:60-96 (NV208F UCODE_INSTRUMENTATION_GET_STATE forwards at :84-90) |
| `NV2080_CTRL_CMD_INTERNAL_UCODE_INSTRUMENTATION_SET_STATE` | 0x20800B13 | 16/16 | internal | REFUSE | 3 | none | src/nvidia/src/kernel/diagnostics/ucode_instrumentation_ctrl.c:99-135 (forwards at :124-130) |
| `NV2080_CTRL_CMD_FLCN_USTREAMER_QUEUE_INFO` | 0x20803120 | 24/24 | user | REFUSE | 3 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces; src/nvidia/src/kernel/rmapi/rmapi_utils.c:222 only lists it as a quiet unsupported vGPU-guest control |
| `NV2080_CTRL_CMD_FLCN_USTREAMER_CONTROL_GET` | 0x20803122 | 44/44 | user | REFUSE | 3 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |
| `NV2080_CTRL_CMD_FLCN_USTREAMER_CONTROL_SET` | 0x20803123 | 44/44 | admin | REFUSE | 3 | none | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |

**Subfamilies.** (1) Fault reporting into RC: `GPU_REPORT_NON_REPLAYABLE_FAULT` 0x20800177. (2) Firmware tracing: `INTERNAL_CONTROL_GSP_TRACE` 0x208001E3, `INTERNAL_UCODE_INSTRUMENTATION_GET_STATE`/`SET_STATE` 0x20800B12/13, `FLCN_USTREAMER_QUEUE_INFO` 0x20803120, `FLCN_USTREAMER_CONTROL_GET`/`SET` 0x20803122/23.

**State a device model needs.** For (1) only what the registry has: channel by instance block, its error notifier (recorded at channel alloc, uvm.md table row 14), and the RC event path to the guest. Per channel, dropped at free. (2) needs none.

**What GSP-RM does.** REPORT_NON_REPLAYABLE_FAULT: closed; the kernel twin (`gpu/subdevice/subdevice_ctrl_gpu_kernel.c:3813-3831`, run only on a vGPU guest; a GSP client routes it, flags 0x40040) calls `kgmmuHandleNonReplayableFaultPacket`, which reports the fault and starts RC teardown of the channel (`ctrl2080gpu.h:2875-2886`). GSP_TRACE starts or stops the RATS trace of GSP-RM into a buffer (`ctrl2080internal.h:4070-4076`, params :4034-4060). UCODE_INSTRUMENTATION controls coverage builds of ucode (`:5568-5601`). uStreamer controls expose and filter the PMU trace queues (`ctrl2080flcn.h:301-339`).

**Who calls it and when.** REPORT_NON_REPLAYABLE_FAULT: UVM `kernel-open/nvidia-uvm/uvm_gpu_non_replayable_faults.c:525-537` (`kill_channel_delayed`) through `rmapi/nv_gpu_ops.c:11441-11472`, when a non-replayable fault on a user channel is fatal. GSP_TRACE: `gpu/gsp/kernel_gsp_trace_rats.c:304-306` and :377-382, from the event-buffer bind `gpu/subdevice/subdevice_ctrl_event_kernel.c:321-345` (a tool). UCODE_INSTRUMENTATION: `diagnostics/ucode_instrumentation_ctrl.c:60-135` behind NV208F diag controls. uStreamer: no caller (`rmapi/rmapi_utils.c:222` only lists it as a quiet unsupported vGPU control).

**Truth source.** Guest state for (1); none for (2).

**Isolation risk.** The fault packet carries guest instance pointers and addresses; passed to the host GMMU it would RC whatever host channel the instance pointer matches. The tracing controls, forwarded, would expose GSP-RM, PMU and ucode traces of every host client, and SET controls would change host firmware behaviour.

**Design proposal.** (1) belongs with the RC work that gr-completion.md lists as item g (`gr-completion.md:189`): a new `rc.rs` or an extension of `channel.rs` that finds the channel by instance block, writes the error notifier, raises `RC_TRIGGERED`, and for a mirrored channel stops the mirror (`mirror.rs` free or DISABLE_CHANNELS). (2): one refusal rule, NOT_SUPPORTED; the GSP trace bind then fails cleanly (`kernel_gsp_trace_rats.c:308-309`, :327-335). Tests: unit test that a packet with an unknown instance block is rejected and one with a guest channel sets the notifier; gate: a UVM test that triggers a fatal non-replayable fault (a CE copy to an unmapped VA from a UVM-managed channel) must end in a CUDA error, not a hang.

**Estimate.** 2 to 4 engineer-days, of which 2 to 3 are the RC reflection shared with gr-completion item g; the refusals are 0.25.

**Client likelihood.** Group rank 2. Decided by REPORT_NON_REPLAYABLE_FAULT (the error path of a faulting CUDA channel).

**Roadmap.** Linux CUDA client (fault robustness). Depends on the RC reflection to the guest (gr-completion item g, not done) and on UVM fault buffers (`uvm.rs`).

**Reclassification.** C (1): 0x20800177 (RC of a guest channel). E (2): 0x20800B12/13 (instrumented ucode builds only). Stay D (4): GSP_TRACE and the three uStreamer controls, REFUSE (tools).

**Measurements.** (a) Guest: provoke a fatal non-replayable fault under UVM and check that 0x20800177 arrives with a packet whose instance block matches a registry channel. (b) None on the host.

### 6.11 vGPU, SR-IOV and MIG internals (8)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV0080_CTRL_CMD_GR_SET_TPC_PARTITION_MODE` | 0x00801108 | 32/32 | user | REFUSE | 3 | host RO | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces; the per-context twin NV0090 SET_TPC_PARTITION_MODE is forwarded by kgrctxCtrlSetTpcPartitionMode src/nvidia/src/kernel/gpu/gr/kernel_graphics_context.c:3355-3383 |
| `NV2080_CTRL_CMD_INTERNAL_GPU_CHECK_CTS_ID_VALID` | 0x20800A46 | 16/16 | internal | REFUSE | 5 | none | src/nvidia/src/kernel/gpu/mig_mgr/kernel_mig_manager.c:10083-10086, only with MIG GPU instances |
| `NV2080_CTRL_CMD_INTERNAL_MEMSYS_SET_PARTITIONABLE_MEM` | 0x20800A51 | 40/40 | internal | REFUSE | 5 | none | src/nvidia/src/kernel/gpu/mem_mgr/mem_mgr.c:2514-2520 memmgrSetPartitionableMem, MIG memory partitioning only (:2494-2498) |
| `NV2080_CTRL_CMD_INTERNAL_FIFO_PROMOTE_RUNLIST_BUFFERS` | 0x20800A53 | 4112/4112 | internal | REFUSE | 5 | none | src/nvidia/src/kernel/gpu/mig_mgr/kernel_mig_manager.c:2631-2636, GPU-instance creation |
| `NV2080_CTRL_CMD_INTERNAL_SEND_CMC_LIBOS_BUFFER_INFO` | 0x20800A89 | 24/24 | kernel | REFUSE | 5 | none | src/nvidia/src/kernel/gpu/gsplite/kernel_gsplite.c:253-258 under LIBOS_LOG_ENABLE, only for a GSP-lite (CMC) engine |
| `NV2080_CTRL_CMD_INTERNAL_MIGMGR_SET_PARTITIONING_MODE` | 0x20800AA3 | 4/4 | kernel | REFUSE | 5 | none | src/nvidia/src/kernel/gpu/mig_mgr/kernel_mig_manager.c:1010-1025 (resetless-MIG boot) and :7795-7798 |
| `NV2080_CTRL_CMD_INTERNAL_FIFO_TOGGLE_ACTIVE_CHANNEL_SCHEDULING` | 0x20800AC3 | 1/1 | internal | MODEL:channel-registry | 2 | guest state | src/nvidia/src/kernel/gpu/fifo/kernel_fifo_init.c:82-93 kfifoStateLoad (enable) and :108-120 kfifoStateUnload (disable), on a GSP client only with GPU_STATE_FLAGS_PM_TRANSITION |
| `NV2080_CTRL_CMD_GR_SET_TPC_PARTITION_MODE` | 0x2080122C | 32/32 | user | REFUSE | 3 | host RO | no caller in the tree (kernel-open, nvidia-modeset, src/common, UVM grep empty); user-space callers closed, none in the 79 native traces |

**Subfamilies.** (1) MIG plumbing: `INTERNAL_GPU_CHECK_CTS_ID_VALID` 0x20800A46, `INTERNAL_MEMSYS_SET_PARTITIONABLE_MEM` 0x20800A51, `INTERNAL_FIFO_PROMOTE_RUNLIST_BUFFERS` 0x20800A53, `INTERNAL_MIGMGR_SET_PARTITIONING_MODE` 0x20800AA3. (2) GSP-lite: `INTERNAL_SEND_CMC_LIBOS_BUFFER_INFO` 0x20800A89. (3) TPC partitioning of a TSG: `NV0080 GR_SET_TPC_PARTITION_MODE` 0x00801108 and its subdevice alias 0x2080122C. (4) Suspend, misnamed into this group by the heuristic: `INTERNAL_FIFO_TOGGLE_ACTIVE_CHANNEL_SCHEDULING` 0x20800AC3.

**State a device model needs.** (4) the per-device `scheduling_disabled` flag of the fifo group. (3) per guest TSG a partition mode if the host shows TU106 supports it. (1) and (2) none.

**What GSP-RM does.** Closed firmware. The MIG controls program partition boundaries, runlist buffers and partitioning mode (`ctrl2080internal.h:828-844`, :1063-1083, :1108-1118, :2018-2025). CMC_LIBOS passes a log buffer to a CMC gsplite engine (`:5323-5334`). TPC partition mode sets NONE/STATIC/DYNAMIC for a TSG context (`ctrl0080gr.h:204-238`; subdevice alias `ctrl2080gr.h:1490-1494`); the per-context NV0090 twin is forwarded by `gpu/gr/kernel_graphics_context.c:3355-3383`. TOGGLE disables all active channels at suspend and resumes scheduling after resume (`ctrl2080internal.h:2936-2945`).

**Who calls it and when.** MIG: `gpu/mem_mgr/mem_mgr.c:2514-2520` (returns early without partitionable memory, :2494-2498), `gpu/mig_mgr/kernel_mig_manager.c:1010-1025` (resetless MIG boot), :2631-2636, :7795-7798, :10083-10086; none reached in any device init run (610 or 615). CMC: `gpu/gsplite/kernel_gsplite.c:253-258`, only with a gsplite engine. TPC partition: no caller, user space unknown, none in the traces. TOGGLE: `gpu/fifo/kernel_fifo_init.c:82-93` (state load, enable) and :108-120 (state unload, disable), on a GSP client only for PM transitions.

**Truth source.** (4) guest state; (3) host RO for whether the feature exists; (1) and (2) none.

**Isolation risk.** TOGGLE forwarded as-is would stop every channel on the physical GPU, the desktop included; it must act only on the guest's channels. MIG and runlist-promotion controls forwarded would repartition host memory or install runlists on the shared scheduler. TPC partition mode on a host TSG other than the guest's mirror would repartition host work.

**Design proposal.** (1) and (2): one refusal rule keyed on the chip file's feature families (MIG and gsplite absent on TU106), as gsp-surface.md section 3 E proposes. (4): same code as `DISABLE_USERMODE_CHANNELS` in the fifo group (`channel.rs` flag, mirror disable on the device's own host channels). (3): NOT_SUPPORTED until the host read below; if TU106 supports it, MIRROR to the guest's mirror TSG after resolving hChannelGroup through the registry. Tests: a refusal-table unit test against the generated ABI ids; the suspend gate of the fifo group covers TOGGLE.

**Estimate.** 1 to 1.5 engineer-days: refusal rule (0.25), TOGGLE on top of the fifo suspend work (0.5), TPC partition decision and optional mirror forward (0.25 to 0.75).

**Client likelihood.** Group rank 2, decided alone by TOGGLE_ACTIVE_CHANNEL_SCHEDULING (every guest suspend). Without it the group is rank 5.

**Roadmap.** Suspend/resume (TOGGLE); nobody on this chip for the MIG and gsplite rows; the TPC rows only if a CUDA MPS client appears.

**Reclassification.** C (3): 0x20800AC3 (suspend path over the guest channel set), 0x00801108 and 0x2080122C (target a guest TSG). E (5): 0x20800A46, 0x20800A51, 0x20800A53, 0x20800AA3 (MIG, absent on Turing), 0x20800A89 (gsplite/CMC, Blackwell).

**Measurements.** (a) Host RO: `NV0080_CTRL_CMD_GR_GET_TPC_PARTITION_MODE` 0x00801107 on a TSG of the device's own host client (a mirror TSG while a CUDA gate runs): NOT_SUPPORTED means REFUSE stays; NONE means the mirror forward is possible. (b) Guest suspend in pure emulation (shared with fifo): order and parameters of 0x20800AC3.

### 6.12 NVLink, C2C and SLI bridge (3)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV2080_CTRL_CMD_INTERNAL_DETECT_HS_VIDEO_BRIDGE` | 0x20800ADD | 0/0 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu_mgr/gpu_mgr_sli.c:411 (gpumgrDetectHighSpeedVideoBridges, from :253), reached only by SLI link detection with at least two GPUs (gpu_mgr_sli.c:114-122, platform/sli/sli.c:148) |
| `NV2080_CTRL_CMD_INTERNAL_DISP_SET_SLI_LINK_GPIO_SW_CONTROL` | 0x20800ADE | 16/16 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/disp/arch/v04/kern_disp_0400.c:73 (setSliLinkGpioSwControl, from kdispDetectSliLink_v04_00 :220, reached from gpu_mgr_sli.c:302 with two GPUs) |
| `NV2080_CTRL_CMD_INTERNAL_MEMSYS_SET_PEER_ATS_CONFIG` | 0x20800AFC | 32/32 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/mem_sys/arch/volta/kern_mem_sys_gv100.c:145 and :191 (ATS peer setup over NVLink); kmemsysSetupAllAtsPeers is a stub on TU102-TU117 (generated/g_kern_mem_sys_nvoc.c:721-729) and the caller needs ATS and KernelNvlink (kern_mem_sys.c:311-314) |

**Subfamilies.**
- SLI bridge detection: `INTERNAL_DETECT_HS_VIDEO_BRIDGE` and `INTERNAL_DISP_SET_SLI_LINK_GPIO_SW_CONTROL`.
- NVLink ATS peering: `INTERNAL_MEMSYS_SET_PEER_ATS_CONFIG`.

**State a device model needs.** None.

**What GSP-RM does.** All three are closed. The header gives no comment for `DETECT_HS_VIDEO_BRIDGE` (`ctrl2080internal.h:3480`). The caller treats NV_OK as "bridge present" (615 `src/nvidia/src/kernel/gpu_mgr/gpu_mgr_sli.c:411-418`). `SET_SLI_LINK_GPIO_SW_CONTROL` puts the pinset's GPIO under software control and returns function, pin and direction (`ctrl2080internal.h:3482-3490`). `SET_PEER_ATS_CONFIG` writes a peer's ATS system-physical window into the local MMU (`ctrl2080internal.h:3930-3946`).

**Who calls it and when.**
- Bridge detection runs only during SLI link detection over at least two GPUs (`gpu_mgr_sli.c:114-122`, `:253`; `platform/sli/sli.c:148`; `gpu/disp/arch/v04/kern_disp_0400.c:73`, called from `kdispDetectSliLink_v04_00` at `:220`).
- ATS peering is called from `mem_sys/arch/volta/kern_mem_sys_gv100.c:145/:191`, which needs ATS and KernelNvlink (`kern_mem_sys.c:311-314`). On TU102-TU117 the `kmemsysSetupAllAtsPeers` HAL is a stub anyway (`generated/g_kern_mem_sys_nvoc.c:721-729`).
- No user-space caller. No hits in any trace or run.

**Truth source.** None.

**Isolation risk.** None as long as the device refuses these controls. Forwarded, the GPIO control would toggle SLI pins on the host board.

**Design proposal.** The E-family refusal in the chip file (gsp-surface.md section 3 E): `refused (feature absent on this chip)`, NOT_SUPPORTED. The kernel treats that as "no bridge" and skips the rest. Add one unit test per control.

**Estimate.** 0.1 to 0.25 engineer-days: table entries and tests, shared with the E rule.

**Client likelihood.** Rank 5. The RTX 2070 has no SLI or NVLink connector, and the guest has one GPU.

**Roadmap.** Nobody on this chip. A TU102 board (NVLink bridge) would turn the family on in its chip file (D14).

**Reclassification.** E (3): all three. They need two GPUs and a bridge, or an NVLink/ATS HAL that TU106 does not have.

**Measurements.** None needed. The absence is structural (HAL stub, GPU count).

### 6.13 Security, fuses and protected memory (4)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV0073_CTRL_CMD_SPECIFIC_HDCP_KEY_DECRYPTION` (615 only) | 0x007302B2 | -/8 | admin | REFUSE | 4 | none | no caller in the tree (new in 615) |
| `NV2080_CTRL_CMD_FB_SETUP_VPR_REGION` | 0x2080133B | 16/16 | kernel | REFUSE | 4 | none | no kernel caller; in the vGPU-guest skip list (615 rmapi_utils.c:218) |
| `NV2080_CTRL_CMD_FB_PATCH_PBR_FOR_MINING` | 0x20801341 | 1/1 | admin | DENY | 4 | none | no caller in the tree |
| `NV2080_CTRL_CMD_FB_SET_DRAM_ENCRYPTION_CONFIGURATION` | 0x20801356 | 4/4 | admin | REFUSE | 5 | none | no kernel caller; the VF body is a TODO stub returning NV_OK (615 src/nvidia/src/kernel/gpu/subdevice/subdevice_ctrl_vgpu.c:254-262) |

**Subfamilies.**
- Protected video memory: `FB_SETUP_VPR_REGION`.
- Board NV settings: `FB_SET_DRAM_ENCRYPTION_CONFIGURATION`, `FB_PATCH_PBR_FOR_MINING`.
- Display content protection: `NV0073 SPECIFIC_HDCP_KEY_DECRYPTION`, new in 615.

**State a device model needs.** None. The emulated board has no VPR, no DRAM encryption, no mining PBR patch, and no HDCP.

**What GSP-RM does.** All four are closed.
- `SETUP_VPR_REGION` carves a Video Protected Region at an FB offset and size, and reports a status (`ctrl2080fb.h:1630-1680`).
- `SET_DRAM_ENCRYPTION_CONFIGURATION` writes the setting to board non-volatile memory for the next reset (`ctrl2080fb.h:2813-2830`). The open tree has only a VF stub that returns NV_OK (615 `src/nvidia/src/kernel/gpu/subdevice/subdevice_ctrl_vgpu.c:254-262`).
- `PATCH_PBR_FOR_MINING` patches the VBIOS PBR values or restores them (`ctrl2080fb.h:1840-1853`).
- `HDCP_KEY_DECRYPTION` triggers and polls HDCP key decryption (`ctrl0073specific.h:3207-3221`; absent in 610).

**Who calls it and when.** No kernel caller for any of the four. `SETUP_VPR_REGION` is in the vGPU-guest skip list (`rmapi/rmapi_utils.c:218`), so some closed client (a protected-playback video stack) sends it and tolerates NOT_SUPPORTED. The DRAM-encryption setter is an nvidia-smi admin operation on boards that have the feature. No hits in traces or runs.

**Truth source.** None. `FB_QUERY_DRAM_ENCRYPTION_INFOROM_SUPPORT` is already forwarded (`ver/v610.rs:928`), so the guest sees the silicon's "unsupported" for the matching query.

**Isolation risk.** High if any of these were forwarded:
- VPR would reserve host FB that the host desktop and the guest's D11 reservation share.
- DRAM encryption would write the host board's InfoROM.
- The PBR patch would change the host card's power behaviour.
- HDCP key handling would act on the host display engine.

Refusing all four leaks nothing.

**Design proposal.** Reuse the refusal tables in `bus.rs`, or the chip file for E:
- `SETUP_VPR_REGION` and `HDCP_KEY_DECRYPTION`: REFUSE, label `refused (feature absent on this board)`. HDCP follows the display model's "sink without HDCP" rule.
- `PATCH_PBR_FOR_MINING`: DENY, label `refused (isolation: board power settings)`.
- `SET_DRAM_ENCRYPTION_CONFIGURATION`: chip-file E, label `refused (feature absent on this chip)`.

Unit tests for the 615-only id: absent from the 610 ABI, 8 bytes in 615.

**Estimate.** 0.25 to 0.5 engineer-days.

**Client likelihood.** The group ranks 4. `SETUP_VPR_REGION` decides it: a protected-playback stack would try it, but the board has no VPR. `HDCP_KEY_DECRYPTION` counts as HDCP, which is also rank 4.

**Roadmap.** Nobody on this chip for DRAM encryption. The display path never needs HDCP for a Wayland/Sunshine stream. Check that the `GPU_GET_VPR_CAPS` answer (another slice) also says no VPR.

**Reclassification.** E (1): `FB_SET_DRAM_ENCRYPTION_CONFIGURATION`. DRAM encryption is a Blackwell-class board feature and is absent on TU106.

**Measurements.** None needed. All four stay refused whatever the card would say.

### 6.14 Timers (2)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV9074_CTRL_CMD_FLUSH` | 0x90740101 | 16/16 | user | MODEL:timed-semaphore | 2 | pure emulation | kernel body 615 src/nvidia/src/kernel/gpu/timed_semaphore.c:272-299, but flags 0x48 route it to GSP in GSP-client mode (615 src/nvidia/generated/g_timed_sema_nvoc.c:202); user space unknown (no 0x9074 object in the 79 native traces) |
| `NV9074_CTRL_CMD_RELEASE` | 0x90740103 | 32/32 | user | MODEL:timed-semaphore | 2 | pure emulation | kernel body 615 src/nvidia/src/kernel/gpu/timed_semaphore.c:317-335, routed to GSP by flags 0x48 (g_timed_sema_nvoc.c:232); completion returns as event NV_VGPU_MSG_EVENT_TIMED_SEMAPHORE_RELEASE 0x1018 (615 src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:279) to _kgspRpcTimedSemaphoreRelease (kernel_gsp.c:1265-1290, dispatched :1637), which fills semaphore and notifier via tsemaRelease_KERNEL (timed_semaphore.c:635-680) |

**Subfamilies.** 2 controls on NV9074 GF100_TIMED_SEMAPHORE_SW (a software class on ENG_SW, on TU106 per 615 src/nvidia/generated/g_gpu_class_list.c:342): FLUSH and RELEASE. GET_TIME 0x90740102 is not routed (flag 0x8, g_timed_sema_nvoc.c:217) and stays in the guest kernel.

**State a device model needs.** Per 9074 object (key hClient, hObject, recorded at GSP_RM_ALLOC together with its hDevice parent chain): a time-ordered queue of entries {notifierGPUVA, semaphoreGPUVA, waitTimestamp, releaseValue}, the flushing flag and the flush-limit timestamp. Lifetime is the object: the queue is dropped on FREE, which matches `tsemaDestruct` (615 src/nvidia/src/kernel/gpu/timed_semaphore.c:247-263). Nothing survives a guest driver reload. The time base must be the device's PTIMER, the clock the guest reads through BAR0 and uses for waitTimestamp.

**What GSP-RM does.** Closed firmware, inferred from the open implementation of the same class. RELEASE queues an entry, releasing immediately if the queue is empty and the time is due (timed_semaphore.c:157-221, :79-99). FLUSH sets the flushing state with limit = now + maxFlushTime and releases everything at or after the limit with DONE_FLUSHED (:272-299, header ctrl9074.h:48-76). In GSP-client mode both are routed to GSP (flags 0x48, g_timed_sema_nvoc.c:202, :232). The release comes back as event NV_VGPU_MSG_EVENT_TIMED_SEMAPHORE_RELEASE 0x1018 (615 src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:279) with {semaphoreVA, notifierVA, hVASpace, releaseValue, completionStatus, hClient, hDevice} (generated/g_rpc-structures.h:1642-1651). The guest kernel's `_kgspRpcTimedSemaphoreRelease` (gpu/gsp/kernel_gsp.c:1265-1290, dispatched at :1637) then writes the semaphore and the notifier timestamp itself via `tsemaRelease_KERNEL` (timed_semaphore.c:635-680). GSP-RM only keeps time and sends the event. It never touches memory. The pushbuffer methods of the class (SET_NOTIFIER/SEMAPHORE/WAIT_TIMESTAMP, SCHEDULE, timed_semaphore.c:341 onward) are software methods. Where they execute in GSP-client mode is not visible in the tree.

**Who calls it and when.** No caller in the tree. There is no 9074 object in any of the 79 native traces (CUDA, GL, EGL-Wayland, Vulkan, NVENC, NVDEC, NVML) or in the Caraxes runs. User space is closed. A presentation-timing path (VDPAU presentation queue, GL present-at-time) is the plausible user, and that is unverified.

**Truth source.** Pure emulation (a timer on the device clock). Guest state for the addresses.

**Isolation risk.** Low. The device writes nothing. It only emits an event naming the guest's own handles and guest VAs, and the guest kernel writes through its own mappings. The one rule: never echo a host handle, and validate that hClient/hDevice belong to the recorded 9074 object.

**Design proposal.** New `MODEL:timed-semaphore` in a new file `crates/vfio-user-nvgpu/src/tsema.rs`. It keeps the per-object queue plus a timer wheel on the device PTIMER, polled from the existing event loop that posts GSP events (`boot.rs:280` sends POST_EVENT through `RpcHeader::event`). The event is emitted with function 0x1018 and the struct above. Status is DONE, or DONE_FLUSHED under flush. RELEASE returns NV_OK with the entry queued or released immediately. FLUSH sets the flag and drains. Tests: unit tests for ordering, immediate release, flush-limit semantics and the event layout against the generated rpc structure for 610 and 615; a guest test that allocates 9074 on a channel, issues RELEASE with now+10 ms, and checks the semaphore value and notifier status.

**Estimate.** 2 to 3 engineer-days: model, event emission, alloc/free hooks, unit tests and one guest test program.

**Client likelihood.** Rank 2 at most, with low confidence. It is a stock-driver class on TU106, but no trace hit. RELEASE decides it.

**Roadmap.** Not needed for Linux CUDA or Vulkan games as traced. Possibly display timing (VDPAU/GL present-at-time), which is outside the Wayland/KMS goal. Implement only after a guest trace shows a 9074 allocation. Depends on the GSP event path (`boot.rs`) and the device PTIMER.

**Reclassification.** None. Both are real GSP work (timer plus event), neither static nor host-forwardable.

**Measurements.**
- Guest: run a VDPAU presentation-queue client (e.g. mpv with the vdpau output under X), a GL swap-interval/present-timing test and the Vulkan present tests, with device tracing on. Look for GSP_RM_ALLOC class 0x9074 and these two controls. If none appear, leave both REFUSE.
- Guest, once the model exists: the RELEASE/FLUSH test program above, comparing the notifier timestamp to waitTimestamp (lateness bounded by the device poll interval).

### 6.15 GPIO and I2C (12)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV0073_CTRL_CMD_SPECIFIC_DISP_I2C_READ_WRITE` | 0x007302AC | 276/276 | admin | REFUSE | 2 | none | no caller in the tree |
| `NV2080_CTRL_CMD_I2C_READ_BUFFER` | 0x20800601 | 532/532 | user | REFUSE | 2 | none | no kernel caller; in the vGPU-guest skip list (615 rmapi_utils.c:216) |
| `NV2080_CTRL_CMD_I2C_WRITE_BUFFER` | 0x20800602 | 276/276 | user | REFUSE | 2 | none | no caller in the tree |
| `NV2080_CTRL_CMD_I2C_READ_REG` | 0x20800603 | 276/276 | user | REFUSE | 2 | none | no kernel caller; in the vGPU-guest skip list (615 rmapi_utils.c:217) |
| `NV2080_CTRL_CMD_I2C_WRITE_REG` | 0x20800604 | 276/276 | user | REFUSE | 2 | none | no kernel caller; in the vGPU-guest skip list (615 rmapi_utils.c:215) |
| `NV2080_CTRL_CMD_INTERNAL_GPU_SET_ILLUM` | 0x20800AEC | 8/8 | internal | REFUSE | 2 | none | 615 src/nvidia/src/kernel/gpu/subdevice/subdevice_ctrl_gpu_kernel.c:3894 (subdeviceCtrlCmdGpuSetIllum_IMPL, the public GPU_SET_ILLUM from user space; on NV_OK it also persists the value in the registry, :3897-3912) |
| `NV2080_CTRL_CMD_INTERNAL_GPIO_PROGRAM_DIRECTION` | 0x20802300 | 8/8 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/disp/arch/v04/kern_disp_0400.c:99, used only by kdispDetectSliLink_v04_00 (:168-260) during SLI link detection between two GPUs (gpu_mgr_sli.c:302) |
| `NV2080_CTRL_CMD_INTERNAL_GPIO_PROGRAM_OUTPUT` | 0x20802301 | 8/8 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/disp/arch/v04/kern_disp_0400.c:117, used only by kdispDetectSliLink_v04_00 (:168-260) during SLI link detection between two GPUs (gpu_mgr_sli.c:302) |
| `NV2080_CTRL_CMD_INTERNAL_GPIO_READ_INPUT` | 0x20802302 | 8/8 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/disp/arch/v04/kern_disp_0400.c:134, used only by kdispDetectSliLink_v04_00 (:168-260) during SLI link detection between two GPUs (gpu_mgr_sli.c:302) |
| `NV2080_CTRL_CMD_INTERNAL_GPIO_ACTIVATE_HW_FUNCTION` | 0x20802303 | 8/8 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/disp/arch/v04/kern_disp_0400.c:156, used only by kdispDetectSliLink_v04_00 (:168-260) during SLI link detection between two GPUs (gpu_mgr_sli.c:302) |
| `NV402C_CTRL_CMD_I2C_INDEXED` | 0x402C0102 | 32/32 | user | REFUSE | 2 | none | no kernel sender; 615 src/nvidia/src/kernel/rmapi/embedded_param_copy.c:487/:1003 only copy the embedded pMessage buffer for user-space callers |
| `NV402C_CTRL_CMD_I2C_TRANSACTION` | 0x402C0105 | 96/96 | user | REFUSE | 2 | none | 615 src/nvidia/arch/nvalloc/unix/src/osapi.c:4127 (rm_i2c_transfer from the Linux i2c adapters of kernel-open/nvidia/nv-i2c.c:67-149; adapters exist only for ports RmI2cAddGpuPorts finds implemented, osapi.c:4332-4376, from osinit.c:2302); kern_gsync_p2060.c:422 (GSYNC board only) |

**Subfamilies.**
- Display I2C: `NV0073 SPECIFIC_DISP_I2C_READ_WRITE`.
- Legacy subdevice I2C: `I2C_READ_BUFFER`, `I2C_WRITE_BUFFER`, `I2C_READ_REG`, `I2C_WRITE_REG`.
- NV40_I2C object: `I2C_INDEXED`, `I2C_TRANSACTION`.
- Board illumination: `INTERNAL_GPU_SET_ILLUM`.
- Raw GPIO: `INTERNAL_GPIO_PROGRAM_DIRECTION`, `_PROGRAM_OUTPUT`, `_READ_INPUT`, `_ACTIVATE_HW_FUNCTION`.

**State a device model needs.** None beyond what already exists:
- `I2C_GET_PORT_INFO` answers "no ports" (`ctrl.rs:1101`).
- `SPECIFIC_GET_I2C_PORTID` answers ddcPortId 0 (`display/mod.rs:680-686`).
- The display static info reports its i2cPort (`display/mod.rs:475`).
- EDID comes from the sink model through `GET_EDID_V2` (`display/mod.rs:1041-1075`).

Every I2C answer must stay consistent with "no ports". There is no GPIO or LED state.

**What GSP-RM does.** All closed. The I2C controls run transactions on the board's CCB ports: the DDC of the HDMI/DVI connectors, and the thermal, fan and VRM devices and EEPROMs (`ctrl0073specific.h:2957-2987`; `ctrl2080i2c.h:71-238`; `ctrl402c.h:250-317`, `:900-967`). `INTERNAL_GPU_SET_ILLUM` sets the logo or SLI-bridge LED attribute (`ctrl2080internal.h:4183-4198`). The public `GPU_SET_ILLUM` kernel body forwards it and, on NV_OK, persists the value in the registry (615 `subdevice_ctrl_gpu_kernel.c:3880-3912`). The raw GPIO controls program, drive and read board GPIO pins (`ctrl2080gpio.h:30-70`).

**Who calls it and when.**
- `I2C_TRANSACTION` has one real kernel sender, `rm_i2c_transfer` (615 `src/nvidia/arch/nvalloc/unix/src/osapi.c:4127`), reached from the Linux i2c adapters of `kernel-open/nvidia/nv-i2c.c:67-149`. The adapters are registered only for ports that `RmI2cAddGpuPorts` finds implemented through `I2C_GET_PORT_INFO` (`osapi.c:4332-4376`, called from `osinit.c:2302`). With "no ports" the guest has no `/dev/i2c-N` from nvidia and the kernel never sends it.
- The GSYNC board path (`kern_gsync_p2060.c:422`) is rank 4.
- nvidia-modeset reaches DDC through DP AUX-channel I2C over `NV0073_CTRL_CMD_DP_AUXCH_CTRL` (`src/common/displayport/src/dp_evoadapter.cpp:1040-1045`), not through this group.
- `I2C_READ_BUFFER`, `I2C_READ_REG`, `I2C_WRITE_REG` and `GPU_SET_ILLUM` are in the vGPU-guest skip list (`rmapi/rmapi_utils.c:215-217`, `:234`), so closed stock clients (nvidia-settings/nvml class) send them.
- The raw GPIO controls come only from SLI link detection between two GPUs (`gpu/disp/arch/v04/kern_disp_0400.c:99-156`, via `gpu_mgr_sli.c:302`).
- No hits in traces or runs.

**Truth source.** None for the answers, which are refusals. Whether the real board has I2C ports or illumination could be read host RO (Measurements) but does not change the answer.

**Isolation risk.** High if forwarded. NV40_I2C allocates non-privileged (`rmapi/resource_list.h:854-860`) and the 2080 I2C controls are NON_PRIVILEGED. An unprivileged guest user could therefore write host board I2C devices: fan controller, VRM, EEPROM. A forwarded display I2C write reaches the host monitor's DDC/CI. The GPIO setters toggle host board pins. `SET_ILLUM` would change the host card's LED, which is harmless but still a host hardware act. With every control refused, nothing reaches the host. The one invariant: `I2C_GET_PORT_INFO` must never be forwarded, because host ports would make the guest kernel register adapters.

**Design proposal.** One refusal rule in `bus.rs` (or `display/` for the NV0073 member): the 8 I2C/ILLUM controls answer NV_ERR_NOT_SUPPORTED with the label `refused (no I2C ports / no illumination on this board)`. The 4 raw GPIO controls go into the chip-file E family. A unit test asserts that the port-info answer and the transaction refusal agree: every port reads "not implemented", so every transaction on any port is refused. A test that no member is in any forward list shares the bus guard test. Gate: a Wayland guest with `ddcutil detect` shows no nvidia i2c bus and no control of this group in the device trace.

**Estimate.** 0.5 to 1 engineer-day, including the consistency test with `I2C_GET_PORT_INFO` and `QUERY_ILLUM_SUPPORT`, and the ddcutil gate.

**Client likelihood.** The group ranks 2. `I2C_READ_REG`/`WRITE_REG` and `GPU_SET_ILLUM` decide it: stock tools send them, per the vGPU skip list. None of them is on the CUDA or Wayland path.

**Roadmap.** Wayland/KMS: none of these, because DDC goes over DP AUX (display slice). NVENC streaming: none. If a TMDS connector model comes (P12), HDMI DDC would still go through the display/AUX model rather than I2C ports, unless `SPECIFIC_GET_I2C_PORTID` starts reporting a port. That decision would have to change this group too.

**Reclassification.** E (4): the raw GPIO quartet. It is used only by two-GPU SLI link detection, and the RTX 2070 has no SLI connector.

**Measurements.**
1. Host RO, read-only queries as an unprivileged client, with the operator's go-ahead per the host-GPU rule: `NV402C_CTRL_CMD_I2C_GET_PORT_INFO` and `NV2080_CTRL_CMD_GPU_QUERY_ILLUM_SUPPORT` on the host subdevice. This records whether the real board exposes ports and an LED. It is for the profile documentation only, since the device answer stays "none".
2. Guest sweep: `nvidia-settings -q all` and `ddcutil detect` in the Wayland guest, and `sensors-detect --auto`. Record the device trace to confirm which I2C or ILLUM controls a stock desktop actually sends, and that none arrive once port info says none.

### 6.16 Profiling and SM debugger (53)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV2080_CTRL_CMD_INTERNAL_PERF_PERFMON_CLIENT_RESERVATION_CHECK` | 0x20800A98 | 1/1 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/perf/kern_perf_pm.c:102-105 (kPerfPerfmonClientDeviceSet); that function has no caller in the 615 tree |
| `NV2080_CTRL_CMD_INTERNAL_PERF_PERFMON_CLIENT_RESERVATION_SET` | 0x20800A99 | 3/3 | internal | REFUSE | 5 | none | 615 src/nvidia/src/kernel/gpu/perf/kern_perf_pm.c:130-133, :171-174, :225-228 (kPerfPerfmonClientDeviceSet acquire, rollback, release); the function has no caller in the 615 tree |
| `NV2080_CTRL_CMD_GR_CTXSW_PM_MODE` | 0x20801207 | 24/24 | user | REFUSE | 3 | none | 615 src/nvidia/src/kernel/gpu/gr/kernel_graphics.c:3984-4036 (subdeviceCtrlCmdKGrCtxswPmMode: GSP-client branch sets up/promotes the PM ctxsw buffer via kgrctxSetupDeferredPmBuffer, then sends the cmd at :4027); user space: profilers (CUPTI/Nsight), closed |
| `NV2080_CTRL_CMD_GR_CTXSW_PM_BIND` | 0x20801209 | 40/40 | user | REFUSE | 3 | none | 615 src/nvidia/src/kernel/gpu/deferred_api.c:536 (class 5080 deferred-API param-size table; the call comes from user space through the deferred API); no trace hit |
| `NV2080_CTRL_CMD_GR_CTXSW_SMPC_MODE` | 0x2080120E | 24/24 | user | DENY | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; user space: profilers (SMPC counters) |
| `NV2080_CTRL_CMD_GR_PC_SAMPLING_MODE` | 0x20801212 | 24/24 | user | REFUSE | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; user space: CUPTI / Nsight Compute PC sampling |
| `NV2080_CTRL_CMD_PERF_RESERVE_PERFMON_HW` | 0x20802093 | 1/1 | user | DENY | 3 | none | 615 src/nvidia/src/kernel/gpu/perf/kern_perf_pm.c:254-277 (subdeviceCtrlCmdPerfReservePerfmonHw_KERNEL refuses non-admins under RmProfilingAdminOnly at :264-267, else sends to GSP at :270); user space unknown; not in the NVML trace (nvml.jsonl carries 0x20802087 and 0xb02, no reservation) |
| `NV83DE_CTRL_CMD_DEBUG_SET_NON_PREEMPTABLE_DEBUGGER_CONTEXT` | 0x83DE022D | 1/1 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_SET_RECOVERY_SUPPRESSION_TIMEOUT` | 0x83DE022E | 4/4 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_SM_DEBUG_MODE_ENABLE` | 0x83DE0301 | 0/0 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_SM_DEBUG_MODE_DISABLE` | 0x83DE0302 | 0/0 | user | ACK | 3 | guest state | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_SET_MODE_MMU_DEBUG` | 0x83DE0307 | 4/4 | user | MODEL:debugger-session | 3 | guest state | no caller in the tree; cuda-gdb sends action=1 (ENABLE) right after allocating its 83DE and 0x79 event, status 0 on the real card (matrix/traces/610.57.04/cuda-gdb.jsonl:2382-2386), followed by WRITE_MEMORY 0x83de0316 (:2390, :2489) |
| `NV83DE_CTRL_CMD_DEBUG_READ_SINGLE_SM_ERROR_STATE` | 0x83DE030B | 56/56 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_READ_ALL_SM_ERROR_STATES` | 0x83DE030C | 4824/4824 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs; the kernel body 615 src/nvidia/src/kernel/gpu/gr/kernel_sm_debugger_session_ctrl.c:757-800 acts only for IS_VIRTUAL, flag 0x40 sends it to GSP first in GSP-client mode |
| `NV83DE_CTRL_CMD_DEBUG_CLEAR_SINGLE_SM_ERROR_STATE` | 0x83DE030F | 8/8 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_CLEAR_ALL_SM_ERROR_STATES` | 0x83DE0310 | 8/8 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs; the kernel body kernel_sm_debugger_session_ctrl.c:803-843 is the vGPU path only |
| `NV83DE_CTRL_CMD_DEBUG_SET_NEXT_STOP_TRIGGER_TYPE` | 0x83DE0313 | 4/4 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_SET_SINGLE_STEP_INTERRUPT_HANDLING` | 0x83DE0314 | 4/4 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_SUSPEND_CONTEXT` | 0x83DE0317 | 8/8 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_RESUME_CONTEXT` | 0x83DE0318 | 0/0 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_SET_MODE_ERRBAR_DEBUG` | 0x83DE031F | 4/4 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_SET_SINGLE_SM_SINGLE_STEP` | 0x83DE0321 | 8/8 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_SET_SINGLE_SM_STOP_TRIGGER` | 0x83DE0322 | 8/8 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_SET_SINGLE_SM_RUN_TRIGGER` | 0x83DE0323 | 8/8 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_SET_SINGLE_SM_SKIP_IDLE_WARP_DETECT` | 0x83DE0324 | 8/8 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_SET_DROP_DEFERRED_RC` | 0x83DE0329 | 1/1 | user | REFUSE | 3 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV83DE_CTRL_CMD_DEBUG_SET_MODE_MMU_GCC_DEBUG` | 0x83DE032A | 4/4 | user | REFUSE | 5 | none | no caller in the tree (user space: libcudadebugger under cuda-gdb, closed); no hit in the 79 native traces or the Caraxes runs |
| `NV90CC_CTRL_CMD_HWPM_RESERVE` | 0x90CC0101 | 0/0 | user | DENY | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NV90CC_CTRL_CMD_HWPM_RELEASE` | 0x90CC0102 | 0/0 | user | REFUSE | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NV90CC_CTRL_CMD_POWER_REQUEST_FEATURES` | 0x90CC0301 | 12/12 | user | DENY | 3 | none | 615 src/nvidia/src/kernel/rmapi/rmapi_utils.c:220 lists it only in the vGPU-guest skip-error-message table (not a sender); no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NV90CC_CTRL_CMD_POWER_RELEASE_FEATURES` | 0x90CC0302 | 4/4 | user | DENY | 3 | none | 615 src/nvidia/src/kernel/rmapi/rmapi_utils.c:221 (vGPU-guest skip-error table only); no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_RELEASE_HWPM_LEGACY` | 0xB0CC0102 | 0/0 | user | REFUSE | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_RESERVE_PM_AREA_SMPC` | 0xB0CC0103 | 1/1 | user | DENY | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_RELEASE_PM_AREA_SMPC` | 0xB0CC0104 | 0/0 | user | REFUSE | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_ALLOC_PMA_STREAM` | 0xB0CC0105 | 56/56 | user | REFUSE | 3 | none | 615 src/nvidia/src/kernel/gpu/hwpm/profiler_v2/kern_profiler_v2_ctrl.c:249-346 runs it in the kernel (flags 0x10008, not routed) and sends INTERNAL_ALLOC_PMA_STREAM 0xB0CC0204 at :316-320; the RPC of this cmd at :381-387 belongs to the vGPU-guest helper |
| `NVB0CC_CTRL_CMD_FREE_PMA_STREAM` | 0xB0CC0106 | 4/4 | user | REFUSE | 3 | none | 615 src/nvidia/src/kernel/gpu/hwpm/profiler_v2/kern_profiler_v2_ctrl.c:54-112 runs it in the kernel and sends INTERNAL_FREE_PMA_STREAM at :107; the RPC of this cmd at :523-529 is the vGPU-guest helper |
| `NVB0CC_CTRL_CMD_EXEC_REG_OPS` | 0xB0CC010A | 3980/3980 | user | DENY | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_RESERVE_PM_AREA_PC_SAMPLER` | 0xB0CC010B | 0/0 | user | DENY | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_RELEASE_PM_AREA_PC_SAMPLER` | 0xB0CC010C | 0/0 | user | REFUSE | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_SET_HS_CREDITS` | 0xB0CC010E | 256/256 | user | REFUSE | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_RESERVE_HES` | 0xB0CC0113 | 8/8 | user | REFUSE | 5 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_RELEASE_HES` | 0xB0CC0114 | 4/4 | user | REFUSE | 5 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_DISABLE_DYNAMIC_MMA_BOOST` | 0xB0CC0117 | 1/1 | user | DENY | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_INTERNAL_QUIESCE_PMA_CHANNEL` | 0xB0CC0201 | 8/8 | user | REFUSE | 3 | none | 615 src/nvidia/src/kernel/gpu/hwpm/profiler_v2/kern_profiler_v2.c:247-280 (profilerBaseQuiesceStreamout), called only from the vGPU-guest free paths kern_profiler_v2.c:323 and kern_profiler_v2_ctrl.c:516 |
| `NVB0CC_CTRL_CMD_INTERNAL_PERMISSIONS_INIT` | 0xB0CC0203 | 7/6 (layout size) | user | DENY | 3 | none | 615 src/nvidia/src/kernel/gpu/hwpm/profiler_v2/kern_profiler_v2.c:564-591 (profilerDevConstructStateInterlude, after the alloc RPC of profilerDevConstructStatePrologue_FWCLIENT :543-561) and :594-619 (ProfilerCtx); every B2CC allocation that passed the guest's own permission check (profilerBaseQueryCapabilities :407-459) |
| `NVB0CC_CTRL_CMD_INTERNAL_ALLOC_PMA_STREAM` | 0xB0CC0204 | 64/64 | internal | DENY | 3 | none | 615 src/nvidia/src/kernel/gpu/hwpm/profiler_v2/kern_profiler_v2_ctrl.c:316-320 (ALLOC_PMA_STREAM), the kernel half of the user control; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_INTERNAL_FREE_PMA_STREAM` | 0xB0CC0206 | 4/4 | internal | REFUSE | 3 | none | 615 src/nvidia/src/kernel/gpu/hwpm/profiler_v2/kern_profiler_v2_ctrl.c:107-111 (FREE_PMA_STREAM), the kernel half of the user control; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_INTERNAL_BIND_PM_RESOURCES` | 0xB0CC0208 | 0/0 | internal | DENY | 3 | none | 615 src/nvidia/src/kernel/gpu/hwpm/profiler_v2/kern_profiler_v2_ctrl.c:176-177 (BIND_PM_RESOURCES), the kernel half of the user control; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_INTERNAL_UNBIND_PM_RESOURCES` | 0xB0CC0209 | 0/0 | internal | REFUSE | 3 | none | 615 src/nvidia/src/kernel/gpu/hwpm/profiler_v2/kern_profiler_v2_ctrl.c:225-226 (UNBIND_PM_RESOURCES), the kernel half of the user control; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_INTERNAL_RESERVE_HWPM_LEGACY` | 0xB0CC020A | 1/1 | internal | DENY | 3 | none | 615 src/nvidia/src/kernel/gpu/hwpm/profiler_v2/kern_profiler_v2_ctrl.c:243-244 (RESERVE_HWPM_LEGACY), the kernel half of the user control; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_POWER_REQUEST_FEATURES` | 0xB0CC0301 | 12/12 | user | DENY | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVB0CC_CTRL_CMD_POWER_RELEASE_FEATURES` | 0xB0CC0302 | 4/4 | user | DENY | 3 | none | no caller in the tree; no hit in the 79 native traces or the Caraxes runs; never arrives on a live object once the profiler class allocation (GSP_RM_ALLOC of 0x90cc or 0xb2cc) is answered NV_ERR_INSUFFICIENT_PERMISSIONS |
| `NVC763_CTRL_CMD_VIDMEM_ACCESS_BIT_DUMP` | 0xC7630103 | 536/536 | user | REFUSE | 5 | none | 615 src/nvidia/src/kernel/rmapi/nv_gpu_ops.c:8210-8240 (nvGpuOpsAccessBitsDump) from 615 kernel-open/nvidia-uvm/uvm_gpu.c:2805, only with access_bits_supported, which uvm_turing.c:65 sets false |

**Subfamilies.** 53 controls. (1) SM debugger NV83DE, 21: mode switches (SM_DEBUG_MODE_ENABLE/DISABLE, SET_MODE_MMU_DEBUG, SET_MODE_ERRBAR_DEBUG, SET_MODE_MMU_GCC_DEBUG), SM error state (READ/CLEAR single and all), run control (SUSPEND/RESUME_CONTEXT, NEXT_STOP_TRIGGER_TYPE, SINGLE_STEP_INTERRUPT_HANDLING, SINGLE_SM single-step/stop/run/skip-idle-warp), RC policy (SET_DROP_DEFERRED_RC, SET_NON_PREEMPTABLE_DEBUGGER_CONTEXT, SET_RECOVERY_SUPPRESSION_TIMEOUT). (2) Legacy profiler NV90CC, 4: HWPM_RESERVE/RELEASE, POWER_REQUEST/RELEASE_FEATURES. (3) ProfilerBase NVB0CC user controls, 16: HWPM legacy release, PM-area reservations (SMPC, PC sampler, HES), PMA streams (ALLOC/FREE), EXEC_REG_OPS, SET_HS_CREDITS, DISABLE_DYNAMIC_MMA_BOOST, POWER_REQUEST/RELEASE_FEATURES. (4) NVB0CC INTERNAL kernel-to-GSP halves, 7: PERMISSIONS_INIT, ALLOC/FREE_PMA_STREAM, BIND/UNBIND_PM_RESOURCES, RESERVE_HWPM_LEGACY, QUIESCE_PMA_CHANNEL. (5) Subdevice GR/PERF profiling switches, 7: GR_CTXSW_PM_MODE, GR_CTXSW_PM_BIND, GR_CTXSW_SMPC_MODE, GR_PC_SAMPLING_MODE, PERF_RESERVE_PERFMON_HW, INTERNAL_PERF_PERFMON_CLIENT_RESERVATION_CHECK/SET. (6) NVC763 VIDMEM_ACCESS_BIT_DUMP, 1.

**State a device model needs.** Almost none, because the recommended answer for the group is refusal. Two small pieces: per 83DE object the MMU debug action last set (enable/disable/released), lifetime of the guest object, dropped on FREE and on guest driver reload (the object table in `ctrl.rs` already goes with the client); and nothing for profilers, because the profiler objects themselves are refused at GSP_RM_ALLOC. A later cuda-gdb-with-breakpoints tier would need per 83DE: the host 83DE handle on the host mirror compute object, the guest-to-host handle map of hAppClient/hClass3dObject, and a queue of translated SM exception events for the guest 0x79 event under the 83DE.

**What GSP-RM does.** All 53 bodies are in closed GSP-RM; the open tree gives the kernel halves. Profiler permission is decided by the kernel, not by GSP: `profilerBaseQueryCapabilities` (615 src/nvidia/src/kernel/gpu/hwpm/profiler_v2/kern_profiler_v2.c:407-459) computes device/context profiling and trace permission from privLevel, `gpuIsRmProfilingPrivileged` and the NV_RM_CAP_SYS_PROFILER_* caps (:35-129), the alloc is sent to GSP (:543-561), and then NVB0CC_CTRL_CMD_INTERNAL_PERMISSIONS_INIT hands GSP-RM the resulting bits (:564-591, :594-619). GSP-RM trusts them. `bRmProfilingPrivileged` comes from RmProfilingAdminOnly (615 src/nvidia/src/kernel/gpu/gpu_register_access_map.c:174-203, :318-326), the module parameter RestrictProfilingToAdminUsers with a built-in default of 1 (615 src/nvidia/arch/nvalloc/unix/include/nv-reg.h:527-543, :1059; the comment at :537 says 0 is the default, the table entry sets 1). The legacy class checks the same flag (profiler_v1/kern_profiler_v1.c:44-55), and PERF_RESERVE_PERFMON_HW returns NV_ERR_INSUFFICIENT_PERMISSIONS to a non-admin (gpu/perf/kern_perf_pm.c:264-267). The header docs describe HWPM reservation as device-scope or context-scope with NV_ERR_STATE_IN_USE on conflict (ctrl90cc/ctrl90cchwpm.h:40-63), regops limited to reserved PM systems (ctrlb0cc/ctrlb0ccprofiler.h:270-290), and power features that switch ELCG/BLCG/SLCG/ELPG/idle slowdown (ctrlb0cc/ctrlb0ccpower.h:222-234). The SM debugger header: SM debug mode and MMU debug mode are per-context (ctrl83dedebug.h:40-59, :96-113; MMU debug is a grctx HW bit, ctrl0090.h:77-88, read during fault handling at gpu/mmu/kern_gmmu.c:3026-3058). SET_NON_PREEMPTABLE returns NV_ERR_NOT_SUPPORTED on Linux (ctrl83dedebug.h:1247), and the recovery-suppression timeout depends on it (:1271). MMU_GCC_DEBUG is Blackwell-onwards (:1163). HES is Blackwell-only (generated/g_kern_hwpm_nvoc.c:244-253). The NV83DE constructor (gpu/gr/kernel_sm_debugger_session.c:213-345) checks RS_ACCESS_DEBUG rights on the app's compute object in the guest kernel and then allocates the object on GSP.

**Who calls it and when.** Only user-space tools, all closed. In the 79 native traces the only control of this group is SET_MODE_MMU_DEBUG from cuda-gdb, status 0 (matrix/traces/610.57.04/cuda-gdb.jsonl:2384). The cuda-gdb run did not set a breakpoint (cuda-gdb.out: the program ran to completion). Its debugger sequence was: alloc 83DE (:2359), alloc 0x79 event under it (:2380), SET_MODE_MMU_DEBUG action 1 (:2384), DEBUG_WRITE_MEMORY 0x83de0316 twice (:2390, :2489). Separately, every CUDA process allocates its own 83DE and sends DEBUG_SET_EXCEPTION_MASK 0x83de0309 (cuda-gdb.jsonl:2055, :2194; also cuda-launch.tsv:1740-1882, cuda-torch.tsv:1735). The device already handles both: generic alloc ACK in `crates/vfio-user-nvgpu/src/ctrl.rs:695-746` and the exception mask in `cuda.rs:47-48`. The NVML trace has no perfmon reservation. NVML uses PERF_GET_VID_ENG_PERFMON_SAMPLE 0x20802087 and GPUACCT_GET_ACCOUNTING_STATE 0xb02 (nvml.jsonl), so utilization sampling does not reach this group. Kernel-side senders: GR_CTXSW_PM_MODE after `kgrctxSetupDeferredPmBuffer` (gpu/gr/kernel_graphics.c:3984-4036); PERF_RESERVE_PERFMON_HW (kern_perf_pm.c:254-277); the INTERNAL B0CC halves from the user B0CC controls (profiler_v2/kern_profiler_v2_ctrl.c:107, :176, :225, :243, :316). Unreachable from a GSP-client guest: INTERNAL_PERF_PERFMON_CLIENT_RESERVATION_CHECK/SET, whose only sender `kPerfPerfmonClientDeviceSet` (kern_perf_pm.c:52) has no caller; the user ALLOC/FREE_PMA_STREAM, handled in the kernel, whose RPC at :381/:523 is the vGPU-guest helper; and QUIESCE_PMA_CHANNEL (vGPU-guest free paths only, kern_profiler_v2.c:323, kern_profiler_v2_ctrl.c:516). The rmapi_utils.c:220-221 hits for NV90CC power are a vGPU-guest skip-error table, not a sender. Stock tool behaviour on failure is closed. Nsight Compute and CUPTI report missing counter permission (ERR_NVGPUCTRPERM) when the profiler object cannot be created. That is known tool behaviour, not visible in the tree, and has to be measured.

**Truth source.** None for anything that is refused. Guest state for SET_MODE_MMU_DEBUG and SM_DEBUG_MODE_DISABLE. Host RO cannot answer anything here: every control is a setter or needs a reserved profiler object. On the host, RmProfilingAdminOnly=1 would refuse a host profiler to the unprivileged device process anyway.

**Isolation risk.** This group carries the biggest isolation risk in category D. HWPM, PMA, SMPC and the PC sampler are chip-wide. A device-scope reservation or EXEC_REG_OPS would stream counters that include the host desktop's work into the guest and lock host tools out. POWER_REQUEST_FEATURES and DISABLE_DYNAMIC_MMA_BOOST change clock gating, power gating and boost policy of the physical GPU. SMPC_MODE with hChannel 0 switches the global mode for every context. The guest kernel's own permission gate does not protect the host: guest root always passes it, and PERMISSIONS_INIT carries whatever the guest kernel decided. The device is therefore the enforcement point. SM debugging acts on the guest's host mirror context. Suspend, resume and SM debug mode are per-context and could be mirrored safely. The single-SM triggers and single-step act on physical SMs while the context is resident. DROP_DEFERRED_RC and a non-preemptable context could leave GR blocked for the host desktop (ctxsw timeout suppressed). The ACKed SET_MODE_MMU_DEBUG changes nothing on the host. The only effect is that a guest fault is RC'd instead of being handed to the debugger.

**Design proposal.** In `ctrl.rs`, next to the generic GSP_RM_ALLOC ACK (ctrl.rs:695-746), add a class rule: 0x90cc GF100_PROFILER, 0xb0cc MAXWELL_PROFILER, 0xb1cc MAXWELL_PROFILER_CONTEXT and 0xb2cc MAXWELL_PROFILER_DEVICE answer NV_ERR_INSUFFICIENT_PERMISSIONS with label `denied (profiling is admin-only on the host)`. All four are on TU106 (615 src/nvidia/generated/g_gpu_class_list.c:291-295, :357). This is the same answer a stock system gives a user under RmProfilingAdminOnly=1, so tools print their standard message. Then two generic control rules keyed on the class byte, labelled D9: (a) DENY, NV_ERR_INSUFFICIENT_PERMISSIONS for PERF_RESERVE_PERFMON_HW and PERMISSIONS_INIT, NV_ERR_NOT_SUPPORTED for the other DENY ids (reservations, regops, power, MMA boost, SMPC mode, INTERNAL alloc/bind/reserve); (b) REFUSE, NV_ERR_NOT_SUPPORTED for the rest. Exceptions: SM_DEBUG_MODE_DISABLE is ACKed, and SET_MODE_MMU_DEBUG goes to a new `MODEL:debugger-session` (a small map in `cuda.rs`, next to the exception-mask answer) that records the action and echoes it through GET_MODE_MMU_DEBUG 0x83de0308. The MIRROR tier for cuda-gdb breakpoints is deferred: a host 83DE on the mirror compute object (`mirror.rs` add_engine_object), per-context controls forwarded after checking the target is the guest's mirror, and SM exception events translated. Tests: unit tests against the generated ABI (param sizes, including PERMISSIONS_INIT 7 bytes in 610 and 6 in 615), per-id answer and status tables, and a check that no id of the group reaches `forward_or`. Gate runs: a CUDA gate under cuda-gdb, and ncu on the same kernel expecting the permission error with the guest otherwise healthy.

**Estimate.** 2 to 3 engineer-days for the refusal set: the class rule, the two generic rules, the debugger-session state, unit tests, one cuda-gdb and one ncu gate. The mirrored cuda-gdb-with-breakpoints tier would add 12 to 20 days (handle translation, event translation, per-context forwarding, host preemption interplay) and is not recommended for the roadmap.

**Client likelihood.** Group rank 3 (tools only). SET_MODE_MMU_DEBUG decides it: it is the only control of the group seen on the real card (cuda-gdb). The profiler allocations decide the answer shape (Nsight Compute, CUPTI).

**Roadmap.** Nobody on the Linux CUDA, Wayland/KMS with NVENC, or Vulkan-games paths needs this group. The refusal set is worth doing with the next CUDA gate work because it is cheap and it pins down the isolation rule. It depends on the existing 83DE alloc and exception-mask handling (`cuda.rs`) and, for a later MIRROR tier, on the `mirror.rs` host compute objects.

**Reclassification.** 11 to E: INTERNAL_PERF_PERFMON_CLIENT_RESERVATION_CHECK and _SET (no live sender); NVB0CC ALLOC_PMA_STREAM, FREE_PMA_STREAM and INTERNAL_QUIESCE_PMA_CHANNEL (vGPU-guest paths only); NV83DE SET_NON_PREEMPTABLE_DEBUGGER_CONTEXT and SET_RECOVERY_SUPPRESSION_TIMEOUT (NV_ERR_NOT_SUPPORTED on Linux); SET_MODE_MMU_GCC_DEBUG (Blackwell-onwards); RESERVE_HES and RELEASE_HES (Blackwell HES); NVC763 VIDMEM_ACCESS_BIT_DUMP (class not on TU106, access bits off in uvm_turing.c:65). None belong to A, B or C: no control here is a static getter, and no answer can safely come from the host.

**Measurements.**
- Guest, device with the refusal set: run the existing CUDA probe under cuda-gdb twice, without and with a breakpoint in the kernel. Record which 83DE controls arrive and cuda-gdb's reaction to the refused SM_DEBUG_MODE_ENABLE (error text or silent fallback). This settles whether ACKing SET_MODE_MMU_DEBUG is enough for "run under cuda-gdb".
- Guest: `ncu --metrics sm__cycles_elapsed.sum` on the probe kernel, as guest root and as a user. Expect the permission error at the 0xb2cc alloc, and check that the process and the device stay healthy. Then `nsys profile` with default options, to confirm that plain tracing touches none of these controls.
- Host, read-only file access with no RM client: read RmProfilingAdminOnly from /proc/driver/nvidia/params, to document the host's own setting next to the device's rule.
- Open: SET_HS_CREDITS and DISABLE_DYNAMIC_MMA_BOOST support on TU106. They cannot be read without a host profiler object, and with DENY/REFUSE the answer does not depend on them.

### 6.17 Other (GPU accounting) (2)

| Control | Id | Bytes 610/615 | Priv | Answer | Rank | Truth | Caller and when (615 tree) |
|---|---|---|---|---|---|---|---|
| `NV0000_CTRL_CMD_GPUACCT_SET_ACCOUNTING_STATE` | 0x00000B01 | 12/12 | admin | ACK | 2 | guest state | 615 src/nvidia/src/kernel/rmapi/client_resource.c:2046-2059 sends the same cmd to GSP in GSP-client mode, then :2061-2073 flips the kernel's own gpuacct state (gpuacctEnableAccounting, 615 src/nvidia/src/kernel/diagnostics/gpu_acct.c:1558); user space: nvidia-smi --accounting-mode (privileged, flag 0x4); no trace hit |
| `NV0000_CTRL_CMD_GPUACCT_CLEAR_ACCOUNTING_DATA` | 0x00000B05 | 8/8 | admin | REFUSE | 2 | none | 615 src/nvidia/src/kernel/rmapi/client_resource.c:2118-2130 sends it to GSP only with IS_VGPU_GSP_PLUGIN_OFFLOAD_ENABLED and pid != 0, else :2136 clears the kernel's own data; user space: nvidia-smi --clear-accounted-apps |

**Subfamilies.** 2 controls on NV01_ROOT GPU accounting: GPUACCT_SET_ACCOUNTING_STATE 0xB01 and GPUACCT_CLEAR_ACCOUNTING_DATA 0xB05. Both are privileged (flag 0x4). 0x20800A3E is not in this input; it is in group gr.

**State a device model needs.** Per gpuId, the accounting-enabled flag as the guest set it, per device, lost on guest driver reload, like the kernel's own state. A later sampler would also need the per-channel busy time the channel registry already tracks, keyed by guest PID.

**What GSP-RM does.** Closed firmware. In GSP-client mode the kernel sends SET to GSP (615 src/nvidia/src/kernel/rmapi/client_resource.c:2046-2059) and, on NV_OK, updates its own GpuAccounting state (:2061-2073, gpuacctEnableAccounting in 615 src/nvidia/src/kernel/diagnostics/gpu_acct.c:1558). The kernel skips its own 1 s sampling timer for GSP clients (gpu_acct.c:1480-1497). Utilization therefore has to come from GSP-RM as NV_VGPU_MSG_EVENT_GPUACCT_PERFMON_UTIL_SAMPLES, handled at gpu/gsp/kernel_gsp.c:1621 and :919-946. Inferred: SET starts or stops that per-process sampling on GSP. CLEAR goes to GSP only with vGPU-on-GSP plugin offload and a non-zero pid (client_resource.c:2118-2130). Otherwise the kernel clears its own data (:2136). Header: ctrl0000/ctrl0000gpuacct.h:40-78 and :230-252.

**Who calls it and when.** nvidia-smi accounting admin operations (`--accounting-mode`, `--clear-accounted-apps`) as root. No hit in the native traces: the NVML probe reads only GET_ACCOUNTING_STATE 0xb02 (nvml.jsonl). No kernel-internal sender.

**Truth source.** Guest state. The host is never asked: host accounting state is host-wide and host PIDs must not reach the guest.

**Isolation risk.** High only if forwarded. SET on the host would turn on accounting for the physical GPU, and the accounting data carries host PIDs. ACK from guest state has no host effect.

**Design proposal.** In `nvml.rs` (the nvidia-smi answer set), SET_ACCOUNTING_STATE gets ACK with label `guest (accounting state recorded; no samples)`. The guest kernel then keeps its own accounting tables, so `nvidia-smi -q -d ACCOUNTING` shows Enabled and accounted apps appear with 0 utilization. CLEAR gets REFUSE (never arrives). Optional follow-up `MODEL:gpuacct`: emit UTIL_SAMPLES events once per second from the channel registry's per-guest-PID busy time. That is also the source nvml.md proposes for per-VM utilization (docs/caraxes/nvml.md:234). Tests: unit test of the 12- and 8-byte layouts against the generated ABI; a guest gate with `nvidia-smi -am 1`, a CUDA probe, `nvidia-smi --query-accounted-apps=pid,gpu_util,max_memory_usage --format=csv`, then `-am 0`.

**Estimate.** 0.5 to 1 engineer-day for the ACK/REFUSE pair with tests and the gate. Plus 1 to 2 days for the utilization-sample events.

**Client likelihood.** Rank 2: nvidia-smi admin operations. SET_ACCOUNTING_STATE decides it.

**Roadmap.** Linux CUDA client, as an nvidia-smi completeness item next to the NVML work in `nvml.rs`. No dependency beyond the existing NVML profile. The sampler depends on the channel registry's busy-time accounting.

**Reclassification.** 1 to E: GPUACCT_CLEAR_ACCOUNTING_DATA never reaches GSP from a non-vGPU GSP-client guest (client_resource.c:2118).

**Measurements.**
- Guest: the accounting gate above with the ACK, capturing the device trace. Confirm that SET arrives once per call, that CLEAR never arrives, and what nvidia-smi prints for utilization and max memory of an accounted app when no UTIL_SAMPLES event is sent.
- Guest: `nvidia-smi -q -d ACCOUNTING` before and after, to confirm that GET 0xb02 is answered by the guest kernel from its own state (no device traffic).

## 7. Roadmap

Ordered. Each step names what it unblocks and what it needs first. Group numbers refer to §6.

**R0. The rule table (2 to 3 days, first, independent).** `dmodel.rs` with the per-control
answer kind from d-controls.tsv for every REFUSE, DENY, ACK and TABLE row (about 260 controls),
each with its D9 label and, where the header names one, the specific status. The display rows go
into the board table of `display/mod.rs` instead (6.5 rule 1), and `is_display_cmd` is extended to
the NV2080 display ids and NV9072. Tests:
- every D id of the 615 inventory has exactly one answer;
- no id of D ever appears in `FORWARD`, the GSS legacy list, `BINAPI` or `MIRROR_FORWARDED` (the
  guard that keeps host-wide setters off the host, 6.1, 6.4, 6.16);
- the profiling class rule (6.16): GSP_RM_ALLOC of 0x90CC, 0xB0CC, 0xB1CC and 0xB2CC answers
  NV_ERR_INSUFFICIENT_PERMISSIONS.

After R0 the only remaining `unanswered` answers in D are the MODEL and MIRROR rows. This step
also moves the 102 E rows into the E family rules and the 7 A rows into the table handler, so it
overlaps with the E and A work that is running now.

**R1. Wayland/KMS display with NVENC (display goal; 1 to 2 days on top of R0).**
- `SPECIFIC_HDCP_CTRL` per cmd (6.5): needed now, it is hit on every unplug.
- `DP_SET_WAR_FLAGS` ACK: needed with the move to the 615 guest (D19).
- `PERF_NOTIFY_VIDEOEVENT` ACK (6.9): for Sunshine's NVENC once the NVENC mirror lands.
- Later, with milestone G (4K144 with DSC): `DP_CONFIGURE_FEC` (6.5), then `GET_RG_STATUS` if the
  mode needs 2Heads1OR.
- Depends on: the `ver/v615` display module, the DP sink model (exists).

**R2. Linux CUDA client, error and teardown paths (5 to 9 days).** In order:
1. `FIFO_DISABLE_CHANNELS` on the guest's own mirror channels (6.1; really C), for a CUDA process
   killed with live channels.
2. `GPU_REPORT_NON_REPLAYABLE_FAULT` (6.10; C) and the RC reflection.
3. `INTERNAL_RC_WATCHDOG_TIMEOUT` and `INTERNAL_LOG_OOB_XID` (6.8), after the watchdog is enabled
   in the guest again (the guest registry carries `RmRcWatchdog=0` today, as the captured 615
   GSP_INIT shows, `crates/vfio-user-nvgpu/src/gmcapi.rs:790`).
4. `GPU_SET_COMPUTE_MODE_RULES` and `GPUACCT_SET_ACCOUNTING_STATE` for nvidia-smi admin
   (6.2, 6.17).
5. The BAR0 mmap policy `GPU_VALIDATE_MEM_MAP_REQUEST` (6.4; C), which makes the status honest.

Depends on:
- the event emitter of gsp-surface.md §4 (RC_TRIGGERED, OS_ERROR_LOG);
- category C's RESET_CHANNEL on mirror channels;
- the mirror (exists).

**R3. Vulkan games (Shadow of the Tomb Raider, the decisive benchmark; 4 to 7 days).**
- The ZBC table model (6.2): guest-local, seeded once from a read-only host sweep of
  `GET_ZBC_CLEAR_TABLE_*`, never written to the host's global table, and also answering the
  `GET_ZBC_*` rows of category A that every GL, EGL and Vulkan trace sends.
- `GR_CTXSW_PREEMPTION_BIND` and `ZCULL_MODE` on mirror channels (6.2; C).
- NV9072 `NOTIFY_ON_VBLANK` and the timed semaphore (6.14), only if the measurements (§8 Q4, Q9)
  show the classes allocated.
- The VRR trio as ACK once VRR is offered.

Depends on: the D16 filter (ZBC kinds are compressible on Turing), the event emitter (NV9072,
NV9074).

**R4. Suspend and resume of the guest (3 to 5 days).**
- `FIFO_DISABLE_USERMODE_CHANNELS` and `INTERNAL_FIFO_TOGGLE_ACTIVE_CHANNEL_SCHEDULING`, over the
  guest's mirror channels only (6.1, 6.11).
- `INTERNAL_FBSR_INIT` recorded (6.3).
- `DMA_FLUSH` onto the existing host L2 flush.
- None of the three milestones needs this step. It comes first when VM suspend or live migration
  becomes a goal.

**R5. Tools (2 to 4 days, whenever convenient).**
- The debugger-session model for `DEBUG_SET_MODE_MMU_DEBUG`, so that cuda-gdb starts (6.16).
- `PERF_RATED_TDP_SET_CONTROL` as recorded requests (6.6).
- The GSP trace and uStreamer refusals are already in R0.
- Mirrored SM debugging with breakpoints (12 to 20 days) is not recommended.

**Nobody on this chip (move to E, 102 controls).**
- Display (22): FRL, DP 2.x, shared generic packets, NV Display 5.0 SOR flush, Tegra DSI and
  safety, LVDS, two Windows-only workaround notifications.
- Perf and thermal (20): the notebook platform-request handler, AUX power, SLI boost sync,
  Dynamic Boost, RTD3/GCX.
- ECC and RAS (9): TU106 has no ECC or row remapping; MODS-only watchdog info; fabric-manager
  recovery.
- Profiling (11): Blackwell-only debugger and HES controls, vGPU-only PMA paths, perfmon
  reservation without a sender, the access-bit buffer.
- FIFO, GR, memory, events and vGPU (25): MIG, the deferred-API class that no trace allocates,
  kernel-privileged controls without a caller, verification builds, Ampere-or-later and
  Hopper-or-later features.
- Bus, NVLink, security and GPIO (14): vGPU-host P2P caps, NVLink P2P mappings, the GA10x PCIe
  relaxed-ordering and Windows-only sysmem-access controls, the SLI bridge
  and its GPIOs, DRAM encryption.
- Other (1): GPU accounting clear.
- The per-control reason is in the `reclass` column of d-controls.tsv.

**Dependencies between groups.**
- 6.1 FIFO and 6.11 vGPU share the suspend path.
- 6.10 events, 6.8 RC and the RESET_CHANNEL work of category C share the RC reflection
  (gr-completion.md item g).
- 6.5 display (NV9072), 6.14 timers and 6.8 RC all need the gsp-surface.md §4 event emitter.
- 6.2 GR (ZBC) and 6.3 memory (CBC) both lean on D16.
- 6.16 profiling depends only on the class-allocation hook in ctrl.rs `Dispatcher::alloc`.

## 8. Open questions that need a measurement

Guest runs touch only the emulated device. Host reads are read-only controls issued by the device
process as an RM client on its own host objects; each one touches the host GPU through its RM and
needs the operator's go-ahead first, like every host read (host-facts.md). None of the
measurements writes host state.

| # | Question | Guest run | Host read (with go-ahead) | Decides |
|---|---|---|---|---|
| Q1 | Does a 615 guest send `DP_SET_WAR_FLAGS` and `HDCP_CTRL` exactly as predicted, and are they the only new D ids of the DP path? | 615 guest, DP plug and unplug through the control file, device trace | none | R1; ranks of 6.5 |
| Q2 | Which controls does 4K144 with DSC add (FEC, merge mode)? | 4K144 DSC with `HEAD_CLK_CAP` above and below the pixel clock; expect `DP_CONFIGURE_FEC`, and `GET_RG_STATUS` only with merge mode | none | 6.5 milestone G |
| Q3 | What reaches the device when a CUDA process dies with live channels, or faults fatally? | `kill -9` of a running CUDA probe; a provoked fatal non-replayable fault under UVM; look for 0x2080110B, 0x20800177 and the RC order | none | R2 items 1 and 2 |
| Q4 | Does GL, EGL or Vulkan vsync allocate `GF100_DISP_SW` (0x9072) or `GF100_TIMED_SEMAPHORE_SW` (0x9074)? | eglprobe with swap interval 1, a Vulkan FIFO swapchain under Weston, VDPAU presentation queue, SotTR; look for the allocations and 0x90720101, 0x9074010x | none | 6.5 NV9072 and 6.14 rank; R3 |
| Q5 | Which ZBC clear colours does a game set, and what is the real default table? | vk-rt, gl-render, SotTR: count 0x9096010x | one sweep of `GET_ZBC_CLEAR_TABLE_SIZE` and `_ENTRY` on a `GF100_ZBC_CLEAR` object of the device's own client | the ZBC seed (6.2) |
| Q6 | In which order does a guest suspend send `DISABLE_USERMODE_CHANNELS`, `TOGGLE_ACTIVE_CHANNEL_SCHEDULING` and `FBSR_INIT`? | suspend and resume with `NVreg_PreserveVideoMemoryAllocations=1`, pure emulation | none | R4 |
| Q7 | Does the watchdog path work once `RmRcWatchdog` is on, and what does a stalled watchdog channel send? | boot without the registry key, CUDA gate, then stall the watchdog channel; with GNOME watch for Xid 10 and 0x20800A56 | `GET_RC_RECOVERY` 0x2080220E and `GET_RC_INFO` 0x20802212 for the defaults | 6.8, R2 item 3 |
| Q8 | What does cuda-gdb do with a breakpoint, and how do cuda-gdb and Nsight Compute react to the refusals? | cuda-gdb with and without a kernel breakpoint; `ncu` as root and as a user (expect a permission error, device healthy); `nsys` defaults (expect none of these controls) | none; `/proc/driver/nvidia/params` for `RmProfilingAdminOnly` is a plain file read | 6.16 answer shape, R5 |
| Q9 | Does NVENC through Sunshine or ffmpeg send `PERF_NOTIFY_VIDEOEVENT`? | Sunshine or ffmpeg NVENC in the guest once the NVENC mirror exists | none | 6.9 |
| Q10 | Which BAR0 ranges does a non-root guest process try to map, and what does the real GSP-RM approve? | a non-root program mapping PTIMER, usermode, PMC and one other BAR0 page, then as root (root skips the check) | optional: an unprivileged mmap of one host BAR0 range, unmapped without access | 6.4 policy (deny-by-default either way) |
| Q11 | Does TU106 support `RATED_TDP`, and does anything in the guest send `SET_CONTROL`? | `ncu --clock-control base` and `none`, device trace | `PERF_RATED_TDP_GET_CONTROL` 0x2080206E for the client slots | 6.6 MODEL or REFUSE |
| Q12 | Does the host GSP-RM support DIFR and does it list the synthetic EDID as adaptive-sync? | none | `LPWR_DIFR_CTRL` support status; `SPECIFIC_DEFAULT_ADAPTIVESYNC_DISPLAY` with the synthetic EDID ids | 6.5 rank 4 versus E; the TABLE answer |
| Q13 | Does the host report a TPC partition mode for the device's own mirror TSG? | none | `GR_GET_TPC_PARTITION_MODE` 0x00801107 on a mirror TSG | 6.11 SET_TPC_PARTITION_MODE REFUSE or mirror |
| Q14 | Does a stock client sweep hit the I2C, LED or PCIe setters? | `nvidia-smi -q`, `-q -d PCIE`, `nvidia-settings -q all`, `ddcutil detect`, `sensors-detect`, device trace | none | 6.4 and 6.15 ranks |

Q1, Q3, Q4 and Q6 are pure guest runs and settle most of the ranks. Q5 is the only host read on a
milestone path (R3).

## 9. How the numbers were made

- Inventory: `python3 caraxes/tools/gsp-surface.py --ogkm <tree> --full --json out.json` for the
  610 tree (`vendor/open-gpu-kernel-modules` of the main checkout) and the 615 tree (the Leandro
  vendor checkout at 615.71.09), at HEAD 61e0b6a. D is `routed and not handled and category D`.
  The native-trace column was produced with the traces directory pointed at
  `matrix/traces/610.57.04/control` for both trees (there are no 615 traces). The root-level
  `cuda-gdb.jsonl` is outside `control/` and was scanned separately.
- Sizes: one C file per tree that includes every header under `src/common/sdk/nvidia/inc/ctrl/`
  and prints `sizeof` of each D parameter struct, `cc -std=gnu11 -DNV_LINUX -I<sdk inc>`. Layout
  changes: the comment-stripped text of each `typedef struct ... } NAME;` (aliases resolved)
  compared between the trees.
- Kernel-side bodies: definitions of the NVOC handler name with `_IMPL`, `_KERNEL` or no suffix
  under `src/nvidia/src` and `src/nvidia/arch` of the 615 tree.
- Run logs: every `cmd=0x...` in `caraxes/runs/**` (device traces and logs, 610 and 615 guests,
  including the uncommitted runs of 2026-09-24) matched against the D ids.
- In/out markings: the SDK comment block of each control, searched for `[in]`, `[out]` and
  `[in/out]`.
- Per-control reading (answer, rank, client, caller, gsp, truth, risk, reclass): five read-only
  helper agents, one per slice, each over the 615 tree with 610 lines where the code differs;
  collected in d-controls.tsv. The group sections in §6 are theirs, edited for form.
