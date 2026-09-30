<!-- SPDX-License-Identifier: MIT -->
# nvidia-uvm on the synthetic device: GPU registration (E5a)

Written 2026-09-23. Driver citations are relative to `vendor/open-gpu-kernel-modules/`
at tag 610.57.04; `D/` is `crates/vfio-user-nvgpu/src/`. The plan is in
[cuda-path.md](cuda-path.md) §1 and §6 (row E5a). The result is in
[experiments.md](../history/caraxes-experiments-2026-09-29.md), section "E5a: UVM registration". The evidence is in
`caraxes/runs/e5-2026-09-23/`.

**Result.** `UVM_REGISTER_GPU` returns `NV_OK` against the host-RAM framebuffer. The
physical GPU is not involved. The device executed every copy-engine push that
registration waits for: 7 in total, on 6 channels. The only precondition is on the
guest side: HMM device-private memory must be off. Either the client passes
`UVM_INIT_FLAGS_DISABLE_PAGEABLE_MIGRATIONS` (flags 1) to `UVM_INITIALIZE`, or
`nvidia-uvm` is loaded with `uvm_disable_hmm=1`. With neither, registration completes on
the GPU side and then fails with `NV_ERR_BUSY_RETRY`. The reason is the VM platform, not
the device (see [HMM device-private memory](#hmm-device-private-memory)).

## The client

`crates/vfio-user-nvgpu/guest/uvm-register.c` is a small C program that is compiled in
the guest. It opens `/dev/nvidia-uvm` and issues `UVM_INITIALIZE`, then
`UVM_REGISTER_GPU` with the UUID from `/proc/driver/nvidia/gpus/*/information`, then
`UVM_UNREGISTER_GPU`. The ioctl layouts are in `kernel-open/nvidia-uvm/uvm_linux_ioctl.h:31-37`
and `uvm_ioctl.h:405-427`. The program passes `rmCtrlFd = -1` and `hClient = 0` (no SMC
partition). `--flags N` sets the `UVM_INITIALIZE` flags word.

`tools/caraxes-vm.sh uvm <label> [flags]` runs the whole check against a running guest.
It copies and compiles the client, reloads `nvidia-uvm` with `uvm_debug_prints=1` plus
`CARAXES_UVM_PARAMS`, runs the client and saves the guest log and the device trace
under `$W/uvm-<label>/`.

Two guest facts, found in iteration 0:
- **The caller must hold `/dev/nvidia0` open.** `nvGpuOpsGetGpuInfo` checks that the
  calling process has the GPU open. Otherwise it returns `NV_ERR_INSUFFICIENT_PERMISSIONS`
  (0x1b) before any RPC (`src/nvidia/src/kernel/rmapi/nv_gpu_ops.c:7671-7674`,
  `kernel-open/nvidia/nv.c:5917-5923`). CUDA always holds it open.
- **The guest image has no `nvidia-modprobe`.** Nothing creates the device nodes, so the
  run creates `/dev/nvidiactl`, `/dev/nvidia0` and `/dev/nvidia-uvm` with `mknod`. The
  UVM major number is taken from `/proc/devices`.

## What UVM sent

### RPCs, in order (run `it3b`; the final build sent the same set in iterations 4 and 5)

Totals: 28 `GSP_RM_ALLOC`, 37 `GSP_RM_CONTROL`, 28 `FREE`. The frees are the teardown
that `UVM_UNREGISTER_GPU` causes. Nothing was unanswered. The first registration after
module load also sends `CE_GET_CE_PCE_MASK` once per CE (run `it3a`); the guest caches
the result after that.

| # | Request | Guest site | Device answer | Label (D9) |
|---|---|---|---|---|
| 1 | client triple (ROOT, DEVICE, SUBDEVICE), twice | `nvUvmInterfaceGetGpuInfo`, `nvUvmInterfaceDeviceCreate` (cuda-path.md §1.2 rows 2 and 4) | echo, objects recorded | synthesized |
| 2 | `GPU_QUERY_ECC_STATUS` 0x2080012f | `nv_gpu_ops.c:1344-1348` | the host's own answer, `NV_ERR_NOT_SUPPORTED` (accepted) | host |
| 3 | `INTERNAL_GMMU_REGISTER_FAULT_BUFFER` 0x20800a9b, size 0x2000 | `kern_gmmu.c:1264` | OK. Size and PTE[0] recorded | guest |
| 4 | `INTERNAL_GMMU_REGISTER_CLIENT_SHADOW_FAULT_BUFFER` 0x20800a9d | `kern_gmmu.c:1818` | OK. Queue PA and size recorded | guest |
| 5 | `INTERNAL_UVM_REGISTER_ACCESS_CNTR_BUFFER` 0x20800a1d, size 0x2000 | `src/nvidia/src/kernel/gpu/uvm/uvm.c:215-255` | OK. Recorded | guest |
| 6 | `FERMI_VASPACE_A` alloc | `nvUvmInterfaceAddressSpaceCreate` | echo. VAS recorded | guest |
| 7 | `VASPACE_COPY_SERVER_RESERVED_PDES` 0x90f10106 | `gpu_vaspace.c:4963` | OK. levels[0] (FB 0x0, 0x20 bytes, aperture video) becomes the VAS root. The server range is [4 GiB, 4.5 GiB) | guest |
| 8 | `CE_IS_DECOMP_LCE_ENABLED` 0x20802a12, 10 times (LCE 0-4, twice) | `kceIsDecompLce_IMPL`, `kernel_ce.c:1172-1197` | false. The control is privileged (flags 0x154), so it is not forwarded | synthesized |
| 9 | `CE_GET_PHYSICAL_CAPS` 0x20802a07, CE0-CE4 | `kernel_ce.c:556-575` | the host's `CE_GET_CAPS_V2` 0x20802a03 for the same engine type (same layout). CE0/CE1 report 0xe3 0x03 (GRCE, shared); CE2-CE4 report 0xe0 0x03 | host |
| 10 | `CE_GET_CE_PCE_MASK` 0x20802a02, CE0-CE4 (first registration only) | `nv_gpu_ops.c` `queryCopyEngines` | forwarded as-is (public, flags 0x30349) | host |
| 11 | `CE_GET_FAULT_METHOD_BUFFER_SIZE` 0x20802a08, once per TSG | TSG construction | 32 KiB (existing answer) | synthesized |
| 12 | `KEPLER_CHANNEL_GROUP_A` alloc, 3 times (engine types 0xb, 0xc, 0xd = COPY2-4) | `nvUvmInterfaceTsgAllocate` | echo. TSG → VAS recorded | guest |
| 13 | `INTERNAL_PROMOTE_FAULT_METHOD_BUFFERS` 0xa06c010a, per TSG | `kernel_channel_group_api.c:426-505` | OK. Recorded only | guest |
| 14 | `TURING_CHANNEL_GPFIFO_A` alloc, 6 times (2 per TSG) | `_kchannelSendChannelAllocRpc`, `kernel_channel.c:2588-2770` | echo. ChID, GPFIFO VA and entry count, USERD, error notifier and VAS recorded | guest |
| 15 | `GP100_UVM_SW` and `TURING_DMA_COPY_A` alloc per channel | `nv_gpu_ops.c` `channelAllocate`/`engineAllocate` | echo | synthesized |
| 16 | `GPFIFO_SCHEDULE` 0xa06f0103 per channel | `engineAllocate` | OK. The emulated channel runs on its doorbell | synthesized |
| 17 | `DMA_SET_PAGE_DIRECTORY` 0x801813, root FB 0x200000, flags 0x8 (all channels) | `nvUvmInterfaceSetPageDirectory`, `dma.c:497-530` | OK. The VAS root moves to UVM's page tree | guest |
| 18 | teardown: frees, `DMA_UNSET_PAGE_DIRECTORY` 0x801814, the three unregister controls 0x20800a9e/0x20800a9c/0x20800a1e | `UVM_UNREGISTER_GPU` | OK | guest |

The guest RM decides the channel IDs (1-6) and puts them in the USERD index fields of the
allocation flags (`kernel_channel.c:2704-2718`). The runlists (1, 2, 3) come from the
synthesized FIFO device-info table in `D/ctrl.rs` (CE2-CE4). UVM uses no GRCE: CE0 and
CE1 report GRCE and SHARED in the host caps it received. Each channel's GPFIFO
(1024 entries) and USERD are in vidmem; the pushbuffers and semaphores are in sysmem
(`uvm_channel.c:3380-3394`).

### Registers

In the run `it3b`, not counting the boot path:
- **TLB invalidates.** 0xB830A0/A4/B0 are written about 110 times and polled; TRIGGER reads back as done (`fb-model.md` §4d).
- **Interrupt enables.** LEAF_EN_SET(2) 0xB81208 and LEAF_EN_CLEAR(2) 0xB81408 are each written about 300 times with 0x2 or 0x3: the kernel masks and unmasks the UVM leaf around its interrupt servicing. UVM itself sets bit 0 (vector 64, REPLAYABLE_FAULT) once, clears it once at teardown, and clears LEAF(2) bit 0 twice with a write-1-to-clear.
- **Replayable fault buffer** (`uvm_turing_fault_buffer.c:36-128`): GET 0xB83028 written 0xc0000000 then 0; PUT 0xB8302C read (0); PAGE_FAULT_CTRL 0xB83070 read-modify-write.
- **Access counter buffer size** 0xB83110: read once, answered 0x100.
- **Link capabilities** 0x88084: read once, 0x453d03 from the config space (speed code 3).
- **Usermode timer** TIME_1/TIME_0 0xBB0084/0xBB0080: 12 and 6 reads.
- **Doorbell** 0xBB0090: 7 writes. The tokens are 0x10001, 0x10002, 0x20003, 0x20004, 0x30005 (twice) and 0x30006.

### Pushes

| Push | Channel | Methods (decoded by the device) | Effect |
|---|---|---|---|
| `channel_init` (`uvm_channel.c:2546-2563`), 6 times | ChID 1-6, 10 dwords each | host `SET_OBJECT` 0xc5b5 on subchannel 0 (`uvm_turing_ce.c:29-38`); `SET_OBJECT` 0xc076 on subchannel 5 (`uvm_turing_host.c:30-34`); CE `SET_SEMAPHORE_A/B/PAYLOAD` then `LAUNCH_DMA` one-word release (`uvm_turing_ce.c:61-81`) | tracking semaphore at VA 0x121000000 + 4 × (ChID - 1) = 1 |
| page-tree init (`uvm_mmu.c:1181-1187`) | ChID 5, 26 dwords | inline data carried in a host `NOP` (non-incrementing); CE memcopy of 32 bytes from that inline data to FB 0x200000 by physical address (the PD3 root, `uvm_mmu.c:398-432`); CE semaphore release | root PDEs invalid; semaphore = 2 |

The CE semaphores are released with the four-word form only for timestamps, and the host
semaphore methods only for other push types. Neither occurred during registration. Both are
implemented and unit-tested.

## What the device emulates now

All of this is in `crates/vfio-user-nvgpu/src`. Every value it produces carries a label
in the trace.

- **`channel.rs` (new), the channel emulation.**
  - *Registry.* It records the guest's VA spaces, TSGs and channels from the allocation
    parameters: `NV_CHANNEL_ALLOC_PARAMS` offsets are taken from the `RmAbi` constants of
    `nvrm-sys` for 610.57.04 (`sys::DefaultAbi`). The VAS roots come from `COPY_SERVER_RESERVED_PDES` and
    `DMA_SET_PAGE_DIRECTORY`/`UNSET`.
  - *Doorbell* (BAR0 0xBB0090). The token decodes as ChID in bits 11:0 and runlist in
    bits 22:16 (`tu102/dev_ctrl.h:36-37`). The device reads GPPut from USERD +0x8c and
    executes each GPFIFO entry from GPGet to GPPut, resolving VAs through the channel's
    VAS.
  - *Pushbuffer decoding.* The method stream uses `clc46f.h:285-358`: INC, NON_INC,
    IMMD, ONE_INC and END_SEGMENT. Control entries of length 0 are skipped.
  - *Host methods.* `SET_OBJECT`, `NOP`, `MEM_OP_A-D` (membar and TLB invalidate, which
    are no-ops because there is no cache), `SEM_ADDR/PAYLOAD/EXECUTE` (release 32/64-bit
    with optional timestamp, acquire EQ/STRICT_GEQ/CIRC_GEQ), `WFI` and
    `NON_STALL_INTERRUPT` (logged, not raised).
  - *Blocked acquires.* An acquire that is not yet satisfied stops the channel at that
    method. The channel resumes on a later doorbell of any channel.
  - *CE methods* (`TURING_DMA_COPY_A` subset). Semaphore A/B/payload,
    `SET_SRC/DST_PHYS_MODE`, `OFFSET_IN/OUT`, `PITCH_IN/OUT`, `LINE_LENGTH_IN`,
    `LINE_COUNT` and the remap constants. `LAUNCH_DMA` does a copy (virtual or physical,
    FB or sysmem, single or multi-line), a constant memset through remap (component size
    1-4, 1-4 components), and a one-word release (with reductions) or a four-word
    release.
  - *Completion.* The device writes GPGet back to USERD +0x88.
  - *Subchannel 5.* Methods to the SW object (GP100_UVM_SW) are logged and ignored.
- **`fb.rs`.** `translate_in` walks page tables that live in guest RAM as well as in the
  fake FB. PDE and PTE sysmem address fields are from `pascal/gp100/dev_mmu.h:98-139`.
  BAR1/BAR3 keep the FB-only walk.
- **`uvm.rs` (new).** It answers the controls in the table above and records the fault
  and access-counter buffers.
  - `VfRegs`: fault-buffer GET/PUT (GET keeps its pointer; the write-1-to-clear status
    bits read 0; PUT is read-only 0), access-counter SIZE (256 entries), GET/PUT/INFO,
    and the usermode timer TIME_0/TIME_1 from the device clock (`dev_vm.h:74-113`,
    `:207-227`).
- **`msgq.rs`.** A `GSP_RM_ALLOC` counts only its fixed part in the RPC length. The
  allocation parameters follow it in the same queue slot (`rpc.c:11002-11025`), and the
  whole slot is copied (`message_queue_cpu.c:503`). `Message::slot_payload` now carries
  them.
  - Before this change, every allocation was answered with zeroed parameters, which the
    guest copies back (`rpc.c:11039-11047`). Channels were recorded with ChID 0 and no
    USERD (iteration 1).
  - The echo now returns the guest's own parameters, as real GSP does for these classes.
- **`boot.rs`, `ctrl.rs`, `bar0.rs`.** The doorbell and `VfRegs` offsets are routed to
  the new code. The dispatcher records the objects and calls `uvm::answer` before
  `nvml::answer`. The register names are added.

Unit tests: two channel tests use the file-backed guest-memory fixture of the `msgq.rs`
tests and the fake FB.
- A UVM-shaped push: SET_OBJECT, a physical memset_8, WFI, membar, a CE semaphore
  release, a virtual copy, and a host release with timestamp.
- A cross-channel acquire that blocks and then resumes.

There are also tests for reductions, `VfRegs`, the UVM control answers, and the
doorbell path through `BootModel`.

## Interrupts

Registration needed no new vectors. UVM enables and clears REPLAYABLE_FAULT (64) and
ACCESS_CNTR (65) in leaf 2 (`uvm_turing_fault_buffer.c:36-66`). The existing table in
`D/ctrl.rs` already satisfies `intrStateLoad_TU102` (cuda-path.md §5.1).

The device raises nothing for a push:
- UVM's registration pushes request no interrupt.
- `NON_STALL_INTERRUPT` and `LAUNCH_DMA` interrupt requests are logged only.

E5c needs the GR0 and CE non-stall entries of cuda-path.md §5.1 for blocking sync. The
table was left unchanged here to keep the milestone-1 boot path as it is.

## HMM device-private memory

With `UVM_INITIALIZE` flags 0 and HMM available, the first registration of a GPU
reserves a free physical address range the size of the GPU memory for device-private
pages:
- `uvm_devmem_init` → `devmem_alloc_pagemap` → `request_free_mem_region`
  (`kernel-open/nvidia-uvm/uvm_devmem.c:514-539`, `:217-226`), called from
  `uvm_gpu.c:2926-2934`.

In the Cloud Hypervisor guest the call fails with -34 (ERANGE), and the error becomes
`NV_ERR_BUSY_RETRY`. The reason is in the guest's `/proc/iomem`:
- The ACPI host bridge window "PCI Bus 0000:00" covers 0x140000000-0x3fffffeeffff.
- That is all physical address space above RAM, up to the 46-bit limit.
- No 4 GiB range is left outside it.

On the host there is space above the PCI window. This is a property of the VM
platform, not of the device. Registration had already passed channel creation, the
page-tree push and `SET_PAGE_DIRECTORY` when it failed.

There are three ways around it. None of them patches the driver:
- The client passes `UVM_INIT_FLAGS_DISABLE_PAGEABLE_MIGRATIONS`. This disables
  `enable_devmem` (`uvm_va_space.c:805`).
- The module parameter `uvm_disable_hmm=1` makes `uvm_devmem_init` skip the pagemap
  (`uvm_hmm.c:130-146`, `uvm_devmem.c:522-526`).
- The VMM leaves a free range, which needs a smaller 64-bit PCI window in Cloud
  Hypervisor.

## What blocks E5b

Seen in these runs, in addition to cuda-path.md §7:

1. **The guest's VRAM layout starts at FB 0.** The kernel VAS root is at FB 0x0, and
   UVM's page tree, GPFIFOs and USERDs are at 0x30000-0x200000. Under option C
   ([fb-model.md](fb-model.md) §5), FB address X must be host VRAM X inside a host
   reservation. The guest's region table therefore has to move the usable range to the
   reservation's base before a single push can go to a host channel.
2. **Sysmem addresses are guest physical.** The pushbuffers (VA 0x120000000 onward)
   and the tracking semaphores (VA 0x121000000) are guest RAM, and their PTEs carry
   guest physical addresses. The device reads them directly today; the physical GPU
   would need requirement S (cuda-path.md §3).
3. **Channel identity has to be mapped.** The ChIDs 1-6 and the runlists 1-3 are guest
   values from the synthesized FIFO table. The host channels get their own IDs, so the
   device needs a guest-token → host-token map at the doorbell (cuda-path.md §4.1 (a)).
4. **CE choice.** UVM selected COPY2-COPY4 because the mirrored host caps mark CE0/CE1
   as GRCE. Host channels must be created on the same host LCEs, or the caps must stay
   consistent with where the device places them.
5. **Execution is synchronous.** The device runs a push inside the trapped doorbell
   write, on the single vfio-user thread. With the physical GPU, completion is
   asynchronous: the semaphores and GPGet land in guest memory later, and interrupts
   must be forwarded (item 6).
6. **No non-stall interrupts.** Neither the interrupt table nor the device has CE/GR
   non-stall vectors yet (see [Interrupts](#interrupts)).
7. **HMM devmem** (above). If CUDA calls `UVM_INITIALIZE` with flags 0, E5c needs
   `uvm_disable_hmm=1` in the guest or a VMM change. That is to be checked in the
   `cuda-core` trace.

## Open

- Whether the kernel VAS root at FB 0x0 (levels[0] of `COPY_SERVER_RESERVED_PDES`) is
  the real root of the device VAS, or a server-mirror level. The emulation translated
  the GPFIFO, pushbuffer and semaphore VAs correctly through it, and after
  `SET_PAGE_DIRECTORY` through UVM's root at 0x200000.
- `GPU_QUERY_ECC_STATUS` is forwarded, and the host answers `NOT_SUPPORTED`. Real TU106
  without ECC answers the same; with ECC reported, more RPCs follow (cuda-path.md §7.1
  item 9).
