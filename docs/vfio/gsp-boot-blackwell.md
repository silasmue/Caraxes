<!-- SPDX-License-Identifier: MIT -->
<!-- Source survey written 2026-09-24 from vendor/open-gpu-kernel-modules at tag 610.57.04, the guest
     driver in use. Every claim cites file:line; values marked "derived" were computed, not read.
     HAL choices were resolved from the generated NVOC tables for ChipHal GB206
     (chipHal_HalVarIdx 68, generated/g_chips2halspec_nvoc.c:144-147; kernel RM variant
     PF_KERNEL_ONLY, :213). Treat as notes to be verified by the N gate
     (blackwell-gate.md), not as specification. -->
# NVIDIA open-gpu-kernel-modules 610.57.04: how the driver boots GSP-RM on GB206 (RTX 5060 Ti) through FSP

The Blackwell counterpart of [gsp-boot-turing.md](gsp-boot-turing.md), for the device model
`crates/vfio-user-nvgpu/src/chip/boot_fsp.rs` and its tables `chips/gb206/boot.toml`.
On GB206 the kernel RM does not load any firmware into a falcon itself: FSP (the
security processor) boots GSP-FMC from a Chain-of-Trust (COT) message, ACR lays out the
WPR, and GSP-RM starts. The CPU sends messages, polls status registers and queues its init
RPCs; everything it observes is a register or a sysmem structure the device can answer.

- Root: `/home/silas/git/Caraxes/vendor/open-gpu-kernel-modules`. Paths are relative to
  `src/nvidia/` unless they start with `src/common`; `hdr:` means
  `src/common/inc/swref/published/`.
- Offsets come from the header the implementing `.c` file includes: `kern_fsp_gh100.c`
  includes `hdr:hopper/gh100/dev_fsp_pri.h` (:42) even on GB206, `kernel_gsp_gh100.c`
  includes `hdr:hopper/gh100/dev_falcon_v4.h`.

## 1. Call tree (GB206 HALs)

```
RmInitAdapter                                   arch/nvalloc/unix/src/osinit.c
 └─ kgspInitRm_IMPL                              src/kernel/gpu/gsp/kernel_gsp.c:4878
     ├─ kgspExtractVbiosFromRom_HAL = _395e98    generated/g_kernel_gsp_nvoc.c:1388-1402 (NOT_SUPPORTED, no ROM read, no FWSEC)
     ├─ bPartitionedFmc = TRUE                   g_kernel_gsp_nvoc.c:470 (no Booter images; kernel_gsp.c:4970)
     ├─ kgspPrepareBootBinaryImage               kernel_gsp.c:5003 -> kgspIsDebugModeEnabled_GB100 (fuse read, §2 A1)
     ├─ kgspWaitForGfwBootOk_GH100               kernel_gsp.c:5032; g_kernel_gsp_nvoc.c:1623
     │    └─ kfspWaitForSecureBoot_GB202          g_kern_fsp_nvoc.c:494-497; src/kernel/gpu/fsp/arch/blackwell/kern_fsp_gb202.c:64-109
     ├─ tmrSetCurrentTime_GH100                   kernel_gsp.c:5039
     ├─ kfspCheckForClockBoostCapability_GB100    kernel_gsp.c:5042-5046; kern_fsp_gb100.c:171-195 (FSP message CAPS_QUERY)
     └─ _kgspBootGspRm                            kernel_gsp.c:4796
         ├─ kmemsysCheckReadoutEccEnablement_GB100  :4799
         ├─ kgspIsWpr2Up_GB100                      :4805; g_kernel_gsp_nvoc.c:1341; src/kernel/gpu/gsp/arch/blackwell/kernel_gsp_gb100.c:332
         ├─ kgspPopulateWprMeta_GH100               :4814; src/kernel/gpu/gsp/arch/hopper/kernel_gsp_gh100.c:304-397 (size requests only)
         ├─ kgspPrepareForBootstrap_GH100           :4822; kernel_gsp_gh100.c:795-835
         │    ├─ kgspSetupGspFmcArgs_GH100           kernel_gsp_gh100.c:419-497 (GSP_FMC_BOOT_PARAMS)
         │    └─ kfspPrepareBootCommands_GH100       src/kernel/gpu/fsp/arch/hopper/kern_fsp_gh100.c:1258-1498 (COT payload)
         └─ kgspBootstrap_GH100                     :4840; g_kernel_gsp_nvoc.c:731-748; kernel_gsp_gh100.c:853-1037
              ├─ gpuResetGspFmcErrorCode_GH100       g_gpu_nvoc.c:2492-2495
              ├─ kfspSendBootCommands_GH100          kern_fsp_gh100.c:1500-1566 (COT message and response)
              ├─ kfspWaitForGspTargetMaskReleased_GH100  kern_fsp_gh100.c:1069-1103
              ├─ kgspSendInitRpcs                    kernel_gsp.c:4686 (SET_SYSTEM_INFO, SET_REGISTRY; doorbell)
              ├─ _kgspLockdownReleasedOrFmcError     kernel_gsp_gh100.c:500-547 (poll)
              ├─ gpuCheckGspFmcErrorCode_GH100, MAILBOX0 == 0   kernel_gsp_gh100.c:986-995
              ├─ NV_PFALCON_FALCON_OS = appVersion   :1000
              ├─ kflcnIsRiscvActive_GA102            :1003; g_kernel_falcon_nvoc.c:397-404
              ├─ GspStatusQueueInit                  :1019-1022; src/kernel/gpu/gsp/message_queue_cpu.c:339
              └─ kgspWaitForRmInitDone               :1025; kernel_gsp.c:6264
```

