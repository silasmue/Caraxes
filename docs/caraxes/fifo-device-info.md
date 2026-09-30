<!-- SPDX-License-Identifier: MIT -->
# FIFO device info table for the synthetic TU106

This file specifies the answer to row 7 of [phase5-rpcs.md](../history/caraxes-phase5-rpcs-610.md):
`NV2080_CTRL_CMD_FIFO_GET_DEVICE_INFO_TABLE` (0x20801112). The concrete table is in
[caraxes/tools/fifo-table-tu106.json](../../caraxes/tools/fifo-table-tu106.json). The host
values in it come from [caraxes/tools/fifo-host-query.rs](../../caraxes/tools/fifo-host-query.rs),
which ran on the RTX 2070 host on 2026-09-23 (§5).

**Paths.** Paths are relative to `vendor/open-gpu-kernel-modules/src/nvidia` unless they start
with `sdk/` or `published/`:
- `sdk/` stands for `vendor/open-gpu-kernel-modules/src/common/sdk/nvidia/inc`.
- `published/` stands for `vendor/open-gpu-kernel-modules/src/common/inc/swref/published`.
- `g_*` files are in `generated/`.

Driver 610.57.04 on both sides. The host runs the same open module
(`/proc/driver/nvidia/version`: 610.57.04), so NVOC class ids and enum values match between
host and guest.

## 1. What the device must return (summary)

1. **Twelve entries** on one page (`numEntries` = 12, `bMore` = 0):
   - GR0
   - CE0, CE1, CE2, CE3, CE4
   - NVDEC0, NVDEC1, NVDEC2
   - NVENC0
   - SEC2
   - SW, which is **last**. `kfifoGetNumEngines_GM107` returns `engineInfoListSize-1` and
     does not count the SW entry at the end of the list (arch/maxwell/kernel_fifo_gm107.c:821-822).
2. **`engineData[RM_ENGINE_TYPE]` holds `RM_ENGINE_TYPE_*`, not `NV2080_ENGINE_TYPE_*`.**
   - The GSP-client path does not translate anything; only a vGPU guest converts from NV2080
     values (src/kernel/gpu/fifo/kernel_fifo.c:1935-1953 and 2162-2165).
   - The GR0 entry must have `RM_ENGINE_TYPE_GR0` = 1, or KernelGraphics is removed
     (src/kernel/gpu/gr/kernel_graphics.c:417-435). That removal caused the E4 crash.
3. **`engineData[ENG_DESC]`** holds the physical descriptor: NVOC class id << 8 | instance.
   GR uses `Graphics`, not `KernelGraphics` (§3).
4. **`engineData[MC]`** is an `MC_ENGINE_IDX_*` index below `MC_ENGINE_IDX_MAX` (181), unique
   per engine. The kernel uses it as an array index into the interrupt service records
   (src/kernel/gpu/falcon/kernel_falcon.c:377-398).
5. **`IS_HOST_DRIVEN_ENGINE`** is 1 for every entry except SW:
   - CE presence requires it (arch/maxwell/kernel_fifo_gm107.c:1104-1121).
   - `numRunlists` counts it (src/kernel/gpu/fifo/kernel_fifo.c:2134-2137).
   - That count excludes SW by definition (g_kernel_fifo_nvoc.h:309-314).
6. **MMU fault ids are the host's:**
   - GR 64
   - CE0-CE4 15-19
   - NVDEC0/1/2 10/25/26
   - NVENC0 11
   - SEC2 14
   - SW 0xFFFFFFFF
7. **GR0 has two PBDMAs.** CE0 shares GR's runlist and its first PBDMA; CE1 shares GR's
   runlist and its second PBDMA. Every other engine has its own runlist and one PBDMA.
8. **PBDMA fault ids are contiguous:** `pbdmaFaultId = base + pbdmaId`. The kernel derives the
   PBDMA id from that difference (src/kernel/gpu/fifo/kernel_fifo.c:3655-3659, contiguity flag
   g_kernel_fifo_nvoc.c:305-315). The range must stay clear of the engine fault ids.

Fields that the Turing kernel RM never reads:
- RC_MASK, RESET, INTR, DEV_TYPE_ENUM, INSTANCE_ID, RUNLIST_ENGINE_ID.
- RUNLIST_PRI_BASE and CHRAM_PRI_BASE, which engine_info.h:71-96 marks Ampere+.
- FIFO_TAG, whose only kernel consumer is `GPU_GET_HW_ENGINE_ID`, unsupported on Turing (§4.6).

These fields are synthesized (§6).

## 2. Wire format

`NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS` (sdk/ctrl/ctrl2080/ctrl2080fifo.h:535-576) is
3212 bytes. That matches the E4 trace `paramsSize=3212` (caraxes/runs/e4-2026-09-23/iter1-boot-events.txt:57).

| Offset | Field | Notes |
|---|---|---|
| 0 | `baseIndex` u32 | in; the kernel sends 0, 32, ... (kernel_fifo.c:2044-2050) |
| 4 | `numEntries` u32 | at most 32 (`MAX_ENTRIES`, ctrl2080fifo.h:538); more is `NV_ERR_INVALID_STATE` (kernel_fifo.c:2062-2068) |
| 8 | `bMore` NvBool | 0 ends the loop (kernel_fifo.c:2077-2080) |
| 12 + 100·i | `entries[i]` | 32 entries |

The 100-byte `NV2080_CTRL_FIFO_DEVICE_ENTRY` (ctrl2080fifo.h:561-567):

| Offset | Field | Kernel copy |
|---|---|---|
| 0 | `engineData[16]` u32 | only the first 15 (`ENGINE_INFO_TYPE_ENGINE_DATA_ARRAY_SIZE`, engine_info.h:104) are copied (kernel_fifo.c:2100-2104); index 15 is ignored |
| 64 | `pbdmaIds[2]` | the first `numPbdmas` are copied |
| 72 | `pbdmaFaultIds[2]` | the first `numPbdmas` are copied |
| 80 | `numPbdmas` | must be ≤ 2, else `NV_ERR_INVALID_STATE` (kernel_fifo.c:2110-2118) |
| 84 | `engineName[16]` | copied as a string (kernel_fifo.c:2129-2132) |

**Empty table.** With `numEntries` = 0 the kernel allocates a zero-length list. The allocation
returns NULL, the check at kernel_fifo.c:2088 fails, and the call returns `NV_ERR_NO_MEMORY`.
`engineInfoList` stays NULL, so every later `kfifoEngineInfoXlate` retries the fetch
(kernel_fifo.c:2195-2196, arch/maxwell/kernel_fifo_gm107.c:692-697). That explains the 20 repeated
0x20801112 RPCs in E4 (iter1-boot-events.txt:57-98).

