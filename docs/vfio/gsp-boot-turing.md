<!-- SPDX-License-Identifier: MIT -->
<!-- Source survey generated 2026-09-23 by reading vendor/open-gpu-kernel-modules at tag 610.57.04
     (and the Leandro VFIO tree at e2f1aec). Every claim cites file:line; values marked "derived" were
     computed, not read. Treat as notes to be verified by experiment, not as specification. -->
# NVIDIA open-gpu-kernel-modules 610.57.04: how the driver boots GSP-RM on TU106 (RTX 2070) in GSP-client mode

This is a trace of the boot path in the source. It covers the call tree, every register access in order, the sysmem structures, what the driver expects the firmware to produce, the bindata images, and open questions.

- Root: `/home/silas/git/Caraxes/vendor/open-gpu-kernel-modules`. All paths below are relative to `src/nvidia/` unless they start with `src/common` or `kernel-open`.
- HAL choices were resolved from the generated NVOC tables for ChipHal TU106 (for example `generated/g_kernel_gsp_nvoc.c:1106-1127`, `generated/g_kernel_falcon_nvoc.c:547-563`).
- Register offsets come from the swref headers under `src/common/inc/swref/published/...`.
- Anything marked **derived** is my own computation (struct packing, arithmetic, decompressed bindata), not a literal from the source.

---

## 1. Call tree

### 1.1 How the driver gets from RmInitAdapter to the GSP boot

```
RmInitAdapter                                  arch/nvalloc/unix/src/osinit.c:1971
 ├─ RmFetchGspRmImages()                       osinit.c:2022 → 1881-1958
 │    nv_get_firmware(NV_FIRMWARE_TYPE_GSP, chipFamily) → "gsp_tu10x"   arch/nvalloc/common/inc/nv-firmware.h:111-119
 │    (ucodes_tu10x only when the "GR3428ReadUcodesBin" regkey is set, osinit.c:1916-1924)
 │    (gsp_log_tu10x only when enable_firmware_logs is set, osinit.c:1933-1941)
 └─ kgspInitRm(pGpu, pKernelGsp, &gspFw)       osinit.c:2115   (only when IS_GSP_CLIENT)
```

Earlier, at construction time (`kgspConstructEngine_IMPL`, `src/kernel/gpu/gsp/kernel_gsp.c:4338`):
- `kgspConfigureFalcon_TU102` (4352): registerBase = 0x110000, riscvRegisterBase = 0x111000, fbifBase = 0x110600, pmcEnableMask = 0 (`kernel_gsp_tu102.c:71-77`).
- `_kgspInitRpcInfrastructure` → `GspMsgQueuesInit` (3126): allocates the queue shared memory.
- `kgspAllocBootArgs_TU102` (4366): allocates the WPR meta, libos args and GSP args pages.
- SEC2 is configured by `ksec2ConfigureFalcon_TU102`: registerBase = 0x840000, fbifBase = 0x840600 (`src/kernel/gpu/sec2/arch/turing/kernel_sec2_tu102.c:37-59`).

### 1.2 kgspInitRm_IMPL (`kernel_gsp.c:4878-5207`)

1. Acquires the GPU group lock with `rmGpuGroupLockAcquire(... GPU_LOCK_GRP_SUBDEVICE ...)` (4902-4905). The caller already holds the API lock; `_kgspPrepareRegistry` asserts `rmapiLockIsOwner()` (4653).
2. If `pFwsecUcode == NULL`:
   - `kgspExtractVbiosFromRom_TU102` (4922) reads the ROM through the BAR0 PROM window (§2, phase A1).
   - `kgspParseFwsecUcodeFromVbiosImg` (4929) builds the FWSEC ucode (`src/kernel/gpu/gsp/kernel_gsp_fwsec.c:1110`).
3. Since `bPartitionedFmc` is false on TU, it allocates the Booter images:
   - `kgspAllocateBooterLoadUcodeImage` (4981)
   - `kgspAllocateBooterUnloadUcodeImage` (4992)
4. `kgspPrepareBootBinaryImage` (5003 → 5562) copies the GSP-RM boot ucode from bindata into sysmem.
5. `_kgspPrepareGspRmBinaryImage` (5011 → 5828) parses the ELF sections, creates the signature memdesc and builds the radix3 image.
6. `_kgspInitLibosLoggingStructures` (5019 → 3680) and `_kgspInitLibosLogDecoder` (5026).
7. `threadStateResetTimeout`, then `kgspWaitForGfwBootOk_TU102` (5031-5032 → `kernel_gsp_tu102.c:1184` → `gpuWaitForGfwBootComplete_TU102`, `src/kernel/gpu/arch/turing/kern_gpu_tu102.c:447`).
8. `tmrSetCurrentTime_HAL` (5039), resolved to `tmrSetCurrentTime_GV100`. Its return value is ignored.
9. `kfspCheckForClockBoostCapability_HAL` only if `pKernelFsp != NULL` (5044); there is no FSP on Turing.
10. `kgspAllocRmStateMonitorBuffer` (5050).
11. `kgspSetupLibosInitArgs` (5053 → 6199).
12. `kgspPopulateGspRmInitArgs(pGpu, pKernelGsp, NULL)` (5056 → 5468).
13. `_kgspPrepareRegistry` (5061) and `_kgspPrepareSystemInfo` (5062) build the payloads for the first two RPCs.
14. **Retry loop** (5070-5119):
    - `maxGspBootAttempts` comes from regkey `NV_REG_STR_RM_GSP_BOOT_RETRY_ATTEMPTS`, default 4 (`interface/nvrm_registry.h:2822`).
    - `bootAttempts` starts from `NV_REG_STR_RM_GSP_BOOT_INITIAL_SHIFT`, default 0 (2805), taken modulo the maximum.
    - Loop: `do { status = _kgspBootGspRm(...&bRetry); assert API lock + GPU lock held } while (bRetry && bootAttempts < max)`.
15. On success (5146-5175):
    - `_kgspHeartbeatInit` if heartbeat is supported.
    - RPC `SET_GUEST_SYSTEM_INFO` (5153), then `GET_GSP_STATIC_INFO` (5160).
    - `kgspInitGspTraceCrashBuffer`, `_kgspInitGpuProperties`, `_kgspSetFwWprLayoutOffset` (only if `PDB_PROP_GPU_FW_WPR_OFFSET_SET_BY_ACR`, 4717), `kgspStartLogPolling`.
16. On failure: `kgspHealthCheck_HAL` (5142), then log preservation, then GPU lock release (5201-5204).

### 1.3 _kgspBootGspRm (`kernel_gsp.c:4796-4865`)

1. `bEccDisabled = !kmemsysCheckReadoutEccEnablement(...)` (4799). On TU this is `_GV100`, which reads `NV_PGRAPH_PRI_FECS_FEATURE_READOUT` (`src/kernel/gpu/mem_sys/arch/volta/kern_mem_sys_gv100.c:429`).
2. If `kgspIsWpr2Up_HAL` and not `PDB_PROP_GPU_PREINITIALIZED_WPR_REGION`, it returns `NV_ERR_INVALID_STATE` (4805-4811).
3. `kgspPopulateWprMeta_TU102` (4814 → `kernel_gsp_tu102.c:755`).
4. `_kgspPrepareScrubberImageIfNeeded` (4818 → 4446):
   - The scrubber is only needed if `fbSize - gspFwRsvdStart > kgspGetPrescrubbedTopFbSize` (256 MB on TU, `generated/g_kernel_gsp_nvoc.h:1985-1987`) or `kgspIsScrubberImageSupported` (NV_FALSE on TU, `g_kernel_gsp_nvoc.h:2017`).
   - If it were needed, `ksec2GetBinArchiveSecurescrubUcode` for TU is an assert stub returning NULL (`generated/g_kernel_sec2_nvoc.h:836`), so the boot would fail with `NV_ERR_NOT_SUPPORTED`.
   - In practice `pScrubberUcode` stays NULL on TU.
5. `kgspPrepareForBootstrap_TU102(..., KGSP_BOOT_MODE_NORMAL)` (4822 → `kernel_gsp_tu102.c:425-469`):
   - Returns an error unless `IS_GSP_CLIENT` holds and the GSP HWCFG2 RISC-V bit is set.
   - Allocates `pPreparedFwsecCmd` and calls `kgspPrepareForFwsecFrts_TU102(pFwsecUcode, pWprMeta->frtsOffset)` (`kernel_gsp_frts_tu102.c:577`).
6. **Locking:** if `_kgspShouldRelaxGspInitLocking` is true, the API lock is released while the GPU lock is kept (4825-4827).
   - That function returns true on Unix unless SPDM is enabled, when regkey `RmRelaxedGspInitLocking` is DEFAULT (2) or ENABLE (4731-4761, `nvrm_registry.h:2847`).
