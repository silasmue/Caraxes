<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Managed memory pinned in guest RAM (2026-09-28)

Scope: can `cudaMallocManaged` work in a Linux 615 guest when every GPU mapping is made up front, so that no replayable fault is ever needed? Covers UVM's own policies, the nvidia-uvm module parameters, a preload shim and what the device alone can do. Also a minimal nvidia-uvm no-fault mode for upstream, and the `GSP_GET_FEATURES` answer per guest OS. Follows [uvm-faults-2026-09-28.md](uvm-faults-2026-09-28.md). `U` = `615 tree: kernel-open/nvidia-uvm/`, `K` = `615 tree: src/nvidia/src/kernel/`, `D` = `crates/vfio-user-nvgpu/src/`. Nothing here was run; every result is from the code.

## Facts

### When UVM maps a GPU without a fault

- `SetAccessedBy(GPU)` maps the GPU only to pages that are resident somewhere. A page with no resident copy gets permission NONE and is skipped [U/uvm_policy.c:439-466; U/uvm_va_block.c:11083-11088, 11187-11198].
- So no policy maps a never-touched page of a managed range, and its first GPU touch needs a fault.
- Once a page becomes resident, UVM maps the accessed-by processors in the same operation:
  - after a CPU fault, step 4 of `uvm_va_block_service_finish` [U/uvm_va_block.c:11935-12003, 12064-12070];
  - after `UVM_MIGRATE` [U/uvm_migrate.c:74-147];
  - after an eviction, deferred to a work item [U/uvm_va_block.c:12916-12923, 13121-13151];
  - when the GPU VA space registers later [U/uvm_va_range.c:1372-1412].
- The preferred location gets no remote mapping while the page lives elsewhere [U/uvm_va_block.c:11038-11044].
  - PreferredLocation = GPU: the GPU loses its mapping whenever the CPU pulls the page.
  - PreferredLocation = CPU: the GPU keeps it.
- Read duplication overrides accessed-by and removes remote mappings [U/uvm_policy.c:479-482; U/uvm_va_range.h:1193].
- `UVM_MIGRATE` to the CPU allocates zeroed CPU pages for never-touched pages, then maps the CPU RWA and the accessed-by GPUs [U/uvm_va_block.c:1906-1913, 4835; U/uvm_migrate.c:84-141]. It is synchronous when `flags` = 0 [U/uvm_ioctl.h:544-554; U/uvm_migrate.c:849].
- Every migration first unmaps every processor from the pages that move [U/uvm_va_block.c:4789-4802]. A GPU that cannot fault loses those pages until the migration ends, whether it is a migration to the GPU, a CPU pull-back or an eviction.
- On PCIe the GPU has no native atomics to CPU memory, so accessed-by mappings of CPU pages are RW with `ATOMIC_DISABLE` [U/uvm_va_space.c:921-943; U/uvm_va_block.c:10899-10936; U/uvm_turing_mmu.c:283-286].
- A later CPU fault on a CPU-resident page revokes only the GPU's atomic permission and does not unmap the page [U/uvm_va_block.c `block_service_finish_revoke_prot`].
- The device drops PTE permission bits:
  - `decode_pte` keeps only valid, aperture and address [D/fb.rs:1475-1488];
  - every shadow mapping is `ACCESS_READ_WRITE` [D/hostmem.rs:1579];
  - the host RM sets `ATOMIC_DISABLE` only for TLB-locked mappings [615 tree: src/nvidia/src/kernel/gpu/mem_mgr/arch/maxwell/virt_mem_allocator_gm107.c:2488-2493].
- Inferred: on Caraxes a GPU atomic on such an RW guest mapping executes, device-scope like mapped pinned memory, instead of raising the atomic fault it raises on the card.
- The shadow maps sysmem leaves that lie in registered guest RAM, and the device registers all of guest RAM (the whole-RAM pin) [D/shadow.rs:15-18, 420-462; memory-model.md].

### Module parameters: none maps without a fault