## 3. Numeric values in this build

**Descriptor.**
- `ENGDESC_CLASS` is bits 31:8 and `ENGDESC_INST` is bits 7:0 (g_eng_desc_nvoc.h:55-56).
- `MKENGDESC(class, inst)` = `class << 8 | inst` (g_eng_desc_nvoc.h:58-59).
- `ENGDESC_FIELD` extracts a field (g_eng_desc_nvoc.h:61-62).

The FIFO engine descriptors use the *physical* engine classes (g_eng_desc_nvoc.h:1007, 1055,
1074, 1104-1107):

| Macro | Class (g_eng_desc_nvoc.h) | Class id | Value |
|---|---|---|---|
| `ENG_GR(0)` (:1249) | `Graphics` (:201) | 0xd334df | **0xd334df00** |
| `ENG_CE(i)` (:1231) | `OBJCE` (:801) | 0x793ceb | **0x793ceb00 + i** |
| `ENG_NVDEC(i)` (:1243) | `OBJBSP` (:537) | 0x8f99e1 | **0x8f99e100 + i** |
| `ENG_NVENC(i)` (:1237) | `OBJMSENC` (:817) | 0xe97b6c | **0xe97b6c00 + i** |
| `ENG_SEC2` (:1190) | `OBJSEC2` (:689) | 0x28c408 | **0x28c40800** |
| `ENG_SW` (:1125) | `OBJSWENG` (:81, g_objsweng_nvoc.h:122) | 0x95a6f5 | **0x95a6f500** |

The kernel-side engine objects have different classes. They never appear in `engineData`:

| Class | Id | Source |
|---|---|---|
| `KernelGraphics` | 0xea3fa9 | g_eng_desc_nvoc.h:257 |
| `KernelCE` | 0x242aca | g_eng_desc_nvoc.h:809 |
| `KernelSec2` | 0x2f36c9 | g_eng_desc_nvoc.h:889 |

There is no `KernelNvdec` or `KernelNvenc` class. Video engines become `KernelVideoEngine` plus
`GenericKernelFalcon` objects, built from the constructed-falcon list (src/kernel/gpu/gpu.c:5706-5846).

**Other values** for the engines in the table:

| Engine | `RM_ENGINE_TYPE` (inc/kernel/gpu/gpu_engine_type.h) | `NV2080_ENGINE_TYPE` (sdk/class/cl2080_notification.h) | `MC_ENGINE_IDX` (inc/kernel/gpu/intr/engine_idx.h) | TU10x MMU fault id |
|---|---|---|---|---|
| GR0 | 0x01 (:35) | 0x01 (:284) | 84 (:128-129) | 64 (published/volta/gv100/dev_fault.h:26) |
| CE0-CE4 | 0x09-0x0d (:43-47) | 0x09-0x0d (:293-297) | 15-19 (:54-58) | 15-19 (published/turing/tu102/dev_fault.h:39-43) |
| NVDEC0/1/2 | 0x1d/0x1e/0x1f (:63-65) | 0x13/0x14/0x15 (:303-306) | 65/66/67 (:107-111) | 10/25/26 (tu102/dev_fault.h:34-36) |
| NVENC0 | 0x25 (:71) | 0x1b (:312) | 38 (:78) | 11 (tu102/dev_fault.h:50) |
| SEC2 | 0x31 (:83) | 0x26 (:325) | 47 (:87) | 14 (`SEC`, tu102/dev_fault.h:31) |
| SW | 0x2d (:79) | 0x22 (:320) | 0 = `MC_ENGINE_IDX_NULL` (:39; SW row src/kernel/gpu/fifo/kernel_fifo.c:3520) | none (host: 0xFFFFFFFF) |

- MC indices: the vGPU guest table maps the same engines to exactly these `MC_ENGINE_IDX`
  values (kernel_fifo.c:3490-3521). The field comment "log2(MC_ENGINE_*)" in engine_info.h:63
  is stale: the value is an index.
- GR fault range: `kfifoEngineInfoXlate_GV100` maps the whole range from the GR fault id to GR
  fault id + maxSubctx - 1 onto GR (arch/volta/kernel_fifo_gv100.c:112-125). With 64
  subcontexts (host, §5) that range is 64..127.

## 4. Semantics of each field and how the kernel consumes it

### 4.1 Fetch

`kfifoConstructEngineList_KERNEL` (kernel_fifo.c:2187-2207) runs lazily, on the first call to
any of these:
- `kfifoEngineInfoXlate_GM107` (arch/maxwell/kernel_fifo_gm107.c:692-697)
- `kfifoGetNumEngines_GM107` (:811-817)
- `kfifoGetEnginePbdmaIds_GM107` (:898-904)

On a GSP client it first calls `gpuConstructDeviceInfoTable_HAL`. For TU106 that is the stub
`_ac1694`: TU10x is not in the FWCLIENT ChipHal mask (g_gpu_nvoc.c:864-881). So
`NV2080_CTRL_CMD_INTERNAL_GET_DEVICE_INFO_TABLE` is **not** sent.

`kfifoGetHostDeviceInfoTable_KERNEL` (kernel_fifo.c:1987-2175) then does the following:
1. Targets the internal client and subdevice (:2013-2017).
2. Pages through the table with `baseIndex` (:2044-2081).
3. Copies each entry (:2096-2132).
4. Counts `numRunlists` = entries with `IS_HOST_DRIVEN_ENGINE` ≠ 0 (:2134-2137).
5. Tracks `maxRunlistId` over **all** entries, SW included (:2138-2139). Sets
   `maxNumRunlists = maxRunlistId + 1` (:2168).
6. Tracks `maxPbdmaId` and sets `maxNumPbdmas = maxPbdmaId + 1` (:2141-2143, 2169).
7. Sets a bit in `validEngineIdsForPbdmas` for each PBDMA fault id of every entry except
   `ENG_SW` (:2145-2152).
   - The bit vector has `PBDMA_FAULT_MAX_ID` = 512 bits (g_kernel_fifo_nvoc.h:116, width of
     `NVC369_BUF_ENTRY_ENGINE_ID`, sdk/class/clc369.h:50).
   - `kfifoReservePbdmaFaultIds` is a no-op on TU106; only GB100+ has a body
     (g_kernel_fifo_nvoc.c:738-748).
8. Skips `_kfifoLocalizeGuestEngineData`, which runs only when `IS_VIRTUAL` (:2162-2165).

On TU106 there is one CHID manager (`bUsePerRunlistChram` = FALSE, g_kernel_fifo_nvoc.c:233-243;
kernel_fifo.c:111-119, 167-170). So runlist ids only need to stay below
`MAX_NUM_RUNLISTS` = 64 (g_kernel_fifo_nvoc.h:106, sdk/ctrl/ctrl2080/ctrl2080internal.h:1114).

