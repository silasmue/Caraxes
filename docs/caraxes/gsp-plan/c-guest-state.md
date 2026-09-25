<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
<!-- Research by a read-only subagent on 2026-09-24 for the GSP surface programme (P23). Sources as in b-host-forwardable.md: the 610.57.04 and 615.71.09 trees, the crate at 61e0b6a, the native traces and the run logs. Nothing was run against a GPU. -->
# Category C, guest-state derived controls: the implementation plan

Category C of [gsp-surface.md](../gsp-surface.md) §3 holds the controls on objects the guest
allocated: channels, TSGs, context shares and graphics contexts. This file takes the category
control by control against both driver trees. For each control it names:

- the guest-visible state the answer derives from;
- where that state lives in the device today;
- what is missing;
- a test recipe.

Section 3 adds the twelve controls that
[b-host-forwardable.md](b-host-forwardable.md) moved here from category B.

## Summary

- **Status (2026-09-25, after round 2 and the reconciliation with the D models).** 28 of
  the 31 controls of §2 are done and 3 are partial; 11 of the 12 from B are done (B1, the ZBC
  entry, is the ZBC model's; B9 is answered by dmodel.rs's debugger record); of the 17 rows
  the D plan moved here, 12 are rows of the table and 5 are answered by the D models
  (rc.rs, dmodel.rs), 14 done and 3 partial. One RC journal and one set of scheduling flags
  (rc.rs `RcState`). Details in §8 "Reconciliation".

- **The set.** 31 controls in 615.71.09, 29 in 610.57.04. The two 615-only controls are:
  - `NV0090_CTRL_CMD_PROGRAM_SYSMEM_PROMOTE`, new in 615;
  - `NV_EVENT_BUFFER_CTRL_CMD_UPDATE_GET`, which exists in 610 but reaches GSP only from 615 on,
    through the new operational-event importer.

  gsp-surface.md counted 30 for 610 because `NVA06F_CTRL_CMD_BIND` has been answered since
  (uvm.rs:303, commit e6e9fff). Sizes and struct text are identical in both trees for every
  control present in both (method as in b-host-forwardable.md §2). With the twelve from B, the
  C plan covers 43 controls.
- **Groups.**

| Group | Meaning | Controls |
|---|---|---|
| M | record, forward to the host mirror object | 8 |
| RC | RC and fault state of a channel | 5 |
| L | record only, never forwarded (host scheduling) | 2 |
| K | kernel-privileged or plugin controls without a caller: record and echo | 5 |
| X | never sent by a GSP client | 2 |
| V | live migration: refuse | 8 |
| EB | event buffer (615) | 1 |

- **Only 21 need work; 10 need nothing or a refusal.** The two INTERNAL TSG controls never
  arrive from a GSP client, because the kernel sends the public form instead. The eight
  NVB06F live-migration controls are refused as a family.
- **What the device lacks is one record per guest object.** The channel registry
  (`channel::Registry`, crates/vfio-user-nvgpu/src/channel.rs:236) knows every channel, TSG,
  VA space and context share by handle. The host mirror (`mirror::Mirror`, mirror.rs:162)
  knows the host handle of every mirrored channel and TSG. Neither keeps the per-object
  attributes these controls set and read: interleave level, TPC partition mode, promotion
  sizes, realtime flag, RC state and the last MMU fault. Nor does the device model the ZBC
  table, the event buffer or the debugger session.
- **Top ten by client likelihood** (§6):
  1. `NV906F_CTRL_CMD_RESET_CHANNEL`
  2. `NV_EVENT_BUFFER_CTRL_CMD_UPDATE_GET`
  3. `NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY` (from B)
  4. `NV506F_CTRL_CMD_RESET_ISOLATED_CHANNEL`
  5. `NV906F_CTRL_CMD_GET_MMU_FAULT_INFO`
  6. `NV506F_CTRL_CMD_INTERNAL_RESET_ISOLATED_CHANNEL`
  7. `NVA06C_CTRL_CMD_SET_INTERLEAVE_LEVEL`
  8. `NVA06F_CTRL_CMD_SET_INTERLEAVE_LEVEL`
  9. `NV208F_CTRL_CMD_FIFO_GET_CHANNEL_STATE` (from B)
  10. `NV0090_CTRL_CMD_GET_MMU_DEBUG_MODE` (from B)

## 1. How a C control reaches the device

Kernel source lines in this file are in the 615.71.09 tree (`src/nvidia/src/kernel/...`) unless a 610 line is given; the table gives both where they differ.

The guest kernel sends a C control with the handles of the guest object it acts on. In most
cases the kernel's own handler runs first and forwards the control; the "Route" column names
that handler. The kernel keeps its own bookkeeping, such as `bIsRcPending`
(kernel_channel.c:3040) or the TSG's `timesliceUs` (kernel_channel_group_api.c:1365),
and sends GSP only the hardware part.

In the device, `Dispatcher::control` (ctrl.rs:797) receives `(hClient, hObject)`. There are
two ways to resolve them:

- **The registry.** It maps handles to ChIDs, TSGs and context shares: `channels`, `tsgs` and
  `ctxshares` (channel.rs:249-259). Duplicated handles resolve through `aliases` via
  `Registry::resolve` (channel.rs:416).
- **The mirror.** For a channel or TSG with a host twin, `Mirror::forward_control`
  (mirror.rs:1711-1735) issues the same control on the host object. It is used today for the
  five controls of `MIRROR_FORWARDED` (ctrl.rs:130): stop, preempt, timeslice and the two
  GPFIFO_SCHEDULE forms. The guest always gets NV_OK, and the host status goes to the log.

A control that names a guest object inside its params (`hChannel`, `hClient`) cannot use
`forward_control`, which works on the object the control is sent to. It needs a lookup of the
host handle and a params rewrite. The mirror's host objects belong to the mirror's own host
client (`HostMemory`, the `hm` field, mirror.rs:163), not to `HostRmClient`. A
subdevice-level control naming a mirrored channel must therefore be issued on the mirror's
subdevice.

## 2. The 31 controls

Columns as in b-host-forwardable.md §2. "Where it lives" names the module and struct in
crates/vfio-user-nvgpu/src. "Answer" is the recipe, and "Test" names a unit test against the
state or a guest gate.

