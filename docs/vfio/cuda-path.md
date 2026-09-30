<!-- SPDX-License-Identifier: MIT -->
<!-- Design note written 2026-09-23 against vendor/open-gpu-kernel-modules at tag 610.57.04 and the
     Leandro VFIO tree after milestone 1. Every claim about the driver cites file:line in that tree
     (paths relative to vendor/open-gpu-kernel-modules/); claims about Leandro v1 cite crates/,
     docs/ or matrix/. "Inference" marks conclusions drawn from the cited code rather than read
     in it. Nothing here is implemented. -->
# The CUDA path: stock driver in the guest, work executed by the physical GPU

Next stage after milestone 1 ([experiments.md](../history/caraxes-experiments-2026-09-29.md), "Milestone 1 reached"): CUDA runs
in the guest on the stock 610.57.04 driver, and the physical RTX 2070 executes its kernels.
Sections:

1. what `nvidia-uvm` needs from the device and from "GSP";
2. the first CUDA context, call by call, and who serves each call;
3. **guest DMA addresses versus host IOVAs**, the question that decides the design;
4. the execution model: three options compared, and a recommendation;
5. the interrupt and fault paths;
6. the milestone ladder E5a-E5d;
7. Open, and the host facts a read-only query could settle now.

Conventions: `S/` is `crates/vhost-user-nvrm/src/`, `D/` is `crates/vfio-user-nvgpu/src/`.
Control flag bits come from `src/nvidia/inc/kernel/rmapi/control.h`: PRIVILEGED 0x4 (:199,
meaning admin or kernel callers only, checked at `src/nvidia/src/kernel/rmapi/control.c:683-696`),
NON_PRIVILEGED 0x8 (:205), ROUTE_TO_PHYSICAL 0x40 (:230), INTERNAL 0x80 (:236).

## Summary

- **Recommended execution model: (a) identity-mapped VRAM, with GR/CE channels created host-side**
  (the hybrid (c) is how (a) builds its channels, not a separate option). The physical GPU walks
  the guest's own page tables; the device creates one host channel per guest channel and
  forwards the guest's doorbell writes.
- **Top three requirements:**
  1. A fixed-address, physically contiguous VRAM reservation at identical guest and host
     addresses ([fb-model.md](fb-model.md) §5 option C).
  2. A **privileged** host RM client in the device process: `DMA_SET_PAGE_DIRECTORY`,
     `GPU_PROMOTE_CTX` and `GET_SURFACE_PHYS_ATTR` are PRIVILEGED (§4.2).
  3. **Guest DMA addresses equal to host IOVAs** for all guest RAM the GPU touches (§3). Option C
     fixes only VRAM; without this, every sysmem PTE the guest writes points the physical GPU at
     the wrong host memory.
- **Ladder:**
  - E5a: `nvidia-uvm` registers the GPU on today's host-RAM FB, with the device emulating the
    handful of CE pushes that registration waits for.
  - E5b: the same pushes run on the physical GPU through host channels bound to the guest's page
    tables.
  - E5c: `cuda-core`, then a vector-add and `cuda-launch` with blocking sync.
  - E5d: the `gpu` gate's CUDA and PyTorch items. Managed memory stays out: it needs replayable
    faults, which the host keeps.
- **Biggest risk:** guest-written sysmem addresses reaching the physical GPU untranslated. They
  would read or corrupt host memory that the host nvidia driver has DMA-mapped, including the
  host GSP message queues, and could hang the only display GPU. The same model also gives the
  guest physical access to the card's whole VRAM, so there is no isolation. That matches v1's
  trusted-guest assumption (`docs/virtio/security.md`), but it rules out a hostile guest.

---

## 1. What `nvidia-uvm` needs

### 1.1 Load, open, UVM_INITIALIZE: nothing from the device

- **modprobe.** `uvm_init` (`kernel-open/nvidia-uvm/uvm.c:1159`) runs `uvm_global_init`, which
  calls two functions in nvidia.ko:
  - `nvUvmInterfaceSessionCreate` (`kernel-open/nvidia-uvm/uvm_global.c:117`) creates a
    kernel-local root client (`src/nvidia/src/kernel/rmapi/nv_gpu_ops.c:809`, `:866`). It sends
    no RPC, because the root reaches GSP only with the first device
    (`src/nvidia/inc/kernel/vgpu/rpc.h:48-104`).
  - `nvUvmInterfaceRegisterUvmEvents` (`uvm_global.c:63`) only stores callbacks
    (`kernel-open/nvidia/nv_uvm_interface.c:1095-1119`).
  - There is no GPU enumeration.
- **open.** `uvm_open` (`uvm.c:144-179`) makes no RM calls.
- **UVM_INITIALIZE.** `uvm_api_initialize` (`uvm.c:927-980`) → `uvm_va_space_create`
  (`uvm_va_space.c:244`) makes no RM calls.

### 1.2 UVM_REGISTER_GPU, in order

Entry: `uvm_api_register_gpu` (`kernel-open/nvidia-uvm/uvm_gpu.c:4049`) → `gpu_retain_by_uuid_locked`
(`uvm_gpu.c:3031`). Every step is fatal unless noted.

