<!-- SPDX-License-Identifier: MIT -->
<!-- Lookahead written 2026-09-23 against vendor/open-gpu-kernel-modules 610.57.04, the native
     610.57.04 CUDA traces in matrix/traces/610.57.04/ and the Caraxes working tree after E5b
     part 1. Research only: nothing here was run in a VM, nothing touched the host GPU. -->
# CUDA lookahead: everything the CUDA path sends to the device

This file is a checklist for the device work on `crates/vfio-user-nvgpu` against the `cuda`
gate ([testing.md](testing.md)). Each gate run on the device shows only the first control it
cannot answer. This file lists everything that will reach the device, phase by phase, so that
one VM cycle can add many rows at once.

For each item it gives:
- the RPC or control id and its name, the class for allocations;
- the parameter fields that matter;
- a classification from A to E;
- the evidence: a native trace line or a source line.

Sections:
1. conventions, sources, classification;
2. the state of the device today;
3. what libcuda maps, and the BAR accesses that follow;
4. the phases, each with an ordered table;
5. bookkeeping the device needs beyond answering RPCs;
6. the three minimal sets;
7. guesses and open points.

## 1. Conventions, sources, classification

**Classification.**

| Class | Meaning |
|---|---|
| **A** | Already answered by a dispatcher row. The row is named, as `file` + match arm or list entry. |
| **B** | Forwardable unchanged to the host RM as an unprivileged client (the public control on the host object of the same class). |
| **C** | Forwardable with translation. The field to translate is named: a handle, an FB address, a sysmem IOVA, or the command id. |
| **D** | Must be synthesized or derived from guest state. The source is named. |
| **E** | Depends on execution: it needs the mirror channel or physical execution (part 2). Where the software channel emulator can stand in, the entry says **E (emulated)**. |

"Kernel" in the *Reaches device* column means that the guest kernel RM serves the call
itself from its static info or its own objects, so nothing reaches the device. Those rows are
kept so that the order of the phase stays readable.

**Evidence notation.**
- `T:n` is line n of `matrix/traces/610.57.04/cuda-core.tsv`: a native run of `nvprobe 2` on the
  host RTX 2070, driver 610.57.04, tracked in git. `TL:n` is `cuda-launch.tsv` (`nvprobe 4`, with
  teardown), `TT:n` is `cuda-torch.tsv`, `TH:n` is `cuda-hostreg.tsv`, `TM:n` is
  `cuda-managed.tsv`, all in the same directory.
  - Columns (writer: `crates/nvrm-trace`):
    - `ioctl dev nr sub size psize ret status fd`: for `0x2a` the sub field is the control
      command, for `0x2b` it is the class.
    - `nvos64`: alloc handles.
    - `memparams`: `NV_MEMORY_ALLOCATION_PARAMS`.
    - `nvos02`: memory allocation.
    - `nvos33`: map memory.
    - `ctrloutin`/`ctrlout` and `allocoutin`/`allocout`: the parameter bytes before and after
      the call.
    - `mmap`: device, fd, length, offset, address.
  - Condensed with a throw-away script (not checked in). The script names controls from the SDK
    headers and flags from the `/*flags=*/` fields of `src/nvidia/generated/g_*_nvoc.c`, the
    ctrl_table.tsv method of [phase5-rpcs.md](phase5-rpcs.md) §0.
- `P:lvlN` is `probe/traces/lvlN.tsv`, the staged `nvprobe N` traces. They are git-ignored and
  carry no driver version, so they are used only for the phase boundaries. The boundaries agree
  with the matrix traces.
- Paths: `K/` is `vendor/open-gpu-kernel-modules/src/nvidia/src/kernel/`, `G/` is
  `.../src/nvidia/generated/`, `SDK/` is `.../src/common/sdk/nvidia/inc/`, `U/` is
  `.../kernel-open/nvidia-uvm/`, `D/` is `crates/vfio-user-nvgpu/src/`.
- Flag bits (`src/nvidia/inc/kernel/rmapi/control.h`, table in phase5-rpcs.md §0): 0x4 PRIVILEGED,
  0x8 NON_PRIVILEGED, 0x40 ROUTE_TO_PHYSICAL, 0x80 INTERNAL, 0x400 CACHEABLE. Neither 0x4 nor
  0x8 set means kernel-privileged.

**How a guest call reaches the device.** The four routes are in phase5-rpcs.md §0. Three more
matter for CUDA:
- **Kernel handlers that issue a physical-RMAPI control themselves.** Examples:
  CE_GET_ALL_CAPS, GPU_GET_SKYLINE_INFO, PERF_BOOST, PERF_CUDA_LIMIT_SET_CONTROL, and the GR
  object constructor (PROMOTE_CTX).
- **Kernel handlers that re-send the user's control** with `NV_RM_RPC_CONTROL` when
  `IS_GSP_CLIENT` holds: GPFIFO_SCHEDULE, SET_TIMESLICE and STOP_CHANNEL. On a GSP client this
  macro is a physical-RMAPI control (`src/nvidia/inc/kernel/vgpu/rpc.h:219-231`).
- **GSS legacy.** Any command with bit 0x8000 goes out opaquely with the user's handles
  (`interface/deprecated/rmapi_deprecated_control.c:134-138`).

Allocations:
- A class whose `RS_FLAGS_ALLOC_RPC_TO_*` is set is mirrored after its constructor
  (`K/rmapi/alloc_free.c:878-938`).
- Some constructors send their RPC explicitly: the device, the subdevice, the VAS, the TSG, the
  channel and the event.
- None of the memory classes (0x40, 0x3e, 0x71, 0x50a0) sends anything on a GSP client:
  - `K/mem_mgr/video_mem.c:988` (`if (!IS_GSP_CLIENT)`);
  - `K/mem_mgr/system_mem.c:434` and `K/mem_mgr/os_desc_mem.c:212` (`IS_VIRTUAL` only);
  - `K/mem_mgr/virtual_mem.c:461-473` (split VAS).

## 2. The device today (what "A" can point at)

- **`GSP_RM_ALLOC` (fn 103).** Every class is acknowledged with the guest's own parameters
  echoed back (`D/ctrl.rs` `Dispatcher::alloc`).
  - VAS, TSG and channel allocations are recorded for the channel emulator
    (`D/channel.rs` `Registry::record_alloc`).
  - No allocation ever fails. Every CUDA failure will therefore come from a control.
- **`FREE` (fn 10).** Always OK (`Dispatcher::free`).
- **`DUP_OBJECT` (fn 21).** Always OK, nothing recorded (`D/boot.rs` arm `FN_DUP_OBJECT`).
- **`GSP_RM_CONTROL` (fn 76).** Tried in this order:
  - `kgr.rs` (rows 35, 39-52);
  - `uvm.rs` (the E5a set: 0x20800a9b/9c/9d/9e, 0x20800a1d/1e, 0x20802a07, 0x20802a02,
    0x20802a12, 0x90f10106, 0x801813/14, 0xa06f0103, 0xa06c0101, 0xa06c010a);
  - `nvml.rs`:
    - the `FORWARD` and `LEGACY` lists of `D/ver/v610.rs`;
    - K1 (`GPU_GET_INFO_V2` marked indices), K2 (`INTERNAL_GPU_GET_SMC_MODE`), K5;
    - `FB_GET_INFO_V2`, `GPU_GET_CHIP_DETAILS`, BINAPI 0x20810108/0110/0111;
  - the `match` in `ctrl.rs` (the phase-5 rows).
  - Anything else gets `NV_ERR_NOT_SUPPORTED`, labelled `unanswered`.
- **Added to the tree after the 2026-09-23 gate run, not yet exercised:** 0x2080182a
  (`BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS`), 0x2080182b (`BUS_GET_C2C_INFO`) and 0x20803801
  (`GRMGR_GET_GR_FS_INFO`) in `FORWARD` (`D/ver/v610.rs`, the comment above the three rows).
  The three targets of testing.md §8 are therefore class A now. The first failure of the next
  run lies further down this file.
- **Deliberate zero.** `kgr.rs` answers row 41 (`STATIC_KGR_GET_FLOORSWEEPING_MASKS`) with all
  zeros, so that the golden channel is skipped (`kgr.rs` arm
  `CMD_INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS`). CUDA's `GR_GET_GPC_MASK` and
  `GR_GET_TPC_MASK` are served from exactly that field (`K/gpu/gr/kernel_graphics.c:3465-3499`).
  See §4.1 row I-1.