### 4.2 Lookup rules

`kfifoEngineInfoXlate_GV100` is bound for TU102-TU117 (g_kernel_fifo_nvoc.c:619-631). It
defers to `kfifoEngineInfoXlate_GM107` (arch/maxwell/kernel_fifo_gm107.c:673-759), which works
as follows:
- It returns the **first** entry whose `engineData[inType]` equals the input value.
- With `ENGINE_INFO_TYPE_INVALID` it indexes the list directly. With `ENGINE_INFO_TYPE_PBDMA_ID`
  it searches `pbdmaIds`.
- It returns `NV_ERR_OBJECT_NOT_FOUND` when no entry matches.
- Asking a non-host-driven entry for RUNLIST, RUNLIST_PRI_BASE, RUNLIST_ENGINE_ID, PBDMA_ID,
  CHRAM_PRI_BASE or FIFO_TAG only logs an error and still returns the value (:640-670, 739-751).

Consequences:
- GR0 must come before CE0 and CE1, so that a RUNLIST 0 → ENG_DESC lookup returns GR.
- SW's zeros are harmless because SW is last.

### 4.3 Engine presence (gpuStatePreInit → gpuRemoveMissingEngines)

`gpuRemoveMissingEngines` (src/kernel/gpu/gpu.c:1749-1847) runs from `gpuStatePreInit`
(gpu.c:2200). It destroys every engine whose `engstateIsPresent` is false (gpu.c:1775-1783). The
engine table decides:

| Engine object | isPresent | Table requirement |
|---|---|---|
| KernelGraphics[0] | `kgraphicsIsPresent_IMPL` (kernel_graphics.c:417-435): Xlate RM_ENGINE_TYPE = `RM_ENGINE_TYPE_GR(0)` succeeds | an entry with `engineData[2]` = 1 |
| KernelCE[i], i = 0..8 (`GPU_CHILD_PRESENT(KernelCE, 9)`, src/kernel/gpu/arch/turing/kern_gpu_tu102.c:189) | `kceIsPresent_IMPL` (src/kernel/gpu/ce/kernel_ce.c:86-95) → `gpuCheckEngine_HAL(ENG_CE(i))` → `ENG_CLASS_CE` case (gpu.c:6884-6911) → `kfifoCheckEngine_GM107` (kernel_fifo_gm107.c:1104-1121) | ENG_DESC = `ENG_CE(i)` with `IS_HOST_DRIVEN_ENGINE` ≠ 0; CE5-CE8 have no entry and are removed |
| video/SEC2 classes | `ENG_CLASS_NVENC/SEC2/NVDEC` cases use `GspStaticConfigInfo.engineCaps` (gpu.c:6842, 6913-6921), **not** the FIFO table | none |
| `ENG_GR(0)` itself (class DB) | always true (gpu.c:6931-6936) | none |

Before this fix the empty table made KernelGraphics[0] absent. Then
`memmgrCalcReservedFbSpaceHal_GM107` failed as follows:
1. It takes `GPU_GET_KERNEL_GRAPHICS(pGpu, 0)` (src/kernel/gpu/mem_mgr/arch/maxwell/mem_mgr_gm107.c:1857).
2. It calls `kgraphicsDoesUcodeSupportPrivAccessMap(pGpu, pKernelGraphics)` on the NULL pointer
   (mem_mgr_gm107.c:1999; inline accessor g_kernel_graphics_nvoc.h:1128). This is the E4 oops.

### 4.4 Interrupt service registration (intrStatePreInitLocked)

`intrStatePreInitLocked_IMPL` (src/kernel/gpu/intr/intr.c:806-825) calls `_intrInitServiceTable`
(intr.c:1080-1096). That registers every IntrService child and then every GenericKernelFalcon
(gpu.c:5921-5929). For generic falcons, `gkflcnRegisterIntrService_IMPL`
(kernel_falcon.c:362-400) works as follows:
- Engines that are neither video nor SEC2 return early (kernel_falcon.c:369-370).
- Video engines translate `ENG_DESC = physEngDesc → MC` (:377-379).
- A lookup failure asserts at kernel_falcon.c:391. This is the E4 assertion; see §8.2.
- A successful lookup writes `pRecords[mcIdx]` (:396-398).

So every video falcon in the host's constructed-falcon list needs an entry with a valid,
unique MC. GR, CE and SEC2 register fixed MC indices without the table:
- `MC_ENGINE_IDX_GRn` (kernel_graphics.c:2650-2667)
- `MC_ENGINE_IDX_CE(publicID)` (kernel_ce.c:677-689)
- `MC_ENGINE_IDX_SEC2` (src/kernel/gpu/sec2/kernel_sec2.c:547-568)

The table's MC values must equal those constants so that the two views agree.

### 4.5 Consumers after init (channels, faults, controls)

| Field | Consumer (TU106 path) | Requirement |
|---|---|---|
| RUNLIST | `kfifoGetDefaultRunlist_GM107` (kernel_fifo_gm107.c:349-378) at TSG creation (src/kernel/gpu/fifo/kernel_channel_group.c:150-153); `kfifoRunlistSetIdByEngine_GM107` (kernel_fifo_gm107.c:448-490, skips ENG_SW/ENG_BUS at :474); `kchannelGetEngine_GM107` (arch/maxwell/kernel_channel_gm107.c:617); FIFO_GET_INFO per-engine indices (src/kernel/gpu/fifo/kernel_fifo_ctrl.c:317-335) | consistent ids; GR0/CE0/CE1 equal |
| RUNLIST + PBDMA ids | `_kfifoIsValidCETag_GP102` (arch/pascal/kernel_fifo_gp102.c:182-260) decides GRCE by "same runlist as GR and a shared PBDMA", used when validating engine/runqueue (:95, :153); `kfifoGetEnginePartnerList_GM107` (kernel_fifo_gm107.c:939-1038) | CE0/CE1 share runlist and PBDMA with GR |
| numPbdmas | runqueue index must be < numPbdmas (kernel_fifo_gm107.c:977-981); `kfifoGetNumRunqueues` = 2 on TU106 (g_kernel_fifo_nvoc.c:587-595, g_kernel_fifo_nvoc.h:2359-2361); SCG COMPUTE1 cannot use runqueue 0 (kernel_fifo_gp102.c:52-58) | GR needs 2 PBDMAs for runqueue 1 |
| MMU_FAULT_ID | `kfifoEngineInfoXlate_GV100` GR range (kernel_fifo_gv100.c:112-125); `kgmmuPrintFaultInfo_GV100` (src/kernel/gpu/mmu/arch/volta/kern_gmmu_gv100.c:583-597); `kgmmuServiceMmuFault_GV100` (:2092); `kfifoPrintInternalEngine_TU102` (src/kernel/gpu/fifo/arch/turing/kernel_fifo_tu102.c:182-271); `GPU_GET_ENGINE_FAULT_INFO` for clients such as UVM (src/kernel/gpu/subdevice/subdevice_ctrl_gpu_kernel.c:1805-1807); CE fault range 15..23 (kern_gmmu_gv100.c:1289-1290) | the TU10x hardware ids of §3 |
| pbdmaFaultIds | `kfifoIsMmuFaultEngineIdPbdma` (kernel_fifo.c:3616-3627), `kfifoGetPbdmaIdFromMmuFaultId` (:3638-3662), `kfifoGetEngineTypeFromPbdmaFaultId` (:3675-3736) | contiguous; outside 0..31 and 64..127 |
| MC | falcon interrupt registration (§4.4) | `MC_ENGINE_IDX_*` |
| RM_ENGINE_TYPE | presence (§4.3), all client controls | `RM_ENGINE_TYPE_*` |
| ENG_DESC | presence of CE, falcons, channel binding; `ENG_SW` excluded from PBDMA fault ids (kernel_fifo.c:2149) and from runlist binding (kernel_fifo_gm107.c:474) | §3 values |

