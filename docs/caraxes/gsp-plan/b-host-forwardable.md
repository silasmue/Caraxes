<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
<!-- Research by a read-only subagent on 2026-09-24 for the GSP surface programme (P23). Sources: the 610.57.04 tree (vendor/open-gpu-kernel-modules of the main checkout) and the 615.71.09 tree (Leandro's vendor/open-gpu-kernel-modules), the crate at 61e0b6a, the native traces under matrix/traces/610.57.04 and the run logs under caraxes/runs. Nothing was run against a GPU. -->
# Category B, host-forwardable controls: the implementation plan

[gsp-surface.md](../gsp-surface.md) §3 sorts the unhandled routed controls into five categories by
name and flag heuristics. This file takes category B ("public getters of live state, forward to
the host") control by control, against both driver trees, and says for each what the device must
do. The work splits into six groups. Only 30 of the 70 controls are real forwards (26 as they
are, 4 with an output filter). The heuristic misplaces the rest:

- 11 must be answered by the device, because the host's answer is wrong for the guest.
- 12 act on guest objects and move to the C plan ([c-guest-state.md](c-guest-state.md) §3).
- 13 are refused.
- 4 never reach the device on this configuration.

## Summary

- **Status (2026-09-24).** 59 of the 70 are done, 1 is partial and 10 are open. The open ones
  are the guest-object rows moved to the C plan. The two forwards of §6 are filtered. Details
  in §8.

- **The set.** 70 controls in both trees, the same 70. The script's categories are the same
  for 610.57.04 and 615.71.09. gsp-surface.md counted 71 because `FB_GET_REMAPPED_BANKS` has
  been forwarded since (commit e2ef5af). Regenerate with
  `python3 caraxes/tools/gsp-surface.py --ogkm <tree> --full`. The sections "Unhandled,
  category B" and "category C" are the lists used here.
- **No layout differences.** Every params struct of the 70 has the same `sizeof` in both trees,
  compiled against each tree's SDK headers with the include set of `crates/xtask/src/abi/mod.rs:22-28`
  (`cc -std=gnu11 -DNV_LINUX -D__linux__`). The struct text is also identical in both trees
  after comments and whitespace are stripped, including every nested type and array bound. So
  a forward sends the guest's bytes unchanged in all four combinations of a 610 or 615 guest on
  a 610 or 615 host. hostabi.rs needs no new layout for any of them; its
  `forwarded_as_is_layouts_are_equal_on_both` test (hostabi.rs:677) should get one assert per
  new row.
- **Groups.**

| Group | Meaning | Controls |
|---|---|---|
| F1 | forward as-is (FORWARD row) | 26 |
| F2 | forward with an output filter | 4 |
| S | device-owned answer, never forwarded | 11 |
| G | guest object: moves to the C plan | 12 |
| R | refuse (NOT_SUPPORTED with a label) | 13 |
| N | never sent on this configuration | 4 |

- **Every forward targets the subdevice.** All 30 forwards (F1, F2) are NV2080 controls on the
  subdevice, and all are NON_PRIVILEGED. The existing path (nvml.rs:141-156, `HostRmClient`
  hostrm.rs:75-85) carries them without new host objects or privileges. None of the other
  classes in category B could be forwarded as they are: ZbcApi, DiagApi, I2cApi,
  GenericEngineApi, KernelSMDebuggerSession, ProfilerBase and RmClientResource. For any class
  other than subdevice, device and BINAPI, `HostRmClient::control` issues the control on the
  host root client (hostrm.rs:84). A forward row for such a class would therefore reach the
  wrong object.
- **Isolation.** Seven controls return data the guest must not see as the host has it:
  - `GPU_QUERY_SCRUBBER_STATUS` and `FB_GET_DYNAMIC_OFFLINED_PAGES`: FB addresses outside the
    reservation.
  - `PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2`: host PIDs and process names.
  - `GET_ZBC_CLEAR_TABLE_ENTRY`: the desktop's ZBC entries.
  - `GR_GET_CURRENT_RESIDENT_CHANNEL`: host ChIDs.
  - `RC_GET_ERROR_COUNT` and `RC_GET_ERROR_V2`: the host's RC records.

  `FB_GET_CTAGS_FOR_CBC_EVICTION` returns comptags and is D16 territory. Two controls are
  primitives, not getters: `BUS_GET_UPHY_DLN_CFG_SPACE` is a raw PHY register read and
  `BUS_GET_EOM_STATUS` runs an eye measurement. The same review finds that two rows forwarded
  today already return host FB addresses (§6).
- **Top ten by client likelihood** (§5):
  1. `NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY`
  2. `GPU_QUERY_ILLUM_SUPPORT`
  3. `GPU_GET_ILLUM`
  4. `GPU_GET_RESET_STATUS`
  5. `GPU_GET_DRAIN_AND_RESET_STATUS`
  6. `ECC_GET_VOLATILE_COUNTS`
  7. `PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2`
  8. `NV90E6_CTRL_CMD_MASTER_GET_ERROR_INTR_OFFSET_MASK`
  9. `BIOS_GET_UEFI_SUPPORT`
  10. `BIOS_GET_NBSI_V2`

## 1. How a B control reaches the device, and how a forward leaves it

**On the real card.** A control with `RMCTRL_FLAGS_ROUTE_TO_PHYSICAL` (0x40) is sent to GSP by
the kernel's prologue before any kernel handler runs. The same code is at `rmresControl_Prologue_IMPL`,
src/nvidia/src/kernel/rmapi/resource.c:254 in both trees, with the flag tests at :220, :244 and
:266. The kernel sends it as `GSP_RM_CONTROL` with the caller's `hClient`/`hObject`. The
`rpc_gsp_rm_control_v03_00` header is 40 bytes in both versions
([gsp-transport-615.md](../gsp-transport-615.md) §7 and the table at :755-766). A control
without the flag reaches GSP only when a kernel handler sends it itself, through
`GPU_GET_PHYSICAL_RMAPI(pGpu)->Control` or `NV_RM_RPC_CONTROL`. The "Route" column below names
that handler and the condition it tests. For the four GPUACCT controls the condition is false
on a bare-metal GSP client, so they never arrive. For five more, the kernel's own send depends
on a condition that is false here, but a user client can still send them:
`GPU_IS_RESET_COUPLED` (self-hosted only), `MASTER_GET_VIRTUAL_FUNCTION_ERROR_CONT_INTR_MASK`
(ECC enabled), `TIMER_GET_GPU_CPU_TIME_CORRELATION_INFO` (GSP clock only), and
`RC_GET_ERROR_COUNT`/`_V2` (admin callers only).

**In the device.** `Dispatcher::control` (crates/vfio-user-nvgpu/src/ctrl.rs:797) looks up the
guest object's class in `objects` (ctrl.rs:95; an unknown handle counts as a subdevice). It
hands the control to `nvml::answer`. If the id is in `FORWARD` (ver/v610.rs:921), that function
forwards it unchanged, to the host device for an NV0080 id and to the host subdevice otherwise
(nvml.rs:141-156, `forward` :84). `Dispatcher::forward_or` (ctrl.rs:1213) is the same forward
with a local fallback when there is no host. On the host side `HostRmClient`
(crates/vfio-user-nvgpu/src/hostrm.rs:75-107) is an unprivileged client (decision D10) holding
one root, one device, one subdevice and a lazily allocated BINAPI object. So:

- a NON_PRIVILEGED subdevice or device control can be forwarded;
- a PRIVILEGED one needs the scoped CAP_SYS_ADMIN window of D12 or stays refused;
- a kernel-only or INTERNAL one cannot be issued from the host's user space at all.

**What forwarding cannot fix.** The host answer describes the host card as every host client
sees it. That is right for card properties (board LED, reset required, ECC counters, link
counters, clocks). It is wrong when the answer depends on the caller or holds state of other
host clients. The "host answer is" and "filter" columns mark those cases.

## 2. The 70 controls

Columns:

- **Export entry.** The line of the `__nvoc_exported_method_def_*` entry in
  `src/nvidia/generated/g_*_nvoc.c`, 610 then 615.
- **Header.** The `typedef struct` line under `src/common/sdk/nvidia/inc/ctrl/`, one number
  where both trees agree.
- **Size.** `sizeof` in each tree.
- **Route.** How the control reaches GSP on a real TU106 with this kernel RM. "GSP" is the
  prologue path of §1. Kernel paths are cited in the 615 tree (`src/nvidia/src/kernel/...`),
  with the 610 line where it differs.
- **Group.** The group of §3. "(to X)" marks a control that belongs in category X of
  gsp-surface.md.

| # | Control, id | NVOC class, export entry (610 / 615) | Params struct, header | Size 610 / 615 | Privilege | Route on the real card | The host answer is | Isolation filter | Known client | Recipe | Group |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | `NV0000_CTRL_CMD_GPUACCT_GET_ACCOUNTING_STATE` 0x00000B02 | RmClientResource, g_client_resource_nvoc.c:2048 / :2034 | `NV0000_CTRL_GPUACCT_GET_ACCOUNTING_STATE_PARAMS`, ctrl0000gpuacct.h:113 | 12 / 12 | NON_PRIVILEGED | kernel only here: `cliresCtrlCmdGpuAcctGetAccountingState_IMPL` (client_resource.c:2140 / 610 :2141) sends to GSP only if `IS_VGPU_GSP_PLUGIN_OFFLOAD_ENABLED` and pid != 0 (send :2182 / 610 :2183); that flag needs `bSriovEnabled` (gpu_registry.c:133, both), false on the PF; otherwise answered from the kernel's own accounting | host process accounting (PIDs) | deny: host PIDs (D9 deny list) | NVML (native nvml.jsonl:181, NV_OK); answered by the guest kernel, never reaches the device | none; if it ever arrives: NOT_SUPPORTED, label `refused (host accounting)` | N (to E) |
| 2 | `NV0000_CTRL_CMD_GPUACCT_GET_PROC_ACCOUNTING_INFO` 0x00000B03 | RmClientResource, g_client_resource_nvoc.c:2063 / :2049 | `NV0000_CTRL_GPUACCT_GET_PROC_ACCOUNTING_INFO_PARAMS`, ctrl0000gpuacct.h:162 | 48 / 48 | NON_PRIVILEGED | kernel only here: same guard (client_resource.c:1905, send :1952 / 610 :1906, :1953) | host process accounting | deny: PIDs | NVML process accounting (`nvidia-smi --query-accounted-apps`); kernel-answered | none; NOT_SUPPORTED if it arrives | N (to E) |
| 3 | `NV0000_CTRL_CMD_GPUACCT_GET_ACCOUNTING_PIDS` 0x00000B04 | RmClientResource, g_client_resource_nvoc.c:2078 / :2064 | `NV0000_CTRL_GPUACCT_GET_ACCOUNTING_PIDS_PARAMS`, ctrl0000gpuacct.h:209 | 16012 / 16012 | NON_PRIVILEGED | kernel only here: same guard (client_resource.c:2194, send :2237 / 610 :2195, :2238) | host PID table | deny: PIDs | NVML accounting; kernel-answered | none; NOT_SUPPORTED if it arrives | N (to E) |
| 4 | `NV0000_CTRL_CMD_GPUACCT_GET_PROC_ACCOUNTING_INFO_V2` 0x00000B06 | RmClientResource, g_client_resource_nvoc.c:2108 / :2094 | `NV0000_CTRL_GPUACCT_GET_PROC_ACCOUNTING_INFO_V2_PARAMS`, ctrl0000gpuacct.h:306 | 64 / 64 | NON_PRIVILEGED | kernel only here: same guard (client_resource.c:1964, send :2001 / 610 :1965, :2002) | host process accounting | deny: PIDs | NVML accounting; kernel-answered | none; NOT_SUPPORTED if it arrives | N (to E) |
| 5 | `NV0090_CTRL_CMD_GET_MMU_DEBUG_MODE` 0x00900105 | KernelChannelGroupApi, g_kernel_channel_group_api_nvoc.c:272 / :279 | `NV0090_CTRL_GET_MMU_DEBUG_MODE_PARAMS`, ctrl0090.h:84 | 1 / 1 | NON_PRIVILEGED | kernel then GSP: `kgrctxCtrlGetMMUDebugMode_IMPL` forwards on a GSP client with the caller's TSG/channel handles (kernel_graphics_context.c:3417, both); the fault-path send is SR-IOV VF only in 615 (kern_gmmu.c:3034 in `kgmmuServiceMmuFault_VF` :3004); in 610 the same code is `kgmmuServiceMmuFault_GV100` (kern_gmmu_gv100.c:2005, send :2042), reached from the non-replayable fault path that MMU_FAULT_QUEUED starts (kernel_gsp.c:1002, :1451), an event 615 no longer dispatches | per GR context (the guest's TSG) | handles are guest handles: resolve to the TSG, never the host's | cuda-gdb and the SM debugger (not in the native cuda-gdb trace); none known | C plan: `Mirror::forward_control` (mirror.rs:1711) on the host TSG, else bMode 0 from the registry | G (to C) |
| 6 | `NV2080_CTRL_CMD_GPU_QUERY_ECC_CONFIGURATION` 0x20800133 | Subdevice, g_subdevice_nvoc.c:4239 / :4330 | `NV2080_CTRL_GPU_QUERY_ECC_CONFIGURATION_PARAMS`, ctrl2080gpu.h:1343 / :1344 | 8 / 8 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x40048) | card | none | none known (NVML ECC mode; `GPU_QUERY_ECC_STATUS` is already forwarded and answers 0x56 on this GeForce, 595-static device-events.txt, log line 99316) | `FORWARD` row (ver/v610.rs:921, nvml.rs:141) | F1 |
| 7 | `NV2080_CTRL_CMD_GPU_QUERY_ILLUM_SUPPORT` 0x20800153 | Subdevice, g_subdevice_nvoc.c:4464 / :4555 | `NV2080_CTRL_CMD_GPU_QUERY_ILLUM_SUPPORT_PARAMS`, ctrl2080gpu.h:1885 / :1886 | 8 / 8 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x40048) | card (board LED) | none | seen: the 595 guest's nvidia-smi run (caraxes/runs/guest-driver-2026-09-24/595-static/device-events.txt:12, answered 0x56 [unanswered]) | `FORWARD` row | F1 |
| 8 | `NV2080_CTRL_CMD_GPU_GET_ILLUM` 0x20800154 | Subdevice, g_subdevice_nvoc.c:4479 / :4570 | `NV2080_CTRL_CMD_GPU_ILLUM_PARAMS`, ctrl2080gpu.h:1901 / :1902 | 8 / 8 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | card (board LED) | none | user space, probably NVML: in the list of controls whose NOT_SUPPORTED a vGPU guest silences (rmapi_utils.c:198-251, both trees, case at :231) | `FORWARD` row | F1 |
| 9 | `NV2080_CTRL_CMD_GPU_QUERY_SCRUBBER_STATUS` 0x2080015F | Subdevice, g_subdevice_nvoc.c:4569 / :4660 | `NV2080_CTRL_GPU_QUERY_SCRUBBER_STATUS_PARAMS`, ctrl2080gpu.h:2127 / :2128 | 24 / 24 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x40048) | card, with FB addresses of the host card | memory range: `scrubStartAddr`/`scrubEndAddr` (@8, @16) are host FB addresses; report them only when inside the reservation [B, E) (memory-model.md §1), else 0 | none known | `Dispatcher::forward_or` (ctrl.rs:1213) plus a range clamp | F2 |
| 10 | `NV2080_CTRL_CMD_GPU_QUERY_FUNCTION_STATUS` 0x20800173 | Subdevice, g_subdevice_nvoc.c:4734 / :4825 | `NV2080_CTRL_CMD_GPU_QUERY_FUNCTION_STATUS_PARAMS`, ctrl2080gpu.h:2497 / :2498 | 12 / 12 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | card (USB-C/PPC function) | none | user space, probably NVML: in the list of controls whose NOT_SUPPORTED a vGPU guest silences (rmapi_utils.c:198-251, both trees, case at :233) | `FORWARD` row | F1 |
| 11 | `NV2080_CTRL_CMD_GPU_GET_PARTITION_CAPACITY` 0x20800181 | Subdevice, g_subdevice_nvoc.c:4869 / :4960 | `NV2080_CTRL_GPU_GET_PARTITION_CAPACITY_PARAMS`, ctrl2080gpu.h:3198 / :3199 | 280 / 280 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x40048) | MIG partitions (none on Turing) | feature absent | none; MIG tools | E family rule (MIG): NOT_SUPPORTED | R (to E) |
| 12 | `NV2080_CTRL_CMD_GPU_GET_RESET_STATUS` 0x208001AB | Subdevice, g_subdevice_nvoc.c:5214 / :5305 | `NV2080_CTRL_GPU_GET_RESET_STATUS_PARAMS`, ctrl2080gpu.h:4385 / :4424 | 1 / 1 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x158) | card (reset required) | none; the guest cannot reset the card, but the flag is the truth for its work too | user space, probably NVML: in the list of controls whose NOT_SUPPORTED a vGPU guest silences (rmapi_utils.c:198-251, both trees, case at :205) | `FORWARD` row | F1 |
| 13 | `NV2080_CTRL_CMD_GPU_GET_DRAIN_AND_RESET_STATUS` 0x208001AE | Subdevice, g_subdevice_nvoc.c:5259 / :5350 | `NV2080_CTRL_GPU_GET_DRAIN_AND_RESET_STATUS_PARAMS`, ctrl2080gpu.h:4431 / :4470 | 1 / 1 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | card | none | user space, probably NVML: in the list of controls whose NOT_SUPPORTED a vGPU guest silences (rmapi_utils.c:198-251, both trees, case at :206) | `FORWARD` row | F1 |
| 14 | `NV2080_CTRL_GPU_GET_FIPS_STATUS` 0x208001E4 | Subdevice, g_subdevice_nvoc.c:5349 / :5440 | `NV2080_CTRL_GPU_GET_FIPS_STATUS_PARAMS`, ctrl2080gpu.h:4547 / :4586 | 1 / 1 | PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x44) | host platform mode (FIPS, Hopper+ CC) | PRIVILEGED on the host: the unprivileged host client (D10) cannot forward it | none known | synthesized `bFipsEnabled` 0 (chip data: no FIPS mode on TU106) | S (to A) |
| 15 | `NV2080_CTRL_CMD_TIMER_GET_GPU_CPU_TIME_CORRELATION_INFO` 0x20800406 | Subdevice, g_subdevice_nvoc.c:5739 / :5845 | `NV2080_CTRL_TIMER_GET_GPU_CPU_TIME_CORRELATION_INFO_PARAMS`, ctrl2080tmr.h:201 | 264 / 264 | NON_PRIVILEGED | kernel; to GSP only for `_PROCESSOR_GSP`: `subdeviceCtrlCmdTimerGetGpuCpuTimeCorrelationInfo_IMPL` (subdevice_ctrl_timer_kernel.c:301, both) answers CPU-clock requests itself and forwards only a GSP-clock request (send :338, `IS_FW_CLIENT`) | a (GSP OS time, PTIMER) pair of the emulated GSP | never forward: the host GSP clock and the host PTIMER are not the guest's time base | nvidia-modeset asks for `_PROCESSOR_CPU` (nvkms-evo.c:8821-8837 / 610 :8864-8880) and NVML for cpuClkId 0x01 (OSTIME, CPU; native nvml.jsonl:105): both answered by the guest kernel; no client known that asks for the GSP clock | synthesized: per sample cpuTime from the device clock, gpuTime from the device PTIMER | S |
| 16 | `NV2080_CTRL_CMD_BIOS_GET_POST_TIME` 0x20800809 | Subdevice, g_subdevice_nvoc.c:5859 / :5965 | `NV2080_CTRL_CMD_BIOS_GET_POST_TIME_PARAMS`, ctrl2080bios.h:410 | 8 / 8 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x40048) | card (VBIOS POST time) | none | none known | `FORWARD` row | F1 |
| 17 | `NV2080_CTRL_CMD_BIOS_GET_UEFI_SUPPORT` 0x2080080B | Subdevice, g_subdevice_nvoc.c:5874 / :5980 | `NV2080_CTRL_BIOS_GET_UEFI_SUPPORT_PARAMS`, ctrl2080bios.h:464 | 8 / 8 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | the VBIOS image the guest sees | must agree with the synthesized VBIOS (vbios-synthesis.md), not with the host card's ROM | user space, probably NVML: in the list of controls whose NOT_SUPPORTED a vGPU guest silences (rmapi_utils.c:198-251, both trees, case at :226) | board table (A): UEFI version and flags of the device's own VBIOS; forward only if the synthesized image carries the host's UEFI part | S (to A) |
| 18 | `NV2080_CTRL_CMD_BIOS_GET_NBSI_V2` 0x2080080E | Subdevice, g_subdevice_nvoc.c:5889 / :5995 | `NV2080_CTRL_BIOS_GET_NBSI_V2_PARAMS`, ctrl2080bios.h:258 | 1308 / 1308 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | NBSI (OEM system data from VBIOS/SBIOS) | host platform data; the synthesized VBIOS has no NBSI | user space, probably NVML: in the list of controls whose NOT_SUPPORTED a vGPU guest silences (rmapi_utils.c:198-251, both trees, case at :225) | synthesized: `errorCode` not found, `retSize` 0 (NOT_SUPPORTED on measurement) | S |
| 19 | `NV2080_CTRL_CMD_CLK_PMUMON_CLK_DOMAINS_GET_SAMPLES` 0x20801037 | Subdevice, g_subdevice_nvoc.c:8874 / :9025 | `NV2080_CTRL_CLK_PMUMON_CLK_DOMAINS_GET_SAMPLES_PARAMS`, ctrl2080clk.h:110 | 6024 / 6024 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x40048) | card telemetry (clock samples) | none (D9 telemetry) | none known | `FORWARD` row | F1 |
| 20 | `NV2080_CTRL_CMD_FIFO_OBJSCHED_SW_GET_LOG` 0x2080110E | Subdevice, g_subdevice_nvoc.c:8979 / :9130 | `NV2080_CTRL_FIFO_OBJSCHED_SW_GET_LOG_PARAMS`, ctrl2080fifo.h:472 / :452 | 24024 / 24024 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | host-wide software scheduler log (all runlists, all tenants) | deny: other tenants' scheduling | none known (vGPU scheduler tools) | NOT_SUPPORTED, label `refused (host scheduler)` | R |
| 21 | `NV2080_CTRL_CMD_FIFO_OBJSCHED_GET_STATE` 0x20801120 | Subdevice, g_subdevice_nvoc.c:9159 / :9310 | `NV2080_CTRL_FIFO_OBJSCHED_GET_STATE_PARAMS`, ctrl2080fifo.h:954 / :934 | 20 / 20 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | host scheduler policy (global) | none (policy only, no per-client data) | none known | `FORWARD` row | F1 |
| 22 | `NV2080_CTRL_CMD_GR_GET_CTXSW_STATS` 0x20801215 | Subdevice, g_subdevice_nvoc.c:9429 / :9580 | `NV2080_CTRL_GR_GET_CTXSW_STATS_PARAMS`, ctrl2080gr.h:1022 / :1020 | 32 / 32 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | per channel (`hChannel` @0 is a guest handle) | handle in the params: translate to the mirror's host channel or answer from the registry | none known | C plan: host channel handle from `Mirror` (mirror.rs:132), forward on the host subdevice | G (to C) |
| 23 | `NV2080_CTRL_CMD_GR_GET_CURRENT_RESIDENT_CHANNEL` 0x2080121C | Subdevice, g_subdevice_nvoc.c:9489 / :9640 | `NV2080_CTRL_GR_GET_CURRENT_RESIDENT_CHANNEL_PARAMS`, ctrl2080gr.h:1246 / :1250 | 24 / 24 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | host-wide: the ChID resident on GR now, possibly another tenant's | deny host ChIDs; answer the guest ChID of a mirrored channel or none | none known | C plan: map host ChID to guest ChID through the mirror, else 0xFFFFFFFF | G (to C) |
| 24 | `NV2080_CTRL_CMD_GR_GFX_POOL_QUERY_SIZE` 0x2080121F | Subdevice, g_subdevice_nvoc.c:9519 / :9670 | `NV2080_CTRL_GR_GFX_POOL_QUERY_SIZE_PARAMS`, ctrl2080gr.h:1296 / :1300 | 40 / 40 | kernel only | GSP (ROUTE_TO_PHYSICAL, flags 0x40) | chip (graphics context pool geometry) | kernel-privileged (flags 0x40): a user client on the host cannot issue it | none: no caller in either tree | E rule (no caller) or chip table | S (to E) |
| 25 | `NV2080_CTRL_CMD_GR_GET_CTXSW_MODES` 0x20801235 | Subdevice, g_subdevice_nvoc.c:9744 / :9895 | `NV2080_CTRL_GR_GET_CTXSW_MODES_PARAMS`, ctrl2080gr.h:1850 / :1856 | 24 / 24 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | per channel (`hChannel` @0) | handle translation as GR_GET_CTXSW_STATS | none known (profilers) | C plan, as 0x20801215 | G (to C) |
| 26 | `NV2080_CTRL_CMD_FB_GET_CALIBRATION_LOCK_FAILED` 0x2080130C | Subdevice, g_subdevice_nvoc.c:9894 / :10045 | `NV2080_CTRL_FB_GET_CALIBRATION_LOCK_FAILED_PARAMS`, ctrl2080fb.h:558 / :559 | 24 / 24 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | card (memory training) | none | user space, probably NVML: in the list of controls whose NOT_SUPPORTED a vGPU guest silences (rmapi_utils.c:198-251, both trees, case at :230) | `FORWARD` row | F1 |
| 27 | `NV2080_CTRL_CMD_FB_GET_LTC_INFO_FOR_FBP` 0x20801328 | Subdevice, g_subdevice_nvoc.c:9999 / :10150 | `NV2080_CTRL_FB_GET_LTC_INFO_FOR_FBP_PARAMS`, ctrl2080fb.h:1263 / :1266 | 20 / 20 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x50158) | card floorsweeping per FBP | must agree with the FBP/LTC masks the device advertises (same silicon, forwarded GR masks, kgr.rs) | none known | `FORWARD` row (or chip data, A) | F1 |
| 28 | `NV2080_CTRL_CMD_FB_GET_CTAGS_FOR_CBC_EVICTION` 0x20801338 | Subdevice, g_subdevice_nvoc.c:10059 / :10210 | `NV2080_CTRL_FB_GET_CTAGS_FOR_CBC_EVICTION_PARAMS`, ctrl2080fb.h:1556 / :1559 | 512 / 512 | PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x44) | comptags of the host's compression backing | D16: never forward; no comptag reaches or leaves the host | none known (PRIVILEGED) | synthesized `numCompTags` 0, label `synthesized (no compression, D16)` | S |
| 29 | `NV2080_CTRL_CMD_FB_GET_ROW_REMAPPER_HISTOGRAM` 0x20801347 | Subdevice, g_subdevice_nvoc.c:10224 / :10375 | `NV2080_CTRL_FB_GET_ROW_REMAPPER_HISTOGRAM_PARAMS`, ctrl2080fb.h:2369 / :2372 | 20 / 20 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x58) | card (row remapper) | none | none known (NVML row-remap histogram) | `FORWARD` row | F1 |
| 30 | `NV2080_CTRL_CMD_FB_GET_DYNAMIC_OFFLINED_PAGES` 0x20801348 | Subdevice, g_subdevice_nvoc.c:10239 / :10390 | `NV2080_CTRL_FB_GET_DYNAMIC_OFFLINED_PAGES_PARAMS`, ctrl2080fb.h:2456 / :2459 | 1040 / 1040 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x50048) | card, host FB page numbers (4 KiB pages: `pageNumber << RM_PAGE_SHIFT`, mem_mgr_ga100.c:416 / 610 :417) | memory range: keep entries whose page lies in [B, E), recount `validEntries` (@1024) | kernel caller only in the Ampere HAL (mem_mgr_ga100.c:385 / 610 :386), not on TU106; NVML retired pages | `forward_or` plus an entry filter | F2 |
| 31 | `NV2080_CTRL_CMD_FB_QUERY_DRAM_ENCRYPTION_PENDING_CONFIGURATION` 0x20801355 | Subdevice, g_subdevice_nvoc.c:10329 / :10480 | `NV2080_CTRL_FB_QUERY_DRAM_ENCRYPTION_PENDING_CONFIGURATION_PARAMS`, ctrl2080fb.h:2802 / :2805 | 4 / 4 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x40048) | card (DRAM encryption, Blackwell) | none | none known; sibling INFOROM_SUPPORT already in `FORWARD` (ver/v610.rs:929) | `FORWARD` row | F1 |
| 32 | `NV2080_CTRL_CMD_FB_QUERY_DRAM_ENCRYPTION_STATUS` 0x20801358 | Subdevice, g_subdevice_nvoc.c:10374 / :10525 | `NV2080_CTRL_FB_QUERY_DRAM_ENCRYPTION_STATUS_PARAMS`, ctrl2080fb.h:2880 / :2883 | 4 / 4 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x40048) | card | none | as 0x20801355 | `FORWARD` row | F1 |
| 33 | `NV2080_CTRL_CMD_BUS_GET_PEX_LANE_COUNTERS` 0x20801816 | Subdevice, g_subdevice_nvoc.c:10689 / :10840 | `NV2080_CTRL_CMD_BUS_GET_PEX_LANE_COUNTERS_PARAMS`, ctrl2080bus.h:962 / :1011 | 18 / 18 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | card PCIe lanes | none (the sibling BUS_GET_PEX_COUNTERS is forwarded, ver/v610.rs:936) | none known | `FORWARD` row | F1 |
| 34 | `NV2080_CTRL_CMD_BUS_GET_PCIE_LTR_LATENCY` 0x20801817 | Subdevice, g_subdevice_nvoc.c:10704 / :10855 | `NV2080_CTRL_CMD_BUS_GET_PCIE_LTR_LATENCY_PARAMS`, ctrl2080bus.h:982 / :1031 | 12 / 12 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | card and host root port (LTR) | none; tells the guest the host root port's LTR support | user space, probably NVML: in the list of controls whose NOT_SUPPORTED a vGPU guest silences (rmapi_utils.c:198-251, both trees, case at :227) | `FORWARD` row | F1 |
| 35 | `NV2080_CTRL_CMD_BUS_GET_UPHY_DLN_CFG_SPACE` 0x20801827 | Subdevice, g_subdevice_nvoc.c:10824 / :10975 | `NV2080_CTRL_CMD_BUS_GET_UPHY_DLN_CFG_SPACE_PARAMS`, ctrl2080bus.h:1105 / :1154 | 12 / 12 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | raw UPHY register at a caller-chosen `regAddress` | deny: a register read primitive | none known | NOT_SUPPORTED, label `refused (raw PHY register)` | R |
| 36 | `NV2080_CTRL_CMD_BUS_GET_EOM_STATUS` 0x20801828 | Subdevice, g_subdevice_nvoc.c:10839 / :10990 | `NV2080_CTRL_BUS_GET_EOM_STATUS_PARAMS`, ctrl2080bus.h:1121 / :1170 | 76 / 76 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | runs an eye-opening measurement on the link | deny: acts on the physical link | none known | NOT_SUPPORTED | R |
| 37 | `NV2080_CTRL_CMD_PERF_GET_LEVEL_INFO_V2` 0x2080200B | Subdevice, g_subdevice_nvoc.c:11049 / :11215 | `NV2080_CTRL_PERF_GET_LEVEL_INFO_V2_PARAMS`, ctrl2080perf.h:679 / :683 | 780 / 780 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x50048) | card (clocks per perf level) | none | none known | `FORWARD` row | F1 |
| 38 | `NV2080_CTRL_CMD_PERF_GET_POWERSTATE` 0x2080205A | Subdevice, g_subdevice_nvoc.c:11064 / :11230 | `NV2080_CTRL_PERF_GET_POWERSTATE_PARAMS`, ctrl2080perf.h:767 / :771 | 4 / 4 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x40048) | host platform (AC/battery) | none | none known | `FORWARD` row | F1 |
| 39 | `NV2080_CTRL_CMD_PERF_GET_TEGRA_PERFMON_SAMPLE` 0x20802069 | Subdevice, g_subdevice_nvoc.c:11124 / :11290 | `NV2080_CTRL_PERF_GET_TEGRA_PERFMON_SAMPLE_PARAMS`, ctrl2080perf.h:920 / :924 | 8 / 8 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | Tegra only | feature absent | Tegra path only (arch/nvalloc/unix/src/osapi.c:6343) | E rule: NOT_SUPPORTED | R (to E) |
| 40 | `NV2080_CTRL_CMD_PERF_RATED_TDP_GET_CONTROL` 0x2080206E | Subdevice, g_subdevice_nvoc.c:11139 / :11305 | `NV2080_CTRL_PERF_RATED_TDP_CONTROL_PARAMS`, ctrl2080perf.h:405 / :409 | 12 / 12 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x4a) | card (rated-TDP arbitration, read) | none (read only; the setter is D) | none known | `FORWARD` row | F1 |
| 41 | `NV2080_CTRL_CMD_PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2` 0x20802096 | Subdevice, g_subdevice_nvoc.c:11214 / :11380 | `NV2080_CTRL_PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2_PARAMS`, ctrl2080perf.h:558 / :562 | 55888 / 55888 | NON_PRIVILEGED | kernel then GSP: `subdeviceCtrlCmdPerfGetGpumonPerfmonUtilSamplesV2_KERNEL` (kern_perf_ctrl.c:95, both) forwards with the caller's handles (:112) and then rewrites `procId` for non-init namespaces; the kernel's own accounting sampler also sends it (diagnostics/gpu_acct.c:508, both) while accounting is on | host-wide utilization samples with per-process attribution of host processes | per sample (776 bytes @16): zero `procId`, `subProcessID`, `subProcessName`, `pOsPidInfo` (@8, @12, @16, @120 of each 128-byte engine sample); zero the engines the guest does not see; the load includes the host desktop (P7) | NVML process utilization (`nvidia-smi pmon`), gsp-surface.md §6 "close behind"; not in the native nvml trace | `forward_or` plus a per-sample filter | F2 |
| 42 | `NV2080_CTRL_CMD_RC_GET_ERROR_COUNT` 0x20802205 | Subdevice, g_subdevice_nvoc.c:11244 / :11410 | `NV2080_CTRL_RC_GET_ERROR_COUNT_PARAMS`, ctrl2080rc.h:90 | 4 / 4 | NON_PRIVILEGED | kernel then GSP: `subdeviceCtrlCmdRcGetErrorCount_IMPL` (kernel_rc_ctrl.c:103, both) requires an admin client (`krcSubdeviceCtrlGetErrorInfoCheckPermissions_KERNEL` :73) and forwards on a GSP client (:123) | the RC error database of GSP: on the host, every tenant's RC records | deny: host RC records (D9 deny list); the unprivileged host client is refused anyway | root tools (RC diagnostics); none known | C plan: count of RC errors the device raised for this guest (0 until RC_TRIGGERED exists) | G (to C) |
| 43 | `NV2080_CTRL_CMD_GET_RC_RECOVERY` 0x2080220E | Subdevice, g_subdevice_nvoc.c:11349 / :11515 | `NV2080_CTRL_CMD_RC_RECOVERY_PARAMS`, ctrl2080rc.h:251 / :254 | 4 / 4 | PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x40154) | host RC recovery policy | PRIVILEGED on the host | none known | synthesized `rcEnable` ENABLED (the device's policy) | S |
| 44 | `NV2080_CTRL_CMD_RC_GET_ERROR_V2` 0x20802213 | Subdevice, g_subdevice_nvoc.c:11409 / :11575 | `NV2080_CTRL_RC_GET_ERROR_V2_PARAMS`, ctrl2080rc.h:124 | 8200 / 8200 | NON_PRIVILEGED | kernel then GSP: `subdeviceCtrlCmdRcGetErrorV2_IMPL` (kernel_rc_ctrl.c:136, both), admin check, send :156 | GSP RC record buffer (host tenants) | deny: host RC records | root tools; none known | C plan: the device's own RC journal (empty until RC events exist) | G (to C) |
| 45 | `NV2080_CTRL_CMD_FLCN_GET_DMEM_USAGE` 0x20803101 | Subdevice, g_subdevice_nvoc.c:12834 / :13090 | `NV2080_CTRL_FLCN_GET_DMEM_USAGE_PARAMS`, ctrl2080flcn.h:75 | 12 / 12 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | card (falcon DMEM heap) | none; `flcnID` of a falcon the guest does not see still answers (read only) | none known | `FORWARD` row | F1 |
| 46 | `NV2080_CTRL_CMD_ECC_GET_CLIENT_EXPOSED_COUNTERS` 0x20803400 | Subdevice, g_subdevice_nvoc.c:12939 / :13195 | `NV2080_CTRL_ECC_GET_CLIENT_EXPOSED_COUNTERS_PARAMS`, ctrl2080ecc.h:66 | 96 / 96 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | card ECC (GeForce: no ECC) | none | none known | `FORWARD` row | F1 |
| 47 | `NV2080_CTRL_CMD_ECC_GET_VOLATILE_COUNTS` 0x20803401 | Subdevice, g_subdevice_nvoc.c:12954 / :13210 | `NV2080_CTRL_ECC_GET_VOLATILE_COUNTS_PARAMS`, ctrl2080ecc.h:104 | 40 / 40 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | card ECC | none | user space, probably NVML: in the list of controls whose NOT_SUPPORTED a vGPU guest silences (rmapi_utils.c:198-251, both trees, case at :210) | `FORWARD` row | F1 |
| 48 | `NV2080_CTRL_CMD_ECC_GET_SRAM_UNIQUE_UNCORR_COUNTS` 0x20803402 | Subdevice, g_subdevice_nvoc.c:12969 / :13225 | `NV2080_CTRL_ECC_GET_SRAM_UNIQUE_UNCORR_COUNTS_PARAMS`, ctrl2080ecc.h:160 | 16804 / 16804 | PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x44) | card ECC SRAM locations | PRIVILEGED on the host (D12 window or refused) | none known | NOT_SUPPORTED as a card without ECC answers (measure, Q-B1) | S |
| 49 | `NV2080_CTRL_CMD_ECC_GET_REPAIR_STATUS` 0x20803404 | Subdevice, g_subdevice_nvoc.c:12999 / :13255 | `NV2080_CTRL_ECC_GET_REPAIR_STATUS_PARAMS`, ctrl2080ecc.h:217 | 2 / 2 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | card | none | user space, probably NVML: in the list of controls whose NOT_SUPPORTED a vGPU guest silences (rmapi_utils.c:198-251, both trees, case at :241) | `FORWARD` row | F1 |
| 50 | `NV2080_CTRL_CMD_ECC_GET_UNREPAIRABLE_MEMORY_FLAG` 0x20803406 | Subdevice, g_subdevice_nvoc.c:13029 / :13285 | `NV2080_CTRL_ECC_GET_UNREPAIRABLE_MEMORY_FLAG_PARAMS`, ctrl2080ecc.h:252 | 1 / 1 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | card | none | user space, probably NVML: in the list of controls whose NOT_SUPPORTED a vGPU guest silences (rmapi_utils.c:198-251, both trees, case at :242) | `FORWARD` row | F1 |
| 51 | `NV2080_CTRL_CMD_GSP_GET_RM_HEAP_STATS` 0x20803602 | Subdevice, g_subdevice_nvoc.c:13119 / :13375 | `NV2080_CTRL_GSP_GET_RM_HEAP_STATS_PARAMS`, ctrl2080gsp.h:131 | 88 / 88 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | host GSP heap (host-wide) | force `gfid` (@0) to 0 (PF) before forwarding; the numbers describe the host's GSP, not the emulated one | none known | `forward_or` with the gfid fixed | F2 |
| 52 | `NV2080_CTRL_CMD_GSP_GET_LIBOS_HEAP_STATS` 0x20803604 | Subdevice, g_subdevice_nvoc.c:13149 / :13405 | `NV2080_CTRL_CMD_GSP_GET_LIBOS_HEAP_STATS_PARAMS`, ctrl2080gsp.h:187 | 1040 / 1040 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x248) | host GSP libos pools | none (aggregate) | none known | `FORWARD` row | F1 |
| 53 | `NV2080_CTRL_CMD_GPU_IS_RESET_COUPLED` 0x208081F3 | Subdevice, g_subdevice_nvoc.c:13554 / :13870 | `NV2080_CTRL_CMD_GPU_IS_RESET_COUPLED_PARAMS`, ctrl2080gpu.h:4921 / :4960 | 1 / 1 | NON_PRIVILEGED | GSP (flags 0x48); the kernel's own send is only for self-hosted GPUs (gpu.c:7322 in `_gpuRefreshRecoveryActionInLock` :7283 / 610 :7423, :7384) | card | none | none on x86 (self-hosted only) | `FORWARD` row | F1 |
| 54 | `NV2080_CTRL_CMD_GPU_GET_UNIT_FS_INFO_FROM_DIELET` 0x208081F5 | Subdevice, g_subdevice_nvoc.c:13584 / :13900 | `NV2080_CTRL_GPU_GET_UNIT_FS_INFO_FROM_DIELET_PARAMS`, ctrl2080gpu.h:4995 / :5034 | 16 / 16 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | Blackwell dielets | feature absent | none | E rule: NOT_SUPPORTED | R (to E) |
| 55 | `NV208F_CTRL_CMD_FIFO_GET_CHANNEL_STATE` 0x208F0403 | DiagApi, g_subdevice_diag_nvoc.c:282 / :283 | `NV208F_CTRL_FIFO_GET_CHANNEL_STATE_PARAMS`, ctrl208ffifo.h:99 | 16 / 16 | NON_PRIVILEGED | kernel then GSP: `diagapiCtrlCmdFifoGetChannelState_IMPL` (kernel_fifo_ctrl.c:630, both) resolves `hClient`/`hChannel` from the params and sends with the channel's handles (`kchannelGetChannelPhysicalState_KERNEL`, kernel_channel.c:4353 / 610 :4139; send :4365 / 610 :4151) | per guest channel | handles are guest handles | diag tools; none known | C plan: bEnabled/bScheduled from the channel registry | G (to C) |
| 56 | `NV208F_CTRL_CMD_GPU_GET_RAM_SVOP_VALUES` 0x208F1101 | DiagApi, g_subdevice_diag_nvoc.c:447 / :448 | `NV208F_CTRL_GPU_RAM_SVOP_VALUES_PARAMS`, ctrl208fgpu.h:58 | 16 / 16 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | SRAM SVOP voltages (diag) | deny: diag class, not opened on the host (nvml.rs refuses VERIFY_INFOROM on it) | none known | NOT_SUPPORTED | R |
| 57 | `NV402C_CTRL_CMD_I2C_GET_PORT_SPEED` 0x402C0103 | I2cApi, g_i2c_api_nvoc.c:200 / :201 | `NV402C_CTRL_I2C_GET_PORT_SPEED_PARAMS`, ctrl402c.h:338 | 64 / 64 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | host I2C port speeds | must agree with the synthesized I2C_GET_PORT_INFO "no ports" (ctrl.rs, row 98) | user space, probably NVML: in the list of controls whose NOT_SUPPORTED a vGPU guest silences (rmapi_utils.c:198-251, both trees, case at :239) | synthesized zeros (or NOT_SUPPORTED) consistent with the port table | S |
| 58 | `NV83DE_CTRL_CMD_DEBUG_GET_MODE_MMU_DEBUG` 0x83DE0308 | KernelSMDebuggerSession, g_kernel_sm_debugger_session_nvoc.c:670 / :672 | `NV83DE_CTRL_DEBUG_GET_MODE_MMU_DEBUG_PARAMS`, ctrl83dedebug.h:148 | 4 / 4 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x10248) | per debugger session | guest object; the host has no debugger session for it | cuda-gdb (flags carry ROUTE_TO_VGPU_HOST); none known | C plan: defaults (disabled) while debugging is refused (D rule 4) | G (to C) |
| 59 | `NV83DE_CTRL_CMD_DEBUG_GET_MODE_ERRBAR_DEBUG` 0x83DE0320 | KernelSMDebuggerSession, g_kernel_sm_debugger_session_nvoc.c:925 / :927 | `NV83DE_CTRL_DEBUG_GET_MODE_ERRBAR_DEBUG_PARAMS`, ctrl83dedebug.h:911 | 4 / 4 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | per debugger session | guest object | cuda-gdb; none known | C plan: default 0 | G (to C) |
| 60 | `NV83DE_CTRL_CMD_DEBUG_GET_SINGLE_SM_DEBUGGER_STATUS` 0x83DE0325 | KernelSMDebuggerSession, g_kernel_sm_debugger_session_nvoc.c:1000 / :1002 | `NV83DE_CTRL_DEBUG_GET_SINGLE_SM_DEBUGGER_STATUS_PARAMS`, ctrl83dedebug.h:1026 | 12 / 12 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | per SM of the debugged context | guest object | cuda-gdb; none known | C plan: zeros | G (to C) |
| 61 | `NV83DE_CTRL_CMD_DEBUG_GET_MODE_MMU_GCC_DEBUG` 0x83DE032B | KernelSMDebuggerSession, g_kernel_sm_debugger_session_nvoc.c:1090 / :1092 | `NV83DE_CTRL_DEBUG_GET_MODE_MMU_GCC_DEBUG_PARAMS`, ctrl83dedebug.h:1226 | 4 / 4 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x248) | per debugger session | guest object | cuda-gdb; none known | C plan: default 0 | G (to C) |
| 62 | `NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY` 0x90960107 | ZbcApi, g_zbc_api_nvoc.c:272 / :273 | `NV9096_CTRL_GET_ZBC_CLEAR_TABLE_ENTRY_PARAMS`, ctrl9096.h:431 | 56 / 56 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x10248) | the GPU-global ZBC table: on the host it holds the desktop's clear values | deny host entries: the guest must see its own table; with D16 no guest surface is compressible, so ZBC entries are never used by the hardware for guest surfaces | every GL, EGL and Vulkan native trace (45 calls in each EGL/GL probe, 135 to 495 in the Vulkan probes, `egl-wayland.jsonl:1468`, NV_OK, 56 bytes); in the guest the sibling GET_ZBC_CLEAR_TABLE_SIZE 0x90960106 comes first and is unanswered (caraxes/runs/display-e-2026-09-24/e14-weston-gl-1080p/device-events.txt:776, 75 times over the display runs), so ENTRY is the next wall once category A answers SIZE | C plan: a device ZBC table (chip defaults plus the guest's SET_ZBC_*), `bIndexValid` 0 for empty slots | G (to C) |
| 63 | `NV90E6_CTRL_CMD_MASTER_GET_ERROR_INTR_OFFSET_MASK` 0x90E60101 | GenericEngineApi, g_generic_engine_nvoc.c:170 / :171 | `NV90E6_CTRL_MASTER_GET_ERROR_INTR_OFFSET_MASK_PARAMS`, ctrl90e6.h:92 | 16 / 16 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | chip: BAR0 offset and mask of the error interrupt register | must match the device's BAR0 model | user space, probably NVML: in the list of controls whose NOT_SUPPORTED a vGPU guest silences (rmapi_utils.c:198-251, both trees, case at :245); the silence applies only below Turing, so a Turing guest expects an answer | chip table (A): offset and masks from the chip file | S (to A) |
| 64 | `NV90E6_CTRL_CMD_MASTER_GET_VIRTUAL_FUNCTION_ERROR_CONT_INTR_MASK` 0x90E60102 | GenericEngineApi, g_generic_engine_nvoc.c:185 / :186 | `NV90E6_CTRL_MASTER_GET_VIRTUAL_FUNCTION_ERROR_CONT_INTR_MASK_PARAMS`, ctrl90e6.h:131 | 8 / 8 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x10248) | chip: VF error-containment masks | must match the device's BAR0 model | UVM `nvGpuOpsGetEccInfo` (nv_gpu_ops.c:1404, both) only when ECC is enabled (:1350-1384) and the NVLink path (:1631) only with NVLink recovery: neither on a GeForce TU106 | chip table (A) | S (to A) |
| 65 | `NVB0CC_CTRL_CMD_PMA_STREAM_UPDATE_GET_PUT` 0xB0CC0109 | ProfilerBase, g_profiler_v2_nvoc.c:464 / :465 | `NVB0CC_CTRL_PMA_STREAM_UPDATE_GET_PUT_PARAMS`, ctrlb0ccprofiler.h:225 | 48 / 48 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x50048) | per PMA stream (profiler); updates `bytesConsumed` | acts on the stream | Nsight/CUPTI; none known | D rule 4: NOT_SUPPORTED (profiling refused) | R (to D) |
| 66 | `NVB0CC_CTRL_CMD_GET_TOTAL_HS_CREDITS` 0xB0CC010D | ProfilerBase, g_profiler_v2_nvoc.c:524 / :525 | `NVB0CC_CTRL_GET_TOTAL_HS_CREDITS_PARAMS`, ctrlb0ccprofiler.h:386 | 4 / 4 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x10248) | profiler HS credits | profiling refused | Nsight/CUPTI | D rule 4: NOT_SUPPORTED | R (to D) |
| 67 | `NVB0CC_CTRL_CMD_GET_HS_CREDITS` 0xB0CC010F | ProfilerBase, g_profiler_v2_nvoc.c:554 / :555 | `NVB0CC_CTRL_GET_HS_CREDITS_PARAMS`, ctrlb0ccprofiler.h:492 | 256 / 256 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x10248) | profiler HS credits | profiling refused | Nsight/CUPTI | D rule 4 | R (to D) |
| 68 | `NVB0CC_CTRL_CMD_GET_CHIPLET_HS_CREDIT_POOL` 0xB0CC0115 | ProfilerBase, g_profiler_v2_nvoc.c:599 / :600 | `NVB0CC_CTRL_GET_CHIPLET_HS_CREDIT_POOL`, ctrlb0ccprofiler.h:614 | 124 / 124 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x10248) | profiler credit pools | profiling refused | Nsight/CUPTI | D rule 4 | R (to D) |
| 69 | `NVB0CC_CTRL_CMD_GET_HS_CREDITS_MAPPING` 0xB0CC0116 | ProfilerBase, g_profiler_v2_nvoc.c:614 / :615 | `NVB0CC_CTRL_GET_HS_CREDITS_POOL_MAPPING_PARAMS`, ctrlb0ccprofiler.h:652 | 194 / 194 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x10248) | profiler credit mapping | profiling refused | Nsight/CUPTI | D rule 4 | R (to D) |
| 70 | `NVB0CC_CTRL_CMD_GET_DYNAMIC_MMA_BOOST_STATUS` 0xB0CC0118 | ProfilerBase, g_profiler_v2_nvoc.c:644 / :645 | `NVB0CC_CTRL_GET_DYNAMIC_MMA_BOOST_STATUS_PARAMS`, ctrlb0ccprofiler.h:695 | 1 / 1 | NON_PRIVILEGED | GSP (ROUTE_TO_PHYSICAL, flags 0x48) | MMA boost status (Hopper+) | profiling refused | Nsight/CUPTI | D rule 4 | R (to D) |