The FSP-less branch `_kgspBootstrapGspFmc_GH100` (BCR DMA address registers and mailbox
writes, kernel_gsp_gh100.c:717-792) is not taken: GB206 has a KernelFsp
(generated/g_kern_fsp_nvoc.c:234) and `PDB_PROP_KFSP_DISABLE_GSPFMC` is set only by a
registry key (src/kernel/gpu/fsp/kern_fsp.c:150).

## 2. Register accesses in the order the CPU makes them

"Model" names what `boot_fsp.rs` answers; "latched" means no model owns the register and
BAR0 reads back what was written (bar0.rs), which is what the CPU needs there.

| # | Function (file:line) | BAR0 | Dir | Value / condition | Model |
|---|---|---|---|---|---|
| A1 | `kgspIsDebugModeEnabled_GB100` (kernel_gsp_gb100.c:296), also `kfspGetGspUcodeArchive` (kern_fsp_gh100.c:799) | 0x82074C = NV_FUSE0_PRI_BASE 0x820000 (hdr:hopper/gh100/hwproject.h:31, `gpuGetPrimaryFuseBaseAddr_GH100`, g_gpu_nvoc.c:2738) + NV_FUSE_ZB_OPT_SECURE_GSP_DEBUG_DIS 0x74C (hdr:blackwell/gb100/dev_fuse_zb.h:41-43) | R | bit 0: 1 = production images | const 1 |
| A2 | `kfspWaitForSecureBoot_GB202` (kern_fsp_gb202.c:50-63) | 0xAD00BC NV_THERM_I2CS_SCRATCH (hdr:blackwell/gb202/dev_therm.h:27) | Poll | all 32 bits == 0xFF (FSP_BOOT_COMPLETE_STATUS_SUCCESS, dev_therm_addendum.h:27-29); timeout max(5 s scaled, default), OS timer | const 0xFF |
| A2.1 | on timeout only | 0x8F0320-0x8F032C NV_PFSP_FALCON_COMMON_SCRATCH_GROUP_2(i) (hdr:blackwell/gb202/dev_fsp_pri.h:27) | R | error report | latched 0 |
| A3 | `tmrSetCurrentTime_GH100` (g_objtmr_nvoc.c:442; timer_gh100.c:48) | 0x118F58, 0x118F54 SEC_TIMER_TIME_1/0 (hdr:hopper/gh100/dev_gc6_island.h:27,31) | R | secTimerNs must be below the OS time | latched 0 |
| A3.1 | same | 0x118DF8, 0x118DF4 (dev_gc6_island.h:35-41) | W | offset hi, then lo with UPDATE_TRIGGER | latched |
| A4 | `kfspCheckForClockBoostCapability_GB100` (kern_fsp_gb100.c:171-195) | FSP EMEM/queues (§3) | msg | NVDM CAPS_QUERY (0x16), payload `subMessageId` 0x20 (fsp/fsp_caps_query_rpc.h:34-37) | FSP_RESPONSE, FSP_OK |
| B1 | `kmemsysCheckReadoutEccEnablement_GB100` (g_kern_mem_sys_nvoc.c:892; kern_mem_sys_gb100.c:216) | 0x823814 = 0x820000 + NV_FUSE_ZB_FEATURE_READOUT 0x3814 (dev_fuse_zb.h:26-29) | R | bit 16 ECC_DRAM; only feeds the retry margin | const 0 |
| B2 | `kgspIsWpr2Up_GB100` (kernel_gsp_gb100.c:332) | 0x88A828 = NV_HUBMMU0_PRI_BASE 0x880000 (hdr:blackwell/gb100/hwproject.h:36) + NV_HUBMMU_PRI_MMU_WPR2_ADDR_HI 0xA828 (hdr:blackwell/gb100/dev_hubmmu_base.h:94-95) | R | VAL 31:4 must be 0 (kernel_gsp.c:4805-4811; `PREINITIALIZED_WPR_REGION` false, g_gpu_nvoc.c:251) | 0 until the COT |
| B3 | `kgspPopulateWprMeta_GH100` -> `_kgspCalculateFwHeapSize` (kernel_gsp.c:6446) -> `kmemsysReadUsableFbSize_GA102` (g_kern_mem_sys_nvoc.c:368-371) | 0x1183A4 NV_PGC6_AON_SECURE_SCRATCH_GROUP_42 = NV_USABLE_FB_SIZE_IN_MB (hdr:ampere/ga102/dev_gc6_island.h:34, dev_gc6_island_addendum.h:33) | R | usable FB in MiB | FB size of the profile |
| B4 | `kfspPrepareBootCommands_GH100` (kern_fsp_gh100.c:1273) | 0xAD00BC | Poll | == 0xFF again | const 0xFF |
| B5 | clock boost, only if A4 answered FSP_OK (kern_fsp_gh100.c:1308; `kfspSendClockBoostRpc_GB100`, kern_fsp_gb100.c:254-265) | FSP EMEM/queues | msg | NVDM CLOCK_BOOST (0x20), `subMessageId` 1 = enable | FSP_RESPONSE, FSP_OK |
| C1 | `gpuResetGspFmcErrorCode_GH100` (g_gpu_nvoc.c:2492-2495), again in `kfspSendBootCommands_GH100` | 0x14DC NV_PBUS_SW_SCRATCH(0x37) (hdr:hopper/gh100/dev_bus.h:27, dev_bus_addendum.h:30) | W | 0 | latched |
| C2 | `kfspSendBootCommands_GH100` -> `kfspSendAndReadMessage` (kern_fsp.c:707-767) | FSP EMEM/queues | msg | NVDM COT (0x14), 860-byte payload, one 868-byte packet (§3) | ACR layout, FSP_RESPONSE FSP_OK, GSP "running" |
| C3 | `_kfspIsGspTargetMaskReleased` (kern_fsp_gh100.c:1069-1090) | 0x1100F4 = NV_PGSP 0x110000 (hdr:hopper/gh100/dev_gsp.h:25) + NV_PFALCON_FALCON_HWCFG2 0xF4 (hdr:hopper/gh100/dev_falcon_v4.h:39) | Poll (unchecked) | != 0 and (val & 0xFFFFFF00) != 0xBADF4100; default timeout; on timeout reads 0x14DC | 0x400 after the COT |
| C4 | `kgspSendInitRpcs` (kernel_gsp.c:4686) -> `kgspSetCmdQueueHead_TU102` (g_kernel_gsp_nvoc.c:704-707) | 0x110C00 NV_PGSP_QUEUE_HEAD(0) (hdr:turing/tu102/dev_gsp.h:38) | W | 0, once per RPC (SET_SYSTEM_INFO, SET_REGISTRY), before the lockdown is released (kernel_gsp_gh100.c:958) | doorbell event with the libos args of the FMC parameters |
| C5 | `_kgspLockdownReleasedOrFmcError` (kernel_gsp_gh100.c:500-547) | 0x110040 MAILBOX0 (dev_falcon_v4.h:27); 0x110044 MAILBOX1 (:30) only if MAILBOX0 equals the FMC-args PA low half | Poll | done when HWCFG2 bit 13 RISCV_BR_PRIV_LOCKDOWN == 0 (UNLOCK, dev_falcon_v4.h:79-81) or MAILBOX0 != 0 | MAILBOX0 0; bit 13 clear after the COT |
| C6 | `gpuCheckGspFmcErrorCode_GH100` (kernel_gsp_gh100.c:986; g_gpu_nvoc.c:2513) | 0x14DC | R | must be 0 | latched 0 |
| C7 | kernel_gsp_gh100.c:991 | 0x110040 | R | must be 0 ("GSP-FMC reported an error") | 0 |
| C8 | kernel_gsp_gh100.c:1000 | 0x110080 NV_PFALCON_FALCON_OS (dev_falcon_v4.h:34) | W | `pGspRmBootUcodeDesc->appVersion` | falcon register |
| C9 | `kflcnIsRiscvActive_GA102` (kernel_gsp_gh100.c:1003; src/kernel/gpu/falcon/arch/ampere/kernel_falcon_ga102.c:47-56; HAL g_kernel_falcon_nvoc.c:397-404) | 0x111388 = riscvRegisterBase 0x111000 (`kgspConfigureFalcon_GA102`, g_kernel_gsp_nvoc.c:604-606; hdr:ampere/ga102/dev_riscv_pri.h:27) + NV_PRISCV_RISCV_CPUCTL 0x388 (:32) | R | bit 7 ACTIVE_STAT == 1 (:33-34) | 0x80 after the COT, 0x10 (HALTED) before |
| C10 | `GspStatusQueueInit` (message_queue_cpu.c:339), `kgspHealthCheck_TU102` (g_kernel_gsp_nvoc.c:1144) | 0x110094 NV_PFALCON_FALCON_DEBUGINFO (hdr:turing/tu102/dev_falcon_v4.h:45) | R | [15:0] != 0xDEAD (no crash report) | 0 |
| C11 | `kgspWaitForRmInitDone` (kernel_gsp.c:6264) | sysmem only | | GSP_INIT_DONE in the status queue | generic model (boot.rs `link_queues`) |