### 4.6 Fields the Turing kernel RM does not read

Grep over src/ for every `ENGINE_INFO_TYPE_*` reader, excluding the Ampere+ arch directories,
finds no reader for RC_MASK, RESET, INTR, DEV_TYPE_ENUM, INSTANCE_ID or RUNLIST_ENGINE_ID. The
remaining fields are read only on paths that do not run on TU106:

| Field | Only reader | Why it does not apply |
|---|---|---|
| RUNLIST_PRI_BASE, CHRAM_PRI_BASE | `nvGpuOpsRetainChannel` (UVM), inside `isDeviceAmperePlus` (src/kernel/rmapi/nv_gpu_ops.c:10478-10509); the privileged control `GPU_GET_ENGINE_RUNLIST_PRI_BASE` (subdevice_ctrl_gpu_kernel.c:2532-2545) | Ampere+ only |
| FIFO_TAG | `GPU_GET_HW_ENGINE_ID` (subdevice_ctrl_gpu_kernel.c:2616-2618) | returns `NV_ERR_NOT_SUPPORTED` unless `bHostEngineExpansion`, which is FALSE on TU106 (g_kernel_fifo_nvoc.c:257-267); the host confirmed status 0x56 (§5) |
| FIFO_TAG | the vGPU manager (src/kernel/virtualization/common_vgpu_mgr.c:207-217) | not used in this setup |

Those fields therefore carry synthesized values.

## 5. What the host card says

The tool `caraxes/tools/fifo-host-query.rs` builds with `rustc` alone. It works as follows:
- It allocates NV01_ROOT_CLIENT, NV01_DEVICE_0 and NV20_SUBDEVICE_0 on `/dev/nvidiactl`.
- It issues only RM controls on the subdevice.
- It frees the client.

It does no register access and no mapping. Run by the agent as uid 1000 on 2026-09-23. Output,
verbatim except that falcon lines are shortened:

```text
GPU_GET_ENGINES_V2 status=0x0
engines count=9 list=GR0(0x1) COPY0(0x9) COPY1(0xa) COPY2(0xb) COPY3(0xc) COPY4(0xd) NVDEC0(0x13) NVENC0(0x1b) SW(0x22)
engine GR0 type=0x1 fault_info_status=0x0 mmuFaultId=64 subctx=1 partners[rq0] status=0x0 [COPY0] partners[rq1] status=0x0 [COPY1]
engine COPY0 type=0x9 fault_info_status=0x0 mmuFaultId=15 subctx=0 partners[rq0] status=0x0 [GR0] partners[rq1] status=0x1f [] pce_mask_status=0x0 pceMask=0x1
engine COPY1 type=0xa fault_info_status=0x0 mmuFaultId=16 subctx=0 partners[rq0] status=0x0 [GR0] partners[rq1] status=0x1f [] pce_mask_status=0x0 pceMask=0x1
engine COPY2 type=0xb fault_info_status=0x0 mmuFaultId=17 subctx=0 partners[rq0] status=0x0 [] partners[rq1] status=0x1f [] pce_mask_status=0x0 pceMask=0x4
engine COPY3 type=0xc fault_info_status=0x0 mmuFaultId=18 subctx=0 partners[rq0] status=0x0 [] partners[rq1] status=0x1f [] pce_mask_status=0x0 pceMask=0x8
engine COPY4 type=0xd fault_info_status=0x0 mmuFaultId=19 subctx=0 partners[rq0] status=0x0 [] partners[rq1] status=0x1f [] pce_mask_status=0x0 pceMask=0x2
engine NVDEC0 type=0x13 fault_info_status=0x0 mmuFaultId=10 subctx=0 partners[rq0] status=0x0 [] partners[rq1] status=0x1f []
engine NVENC0 type=0x1b fault_info_status=0x0 mmuFaultId=11 subctx=0 partners[rq0] status=0x0 [] partners[rq1] status=0x1f []
engine SW type=0x22 fault_info_status=0x0 mmuFaultId=4294967295 subctx=0 partners[rq0] status=0x0 [] partners[rq1] status=0x1f []
probe NVDEC1 type=0x14 fault_info_status=0x0 mmuFaultId=25 partners[rq0] status=0x0 n=0 partners[rq1] status=0x1f n=0
probe NVDEC2 type=0x15 fault_info_status=0x0 mmuFaultId=26 partners[rq0] status=0x0 n=0 partners[rq1] status=0x1f n=0
probe SEC2 type=0x26 fault_info_status=0x0 mmuFaultId=14 partners[rq0] status=0x0 n=0 partners[rq1] status=0x1f n=0
probe type0x40 type=0x40 fault_info_status=0x0 mmuFaultId=15 partners[rq0] status=0x0 n=1 partners[rq1] status=0x1f n=0
probe type0x41 type=0x41 fault_info_status=0x0 mmuFaultId=16 partners[rq0] status=0x0 n=1 partners[rq1] status=0x1f n=0
probe type0x42 type=0x42 fault_info_status=0x0 mmuFaultId=17 partners[rq0] status=0x0 n=0 partners[rq1] status=0x1f n=0
probe type0x43 type=0x43 fault_info_status=0x0 mmuFaultId=18 partners[rq0] status=0x0 n=0 partners[rq1] status=0x1f n=0
probe type0x44 type=0x44 fault_info_status=0x0 mmuFaultId=19 partners[rq0] status=0x0 n=0 partners[rq1] status=0x1f n=0
CE_GET_ALL_CAPS status=0x0 present=0x1f
ce 0 caps0=0xe3 caps1=0x03
ce 1 caps0=0xe3 caps1=0x03
ce 2 caps0=0xe0 caps1=0x03
ce 3 caps0=0xe0 caps1=0x03
ce 4 caps0=0xe0 caps1=0x03
FIFO_GET_INFO status=0x0 index1=4096 index2=128 index3=64 index5=1024 index7=0 index10=64
GPU_GET_CONSTRUCTED_FALCON_INFO status=0x0 num=7
falcon 0 engDesc=0x8a20bf00 ctxAttr=0x2 ctxBufferSize=0x1000 addrSpaceList=0x3 registerBase=0x9a4000
falcon 1 engDesc=0x5ee8dc00 ctxAttr=0x2 ctxBufferSize=0x1000 addrSpaceList=0x3 registerBase=0x409000
falcon 2 engDesc=0x4781e800 ctxAttr=0x2 ctxBufferSize=0x1000 addrSpaceList=0x3 registerBase=0x41a000
falcon 3 engDesc=0x8f99e100 ctxAttr=0x2 ctxBufferSize=0x1000 addrSpaceList=0x3 registerBase=0x830000
falcon 4 engDesc=0xf3d72200 ctxAttr=0x1 ctxBufferSize=0x1000 addrSpaceList=0x3 registerBase=0x10a000
falcon 5 engDesc=0xe97b6c00 ctxAttr=0x2 ctxBufferSize=0x1000 addrSpaceList=0x3 registerBase=0x1c8000
falcon 6 engDesc=0x28c40800 ctxAttr=0x2 ctxBufferSize=0x10000 addrSpaceList=0x3 registerBase=0x840000
GPU_GET_HW_ENGINE_ID status=0x56
GPU_GET_ENGINE_RUNLIST_PRI_BASE status=0x1b
FIFO_GET_DEVICE_INFO_TABLE status=0x1b numEntries=0
CE_GET_FAULT_METHOD_BUFFER_SIZE status=0x1b size=0x0
```