7. If `bootAttempts > 0 && bEccDisabled` and regkey ScanWprEndMargin (default 1) is set, `GSP_FW_FLAGS_SCAN_RECOVERY_MARGIN` is ORed into `pWprMeta->flags` (4829-4837).
8. `kgspBootstrap_TU102(..., KGSP_BOOT_MODE_NORMAL)` (4840).
9. On failure on a non-zero-FB part (4842-4854):
   - `bootAttempts++`.
   - `*pbRetry = TRUE` if `gpuCheckEccCounts_HAL()` reports errors **or** `bEccDisabled`.
   - When `bEccDisabled`, it also persists the regkey `RmGspBootInitialShift = bootAttempts`.
10. If locking was relaxed, `_kgspBootReacquireLocks` releases the GPU lock, then takes the API lock, then retakes the GPU lock (4764-4793, 4860-4862).

### 1.4 kgspBootstrap_TU102 (`kernel_gsp_tu102.c:522-619`)

1. The scrubber branch (533-537) is skipped because `pScrubberUcode == NULL`. The TU HAL `kgspExecuteScrubberIfNeeded_5baef9` is an assert stub.
2. Normal boot with `kgspGetFrtsSize > 0` (always 1 MB on TU):
   - `kflcnReset_HAL(GSP)` (550)
   - `kgspExecuteFwsec_TU102(pPreparedFwsecCmd)` (552 → `kernel_gsp_frts_tu102.c:463`), which calls `kgspExecuteHsFalcon_TU102(fwsec, GSP falcon, NULL, NULL)` (`kernel_gsp_falcon_tu102.c:380`)
   - Frees the prepared command.
3. `kflcnResetIntoRiscv_TU102(GSP)` (559 → `src/kernel/gpu/falcon/arch/turing/kernel_falcon_tu102.c:200`). This is `kflcnReset_TU102` plus a software flag; there is no hardware core switch on TU.
4. `kgspProgramLibosBootArgsAddr_TU102` (562 → 392-403).
5. `kgspExecuteBooterLoad_TU102(sysmem PA of pWprMetaDescriptor)` (566-572 → `kernel_gsp_booter_tu102.c:82`):
   - `kflcnReset_HAL(SEC2)`
   - `s_executeBooterUcode_TU102` → `kgspExecuteHsFalcon_TU102(booterLoad, SEC2, &mb0, &mb1)`
6. `kgspSendInitRpcs` (578 → `kernel_gsp.c:4686`) sends `GSP_SET_SYSTEM_INFO` then `SET_REGISTRY`. Both are asynchronous (`src/kernel/vgpu/rpc.c:10458`, `10527/10532`).
7. Writes `NV_PFALCON_FALCON_OS = pGspRmBootUcodeDesc->appVersion` (587-588).
8. Checks `kflcnIsRiscvActive || _kgspIsProcessorSuspended`; otherwise returns `NV_ERR_NOT_READY` (592-601).
9. `GspStatusQueueInit` (611 → `src/kernel/gpu/gsp/message_queue_cpu.c:339`).
10. `kgspWaitForRmInitDone` (614 → `kernel_gsp.c:6264`) → `rpcRecvPoll(NV_VGPU_MSG_EVENT_GSP_INIT_DONE)` → `_kgspRpcRecvPoll` (2848).

**Timeouts.** "Default timeout" below means `GPU_TIMEOUT_DEFAULT` (= 0; thread-state based unless `BYPASS_THREAD_STATE` is set; `inc/kernel/gpu/gpu_timeout.h:32-40`). On Unix graphics mode the default is 4 s; compute mode is 30 s (`arch/nvalloc/unix/src/os.c:2182-2191`). GSP clients use the OS timer (`gpu_timeout.c:63-77`).

---

## 2. Register accesses in the order the driver performs them

**Base addresses.**
- GSP falcon: `DRF_BASE(NV_PGSP)` = 0x110000 (`dev_gsp.h:26`); FBIF 0x110600 (`dev_gsp_addendum.h:26`); RISC-V 0x111000 (`dev_riscv_pri.h:27`).
- SEC2 falcon: `DRF_BASE(NV_PSEC)` = 0x840000 (`dev_sec_pri.h:27`); FBIF 0x840600 (`dev_sec_addendum.h:27`).

**Falcon-relative offsets** (`dev_falcon_v4.h`), absolute value = base + offset:

| Register | Offset |
|---|---|
| IRQSCLR | 0x004 |
| IRQSTAT | 0x008 |
| IRQMASK | 0x018 |
| IRQDEST | 0x01c |
| MAILBOX0 | 0x040 |
| MAILBOX1 | 0x044 |
| OS | 0x080 |
| RM | 0x084 |
| DEBUGINFO | 0x094 |
| HWCFG2 | 0x0f4 (RISCV = bit 10) |
| CPUCTL | 0x100 (STARTCPU = bit 1, HALTED = bit 4, ALIAS_EN = bit 6) |
| BOOTVEC | 0x104 |
| HWCFG | 0x108 (IMEM_SIZE = 8:0) |
| DMACTL | 0x10c (DMEM_SCRUBBING = bit 1, IMEM_SCRUBBING = bit 2, DONE = 0) |
| CPUCTL_ALIAS | 0x130 |
| IMEMC(i) | 0x180 + 16i (OFFS 7:2, BLK 15:8, AINCW bit 24, SECURE bit 28) |
| IMEMD(i) | 0x184 + 16i |
| IMEMT(i) | 0x188 + 16i (TAG 15:0) |
| DMEMC(i) | 0x1c0 + 8i (AINCW bit 24, AINCR bit 25) |
| DMEMD(i) | 0x1c4 + 8i |

**FBIF-relative offsets** (`dev_fbif_v4.h`):
- `TRANSCFG(i)` = 0x00 + 4i: TARGET 1:0 (LOCAL_FB 0, COHERENT_SYSMEM 1, NONCOHERENT_SYSMEM 2), MEM_TYPE bit 2 (PHYSICAL = 1).
- `CTL` = 0x24: ALLOW_PHYS_NO_CTX bit 7.

**Engine reset registers:**
- `NV_PGSP_FALCON_ENGINE` = 0x1103c0 (`dev_gsp.h:31`)
- `NV_PSEC_FALCON_ENGINE` = 0x8403c0 (`dev_sec_pri.h:28`)
- RESET is bit 0.

**Standard falcon reset.** I refer to this as "RESET-SEQ(base)"; it is `kflcnReset_TU102` (`kernel_falcon_tu102.c:177-194`):

1. `kflcnPreResetWait`: no operation on TU (`g_kernel_falcon_nvoc.c:559-561`, `g_kernel_falcon_nvoc.h:840`).
2. `kflcnResetHw`, which is `kgspResetHw_TU102` (`kernel_gsp_tu102.c:947-970`) or `ksec2ResetHw_TU102` (`kernel_sec2_tu102.c:62-83`):
   - read-modify-write `ENGINE`, setting RESET = 1 (`GPU_FLD_WR_DRF_DEF`)
   - 10 reads of `ENGINE` (`FLCN_RESET_PROPAGATION_DELAY_COUNT` = 10, `inc/kernel/gpu/falcon/falcon_common.h:64`)
   - read-modify-write `ENGINE`, setting RESET = 0
   - 10 more reads of `ENGINE`
3. `kflcnWaitForResetToFinish_TU102` (305-320): polls `base+DMACTL` until bits 1 and 2 are both 0 (`gpuTimeoutCondWait`, default timeout).
   - Returns `NV_ERR_GPU_IN_FULLCHIP_RESET` if the GPU is in reset.
   - On timeout, `kflcnReset` returns the error through `NV_ASSERT_OR_RETURN` (187).
4. `kflcnSwitchToFalcon_TU102`: software state only (219-230).
5. Write `base+RM (0x084)` = `pGpu->chipId0`. This is the pmcBoot0 value (`src/kernel/gpu_mgr/gpu_mgr.c:1324`; `kernel_falcon_tu102.c:191`).

**Start CPU** (`kflcnStartCpu_TU102`, 236-249): read `CPUCTL`. If ALIAS_EN (bit 6) is set, write `CPUCTL_ALIAS` = 0x2; otherwise write `CPUCTL` = 0x2.

**Wait for halt** (`kflcnWaitForHalt_TU102`, 332-359): poll `CPUCTL` bit 4 until HALTED, using `gpuCheckTimeout` and `osSpinLoop`. On timeout it returns `NV_ERR_TIMEOUT` and hits `DBG_BREAKPOINT`.

### Main table