Sources of the client column:

- The native traces of the real card (`matrix/traces/610.57.04/control/*.jsonl` in the main
  checkout, ioctl 0x2a). There are no 615 traces.
- The device logs under `caraxes/runs`: every unanswered control shows as
  `ctrl 0x........ ... [unanswered]`. Of the 101 B and C ids, only `GPU_QUERY_ILLUM_SUPPORT`
  appears in any run log.
- The kernel, nvidia-modeset and UVM call sites in both trees.
- For user-space controls without a trace: the list of controls whose NOT_SUPPORTED a vGPU
  guest silences, `rmapiutilSkipErrorMessageForUnsupportedVgpuGuestControl`
  (src/nvidia/src/kernel/rmapi/rmapi_utils.c:198-251, identical in both trees). Its neighbours
  in that list are the controls behind `nvidia-smi -q` fields: INFOROM versions, OEM board
  info, PEX counters, offlined pages and remapped rows. So the list is the best source-only
  evidence of what NVML sends, but it is not proof.

## 3. Groups and their mechanical recipes

**F1, forward as-is (26).** Public NON_PRIVILEGED subdevice getters whose answer is card truth,
with no handles, addresses, PIDs or other clients' state. The recipe is one line per control
in `FORWARD` (ver/v610.rs:921) and one size assert in hostabi.rs:677. The label is the existing
`host (forwarded as-is, public control)`. On a GeForce TU106 the host answers several of these
with NOT_SUPPORTED: ECC, row remapper and DRAM encryption. `GPU_QUERY_ECC_STATUS` already gets
0x56 from the host (caraxes/runs/guest-driver-2026-09-24/595-static/device-events.txt, log line
99316). Passing that status on is correct: it is what the real card says.

