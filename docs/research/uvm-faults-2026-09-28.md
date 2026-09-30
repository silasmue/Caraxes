<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# UVM faults and managed memory (2026-09-28)

Scope: why `cudaMallocManaged` in a 615 guest ends in a host Xid 31, how the guest's nvidia-uvm expects replayable faults on TU10x in GSP-client mode, what UVM does without them, how NVIDIA vGPU handles it, what an unprivileged host process can do, and the options. `U` = `615 tree: kernel-open/nvidia-uvm/`, `K` = `615 tree: src/nvidia/src/kernel/`, `SDK` = `615 tree: src/common/sdk/nvidia/inc/`, `REF` = `615 tree: src/common/inc/swref/published/turing/tu102/`, `D` = `crates/vfio-user-nvgpu/`. Run: `caraxes/runs/night-2026-09-28/llama/`.

## Facts

### Tonight's failure

- llama-bench with `GGML_CUDA_ENABLE_UNIFIED_MEMORY=1` allocates with `cudaMallocManaged` and aborts in `ggml_backend_cuda_buffer_set_tensor` (`cudaMemcpyAsync` + sync); without the variable it runs (pp512 7084 t/s, tg128 242 t/s) [small-1b-uvm.txt; small-1b-plain.txt; llama src ggml-cuda.cu:138-166, 782-788].
- Host dmesg (operator's record, 21:16): Xid 31, `ENGINE GRAPHICS GPC1 GPCCLIENT_T1_8`, `FAULT_PDE`, `ACCESS_TYPE_VIRT_WRITE` at 0x7287_aa001000 on the device's pid. The write came from an SM, not a copy engine.
- The shadow VA space is a normal host-managed `FERMI_VASPACE_A`: flags `ALLOW_ZERO_ADDRESS` or 0, never `ENABLE_PAGE_FAULTING` [D/src/shadow.rs:214-220, 1047]. The guest's UVM maps a managed page only when it services a fault or migrates, so the shadow has no PDE there, and the host GR channel takes a non-replayable fault and an RC.
- Before 2026-09-26 the same allocation failed cleanly: `managedprobe: attrs managed=0 concurrent=0 pageable=0`, `AllocManaged a: 801`, 65 runs, no Xid [caraxes/runs/matrix-2026-09-25/gates-615-rerun/gate-cuda-2026-09-25/guest/cuda-managed.rm.out].
- Commit 9ddc665c (2026-09-26 03:04) changed `GSP_GET_FEATURES` `gspFeatures` from 0 to 1, "as the card answers" [D/src/ctrl.rs:2172-2187, the `put32` at :2180; caraxes/reference/rtx2070/kmd-controls-2026-09-26.tsv:130]. Bit 0 is `NV2080_CTRL_GSP_GET_FEATURES_UVM_ENABLED` [SDK/ctrl/ctrl2080/ctrl2080gsp.h:64-80].
- libcuda itself sends 0x20803601: the call is in managedprobe's own trace [cuda-managed.rm.tsv, same folder]. On a GSP client the control is ROUTE_TO_PHYSICAL (flags 0x40549), so the device's answer is what libcuda reads; a vGPU VF answers `bValid` FALSE [615 tree: src/nvidia/generated/g_subdevice_nvoc.c:13356-13361, 14824-14832; K/gpu/subdevice/subdevice_ctrl_gpu_kernel.c:3713-3720].
- v1 forwards the card's own answer (1) and reports managed=1 concurrent=1 pageable=1 [caraxes/runs/cutover-615-2026-09-24/v1/cuda/guest/v1-615/cuda-managed.log].
- Inferred: bit 0 is libcuda's switch for `CU_DEVICE_ATTRIBUTE_MANAGED_MEMORY`. This rests on the before/after correlation and has not been isolated in a single run.

### How the guest's UVM expects replayable faults (TU10x, GSP client)

- Every GPU UVM registers is faultable. `uvm_parent_gpu_init_isr` sets up the fault buffer and sets `replayable_faults.handling = true` without a condition [U/uvm_gpu_isr.c:353-382]. `replayable_faults_supported` no longer exists; the name survives only in a comment [U/uvm_user_channel.h:115].
- Buffer: the guest kernel RM allocates it, in guest sysmem by default (`RmInstLoc4` can move it) [K/gpu/mmu/kern_gmmu.c:1019-1041]. The size comes from `INTERNAL_GMMU_GET_STATIC_INFO.replayableFaultBufferSize`; the device answers 256 entries of 32 bytes [K/gpu/mmu/kern_gmmu.c:1246; D/chips/tu106/chip.toml:149-155]. The PTE array goes to GSP through `INTERNAL_GMMU_REGISTER_FAULT_BUFFER` [K/gpu/mmu/kern_gmmu.c:1256-1296].
- Entry: 32 bytes, with INST_LO/HI (the faulting channel's instance block), ADDR, FAULT_TYPE, CLIENT, GPC_ID, ACCESS_TYPE, REPLAYABLE_FAULT_EN, and VALID in word 7 bit 31 [U/clc369.h:35-66].
- Registers: UVM gets BAR0 pointers into the VF window at 0xB80000 [K/rmapi/nv_gpu_ops.c:9557-9580; K/gpu/mmu/arch/turing/kern_gmmu_tu102.c:188-231]:
  - replayable buffer (index 1): GET 0xB83028, PUT 0xB8302C;
  - non-replayable buffer: GET 0xB83008, PUT 0xB8300C;
  - `PAGE_FAULT_CTRL` 0xB83070;
  - interrupt leaf 0xB81000+4·reg, EN_SET 0xB81200+4·reg, EN_CLEAR 0xB81400+4·reg, for the `REPLAYABLE_FAULT` vector [REF/dev_vm.h:28, 54, 58, 73-113].
- Detection: nvidia.ko's ISR calls UVM on every interrupt [615 tree: kernel-open/nvidia/nv.c:3064-3082]. UVM compares cached GET and PUT, reads the VALID bit, and reads PUT over BAR0 only on the slow path [U/uvm_gpu_replayable_faults.c:312-334].
  - The interrupt is cleared through the leaf register and enabled through EN_SET/EN_CLEAR with `replayableFaultMask`.
  - Writing GET clears OVERFLOW and GETPTR_CORRUPTED [U/uvm_turing_fault_buffer.c:36-134].
- Replay: pushed on UVM's own channel, not a register write. The push is `MEM_OP_A..D` with OPERATION `MMU_TLB_INVALIDATE_TARGETED` (0xA) and `MEM_OP_C.TLB_INVALIDATE_REPLAY` (4:2) = START or START_ACK_ALL [U/uvm_turing_host.c:494-527; U/clc46f.h:130-181].
- Cancel: `GP100_UVM_SW` (0xC076) methods `FAULT_CANCEL_A..C` at 0x104-0x10C, global or targeted; GSP-RM executes them as software methods [U/uvm_turing_host.c:554-590; U/clc076.h:40-52]. VA cancel is `MEM_OP` [U/uvm_turing_host.c:607-].
- Non-replayable faults: GSP copies the "designated" faults into the client shadow buffer (`REGISTER_CLIENT_SHADOW_FAULT_BUFFER`) and sends `MMU_FAULT_QUEUED` [K/gpu/gsp/kernel_gsp.c:1170-1183; K/gpu/mmu/kern_gmmu.c:1770-1842]. CE and host faults are cleared with the `CLEAR_FAULTED` method on Turing [U/uvm_turing_host.c:159-180].
- What the device does today:
  - It records the replayable, shadow and access-counter buffers [D/src/uvm.rs:163-217].
  - It traps GET and PUT of both buffers (PUT reads 0, GET is stored) [D/src/uvm.rs:27-90; D/chips/tu106/chip.toml:51-54].
  - It models the interrupt tree and announces `REPLAYABLE_FAULT` at vector 64 (leaf 2) [D/chips/tu106/interrupts.toml:18-19; D/chips/tu106/controls.toml:75-90].
  - `PAGE_FAULT_CTRL` is only named [D/chips/tu106/regmap.toml:57].
  - UVM's channels (they carry `GP100_UVM_SW`) run on the software executor [D/src/mirror.rs:2733-2747]. There, `MEM_OP` 9/0xA becomes a shadow invalidate, but REPLAY is not decoded [D/src/channel.rs:2277-2311], and UVM_SW methods are ignored [D/src/channel.rs:2085-2088].
  - It never writes a fault entry, never sends `MMU_FAULT_QUEUED`, and never raises vector 64.

### UVM without replayable faults

- No mode exists in 615:
  - no faultable or non-faultable processor mask in the VA space (`uvm_va_space.h`);
  - the ISR path is unconditional (above);
  - a GPU fault can only be serviced in a managed range or an HMM VMA; anything else is `NV_ERR_INVALID_ADDRESS` and fatal [U/uvm_va_block.h:1454-1473].
- `virtMode` = LEGACY: UVM refuses the GPU ("UVM is not supported in legacy virtualization mode") [U/uvm_gpu.c:1516-1521].
  - `virtMode` comes from the guest RM's own `GET_VIRTUALIZATION_MODE`, and on VGX from `KEPLER_DEVICE_VGPU` `VGPU_GET_CONFIG` SR-IOV caps [K/rmapi/nv_gpu_ops.c:7117-7168].
  - Reaching it takes a vGPU-guest personality, which D17 excludes.
- SR-IOV modes still assume hardware replayable faults (the VF's own buffer).
  - Clear-faulted goes through a SW method [U/uvm_gpu_non_replayable_faults.c:262-282].
  - Access counters are off in any vGPU mode [U/uvm_hal.c:715-721].
- No guest-visible answer gives "managed works, no oversubscription". What can work without faults is code that prefetches before the GPU touches the data: `UVM_MIGRATE` to a GPU maps the pages there [U/uvm_migrate.c:651]. Code that simply dereferences managed pointers on the GPU needs faults.
- The only guest-visible switch is libcuda's: `gspFeatures` bit 0 (managed on or off).

### NVIDIA vGPU

- Legacy (time-sliced, pre-SR-IOV, e.g. Turing):
  - the guest allocates no replayable buffer [K/gpu/mmu/kern_gmmu.c:1234-1238];
  - it ignores the replayable interrupt [K/gpu/mmu/arch/turing/kern_gmmu_tu102.c:271-272];
  - UVM classes need the plugin's `UVM_FEATURES_API_ENABLED` [K/vgpu/rpc.c:3913-3923; SDK/ctrl/ctrla080.h:312-315, 409-415];
  - guest UVM refuses LEGACY (above).
- SR-IOV (Ampere+): each VF has its own hardware fault buffer, registered like the PF's [K/gpu/mmu/kern_gmmu.c:1256-1296]. VF PRI faults and targeted cancels go through RPCs to the host [K/gpu/mmu/kern_gmmu.c:2958-2983].
- Public documentation: unified memory on vGPU arrived in 12.2 (R460), off by default (`enable_uvm`), and Linux only.
  - Only time-sliced vGPUs holding the whole framebuffer (later also MIG-backed).
  - The board lists hold Ampere and newer only; no Turing or Volta board appears in 12.x, 13.x or 15.x [NVIDIA vGPU release notes for Linux KVM, RN-09065-001 v12.2 §2.9, v13 §2.9, v15 §2.9; docs.nvidia.com/ai-enterprise/release-8/latest/infra-software/vgpu/features/uvm-support.html].
- NVIDIA never offered managed memory on a Turing vGPU. On SR-IOV parts the faults reach the guest through per-VF hardware, not through the host driver.

### Host side, unprivileged process, unmodified driver

- The replayable fault buffer cannot be owned by the device.
  - `MMU_FAULT_BUFFER` (0xC369) is `RS_FLAGS_ALLOC_KERNEL_PRIVILEGED` and not multi-instance [K/rmapi/resource_list.h:1039-1048; K/rmapi/alloc_free.c:671-679].
  - There is one replayable buffer per GPU [K/gpu/mmu/kern_gmmu.c:1240-1244; the second allocation gets NOT_SUPPORTED]. Host nvidia-uvm holds it once any host process has registered the GPU.
- Fault-capable VA spaces:
  - `ENABLE_PAGE_FAULTING` is accepted from a user client only together with `IS_EXTERNALLY_OWNED` (bug 1766112) [K/gpu/mem_mgr/vaspace_api.c:613-616, 670-684; SDK/nvos.h:3100-3104, 3161-3176].
  - The instance block's `FAULT_REPLAY_TEX/GCC` follow `bIsFaultReplayable` [K/gpu/mmu/arch/volta/kern_gmmu_gv100.c:372-376; K/mem_mgr/gpu_vaspace.c:487-490].
  - Inferred: GSP-RM sets that flag from the VA space's fault capability. The channel code is not in the client tree.
- Non-replayable fault on a normal VAS (today): GSP-RM RCs the channel (Xid 31). Only faults of UVM-registered channels are copied to a shadow buffer.
  - The device sees the fault VA only after the RC, through `NV906F_CTRL_CMD_GET_MMU_FAULT_INFO` [K/gpu/gsp/kernel_gsp.c:1170-1183; D/src/mirror.rs:427-445, 476-477; D/src/rc.rs:430].
- Replayable fault, VAS registered with host UVM (`UVM_REGISTER_GPU_VASPACE`/`_CHANNEL`):
  - Host UVM services it from the device process's memory (managed ranges or HMM) or cancels it. External ranges never fault in [U/uvm_va_block.h:1454-1473].
  - There is no userspace hook: userfaultfd VMAs are refused [U/uvm_hmm.c:573-587], and UVM tools events arrive after the decision.
  - With HMM on, a fault at any device-process VA would map device memory into the VM's GPU context. `UVM_INIT_FLAGS_DISABLE_HMM` exists [U/uvm_types.h:64].
- Replayable fault, externally owned VAS not registered with host UVM:
  - Host UVM finds no channel for the instance pointer (`NV_ERR_INVALID_CHANNEL`), treats the entry as stale, flushes and replays. The access faults again, which is the "hang until the app is killed" of bug 1766112 [U/uvm_gpu.c:3834-3850; U/uvm_gpu_replayable_faults.c:1025-1085; K/gpu/mem_mgr/vaspace_api.c:670-678].
  - The access waits, but nothing tells the device its address.
  - GR on an externally owned VAS is the known set-B wall (Xid 38) [D/src/shadow.rs:4-12].
- Conclusion: an unmodified host gives a userspace process no GPU fault it can hand to the guest and then retry. Faults end in host UVM's own decision or in an RC.

## Options

| # | Option | (a) managed works | (b) beyond the slice | Isolation | Effort |
|---|---|---|---|---|---|
| 1 | **Fail closed**: `gspFeatures` bit 0 = 0 (UVM_ENABLED FALSE), keep `bValid`, `bDefaultGspRmGpu` and the version | no: 801 at allocation, like vGPU without `enable_uvm`; libraries that check the attribute take their non-managed path | no | best: no host fault, no RC, no host Xid | one line in D/src/ctrl.rs:2180, a gate run, one Windows-loop check |
| 2 | Status quo, bit 0 = 1 | only prefetch-first code; the first GPU touch of an unprefetched page RCs the context (Xid 31 on the host) | no | contained per channel (tonight) but host Xids and dead contexts | 0 |
| 3 | FB tail (a P22 variant): the guest sees slice + N GiB, the shadow maps tail pages as sysmem (guest or pinned host RAM); optional hot/cold moves behind the existing idle-and-preempt barrier (bar1-pool machinery) | no (cudaMalloc only) | yes, capacity at PCIe speed; llama spills without the UVM variable | D18 filter unchanged; RAM budget per VM | 2-4 weeks |
| 4 | Re-execute after RC: read the fault VA post mortem, write a synthetic replayable entry for the guest, wait for its replay `MEM_OP`, rebuild the host TSG, resubmit | wrong for non-idempotent kernels (atomics, partial writes); context state lost | in principle | an RC per fault | large; rejected |
| 5 | Host UVM owns the shadow VAS (register VAS and channels, map guest memory as external ranges) | no: host UVM decides from host memory and never asks the guest | host-side only (host VRAM outside the slice) | breaks the slice and VRAM floors; HMM exposes device memory unless disabled | large; rejected |
| 6 | Unregistered fault-capable VAS as a "replay pump" | needs the fault VA while stalled (no source) and GR on an externally owned VAS (Xid 38 wall) | - | spins host UVM's bottom half for every host CUDA client | research only; rejected |
| 7 | Host kernel help (a fault-forwarding host UVM or module), what SR-IOV vGPU gets from hardware | yes | yes | good | excluded by the host rule |

## Consequence for Leandro VFIO

- Recommendation: option 1 now, and declare demand-paged managed memory out of reach on TU106 with an unmodified host. This is the line NVIDIA drew as well: no vGPU unified memory before SR-IOV.
- If memory beyond the slice matters (llama models larger than the slice), option 3 is the route that needs no faults.
- The exact change: `NV2080_CTRL_CMD_GSP_GET_FEATURES` (0x20803601), `gspFeatures` @0 bit 0 `UVM_ENABLED` from 1 to 0 [D/src/ctrl.rs:2180; SDK/ctrl/ctrl2080/ctrl2080gsp.h:78-80]. Expected result:
  - the managedprobe attrs line reads managed=0 again;
  - `AllocManaged` returns 801;
  - the llama UVM run fails at allocation with no host Xid;
  - the cuda gate stays 10 of 11.
- The guest half of fault delivery is cheap if a producer ever exists:
  - write the entry into the recorded buffer, advance PUT, set the leaf bit and raise vector 64;
  - decode `MEM_OP_C` REPLAY in channel.rs and hold the access until then;
  - implement `FAULT_CANCEL_*`.
  - The missing piece is the host side, a fault that waits.

## Open

- Confirm the libcuda switch in one run: managedprobe stage 0 with bit 0 = 0 and = 1 on the same build.
- Whether the Windows KMD needs bit 0. It was set to match the card; the Windows driver has no UVM.
- `concurrentManagedAccess` and `pageableMemoryAccess`: which answer libcuda derives them from once managed is on (not needed for option 1).
- Option 3 sizing: the host RAM budget (31 GB host, RAM-conservative rule) against the tail size, and whether the tail comes out of the guest's own RAM.