| # | Function (file:line) | Macro | BAR0 address | Dir | Value written / condition checked | Timeout | On failure |
|---|---|---|---|---|---|---|---|
| **A1 VBIOS extraction** | `kgspExtractVbiosFromRom_TU102` `kernel_gsp_vbios_tu102.c:442-569` | | | | | | |
| A1.1 | `kbifPreOsGlobalErotGrantRequest` | — | — | — | No-op on TU (`g_kernel_bif_nvoc.h:1754`) | — | — |
| A1.2 | `s_romImgRead16(OFFSETOF_PCI_EXP_ROM_SIG=0)` (492) → `s_romImgReadGeneric` (99-169) → `s_promRead32` (79-86) | `GPU_REG_RD32_UNCHECKED(NV_PROM_DATA(off))` | `0x300000 + off` (`dev_ext_devices.h:27`, `NV_PROM_DATA(i) = 0x00300000+(i)`) | R | Aligned 32-bit reads; unaligned values are assembled from 1-2 dwords. Valid signatures: 0xAA55, 0x4E56, 0xBB77 (`pci_exp_table.h:31-36`) | none | status error |
| A1.3 | If the signature is invalid: `s_romImgFindPciHeader_TU102` (202-273) | same | 0x300000, 0x300004, 0x300008 (`NV_PBUS_IFR_FMT_FIXED0/1/2`, `dev_bus.h:30-37`) | R | FIXED0 must equal 0x4947564E ("NVGI"). IFR v1/v2: image offset = dword at `FIXED1[30:16] + 4`. IFR v3: `flashStatusOffset = dword(FIXED2[19:0])`; ROM directory at +4096 must be 0x44524652 ("RFRD"); image offset = dword(dir + 8) | none | `NV_ERR_INVALID_DATA` |
| A1.4 | `s_locateExpansionRoms` (276-416) | same | per image: +0x18 (PCI data pointer); PCIR +0 (signature), +0x0a (length), +0x10 (image length), +0x14 (code type), +0x15 (last image); NPDE at `(pcir + len + 0xF) & ~0xF` +0/+4/+6/+8/+0xa | R | Walks the images in 512-byte blocks. Code type 0x00 = base, 0xE0 = extension. `expansionRomOffset = extRomOffset - baseRomSize` | none | error |
| A1.5 | copy loop (540-550) | `s_promRead32` / `s_promRead08` | `0x300000 + pciOffset + i` | R | `biosSize` bytes, capped at 0x100000 (`s_getBaseBiosMaxSize_TU102`, 423-429); `maxOffset` limits the parser reads | none | error if larger than 1 MB |
| **A2 FWSEC parse** | `kgspParseFwsecUcodeFromVbiosImg_IMPL` (`fwsec.c:1110-1158`) | | | | | | |
| A2.1 | `kgspIsDebugModeEnabled_TU102` (`kernel_gsp_tu102.c:93-104`) | `NV_FUSE_OPT_SECURE_GSP_DEBUG_DIS` | 0x0002174C (`dev_fuse.h:26`) | R | bit 0 == 0 means debug. Debug selects APPID 0x45; production selects 0x85; 0x05 is always accepted (`fwsec.c:125-127, 620-624`) | — | — |
| **A3 Booter image allocation** | `s_allocateUcodeFromBinArchive` (`kernel_gsp_booter.c:216`), called for Load and again for Unload | `NV_FUSE_OPT_SECURE_GSP_DEBUG_DIS` | 0x2174C | R ×2 | Selects DBG or PROD images | — | — |
| **A5 GFW boot wait** | `gpuWaitForGfwBootComplete_TU102` (`kern_gpu_tu102.c:447-481`) | | | | | | |
| A5.1 | `kflcnWaitForHalt_HAL(GSP, timeout, OSTIMER)` (461) | `NV_PFALCON_FALCON_CPUCTL` | 0x110100 | Poll | bit 4 (HALTED) == 1 | `gpuScaleTimeout(50 ms + 2 s)` = 2.05 s (437-440) | `NV_ERR_TIMEOUT`: "GSP failed to halt with GFW_BOOT" |
| A5.2 | `_gpuIsGfwBootCompleted_TU102` (391-431), read regardless of A5.1's result | `NV_PGC6_AON_SECURE_SCRATCH_GROUP_05_PRIV_LEVEL_MASK` | 0x00118128 (`dev_gc6_island.h:28`) | R | bit 0 (READ_PROTECTION_LEVEL0) == 1 | — | treated as "not complete" |
| A5.3 | same | `NV_PGC6_AON_SECURE_SCRATCH_GROUP_05(0)` (`_0_GFW_BOOT`) | 0x00118234 (`dev_gc6_island.h:34`, `addendum.h:30-32`) | R | bits 7:0 == 0xFF (`_PROGRESS_COMPLETED`). Only read if A5.2 passed | — | `NV_ERR_NOT_READY` |
| **A6 PTIMER** | `tmrSetCurrentTime_GV100` (`src/kernel/gpu/timer/arch/volta/timer_gv100.c:47-85`) | | | | | | |
| A6.1 | | `NV_PTIMER_TIME_PRIV_LEVEL_MASK` | 0x00009430 | R | bit 4 must be 1 | — | assert; returns `NV_ERR_PRIV_SEC_VIOLATION`, which the caller ignores |
| A6.2 | | `NV_PTIMER_TIME_1` | 0x00009410 | W | high 32 bits of wall-clock ns | — | — |
| A6.3 | | `NV_PTIMER_TIME_0` | 0x00009400 | W | low 32 bits. This write triggers the load | — | — |
| **B _kgspBootGspRm** | | | | | | | |
| B1 | `kmemsysCheckReadoutEccEnablement_GV100` | `NV_PGRAPH_PRI_FECS_FEATURE_READOUT` | 0x00409660 | R | bit 16 (ECC_DRAM) | — | — |
| B2 | `kgspIsWpr2Up_TU102` (`kernel_gsp_tu102.c:1172-1181`) | `NV_PFB_PRI_MMU_WPR2_ADDR_HI` | 0x001FA828 (`dev_fb.h:42`) | R | VAL (31:4) must be 0 | — | `NV_ERR_INVALID_STATE` |
| B3.1 | `kgspPopulateWprMeta_TU102` → `kmemsysReadUsableFbSize_GP102` (`kern_mem_sys_gp102.c:36-58`) | `NV_PFB_PRI_MMU_LOCAL_MEMORY_RANGE` | 0x00100CE0 (`pascal/gp102/dev_fb.h:26`) | R | `fbSize = MAG[9:4] << (SCALE[3:0] + 20)`; ×15/16 if ECC_MODE bit 30 | — | — |
| B3.2 | `gpuFuseSupportsDisplay_GM107` | `NV_FUSE_STATUS_OPT_DISPLAY` | 0x00021C04 | R | DATA == ENABLE | — | — |
| B3.3 | `kdispGetVgaWorkspaceBase_v04_00` (`src/kernel/gpu/disp/arch/v04/kern_disp_0400.c:46-62`), only if display is fused on | `NV_PDISP_VGA_WORKSPACE_BASE` | 0x00625F04 (`disp/v04_00/dev_disp.h:27`) | R | if STATUS bit 3 is valid: offset = ADDR[31:8] << 16 | — | — |
| B3.4 | `memmgrReadMmuLock` | — | — | — | TU stub returns invalid (`g_mem_mgr_nvoc.h:3169`) | — | — |
| B3.5 | `kgspGetWprEndMargin` → `kgspGetFwHeapSize(0,0)` → `_kgspCalculateFwHeapSize`; then `kgspGetFwHeapSize(pre, post)` (`kernel_gsp.c:6552-6608, 6417-6550`) | `LOCAL_MEMORY_RANGE` | 0x100CE0 | R ×2 | used for the heap size | — | — |
| B5 | `kgspPrepareForBootstrap_TU102` → `kflcnIsRiscvCpuEnabled_TU102` (`kernel_falcon_tu102.c:124-133`) | `NV_PFALCON_FALCON_HWCFG2` | 0x1100F4 | R | bit 10 == 1 | — | `NV_ERR_NOT_SUPPORTED` |
| **C kgspBootstrap_TU102** | | | | | | | |
| C1 | `kflcnReset_HAL(GSP)` (550) | RESET-SEQ | 0x1103C0 (read-modify-write, 10 reads, read-modify-write, 10 reads); poll 0x11010C; write 0x110084 | R/W/Poll | RESET=1, then 0; DMACTL[2:1] == 0; RM = pmcBoot0 | default | error returned |
| **C2 FWSEC-FRTS on the GSP falcon** (boot type WITH_LOADER, V2 descriptor) | `s_prepareHsFalconWithLoader` (`kernel_gsp_falcon_tu102.c:322-361`) → `s_setupLoader` (260-319) | | | | | | |
| C2.1 | `kflcnDisableCtxReq_TU102` (`kernel_falcon_tu102.c:255-271`) | `NV_PFALCON_FBIF_CTL` | 0x110624 | read-modify-write | set bit 7 (ALLOW_PHYS_NO_CTX) | — | — |
| C2.2 | same | `NV_PFALCON_FALCON_DMACTL` | 0x11010C | W | 0x0 | — | — |
| C2.3 | `s_setupLoaderAperture(ctxDma=4)` (219-257) | `NV_PFALCON_FBIF_TRANSCFG(4)` | 0x110610 | read-modify-write | TARGET = COHERENT_SYSMEM (1), because the call passes `ADDR_SYSMEM` + `NV_MEMORY_CACHED` (358); MEM_TYPE = PHYSICAL (bit 2) | — | — |
| C2.4 | `s_dmemCopyTo_TU102(0, blDmemDesc, sizeof)` (52-90) | `DMEMC(0)` | 0x1101C0 | W | 0x01000000 (offset 0, AINCW) | — | — |
| C2.5 | same | `DMEMD(0)` | 0x1101C4 | W ×21 | `RM_FLCN_BL_DMEM_DESC` (84 bytes, **derived**; contents in §3.7) | — | — |
| C2.6 | block computation (300-303) | `NV_PFALCON_FALCON_HWCFG` | 0x110108 | R | `imemDstBlk = IMEM_SIZE[8:0] - 0x200/256` = IMEM_SIZE - 2 | — | — |
| C2.7 | `s_imemCopyTo_TU102(imemDstBlk<<8, blImg, 0x200, !secure, 0xfd00)` (104-157) | `IMEMC(0)` | 0x110180 | W | `(imemDstBlk << 8) \| 0x01000000` | — | — |
| C2.8 | same | `IMEMT(0)` / `IMEMD(0)` | 0x110188 / 0x110184 | W | tag 0xFD, then 64 words; tag 0xFE, then 64 words. Tag is written every 64 words (`FALCON_IMEM_BLKSIZE2=8`) | — | — |
| C2.9 | (316) | `NV_PFALCON_FALCON_BOOTVEC` | 0x110104 | W | 0xFD00 (`blStartTag << 8`; `blStartTag = 0xfd`, `generated/g_bindata_ksec2GetBinArchiveBlUcode_TU102.c:92-100`) | — | — |
| C2.10 | `kgspExecuteHsFalcon_TU102` (417-420) | — | — | — | **No mailbox writes** (FWSEC passes NULL) | — | — |
| C2.11 | `kflcnStartCpu` | `CPUCTL` then `CPUCTL_ALIAS` or `CPUCTL` | 0x110100 R; 0x110130 or 0x110100 W | R/W | 0x2 | — | — |
| C2.12 | `kflcnWaitForHalt(GPU_TIMEOUT_DEFAULT)` (426) | `CPUCTL` | 0x110100 | Poll | bit 4 == 1 | default | `NV_ERR_TIMEOUT` → "failed to execute FWSEC cmd" |
| **C3 FWSEC result checks** | `kgspExecuteFwsec_TU102` (`kernel_gsp_frts_tu102.c:489-526`) | | | | | | |
| C3.1 | (497-504) | `NV_PBUS_VBIOS_SCRATCH(0x0E)` | 0x00001438 (`dev_bus.h:27`: 0x1400 + i·4) | R | bits 31:16 (FRTS_ERR_CODE) == 0 | — | `NV_ERR_GENERIC` |
| C3.2 | (506-513) | `NV_PFB_PRI_MMU_WPR2_ADDR_HI` | 0x1FA828 | R | VAL[31:4] != 0 | — | `NV_ERR_GENERIC` |
| C3.3 | (515-525) | `NV_PFB_PRI_MMU_WPR2_ADDR_LO` | 0x1FA824 (`dev_fb.h:39`) | R | VAL[31:4] == `frtsOffset >> 12` (`_LO_ALIGNMENT = 0xc`) | — | `NV_ERR_GENERIC` |
| C4 | `kflcnResetIntoRiscv_TU102(GSP)` (559) | RESET-SEQ on GSP | 0x1103C0, 0x11010C, 0x110084 | same as C1 | same as C1 | default | error |
| C5 | `kgspProgramLibosBootArgsAddr_TU102` (401-402) | `NV_PGSP_FALCON_MAILBOX0` / `MAILBOX1` | 0x110040 / 0x110044 (`dev_gsp.h:27,29`) | W | low / high 32 bits of the libos init-args PA (`pLibosInitArgumentsDescriptor`, AT_GPU) | — | — |
| **C6 Booter-Load on SEC2** | `kgspExecuteBooterLoad_TU102` (`kernel_gsp_booter_tu102.c:82-122`) | | | | | | |
| C6.1 | `kflcnReset_HAL(SEC2)` (109) | RESET-SEQ | 0x8403C0 read-modify-write ×2 with 10 reads each; poll 0x84010C; write 0x840084 = pmcBoot0 | | | default | error |
| C6.2 | `s_executeBooterUcode_TU102` (50-51) | SEC2 `MAILBOX0` / `MAILBOX1` | 0x840040 / 0x840044 | R | informational only | — | — |
| C6.3 | `s_prepareHsFalconDirect` (160-202) → `kflcnDisableCtxReq` | SEC2 FBIF `CTL`; `DMACTL` | 0x840624 (read-modify-write, bit 7); 0x84010C (W 0) | | | | |
| C6.4 | non-secure IMEM: `s_imemCopyTo(0, img+0, 0x100, NS, tag 0)` | `IMEMC(0)` / `IMEMT(0)` / `IMEMD(0)` | 0x840180 / 0x840188 / 0x840184 | W | IMEMC = 0x01000000; IMEMT = 0; 64 data words | — | — |
| C6.5 | secure IMEM: `s_imemCopyTo(0x100, img+0x100, 0x8400, SECURE, tag 0x100)` | same | same | W | IMEMC = 0x11000100 (offset 0x100, AINCW, SECURE bit 28); IMEMT = 1, 2, …, 0x84 (one per 256 bytes); 0x2100 data words | — | — |
| C6.6 | DMEM: `s_dmemCopyTo(0, img+0x8500, 0x6200)` | `DMEMC(0)` / `DMEMD(0)` | 0x8401C0 / 0x8401C4 | W | DMEMC = 0x01000000; 0x1880 words. The DMEM image has the signature patched at DMEM 0x200 (§3.7, §5) | — | — |
| C6.7 | (199) | `BOOTVEC` | 0x840104 | W | 0 | — | — |
| C6.8 | `kgspExecuteHsFalcon_TU102` (417-420) | SEC2 `MAILBOX0` / `MAILBOX1` | 0x840040 / 0x840044 | W | low / high 32 bits of the **GspFwWprMeta sysmem PA** (`_kgspGetBooterLoadArgs`, `kernel_gsp_tu102.c:486-487`) | — | — |
| C6.9 | `kflcnStartCpu(SEC2)` | `CPUCTL` / `CPUCTL_ALIAS` | 0x840100 R; 0x840130 or 0x840100 W | | 0x2 | | |
| C6.10 | `kflcnWaitForHalt(SEC2, default)` | `CPUCTL` | 0x840100 | Poll | bit 4 == 1 | default | returns the status (mailboxes are still read) |
| C6.11 | (429-432) | SEC2 `MAILBOX0` / `MAILBOX1` | 0x840040 / 0x840044 | R | MAILBOX0 must be 0 (`kernel_gsp_booter_tu102.c:72-76`) | — | `NV_ERR_GENERIC` "Booter failed with non-zero error code" |
| **C7 Init RPCs** | `kgspSendInitRpcs` → `_issueRpcAsync` → `_kgspRpcSendMessage` (`kernel_gsp.c:~390-428`) → `GspMsgQueueSendCommand` (`message_queue_cpu.c:478`) | sysmem command queue | — | sysmem W | waits up to 1 s per element for queue space (562-579) | 1 s | `NV_ERR_TIMEOUT` / `BUSY_RETRY` |
| C7.1 | `kgspSetCmdQueueHead_TU102(queueIdx=0, 0)` (`kernel_gsp.c:424` → `kernel_gsp_tu102.c:372-386`) | `NV_PGSP_QUEUE_HEAD(0)` | 0x00110C00 (`dev_gsp.h:38`: 0x110c00 + i·8) | W | **0**, once after each RPC. Two writes here (SET_SYSTEM_INFO, SET_REGISTRY; SET_REGISTRY may be a multi-record "large" RPC) | — | — |
| C8 | (588) | GSP `NV_PFALCON_FALCON_OS` | 0x110080 | W | `pGspRmBootUcodeDesc->appVersion`. This is **0** in the TU102 GSP-RM boot descriptor (**derived** by decompressing the bindata, §5) | — | — |
| C9 | `kflcnIsRiscvActive_TU102` (`kernel_falcon_tu102.c:139-150`) | `NV_PRISCV_RISCV_CORE_SWITCH_RISCV_STATUS` | 0x111240 (0x111000 + 0x240) | R | bit 0 (ACTIVE_STAT) == 1 | — | falls through to C10 |
| C10 | `_kgspIsProcessorSuspended` (336-349) | GSP `MAILBOX0` | 0x110040 | R | bit 31 set (`INTERRUPT_PROCESSOR_SUSPENDED_VALUE 0x80000000`, 333) | — | `NV_ERR_NOT_READY` "Failed to boot GSP" |
| **C11 Status queue link** | `GspStatusQueueInit` (`message_queue_cpu.c:339-415`) | sysmem poll of `msgqRxLink` | — | sysmem R | the GSP-written `msgqTxHeader` at status-queue base must validate (§4) | **4 s** (345) | `NV_ERR_TIMEOUT`; or `NV_ERR_RESET_REQUIRED` if the health check fails |
| C11.1 | every loop iteration → `kgspHealthCheck_TU102` (989) → `crashcatEngineGetNextCrashReport` → `crashcatEngineLoadWayfinder` (`src/libraries/crashcat/crashcat_engine.c:39-55`) | GSP `NV_PFALCON_FALCON_DEBUGINFO` (WFL0, `kcrashcatEngineGetWFL0Offset_TU102`) | 0x110094 | R | if [15:0] == 0xDEAD (`NV_CRASHCAT_SIGNATURE`, `nv-crashcat.h:50,317`), a crash report is decoded; a report affecting GSP-RM marks the boot fatal | — | fatal |
| **C12 Wait for INIT_DONE** | `kgspWaitForRmInitDone_IMPL` (6264-6313) → `_kgspRpcRecvPoll` (2848-3110) | sysmem status queue | — | sysmem R | event `NV_VGPU_MSG_EVENT_GSP_INIT_DONE` (0x1001, `inc/kernel/vgpu/rpc_global_enums.h:254`), sequence 0; then `rpc_result == 0` | `defaultus × 1.5` (2927), i.e. 6 s in graphics mode; `BYPASS_THREAD_STATE` | `NV_ERR_TIMEOUT` (with timeout classification) or the `rpc_result` error |
| C12.1 | every drain → `kgspHealthCheck` | `DEBUGINFO` | 0x110094 | R | as C11.1 | | `NV_ERR_RESET_REQUIRED` |

