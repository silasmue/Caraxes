<!-- SPDX-License-Identifier: MIT -->
<!-- Source survey of 2026-09-24 against the open-gpu-kernel-modules trees 615.71.09 and 610.57.04 and the device crate (read only); payload sizes compiled with the flag set of caraxes/tools/gsp-abi-sizes.sh. -->
# Events of 615.71.09: what GSP-RM tells the kernel RM, and what the device must raise

Part of the GSP plan ([README.md](README.md)); the functions of the other direction are in
[rpc-functions.md](rpc-functions.md), the boot order (including `GSP_RESUME_DONE`) in
[boot-exchange.md](boot-exchange.md).

Target driver 615.71.09 (guest and host), 610.57.04
noted where it differs. Tree paths are relative to the open-gpu-kernel-modules root; "610:" marks a
610.57.04 location. Device paths are `crates/vfio-user-nvgpu/src/...`.

## Status (implementation of 2026-09-24)

Commits on the implementation branch; the table of what is raised and from which trigger is
the module comment of `crates/vfio-user-nvgpu/src/events.rs`. Run records:
`caraxes/runs/gsp-rpc-2026-09-24/`.

- Status GMCAPI request send path (the transport finding): **done**, 4bacf2a
  (`QueueStateV2::send_gmcapi_request`: id without the response flag, sequence 0).
- Status `rpc_rc_triggered_v17_02` per layout (64 bytes on 615, 48 on 610): **done**, 4bacf2a
  (`events::rc_layout` by the transport's guest ABI).
- Status R1 `RC_TRIGGERED` and R2 `OS_ERROR_LOG`: **partial**, 4bacf2a. Raised from the error
  notifier of a guest channel on a host mirror: the host RM writes it because the mirror
  channel's `hObjectError` is the guest's own page, so only this device's channels can raise
  it; one RC per channel lifetime; `RmLogonRC=0` drops the Xid line (fc9b537). The host-global
  subdevice RC subscription (`mirror.rs`, `NV2080_NOTIFIERS_RC_ERROR`) is only a wake-up for
  that scan, never a source. Not yet: the software executor's own faults, M1's per-channel
  host event.
- Status R3 `MMU_FAULT_QUEUED` (GMCAPI 0x40001): **not raised, per source (round 2,
  2026-09-25)**. The send path exists; the event announces a packet in UVM's shadow buffer,
  which only a fault of a UVM-owned channel produces. Software executor: raises no faults
  (channel.rs), so no packet. Per-VM mirror faults: the mirrored channels are not UVM's (UVM's
  own channels carry GP100_UVM_SW and stay in software, mirror.rs `eligible`), so a mirror
  fault is an RC (R1, Xid 31), and its details come from the host's record of the channel's own
  mirror (`NV906F_CTRL_CMD_GET_MMU_FAULT_INFO` on the device's own host channel,
  `Mirror::mmu_fault_info`), which now feeds the guest's `GET_MMU_FAULT_INFO` and the RC
  journal. Host fault buffer: host-global, stays out.
- Status operational-event records (615 import, round 2): **done**. For every RC the device
  raises (R1) it writes a GOE record (`GPU-RC`, engine error or translation fault for Xid 31,
  recoverable, one legacy-Xid context with the Xid and its text) into the import's registered
  guest buffer (`gspmem.rs` `GspMemory::produce`: vardata, record, then `recordPut`) and wakes
  the kernel with `POST_EVENT` on the import's callback event (the bind's handle), once until
  the drain's `UPDATE_GET`. `CARAXES_GOE_SELFTEST=<s>` adds a SIMULATED debug record every `s`
  seconds for the guest probe `caraxes/tools/goe-probe.sh`.
- Status R4 `PMU_HALTED`: **open** (no lost-GPU decision in the device).
- Status R5 `RECOVERY_ACTION`, R10 `GSP_POST_NOCAT_RECORD`, R12 `UPDATE_GSP_TRACE`, R14: **done
  by decision**: never sent.
- Status R6 general `POST_EVENT` registry: **open** (hotplug only, as before).
- Status R7 `SEMAPHORE_SCHEDULE_CALLBACK`, R8 `TIMED_SEMAPHORE_RELEASE`, R9 `FECS_ERROR`, R11
  `GPUACCT_PERFMON_UTIL_SAMPLES`: **open**, each needs a model the device does not have
  (0x9072 methods at vblank, 0x9074 methods on a device timer, a FECS trace, per-context busy
  accounting).
- Status R13 `GSP_RESUME_DONE` (GMCAPI 0x10005): **done** on the device side, fc9b537
  (unit-tested; guest suspend in the VMM is untested, boot-exchange.md open question 4).
- Status SWGEN0 without the display: **done**, 4bacf2a (GSP IRQSTAT/MASK/DEST in the boot
  model, the GSP stall vector raised through the interrupt tree).

## Method

The event list is the `E(...)` block of `src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:255-301`
(610: `:253-295`). The kernel handlers were read in `src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c`: the
switch of `_kgspProcessRpcEvent` (`:1556`, cases `:1597-1739`, default "Unexpected RPC event" `:1741-1751`;
610: `:1399`, cases `:1443-1587`) and every handler it calls. The GMCAPI channel that 615 added was
read from `src/nvidia/interface/gmcapi/gmcapi_table.h` and the drain loop
(`kernel_gsp.c:1989-2020`). Payload sizes come from a C program compiled against each tree's
`g_rpc-structures.h` with the include and define set of `caraxes/tools/gsp-abi-sizes.sh`
(`minimal_flags`, plus `-DPORT_MODULE_time=1 -DGSP_ABI=615` for 615 and `-DGSP_ABI=610` for 610) and
the include order of `gsp-abi-sizes.c`; the struct text of each versioned name was compared between
the trees. The device was checked by grep of `crates/vfio-user-nvgpu/src` at the base commit of this plan (61e0b6a). What GSP-RM does on its side is not in the open tree; conditions
marked "inferred" come from the kernel handler, its comments, or the payload.

## Summary

- Events in 615: 45 (0x1001 to 0x102D); 610 has 41 (0x1001 to 0x1029). The four new ones
  (ECC_POSSIBLE_ERR_FAST_PATH 0x102A, NVLINK_TRAFFIC_QUIESCE_NOTIFY 0x102B, GPU_GFM_STATE_CHANGE
  0x102C, NVLINK_ABM_FABRIC_HEALTH_MASK_UPDATE 0x102D) are all dispatched by 615.