| # | Call | Site | Becomes, on the device side |
|---|---|---|---|
| 1 | `nvUvmInterfaceRegisterGpu` | `uvm_gpu.c:3058`; `nv_uvm_interface.c:142-187` | nothing (the GPU is already open) |
| 2 | `nvUvmInterfaceGetGpuInfo` | `uvm_gpu.c:3063`; `nv_gpu_ops.c:7640` | client triple ALLOC/FREE. The rest is kernel-served from cached static info, except `BUS_GET_INFO_V2[PCIE_GPU_LINK_CAPS]`, which reads BAR0 0x88084 (config mirror, link capabilities). Speed code 1..6 in bits 3:0 is required, otherwise `NV_ERR_INVALID_STATE` (`nv_gpu_ops.c:2166-2168`; `src/nvidia/src/kernel/gpu/bif/kernel_bif.c:1256-1279`). Arch must be TU100. `ACCESS_COUNTER_BUFFER_COUNT` is hardcoded 1 for TU10x (`src/nvidia/generated/g_uvm_nvoc.c:280`) |
| 3 | `add_gpu` / `gpu_supports_uvm` | `uvm_gpu.c:2870`, `:353-358` | `subdeviceCount == 1` |
| 4 | `nvUvmInterfaceDeviceCreate` | `uvm_gpu.c:1455`; `nv_gpu_ops.c:2247` | client triple. A kernel-only `VOLTA_USERMODE_A` maps BAR0 0xBB0000 (`nv_gpu_ops.c:5637`). `GPU_QUERY_ECC_STATUS` 0x2080012f is **routed**; NOT_SUPPORTED is accepted (`nv_gpu_ops.c:1344-1348`). The classlist must contain C46F, C5B5, C369 (MMU_FAULT_BUFFER) and C365 (ACCESS_COUNTER_NOTIFY_BUFFER) (`nv_gpu_ops.c:8838`) |
| 5 | HAL selection | `kernel-open/nvidia-uvm/uvm_hal.c:637-710` | `ceClass` 0xC5B5, `hostClass` 0xC46F, arch TU100, otherwise `NV_ERR_INVALID_CLASS` |
| 6 | `nvUvmInterfaceInitFaultInfo` | `uvm_gpu_replayable_faults.c:255`; `nv_gpu_ops.c:9385-9545` | **GSP_RM_CONTROL** `INTERNAL_GMMU_REGISTER_FAULT_BUFFER` 0x20800a9b (`src/nvidia/src/kernel/gpu/mmu/kern_gmmu.c:1264`) carrying the PTE array of a guest-sysmem buffer, and `INTERNAL_GMMU_REGISTER_CLIENT_SHADOW_FAULT_BUFFER` 0x20800a9d (`kern_gmmu.c:1818`). The buffer size comes from row 19 of [phase5-rpcs.md](../history/caraxes-phase5-rpcs-610.md) (`kern_gmmu.c:1222`, `:1920`). The device already answers 256 × 32 bytes (`D/ctrl.rs:381-388`); it must be non-zero and a multiple of 32 (`uvm_gpu_replayable_faults.c:146`) |
| 7 | `nvUvmInterfaceOwnPageFaultIntr(TRUE)` | `uvm_gpu_replayable_faults.c:187`; `nv_gpu_ops.c:2477-2489` | nothing: a kernel-only mask flip (`src/nvidia/src/kernel/gpu/mmu/arch/volta/kern_gmmu_gv100.c:86-107`) |
| 8 | replayable reinit | `uvm_gpu_replayable_faults.c:119-135` | BAR0 reads of GET 0xB83028 and PUT 0xB8302C, and a read-modify-write of PAGE_FAULT_CTRL 0xB83070 (`src/nvidia/src/kernel/gpu/mmu/arch/turing/kern_gmmu_tu102.c:188-236`; `src/common/inc/swref/published/turing/tu102/dev_vm.h:74-113`). GET must be below the entry count, and PUT bit 30 (GETPTR_CORRUPTED) must be 0 (`kernel-open/nvidia-uvm/uvm_turing_fault_buffer.c:85-100`) |
| 9 | `nvUvmInterfaceInitAccessCntrInfo` | `uvm_gpu_access_counters.c:425` | the size is read from **BAR0 0xB83110** × 32 (`src/nvidia/src/kernel/gpu/uvm/arch/turing/uvm_tu102.c:265-274`; `dev_vm.h:209`), and **0 fails registration**. Then GSP_RM_CONTROL `INTERNAL_UVM_REGISTER_ACCESS_CNTR_BUFFER` 0x20800a1d (`src/nvidia/src/kernel/gpu/uvm/uvm.c:215-255`). Counters are allocated but not enabled on x86 by default (`uvm_gpu_access_counters.c:69`, `:146-157`, `:522-531`) |
| 10 | `nvUvmInterfaceQueryCaps`, `AddressSpaceCreate` | `uvm_gpu.c:154`, `:236`; `nv_gpu_ops.c:6919`, `:2604` | GSP_RM_ALLOC FERMI_VASPACE_A plus `COPY_SERVER_RESERVED_PDES` (§2). TIME_0/1 at 0xBB0080/0xBB0084 are read |
| 11 | `nvUvmInterfaceQueryCopyEnginesCaps` | `uvm_channel.c:3191`; `nv_gpu_ops.c:8645` | per CE, GSP_RM_CONTROL `CE_GET_PHYSICAL_CAPS` 0x20802a07 (`src/nvidia/src/kernel/gpu/ce/kernel_ce.c:563-569`) and routed `CE_GET_CE_PCE_MASK`. At least one non-GRCE CE must have SYSMEM and P2P caps (`uvm_channel.c:2915-2927`) |
| 12 | channel manager | `uvm_channel.c:3877`, `:3426-3456`, `:2842` | one TSG per usable CE (`nvUvmInterfaceTsgAllocate`, `:2675`) with 2 channels (`:2580-2627`). Per channel: GSP_RM_ALLOC TURING_CHANNEL_GPFIFO_A, GP100_UVM_SW, TURING_DMA_COPY_A, then GSP_RM_CONTROL `GPFIFO_SCHEDULE` 0xa06f0103 (`nv_gpu_ops.c:5832-6347`). GPFIFO and USERD are in **vidmem** by default (`uvm_channel.c:3393-3394`); pushbuffer and semaphores in sysmem |
| 13 | `channel_init`: **needs execution** | `uvm_channel.c:2546-2563` | a push with CE `SET_OBJECT`, host `SET_OBJECT` and a CE semaphore release, then `uvm_push_end_and_wait` |
| 14 | `uvm_page_tree_init`: **needs execution** | `uvm_mmu.c:1182-1188` | a CE memset of the page-tree root, waited on |
| 15 | `nvUvmInterfaceSetPageDirectory` | `uvm_gpu.c:1346`; `nv_gpu_ops.c:8990` | GSP_RM_CONTROL `NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY` 0x801813 (`src/nvidia/src/kernel/gpu/mem_mgr/dma.c:497-510`), carrying the root PA of UVM's page tree. The kernel then copies its PDEs into that root and invalidates the TLB through 0xB830B0 |
| 16 | enable/clear replayable faults | `uvm_gpu.c:2978-2982`; `uvm_turing_fault_buffer.c:36-66`, `:102-128` | writes to LEAF_EN_SET, LEAF (W1C) and GET for the REPLAYABLE_FAULT vector |

**The wait has no timeout.** `uvm_push_end_and_wait` spins, printing every 30 s
(`kernel-open/nvidia-uvm/uvm_common.c:249-279`, `uvm_tracker.c:300-321`). It gives up only if the
channel's error notifier holds a non-zero status, and then fails with `NV_ERR_RC_ERROR`
(`uvm_channel.c:2058-2081`).