**F2, forward with an output filter (4).** The same forward through `Dispatcher::forward_or`
(ctrl.rs:1213), followed by a filter on the answer. There are three filter kinds:

- *Reservation range:* drop or zero addresses outside the reservation `[B, E)`
  ([memory-model.md](../memory-model.md) §1: guest FB address X is host VRAM address X, so no
  translation is needed, only a range test). Used by `GPU_QUERY_SCRUBBER_STATUS` and
  `FB_GET_DYNAMIC_OFFLINED_PAGES`.
- *Process fields:* zero the host process attribution (`PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2`).
- *Input pin:* force an input field before the forward (`GSP_GET_RM_HEAP_STATS`, `gfid` 0).

The label is `host (forwarded, filtered: <filter>)`.

**S, device-owned answer (11).** The guest must see the device's truth, not the host's. The
device answers from its own model:

- the synthesized VBIOS for `BIOS_GET_UEFI_SUPPORT` and `BIOS_GET_NBSI_V2`;
- the synthesized I2C port table for `I2C_GET_PORT_SPEED`;
- its own clock for `TIMER_GET_GPU_CPU_TIME_CORRELATION_INFO`;
- its own policy for `GET_RC_RECOVERY`;
- D16 for `FB_GET_CTAGS_FOR_CBC_EVICTION`;
- chip data for `GPU_GET_FIPS_STATUS`, the two NV90E6 masks and `GR_GFX_POOL_QUERY_SIZE`.