**Registers the TU102 GSP boot path does not touch:**
- `NV_PMC_ENABLE`: `pmcEnableMask = 0`, `bIsPmcDeviceEngine = FALSE` for both GSP and SEC2 (`kernel_gsp_tu102.c:75-76`, `kernel_sec2_tu102.c:52-53`).
- `NV_PMC_BOOT_42` is only read earlier, in `osinit.c:1539` (`NV_PMC_BOOT_42` = 0xA00, `src/common/inc/swref/published/nv_ref.h:150`).
- `NV_PGC6_BSI_SECURE_SCRATCH_14` (0x1180F8) is read only in the resume path (`kgspExecuteCoreResume_TU102`, `kernel_gsp_falcon_tu102.c:441-452`).
- `NV_PGC6_BSI_SECURE_SCRATCH_15` is not referenced anywhere in this path.
- `NV_PBUS_VBIOS_SCRATCH(0x15)` (0x1454) and the GFW PLM/progress checks at `frts.c:532-555` belong to FWSEC-SB at teardown only (`kgspTeardown_TU102`, `kernel_gsp_tu102.c:660`).

---

## 3. Sysmem structures the CPU prepares, and how their addresses reach the GPU

All addresses are `memdescGetPhysAddr(..., AT_GPU, 0)`, i.e. the DMA/IOMMU address.