Status codes (sdk/nvstatuscodes.h:56, 60, 115, 116):
- 0x1b = `NV_ERR_INSUFFICIENT_PERMISSIONS`
- 0x1f = `NV_ERR_INVALID_ARGUMENT`
- 0x56 = `NV_ERR_NOT_SUPPORTED`
- 0x57 = `NV_ERR_OBJECT_NOT_FOUND`

The probes that returned 0x57 are not printed. They covered every other NV2080 engine type up to
0x4f: COPY5-COPY9, NVENC1/2, NVJPG, OFA, HOST, GR1-GR7 and others.

### 5.1 How to read the output

The host is itself a GSP client. Its kernel RM holds the table that its GSP returned for
0x20801112, unmodified (kernel_fifo.c:2096-2165 with `IS_VIRTUAL` false). The public controls are
answered from that copy:

1. **Membership.** `GPU_GET_ENGINE_FAULT_INFO` translates `RM_ENGINE_TYPE → MMU_FAULT_ID`
   through the host's table (subdevice_ctrl_gpu_kernel.c:1805-1807).
   - NV_OK proves that the engine has an entry, even when it has no client class.
   - `GPU_GET_ENGINES_V2` lists only engines with classes (`gpuUpdateEngineTable_IMPL`,
     src/kernel/gpu/gpu.c:5187-5228), which is why SEC2, NVDEC1 and NVDEC2 are missing there.
   - The host table therefore has exactly **GR0, CE0-CE4, NVDEC0-NVDEC2, NVENC0, SEC2, SW**:
     12 entries.
   - The 0x40-0x44 hits are `COMP_DECOMP_COPY0-4`, which map onto `RM_ENGINE_TYPE_COPY0-4`
     (src/kernel/gpu/gpu_engine_type.c:125 and following).
   - The table also proves that `engineData[RM_ENGINE_TYPE]` holds RM values. SEC2 was found
     under `RM_ENGINE_TYPE_SEC2` = 0x31, not under the NV2080 value 0x26.
2. **Fault ids.** They match published/turing/tu102/dev_fault.h and published/volta/gv100/dev_fault.h
   exactly. SW reports 0xFFFFFFFF.
3. **PBDMA count per entry.** `GPU_GET_ENGINE_PARTNERLIST` with runqueue r returns
   `NV_ERR_INVALID_ARGUMENT` when `r ≥ numPbdmas` (kernel_fifo_gm107.c:977-980).
   - GR0 accepts runqueue 1, so it has 2 PBDMAs.
   - Every other entry, SW included, rejects runqueue 1 and accepts 0, so each has exactly 1.
   - The partnership class must be a FIFO class (subdevice_ctrl_gpu_kernel.c:1668-1683); the
     tool passes TURING_CHANNEL_GPFIFO_A 0xC46F (sdk/class/clc46f.h:43).
4. **Runlist and PBDMA sharing.** A partner is an engine with a class that has the same runlist
   **and** the same PBDMA as the source's runqueue-r PBDMA (kernel_fifo_gm107.c:983-1030).
   - GR0 rq0 ↔ CE0 and GR0 rq1 ↔ CE1. So CE0 and CE1 are on GR's runlist, CE0 on GR's first
     PBDMA and CE1 on GR's second.
   - The CE caps agree: bit 0 `CE_GRCE` and bit 1 `CE_SHARED` are set for CE0/CE1 only
     (caps0 0xe3 against 0xe0; sdk/ctrl/ctrl2080/ctrl2080ce.h:91-92).
   - The others have no partner. SW's (runlist, PBDMA) therefore equals no classed engine's,
     so SW is not (runlist 0, PBDMA 0 or 1).
5. **FIFO_GET_INFO:**
   - 4096 channel groups
   - 128 channels per group
   - **64 subcontexts per group** (the GR fault range is 64..127)
   - timeslice 1024
   - no per-runlist CHRAM
   - max lower subcontext 64
6. **PCE masks.** The five LCEs use PCEs 0-3 (union 0xF), so TU106 has 4 PCEs.
7. **Constructed falcons** (identified by class id in g_eng_desc_nvoc.h):
   - `OBJFBFLCN` (:161)
   - `FECS` (:785)
   - `GPCCS` (:793)
   - `OBJBSP` inst 0 = NVDEC0 (:537)
   - `Pmu` (:609)
   - `OBJMSENC` inst 0 = NVENC0 (:817)
   - `OBJSEC2` (:689)

   Only one NVDEC has a falcon. NVDEC1 and NVDEC2 exist only as FIFO table entries. With no
   falcon and no class, the guest never uses them.

### 5.2 Controls and their routing