Five of the eleven are really category A (table-driven chip or board data) and belong in that
handler's table: `GPU_GET_FIPS_STATUS`, `BIOS_GET_UEFI_SUPPORT` and the two NV90E6 masks move to
A, `GR_GFX_POOL_QUERY_SIZE` to E. The label is the table's own (`chip`, `board`) or
`synthesized (...)`.

**G, guest object (12).** The answer depends on a guest object: a TSG, a channel named in the
params, a debugger session or the ZBC table. It is recipe C, taken up in
[c-guest-state.md](c-guest-state.md) §3.

**R, refuse (13).** NOT_SUPPORTED with a label, as the E family rules do. Reasons by control:

- `FIFO_OBJSCHED_SW_GET_LOG`: the host scheduler's log of all tenants.
- `BUS_GET_UPHY_DLN_CFG_SPACE`, `BUS_GET_EOM_STATUS`: they act on or read the physical link.
- `NV208F_CTRL_CMD_GPU_GET_RAM_SVOP_VALUES`: diag class.
- The six NVB0CC profiler controls: profiling is refused (gsp-surface.md §3 D, rule 4).
- `GPU_GET_PARTITION_CAPACITY` (MIG), `PERF_GET_TEGRA_PERFMON_SAMPLE` (Tegra),
  `GPU_GET_UNIT_FS_INFO_FROM_DIELET` (Blackwell): absent features, so they belong in E's family
  rules.