- Dispatched by the 615 kernel: 36. Not dispatched (9): GSP_INIT_DONE, GSP_RUN_CPU_SEQUENCER,
  MMU_FAULT_QUEUED, RG_LINE_INTR, VGPU_GSP_PLUGIN_TRIGGERED, DISPLAY_MODESET, BIND_BAR2,
  GSP_LOAD_EXEC_GENERIC_BOOTLOADER, GSP_LOAD_EXEC_HS_BINARY. Five of them moved to GMCAPI or went
  away: init done became the answer to `GMCAPI_CMD_GSP_INIT` (`kernel_gsp.c:5328`), MMU_FAULT_QUEUED is GMCAPI
  0x40001, the plugin trigger is GMCAPI 0x20024, the two loader requests are GMCAPI 0x10002 and
  0x10003. RG_LINE_INTR and DISPLAY_MODESET are handled only by the Tegra DCE client
  (`src/nvidia/src/kernel/gpu/dce_client/dce_client_rpc.c:359-382`); GSP_RUN_CPU_SEQUENCER and
  BIND_BAR2 have no handler in either tree.
- Needed by a stock Linux client on a desktop TU106 GeForce (615): always 1 (POST_EVENT); when a
  feature is used 5 (SEMAPHORE_SCHEDULE_CALLBACK, TIMED_SEMAPHORE_RELEASE,
  GPUACCT_PERFMON_UTIL_SAMPLES, FECS_ERROR, UPDATE_GSP_TRACE); on error 5 (RC_TRIGGERED, OS_ERROR_LOG,
  RECOVERY_ACTION, PMU_HALTED, and GSP_POST_NOCAT_RECORD as optional); never 34. On 610,
  GSP_INIT_DONE is needed at every boot and MMU_FAULT_QUEUED on a non-replayable fault.
- GMCAPI GSP-to-CPU commands (615): 6; 2 matter on TU106 Linux (MMU_FAULT_QUEUED 0x40001 on a UVM
  fault, GSP_RESUME_DONE 0x10005 on resume); EXEC_GENERIC_BOOTLOADER 0x10002 has a Turing
  handler but is not expected at run time (inferred, see below).
- Sent by the device today: POST_EVENT (both transports, display hotplug only) and GSP_INIT_DONE
  (610 transport only). The 615 path sends one event (POST_EVENT) and no GMCAPI request.
- Gaps on 615: 10 events a TU106 client can need (5 on use, 5 on error) plus GMCAPI
  MMU_FAULT_QUEUED and GSP_RESUME_DONE. Errors in host-mirrored channels reach the guest today only
  as the host RM's write into the guest's own error-notifier page (`mirror.rs:778-791`,
  `:1405-1431`; `execution.md:115-118`): the guest kernel never hears of the RC, prints no Xid and
  does no RC bookkeeping.
- ABI finding: `rpc_rc_triggered_v17_02` keeps its version name but grew from 48 to 64 bytes in 615
  (four new fields before `rcJournalBuffer`, below). Every other event struct has the same text and
  size in both trees.

## The table

Size column: sizeof in 610 / 615 and the struct's line in 615 (610); "flex" marks a trailing
flexible array (the size is the fixed part). Needed column: always / on use / on error / never.
Isolation risk refers to a host-mirrored implementation.

