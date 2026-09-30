<!-- SPDX-License-Identifier: MIT -->
# Phase 5: every RPC from GET_GSP_STATIC_INFO to a working nvidia-smi

This file lists, in call order, what the guest kernel RM (610.57.04, open module,
GSP-client mode, TU106) sends to GSP after `GET_GSP_STATIC_INFO`. The list runs
through the log line RmInitAdapter succeeded!, then covers what the first user-space open and
`nvidia-smi -q` add. For each RPC it proposes where the synthetic device gets the
answer. Everything here was read from source; nothing has run yet. E3/E4
([experiments.md](caraxes-experiments-2026-09-29.md)) will confirm or correct it.

## 0. Conventions, method, baseline

**Paths.** Paths are relative to `vendor/open-gpu-kernel-modules/src/nvidia` unless
they start with `kernel-open/` or `sdk/`. `sdk/` stands for
`vendor/open-gpu-kernel-modules/src/common/sdk/nvidia/inc`. `g_*.c` files are in
`generated/`.

**How an RPC is recognized.** Kernel RM reaches GSP in four ways:
- **Physical RMAPI.** On a GSP client, `GPU_GET_PHYSICAL_RMAPI(pGpu)` is switched to
  `rpcRmApiControl_GSP`, `rpcRmApiAlloc_GSP`, `rpcRmApiFree_GSP` and
  `rpcRmApiDupObject_GSP` (src/kernel/rmapi/rpc_common.c:74-79). Every `->Control` on it
  becomes GSP_RM_CONTROL (fn 76, src/kernel/vgpu/rpc.c:10659). Every `->AllocWithHandle`
  becomes GSP_RM_ALLOC (fn 103, rpc.c:10945), and every `->Free` becomes FREE (fn 10, rpc.c:11120).
  - Most of these calls target the internal handles. Those come from
    `GspStaticConfigInfo.hInternalClient/Device/Subdevice` (src/kernel/gpu/gpu.c:984-991),
    so the device chooses them itself.
- **ROUTE_TO_PHYSICAL.** A control issued through the internal RMAPI on a kernel-owned
  object is forwarded with the **caller's own** hClient/hObject when its flags contain
  ROUTE_TO_PHYSICAL (src/kernel/rmapi/resource.c:254-293).
- **Mirrored allocations.** Allocations of classes with `RS_FLAGS_ALLOC_RPC_TO_PHYS_RM`
  are mirrored as GSP_RM_ALLOC (src/kernel/rmapi/alloc_free.c:878-938). Some
  constructors also send explicitly:
  - NV01_DEVICE_0 sends the ROOT and DEVICE allocs through `NV_RM_RPC_ALLOC_SHARE_DEVICE_FWCLIENT`, inc/kernel/vgpu/rpc.h:48-110.
  - NV20_SUBDEVICE_0 sends through `NV_RM_RPC_ALLOC_SUBDEVICE`, rpc.h:363-387.
  - Channels send through `NV_RM_RPC_ALLOC_CHANNEL`, rpc.h:241.
  - Events send through `NV_RM_RPC_ALLOC_EVENT`, rpc.h:333.
- **Dedicated RPCs.** Some RPCs have their own function number, for example
  `UPDATE_BAR_PDE` (70) and `INIT_GSP_TRACE_CRASH_BUFFER` (228)
  (inc/kernel/vgpu/rpc_global_enums.h).
- **Caching.** Controls flagged CACHEABLE reach GSP only on their first call
  (rpc.c:10699-10776).

**Engine order.** The order is derived from these sources, not guessed:
- `gpuStatePreInit` (src/kernel/gpu/gpu.c:2154), `gpuStateInit` (:2302), `gpuStatePreLoad`
  (:2416), `gpuStateLoad` (:2652) and `gpuStatePostLoad` (:3646) walk the engine
  lists that `gpuGetChildrenOrder_GM200` supplies (g_gpu_nvoc.c:1610).
- That table is `gpuChildOrderList_GM200` (src/kernel/gpu/arch/maxwell/kern_gpu_gm107.c:457-556).
- It is filtered by the TU106 child list (src/kernel/gpu/arch/turing/kern_gpu_tu102.c:177-205).
- It is further filtered by the modules compiled into the kernel build (generated/rmconfig.h).

The resulting engine lists:
- **INIT order:** KernelBif, NvDebugDump, KernelMc, SwIntr, Intr, OBJTMR, KernelMemorySystem,
  MemoryManager, KernelNvlink, KernelGmmu, KernelSec2, KernelGsp, KernelBus,
  VirtMemAllocator, KernelDisplay, KernelMIGManager, KernelGraphicsManager, KernelGraphics,
  KernelPmu, KernelPerf, KernelCE (up to 9), KernelFifo, OBJUVM, KernelHwpm, OBJSWENG,
  OBJGRIDDISPLAYLESS.
- **LOAD order:** the same, except that KernelPmu and KernelPerf move to just after
  KernelBus, and KernelDisplay moves to just after KernelGraphics.

Which body each engine hook runs was read from the NVOC bindings in the
`g_<engine>_nvoc.c` files. Two facts decide those bindings:
- TU106 is ChipHal index 39 (g_chips2halspec_nvoc.c:29-33).
- A GSP client is RmVariant PF_KERNEL_ONLY (g_chips2halspec_nvoc.c:212-216).

**Control flag lookup (ctrl_table.tsv method).** The routing flags quoted below come from a
table built mechanically from the generated sources:
1. From every `generated/g_*_nvoc.c`, take each exported-method entry's `/*flags=*/`,
   `/*methodId=*/` and `/*func=*/` fields, plus the file:line of the methodId (awk over the
   files; 1372 entries).
2. From every `sdk/ctrl/**` header, take each
   `#define NVxxxx_CTRL_CMD_<NAME> (0x...)` (U suffix stripped), with its file:line.
3. Join the two on the lowercase method id.

The flag bits used here are from inc/kernel/rmapi/control.h:167-344:

| Bit | Meaning |
|---|---|
| 0x4 | PRIVILEGED |
| 0x8 | NON_PRIVILEGED |
| 0x40 | ROUTE_TO_PHYSICAL |
| 0x80 | INTERNAL |
| 0x200 | ROUTE_TO_VGPU_HOST |
| 0x400 | CACHEABLE |
| 0x800 | COPYOUT_ON_ERROR |
| 0x10000 | GSP_PLUGIN_FOR_VGPU_GSP |
| 0x40000 | PHYSICAL_IMPLEMENTED_ON_VGPU_GUEST |

With no privilege bit set (neither 0x4 nor 0x8), the control is kernel-privileged.

The table is not checked in, because only files under docs/caraxes may be added. To
rebuild it, run the awk pass described above. Each flag value below cites its
`g_*_nvoc.c:line`.

**Answer categories.**
- (a) Forward as-is to the host RM as an unprivileged client: the same public control or class on a host object of the same class.
- (b) Forward with translation: a named public equivalent, or a handle map.
- (c) Synthesize from static data.
- (d) Fake with care: side effects on channels, interrupts, memory or BAR mappings.

**Baseline configuration.** The counts in this file assume the following. Each choice
comes from a reason in the text.
- Display absent. The device answers bit 0 = 1 in BAR0 `NV_FUSE_STATUS_OPT_DISPLAY`
  0x21C04, as E2 already plans. `kdispStatePreInitLocked` then returns NOT_SUPPORTED
  before any RPC (src/kernel/gpu/disp/kern_disp.c:444-445; gpu.c:2231-2270).
- Not mobile. `bIsMobile` and the RTD3/GC6/GCOFF flags in static info are false
  (src/kernel/gpu/gsp/kernel_gsp.c:6611-6622).
- Default registry. The watchdog, CeUtils and the scrubber are on; the effect of
  disabling them is described in 1.6.
- The upstream PCIe bridge is not an Intel part.
- `GspStaticConfigInfo.gidInfo` is non-zero.
- `INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO` is answered NOT_SUPPORTED.

## 1. RmInitAdapter after GET_GSP_STATIC_INFO: ordered inventory

Column **Fatal** says whether a non-OK status aborts `RmInitAdapter`; "tol." means
the status is tolerated. Column **Ans** gives the answer category (a)-(d) from 0.
Flags are quoted as `value (g_file:line)`.

### 1.1 End of kgspInitRm and gpuStatePreInit (before the engine loop)