| # | Control, id | NVOC class, export entry (610 / 615) | Params struct, header | Size 610 / 615 | Privilege | Route on the real card | Derived from | Where it lives in the device | Missing | Answer | Test | Group |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | `NV0090_CTRL_CMD_SET_TPC_PARTITION_MODE` 0x00900101 | KernelChannelGroupApi, g_kernel_channel_group_api_nvoc.c:242 / :249 | `NV0090_CTRL_TPC_PARTITION_MODE_PARAMS`, ctrl0090.h:67 | 8 / 8 | NON_PRIVILEGED | kernel then GSP: `kgrctxCtrlSetTpcPartitionMode_IMPL` (kernel_graphics_context.c:3355, both) forwards on a GSP client with the caller's handles (send :3371) | the TSG or channel the handle names, its GR context | `channel::Registry` `tsgs` (channel.rs:249) and `Channel` (:195); host side `mirror::HostTsg` (mirror.rs:122) | a per-TSG record of TPC partition mode and bEnableAllTpcs | record; when the TSG has a host mirror, `Mirror::forward_control` (mirror.rs:1711) on the host TSG (NON_PRIVILEGED on the host); NV_OK | unit: SET then GET round trip on the registry; guest gate: none known that sends it | M |
| 2 | `NV0090_CTRL_CMD_GET_TPC_PARTITION_MODE` 0x00900103 | KernelChannelGroupApi, g_kernel_channel_group_api_nvoc.c:257 / :264 | `NV0090_CTRL_TPC_PARTITION_MODE_PARAMS`, ctrl0090.h:67 | 8 / 8 | NON_PRIVILEGED | kernel then GSP (kernel_graphics_context.c:3324, send :3340, both) | as SET_TPC_PARTITION_MODE | as above | as above | the recorded mode, default NONE | unit round trip | M |
| 3 | `NV0090_CTRL_CMD_PROGRAM_VIDMEM_PROMOTE` 0x00900107 | KernelChannelGroupApi, g_kernel_channel_group_api_nvoc.c:287 / :294 | `NV0090_CTRL_PROGRAM_VIDMEM_PROMOTE_PARAMS`, ctrl0090.h:123 | 8 / 8 | NON_PRIVILEGED | kernel then GSP: `kgrctxCtrlProgramVidmemPromote_IMPL` (kernel_graphics_context.c:3448, `NV_RM_RPC_CONTROL` :3464, both) | the GR context's L1/T1 promotion | as above | per-TSG promotion record | record; forward to the host TSG when mirrored | unit round trip | M |
| 4 | `NV0090_CTRL_CMD_SET_LG_SECTOR_PROMOTION` 0x0090010B | KernelChannelGroupApi, g_kernel_channel_group_api_nvoc.c:302 / :309 | `NV0090_CTRL_SET_LG_SECTOR_PROMOTION_PARAMS`, ctrl0090.h:148 / :185 | 4 / 4 | NON_PRIVILEGED | kernel then GSP: `kgrctxCtrlSetLgSectorPromotion_IMPL` (kernel_graphics_context.c:3386, send :3402, both) | the GR context's sector promotion | as above | per-TSG record | record; forward when mirrored | unit round trip | M |
| 5 | `NV0090_CTRL_CMD_PROGRAM_SYSMEM_PROMOTE` 0x0090010E | KernelChannelGroupApi, g_kernel_channel_group_api_nvoc.c:324 (615 only) | `NV0090_CTRL_PROGRAM_SYSMEM_PROMOTE_PARAMS`, ctrl0090.h:161 (615 only) | - / 4 | NON_PRIVILEGED | 615 only: `kgrctxCtrlProgramSysmemPromote_IMPL` (kernel_graphics_context.c:3482, send :3498); absent in 610 | the GR context's sysmem promotion | as above | per-TSG record; the 615 host must know the control (it does: same id) | record; forward when mirrored | unit round trip | M |
| 6 | `NV506F_CTRL_CMD_RESET_ISOLATED_CHANNEL` 0x506F0105 | KernelChannel, g_kernel_channel_nvoc.c:420 / :442 | `NV506F_CTRL_CMD_RESET_ISOLATED_CHANNEL_PARAMS`, ctrl506f.h:81 | 8 / 8 | NON_PRIVILEGED | kernel then GSP: the RC callback sends it after an isolated RC (kernel_rc_callback.c:88 `NV_RM_RPC_CONTROL`, :198, :414 under `IS_GSP_CLIENT` and `RC_CALLBACK_ISOLATE`, both); a user call goes through `kchannelCtrlCmdResetIsolatedChannel_IMPL`, which sends the INTERNAL form instead (kernel_channel.c:2983 / 610 :2941) | the channel's RC state (isolated, error notifier written) | `Channel` (channel.rs:195: `error_notifier` :213); mirror `HostChannel` (mirror.rs:132) and the RC notifier watched by the event thread (mirror.rs:156, NOTIFIER_RC_ERROR :63) | per-channel RC state; the device never sends RC_TRIGGERED, so the guest kernel reaches this only after the event exists (gsp-surface.md §4) | software channel: clear the error, back to idle; mirrored: forward the public 0x506F0105 to the host channel | guest gate: a CUDA kernel with an illegal address (fault gate) once RC events exist; unit: reset clears the registry state | RC |
| 7 | `NV506F_CTRL_CMD_INTERNAL_RESET_ISOLATED_CHANNEL` 0x506F0106 | KernelChannel, g_kernel_channel_nvoc.c:435 / :457 | `NV506F_CTRL_CMD_INTERNAL_RESET_ISOLATED_CHANNEL_PARAMS`, ctrl506f.h:99 | 8 / 8 | INTERNAL | kernel then GSP: sent by `kchannelCtrlCmdResetIsolatedChannel_IMPL` when a client calls 0x506F0105 (kernel_channel.c:3002 / 610 :2960) | as 0x506F0105 | as 0x506F0105 | as 0x506F0105 | as 0x506F0105; the host side takes the public 0x506F0105 (an INTERNAL control cannot be issued by a host user client) | as 0x506F0105 | RC |
| 8 | `NV9067_CTRL_CMD_SET_TPC_PARTITION_TABLE` 0x90670102 | KernelCtxShareApi, g_kernel_ctxshare_nvoc.c:398 / :400 | `NV9067_CTRL_TPC_PARTITION_TABLE_PARAMS`, ctrl9067.h:101 | 1026 / 1026 | kernel only | GSP (flags 0x40, kernel-privileged: only kernel clients may call it) | the context share's TPC table | `Registry` `ctxshares` (channel.rs:252) | a per-ctxshare record | record and echo (`cuda.rs` ACKNOWLEDGED list, cuda.rs:29) | unit echo | K |
| 9 | `NV9067_CTRL_CMD_GET_CWD_WATERMARK` 0x90670201 | KernelCtxShareApi, g_kernel_ctxshare_nvoc.c:413 / :415 | `NV9067_CTRL_CWD_WATERMARK_PARAMS`, ctrl9067.h:143 | 4 / 4 | kernel only | GSP (flags 0x40, kernel-privileged) | the context share's CWD watermark | `Registry` `ctxshares` (channel.rs:252) | a per-ctxshare record | the recorded value, else the chip default (A) | unit | K |
| 10 | `NV9067_CTRL_CMD_SET_CWD_WATERMARK` 0x90670202 | KernelCtxShareApi, g_kernel_ctxshare_nvoc.c:428 / :430 | `NV9067_CTRL_CWD_WATERMARK_PARAMS`, ctrl9067.h:143 | 4 / 4 | kernel only | GSP (flags 0x40, kernel-privileged) | as GET_CWD_WATERMARK | as above | as above | record | unit | K |
| 11 | `NV906F_CTRL_CMD_RESET_CHANNEL` 0x906F0102 | KernelChannel, g_kernel_channel_nvoc.c:465 / :487 | `NV906F_CTRL_CMD_RESET_CHANNEL_PARAMS`, ctrl906f.h:143 | 16 / 16 | NON_PRIVILEGED | kernel then GSP: `kchannelCtrlCmdResetChannel_IMPL` (kernel_channel.c:3021 / 610 :2979) fills `bIsRcPending` and sends on a GSP client (:3049 / 610 :3007); the RC watchdog sends it with `_FAKE_ERROR` (kernel_rc_watchdog_callback.c:88, both); the fault path after GET_MMU_DEBUG_MODE (615 kern_gmmu.c:3077, SR-IOV VF only; 610 kern_gmmu_gv100.c:2082 in `kgmmuServiceMmuFault_GV100`, reached after MMU_FAULT_QUEUED) | the channel's run state and RC state | `Channel` (channel.rs:195), mirror `HostChannel` (mirror.rs:132) | per-channel RC state; RC events (gsp-surface.md §4) | software channel: stop, drop the resume point, mark reset; mirrored: forward to the host channel (NON_PRIVILEGED on the host) | fault gate (CUDA illegal address, then context destroy); unit on the registry | RC |
| 12 | `NV906F_CTRL_CMD_GET_DEFER_RC_STATE` 0x906F0105 | KernelChannel, g_kernel_channel_nvoc.c:480 / :502 | `NV906F_CTRL_CMD_GET_DEFER_RC_STATE_PARAMS`, ctrl906f.h:171 | 1 / 1 | NON_PRIVILEGED | GSP (flags 0x48) | deferred RC pending on the channel | `Channel` (channel.rs:195) | no deferred-RC model | `bDeferRCPending` 0 | unit | RC |
| 13 | `NV906F_CTRL_CMD_GET_MMU_FAULT_INFO` 0x906F0106 | KernelChannel, g_kernel_channel_nvoc.c:495 / :517 | `NV906F_CTRL_GET_MMU_FAULT_INFO_PARAMS`, ctrl906f.h:214 / :213 | 104 / 104 | NON_PRIVILEGED | GSP (flags 0x10048); may bypass the RM lock (`rmapiRmControlCanBeBypassLock`, rmapi_specific.c:183-189, both), i.e. meant for the RC path | the last MMU fault of the channel (address, type, text) | nowhere: the mirror's event thread only counts the RC notifier (mirror.rs:1045); the software executor refuses launches (`launches_refused`, channel.rs) | a per-channel last-fault record (from the host's answer on the mirror channel, or from the device's own executor) | mirrored: forward to the host channel (the host fault address is the guest VA: the mirror runs in the guest's VA space); software: zeros | fault gate: the guest's Xid text shows the faulting VA | RC |
| 14 | `NV_EVENT_BUFFER_CTRL_CMD_UPDATE_GET` 0x90CD0102 | EventBuffer, g_event_buffer_nvoc.c:153 / :154 | `NV_EVENT_BUFFER_CTRL_CMD_UPDATE_GET_PARAMS`, ctrl90cd.h:124 | 8 / 8 | NON_PRIVILEGED | 615 only: `_goeimportIssueUpdateGet` (gpu_op_event_import.c:1260, send :1278) on the event buffer the 615 kernel allocates on GSP after state load (gpu.c:2796, `gpuOpEventImportConstruct_IMPL` :837); today the importer stops at step 3a with 0x56 (caraxes/runs/transport-615-2026-09-24/boot/guest.txt:36) because its sysmem is registered with `_REGISTER_MEMDESC_TO_PHYS_RM` (`_goeimportAllocSysmemRegion`, :295-324), i.e. RPC ALLOC_MEMORY, which the device does not answer | the event buffer the guest allocated (class 0x90CD) and its record/vardata get pointers | nowhere yet: the alloc is acknowledged and recorded by class only (ctrl.rs `Dispatcher::alloc`) | an event-buffer object (header, records, vardata in guest sysmem) with the device as the producer; depends on ALLOC_MEMORY (RPC 4, the RPC plan) | record the get pointers, NV_OK | 615 guest boot: no `goeimport` error in dmesg; unit on the object | EB (depends on ALLOC_MEMORY) |
| 15 | `NVA06C_CTRL_CMD_SET_INTERLEAVE_LEVEL` 0xA06C0107 | KernelChannelGroupApi, g_kernel_channel_group_api_nvoc.c:407 / :429 | `NVA06C_CTRL_INTERLEAVE_LEVEL_PARAMS`, ctrla06c.h:270 | 4 / 4 | NON_PRIVILEGED | kernel then GSP: `kchangrpapiCtrlCmdSetInterleaveLevel_IMPL` (kernel_channel_group_api.c:1404 / 610 :1379; send :1431 / 610 :1406); a failure turns into NOT_SUPPORTED for the caller | the TSG's interleave level | `Registry` `tsgs` (channel.rs:249), `HostTsg` (mirror.rs:122) | a per-TSG level | record; forward to the host TSG when mirrored (NON_PRIVILEGED on the host); levels above MEDIUM stay recorded only (isolation: no guest priority over the host desktop) | unit; CUDA stream-priority gate (Q-C2) | M |
| 16 | `NVA06C_CTRL_CMD_PROGRAM_VIDMEM_PROMOTE` 0xA06C0109 | KernelChannelGroupApi, g_kernel_channel_group_api_nvoc.c:422 / :444 | `NVA06C_CTRL_PROGRAM_VIDMEM_PROMOTE_PARAMS`, ctrla06c.h:323 | 8 / 8 | NON_PRIVILEGED | GSP (flags 0x48) | the TSG's vidmem promotion | as SET_INTERLEAVE_LEVEL | per-TSG record | record; forward when mirrored | unit | M |
| 17 | `NVA06C_CTRL_CMD_MAKE_REALTIME` 0xA06C0110 | KernelChannelGroupApi, g_kernel_channel_group_api_nvoc.c:452 / :474 | `NVA06C_CTRL_MAKE_REALTIME_PARAMS`, ctrla06c.h:399 | 1 / 1 | NON_PRIVILEGED | GSP (flags 0x48) | the TSG's realtime flag | `Registry` `tsgs` | per-TSG flag | record only, never forwarded: a realtime guest TSG would preempt the host desktop (P7) | unit | L |
| 18 | `NVA06C_CTRL_CMD_INTERNAL_GPFIFO_SCHEDULE` 0xA06C0201 | KernelChannelGroupApi, g_kernel_channel_group_api_nvoc.c:467 / :489 | `NVA06C_CTRL_GPFIFO_SCHEDULE_PARAMS`, ctrla06c.h:101 | 3 / 3 | INTERNAL | never on a GSP client: `kchangrpapiCtrlCmdGpFifoSchedule_IMPL` (kernel_channel_group_api.c:1091 / 610 :1066) sends the public 0xA06C0101 under `IS_VIRTUAL // IS_GSP_CLIENT` and the INTERNAL form only otherwise (:1227 / 610 :1202) | - | - | - | none needed (E); NOT_SUPPORTED if it arrives | - | X |
| 19 | `NVA06C_CTRL_CMD_INTERNAL_SET_TIMESLICE` 0xA06C0202 | KernelChannelGroupApi, g_kernel_channel_group_api_nvoc.c:482 / :504 | `NVA06C_CTRL_TIMESLICE_PARAMS`, ctrla06c.h:148 | 8 / 8 | INTERNAL | never on a GSP client: `kchangrpapiCtrlCmdSetTimeslice_IMPL` (kernel_channel_group_api.c:1322 / 610 :1297) sends the public 0xA06C0103 on a GSP client and the INTERNAL form only otherwise (:1375 / 610 :1350) | - | - | - | none needed (E) | - | X |
| 20 | `NVA06F_CTRL_CMD_SET_INTERLEAVE_LEVEL` 0xA06F0109 | KernelChannel, g_kernel_channel_nvoc.c:555 / :577 | `NVA06F_CTRL_INTERLEAVE_LEVEL_PARAMS`, ctrla06fgpfifo.h:146 | 4 / 4 | NON_PRIVILEGED | kernel then GSP: `kchannelCtrlCmdSetInterleaveLevel_IMPL` (kernel_channel.c:3215 / 610 :3180; send :3229 / 610 :3194) | the channel's interleave level | `Channel` (channel.rs:195), `HostChannel` (mirror.rs:132) | a per-channel level | as the TSG form | unit | M |
| 21 | `NVA06F_CTRL_CMD_RESTART_RUNLIST` 0xA06F0111 | KernelChannel, g_kernel_channel_nvoc.c:570 / :592 | `NVA06F_CTRL_RESTART_RUNLIST_PARAMS`, ctrla06fgpfifo.h:211 | 2 / 2 | NON_PRIVILEGED | GSP (flags 0x48) | the channel's runlist | `Channel` | nothing for a software channel | NV_OK, recorded; not forwarded (a runlist restart on the host acts on every tenant of that runlist) | unit | L |
| 22 | `NVB06F_CTRL_CMD_GET_ENGINE_CTX_SIZE` 0xB06F010B | KernelChannel, g_kernel_channel_nvoc.c:615 / :637 | `NVB06F_CTRL_GET_ENGINE_CTX_SIZE_PARAMS`, ctrlb06f.h:82 | 8 / 8 | NON_PRIVILEGED | GSP (flags 0x10048) | engine context size (live migration) | chip data would do (engine context properties) | migration is not a goal | NOT_SUPPORTED, label `refused (no live migration)` | unit | V |
| 23 | `NVB06F_CTRL_CMD_GET_ENGINE_CTX_DATA` 0xB06F010C | KernelChannel, g_kernel_channel_nvoc.c:630 / :652 | `NVB06F_CTRL_GET_ENGINE_CTX_DATA_PARAMS`, ctrlb06f.h:118 | 16 / 16 | NON_PRIVILEGED | GSP (flags 0x10048); embedded pointer `pEngineCtxBuff` (embedded_param_copy.c:699, :1140) | engine context contents | - | migration | NOT_SUPPORTED | - | V |
| 24 | `NVB06F_CTRL_CMD_MIGRATE_ENGINE_CTX_DATA` 0xB06F010D | KernelChannel, g_kernel_channel_nvoc.c:645 / :667 | `NVB06F_CTRL_MIGRATE_ENGINE_CTX_DATA_PARAMS`, ctrlb06f.h:150 | 16 / 16 | PRIVILEGED | GSP, PRIVILEGED; embedded pointer (embedded_param_copy.c:687, :1132) | engine context contents | - | migration | NOT_SUPPORTED | - | V |
| 25 | `NVB06F_CTRL_CMD_GET_ENGINE_CTX_STATE` 0xB06F010E | KernelChannel, g_kernel_channel_nvoc.c:660 / :682 | `NVB06F_CTRL_GET_ENGINE_CTX_STATE_PARAMS`, ctrlb06f.h:179 | 12 / 12 | NON_PRIVILEGED | GSP (flags 0x10048) | engine context state | - | migration | NOT_SUPPORTED | - | V |
| 26 | `NVB06F_CTRL_CMD_GET_CHANNEL_HW_STATE` 0xB06F010F | KernelChannel, g_kernel_channel_nvoc.c:675 / :697 | `NVB06F_CTRL_GET_CHANNEL_HW_STATE_PARAMS`, ctrlb06f.h:254 | 4 / 4 | NON_PRIVILEGED | GSP (flags 0x10048) | channel hardware state | - | migration | NOT_SUPPORTED | - | V |
| 27 | `NVB06F_CTRL_CMD_SET_CHANNEL_HW_STATE` 0xB06F0110 | KernelChannel, g_kernel_channel_nvoc.c:690 / :712 | `NVB06F_CTRL_SET_CHANNEL_HW_STATE_PARAMS`, ctrlb06f.h:304 | 4 / 4 | PRIVILEGED | GSP, PRIVILEGED | channel hardware state | - | migration | NOT_SUPPORTED | - | V |
| 28 | `NVB06F_CTRL_CMD_SAVE_ENGINE_CTX_DATA` 0xB06F0111 | KernelChannel, g_kernel_channel_nvoc.c:705 / :727 | `NVB06F_CTRL_SAVE_ENGINE_CTX_DATA_PARAMS`, ctrlb06f.h:337 | 4104 / 4104 | NON_PRIVILEGED | GSP (flags 0x10048) | engine context contents (4 KiB inline) | - | migration | NOT_SUPPORTED | - | V |
| 29 | `NVB06F_CTRL_CMD_RESTORE_ENGINE_CTX_DATA` 0xB06F0112 | KernelChannel, g_kernel_channel_nvoc.c:720 / :742 | `NVB06F_CTRL_RESTORE_ENGINE_CTX_DATA_PARAMS`, ctrlb06f.h:355 | 4104 / 4104 | PRIVILEGED | GSP, PRIVILEGED | engine context contents | - | migration | NOT_SUPPORTED | - | V |
| 30 | `NVC365_CTRL_CMD_ACCESS_CNTR_BUFFER_RESET_COUNTERS` 0xC3650108 | AccessCounterBuffer, g_access_cntr_buffer_nvoc.c:318 / :319 | `NVC365_CTRL_ACCESS_CNTR_BUFFER_RESET_COUNTERS_PARAMS`, ctrlc365.h:249 | 8 / 8 | kernel only | GSP (flags 0x40, kernel-privileged); the kernel body `accesscntrCtrlCmdAccessCntrBufferResetCounters_IMPL` (access_cntr_buffer_ctrl.c:179, both) is not reached on a GSP client | the access counter buffer UVM registered | `uvm::UvmBuffers` `access_counter_buffer` (uvm.rs:95-103) | counter state (the device has no access counters) | `resetFlag` 1 when a buffer is registered, NV_OK | unit on `UvmBuffers` | K |
| 31 | `NVC36F_CTRL_CMD_GPFIFO_UPDATE_FAULT_METHOD_BUFFER` 0xC36F0109 | KernelChannel, g_kernel_channel_nvoc.c:750 / :772 | `NVC36F_CTRL_GPFIFO_UPDATE_FAULT_METHOD_BUFFER_PARAMS`, ctrlc36f.h:107 | 16 / 16 | PRIVILEGED | GSP (flags 0x10244: PRIVILEGED, ROUTE_TO_VGPU_HOST, GSP plugin): a vGPU-plugin control, no caller in either tree | the fault method buffers of the channel | promoted fault method buffers (uvm.rs:311, `INTERNAL_PROMOTE_FAULT_METHOD_BUFFERS`) | nothing needed | record `bar2Addr`, NV_OK | unit | K |