### 3.1 GspFwWprMeta

Defined in `arch/nvalloc/common/inc/gsp/gsp_fw_wpr_meta.h`; `ct_assert` size 256 (`kernel_gsp_tu102.c:772`). Allocated as 4 KB contiguous sysmem, cached (`kgspAllocBootArgs_TU102`, 124-146), and zeroed. **Its PA reaches the GPU through SEC2 MAILBOX0/1 (0x840040/0x840044) before Booter-Load starts** (step C6.8).

Field values are set in `kgspPopulateWprMeta_TU102` (755-939). Offsets are **derived** from natural alignment; total 0x100.

| Offset | Field | Value / derivation |
|---|---|---|
| 0x00 | magic | `GSP_FW_WPR_META_MAGIC` 0xdc3aae21371a60b3 (h:212; set at 907) |
| 0x08 | revision | `GSP_FW_WPR_META_REVISION` 1 (h:211; 906) |
| 0x10 | sysmemAddrOfRadix3Elf | PA of the level-0 page of `pGspUCodeRadix3Descriptor` (864-865) |
| 0x18 | sizeOfRadix3Elf | `pGspFw->imageSize`, the **`.fwimage` section size** (814) |
| 0x20 | sysmemAddrOfBootloader | PA of `pGspRmBootUcodeMemdesc` (868-869) |
| 0x28 | sizeOfBootloader | `gspRmBootUcodeSize`, the uncompressed bindata size, 4096 on TU102 (823) |
| 0x30 | bootloaderCodeOffset | `pRiscvDesc->monitorCodeOffset` (872) |
| 0x38 | bootloaderDataOffset | `pRiscvDesc->monitorDataOffset` (873) |
| 0x40 | bootloaderManifestOffset | `pRiscvDesc->manifestOffset` (874) |
| 0x48 | sysmemAddrOfSignature | PA of `pSignatureMemdesc` (878) |
| 0x50 | sizeOfSignature | `memdescGetSize`, i.e. `ALIGN_UP(signatureSize, 256)` (879; `kernel_gsp.c:5684-5686`) |
| 0x58 | gspFwRsvdStart | = nonWprHeapOffset (861) |
| 0x60 | nonWprHeapOffset | `gspFwWprStart - nonWprHeapSize` (859) |
| 0x68 | nonWprHeapSize | `ALIGN_UP(kgspGetNonWprHeapSize, 1MB)`; TU returns 1048576 (`g_kernel_gsp_nvoc.h:1885`), so 1 MB (837, 858) |
| 0x70 | gspFwWprStart | `gspFwHeapOffset - ALIGN_UP(256, 1MB)` = heapOffset − 1 MB (838, 855) |
| 0x78 | gspFwHeapOffset | `ALIGN_DOWN(gspFwOffset - wprHeapSize, 1MB)` (844) |
| 0x80 | gspFwHeapSize | `ALIGN_DOWN(gspFwOffset - gspFwHeapOffset, 1MB)` (845) |
| 0x88 | gspFwOffset | `ALIGN_DOWN(bootBinOffset - sizeOfRadix3Elf, 0x10000)` (830) |
| 0x90 | bootBinOffset | `ALIGN_DOWN(frtsOffset - sizeOfBootloader, 0x1000)` (824) |
| 0x98 | frtsOffset | `gspFwWprEnd - frtsSize` (820) |
| 0xA0 | frtsSize | 0x100 × 0x1000 = 1 MB (`NV_PGC6_AON_FRTS_INPUT_WPR_SIZE_SECURE_SCRATCH_GROUP_03_0_WPR_SIZE_1MB_IN_4K` 0x100, `kernel_gsp_frts_tu102.c:49-57`) |
| 0xA8 | gspFwWprEnd | `ALIGN_DOWN(vbiosReservedOffset - wprEndMargin, WPR_ALIGNMENT=128K)` (817; `gsp_init_args.h` `WPR_ALIGNMENT RM_PAGE_SIZE_128K`). `vbiosReservedOffset = vgaWorkspaceOffset` because the MMU lock is invalid on TU (808-811) |
| 0xB0 | fbSize | from `LOCAL_MEMORY_RANGE` (step B3.1) |
| 0xB8 | vgaWorkspaceOffset | If display is fused and `VGA_WORKSPACE_BASE` is valid: that value, but replaced by `fbSize - 0x20000` if it is below `fbSize - DRF_SIZE(NV_PRAMIN)`. Otherwise `fbSize - DRF_SIZE(NV_PRAMIN)` (787-801) |
| 0xC0 | vgaWorkspaceSize | `fbSize - vgaWorkspaceOffset` (802) |
| 0xC8 | bootCount | 0 (904) |
| 0xD0–0xEF | partition-RPC union / crash-report queue | `sysmemAddrOfCrashReportQueue` (0xE0) and `sizeOfCrashReportQueue` (0xE8) are set only if the CrashCat queue memdesc exists (883-890). GSP's CrashCat config sets no `allocQueueSize` (`kernel_gsp_tu102.c:80-82`; `kernel_crashcat_engine.c:52`), so these stay 0 |
| 0xF0 | gspFwHeapVfPartitionCount | 0 unless vGPU offload (848) |
| 0xF1 | flags | `GSP_FW_FLAGS_CLOCK_BOOST` (bit 0) set by default on TU (HAL field `bBootGspRmWithBoostClocks` = TRUE, `g_kernel_gsp_nvoc.c:505-509`); cleared by regkey or for 0x20BB/0x14A1 (892-912). `RECOVERY_MARGIN_PRESENT` (bit 1) comes from `kgspGetWprEndMargin` (6601/6606). `SCAN_RECOVERY_MARGIN` (bit 4) is set on retry (4836) |
| 0xF4 | pmuReservedSize | not set (0) |
| 0xF8 | verified | 0 (905). Booter sets 0xa0a0a0a0a0a0a0a0 (`GSP_FW_WPR_META_VERIFIED`, h:210); the driver does not read it back during boot |