Registers the Turing path reads and GB206 does not: `NV_PGC6_AON_SECURE_SCRATCH_GROUP_05`
GFW_BOOT (0x118234; `gpuWaitForGfwBootComplete` is the `_5baef9` stub, g_gpu_nvoc.c:2649),
`NV_PFB_PRI_MMU_LOCAL_MEMORY_RANGE` (the FB size comes from A-B3), the PROM window, every
FWSEC and Booter falcon register, and the GSP mailboxes as the libos-args carrier.

## 3. The FSP transport

The GB206 HALs are the `_GB100` wrappers (g_kern_fsp_nvoc.c:345-358 and the entries after
them), which take the MNOC mailbox only with `PDB_PROP_KFSP_USE_MNOC_CPU`, set by the
registry key `RmFspUseMnoc` alone (kern_fsp.c:170-181); otherwise they call the `_GH100`
EMEM code (kern_fsp_gb100.c:311-420). Channel `FSP_EMEM_CHANNEL_RM` = 0, 1024 bytes
(arch/nvalloc/common/inc/fsp/fsp_emem_channels.h:34-37).

| Register | BAR0 | Header |
|---|---|---|
| NV_PFSP_EMEMC(0): OFFS 7:2 (dword in a 64-dword block), BLK 15:8, AINCW 24, AINCR 25 | 0x8F2AC0 | hdr:hopper/gh100/dev_fsp_pri.h:26-39 |
| NV_PFSP_EMEMD(0) | 0x8F2AC4 | :40-42 |
| NV_PFSP_QUEUE_HEAD(0) / TAIL(0) (CPU to FSP) | 0x8F2C00 / 0x8F2C04 | :53-60 |
| NV_PFSP_MSGQ_HEAD(0) / TAIL(0) (FSP to CPU) | 0x8F2C80 / 0x8F2C84 | :44-51 |

