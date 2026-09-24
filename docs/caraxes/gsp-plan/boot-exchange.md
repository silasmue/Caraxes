<!-- SPDX-License-Identifier: MIT -->
<!-- Source survey of 2026-09-24 against the open-gpu-kernel-modules trees 615.71.09 and 610.57.04 and the device crate at the base commit of this plan (61e0b6a), read only; the run evidence is caraxes/runs/transport-615-2026-09-24/. -->
# The boot-time exchange of 615.71.09 on TU106, step by step

Part of the GSP plan ([README.md](README.md)). The transport itself (queue v2, MCTP elements,
GMCAPI, NVKV, `GSP_ARGUMENTS_CACHED`) is described in
[gsp-transport-615.md](../gsp-transport-615.md) and the registers of the Turing boot in
[gsp-boot-turing.md](../gsp-boot-turing.md) (610 lines); this file puts the whole 615 exchange
in order, from driver load to unload and through suspend, marks what `NV_ABI_STABLE` covers,
and lists per step what the device does and what is missing. The Blackwell boot through FSP
is the contrast case ([gsp-boot-blackwell.md](../gsp-boot-blackwell.md)): there the kernel
loads no falcon ucode itself and GSP-FMC boots from a Chain-of-Trust message, with the
`GSP_FMC_*` structures that 615 tags stable; none of that runs on Turing.

Short paths as in [rpc-functions.md](rpc-functions.md): `kernel_gsp.c` is
`src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c`, `kernel_gsp_tu102.c` is
`src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_tu102.c`, lines are 615 unless marked
610. Device paths are `crates/vfio-user-nvgpu/...`; `boot.rs` is `src/boot.rs`, `turing.rs` is
`src/chip/turing.rs`. "run" is `caraxes/runs/transport-615-2026-09-24/boot/device-events.txt`,
the first 615 boot on the device.

## 1. What `NV_ABI_STABLE` covers, and what NVIDIA says about it

**The tag.** `src/common/sdk/nvidia/inc/nvtypes.h:521-533`: "This tag can be applied to a type
to mark it as ABI-stable and block most changes to the internal layout. In all driver builds,
this tag is a NOP and is there for human benefit only. However, static checker tools may
support the custom attribute and use it to verify no changes across releases." It expands to
`__attribute__((nv_abi_stable))` only under `NV_ABI_STABLE_CHECKED_BUILD`. The same text is in
`kernel-open/common/inc/nvtypes.h:537-540`. 610 has no such tag.

**The carriers in 615** (every use in the tree, grep of `NV_ABI_STABLE`):

| Type | Where | Used on Turing |
|---|---|---|
| `msgqTxHeader` | `src/nvidia/inc/libraries/msgq/msgq_priv.h:82` | yes, both queue headers |
| `GSP_MSG_QUEUE_ELEMENT` | `src/nvidia/inc/kernel/gpu/gsp/message_queue_priv.h:90` | yes, every element |
| `MESSAGE_QUEUE_INIT_ARGUMENTS`, `GSP_SR_INIT_ARGUMENTS`, `GSP_ARGUMENTS_CACHED` | `src/nvidia/inc/kernel/gpu/gsp/gsp_init_args.h:33, 50, 60` | yes, RMARGS |
| `GMCAPI_HEADER`, enums `GMCAPI_HEADER_COMMAND_FLAGS`, `GMCAPI_FLAGS`, `GMCAPI_COMMANDS` | `src/nvidia/interface/gmcapi/gmcapi_base.h:27, 56, 63, 74` | yes, `GSP_INIT`, `GSP_SUSPEND` |
| enum `NVGMC_ENGINE_TYPE` | `src/nvidia/interface/gmcapi/gmcapi_engine_types.h:29` | yes, the `ENGINE_MASK` key |
| enum `GMCAPI_VGPU_PLUGIN_EVENT` | `src/nvidia/interface/gmcapi/gmcapi_vgpu.h:194` | no (vGPU host); not in gsp-transport-615.md §1 |
| `GSP_FMC_INIT_PARAMS`, `GSP_ACR_BOOT_GSP_RM_PARAMS`, `GSP_RM_PARAMS`, `GSP_SPDM_PARAMS`, `GSP_RM_MEM_PARAMS`, `GSP_FMC_BOOT_PARAMS` | `src/nvidia/arch/nvalloc/common/inc/gsp/gspifpub.h:50-127` | no (GSP-FMC boot, Hopper and later) |