| Id | Name | Payload (610/615 size, change) | Kernel handler 615 (610) | Raised when | Needed on TU106 Linux | Device today | Recipe, truth source, isolation |
|---|---|---|---|---|---|---|---|
| 0x1001 | GSP_INIT_DONE | `rpc_init_done_v17_00` 1/1, same; g_rpc-structures.h:1622 (610 :1589) | 615: not dispatched; init completes with the GMCAPI_CMD_GSP_INIT response (`kernel_gsp.c:5328`). 610: waited for in `_kgspRpcRecvPoll` (610: `kernel_gsp.c:2872-2877`, `:6282`), listed "handled by _kgspRpcRecvPoll" (610: `:1587`) | GSP-RM finished its init | 610: always at boot; 615: never | 610: sent (`boot.rs:1293-1300`); 615: not sent (`boot.rs:1451-1454`, `:1480`) | done; pure emulation; no risk |
| 0x1002 | GSP_RUN_CPU_SEQUENCER | none in either tree | not dispatched, no handler in either tree | older drivers' CPU sequencer (inferred from the name) | never | never | none |
| 0x1003 | POST_EVENT | `rpc_post_event_v17_00` 32/32 + flex `eventData`, same; :1495 (:1466) | `_kgspRpcPostEvent` `:632` (610 `:500`): finds the EventApi by hClient/hEvent, ECC_DBE page offlining (`:659`), then `gpuNotifySubDeviceEvent` or `osEventNotificationWithInfo` (bNotifyList) or `osNotifyEvent` on the one hEvent | a notifier of a GSP-owned object fires for an event the client allocated (`kernel_gsp.c:623-630` comment) | always (nvkms registers HOTPLUG and DP_IRQ, `nvidia-modeset/src/nvkms-rm.c:2064-2103`) | sent for HOTPLUG only (`boot.rs:264-298`, payload `display/mod.rs:1440-1503`) | general event registry, see recipe R6; truth: the device models (display today); low |
| 0x1004 | RC_TRIGGERED | `rpc_rc_triggered_v17_02` 48/64 + flex journal, CHANGED (615 adds `rcChannelInfoType`, `instblkAddrLo/Hi`, `instblkAperture`); :1510 (:1481) | `_kgspRpcRCTriggered` `:717` (610 `:585`) | channel teardown done in GSP-RM after a channel error (`:710-715` comment) | on error | never | R1; truth: host channel RC event + guest notifier page (mirror), executor (software); medium |
| 0x1005 | MMU_FAULT_QUEUED | none (payload ignored) | 615: not an event; GMCAPI 0x40001 `gmcapiMmuFaultQueued` `:1178`. 610: `_kgspRpcMMUFaultQueued` 610 `:1002` | GSP-RM copied a non-replayable fault into the client shadow buffer (`:1170-1177` comment) | 610: on error with UVM; 615: never as event | never (`uvm.rs:185-190` "MMU_FAULT_QUEUED is never sent") | R3 |
| 0x1006 | OS_ERROR_LOG | `rpc_os_error_log_v17_00` 272/272, same; :1533 (:1500) | `_kgspRpcOsErrorLog` `:849` (610 `:724`): `nvErrorLog2_va` (`:883`), Xid line in dmesg (`src/nvidia/src/kernel/gpu/rc/kernel_rc.c:388-400`) | any Xid GSP-RM raises | on error | never | R2; truth as R1; medium |
| 0x1007 | RG_LINE_INTR | `rpc_rg_line_intr_v17_00` 8/8, same; :1544 (:1511) | not dispatched; DCE only (`dce_client_rpc.c:359-367`) | Tegra display | never | never | none |
| 0x1008 | GPUACCT_PERFMON_UTIL_SAMPLES | `rpc_gpuacct_perfmon_util_samples_v1F_0E` 4048/4048, same; :1561 (:1528) | `_kgspRpcGpuacctPerfmonUtilSamples` `:892` (610 `:767`): per-process util into GPUACCT (`gpu_acct.c:545`) | 1 s GSP sampling timer (`:909-916` comment) while accounting is on | on use (`nvidia-smi -am 1`; buffer allocated only then, `src/nvidia/src/kernel/diagnostics/gpu_acct.c:1477-1496`) | never | R11; truth: device-side busy time of the guest's own channels; low |
| 0x1009 | SIM_READ | `rpc_sim_read_v1E_01` 264/264, same; :1594 (:1561) | `_kgspRpcSimRead` `:1186` (610 `:1013`), only `IS_SIMULATION` | simulator escape | never | never | none |
| 0x100a | SIM_WRITE | `rpc_sim_write_v1E_01` 268/268, same; :1603 (:1570) | `_kgspRpcSimWrite` `:1217` (610 `:1044`) | simulator | never | never | none |
| 0x100b | SEMAPHORE_SCHEDULE_CALLBACK | `rpc_semaphore_schedule_callback_v17_00` 32/32, same; :1629 (:1596) | `_kgspRpcSemaphoreScheduleCallback` `:1237` (610 `:1064`) -> `dispswReleaseSemaphoreAndNotifierFill` (`src/nvidia/src/kernel/disp/disp_sw.c:129-179`) | a GF100_DISP_SW (0x9072) method scheduled a semaphore release or notifier fill (vblank), inferred from the handler | on use (GL allocates one DispSw per head, `display/mod.rs:538-541`, display-e.md) | never | R7; truth: device display vblank clock; low (medium if the object is forwarded to a host channel) |
| 0x100c | UCODE_LIBOS_PRINT | `rpc_ucode_libos_print_v1E_08` 8/8 + flex, same; :1613 (:1580) | `_kgspRpcUcodeLibosPrint` `:1294` (610 `:1121`), ENG_PMU only (`kpmuLogBuf`) | a libos-based PMU prints (inferred) | never on TU106 (Turing PMU is not a libos ucode, inferred) | never | none |
| 0x100d | VGPU_GSP_PLUGIN_TRIGGERED | `rpc_vgpu_gsp_plugin_triggered_v17_00` 8/8, same; :1568 (:1535) | 615: GMCAPI 0x20024 `gmcapiVgpuPluginTriggeredEvent` (`kernel_hostvgpudeviceapi.c:321`). 610: `_kgspRpcVgpuGspPluginTriggered` 610 `:1149` | vGPU host plugin offload | never | never | none |
| 0x100e | PERF_GPU_BOOST_SYNC_LIMITS_CALLBACK | `rpc_perf_gpu_boost_sync_limits_callback_v17_00` 16/16, same; :1655 (:1622) | `_kgspRpcPerfGpuBoostSyncLimitsCallback` `:955` (610 `:830`) | GPU Boost sync across GPUs (SLI) | never (single GPU) | never | none |
| 0x100f | PERF_BRIDGELESS_INFO_UPDATE | `rpc_perf_bridgeless_info_update_v17_00` 8/8, same; :1662 (:1629) | `_kgspRpcPerfBridgelessInfoUpdate` `:985` (610 `:860`) | bridgeless multi-GPU boost sync | never | never | none |
| 0x1010 | VGPU_CONFIG | `rpc_vgpu_config_event_v17_00` 4/4, same; :1576 (:1543) | `_kgspRpcGspVgpuConfig` `:1322` (610 `:1165`), NVA081 notifier | vGPU host config | never | never | none |
| 0x1011 | DISPLAY_MODESET | `rpc_display_modeset_v01_00` 12/12, same; :1552 (:1519) | not dispatched; DCE only (`dce_client_rpc.c:369-381`) | Tegra display | never | never | none |
| 0x1012 | EXTDEV_INTR_SERVICE | `rpc_extdev_intr_service_v17_00` 4/4, same; :1761 (:1707) | `_kgspRpcGspExtdevIntrService` `:1339` (610 `:1182`), `extdevGsyncService` | G-Sync / Quadro Sync board interrupt | never | never | none |
| 0x1013 | NVLINK_INBAND_RECEIVED_DATA_256 | `rpc_nvlink_inband_received_data_256_v17_00` 260/260, same; :1676 (:1643) | `:1011` (610 `:886`) | NVLink inband message | never (TU106 has no NVLink) | never | none |
| 0x1014 | NVLINK_INBAND_RECEIVED_DATA_512 | `..._512_v17_00` 516/516, same; :1683 (:1650) | `:1026` (610 `:901`) | same | never | never | none |
| 0x1015 | NVLINK_INBAND_RECEIVED_DATA_1024 | `..._1024_v17_00` 1028/1028, same; :1690 (:1657) | `:1041` (610 `:916`) | same | never | never | none |
| 0x1016 | NVLINK_INBAND_RECEIVED_DATA_2048 | `..._2048_v17_00` 2052/2052, same; :1697 (:1664) | `:1056` (610 `:931`) | same | never | never | none |
| 0x1017 | NVLINK_INBAND_RECEIVED_DATA_4096 | `..._4096_v17_00` 4100/4100, same; :1704 (:1671) | `:1071` (610 `:946`) | same | never | never | none |
| 0x1018 | TIMED_SEMAPHORE_RELEASE | `rpc_timed_semaphore_release_v01_00` 40/40, same; :1642 (:1609) | `_kgspRpcTimedSemaphoreRelease` `:1265` (610 `:1092`) -> `tsemaRelease_KERNEL` (`src/nvidia/src/kernel/gpu/timed_semaphore.c:635-687`) | a GF100_TIMED_SEMAPHORE_SW (0x9074) release reached its wait timestamp (methods `timed_semaphore.c:601-610`, inferred) | on use (gsp-surface.md §4 names GL) | never | R8; truth: device timer; low |
| 0x1019 | NVLINK_IS_GPU_DEGRADED | `rpc_nvlink_is_gpu_degraded_v17_00` 8/8, same; :1711 (:1678) | `:1089` (610 `:964`) | NVLink | never | never | none |
| 0x101a | PFM_REQ_HNDLR_STATE_SYNC_CALLBACK | `rpc_pfm_req_hndlr_state_sync_callback_v21_04` 16/16, same; :1771 (:1717) | `:1462` (610 `:1305`); allowed at boot without API lock (`:1585`) | platform request handler (notebook power, SMBPBI) | never on a desktop (inferred) | never | none |
| 0x101b | NVLINK_FAULT_UP | `rpc_nvlink_fault_up_v17_00` 4/4, same; :1669 (:1636) | `:997` (610 `:872`) | NVLink | never | never | none |
| 0x101c | GSP_LOCKDOWN_NOTICE | `rpc_gsp_lockdown_notice_v17_00` 1/1, same; :1788 (:1734) | `_kgspRpcGspLockdownNotice` `:1503` (610 `:1346`): sets `bInLockdown`, which gates the GSP interrupt service (`arch/turing/kernel_gsp_tu102.c:1048`) | GSP RISC-V core enters or leaves lockdown (inferred) | never (the device has no lockdown; sending "engaging" would stop the kernel servicing SWGEN0) | never | R14: never send |
| 0x101d | MIG_CI_CONFIG_UPDATE | `rpc_vgpu_gsp_mig_ci_config_v21_03` 44/44, same; :1778 (:1724) | `_kgspRpcMigCiConfigUpdate` `:1371` (610 `:1214`) | MIG compute instance change for vGPU | never | never | none |
| 0x101e | UPDATE_GSP_TRACE | `rpc_update_gsp_trace_v01_00` 8/8 + flex, same; :1746 (:1692) | `_kgspRpcGspUpdateTrace` `:1412` (610 `:1255`), RATS records (`KERNEL_GSP_TRACING_RATS_ENABLED` = 1, `inc/kernel/gpu/gsp/gsp_trace_rats_macro.h:41`) | GSP trace records while a client bound a RATS event buffer | on use (profilers only) | never | R12; never forward host trace (high); synthesize nothing |
| 0x101f | NVLINK_FATAL_ERROR_RECOVERY | `rpc_nvlink_fatal_error_recovery_v17_00` 2/2, same; :1725 (:1685) | `:1106` (610 `:981`) | NVLink | never | never | none |
| 0x1020 | GSP_POST_NOCAT_RECORD | `rpc_gsp_post_nocat_record_v01_00` 4/4 + flex record, same; :1754 (:1700) | `_kgspRpcGspPostNocatRecord` `:1430` (610 `:1273`): `rcdbNocatInsertNocatError` (`src/nvidia/src/kernel/diagnostics/journal.c:3933`) | GSP-RM logs a NOCAT error record | on error, optional (journal only; no Linux-visible effect, inferred) | never | R10; skip |
| 0x1021 | FECS_ERROR | `rpc_fecs_error_v26_02` 8/8, same; :1845 (:1791) | `_kgspRpcGspEventFecsError` `:1776` (610 `:1623`) -> work item -> `fecsHandleFecsLoggingError` (`src/nvidia/src/kernel/gpu/gr/fecs_event_list.c:942-974`) | FECS context-switch trace buffer full or needs reset (`rpc_headers.h:201-205`), not a GR fault | on use (FECS ctxsw trace via event buffer 0x90CD) | never | R9; truth: device's own FECS trace model; low |
| 0x1022 | RECOVERY_ACTION | `rpc_recovery_action_v28_01` 8/8, same; :1872 (:1818) | `_kgspRpcGspEventRecoveryAction` `:1836` (610 `:1683`): refresh (Xid 154 path `gpu.c:7404`), drain P2P, bus reset, OS or system reboot required | GSP-RM decides a recovery action after an error | on error | never | R5; truth: device policy (never host-derived); low |
| 0x1023 | TRIGGER_BUGCHECK | `rpc_trigger_bugcheck_v01_00` 4/4, same; :1924 (:1870) | `_kgspRpcGspTriggerBugcheck` `:1889` (610 `:1736`): `osBugCheck` -> `panic()` (`arch/nvalloc/unix/src/os.c:4192-4200`, `kernel-open/nvidia/os-interface.c:1563-1566`) | GSP-RM asks for a system bugcheck | never | never | R14: never send |
| 0x1024 | BIND_BAR2 | `rpc_bind_bar2_v01_00` 8/8, same; :1941 (:1887) | no handler in either tree | none in these drivers | never | never | none |
| 0x1025 | FORCED_DRIVER_SHUTDOWN | none | `_kgspRpcGspForcedDriverShutdown` `:1873` (610 `:1720`): mark for reset, `bFatalError` | GSP-RM error-injection test (`:1876-1880` comment) | never | never | R14: never send |
| 0x1026 | GSP_LOAD_EXEC_GENERIC_BOOTLOADER | `rpc_load_exec_generic_bootloader_v1_00` 4/4 + data, same; :1481 (:1452) | 615: GMCAPI 0x10002 `gmcapiLoadAndExecGenericBootloader` `:542`. 610: `_kgspRpcLoadAndExecuteGenericBootloader` 610 `:432`. Turing HAL resets the GSP falcon and runs a bootloader (`arch/turing/kernel_gsp_falcon_tu102.c:512-541`) | GSP asks the CPU to run a falcon bootloader (inferred: power transitions) | never observed; not expected at run time (inferred) | never | R13 |
| 0x1027 | GSP_LOAD_EXEC_HS_BINARY | `rpc_load_exec_hs_binary_v1_00` 4/4 + data, same; :1488 (:1459) | 615: GMCAPI 0x10003 `gmcapiLoadAndExecHsBinary` `:549`; TU106 HAL is the stub `_5baef9` (`generated/g_kernel_gsp_nvoc.c:2211-2227`). 610: 610 `:446` | GA102+ HS ucode load | never on TU106 | never | none |
| 0x1028 | UPDATE_GRID_DISPLAYLESS_PARAMS | `rpc_update_grid_displayless_params_v01_00` 24/24, same; :1931 (:1877) | `_kgspRpcGspUpdateGridDisplaylessParams` `:1906` (610 `:1753`): HOTPLUG subdevice event (`:1922`) | GRID displayless vGPU | never | never | none |
| 0x1029 | PMU_HALTED | none | `_kgspRpcGspEventPmuHalted` `:1819` (610 `:1666`): `bFatalError`, no more RPCs (`:366`, `:2478`) | physical PMU halted, GPU marked for reset | on error (host-fatal only) | never | R4; truth: host device lost, not a host event; medium |
| 0x102a | ECC_POSSIBLE_ERR_FAST_PATH | none | `_kgspRpcEccPossibleErrFastPath` `:1145`: kernel answers with the RPC ECC_NOTIFIER_WRITE_ACK (`src/nvidia/src/kernel/vgpu/rpc.c:11176`). 610: carried as RC_TRIGGERED with exceptType 141 (610 `:609-615`) | after an ECC/contained error's notifier writes (`:1139-1143` comment) | never (GeForce TU106 has no ECC) | never | none; if ever sent the device must accept the ACK RPC |
| 0x102b | NVLINK_TRAFFIC_QUIESCE_NOTIFY | `rpc_nvlink_traffic_quiesce_notify_v17_00` -/4, new; :1732 | `:1158` | NVLink | never | never | none |
| 0x102c | GPU_GFM_STATE_CHANGE | `rpc_gpu_gfm_state_change_v17_00` -/4, new; :1718 | `:1122` | NVSwitch fabric manager | never | never | none |
| 0x102d | NVLINK_ABM_FABRIC_HEALTH_MASK_UPDATE | `rpc_nvlink_abm_fabric_health_mask_update_v01_00` -/4, new; :1739 | `_kgspRpcNvlinkAbmFabricHealthMaskUpdate` `:575` | NVLink fabric | never | never | none |