**Heap sizing on TU** (`kgspGetFwHeapSize_IMPL`, `kernel_gsp.c:6494`; `_kgspCalculateFwHeapSize`, 6417):
- heap = `OsCarveout` (0 on TU, `g_kernel_gsp_nvoc.h:2136`) + `fwHeapParamBaseSize` (8 MB, `g_kernel_gsp_nvoc.c:490-493`) + `ALIGN_UP(96KB × memSizeGB, 1MB)` + `ALIGN_UP(48KB × 2048, 1MB)` (`inc/kernel/gpu/gsp/gsp_fw_heap.h:41,50,70`).
- Clamped to min 64 MB and max 256 MB (`g_kernel_gsp_nvoc.h:2112, 2124`).
- Since `bScrubberUcodeSupported = FALSE` on TU (`g_kernel_gsp_nvoc.c:478-487`), it is also clamped to `256MB − (pre + post)`, where pre = 2 MB (meta + non-WPR heap) and post = `ALIGN_UP(fbSize − gspFwOffset, 1MB)` (6509-6517; 836-841).
- For an 8 GB card, before clamping, this is 8 + 1 + 96 = 105 MB (**derived**).
- `kgspGetWprEndMargin` returns 0 on a first normal boot. With `bootAttempts > 0` (or regkey `APPLY_ALWAYS`) it returns `(gspFwWprEnd − nonWprHeapOffset from the previous attempt) × bootAttempts` (6552-6608).

### 3.2 Libos init-args array (`LibosMemoryRegionInitArgument[]`)

- Allocation: `LIBOS_MEMORY_REGION_INIT_ARGUMENTS_MAX` = 4096 bytes, contiguous, **uncached** (`kernel_gsp_tu102.c:151-175`; `src/common/uproc/os/common/include/libos_init_args.h:31`).
- **The PA reaches the GPU through GSP MAILBOX0 (low) and MAILBOX1 (high) at 0x110040/0x110044** (step C5). They are written after the RISC-V reset and before Booter-Load.
- Entry layout (`libos_init_args.h:48-56`; offsets **derived**, 0x20 stride):

| Offset | Field |
|---|---|
| 0x00 | id8 (u64) |
| 0x08 | pa (u64) |
| 0x10 | size (u64) |
| 0x18 | kind (u8): NONE 0, CONTIGUOUS 1, RADIX3 2 |
| 0x19 | loc (u8): NONE 0, SYSMEM 1, FB 2 |

- `id8` is built by `_kgspGenerateInitArgId` (`kernel_gsp.c:3219-3237`): up to 8 characters, `id = (id << 8) | c`.
- Contents on TU, filled by `kgspSetupLibosInitArgs_IMPL` (6199-6232):
  - LIBOS version is 2 (`g_kernel_gsp_nvoc.h:2160`).
  - `kgspGetLogCount` = `LOG_IDX_LIBOS2_SIZE` = 2 (`g_kernel_gsp_nvoc.h:2100, 317-319`).
  - Kernel logging is off (`g_kernel_gsp_nvoc.h:2087`).
  - Only LIBOS2-supported tasks are registered (`kernel_gsp.c:3751-3757`; `arch/nvalloc/common/inc/nv-firmware-chip-family-select.h:110-117`).

| Index | id8 string | kind / loc | pa | size |
|---|---|---|---|---|
| 0 | "LOGINIT" | CONTIGUOUS / SYSMEM | log buffer PA | 0x10000 (`nvrm_registry.h:3283`) |
| 1 | "LOGRM" | CONTIGUOUS / SYSMEM | log buffer PA | 0x10000 release, 0x40000 debug/develop (`nvrm_registry.h:3302-3307`) |
| 2 | "RMARGS" | CONTIGUOUS / SYSMEM | PA of the GSP_ARGUMENTS_CACHED page | 0x1000 |

- Each log buffer is cached contiguous sysmem. Word 0 is the put pointer; **word [1] holds the buffer's own PA** (`_setupLogBufferBaremetal`, `kernel_gsp.c:3622-3667`).

### 3.3 GSP_ARGUMENTS_CACHED ("RMARGS")

Defined in `inc/kernel/gpu/gsp/gsp_init_args.h`. Allocated as a 4 KB cached contiguous page (`kernel_gsp_tu102.c:178-202`) and filled by `kgspPopulateGspRmInitArgs_IMPL` (`kernel_gsp.c:5468-5553`). Offsets are **derived** (x86-64 packing, `NvLength` = u64).

- `messageQueueInitArguments` (0x00-0x3F):
  - `sharedMemPhysAddr` (0x00) = `pMQCollection->sharedMemPA` = `pPageTbl[0]`, the PA of the first page of the shared queue buffer (`message_queue_cpu.c:329`)
  - `pageTableEntryCount` (0x08) = 129 (**derived** for 256 KB + 256 KB queues: 128 pages + 1 page-table page; 125-137)
  - `cmdQueueOffset` (0x10) = `pageTableSize` = 0x1000
  - `statQueueOffset` (0x18) = 0x1000 + 0x40000 = 0x41000
  - `queueElementHdrSize` (0x20) = 16 (`offsetof(GSP_MSG_QUEUE_ELEMENT, payload)`; `inc/kernel/gpu/gsp/message_queue_priv.h:52-67`)
  - `queueElementSizeMin` (0x28) = 0x1000
  - `queueElementSizeMax` (0x30) = 0x10000
  - `queueHeaderAlign` (0x38) = 4 (log2)
  - `queueElementAlign` (0x3C) = 12 (log2) (`message_queue_cpu.c:82-91`)
- `srInitArguments` (0x40): oldLevel 0, flags 0, bInPMTransition 0 (5505-5507)
- `gpuInstance` (0x4C)
- `bDmemStack` (0x50) = 1 by default (regkey STACK_PLACEMENT default 0, `nvrm_registry.h:2832`; 5497-5503)
- `profilerArgs` (0x58): {pa, size}, 0 unless allocated
- `sysmemHeapArgs` (0x68) = 0 (not zero-FB)
- `rmStateMonitorBufferArgs` (0x78): {pa, size}, if allocated
- `bindataArgs` (0x88): {radix3, size}; set only if `ucodes_tu10x` was loaded (5548-5552)

### 3.4 Message-queue shared memory

Built by `GspMsgQueuesInit` (`message_queue_cpu.c:205-337`) at construction time. It is 0x81000 bytes, non-contiguous, cached, zeroed.

- `[0x0000, 0x1000)`: page table, an array of u64 PAs of every 4 KB page of the whole buffer (305-310)
- `[0x1000, 0x41000)`: CPU→GSP command queue. `msgqTxCreate` writes its `msgqTxHeader` there (`src/libraries/msgq/msgq.c:~236-316`):
  - version 0 (`MSGQ_VERSION`, `inc/libraries/msgq/msgq_priv.h:38`), size 0x40000, msgSize 0x1000
  - msgCount = (0x40000 − 0x1000) / 0x1000 = 63 (**derived**)
  - writePtr 0, flags `MSGQ_FLAGS_SWAP_RX` = 1, rxHdrOff 0x20, entryOff 0x1000 (**derived**)
- `[0x41000, 0x81000)`: GSP→CPU status queue, written by GSP.
- Element format: `GSP_MSG_QUEUE_ELEMENT` {mctpHeader, nvdmHeader(`NVDM_TYPE_RM_RPC`), checkSum, seqNum, payload = `rpc_message_header_v` + params}. The checksum makes `_checkSum32` equal 0 (`message_queue_cpu.c:505-546`).
- **This buffer's address reaches GSP only indirectly: MAILBOX → libos args → RMARGS → `sharedMemPhysAddr`.**

### 3.5 Radix3 image of the GSP-RM ELF