**What else NVIDIA says in the tree about stability:**

- "If extending any of the NV_ABI_STABLE sub-structs, make sure to remove the corresponding
  reserved fields from the parent struct" (`gspifpub.h:33-34`): stable structs grow only into
  reserved space, which is how 615's `GSP_ARGUMENTS_CACHED` carries `reserved2[4]` and
  `MESSAGE_QUEUE_INIT_ARGUMENTS` `reserved[8]` (gsp-transport-615.md §6).
- The queue header carries its own version: "Major versions get increased with incompatible
  changes", "Minor versions are compatible" (`msgq_priv.h:31-34`, major 2, minor 0).
- "GSP Message Queue encryption is currently *NOT* ABI stable" (`message_queue_priv.h:45`).
- GMCAPI ids up to 0xFFFF are reserved so they never collide with RPC function numbers
  (`gmcapi_base.h:76-80`).
- The NVKV key tables are split into a promoted part and an "unstable" part: "Keys that are
  still under development (or can change depending on build configuration) live in this file
  until they get promoted" (`gmcapi_system_info_unstable.h:27-28`,
  `gmcapi_gsp_config_unstable.h:31-32`). `CL_FLAGS` (0x1033) is such an unstable system-info key
  (`gmcapi_system_info_unstable.h:30`).
- **The RM RPC layer is declared unstable.** The kernel must opt in to "unstable interfaces to
  GSP", with one bit for RPCs and one for events (`NVGMC_SI_UNSTABLE_INTERFACE_SUPPORT` 0x1110,
  bits RPC and EVENTS, `gmcapi_system_info.h:200-203`), and the 615 kernel sets both
  (`kernel_gsp.c:5203-5206`). The internal client, device and subdevice handles "are only valid
  if using RMAPI RPCs with GSP and should be avoided/ignored as any RPCs that rely on them are
  considered ABI unstable" (`gmcapi_gsp_config.h:74-78`).
- The firmware is still locked to the driver: the kernel accepts only a GSP-RM ELF whose
  `.fwversion` equals its own `NV_VERSION_STRING` (`_kgspFwContainerVerifyVersion`,
  `kernel_gsp.c:6397-6440`, called at :4190).

**What that means for the device.** Stable, and therefore written once for every later
driver: the queue memory and registers, the element framing, the boot arguments, the GMCAPI
header and the NVKV encoding of `GSP_INIT` with its promoted keys. Explicitly not stable: the
RPCs (`GSP_RM_ALLOC`, `GSP_RM_CONTROL`, ...), the events, the internal handles, and every
control and alloc parameter layout; these stay in the per-version modules of D14. The version
lock means that NVIDIA itself never mixes a kernel and a GSP-RM of different versions; "stable"
is a promise about the boundary, not a compatibility mode, and the device keeps serving exactly
one guest ABI per build (gsp-transport-615.md §8). Not tagged although the device reads them
on every boot: `GspFwWprMeta` (`src/nvidia/arch/nvalloc/common/inc/gsp/gsp_fw_wpr_meta.h`),
`LibosMemoryRegionInitArgument`, `rpc_message_header_v`, the NVKV instruction format
(`src/nvidia/inc/libraries/nvkv/nvkv.h`) and `GmcApiGspSuspendParams`/`GmcApiGspResumeDoneParams`
(`gmcapi_gsp.h:39-66`).

## 2. The sequence on 615, in order

The kernel path is `RmInitAdapter` -> `kgspInitRm_IMPL` (`kernel_gsp.c:5521`) ->
`_kgspBootGspRm` (:5429) -> `kgspBootstrap_TU102` (`kernel_gsp_tu102.c:392`). 610 has the
same frame at `kernel_gsp.c:4878`, :4796 and `kernel_gsp_tu102.c:522` (gsp-boot-turing.md §1).