Flags are from the exported-method tables in g_subdevice_nvoc.c. The bits are defined in
inc/kernel/rmapi/control.h:167-344:
- 0x4 PRIVILEGED
- 0x8 NON_PRIVILEGED
- 0x40 ROUTE_TO_PHYSICAL
- 0x80 INTERNAL
- no privilege bit = kernel-privileged (control.h:169-175)

| Control | Id (sdk/ctrl/ctrl2080) | Flags (g_subdevice_nvoc.c) | Unprivileged | Supplies |
|---|---|---|---|---|
| GPU_GET_ENGINES_V2 | 0x20800170 (ctrl2080gpu.h:778) | 0x10109 (:4719) | yes | engines with classes |
| GPU_GET_ENGINE_FAULT_INFO | 0x20800125 (ctrl2080gpu.h:855) | 0x9 (:4134) | yes | MMU_FAULT_ID, table membership, subcontext support |
| GPU_GET_ENGINE_PARTNERLIST | 0x20800147 (ctrl2080gpu.h:1691) | 0x118 (:4389) | yes | numPbdmas, runlist and PBDMA sharing (not the ids) |
| FIFO_GET_INFO | 0x20801109 (ctrl2080fifo.h:178) | 0x30008 (:8919) | yes | max subcontexts, channel counts |
| CE_GET_ALL_CAPS | 0x20802a0a (ctrl2080ce.h:333) | 0x10108 (:11799) | yes | CE present mask, GRCE flag |
| CE_GET_CE_PCE_MASK | 0x20802a02 (ctrl2080ce.h:171) | 0x30349 (:11694) | yes, routed to the host GSP | PCE count |
| GPU_GET_CONSTRUCTED_FALCON_INFO | 0x208001b0 (ctrl2080gpu.h:4463) | 0x10048 (:5289) | yes, routed | falcon list (§8.2) |
| GPU_GET_HW_ENGINE_ID | 0x2080017a (ctrl2080gpu.h:2969) | 0x8 (:4809) | yes, but `NV_ERR_NOT_SUPPORTED` on Turing | nothing (FIFO_TAG) |
| GPU_GET_ENGINE_RUNLIST_PRI_BASE | 0x20800179 (ctrl2080gpu.h:2932) | 0x4 (:4794) | **no** (0x1b) | would give runlist ids |
| FIFO_GET_CHANNEL_MEM_INFO | 0x2080110c (ctrl2080fifo.h:339) | 0x10004 (:8949) | **no** (PRIVILEGED; not called) | not needed |
| FIFO_GET_DEVICE_INFO_TABLE | 0x20801112 (ctrl2080fifo.h:535) | 0x5c040 (:9009) | **no** (kernel-privileged; 0x1b) | the whole table |
| CE_GET_FAULT_METHOD_BUFFER_SIZE | 0x20802a08 (ctrl2080ce.h:295) | 0x1c040 (:11769) | **no** (kernel-privileged; 0x1b) | §8.1 |

The unprivileged client therefore has no way to observe the following. They are synthesized
under the constraints above:
- runlist ids
- PBDMA ids
- PBDMA fault ids
- FIFO_TAG
- reset, interrupt and device-type fields
- engine names

`GPU_GET_ENGINE_RUNLIST_PRI_BASE` as root would return runlist ids, but PRIVILEGED means admin
(control.h:195-199), and running the device as root is outside decision D10.

## 6. The TU106 table

Column provenance:
- **H**: host (§5).
- **T**: tree constant (§3).
- **S**: synthesized.

Runlist ids and PBDMA ids are S but respect the host's sharing structure. PBDMA fault ids are
`32 + pbdmaId` (S). The value 32 is chosen as the first free id above the TU10x engine ids
(dev_fault.h tops out at `PHYSICAL` = 31, published/turing/tu102/dev_fault.h:53) and below the GR
range 64..127. It is also the Volta/Turing `HOST0` value outside this tree (see Open).

| # | Name | ENG_DESC (T) | RM type (T) | RUNLIST (S) | MMU fault (H) | MC (T) | host-driven | PBDMA ids (S; count H) | PBDMA fault ids (S) | INSTANCE (S) | DEV_TYPE (S) | FIFO_TAG (S) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | GR0 | 0xd334df00 | 0x01 | 0 | 64 | 84 | 1 | 0, 1 | 32, 33 | 0 | 0x00 | 0 |
| 1 | CE0 | 0x793ceb00 | 0x09 | 0 (H: shares GR) | 15 | 15 | 1 | 0 (H: GR rq0) | 32 | 0 | 0x13 | 1 |
| 2 | CE1 | 0x793ceb01 | 0x0a | 0 (H: shares GR) | 16 | 16 | 1 | 1 (H: GR rq1) | 33 | 1 | 0x13 | 2 |
| 3 | CE2 | 0x793ceb02 | 0x0b | 1 | 17 | 17 | 1 | 2 | 34 | 2 | 0x13 | 3 |
| 4 | CE3 | 0x793ceb03 | 0x0c | 2 | 18 | 18 | 1 | 3 | 35 | 3 | 0x13 | 4 |
| 5 | CE4 | 0x793ceb04 | 0x0d | 3 | 19 | 19 | 1 | 4 | 36 | 4 | 0x13 | 5 |
| 6 | NVDEC0 | 0x8f99e100 | 0x1d | 4 | 10 | 65 | 1 | 5 | 37 | 0 | 0x10 | 6 |
| 7 | NVDEC1 | 0x8f99e101 | 0x1e | 5 | 25 | 66 | 1 | 6 | 38 | 1 | 0x10 | 7 |
| 8 | NVDEC2 | 0x8f99e102 | 0x1f | 6 | 26 | 67 | 1 | 7 | 39 | 2 | 0x10 | 8 |
| 9 | NVENC0 | 0xe97b6c00 | 0x25 | 7 | 11 | 38 | 1 | 8 | 40 | 0 | 0x0e | 9 |
| 10 | SEC2 | 0x28c40800 | 0x31 | 8 | 14 | 47 | 1 | 9 | 41 | 0 | 0x0d | 10 |
| 11 | SW | 0x95a6f500 | 0x2d | 0 | 0xFFFFFFFF | 0 | 0 (T) | none | none | 0 | 0 | 0 |

Notes on individual fields:
- **host-driven.** 1 for every entry except SW. CE entries need it (§4.3). The value for GR,
  NVDEC, NVENC and SEC2 follows from each owning a PBDMA on the host. SW is 0 because
  `numRunlists` excludes it (g_kernel_fifo_nvoc.h:309-314).
- **All other fields** are 0: RC_MASK, RESET, INTR, RUNLIST_PRI_BASE, RUNLIST_ENGINE_ID,
  CHRAM_PRI_BASE and `engineData[15]` (§4.6).