## 3. The twelve controls moved here from category B

| # | Control, id | Derived from | Where it lives in the device | Missing | Answer | Test |
|---|---|---|---|---|---|---|
| B1 | `NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY` 0x90960107 (56 bytes both; `index` @44, `bIndexValid` @48, `tableType` @52) | the ZBC clear table the guest sees: chip defaults plus its own `SET_ZBC_COLOR/DEPTH/STENCIL_CLEAR` | nowhere: `GF100_ZBC_CLEAR` allocations are acknowledged by class only (ctrl.rs `Dispatcher::alloc`) | a `ZbcTable` per device (RM keeps one table per GPU), seeded from chip data (b-host-forwardable.md Q-B3); the setters (category D) record into it | the entry at `index` of `tableType`, `bIndexValid` 0 for an empty slot; never the host's table, which holds the host desktop's clear values. With D16 no guest surface is compressible, so the hardware never uses a ZBC entry for guest surfaces, and a table the guest reads from the device is safe | unit: seed, set, get; guest gate: the display gate (weston or mutter on EGL) with no `0x9096` control unanswered |
| B2 | `NV0090_CTRL_CMD_GET_MMU_DEBUG_MODE` 0x00900105 (1 byte) | the GR context's MMU debug mode | `Registry` `tsgs` (channel.rs:249) | a per-TSG debug flag, set by the SM debugger controls (category D) | mirrored TSG: `Mirror::forward_control`; otherwise `bMode` 0 | unit |
| B3 | `NV2080_CTRL_CMD_GR_GET_CTXSW_STATS` 0x20801215 (32 bytes, `hChannel` @0) | the channel's context-switch counters | `Channel` (channel.rs:195), `HostChannel.handle` (mirror.rs:132-134) | a params rewrite (guest `hChannel` to the host handle) and a call on the mirror's subdevice | mirrored: host counters; software channel: zeros (it never context-switches) | unit with a fake host; cuda gate |
| B4 | `NV2080_CTRL_CMD_GR_GET_CTXSW_MODES` 0x20801235 (24 bytes, `hChannel` @0) | the channel's zcull, PM, SMPC and preemption modes | as B3; the zcull mode is recorded by `GR_CTXSW_ZCULL_BIND` (ctrl.rs, `guest (recorded ...)`) | as B3, plus the recorded preemption modes (`GR_SET_CTXSW_PREEMPTION_MODE` is echoed today, cuda.rs:29) | mirrored: host modes; software: the recorded values | unit |
| B5 | `NV2080_CTRL_CMD_GR_GET_CURRENT_RESIDENT_CHANNEL` 0x2080121C (24 bytes) | which guest channel is on GR now | `Mirror` `channels` by guest ChID (mirror.rs:173); `chid_of_host_token` (mirror.rs:915) | a host-ChID to guest-ChID map | the guest ChID if the host's resident ChID is one of this VM's mirrors, else `chID` 0xFFFFFFFF; never another tenant's ChID | unit |
| B6 | `NV2080_CTRL_CMD_RC_GET_ERROR_COUNT` 0x20802205 (4 bytes) | RC errors the device reported to this guest | nowhere: the device sends no RC_TRIGGERED yet (gsp-surface.md §4) | an RC journal fed by the future `emit_event` path (the RPC and event plan) | the journal's count, 0 today | unit; fault gate once RC events exist |
| B7 | `NV2080_CTRL_CMD_RC_GET_ERROR_V2` 0x20802213 (8200 bytes) | the journal's records | as B6 | as B6, with the record format of `whichBuffer` | `outputRecordSize` 0 today | unit |
| B8 | `NV208F_CTRL_CMD_FIFO_GET_CHANNEL_STATE` 0x208F0403 (16 bytes; `hChannel` @0, `hClient` @4) | the channel's enabled/scheduled state | `Channel` (channel.rs:195), found by `(hClient, hChannel)` from the params | a scheduled flag per channel (the GPFIFO_SCHEDULE answer, uvm.rs:294, records none) | `bEnabled`/`bScheduled` from the registry, `bDeferRC` 0; the kernel fills `bCpuMap` and `bRunlistSet` itself (kernel_fifo_ctrl.c:652-653) | unit |
| B9 | `NV83DE_CTRL_CMD_DEBUG_GET_MODE_MMU_DEBUG` 0x83DE0308 | the debugger session's mode | nowhere: `GT200_DEBUGGER` is acknowledged by class only | a debugger-session record (or none while debugging is refused) | default 0 (disabled), consistent with D rule 4 | unit |
| B10 | `NV83DE_CTRL_CMD_DEBUG_GET_MODE_ERRBAR_DEBUG` 0x83DE0320 | as B9 | as B9 | as B9 | 0 | unit |
| B11 | `NV83DE_CTRL_CMD_DEBUG_GET_SINGLE_SM_DEBUGGER_STATUS` 0x83DE0325 | per-SM debugger state | as B9 | as B9 | zeros | unit |
| B12 | `NV83DE_CTRL_CMD_DEBUG_GET_MODE_MMU_GCC_DEBUG` 0x83DE032B | as B9 | as B9 | as B9 | 0 | unit |