| Parameter | What it does | Use here |
|---|---|---|
| `uvm_fault_force_sysmem` | puts faulted pages in sysmem; works only with `uvm_enable_builtin_tests` [U/uvm_va_block.c:64-65, 158-162, 11303-11307] | none: still needs a fault |
| `uvm_perf_map_remote_on_eviction` | remote-maps evicted pages, only with access-counter migrations [U/uvm_va_block.c:68-69, 164-168] | none |
| `uvm_exp_gpu_cache_sysmem` | L2 caching of sysmem mappings, "may cause correctness issues" [U/uvm_va_block.c:86-89] | none |
| `uvm_global_oversubscription` | lets the RM evict UVM memory [U/uvm_pmm_gpu.c:179-181] | none |
| `uvm_perf_prefetch_*`, `uvm_perf_thrashing_*`, `uvm_perf_fault_*`, `uvm_perf_map_remote_on_native_atomics_fault` | heuristics inside fault servicing [U/uvm_perf_prefetch.c:59-61; U/uvm_perf_thrashing.c:319-331; U/uvm_gpu_replayable_faults.c:70-116] | none |
| `uvm_disable_hmm=1` | turns HMM off, so `pageable.access_enabled` is false and `UVM_PAGEABLE_MEM_ACCESS(_ON_GPU)` return false [U/uvm_hmm.c:30-37; U/uvm_va_space.c:189-203; U/uvm.c:989-995; U/uvm_gpu.c:4234-4253] | yes: pageableMemoryAccess 0 (Inferred: libcuda reads these ioctls) |