- **engineName** is the Name column. It is only printed (kernel_fifo_gm107.c:835-859).
- **DEV_TYPE.** Only LCE = 19 is in the tree (published/ampere/ga100/dev_top.h:34). GRAPHICS 0,
  NVDEC 0x10, NVENC 0x0e and SEC 0x0d are nouveau's `top` enumeration (outside the tree). The
  field is unread (§4.6).
- **SW PBDMA.** The host has one PBDMA on SW (§5.1). Its id is unobservable, and its
  (runlist, PBDMA) pair matches no classed engine. The table gives SW `numPbdmas` = 0. This has
  two effects:
  - SW adds nothing to `maxPbdmaId`, and SW PBDMA fault ids are ignored anyway (kernel_fifo.c:2145-2152).
  - Only `GPU_GET_ENGINE_PARTNERLIST(SW, rq0)` differs from the host. It returns
    `NV_ERR_INVALID_ARGUMENT` instead of an empty list.

  Setting 1 PBDMA with id 0 would instead make GR a partner of SW, which the host rules out.
- **Resulting kernel state:**
  - `engineInfoListSize` 12
  - `kfifoGetNumEngines` 11
  - `numRunlists` 11
  - `maxNumRunlists` 9
  - `maxNumPbdmas` 10
  - the lowest PBDMA fault id is 32, so `pbdmaId = faultId - 32`

The JSON file carries each entry as the 16-word `engineData` array in wire order, plus
`pbdmaIds`, `pbdmaFaultIds`, `numPbdmas` and `engineName`.

**Alternative.** Dropping NVDEC1 and NVDEC2 (10 entries) is also safe for the kernel: no
falcon, class or presence check refers to them. They are kept so that the guest's table equals
the host's.

**Consistency with other answers:**
- `GspStaticConfigInfo.engineCaps` decides NVDEC, NVENC and SEC2 presence (gpu.c:6913-6921). It
  should list what the host's `GPU_GET_ENGINES_V2` lists plus SEC2, and must not add NVDEC1/2 or
  NVENC1.
- The interrupt kernel table (row after this one, 0x20800a5c) should carry the nonstall vectors
  for MC 65 (NVDEC0) and 38 (NVENC0). Those engines now register notification services (§4.4).

## 7. Differences from the provisional table in crates/vfio-user-nvgpu/src/ctrl.rs

The crate currently answers 0x20801112 with an 8-entry provisional table. It must change as follows:

| Field | Provisional | Required | Why |
|---|---|---|---|
| MC | `1 << (12 + i)` | `MC_ENGINE_IDX_*` (84, 15-19, 65-67, 38, 47, 0) | used as `pRecords[mcIdx]` index with bound 181 (kernel_falcon.c:377-398; engine_idx.h:169). 1<<18 and 1<<17 for NVDEC0/NVENC0 are out-of-bounds writes in the guest kernel |
| MMU fault ids | GR 0, CE 0x15-0x17, NVDEC 0x10, NVENC 0x11, SEC2 0x12 | GR 64, CE 15-19, NVDEC 10/25/26, NVENC 11, SEC2 14 | GR 0 makes fault ids 0..63 resolve to GR (kernel_fifo_gv100.c:112-125) |
| PBDMA fault ids | 0x1C-0x22 | 32-41, contiguous with PBDMA id | 28-31 collide with engine ids and fall inside the GR range when GR is 0 |
| GR PBDMAs | 1 | 2 | runqueue 1 (async compute, SCG COMPUTE1) needs a second PBDMA (kernel_fifo_gm107.c:977-981; kernel_fifo_gp102.c:52-58) |
| CE0/CE1 runlist | own runlists 1, 2 | GR's runlist 0, GR's PBDMAs | host says GRCE (§5.1); `_kfifoIsValidCETag_GP102` classifies by sharing |
| CE count | 3 | 5 | host has CE0-CE4; CE3/CE4 would be removed |
| NVDEC1/2 | absent | present | host table (optional, §6) |
| RESET, INTR, RC_MASK | per-entry bits | 0 | unread (§4.6); zero avoids suggesting meaning |

## 8. Follow-ups from the same run

### 8.1 `NV2080_CTRL_CMD_CE_GET_FAULT_METHOD_BUFFER_SIZE` (0x20802a08)

**Parameters.** `NV2080_CTRL_CE_GET_FAULT_METHOD_BUFFER_SIZE_PARAMS` is `{ NvU32 size; }`, 4
bytes (sdk/ctrl/ctrl2080/ctrl2080ce.h:289-295). The size is out.

**Routing.** Flags 0x1c040 (g_subdevice_nvoc.c:11769):
- ROUTE_TO_PHYSICAL
- GSP_PLUGIN_FOR_VGPU_GSP
- CACHEABLE_BY_INPUT
- PHYSICAL_IMPLEMENTED_ON_VGPU_GUEST
- kernel-privileged

The physical implementation `subdeviceCtrlCmdCeGetFaultMethodBufferSize_IMPL` is only declared
in the tree (g_subdevice_nvoc.h:4889).

**Caller chain in E4:**
1. `memmgrCalcReservedFbSpaceHal_GM107` (mem_mgr_gm107.c:1990-1996, guarded by
   `RMCFG_FEATURE_PLATFORM_GSP` being false, which holds in the kernel build).
2. `kfifoCalcTotalSizeOfFaultMethodBuffers_GV100` (arch/volta/kernel_fifo_gv100.c:293-325; call at :313).
3. `gpuGetCeFaultMethodBufferSize_KERNEL` (gpu.c:6393-6405).
4. `kceGetFaultMethodBufferSize_IMPL` (kernel_ce.c:841-862). It sends the control on the
   internal subdevice at kernel_ce.c:857-858. This is the logged assertion.

Other callers of the same function:
- `kbusCalcCpuInvisibleBar2ApertureSize_GV100` (src/kernel/gpu/bus/arch/volta/kern_bus_gv100.c:454)
- `kchangrpAllocFaultMethodBuffers_GV100` (src/kernel/gpu/fifo/arch/volta/kernel_channel_group_gv100.c:77-78)

**Tolerance.** An error is tolerated during init and fatal later:
- `gpuGetCeFaultMethodBufferSize_KERNEL` returns NV_OK even when the control fails and leaves
  `*size` untouched (gpu.c:6395-6404).
- The FB-reservation and BAR2 callers start from 0, so they continue with a 0-byte method
  buffer term.
- `pGpu->ceFaultMethodBufferSize` is never assigned anywhere in the tree (only read at gpu.c:6395).
  The control is therefore re-sent on each call, unless the RPC cache keeps a successful answer
  (CACHEABLE_BY_INPUT, see phase5-rpcs.md §0).
