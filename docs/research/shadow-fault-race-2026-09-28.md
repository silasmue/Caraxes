<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Shadow fault race: nvidia-uvm's MEM_OP invalidates (2026-09-28)

Scope: the host Xid 31 of Linux guest desktop-b at 21:20:24 (PyTorch ResNet-18, `CARAXES_COMPRESSION=slice`, FB 4096 MiB, slot 0), its cause, the fix on branch `shadowrace-2026-09-28`, and the F3 fault-record lookup. `T` = `/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b/gpu-trace.log` (line numbers; times in µs since the device start, 1790621954.632 s Unix by the wall times of the ran-ahead lines, e.g. `T:268742`). Background: [bar1-pool.md §9](../caraxes/bar1-pool.md), [execution.md §8](../caraxes/execution.md).

## Facts

### The fault

- Host kernel log: `Xid 31, pid=725363 vfio-user-nvgpu, GPC0 GPCCLIENT_T1_6 @ 0x7794_b5c03000, FAULT_PDE ACCESS_TYPE_VIRT_WRITE`, 21:20:24.020 [`sudo dmesg`, read-only].
- At the RC, the guest's tables and the worker's post-apply shadow both hold the 2 MiB leaf VA 0x7794b5c00000 → Fb 0x138800000, kind 6 [T:4786366; mirror.rs `fault_diagnostic` reads the shadow through `Job::Coverage`, after every earlier job].
- `Fb()` in the diagnostic is a host FB address. Guest and host addresses are identical in this layout (`reservation == host_reservation`, [0xe9800000, 0x1e9800000)), so 0x138800000 lies 1264 MiB into slot 0 [T:1].
- Kind 6 is `GENERIC_MEMORY`, not compressible, so the compression kind policy does not apply [615 tree: kernel-open/nvidia-uvm/hwref/turing/tu102/dev_mmu.h:113].
- The "1 refused now" of every sync is one 4 KiB kind 0xf leaf at Fb(0) per context, never this leaf [T:5426415 and similar].
- The shadow maps without TLB deferral: every sync shows `invalidate 0` and no `[TLB deferred]` [T:4786339; shadow.rs:1823].
- So a FAULT_PDE at that VA can only happen before the leaf's apply.

### Who maps that leaf, and how the device learns of it

- Root 0xef201000 is the CUDA VA space. It gets 3139 `MEM_OP` TLB invalidates and 0 register invalidates over the whole trace (15 shadow VASes on that root) [`grep -c` over T].
- The register invalidate is the only one that plans and holds a shadow (boot.rs `mmu_invalidate_write` → mirror.rs `shadow_invalidated`, bar1-pool.md §9). For this root that hold never engaged.
- nvidia-uvm's CE channels (chid 11, 13) carry `GP100_UVM_SW` and run on the software executor [T:4786308; mirror.rs `eligible`].
- chid 11 gp 86 does the following in one software pass [T:4786303-4786307]:
  - it writes 896 bytes (56 PDE0 entries, 128 to 183 of the PD0 page 0xef23d000, which is VA 0x7794b0000000 to 0x7794b7000000; the fault VA is entry 174);
  - it invalidates VA 0x7794b0000000 +128 MiB of pdb 0xef201000;
  - it releases UVM's semaphore (0x57).
- That invalidate went to 0 mirror VA spaces and planned no shadow [T:4786309; ctrl.rs software branch; mirror.rs `invalidate_tlb`].
- The next plan of the root came at chid 20's next doorbell (eventfd, its 67th) [T:4786314-4786316].
- The worker's note for that plan is drained at the next trapped BAR0 write: 56 leaves mapped, 56 new region objects, RM mappings 428 → 484, walk 41 µs, apply 3123 µs [T:4786339].
- chid 20's USERD is guest RAM shared with its host channel. There is no `shadow USERD` line, and mirror.rs `untrapped_roots` counts Sys USERDs.
- A loaded host channel fetches a new GPPut without a doorbell (bar1-pool.md §9). This device logged 16 such ran-ahead ERRORs [T:268742 ff.].
- The window: UVM learns that the map is done at 1269272524. The shadow holds the leaf only after the doorbell's apply, stamped 1269275926. For about 3.4 ms the host channel could run PyTorch's next kernels on the new 2 MiB regions.
- After that: error notifiers at 1269291411, the host RC event at 1269291655 [T:4786344, T:4786398].

### Why not on 2026-09-25 [I]

- The matrix guests had no working UVM: `cuda-managed` failed with 801 [caraxes/runs/matrix-2026-09-25/gates-615].
- So CUDA mapped through RM and the register invalidate, which db9bcda0 holds. Those faults sat at RM-style VAs (0x100_xxxx_xxxx) [workload-615/xid31-note.txt].
- The stock 615 guest runs nvidia-uvm (UVM channels, managed memory reaches demand paging), and its allocations are UVM external mappings at CPU-shaped VAs.
- The software `MEM_OP` path is unchanged since 66682ae2. e8f281a9, 7c1ce283, 8f7a7f38, 5b72a19a and the worker changes do not touch it.

### What the trace cannot show

- The fault itself has no device timestamp.
- dmesg wall time maps to device time 1269387995, about 96 ms after the RC scan: the clocks differ by more than the window.
- The ran-ahead counter did not fire: it counts only planned or deferred plans, and unplanned table writes are neither.

## Consequence for Caraxes

- Fix (fail closed): a software `MEM_OP` TLB invalidate that names a shadow root stops its push right after the method. The dispatcher then plans the named roots and waits for the worker. Only then does the push continue, including UVM's semaphore release. Code:
  - channel.rs: `doorbell_holding`, `Stop::HeldAtInvalidate`;
  - mirror.rs: `memop_hold_roots`, `software_invalidated`;
  - untrapped.rs: `memop_holds`, `memop_roots`;
  - ctrl.rs: the software branch of `doorbell_chid_inner`.
- The hold covers every shadow root, not only untrapped ones: a trapped GPPut store waits only for plans already made.
- `CARAXES_SHADOW_UNTRAPPED_HOLD=0` turns it off together with the register rule (A/B only).
- Cost: each UVM mapping waits for its apply under the device lock, like the shadow doorbell's `wait_root` (3.1 ms for 56 fresh 2 MiB regions here). Counted in the exit line: "software MEM_OP invalidate(s) of a shadow root held until applied".
- F3: an RC'd channel whose `GET_MMU_FAULT_INFO` is empty takes the first non-empty record of the other members of its host TSG (mirror.rs `mmu_fault_info_in_tsg`). The trace, the diagnostic and the guest's own `GET_MMU_FAULT_INFO` then show the fault.
- Tests:
  - `channel::tests::a_memop_invalidate_of_a_held_root_releases_nothing_before_the_shadow_is_applied`;
  - `without_a_hold_the_release_comes_before_the_invalidate_is_handed_over` (the old order);
  - `every_held_invalidate_of_a_push_stops_it_once`;
  - `untrapped::tests::a_memop_invalidate_holds_the_shadow_roots_it_names`;
  - `mirror::tests::an_empty_fault_record_is_taken_from_the_tsg_member_that_faulted`.

## Open

- VM validation: convnet 60 s ×3, transformer, llama 1B plain, nvprobe short-kernel.
- If the lock holds show up next to Sunshine: stop the channel, release the lock, resume it once `Progress` reaches the plan.
- The ran-ahead counter should treat dirty, unplanned table pages of the channel's root as pending.
- Catalogue row in [isolation-and-xids.md](isolation-and-xids.md) after the validation.