The route, flags, export lines and header lines of these twelve are in the table of
b-host-forwardable.md §2.

## 4. The mechanical form

Most of the plan is one table plus one record. The table is kept in the version module next to
`MIRROR_FORWARDED` and replaces that list:

```rust
/// A control on a guest object (gsp-plan/c-guest-state.md).
pub struct GuestControl {
    pub id: u32,
    pub name: &'static str,
    pub size: usize,
    /// Which guest object the handle (or a params handle) names.
    pub object: GuestObj,       // Tsg | Channel | TsgOrChannel | CtxShare | UvmAccessCntr
    /// Which attribute of the object's record it reads or writes.
    pub attr: Attr,             // InterleaveLevel | TpcPartition | Promote(kind) | Realtime | ...
    /// Forward to the host twin when there is one (flags NON_PRIVILEGED on the host).
    pub mirror: bool,
    /// What a software object answers.
    pub software: SoftAnswer,   // Record | RecordedValue | Zeros | NotSupported
}
```

The record is an `ObjAttrs` map in `channel::Registry`, keyed like `tsgs` by
`(hClient, hObject)`, resolved through `Registry::resolve`, and freed in `record_free`
(channel.rs:625). It holds `interleave`, `tpc_partition`, `promote` (vidmem l1, t1, sysmem, lg
sector), `realtime`, `cwd_watermark`, `mmu_debug`, `rc` (pending, isolated, reason) and
`last_fault`.