The RPC header before every payload is `rpc_message_header_v03_00`, 32 bytes in both trees
(`g_rpc-message-header.h:41`).

### GMCAPI commands from GSP to the kernel (615, same queue, NVDM type 0x26)

Dispatched by the drain loop, which calls `gmcapiCommand` for any GMCAPI element that is not the
awaited response (`kernel_gsp.c:1989-2020`, call at `:2006`); `gmcapiDispatch` runs the local
(GMCAPI_KERNEL) handler (`src/nvidia/src/kernel/core/gmcapi_impl.c:43-156`); every kernel-side
command must be fire-and-forget (`gmcapi_impl.c:212-215`).

| Id | Name | Handler | Payload | Needed on TU106 Linux | Device today |
|---|---|---|---|---|---|
| 0x10002 | EXEC_GENERIC_BOOTLOADER | `gmcapiLoadAndExecGenericBootloader` `kernel_gsp.c:542` | `GspLoadExecGenericBootloaderParams` (`gmcapi_table.h:93-100`) | not expected (inferred) | never |
| 0x10003 | EXEC_HS_BINARY | `gmcapiLoadAndExecHsBinary` `:549` | `GspLoadExecHsBinaryParams` (`:101-108`) | never (stub HAL on TU106) | never |
| 0x10005 | GSP_RESUME_DONE | `gmcapiGspResumeDone` `:7104-7124`, awaited by `kgspWaitForRmResumeDone` (`:7151`) | `GmcApiGspResumeDoneParams` (`:117-124`) | on use (guest suspend/resume) | never; the device answers GSP_SUSPEND 0x10004 (`boot.rs:961-971`) |
| 0x20022 | SHUTDOWN_GSP_VGPU_PLUGIN_TASK_COMPLETE | `kernel_hostvgpudeviceapi.c:285` | `GmcApiShutdownGspVgpuPluginTaskCompleteEvent` | never | never |
| 0x20024 | VGPU_PLUGIN_TRIGGERED_EVENT | `kernel_hostvgpudeviceapi.c:321` | `GmcApiVgpuPluginTriggeredEvent` | never | never |
| 0x40001 | MMU_FAULT_QUEUED | `gmcapiMmuFaultQueued` `kernel_gsp.c:1178-1183` -> `osQueueMMUFaultHandler` | none (`gmcapi_table.h:268-275`) | on UVM non-replayable fault | never |