So registration needs, per usable CE, two executed pushes, plus one for the page tree. For each
one, the device (or the physical GPU) must do the following after the doorbell write to 0xBB0090:
- read the GPFIFO entry (in vidmem, at a GPU VA of UVM's VA space);
- fetch the pushbuffer (in sysmem, through the same VA space);
- perform the CE semaphore release (sysmem) and, for the page tree, a memset by physical address.

With three CEs advertised (CE0-2, [fifo-device-info.md](fifo-device-info.md)) that is up to seven
pushes. Advertising one usable async CE reduces it to three.

### 1.3 What the device must add for UVM (beyond milestone 1)

| Item | Now | Needed |
|---|---|---|
| 0xB83110 access-counter size | unmapped | non-zero, e.g. 256 |
| 0xB83028/0xB8302C/0xB83070 | unmapped | GET writable; GET = PUT = 0; PAGE_FAULT_CTRL latched |
| 0x88084 link capabilities | from the config space | speed code 1..6 (check the captured value) |
| RPCs 0x20800a9b, 0x20800a9d, 0x20800a1d, 0x20802a07, 0x20802a02, 0x801813, 0x2080012f | not answered | record the buffers and PDB; reply OK / the caps / NOT_SUPPORTED for ECC |
| GP100_UVM_SW, TSG, channel and CE allocations | record-only for the golden/watchdog rows | per-channel state: ChID, instance, USERD, GPFIFO VA, VAS root (§2.2) |
| CE execution for 3 to 7 pushes | none | E5a: emulated; E5b: physical (§6) |

---

## 2. The first CUDA context, call by call

### 2.1 What v1 observed

Evidence: `matrix/traces/610.57.04/cuda-core.jsonl` (context plus memory, `matrix/PROBES.md:33`) and
`cuda-launch.jsonl` (100 launches with blocking sync, `matrix/PROBES.md:37`), recorded natively on
the RTX 2070. The two probes allocate the **same set of RM objects**. So a kernel launch creates
no RM object; all work after context creation goes through the mapped GPFIFO, USERD and doorbell.

In order (handles from cuda-launch):
1. `NV01_ROOT_CLIENT`, NV_CONFIDENTIAL_COMPUTE.
2. `NV01_DEVICE_0`, then under the subdevice: `NV20_SUBDEVICE_0`, BINAPI 0x2081,
   `TURING_USERMODE_A` 0xc461 (mapped) and `RM_USER_SHARED_DATA` 0xde (mapped).
3. About 40 identity and caps controls (FB_GET_INFO_V2, GR_GET_*, CE_GET_ALL_CAPS, GET_ENGINES_V2,
   CLASSLIST_V2, GSP_GET_FEATURES and others).
4. `FERMI_VASPACE_A` twice; UVM_REGISTER_GPU; `NV50_MEMORY_VIRTUAL` (0xfb000000 bytes);
   UVM_REGISTER_GPU_VASPACE.
5. 15 `NV01_MEMORY_LOCAL_USER` (mostly 2 MiB) and 26 `NV01_MEMORY_SYSTEM`, each followed by
   UVM_CREATE_EXTERNAL_RANGE plus UVM_MAP_EXTERNAL_ALLOCATION.
6. 4 `KEPLER_CHANNEL_GROUP_A`, 1 `FERMI_CONTEXT_SHARE_A` and 20 `TURING_CHANNEL_GPFIFO_A`:
   - 8 in the first TSG, each with `TURING_COMPUTE_A` 0xc5c0 and `TURING_DMA_COPY_A` 0xc5b5;
   - 12 CE-only channels in three more TSGs.
   - Each channel is preceded by a 4 KiB `NV01_MEMORY_SYSTEM` (its USERD, see §2.2).
   - Per channel: FIFO_GET_CHANNELLIST 0x80170d, GET_CLASS_ENGINEID 0x906f0101,
     GET_WORK_SUBMIT_TOKEN 0xc36f0108, and GR_GET_CTX_BUFFER_SIZE on the compute channels;
     UVM_REGISTER_CHANNEL.
   - Per TSG: GPFIFO_SCHEDULE 0xa06c0101.
7. 8 `NV01_EVENT_OS_EVENT`, 5 OS descriptors (0x71), `GT200_DEBUGGER` 0x83de with
   SET_EXCEPTION_MASK, SET_CTXSW_PREEMPTION_MODE, SET_TIMESLICE, PREEMPT, and
   PERF_CUDA_LIMIT_SET_CONTROL.

`cuda-managed` adds the UVM ioctls 0x2d, 0x33 and 0x2c (managed memory); `cuda-torch` adds
allocations only.

### 2.2 Who serves each call in a GSP-client guest

The RPC mechanics:
- A class whose resource flags carry `RS_FLAGS_ALLOC_RPC_TO_*` is forwarded generically after its
  constructor (`src/nvidia/src/kernel/rmapi/alloc_free.c:878-938`). Constructor-side RPCs (such as
  PROMOTE_CTX) therefore arrive **before** the object's own GSP_RM_ALLOC.
- A control with 0x40 is forwarded unchanged (`src/nvidia/src/kernel/rmapi/resource.c:255-290`).
- The label "device answers" below means the synthetic GSP; "physical GPU" means the call only
  works if real hardware executes.

| Object / call | Guest kernel alone | Reaches the device | Needs the physical GPU |
|---|---|---|---|
| ROOT, DEVICE, SUBDEVICE | – | client triple (`src/nvidia/src/kernel/gpu/device.c:181-193`, `subdevice/subdevice.c:121-131`); **record** | – |
| `TURING_USERMODE_A` + map | yes: a BAR0 REGMEM mapping of 0xBB0000, 64 KiB (`src/nvidia/src/kernel/gpu/fifo/usermode_api.c:40-104`, `arch/volta/kernel_fifo_gv100.c:353-376`, `src/nvidia/src/kernel/rmapi/mapping_cpu.c:666-690`) | the page is BAR0 → trapped | the doorbell at +0x90 is the submission point (§4) |
| `RM_USER_SHARED_DATA` | yes (`src/nvidia/src/kernel/gpu/gpu_user_shared_data.c:106-148`) | `SET_DATA_POLL` if polling is requested (:484-531) | – |
| identity and caps controls | yes, from static info (FB_GET_INFO_V2 `kern_mem_sys_ctrl.c:1018-1038`; GR_GET_INFO `kernel_graphics.c:3023`; GET_ENGINES_V2 `subdevice_ctrl_gpu_kernel.c:1451`; CLASSLIST_V2 `device_ctrl.c:78`) | CE_GET_ALL_CAPS → `CE_GET_ALL_PHYSICAL_CAPS` 0x20802a0b (`kernel_ce_shared.c:283-320`); PERF_BOOST → 0x20800a9a (`kern_perf_boost.c:43-118`); CUDA_LIMIT → 0x802009 (`kern_cuda_limit.c:95-135`) | – |
| `FERMI_VASPACE_A` | VAS built in the guest | GSP_RM_ALLOC (`src/nvidia/src/kernel/gpu/mem_mgr/vaspace_api.c:227-275`), then `COPY_SERVER_RESERVED_PDES` 0x90f10106 whose `levels[0].physAddress` is the **root PDB** (`src/nvidia/src/kernel/mem_mgr/gpu_vaspace.c:3825-3916`, `:4944-4972`; `src/common/sdk/nvidia/inc/ctrl/ctrl90f1.h:268-330`). The range [4 GiB, 4.5 GiB) belongs to the server (`gpu_vaspace.c:397-420`, `:303-367`) | – |
| `NV50_MEMORY_VIRTUAL`, GPU mappings | yes: split VAS, the guest writes PTEs itself (`src/nvidia/src/kernel/mem_mgr/virtual_mem.c:461-473`; `src/nvidia/src/kernel/gpu/mmu/gmmu_walk.c:782-840`) | TLB invalidate registers 0xB830A0-0xB830B0 (`kern_gmmu_tu102.c:95-150`) | the physical GPU walks these tables (§3, §4) |
| `NV01_MEMORY_LOCAL_USER` | yes: the guest PMA; the vidmem RPC is skipped on a GSP client (`src/nvidia/src/kernel/mem_mgr/video_mem.c:988-1008`) | only with `ATTR2_REGISTER_MEMDESC_TO_PHYS_RM` (`src/nvidia/src/kernel/mem_mgr/mem.c:484-580`) | its bytes live in (real) VRAM |
| `NV01_MEMORY_SYSTEM`, OS descriptor 0x71 | yes (`src/nvidia/src/kernel/mem_mgr/system_mem.c:434-470`, `os_desc_mem.c:212-222`) | – | its guest DMA addresses end up in PTEs (§3) |
| UVM external mappings | yes: guest kernel RM computes the PTEs; UVM writes them with **CE pushes** (`kernel-open/nvidia-uvm/uvm_pte_batch.c:56`, `:74`, `:207`), and TLB invalidates are host methods in the pushbuffer (`uvm_tlb_batch.c:70`, `:86`) | nothing | yes: the writes are GPU work |
| `KEPLER_CHANNEL_GROUP_A` | TSG ID chosen in the guest (`src/nvidia/src/kernel/gpu/fifo/kernel_channel_group.c:173-175`) | GSP_RM_ALLOC, then `INTERNAL_PROMOTE_FAULT_METHOD_BUFFERS` 0xa06c010a with guest-sysmem method buffers (`kernel_channel_group_api.c:426-505`) | host TSG |
| `FERMI_CONTEXT_SHARE_A` | VEID chosen in the guest (`kernel_ctxshare.c:139-147`) | GSP_RM_ALLOC (generic) | host subcontext |
| `TURING_CHANNEL_GPFIFO_A` | ChID chosen in the guest (`kernel_channel.c:842-857`). The instance block is allocated in guest FB and **only zeroed** (`arch/maxwell/kernel_channel_gm107.c:239-274`). USERD **must** be client-provided (`kernel_channel.c:2229-2255`); CUDA provides a 4 KiB sysmem page | GSP_RM_ALLOC with PAs of instance, RAMFC, USERD, method buffer and error notifier, the GPFIFO **VA**, and the ChID in the USERD-index flags (`kernel_channel.c:2588-2770`; `src/common/sdk/nvidia/inc/alloc/alloc_channel.h:296-341`). No code in the kernel fills RAMFC for user channels (`kfifoInitRamfc*_TU102` at `kernel_fifo_tu102.c:905-1065` has no caller): **GSP is expected to fill the instance block** | yes |
| `TURING_COMPUTE_A` | the guest allocates and maps **all GR context buffers** (`kernel_graphics_object.c:180-228`; `kernel_graphics_context.c:1379-1465`, `:1615-1705`; `gpu_registry.c:151-155`) | GSP_RM_CONTROL `GPU_PROMOTE_CTX` 0x2080012b first (flags 0x10244: routed and PRIVILEGED, `src/nvidia/generated/g_subdevice_nvoc.c:4147-4153`), then GSP_RM_ALLOC. For compute: MAIN, PM, PATCH, FECS_EVENT and one of the priv access maps, each `{gpuPhysAddr, gpuVirtAddr, size, physAttr, bufferId, bInitialize, bNonmapped}` (`kernel_graphics_object.c:580-661`; `ctrl2080gpu.h:893-971`). GSP is expected to initialise buffers marked `bInitialize` from the golden image (`kernel_graphics_object.c:143-158`) | yes |
| `TURING_DMA_COPY_A` | stub constructor (`kernel_ce_context.c:195-200`) | GSP_RM_ALLOC | yes |
| GET_CLASS_ENGINEID, GET_WORK_SUBMIT_TOKEN, GR_GET_CTX_BUFFER_SIZE, FIFO_GET_CHANNELLIST | yes. The token is `runlist << 16 | ChID` (`kernel_fifo_tu102.c:75-130`; `tu102/dev_ctrl.h:36-37`) | – | – |
| GPFIFO_SCHEDULE, SET_TIMESLICE | kernel handler | explicitly re-sent as RPC (`kernel_channel_group_api.c:1066-1192`, `:1297-1345`) | host schedule |
| PREEMPT, SET_CTXSW_PREEMPTION_MODE, SET_EXCEPTION_MASK | – | routed (`g_kernel_channel_group_api_nvoc.c:375-381`; `g_subdevice_nvoc.c:9367-9373`; `g_kernel_sm_debugger_session_nvoc.c:683-689`) | host equivalents |
| `GT200_DEBUGGER` | internal triple | GSP_RM_ALLOC | – for the first milestone |
| `NV01_EVENT_OS_EVENT` | registered in the guest (`src/nvidia/src/kernel/rmapi/event_api.c:186-193`) | also GSP_RM_ALLOC (`event_api.c:149-169`); **record** | completion wakeups come from non-stall interrupts (§5) |
| UVM ioctls | yes, in guest `nvidia-uvm` | via its RM calls (§1) | yes, for UVM's own pushes |

**Consequence.** After context creation, the device holds for each guest channel:
- its ChID and runlist;
- the instance-block, USERD, method-buffer and error-notifier addresses;
- the GPFIFO VA and entry count;
- its VAS root (from COPY_SERVER_RESERVED_PDES or SET_PAGE_DIRECTORY);
- its VEID;
- its GR context buffers (PA and VA per buffer ID).

That is everything real GSP would program. What the device cannot see are the guest's PTE writes
through the GPU (UVM) and the contents of the pushbuffers.

---

## 3. Guest DMA addresses versus host IOVAs: the deciding question

### 3.1 The problem

- **What the guest writes into sysmem PTEs.** The guest has no vIOMMU, so its DMA addresses are
  guest physical addresses ([vfio-user.md](vfio-user.md):42). Every sysmem PTE the guest writes
  carries a GPA: its own mappings, the external-allocation PTEs UVM writes, the USERD and GPFIFO
  addresses of CUDA channels, semaphores, pinned host buffers. So do the sysmem addresses in the
  RPCs it sends: fault buffers, method buffers, USERD PAs.
- **How the physical GPU reads them.** It treats a sysmem address as a host bus address. The host
  IOMMU is on (AMD-Vi, group 28 holding exactly the card, [passthrough-procedure.md](passthrough-procedure.md)
  §1.1). Its domain type is not recorded yet.
  - **Translated domain:** only IOVAs the host nvidia driver mapped are valid. A GPA either faults
    (IO_PAGE_FAULT, then a host MMU/RC error) or, if it happens to equal an IOVA the host driver
    mapped, reads or writes that host buffer. Linux tries 32-bit IOVAs first for PCI devices, so
    collisions with low GPAs are likely. The buffers at risk include the host GSP queues.
  - **Identity domain:** the GPU accesses host physical memory at the GPA, which is corruption.
- **What option C does not cover.** It makes guest FB addresses real VRAM addresses; nothing
  corresponds to it for sysmem.
- **Why no device-side fix exists.** The guest kernel RM computes sysmem PTEs from its own DMA
  mappings, and UVM writes them with CE pushes (§2.2). The device neither sees nor can rewrite
  them.
- **How NVIDIA solves it.** Direct guest page tables are used only with SR-IOV, where each VF has
  its own IOMMU domain with IOVA = GPA. Turing has no SR-IOV; the vGPU-VF path is dead for TU10x
  ([driver-modes.md](driver-modes.md)).

**Requirement S:** for every guest page the physical GPU may access, the address the guest writes
into a PTE must be the host IOVA of that page. For no other guest-writable address may the host
domain hold a mapping. The second half cannot be guaranteed against a buggy or hostile guest; see
the risk in §4.4.

### 3.2 Candidate fixes

| Fix | How | Stock guest | Host changes | Verdict |
|---|---|---|---|---|
| S1: pinned guest RAM plus an IOVA table, applied by the guest | At VM start the device registers all guest RAM with the host RM as OS descriptors. v1 already does this per range: `host_pool::Arena::build` (`S/host_pool.rs:268-373`), a 0x71 allocation on a registered GPU FD (`S/host_pool.rs:486-504`), and `PinBudget` accounting (`S/host_pool.rs:40-102`). The host RM pins and DMA-maps the pages. The device learns each descriptor's IOVA segments with `NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR` 0x410103 (`memOffset`, `contigSegmentSize` at `AT_GPU`, `src/nvidia/src/kernel/gpu/mem_mgr/mem_ctrl.c:278-294`). That control is **PRIVILEGED** (flags 0x4, `src/nvidia/generated/g_mem_nvoc.c:166`). The guest then returns host IOVAs as DMA addresses for the synthetic GPU through (i) a small guest DMA-ops/IOVA aid (a permitted temporary aid) or (ii) a per-device DMA offset table from firmware (ACPI `_DMA`; x86 support not verified here) | (i) no, temporary aid; (ii) yes if Linux honours it on x86 | none; admin RM client | **recommended**, with the aid first and (ii) as the stock-guest follow-up |
| S2: GPU work touches VRAM only | UVM parameters `uvm_channel_gpfifo_loc`, `uvm_channel_gpput_loc`, `uvm_channel_pushbuffer_loc` (`kernel-open/nvidia-uvm/uvm_channel.c:84-86`) and `uvm_page_table_location` (`uvm_mmu.c:73`) | yes | none | enough for UVM's own channels, **not for CUDA**, whose USERD, pushbuffers, semaphores and pinned staging are sysmem by its own choice (§2.1). Useful for E5a/E5b as a partial measure |
| S3: guest RAM placed at GPA equal to host IOVA | the VMM places a host-RM sysmem allocation at a GPA equal to its IOVA | yes | VMM patch | fragile: IOVAs are chosen by the host (often below 4 GiB), guest low RAM cannot be relocated, and the guest cannot be steered to allocate GPU DMA only from that range. Rejected |
| S4: host kernel helper maps guest RAM at IOVA = GPA in the GPU's domain | a host module calls `iommu_map` on the nvidia-owned domain | yes | a host kernel module touching the nvidia driver's domain | conflicts with "do not modify the host driver" in spirit and needs the operator's explicit go-ahead. Last resort |
| S5: shadow page tables | device rewrites sysmem PTEs | yes | none | impossible for UVM-written tables (GPU writes, in-band invalidates, §4.1 (b)) |

S1 in detail:
- **Pinning cost.** All guest RAM is pinned, as VFIO passthrough does. `LEA_MAX_PIN_TOTAL_MIB`
  (default 1 GiB, [architecture.md](../virtio/architecture.md) "Guest-page backing") must cover the
  guest.
- **Table size.** With the IOMMU on, the host RM maps each descriptor's scatterlist into one IOVA
  range, so the table has a few entries per descriptor (inference; to confirm with
  `contigSegmentSize`).
- **CPU side.** Unchanged: the guest CPU still uses GPAs; only device-visible DMA addresses change.
- **Addresses in RPCs.** Once the aid is in place, the sysmem PAs the guest sends in RPCs (fault
  buffer PTE arrays, method buffers, USERD, error notifiers) are host IOVAs as well. The device
  must map them back to guest RAM with the same table for its own accesses.

---

## 4. Execution model

### 4.1 The three options

**(a) Identity-mapped VRAM range** (fb-model option C), channels on the host RM.
- **Memory.** Guest FB address X means host VRAM X. The guest's region table exposes only
  `[base, base + L)` as usable ([fb-model.md](fb-model.md) §5), backed by one host
  `NV01_MEMORY_LOCAL_USER` allocation with `NVOS32_ALLOC_FLAGS_FIXED_ADDRESS_ALLOCATE`
  (`src/common/sdk/nvidia/inc/nvos.h:1441`) and `NVOS32_ATTR_PHYSICALITY_CONTIGUOUS` (`nvos.h:1083`),
  made early.
- **CPU access to FB.** Guest BAR1/BAR3/PRAMIN accesses still trap into the device (as now,
  `D/fb.rs`), which forwards them through a host BAR1 mapping of the reserved allocation. That is
  windowed, since host BAR1 is 256 MiB (cardinfo `fb 0x7fe0000000+0x10000000` in
  `cuda-launch.jsonl`).
- **Instance blocks cannot be the guest's.** The host RM (itself a GSP client) builds instance
  block and RAMFC with `kchannelAllocMem_HAL`. `instanceMem`/`ramfcMem` from the params are honoured
  only inside GSP firmware or a VF (`src/nvidia/src/kernel/gpu/fifo/kernel_channel.c:2265-2285`), and
  internal fields are cleared for callers (`kernel_channel.c:215-226`). So the physical channel is
  a **host** channel; the guest's ChID, instance block and token are shadow values.
- **Page tables.** The host channel uses a host `FERMI_VASPACE_A` made externally owned
  (`NV_VASPACE_ALLOCATION_FLAGS_IS_EXTERNALLY_OWNED`, `nvos.h:3162`) whose root is set to the
  guest's PDB with `NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY` 0x801813
  (`src/common/sdk/nvidia/inc/ctrl/ctrl0080/ctrl0080dma.h:798-814`). On the host (a GSP client) that
  control is forwarded to host GSP (`dma.c:497-510`). Its flags 0x14004 include PRIVILEGED
  (`src/nvidia/generated/g_device_nvoc.c:1218`). The physical GPU then walks the guest's tables
  directly; the vidmem PTEs are right by identity, the sysmem PTEs by requirement S (§3).
- **USERD.** Must be a client memory object (`kernel_channel.c:2229-2255`). The device passes the
  guest's USERD: for a vidmem USERD, the reserved allocation with `userdOffset = PA - base`; for a
  sysmem USERD (CUDA), an OS descriptor over that guest page. GPPut (USERD +0x8c) and GPGet (+0x88,
  `src/common/sdk/nvidia/inc/class/clc46f.h:48-64`) are then shared bytes: the guest process writes
  GPPut, the physical GPU updates GPGet.
- **Submission.** The guest writes its token to 0xBB0090, a trapped BAR0 page. The device maps
  the token to the host channel and writes the host token to the host doorbell (`nvrm-client`
  `map_doorbell`, `crates/nvrm-client/src/mem.rs:325`). It does not interpret the GPFIFO.

**(b) Full translation** (shadow page tables, PTE rewriting, GPFIFO rewriting).
- **Why it cannot be complete.** UVM writes PTEs with CE pushes and invalidates with host methods
  in the same push (`uvm_pte_batch.c:56-207`, `uvm_tlb_batch.c:70-86`). The shadow would have to
  change between two methods of one push. That needs pushbuffer parsing plus splitting and waiting
  at submission.
- **GR contexts.** Context buffers also hold VAs of the guest VAS.
- **Fault reporting.** Fault entries would carry shadow addresses the guest does not know.
- **Verdict.** Only kernel-RM-written tables (CPU writes through the trapped BAR2/PRAMIN path)
  could be shadowed. Rejected as an execution model; kept only as an isolation idea for later.

**(c) Hybrid: GR/CE contexts host-side, memory identity-mapped.**
- **What it means.** The host RM allocates and initialises the GR context of the host channel from
  the host's own golden image, instead of promoting the guest's buffers.
- **What it saves.** The device never has to produce a golden context image (GSP's job for buffers
  marked `bInitialize`, `kernel_graphics_object.c:143-158`).
- **What it needs.** The host RM maps its context buffers into the host channel's VAS. With an
  externally owned root this happens in the server-reserved range: the host kernel copies its PDEs
  for its own ranges into the external root (`gvaspaceExternalRootDirCommit`,
  `src/nvidia/src/kernel/mem_mgr/gpu_vaspace.c:2813`, via `dma.c:523`).
- **Why that looks compatible.** The guest also reserves [4 GiB, 4.5 GiB) for "GSP" and never maps
  there itself (`gpu_vaspace.c:303-367`, `:397-420`), so both sides use the same convention
  (inference; this is Open item 1).
- **Its two possible variants.**
  - (c1) Host-allocated contexts, as above.
  - (c2) The guest's context buffers promoted onto the host channel with a host `GPU_PROMOTE_CTX`
    (PRIVILEGED), with the golden image made by the physical GPU through a host channel. That
    requires the host RM to accept a second promotion over the buffers it allocated itself; not
    established.

### 4.2 Reuse of Leandro v1

| Piece | (a) | (b) | (c) |
|---|---|---|---|
| `nvrm-client` `RmClient`, handle allocator, object tree, `alloc_virtual`/`map_gpu`/`map_cpu`/`map_doorbell` (`crates/nvrm-client/src/lib.rs:28-177`, `mem.rs:32-325`) | host channels, VAS, doorbell | yes | yes |
| `nvrm-abi` channel layouts (`crates/nvrm-abi/src/nvgpu.rs:412-700`: `nvos04_flags`, `clc46f`, `NV_CHANNEL_ALLOC_PARAMS` 376 bytes, TSG params) | yes | yes | yes |
| `host_pool::Arena` + OS descriptor + `PinBudget` (`S/host_pool.rs:30-373`, `:486-504`, `:738-772`) | S1 (all guest RAM) and sysmem USERD objects | same | same |
| `vram::Profile`/`decide` (`S/vram.rs:88-104`, `:187-362`) | reservation size `size = fb_length + reservation` | – | same as (a) |
| `mirror::Mirror` (`S/mirror.rs:18-57`) | the FDs host clients ride on | yes | yes |
| event drain / OS-event substitution (`S/session.rs:594-661`, `:885-965`), `waiters::WaiterPoller` (`S/waiters.rs:211-340`) | host non-stall and RC events → guest vectors | yes | yes |
| `Session::prepare/execute` (wire-shaped, [leandro-reuse.md](leandro-reuse.md) §4.2) | not needed: the device issues typed host calls | – | – |
| **New** | guest channel model with a guest→host channel map; doorbell translation; SET_PAGE_DIRECTORY/PROMOTE_CTX flows; reserved-range BAR1 windowing; the S1 IOVA table and guest aid; event translation | a PTE shadow and pushbuffer parser (large) | as (a), plus context-buffer ownership handling |

`nvrm-client` has no VRAM allocation helper and v1 never created a host channel of its own, so
the host channel path is new code on existing layouts.

### 4.3 Recommendation for the first CUDA milestone

**(a) with (c1)**: identity-mapped VRAM, S1 for sysmem, host channels bound to the guest's page
tables, GR contexts allocated by the host RM, doorbells forwarded.

What the golden channel needs then:
- **gpcMask from the host.** The row 41 reply must carry the host floorsweeping masks (assembly
  already specified in `D/kgr.rs:34-52`). gpcMask 0 leaves GR unusable for clients
  (`kernel_graphics.c:487-489`).
- **The golden and WAR rows 56-78** ([phase5-rpcs.md](../history/caraxes-phase5-rpcs-610.md) §1.6) then arrive. Under
  (c1) they can stay record-only: no guest context is ever loaded from the guest's golden buffer,
  because each host channel gets its context from the host RM.
- **The KGR rows currently synthesized** (39-52, `D/kgr.rs`) must stay consistent with the host's
  GR, because CUDA sizes its launches from them. For PROMOTE_CTX and CONTEXT_BUFFERS_INFO the host
  values are required; the rest are host-derived already.
- **PROMOTE_CTX with real FB addresses.** Under (c1) the device records the entries (PAs are in
  the reserved range, i.e. real) but does not apply them. Under (c2) it would apply them on the
  host channel.
- **Global CeUtils and the watchdog** (`RmDisableGlobalCeUtils`, `RmRcWatchdog`) can be re-enabled
  once E5b works. They are the first kernel-originated real work ([open-questions.md](../history/caraxes-open-questions-2026-09-23.md) Q19).

### 4.4 Risks of the recommended model

1. **Untranslated sysmem addresses** (§3) reach the physical GPU. This is the biggest risk. The
   guard: nothing is ever submitted on the physical GPU before S1 is active and verified (E5b's
   first check). The host IOMMU domain must be translated, not identity (host fact H1), so that a
   stray address faults instead of corrupting memory.
2. **No isolation.** Guest page tables can name any VRAM address, including host framebuffers and
   host RM structures outside the reserved range. The GPU has no per-channel physical bounds on
   Turing. Acceptable only for a trusted guest, as in v1.
3. **Host RC.** A guest fault kills a host channel. Host RM recovery (RC) affects only that TSG in
   principle, but it runs on the desktop GPU.
4. **The server-reserved range.** If the host's and the guest's use of [4 GiB, 4.5 GiB) collide,
   context buffers get mismapped (Open item 1).

---

## 5. Interrupt and fault paths

### 5.1 Facts that constrain the interrupt table

| Constraint | Source |
|---|---|
| VREG base 0xB80000 on a PF; LEAF(i) 0xB81000+4i, LEAF_EN_SET 0xB81200, LEAF_EN_CLEAR 0xB81400, TOP 0xB81600, TOP_EN_SET 0xB81608 | `src/nvidia/src/kernel/gpu/arch/turing/kern_gpu_tu102.c:94-101`; `dev_vm.h:28-66` |
| subtree s = leaves 2s, 2s+1; vector v → leaf v/32 | `src/nvidia/arch/nvalloc/common/inc/dev_ctrl_defines.h:46-78` |
| **Stall vectors must sit in even leaves**: TU10x reads only leaf 2s for stall | `src/nvidia/generated/g_intr_nvoc.c:256-265`; `src/nvidia/src/kernel/gpu/intr/arch/turing/intr_tu102.c:849-864` |
| Stall scan covers ESCHED_DRIVEN_ENGINE, UVM_OWNED and UVM_SHARED only; entries outside them break | `intr_tu102.c:908-983`, `:1510-1521` |
| REPLAYABLE_FAULT and ACCESS_CNTR in one leaf; UVM_OWNED mask exactly that subtree; UVM_SHARED one bit; LEAF_EN_SET must read back | `intr_tu102.c:174-276` |
| Non-stall is found through TOP(0) and `LEAF & LEAF_EN_SET` in either leaf of the subtree; W1C | `src/nvidia/src/kernel/gpu/intr/arch/turing/intr_nonstall_tu102.c:398-490`, `:318-383` |
| GSP events need the GSP stall vector **and** falcon IRQSTAT.SWGEN0 routed to host | `src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_tu102.c:1088-1143` |

The device's table today (`D/ctrl.rs:291-320`) satisfies these rules for its six entries: GSP 192,
FIFO 193, TMR 194, REPLAYABLE_FAULT 64, ACCESS_CNTR 65, NON_REPLAYABLE_FAULT 195. It has **no GR
or CE non-stall entries**, and needs (engine indices from `src/nvidia/inc/kernel/gpu/intr/engine_idx.h:54`,
`:128-129`, `:176`):
- GR0 (84) with a non-stall vector in subtree 0 (vectors 0..63);
- CE0..CE2 (15..17), each with its own non-stall vector;
- stall vectors in leaf 6 or `NV_INTR_VECTOR_INVALID` (0xFFFFFFFF), and `pmcIntrMask` invalid so
  that LEAF_EN is set (`intr_tu102.c:108-118`).

### 5.2 Channel completion (blocking sync)

- **The guest chain.** CUDA blocking sync waits on `NV01_EVENT_OS_EVENT`s registered with
  `NV01_EVENT_NONSTALL_INTR` on the subdevice (`src/nvidia/src/kernel/rmapi/event_notification.c:660-760`;
  `src/common/sdk/nvidia/inc/nvos.h:435`). The guest kernel fires them from its own non-stall
  service:
  - GR: `kgraphicsServiceNotificationInterrupt` → `engineNonStallIntrNotify`
    (`kernel_graphics.c:2650-2689`, `event_notification.c:455-461`).
  - CE: the same through `kernel_ce.c:700-720`.
  - The wakeup then follows `postEvent` (`src/nvidia/arch/nvalloc/unix/src/os.c:1541-1560`) →
    `nv_post_event` (`kernel-open/nvidia/nv.c:4049`).
  - **No GSP RPC is involved.**
- **What the device does.** It registers host OS events with the non-stall flag for GR0 and each
  CE on its host subdevice, which v1 does already (8 OS events in `cuda-launch.jsonl`; drain logic
  in `S/session.rs:885-965`). On each host event it sets the matching guest vector's LEAF bit and
  raises MSI (`D/intr.rs` `trigger`).
- **Coarseness.** A host non-stall event is per engine, not per channel, exactly like the hardware
  interrupt. The guest kernel notifies all waiters of that engine, and CUDA re-checks its
  semaphores.
- **What else completes.** Semaphore releases and GPGet updates land in guest memory directly by
  S1 and identity. Polling-mode CUDA therefore works without interrupts.

### 5.3 Errors (RC)

- **The guest's handler.** A host-side RC on a host channel must become
  `NV_VGPU_MSG_EVENT_RC_TRIGGERED` 0x1004 to the guest: handler `_kgspRpcRCTriggered`
  (`src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:585-716`); payload `rpc_rc_triggered_v17_02` with
  engine type, **guest** ChID, exception type and MMU fault address
  (`src/nvidia/generated/g_rpc-structures.h:1481-1496`).
- **The error notifier.** The device also writes the channel's error notifier (guest address from
  the channel params). UVM and CUDA read it, and UVM fails registration cleanly on a non-zero
  status (`uvm_channel.c:2058-2081`).