- libcuda 615.71.09 contains `CUDA_MANAGED_FORCE_DEVICE_ALLOC` and `CUDA_MANAGED_FORCE_STREAM_ATTACH`, and no variable that turns faults off [`strings` of the host's `/usr/lib/libcuda.so.615.71.09`].
  - FORCE_DEVICE_ALLOC moves data toward VRAM, the wrong direction here.

### What a preload shim can reach

- libcuda imports `mmap`, `mmap64`, `ioctl`, `munmap`, `dlsym` and `syscall` from glibc [`nm -D` of the host's libcuda.so.615.71.09].
- Inferred: an `LD_PRELOAD` library sees every UVM ioctl and every managed-range mmap, however the app links cudart (static cudart, `__managed__` variables, libraries).
- Every managed range is an mmap of the UVM fd with offset == VA. Only two other ranges are mmapped, and both are created by ioctl first: semaphore pools and device-p2p ranges [U/uvm.c:759-858, 793-796].
- The 615 ioctl numbers the shim needs (a per-version ABI, D14) [U/uvm_ioctl.h:405-416, 440-503, 544-615, 927; U/nvCpuUuid.c:26-32]:
  - to set up the mapping: `UVM_REGISTER_GPU` 37 (it returns the GPU UUID), `UVM_SET_PREFERRED_LOCATION` 42, `UVM_SET_ACCESSED_BY` 46 and `UVM_MIGRATE` 51;
  - the ones that would break it: 43 (unset preferred), 44 (read duplication), 47 (unset accessed-by), 51 with a GPU destination, and 80 (`UVM_DISCARD`);
  - the CPU's UUID is `NV_PROCESSOR_UUID_CPU_DEFAULT`.

### The device alone

- The device cannot map a managed page in advance:
  - the guest kernel creates managed ranges by mmap, with no RPC;
  - the guest's UVM picks the backing page when it populates;
  - a page the device picked instead would swallow writes.
- It sees UVM's PTE writes and MEM_OP invalidates only after UVM has decided, and the fault VA only after the RC (uvm-faults note).
- What it can do:
  - the `gspFeatures` answer;
  - keep mapping guest sysmem leaves RW (the atomics above);
  - a post-RC diagnostic: "GPU touched a VA with no guest PTE: managed memory without faults";
  - the handshake with a patched guest (below).

### Results per candidate

| Candidate | CPU write, then GPU read/write | GPU-first touch | GPU atomics on Caraxes |
|---|---|---|---|
| bit 0 = 1, no advice (today) | fault: CPU-resident page without a GPU PTE | fault | - |
| App: PreferredLocation CPU + AccessedBy GPU | works | fault (nothing resident) | work (device scope) |
| App: the same + `cudaMemPrefetchAsync(p, n, cudaCpuDeviceId)` at allocation | works | works | work |
| App: prefetch to the GPU before kernels | works in stream order | works if prefetched | work |
| Module parameters | no change | no change | - |
| Preload shim (Consequence) | works | works | work |
| Device alone | no | no | - |

- Prefetching to the GPU leaves fatal windows:
  - a CPU touch migrates the page back and unmaps the GPU while another stream's kernel may be running;
  - VRAM pressure evicts pages and remaps them only later [U/uvm_va_block.c:4789-4802, 13121-13151].
- libcuda derives `concurrentManagedAccess` itself. No UVM ioctl reports whether a GPU can fault; `UVM_REGISTER_GPU` returns only NUMA information [U/uvm_ioctl.h:405-416].
- Inferred: it stays 1, as on v1. With all data in coherent sysmem, concurrent CPU access is then actually safe.

### A no-fault mode in nvidia-uvm (design)

- NVIDIA once had such a mode. Its only trace is a tools event cause, "remote mapping ... on systems with no GPU fault support (UVM-Lite)" [U/uvm_types.h:1069-1071]. No code for it exists in 595.91.07, 610.43.02, 610.57.04 or 615.71.09 [grep of `/mnt/vmstore/caraxes/ogkm-*`].
- The hooks are half there:
  - `isr.replayable_faults.handling` already guards enable, ISR and procfs [U/uvm_gpu.c:631, 3007, 4273];
  - only init sets it unconditionally, and a failed fault-buffer init fails the GPU [U/uvm_gpu_isr.c:356-363, 382].
- UVM refuses LEGACY vGPU, which is NVIDIA's own case of a GPU without fault delivery [U/uvm_gpu.c:1516-1521].
- An RM switch already exists: `RmDisableHwFaultBuffer` sets `PDB_PROP_KGMMU_FAULT_BUFFER_DISABLED`, which skips the replayable buffer [615 tree: src/nvidia/interface/nvrm_registry.h:1487-1490; K/gpu/mmu/kern_gmmu.c:568-576, 1234-1238].
  - Inferred: today UVM then fails the GPU in `nvGpuOpsInitFaultInfo`.
- The design below is sysmem-only; performance is secondary.

| # | Where | Change |
|---|---|---|
| 1 | `nv_uvm_types.h` `UvmGpuInfo`; K/rmapi/nv_gpu_ops.c `nvGpuOpsGetGpuInfo` | new `replayableFaultsUnavailable`, TRUE on `FAULT_BUFFER_DISABLED` or `IS_VIRTUAL_WITHOUT_SRIOV` |
| 2 | U/uvm_gpu.c parent init; U/uvm_gpu_isr.c `uvm_parent_gpu_init_isr`; U/uvm_gpu_replayable_faults.c `uvm_parent_gpu_fault_buffer_init` | skip the replayable half and leave `handling` false; module parameter `uvm_no_fault_mode` (auto, 0, 1) |
| 3 | U/uvm_va_space.c GPU register and unregister | a `non_faultable_gpus` mask; count them as native-atomic to CPU memory, so sysmem PTEs are RWA and CPU faults never revoke them (mapped-pinned semantics) |
| 4 | U/uvm_va_space.c; U/uvm_gpu.c `uvm_api_pageable_mem_access_on_gpu`; U/uvm_hmm.c | pageable access and HMM off |
| 5 | U/uvm.c `uvm_mmap` → U/uvm_va_range.c `uvm_va_range_create_mmap` | policy PreferredLocation CPU + accessed-by the non-faultable GPUs; populate on the CPU and map the GPUs (the CPU maps itself on its own fault) |
| 6 | U/uvm_va_range.c `va_range_add_gpu_va_space_managed` | the same for ranges created before the GPU |
| 7 | U/uvm_va_block.c `uvm_va_block_add_mappings_after_migration` | never exclude a non-faultable GPU under the preferred-location rule |
| 8 | U/uvm_policy.c, U/uvm_va_range.c | read duplication, PreferredLocation GPU, unsetting such a GPU's accessed-by, `UVM_DISCARD`: refuse, or accept as a no-op |
| 9 | U/uvm_migrate.c `uvm_api_migrate` | GPU destination: a successful no-op by default; migrate and map only as an opt-in, with the windows above |
| 10 | U/uvm_test*.c | a test: fresh range, GPU-first write, CPU read, no fault entry |

- Size: about 12 files, 20 functions, 300 lines of logic plus 150-250 lines of tests. This is an inferred estimate.
- Tools events can reuse `UvmEventMapRemoteCauseCoherence` [U/uvm_types.h:1069-1071].
- Detection for Caraxes:
  - the device reads the guest registry in GMCAPI `GSP_INIT` [D/gmcapi.rs:606-652];
  - a patched guest sets `RmDisableHwFaultBuffer=1` and so announces itself before the first `GSP_GET_FEATURES`.
- A signal from the device to the guest would need a field NVIDIA defines, such as a new `gspFeatures` bit or `replayableFaultBufferSize` 0. Size 0 would break an unpatched guest's UVM (Inferred).
- Contribution policy [615 tree: README.md:81-110; CONTRIBUTING.md]:
  - PRs on GitHub, with a CLA;
  - NVIDIA merges them by hand into its shared code base, one commit per release;
  - for large changes, contact NVIDIA in advance; cosmetic changes are not accepted.
- nouveau and nova are not a venue for CUDA managed memory (Inferred): libcuda needs nvidia.ko and nvidia-uvm, nouveau's SVM is fault-driven, and nova has no compute stack.

### Guest OS and the `GSP_GET_FEATURES` answer

- No kernel code reads `gspFeatures`. The kernel RM takes only `firmwareVersion` [615 tree: src/nvidia/arch/nvalloc/unix/src/osinit.c:542-576; grep of src/nvidia and kernel-open]. Bit 0 matters to userspace only.
- The Windows 617.14 KMD sends 0x20803601 once per boot, at rx seq 178 [caraxes/runs/win11-2026-09-25/night-2026-09-25/X0-rpc-sequence.txt:414-415].
- Bit 0 = 1 changed nothing on the Windows path (Y27) [same folder, WINDOWS-LOOP.md:245-247]. Every later Windows run that started had bit 0 = 1; no run shows that Windows needs it.
- Windows managed memory migrates at kernel launch and has `concurrentManagedAccess` 0, so it needs no replayable faults [CUDA Programming Guide, unified memory on Windows; not checked offline].
- The guest's OS is stated in `NVGMC_SI_HOST_OS_TYPE` (0x0071): OTHER 0, UNIX 1, WINDOWS 2. It is set from the driver's build platform and sent in GMCAPI `GSP_INIT` at rx seq 0 [615 tree: src/nvidia/interface/gmcapi/gmcapi_system_info.h:70-73; K/gpu/gsp/kernel_gsp.c:5010-5013].
- The Linux guest's first 0x20803601 comes at rx seq 134 [caraxes/runs/transport-615-2026-09-24/boot/device-events.txt:29, 554]. The control is CACHEABLE, so the first answer is the one that sticks.
- The device names that key but counts it as "other" [D/gmcapi.rs:412, 517, 652]. The natural hook is `gsp_init` [D/boot.rs:2683-2686].
- The 610 `GspSystemInfo` has no OS field [615 tree: src/nvidia/inc/kernel/gpu/gsp/gsp_static_config.h:191-235].

## Consequence for Caraxes

- Best option without a patch: managed memory pinned in guest sysmem by a preload shim. It needs no fault, not even for a GPU-first touch.
  - Guest: nvidia-uvm option `uvm_disable_hmm=1`, and `libcaraxes-managed.so` in `/etc/ld.so.preload`.
  - In the shim's `ioctl` wrapper, on the UVM fd:
    - record each GPU UUID from `UVM_REGISTER_GPU`;
    - turn 43, 44, 47, 80, and 51 with a GPU destination, into successful no-ops.
  - In the shim's `mmap`/`mmap64` wrapper, after a successful MAP_SHARED mmap of the UVM fd with offset == addr, call on `[addr, addr+len)`:
    1. `UVM_SET_PREFERRED_LOCATION` (CPU UUID, node -1);
    2. `UVM_SET_ACCESSED_BY` (each GPU);
    3. `UVM_MIGRATE` (CPU UUID, flags 0, semaphore 0, node -1).
  - If `UVM_MIGRATE` returns out of memory, unmap the range and return ENOMEM. The allocation then fails cleanly instead of faulting later.
  - Before any GPU is registered, queue the ranges and treat them after `UVM_REGISTER_GPU`.
  - Device: `gspFeatures` bit 0 = 1 for that VM, and c8f9ce08 (the MEM_OP hold) merged. Without it, UVM mappings race the shadow.
  - In one app, the same effect is: `cudaMemAdvise(PreferredLocation, cudaCpuDeviceId)`, `cudaMemAdvise(AccessedBy, dev)`, `cudaMemPrefetchAsync(p, n, cudaCpuDeviceId)`, then a sync.
- What breaks with it:
  - oversubscription: managed data is capped by guest RAM and committed at allocation;
  - `cudaMemPrefetchAsync` to the GPU and ReadMostly are accepted but have no effect;
  - pageableMemoryAccess is 0;
  - system-scope atomics are not atomic against concurrent CPU atomics;
  - every GPU access to managed data crosses PCIe (zero-copy speed);
  - the shim follows the 615 ioctl ABI.
- Atomics depend on the device keeping guest sysmem leaves RW, with permission bits ignored. If the shadow ever mirrors `ATOMIC_DISABLE`, GPU atomics on managed memory fault again.
- With a patch: the no-fault mode above, as a PR against open-gpu-kernel-modules framed as "UVM for GPUs without replayable fault delivery (legacy vGPU, emulated GPUs)".
  - A CLA and a public PR need the licence decision first (Caraxes licence rule).
- Device answer per guest OS:

| `HOST_OS_TYPE` | `gspFeatures` bit 0 | Why |
|---|---|---|
| 2 Windows | 1 | what every started Windows run used; WDDM managed memory needs no replayable faults |
| 1 UNIX | 0 | fails closed: `AllocManaged` returns 801, no host Xid (uvm-faults option 1) |
| 1 UNIX with opt-in (a per-VM switch for shim VMs, or `RmDisableHwFaultBuffer=1` in the registry from a patched guest) | 1 | managed memory with the recipe above |
| 0, key missing, 610 transport | 0 | fail closed |

- Keep `bValid`, `bDefaultGspRmGpu` and the version string as they are [D/ctrl.rs:2172-2187].

## Open

- A VM run of the shim: managedprobe with a CPU-first and a GPU-first touch, an atomicAdd kernel, llama with `GGML_CUDA_ENABLE_UNIFIED_MEMORY=1`; host dmesg must show 0 Xids.
- Whether libcuda issues every managed mmap and UVM ioctl through the glibc PLT; it also imports `syscall`.
- The `HOST_OS_TYPE` value a 617.14 Windows guest sends (one boot), and one Windows boot with bit 0 = 0.
- Whether libcuda with bit 0 = 0 opens nvidia-uvm at all; this decides whether `RmDisableHwFaultBuffer=1` is harmless on an unpatched guest.
- The shadow's map budget (32768 live mappings) against large managed sysmem ranges mapped at 4 KiB [D/shadow.rs:104].