| # | RPC | Issued at | Flags | Params | Reply use, fatal | Ans |
|---|---|---|---|---|---|---|
| 1 | dedicated `INIT_GSP_TRACE_CRASH_BUFFER` (228) | `kgspInitGspTraceCrashBuffer` src/kernel/gpu/gsp/kernel_gsp.c:3994, called at :5167. The regkey RmGspTraceCrashLogging defaults to on (interface/nvrm_registry.h:3326) | n/a | `rpc_init_gsp_trace_crash_buffer_v03_00 {pa,size}` g_rpc-structures.h:1894-1898. The buffer is 1600 B of guest sysmem | status only; fatal (kernel_gsp.c:5167) | c: NV_OK; optionally record pa |
| 2 | `NV2080_CTRL_CMD_INTERNAL_GPU_GET_CHIP_INFO` 0x20800a36, internal subdevice | `_gpuInitChipInfo` gpu.c:905-910, called at :2182 | 0x404c0 ROUTE+INTERNAL+CACHEABLE (g_subdevice_nvoc.c:6219) | sdk/ctrl/ctrl2080/ctrl2080internal.h:574-592 | chipSubRev, pciDeviceId, pciSubDeviceId and pciRevisionId are copied into pGpu (gpu.c:912-915). regBases[] feeds `gpuGetRegBaseOffset_FWCLIENT` (src/kernel/gpu/gpu_gspclient.c:318-335) for the TIMER and USERMODE bases. Fatal | c: ids as in config space, in the (id<<16)\|vendor layout; regBases[2]=0x9000, [4]=0xBB0000, all others 0xFFFFFFFF (from `gpuGetRegBaseOffset_TU102`, src/kernel/gpu/arch/turing/kern_gpu_tu102.c:275-306) |
| 3 | `NV2080_CTRL_CMD_INTERNAL_GPU_GET_USER_REGISTER_ACCESS_MAP` 0x20800a41 | src/kernel/gpu/gpu_register_access_map.c:244-247, called at gpu.c:2183 | 0x4c0 (g_subdevice_nvoc.c:6369) | ctrl2080internal.h:775-788 (about 20 KB) | Size 0 means no map. Otherwise the map is all-allow if compressedSize is 0 (:260-298). The size also enters the reserved-FB estimate (src/kernel/gpu/mem_mgr/arch/maxwell/mem_mgr_gm107.c:1999-2003). Fatal | c: size 0x80000, compressedSize 0, profilingRangesSize 0 |
| 4 | `NV2080_CTRL_CMD_GPU_GET_CONSTRUCTED_FALCON_INFO` 0x208001b0 | `gpuBuildGenericKernelFalconList` gpu.c:5730-5733, called at :2184 | 0x10048 NON_PRIV+ROUTE, not cacheable (g_subdevice_nvoc.c:5289) | sdk/ctrl/ctrl2080/ctrl2080gpu.h:4448-4470 | One GenericKernelFalcon per entry, SEC2 skipped (gpu.c:5741-5770). Fatal | a: forward to the host subdevice. Must agree with engineCaps and row 7 |
| 5 | same control again (not cacheable) | `gpuBuildKernelVideoEngineList` gpu.c:5825-5828, called at :2186 | same | same | One KernelVideoEngine per video engDesc (gpu.c:5832-5849). Fatal | a: same bytes as row 4 |
| 6 | `NV2080_CTRL_CMD_INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO` 0x20800a87 | `knvlinkIsPresent` src/kernel/gpu/nvlink/kernel_nvlinkstate.c:292, which goes to kernel_nvlink.c:1900 and :2761. Reached from `gpuRemoveMissingEngines` (gpu.c:2200, loop :1760-1800). KernelNvlink exists because nvidia.ko always initialises the nvlink core (kernel-open/nvidia/nv.c:525) | 0xc0 (g_subdevice_nvoc.c:7284) | ctrl2080internal.h:4976-4989 | Any error makes IsPresent false, and KernelNvlink is removed. Tolerated | c: **NV_ERR_NOT_SUPPORTED**. Its follow-ups (IOCTRL info, PMGR_GET_MODULE_INFO at gpu.c:233) then never happen |
| 7 | `NV2080_CTRL_CMD_FIFO_GET_DEVICE_INFO_TABLE` 0x20801112, repeated while bMore | `kfifoGetHostDeviceInfoTable_KERNEL` src/kernel/gpu/fifo/kernel_fifo.c:2051-2056, reached from `kfifoConstructEngineList` (:2187-2206). The first caller is `kgraphicsIsPresent` (src/kernel/gpu/gr/kernel_graphics.c:417-435) inside `gpuRemoveMissingEngines`. `gpuConstructDeviceInfoTable_HAL` is the no-op `_ac1694` on TU106 (g_gpu_nvoc.c:864-881), so no INTERNAL_GET_DEVICE_INFO_TABLE is sent | 0x5c040: ROUTE, no privilege bit, so kernel-privileged (g_subdevice_nvoc.c:9009) | sdk/ctrl/ctrl2080/ctrl2080fifo.h:535-576 | Copied into engineInfo, which sets numRunlists and max PBDMA (kernel_fifo.c:2062-2168). An error or empty table silently removes KernelGraphics, then the CEs (gpu.c:1776-1782; src/kernel/gpu/ce/kernel_ce.c:86-95). Not fatal, but it leaves no GR | c: a TU106 engine table (GR0, the COPYn from engineCaps, NVDEC0, NVENC0/1, SEC2, SW) with runlist, PBDMA, fault ids and reset/intr data. Not forwardable (kernel-privileged) |

### 1.2 Engine StatePreInit (INIT order)

Engines with no RPC in PreInit:
- KernelBif, NvDebugDump, KernelMc, SwIntr, OBJTMR, MemoryManager, KernelGmmu, KernelSec2, KernelGsp, VirtMemAllocator, KernelMIGManager, KernelGraphicsManager, KernelGraphics, KernelPmu, KernelPerf, KernelCE, KernelFifo, OBJUVM, KernelHwpm, OBJSWENG, OBJGRIDDISPLAYLESS.
- Their hooks either inherit `engstateStatePreInit*_IMPL`, which just returns OK (src/kernel/gpu/eng_state.c:226-235), or make no call.
- KernelDisplay drops out here in the baseline (see 1.7).

| # | RPC | Issued at | Flags | Params | Reply use, fatal | Ans |
|---|---|---|---|---|---|---|
| 8 | `NV2080_CTRL_CMD_INTERNAL_INTR_GET_KERNEL_TABLE` 0x20800a5c | `intrInitInterruptTable_KERNEL` src/kernel/gpu/intr/intr.c:1041-1046, from `intrStatePreInitLocked` (:819) | 0xc0 (g_subdevice_nvoc.c:6699) | ctrl2080internal.h:1463-1480 (`table[128]`, `subtreeMap[7]`) | Entries are copied (intr.c:1057-1071). Error or tableLen > 128 is fatal. `intrStateLoad_TU102` (src/kernel/gpu/intr/arch/turing/intr_tu102.c:80-147, 174-275) later requires: entries for ACCESS_CNTR (60) and REPLAYABLE_FAULT (59) in the same 32-bit leaf; subtreeMap[UVM_OWNED] exactly the subtree bit of that leaf; UVM_SHARED with exactly one bit; ESCHED_DRIVEN_ENGINE non-zero | c: a TU102-style map with ESCHED 0x8, ESCHED_NOTIFICATION 0x1, UVM_OWNED 0x2, UVM_SHARED 0x4 (intr_tu102.c:1386-1417), plus entries for GSP (50) stall, FIFO/CE/GR non-stall, ACCESS_CNTR and REPLAYABLE_FAULT. The device must raise MSI for exactly these vectors (1.8). Optional (b): host `MC_GET_STATIC_INTR_TABLE` 0x2080170e, `MC_GET_ENGINE_NOTIFICATION_INTR_VECTORS` 0x2080170d, `MC_GET_INTR_CATEGORY_SUBTREE_MAP` 0x2080170f |
| 9 | `NV2080_CTRL_CMD_INTERNAL_MEMSYS_GET_STATIC_CONFIG` 0x20800a1c | `kmemsysInitStaticConfig_KERNEL` src/kernel/gpu/mem_sys/kern_mem_sys.c:495-497, from PreInit :128-131 | 0x5c0c0 (g_subdevice_nvoc.c:5964) | ctrl2080internal.h:94-135 | l2CacheSize, bFbpaPresent (PRAMIN window placement, src/kernel/gpu/bus/arch/maxwell/kern_bus_gm107.c:233-246), comptag policy, bDisableCompbitBacking (src/kernel/gpu/mem_mgr/mem_mgr.c:379), ramType. Fatal | b: L2/RAM type/LTC/LTS from host `FB_GET_INFO_V2` 0x20801303 (indices 0x1B, 0x0D, 0x22, 0x23; sdk/ctrl/ctrl2080/ctrl2080fb.h:319-375). c fallback: 4 MiB L2, 8 LTC x 2 LTS, GDDR6 (0x11), bDisableCompbitBacking TRUE, other booleans FALSE |
| 10 | `NV2080_CTRL_CMD_BUS_GET_PCI_BAR_INFO` 0x20801803 | `kbusInitBarsSize_KERNEL` src/kernel/gpu/bus/kern_bus.c:601-609, from `kbusStatePreInitLocked_GM107` kern_bus_gm107.c:154-171 | 0x10518 NON_PRIV+CACHEABLE, no ROUTE, sent explicitly (g_subdevice_nvoc.c:10584) | sdk/ctrl/ctrl2080/ctrl2080bus.h:612-638 | pciBarSizes[i] = barSizeBytes. BAR1 size bounds the BAR1 aperture (kern_bus_gm107.c:176). Fatal | c: count 4 with the **emulated** BAR sizes: 16 MiB, BAR1, 32 MiB, 0x80. (a) would return the host card's BARs |

### 1.3 Engine StateInit (INIT order)

Engines with no RPC in Init:
- NvDebugDump, KernelMc, SwIntr, Intr, OBJTMR, KernelMemorySystem, KernelSec2, VirtMemAllocator, KernelMIGManager, KernelGraphicsManager, KernelGraphics, KernelPmu, KernelCE, KernelHwpm, OBJSWENG, OBJGRIDDISPLAYLESS.
- KernelGsp's `kgspStateInitLocked` only signs up for NVD (kernel_gsp.c:4402-4422).
- KernelMIGManager returns early because TU106 is not MIG-capable (src/kernel/gpu/mig_mgr/kernel_mig_manager.c:1191; g_gpu_nvoc.c:271-281). Its GET_SMC_MODE and `INTERNAL_STATIC_KMIGMGR_*` controls are therefore never sent.

If `RmInitNvDevice` runs an EGPU bridge check between PreInit and Init, it sends
nothing in the baseline (see 1.7).