- **Guest configuration** (experiments.md, E5b part 1):
  - `RmDisableGlobalCeUtils=1;RMDisableScrubOnFree=1;RmRcWatchdog=0`;
  - `uvm_disable_hmm=1`. CUDA calls `UVM_INITIALIZE` with flags 0 (`T:60`
    `uvminit flags=0x0`), so this module option is required.

## 3. What libcuda maps, and which device accesses follow

From the `nvos33`/`mmap` rows of `T:` and the channel parameters (§4.3).

| Mapping | Object | Size, fd | What the CPU does there | What the device sees |
|---|---|---|---|---|
| Usermode page | `TURING_USERMODE_A` 0xC461 (`T:236`, map `T:246`, flags 0x3080002, fd `gpu`) | 64 KiB of BAR0 at 0xBB0000 (`K/gpu/fifo/usermode_api.c:40-104`, regBases[USERMODE] from row 2) | reads TIME_0/TIME_1 (+0x80/+0x84), writes the work-submit token to +0x90 | trapped BAR0 accesses. A: `uvm.rs` `VfRegs` (TIME), `boot.rs` `Reg::UsermodeDoorbell` → `channel.rs` `Registry::doorbell` |
| RUSD | `RM_USER_SHARED_DATA` 0xDE (`T:251`, map `T:261`, 0xc50 bytes, fd `ctl`) | a guest sysmem page; allocation params 8 zero bytes, so no data polling | reads | nothing. Real GSP would fill the page; the device leaves the page at row 12's PA zero (guess: CUDA tolerates zero, §7) |
| GPFIFO + USERD block | `NV01_MEMORY_LOCAL_USER` 0x40, 2 MiB, handle 0x5c000014 (`T:399`, map `T:410`, flags 0x10d0000, fd `gpu`: a **BAR1** mapping; CPU VA 0x200200000) | vidmem | writes GPFIFO entries and GPPut, reads GPGet, for all 20 CUDA channels | trapped **BAR1** writes and reads, through `fb.rs` BAR1 translation (`bar1PdeBase` root). A (the path UVM's vidmem GPFIFOs already use, uvm.md) |
| Pushbuffers | `NV01_MEMORY_SYSTEM` 0x3E, 56 MiB (`T:425`, map `T:436`, CPU VA 0x200400000) | guest sysmem | writes methods | only DMA reads by the channel emulator, or by the physical GPU (requirement S) |
| Error notifiers | 0x3E, 4 KiB, type 0xd NOTIFIER, one per channel (`T:448` onward, `hObjectError` of each channel) | guest sysmem | reads | written by the device on RC (cuda-path.md §5.3) |
| Staging and host buffers | 0x3E 2 MiB (`T:1461`, `T:1536`), 0x71 OS descriptors (`T:1563` onward) | guest sysmem | memcpy staging | DMA by CE: emulated through `guestmem.rs` `translate` (E5b part 1), physically through requirement S |
| UVM semaphore pool | `UVM_ALLOC_SEMAPHORE_POOL` (`T:1526`), mmap of the uvm fd (`T:1528`) | guest sysmem | tracking semaphores | DMA writes by semaphore releases |

**Correction to cuda-path.md §2.1/§2.2.**
- The 4 KiB sysmem object allocated before each channel is its **error notifier**
  (`hObjectError`), not its USERD.
- USERD is in vidmem: `hUserdMemory[0]` = 0x5c000014, `userdOffset[0]` = 0x2000 + 0x3000·i.
- The GPFIFO is in the same block: `gpFifoOffset` = 0x200200000 + 0x3000·i, 1024 entries.
- Evidence: the 20 `allocout 0xc46f` rows `T:465`-`T:1398`, decoded with
  `SDK/alloc/alloc_channel.h:296-341`.
- Consequence: CUDA submits work entirely through BAR1 (GPFIFO, GPPut) and BAR0 (doorbell).
  Under option C the USERD is therefore inside the VRAM reservation, not a sysmem page.

## 4. Phase tables

Handles in the parameter columns are from `T:`. Phase boundaries come from the staged traces:
- `P:lvl0` ends with the `NV50_MEMORY_VIRTUAL` alloc (cuInit);
- `P:lvl1` ends with `PERF_CUDA_LIMIT_SET_CONTROL` (context);
- `P:lvl2` adds one `NV01_MEMORY_LOCAL_USER` plus a UVM mapping (memory);
- `P:lvl3` adds no RM call before teardown (module load and launch);
- `P:lvl4` adds none either, only `poll` on the event fds.

### 4.0 Module load time (`modprobe nvidia`): consequences of fixing row 41

This is not CUDA's own traffic. It is needed because CUDA needs a non-zero GPC mask (row I-1).

| # | RPC | Params that matter | Class | Evidence |
|---|---|---|---|---|
| I-1 | `INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS` 0x20800a26 (row 41) | gpcMask, tpcMask[g], numPesPerGpc, zcullMask, physGpcMask | **C** today D-zero. Assemble it from the host's public GR controls as `D/kgr.rs:34-52` already specifies (`GR_GET_GPC_MASK` 0x2080122a, `GR_GET_TPC_MASK` 0x2080122b, `GR_GET_NUM_TPCS_FOR_GPC` 0x20801234, `GR_GET_ZCULL_MASK` 0x20801237, `GR_GET_GFX_GPC_AND_TPC_INFO` 0x20801239, `GPU_GET_PES_INFO` 0x20800168; physGpcMask = gpcMask). Translation: none, but it must be consistent with rows 40/42, which are host data already | `K/gpu/gr/kernel_graphics.c:487-489` (gpcMask 0 skips golden), `:3465-3499` (CUDA's GPC mask is this field) |
| I-2..I-9 | golden channel (phase5 rows 56-69): client triple, `FERMI_VASPACE_A` alloc, 0x90f10106, `TURING_CHANNEL_GPFIFO_A` alloc, **`GPU_PROMOTE_CTX` 0x2080012b**, `TURING_A` 0xC597 alloc, FREEs | PROMOTE_CTX: engineType, hChanClient/hObject, entryCount, promoteEntry[] {gpuPhysAddr, gpuVirtAddr, size, physAttr, bufferId, bInitialize, bNonmapped} (`SDK/ctrl/ctrl2080/ctrl2080gpu.h:893-971`) | allocs, frees and 0x90f10106: A. **PROMOTE_CTX: D.** Echo it and record the entries per (hChanClient, hObject). Flags 0x10244 are PRIVILEGED, so it cannot be forwarded to an unprivileged host client | phase5-rpcs.md §1.6; `G/g_subdevice_nvoc.c:4149`; `K/gpu/gr/kernel_graphics_object.c:133-158` (the reply's bInitialize is re-read) |
| I-10..I-18 | Bug 4208224 WAR (phase5 rows 70-78): the same eight, plus **`NV0080_CTRL_CMD_INTERNAL_KGR_INIT_BUG4208224_WAR` 0x802046** {bTeardown} | - | 0x802046: **D** (NV_OK). The rest as I-2..I-9 | `K/gpu/gr/arch/turing/kgraphics_tu102.c:236-275`; `G/g_device_nvoc.c:1385` (flags 0x1d0) |

No execution is involved: on a GSP client the kernel neither pushes to nor waits on the golden
or WAR channel (phase5-rpcs.md §1.6). This is an inference from the RPC list; a real run
confirms it.

### 4.1 cuInit (includes cuDeviceGet; nvprobe prints "stage 0 ok")

| # | Guest call (trace) | Reaches device as | Params that matter | Class | Evidence |
|---|---|---|---|---|---|
| 1 | NV01_ROOT_CLIENT 0x41 alloc, BUILD_VERSION 0x101, client free, a second client, SET_INHERITED_SHARE_POLICY 0xd04 ×2 | kernel | - | - | `T:20-42`; the root has no RPC flag (`K/rmapi/resource_list.h` NV01_ROOT_CLIENT) |
| 2 | `NV_CONFIDENTIAL_COMPUTE` 0xCB33 alloc; `SYSTEM_GET_FEATURES` 0x1f0; CC caps 0xcb330101 | kernel | - | - | `T:50-79`; `resource_list.h:2359` (no RPC flag) |
| 3 | UVM_INITIALIZE (flags 0), UVM_MM_INITIALIZE 0x4b, UVM_PAGEABLE_MEM_ACCESS 0x27 | nothing | - | guest config: `uvm_disable_hmm=1` | `T:58-67`; cuda-path.md §1.1 |
| 4 | NV0000 GPU id controls 0x214, 0xa04, 0x215, 0x288, 0x205, 0x201, 0x202; REGISTER_FD, NUMA_INFO | kernel | - | - | `T:69-104` |
| 5 | `NV01_DEVICE_0` 0x80 alloc | GSP_RM_ALLOC NV01_ROOT (the GSP-side client, created lazily), then GSP_RM_ALLOC NV01_DEVICE_0 | hClientShare | A (`Dispatcher::alloc`, synthesized) | `T:109`; `K/gpu/device.c:186` |
| 6 | HOST_GET_CAPS_V2 0x801402, GET_CLASSLIST_V2 0x800292, GET_NUM_SUBDEVICES 0x800280 | kernel | the class list must contain 0xC5C0, 0xC5B5, 0xC46F, 0xC461 (it follows from engineCaps and the CE class DB) | - | `T:112-118` |
| 7 | `NV20_SUBDEVICE_0` 0x2080 alloc | GSP_RM_ALLOC NV20_SUBDEVICE_0 | subDeviceId 0 | A | `T:136`; `K/gpu/subdevice/subdevice.c:124` |
| 8 | `NV2081_BINAPI` 0x2081 alloc | GSP_RM_ALLOC 0x2081 (generic, RPC_TO_PHYS_RM) | - | A | `T:141`; `resource_list.h:493` |
| 9 | FB_GET_INFO_V2 0x20801303 (5 calls, indices 0x8, 0x3b, 0x16/0x9/0x10, 0xb/0x19/0x1b/0x18/0xd…, 0x1a/0x22/0x23), GPU_GET_GID_INFO 0x2080014a, GET_VIRTUALIZATION_MODE 0x800289 | kernel. Indices the kernel cannot compute go out as FB_GET_INFO_V2 with the user's handles | - | A if any arrives (`nvml.rs` `fb_get_info_v2`: host answer, sizes rewritten to the profile) | `T:144-231`; `K/gpu/mem_sys/kern_mem_sys_ctrl.c:958-1000` |
| 10 | BINAPI 0x20810108 (992 bytes) | GSP_RM_CONTROL 0x20810108 | opaque | A (`nvml.rs`, BINAPI list) | `T:156`; `K/rmapi/binary_api.c:104-118` |
| 11 | MC_GET_ARCH_INFO 0x20801701, BUS_GET_PCI_INFO 0x20801801, BUS_GET_PCI_BAR_INFO 0x20801803 | kernel (cached) | - | - | `T:162-174` |
| 12 | BUS_GET_INFO_V2 0x20801823. First call: TYPE, CAPS, COHERENT_DMA_FLAGS. Second call: 6 indices including PCIE_GEN_INFO 0x2d | second call: GSP_RM_CONTROL 0x20801823 {PCIE_GEN_INFO} on the internal handles (K4) | - | A (`FORWARD` list) | `T:164`, `T:170`; `K/gpu/bus/kern_bus_ctrl.c:223-257` |
| 13 | **BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS 0x2080182a** (112 bytes, output only; native all zero) | GSP_RM_CONTROL, user handles (flags 0x40048) | none (plain data) | **A** (`FORWARD`, new, unexercised) | `T:177`; `G/g_subdevice_nvoc.c:10869` |
| 14 | GPU_QUERY_ECC_STATUS 0x2080012f | GSP_RM_CONTROL (0x50158) | - | A (`FORWARD`). The host answers 0x56, as natively (`T:180` st=0x56), and CUDA accepts it | `G/g_subdevice_nvoc.c:4194` |
| 15 | GPU_GET_ENGINES_V2 0x20800170, GET_SIMULATION_INFO 0x20800119, NV0000 GET_MEMOP_ENABLE 0x27b | kernel | engine list from the device-info table | - | `T:186-192` |
| 16 | GPU_GET_INFO_V2 0x20800102, 12 indices: 0x11, 0x22, 0x27, 0x2a, 0x37, 0x3b, 0x3c, 0x3d, 0x2d, 0x3a, 0x41, 0x44 | index 0x2a → `INTERNAL_GPU_GET_SMC_MODE` 0x20800a4c (internal handles). Index 0x11 (unnamed, default case) → GSP_RM_CONTROL 0x20800102 with bit 31 set on the index (user handles) | index list | A (`nvml.rs` K2 and K1 `get_info_v2`) | `T:194-196`; `K/gpu/subdevice/subdevice_ctrl_gpu_kernel.c:599-630` |
| 17 | GR_GET_INFO 0x20801201; GR_GET_GPC_MASK 0x2080122a; GR_GET_TPC_MASK 0x2080122b ×3 (natively gpcId 0..2); GR_GET_CAPS_V2 0x20801227 | kernel, from KGR static info (rows 39-41) | the GPC and TPC masks are row 41 | rows 39/40: A. **Row 41: C, see I-1.** With gpcMask 0, CUDA sees no GPC and makes no TPC query (guess: cuInit or cuCtxCreate fails on it) | `T:198-213`; `kernel_graphics.c:3465-3554` |
| 18 | **CE_GET_ALL_CAPS 0x20802a0a** (0x88 bytes) | **GSP_RM_CONTROL `CE_GET_ALL_PHYSICAL_CAPS` 0x20802a0b**, user client/subdevice (flags 0x101d0). The kernel then overwrites capsTbl for every non-stubbed KCE | `NV2080_CTRL_CE_GET_ALL_CAPS_PARAMS` {capsTbl[64][2], present}; the physical variant is a typedef of it | **C**: send the host's public `CE_GET_ALL_CAPS` 0x20802a0a (NON_PRIVILEGED 0x10108) with the same buffer; translation = the command id only. The `present` bits must match the CEs the guest's device-info table advertises (CE0-CE4, uvm.md). Failure is fatal to the control (`NV_ASSERT_OK_OR_RETURN`) | `T:216`; `K/gpu/ce/kernel_ce_shared.c:314-319`; `SDK/ctrl/ctrl2080/ctrl2080ce.h` (0x20802a0b typedef) |
| 19 | **GPU_GET_SKYLINE_INFO 0x2080019f** (688 bytes) | **GSP_RM_CONTROL `INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO` 0x20800aa2** on the internal handles, because TU106 has no MIG static info (`kmigmgrGetStaticInfo` is NULL without MIG init) | {skylineTable[], validEntries} | **D**: NV_OK, all zero (validEntries 0). This is exactly the native reply (`T:220` all zero). An error is returned to the caller (`NV_CHECK_OK_OR_RETURN`) | `K/gpu/subdevice/subdevice_ctrl_gpu_smc.c:59-70`; `K/gpu/mig_mgr/kernel_mig_manager.c:1392-1400`; `SDK/ctrl/ctrl2080/ctrl2080internal.h:2008-2023` |
| 20 | GR_GET_GLOBAL_SM_ORDER 0x2080121b | kernel (row 42) | - | - | `T:222` |
| 21 | **GRMGR_GET_GR_FS_INFO 0x20803801** (0x788 bytes) | GSP_RM_CONTROL, user handles (flags 0x10248) | queries in, answers out (plain data) | **A** (`FORWARD`, new, unexercised). The host's GPC data must agree with I-1 | `T:225`; `G/g_subdevice_nvoc.c:13194` |
| 22 | NV0080 FB_GET_CAPS_V2 0x801307 | kernel | - | - | `T:228` |
| 23 | `TURING_USERMODE_A` 0xC461 alloc; CLIENT_GET_ADDR_SPACE_TYPE 0xd01; map | nothing (no RPC flag); BAR0 mapping of 0xBB0000 | - | A (§3) | `T:236-247`; `resource_list.h:896` |
| 24 | `RM_USER_SHARED_DATA` 0xDE alloc {polledDataMask 0}; map | nothing (`SET_DATA_POLL` only when a poll mask is given) | - | - | `T:249-262`; `K/gpu/gpu_user_shared_data.c:480-531` |
| 25 | GSP_GET_FEATURES 0x20803601 | CACHEABLE and routed: sent once per adapter; RmGetFirmwareVersion at init already cached it | - | A (`ctrl.rs` arm `CMD_GSP_GET_FEATURES`) | `T:264`; `G/g_subdevice_nvoc.c:13104` |
| 26 | `FERMI_VASPACE_A` 0x90F1 ×2. 0x5c000007: flags 0x48 = IS_EXTERNALLY_OWNED\|ENABLE_PAGE_FAULTING, the CUDA VAS. 0x5c000008: flags 0 | GSP_RM_ALLOC 0x90F1 (explicit), then `COPY_SERVER_RESERVED_PDES` 0x90f10106 (levels[0].physAddress = root) | index, flags, bigPageSize; PDE levels | A (`channel.rs` `record_alloc`, `uvm.rs` → `record_server_reserved_pdes`) | `T:269`, `T:274`; `K/gpu/mem_mgr/vaspace_api.c:227-275`; `SDK/nvos.h:3158-3173` |
| 27 | GPU_GET_SHORT_NAME_STRING 0x20800111, GET_NAME_STRING 0x20800110, GID_INFO, QUERY_COMPUTE_MODE_RULES 0x20800131 | kernel (static info) | - | - | `T:277-286` |
| 28 | **0x20808159** (GSS legacy, NV2080 category 0x81 GPU_LEGACY_NON_PRIVILEGED, message 0x59; 332 bytes; in: {2, 0xc, 0, 0x25, …}) | GSP_RM_CONTROL 0x20808159, user handles, opaque | opaque | **B**: add `(0x2080_8159, 332, "gpu 0x59")` to `LEGACY` in `D/ver/v610.rs`. Native answer OK, bytes unchanged in the first 96 | `T:288-290`; `rmapi_deprecated_control.c:134-138`; `SDK/ctrl/ctrl2080/ctrl2080base.h` (0x81) |
| 29 | SYSTEM_GET_FABRIC_STATUS 0x136; RC_RELEASE_WATCHDOG_REQUESTS 0x2080220c; RC_SOFT_DISABLE_WATCHDOG 0x20802210; SYSTEM_GET_P2P_CAPS_MATRIX 0x13a | kernel (the RPC in the P2P matrix handler is `IS_VIRTUAL` only) | - | - | `T:292-297`; `K/gpu/rc/kernel_rc_watchdog_ctrl.c:84-132`; `K/rmapi/client_resource.c:4404-4536` |
| 30 | **0x20808162** (GSS legacy, category 0x81, message 0x62; 1 byte, in 0, out 1) | GSP_RM_CONTROL 0x20808162, opaque | opaque | **B**: add `(0x2080_8162, 1, "gpu 0x62")` to `LEGACY` | `T:299-301` |
| 31 | **BUS_GET_C2C_INFO 0x2080182b** (28 bytes, native all zero) | GSP_RM_CONTROL, user handles (0x50048) | - | **A** (`FORWARD`, new, unexercised) | `T:302-304`; `G/g_subdevice_nvoc.c:10884` |
| 32 | NVLINK_GET_NVLINK_STATUS 0x20803002 | kernel. KernelNvlink is removed (row 6) and TU106 has no MIG NVLink P2P, so it returns NV_OK with no links before any RPC | - | - | `T:306`; `K/gpu/nvlink/common_nvlinkapi.c:825-830`, `:850-855` |
| 33 | UVM_REGISTER_GPU 0x25 (rmCtrlFd -1, hClient 0) | the complete E5a set: 2 client triples, 0x2080012f, 0x20800a9b, 0x20800a9d, 0x20800a1d, 0x90F1 + 0x90f10106, 0x20802a12 ×10, 0x20802a07 ×5, 0x20802a02 ×5 (first time only), 0x20802a08, 3 TSGs, 6 channels + UVM_SW + CE, 0xa06f0103, 0xa06c010a, 0x801813; 7 pushes | see uvm.md | A (`uvm.rs`, `channel.rs`); the pushes are E (emulated) | `T:309-311`; uvm.md "RPCs, in order" |
| 34 | UVM_PAGEABLE_MEM_ACCESS_ON_GPU 0x46 | nothing | - | - | `T:313` |
| 35 | `NV50_MEMORY_VIRTUAL` 0x50A0, 0xfb000000 bytes at VA 0x5000000 in VAS 0x5c000008 | nothing (split VAS) | - | - | `T:320-322`; `virtual_mem.c:461-473` |

### 4.2 Device query

`nvprobe` stage 0 prints after `cuDeviceGet`. `P:lvl0` ends at row 35 above, so
`cuDeviceGet` and the attribute queries are answered from what cuInit collected. In the
traced programs no RM call belongs to device query alone.

### 4.3 cuCtxCreate: VA space, channels, engines (nvprobe "stage 1 ok")

Rows in `T:` order. "×n" repeats per channel or TSG.

| # | Guest call (trace) | Reaches device as | Params that matter | Class | Evidence |
|---|---|---|---|---|---|
| C1 | GSP_GET_FEATURES 0x20803601 | cached (see 25) | - | A | `T:324` |
| C2 | **PERF_BOOST 0x2080200a** {flags 0x12, duration 0xffffffff} | **GSP_RM_CONTROL `INTERNAL_PERF_BOOST_SET_2X` 0x20800a9a**, user client/subdevice | flags, duration | **D**: NV_OK, nothing boosted. B would forward the public `PERF_BOOST` to the host subdevice and boost the host card's clocks: possible, not recommended (D9) | `T:326-328`; `K/gpu/perf/kern_perf_boost.c:50`, `:100-122`; `G/g_subdevice_nvoc.c:7554` |
| C3 | GPU_ACQUIRE_COMPUTE_MODE_RESERVATION 0x20800145 | kernel | - | - | `T:329`; `subdevice_ctrl_gpu_kernel.c:2186-2252` |
| C4 | **UVM_REGISTER_GPU_VASPACE** 0x19 {uuid, rmCtrlFd 0xa, hClient 0xc1d53323, hVaSpace 0x5c000007} | (a) **DUP_OBJECT (fn 21)** of VAS 0x5c000007 into UVM's session client (NVOS55 {hClient, hParent, hObject, hClientSrc, hObjectSrc, flags}). (b) UVM page-tree init: one CE memset push on UVM's channel. (c) `DMA_SET_PAGE_DIRECTORY` 0x801813 on UVM's client/device with **hVASpace = the dup handle**, physAddress = UVM's new root | (a) all five handles; (c) physAddress, flags (aperture), hVASpace | (a) reply: A (`boot.rs` `FN_DUP_OBJECT`). **Bookkeeping D: record (hClient, hObject) → (hClientSrc, hObjectSrc) as a VAS alias** (§5 item 1). (b) E (emulated, as E5a). (c) A (`uvm.rs` → `record_set_page_directory`) **only with that alias**; without it the root lands on (UVM client, dup handle) and every CUDA channel keeps walking the kernel's old root | `T:331-332`; `U/uvm_va_space.c:1560`, `:1589`, `:1424`; `K/rmapi/nv_gpu_ops.c:2754`; `K/gpu/mem_mgr/vaspace_api.c:474-479`; `G/g_rpc-structures.h:200-203`, `G/g_sdk-structures.h:368-377` |
| C5 | `NV01_MEMORY_LOCAL_USER` 0x40 ×4 (2 MiB each: FB offsets 0x20000000, 0x21000000, 0x21400000, 0x22c00000) + UVM_CREATE_EXTERNAL_RANGE 0x49 + UVM_MAP_EXTERNAL_ALLOCATION 0x21 | nothing for the alloc; the UVM mapping writes PTEs with CE pushes (inline memcopy, memset_8) plus host TLB-invalidate methods | FB offsets = guest PMA addresses inside the region table | E (emulated; A for the method set, which is the one E5a runs) | `T:337-383`; `U/uvm_pte_batch.c:56`, `:74`; `U/uvm_tlb_batch.c:70`, `:86` |
| C6 | `KEPLER_CHANNEL_GROUP_A` 0xA06C (GR TSG) {hVASpace 0, engineType 1 = GR0} | `CE_GET_FAULT_METHOD_BUFFER_SIZE` 0x20802a08; GSP_RM_ALLOC 0xA06C (explicit); `INTERNAL_PROMOTE_FAULT_METHOD_BUFFERS` 0xa06c010a | hVASpace @8, engineType @12 (`SDK/nvos.h:2900-2907`) | A (`ctrl.rs`, `record_alloc`, `uvm.rs`). Note: hVASpace 0 here, so the VAS comes from the ctxshare (C7) | `T:388-389`; `K/gpu/fifo/kernel_channel_group_api.c:429`, `:426-505` |
| C7 | `FERMI_CONTEXT_SHARE_A` 0x9067 {hVASpace 0x5c000007, flags 1 = SUBCONTEXT_ASYNC, subctxId out} | GSP_RM_ALLOC 0x9067 (generic, RPC_TO_ALL) | hVASpace @0, flags @4, subctxId @8 (`SDK/nvos.h:3223-3234`) | A for the reply (echo). **Bookkeeping D: record ctxshare → hVASpace** (§5 item 2) | `T:391-394`; `resource_list.h:449` |
| C8 | 0x40, 2 MiB, handle 0x5c000014 (GPFIFO + USERD block, §3); BAR1 map; UPDATE_DEVICE_MAPPING_INFO 0x5e; UVM map | nothing. The BAR1 map makes the guest kernel write BAR1 PTEs (PRAMIN/BAR2) and TLB invalidates | - | A (fb.rs BAR paths); E (emulated) for the UVM PTE push | `T:399-419` |
| C9 | 0x3E 56 MiB pushbuffer (attr 0x4a800000) + map + UVM map | nothing; UVM PTE push | - | E (emulated) | `T:425-442` |
| C10 ×8 | per GR channel: 0x3E 4 KiB NOTIFIER + map (error notifier) | nothing | - | - | `T:448-460`, … |
| C11 ×8 | `TURING_CHANNEL_GPFIFO_A` 0xC46F under the GR TSG {hObjectError, gpFifoOffset 0x200200000+0x3000·i, 1024 entries, hContextShare 0x5c000013, hVASpace 0, hUserdMemory 0x5c000014, userdOffset 0x2000+0x3000·i, flags 0 or 0x10 alternating} | GSP_RM_ALLOC 0xC46F, parameters rewritten by the kernel: ChID in the USERD-index flags; instanceMem, ramfcMem and mthdbufMem in guest FB; userdMem = FB PA of 0x5c000014 + userdOffset; errorNotifierMem = sysmem DMA address; hContextShare and hVASpace copied | ChID (flags 10:8, 20:12), userdMem, gpFifoOffset/Entries, hContextShare, errorNotifierMem | A (`record_alloc`) **plus the VAS resolution through hContextShare (§5 item 2)**. Under physical execution: C (host channel; the guest ChID and token are shadow values, cuda-path.md §4.1) | `T:462-465` (and 19 more, decoded in §3); `K/gpu/fifo/kernel_channel.c:2588-2770` (rewrite at `:2609-2665`); `SDK/alloc/alloc_channel.h:296-341` |
| C12 ×20 | FIFO_GET_CHANNELLIST 0x80170d | kernel | - | - | `T:467` |
| C13 ×8 | `TURING_COMPUTE_A` 0xC5C0 on each GR channel | (a) **`GPU_PROMOTE_CTX` 0x2080012b** with PAs: the guest allocated MAIN, PM, PATCH, FECS_EVENT, the PRIV_ACCESS_MAPs and the global buffers in guest FB, bInitialize for fresh buffers. Sent on the user's client/subdevice, hChanClient/hObject = the channel. (b) GSP_RM_ALLOC 0xC5C0 (generic, CHANNEL_DESCENDANT_COMMON) | PROMOTE: engineType (GR0), hChanClient, hObject (channel), entryCount, promoteEntry[] | (a) **D**: echo, record per channel (the list goes to part 2, cuda-path.md §4.3 c1/c2). PRIVILEGED (0x10244), not forwardable unprivileged. (b) A (echo) | `T:470-473`; `K/gpu/gr/kernel_graphics_object.c:40-158`, `:580-661`; `G/g_subdevice_nvoc.c:4149`; `resource_list.h:2293` |
| C14 ×28 | NV906F GET_CLASS_ENGINEID 0x906f0101 | kernel (the RPC is `IS_VIRTUAL` only) | - | - | `T:474-476` (reply: class 0xc5c0, engine 1); `K/gpu/fifo/kernel_channel.c:2885-2905` |
| C15 ×20 | `TURING_DMA_COPY_A` 0xC5B5 on each channel | GSP_RM_ALLOC 0xC5B5 (generic) | - | A | `T:480`; `resource_list.h:1682` |
| C16 ×20 | GPFIFO_GET_WORK_SUBMIT_TOKEN 0xc36f0108 (native reply 0x35 = runlist 0, ChID 0x35) | kernel: `runlist << 16 \| ChID` (the RPC is `IS_VIRTUAL` only) | - | - | `T:485-487`; `K/gpu/fifo/kernel_channel.c:3225-3287` |
| C17 ×8 | GR_GET_CTX_BUFFER_SIZE 0x20801218 | kernel, from row 52 (host data). The RPC is `IS_VIRTUAL` only | - | - | `T:488-490`; `K/gpu/gr/kernel_graphics.c:4142-4158` |
| C18 ×8 | UVM_REGISTER_CHANNEL 0x1b, GR channels | (a) GSP_RM_ALLOC `UVM_CHANNEL_RETAINER` 0xC574 in UVM's client {hClient, hChannel of the CUDA channel}. (b) GET_WORK_SUBMIT_TOKEN (kernel). (c) UVM maps the channel's context buffers into the CUDA VAS: CE PTE pushes. (d) **`GPU_PROMOTE_CTX` 0x2080012b** on UVM's session client/subdevice, **VA only** (bufferId, gpuVirtAddr per resource) | (a) handles; (d) promoteEntry[].gpuVirtAddr | (a) A (echo; C under physical execution, as the params hold guest handles). (c) E (emulated). (d) **D**: echo, record the VAs next to C13's PAs | `T:844-863`; `K/rmapi/nv_gpu_ops.c:10446` (retainer), `:11060-11135` (bind → PROMOTE_CTX); `resource_list.h:436-446` (RPC_TO_ALL); `SDK/class/clc574.h:33` |
| C19 | TSG GPFIFO_SCHEDULE 0xa06c0101 {bEnable} | GSP_RM_CONTROL 0xa06c0101, user handles (re-sent) | - | A (`uvm.rs`) | `T:868`; `kernel_channel_group_api.c:1177-1191` |
| C20 | ALLOC_OS_EVENT; `NV01_EVENT_OS_EVENT` 0x79 on the subdevice {notifyIndex 0x3800000c = GRAPHICS (12) \| NONSTALL_INTR \| WITHOUT_EVENT_DATA \| SUBDEVICE_SPECIFIC} | GSP_RM_ALLOC 0x79 with NV0005 {hParentClient, hClass, notifyIndex \| CLIENT_RM} | notifyIndex | A (echo). Record it for non-stall delivery (§4.8) | `T:875-882`; `K/rmapi/event_api.c:141-171`; `src/nvidia/inc/kernel/vgpu/rpc.h:333-352`; `SDK/class/cl2080_notification.h:48`; `SDK/nvos.h:420-438` |
| C21 ×3 | CE TSGs 0xA06C {hVASpace 0x5c000007, engineType 0xb/0xc/0xd = COPY2/3/4}, each with 4 × (0x3E NOTIFIER, 0xC46F without ctxshare, 0xC5B5, CLASS_ENGINEID, TOKEN), 4 × UVM_REGISTER_CHANNEL (retainer only; resourceCount 0, so no PROMOTE), GPFIFO_SCHEDULE, one OS event with notifyIndex CE2/CE3/CE4 (0x19/0x1a/0x1b) \| NONSTALL | as C6, C11, C15, C18 (a), C19, C20 | TSG hVASpace (resolves the channels' VAS) | A (the same rows UVM's registration already uses) | `T:886-1448`; `K/rmapi/nv_gpu_ops.c:11092` (resourceCount 0 skips PROMOTE) |
| C22 | 0x3E 2 MiB, 0x40 ×n, UVM_ALLOC_SEMAPHORE_POOL 0x44 (+ mmap of the uvm fd), UVM_VALIDATE_VA_RANGE 0x48, 0x71 OS descriptors ×5 (flags 0x40001010), UVM_MAP_DYNAMIC_PARALLELISM_REGION 0x41, 0x3E 1.5 MiB, 0x40 14 MiB | nothing; UVM PTE pushes (sysmem PTEs carry guest DMA addresses) | - | E (emulated) | `T:1461-1801` |
| C23 | `GT200_DEBUGGER` 0x83DE {hAppClient 0xc1d53323, hClass3dObject 0x5c00001a (the first compute object)} | client triple for the session's internal client (GSP_RM_ALLOC ROOT/DEVICE/SUBDEVICE), then GSP_RM_ALLOC 0x83DE (generic, RPC_TO_ALL) | hAppClient, hClass3dObject | A (echo); C under physical execution | `T:1750-1753`; `K/gpu/gr/kernel_sm_debugger_session.c:105-200`; `resource_list.h:186` |
| C24 ×4 | OS events 0x79 on the compute object 0x5c00001a, notifyIndex 1..4 \| 0x30000000 (no NONSTALL) | GSP_RM_ALLOC 0x79 | notifyIndex, hSrcResource | A (echo). Signalled by `POST_EVENT` 0x1003 from GSP (guess: only on compute-class NOTIFY/awaken, §7) | `T:1814-1868`; `K/gpu/gsp/kernel_gsp.c:500`, `:1443-1444` |
| C25 | **DEBUG_SET_EXCEPTION_MASK 0x83de0309** {0x3a} | GSP_RM_CONTROL (routed, 0x10248) on the debugger object | exceptionMask | **D**: NV_OK, record it. C under physical execution (the host debugger object) | `T:1877-1879`; `G/g_kernel_sm_debugger_session_nvoc.c:685` |
| C26 | **GR_SET_CTXSW_PREEMPTION_MODE 0x20801210** {flags 1, hChannel 0x5c000012 = the GR **TSG**, gfxpPreemptMode 0, cilpPreemptMode 2} | GSP_RM_CONTROL (routed, 0x10348), user handles | hChannel, modes | **D**: NV_OK. C under physical execution (hChannel → host TSG) | `T:1880-1882`; `G/g_subdevice_nvoc.c:9369` |
| C27 | **SET_TIMESLICE 0xa06c0103** {timesliceUs 0x800} | GSP_RM_CONTROL 0xa06c0103 on the GR TSG (re-sent); the kernel keeps timesliceUs when OK | timesliceUs | **D**: NV_OK. C under physical execution (host TSG) | `T:1883-1893`; `kernel_channel_group_api.c:1321-1345` |
| C28 | 0x40 58 MiB (0x3a00000, probably local memory) + UVM map | nothing; PTE push | - | E (emulated) | `T:1898-1905` |
| C29 | **PERF_CUDA_LIMIT_SET_CONTROL 0x801909** {bCudaLimit 1} | **GSP_RM_CONTROL `NV0080_CTRL_CMD_INTERNAL_PERF_CUDA_LIMIT_SET_CONTROL` 0x802009** on the user's device, only when the per-device refcount changes (0→1 here, 1→0 at teardown) | bCudaLimit | **D**: NV_OK | `T:1923-1973`; `K/gpu/perf/kern_cuda_limit.c:126-134`; `G/g_device_nvoc.c:1355` |
| C30 | doorbells from the context's own pushes (SET_OBJECT per subchannel, compute-class state, possibly a CE memset of local memory) | BAR1 GPFIFO/GPPut writes and BAR0 0xBB0090 tokens | per push | **E**; emulated only if the channel emulator handles the compute class (§5 item 3). The native traces carry no push content, so the method set is unknown (§7) | cuda-path.md §2.1 ("all work after context creation goes through the mapped GPFIFO, USERD and doorbell") |

### 4.4 Memory allocation and mapping (nvprobe "stage 2")

| # | Guest call | Reaches device as | Params | Class | Evidence |
|---|---|---|---|---|---|
| M1 | cuMemAlloc ×3 (small): one `NV01_MEMORY_LOCAL_USER` 0x40, 2 MiB (flags 0x1c101, attr 0x11800000, attr2 0x100005, no REGISTER_MEMDESC_TO_PHYS_RM bit 31) | nothing: the guest PMA hands out FB inside the region table | FB offset | - | `T:1986-1988`; `P:lvl2` vs `P:lvl1` (4 lines); `K/mem_mgr/video_mem.c:988`; `SDK/nvos.h:1320` |
| M2 | UVM_CREATE_EXTERNAL_RANGE + UVM_MAP_EXTERNAL_ALLOCATION of M1 | `nvUvmInterfaceDupMemory`: no RPC for vidmem (`bRpcAlloc` false). `GetExternalAllocPtes` is kernel-local. PTE writes are CE pushes on UVM's channel into the root set at C4 | PTEs: vidmem = guest FB addresses (identity under option C) | E (emulated, method set A) | `T:1990-1993`; `K/mem_mgr/mem.c:1044-1050`; `U/uvm_map_external.c:187`, `:916` |
| M3 | cuMemAllocHost / pinned staging: 0x3E (attr 0x2a800000 or 0x4a800000) + map, or 0x71 OS descriptor over process memory (`TH:` 8 instead of 5) + UVM map | nothing; UVM PTE push with **guest DMA addresses** (host IOVAs once the DMA table is active) | sysmem PTE addresses | E (emulated through `guestmem.rs` translate); physically, requirement S | `T:1461`, `T:1563`; `K/mem_mgr/system_mem.c:434`; `K/mem_mgr/os_desc_mem.c:212`; sysmem-iova.md |
| M4 | cuda-torch: 25 × 0x40 and 9 × 0x71 instead of 15/5; nothing else new at RM level except GSS legacy telemetry (§4.10) | as M1-M3 | - | as M1-M3 | signature table of the six traces (§1 method) |

### 4.5 Module load (cuModuleLoadData with PTX JIT; nvprobe "stage 3" first half)

No RM call. `P:lvl3` differs from `P:lvl2` only by the teardown (§4.9), and the JIT runs in
libnvidia-ptxjitcompiler on the CPU. The device sees pushes only:
- code upload into vidmem, as a CE copy from staging or as inline-to-memory on the compute
  class (`NVC5C0_LINE_LENGTH_IN` 0x180 … `LAUNCH_DMA` 0x1b0, `LOAD_INLINE_DATA` 0x1b4,
  `SDK/class/clc5c0.h:90-176`);
- shader-cache invalidates (`INVALIDATE_SHADER_CACHES` 0x21c, `clc5c0.h:221`).

Class: **E**. With the software emulator it passes if it runs I2M and ignores the invalidates
(guess, §7).

### 4.6 Kernel launch (cuLaunchKernel; nvprobe "stage 3" and "stage 4")

No RM call: `P:lvl4*` equals `P:lvl3` at RM level, and `TL:2020`-`TL:10350` holds only `poll`
rows. The push on a GR channel carries:
- constant-buffer and parameter updates (I2M);
- the QMD in memory;
- `SEND_PCAS_A` 0x2b4 / `SEND_SIGNALING_PCAS_B` 0x2bc (`clc5c0.h:357`, `:364`);
- completion through `SET_REPORT_SEMAPHORE_A-D` 0x1b00-0x1b0c (`clc5c0.h:622-638`) or a host
  semaphore release, and `NON_STALL_INTERRUPT` for blocking sync.

Class: **E**. The result (vecAdd correct) needs SM execution, which no emulator in the device
provides. This is part 2.

### 4.7 memcpy (cuMemcpyHtoD / DtoH)

No RM call (`P:lvl2`: the two HtoD copies add no ioctl). The device sees:
- the GPFIFO and GPPut writes through BAR1;
- the doorbell;
- a push on a CE channel of one of the COPY2-4 TSGs, or on the CE subchannel of a GR channel:
  `TURING_DMA_COPY_A` offsets, `LAUNCH_DMA` virtual copy between the staging sysmem and vidmem,
  a semaphore release;
- possibly host semaphore acquires across channels.

Class: **E (emulated)**. The CE subset in `channel.rs` covers virtual and physical copies, remap
memsets, and one- and four-word releases (uvm.md). It resolves VAs through the channel's VAS,
which is correct only after §5 items 1-2. Small HtoD copies may use compute-class I2M instead
(guess). This is §5 item 3.

### 4.8 Synchronization

| Mode | What CUDA does | Device side | Class |
|---|---|---|---|
| `sched=auto` (spin then yield; `P:lvl4auto`) | polls a semaphore in memory, `poll` on the event fds as fallback | the semaphore release inside the executed push | E (emulated for CE work) |
| `CU_CTX_SCHED_BLOCKING_SYNC` (`P:lvl4blocking`, `cuda-launch`) | sleeps in `poll` on the 8 OS-event fds (`TL:` 8658 `poll` rows): the subdevice NONSTALL events GRAPHICS (C20) and CE2/3/4 (C21), plus the 4 compute-object events (C24) | **D**: the interrupt table (`crates/vfio-user-nvgpu/chips/tu106/interrupts.toml`, 6 entries today) needs non-stall entries for GR0 (MC_ENGINE_IDX 84) and CE2-CE4 (17-19) in a non-stall subtree. When an executed push reaches the host `NON_STALL_INTERRUPT` method (0x20, today only logged: `channel.rs` `host_method`) or a CE `LAUNCH_DMA` with a non-stall interrupt type, the device sets the matching LEAF bit and raises MSI. The guest kernel then posts the events itself; no RPC is involved | D (vectors), E (the trigger) |
| compute-object events (C24) | notified with no NONSTALL flag | `POST_EVENT` 0x1003 {hClient, hEvent, notifyIndex, …} from the device, when a compute `SET_REPORT_SEMAPHORE_D` carries AWAKEN_ENABLE (bit 20) | E; guess whether CUDA needs it (§7) |

Evidence: cuda-path.md §5.1-5.2; `src/nvidia/inc/kernel/gpu/intr/engine_idx.h:54-58`, `:129`;
`G/g_rpc-structures.h:1466-1477`; `K/gpu/gsp/kernel_gsp.c:500`.

### 4.9 Teardown (explicit at nvprobe level ≥ 3; cuCtxDestroy, cuMemFree, cuModuleUnload)

Order from `TL:10351`-`TL:11016`. On a plain `exit()` the same FREEs arrive as a cascade when
the fds close; they are not visible in the traces.

| # | Guest call | Reaches device as | Class | Evidence |
|---|---|---|---|---|
| X1 | UVM_FREE 0x22 (×28), NV_ESC_RM_FREE, UNMAP_MEMORY | UVM PTE-clear pushes; **FREE (fn 10)** for every RPC'd object (channels, CE/compute objects, TSGs, ctxshare, events, retainers, debugger, VAS, BINAPI, subdevice, device, client) | E (emulated); FREE: A (`Dispatcher::free`, `record_free`) | `TL:10351` onward |
| X2 | FREE_OS_EVENT ×8 | FREE of the 0x79 objects | A | `TL:10416-10438`, … |
| X3 | **PREEMPT 0xa06c0105** ×2 {…, 8 bytes} on the TSGs | GSP_RM_CONTROL (routed, 0x10248) | **D**: NV_OK once the emulated channel is idle. C under physical execution (host TSG) | `TL:10484`, `TL:10497`; `G/g_kernel_channel_group_api_nvoc.c:377` |
| X4 | PERF_CUDA_LIMIT_SET_CONTROL {0} | 0x802009 again (refcount 1→0) | D (as C29) | `TL:10506` |
| X5 | PERF_BOOST (clear) | 0x20800a9a again | D (as C2) | `TL:10547` |
| X6 | UVM_UNREGISTER_CHANNEL 0x1c ×20 | UVM stops each channel: **`STOP_CHANNEL` 0xa06f0112** {bImmediate} (re-sent by `kchannelCtrlCmdStopChannel`), for GR channels also **`GPU_EVICT_CTX` 0x2080012c** (routed, kernel-privileged, flags 0x1c240); then FREE of the retainer | **D**: NV_OK for both; stop the emulated channel. C (STOP) and E (EVICT) under physical execution | `TL:10567`, `TL:10727`, …; `K/rmapi/nv_gpu_ops.c:11153-11225`; `K/gpu/fifo/kernel_channel.c:1882-1890`; `U/uvm_user_channel.c:788`, `:890`; `G/g_subdevice_nvoc.c:4164` |
| X7 | UVM_UNREGISTER_GPU_VASPACE 0x1a | **`DMA_UNSET_PAGE_DIRECTORY` 0x801814** on the dup handle, then FREE of the dup | A (`uvm.rs`), again only correct with the alias (§5 item 1) | `TL:11014`; `U/uvm_va_space.c:1461` |
| X8 | GPU_RELEASE_COMPUTE_MODE_RESERVATION 0x20800146 | kernel | - | `TL:11016` |
| X9 | last close (no persistence) | adapter shutdown, the unload RPCs (phase5-rpcs.md Open 10) | as today | - |

### 4.10 Deltas of the other gate programs

| Program | New at RM level | Reaches device as | Class | Evidence |
|---|---|---|---|---|
| cuda-hostreg | 8 × 0x71 (cuMemHostRegister) | nothing; UVM maps the registered pages (sysmem PTEs, guest DMA addresses) | E (emulated) | `TH:` signature table |
| cuda-managed | UVM_DISABLE_READ_DUPLICATION 0x2d ×3, UVM_MIGRATE 0x33 ×2, UVM_ENABLE_READ_DUPLICATION 0x2c | nothing new at RM level; migrations are UVM CE pushes (emulatable), and GPU-side access to managed pages needs **replayable faults**, which the device never produces | E and out of scope (cuda-path.md §5.4) | `TM:1992`, `TM:2033`, `TM:2036` |
| cuda-torch, rlprobe, convburn | GPU_GET_NAME_STRING ×7 (kernel); GSS legacy telemetry **0x20809009** (8 bytes), **0x20809064** (520), **0x20809001** (8), **0x2080a084** (4), **0x2080a026** (532) | GSP_RM_CONTROL opaque, user handles | 0x20809009 and 0x20809064: A (`LEGACY`). **0x20809001, 0x2080a084 and 0x2080a026: B**, to add to `LEGACY` | `TT:2207-2219` |
| robustness (SIGKILL mid-run) | nothing new; the FREE cascade arrives in arbitrary order while pushes may be in flight | FREE | A; the channel emulator must drop state for freed channels (`record_free` does) | testing.md §3 |

## 5. Bookkeeping the device needs besides answers

These items break CUDA even if every RPC is answered OK. They are D (derived from guest
state).

1. **DUP_OBJECT alias for VA spaces.**
   - `nvUvmInterfaceDupAddressSpace` dups CUDA's VAS 0x5c000007 into UVM's session client
     (C4 a).
   - UVM then sends `DMA_SET_PAGE_DIRECTORY` (C4 c) and at teardown `UNSET` (X7) naming
     **UVM's dup handle**.
   - `channel.rs` keys VA spaces by (client, handle) (`record_set_page_directory`). The root
     therefore never reaches the entry the CUDA channels use.
   - `root_of`'s fallback ("the only root of that client") does not rescue it: CUDA's client
     has two VAS (0x5c000007 and 0x5c000008).
   - Fix: parse `rpc_dup_object_v03_00` (`D/ver/v610.rs` has `DUP_SIZE`/`DUP_STATUS`, fields
     `G/g_sdk-structures.h:368-377`). Keep (hClient, hObject) → (hClientSrc, hObjectSrc) and
     resolve through it in `record_set_page_directory`, `record_unset_page_directory` and
     `root_of`.
2. **VAS through the context share.**
   - The GR TSG has hVASpace 0 (C6), and the GR channels have hVASpace 0 and hContextShare
     0x5c000013 (C11).
   - `record_alloc` resolves only a channel's own hVASpace or its TSG's
     (`channel.rs:280-288`).
   - Record `FERMI_CONTEXT_SHARE_A` {hVASpace @0} (C7) and resolve channel → hContextShare →
     VAS.
   - The CE TSGs carry hVASpace 0x5c000007 directly (C21), so they work once item 1 is in.
3. **Per-subchannel class dispatch.**
   - `channel.rs` sends every method at offset ≥ 0x100 that is not addressed to
     `GP100_UVM_SW` to the **CE decoder** (`segment`: `else { Self::ce_method(...) }`, `channel.rs:592-599`).
   - CUDA's GR channels bind `TURING_COMPUTE_A` 0xC5C0 and `TURING_DMA_COPY_A` 0xC5B5 to
     different subchannels. Compute methods would therefore run as CE methods: `NVC5C0_INVALIDATE_TEXTURE_HEADER_CACHE_NO_WFI`
     0x244 and `SET_CWD_REF_COUNTER` 0x248 land on the CE's `SET_SEMAPHORE_B`/`SET_SEMAPHORE_PAYLOAD`
     (`SDK/class/clc5c0.h`, `SDK/class/clc5b5.h:39-43`); a compute push can thereby set up and
     fire bogus CE state.
   - Dispatch on `ch.objects[subch]` instead:
     - 0xC5B5 → CE;
     - 0xC5C0 → a compute stub that runs `SET_REPORT_SEMAPHORE_A-D` 0x1b00-0x1b0c (release,
       one or four words, reductions as the CE release), inline-to-memory 0x180-0x1b4
       (`LINE_LENGTH_IN`, `LINE_COUNT`, `OFFSET_OUT_UPPER/OFFSET_OUT`, `LAUNCH_DMA`,
       `LOAD_INLINE_DATA`), and `SET_I2M_SEMAPHORE_A-C` 0x1dc-0x1e4;
     - logs `SEND_PCAS_A`/`SEND_SIGNALING_PCAS_B` as "launch not executed", and logs and skips
       everything else;
     - unknown class → log and skip.
4. **Non-stall vectors** for GR0 and CE2-CE4 (§4.8), and raising them from the emulator.
5. **Channel keyed by (runlist, ChID).** The doorbell key is the ChID
   (`Registry::doorbell`). CUDA's GR channels are on runlist 0 and its CE channels on the CE
   runlists; ChIDs are allocated per runlist in the guest only if per-runlist channel RAM is
   enabled. It is not on TU106 (`kfifoIsPerRunlistChramEnabled`), so ChID alone is unique.
   Keep the assertion. (Inference; confirmed if no two recorded channels share a ChID in the
   first run.)

## 6. The minimal sets

### 6.1 cuInit and cuCtxCreate succeed without execution

Execution here means the physical GPU. The software channel emulator stays on and is
extended only as listed. Ordered by where each item stops the program today.

| Order | Item | Class | Where |
|---|---|---|---|
| 1 | 0x2080182a, 0x2080182b, 0x20803801 in `FORWARD` | A (already in the tree) | 4.1 rows 13, 21, 31 |
| 2 | 0x20802a0b `CE_GET_ALL_PHYSICAL_CAPS` → host public 0x20802a0a, same buffer | C (command id) | 4.1 row 18 |
| 3 | 0x20800aa2 `INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO` → NV_OK, zeros | D | 4.1 row 19 |
| 4 | GSS legacy 0x20808159 (332 B) and 0x20808162 (1 B) in `LEGACY` | B | 4.1 rows 28, 30 |
| 5 | Row 41 from the host's GR masks, and with it at module load: `GPU_PROMOTE_CTX` 0x2080012b (echo, record) and `INTERNAL_KGR_INIT_BUG4208224_WAR` 0x802046 (NV_OK) | C, D | 4.0, 4.1 row 17 |
| 6 | UVM registration in the CUDA process (E5a set) with `uvm_disable_hmm=1` | A, guest config | 4.1 row 33 |
| 7 | `INTERNAL_PERF_BOOST_SET_2X` 0x20800a9a → NV_OK | D | C2 |
| 8 | DUP_OBJECT VAS alias (§5.1) | D | C4, X7 |
| 9 | `GPU_PROMOTE_CTX` 0x2080012b for user channels (PA at compute alloc, VA from UVM bind) → echo, record | D | C13, C18 |
| 10 | ctxshare → VAS resolution (§5.2) | D | C7, C11 |
| 11 | `DEBUG_SET_EXCEPTION_MASK` 0x83de0309, `GR_SET_CTXSW_PREEMPTION_MODE` 0x20801210, `SET_TIMESLICE` 0xa06c0103, `INTERNAL_PERF_CUDA_LIMIT_SET_CONTROL` 0x802009 → NV_OK | D | C25-C27, C29 |
| 12 | Per-subchannel class dispatch with the compute stub (§5.3), so that the context's own init pushes retire and their semaphores are released | D / E (emulated) | C30 |

Items 1-6 are cuInit. Items 7-12 are cuCtxCreate. Items 1-4 and 7 are table rows that can be
added in one sitting. Items 8-12 change `channel.rs`, and item 12 is the only one whose extent
is unknown (§7).

### 6.2 cuMemAlloc and cuMemcpy succeed with the software copy engine

On top of 6.1:

| Item | Class | Where |
|---|---|---|
| Nothing new at RPC level for cuMemAlloc: vidmem and sysmem allocations send no RPC, and UVM's external mappings are CE pushes of the method set E5a already runs (inline memcopy, memset_8, TLB-invalidate host methods) | A / E (emulated) | M1-M3 |
| The VAS root of CUDA's channels is UVM's tree (§5.1) and GR channels find their VAS (§5.2); without both, every GPFIFO fetch of a CUDA channel fails | D | §5 |
| CUDA's CE pushes on COPY2-4 channels (virtual copy sysmem ↔ vidmem, releases, host acquires across channels) through the existing CE subset; small HtoD copies may use compute I2M (§5.3) | E (emulated) | 4.7 |
| For `nvprobe 2` as the gate runs it, polling completion is enough. Blocking sync (the `cuda-launch` criterion) additionally needs §5.4 | D | 4.8 |
| Teardown answers so that cuMemFree and cuCtxDestroy return cleanly: `PREEMPT` 0xa06c0105, `STOP_CHANNEL` 0xa06f0112, `GPU_EVICT_CTX` 0x2080012c → NV_OK (the second 0x802009 and 0x20800a9a are covered by 6.1) | D | X3, X6 |

With these, `cuda-core` ("stage 2 ok") should pass on the fake FB or the reservation. The copy
happens in the device, so the data round-trip is real.

### 6.3 What the kernel launch needs beyond that

A kernel's result needs the SMs, so everything here is **E**, part 2 (cuda-path.md §4, the
E5b/E5c ladder):
- **Host channels.** A host TSG and host channels per guest GR/CE channel. The GR contexts come
  from the host RM (c1). The guest's C13/C18 PROMOTE entries are recorded but not applied.