`kgspCreateRadix3_IMPL` (`kernel_gsp.c:5930-6095`) is called with the `.fwimage` section data and size (5891-5893):

- Allocation: non-contiguous sysmem, cached, 4 KB alignment. It tries huge-page GPU mappings first and falls back (6004-6024).
- Four levels:
  - `n[3] = ceil(size / 4K)`
  - `n[i-1] = ceil(n[i] / 512)`
  - `n[0]` must be 1
- Offsets: level 0 at 0, level 1 at 4K, level 2 at (1 + n1) × 4K, data at (1 + n1 + n2) × 4K.
- Level-0 entries are u64 PAs of level-1 pages; level-1 entries point to level-2 pages; level-2 entries point to the data pages (6045-6075).
- The data is copied in and the tail of the last page is zeroed.
- `sysmemAddrOfRadix3Elf` = PA at offset 0, carried in WPR meta.

### 3.6 Signature memdesc and GSP-RM boot ucode

- **Signature:** the `.fwsignature_tu10x` section. The prefix is `".fwsignature_"` (`g_kernel_gsp_nvoc.h:2188`) and the chip family string "tu10x" comes from `nv-firmware.h:71`. It is copied into a 256-byte-aligned, cached contiguous buffer of `ALIGN_UP(size, 256)` bytes (`kernel_gsp.c:5666-5713`). PA and size go into WPR meta at 0x48/0x50.
- **GSP-RM boot ucode:** bindata `kgspGetBinArchiveGspRmBoot_TU102` `UCODE_IMAGE`, copied into a 4 KB-aligned, cached contiguous buffer (`kernel_gsp.c:5562-5634`). PA goes into WPR meta at 0x20.

### 3.7 Falcon DMEM contents

**FWSEC (generic-BL path on the GSP falcon):** `RM_FLCN_BL_DMEM_DESC` at DMEM 0 (`arch/nvalloc/common/inc/rmflcnbl.h:~34-102`). Fields as built in `kernel_gsp_falcon_tu102.c:340-356`:

- `reserved[4]`: **not initialised** (stack memory)
- `signature[4]` = 0
- `ctxDma` = 4
- `codeDmaBase` = PA of the FWSEC code memdesc
- `nonSecureCodeOff/Size` = `imemNsPa` / `imemNsSize`
- `secureCodeOff/Size` = `imemSecPa` / `imemSecSize`
- `codeEntryPoint` = 0
- `dataDmaBase` = PA of the FWSEC data memdesc
- `dataSize` = `dmemSize`
- `argc` / `argv`: **not initialised**

How the FWSEC V2 memdescs are built (`fwsec.c:741-864`):
- Code buffer: `ALIGN_UP(IMEMLoadSize, 256)`; data buffer: `ALIGN_UP(DMEMLoadSize, 256)`; both 256-aligned, physically contiguous, **uncached** sysmem.
- Code comes from `image[descOffset + descSize ...]`, placed at `IMEMPhysBase`.
- Data comes from `image[descOffset + descSize + DMEMOffset ...]`, placed at `DMEMPhysBase`.
- `imemSecPa = IMEMSecBase − IMEMVirtBase + IMEMPhysBase`; `imemNsSize = IMEMLoadSize − IMEMSecSize`.

The data buffer is patched in `s_prepareForFwsec_TU102` → `s_vbiosPatchInterfaceData` (`frts.c:156-261, 411-438`):
- The FALCON_APPLICATION_INTERFACE header is at `InterfaceOffset`. The driver finds entry id 4 (DMEMMAPPER), sets `init_cmd = 0x15` (FRTS), and copies `FWSECLIC_FRTS_CMD` to `cmd_in_buffer_offset`.
- `FWSECLIC_FRTS_CMD` contents:
  - `readVbiosDesc` {version 1, size 24, gfwImageOffset 0, gfwImageSize 0, flags 2}
  - `frtsRegionDesc` {version 1, size 20, frtsRegionOffset4K = frtsOffset >> 12, frtsRegionSize 0x100, frtsRegionMediaType 2 (FB)} (311-331)

**Booter-Load on SEC2:** DMEM[0..0x6200) = image[0x8500..0xE700) with the signature patched in (§5). The only arguments are SEC2 MAILBOX0/1 = the WprMeta PA.

---

## 4. What the driver expects the firmware to produce, and when

1. **Before the driver starts:** the VBIOS devinit / FWSEC boot flow must leave:
   - GSP falcon `CPUCTL.HALTED` (0x110100 bit 4) = 1 within 2.05 s (A5.1)
   - `0x118128` bit 0 = 1 (PLM lowered)
   - `0x118234[7:0]` = 0xFF
   - `WPR2_ADDR_HI` (0x1FA828) VAL = 0 (B2)
   - Also `PTIMER_TIME_PRIV_LEVEL_MASK` (0x9430) bit 4 = 1 (A6)
2. **After FWSEC-FRTS on the GSP falcon:**
   - the falcon halts (0x110100 bit 4)
   - `NV_PBUS_VBIOS_SCRATCH(0xE)` (0x1438) bits 31:16 = 0
   - `WPR2_HI` VAL ≠ 0
   - `WPR2_LO` VAL (bits 31:4) = `frtsOffset >> 12`
   - No mailbox values are checked.
3. **After Booter-Load on SEC2:**
   - SEC2 halts (0x840100 bit 4) within the default timeout
   - SEC2 `MAILBOX0` (0x840040) = 0
   - MAILBOX1 is read but not checked.
4. **After Booter-Load returns and the two init RPCs are queued:**
   - `0x111240` bit 0 (RISC-V active) = 1, **or** GSP MAILBOX0 (0x110040) bit 31 set (processor suspended). Checked once, with no poll (C9/C10).
5. **Within 4 s**, GSP-RM must write its `msgqTxHeader` at the start of the status queue (shared + 0x41000) so that `msgqRxLink` passes (`msgq.c:330-420`):
   - version = 0
   - size = 0x40000
   - msgSize = 0x1000
   - `rxHdrOff >= 0x20`
   - `entryOff >= the CPU queue's rxHdrOff + 4`
   - `msgCount == (0x40000 − entryOff) / 0x1000`
   - flags bit 0 (SWAP_RX) makes read pointers swap (each side's readPtr lives in its own buffer's rxHdr).
6. **Within 1.5 × the default timeout:** a status-queue record whose `rpc_message_header_v.function == NV_VGPU_MSG_EVENT_GSP_INIT_DONE` (0x1001) and `sequence == 0`, followed by `rpc_result == 0` (`kernel_gsp.c:6281-6288`).
   - Its `rpc_init_done_v17_00.bIsD3Hot` is read (6293-6305).
   - Every record must have mctp version 1, NV vendor id, a zero checksum, and an incrementing `seqNum` starting at 0 (`message_queue_cpu.c:640-846`).
   - Earlier events are handled in the "POLL_BOOTUP" context. Without the API lock only these are allowed: UCODE_LIBOS_PRINT, GSP_LOCKDOWN_NOTICE, GSP_POST_NOCAT_RECORD, OS_ERROR_LOG, PFM_REQ_HNDLR_STATE_SYNC_CALLBACK, GSP_LOAD_EXEC_GENERIC_BOOTLOADER, GSP_LOAD_EXEC_HS_BINARY (`kernel_gsp.c:1418-1439`).
7. **Crash signalling:** `0x110094[15:0] = 0xDEAD` together with a wayfinder L1 pointer. It is polled during every health check.
8. **Interrupts** (not needed for boot, which polls): `kgspService_TU102` (1088-1169) reads IRQSTAT/IRQMASK/IRQDEST, or the RISC-V variants at 0x1112b4/0x1112b8 in RISC-V mode (`kernel_falcon_tu102.c:384-393`).
   - SWGEN0 (IRQSTAT bit 6) means a message: the driver clears it via `IRQSCLR` (0x110004) bit 6, then drains the queue.
   - HALT (bit 4) is the CrashCat notification: cleared via IRQSCLR bit 4.
9. **RPC sequence around boot** (briefly):
   - During bootstrap: `GSP_SET_SYSTEM_INFO` (72), then `SET_REGISTRY` (73), both async (`rpc_global_enums.h:82-83`).
   - After INIT_DONE, back in `kgspInitRm`: `SET_GUEST_SYSTEM_INFO` (1), then `GET_GSP_STATIC_INFO` (65) (`kernel_gsp.c:5153-5165`).

---

## 5. Driver bindata images used on TU106, and gsp_tu10x.bin sections

Sizes come from the `generated/g_bindata_*.c` headers. Small blobs were decoded **in memory** with a raw-deflate decompress (**derived**). No files were written.