| # | RPC | Issued at | Flags | Params | Reply use, fatal | Ans |
|---|---|---|---|---|---|---|
| 11 | `NV2080_CTRL_CMD_INTERNAL_BIF_GET_STATIC_INFO` 0x20800aac | `kbifStaticInfoInit` src/kernel/gpu/bif/kernel_bif.c:790-803, from :140 | 0x400c0 (g_subdevice_nvoc.c:7719) | ctrl2080internal.h:2188-2197 | Sets the PCIE_GEN4, C2C, MULTIFUNCTION and GCX restore properties. The return is ignored at :140, so tolerated | c: all FALSE. Set MULTIFUNCTION only if the device exposes function 1 |
| 12 | `NV2080_CTRL_CMD_INTERNAL_INIT_USER_SHARED_DATA` 0x20800afe | src/kernel/gpu/gpu_user_shared_data.c:313-316, from `gpuCreateRusdMemory` (:432), which mem_mgr.c:744 calls | 0xc0 (g_subdevice_nvoc.c:8724) | ctrl2080internal.h:4011-4015 {bInit, physAddr of a guest sysmem page} | tol. (the return is ignored at :432) | c: NV_OK and record physAddr. GSP would later fill the RUSD page; leaving it zero is fine |
| 13 | `NV2080_CTRL_CMD_INTERNAL_USER_SHARED_DATA_SET_DATA_POLL` 0x20800aff | gpu_user_shared_data.c:378-381 (from :434) | 0xc0 (g_subdevice_nvoc.c:8739) | ctrl2080internal.h:4030-4034 | tol. | c: NV_OK |
| 14-16 | GSP_RM_ALLOC NV01_ROOT, NV01_DEVICE_0 (parent client), NV20_SUBDEVICE_0 (parent device): MemoryManager's internal client | `_memmgrAllocInternalClientObjects` mem_mgr.c:1251 goes to `rmapiutilAllocClientAndDeviceHandles` (src/kernel/rmapi/rmapi_utils.c:62-106). From there, `deviceConstruct` (src/kernel/gpu/device.c:180-191) uses rpc.h:85-87 and :103-105, and `subdeviceConstruct` (src/kernel/gpu/subdevice/subdevice.c:122-131) uses rpc.h:378 | n/a | NV0000_ALLOC_PARAMETERS {hClient, processID=KERNEL_PID}; NV0080 {hClientShare=hClient, rest 0; deviceId is not copied}; NV2080 {subDeviceId 0} (sdk/class/cl0000.h:47-52, cl0080.h:53-64, cl2080.h:41-45) | status only; fatal (mem_mgr.c:755-773) | c: NV_OK and record the handle tree. This "client triple" recurs many times below; the device needs one generic handler for it |
| 17 | GSP_RM_ALLOC NV01_EVENT_KERNEL_CALLBACK_EX (0x7E) on that subdevice | `memmgrRegisterSuspendCallbacks` mem_mgr.c:597-603 goes to `eventapiConstruct`, src/kernel/rmapi/event_api.c:141-171, which uses rpc.h:341-352 | n/a | NV0005_ALLOC_PARAMETERS notifyIndex 0x040000C2 (POWER_RESUME) | fatal | c: NV_OK. Signal it only once suspend/resume is supported |
| 18 | `NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION` 0x20800301 on that subdevice (not the internal one) | src/kernel/gpu/subdevice/subdevice_ctrl_event_kernel.c:109-118, from mem_mgr.c:606-613 | 0x10118 public, no ROUTE; sent by an explicit FW-client branch (g_subdevice_nvoc.c:5544) | sdk/ctrl/ctrl2080/ctrl2080event.h:79-83 {event 194, REPEAT} | fatal | c: NV_OK |
| 19 | `NV2080_CTRL_CMD_INTERNAL_GMMU_GET_STATIC_INFO` 0x20800a59 | `_kgmmuInitStaticInfo` src/kernel/gpu/mmu/kern_gmmu.c:154-156, from :182 | 0x400c0 (g_subdevice_nvoc.c:6669) | ctrl2080internal.h:1417-1426 | Stored as pStaticInfo; fatal. The fault buffer sizes are read only when UVM allocates fault buffers (kern_gmmu.c:1222, 1920) | c: replayable = nonReplayable = N x 32 with N non-zero; metadata sizes 0 |
| 20 | dedicated `UPDATE_BAR_PDE` (70), barType BAR_2 | `kbusPatchBar2Pdb_GSPCLIENT` src/kernel/gpu/bus/kern_bus.c:848-902 (RPC at :899), from kern_bus_gm107.c:1925 in `kbusSetupBar2GpuVaSpace_GM107` | n/a | `rpc_update_bar_pde_v15_00` g_rpc-structures.h:384-387: entryValue = the kernel's own PDE3[0] (read through PRAMIN, kern_bus.c:891-892), entryLevelShift 47 | The status is ignored (kern_bus.c:899-901), but the call is synchronous | **d**: keep entryValue as PDE3[0] of the BAR2 root at `bar2PdeBase`, and translate CPU accesses to PCI BAR3 by walking the kernel's ver2 tables in emulated FB (formats: published/pascal/gp100/dev_mmu.h:98-140). BAR2 is used right after this call |
| 21-23 | `NV2080_CTRL_CMD_INTERNAL_MEMSYS_L2_INVALIDATE_EVICT` 0x20800a6c, three times | `kmemsysSendL2InvalidateEvict` kern_mem_sys.c:1133-1135, from `kbusVerifyBar2_GM107` kern_bus_gm107.c:4126, 4191, 4240 (itself reached from :362) | 0xc0 (g_subdevice_nvoc.c:6894) | ctrl2080internal.h:2148-2154 {flags 0x31} | status; fatal | c: NV_OK. The surrounding test needs PRAMIN (0x1700/0x700000) and BAR2 to alias the same FB bytes |
| 24 | `NV2080_CTRL_CMD_INTERNAL_PERF_GPU_BOOST_SYNC_GET_INFO` 0x20800a80 | `kperfGpuBoostSyncStateInit` src/kernel/gpu/perf/kern_perf_gpuboostsync.c:53-57, from kern_perf.c:61 | 0xc0 (g_subdevice_nvoc.c:7179) | ctrl2080internal.h:1782-1788 | tol. (kern_perf_gpuboostsync.c:59-78) | c: zeros, NV_OK |
| 25 | `NV2080_CTRL_CMD_INTERNAL_FIFO_GET_NUM_CHANNELS` 0x20800a61, runlist 0 | `kfifoRunlistQueryNumChannels_KERNEL` kernel_fifo.c:1319-1326, from `kfifoChidMgrConstruct` (:193), which `kfifoStateInitLocked` calls (src/kernel/gpu/fifo/kernel_fifo_init.c:226) | 0x1d8 (g_subdevice_nvoc.c:6744) | ctrl2080internal.h:1579-1586 | Sizes the ChID heaps and the preallocated USERD (src/kernel/gpu/fifo/arch/turing/kernel_fifo_tu102.c:554-558). 0 is fatal (kernel_fifo.c:303-308) | c: 4096, or less to save FB/BAR1 (for example 512). (b) via host `FIFO_GET_INFO` 0x20801109 |
| 26-28 | client triple (ROOT, DEVICE, SUBDEVICE) for OBJUVM | `uvmStateInitUnlocked` src/kernel/gpu/uvm/uvm.c:129, which goes through rmapi_utils.c:62-106 | n/a | as rows 14-16 | fatal (uvm.c:143) | c |

### 1.4 gpuStateLoad: PreLoad, Load and the calls around them

These steps send no RPC:
- `gpuInitSriov_FWCLIENT` (gpu_gspclient.c:56-97)
- `gpuCreateDefaultClientShare` (a no-op, g_gpu_nvoc.c:1984-1993)
- every StatePreLoad
- `_gpuDetectNvswitchSupport`, which needs KernelNvlink (gpu.c:227-228)

In the Load loop, the only senders are KernelPerf and the first KernelCE:
- KernelGraphics' Load registers the golden-channel callback (kernel_graphics.c:370-378) but sends nothing.
- KernelBus Load sends nothing (kern_bus_gm107.c:526-560).