| # | Step (615) | Kernel RM | 610 difference | Device today | Gap |
|---:|---|---|---|---|---|
| 0 | Queue construction: `GspMsgQueuesInit`, `msgqTxCreate` writes the command-queue header and `QUEUE_HEAD(0) = 0` | `kernel_gsp.c:4786-4803`, `message_queue_cpu.c:207-340`, `msgq.c:175-177` | no register write; v0 header with in-memory pointers | the head write is logged, not served before the link (`boot.rs:827-831`) | none |
| 1 | VBIOS from ROM, FWSEC ucode parsed | `kernel_gsp.c:5560-5600` (`kgspExtractVbiosFromRom_HAL`, `kgspParseFwsecUcodeFromVbiosImg`) | same | synthetic ROM image (vbios-synthesis.md) | none |
| 2 | Booter Load and Unload images, boot binary, GSP-RM radix3 image | `kernel_gsp.c:5617-5657` | same | not read (the device never executes firmware) | none |
| 3 | Libos log buffers LOGINIT, LOGINTR, LOGRM, LOGMNOC and their decoder | `_kgspInitLibosLoggingStructures`, `kernel_gsp.c:4044-4120`; decoder :5669 | same set | nothing written; the kernel's poller finds empty buffers | none for function; GSP-RM log lines never appear in the guest |
| 4 | Wait for GFW boot | `kgspWaitForGfwBootOk_TU102`, `kernel_gsp_tu102.c:1083-1101` | same | `GFW_BOOT` reads 0xFF (`chips/tu106/boot.toml:65`) | none |
| 5 | RM state monitor page (4 KiB, user read-only) | `kgspAllocRmStateMonitorBuffer`, `kernel_gsp.c:4413-4440` | new in 615 | not written | none (the kernel never reads it) |
| 6 | Libos init arguments: the log regions, then RMARGS | `kgspSetupLibosInitArgs`, `kernel_gsp.c:7040-7067` | same | read at the link (`boot.rs:1426-1454`) | none |
| 7 | `GSP_ARGUMENTS_CACHED`: magic, size, flags (`STACK_IN_DMEM`), queue arguments, SR arguments zero | `kgspPopulateGspRmInitArgs`, `kernel_gsp.c:6139-6230` | 152-byte layout without magic | read and checked (`msgq/v2.rs:140-195`) | the flags are logged, not acted on (§4) |
| 8 | Registry and system information encoded as NVKV | `_kgspPrepareRegistry` :5273, `_kgspPrepareSystemInfo` :5232, encoders :4893-5217 | `GspSystemInfo` and `PACKED_REGISTRY_TABLE` structs | decoded at `GSP_INIT` (`boot.rs:1492-1515`) | registry keys are traced, not applied (§3) |
| 9 | WPR2 must be down | `kgspIsWpr2Up_HAL`, `kernel_gsp.c:5439-5451` | same | WPR2 registers read 0 until FWSEC (`turing.rs:201-202`) | none |
| 10 | `GspFwWprMeta` filled (256 bytes, `pagingConfig` new) | `kgspPopulateWprMeta_TU102`, `kernel_gsp_tu102.c:619-823` | `padding[2]` in place of `pagingConfig` | read at Booter Load (`boot.rs:752-773`) | none |
| 11 | Scrubber image when the pre-scrubbed top of FB is too small | `kgspPrepareScrubberImageIfNeeded_TU102`, `kernel_gsp_tu102.c:1290-1313`; run at :406 | same | no scrubber run observed in the run | open question 3 |
| 12 | FWSEC-FRTS prepared, then run on the GSP falcon | `kgspPrepareForBootstrap_TU102` :294-339, `kernel_gsp_tu102.c:417-426` | same | WPR2 set from the FRTS offset in the FWSEC command (`turing.rs:140-160`; run :18) | none |
| 13 | GSP falcon reset into RISC-V; libos arguments PA into `MAILBOX0/1` | `kernel_gsp_tu102.c:429-432`, :261-272 | same | mailboxes kept (`turing.rs:170-173`) | none |
| 14 | Booter Load on SEC2 with the WPR meta PA | `kernel_gsp_tu102.c:436-442` | same | MAILBOX0 := 0, RISC-V active, `WprMeta` event (`turing.rs:246-262`; run :21) | see §4: the SEC2 run counter |
| 15 | `NV_PFALCON_FALCON_OS` := app version; check that the RISC-V core runs | `kernel_gsp_tu102.c:446-459` | same | answered by the falcon model | none |
| 16 | Status-queue link: poll for GSP's queue header, then `MSGQ_TAIL(0) = 0` | `GspStatusQueueInit`, `kernel_gsp_tu102.c:469`, `message_queue_cpu.c:342-420`, `msgq.c:240-272` | the CPU waited for `GSP_INIT_DONE` after `SET_SYSTEM_INFO` and `SET_REGISTRY` (610 `kernel_gsp_tu102.c:578-614`) | linked at Booter Load (`boot.rs:1397-1424, 1456-1488`; run :27) | none |
| 17 | `GMCAPI_CMD_GSP_INIT`: system info and registry in, static configuration out | `kgspSendInitRpcs`, `kernel_gsp.c:5313-5361`, from `kernel_gsp_tu102.c:473` | four RPCs plus `GSP_INIT_DONE` (rpc-functions.md §4.3) | answered, 424 bytes of NVKV (`boot.rs:958-960, 1492-1542`; run :29-95) | static keys not sent (§3) |
| 18 | `INIT_GSP_TRACE_CRASH_BUFFER` | `kernel_gsp.c:5789` -> :4358 | 610 `kgspInitGspTraceCrashBuffer` :5167 | answered (`boot.rs:1138-1146`; run :97) | none |
| 19 | GPU properties from the static configuration, SR regions, log polling, health-check callback | `_kgspInitGpuProperties` :7432-7440, `kgspPopulateSrRegionsInfo_TU102` (`kernel_gsp_tu102.c:1375`), `kgspStartLogPolling` :7180/:7205, :5795-5803 | 610 also initialised the heartbeat here | nothing to do; Turing has no GSP heartbeat (`kgspIsHeartbeatSupported` is `NV_FALSE` for TU102-TU117, `src/nvidia/generated/g_kernel_gsp_nvoc.c:2292-2295`) | none |
| 20 | First RM traffic: the kernel's internal clients (`NV01_ROOT`, `NV01_DEVICE_0`, `NV20_SUBDEVICE_0`), `GSP_RM_CONTROL`s on the internal subdevice 0xCA7A0003, `UPDATE_BAR_PDE` when BAR2 is set up, the event import's `ALLOC_MEMORY` | `gpu.c` state init; `kern_bus.c:902`; `gpu.c:2796` | no event import | answered except `ALLOC_MEMORY` (run :101-189, :517) | `ALLOC_MEMORY` (rpc-functions.md §4.1) |