The device's v2 transport sends a message whose `function` is above 0xFFFF as the GMCAPI
*response* to a request (`msgq/v2.rs:638-644`). An unsolicited request (0x40001, 0x10005) needs a
request variant: command id without `GMCAPI_HEADER_COMMAND_FLAGS_RESPONSE`
(`interface/gmcapi/gmcapi_base.h`, `gmcapi.rs:38-39`), sequence 0, size 0 or the params. The kernel
would still dispatch one carrying the response flag, because it masks the id
(`kernel_gsp.c:1993-2006`), but it would then match a pending wait with the same id and
sequence.

## Detailed recipes

### What the device has today for errors in host-mirrored channels

- The host channel's error notifier is the guest's own page: `cp.hObjectError` is an OS descriptor
  over the guest's error-notifier page for the normal mirror (`mirror.rs:1405-1431`) and the GR
  shadow channel (`mirror.rs:778-791`); d-gr-launch7 added it so that "a fault returns 719 not a
  30 s hang" (`experiments.md:1003`). The host RM's RC writes `NvNotification`
  (`src/common/sdk/nvidia/inc/nvgputypes.h:57-64`: timeStamp, info32 = exceptType, info16 = engine,
  status = 0xFFFF) at index `NV_CHANNELGPFIFO_NOTIFICATION_TYPE_ERROR` 0 (`nvos.h:2855`) of that page
  through `krcErrorWriteNotifier_CPU` (`src/nvidia/src/kernel/gpu/rc/kernel_rc_notification.c:93`).
- The device subscribes one host OS event on the subdevice notifier `NV2080_NOTIFIERS_RC_ERROR` 37
  (`mirror.rs:63`, `:1042-1051`; `hostmem.rs:1180-1226` allocates `NV01_EVENT_OS_EVENT` and arms it
  with `NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION`). The event thread only counts it
  (`intr.rs:37`, `mirror.rs:2328-2340`) and logs "HOST RC EVENT on a mirrored engine".
- The guest learns of the RC only by reading the notifier page (CUDA returns 719, UVM
  `NV_ERR_RC_ERROR`, `execution.md:115-118`). Its kernel RM gets no RC_TRIGGERED and no
  OS_ERROR_LOG, so there is no guest Xid, no `NV2080_NOTIFIERS_RC_ERROR` to guest clients, no RC
  journal (nvidia-smi RC counters), and the guest channel stays scheduled in the guest's view.
- Isolation problem in today's subscription: host `krcErrorNotifyClients_KERNEL`
  (`kernel_rc_notification.c:483-508`) calls `gpuNotifySubDeviceEvent(RC_ERROR)`, which walks every
  Subdevice of the GPU whoever owns it (`src/nvidia/src/kernel/gpu/gpu_rmapi.c:624`, loop
  `:649-651`). The device therefore also receives the RCs of the host desktop and of any other host
  client, with exceptType (info32) and partitionAttributionId (info16) but no chid. Today these are
  only counted and printed; nothing may be forwarded from this source.
- The software executor raises no error of any kind (no RC, no fault); `uvm.rs:160-195` records the
  fault buffers and never writes an entry.

### R1. RC_TRIGGERED (0x1004)