| # | RPC | Issued at | Flags | Params | Reply use, fatal | Ans |
|---|---|---|---|---|---|---|
| 29 | `INTERNAL_PERF_GPU_BOOST_SYNC_GET_INFO` again (not cacheable) | kern_perf.c:85 | as row 24 | | tol. | c |
| 30 | `NV2080_CTRL_CMD_INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_TYPE` 0x20802a0f (DECOMP) | `kceGetPceConfigForLceType` kernel_ce.c:1088, from `kceStateLoad_GP100` src/kernel/gpu/ce/arch/pascal/kernel_ce_gp100.c:64. Only the shim-owner KCE (the lowest present CE) sends it | 0x100c0 (g_subdevice_nvoc.c:11874) | sdk/ctrl/ctrl2080/ctrl2080ce.h:465-478 | supportedPceMask becomes decompPceMask (kernel_ce_gp100.c:74). Fatal | c: all zero (Turing has no decompression LCE) |
| 31 | `NV2080_CTRL_CMD_CE_UPDATE_CLASS_DB` 0x20802a06 | `kceUpdateClassDB_KERNEL` kernel_ce.c:638, from :735 | 0xc0 (g_subdevice_nvoc.c:11739) | ctrl2080ce.h:267-273 | stubbedCeMask adds or removes CE classes per KCE (kernel_ce.c:647-668). Fatal | c: stubbedCeMask 0, and advertise in engineCaps only the CEs that should exist |
| 32 | `NV2080_CTRL_CMD_CE_UPDATE_PCE_LCE_MAPPINGS_V2` 0x20802a0d | kernel_ce.c:814. bUpdateNvlinkPceLce is FALSE because KernelNvlink was removed (src/kernel/gpu/ce/arch/turing/kernel_ce_tu102.c:97-101) | 0x4 PRIVILEGED, sent on the physical RMAPI (g_subdevice_nvoc.c:11844) | ctrl2080ce.h:388-398 | status; fatal (kernel_ce.c:821-826) | c: NV_OK. On real GSP this reprograms the PCE2LCE registers; the fake only records the map. Not forwardable (privileged) |
| 33 | `CE_UPDATE_CLASS_DB` again | kernel_ce.c:638 via :834 | as row 31 | | fatal | c: same mask |
| 34 | `NV2080_CTRL_CMD_GPU_GET_VMMU_SEGMENT_SIZE` 0x2080017e | `gpuInitVmmuInfo` gpu.c:938-950, called at :2847, after `gpuInitVideoLogging`. Video logging sends no RPC, but it maps 32 KiB of FB per video engine through BAR2 (src/kernel/gpu/video/kernel_video_engine.c:127-142) | 0x10448 NON_PRIV+ROUTE+CACHEABLE (g_subdevice_nvoc.c:4854) | ctrl2080gpu.h:3135-3141 | NV_ERR_NOT_SUPPORTED leaves the size at 0. Any other error is fatal | a: forward (Leandro's launcher already queries it, [identity-profile.md](../caraxes/identity-profile.md)), or c: NOT_SUPPORTED |

### 1.5 gpuStatePostLoad (LOAD order), then the end of gpuStateLoad

| # | RPC | Issued at | Flags | Params | Reply use, fatal | Ans |
|---|---|---|---|---|---|---|
| 35 | `NV2080_CTRL_CMD_INTERNAL_GMMU_COPY_RESERVED_SPLIT_GVASPACE_PDES_TO_SERVER` 0x20800a9f | The global VAS build: `kgmmuStatePostLoad` (kern_gmmu.c:322) goes through `gpugrpCreateGlobalVASpace` (src/kernel/gpu_mgr/gpu_group.c:219) to src/kernel/mem_mgr/gpu_vaspace.c:3934-3938. Split VAS is on by default for GSP clients (src/kernel/gpu/gpu_registry.c:170-181) | 0xc0 (g_subdevice_nvoc.c:7629) | ctrl2080internal.h:1905-1911, which wraps the NV90F1 params (sdk/ctrl/ctrl90f1.h:272ff): FB addresses of the kernel's PDEs for [4 GiB, 4.5 GiB) | status; fatal (kern_gmmu.c:324-330) | c: NV_OK. The fake never maps into the server range |
| (none) | KernelBus PostLoad sends no RPC but writes the BAR1 root | `kbusPatchBar1Pdb_GSPCLIENT` kern_bus.c:766-826 copies the kernel's BAR1 root entries into FB at `GspStaticConfigInfo.bar1PdeBase` (:794, :805-812) | | | | d (not an RPC): BAR1 translation rooted at bar1PdeBase; big/small dual PDE0; sparse PTEs read 0 |
| 36-38 | client triple for KernelGraphics' temporary static-info client | `_kgraphicsInternalClientAlloc` kernel_graphics.c:613, from `kgraphicsLoadStaticInfo_KERNEL` :1167, which `kgraphicsStatePostLoad` calls (:446) | n/a | as rows 14-16 | fatal (:1173-1174) | c |
| 39 | `NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_GET_CAPS` 0x20800a1f on the temp subdevice (ROUTE_TO_PHYSICAL path, resource.c:262-292) | kernel_graphics.c:1217 | 0x100c0 (g_subdevice_nvoc.c:6009) | ctrl2080internal.h:204 | engineCaps[0] becomes grCaps. Fatal | b: host `GR_GET_CAPS_V2` 0x20801227, capsTbl byte for byte |
| 40 | `...STATIC_KGR_GET_INFO` 0x20800a2a | :1230 | 0x1c0c0 (g_subdevice_nvoc.c:6129) | ctrl2080internal.h:421 | infoList indexed by info index; tol. (a NULL pGrInfo means compute-only golden) | b: host `GR_GET_INFO_V2` 0x20801228 with index = i for every i (lossless, kernel_graphics.c:2850-2854) |
| 41 | `...STATIC_KGR_GET_FLOORSWEEPING_MASKS` 0x20800a26 | :1258 | 0x1c0c0 (g_subdevice_nvoc.c:6084) | ctrl2080internal.h:292-330 | Fatal. gpcMask 0 skips the golden channel (kernel_graphics.c:487-489) and leaves GR unusable | b: assembled from host `GR_GET_GPC_MASK` 0x2080122a, `GR_GET_TPC_MASK` 0x2080122b, `GR_GET_NUM_TPCS_FOR_GPC` 0x20801234, `GR_GET_ZCULL_MASK` 0x20801237, `GR_GET_GFX_GPC_AND_TPC_INFO` 0x20801239 and `GPU_GET_PES_INFO` 0x20800168. physGpcMask = gpcMask, because `GR_GET_PHYS_GPC_MASK` is PRIVILEGED |
| 42 | `...STATIC_KGR_GET_GLOBAL_SM_ORDER` 0x20800a22 | :1290 | 0x1c0c0 (g_subdevice_nvoc.c:6039) | ctrl2080internal.h:218-242 | fatal | b: host `GR_GET_GLOBAL_SM_ORDER` 0x2080121b, field by field |
| 43 | `...GET_PPC_MASKS` 0x20800a30 | :1303 | 0x1c0c0 (:6174) | ctrl2080internal.h:498 | tol. (NOT_SUPPORTED explicitly, :1320-1327) | b: host `GR_GET_PPC_MASK` 0x20801233 per GPC |
| 44 | `...GET_ZCULL_INFO` 0x20800a2c | :1332 | 0x1c0c0 (:6144) | ctrl2080internal.h:437-452 | tol. | b: host `GR_GET_ZCULL_INFO` 0x20801206 (same layout) |
| 45 | `...GET_ROP_INFO` 0x20800a2e | :1362 | 0x1c0c0 (:6159) | ctrl2080internal.h:463-476 | tol. | b: host `GR_GET_ROP_INFO` 0x20801213 |
| 46 | `...GET_SM_ISSUE_RATE_MODIFIER` 0x20800a34 | :1392 | 0x1c0c0 (:6204) | ctrl2080internal.h:541-558 | tol. | b: host 0x20801230, or NOT_SUPPORTED |
| 47 | `...GET_SM_ISSUE_RATE_MODIFIER_V2` 0x20800b03 | :1418 | 0x1c0c0 (:8769) | ctrl2080internal.h:5396-5408 | tol. | b: host 0x2080123c, or NOT_SUPPORTED |
| 48 | `...GET_SM_ISSUE_THROTTLE_CTRL` 0x20800b05 | :1444 | 0x1c0c0 (:8784) | ctrl2080internal.h:5424-5436 | tol. | b: host 0x2080123d, or NOT_SUPPORTED |
| 49 | `...GET_FECS_RECORD_SIZE` 0x20800a3d | :1471 | 0x1c0c0 (:6309) | ctrl2080internal.h:694 | fatal; the value is unused during init | c: at least 120 (sizeof FECS_EVENT_RECORD, src/kernel/gpu/gr/fecs_event_list.c:58-69) |
| 50 | `...GET_FECS_TRACE_DEFINES` 0x20800a3f | :1483 | 0x1c0c0 (:6339) | ctrl2080internal.h:720-735 | tol. A zero fecsRecordSize would divide by zero later (fecs_event_list.c:1038) | c: NV_ERR_NOT_SUPPORTED |
| 51 | `...GET_PDB_PROPERTIES` 0x20800a48 | :1510 | 0x1c0c0 (:6444) | ctrl2080internal.h:862-865 | fatal; sets per-subcontext ctx header support | c: bPerSubCtxheaderSupported TRUE (the TU10x default, g_kernel_graphics_nvoc.c:306-314) |
| 52 | `...GET_CONTEXT_BUFFERS_INFO` 0x20800a32 | `kgraphicsInitializeDeferredStaticData` kernel_graphics.c:748, from :1530 | 0x1c1c0 (:6189) | ctrl2080internal.h:516-528 | fatal. Sizes the FECS, global and golden buffers. NV_U32_MAX means "not supported" | b: host `GR_GET_ENGINE_CONTEXT_PROPERTIES` 0x2080122d per engineId 0..0x19; NOT_SUPPORTED maps to NV_U32_MAX (lossless, kernel_graphics.c:4097-4111) |
| 53-55 | FREE subdevice, device, client (temp client) | kernel_graphics.c:1577, which reaches subdevice.c:250 and device.c:280, 322 (rpc.h:291-309) | n/a | NVOS00 v03_00 | not propagated | c: NV_OK |
| 56-78 | KernelFifo PostLoad: post-scheduling callbacks | `kfifoStatePostLoad_GM107` src/kernel/gpu/fifo/arch/maxwell/kernel_fifo_gm107.c:226-231 goes to `kfifoTriggerPostSchedulingEnableCallback` (kernel_fifo.c:3056-3120), which runs callbacks newest first (prepend, :2972) until all succeed. It also preallocates numChannels x USERD in FB and maps it through BAR1 (kernel_fifo_tu102.c:489-711) | | | Any callback error is fatal | see 1.6 |
| 79 | `NV2080_CTRL_CMD_GPU_GET_OEM_BOARD_INFO` 0x2080013f | `gpuStatePostLoad` gpu.c:3755-3760, after the engine PostLoad loop | 0x448 (g_subdevice_nvoc.c:4329) | ctrl2080gpu.h:1546-1568 | tol. (:3761-3765) | a: forward, or c: zeros |
| 80 | `NV2080_CTRL_CMD_INTERNAL_GET_PCIE_P2P_CAPS` 0x20800ab8 | `_gpuInitPcieP2PCapability` gpu.c:2511-2520, the last call of `gpuStateLoad` (:2942) | 0xc0 (g_subdevice_nvoc.c:7869) | ctrl2080internal.h:2572-2580 | tol. (assert only) | c: both statuses NV0000_P2P_CAPS_STATUS_NOT_SUPPORTED (0x05, sdk/ctrl/ctrl0000/ctrl0000system.h:1097) |

Rows 56-78 are the golden channel and the WAR (1.6). The numbering assumes the
scrubber and global CeUtils RPCs, whose count is open, come before them (see 1.6).

### 1.6 Post-scheduling callbacks run from KernelFifo PostLoad

**How the callbacks are ordered.** The memory manager's callback is registered in
StateInit (mem_mgr.c:644-650). The GR callback is registered in KernelGraphics Load
(kernel_graphics.c:374). The MIG callback is never registered on TU106
(kernel_mig_manager.c:1191, 1206-1209). Because callbacks are prepended, GR runs first.
It returns NV_WARN_MORE_PROCESSING_REQUIRED until the PMA scrubber is valid
(kernel_graphics.c:491-507), so the scrubber and CeUtils RPCs go out before the golden
channel.

**Memory manager: scrubber CeUtils, then the global CeUtils.** This comes from
`memmgrInitInternalChannels`, mem_mgr.c:462-512.
- Each CeUtils instance goes through `ceutilsConstruct` (src/kernel/gpu/mem_mgr/ce_utils.c:164-313)
  and sends the following. The exact list and order were not established statically (Open 1):
  - a client triple
  - FERMI_VASPACE_A, then NV90F1 COPY_SERVER_RESERVED_PDES, then FREE
  - `CE_GET_FAULT_METHOD_BUFFER_SIZE`
  - a TURING_CHANNEL_GPFIFO_A alloc
  - a TURING_DMA_COPY_A alloc
  - `GPFIFO_SCHEDULE`
- `memmgrTestCeUtils` (mem_mgr.c:389-453, called at :4165) then runs a real CE memset
  and copy between vidmem and sysmem and compares the data. A mismatch is fatal.
- Module-parameter workaround for milestone 1: NVreg_RegistryDwords with
  `RmDisableGlobalCeUtils=1` (interface/nvrm_registry.h:975, read at mem_mgr.c:331-335) and
  `RMDisableScrubOnFree=1` (nvrm_registry.h:926, mem_mgr.c:204-208). With both set, no
  CeUtils RPCs are sent.

**GR: golden-context channel, 14 RPCs.** This comes from `kgraphicsCreateGoldenImageChannel`,
kernel_graphics.c:2138. It is skipped if the gpcMask in row 41 is 0.

| # | RPC | Issued at | Reply use | Ans |
|---|---|---|---|---|
| 56-58 | client triple | kernel_graphics.c:2176 | fatal | c |
| 59 | GSP_RM_ALLOC FERMI_VASPACE_A 0x90F1, hObject 0xbaba0042, parent device | src/kernel/gpu/mem_mgr/vaspace_api.c:263 (explicit, gated on IS_GSP_CLIENT at :227) | The reply is copied back. index must stay 0 (:279). bigPageSize must be 0, 64K or 128K (:320-324). Fatal | d: echo the params, record the VAS |
| 60 | `NV90F1_CTRL_CMD_VASPACE_COPY_SERVER_RESERVED_PDES` 0x90f10106 on that VAS | gpu_vaspace.c:4963 (from :367, :3914) | 0x14004 PRIVILEGED, sent explicitly (g_vaspace_api_nvoc.c:236); params sdk/ctrl/ctrl90f1.h:272-340; status only; fatal | d: record levels[0] as the VAS root |
| 61 | GSP_RM_ALLOC TURING_CHANNEL_GPFIFO_A 0xC46F, hObject 0xbaba0045, parent device | `_kchannelSendChannelAllocRpc` src/kernel/gpu/fifo/kernel_channel.c:2761 (params built at :2604-2750) | NV_CHANNEL_ALLOC_PARAMS (sdk/alloc/alloc_channel.h:296) carries the kernel-chosen ChID (USERD_INDEX_PAGE_FIXED) and the PAs of the instance block, RAMFC, USERD and method buffer. No reply field is read. Fatal | d: record ChID with its PAs and VAS. The channel never runs |
| 62 | `NV2080_CTRL_CMD_GPU_PROMOTE_CTX` 0x2080012b on the golden subdevice | `kgrobjPromoteContext` src/kernel/gpu/gr/kernel_graphics_object.c:135 (ROUTE_TO_PHYSICAL) | 0x10244 PRIVILEGED+ROUTE (g_subdevice_nvoc.c:4149); ctrl2080gpu.h:893-971. bInitialize is re-read (:146-156). Fatal | d: echo the params |
| 63 | GSP_RM_ALLOC TURING_A 0xC597 (TURING_COMPUTE_A 0xC5C0 if GFX is unsupported), parent the channel | alloc_free.c:925 (RS_FLAGS_CHANNEL_DESCENDANT_COMMON, resource_list.h:2282-2301) | fatal | d: record |
| 64-69 | FREE TURING_A, channel, VAS, subdevice, device, client | kernel_graphics.c:2552 fans out to alloc_free.c:994, kernel_channel.c:1257, vaspace_api.c:572, subdevice.c:250, device.c:280 and :322 | Always return NV_OK. An error at the device free propagates (device.c:282-286) | c |

**GR: Bug 4208224 WAR, 9 RPCs.** `kgraphicsInitializeBug4208224WAR_TU102`
(src/kernel/gpu/gr/arch/turing/kgraphics_tu102.c:236) is enabled on TU102/104/106
(g_kernel_graphics_nvoc.c:366-371).

| # | RPC | Issued at | Reply use | Ans |
|---|---|---|---|---|
| 70-77 | the same eight as rows 56-63 with the 0xdada004x handles: client triple, FERMI_VASPACE_A, COPY_SERVER_RESERVED_PDES, TURING_CHANNEL_GPFIFO_A, PROMOTE_CTX, TURING_A | kgraphics_tu102.c:335, 396, 494, 512 | fatal. The client is kept, not freed (:532) | as rows 56-63 |
| 78 | `NV0080_CTRL_CMD_INTERNAL_KGR_INIT_BUG4208224_WAR` 0x802046 on the WAR device | kgraphics_tu102.c:270-275 (ROUTE_TO_PHYSICAL) | 0x1d0 ROUTE+INTERNAL (g_device_nvoc.c:1385); sdk/ctrl/ctrl0080/ctrl0080internal.h:137-145 {bTeardown FALSE}; status only; fatal | c: NV_OK |

### 1.7 After RmInitNvDevice (osinit.c:2182-2355)

**`osVerifySystemEnvironment`** (arch/nvalloc/unix/src/osinit.c:2212, src/kernel/os/os_sanity.c:146-316) sends **no RPC**. It needs a real interrupt:
- The CPU writes 129 to BAR0 0xB81640 (`intrSetStallSWIntr_TU102`, src/kernel/gpu/intr/arch/turing/intr_swintr_tu102.c:40-50).
- The device must set leaf 4 bit 1 and deliver MSI while the LEAF_EN/TOP_EN bits are set.
- If no interrupt arrives, init fails with NV_ERR_IRQ_NOT_FIRING (osinit.c:2218-2229).

The rows below are in call order:
- **Watchdog, rows 81-91.** `krcWatchdogInit_HAL` (osinit.c:2246 → src/kernel/gpu/rc/kernel_rc_watchdog.c:467). NV_ERR_NOT_SUPPORTED is tolerated; any other error is fatal (osinit.c:2253-2265).
- **Kernel RM API client, rows 92-95.** `RmUnixAllocRmApi` (osinit.c:2274, body :1775-1880).
- **GPU info, rows 96-97.** `RmInitGpuInfoWithRmApi` (osinit.c:2282, body :625-740).
- **I2C ports, row 98.** `RmI2cAddGpuPorts` (osinit.c:2291, arch/nvalloc/unix/src/osapi.c:4266-4389).

| # | RPC | Issued at | Flags / params | Reply use, fatal | Ans |
|---|---|---|---|---|---|
| 81-83 | client triple (watchdog) | kernel_rc_watchdog.c:578, 602 | as rows 14-16 | fatal | c |
| 84-86 | GSP_RM_ALLOC FERMI_VASPACE_A (index GPU_DEVICE), then COPY_SERVER_RESERVED_PDES, then FREE. This happens when the first Map creates the device's default VAS | kernel_rc_watchdog.c:878, which reaches src/kernel/gpu/device_share.c:341 and then gpu_vaspace.c:3892, 3914, 3921 | 0x14004 (g_vaspace_api_nvoc.c:236) | fatal | d (VAS + record the root), then c for the FREE |
| 87 | `NV2080_CTRL_CMD_CE_GET_FAULT_METHOD_BUFFER_SIZE` 0x20802a08 | kernel_ce.c:857, reached via the implicit TSG: src/kernel/gpu/fifo/arch/volta/kernel_channel_group_gv100.c:77-78 | 0x1c040, kernel-privileged (g_subdevice_nvoc.c:11769); ctrl2080ce.h:293-295 | size > 0 is asserted; fatal | c: 0x1000 (only the fake uses this buffer) |
| 88 | GSP_RM_ALLOC TURING_CHANNEL_GPFIFO_A, parent watchdog device, GR0, 4 GPFIFO entries | kernel_channel.c:2761, from kernel_rc_watchdog.c:1056 | NV_CHANNEL_ALLOC_PARAMS as row 61, with the error notifier in sysmem | fatal | d |
| 89 | `GPU_PROMOTE_CTX` on the watchdog subdevice | kernel_graphics_object.c:135 | as row 62 | fatal | d |
| 90 | GSP_RM_ALLOC FERMI_TWOD_A 0x902D, parent the channel | alloc_free.c:922 (resource_list.h:2267) | none | fatal | d |
| 91 | `NVA06F_CTRL_CMD_GPFIFO_SCHEDULE` 0xa06f0103 on the channel | kernel_channel.c:3064, from kernel_rc_watchdog.c:1216 | 0x10008, sent explicitly (g_kernel_channel_nvoc.c:510); sdk/ctrl/ctrla06f/ctrla06fgpfifo.h:66-73 | fatal | d: mark schedulable. The CPU then writes GPPut through BAR1 USERD and rings the doorbell at BAR0 0xBB0090 twice. No completion is awaited, and `krcWatchdogDisable` follows |
| 92-94 | client triple of the kernel RM API client (processID 0xFFFFFFFF; device params all 0) | osinit.c:1795-1831, reaching device.c:180-191 and subdevice.c:122-131 | as rows 14-16 | fatal (RM_INIT_ALLOC_RMAPI_FAILED) | c: shadow objects. Rows 96-98 arrive on **these** handles |
| 95 | GSP_RM_ALLOC NV40_I2C 0x402C, parent subdevice | osinit.c:1837-1850, reaching alloc_free.c:880-928 (resource_list.h:832-840) | none | tol. (hI2C = 0 skips row 98) | c |
| 96 | `NV2080_CTRL_CMD_GSP_GET_FEATURES` 0x20803601 on the kernel client's subdevice | `RmGetFirmwareVersion` osinit.c:542-566 (ROUTE_TO_PHYSICAL) | 0x40549 (g_subdevice_nvoc.c:13104); ctrl2080gsp.h:64-75 | firmwareVersion appears in /proc and nvidia-smi. tol. | c: bValid 1, firmwareVersion "610.57.04" |
| 97 | `NV2080_CTRL_CMD_BIOS_GET_INFO_V2` 0x20800810 {REVISION, OEM_REVISION} | `RmGetVbiosVersion` osinit.c:592-614 | 0x60048 (g_subdevice_nvoc.c:5904); ctrl2080bios.h:97-104 | Builds the vbios_version string. tol. | c: vbiosRevision/vbiosOemRevision from static info (matching the synthetic VBIOS, [vbios-synthesis.md](../caraxes/vbios-synthesis.md)) |
| 98 | `NV402C_CTRL_CMD_I2C_GET_PORT_INFO` 0x402c0101 on hI2C | osapi.c:4293-4312 | 0x48, always forwarded (g_i2c_api_nvoc.c:163-170); sdk/ctrl/ctrl402c.h:110-118 | One Linux i2c adapter per valid port. tol. | c: no ports |

The rest of RmInitAdapter sends **no RPC**:
- `kfifoGetUserdBar1MapInfo_TU102` (kernel_fifo_tu102.c:837-867). It is fatal if the USERD BAR1 mapping from row 56ff is missing.
- `RmInitPowerManagement` (arch/nvalloc/unix/src/dynamic-power.c:3085-3110).
- `RmRegisterGpudb`.
- `_checkP2pChipsetSupport`.
- Everything in kernel-open/nvidia/nv.c after `rm_init_adapter` (nv.c:1528-1546). The UUID is already cached at that point.

**RPCs not sent in the baseline, and what triggers them:**

| RPC | Trigger |
|---|---|
| `INTERNAL_GET_EGPU_BRIDGE_INFO` 0x20800a55 | An Intel upstream bridge (osinit.c:458-477; QEMU ioh3420 but not pcie-root-port) |
| `INTERNAL_NVLINK_GET_IOCTRL_DEVICE_INFO`, `PMGR_GET_MODULE_INFO` 0x20802609, and the KernelNvlink engine RPCs | Row 6 answered NV_OK |
| `PERF_SET_POWERSTATE` 0x2080205b | bIsMobile set (src/kernel/gpu/perf/kern_perf_pwr.c:236-241) |
| `INTERNAL_USER_SHARED_DATA_SET_DATA_POLL`, sent again by `gpuHandleRusdPollingRegistry` | Regkey RmDebugRusdPolling (gpu_registry.c:208-211) |
| PFM request-handler PERF/THERM/PMGR controls | Guest ACPI exposing an NVIDIA GPS `_DSM` (src/kernel/platform/platform_request_handler.c:384-627) |
| `INTERNAL_GPU_GET_PF_BAR1_SPA` 0x20800aee | Regkey RmGpuDirectRdmaForceSpa (kern_bus.c:204-210) |
| `INTERNAL_DISPLAY_*`, the display client and NV04_DISPLAY_COMMON (display present) | See the list below |

With display present, the display RPCs are:
- **PreInit, 5 RPCs:** `INTERNAL_DISPLAY_GET_IP_VERSION` 0x20800a4b (answer 0x04000000, DISPv0400), a client triple, and NV04_DISPLAY_COMMON (kern_disp.c:447, 463, 391-406).
- **Init, 4 RPCs, all fatal on error:**
  - `INTERNAL_DISPLAY_GET_STATIC_INFO` 0x20800a01 (kern_disp.c:570). numDispChannels must be at least 81 and i2cPort must be 16 (kern_disp.c:576-628).
  - `INTERNAL_INIT_BRIGHTC_STATE_LOAD` 0x20800ac6 (:502).
  - `INTERNAL_SET_STATIC_EDID_DATA` 0x20800adf (:543).
  - `INTERNAL_DISPLAY_WRITE_INST_MEM` 0x20800a49, with the base at reserved memory and size 0x10000 (src/kernel/gpu/disp/inst_mem/disp_inst_mem.c:393).
- **Load, 1 RPC:** `NV0073_CTRL_CMD_SYSTEM_MAP_SHARED_DATA` 0x730151 (kern_disp.c:723; tolerated).
- **After init:** row 95 gains NV04_DISPLAY_COMMON (osinit.c:1856-1868) and row 98 gains up to three NV0073 controls. One of them, `SPECIFIC_OR_GET_INFO` 0x73028b, is PRIVILEGED (g_disp_objs_nvoc.c:4298).

### 1.8 Non-RPC conditions the replies depend on

These are listed because a correct reply is useless without them:
- **PRAMIN window.** Needed by rows 20-23. Details in [kernel-rm-hw-surface.md](../caraxes/kernel-rm-hw-surface.md).
- **BAR2 walker.** Also needed by rows 20-23; same reference.
- **BAR1 root at bar1PdeBase.** Required after row 35.
- **TLB-invalidate trigger reading back 0.** See src/kernel/gpu/mmu/arch/turing/kern_gmmu_tu102.c:60-160.
- **MSI for the table vectors of row 8 and for the self-test vector 129.** See 1.7.
- **An empty PMA after load.** Otherwise `memmgrCheckZeroPmaUsage` is fatal (src/kernel/gpu/mem_mgr/mem_mgr.c:1387-1410).
- **A non-zero gidInfo.** Needed for the UUID (osinit.c:721-725).

## 2. First user-space open and nvidia-smi

**Source.** The ioctl sequence comes from Leandro's `matrix/traces/610.57.04/nvml.tsv`,
recorded on the host RTX 2070. It is the probe `nvml`, which runs `nvidia-smi -q`
([matrix/PROBES.md](../../matrix/PROBES.md):46), and it matches
`matrix/catalog-610.57.04.json`. The host is itself a GSP client of the same build,
so each routing decision below is the one the guest kernel will make. There is no
trace of plain `nvidia-smi`; it is expected to be a subset of `-q` (Open 8).

**Opens and escapes send no RPC:**
- open of /dev/nvidiactl (kernel-open/nvidia/nv.c:1853-1887)
- `CHECK_VERSION_STR` (nv.c:2682-2689)
- `SYS_PARAMS` (:2691-2709)
- `CARD_INFO` (:2606-2614)
- open of /dev/nvidia0. RmInitAdapter runs here only if it has not run yet (nv.c:1680-1700); `NVreg_GpuInitOnProbe=1` makes it run at probe time instead.
- `WAIT_OPEN_COMPLETE` (:2546-2553)
- `REGISTER_FD` (arch/nvalloc/unix/src/escape.c:921-980)
- 0xd7 `NV_ESC_NUMA_INFO`. It runs the kernel-only `FB_GET_NUMA_INFO` and returns early (src/kernel/gpu/mem_sys/kern_mem_sys_ctrl.c:1163-1222).
- the UVM ioctls

**Allocations and frees (12 RPCs).** The user's NV01_ROOT_CLIENT alloc sends nothing
itself; it has no RPC flag (src/kernel/rmapi/resource_list.h:86-91). The GSP-side
client is created lazily with the first device.

| # | RPC (user's handles) | Issued at | Ans |
|---|---|---|---|
| U1 | GSP_RM_ALLOC NV01_ROOT (on the first NV01_DEVICE_0). The params carry processID and pOsPidInfo, a guest kernel pointer | device.c:180-193 → rpc.h:84-86 | b: create or reuse a host client; drop pOsPidInfo |
| U2 | GSP_RM_ALLOC NV01_DEVICE_0. deviceId and vaMode are not copied (rpc.h:96-101) | rpc.h:94-104 | b: host device with the host's device instance; translate hClientShare and hTarget* |
| U3 | GSP_RM_ALLOC NV20_SUBDEVICE_0 | subdevice.c:122-130 → rpc.h:378 | b |
| U4 | GSP_RM_ALLOC NV2081_BINAPI | alloc_free.c:878-938 (resource_list.h:492-503) | b |
| U5 | GSP_RM_ALLOC NV04_DISPLAY_COMMON. Only with display present; otherwise the kernel fails it before any RPC (src/kernel/gpu/disp/disp_objs.c:96-100) | alloc_free.c:878-938 (resource_list.h:1194-1199) | b, or absent |
| U6 | GSP_RM_ALLOC GF100_SUBDEVICE_INFOROM 0x90E7 | same (resource_list.h:812-818) | b, or c: accept and answer its controls NOT_SUPPORTED as the host does |
| F1-F6 | FREE of dispcommon, BINAPI, InfoROM, subdevice, device, then the GSP client (device.c:319-323) | alloc_free.c:985-996; subdevice.c:248-253; device.c:278-323 | b: free the mapped host object; always NV_OK |

AMPERE_SMC_MONITOR_SESSION 0xC640 is kernel-only: its flags are NON_PRIVILEGED
only (resource_list.h:1093-1098) and its constructor sends nothing
(src/kernel/gpu/mig_mgr/mig_monitor_session.c:45-80).

**Controls.** nvidia-smi issues 87 unique commands in 157 calls. About 105 reach GSP,
by four routes:

| Route | Calls reaching GSP | Mechanism | Ans |
|---|---|---|---|
| ROUTE_TO_PHYSICAL public controls with the user's handles | 37 | resource.c:254-293. CACHEABLE ones go only once per adapter lifetime | b: the same command on the mapped host object |
| "GSS legacy": any unknown command with bit 0x8000 (the 0x2080_85xx/90xx/a0xx/a6xx numbers, unnamed in the open headers) | 54 calls, 30 unique | interface/deprecated/rmapi_deprecated_control.c:95-141 → interface/deprecated/rmapi_gss_legacy_control.c:38-139, with the user's handles; none of them have both 0xC000 bits set, so all are non-privileged | b: forward opaquely. Reply with rmctrlFlags 0, otherwise the guest caches the result (rpc.c:10902-10907) |
| BINAPI 0x20810108 | 2 | src/kernel/rmapi/binary_api.c:104-118 forwards any command | b |
| Kernel-served controls that issue their own RPC | 12 | see the next table | mixed |

The ROUTE_TO_PHYSICAL controls are:

| Group | Controls |
|---|---|
| GPU | `GPU_GET_BRAND_CAPS` 0x800294, `GPU_GET_OEM_BOARD_INFO` 0x2080013f, `GPU_GET_CHIP_DETAILS` 0x208001a4, `GSP_GET_FEATURES` 0x20803601 |
| InfoROM, ECC, FB | `GPU_GET_INFOROM_IMAGE_VERSION` 0x20800156, `GPU_GET_INFOROM_OBJECT_VERSION` 0x2080014b, `GPU_QUERY_INFOROM_ECC_SUPPORT` 0x20800157, `FB_QUERY_DRAM_ENCRYPTION_INFOROM_SUPPORT` 0x20801357, `GPU_QUERY_ECC_STATUS` 0x2080012f, `FB_GET_OFFLINED_PAGES` 0x20801322, `FB_GET_REMAPPED_ROWS` 0x20801344 |
| InfoROM BBX | `NV90E7 BBX_GET_TIME_DATA` 0x90e70102, `BBX_GET_LAST_FLUSH_TIME` 0x90e70113 |
| Bus | `BUS_GET_PEX_COUNTERS` 0x20801813, `BUS_GET_PEX_UTIL_COUNTERS` 0x20801819, `BUS_GET_PCIE_REQ_ATOMICS_CAPS` 0x20801829, `BUS_GET_PCIE_CPL_ATOMICS_CAPS` 0x20801830 |
| Perf, NVLink | `PERF_GET_CURRENT_PSTATE` 0x20802068, `PERF_GET_VID_ENG_PERFMON_SAMPLE` 0x20802087, `NVLINK_GET_PLATFORM_INFO` 0x20803083 |
| Display (only with display present) | NV0073 `SYSTEM_GET_SUPPORTED`, `GET_CONNECT_STATE`, `GET_NUM_HEADS`, `GET_ACTIVE` |

The InfoROM, ECC and FB controls and the two BBX controls returned 0x56 or 0x57 on the
host, so answering with the same status (c) is also safe.

Kernel-served controls that issue an RPC of their own:

| # | RPC | Issued at | Handles | Ans |
|---|---|---|---|---|
| K1 | `GPU_GET_INFO_V2` 0x20800102, carrying only the unknown indices 0x11, 0x23 and 0x24 with bit 31 set (2 RPCs) | src/kernel/gpu/subdevice/subdevice_ctrl_gpu_kernel.c:600-635 | user | b: clear bit 31 and forward to the host subdevice, or c: 0x11 = 0, and synthesize 0x23/0x24 (the host returns chip-unique values) |
| K2 | `INTERNAL_GPU_GET_SMC_MODE` 0x20800a4c, once per GET_INFO_V2 call that asks index 0x2a (3 RPCs) | same file :255-289; flags 0xc0 (g_subdevice_nvoc.c:6504) | internal | c: 0 (UNSUPPORTED). Internal, so not forwardable |
| K3 | `BIOS_GET_INFO_V2` 0x20800810, from the deprecated `BIOS_GET_INFO` (1 RPC, cacheable) | rmapi_deprecated_control.c:561-579 | user | c: as row 97 |
| K4 | `BUS_GET_INFO_V2` 0x20801823 {PCIE_GEN_INFO} (3 RPCs) | src/kernel/gpu/bus/kern_bus.c:1084-1118 from kern_bus_ctrl.c:223-257 | internal | b: the public control on the host subdevice, or c: Gen3 matching the emulated link registers |
| K5 | `THERMAL_SYSTEM_EXECUTE_V2_PHYSICAL` 0x20808513 (up to 3 RPCs) | subdevice_ctrl_gpu_kernel.c:4057-4345; define at sdk/ctrl/ctrl2080/ctrl2080thermal.h:137 | internal | b: host public 0x20800513, or c: synthetic temperatures |

**Kernel-served controls with no RPC** (they read cached or static data):
- NV0000 GPU id, attach and PCI queries
- `MC_GET_ARCH_INFO`, `BUS_GET_PCI_INFO`
- `GPU_GET_GID_INFO` and `GPU_GET_NAME_STRING`, both from GspStaticConfigInfo (gpu_gspclient.c:169-191, 288-298)
- `FB_GET_INFO_V2`
- `GPU_GET_PIDS`, `GPU_GET_PID_INFO`
- `TIMER_GET_GPU_CPU_TIME_CORRELATION_INFO`, which reads PTIMER in BAR0 (src/kernel/gpu/subdevice/subdevice_ctrl_timer_kernel.c:352-372)
- `GPU_GET_VIRTUALIZATION_MODE`, `GET_VGX_CAPS`, `GET_CLASSLIST_V2`

**Handle translation.** Every row in this section except K2/K4/K5 carries the guest
user's hClient/hObject. Those handles are guest-chosen (0xc1d5xxxx, 0xa55axxxx in the
trace), so the device needs a per-guest-client map onto Leandro host objects, filled at
U1-U6 and cleared at F1-F6 ([leandro-reuse.md](../caraxes/leandro-reuse.md) §4.4 items 4-5).