- **Delivery.** Every GSP event rides the status queue, then the GSP stall vector, then SWGEN0
  (§5.1).

### 5.4 Faults

- **Replayable faults (UVM demand paging, managed memory).**
  - What the hardware does: it writes 32-byte entries (VALID = word 7 bit 31,
    `kernel-open/nvidia-uvm/clc369.h:36-65`) into the buffer GSP programmed, advances PUT
    (0xB8302C) and raises the REPLAYABLE_FAULT vector.
  - What UVM does: it detects pending faults from GET/PUT and the VALID bit, not from the LEAF
    register (`uvm_gpu_replayable_faults.c:312-331`). It services them with CE pushes and replays
    with a `MEM_OP` TLB invalidate method on its own channel, not a register write
    (`uvm_gpu_replayable_faults.c:490-530`; `kernel-open/nvidia-uvm/uvm_turing_host.c:494-525`).
  - Under (a): the physical GPU reports faults of host channels into the **host's** fault buffer,
    owned by the host RM or host UVM. An unprivileged host client cannot read it. So managed memory
    and demand paging are **out of scope** for E5.
  - The device only needs registration to succeed and GET = PUT to stay stable.
  - A future path would need the host to hand the device the fault entries of its channels. There
    is no public control for that (Open item 5).
- **Non-replayable faults.** GSP fills the client shadow buffer registered through 0x20800a9d, then
  sends `MMU_FAULT_QUEUED` 0x1005 (`kernel_gsp.c:1002-1010`, `src/nvidia/arch/nvalloc/unix/src/os.c:877-882`).
  Under (a), a non-replayable fault of a host channel becomes a host RC. The device reports it as
  RC_TRIGGERED (§5.3); filling the shadow queue is optional at first.