**N, never sent here (4).** The four NV0000 GPUACCT controls are answered by the guest kernel
from its own accounting. They go to GSP only under `IS_VGPU_GSP_PLUGIN_OFFLOAD_ENABLED`, which
requires SR-IOV (gpu_registry.c:133 in both trees). Nothing is needed; if one ever arrives,
NOT_SUPPORTED. They move to E.

## 4. The table-driven form

Groups F1, F2 and R are data, not code. In place of the `FORWARD` list, a `const` table in the
version module (ver/v610.rs, overridden in ver/v615.rs only where a row differs; no row differs
today):

```rust
/// How the device answers a public control it does not model (gsp-plan/b-host-forwardable.md).
pub enum Rule {
    /// Forward unchanged to the host object of `target` (nvml.rs `forward`).
    Forward { target: HostObj },
    /// Forward, then filter the answer (or pin an input) before it reaches the guest.
    ForwardFiltered { target: HostObj, filter: Filter },
    /// NOT_SUPPORTED with a label (feature absent, refused for isolation).
    Refuse { label: &'static str },
}
pub enum HostObj { Subdevice, Device }
pub enum Filter {
    /// u64 FB addresses at the given offsets: kept inside the reservation, else 0.
    ReservationAddrs(&'static [usize]),
    /// FB_GET_DYNAMIC_OFFLINED_PAGES: drop entries outside the reservation, recount.
    OfflinedPages,
    /// PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2: zero the process fields of every sample.
    GpumonProcess,
    /// Force a u32 input field to a value before forwarding (GSP heap stats: gfid 0).
    PinInput { off: usize, value: u32 },
}
pub struct PublicControl {
    pub id: u32,
    pub name: &'static str,
    /// sizeof of the params struct in this driver version; a guest buffer of another size
    /// is refused (NV_ERR_INVALID_PARAM_STRUCT) instead of forwarded.
    pub size: usize,
    pub rule: Rule,
}
```