Send (`kfspSendPacket_GH100`, kern_fsp_gh100.c:621-665): poll until QUEUE_HEAD == QUEUE_TAIL
and MSGQ_HEAD == MSGQ_TAIL (`kfspCanSendPacket_GH100`, :249-275); EMEMC = AINCW at offset 0;
read EMEMC; write the packet dword by dword to EMEMD, the last one zero-padded; read EMEMC
and assert that it advanced by the dword count (`_kfspWriteToEmem_GH100`, :562-618); write
QUEUE_TAIL = padded size - 4, then QUEUE_HEAD = 0 (`_kfspUpdateQueueHeadTail_GH100`, :96-109;
"the write to HEAD ... will interrupt FSP"). FSP must then make HEAD == TAIL.

Receive (`kfspReadPacket_GH100`, :668-725): a response is there when MSGQ_HEAD != MSGQ_TAIL
(`kfspIsResponseAvailable_GH100`, :225-247); size = TAIL - HEAD + 4, at most 1024; EMEMC =
AINCR at offset 0; read size/4 dwords; assert EMEMC advanced by size/4; write MSGQ_TAIL =
MSGQ_HEAD = HEAD.

Framing (`kfspSendMessage`, kern_fsp.c:485-597; arch/nvalloc/common/inc/mctp_format.h):

