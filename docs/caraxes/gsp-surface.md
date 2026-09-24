<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
<!-- Source survey and tooling by a read-only subagent on 2026-09-24 (operator's direction of the same day: emulate as many device answers as possible, so that Windows can run later without trace-driven bring-up). Numbers are the output of caraxes/tools/gsp-surface.py on the working tree of that day (HEAD fd1e5e7 plus the uncommitted crate changes); rerun the script for current numbers. -->
# The GSP surface of 610.57.04: what a GSP-RM stand-in must answer

Caraxes plays GSP-RM towards a stock kernel RM in GSP-client mode. Until now the device has
answered what the Linux driver happened to ask on our gates, and every unanswered RPC showed up
as a wall. This file inventories, from source only, everything the kernel RM of this driver
generation can send to GSP, marks what the device handles today and with which D9 provenance,
and sorts the rest into five categories with a generic recipe for each. It is the work list for
making the device complete systematically, so that another client of the same RM core (the
Windows driver of the same generation, later nova-core) finds its answers without a trace.

The inventory is produced by [`caraxes/tools/gsp-surface.py`](../../caraxes/tools/gsp-surface.py)
(python3, standard library, about one second):

```
python3 caraxes/tools/gsp-surface.py                 # the summary below
python3 caraxes/tools/gsp-surface.py --full          # plus every control, by category
python3 caraxes/tools/gsp-surface.py --json out.json # the raw inventory
python3 caraxes/tools/gsp-surface.py --explain       # how each handled id was read
python3 caraxes/tools/gsp-surface.py --top 30        # unhandled, ranked by call sites and traces
python3 caraxes/tools/gsp-surface.py --ogkm DIR      # another driver tree (or CARAXES_OGKM=DIR)
```

## Summary

- **The surface.** 1377 exported controls in the NVOC tables; **829 reach GSP** (ROUTE_TO_PHYSICAL,
  or sent by the kernel RM itself; 239 of them INTERNAL). 230 RPC function numbers, of which a
  GSP-client kernel RM sends 20 (15 regularly, 5 on some paths); 41 event numbers, of which the
  CPU side dispatches 37. 222 classes, of which 149 reach GSP on allocation.
- **Handled today.** 171 of the 829 routed controls (host-forwarded 37, synthesized 56,
  guest-derived 21, echo 46, refused 11), plus 38 GSS-legacy and BINAPI ids outside the export
  tables (all host-forwarded). 11 of the 20 RPC functions. 2 of the 37 events (GSP_INIT_DONE,
  POST_EVENT). All 149 GSP-bound classes are acknowledged; 21 are modelled or recorded by number.
- **Unhandled: 658 routed controls.** A pure chip data 62, B host-forwardable 71, C guest-state
  derived 30, D needs device modelling 306, E never expected from a stock client 189.
- **What that means in work.** A and E are cheap and mechanical (a table-driven answer and a
  per-family refusal rule, about 5 agent-days together). B is an allowlist with an isolation
  review (2 days). C belongs to the channel registry (5 to 8 days). D is where the real work is:
  about 300 controls, of which roughly 105 display link, HDMI and DP operations and 45
  profiler and debugger controls; a generic "acknowledge and log" policy covers the setters without an
  observable result, the rest needs engine behaviour (20 to 35 days). The RPC and event gaps
  are small (3 to 5 days).
- **Native traces agree.** Every ROUTE_TO_PHYSICAL control that appears in the native CUDA traces
  (`matrix/traces/610.57.04/control/cuda-*.jsonl`) is already handled; the unhandled controls
  seen on the real card come from the EGL, GL, Vulkan, NVDEC/NVENC and NVML probes. The next
  walls are graphics clients (a Wayland compositor on EGL), display connectors other than DP,
  and the error and teardown paths of CUDA.

## 1. Method

**Vendor side** (all paths under `vendor/open-gpu-kernel-modules/`):

- *RPC functions and events:* the X-macro `X(UNIT, NAME, NUM)` and `E(NAME, NUM)` lists in
  `src/nvidia/inc/kernel/vgpu/rpc_global_enums.h`, with the versioned payload structs from
  `src/nvidia/generated/g_rpc-structures.h` (`rpc_<name>_vXX_YY`). Which functions a GSP client
  sends is taken from the source survey in [gsp-rpc-protocol.md](gsp-rpc-protocol.md) §3 (a
  constant in the script, cited there); which events the CPU dispatches is parsed from the
  `case NV_VGPU_MSG_EVENT_*` list of `_kgspProcessRpcEvent` (`src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:1399`).
- *Controls:* every `__nvoc_exported_method_def_*` table of every `src/nvidia/generated/g_*_nvoc.c`
  (methodId, flags, class, handler name). A control reaches GSP when it has
  `RMCTRL_FLAGS_ROUTE_TO_PHYSICAL` (0x40, `src/nvidia/inc/kernel/rmapi/control.h:230`):
  `rmresControl_Prologue` sends it as `GSP_RM_CONTROL` with the caller's handles
  (`src/nvidia/src/kernel/rmapi/resource.c:254-296`). It also reaches GSP when the kernel RM sends
  it itself: through `NV_RM_RPC_CONTROL` or `GPU_GET_PHYSICAL_RMAPI(pGpu)->Control` with a named
  command, through `rmCtrlParams.cmd = ...` a few lines above such a call, or through a `_KERNEL`
  or `_IMPL` body that forwards its own command (`pRmCtrlParams->cmd`, matched to the export
  entry by the enclosing function name, for example `kchannelCtrlCmdStopChannel_IMPL`,
  `kernel_channel.c:1884`). Calls inside an `if` that tests `IS_VIRTUAL` or a vGPU flag without
  `IS_GSP_CLIENT`/`IS_FW_CLIENT`, and everything under `src/kernel/vgpu/`, are vGPU-guest paths and
  do not count.
- *Classes:* the `RS_ENTRY` table in `src/nvidia/src/kernel/rmapi/resource_list.h`; a class reaches
  GSP when its flags carry `RS_FLAGS_ALLOC_RPC_TO_PHYS_RM` (NVBIT(6),
  `resource_desc_flags.h:39`; `alloc_free.c:878-930`), or when its constructor sends
  `NV_RM_RPC_ALLOC_*` (device.c:186, subdevice.c:124, kernel_channel.c:2761,
  kernel_channel_group_api.c:429, event_api.c:162, vaspace_api.c:263 and a few profiler, P2P
  and fabric constructors, listed in the script).
- *Call sites:* every reference to a control name outside `#define`s, counted per component:
  kernel RM (`src/nvidia/src`, `src/nvidia/arch`), display (`src/nvidia-modeset`,
  `src/common/{modeset,displayport,unix}`, `kernel-open/nvidia-drm`, `kernel-open/nvidia-modeset`)
  and UVM (`kernel-open/nvidia-uvm`). User-space callers (CUDA, EGL, Vulkan, NVML) are not in the
  tree; for them the native traces of the real card (`matrix/traces/610.57.04/control/*.jsonl`,
  ioctl 0x2a) are used as a second signal.

**Crate side** (read only, `crates/vfio-user-nvgpu/src`, test modules cut off): the named
constants `CMD_*`, `FN_*`, `EV_*` of `ver/v610.rs` and `ver/v610/*.rs`; the `match` arms of
`ctrl.rs`, `cuda.rs`, `nvml.rs`, `uvm.rs`, `kgr.rs`, `display/mod.rs` and `boot.rs`; the tables
`FORWARD`, `LEGACY`, `BINAPI` (host), `ACKNOWLEDGED` (echo) and `MIRROR_FORWARDED` (host, only
when a mirror exists); and the 0x90E7 range rule of `nvml.rs`. The provenance of an arm is read
from the D9 label strings in its body and one level of local helpers: a `host ...` label or a
forward call is host-forwarded, `guest ...` is guest-derived, NOT_SUPPORTED without a success
path is refused, an NV_OK answer that returns the request unchanged or says "acknowledged" or
"echo" is echo, everything else (`synthesized`, `card`, `profile`, a chip-table label) is
synthesized. The display fallback arm `c if is_display_cmd(c)` (label `unanswered`) is not
counted as handled. `--explain` prints the reading for every id.

**Limits.** The categories A to E are name and flag heuristics (`classify()` in the script);
they sort 658 items well enough to plan, and every single item should be checked when it is
implemented. The export tables are those of the profile `shipping-gpus-openrm`
(`src/nvidia/generated/rmconfig.h:5`, `RMCFG_FEATURE_PLATFORM_UNIX 1`,
`RMCFG_FEATURE_PLATFORM_WINDOWS 0` at :260-261), so controls compiled only into other profiles are
invisible here (section 5). Which of the routed controls a given client sends at run time is not
decidable from source; the call-site counts and the traces rank, they do not prove.

## 2. The inventory

As printed by the script (section headings demoted by one level).

### Totals

| Item | Count |
|---|---|
| Exported controls (all NVOC classes, unique ids) | 1377 |
| Controls that reach GSP (ROUTE_TO_PHYSICAL or explicit kernel-RM send) | 829 |
| of which INTERNAL (0x80) | 239 |
| of which sent explicitly by kernel RM | 257 |
| Routed controls the device handles | 171 |
| Routed controls unhandled | 658 |
| Handled ids outside the export tables (GSS legacy and others) | 38 |
| RPC functions (rpc_global_enums.h) | 230 |
| of which sent by a GSP-client kernel RM (always or maybe) | 20 |
| of which the device handles | 11 |
| RPC events | 41 |
| of which the CPU side dispatches | 37 |
| of which the device sends | 2 |
| Classes in resource_list.h | 222 |
| of which reach GSP on alloc | 149 |
| of which the crate names | 21 |

### Handled routed controls by provenance

| Provenance | Controls |
|---|---|
| host-forwarded | 37 |
| synthesized | 56 |
| guest-derived | 21 |
| echo | 46 |
| refused | 11 |

Outside the export tables the device also handles 38 ids (host-forwarded 38): the GSS-legacy THERMAL/CLK/PERF/PMGR numbers and the NV2081 BINAPI commands, which have no NVOC export entry.

### Unhandled routed controls by category

| Category | Controls | with a kernel/modeset/UVM call site | seen in a native trace |
|---|---|---|---|
| A pure chip data | 62 | 27 | 4 |
| B host-forwardable as-is | 71 | 28 | 3 |
| C guest-state derived | 30 | 15 | 1 |
| D needs device modelling | 306 | 156 | 1 |
| E never expected from a stock client | 189 | 126 | 1 |

### Per resource class (NVOC class of the exporting object)

| NVOC class | Routed | INTERNAL | Handled | host | synth | guest | echo | refused | Unhandled | A | B | C | D | E |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Subdevice | 470 | 220 | 94 | 35 | 24 | 11 | 19 | 5 | 376 | 39 | 50 | 0 | 119 | 168 |
| DispCommon | 142 | 4 | 45 | 0 | 25 | 3 | 15 | 2 | 97 | 10 | 0 | 0 | 87 | 0 |
| ProfilerBase | 30 | 6 | 0 | 0 | 0 | 0 | 0 | 0 | 30 | 1 | 6 | 0 | 21 | 2 |
| KernelSMDebuggerSession | 25 | 0 | 1 | 0 | 0 | 0 | 1 | 0 | 24 | 0 | 4 | 0 | 20 | 0 |
| NvDispApi | 24 | 0 | 8 | 0 | 2 | 2 | 4 | 0 | 16 | 0 | 0 | 0 | 16 | 0 |
| Device | 23 | 3 | 6 | 1 | 0 | 2 | 3 | 0 | 17 | 7 | 0 | 0 | 9 | 1 |
| KernelChannel | 21 | 2 | 2 | 0 | 1 | 0 | 1 | 0 | 19 | 0 | 0 | 17 | 0 | 2 |
| DiagApi | 16 | 0 | 1 | 0 | 0 | 0 | 0 | 1 | 15 | 0 | 2 | 0 | 11 | 2 |
| KernelChannelGroupApi | 14 | 2 | 4 | 0 | 1 | 1 | 2 | 0 | 10 | 0 | 1 | 9 | 0 | 0 |
| Subdevice (not exported) | 13 | 0 | 2 | 1 | 1 | 0 | 0 | 0 | 11 | 1 | 0 | 0 | 2 | 8 |
| RmClientResource | 7 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 7 | 0 | 4 | 0 | 2 | 1 |
| ZbcApi | 7 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 7 | 2 | 1 | 0 | 4 | 0 |
| GenericEngineApi | 5 | 0 | 3 | 0 | 0 | 0 | 0 | 3 | 2 | 0 | 2 | 0 | 0 | 0 |
| I2cApi | 5 | 0 | 1 | 0 | 1 | 0 | 0 | 0 | 4 | 1 | 1 | 0 | 2 | 0 |
| Profiler | 5 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 5 | 1 | 0 | 0 | 4 | 0 |
| GPUInstanceSubscription | 4 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 4 | 0 | 0 | 0 | 0 | 4 |
| DeferredApiObject | 3 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | 3 | 0 |
| DispSwObj | 3 | 0 | 2 | 0 | 1 | 0 | 1 | 0 | 1 | 0 | 0 | 0 | 1 | 0 |
| KernelCtxShareApi | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 3 | 0 | 0 | 3 | 0 | 0 |
| TimedSemaSwObject | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 2 | 0 | 0 | 0 | 2 | 0 |
| AccessCounterBuffer | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 1 | 0 | 0 |
| Device (not exported) | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 1 | 0 |
| DispCommon (not exported) | 1 | 0 | 1 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| DispSwObject | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 1 | 0 |
| MemoryFabric | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 1 |
| VaSpaceApi | 1 | 0 | 1 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| VidmemAccessBitBuffer | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 0 | 1 | 0 |

### RPC functions

Only functions a GSP-client kernel RM sends, nova-core uses, or the device handles; the other 199 are vGPU-guest (IS_VIRTUAL) or deprecated numbers.

| # | Function | Struct versions | GSP client | nova-core | Device |
|---|---|---|---|---|---|
| 0 | NOP | - | - | decodes | not sent to GSP by either client |
| 1 | SET_GUEST_SYSTEM_INFO | v03_00 | sent | decodes | answered (`crates/vfio-user-nvgpu/src/boot.rs:780`) |
| 2 | ALLOC_ROOT | v07_00 | - | decodes | not sent to GSP by either client |
| 3 | ALLOC_DEVICE | - | - | decodes | not sent to GSP by either client |
| 4 | ALLOC_MEMORY | v13_01 | sent (memRegisterWithGsp) | decodes | **unhandled** |
| 5 | ALLOC_CTX_DMA | - | - | decodes | not sent to GSP by either client |
| 6 | ALLOC_CHANNEL_DMA | v1F_04 | - | decodes | not sent to GSP by either client |
| 7 | MAP_MEMORY | - | - | decodes | not sent to GSP by either client |
| 8 | BIND_CTX_DMA | - | - | decodes | not sent to GSP by either client |
| 9 | ALLOC_OBJECT | v25_08, v26_00, v27_00, v29_06, v2B_04, v2C_01 | - | decodes | not sent to GSP by either client |
| 10 | FREE | v03_00 | sent | decodes | answered (`crates/vfio-user-nvgpu/src/boot.rs:902`) |
| 11 | LOG | v03_00 | - | decodes | not sent to GSP by either client |
| 14 | MAP_MEMORY_DMA | v2C_05 | maybe | - | **unhandled** |
| 15 | UNMAP_MEMORY_DMA | v2C_05 | maybe | - | **unhandled** |
| 21 | DUP_OBJECT | v03_00 | sent | - | answered (`crates/vfio-user-nvgpu/src/boot.rs:917`) |
| 22 | IDLE_CHANNELS | v03_00 | maybe | - | **unhandled** |
| 47 | UNLOADING_GUEST_DRIVER | v1F_07 | sent (unload) | sends | **unhandled** |
| 50 | GPU_EXEC_REG_OPS | v12_01 | maybe | - | **unhandled** |
| 51 | GET_STATIC_INFO | - | - | decodes | not sent to GSP by either client |
| 65 | GET_GSP_STATIC_INFO | v14_00 | sent | sends | answered (`crates/vfio-user-nvgpu/src/boot.rs:793`) |
| 70 | UPDATE_BAR_PDE | v15_00 | sent | - | answered (`crates/vfio-user-nvgpu/src/boot.rs:946`) |
| 71 | CONTINUATION_RECORD | - | sent (large RPCs) | sends | handled in the queue code (`crates/vfio-user-nvgpu/src/boot.rs:709`) |
| 72 | GSP_SET_SYSTEM_INFO | v17_00 | sent | sends | answered (`crates/vfio-user-nvgpu/src/boot.rs:771`) |
| 73 | SET_REGISTRY | - | sent | sends | consumed (no reply) (`crates/vfio-user-nvgpu/src/boot.rs:779`) |
| 74 | GSP_INIT_POST_OBJGPU | - | - | decodes | not sent to GSP by either client |
| 76 | GSP_RM_CONTROL | v03_00 | sent | decodes | answered (`crates/vfio-user-nvgpu/src/boot.rs:808`) |
| 78 | DUMP_PROTOBUF_COMPONENT | v18_12 | sent (debug dump) | - | **unhandled** |
| 103 | GSP_RM_ALLOC | v03_00 | sent | - | answered (`crates/vfio-user-nvgpu/src/boot.rs:845`) |
| 200 | INVALIDATE_TLB | v23_03 | maybe | - | **unhandled** |
| 202 | ECC_NOTIFIER_WRITE_ACK | - | sent (RC path) | - | **unhandled** |
| 228 | INIT_GSP_TRACE_CRASH_BUFFER | v03_00 | sent (regkey) | - | answered (`crates/vfio-user-nvgpu/src/boot.rs:937`) |

### RPC events (GSP to CPU)

| Event | Name | CPU dispatches | nova-core | Device sends |
|---|---|---|---|---|
| 0x1001 | GSP_INIT_DONE | yes | waits | yes |
| 0x1002 | GSP_RUN_CPU_SEQUENCER | no | waits | no |
| 0x1003 | POST_EVENT | yes | decodes | yes |
| 0x1004 | RC_TRIGGERED | yes | decodes | no |
| 0x1005 | MMU_FAULT_QUEUED | yes | decodes | no |
| 0x1006 | OS_ERROR_LOG | yes | decodes | no |
| 0x1007 | RG_LINE_INTR | no | - | no |
| 0x1008 | GPUACCT_PERFMON_UTIL_SAMPLES | yes | - | no |
| 0x1009 | SIM_READ | yes | - | no |
| 0x100a | SIM_WRITE | yes | - | no |
| 0x100b | SEMAPHORE_SCHEDULE_CALLBACK | yes | - | no |
| 0x100c | UCODE_LIBOS_PRINT | yes | decodes | no |
| 0x100d | VGPU_GSP_PLUGIN_TRIGGERED | yes | - | no |
| 0x100e | PERF_GPU_BOOST_SYNC_LIMITS_CALLBACK | yes | - | no |
| 0x100f | PERF_BRIDGELESS_INFO_UPDATE | yes | - | no |
| 0x1010 | VGPU_CONFIG | yes | - | no |
| 0x1011 | DISPLAY_MODESET | no | - | no |
| 0x1012 | EXTDEV_INTR_SERVICE | yes | - | no |
| 0x1013 | NVLINK_INBAND_RECEIVED_DATA_256 | yes | - | no |
| 0x1014 | NVLINK_INBAND_RECEIVED_DATA_512 | yes | - | no |
| 0x1015 | NVLINK_INBAND_RECEIVED_DATA_1024 | yes | - | no |
| 0x1016 | NVLINK_INBAND_RECEIVED_DATA_2048 | yes | - | no |
| 0x1017 | NVLINK_INBAND_RECEIVED_DATA_4096 | yes | - | no |
| 0x1018 | TIMED_SEMAPHORE_RELEASE | yes | - | no |
| 0x1019 | NVLINK_IS_GPU_DEGRADED | yes | - | no |
| 0x101a | PFM_REQ_HNDLR_STATE_SYNC_CALLBACK | yes | - | no |
| 0x101b | NVLINK_FAULT_UP | yes | - | no |
| 0x101c | GSP_LOCKDOWN_NOTICE | yes | decodes | no |
| 0x101d | MIG_CI_CONFIG_UPDATE | yes | - | no |
| 0x101e | UPDATE_GSP_TRACE | yes | - | no |
| 0x101f | NVLINK_FATAL_ERROR_RECOVERY | yes | - | no |
| 0x1020 | GSP_POST_NOCAT_RECORD | yes | decodes | no |
| 0x1021 | FECS_ERROR | yes | - | no |
| 0x1022 | RECOVERY_ACTION | yes | - | no |
| 0x1023 | TRIGGER_BUGCHECK | yes | - | no |
| 0x1024 | BIND_BAR2 | no | - | no |
| 0x1025 | FORCED_DRIVER_SHUTDOWN | yes | - | no |
| 0x1026 | GSP_LOAD_EXEC_GENERIC_BOOTLOADER | yes | - | no |
| 0x1027 | GSP_LOAD_EXEC_HS_BINARY | yes | - | no |
| 0x1028 | UPDATE_GRID_DISPLAYLESS_PARAMS | yes | - | no |
| 0x1029 | PMU_HALTED | yes | - | no |

### Classes that reach GSP on allocation

One row per internal class. The device acknowledges every GSP_RM_ALLOC (ctrl.rs `Dispatcher::alloc`, the parameters echoed); "named" means the crate models or records the class by number.

| Internal class | Route | Classes named by the crate | Other classes (acknowledged only) |
|---|---|---|---|
| RmClientResource | constructor RPC | NV01_ROOT 0x0000 | NV01_ROOT_NON_PRIV 0x0001, NV01_ROOT_CLIENT 0x0041 |
| P2PApi | constructor RPC | - | NV50_P2P 0x503b |
| Device | constructor RPC | NV01_DEVICE_0 0x0080 | - |
| KernelSMDebuggerSession | RPC_TO_ALL | - | GT200_DEBUGGER 0x83de |
| Profiler | RPC_TO_PHYS_RM | - | GF100_PROFILER 0x90cc |
| ProfilerCtx | constructor RPC | - | MAXWELL_PROFILER_CONTEXT 0xb1cc |
| ProfilerDev | constructor RPC | - | MAXWELL_PROFILER_DEVICE 0xb2cc |
| PerfBuffer | constructor RPC | - | G84_PERFBUFFER 0x844c |
| Hdacodec | RPC_TO_ALL | - | GF100_HDACODEC 0x90ec |
| KernelChannel | constructor RPC | TURING_CHANNEL_GPFIFO_A 0xc46f | GF100_CHANNEL_GPFIFO 0x906f, KEPLER_CHANNEL_GPFIFO_A 0xa06f, KEPLER_CHANNEL_GPFIFO_B 0xa16f, MAXWELL_CHANNEL_GPFIFO_A 0xb06f, PASCAL_CHANNEL_GPFIFO_A 0xc06f, VOLTA_CHANNEL_GPFIFO_A 0xc36f, AMPERE_CHANNEL_GPFIFO_A 0xc56f, HOPPER_CHANNEL_GPFIFO_A 0xc86f, BLACKWELL_CHANNEL_GPFIFO_A 0xc96f, BLACKWELL_CHANNEL_GPFIFO_B 0xca6f |
| UvmChannelRetainer | RPC_TO_ALL | - | UVM_CHANNEL_RETAINER 0xc574 |
| KernelCtxShareApi | RPC_TO_ALL | FERMI_CONTEXT_SHARE_A 0x9067 | - |
| Subdevice | constructor RPC | NV20_SUBDEVICE_0 0x2080 | - |
| BinaryApi | RPC_TO_ALL | NV2081_BINAPI 0x2081 | - |
| BinaryApiPrivileged | RPC_TO_ALL | - | NV2082_BINAPI_PRIVILEGED 0x2082 |
| KernelChannelGroupApi | constructor RPC | KEPLER_CHANNEL_GROUP_A 0xa06c | - |
| VirtualMemoryRange | constructor RPC | - | NV01_MEMORY_VIRTUAL 0x0070 |
| MemoryFabric | constructor RPC | - | NV_MEMORY_FABRIC 0x00f8 |
| VaSpaceApi | constructor RPC | FERMI_VASPACE_A 0x90f1 | - |
| NvencSession | constructor RPC | - | NVENC_SW_SESSION 0xa0bc |
| NvfbcSession | constructor RPC | - | NVFBC_SW_SESSION 0xa0bd |
| GenericEngineApi | RPC_TO_ALL | GF100_SUBDEVICE_INFOROM 0x90e7 | GF100_SUBDEVICE_MASTER 0x90e6 |
| TimerApi | RPC_TO_PHYS_RM | - | NV01_TIMER 0x0004 |
| I2cApi | RPC_TO_PHYS_RM | - | NV40_I2C 0x402c |
| DiagApi | RPC_TO_ALL | NV20_SUBDEVICE_DIAG 0x208f | - |
| ZbcApi | RPC_TO_ALL | - | GF100_ZBC_CLEAR 0x9096 |
| VidmemAccessBitBuffer | RPC_TO_PHYS_RM | - | MMU_VIDMEM_ACCESS_BIT_BUFFER 0xc763, HOPPER_MMU_VIDMEM_ACCESS_BIT_BUFFER 0xc863 |
| GPUInstanceSubscription | RPC_TO_ALL | - | AMPERE_SMC_PARTITION_REF 0xc637 |
| ComputeInstanceSubscription | RPC_TO_ALL | - | AMPERE_SMC_EXEC_PARTITION_REF 0xc638 |
| NvDispApi | RPC_TO_ALL | NVC570_DISPLAY 0xc570 | NVC670_DISPLAY 0xc670, NVC770_DISPLAY 0xc770, NVC870_DISPLAY 0xc870, NVC970_DISPLAY 0xc970, NVCA70_DISPLAY 0xca70, NVCB70_DISPLAY 0xcb70, NVCC70_DISPLAY 0xcc70 |
| DispSwObj | RPC_TO_ALL | NVC372_DISPLAY_SW 0xc372 | - |
| DispCommon | RPC_TO_ALL | NV04_DISPLAY_COMMON 0x0073 | - |
| DispChannelPio | RPC_TO_ALL | NVC57A_CURSOR_IMM_CHANNEL_PIO 0xc57a | NVC67A_CURSOR_IMM_CHANNEL_PIO 0xc67a, NVC97A_CURSOR_IMM_CHANNEL_PIO 0xc97a, NVCA7A_CURSOR_IMM_CHANNEL_PIO 0xca7a, NVCB7A_CURSOR_IMM_CHANNEL_PIO 0xcb7a, NVCC7A_CURSOR_IMM_CHANNEL_PIO 0xcc7a |
| DispChannelDma | RPC_TO_ALL | NVC57B_WINDOW_IMM_CHANNEL_DMA 0xc57b, NVC57D_CORE_CHANNEL_DMA 0xc57d, NVC57E_WINDOW_CHANNEL_DMA 0xc57e | NVC67B_WINDOW_IMM_CHANNEL_DMA 0xc67b, NVC67D_CORE_CHANNEL_DMA 0xc67d, NVC77D_CORE_CHANNEL_DMA 0xc77d, NVC67E_WINDOW_CHANNEL_DMA 0xc67e, NVC87B_WINDOW_IMM_CHANNEL_DMA 0xc87b, NVC87D_CORE_CHANNEL_DMA 0xc87d, NVC87E_WINDOW_CHANNEL_DMA 0xc87e, NVC97B_WINDOW_IMM_CHANNEL_DMA 0xc97b, NVC97D_CORE_CHANNEL_DMA 0xc97d, NVC97E_WINDOW_CHANNEL_DMA 0xc97e, NVCA7B_WINDOW_IMM_CHANNEL_DMA 0xca7b, NVCA7D_CORE_CHANNEL_DMA 0xca7d, NVCA7E_WINDOW_CHANNEL_DMA 0xca7e, NVCB7B_WINDOW_IMM_CHANNEL_DMA 0xcb7b, NVCB7D_CORE_CHANNEL_DMA 0xcb7d, NVCB7E_WINDOW_CHANNEL_DMA 0xcb7e, NVCC7B_WINDOW_IMM_CHANNEL_DMA 0xcc7b, NVCC7D_CORE_CHANNEL_DMA 0xcc7d, NVCC7E_WINDOW_CHANNEL_DMA 0xcc7e |
| DispSwObject | RPC_TO_ALL | - | GF100_DISP_SW 0x9072 |
| TimedSemaSwObject | RPC_TO_ALL | - | GF100_TIMED_SEMAPHORE_SW 0x9074 |
| DeferredApiObject | RPC_TO_ALL | - | NV50_DEFERRED_API_CLASS 0x5080 |
| UvmSwObject | RPC_TO_ALL | GP100_UVM_SW 0xc076 | - |
| SoftwareMethodTest | RPC_TO_ALL | - | NV04_SOFTWARE_TEST 0x007d |
| KernelCeContext | RPC_TO_ALL | TURING_DMA_COPY_A 0xc5b5 | MAXWELL_DMA_COPY_A 0xb0b5, PASCAL_DMA_COPY_A 0xc0b5, AMPERE_DMA_COPY_A 0xc6b5, AMPERE_DMA_COPY_B 0xc7b5, HOPPER_DMA_COPY_A 0xc8b5, BLACKWELL_DMA_COPY_A 0xc9b5, BLACKWELL_DMA_COPY_B 0xcab5 |
| NvdecContext | RPC_TO_ALL | - | NVB8B0_VIDEO_DECODER 0xb8b0, NVC4B0_VIDEO_DECODER 0xc4b0, NVC6B0_VIDEO_DECODER 0xc6b0, NVC7B0_VIDEO_DECODER 0xc7b0, NVC9B0_VIDEO_DECODER 0xc9b0, NVCDB0_VIDEO_DECODER 0xcdb0, NVCEB0_VIDEO_DECODER 0xceb0, NVCFB0_VIDEO_DECODER 0xcfb0, NVD1B0_VIDEO_DECODER 0xd1b0, NVD2B0_VIDEO_DECODER 0xd2b0 |
| NvjpgContext | RPC_TO_ALL | - | NVB8D1_VIDEO_NVJPG 0xb8d1, NVC4D1_VIDEO_NVJPG 0xc4d1, NVC9D1_VIDEO_NVJPG 0xc9d1, NVCDD1_VIDEO_NVJPG 0xcdd1, NVCED0_VIDEO_NVJPG 0xced0, NVCFD1_VIDEO_NVJPG 0xcfd1, NVD2D1_VIDEO_NVJPG 0xd2d1 |
| OfaContext | RPC_TO_ALL | - | NVB8FA_VIDEO_OFA 0xb8fa, NVC6FA_VIDEO_OFA 0xc6fa, NVC7FA_VIDEO_OFA 0xc7fa, NVC9FA_VIDEO_OFA 0xc9fa, NVCDFA_VIDEO_OFA 0xcdfa, NVCEFA_VIDEO_OFA 0xcefa, NVCFFA_VIDEO_OFA 0xcffa, NVD1FA_VIDEO_OFA 0xd1fa, NVD2FA_VIDEO_OFA 0xd2fa |
| MsencContext | RPC_TO_ALL | - | NVD1B7_VIDEO_ENCODER 0xd1b7, NVC4B7_VIDEO_ENCODER 0xc4b7, NVB4B7_VIDEO_ENCODER 0xb4b7, NVC7B7_VIDEO_ENCODER 0xc7b7, NVC9B7_VIDEO_ENCODER 0xc9b7, NVCEB7_VIDEO_ENCODER 0xceb7, NVCFB7_VIDEO_ENCODER 0xcfb7 |
| Sec2Context | RPC_TO_ALL | - | HOPPER_SEC2_WORK_LAUNCH_A 0xcba2 |
| KernelGraphicsObject | RPC_TO_ALL | TURING_COMPUTE_A 0xc5c0 | AMPERE_A 0xc697, AMPERE_COMPUTE_A 0xc6c0, AMPERE_B 0xc797, AMPERE_COMPUTE_B 0xc7c0, ADA_A 0xc997, ADA_COMPUTE_A 0xc9c0, HOPPER_A 0xcb97, HOPPER_COMPUTE_A 0xcbc0, BLACKWELL_A 0xcd97, BLACKWELL_COMPUTE_A 0xcdc0, BLACKWELL_INLINE_TO_MEMORY_A 0xcd40, BLACKWELL_B 0xce97, BLACKWELL_COMPUTE_B 0xcec0, FERMI_TWOD_A 0x902d, KEPLER_INLINE_TO_MEMORY_B 0xa140, TURING_A 0xc597 |
| EventApi | constructor RPC | NV01_EVENT_KERNEL_CALLBACK_EX 0x007e | NV01_EVENT 0x0005, NV01_EVENT_OS_EVENT 0x0079, NV01_EVENT_KERNEL_CALLBACK 0x0078 |


## 3. The unhandled controls by category, with a generic recipe each

`--full` lists every control of every category with its class, privilege, flags, call-site
counts (kernel RM / display / UVM), native-trace hits and the reason for its category. The
counts below are from section 2.

### A. Pure chip data (62)

**What.** Static getters: `*_GET_*INFO`, `*_CAPS`, `*_STATIC_*`, masks, tables, sizes, and
everything flagged `RMCTRL_FLAGS_CACHEABLE` (0x400; the kernel RM caches the answer, which
already says it never changes). Examples: `NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES`,
`NV2080_CTRL_CMD_FB_GET_GPU_CACHE_INFO`, `NV0080_CTRL_CMD_NVDEC_GET_CAPS_V2`,
`NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_SIZE`, `NV2080_CTRL_CMD_GPU_GET_IP_VERSION`,
`NV2080_CTRL_CMD_CE_GET_HUB_PCE_MASK_V2`, `NV2080_CTRL_CMD_INTERNAL_GET_DEVICE_INFO_TABLE`,
`NV0073_CTRL_CMD_DFP_GET_DISPLAYPORT_DONGLE_INFO`.

**Recipe: a table-driven answer.** One generic handler in `ctrl.rs` that looks the control id up
in a per-chip (and, for connector data, per-board) table and writes the listed fields into a
zeroed parameter block, the way `FIFO_GET_DEVICE_INFO_TABLE` and `INTR_GET_KERNEL_TABLE` already
do by hand. Field offsets come from the generated per-version ABI (D14: `ver::v610`, measured
with `caraxes/tools/gsp-abi-sizes.sh`), never hand-typed; the values come from (1) source
constants where the RM computes them from the chip (engine lists, class lists), (2) the chip
files that exist (`chips/tu106/*.toml`), or (3) one capture of the real card per chip, done once
with the operator's consent like the host facts (host-facts.md), and stored as chip data. That
is not trace-driven bring-up: the capture is a complete sweep of a known list, not a reaction
to a wall. The label is the table's own (`chip`, `board` or `card`), as for the existing rows.

**Estimate.** The generic handler and its table format: 1 agent-day. Filling: about 20 minutes
per control once the table exists, so 2 to 3 days for all 62, less if the capture tool fills
most of them.

### B. Host-forwardable as-is (71)

**What.** Public (not INTERNAL) getters of live state with a flat parameter struct that is the
same on host and guest (same driver version, D14): ECC and row-remapper counters, BIOS and POST
data, PCIe and link counters, GR context-switch statistics, falcon and GSP heap statistics,
RC error counts, GPU accounting. 66 of the 71 are NON_PRIVILEGED.

**Recipe: a forward rule.** Extend the `FORWARD` table of `ver/v610.rs` from a list into a rule:
forward when the control is public, the target object maps to a host object the device holds
(subdevice or device, `nvml.rs` `forward`), the parameters are not FINN-serialized (the embedded-
pointer controls listed in [gsp-rpc-protocol.md](gsp-rpc-protocol.md) §5 are excluded), and the
control is not on a D9 deny list. The deny list is what must not leave the host: process and
accounting data of the host (`NV0000_CTRL_CMD_GPUACCT_*`, anything that returns PIDs), the host's
own RC error records, and identity the profile replaces (nvml.rs). PRIVILEGED controls (4) need
the scoped CAP_SYS_ADMIN window of D10/D12 or stay refused. Label `host (forwarded by rule)`.

**Estimate.** 1 day for the rule and the deny list, 1 day to review the 71 against D9.

### C. Guest-state derived (30)

**What.** Controls on objects the guest allocated: channels (`NVA06F_CTRL_CMD_BIND`,
`NV906F_CTRL_CMD_RESET_CHANNEL`, `NV506F_CTRL_CMD_RESET_ISOLATED_CHANNEL`, `*_SET_INTERLEAVE_LEVEL`,
`NVA06F_CTRL_CMD_RESTART_RUNLIST`), TSGs (`NVA06C_CTRL_CMD_INTERNAL_SET_TIMESLICE`,
`NVA06C_CTRL_CMD_MAKE_REALTIME`), context shares (`NV9067_CTRL_CMD_*_TPC_PARTITION_*`,
`*_CWD_WATERMARK`), graphics contexts (`NV0090_CTRL_CMD_*`), and the channel context save/restore
set `NVB06F_CTRL_CMD_*_ENGINE_CTX_*`.

**Recipe: resolve through the channel registry.** `channel.rs` already knows every guest channel,
TSG and VA space by handle. For a channel that runs on the host, forward to the mirror object
(`mirror.rs` `forward_control`, the path `MIRROR_FORWARDED` takes); for a software channel,
record the request in the registry and answer from it (BIND stores the engine type, RESET puts
the software channel back to idle, INTERLEAVE and TIMESLICE are recorded). The context
save/restore and migrate set is live migration: refuse until migration is a goal. Labels
`guest (recorded)` or `host (mirror object)`.

**Estimate.** Half a day per family, about 5 families plus the mirror side: 5 to 8 days.

### D. Needs device modelling (306)

**What.** Everything that acts on engines, links or power, or reads state only the engine has.
By class: DispCommon 87 and NvDispApi 16 (DP link training and configuration, HDMI, HDCP, PSR,
VRR, lock pins, audio), Subdevice 119 (GR context-switch binds, FIFO disable and scheduling,
perf and power setters, bus and GPIO, FB save/restore), ProfilerBase 21 and Profiler 4 (HWPM,
PMA streams), KernelSMDebuggerSession 20 (SM debugger), DiagApi 11, ZbcApi 4.

**Recipe: four generic rules, then modelling where they do not reach.**

1. *Acknowledge and log* the setters that have no result the client reads back (perf-limit,
   power and clock setters, audio mute, stereo and MSA properties, trigger selects): NV_OK with
   the request echoed, a note naming the control, label `synthesized (acknowledged, no effect)`,
   exactly as `cuda.rs` `ACKNOWLEDGED` does today. Rule: a control whose output fields are all
   inputs (checkable from the generated struct: no `OUT` fields in the SDK comment block, or
   `sizeof` of the output part zero). Probably half of D.
2. *Display link and sink operations* go to the connector and sink model of `display/`
   (`display/engine.rs`, the DP sink behind the connector): link training, lane data, link
   rates, MST topology (refused for a single-stream connector like a monitor without a branch
   device), HDCP (refused like a sink without HDCP), HDMI (when a TMDS connector exists, P12).
3. *Engine operations on mirrored objects* go to the host mirror when the object runs on the
   host (GR ctxsw binds, `FIFO_DISABLE_CHANNELS`, debugger controls on a mirrored channel), with
   the isolation check that the target is the guest's own mirror object.
4. *Profiling and debugging* (HWPM, PMA, SM debugger) are refused with NOT_SUPPORTED, as on a
   system with profiling disabled for non-admin users, until a tool needs them.

**Estimate.** Rules 1 and 4: 2 days including the struct-based output test. Rule 2: the
display work already planned (P12, P16), about 10 to 15 days for DP, DVI and HDMI. Rule 3: 3 to
5 days on top of the mirror. What remains (FB save/restore for suspend, GPIO, some bus
operations): 5 to 10 days when needed. Total 20 to 35 days.

### E. Never expected from a stock client on this chip (189)

**What.** NVLink (83 including INTERNAL_NVLINK; TU106 has no NVLink), vGPU and SR-IOV (15 plus
VF controls; the PF is not in SR-IOV mode), MIG and SMC (Turing has no MIG), Confidential
Computing, SPDM, FSP and CCU (Hopper and later), GSYNC (no board), HSHUB and FLA (Ampere and
later), test-only controls (`RMCTRL_FLAGS_RM_TEST_ONLY_CODE`, 10), and 6 INTERNAL controls
without any call site in the kernel RM.

**Recipe: one refusal rule per feature family, kept as chip data.** Answer NOT_SUPPORTED with
the label `refused (feature absent on this chip)`, the way `NVLINK_GET_NVLINK_DEVICE_INFO` is
refused today (ctrl.rs, which removes KernelNvlink from the guest). The family list lives in the
chip file, so a chip that has the feature (TU102 has NVLink, GA100 has MIG) turns it on there
(D14). Where the real card answers NV_OK with an empty result instead of NOT_SUPPORTED (as for
`NVLINK_GET_PLATFORM_INFO`, nvml.rs), the capture of category A decides.

**Estimate.** 1 day.

## 4. RPC functions, events and classes

**Functions.** Of the 20 functions a GSP client sends, the device answers 11. Missing:

| # | Function | When the Linux driver sends it | Recipe |
|---|---|---|---|
| 4 | ALLOC_MEMORY | `memRegisterWithGsp` (mem.c:513-575) | record the page list, guest-derived; this is the sysmem registration of D18 |
| 47 | UNLOADING_GUEST_DRIVER | module unload and power transitions (kernel_gsp.c kgspUnloadRm) | NV_OK, drop the guest's objects (the reset path exists); nova sends it too |
| 78 | DUMP_PROTOBUF_COMPONENT | debug dumps (nv_debug_dump.c:198, journal.c:1218) | an empty dump |
| 202 | ECC_NOTIFIER_WRITE_ACK | async, from the RC handler (kernel_gsp.c:611) | consume, no reply |
| 14/15 | MAP/UNMAP_MEMORY_DMA | only without the split VA space (virtual_mem.c:1537/1892) | refuse until seen |
| 22 | IDLE_CHANNELS | kernel_idle_channels.c:44 | channel registry (category C) |
| 200 | INVALIDATE_TLB | kern_gmmu_gm107.c:249 with `bDoVgpuRpc` | the mirror's invalidate (exists) |
| 50 | GPU_EXEC_REG_OPS | subdevice_ctrl_gpu_regops.c:224 | refuse (debugger, category D rule 4) |

Unanswered functions are already answered NOT_SUPPORTED and collected in `boot.rs`
`unanswered`, so none of them hangs the queue. About 2 days.

**Events.** The device sends GSP_INIT_DONE and POST_EVENT. The CPU side dispatches 37 events
(`kernel_gsp.c:1399`); the ones a complete device needs are how GSP reports trouble:
RC_TRIGGERED (0x1004) and OS_ERROR_LOG (0x1006, the Xid) for a channel error, MMU_FAULT_QUEUED
(0x1005) for a non-replayable fault, and FECS_ERROR (0x1021) and RECOVERY_ACTION (0x1022). Without
them a fault in a guest channel is a timeout instead of an error (experiments.md, d-gr-launch7:
"so a fault returns 719 not a 30 s hang", there obtained through the host's error notifier).
SEMAPHORE_SCHEDULE_CALLBACK (0x100b) and TIMED_SEMAPHORE_RELEASE (0x1018) belong to the software
classes GF100_DISP_SW and GF100_TIMED_SEMAPHORE_SW that GL uses. The rest (NVLink, vGPU, MIG, PFM,
simulation) is category E. The payload structs are in `g_rpc-structures.h` (`rpc_rc_triggered_*`,
`rpc_os_error_log_*`, ...). Recipe: one `emit_event` helper beside the POST_EVENT path of
`boot.rs`, fed by the mirror's error notifier and the software executor. 2 to 3 days.

**Classes.** Every `GSP_RM_ALLOC` is acknowledged and echoed (`ctrl.rs` `Dispatcher::alloc`), so
no class allocation is a wall today; 21 of the 149 GSP-bound classes are modelled or recorded.
The other Turing classes a graphics client allocates (`TURING_A` 0xC597, `GF100_ZBC_CLEAR`
0x9096, `NV01_TIMER`, `GF100_TIMED_SEMAPHORE_SW`, `NV50_DEFERRED_API_CLASS`, the NVDEC/NVENC/NVJPG
contexts `NVC4B0` and `NVC4B7`) need the same treatment as `TURING_COMPUTE_A`: an engine
object on a channel, added to the host mirror when the channel runs there (`mirror.rs`
`add_engine_object`). Classes of other generations are acknowledged but never allocated by a
driver that sees a TU106.

## 5. What other clients of the same RM core share

**A Windows driver of the same generation.** What is shared, from this tree:

- *The RPC vocabulary and payloads.* `rpc_global_enums.h`, `g_rpc-structures.h` and
  `g_rpc-message-header.h` have no OS conditionals (and no chip conditionals,
  [other-architectures.md](other-architectures.md) §3 and §6a); the grep for `WINDOWS` over them
  and over `g_subdevice_nvoc.c` finds nothing.
- *The control surface and its routing.* The export tables, flags and routing
  (`ROUTE_TO_PHYSICAL`, `rmresControl_Prologue`) belong to the RM core, one source compiled per
  platform: `rmconfig.h:260-265` selects `RMCFG_FEATURE_PLATFORM_WINDOWS`, `_UNIX` and `_GSP`
  (the last is GSP-RM itself). Of the 39 `RMCFG_FEATURE_PLATFORM_WINDOWS` conditionals under
  `src/nvidia/src/kernel`, none is in the control or RPC definitions (they are mostly in memory
  management, BIF, NVLink glue and OS stubs).
- *The firmware lock.* The kernel RM accepts only a GSP firmware whose `.fwversion` equals its
  own version string (`_kgspFwContainerVerifyVersion`, `kernel_gsp.c:5719-5774`,
  other-architectures.md §6d); a Windows driver of the same branch therefore speaks this ABI,
  and the device's version module (D14) serves it.

What differs is the OS-side driver: on Linux `src/nvidia/arch/nvalloc/unix` plus `kernel-open`
(nvidia-modeset, nvidia-drm, nvidia-uvm), on Windows a WDDM kernel-mode driver with its own
display stack and memory management. So the *set of controls actually called* differs even
though the surface is the same: expect Windows to use more of category D display (ACPI display
maps, backlight, mux, HDCP, which are PRIVILEGED and unused by nvidia-modeset), the RC and
reset set of category C (TDR recovery), and less of UVM. That is the argument for completing
the surface by category rather than by trace. Two limits: the tables here are those of the
`shipping-gpus-openrm` profile, so a control compiled only into a Windows profile is invisible
to the script (the surface above is a lower bound for Windows); and this tree cannot tell
whether the Windows driver of this branch runs a TU106 in GSP-offload mode at all. If it runs
the RM monolithically on the CPU, it never talks to GSP and this surface does not apply to it.
Checking that on the Windows package (the GSP firmware files it ships and its GSP enable
setting) is the first Windows step (open item 1).

**nova-core.** A much smaller subset. At drm-rust-next `896ed0833627` it sends five functions,
GSP_SET_SYSTEM_INFO, SET_REGISTRY, GET_GSP_STATIC_INFO, UNLOADING_GUEST_DRIVER and
CONTINUATION_RECORD (`gsp/commands.rs`, `gsp/cmdq/continuation.rs`, `const FUNCTION`), waits for
two events, GSP_INIT_DONE and GSP_RUN_CPU_SEQUENCER (`gsp/sequencer.rs`), decodes 29 function and
event numbers (`gsp/fw.rs` `MsgFunction`), and sends no GSP_RM_ALLOC or GSP_RM_CONTROL at all
([nova.md](nova.md) §1.6, §2.1, §2.3). Its ABI is r570.144, not 610 (nova.md §2), so it needs a
second version module, not more controls. The upstream vGPU manager on top of it
([nova-vgpu-upstream.md](nova-vgpu-upstream.md) §3.1-3.7) boots GSP on the r000 firmware
interface and talks NVKV-keyed GSP_INIT replies (keys 0x0500-0x0502, 0x1050), GMC transactions
and the plugin protocol, none of which is part of the RM control surface. Of this inventory
nova uses 5 of the 20 functions (the device answers 4; UNLOADING_GUEST_DRIVER is missing) and
2 events (the device sends GSP_INIT_DONE but not GSP_RUN_CPU_SEQUENCER, which nova must see
first, nova.md §1.6).

## 6. The ten unhandled controls a Linux CUDA or display client is most likely to hit next

Ranked by the driver's own call sites and the native traces of the real card (`--top` gives the
raw ranking; this list drops items that need hardware the device does not present, such as
DP 2.x, HDMI FRL on Ampere classes, P2P between two GPUs and FLA).

| # | Control | Cat | Why next (call sites) |
|---|---|---|---|
| 1 | `NVA06F_CTRL_CMD_BIND` 0xA06F0104 | C | sent by the kernel RM when a channel is bound to an engine's runlist on a GSP client (`kernel_channel.c:2810-2828`); callers include nvidia-modeset's push channels (`src/common/unix/nvidia-push/src/nvidia-push-init.c:340`) and CeUtils (`mem_utils.c:1972`); in every EGL, GL, Vulkan and NVDEC/NVENC trace. The first control a Wayland compositor on EGL needs. |
| 2 | `NV2080_CTRL_CMD_TIMER_GET_GPU_CPU_TIME_CORRELATION_INFO` 0x20800406 | B | nvidia-modeset `nvUpdateGpuCpuTimeDiff` (`nvkms-evo.c:8877`) at device allocation (`:5569`), at modeset (`nvkms-modeset.c:3002`) and on a refresh timer (`nvkms-evo.c:8904`); the kernel sends it to GSP (`subdevice_ctrl_timer_kernel.c:338`). A failure zeroes the GPU-CPU time offset, which skews vblank and flip timestamps, the Wayland presentation clock. Answer from the device's own PTIMER, not the host's. |
| 3 | `NV2080_CTRL_CMD_GR_CTXSW_ZCULL_BIND` 0x20801208 | D | every GL, EGL and Vulkan trace; ROUTE_TO_PHYSICAL (flags 0x90348). Echo for software channels, mirror for host ones. |
| 4 | `NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_SIZE` / `_ENTRY` 0x90960106/7 | A/B | every GL, EGL and Vulkan trace, on the `GF100_ZBC_CLEAR` object; the table size is chip data, the entries are the guest's own ZBC state once SET_ZBC_* is modelled. |
| 5 | `NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES` 0x00801707 | A | every GL, EGL and Vulkan trace; per-engine context buffer sizes, chip data (the host answer can be captured once). |
| 6 | `NV2080_CTRL_CMD_FB_GET_GPU_CACHE_INFO` 0x20801315 | A | every GL, EGL and Vulkan trace; L2 parameters, partly in the `chips/tu106` memsys data already. |
| 7 | `NV0080_CTRL_CMD_NVDEC_GET_CAPS_V2` 0x00801C02 | A | Vulkan device enumeration (vk-enum, vk-offscreen, vk-rt traces) and NVDEC/NVENC. |
| 8 | `NV906F_CTRL_CMD_RESET_CHANNEL` 0x906F0102 and `NV506F_CTRL_CMD_RESET_ISOLATED_CHANNEL` 0x506F0105 | C | the RC path after any channel error or watchdog timeout (`kernel_rc_watchdog_callback.c:88`, `kernel_channel.c:3007`, `kernel_rc_callback.c:88,198,414`); the first CUDA fault reaches it. Pairs with the RC_TRIGGERED event (section 4). |
| 9 | `NV2080_CTRL_CMD_FIFO_DISABLE_CHANNELS` 0x2080110B | D | UVM's `nvGpuOpsDisableVaSpaceChannels` (`nv_gpu_ops.c:1071`) from `nvGpuOpsUnsetPageDirectory` (`:9158`) whenever a VA space with channels is released, that is at every CUDA context teardown; the kernel forwards it (`kernel_fifo_ctrl.c:765`). |
| 10 | `NV0073_CTRL_CMD_DFP_GET_DISPLAYPORT_DONGLE_INFO` 0x00731142 | A | nvidia-modeset queries it for every TMDS connector (`nvkms-dpy.c:737`, from the pixel-clock limit at `:953`); the first control of the DVI-D connector of P12, followed by the HDMI set (`SET_HDMI_SINK_CAPS`, `nvkms-hdmi.c:338`). |

Close behind: `NV5070_CTRL_CMD_GET_FRAMELOCK_HEADER_LOCKPINS` (nvidia-modeset lock-pin query,
`nvkms-evo.c:5721`, used when heads are raster- or flip-locked), `NV0073_CTRL_CMD_DP_TOPOLOGY_ALLOCATE_DISPLAYID`
(only on an MST connector, `nvkms-rm.c:2200`), and `NV2080_CTRL_CMD_PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2`
(nvidia-smi utilization, forwarded by `kern_perf_ctrl.c:112`).

## 7. Work to full coverage

| Part | Items | Recipe | Agent-days |
|---|---|---|---|
| A chip data | 62 | table-driven answer, one capture per chip | 3 to 4 |
| B host-forwardable | 71 | forward rule with a D9 deny list | 2 |
| C guest state | 30 | channel registry and mirror | 5 to 8 |
| D device modelling | 306 | acknowledge rule, display model, mirror, refusal of profiling | 20 to 35 |
| E never expected | 189 | refusal per feature family in chip data | 1 |
| RPC functions | 9 | table in section 4 | 2 |
| Events | about 7 | `emit_event` fed by mirror and executor | 2 to 3 |
| Classes | about 10 Turing engine classes | engine objects on channels, mirror | 3 to 5 |
| **Total** | | | **about 40 to 60** |

A and E together remove about 250 of the 658 unhandled controls in about 5 days; with B and
rules 1 and 4 of D, roughly two thirds of the surface is answered generically in about two
weeks, and what remains is the engine and display modelling that the milestones need anyway.

## 8. Open items

1. Does the Windows driver of the 610 branch boot a TU106 with GSP-RM? Check on the Windows
   package (GSP firmware files, the GSP enable setting) before any Windows work. If not, the
   Windows route needs a different device model (the monolithic RM's register and falcon
   surface) and this file only covers Linux and nova.
2. Controls compiled into other profiles: a Windows build may export controls absent from the
   `shipping-gpus-openrm` tables. Only a Windows trace or the Windows RM tables would show them.
3. The categories are heuristics. Before implementing a category, run `--full` and correct the
   misplaced items in `classify()` (the known weak spots: display getters that are chip data but
   look dynamic, and INTERNAL getters on the boundary of A and D).
4. The call-site detection of explicit sends skips calls under `IS_VIRTUAL` guards; a guard
   hidden in a helper function is not seen, so a few vGPU-only sends may still be counted.
5. For category A the one-time capture of the real card touches the host GPU through its RM;
   it needs the operator's go-ahead like every host read.

## 615.71.09

The same script against the 615.71.09 tree (`/mnt/vmstore/caraxes/ogkm-615.71.09`, commit 61dcc93,
see [blackwell-gb206.md](blackwell-gb206.md) §3.1), run on 2026-09-24 as
`python3 caraxes/tools/gsp-surface.py --ogkm /mnt/vmstore/caraxes/ogkm-615.71.09` (or with
`CARAXES_OGKM` set; the version comes from the tree's `version.mk`). The 610 column is a run of
the same day against the same crate, so the two differ only by the tree. Both columns are without
native traces (there are none for 615, and the run checkout had none for 610); the 610 numbers
equal the tables above, except that the crate now names 24 classes (21 above). The crate side is
still the `ver/v610` module in both runs: "handled" means "the 610 device has an arm for this id".
File paths in this section are in the 615 tree (`src/...`, and bare header names under
`src/common/sdk/nvidia/inc/ctrl/`), except `ver/`, `ctrl.rs` and `display/` (the crate,
`crates/vfio-user-nvgpu/src`) and those marked 610.

| Item | 610.57.04 | 615.71.09 |
|---|---:|---:|
| Exported controls (unique ids) | 1377 | 1397 |
| Controls that reach GSP | 829 | 844 |
| of which INTERNAL | 239 | 246 |
| of which sent explicitly by kernel RM | 257 | 260 |
| Routed controls the device handles | 171 | 170 |
| (host-forwarded / synthesized / guest-derived / echo / refused) | 37/56/21/46/11 | 36/56/21/46/11 |
| Routed controls unhandled | 658 | 674 |
| (A / B / C / D / E) | 62/71/30/306/189 | 62/71/32/309/200 |
| Handled ids outside the export tables | 38 | 38 |
| RPC functions | 230 | 232 |
| of which a GSP client sends / the device handles | 20 / 11 | 20 / 11 (see note 1) |
| RPC events | 41 | 45 |
| of which the CPU side dispatches / the device sends | 37 / 2 | 36 / 2 |
| Classes in resource_list.h / reach GSP on alloc / crate names | 222 / 149 / 24 | 224 / 149 / 24 |

Id sets: 1376 ids in both, one only in 610 (`NV0000_CTRL_CMD_GPU_DISABLE_NVLINK_INIT` 0x00000281, not
routed), 21 only in 615 (15 of them routed: NVLink metrics and LFM, vGPU-manager scrub and CE
utilities, `NV0073_CTRL_CMD_SPECIFIC_HDCP_KEY_DECRYPTION` 0x007302B2, `NV0073_CTRL_CMD_DP_SET_WAR_FLAGS`
0x00731391, `NV0090_CTRL_CMD_PROGRAM_SYSMEM_PROMOTE` 0x0090010E,
`NV2080_CTRL_CMD_INTERNAL_PERF_CLIENT_PERF_MODES_SET_CONTROL` 0x20800B20; the `--full` list names
all). New RPC functions 230, 231 (`CTRL_FIFO_VGPU_SWRUNLIST_SUBMIT`, `_CHANNEL_UPDATE`,
`src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:240-241`), new events 0x102A to 0x102D
(`rpc_global_enums.h:297-300`, all dispatched), new classes 0x90D0 and 0xD0CD
(`src/nvidia/src/kernel/rmapi/resource_list.h:144, 282`, neither reaches GSP). The dispatcher
(`src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:1556`) no longer lists GSP_INIT_DONE,
MMU_FAULT_QUEUED, VGPU_GSP_PLUGIN_TRIGGERED, GSP_LOAD_EXEC_GENERIC_BOOTLOADER and
GSP_LOAD_EXEC_HS_BINARY (37 - 5 + 4 = 36), and `rpc_gsp_set_system_info_v17_00` is gone.

**Controls whose id changed.**

| Control | 610.57.04 | 615.71.09 | Device today |
|---|---|---|---|
| `NV2080_CTRL_CMD_INTERNAL_GPU_GET_CHIP_INFO` | 0x20800A36, 88-byte params | removed; 0x20800A36 is now `NV2080_CTRL_CMD_INTERNAL_GPU_SYNC_FABRIC_PROBE_INFO` (`src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080internal.h:585`), sent by kernel RM through the physical RM API (`src/nvidia/src/kernel/gpu/gpu_fabric_probe.c:1938`) | synthesized as chip info (`ver/v610.rs:323`); the script counts it as handled on 615, but the answer is for the wrong control (§3.4 of blackwell-gb206.md) |
| `NV2080_CTRL_CMD_NVLINK_GET_REMAP_TABLE_INFO_V2` | 0x208030A1 | 0x208030A2 (`ctrl2080nvlink.h:3573`) | unhandled (E) |
| `NV2080_CTRL_NVLINK_GET_UPDATE_NVLE_LIDS_V2` | 0x208030A2 | 0x208030A3 (`ctrl2080nvlink.h:3604`) | unhandled (E) |
| `NV2080_CTRL_CMD_NVLINK_SET_BW_MODE_ASYNC` | none | 0x208030A1, the old REMAP_TABLE_INFO_V2 id (`ctrl2080nvlink.h:2892`), not routed | none |

**Routing changed for a handled control.** `NV2080_CTRL_CMD_GPU_GET_VMMU_SEGMENT_SIZE` 0x2080017E: flags
0x10448 (ROUTE_TO_PHYSICAL) in 610, 0x50B in 615 (`src/nvidia/generated/g_subdevice_nvoc.c:4943`); the
kernel RM answers it itself (`src/nvidia/src/kernel/gpu/subdevice/subdevice_ctrl_gpu_kernel.c:2626`)
and the 610 explicit send (`gpu.c:936` in 610) is gone. The device's host-forward arm
(`ctrl.rs:996`) is dead for 615; this is the 171 to 170 and the host-forwarded 37 to 36. Also newly
routed but unhandled: `NV2080_CTRL_CMD_NVLINK_GET_SUPPORTED_COUNTERS` 0x20803074 and
`NV_EVENT_BUFFER_CTRL_CMD_UPDATE_GET` 0x90CD0102.

**Controls whose parameter size changed.** `sizeof` of all 1261 distinct parameter structs of the
ids present in both trees, compiled against each tree's SDK headers (`cc -std=gnu11 -DNV_LINUX`);
25 differ. Those the device handles or that reach GSP:

| Control | Id | 610 | 615 | Struct (615) | Device today |
|---|---|---:|---:|---|---|
| `NV0073_CTRL_CMD_SYSTEM_GET_ACTIVE` | 0x0073010C | 16 | 20 | `ctrl0073/ctrl0073system.h:664-670` (`staleSorIndex` appended) | synthesized (`display/mod.rs:612`); not in blackwell-gb206.md §3.4, which covered `ver/v610.rs` only, not `ver/v610/display.rs` |
| `NV2080_CTRL_CMD_FIFO_GET_DEVICE_INFO_TABLE` | 0x20801112 | 3212 | 3340 | `ctrl2080/ctrl2080fifo.h:556` | synthesized |
| `NV2080_CTRL_CMD_INTERNAL_GET_PCIE_P2P_CAPS` | 0x20800AB8 | 3 | 4 | `ctrl2080/ctrl2080internal.h:2576` | synthesized |
| `NV2080_CTRL_CMD_INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO` | 0x20800A87 | 1088 | 1072 | `ctrl2080/ctrl2080internal.h:5010` | refused |
| `NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS` | 0x20800A26 | 10688 | 10848 | `ctrl2080/ctrl2080internal.h:338` | host-forwarded |
| `NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER` | 0x20800A22 | 73760 | 81952 | `ctrl2080/ctrl2080internal.h:247` | host-forwarded |
| `NV2080_CTRL_CMD_NVLINK_GET_COUNTERS_V2` | 0x20803050 | 28712 | 28720 | `ctrl2080/ctrl2080nvlink.h:1110` | unhandled (E) |
| `NV2080_CTRL_CMD_NVLINK_CLEAR_COUNTERS_V2` | 0x20803051 | 40 | 48 | `ctrl2080/ctrl2080nvlink.h:1136` | unhandled (E) |
| `NV2080_CTRL_CMD_NVLINK_GET_SUPPORTED_COUNTERS` | 0x20803074 | 16 | 24 | `ctrl2080/ctrl2080nvlink.h:2698` | unhandled (E, routed from 615) |
| `NV2080_CTRL_CMD_NVLINK_PROCESS_INIT_DISABLED_LINKS` | 0x2080303B | 32 | 24 | `ctrl2080/ctrl2080nvlink.h:2393` | unhandled (E) |
| `NV2080_CTRL_CMD_VGPU_MGR_INTERNAL_PGPU_ADD_VGPU_TYPE` | 0x20804003 | 694280 | 695432 | `ctrl2080/ctrl2080vgpumgrinternal.h:148` | unhandled (E) |
| `NVB0CC_CTRL_CMD_INTERNAL_PERMISSIONS_INIT` | 0xB0CC0203 | 7 | 6 | `ctrlb0cc/ctrlb0ccinternal.h:157` | unhandled (D) |

Not routed (kernel-side or vGPU host), size changed: `NV2080_CTRL_CMD_GPU_GET_PES_INFO` 0x20800168
56 to 76 (`ctrl2080gpu.h:2222`), `NV2080_CTRL_CMD_GR_GET_GLOBAL_SM_ORDER` 0x2080121B 9240 to 10264
(`ctrl2080gr.h:1233`), `NV2080_CTRL_CMD_GET_GPU_FABRIC_PROBE_INFO` 0x208001A3 56 to 320,
`NV2080_CTRL_CMD_GPU_GET_ENGINE_LOAD_TIMES` 0x2080019B 2608 to 2624,
`NV2080_CTRL_CMD_EVENT_RATS_GSP_TRACE_BIND_EVTBUF` 0x2080030A 24 to 32,
`NV2080_CTRL_CMD_FLA_GET_FABRIC_MEM_STATS` 0x20803504 16 to 32, `NV2080_CTRL_CMD_NVLINK_SET_BW_MODE`
0x20803086 8 to 2, `NV0000_CTRL_CMD_SYSTEM_NVPCF_GET_POWER_MODE_INFO` 0x0000013B 884 to 876,
`NV0000_CTRL_CMD_SYSTEM_READ_CPER` 0x00000149 1056 to 1064, `NV00FD_CTRL_CMD_ATTACH_REMOTE_GPU`
0x00FD0106 40 to 48, and three NVA081 vGPU-config controls (0xA0810101, 0xA0810103, 0xA0810124). The
PES_INFO and FIFO_GET_DEVICE_INFO_TABLE changes are also what `caraxes/tools/lab-abi-check.sh`
reports for the host query tools (2 items differ from 610.57.04).

All seven control rows of blackwell-gb206.md §3.4 are confirmed with the same sizes; the new
findings are `SYSTEM_GET_ACTIVE` (handled), the VMMU_SEGMENT_SIZE routing change and the NVLink id
shift. Equal sizes with changed field layout are not detected by this comparison.

Notes. (1) The "a GSP client sends" set is the 610 source survey kept as a constant in the script
(`GSP_CLIENT_SENDS`); on 615, GSP_SET_SYSTEM_INFO and SET_REGISTRY are deprecated and replaced by
GMCAPI_CMD_GSP_INIT (blackwell-gb206.md §3.3), so 20 overstates what a 615 kernel RM sends. (2) The
A to E classification is the same heuristic; the 16 new unhandled controls fall into C (2), D (3)
and E (11).