**Persistence.** Without nvidia-persistenced, the last close of /dev/nvidia0 shuts the
adapter down (nv.c:2065-2140). Each nvidia-smi run then replays all of section 1 and
the unload RPCs.

## 3. Answer sources

### 3.1 (a) Forward as-is (host RM as an unprivileged client)

- `GPU_GET_CONSTRUCTED_FALCON_INFO` (rows 4-5)
- `GPU_GET_VMMU_SEGMENT_SIZE` (row 34)
- `GPU_GET_OEM_BOARD_INFO` (row 79)

All three are NON_PRIVILEGED public controls on a subdevice. The host object is
Leandro's own subdevice, so the only change is the handle substitution.

### 3.2 (b) Forward with translation

| Guest RPC | Host public source (all non-privileged) | Mapping |
|---|---|---|
| INTERNAL_MEMSYS_GET_STATIC_CONFIG (row 9) | FB_GET_INFO_V2 0x20801303 | L2 size, RAM type, LTC/LTS counts; synthesize the rest |
| STATIC_KGR_GET_CAPS (39) | GR_GET_CAPS_V2 0x20801227 | capsTbl into engineCaps[0] |
| STATIC_KGR_GET_INFO (40) | GR_GET_INFO_V2 0x20801228 | index = i for all i |
| STATIC_KGR_GET_FLOORSWEEPING_MASKS (41) | GR_GET_GPC_MASK, GR_GET_TPC_MASK, GR_GET_NUM_TPCS_FOR_GPC, GR_GET_ZCULL_MASK, GR_GET_GFX_GPC_AND_TPC_INFO, GPU_GET_PES_INFO | per GPC; physGpcMask = gpcMask |
| STATIC_KGR_GET_GLOBAL_SM_ORDER (42) | GR_GET_GLOBAL_SM_ORDER 0x2080121b | field by field |
| PPC_MASKS / ZCULL_INFO / ROP_INFO / SM_ISSUE_* (43-48) | GR_GET_PPC_MASK / GR_GET_ZCULL_INFO / GR_GET_ROP_INFO / GR_GET_SM_ISSUE_* | ZCULL and ROP are identical layouts |
| STATIC_KGR_GET_CONTEXT_BUFFERS_INFO (52) | GR_GET_ENGINE_CONTEXT_PROPERTIES 0x2080122d per engineId | NOT_SUPPORTED maps to size NV_U32_MAX |
| All user-handle RPCs of section 2 | the same command or class on the mapped host object | handle map |

