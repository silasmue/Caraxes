<!-- SPDX-License-Identifier: MIT -->
# Host-side execution primitive (E5b core) and the part-2 wiring spec

Written 2026-09-23. Driver citations are relative to `vendor/open-gpu-kernel-modules/`
(tag 610.57.04); `sdk/` = `src/common/sdk/nvidia/inc`, `D/` = `crates/vfio-user-nvgpu/src/`.
The tool is [caraxes/tools/host-exec.rs](../../caraxes/tools/host-exec.rs), built and run by
[caraxes/tools/host-exec.sh](../../caraxes/tools/host-exec.sh). Evidence is in
`caraxes/runs/host-exec-2026-09-23/` (`run.txt`, `host-before.txt`, `host-after.txt`, all
SPDX-headed).

**What this proves.** As an ordinary RM client (CAP_SYS_ADMIN scoped to two controls), the
physical RTX 2070 (TU106) executes work through a GMMU v2 page table that "the guest" built
in "guest" VRAM: a host channel is bound to a page directory that lives in a reserved VRAM
range, and the GPU walks it. This is the standalone form of the execution model in
[cuda-path.md](cuda-path.md) §4 (option (a): identity-mapped VRAM, host channels bound to
guest page tables, S1 IOVAs for sysmem). It is what the device (`crates/vfio-user-nvgpu`,
another agent's crate) must drive on its mirror objects when the guest boots CUDA.

**Host GPU access.** Only the RM API through `/dev/nvidiactl` and `/dev/nvidia0`. No register
access, no host module change, no passthrough. Every object belongs to one RM client; closing
`/dev/nvidiactl` frees the whole tree in the kernel. A SIGINT/SIGTERM handler closes that fd
and `_exit`s, and `Drop` frees everything on the normal path. Every run's teardown reported
`status=0x0` for every free, `nvidia-smi` memory returned to its prior value, and no `Xid`
appeared in `/dev/kmsg` (checked after every stage, opened as root before the privilege drop).

**Privilege (D12).** Started with `sudo -n`; before opening any node it drops to the invoking
user keeping only CAP_SYS_ADMIN in the permitted set, and raises it into the effective set only
around `NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR` (flags 0x4), `NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY`
(flags 0x14004) and `NV2080_CTRL_CMD_GPU_PROMOTE_CTX` (flags 0x10244). An A-F run opened 5
privileged windows; everything else ran as uid 1000.

## 1. Stages, RM calls, and status codes (measured 2026-09-23)

All statuses are from the recorded `run.txt`. `NV_OK` = 0x0.

### Stage A - reserve VRAM at the top of the FB region, map through BAR1
- `FB_GET_FB_REGION_INFO` 0x20801320: one region `[0x3600000, 0x1e918ffff]`, reserved 0.
  Aligned end **E = 0x1e9000000** (host-facts.md §3). Reservation `[E-64 MiB, E)` = `[0x1e5000000, 0x1e9000000)`.
- `NV01_MEMORY_LOCAL_USER` (0x40), `FIXED_ADDRESS_ALLOCATE`, CONTIGUOUS | HUGE, `offset=0x1e5000000`:
  **`NV_ERR_NO_MEMORY` (0x51)**. The top of the region is occupied - here by the three
  `vfio-user-nvgpu --host-rm` VMs running on the host (`used 5.3 GiB`). Per D11 the device
  **refuses** in this case (the guest FB layout is fixed at device start); this standalone tool,
  which only needs a real contiguous VRAM range, falls back to a floating contiguous 64 MiB
  allocation: **NV_OK**, `phys=0x65800000`, `attr=0x11800000` (CONTIGUOUS, HUGE). This is a
  live confirmation of host-facts.md §5 item 4 (the host does allocate at the top over time).
- `RM_MAP_MEMORY` (0x4E) of the reservation through BAR1: **64 MiB window NV_OK**. A second,
  disjoint 16 MiB window mapped at the same time: **NV_OK** -> several BAR1 windows can be held
  at once. Each window is a fresh `/dev/nvidia0` fd registered to the client ctl fd; the ioctl
  is issued on the main ctl fd, mmap on the target fd (`nvrm-client` `map_doorbell` pattern).
  BAR1 is 256 MiB total, about 232 MiB free (H2); the largest single window here is the whole
  64 MiB reservation.
- CPU write then read-back at reservation +0x300000 through the BAR1 window: **OK** (`0xA5A51234`).

### Stage B - pin sysmem, read its IOVA (admin), build the page table in VRAM
- `NV01_MEMORY_SYSTEM_OS_DESCRIPTOR` (0x71), NONCONTIGUOUS | PCI | CACHED | NO_MAP, over an
  8-page memfd: **NV_OK**. `GET_SURFACE_PHYS_ATTR` (admin) IOVA = **0x7ffe7ffd0000**, aperture 1
  (SYSMEM). A second OS descriptor for USERD: IOVA e.g. `0x7ffe7dfd1000`.
  - The IOVAs are **64-bit** (just under 2^47). The host GPU has already switched to 64-bit DAC
    because the running VMs registered more than 1-2 GiB of guest RAM (sysmem-iova.md §1.4). A
    fresh host would hand out 32-bit IOVAs first. The tool and the device treat the IOVA as
    opaque; the 46/47-bit address fits the GMMU PTE SYS field (bits 53:8).
- GMMU v2 tables built in the reservation through the BAR1 window and validated before use
  (aperture, page alignment, vidmem-in-reservation, sysmem-in-pinned-IOVA-range), then the
  write-combined BAR1 window is flushed by read-back so RM sees the entries:
  - PD3 (root) at reservation +0, PD2 +0x1000, PD1 +0x2000, PD0 +0x3000, PT (4 KiB pages) +0x4000.
  - PDE encoding `(aperture<<1) | ((phys>>12)<<8)`; dual PDE small half for PD0->PT; PTE
    `VALID | (aperture<<1) | ((phys>>12)<<8)` (fb-model.md §1.5, `pascal/gp100/dev_mmu.h`,
    `turing/tu102/dev_mmu.h`). Six PTEs map GPFIFO, pushbuffer, semaphore and data pages
    (SYSMEM_COH, address = IOVA) and one VRAM data page (VIDEO, address = reservation phys).

### Stage C - VA space, channel, page directory, schedule, token, doorbell
- `FERMI_VASPACE_A` (0x90f1) with `IS_EXTERNALLY_OWNED` (RM owns no page tables): **NV_OK**.
- **`DMA_SET_PAGE_DIRECTORY` 0x801813 (admin)**: PDB = reservation phys `0x65800000`,
  `numEntries=1`, aperture **VIDMEM**, `ALL_CHANNELS`, `subDeviceId=1`: **NV_OK**. This is the
  crux: the host RM accepts an externally-owned VA space rooted at a page directory the client
  built in VRAM, from an unprivileged client with CAP_SYS_ADMIN scoped to this one call
  (`gvaspaceExternalRootDirCommit`, `dma.c:427`, `gpu_vaspace.c:2813`). It must be called
  **before** the channel is allocated: a channel cannot bind to an externally-owned VAS that has
  no PDB yet (channel alloc then returns `NV_ERR_INVALID_STATE` 0x40).
- `TURING_CHANNEL_GPFIFO_A` (0xc46f) on that VAS, GPFIFO at a **guest VA** (0x200000), **USERD in
  sysmem** as a client-allocated OS descriptor: **NV_OK**. RM validates the GPFIFO VA by walking
  the VRAM tables; a not-yet-flushed BAR1 write can still surface as `NV_ERR_INVALID_ADDRESS`
  (0x1e), so the tool retries (a device driving BAR1 must fence its page-table writes before the
  channel/PDB calls).
- `TURING_DMA_COPY_A` (0xc5b5) on the channel, `NVB0B5_ALLOCATION_PARAMETERS` **version 1,
  engineType = NV2080_ENGINE_TYPE_COPY2**: **NV_OK**. (Version 0 reads engineType as a CE
  instance; version 1 as the NV2080 ordinal, `clb0b5sw.h`.) `GPFIFO_SCHEDULE` requires an engine
  object (or `NVA06F_CTRL_CMD_BIND`) on the channel first.
- `GPFIFO_SCHEDULE` 0xa06f0103: **NV_OK**. `GET_WORK_SUBMIT_TOKEN` 0xc36f0108: **NV_OK**,
  token e.g. `0x8002c` (chid in 11:0, runlist in 22:16). `VOLTA_USERMODE_A` (0xc361) + doorbell
  map (write-only BAR0 page): **NV_OK**.

### Stage D - submit through the doorbell (the execution proof)
- **Semaphore release.** A one-entry GPFIFO -> a pushbuffer with the NVC46F host methods
  `SEM_ADDR_LO/HI`, `SEM_PAYLOAD_LO/HI`, `SEM_EXECUTE(RELEASE)`, `SEM_ADDR` = the semaphore
  **VA** 0x202000. Ring the doorbell with the token. Result: **the semaphore page reads 0xcafe,
  GPGet advances to 1, latency ~9-17 microseconds.** The physical GPU translated the semaphore
  VA through the page table the client built in VRAM (VIDMEM PD3 -> ... -> a SYSMEM_COH PTE) and
  wrote the pinned sysmem page at its IOVA. This is the decisive walk proof.
- **CE copy.** On the `TURING_DMA_COPY_A` object: the CPU seeds the reserved VRAM through the
  BAR1 window, then a CE `LAUNCH_DMA` reads it at a **virtual** address (OFFSET_IN = VA 0x203000,
  SRC_TYPE VIRTUAL) and writes the sysmem page (VA 0x205000). **Bytes verify (MATCH),
  latency ~22 microseconds** - the CE also walked the guest-built tables to read the reservation.
  - The **first** CE `LAUNCH_DMA` on a freshly scheduled channel stalls once (the channel then
    RC-recovers on the watchdog); every later CE copy runs. The tool issues one warm-up copy to
    absorb that first-launch stall, then the measured copy. This positional stall is a finding
    for the device path (its first CE submission on a new host channel should expect one RC, or a
    priming push should precede real work).
  - A CE **write into** the reservation (sysmem->VRAM, dst VIDEO) via a 4 KiB PTE over the 2 MiB
    contiguous HUGE allocation also stalled; it is left out of the automated path and the CPU
    (BAR1) writes the reserved VRAM instead. The reservation is uncompressed (COMPR default NONE),
    so this is a page-size/aperture interaction to resolve before the device relies on CE writes
    into the reservation (open item 1).

### Stage E - non-stall completion
- eventfd -> `NV_ESC_ALLOC_OS_EVENT` (206) -> `NV01_EVENT_OS_EVENT` (0x79) on the subdevice with
  `notifyIndex = FIFO_EVENT_MTHD (35) | NV01_EVENT_NONSTALL_INTR (0x08000000)` ->
  `EVENT_SET_NOTIFICATION` 0x20800301 action REPEAT: **all NV_OK**. A pushbuffer with a
  `NON_STALL_INTERRUPT` host method is submitted.
- **Finding:** no wakeup arrived on the fd (`poll` returned 0) in either channel state. The
  PBDMA `NON_STALL_INTERRUPT` method raises the HOST-engine non-stall interrupt
  (`eventGetEngineTypeFromSubNotifyIndex`: FIFO_EVENT_MTHD -> RM_ENGINE_TYPE_HOST), and the event
  registration/arming succeeds, but the host RM did not post it to this client's OS event fd for
  a software-triggered method. Real CUDA completion is a **CE/GR** non-stall interrupt raised by
  `LAUNCH_DMA INTERRUPT_TYPE=NON_BLOCKING`, not the host method; the device's event path
  (cuda-path.md §5.2) must register on the engine notifier that the completing engine actually
  raises and confirm delivery (open item 2). The OS-event registration and arming themselves work.

### Stage F - GR channel, compute object, PROMOTE_CTX
- A second externally-owned `FERMI_VASPACE_A` + `DMA_SET_PAGE_DIRECTORY` (admin) + a GR
  `TURING_CHANNEL_GPFIFO_A` (engineType GRAPHICS) + `TURING_COMPUTE_A` (0xc5c0): **all NV_OK**.
- `GPU_PROMOTE_CTX` 0x2080012b (admin), one MAIN buffer entry with `gpuPhysAddr` in the
  reservation: **`NV_ERR_OBJECT_NOT_FOUND` (0x57)**. Acceptance was the finding; the refusal is
  the code the device path must handle. The promotion needs the channel's GR context state RM
  expects to exist (a bound KGR context / the golden-context flow, cuda-path.md §4.3 (c1)); a
  bare externally-owned GR channel with a hand-picked physical address is not enough. Under (c1)
  the device records PROMOTE_CTX entries without applying them and lets the host RM build the GR
  context; this run confirms a naive apply is rejected cleanly (no Xid).

### Stage G - teardown
Every `RM_FREE` returned 0x0; the client free returned 0x0; BAR1 windows, the doorbell mapping
and the memfds were unmapped; `nvidia-smi` free VRAM and pinned-page counters returned to their
prior values; no `Xid`.

## 2. Part-2 wiring spec: what the device must do per guest event

The device (`crates/vfio-user-nvgpu`) holds one host RM client (D10) and, for each guest RM
object, a **mirror** host object. Guest FB addresses inside the reservation are real VRAM
addresses (identity, option C); guest sysmem addresses are host IOVAs once the D13 aid is
installed (sysmem-iova.md). The table below maps each guest-visible event to the exact host RM
call on the mirror, which addresses pass through unchanged, which are translated, and what to
return to the guest.

| Guest event (device sees) | Host RM call on the mirror | Addresses: pass-through vs translate | Return to guest |
|---|---|---|---|
| `GSP_RM_ALLOC` of `FERMI_VASPACE_A` | `RM_ALLOC` `FERMI_VASPACE_A` with **`IS_EXTERNALLY_OWNED`** (nvos.h:3162); do not let RM own page tables | none (VAS has no address yet) | echo the guest's VAS handle; record guest->host VAS map |
| `GSP_RM_ALLOC` of a channel (`TURING_CHANNEL_GPFIFO_A`) | `RM_ALLOC` on the mirror VAS. **USERD**: a host memory object over the guest's USERD page - vidmem USERD -> reservation offset `PA-base`; sysmem USERD -> an OS descriptor over that guest page (accepted here). **GPFIFO offset** = the guest VA, unchanged | GPFIFO/USERD **FB** addresses in the reservation pass through as real VRAM; **sysmem** USERD IOVA is the host IOVA of that guest page (D13) | host ChID and, later, the work-submit token; keep a guest-ChID -> host-ChID and guest-token -> host-token map |
| `GSP_RM_ALLOC` of a CE object (`TURING_DMA_COPY_A`) | `RM_ALLOC` `TURING_DMA_COPY_A`, **version 1, engineType = the host LCE** the device placed the channel on (COPY2..COPY4; not GRCE, H6) | none | echo; needed before `GPFIFO_SCHEDULE` |
| `GSP_RM_ALLOC` of a compute object (`TURING_COMPUTE_A`) | `RM_ALLOC` `TURING_COMPUTE_A` on a GR channel | none | echo (acceptance confirmed) |
| guest `DMA_SET_PAGE_DIRECTORY` RPC (or `VASPACE_COPY_SERVER_RESERVED_PDES`) | `DMA_SET_PAGE_DIRECTORY` 0x801813 **(scope CAP_SYS_ADMIN around this one call)**, aperture **VIDMEM**, PDB = the guest's PDB **FB address, unchanged** (it is a real VRAM address by identity), `ALL_CHANNELS`, `subDeviceId=1`. Call it **before** scheduling channels on that VAS | the PDB FB address passes through unchanged; the PDEs/PTEs inside carry guest FB addresses (real) and guest sysmem IOVAs (host IOVAs by D13) - both correct without rewriting | status only |
| guest `GPFIFO_SCHEDULE` control | `GPFIFO_SCHEDULE` 0xa06f0103 on the mirror channel, after a CE/compute object (or `BIND`) exists on it | none | status |
| guest doorbell write in BAR0's usermode region (`0xBB0090`) | map `VOLTA_USERMODE_A`; decode the guest token (ChID 11:0, runlist 22:16), look up the host token, write it to the host doorbell page (`nvrm-client` `map_doorbell`). Do **not** interpret the GPFIFO | the GPFIFO entries and pushbuffer the GPU then fetches carry guest VAs/addresses that the guest's own page tables (now the host channel's PDB) resolve; the device forwards only the doorbell | nothing (fire-and-forget); completion lands in guest memory |
| guest `GPU_PROMOTE_CTX` RPC | under (c1): **record** the entries (their `gpuPhysAddr` are real reservation addresses) and let the host RM build the GR context from its own golden image; do **not** replay a bare `GPU_PROMOTE_CTX` - it returns `NV_ERR_OBJECT_NOT_FOUND` (measured). Under (c2) it needs the channel's GR context already bound | promote-entry FB addresses are real; do not translate | status the guest expects (NV_OK from the recorded flow) |
| guest non-stall event registration (`NV01_EVENT` + `NV01_EVENT_NONSTALL_INTR` on the subdevice) | register a **host** OS event with the non-stall flag on the notifier the completing engine raises (GR0, CE2..CE4), arm it with `EVENT_SET_NOTIFICATION` REPEAT; on each host event set the guest's LEAF vector and raise MSI (`D/intr.rs`, cuda-path.md §5.2). The HOST/FIFO method notifier did **not** deliver here - use the engine notifier | none | forward as the guest's own vector + MSI |

**Which guest-provided addresses pass through unchanged:** every FB address inside the
reservation `[base, base+L)` - PDB, page-directory and page-table FB addresses, vidmem USERD,
vidmem GPFIFO, promote-ctx buffer addresses. **Which the device must translate:** every sysmem
address - USERD/GPFIFO/pushbuffer/semaphore/method-buffer/fault-buffer addresses in sysmem, which
become host IOVAs through the D13 guest aid (the guest writes IOVAs into its PTEs and RPCs; the
device maps IOVA->GPA for its own reads). **What the device sends back:** host handles are hidden
behind the guest's own handles (echoed), but the **work-submit token** and **ChID/runlist** are
host values the device must map at the doorbell, and completion is an **MSI** the device raises
from a host OS event.

## 3. Numbers, refusals and how they were handled

| Item | Value / status | Note |
|---|---|---|
| FB region end E | 0x1e9000000 | `FB_GET_FB_REGION_INFO`; matches host-facts H4 |
| Fixed-top 64 MiB reservation | NV_ERR_NO_MEMORY (0x51) | top occupied by the running VMs; D11 device refuses, tool falls back to floating |
| Floating contiguous 64 MiB | NV_OK, phys 0x65800000, attr 0x11800000 | CONTIGUOUS, HUGE |
| Largest BAR1 window granted | 64 MiB (whole reservation) | BAR1 256 MiB total, ~232 free (H2) |
| Concurrent BAR1 windows | yes (64 MiB + 16 MiB held together) | each on its own registered nvidia0 fd |
| sysmem OS-descriptor IOVA | 0x7ffe7ffd0000 (64-bit) | host already on 64-bit DAC (sysmem-iova §1.4) |
| `RM_MAP_MEMORY` on subdevice hDevice | NV_ERR_INVALID_ARGUMENT (0x1f) | FB memory is a **device** child; use hDevice = device, target = nvidia0 fd |
| Channel alloc first try (race) | occasional NV_ERR_INVALID_ADDRESS (0x1e) | RM read the BAR1-written tables before the WC flush; fence + retry |
| Channel alloc before SET_PAGE_DIRECTORY | NV_ERR_INVALID_STATE (0x40) | externally-owned VAS needs its PDB first |
| CE object version 0 + NV2080 engineType | NV_ERR_INVALID_CLASS (0x22) | use version 1 for NV2080 ordinal, or version 0 with a CE instance |
| GPFIFO_SCHEDULE before an engine object | NV_ERR_INVALID_STATE (0x40) | allocate the CE/compute object (or BIND) first |
| **DMA_SET_PAGE_DIRECTORY to a VRAM PDB (admin)** | **NV_OK** | the key privileged call; PDB FB address passes through |
| Doorbell semaphore release | NV_OK, semaphore 0xcafe, ~9-17 us | GPU walked the guest-built VRAM page table |
| CE VRAM->sysmem copy (2nd launch) | NV_OK, bytes MATCH, ~22 us | CE also walked the tables |
| First CE LAUNCH_DMA on a fresh channel | stalls once, RC recovers | warm-up push absorbs it |
| CE write into the reservation (4 KiB PTE / 2 MiB alloc) | stalls | left out; CPU writes reserved VRAM (open item 1) |
| Non-stall OS event delivery | registered+armed NV_OK, fd not woken | use the engine notifier, confirm delivery (open item 2) |
| GPU_PROMOTE_CTX (admin), buffer in reservation | NV_ERR_OBJECT_NOT_FOUND (0x57) | record-only under (c1); do not replay bare |
| Teardown | every free NV_OK, memory returned, no Xid | client-close frees the tree |

## 4. Open items

1. **CE writes into the reservation.** A CE `LAUNCH_DMA` with a VIDEO destination through a
   4 KiB PTE over the 2 MiB-HUGE contiguous allocation stalled. The reservation is uncompressed;
   likely a page-size/kind interaction. Resolve (allocate the reservation with 4 KiB pages, or
   set the PTE KIND to the allocation's) before the device relies on CE writes into VRAM. CE
   reads of the reservation and CE writes to sysmem both work today.
2. **Non-stall event delivery.** Register the OS event on the notifier the completing engine
   (GR0, CE2..CE4) raises via `LAUNCH_DMA INTERRUPT_TYPE=NON_BLOCKING`, not the host FIFO method,
   and confirm the fd wakes; then wire it to the guest MSI (cuda-path.md §5.2).
3. **First-launch stall.** The first CE `LAUNCH_DMA` on a freshly scheduled host channel RC-once.
   Establish whether a priming push (a NOP LAUNCH or a CE init) avoids it, or budget one RC.
4. **PROMOTE_CTX under (c1) vs (c2).** This run confirms a bare replay is rejected; the device
   should record entries and let the host RM build the GR context (c1). (c2) - replaying the
   guest's context buffers onto the host channel - remains unestablished (cuda-path.md §4.1).
5. **Fixed-top reservation on a busy host.** With the three vfio-user VMs running, the top 64 MiB
   was NO_MEMORY; the device's D11 refusal path is therefore exercised in practice. Reserve early,
   at device start, before other host GPU work (host-facts.md §5 item 4).