## 3. Gaps in the init exchange

1. **Static configuration keys the device does not send** (gsp-transport-615.md §5.3): `SKU_*`,
   `CHIP_SUB_REV`, `EMULATION_REV1`, `CHIP_SKU*`, `PROJECT*`, `CDP`, `PDI`, `GC6_*`, `SRIOV_*`,
   `ECID_INFO`, `VMMU_SEGMENT_SIZE`, `WPR_LAYOUT_*`, `FB_REGION_TAG`, `VGX_VERSION`. All were
   zero in the 610 reply too. Two matter for later clients: `POWER_FEATURES` decides
   GC6/GC8/GCOFF RTD3 (`kernel_gsp.c:7809-7811`, used at :7435-7437) and must stay 0 for a
   desktop part (§5); `CHIP_SUB_REV` replaced the removed control 0x20800A36 and should carry
   the card's value (chip data). Recipe: send every key of `gmcapi_gsp_config.h:30-148` that
   the real card sends, from one capture of the host's own `GSP_INIT` response (open
   question 1), labelled chip data.
2. **Registry.** The 44 registry entries of the captured request are traced
   (`boot.rs:1512-1514`) but none changes the device's behaviour. Keys that change GSP-RM
   behaviour and that a user may set: `RMSplitVasMgmtServerClientRm` (the kernel reads it itself,
   `gpu_registry.c:173`), `RmGspTraceCrashLogging*` (kernel side), `RmRcWatchdog` (the captured
   guest sets 0). Recipe: a table of registry keys the device honours, starting empty, each with
   the GSP-RM behaviour it emulates; refuse none.