| Word | Bits | CPU sends | Device answers |
|---|---|---|---|
| MCTP transport header | VERSION 3:0, DEID 15:8, SEID 23:16, TAG 26:24, SEQ 29:28, EOM 30, SOM 31 (:40-48) | 0xC0000001 for a single packet; continuation packets SOM 0 with only this word | 0xC0000001 |
| MCTP message header | TYPE 6:0 = 0x7E vendor PCI, VENDOR_ID 23:8 = 0x10DE, NVDM_TYPE 31:24 (:51-58) | 0x1410DE7E for COT | 0x1510DE7E (FSP_RESPONSE) |
| payload | NVDM message | COT 860 bytes | `NVDM_PAYLOAD_COMMAND_RESPONSE`: taskId, commandNvdmType, errorCode (arch/nvalloc/common/inc/fsp/nvdm_payload_cmd_response.h, pack(1)) |

The CPU takes the payload from byte 7 (`sizeof(MCTP_HEADER)` = 7 packed,
inc/kernel/gpu/fsp/kern_fsp_cot_payload.h:27-33), so byte 7, the NVDM type, must be 0x15
(`kfspProcessNvdmMessage_GH100`, kern_fsp_gh100.c:426-463), and only `errorCode` is checked:
FSP_OK 0 (inc/kernel/gpu/fsp/kern_fsp_retval.h:24) maps to NV_OK; 0x9F and invalid-knob
codes map to quiet errors; anything else dumps FSP state (`kfspErrorCode2NvStatusMap_GH100`,
kern_fsp_gh100.c:1584-1612; :493). NVDM types: COT 0x14, FSP_RESPONSE 0x15, CAPS_QUERY
0x16, CLOCK_BOOST 0x20 (arch/nvalloc/common/inc/nvdm_format.h:47-57).

The device answers every message with FSP_OK. For the CAPS_QUERY that means "clock boost
supported", so a 610 guest follows with CLOCK_BOOST enable (B5) and a disable at teardown;
answering 0x9F instead would skip both but print "FSP doesn't have clock boost
capability" at error level (kern_fsp_gb100.c:183-194). The acknowledging FSP is the quieter
one.

## 4. Sysmem structures

### 4.1 NVDM_PAYLOAD_COT (inc/kernel/gpu/fsp/kern_fsp_cot_payload.h:36-55, pack(1), 860 bytes, derived offsets)

| Offset | Field | GB206 value |
|---|---|---|
| 0 | version (u16) | `cotPayloadVersion` 2 (g_kern_fsp_nvoc.c:288-291) |
| 2 | size (u16) | 860 |
| 4 | gspFmcSysmemOffset (u64) | PA of the GSP-FMC image (`kfspSetupGspImages`, kern_fsp_gh100.c:953-1058) |
| 12 / 20 | frtsSysmemOffset (u64) / Size (u32) | 0: `PDB_PROP_KFSP_DISABLE_FRTS_SYSMEM` is set for PF_KERNEL_ONLY (g_kern_fsp_nvoc.c:254-259) |
| 24 / 32 | frtsVidmemOffset (u64, from the end of FB) / Size (u32) | derived 0x1BC0000 / 0x100000: FB-end reserve 0x220000 (`memmgrGetFBEndReserveSizeEstimate_GB100`, g_mem_mgr_nvoc.c:1422-1425) + PMU reserve 0x19A0000, aligned to 128 KiB (kern_fsp_gh100.c:1425-1446); FRTS 1 MiB (:1338) |
| 36 | hash384[12] | image hash |
| 84 | publicKey[96] | |
| 468 | signature[96] | |
| 852 | gspBootArgsSysmemOffset (u64) | PA of GSP_FMC_BOOT_PARAMS (`kfspGetGspBootArgs`, kern_fsp_gh100.c:919-940) |

### 4.2 GSP_FMC_BOOT_PARAMS (arch/nvalloc/common/inc/gsp/gspifpub.h:115-122; measured in gsp-abi-610.md)

`kgspSetupGspFmcArgs_GH100` (kernel_gsp_gh100.c:419-497) fills it; 88 bytes, 4 KiB aligned
sysmem (:172). Offsets are the measured ones (`src/ver/v610.rs` `FMC_*`, tested against
gsp-abi-610.md): `bootGspRmParams.gspRmDescOffset` +0x10 = PA of the GspFwWprMeta,
`gspRmDescSize` +0x0C = 256, `gspRmParams.bootArgsOffset` +0x30 = PA of the libos init
arguments. This is the only way the libos arguments reach GSP on this path: there is no
mailbox write (the mailbox writes at kernel_gsp_gh100.c:732-733 belong to the FSP-less
branch).