Kernel effect (615, `kernel_gsp.c:717-840`): bus-error check; for gfid 0 (PF) find the
KernelChannel by chid in the chid manager of `nv2080EngineType` (or by instance block when
`rcChannelInfoType` is INSTBLK, `:746-764`), failing with INVALID_CHANNEL if absent (`:766-768`);
append the rcJournal records to the RC database (`:772-817`); write the error notifier
(`krcErrorSetNotifier`, `:826`; `kernel_rc_notification.c:235-351`, TSG-wide when scope is TSG, and a
UVM-owned channel sets "OS reboot required" `:260-263`); then `krcErrorNotifyClients_HAL` (`:832`):
wake the channel's RC notifier events (`krcErrorSendEventNotificationsCtxDma_FWCLIENT`,
`kernel_rc_notification.c:397-479`, `kchannelNotifyEvent` with `rcNotifierIndex`
`NVC46F_NOTIFIERS_RC` 0, `kernel_channel.c:1529`, `:1747-1786`) and the subdevice
`NV2080_NOTIFIERS_RC_ERROR` (`kernel_rc_notification.c:503-508`). 610 differs
(610 `kernel_gsp.c:585-716`): exceptType 141 is the ECC fast-path ack (610 `:609-615`), the kernel
writes the notifier only when Confidential Computing is on (610 `:697-704`; GSP-RM writes it
otherwise) and it calls `krcErrorSendEventNotifications_HAL` (610 `:706`).

Payload to build (615, 64 bytes): `nv2080EngineType` = the guest channel's engine type (the
channel registry has it, `channel.rs:577-595`); `chid` = the guest chid; `gfid` 0; `exceptLevel` 0;
`exceptType` = the Xid number (`src/common/sdk/nvidia/inc/nverror.h`, e.g. 31 MMU_ERR_FLT `:49`, 32
PBDMA `:50`, 13 GR exception `:37`); `scope` `RC_NOTIFIER_SCOPE_CHANNEL` 0 or `_TSG` 1
(`generated/g_kernel_rc_nvoc.h:73-74`); `partitionAttributionId` 0; `rcChannelInfoType`
`RC_CHANNEL_INFO_TYPE_CHANNEL_INFO` (first enumerator, `inc/kernel/gpu/rc/rc_channel_info.h:31`);
instblk fields 0; `rcJournalBufferSize` 0 (the loop then adds nothing, `:789`). 610 uses the 48-byte
layout without the four fields; the device's per-version ABI tables must carry both, since the
versioned name did not change.