- **Page tables and the doorbell.**
  - Host `DMA_SET_PAGE_DIRECTORY` (PRIVILEGED, D12) points each host channel's VAS at the
    guest's UVM root: the one §5.1 recorded.
  - The doorbell token is translated guest → host.
  - GPFIFO and USERD sit in the reservation (they are guest vidmem, §3), so option C's identity
    makes them host-visible without copying.
- **Sysmem.** Every sysmem PTE the GPU follows (pushbuffers, semaphores, staging, error
  notifiers, 0x71 host memory) must be a host IOVA: requirement S, in place since E5b part 1.
- **Translation of the routed controls** that 6.1 answers with NV_OK:
  - C25 (exception mask on a host debugger object);
  - C26/C27/X3 (host TSG handle);
  - X6 (STOP_CHANNEL on the host channel; EVICT through the privileged client);
  - C18 a (retainer).
- **Events back to the guest.**
  - Host non-stall events on GR0/CE* become guest MSI on the §5.4 vectors.
  - A host RC becomes `RC_TRIGGERED` 0x1004 plus the error notifier (cuda-path.md §5.3).
- **Switch-over.** The software emulator is switched off per channel once the host channel
  exists.
- **Out of scope.** Managed memory (`cuda-managed` stage 2) additionally needs replayable
  fault delivery, which no public host interface offers (cuda-path.md §7.1 item 5).