The two sizes per version live in the table as each version module's own `size`. Both are
equal for all 70 today; the machine-readable form with both sizes, the flags, the group and the
filter of every B row is [b-host-forwardable.toml](b-host-forwardable.toml). Where
`nvrm_sys::v610`/`v615` bind the struct, `size` should come from `size_of`, as the other
version-module constants do (ver/mod.rs:21-23).

**Worked example 1, F1: `NV2080_CTRL_CMD_GPU_GET_RESET_STATUS` 0x208001AB.**

- Params: `{NvBool bResetRequired}`, 1 byte in both trees (ctrl2080gpu.h:4385 in 610, :4424 in
  615). Flags 0x158 (NON_PRIVILEGED, ROUTE_TO_PHYSICAL, API_LOCK_READONLY, GPU_LOCK_DEVICE_ONLY;
  g_subdevice_nvoc.c:5214 / :5305).
- Row: `PublicControl { id: 0x2080_01AB, name: "GPU_GET_RESET_STATUS", size: 1, rule: Rule::Forward { target: HostObj::Subdevice } }`.
- The dispatcher forwards the byte, the host fills it, and the guest gets the host status and
  byte. Test: a unit test with a fake `HostRm` that sets byte 0 to 1 and checks the answer and
  label, plus the size assert in hostabi.rs.

**Worked example 2, F2 range: `NV2080_CTRL_CMD_FB_GET_DYNAMIC_OFFLINED_PAGES` 0x20801348.**

- Params: 1040 bytes in both trees (ctrl2080fb.h, see the table): `offlined[64]` of
  `{u64 pageNumber @0, u8 source @8}`, 16 bytes each, then `validEntries` @1024, `baseIndex`
  @1028, `bMore` @1032. The offsets are compiled, and identical in both trees.
- `pageNumber` counts 4 KiB pages. The kernel's own reader shifts it by `RM_PAGE_SHIFT`
  (src/nvidia/src/kernel/gpu/mem_mgr/arch/ampere/mem_mgr_ga100.c:416, 610 :417).
- Row: `{ id: 0x2080_1348, name: "FB_GET_DYNAMIC_OFFLINED_PAGES", size: 1040, rule: ForwardFiltered { target: Subdevice, filter: OfflinedPages } }`.
- The filter walks `validEntries` entries and keeps those with `B <= pageNumber << 12 < E`,
  compacted to the front. It then rewrites `validEntries` and passes `bMore` on. The page number
  needs no translation, because guest FB address X is host VRAM address X.
- Test: a unit test with a fake host answer of three pages (below `B`, inside, above `E`)
  expecting one entry.

**Worked example 3, F2 process: `NV2080_CTRL_CMD_PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2` 0x20802096.**

- Params: 55888 bytes in both trees: `type`, `bufSize` @4, `count` @8, `tracker`, then
  `samples[72]` @16 of 776 bytes each. Each sample is `base` (timestamp) followed by six
  128-byte engine samples at @8 (fb), @136 (gr), @264 (nvenc), @392 (nvdec), @520 (nvjpg) and
  @648 (nvofa). Each engine sample is `{util @0, vgpuScale @4, procId @8, subProcessID @12,
  subProcessName[100] @16, pOsPidInfo @120}`.
- The guest kernel's `_KERNEL` handler forwards it with the caller's handles
  (kern_perf_ctrl.c:95, send :112, both trees). It then rewrites `procId` for callers outside
  the init PID namespace. So what the device returns is what a guest NVML sees.
- Row: `{ id: 0x2080_2096, name: "PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2", size: 55888, rule: ForwardFiltered { target: Subdevice, filter: GpumonProcess } }`.
- The filter zeroes `procId`, `subProcessID`, `subProcessName` and `pOsPidInfo` in all six
  engine samples of all `count` samples. It also zeroes `util` of the engines the device does
  not present (nvjpg and nvofa on TU106, and nvenc and nvdec while the device has no video
  engines).
- The utilization values themselves include the host desktop's load. That is the P7 timing
  channel, accepted for telemetry under D9.
- Test: a unit test with a fake answer carrying a host PID, which must come back as 0; and
  `nvidia-smi pmon -c 1` in the guest.

## 5. Order of implementation

Client likelihood first. Where the evidence ties, the order goes by what the client does with a
failure.

1. **`NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY` (G, C plan).** Every GL, EGL and Vulkan client
   on the real card sends it: 45 calls in each EGL/GL probe, 135 to 495 in the Vulkan probes
   (for example matrix/traces/610.57.04/control/egl-wayland.jsonl:1468). It comes right after
   `GET_ZBC_CLEAR_TABLE_SIZE`, which category A answers and which is unanswered in every
   display run (for example
   caraxes/runs/display-e-2026-09-24/e14-weston-gl-1080p/device-events.txt:776). This is the
   next wall of the Wayland compositor on EGL.
2. **The 26 F1 rows in one change.** Seven of them sit in the NVML list of
   rmapi_utils.c:198-251, so `nvidia-smi -q` is their likely client: `GPU_GET_ILLUM`,
   `GPU_QUERY_FUNCTION_STATUS`, `GPU_GET_RESET_STATUS`, `GPU_GET_DRAIN_AND_RESET_STATUS`,
   `ECC_GET_VOLATILE_COUNTS`, `ECC_GET_REPAIR_STATUS` and `ECC_GET_UNREPAIRABLE_MEMORY_FLAG`.
   `GPU_QUERY_ILLUM_SUPPORT` was seen in the guest (595-static run, device-events.txt:12).
   The change is mechanical and costs about half a day with the tests.
3. **The NVML-list S controls:**
   - `BIOS_GET_UEFI_SUPPORT` and `BIOS_GET_NBSI_V2` (board data of the synthesized VBIOS);
   - `I2C_GET_PORT_SPEED` (consistent with the synthesized "no ports");
   - `NV90E6_CTRL_CMD_MASTER_GET_ERROR_INTR_OFFSET_MASK` (chip data; NVML's error is silenced
     only below Turing, rmapi_utils.c:245-247, so a Turing NVML expects an answer);
   - `FB_GET_CALIBRATION_LOCK_FAILED` and `BUS_GET_PCIE_LTR_LATENCY` (both F1, done in step 2).
4. **`PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2` (F2).** nvidia-smi `pmon` and NVML process
   utilization. gsp-surface.md §6 lists it as "close behind" the top ten.
5. **The remaining S rows:** `TIMER_GET_GPU_CPU_TIME_CORRELATION_INFO` with a GSP clock,
   `GET_RC_RECOVERY`, `GPU_GET_FIPS_STATUS`, `FB_GET_CTAGS_FOR_CBC_EVICTION`,
   `ECC_GET_SRAM_UNIQUE_UNCORR_COUNTS`, `GR_GFX_POOL_QUERY_SIZE` and the VF error mask. No
   client is known for any of them.
6. **The remaining F2 rows** (`GPU_QUERY_SCRUBBER_STATUS`, `FB_GET_DYNAMIC_OFFLINED_PAGES`,
   `GSP_GET_RM_HEAP_STATS`) and the R rows, with E's family rules.
7. **The other eleven G rows**, with the C plan (RC journal, channel-state lookups, debugger
   defaults).

## 6. Findings beyond the category

- **gsp-surface.md §6 row 2 does not hold.** `TIMER_GET_GPU_CPU_TIME_CORRELATION_INFO` from
  nvidia-modeset never reaches the device. `nvUpdateGpuCpuTimeDiff` asks for
  `_PROCESSOR_CPU` (src/nvidia-modeset/src/nvkms-evo.c:8821-8837, 610 :8864-8880). The kernel
  answers CPU-clock requests itself and sends only `_PROCESSOR_GSP` requests to GSP
  (subdevice_ctrl_timer_kernel.c:318-343, both trees). The native NVML trace uses cpuClkId
  0x01 (OSTIME, CPU; matrix/traces/610.57.04/control/nvml.jsonl:105). The Wayland presentation
  clock depends on the guest kernel's own timer reads, not on this control.
  Status: gsp-surface.md §6 row 2 corrected on 2026-09-24; the device answers a GSP-clock request from its own clock (§8).
- **Two rows forwarded today return host FB addresses.** `FB_GET_OFFLINED_PAGES` 0x20801322
  returns `pageAddressWithEccOn/Off` per entry (ctrl2080fb.h:996 in 610, :999 in 615;
  params :1080 / :1083). `FB_GET_REMAPPED_ROWS` 0x20801344 is the same kind of data. Both are
  in `FORWARD` (ver/v610.rs:921-930) without a reservation filter. On a card with retired
  pages, the guest would see host addresses outside its reservation. The F2 `OfflinedPages`
  filter applies to them as well: it is the D9 cut-down item for this pair.
  Status: done in feff894. Both rows are filtered forwards (§8).
- **Heuristic corrections for `classify()`** in caraxes/tools/gsp-surface.py:
  - A control with a handle-carrying params struct (`hChannel`, `hClient`) or on
    ZbcApi/KernelSMDebuggerSession is C, not B.
  - A control on ProfilerBase is D rule 4.
  - A control whose only kernel send is under `IS_VGPU_GSP_PLUGIN_OFFLOAD_ENABLED`, an SR-IOV
    VF function or `gpuIsSelfHosted` does not reach the device here.

  The "Route" column lists these guards control by control.

## 7. Open questions for category B, and the measurement that settles each

Every measurement that touches the host GPU needs the operator's go-ahead (host GPU state is
off limits otherwise), and every host read must be reported.

- **Q-B1. Which F1/F2 controls does the host card answer, and with what?** Source cannot tell
  which of the ECC, row-remapper, DRAM-encryption, scrubber and offlined-page getters a GeForce
  TU106 under 615.71.09 answers with NOT_SUPPORTED. Measurement: one read-only sweep with an
  unprivileged client (extend caraxes/tools/lab-rm-query.rs, which already allocates root,
  device and subdevice and issues only getters). Issue each of the 30 F1/F2 ids once with a
  zeroed buffer of the size in the table, and record status and output bytes. The result is
  kept as chip data next to the host facts. It also decides whether the scrubber and offlined
  filters ever see an address.
  Status: answered 2026-09-24 by a read-only sweep (§8, caraxes/runs/gsp-bc-2026-09-24/qb1-sweep).
- **Q-B2. Which NVML-list controls does nvidia-smi really send?** The rmapi_utils.c list is
  indirect evidence. Measurement, with no host touch beyond the usual gate: in the 615 guest,
  `nvidia-smi -q`, `nvidia-smi -q -d ECC,PAGE_RETIREMENT,ROW_REMAPPER` and `nvidia-smi pmon -c 1`,
  with the device logging every control (not only the unanswered ones). Then compare with
  matrix/traces/610.57.04/control/nvml.jsonl, which has none of the list's ids.