The KGR group is the only internal-control group with lossless public equivalents
(the host kernel serves the public GR controls from the same internal structs,
kernel_graphics.c:2850-2854, 4097-4111). A capture of these replies from the host
can be made now, with no GPU risk, through `crates/nvrm-client`.

### 3.3 (c) Synthesize

The values are given per row in section 1. They come from four sources:
- **The device's own configuration:** BAR sizes (row 10), PCI ids (row 2), bar1/bar2 PDE bases, handles, the VBIOS revision (row 97), and the firmware string "610.57.04" (row 96).
- **Vendored headers and HAL code:**
  - regBases (row 2, kern_gpu_tu102.c:275-306)
  - the interrupt subtree layout (row 8, intr_tu102.c:1386-1417)
  - the display IP version (1.7)
  - the TU10x per-subcontext default (row 51)
  - P2P status codes (row 80)
- **Fixed NV_OK or NV_ERR_NOT_SUPPORTED** where the caller ignores or tolerates the reply:
  rows 1, 6, 11-13, 17-18, 21-24, 29-33, 35, 50, 53-55, 64-69, 78, 80, 95, 98. Row 87 is a constant (0x1000).
- **Model tables that have no public source:**
  - FIFO_GET_DEVICE_INFO_TABLE (row 7)
  - the interrupt table (row 8)
  - the fault-buffer sizes (row 19)
  - the channel count (row 25)
  - the FECS record size (row 49)