- At the first channel group:
  - `kchangrpAllocFaultMethodBuffers_GV100` asserts `bufSizeInBytes > 0`
    (kernel_channel_group_gv100.c:78).
  - It then creates a 0-byte memdesc per runqueue (:106-112).
  - The kernel RM itself allocates these buffers on a GSP client ("For GSP config, method buffer
    allocation is done by CPU-RM", :58-68).

So an error is not acceptable once channels are created.

**Value.** The device must return a nonzero, page-aligned size with status NV_OK. The value is
not derivable from the tree, and the host refuses the control to an unprivileged client (0x1b, §5).
- The vGPU path carries the same number as `ceFaultMethodBufferDepth`
  (inc/kernel/vgpu/sdk-structures.h:442; used at kernel_ce.c:846-853).
- Proposal: **0x8000** (32 KiB). This follows the nvgpu/nouveau formula for Volta+
  (27 × 5 × ((9 + 1 + 3) × nPCE + 2) method entries), with nPCE = 4 from the host's PCE masks
  (§5.1 item 6). That gives 7290 entries. At 4 bytes each this is 29160 bytes, rounded up to
  32 KiB. The formula is outside the tree (see Open).
- Because the buffer lives in guest sysmem and is only written by hardware on a CE fault, any
  nonzero page multiple satisfies the kernel.
- Resulting FB reservation: with 0x8000 the term at mem_mgr_gm107.c:1992 becomes
  0x8000 × `kfifoGetMaxCeChannelGroups` × 2 runqueues (kernel_fifo_gv100.c:310-315). It is zeroed
  unless the FAULT_METHOD_BUFFER instance-location regkey selects vidmem (:317-322).

### 8.2 `status == NV_OK @ kernel_falcon.c:391`

This is `gkflcnRegisterIntrService_IMPL`, not a register read.

**Sequence:**
1. `gpuStatePreInit` builds the generic falcon list from `GPU_GET_CONSTRUCTED_FALCON_INFO`
   (gpu.c:2184 → 5706-5784). This is the forwarded 0x208001b0 at iter1-boot-events.txt:40.
2. It builds the video engine list from a second copy of the same control (gpu.c:2186 → 5797-5865).
   This is the second 0x208001b0 at :44.
3. It runs `gpuRemoveMissingEngines` (gpu.c:2200).
4. Intr's state pre-init registers interrupt services (intr.c:819-820).
5. For each GenericKernelFalcon that is a video engine, it translates ENG_DESC → MC
   (kernel_falcon.c:377-379).
6. With the empty table that lookup failed. `IS_VIRTUAL` is false, so
   `NV_ASSERT_OR_RETURN_VOID(status == NV_OK)` fired at :391.

**What was in the host's list and what the guest did with it** (§5):

| Falcon | Guest treatment |
|---|---|
| FBFLCN, FECS, GPCCS, PMU | became GenericKernelFalcons, returned early at :369-370 (not video, not SEC2) |
| SEC2 | skipped at gpu.c:5746-5754 because KernelSec2 exists (TU106 child list, kern_gpu_tu102.c:204) |
| NVDEC0 (`0x8f99e100`), NVENC0 (`0xe97b6c00`) | the two lookups that asserted (E4 log: two assertions at :391) |

**Remedy.** The list is acceptable as forwarded. No filtering is needed once the FIFO table has:
- `ENG_DESC = 0x8f99e100` with MC 65 (`MC_ENGINE_IDX_NVDEC0`)
- `ENG_DESC = 0xe97b6c00` with MC 38 (`MC_ENGINE_IDX_NVENC`)
- MC values that differ from every other registered service. Otherwise `NV_ASSERT` at
  kernel_falcon.c:396 fires. GR uses 84 and the FECS-log index; CE uses 15-19; SEC2 uses 47.

The table in §6 satisfies this.

**If the device ever forwards a list with a video falcon that has no table entry**, it must
either:
- add the entry, or
- drop that falcon from the forwarded list, which also removes its KernelVideoEngine
  (gpu.c:5836-5846).

**Other falcon-list content the kernel keeps:**
- `ctxBufferSize` for NVDEC0/NVENC0 is 0x1000 and for SEC2 0x10000. These are used for engine
  context allocation at channel creation (kernel_falcon.c:206-214 uses the table again, ENG_DESC
  → RM_ENGINE_TYPE).
- `registerBase` values: 0x830000 for NVDEC0, 0x1c8000 for NVENC0, 0x409000/0x41a000 for
  FECS/GPCCS, 0x10a000 for PMU and 0x9a4000 for FBFLCN. They point into the synthetic BAR0.
  Whether any kernel path reads them for generic falcons on a GSP client is open.

## 9. Open

1. **Real runlist ids, PBDMA ids and PBDMA fault ids of TU106.** They are not in the tree.
   `published/turing/tu102/` has no `dev_top.h` or `dev_fifo.h`. The unprivileged controls
   cannot see them, and `GPU_GET_ENGINE_RUNLIST_PRI_BASE` and the table itself are refused.
   - The synthesized values satisfy every kernel constraint found in §4.
   - The HOST0 = 32 base is from nouveau's Volta fault enumeration, not from this tree.
   - To get real values: run `GPU_GET_ENGINE_RUNLIST_PRI_BASE` (0x20800179, PRIVILEGED) as
     root on the host once, which gives runlist ids per engine. PBDMA ids stay unavailable
     without register access to PFIFO, which is off limits.
2. **SW entry.** The host's SW entry has one PBDMA with an unknown id and an unknown runlist
   (§5.1). The table uses `numPbdmas` = 0 and runlist 0. Whether the physical RM gives SW a
   runlist larger than 8, which would change `maxNumRunlists` on the host, is unknown.
3. **CE fault method buffer size.** 0x8000 is a proposal from an out-of-tree formula. The true
   GSP answer needs a kernel-privileged client on the host, for example a small kernel module,
   which is outside the current rules.
4. **DEV_TYPE_ENUM, FIFO_TAG, RESET, INTR, RC_MASK** carry synthesized values. No Turing
   kernel-RM reader was found (§4.6). If one appears on a later path (for example RC recovery
   through GSP events), revisit.
5. **NVDEC1/NVDEC2.** Why the host's physical table carries two NVDEC entries without falcons
   (fault ids 25/26) is not explained by the tree. The host's own `GPU_GET_ENGINES_V2` hides
   them. Keeping or dropping them is a choice (§6); the kernel is indifferent.
6. **Next RPCs.** With GR0 and CE0-CE4 present, KernelGraphics and KernelCE run their full
   pre-init and init. Expect new internal controls, for example GR static info and CE
   PCE/LCE configuration. They are not covered here and should be taken from the next run's
   trace.
7. **engineCaps and interrupt table.** These must be made consistent with this table (§6). That
   is checked in their own rows, not here.