- **Q-B3. The ZBC table: defaults against desktop entries.** Source does not show which ZBC
  entries GSP-RM installs at init; the defaults are set in firmware. Measurement on the host,
  read-only: `GET_ZBC_CLEAR_TABLE_SIZE` and every `GET_ZBC_CLEAR_TABLE_ENTRY` index on a
  GF100_ZBC_CLEAR object of a fresh client, once from the console before the desktop starts and
  once with the desktop running. Entries present in both runs are the chip defaults for the
  device's table. The rest belong to host clients and must not reach the guest.
- **Q-B4. GPUMON attribution of mirrored work.** When a guest channel runs on a host mirror,
  does the host attribute its utilization to the device process's PID? If so, the filter could
  map that PID to the guest process that owns the channel instead of zeroing it. That needs a
  registry of guest PIDs, which the device does not have. Measurement: host `nvidia-smi pmon`
  while the cuda gate runs in the guest.
- **Q-B5. VBIOS-derived answers.** Does the synthesized VBIOS ([vbios-synthesis.md](../vbios-synthesis.md))
  carry the host's UEFI image and NBSI? Measurement: compare the host's
  `BIOS_GET_UEFI_SUPPORT` and `BIOS_GET_NBSI_V2` answers (Q-B1 sweep) with a parse of the
  synthesized image. If they match, the two controls can be F1.
  Status: open. `BIOS_GET_UEFI_SUPPORT` answers from the board data with version 0 (§8); the comparison was outside this package's host reads.
- **Q-B6. GSP heap statistics.** Do the host GSP's heap numbers belong in the guest at all? The
  guest's GSP is the device. This is an operator decision under D9 (generous first, cut
  later). Measurement for the decision: the Q-B1 answer for `gfid` 0.
  Status: decided 2026-09-24. The guest gets the device's own heap view, and the host's numbers are not forwarded (§8).
- **Q-B7. Does any guest client ask for the GSP clock?** No known client does. Measurement: the
  device logs `cpuClkId` of every `TIMER_GET_GPU_CPU_TIME_CORRELATION_INFO` it receives in the
  display and cuda gates. No host touch.

## 8. Status (2026-09-24)

Implemented in `crates/vfio-user-nvgpu` in four commits:

- feff894: the isolation fix of §6;
- c6b98ee: F1, F2 and the guard;
- d1ce87f: S, R and N;
- b49cc5c: the two RC rows of G, with the C plan.

The rows that are not plain forwards live in one table, `PUBLIC` in
`src/ver/v610/public.rs`, shared by the 615 module (every struct has the same layout in both
trees). The handler is `src/public.rs`, which runs in `Dispatcher::control` before nvml.rs.

**Counts.**

| Group | Rows | Done | Partial | Open |
|---|---:|---:|---:|---:|
| F1 | 26 | 26 | 0 | 0 |
| F2 | 4 | 4 | 0 | 0 |
| S | 11 | 10 | 1 | 0 |
| G | 12 | 11 | 0 | 1 |
| R | 13 | 13 | 0 | 0 |
| N | 4 | 4 | 0 | 0 |
| **all** | **70** | **68** | **1** | **1** |

Round 2 (2026-09-25, this file's rows 5, 22, 23, 25, 55 and 58-61; details in
[c-guest-state.md](c-guest-state.md) §8, "Round 2"): nine of the ten open G rows are rows of
`PUBLIC_C`. Row 62, the ZBC table entry, belongs to the ZBC model of the D plan (R3) and
stays open here.

**What each group became.**

- **F1.** The 26 rows are in `FORWARD` (ver/v610.rs), including `GPU_QUERY_ILLUM_SUPPORT` and
  `GPU_GET_ILLUM`, the rows seen in the run logs. A test pins that their host status and bytes
  reach the guest unchanged.
- **F2.** Three rows are filtered forwards: the scrubber range, the dynamic offlined pages and
  the GPUMON process fields.
  - The GPUMON filter works on all 72 ring slots, not only the first `count`, because
    `tracker` can point anywhere.
  - It also zeroes the utilization of nvjpg and nvofa, which TU106 does not have. nvenc and
    nvdec stay, because the device presents them (chips/tu106/device-info.toml).
- **S.** Five rows are rows of `chips/tu106/controls.toml`: FIPS status, UEFI support, the two
  NV90E6 masks, and `GR_GFX_POOL_QUERY_SIZE`, refused.
  - chipctl.rs accepts them only with `moved_from = "B"`, which must match the inventory's
    category.
  - The other six are device handlers in public.rs.
  - UEFI support is partial. The device answers PRESENCE_YES, because the synthesized VBIOS
    keeps the captured EFI image. It answers not running, and version 0, because the device
    does not parse the version from the image. Q-B5 is open.
- **R and N.** NOT_SUPPORTED, request echoed, with one of the labels `refused (host scheduler
  log ...)`, `(raw PHY register read)`, `(eye measurement ...)`, `(diag class ...)`,
  `(profiler ...)`, `(MIG absent ...)`, `(Tegra only)`, `(no dielets ...)` or `(host accounting
  ...)`.
- **The two forwards of §6.** `FB_GET_OFFLINED_PAGES` and `FB_GET_REMAPPED_ROWS` left
  `FORWARD` and are filtered forwards.
  - Offlined pages keep only the entries whose pages lie in the reservation. Guest FB address
    X is host VRAM address X, so a kept page is already the guest's FB offset. The pending
    flags stay only for a kept pending page.
  - Remapped rows name DRAM rows, not FB addresses. So the list is emptied and the card-level
    flags stay.
  - Without a reservation, both lists are empty.
  - The test `offlined_pages_outside_the_reservation_never_reach_the_guest` (public.rs)
    checks every address of all 64 slots.

**The guard for hostrm.rs:84.**

- `HostRmClient::control` now refuses a class other than subdevice, device or BINAPI. It no
  longer issues the control on the host root client (`hostrm::host_object`).
- `FORWARD` and `Dispatcher::forward_or` pick the host object by the control's interface
  class (`hostrm::class_of_control`), not by the guest object's class.
- The test `every_forward_targets_subdevice_device_or_binapi` (public.rs) drives every
  `FORWARD` row and every filtered row through the dispatcher, on a subdevice, a diag object
  and a ZBC object. It checks that the host saw exactly the object the id addresses.

**Decision under D9 (Q-B6), taken 2026-09-24 by the operator.** `GSP_GET_RM_HEAP_STATS` is
not forwarded. The guest's GSP is the device, and the guest gets the device's own heap view:
the emulated GSP has no RM heap, so all numbers are 0 with NV_OK. The host GSP's heap numbers,
which the Q-B1 sweep shows the host would give, never reach the guest. The plan's F2 recipe
(pin `gfid` to 0, then forward) is dropped. `GSP_GET_LIBOS_HEAP_STATS` stays F1 as planned;
the host answers it NOT_SUPPORTED (Q-B1). Should it ever answer, the same reasoning applies
and it should move to the device.

**Q-B1, answered (read-only sweep, 2026-09-24 19:08).**

- Method: `cargo run --release -p vfio-user-nvgpu --example qb1-sweep`, on the subdevice of
  the device's own unprivileged host RM client, host driver 615.71.09. Each of the 30 F1/F2
  getters was issued once, with a zeroed buffer of the struct's size. No other control and
  no other object were used.
- Record: caraxes/runs/gsp-bc-2026-09-24/qb1-sweep/sweep.tsv (every control with its status).
  No kernel log line and no Xid in the window.
- NV_OK (15): `GPU_QUERY_ILLUM_SUPPORT` (all 0), `GPU_QUERY_SCRUBBER_STATUS`,
  `GPU_QUERY_FUNCTION_STATUS`, `GPU_GET_RESET_STATUS`, `FB_GET_CALIBRATION_LOCK_FAILED`,
  `FB_GET_LTC_INFO_FOR_FBP`, `FB_GET_DYNAMIC_OFFLINED_PAGES`, `BUS_GET_PEX_LANE_COUNTERS`,
  `BUS_GET_PCIE_LTR_LATENCY`, `PERF_GET_LEVEL_INFO_V2`, `PERF_GET_POWERSTATE`,
  `PERF_RATED_TDP_GET_CONTROL`, `ECC_GET_REPAIR_STATUS`, `GPU_IS_RESET_COUPLED` and
  `GSP_GET_RM_HEAP_STATS`.
- NOT_SUPPORTED 0x56 (11): the ECC getters (`QUERY_ECC_CONFIGURATION`,
  `CLIENT_EXPOSED_COUNTERS`, `VOLATILE_COUNTS`, `UNREPAIRABLE_MEMORY_FLAG`), `GPU_GET_ILLUM`,
  `GPU_GET_DRAIN_AND_RESET_STATUS`, `CLK_PMUMON_CLK_DOMAINS_GET_SAMPLES`,
  `FB_GET_ROW_REMAPPER_HISTOGRAM`, both DRAM encryption queries and
  `GSP_GET_LIBOS_HEAP_STATS`.
- INVALID_ARGUMENT 0x1f (3), because a zeroed request is not a valid one:
  `FIFO_OBJSCHED_GET_STATE`, `PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2` and
  `FLCN_GET_DMEM_USAGE`.
- NV_ERR_GENERIC 0xffff (1): `BIOS_GET_POST_TIME`.
- What the filters see:
  - The scrubber reports status 1 over [0x10000000, 0x1effff000). That range covers far more
    than any reservation, so the clamp is needed on this card.
  - The dynamic offlined list is empty, so its filter has nothing to cut here.

**Verification.**

- Unit tests: `cargo test -p vfio-user-nvgpu`, default and `--features v615`.
- Clippy as tools/check.sh runs it.
- The GPU gates on vm5 could not run. The host RM refused the reservation at slot 6
  ([0x29800000, 0x69800000), NV_ERR_NO_MEMORY; the largest free region was 2052 to 3072 MiB
  higher up while the other VMs ran), and the device failed closed. Both phases were run in
  pure emulation instead (caraxes/runs/gsp-bc-2026-09-24, `compare-emu.md`). The nvml and
  cuda rows are identical before and after, with no new refusal and no changed answer. In
  pure emulation neither nvidia-smi nor cuInit reaches a B or C row, so this shows the absence
  of regressions only.

**Per row.**