### 4.3 GspFwWprMeta: the CPU requests, ACR lays out

Same 256-byte struct as on Turing (gsp_fw_wpr_meta.h; `ct_assert` kernel_gsp_gh100.c:314). On
GH100 and later the CPU writes only sizes and addresses (kgspPopulateWprMeta_GH100,
:304-397): sizeOfRadix3Elf, sizeOfBootloader, nonWprHeapSize (0x300000), gspFwHeapSize (a
minimum), frtsSize (1 MiB), vgaWorkspaceSize (128 KiB), pmuReservedSize, magic, revision.
ACR fills gspFwRsvdStart, nonWprHeapOffset, gspFwWprStart, gspFwHeapOffset, gspFwOffset,
bootBinOffset, frtsOffset, gspFwWprEnd, fbSize, vgaWorkspaceOffset (the picture at
:283-301). `PDB_PROP_GPU_FW_WPR_OFFSET_SET_BY_ACR` is TRUE for GB206 (g_gpu_nvoc.c:364-375),
so after boot the kernel RM takes nonWprHeapOffset and frtsOffset from the static info's
`fwWprLayoutOffset` (`_kgspSetFwWprLayoutOffset`, kernel_gsp.c:4712-4728), not from the meta.

## 5. What the device model does (`src/chip/boot_fsp.rs`)

- Out of reset: FSP booted (A2), GSP halted and locked down (HWCFG2 = RISCV | LOCKDOWN,
  RISCV_CPUCTL = HALTED), WPR2 down, EMEM empty, queues empty.
- EMEM: 256 dwords with the OFFS/BLK auto-increment the CPU asserts on.
- A QUEUE_HEAD write consumes the packet (HEAD = TAIL), reassembles multi-packet messages
  (SOM/EOM), and answers every NVDM message with the 20-byte FSP_RESPONSE above.
- The COT: read GSP_FMC_BOOT_PARAMS, take the meta and the libos arguments from it, compute
  the WPR layout as ACR would (`AcrLayout::compute`: VGA workspace at the end, FRTS
  `frtsVidmemOffset` below the end, boot binary 4 KiB aligned, ELF 64 KiB aligned, heap and
  meta 1 MiB aligned, non-WPR heap under the WPR; the TU102 alignments of
  kgspPopulateWprMeta_TU102, kernel_gsp_tu102.c:755-860, derived: the real ACR is not
  documented), patch those fields into the guest's meta, report WPR2 up, release the target
  mask and the lockdown, mark the RISC-V core active, and raise `BootEvent::WprMeta`. The
  device plays GSP-RM from there, so it reads the meta the way GSP-RM would see it after
  ACR; the static info then reports `fwWprLayoutOffset` from it (rpc.rs).
- NV_PGSP_QUEUE_HEAD(0) raises `BootEvent::CmdQueueDoorbell` with the libos arguments of
  the FMC parameters; the generic model (boot.rs, unchanged) links the queues, answers the
  init RPCs and posts GSP_INIT_DONE.

Unit tests (`src/chip/boot_fsp/tests.rs`) drive this exactly as §2 orders it, including
the CPU's EMEMC checks, and end with the queues linked and GSP_INIT_DONE queued
(`gb206_bootstrap_reaches_the_message_queues`).

## 6. Open points

1. The ACR layout is derived. A GB206 guest takes only nonWprHeapOffset and frtsOffset from
   the static info; whether its FB accounting (mem_mgr.c:1352-1353 uses frtsOffset + frtsSize
   and the non-WPR heap) agrees with the fake FB is for the N gate to show.
2. The PMU reserve (0x19A0000) and so frtsVidmemOffset assume the KernelPmu object
   survives construction on GB206; the model uses whatever the COT says.
3. `appVersion` of the GB20x GSP-RM boot descriptor was not extracted; the model accepts
   any value.
4. Everything the kernel RM touches before kgspInitRm (the XAL/XTL windows, fuses beyond
   A1/B1, the interrupt tree, the display fuse on GB20x) is not traced here; the N gate's
   E0 step records it (blackwell-gate.md).
5. The COT and NVDM layouts are not in gsp-abi-610.md yet (the COT offsets are derived
   from the packed struct); a gsp-abi-sizes section would pin them.