| Archive (HAL) | Label | Size (uncompressed) | Notes |
|---|---|---|---|
| `kgspGetBinArchiveBooterLoadUcode_TU102` (TU102/TU104/TU106; `g_kernel_gsp_nvoc.c`) | IMAGE_PROD / IMAGE_DBG | 59136 (0xE700) each (`generated/g_bindata_kgspGetBinArchiveBooterLoadUcode_TU102.c:4326, 2156`) | HEADER (36 bytes) decodes to osCodeOffset 0, osCodeSize 0x100, osDataOffset 0x8500, osDataSize 0x6200, numApps 1, appCodeOffset 0x100, appCodeSize 0x8400, appDataOffset 0x100, appDataSize 0 |
| | SIG_PROD / SIG_DBG | 16 each | PROD = `32 60 43 67 98 89 7b 06 e8 5f 1a a8 71 43 5d cb` (4417); source `g_booteruc_load_tu10x_tu102_aes_sig.h` |
| | PATCH_LOC / PATCH_SIG / PATCH_META / NUM_SIGS | 4 / 4 / 12 / 4 | patchLoc = 0x8700 (→ DMEM 0x200); patchSig = 0; meta = {fuseVer 0, engineId 1, ucodeId 0xD}; numSigs = 1 |
| `kgspGetBinArchiveBooterUnloadUcode_TU102` | IMAGE_PROD / IMAGE_DBG | 39168 (0x9900) | header: osCodeSize 0x100, osDataOffset 0x4C00, osDataSize 0x4D00, appCodeOffset 0x100, appCodeSize 0x4B00; patchLoc 0x4E00; meta {0, 1, 0xD}; numSigs 1 |
| `kgspGetBinArchiveGspRmBoot_TU102` | UCODE_IMAGE | 4096 (`generated/g_bindata_kgspGetBinArchiveGspRmBoot_TU102.c:103`) | copied to sysmem; `sizeOfBootloader` |
| | UCODE_DESC (`RM_RISCV_UCODE_DESC`, 76 bytes) | 76 | decodes to version 4, bootloaderOffset 0, bootloaderSize 0x488, bootloaderParamOffset 0x488, bootloaderParamSize 0x10, **all remaining fields 0** (appVersion, manifestOffset, monitorData/CodeOffset, …) |
| `ksec2GetBinArchiveBlUcode_TU102` (generic falcon bootloader, used for FWSEC on GSP) | UCODE_IMAGE | 768 | DESC `RM_FLCN_BL_DESC` = {blStartTag 0xFD, blDmemDescLoadOff 0, {blCodeOffset 0, blCodeSize 0x200, blDataOffset 0x200, blDataSize 0x100}} (`g_bindata_ksec2GetBinArchiveBlUcode_TU102.c:92-100`); source `g_sec2_bl_gp10x.h` |
| Scrubber | — | — | none on TU; `ksec2GetBinArchiveSecurescrubUcode` is an assert stub |

Booter ucode handling (`kernel_gsp_booter.c`):
- DBG or PROD is chosen by the fuse at 0x2174C (216-227).
- `numApps` must be 1 and `patchSig` must be 0.
- `ksec2ReadUcodeFuseVersion` returns 0 on TU (`g_kernel_sec2_nvoc.h:824`). With numSigs = 1 the single signature is copied to `image + patchLoc` (131-170).
- Boot type is DIRECT because `bBootFromHs = FALSE` (374-411).

**gsp_tu10x.bin ELF sections parsed** (`_kgspFwContainerGetSection`, `kernel_gsp.c:6098-6193`; requires ELF64 little-endian, magic 0x464C457F):
- `.fwversion`: must equal `NV_VERSION_STRING` plus NUL, i.e. "610.57.04" (5720-5774, 5848-5864)
- `.fwimage`: the GSP-RM ELF image that goes into radix3 (5866-5872)
- `.fwsignature_tu10x` (5874-5885)
- `.note.gnu.build-id` (5904-5924)
- Section name defines: `generated/g_kernel_gsp_nvoc.h:273-277`
- The optional `gsp_log_tu10x.bin` is used only by the log decoder (`.fwlogging_init`, `.fwlogging_rm`).

---

## 6. Open questions and ambiguities

1. **FWSEC DMA aperture.** `ctxDma = 4` is commented "dmaIdx for PHYS_SYS_NCOH", but `s_setupLoader` is called with `NV_MEMORY_CACHED`, which programs `TRANSCFG(4)` TARGET = COHERENT_SYSMEM (`kernel_gsp_falcon_tu102.c:344, 358, 235-238`). The FWSEC memdescs themselves are allocated uncached (`fwsec.c:813-818`).
2. **Uninitialised BL DMEM fields.** `RM_FLCN_BL_DMEM_DESC.reserved[4]`, `argc` and `argv` are never set before the struct is copied to DMEM (`kernel_gsp_falcon_tu102.c:329-358`).
3. **FWSEC V3 descriptors are not handled on TU.** A V3 descriptor produces `KGSP_FLCN_UCODE_BOOT_FROM_HS` (`fwsec.c:908`), but `kgspExecuteHsFalcon_TU102` only supports WITH_LOADER and DIRECT and returns `NV_ERR_NOT_SUPPORTED` (`kernel_gsp_falcon_tu102.c:398-409`). A TU VBIOS is therefore expected to carry a V2 FWSEC; the source does not state this.
4. **FWSEC parse failure is reported as success.** `s_vbiosNewFlcnUcodeFromDesc` returns `NV_OK` even when filling fails, leaving `*ppFlcnUcode = NULL` (`fwsec.c:1080-1094`). The failure then surfaces later as an `INVALID_ARGUMENT` assert in `s_prepareForFwsec_TU102` (`frts.c:295`).
5. **Interface patch quirks.** `s_vbiosPatchInterfaceData` bounds-checks `pIntFaceEntry->dmemOffset + cmdBufferSize` rather than `cmd_in_buffer_offset + size` before copying (`frts.c:251-258`). It only prints an error when `cmd_in_buffer_size` is too small and does not fail (241-244).
6. **GFW-boot precondition.** The wait requires the **GSP falcon to be HALTED** (0x110100 bit 4) before the PGC6 registers are checked (`kern_gpu_tu102.c:461`). The VBIOS ROM is read through the PROM window before this wait (`kernel_gsp.c:4922` vs 5032).
7. **Retry on non-ECC boards.** With ECC disabled (typical RTX 2070), any boot failure sets `bRetry` (`kernel_gsp.c:4846-4853`). If FWSEC already created WPR2, the next attempt fails immediately at the WPR2-up check (4805-4811). No WPR2 teardown happens between attempts in this path.
8. **Queue-head register semantics.** `NV_PGSP_QUEUE_HEAD(0)` is always written with 0 after each command-queue send (`kernel_gsp.c:424`). How GSP uses it (doorbell or interrupt) is not visible in the driver.
9. **How GSP RISC-V is started.** The driver never writes GSP RISC-V CPUCTL or boot vectors on TU. Booter-Load, given the WprMeta PA, is presumably what loads the boot binary and GSP-RM into WPR2 and starts the core. This is not visible in the source (`kernel_gsp_tu102.c:405-423` comments only).
10. **appVersion and monitor offsets decode as 0.** The decompressed TU102 GSP-RM boot descriptor has appVersion = 0 (so FALCON_OS is written with 0) and monitorCode/Data/manifest offsets = 0. This is derived from `g_bindata_kgspGetBinArchiveGspRmBoot_TU102.c` and should be verified against a real dump.
11. **Heap-size ordering.** `kgspGetWprEndMargin` calls `kgspGetFwHeapSize(0,0)` even on a normal boot where it then returns 0 (`kernel_gsp.c:6588, 6605-6607`). This only causes extra `LOCAL_MEMORY_RANGE` reads.
12. **Timer PLM failure is ignored.** If `tmrSetCurrentTime` fails the PLM check, the error is dropped (`kernel_gsp.c:5039`).
13. **Scratch registers not used on this path.** The `NV_PGC6_BSI_SECURE_SCRATCH_14` handoff bit (26) is only used when resuming via SEC2 (`kernel_gsp_falcon_tu102.c:441-452`). `SECURE_SCRATCH_15` is not referenced. `NV_PGC6_AON_SECURE_SCRATCH_GROUP_03(0)` (0x118214) is only used as the source of the 1 MB constant and is never read.
14. **Health-check scope.** The CrashCat wayfinder (0x110094) is the only hardware register `kgspHealthCheck` reads during boot. The L1 wayfinder scratch-group map comes from `NV_CRASHCAT_DEF_SCRATCH_GROUP_V1_REGMAP_TABLE`, which I did not expand (`src/kernel/gpu/falcon/arch/turing/kernel_crashcat_engine_tu102.c:60-70`).