| # | Control | Group | Status | Commit | Note |
|---|---|---|---|---|---|
| 1 | `NV0000_GPUACCT_GET_ACCOUNTING_STATE` | N | done | d1ce87f | refused with a label |
| 2 | `NV0000_GPUACCT_GET_PROC_ACCOUNTING_INFO` | N | done | d1ce87f | refused with a label |
| 3 | `NV0000_GPUACCT_GET_ACCOUNTING_PIDS` | N | done | d1ce87f | refused with a label |
| 4 | `NV0000_GPUACCT_GET_PROC_ACCOUNTING_INFO_V2` | N | done | d1ce87f | refused with a label |
| 5 | `NV0090_GET_MMU_DEBUG_MODE` | G | done | 8989bbc | C plan B2: `bMode` 0 from device state (the device never enables MMU debug; the SM debugger setters are refused); not forwarded |
| 6 | `NV2080_GPU_QUERY_ECC_CONFIGURATION` | F1 | done | c6b98ee | FORWARD row, host 0x56 on a zeroed request (Q-B1) |
| 7 | `NV2080_GPU_QUERY_ILLUM_SUPPORT` | F1 | done | c6b98ee | FORWARD row, host 0x0 on a zeroed request (Q-B1) |
| 8 | `NV2080_GPU_GET_ILLUM` | F1 | done | c6b98ee | FORWARD row, host 0x56 on a zeroed request (Q-B1) |
| 9 | `NV2080_GPU_QUERY_SCRUBBER_STATUS` | F2 | done | c6b98ee | filtered: scrub range clamped to the reservation (Q-B1: the host reports status 1 over [0x10000000, 0x1effff000)) |
| 10 | `NV2080_GPU_QUERY_FUNCTION_STATUS` | F1 | done | c6b98ee | FORWARD row, host 0x0 on a zeroed request (Q-B1) |
| 11 | `NV2080_GPU_GET_PARTITION_CAPACITY` | R | done | d1ce87f | refused with a label |
| 12 | `NV2080_GPU_GET_RESET_STATUS` | F1 | done | c6b98ee | FORWARD row, host 0x0 on a zeroed request (Q-B1) |
| 13 | `NV2080_GPU_GET_DRAIN_AND_RESET_STATUS` | F1 | done | c6b98ee | FORWARD row, host 0x56 on a zeroed request (Q-B1) |
| 14 | `NV2080_GPU_GET_FIPS_STATUS` | S | done | d1ce87f | controls.toml, chip: bFipsEnabled 0 |
| 15 | `NV2080_TIMER_GET_GPU_CPU_TIME_CORRELATION_INFO` | S | done | d1ce87f | device clock for every sample (§6: no known client asks for the GSP clock) |
| 16 | `NV2080_BIOS_GET_POST_TIME` | F1 | done | c6b98ee | FORWARD row, host 0xffff on a zeroed request (Q-B1) |
| 17 | `NV2080_BIOS_GET_UEFI_SUPPORT` | S | partial | d1ce87f | controls.toml, board: PRESENCE_YES, not running; version 0 until Q-B5 compares it with the image |
| 18 | `NV2080_BIOS_GET_NBSI_V2` | S | done | d1ce87f | NV_OK, errorCode NOT_FOUND, retSize 0 |
| 19 | `NV2080_CLK_PMUMON_CLK_DOMAINS_GET_SAMPLES` | F1 | done | c6b98ee | FORWARD row, host 0x56 on a zeroed request (Q-B1) |
| 20 | `NV2080_FIFO_OBJSCHED_SW_GET_LOG` | R | done | d1ce87f | refused with a label |
| 21 | `NV2080_FIFO_OBJSCHED_GET_STATE` | F1 | done | c6b98ee | FORWARD row, host 0x1f on a zeroed request (Q-B1) |
| 22 | `NV2080_GR_GET_CTXSW_STATS` | G | done | 8989bbc | C plan B3: mirrored channel: hChannel rewritten to its own host channel on the mirror's subdevice; software channel: zeros |
| 23 | `NV2080_GR_GET_CURRENT_RESIDENT_CHANNEL` | G | done | 8989bbc | C plan B5: the host's resident ChID (a host read on the mirror's subdevice) mapped through the mirrors' work-submit tokens; a guest ChID only for this VM's own mirror, else 0xFFFFFFFF |
| 24 | `NV2080_GR_GFX_POOL_QUERY_SIZE` | S | done | d1ce87f | controls.toml, refused (no caller) |
| 25 | `NV2080_GR_GET_CTXSW_MODES` | G | done | 8989bbc | C plan B4: mirrored: the own mirror's modes; software: the recorded zcull and preemption modes, PM and SMPC off |
| 26 | `NV2080_FB_GET_CALIBRATION_LOCK_FAILED` | F1 | done | c6b98ee | FORWARD row, host 0x0 on a zeroed request (Q-B1) |
| 27 | `NV2080_FB_GET_LTC_INFO_FOR_FBP` | F1 | done | c6b98ee | FORWARD row, host 0x0 on a zeroed request (Q-B1) |
| 28 | `NV2080_FB_GET_CTAGS_FOR_CBC_EVICTION` | S | done | d1ce87f | numCompTags 0 (D16) |
| 29 | `NV2080_FB_GET_ROW_REMAPPER_HISTOGRAM` | F1 | done | c6b98ee | FORWARD row, host 0x56 on a zeroed request (Q-B1) |
| 30 | `NV2080_FB_GET_DYNAMIC_OFFLINED_PAGES` | F2 | done | c6b98ee | filtered: entries inside the reservation (Q-B1: 0 entries on this card) |
| 31 | `NV2080_FB_QUERY_DRAM_ENCRYPTION_PENDING_CONFIGURATION` | F1 | done | c6b98ee | FORWARD row, host 0x56 on a zeroed request (Q-B1) |
| 32 | `NV2080_FB_QUERY_DRAM_ENCRYPTION_STATUS` | F1 | done | c6b98ee | FORWARD row, host 0x56 on a zeroed request (Q-B1) |
| 33 | `NV2080_BUS_GET_PEX_LANE_COUNTERS` | F1 | done | c6b98ee | FORWARD row, host 0x0 on a zeroed request (Q-B1) |
| 34 | `NV2080_BUS_GET_PCIE_LTR_LATENCY` | F1 | done | c6b98ee | FORWARD row, host 0x0 on a zeroed request (Q-B1) |
| 35 | `NV2080_BUS_GET_UPHY_DLN_CFG_SPACE` | R | done | d1ce87f | refused with a label |
| 36 | `NV2080_BUS_GET_EOM_STATUS` | R | done | d1ce87f | refused with a label |
| 37 | `NV2080_PERF_GET_LEVEL_INFO_V2` | F1 | done | c6b98ee | FORWARD row, host 0x0 on a zeroed request (Q-B1) |
| 38 | `NV2080_PERF_GET_POWERSTATE` | F1 | done | c6b98ee | FORWARD row, host 0x0 on a zeroed request (Q-B1) |
| 39 | `NV2080_PERF_GET_TEGRA_PERFMON_SAMPLE` | R | done | d1ce87f | refused with a label |
| 40 | `NV2080_PERF_RATED_TDP_GET_CONTROL` | F1 | done | c6b98ee | FORWARD row, host 0x0 on a zeroed request (Q-B1) |
| 41 | `NV2080_PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2` | F2 | done | c6b98ee | filtered: process fields zeroed in all 72 slots, nvjpg/nvofa utilization zeroed (Q-B1: host 0x1f for a zeroed request) |
| 42 | `NV2080_RC_GET_ERROR_COUNT` | G | done | b49cc5c | C plan B6: the device's RC journal, count 0 |
| 43 | `NV2080_GET_RC_RECOVERY` | S | done | d1ce87f | rcEnable ENABLED |
| 44 | `NV2080_RC_GET_ERROR_V2` | G | done | b49cc5c | C plan B7: no record, outputRecordSize 0 |
| 45 | `NV2080_FLCN_GET_DMEM_USAGE` | F1 | done | c6b98ee | FORWARD row, host 0x1f on a zeroed request (Q-B1) |
| 46 | `NV2080_ECC_GET_CLIENT_EXPOSED_COUNTERS` | F1 | done | c6b98ee | FORWARD row, host 0x56 on a zeroed request (Q-B1) |
| 47 | `NV2080_ECC_GET_VOLATILE_COUNTS` | F1 | done | c6b98ee | FORWARD row, host 0x56 on a zeroed request (Q-B1) |
| 48 | `NV2080_ECC_GET_SRAM_UNIQUE_UNCORR_COUNTS` | S | done | d1ce87f | NOT_SUPPORTED: no ECC on this board |
| 49 | `NV2080_ECC_GET_REPAIR_STATUS` | F1 | done | c6b98ee | FORWARD row, host 0x0 on a zeroed request (Q-B1) |
| 50 | `NV2080_ECC_GET_UNREPAIRABLE_MEMORY_FLAG` | F1 | done | c6b98ee | FORWARD row, host 0x56 on a zeroed request (Q-B1) |
| 51 | `NV2080_GSP_GET_RM_HEAP_STATS` | F2 | done | c6b98ee | device answer, not forwarded: all 0 (decision D9/Q-B6, §8) |
| 52 | `NV2080_GSP_GET_LIBOS_HEAP_STATS` | F1 | done | c6b98ee | FORWARD row, host 0x56 on a zeroed request (Q-B1) |
| 53 | `NV2080_GPU_IS_RESET_COUPLED` | F1 | done | c6b98ee | FORWARD row, host 0x0 on a zeroed request (Q-B1) |
| 54 | `NV2080_GPU_GET_UNIT_FS_INFO_FROM_DIELET` | R | done | d1ce87f | refused with a label |
| 55 | `NV208F_FIFO_GET_CHANNEL_STATE` | G | done | 8989bbc | C plan B8: `bEnabled`/`bScheduled` from the device's scheduling flags (DISABLE_CHANNELS, DISABLE_USERMODE_CHANNELS, the toggle), `bDeferRC` 0 |
| 56 | `NV208F_GPU_GET_RAM_SVOP_VALUES` | R | done | d1ce87f | refused with a label |
| 57 | `NV402C_I2C_GET_PORT_SPEED` | S | done | d1ce87f | zeros: no I2C ports |
| 58 | `NV83DE_DEBUG_GET_MODE_MMU_DEBUG` | G | done | 8989bbc | refused NOT_SUPPORTED: no debugger session model (its setter `SET_MODE_MMU_DEBUG` is the open MODEL row of D R5) |
| 59 | `NV83DE_DEBUG_GET_MODE_ERRBAR_DEBUG` | G | done | 8989bbc | DISABLED: its setter is refused (D REFUSE), so no session has it on |
| 60 | `NV83DE_DEBUG_GET_SINGLE_SM_DEBUGGER_STATUS` | G | done | 8989bbc | refused NOT_SUPPORTED: per-SM run-control state needs a session with SM debug mode (refused, D 6.16) |
| 61 | `NV83DE_DEBUG_GET_MODE_MMU_GCC_DEBUG` | G | done | 8989bbc | refused NOT_SUPPORTED: Blackwell onwards, absent on TU106 (its setter is in the `blackwell` family) |
| 62 | `NV9096_GET_ZBC_CLEAR_TABLE_ENTRY` | G | open | - | C plan B1: a device ZBC table; its defaults need Q-B3 (not permitted in this package) |
| 63 | `NV90E6_MASTER_GET_ERROR_INTR_OFFSET_MASK` | S | done | d1ce87f | controls.toml, chip: masks 0 (no ECC, no NVLink), offset 0 |
| 64 | `NV90E6_MASTER_GET_VIRTUAL_FUNCTION_ERROR_CONT_INTR_MASK` | S | done | d1ce87f | controls.toml, chip: both masks 0 |
| 65 | `NVB0CC_PMA_STREAM_UPDATE_GET_PUT` | R | done | d1ce87f | refused with a label |
| 66 | `NVB0CC_GET_TOTAL_HS_CREDITS` | R | done | d1ce87f | refused with a label |
| 67 | `NVB0CC_GET_HS_CREDITS` | R | done | d1ce87f | refused with a label |
| 68 | `NVB0CC_GET_CHIPLET_HS_CREDIT_POOL` | R | done | d1ce87f | refused with a label |
| 69 | `NVB0CC_GET_HS_CREDITS_MAPPING` | R | done | d1ce87f | refused with a label |
| 70 | `NVB0CC_GET_DYNAMIC_MMA_BOOST_STATUS` | R | done | d1ce87f | refused with a label |