- **Access counters.** Not enabled by default (§1.2 row 9). Nothing to deliver.

---

## 6. Milestone ladder

Estimates are rough agent-token budgets including debugging, on the pattern of E3/E4.

| Step | Content | Falsifiable check | Expected effort |
|---|---|---|---|
| **E5a** UVM registers on the fake FB | §1.3 registers and RPCs; interrupt table with GR/CE non-stall entries (§5.1); per-channel state; one usable CE; a **device-side emulation of the few CE methods UVM's init pushes use**: SET_OBJECT, semaphore release, and memset by physical address. Implementation: on a doorbell, read GPPut, the GPFIFO entry and the pushbuffer through the guest VAS using the existing walker (`D/fb.rs` `translate`) and guest RAM; write the payload; advance GPGet. No host GPU involvement | In the guest, `modprobe nvidia-uvm` and a small program issuing UVM_INITIALIZE plus UVM_REGISTER_GPU (or `nvidia-smi` followed by the `cuda-core` probe, which must reach UVM_REGISTER_GPU in its trace) returns success. The device log shows the 3 pushes executed and the SET_PAGE_DIRECTORY root. Falsifier: registration hangs on a push the emulator does not recognise | 200-350k tokens; 600-900 lines |
| **E5b** first physical execution | Host facts H1-H8 (§7.2) settled. Reserved VRAM allocation at a fixed address (operator go-ahead needed: it is an allocation on the host card). S1 with the temporary guest aid. Admin host RM client. Host TSG, channels and CE objects created per guest channel; SET_PAGE_DIRECTORY to the guest root; guest USERD as host memory; doorbell forwarding. The E5a emulator switched off | UVM registration succeeds with every push executed by the physical GPU: GPGet in the guest's USERD advances, and the semaphore payloads appear in guest RAM. Then, as the first check, a guest-visible CE memset into the reserved range is read back through guest BAR1. Falsifier: host IOMMU faults or host Xids in the host log (read-only `dmesg`) | 500-800k tokens; the largest step |
| **E5c** first CUDA context and kernel | Compute objects on host channels with host-allocated contexts (c1); host gpcMask in row 41; golden/WAR rows record-only; the CE and GR non-stall event bridge; RC_TRIGGERED translation | `cuda-core` prints "stage 2 ok" (device memory written, read back, compared). Then a vector-add (`cuda-jit` "stage 3 ok", or a CUDA runtime vectorAdd whose result is checked). Then `cuda-launch` "stage 4 ok" with `CU_CTX_SCHED_BLOCKING_SYNC` (100 launches, `matrix/PROBES.md:37`). Falsifier: a hang in cuCtxSynchronize, meaning a missing non-stall vector | 300-500k tokens |
| **E5d** the gate items | The remaining controls CUDA and PyTorch ask (§2.1 list, checked against the device log); OS descriptors (`cuda-hostreg`); teardown and FREE cascades; per-VM host cleanup | the `gpu` gate's CUDA and PyTorch items of [testing.md](../virtio/testing.md) ("Tables, native/guest NVML, mapping, CUDA, PyTorch results, cleanup, …"): `cuda-torch` "stage 5 ok", run through Leandro-Test on a Leandro VFIO VM. Managed memory (`cuda-managed`) is expected to fail and is reported, not counted | 300-600k tokens |