Truth sources:
1. Mirrored channel. Allocate per host channel an `NV01_EVENT_OS_EVENT` (`nvos.h:390`) whose parent
   is the host channel handle and `notifyIndex` = `NVC46F_NOTIFIERS_RC` 0
   (`src/common/sdk/nvidia/inc/class/clc46fsw.h:29`), alloc params `NV0005_ALLOC_PARAMETERS`
   (`class/cl0005.h:40-47`). Channel events need no SET_NOTIFICATION arming (`kchannelNotifyEvent`
   posts to any event on the channel's list, `kernel_channel.c:1767-1777`). Because the mirror's
   `hObjectError` is memory and not a ContextDma, the host takes the `kchannelNotifyEvent` branch
   (`kernel_rc_notification.c:447-470`), so the event fires (to be confirmed, M1). The event carries
   no exceptType; read it from info32 of the notifier the host just wrote into the guest page (the
   device can read guest memory). Map host channel to guest chid through the mirror's own table
   (`mirror.rs` `channels`, keyed by guest chid), never by a host chid from a message. For an MMU
   fault, `NV906F_CTRL_CMD_GET_MMU_FAULT_INFO` 0x906F0106 on the host channel
   (`ctrl/ctrl906f.h:175`) may give the address and type (M4).
2. Software executor. The executor already refuses or cannot complete some methods
   (`channel.rs:1494`); a decode error, an unmapped GPU VA or a timeout becomes an RC with the Xid
   a real GPU would give (32 PBDMA for a bad method, 31 for an unmapped VA, 13 for a GR exception).
3. Drop the subdevice `RC_ERROR` subscription as a source (keep it at most as a host-health
   counter, not logged per record).

Order: host event -> read notifier -> mark the guest channel faulted in the device (stop
executing, stop forwarding doorbells) -> send OS_ERROR_LOG (R2) then RC_TRIGGERED, then raise
SWGEN0 as `post_display_command` does (`boot.rs:293-297`). The guest kernel then runs its RC recovery
and may send `NV906F_CTRL_CMD_RESET_CHANNEL` 0x906F0102 (`ctrl906f.h:139`; gsp-surface.md §6 row
8), which the device forwards to the host channel or answers by freeing and rebuilding the mirror.
Isolation: medium. Contained if the only source is events on the device's own host channels and
the only data read is the guest's own notifier page; the subdevice RC_ERROR source is not.

### R2. OS_ERROR_LOG (0x1006)

Kernel effect: `nvErrorLog2_va` (`kernel_gsp.c:849-885`) prints
"NVRM: Xid (PCI:...): N, pid=..., name=..., <errString>" when the chid resolves to a channel with a
process (`kernel_rc.c:388-400`), feeds the Xid tracepoint (`kernel-open/nvidia/nv-tracepoint.h:57`)
and the OS error notifications. Payload (272 bytes, same in both): `exceptType` Xid,
`preemptiveRemovalPreviousXid` 0, `runlistId` and `chid` of the guest channel (or `INVALID_CHID` for
no channel), `errString` 256 bytes. Truth: the same sources as R1; the string mimics the host's
text for that Xid class but only with guest facts (guest VA, guest engine name); never copy a host
Xid line (it names host pids and host chids). Allowed during boot without the API lock
(`:1584`). Isolation: medium (the same containment as R1).

### R3. MMU faults

615 path: replayable faults are serviced by the kernel from its own interrupt vector
(`src/nvidia/src/kernel/gpu/mmu/arch/turing/kern_gmmu_tu102.c:206`); non-replayable faults are
copied by GSP-RM into the client shadow buffer registered by UVM (`mmu_fault_buffer_ctrl.c:45-126`)
and announced by GMCAPI 0x40001, whose handler only schedules the UVM bottom half
(`kernel_gsp.c:1178-1183`). On 610 the same announcement is event 0x1005 (610 `:1002-1010`). A
non-replayable fault on a channel that UVM does not own is an RC (Xid 31) instead, which is R1.
Today: guest faults in mirrored channels become host RCs on the host channel (Xid 31 seen in
experiments, `experiments.md:984`, `:1000`), visible to the guest only through the notifier page.
Recipe: (a) for every non-UVM channel, R1 + R2 with exceptType 31; (b) UVM fault servicing needs
the device to write fault packets into the recorded shadow buffer (`uvm.rs:185-190`) or the
replayable buffer and raise GMCAPI 0x40001 or the replayable vector; the truth for a mirrored
channel would be the host's fault buffer, which belongs to the host RM and host UVM and is not
per-client, so a host-mirrored UVM fault path is not contained. Only software-executor faults
can feed (b). UVM managed memory is not supported today (`experiments.md:1010`, 801). Isolation:
low for (a), high for any host fault-buffer source.

### R4. PMU_HALTED (0x1029) and FORCED_DRIVER_SHUTDOWN (0x1025)

Both set `bFatalError`, after which the kernel refuses every RPC (`kernel_gsp.c:366`, `:2478`),
FORCED_DRIVER_SHUTDOWN also marks the device for reset (`:1873-1887`, `gpu.c:7078`). The device has
no PMU. The only honest use: the host GPU is lost (host RM calls fail with GPU_IS_LOST) and the
device wants the guest to see a dead GPU rather than time out. That is a device-level decision from
its own host client's return codes, not a forwarded host event. FORCED_DRIVER_SHUTDOWN is for GSP
error-injection tests: never send.

### R5. RECOVERY_ACTION (0x1022)

Types (`inc/kernel/vgpu/rpc_headers.h:253-257`) drive nvidia-smi's recovery-action field and Xid
154 (`gpuRefreshRecoveryAction_KERNEL`, `gpu.c:7404`). Note that the guest kernel already raises
"OS reboot required" itself when a UVM-owned channel gets an RC (`kernel_rc_notification.c:260-263`;
the guest Xid 154 of run a2, `experiments.md:969`). The device should not send any reboot or bus
reset action for contained channel errors; a virtual GPU recovers by freeing its mirrors. Send
nothing, or at most REFRESH. Never derive from the host's recovery state (host-global).

### R6. POST_EVENT variants

Mechanism: the guest allocates `NV01_EVENT_OS_EVENT` 0x79 or `NV01_EVENT_KERNEL_CALLBACK_EX` 0x7E on
a GSP-owned object (routed to GSP by `GSP_RM_ALLOC`); the device must answer later with POST_EVENT
naming hClient and hEvent (`_kgspRpcPostEvent`, `kernel_gsp.c:632-708`). Today only HOTPLUG events
are recorded (`display/mod.rs:1425-1437`, called from `ctrl.rs:704-706`), all others are echoed,
and `NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION` is answered "event registered, never raised yet"
(`ctrl.rs:1037`). The payload today has `bNotifyList` 0, `eventDataSize` 0 and the plug mask in
`data` (`display/mod.rs:1489-1496`).
Recipe: one event registry (hClient, hEvent, parent class, notifyIndex, armed mode from
SET_NOTIFICATION) and one `emit_post_event(notifyIndex, data, info16, eventData)` used by every
model. Candidates on TU106 Linux: `NV2080_NOTIFIERS_HOTPLUG` 1 and `DP_IRQ` 7 (nvkms,
`nvkms-rm.c:2064-2103`; `cl2080_notification.h:37`, `:43`) from the display model;
`HDMI_FRL_RETRAINING_REQUEST` and `HDCP_STATUS_CHANGE` (nvkms) never for the device's links;
`FIFO_EVENT_MTHD` 35 (`:72`) if a client uses the event method; `GR_DEBUG_INTR` 8 (`:44`) only for
a debugger; `POWER_EVENT` 118, `CLOCKS_CHANGE` 119, `PSTATE_CHANGE` 33, `THERMAL_*` 3/4 only if the
device ever models them. Engine non-stall notifiers (GRAPHICS 12, CE 25-27) are not POST_EVENTs:
the kernel services those from the interrupt tree, which the device already raises
(`mirror.rs:23-25`, `:2318-2327`). Include `eventData` where nvkms reads it (the
`Nv2080HotplugNotification` masks, cf. `disp_common_kern_ctrl_minimal.c:305-319`); a measurement
(M9) settles whether nvkms reads `data` or `eventData`. Isolation: low (guest-registered events,
device-model truth); never forward host display events.

### R7. SEMAPHORE_SCHEDULE_CALLBACK (0x100b)

The kernel writes a semaphore release or a notifier (index `NV9072_NOTIFIERS_NOTIFY_ON_VBLANK`) at
a GPU VA of the client's VA space on the CPU, identifying the client by hClient and the Device
handle passed in `hEvent` (`kernel_gsp.c:1237-1263`, `disp_sw.c:129-179`). GSP-RM raises it after
it processed GF100_DISP_SW methods and the vblank came (inferred). The guest GL driver allocates a
DispSw per head (`display/mod.rs:538-541`). Recipe: when the device sees 0x9072 methods (software
executor, subchannel bound to class 0x9072; `channel.rs:920-935` is where SW classes are decoded)
queue the release and emit the event at the device display's next vblank tick
(`display/mod.rs` `tick`). For a mirrored channel the GPFIFO is not interpreted
(`mirror.rs:19-20`) and `add_engine_object` would allocate 0x9072 on the host channel with the
guest's parameters (`mirror.rs:1583-1601`): the host would then release on the host display's
vblank of that head. Keep 0x9072 channels off the host or refuse the host object. Isolation:
low in the executor, medium if forwarded (host vblank timing).

### R8. TIMED_SEMAPHORE_RELEASE (0x1018)

`tsemaRelease_KERNEL` writes the semaphore value with a timestamp and the notifier through the
client's VA space (`timed_semaphore.c:635-687`). Recipe: execute the 0x9074 methods (set notifier,
semaphore, wait timestamp, release value, schedule, `timed_semaphore.c:601-610`) in the device,
arm a device timer at the wait timestamp (device PTIMER time), then emit the event. Truth: device
timer. Isolation: low.

### R9. FECS_ERROR (0x1021)

Only about the FECS context-switch trace buffer (`rpc_headers.h:201-205`,
`fecs_event_list.c:942-974`): BUFFER_FULL makes the kernel drain it into the client's event buffer,
BUFFER_RESET_REQUIRED disables and resets it. It matters only when a profiler binds a FECS trace
event buffer. The device has a FECS record size answer (`kgr.rs:444-449`) but no trace; emit
nothing until it models the trace, then emit BUFFER_FULL from its own buffer fill level.
Isolation: low; never mirror the host's FECS trace (it records every context on the GPU).

### R10. GSP_POST_NOCAT_RECORD (0x1020)

NOCAT journal insert (`kernel_gsp.c:1430-1459`); the event is on the boot-time whitelist
(`:1583`). No Linux-visible effect beyond the journal read by `NV2080_CTRL_CMD_NVD_*` (inferred).
Skip.

### R11. GPUACCT_PERFMON_UTIL_SAMPLES (0x1008)

Only with accounting mode on (`gpu_acct.c:1477-1496` allocates `pSamplesParams` only then; the
handler drops samples before that, `kernel_gsp.c:907-916`). A GSP client has no local sampling
timer (`gpu_acct.c:1496`, `:489`). Recipe: a 1 s device timer while accounting is on, samples of
gr/fb/nvenc/nvdec util with the guest pid and subpid of the busiest context, from the device's own
busy accounting of guest channels (executor time, host-channel wall time between doorbell and
GPGet catch-up). Isolation: low if computed from the device's own channels; host per-process
utilization is other tenants' data (high).