### 3.4 (d) Fake with care

- **UPDATE_BAR_PDE (row 20).** It makes the device a GMMU walker for BAR2.
- **The three FERMI_VASPACE_A + COPY_SERVER_RESERVED_PDES pairs (rows 59-60, 73-74, 84-85).** Record each VAS root.
- **The three channel allocs (61, 75, 88).** Record the ChID with its instance block, RAMFC, USERD, method buffer and VAS.
- **PROMOTE_CTX (62, 76, 89).** Echo the params and record the context-buffer VAs.
- **TURING_A (63, 77) and FERMI_TWOD_A (90).**
- **GPFIFO_SCHEDULE (91).** It is followed by two real doorbell writes; the device may ignore them while the watchdog is disabled.
- **The CeUtils channels (1.6).** These are not a fake at all: the kernel verifies CE results.

### 3.5 Internal controls that occur, and what the kernel does with the reply

"INTERNAL" here means flag 0x80 or an `_INTERNAL_` name. There are 35 occurrences in
the baseline and 3 more from nvidia-smi.

| Control | Rows | What the kernel does with the reply |
|---|---|---|
| INTERNAL_GPU_GET_CHIP_INFO | 2 | PCI ids into pGpu; regBases for the TIMER and USERMODE apertures |
| INTERNAL_GPU_GET_USER_REGISTER_ACCESS_MAP | 3 | user regops permission bitmap; reserved-FB size |
| INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO | 6 | NVLink presence (error means the engine is removed) |
| INTERNAL_INTR_GET_KERNEL_TABLE | 8 | the whole interrupt routing: LEAF_EN programming, ISR engine lookup, UVM vectors |
| INTERNAL_MEMSYS_GET_STATIC_CONFIG | 9 | L2 size, comptag policy, PRAMIN placement |
| INTERNAL_BIF_GET_STATIC_INFO | 11 | four PCIe properties |
| INTERNAL_INIT_USER_SHARED_DATA / USER_SHARED_DATA_SET_DATA_POLL | 12-13 | nothing (RUSD page address handed to GSP) |
| INTERNAL_GMMU_GET_STATIC_INFO | 19 | fault buffer sizes for UVM |
| INTERNAL_MEMSYS_L2_INVALIDATE_EVICT | 21-23 | status only |
| INTERNAL_PERF_GPU_BOOST_SYNC_GET_INFO | 24, 29 | SLI boost-sync parameters |
| INTERNAL_FIFO_GET_NUM_CHANNELS | 25 | ChID heap and USERD sizes |
| INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_TYPE | 30 | decompPceMask |
| CE_UPDATE_CLASS_DB (flag 0x80) | 31, 33 | which CE classes exist |
| INTERNAL_GMMU_COPY_RESERVED_SPLIT_GVASPACE_PDES_TO_SERVER | 35 | status only |
| INTERNAL_STATIC_KGR_* (14 controls) | 39-52 | the whole KernelGraphics static info, served back to clients through the public GR controls |
| NV0080 INTERNAL_KGR_INIT_BUG4208224_WAR | 78 | status only |
| INTERNAL_GET_PCIE_P2P_CAPS | 80 | P2P capability reporting |
| INTERNAL_GPU_GET_SMC_MODE | K2 | GET_INFO_V2 index 0x2a |