3. **System information.** Only the BAR addresses, the driver version and the registry are
   used (`boot.rs:1502-1515`). The rest is stored (`self.system_info`) for later controls
   (`HOST_PAGE_SIZE`, `MAX_USER_VA`, the PCIe flags). Recipe: nothing now; new controls read
   what they need from it.
4. **`UNSTABLE_INTERFACE_SUPPORT`.** The kernel opts in to RPC and event support; a future
   client that does not (nova-core speaks r570, a Windows kernel of another branch) would
   expect `GSP_INIT` alone. Recipe: log the bits; refuse `GSP_RM_*` from a client that did not
   opt in (what a real GSP-RM does is not in the tree: open question 2).

## 4. Teardown and reload

| # | Step (615) | Kernel RM | 610 difference | Device today | Gap |
|---:|---|---|---|---|---|
| 1 | State unload, then `kgspUnloadRm` from `gpuStateDestroy_IMPL` (normal unload, level 0) or `gpuEnterShutdown_IMPL` (fast unload) | `gpu.c:4167-4172`, `gpu.c:3794-3811` | same frame | - | - |
| 2 | `GMCAPI_CMD_GSP_SUSPEND`, fire and forget, flags PM_TRANSITION, GC6_ENTERING, HIBERNATE, FAST, FORCE | `kernel_gsp.c:5866-5892`, `gmcapi_gsp.h:39-56` | `UNLOADING_GUEST_DRIVER` RPC with the same three facts (610 `kernel_gsp.c:5231`) | no reply; MAILBOX0 bit 31 (`boot.rs:961-974`); flags not read | log the flags; keep state when PM_TRANSITION is set (§5) |
| 3 | Wait for the processor-suspended bit in MAILBOX0 | `kgspWaitForProcessorSuspend_TU102`, `kernel_gsp_tu102.c:218-255` | same | set in step 2 | none |
| 4 | Heartbeat off, GSP-RM logs dumped and optionally preserved | `kernel_gsp.c:5908-5935` | same | empty logs | none |
| 5 | GSP falcon reset and FWSEC-SB (puts the pre-OS apps back), not for GC6 entry | `kgspTeardown_TU102`, `kernel_gsp_tu102.c:524-563` | same | the second GSP falcon run is only noted (`turing.rs:224-229`) | none |
| 6 | Booter Unload on SEC2 (argument 0, or the SR meta PA for a suspend) | `kernel_gsp_tu102.c:566-567`, `_kgspGetBooterUnloadArgs` :491-515 | same | WPR2 down, RISC-V stopped (`turing.rs:234-245`; reload-a :125) | the SR meta argument is ignored (§5) |
| 7 | The OS layer resets the function before the next load | reload runs: `flr: function-level reset` (`reload-a/device-events.txt:126`) | same | `BootModel::reset` (`boot.rs:484-514`) keeps the transport, host client and token pool | see below |

**Reload without a reset.** The falcon model counts SEC2 starts since creation
(`src/falcon.rs:51-52, 205-207`) and treats every start after the first as Booter Unload
(`turing.rs:234`). A second Booter Load without a function-level reset in between (a driver
reload where the OS skips the FLR, or any resume, §5) is therefore taken for an unload: WPR2
goes down and the RISC-V core stops, and the kernel fails with "Failed to boot GSP"
(`kernel_gsp_tu102.c:450-459`). The reload runs pass only because an FLR recreates the model
(reload-a :126, reload-b :124). Recipe: tell Load from Unload by the booter image or by the
state (WPR2 up and RISC-V running means Unload; otherwise Load), not by the run count. Effort:
half a day with a test that runs load, unload, load on one model.