Groups M, L and K become rows of the table. Group V becomes one `Refuse` rule for the NVB06F
family. Group X needs nothing. RC and EB need behaviour: a reset puts a software channel back to
idle, and the event buffer needs an object.

Worked example, `NVA06C_CTRL_CMD_SET_INTERLEAVE_LEVEL` 0xA06C0107:

- Params: `{NvU32 tsgInterleaveLevel}`, 4 bytes in both trees.
- Row: `GuestControl { id: 0xA06C_0107, name: "TSG_SET_INTERLEAVE_LEVEL", size: 4, object: Tsg, attr: InterleaveLevel, mirror: true, software: Record }`.
- The dispatcher resolves the TSG, stores the level, and forwards it to the host TSG when one
  exists. It answers NV_OK. A level above MEDIUM is stored but not forwarded, so a guest cannot
  outrank the host desktop on the runlist (P7).
- The guest kernel turns any failure into NOT_SUPPORTED for its caller
  (kernel_channel_group_api.c:1438), so the answer matters to the caller.

## 5. What is missing, by device module

| Module (crates/vfio-user-nvgpu/src) | Missing | For |
|---|---|---|
| channel.rs `Registry` | per-object attribute record (`ObjAttrs`), a scheduled flag per channel, RC state per channel, last MMU fault per channel | M, L, K, RC, B8 |
| mirror.rs `Mirror` | a params-handle rewrite for subdevice controls naming a mirrored channel (`hChannel` to the host handle, issued on the mirror's subdevice), a host-ChID to guest-ChID map, the last fault read back from the host channel when the RC notifier fires (mirror.rs:1040-1051) | B3, B4, B5, 906F0106 |
| a new `zbc.rs` | the device's ZBC table (chip defaults plus the guest's entries) | B1, and the D setters |
| a new event-buffer object (with the RPC plan's ALLOC_MEMORY) | the 0x90CD object: header, record ring and vardata in guest sysmem, producer side in the device | 90CD0102 |
| the RC journal (with the RPC plan's `emit_event`) | the list of RC errors the device reported | B6, B7, RC |

## 6. Order of implementation

Client likelihood first.

1. **The RC family:** `NV906F_CTRL_CMD_RESET_CHANNEL`, `NV506F_CTRL_CMD_RESET_ISOLATED_CHANNEL`,
   its INTERNAL form, `NV906F_CTRL_CMD_GET_MMU_FAULT_INFO` and `NV906F_CTRL_CMD_GET_DEFER_RC_STATE`.
   Any CUDA or GL fault ends here: gsp-surface.md §6 row 8, and the watchdog at
   kernel_rc_watchdog_callback.c:88. It comes together with the RC_TRIGGERED and OS_ERROR_LOG
   events of the RPC plan, which are what make the guest kernel send the reset. Without the
   events, a fault is a timeout (gsp-surface.md §4).
2. **`NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY` (B1).** Every EGL, GL and Vulkan client; the
   next display wall once category A answers `GET_ZBC_CLEAR_TABLE_SIZE`
   (b-host-forwardable.md §5 item 1). Placed second only because the RC family blocks every
   CUDA error path. If the display gate comes first, swap the two.
3. **`NV_EVENT_BUFFER_CTRL_CMD_UPDATE_GET`.** Every 615 boot builds the operational-event
   importer (src/nvidia/src/kernel/gpu/gpu.c:2796). Today it fails at step 3a
   (caraxes/runs/transport-615-2026-09-24/boot/guest.txt:36) because ALLOC_MEMORY is
   unanswered. It becomes live the moment the RPC plan answers ALLOC_MEMORY, together with the
   0x90CD and 0x90D0 allocations of the importer (gpu_op_event_import.c:633-690, steps 5a to 8).
4. **The interleave pair and the rest of group M:** `SET_INTERLEAVE_LEVEL` (TSG and channel),
   the NV0090 TPC and promotion controls, and `NVA06C_CTRL_CMD_PROGRAM_VIDMEM_PROMOTE`. These
   are setters that CUDA or GL may send (Q-C2). One table and one record cover all eight.
5. **B8 and B2 to B5.** Channel-state and GR-statistics lookups, for diag tools, profilers and
   cuda-gdb.
6. **Groups L and K and the debugger defaults (B9 to B12):** record-and-echo rows.
7. **Groups V and X:** one refusal rule, and nothing.

## 7. Open questions for category C, and the measurement that settles each

- **Q-C1. Which controls follow an RC event?** Source shows the senders (kernel_rc_callback.c:88,
  :198, :414; kernel_channel.c:3049), but not which of them a given guest client triggers after
  a fault. Measurement: the fault gate (a CUDA kernel with an illegal address) once the device
  sends RC_TRIGGERED, with the device logging every control. No extra host touch: the fault
  happens on the device's own software or mirror path.
- **Q-C2. Does libcuda set interleave levels or TPC partition modes?** None of these ids is in
  the native CUDA traces (matrix/traces/610.57.04/control/cuda-*.jsonl), but those probes create
  no prioritized streams. Measurement: a native user-level trace of a program that creates
  streams with `cudaStreamCreateWithPriority` and runs under MPS-free default settings. This
  needs the operator's go-ahead, like every native probe. Alternatively, and without host
  touch, the same program in the guest with the device logging.
- **Q-C3. What `GET_MMU_FAULT_INFO` returns on a mirrored channel.** The mirror runs in the
  guest's VA space, so the host's fault address should be the guest VA. Measurement: the fault
  gate on a mirrored CE or GR channel, comparing the host answer with the VA the test program
  touched.
- **Q-C4. The 615 event buffer after ALLOC_MEMORY.** Which allocations and controls follow step
  3a when it succeeds? Source lists steps 4 to 8 (gpu_op_event_import.c:625-690: bind alias,
  EventBuffer alloc on GSP, CPU mappings, ENABLE_EVENTS, the 0x90D0 bind). Measurement: a 615
  guest boot with ALLOC_MEMORY answered and the device logging.
- **Q-C5. Does a 610 GSP-client kernel service non-replayable faults on the CPU?** In 610, the
  MMU_FAULT_QUEUED event queues the fault handler (kernel_gsp.c:1002, :1451). That handler
  reaches `kgmmuServiceMmuFault_GV100` (kern_gmmu_gv100.c:2005), which sends
  `GET_MMU_DEBUG_MODE` and `RESET_CHANNEL` (:2042, :2082). The 615 dispatcher no longer
  handles the event, and the code became `_VF`. Measurement: a 610 guest test in which the
  device sends MMU_FAULT_QUEUED for a faulting software channel, with the device logging which
  controls follow.
- **Q-C6. Host answers for the mirror forwards.** The host flags of the M-group controls are
  NON_PRIVILEGED (see the table), but whether the host accepts them on a TSG allocated by
  another client's VA space (the replayed mirror) is not visible in source. Measurement: the
  device's own mirror TSG in the cuda gate, one `SET_INTERLEAVE_LEVEL` and one
  `SET_TPC_PARTITION_MODE` with `NONE`, logging the host status. This acts only on the
  device's own host objects.
  Status: open. The forwards exist since b49cc5c, but no GPU gate could run on 2026-09-24
  (§8).
- **Q-C7. Policy for MAKE_REALTIME and RESTART_RUNLIST.** Recording without forwarding is an
  isolation choice: a guest must not raise its priority over the host desktop or restart a
  shared runlist. That is for the operator to confirm (P7).
  Status: decided 2026-09-24. Recorded, never forwarded (§8).

## 8. Status (2026-09-24)

Implemented in `crates/vfio-user-nvgpu` in commit b49cc5c.

- **Where it lives.** The C rows are the table `PUBLIC_C` in `src/ver/v610/public.rs`, answered
  by `src/public.rs`.
- **The record.** Each guest object has a record: `public::GuestAttrs`, keyed like the
  registry by `(hClient, hObject)` with duplicates resolved, and freed with its object in
  `Dispatcher::free`. It keeps the last parameter block of every setter, the reset count and
  the last fault. It does not live in `channel::Registry`, so that change stays out of the
  channel code.
- **The only host path.** It is the object's own mirror: `Mirror::forward_control`, which
  finds the host TSG by the guest's TSG key and the host channel through the registry by the
  guest's channel handle. No C row uses the device's host RM client. The unit tests drive
  every row with a host RM that fails the test when called and with a fake mirror that
  records each forward.

**Counts.**

| Group | Rows | Done | Partial | Open |
|---|---:|---:|---:|---:|
| M | 8 | 8 | 0 | 0 |
| RC | 5 | 2 | 3 | 0 |
| L | 2 | 2 | 0 | 0 |
| K | 5 | 5 | 0 | 0 |
| X | 2 | 2 | 0 | 0 |
| V | 8 | 8 | 0 | 0 |
| EB | 1 | 1 | 0 | 0 |
| **§2 total** | **31** | **28** | **3** | **0** |
| from B (§3) | 12 | 11 | 0 | 1 |
| from D (d-device-model.md §10) | 17 | 14 | 3 | 0 |

(EB, B2-B5, B8-B12 and the D rows: round 2 below.)

**Decision Q-C7, taken 2026-09-24 by the operator.** `NVA06C_CTRL_CMD_MAKE_REALTIME` and
`NVA06F_CTRL_CMD_RESTART_RUNLIST` are recorded and never forwarded. A guest must not outrank
the host desktop, nor restart a runlist the host shares with other tenants. The same rule caps
the forwarded interleave level at MEDIUM: a higher level is recorded only.

**Changes against the plan's recipes.**

- **The K rows are refused.** The plan's recipe was to record and echo; the refusal is the
  operator's instruction. None has a caller on a GSP client.
- **The RC rows are partial.**
  - A software channel's reset is recorded (reset count, last fault cleared) and answered
    NV_OK. The software executor's own state (the resume point) is not reset; that needs a
    reset entry point in channel.rs.
  - `GET_MMU_FAULT_INFO` answers from the device's fault record. Nothing fills that record
    until the RC events of the RPC plan exist, so it answers zeros.
  - Q-C3 (what the host returns on a mirrored channel) stays open. The device does not forward
    this getter.
- **A reset on the host resets an engine.** `RESET_CHANNEL` "resets the channel ... and also
  resets the specified engine" (ctrl906f.h:105-124). Forwarded, it acts only on the device's
  own host channel object, as it would for any host process on its own channel. But
  `engineID` is the guest's, and the host GSP's engine reset can still disturb other tenants
  of that engine, the host desktop included. This was not measured, because no GPU run was
  possible. Decision at the merge (2026-09-24, main): the three reset controls are never
  forwarded; the device records the reset and answers NV_OK, the mirror channel is left alone
  (`public.rs`, `Guest::Reset`). A guest-named engine reset must not reach a host engine; if
  the fault gate (Q-C1) shows that a guest needs the real reset, it goes through the device's
  own RC path on its own mirror channel, with the engine taken from the mirror, not from the
  guest.
- **The M rows have not been seen on the card.** Their mirror forwards were not exercised,
  because the GPU gates could not run (see b-host-forwardable.md §8, Verification), so Q-C6
  is still open. The guest sees NV_OK and the host status goes to the log, as for
  `MIRROR_FORWARDED`.
- **`NV_EVENT_BUFFER_CTRL_CMD_UPDATE_GET` is open.** It depends on `ALLOC_MEMORY`. It belongs
  to the event-buffer model the RPC plan builds and has no row in the table (a test pins
  that).

**Per row (§2).**

| # | Control | Group | Status | Commit | Note |
|---|---|---|---|---|---|
| 1 | `NV0090_CTRL_CMD_SET_TPC_PARTITION_MODE` | M | done | b49cc5c | recorded; forwarded to the own mirror TSG or channel (Q-C6 open) |
| 2 | `NV0090_CTRL_CMD_GET_TPC_PARTITION_MODE` | M | done | b49cc5c | the recorded mode, NONE before |
| 3 | `NV0090_CTRL_CMD_PROGRAM_VIDMEM_PROMOTE` | M | done | b49cc5c | recorded, forwarded when mirrored |
| 4 | `NV0090_CTRL_CMD_SET_LG_SECTOR_PROMOTION` | M | done | b49cc5c | recorded, forwarded when mirrored |
| 5 | `NV0090_CTRL_CMD_PROGRAM_SYSMEM_PROMOTE` | M | done | b49cc5c | recorded, forwarded when mirrored (615 only) |
| 6 | `NV506F_CTRL_CMD_RESET_ISOLATED_CHANNEL` | RC | partial | b49cc5c | own mirror channel; without a mirror recorded only, software executor not reset |
| 7 | `NV506F_CTRL_CMD_INTERNAL_RESET_ISOLATED_CHANNEL` | RC | partial | b49cc5c | as 6, sent to the host as the public 0x506F0105 |
| 8 | `NV9067_CTRL_CMD_SET_TPC_PARTITION_TABLE` | K | done | b49cc5c | refused |
| 9 | `NV9067_CTRL_CMD_GET_CWD_WATERMARK` | K | done | b49cc5c | refused |
| 10 | `NV9067_CTRL_CMD_SET_CWD_WATERMARK` | K | done | b49cc5c | refused |
| 11 | `NV906F_CTRL_CMD_RESET_CHANNEL` | RC | partial | b49cc5c | as 6 |
| 12 | `NV906F_CTRL_CMD_GET_DEFER_RC_STATE` | RC | done | b49cc5c | bDeferRCPending 0 |
| 13 | `NV906F_CTRL_CMD_GET_MMU_FAULT_INFO` | RC | done | b49cc5c, round 2 | the device's fault record: for an Xid 31 the device raised, the host's record of the channel's own mirror (round 2) |
| 14 | `NV_EVENT_BUFFER_CTRL_CMD_UPDATE_GET` | EB | done | 29f3366, round 2 | gspmem.rs: the drain's get pointers, bounded by the ring; clears the producer's notify latch; the records are the device's own operational events (round 2) |
| 15 | `NVA06C_CTRL_CMD_SET_INTERLEAVE_LEVEL` | M | done | b49cc5c | recorded; forwarded up to MEDIUM |
| 16 | `NVA06C_CTRL_CMD_PROGRAM_VIDMEM_PROMOTE` | M | done | b49cc5c | recorded, forwarded when mirrored |
| 17 | `NVA06C_CTRL_CMD_MAKE_REALTIME` | L | done | b49cc5c | recorded, never forwarded (Q-C7) |
| 18 | `NVA06C_CTRL_CMD_INTERNAL_GPFIFO_SCHEDULE` | X | done | b49cc5c | refused |
| 19 | `NVA06C_CTRL_CMD_INTERNAL_SET_TIMESLICE` | X | done | b49cc5c | refused |
| 20 | `NVA06F_CTRL_CMD_SET_INTERLEAVE_LEVEL` | M | done | b49cc5c | recorded; forwarded up to MEDIUM |
| 21 | `NVA06F_CTRL_CMD_RESTART_RUNLIST` | L | done | b49cc5c | recorded, never forwarded (Q-C7) |
| 22-29 | `NVB06F_CTRL_CMD_*_ENGINE_CTX_*`, `*_CHANNEL_HW_STATE` | V | done | b49cc5c | refused (no live migration) |
| 30 | `NVC365_CTRL_CMD_ACCESS_CNTR_BUFFER_RESET_COUNTERS` | K | done | b49cc5c | refused |
| 31 | `NVC36F_CTRL_CMD_GPFIFO_UPDATE_FAULT_METHOD_BUFFER` | K | done | b49cc5c | refused |

Row 13 counts as done: it answers from the device's fault state, as the plan asks, and the
state has no source yet. Rows 6, 7 and 11 are partial because the software path records the
reset but does not act on it.

**Per row (§3).**

| # | Control | Status | Commit | Note |
|---|---|---|---|---|
| B1 | `NV9096_CTRL_CMD_GET_ZBC_CLEAR_TABLE_ENTRY` | open | - | needs a device ZBC table; its defaults need Q-B3, not permitted in this package |
| B2 | `NV0090_CTRL_CMD_GET_MMU_DEBUG_MODE` | done | round 2 | `bMode` 0: the device never enables MMU debug; not forwarded (the host TSG's mode is the same default) |
| B3 | `NV2080_CTRL_CMD_GR_GET_CTXSW_STATS` | done | round 2 | mirror: `hChannel` rewritten to the own host channel on the mirror's subdevice; software: zeros |
| B4 | `NV2080_CTRL_CMD_GR_GET_CTXSW_MODES` | done | round 2 | mirror: as B3; software: recorded zcull and preemption modes |
| B5 | `NV2080_CTRL_CMD_GR_GET_CURRENT_RESIDENT_CHANNEL` | done | round 2 | guest ChID only for this VM's own mirror (host read, reported), else 0xFFFFFFFF |
| B6 | `NV2080_CTRL_CMD_RC_GET_ERROR_COUNT` | done | b49cc5c, reconciliation | the count of the device's one RC journal (rc.rs `RcState`, since the reconciliation of 2026-09-25) |
| B7 | `NV2080_CTRL_CMD_RC_GET_ERROR_V2` | done | b49cc5c, reconciliation | record `whichBuffer` of that journal: the `RmProtoBuf_RECORD` header, no error blocks (kernel_rc_ctrl.c:186-283); outputRecordSize 0 past the last |
| B8 | `NV208F_CTRL_CMD_FIFO_GET_CHANNEL_STATE` | done | round 2 | the device's scheduling flags |
| B9 | `NV83DE_CTRL_CMD_DEBUG_GET_MODE_MMU_DEBUG` | done | reconciliation | no C row: dmodel.rs answers it from the debugger object's record its setter fills (d-device-model.md §10, 5699cde); the C row's refusal had shadowed it |
| B10 | `NV83DE_CTRL_CMD_DEBUG_GET_MODE_ERRBAR_DEBUG` | done | round 2 | DISABLED (its setter is refused) |
| B11 | `NV83DE_CTRL_CMD_DEBUG_GET_SINGLE_SM_DEBUGGER_STATUS` | done | round 2 | refused NOT_SUPPORTED: needs SM run control of a session |
| B12 | `NV83DE_CTRL_CMD_DEBUG_GET_MODE_MMU_GCC_DEBUG` | done | round 2 | refused NOT_SUPPORTED: not on TU106 |

### Round 2 (2026-09-25)

Commits 672d561 (TSG key), 5591b9c (IDLE_CHANNELS and the mirror's host calls), e52824f (the
17 D-plan rows), 8989bbc (the B rows), 1f8d564 (operational-event records); runs in
`caraxes/runs/gsp-round2-c-2026-09-24/`. Everything lives where §8 says: rows of `PUBLIC_C`
(`src/ver/v610/public.rs`), handlers in `src/public.rs`, the host side only through the
guest object's own mirror (`public::MirrorObjects`, implemented by `mirror.rs`).

**What the rows keep.** `GuestAttrs` gained the device-wide channel state: a per-channel
`disabled` flag, the usermode and active-channel scheduling flags, and the RC journal
(`RcRecord`, at most 256 entries: the RCs the device raised, boot.rs, and the guest's reports).
`RC_GET_ERROR_COUNT` counts it, `RC_SET_CLEAN_ERROR_HISTORY` empties it. (Superseded by the
reconciliation below: the journal and the flags are rc.rs's.) The public context
now carries the channel registry (for rows that name a channel in their params) and the
emulated Link Control register (config.rs, set by device.rs at every config write).

**The debugger getters (B9-B12) and the session model.** B10 has a device answer because its
setter is refused. B9 and B11 are refused because their answer is session state the device
does not model. The minimal session model of the D plan's R5 would need, per `GT200_DEBUGGER`
object: the guest object it debugs (`hAppClient`, `hClass3dObject` of its alloc params), the
MMU debug action last set by `SET_MODE_MMU_DEBUG` (ENABLE/DISABLE, cuda-gdb sends ENABLE right
after the alloc), freed with the object; B9 then answers that action. B11 needs more: SM debug
mode on the session's context and the stop/trap state of each SM, which only a mirrored
session (the host `GT200_DEBUGGER` on the mirror's compute object) could report; until then it
stays refused. B12 is Blackwell-only.

**The 17 rows of d-device-model.md §10.**

| Id | Control | Status | Answer |
|---|---|---|---|
| 0x00801714 | `NV0080 FIFO_IDLE_CHANNELS` | done | each handle resolved in the caller's client (0x33 otherwise); software channels idle; mirrored ones through the host RM's `FIFO_IDLE_CHANNELS` on the mirror's own device and channels, timeout capped at 50 ms (the RPC's recipe, rpc-functions.md) |
| 0x2080110B | `FIFO_DISABLE_CHANNELS` | done (D model) | no C row since the reconciliation: rc.rs `disable_channels` (rc.rs:299) answers, the channel's flag in rc.rs's one set of flags, `GPFIFO_SCHEDULE` and `PREEMPT` on the own mirrors; the software executor holds the channel's doorbells |
| 0x20801117 | `FIFO_DISABLE_USERMODE_CHANNELS` | done | the device's usermode flag, `RcState::usermode_disabled` (channels of a guest TSG; the kernel's bare channels stay); mirrored ones on their own twins; the software executor holds the doorbells of a disabled channel and replays them at re-enable (ctrl.rs:379-420) |
| 0x20800AC3 | `INTERNAL_FIFO_TOGGLE_ACTIVE_CHANNEL_SCHEDULING` | done | the device-wide flag, `RcState::scheduling_disabled`; every mirror channel on its own twin (scheduling only); software channels as above |
| 0x208F0401 | `NV208F FIFO_CHECK_ENGINE_CONTEXT` | done | `exists` from the GR context the guest promoted for the channel or its TSG; engine other than GRAPHICS 0x1F, unknown channel 0x21 |
| 0x20801205 | `GR_CTXSW_ZCULL_MODE` | done | recorded on the named channel, never forwarded; GET_CTXSW_MODES reads it |
| 0x20801211 | `GR_CTXSW_PREEMPTION_BIND` | partial | recorded on the named channel; not forwarded: the plan's MIRROR recipe needs the D17 check that the nine buffer VAs are mapped in the channel's VA space, and a host GR context switch into unmapped GfxP/CILP buffers would fault the host engine |
| 0x2080123A | `GR_CTXSW_SETUP_BIND` | done | refused (the plan's REFUSE; TU106's GRAPHICS_SETUP context property unmeasured) |
| 0x20800198 | `GPU_VALIDATE_MEM_MAP_REQUEST` | done (D model) | no C row since the reconciliation: the BAR0 map policy of dmodel.rs (dmodel.rs:458), deny by default with the guest BAR0 base, `NV_ERR_PROTECTION_FAULT` |
| 0x208F0704 | `NV208F BIF_INFO` | done | L0s/L1 enables from the emulated Link Control; other indices 0x1F |
| 0x0073137E | `NV0073 DP_GET_GENERIC_INFOFRAME` | partial | refused with a label: the display model keeps no SDP capture yet (display-engine state, the display branch) |
| 0x0073137F | `NV0073 DP_GET_MSA_ATTRIBUTES` | partial | as above, for the MSA attributes |
| 0x20800A6A | `INTERNAL_RC_WATCHDOG_TIMEOUT` | done (D model) | no C row since the reconciliation: rc.rs `watchdog_timeout` (rc.rs:451), the stalled GR channels RC'd with Xid 45 at the next event scan, no host engine reset |
| 0x20802207 | `RC_SET_CLEAN_ERROR_HISTORY` | done (D model) | no C row since the reconciliation: rc.rs `clean_history` (rc.rs:498) empties the one journal, the host's never touched |
| 0x20800177 | `GPU_REPORT_NON_REPLAYABLE_FAULT` | done (D model) | no C row since the reconciliation: rc.rs `report_nr_fault` (rc.rs:394), the channel by its instance block, the RC reflected with Xid 31, never passed to the host GMMU |
| 0x00801108 | `NV0080 GR_SET_TPC_PARTITION_MODE` | done | refused (the plan's REFUSE) |
| 0x2080122C | `GR_SET_TPC_PARTITION_MODE` | done | refused (the plan's REFUSE) |

`NVA06C_CTRL_CMD_GET_INFO` (answered by the kernel itself on a GSP client) got a row too: the
kernel's TSG id the device recorded (classes.md §8, TSG), NOT_SUPPORTED on 610.

**Tests.** `public::tests`: `the_17_d_plan_c_rows_are_c_rows`, `idle_channels_control`,
`scheduling_rows_act_on_the_guest_channel_set`, `context_rows`, `device_state_rows`,
`rc_journal`, `b_rows_from_device_state` (a registry with a TSG, a mirrored and a software
channel and a bare kernel channel; a host RM client that fails the test when called; a fake
mirror that records every host call); `dmodel::tests` checks that every D row moved to C is
answered by this table (since the reconciliation: by this table or by a D model).

### Reconciliation with the D models (2026-09-25)

Round 2 of C and round 2 of D were built the same night and both merged; the C table runs
first in `Dispatcher::control` (ctrl.rs:999-1040: public.rs, then nvml.rs and chipctl.rs,
then the D hook), so five C rows that the C work had marked partial shadowed the D models that do the
work, and the device kept two RC journals (public.rs's, read by `RC_GET_ERROR_COUNT`, and
rc.rs's, written by `LOG_OOB_XID` and the model RCs). Commit 1a96d24 makes it one device:

- **Controls moved from the C table to the D models** (the C rows removed; the D models of
  d-device-model.md §10 R2 answer): `FIFO_DISABLE_CHANNELS` 0x2080110B (rc.rs:299),
  `GPU_REPORT_NON_REPLAYABLE_FAULT` 0x20800177 (rc.rs:394), `INTERNAL_RC_WATCHDOG_TIMEOUT`
  0x20800A6A (rc.rs:451), `RC_SET_CLEAN_ERROR_HISTORY` 0x20802207 (rc.rs:498),
  `GPU_VALIDATE_MEM_MAP_REQUEST` 0x20800198 (dmodel.rs:458), and the B row
  `NV83DE DEBUG_GET_MODE_MMU_DEBUG` 0x83DE0308, whose C refusal shadowed dmodel.rs's
  debugger record. No C row carried state the D models lacked: the per-channel `disabled`
  flag of the C row is rc.rs's `disabled` set, the journal records of the C rows are rc.rs's
  records.
- **One RC journal** (`RcState::journal`, rc.rs:141-190, at most 256 records): the
  channel-error scan (boot.rs:618), the non-replayable fault and the watchdog RCs and
  `LOG_OOB_XID` write it; the C rows `RC_GET_ERROR_COUNT` and `RC_GET_ERROR_V2` read it
  (public.rs:294-333; V2 returns the `RmProtoBuf_RECORD` header of the record,
  rmcd.h:108-111, with no error blocks); `RC_SET_CLEAN_ERROR_HISTORY` clears it. The
  per-channel fault record `GET_MMU_FAULT_INFO` reads stays in `GuestAttrs` (`set_fault`).
  On a 615 guest every RC the device raises writes exactly one operational-event record
  (boot.rs:404-424 `goe_rc`, called by the scan and by `post_model_rcs`); the guest's own
  `LOG_OOB_XID` is no RC of the device and writes none.
- **One set of scheduling flags** (`RcState::disabled`, `usermode_disabled`,
  `scheduling_disabled`; `channel_enabled`, rc.rs:259): written by `FIFO_DISABLE_CHANNELS`
  and the C rows `FIFO_DISABLE_USERMODE_CHANNELS` and
  `INTERNAL_FIFO_TOGGLE_ACTIVE_CHANNEL_SCHEDULING` (public.rs:471), read by
  `NV208F FIFO_GET_CHANNEL_STATE` (public.rs:589) and by the software executor: a software
  channel that is not enabled runs nothing at its doorbell, the doorbell is held and runs
  once the channel is enabled again (ctrl.rs:379-420 `replay_held` and `doorbell_chid`,
  replayed on the RC event path, boot.rs `scan_channel_errors`). A mirrored channel's
  doorbell goes on: its host twin is off on the host by the same control.
- **The guard.** `dmodel::tests::no_control_is_answered_by_two_tables` (dmodel.rs:1038)
  walks, per ABI, the ids of `PUBLIC` and `PUBLIC_C`, of chipctl.rs (controls.toml of every
  chip, the E families and the D rows moved to E) and of the D models and rule table, and
  asserts the three sets disjoint; then, for the ABI of the build, it asks the three
  handlers for every routed id and asserts at most one answers.
- **Generated tables.** The generator drops from `D_RULES` a control another handler
  answers: the twelve C rows of the table left it, the five the models answer are back
  (reclass C). Rule table after the reconciliation (615; 610 in brackets): REFUSE 199 (197),
  DENY 37, ACK 20 (19), TABLE 5, MODEL 34, MIRROR 1.

**Open, written down here.** `GR_CTXSW_PREEMPTION_BIND` stays recorded and not forwarded.
The follow-up is the D17 check the plan names: before the bind may reach the channel's own
host mirror, every one of the nine `vMemPtrs` (ctrl2080gr.h:921-929) must be a VA mapped in
the channel's VA space, with at least the size the host GR needs for its kind (the kinds
MAIN, SPILL, PAGEPOOL, BETACB, RTV, the three context-pool buffers and SETUP,
ctrl2080gr.h:862-871; the sizes from the context-buffer part of the static GR info, kgr.rs),
checked against the device's mirror page tables; an unmapped or short buffer is refused and never forwarded, since a host
GR context switch into it faults the host engine.

**Gates** (`caraxes/runs/gsp-round2-reconcile-2026-09-25/`, binaries of 1a96d24): the 615 guest
on vm5-615 passes nvml and every scored cuda stage but cuda-managed (managed memory, as in every
gate of the day); the 610 guest on vm3 the same, with nvml failing on the power number (N/A) as
in every 610 gate. The RC journal round trip through the guest kernel (the device's
SIMULATED RC, `CARAXES_RC_SELFTEST`; caraxes/tools/rc-journal-probe.sh: count, V2 records,
clean, count 0) passes on both guests; the goe-probe on 615 drained 11 records, two of them the
self-test RCs. No host Xid in any window.