Controls that are not INTERNAL but are still not forwardable, because they are
kernel-privileged or PRIVILEGED:
- FIFO_GET_DEVICE_INFO_TABLE (row 7)
- CE_UPDATE_PCE_LCE_MAPPINGS_V2 (32)
- CE_GET_FAULT_METHOD_BUFFER_SIZE (87)
- GPU_PROMOTE_CTX (62, 76, 89)
- NV90F1 COPY_SERVER_RESERVED_PDES (60, 74, 85)

## 4. Summary

### 4.1 Counts

**Section 1 baseline** (display absent, default registry, not counting the CeUtils
channels): **98 RPCs**.

| By kind | Count |
|---|---|
| Dedicated RPCs (fn 228, fn 70) | 2 |
| GSP_RM_ALLOC | 32 |
| FREE | 10 |
| GSP_RM_CONTROL | 54 |

| By answer category | Count |
|---|---|
| (a) forward as-is | 4 |
| (b) forward with translation | 12 |
| (c) synthesize | 65 |
| (d) fake with care | 17 |

**Variations on the baseline:**
- Display present adds 10 RPCs (1.7).
- `RmDisableGlobalCeUtils=1` and `RMDisableScrubOnFree=1` remove the CeUtils channel RPCs, which are not counted above.
- `RmRcWatchdog=0` removes rows 81-91.
- A gpcMask of 0 in row 41 removes rows 56-78, at the cost of GR.

**Section 2** (`nvidia-smi -q`, display present on the host trace): about **117 RPCs** =
6 GSP_RM_ALLOC + 6 FREE + about 105 GSP_RM_CONTROL.
- By category: (b) about 113, (c) 4 (K2 x3, K3), (a) 0.
- With display absent, the DispCommon alloc, its FREE and 6 NV0073 control calls drop out.

### 4.2 The five most difficult RPCs

1. **UPDATE_BAR_PDE (row 20).** The RPC itself is trivial. Answering it correctly
   means the device must translate every CPU access to BAR2 through the kernel's own
   GMMU page tables in emulated FB, and keep PRAMIN, BAR2 and (after row 35) BAR1
   coherent over the same bytes. `kbusVerifyBar2` checks that coherence immediately
   (rows 21-23) and fails init otherwise.
2. **CeUtils channels and memmgrTestCeUtils (1.6).** This is the only point in init
   where the kernel checks that the GPU really did work: a CE memset and copy with a
   compare. It needs channel state, GPFIFO fetch, CE execution and semaphore release.
   The workaround is two registry keys; forwarding the channel to the host means guest
   FB PAs must be real memory.
3. **FIFO_GET_DEVICE_INFO_TABLE (row 7).** It is kernel-privileged on the host, so it
   cannot be forwarded, and there is no public equivalent for the whole table. It must
   agree with engineCaps, CONSTRUCTED_FALCON_INFO and the interrupt table. A wrong
   answer does not fail init; it silently removes GR and then all CEs.
4. **INTERNAL_INTR_GET_KERNEL_TABLE (row 8).** The reply is a contract the device
   must then honour with MSI. There are six structural constraints; three are checked
   only later, in `intrStateLoad_TU102`. The GSP message-queue interrupt, the UVM
   vectors and the non-stall notifications all depend on it. The public host
   equivalents may not be served on bare metal (Open 4).
5. **The golden-channel and WAR sequences, TURING_CHANNEL_GPFIFO_A + PROMOTE_CTX
   (rows 61-62, 75-76, and 88-89 for the watchdog).** The params carry guest-FB
   physical addresses and a kernel-chosen ChID, so nothing can be forwarded. A faked
   GSP can accept them only as long as those channels never execute. Once real GR work
   is wanted (E4 and later), this is where the design question "whose FB is it"
   ([open-questions.md](caraxes-open-questions-2026-09-23.md) Q5) must be answered.

Runner-up: the section 2 user-handle forwards. Each one is easy, but together they need
a complete per-client handle map, including handles embedded in params, and correct
FREE cascades.

## 5. Open

1. **CeUtils RPC list (1.6).** The exact RPCs and order of the scrubber and global
   CeUtils setups were not traced statically. They start with a client triple, VAS,
   channel and CE object like rows 81-91; see src/kernel/gpu/mem_mgr/ce_utils.c:164-313 and
   src/kernel/gpu/mem_mgr/arch/maxwell/mem_utils_gm107.c. Nor is it known whether
   anything else still needs a CE channel with both registry keys set.
2. **First caller of the engine list (row 7).** The first sender was placed at
   `gpuRemoveMissingEngines` → `kgraphicsIsPresent`. Another path
   (`gkflcnRegisterIntrService`, src/kernel/gpu/falcon/kernel_falcon.c:377, from Intr
   PreInit intr.c:820) triggers it later. No earlier IsPresent caller was found, but
   none was ruled out exhaustively. Either way it is sent once.
3. **Values not in the open tree:**
   - the TU106 access map (row 3)
   - the constructed-falcon table (rows 4-5)
   - the device-info table (row 7)
   - the fault-buffer entry count (row 19)
   - the channel count (row 25)
   - the FECS record size and trace defines (rows 49-50)
   - the fault-method buffer size (row 87)
   - numDispChannels (display present)

   Forward, or capture from the host through public controls with nvrm-client; never
   read the host GPU's registers.
4. **Interrupt table source.** Whether a bare-metal host answers
   `MC_GET_STATIC_INTR_TABLE`, `MC_GET_ENGINE_NOTIFICATION_INTR_VECTORS` and
   `MC_GET_INTR_CATEGORY_SUBTREE_MAP` for an ordinary client. The SDK describes them
   as vGPU-oriented.
5. **Row 35 branch.** Row 35 assumes that no resource-server call context is live
   during RmInitAdapter. If one were, the RPCs would be FERMI_VASPACE_A, then NV90F1
   COPY_SERVER_RESERVED_PDES, then FREE, instead of the internal control
   (gpu_vaspace.c:3852-3938).
6. **FREE ordering.** The order of FREEs inside `Free(hClient,hClient)` cascades
   (golden rows 66-67; section 2 F2/F3) is not fixed statically. The device must accept
   any order and FREEs of already-freed children.
7. **PROMOTE_CTX entry set.** The exact entries in rows 62, 76 and 89 depend on the
   static GR info: FECS logging, the priv access map, RTV and GFXP.
8. **nvidia-smi coverage.** There is no trace of plain `nvidia-smi` or of `nvidia-smi`
   with display absent. Whether NVML skips NV04_DISPLAY_COMMON when the class list
   lacks it is unverified. The guest tracer (open-questions Q23) resolves both.
9. **GSS-legacy semantics.** The meaning of the 30 GSS-legacy command numbers and of
   0x20810108 is unknown. Forwarding works without knowing it, but it prevents
   identity mediation of anything they return.
10. **Adapter teardown.** Unload, including the `UNLOADING_GUEST_DRIVER` RPC and the
    FREE cascades, is out of scope here, yet it happens after every nvidia-smi run
    without persistence mode.
11. **Topology-dependent RPCs.** EGPU_BRIDGE_INFO and the PFM request-handler controls
    depend on the VM topology and ACPI tables. The baseline assumes Cloud Hypervisor
    with no Intel root port and no NVIDIA `_DSM`; verify this in E3.
12. **Verification.** Every order in this file is static. E3's device log is the check
    ([open-questions.md](caraxes-open-questions-2026-09-23.md) Q8, Q9).