## 5. Suspend, resume, GC6 and RTD3

**What the kernel does** (`gpu_suspend.c`):

- Entry (`gpuPowerManagementEnter`, :71-139): the unload mode is `GC6_ENTER` when entering GC6,
  `SR_WITH_WPR_IN_SYSMEM` for S3 with a pre-initialised WPR region, else `SR_SUSPEND`, for which
  `kgspPrepareSuspendResumeData_TU102` builds a `GspFwSRMeta` with a radix3 buffer for the
  WPR2 contents (`kernel_gsp_tu102.c:1127-1200`). Then `gpuStateUnload` with
  `PRESERVING | PM_TRANSITION`, which runs FB save/restore: a copy of every preserved video
  memory allocation to sysmem (`memmgrSavePowerMgmtState_KERNEL`,
  `src/nvidia/src/kernel/gpu/mem_mgr/mem_mgr_pwr_mgmt.c:44`) and a sysmem backing store for
  GSP's heap, non-WPR region and VGA workspace, registered with GSP by `ALLOC_MEMORY`
  (`mem_mgr_pwr_mgmt.c:338-400`). Then `kgspUnloadRm` with the mode (§4 steps 2-6; Booter
  Unload gets the SR meta PA).
- Resume (`gpuPowerManagementResume`, :160-330): sysmem flush buffer reprogrammed (:200),
  `GSP_SR_INIT_ARGUMENTS` {oldLevel, flags} into RMARGS with the `IN_PM_TRANSITION` flag
  (:208-212, `kernel_gsp.c:6192-6197`), wait for GFW boot, boot mode `SR_RESUME` (or
  `GC6_EXIT`, `SR_WITH_WPR_IN_SYSMEM`), `kgspPrepareForBootstrap` and `kgspBootstrap_TU102`
  (:262-278). For a resume the bootstrap skips FWSEC, the reset into RISC-V and the mailbox
  write (`kernel_gsp_tu102.c:414-433`), runs Booter Load with the SR meta PA
  (`_kgspGetBooterLoadArgs`, :346-366), does not link the status queue again ("GSP-RM will
  restore queue state", :464-466) and waits for the GMCAPI event `GSP_RESUME_DONE` (0x10005)
  instead of sending `GSP_INIT` (:474-478, `kgspWaitForRmResumeDone`, `kernel_gsp.c:7135-7160`;
  handler `gmcapiGspResumeDone` :7104-7124, flag `D3HOT`, `gmcapi_gsp.h:63-69`). Then
  `gpuStateLoad` with `PRESERVING | PM_TRANSITION` restores FB.

**Does a desktop GeForce on Linux exercise them?**

- *S3 and hibernate:* yes, whenever the guest suspends (`systemctl suspend`, or the VMM's
  guest PM suspend) and the NVIDIA PM callbacks run. The FB copy then goes through the
  device's copy engine or BAR mappings, and the kernel needs `ALLOC_MEMORY` and
  `GSP_RESUME_DONE`.
- *GC6, GC8, GCOFF (RTD3):* no. The kernel enables them only from the static configuration's
  `POWER_FEATURES` bits (`kernel_gsp.c:7809-7811`, `_kgspInitGpuProperties` :7435-7437), which
  the device sends as 0, and the OS-level runtime power management defaults to mode 3, "Power
  down the GPU after a period of inactivity ... on Ampere or later notebooks. Otherwise, do not
  power down the GPU" (`src/nvidia/arch/nvalloc/unix/include/nv-reg.h:664-685`). A desktop
  TU106 is neither (the real card's `POWER_FEATURES` is open question 1).

**What the device does today:** `GSP_SUSPEND` is answered with the suspended bit; the flags,
the SR meta PA and `IN_PM_TRANSITION` are not read; `GSP_RESUME_DONE` is never sent; the second
Booter Load is misread (§4); `ALLOC_MEMORY` is refused. So a guest suspend fails at entry
(the GSP heap backing store) or at resume (the Booter Load, then the wait for
`GSP_RESUME_DONE`).

**Recipe (about 2 agent-days, after the `ALLOC_MEMORY` registry):**

1. On `GSP_SUSPEND` with `PM_TRANSITION`: keep the whole model (objects, channel registry,
   queue state, host mirrors); stop executing guest work; set the suspended bit.
2. Booter Unload with a nonzero SR meta PA: record it; nothing to save, the device's state
   lives in its process.
3. Booter Load recognised by state (§4); with the SR meta PA and `IN_PM_TRANSITION` in RMARGS,
   do not relink; queue pointers continue where they were.
4. Send `GSP_RESUME_DONE` as a GMCAPI request (command id without the response flag, sequence
   0, `GmcApiGspResumeDoneParams` {flags 0}); the v2 send path needs a request variant
   ([events.md](events.md), GMCAPI section).
5. FB save/restore needs no device support beyond working copy engines and BAR mappings; the
   device's FB store survives a guest suspend anyway.

## 6. Gap list

| # | Gap | Where | Truth source | Effort |
|---:|---|---|---|---|
| B1 | `ALLOC_MEMORY` refused: the event import fails at every boot; suspend's GSP heap store fails | §2 step 20, §5 | guest state | 1 day (rpc-functions.md §4.1) |
| B2 | Static configuration keys missing (`CHIP_SUB_REV`, `SKU_*`, `ECID_INFO`, `VMMU_SEGMENT_SIZE`, ...) | §3.1 | chip data from one capture | 0.5 day plus the capture |
| B3 | Registry traced, never honoured | §3.2 | emulation | 0.25 day for the table |
| B4 | Second Booter Load without FLR misread as Unload | §4 | emulation | 0.5 day |
| B5 | `GSP_SUSPEND` flags and `IN_PM_TRANSITION` ignored; `GSP_RESUME_DONE` never sent; no GMCAPI request path | §5 | emulation | 1.5 days |
| B6 | GSP-RM log buffers stay empty | §2 step 3 | emulation | optional: a device log line into LOGRM would make device notes visible in the guest's dmesg |
| B7 | `UNSTABLE_INTERFACE_SUPPORT` not checked | §3.4 | emulation | 0.1 day |

## 7. Open questions needing a measurement

1. **The real card's `GSP_INIT` response.** Which static keys, with which values, does GSP-RM
   615 send for this TU106 (including `POWER_FEATURES`, `CHIP_SUB_REV`, `SKU_*`, `ECID_INFO`)?
   Proposal: on the host (operator's go-ahead, host GPU state is off limits without it), no
   driver change: the host kernel logs nothing of it, so the capture needs a passthrough VM
   with a 615 guest and `NVreg_RmMsg` for `_kgspStaticInfoKeyHandler`, or a patched guest
   kernel module that dumps the NVKV words; the passthrough needs the prepared SSH/tmux
   procedure first (passthrough-procedure.md). Store the words as chip data.
2. **What does GSP-RM do when the kernel does not opt in to unstable RPCs?** Proposal: none
   possible without GSP-RM source; decide by policy (refuse), revisit when a non-Linux client
   appears.
3. **Does a TU106 with this FB size need the scrubber?** The run shows no scrubber falcon
   start; `kgspGetPrescrubbedTopFbSize` decides. Proposal: log the guest's
   "pre-scrubbed memory" INFO line (`kernel_gsp_tu102.c:1304-1305`) with `NVreg_RmMsg` in a
   device run, for both FB sizes the profiles use.
4. **Guest suspend in the VMM.** Does Cloud Hypervisor's guest PM suspend (or `virsh
   dompmsuspend` under QEMU) reach the NVIDIA PM callbacks, and does the VMM reset the function
   on resume? Proposal: with trace on, suspend and wake a device VM after B1 and B5 exist; the
   device trace shows `GSP_SUSPEND` flags, the Booter runs and whether an FLR comes. This also
   answers events.md M8.
5. **Reload without FLR.** Does any path of the Linux driver unload and load GSP-RM without a
   function-level reset (for example `rmmod nvidia` with the device held by a VFIO user)?
   Proposal: count FLRs per reload in the device log over the reload probes; if one comes
   without, B4 is a live bug, not only a resume issue.