### R12. UPDATE_GSP_TRACE (0x101e)

RATS trace records into a kernel event buffer (`kernel_gsp.c:1412-1428`), only when a client
bound one (gsp-surface.md 615 section, `NV2080_CTRL_CMD_EVENT_RATS_GSP_TRACE_BIND_EVTBUF`). The
device is not GSP-RM and has nothing to trace; answer the bind with NOT_SUPPORTED and never send.

### R13. GSP_LOAD_EXEC_* and the GMCAPI loader requests

On TU106 the generic bootloader HAL resets the GSP falcon and runs a bootloader
(`kernel_gsp_falcon_tu102.c:512-541`), the HS binary HAL is a stub. The device's boot model
already runs the boot sequence without these requests; never send them. Suspend and resume:
the device answers GSP_SUSPEND (`boot.rs:961-971`); after resume the kernel waits for
GSP_RESUME_DONE (`kernel_gsp.c:7135-7160`), which the device must send as a GMCAPI request with
`GmcApiGspResumeDoneParams` (flags 0 unless D3hot), see the transport note above and M8.

### R14. Events the device must never send

TRIGGER_BUGCHECK panics the guest (`os-interface.c:1563-1566`); FORCED_DRIVER_SHUTDOWN and
PMU_HALTED end the guest driver; GSP_LOCKDOWN_NOTICE with `bLockdownEngaging` stops the kernel's
GSP interrupt service (`kernel_gsp_tu102.c:1048`); ECC_POSSIBLE_ERR_FAST_PATH makes the kernel send
ECC_NOTIFIER_WRITE_ACK. The dead, NVLink, vGPU, MIG, simulation and Tegra events have no meaning
for the device. A decode helper for every id is still useful for the trace.

### Implementation shape

One `events.rs` beside `boot.rs`: `emit_event(function, payload)` over `QueueState::send`
(`msgq.rs:747-755`, both transports; v2 via `msgq/v2.rs:644`) plus SWGEN0 as in
`post_display_command` (`boot.rs:293-297`), and `emit_gmcapi_request(id, params)` for v2. The
payload builders are per ABI version (RC_TRIGGERED differs). Sources feed it through a channel:
the mirror's per-channel host RC events (R1/R2), the executor's error paths, the display tick (R6,
R7), a device timer (R8, R11), and the event registry (R6). A per-guest-channel "faulted" state
stops later doorbells and executor work until the guest frees or resets the channel.

## Measurement proposals

- M1. Does the host deliver a channel-level RC event for a device-owned mirror channel whose
  `hObjectError` is an OS-descriptor memory object? Experiment: in `hostmem.rs` add
  `os_event_on(parent=host channel, notifyIndex=0)` and allocate it in `mirror.rs` after each host
  channel; rerun a known contained fault (the c-pbentry push, `experiments.md:974`, or a CE copy to
  an unmapped guest VA) and log per-channel events next to the subdevice RC_ERROR count and the
  info32 of the guest notifier page. Pass: one channel event per RC, exceptType matches.
- M2. How much host-global RC traffic the current subdevice RC_ERROR subscription receives.
  Experiment (needs the operator's go-ahead: host GPU state is off limits without it): with the device idle, run a host-side
  CUDA program owned by another user that triggers an illegal-address fault, and check whether the
  device logs "HOST RC EVENT". Without that go-ahead: count RC_ERROR events during a desktop
  session where the device has no channels.
- M3. What real GSP-RM sends for a guest MMU fault on 615 (order of OS_ERROR_LOG and RC_TRIGGERED,
  `rcChannelInfoType`, journal size, whether GMCAPI 0x40001 also comes, errString text).
  Experiment: in a passthrough VM (passthrough-procedure.md, with the SSH/tmux procedure prepared first, since the session runs on the GPU desktop) load the 615 guest
  driver with `NVreg_RmMsg` enabling `kernel_gsp.c` INFO prints ("received event", `:1567`) and run
  a CUDA kernel with an illegal address and one with a bad pushbuffer method; save dmesg and
  `nvidia-smi -q -d ROW_REMAPPER,PAGE_RETIREMENT` for reference.
- M4. Does `NV906F_CTRL_CMD_GET_MMU_FAULT_INFO` on a host GSP-client channel return the fault after
  an Xid 31? Experiment: after M1's fault, call it on the host channel handle from the device's
  client and log addrHi/addrLo/faultType against the host journal line.
- M5. What the guest kernel does after a synthesized RC_TRIGGERED on 615 and 610. Experiment: a
  debug hook `CARAXES_INJECT_RC=<chid>:<xid>` that emits OS_ERROR_LOG + RC_TRIGGERED for a
  software-executor channel; record the RPCs that follow (RESET_CHANNEL, frees), guest dmesg Xid
  line, nvidia-smi RC count, CUDA return code. On 610 also check that the kernel accepts the
  48-byte layout and does not need the notifier written by the device.
- M6. Does the guest's GL/EGL stack push GF100_DISP_SW (0x9072) or GF100_TIMED_SEMAPHORE_SW (0x9074)
  methods, and on which channels? Experiment: grep the device traces of the GNOME and Weston runs
  (`caraxes/runs/display-f-2026-09-24/`) for allocs of 0x9072/0x9074 and "host object class 0x9072";
  run glxgears with swap interval 1 and log SW-class methods in the executor.
- M7. Which events a 615 GSP-RM sends on a desktop TU106 at boot and idle (e.g.
  PFM_REQ_HNDLR_STATE_SYNC, NOCAT, LIBOS_PRINT, GPUACCT). Experiment: the M3 VM, boot with RmMsg
  on, 10 minutes idle desktop, count event ids.
- M8. Guest suspend and resume on 615: after `GSP_SUSPEND`, does the kernel re-run
  `GMCAPI_CMD_GSP_INIT` or only wait for GSP_RESUME_DONE, and which flags? Experiment: `virsh
  dompmsuspend`/`dompmwakeup` of a device VM with trace on; if it waits, send RESUME_DONE (flags 0)
  and check `nvidia-smi` after wakeup.
- M9. HOTPLUG delivery detail: does nvkms read the plug mask from `data` or from `eventData`
  (`Nv2080HotplugNotification`)? Experiment: send a variant with `eventData` filled and
  `eventDataSize` 8 in the display-cd test and compare nvkms's reaction (connector probe
  count) with today's payload.
- M10. Accounting: with `nvidia-smi -am 1` in the guest and no samples, does
  `--query-accounted-apps` fail or report 0? Then check R11's samples appear there.