---

## 7. Open, and host facts

### 7.1 Open

1. **Sharing the server-reserved range.** Does the host kernel's `gvaspaceExternalRootDirCommit`
   write only PDEs for [4 GiB, 4.5 GiB) into the guest root, and does the guest never rewrite those
   entries? (`gpu_vaspace.c:303-420`, `:2813`). This decides whether (c1) works as described or
   whether (c2) is needed.
2. **Golden image under (c2).** Does a host `GPU_PROMOTE_CTX` with `bInitialize` on a host channel
   make host GSP initialise the guest's buffers, and does it override the buffers the host RM
   allocated at object creation?
3. **Guest DMA aid.** How the S1 table reaches the guest: a DMA-ops aid module, or firmware
   `_DMA` ranges. Whether Linux on x86 honours per-device DMA ranges from ACPI is not verified.
4. **IOVA contiguity.** Is an OS descriptor over a large guest RAM range mapped into one host IOVA
   range? This is `contigSegmentSize` in the reply (§3.2).
5. **Replayable faults.** No public host interface hands a client the fault entries of its
   channels. Managed memory stays out until one is found.
6. **Doorbell latency.** Every submission traps through vfio-user. v1 measured 12 µs per control
   call on virtio-nvrm ([virtio-uapi.md](../virtio/virtio-uapi.md)); a sparse-mapped doorbell page cannot
   work, because the token must be translated.