## 7. Guesses and open points

1. **Row 41 and cuInit.** Whether cuInit already fails with gpcMask 0 or only cuCtxCreate does
   is a guess. Either way the fix is needed before a context works.
2. **The PROMOTE_CTX entry set** for user channels (which buffer IDs, which with bInitialize)
   depends on the static GR info. It was not traced, because PROMOTE_CTX is guest-kernel
   internal. The device logs it on first sight (phase5-rpcs.md Open 7).
3. **The context-init pushes** (C30) and the module-upload pushes (4.5) are not in any native
   trace; the v1 tracer sees ioctls only. The compute stub of §5.3 is a minimum. The first run
   should log every unhandled method with class, subchannel and offset.
4. **RUSD contents.** CUDA maps RUSD and reads it. The device leaves the page zero. Whether
   libcuda needs a non-zero field (a timestamp, for example) is unknown.
5. **POST_EVENT for compute-object events** (C24). The four events carry no NONSTALL flag.
   Whether blocking sync waits on them, on the NONSTALL subdevice events, or on both, is
   inferred from the notify indices, not observed. If a blocking-sync program hangs with §5.4
   in place, this is the next suspect.
6. **Tolerance of the D-class NV_OK answers** (C2, C25-C27, C29, X3, X6) is by construction:
   natively each returns NV_OK (`T:`/`TL:` status 0x0). Real GSP would also have acted on
   them. None of those effects are observable without execution.
7. **FB_GET_INFO_V2 forwarding** (4.1 row 9): which of CUDA's indices the kernel cannot serve
   locally, and so forwards, was not established statically. `nvml.rs` covers the control
   either way.
8. **GR TSG extras.** E5a saw only CE TSGs. A GR TSG may send additional internal controls
   at construction (a context header, for example). None were found in
   `kernel_channel_group_api.c` or `kernel_graphics_context.c` for a GSP client, but the
   search was not exhaustive. The dispatcher's `unanswered` list will name any.