7. **PMA scrub.** Does `PmaAllocPages` depend on the kernel scrubber CE when `RMDisableScrubOnFree`
   is removed (`src/nvidia/src/kernel/gpu/mem_mgr/phys_mem_allocator/phys_mem_allocator.c:663-693`,
   `:1057-1065`)? Not traced.
8. **Instance-block contents on the device side.** Under (a) the guest's instance blocks are never
   run, so the device needs no RAMFC writer. Under E5a's emulator, GPPut is read from USERD
   directly. Confirm that nothing in the guest reads back RAMFC fields.
9. **ECC.** `GPU_QUERY_ECC_STATUS` answered NOT_SUPPORTED. With ECC reported, more RPCs follow
   (`nv_gpu_ops.c:1289-1348`).
10. **No gaps from the surveys.** All three surveys returned (UVM init, interrupt/fault,
    channel path).

### 7.2 Host facts a small read-only query could settle now

Unprivileged controls through `nvrm-client`, the same way `vgpuprofile` runs
(`crates/nvrm-client/src/bin/vgpuprofile.rs`), plus two sysfs reads. None writes to the card; each
must be reported afterwards.

| # | Fact | How | Flags (non-privileged) |
|---|---|---|---|
| H1 | IOMMU domain type of group 28 (translated or identity) | `cat /sys/kernel/iommu_groups/28/type` (sysfs, not the GPU) | – |
| H2 | Host BAR1 size (windowing for the reserved range) | `BUS_GET_PCI_BAR_INFO` 0x20801803, or FB_GET_INFO_V2 index `BAR1_SIZE` (`ctrl2080fb.h:323`) | 0x10518 (`g_subdevice_nvoc.c:10582`) |
| H3 | Free VRAM and the largest free region, i.e. how big a reservation can be | `FB_GET_INFO_V2` 0x20801303 indices `HEAP_FREE` and `LARGEST_FREE_REGION_SIZE_KB` (`ctrl2080fb.h:340-344`) | 0x10118 (`g_subdevice_nvoc.c:9877`) |
| H4 | Host FB region layout (where a fixed range could go) | `FB_GET_FB_REGION_INFO` 0x20801320 (`ctrl2080fb.h:890`) | 0x10108 (`g_subdevice_nvoc.c:9967`) |
| H5 | GPC/TPC masks for row 41 | `GR_GET_GPC_MASK`, `GR_GET_TPC_MASK`, `GR_GET_NUM_TPCS_FOR_GPC`, `GR_GET_ZCULL_MASK`, `GPU_GET_PES_INFO` (`D/kgr.rs:40-49`) | NON_PRIVILEGED per `D/kgr.rs:23-30` |
| H6 | CE caps (SYSMEM/P2P, GRCE) to mirror into `CE_GET_PHYSICAL_CAPS` | `CE_GET_ALL_CAPS` 0x20802a0a | 0x10108 (`g_subdevice_nvoc.c:11797`) |
| H7 | Host interrupt table and non-stall vectors (open 4 in phase5-rpcs.md) | `MC_GET_STATIC_INTR_TABLE` 0x2080170e | 0x10048 (`g_subdevice_nvoc.c:10537`) |
| H8 | Link capabilities at 0x88084 in the captured config space | `caraxes/reference/rtx2070/config-space.txt` (already on disk) | – |

Not settleable read-only, and needing an operator go-ahead:
- whether a fixed-address contiguous allocation of the profile size succeeds (an allocation on the
  host card);
- whether the admin-only controls behave as read (`SET_PAGE_DIRECTORY`, `GPU_PROMOTE_CTX`,
  `GET_SURFACE_PHYS_ATTR`), which needs a privileged host client (`LEA_ADMIN_PRIV`).
