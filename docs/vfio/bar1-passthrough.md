<!-- SPDX-License-Identifier: MIT -->
# P21: BAR1 window passthrough

Written 2026-09-24 for idea P21 (README.md, ideas table; operator's order "das Fenster brauchen
wir"). Spec first: every claim names a file and line. Sources:

- NVIDIA open kernel modules 610.57.04: `vendor/open-gpu-kernel-modules` (`kernel-open/...`,
  `src/nvidia/...`).
- vfio-user specification: `vendor/qemu-vfio-user/vfio-user.rst`.
- Cloud Hypervisor v53: stock in `vendor/cloud-hypervisor`, and with patch 0005 in
  `/mnt/vmstore/caraxes/ch-0005`.
- vfio-user crate: `vendor/vfio-user-rs/src/lib.rs`.
- Device traces: `caraxes/runs/`.

The prototype (§6) is in `crates/vfio-user-nvgpu` behind `CARAXES_BAR1_PASSTHROUGH=1`. It builds
clean, the unit tests pass, and clippy runs clean with the flags of `tools/check.sh`. It has not run
on a VM or on the host GPU. The measurement commands for the coordinator are in §8.

## Summary

- **The guest's BAR1 traffic is tiny and regular.** Every trace in `caraxes/runs` shows the same
  pattern:
  - only 4-byte CPU writes, and no reads;
  - only libcuda's GPFIFO entries and USERD GPPut;
  - only in guest BAR1 `[0x80000, 0xbc000)`;
  - a single linear piece: guest BAR1 offset X is FB `reservation base + 0x980000 + X`.

  Page tables go through BAR2 (region 3), never through BAR1 (§1).
- **Cloud Hypervisor maps sparse areas once and never changes them.** It maps each area once, when
  the device is created, before the guest runs. It mmaps the region's single descriptor at
  `region offset + area offset`, with an overflow check. It never asks again, and the
  specification has no message that would change an area later (§2).
- **The NVIDIA RM mapping descriptor fits only one case.** It takes one mapping context per
  descriptor, refuses `vm_pgoff != 0`, and requires the exact size. So it can back only one area at
  guest BAR1 offset 0. It also does not keep its host BAR1 range alive for other processes'
  mappings (§3).
- **A dma-buf of the reservation is the right descriptor.** The NVIDIA dma-buf of reservation
  pieces allows mmap at any page offset for vidmem, is open to a plain RM client, and holds the
  memory and its host BAR1 mapping for as long as any mapping of it exists (§4 B).
- **What remains is correctness, not isolation.** The area's backing is fixed before the guest
  boots, and the guest chooses its own BAR1 page tables. Two ways out (§4 G, H):
  - the prototype matches the layout the guest has always used and checks it at every change;
  - the target design has the device give each guest page that the guest maps into the area
    one of the area's own pages, taken from a private pool.

## 1. What the guest driver does with BAR1 (Turing, GSP client mode)

**The guest's BAR1.**
- The device offers a 256 MiB 64-bit prefetchable BAR1, like the real card (`board.rs:207-220`).
- A trapped BAR1 access at region offset X is a BAR1 virtual address X. The device walks it
  through the guest's own BAR1 page tables, rooted at `bar1PdeBase`:
  - `boot.rs:310-325` (`bar_root`);
  - `device.rs` `region_read`/`region_write`, arms `1 | 3`;
  - `fb.rs` `bar_translate`, `bar_access`.
- No hardware ever walks the guest's BAR1 tables. Guest BAR1 is emulated in full.

**Who manages BAR1 in the guest.**
- The kernel RM (CPU RM) owns the BAR1 address space, also in GSP client mode (`kbusInitBar1_GM107`,
  `src/nvidia/src/kernel/gpu/bus/arch/maxwell/kern_bus_gm107.c:879`).
- CPU mappings of vidmem (`RM_MAP_MEMORY`) get a BAR1 virtual address that the RM's own allocator
  picks (`_kbusMapAperture_GM107`, `kern_bus_gm107.c:3378-3401`: `dmaAllocMapping_HAL` on the BAR1
  VAS).
- A fixed BAR1 offset exists only as an internal RM flag: `BUS_MAP_FB_FLAGS_MAP_OFFSET_FIXED`
  becomes `NVOS46 _DMA_OFFSET_FIXED` (`src/nvidia/src/kernel/gpu/bus/kern_bus.c:965-968`). Its only
  users are the static BAR1 mapping and the USERD remap (`kern_bus_tu102.c:539`,
  `kernel_fifo_tu102.c:525-532`).

**Static BAR1.**
- It exists for TU102-TU117 (`generated/g_kern_bus_nvoc.c:1024-1034`, `kbusIsStaticBar1Supported_TU102`).
- It is enabled automatically only when BAR1 covers the whole FB address space, plus USERD for every
  channel, plus MMIO and doorbell pages (`kern_bus_tu102.c:367-497`).
- The guest is told an FB address space of 8 GiB (memory-model.md §1), against a BAR1 of 256 MiB. So
  the guest runs with dynamic BAR1, which is what the traces show.
- Static BAR1 would map FB address X at BAR1 `startOffset + X` (`kern_bus_tu102.c:525-600`,
  `kern_bus_gm107.c:1162-1172`). That puts the reservation at guest BAR1 offsets of 3.6 GiB and more
  (§4 D).

**RM-owned USERD.**
- It is allocated once for all channels and mapped through BAR1 top-down
  (`kfifoPreAllocUserD_TU102`, `src/nvidia/src/kernel/gpu/fifo/arch/turing/kernel_fifo_tu102.c:487-531`
  and `:655`).
- In the traces the guest CPU never touches it: libcuda brings its own USERD (below).

**Trace evidence.** `caraxes/runs/.../*trace*` were counted with a small script: the region
column, the operation, the size, and the `fb` target of every `bar1` line.

| Trace | bar1 accesses | bar3 (BAR2) accesses | Kinds | BAR1 offsets | FB minus BAR1 offset |
|---|---|---|---|---|---|
| `mapping-2026-09-24/c0-doorbell-study/short-trace.txt` (nvprobe 4) | 978 | 96676 | W 4 bytes only | `[0x80000, 0xbb08c]`, 40 pages | `0x169980000`, one value |
| `mapping-2026-09-24/p3-profile/init-trace.txt` (context create) | 642 | 96665 | W 4 only | first MiB | `0x169980000` |
| `mapping-2026-09-24/b1-async-streams/.../suite-test_async_streams.full.trace` | 6240 | 2287667 | W 4 only | `[0x80000, 0xbb08c]`, 41 pages | `0x169980000` |
| `e5b2-2026-09-24/d-gr-mirror1/device-trace.txt` (4 GiB profile) | 666 | 96758 | W 4 only | first MiB | `0xe9980000` |
| `e5b2-2026-09-24/d-launch4-1/device-trace.txt` (4 GiB profile) | 978 | 96676 | W 4 only | first MiB | `0xe9980000` |

**Reading the traces.**
- The reservation bases are `0x169000000` (2 GiB, slot 0) and `0xe9000000` (4 GiB). In all five
  runs, and with both profiles, guest BAR1 X maps FB `base + 0x980000 + X`.
- There is one libcuda buffer per channel, 0x3000 bytes:
  - offset `+0x0`: the GPFIFO entry, low and high word;
  - offset `+0x208c`: GPPut, the USERD at `+0x2000`.

  Example: `short-trace.txt`, `W bar1 0x000b0000 ... fb 0x169a30000`, then `0x000b0004`, then
  `0x000b208c`. That is the accounting of mapping.md §6: three of the four trapped writes of a
  submission are BAR1 writes.
- BAR1 is never read. No small copies through BAR1 appear in these workloads.
- The guest RM's page-table traffic is the 96k BAR2 writes of a context creation (mapping.md §3),
  not BAR1.

**D17 check (the shadow sees table writes through BAR2, not BAR1).**
- The incremental walk marks a table page dirty only in `FakeFb::write` (`fb.rs:862`, `note_write`
  `fb.rs:746`).
- Every write to a table page reaches that function today, because BAR1, BAR2 and PRAMIN are all
  trapped (mapping.md §1).
- All BAR1 writes in the traces land in the channel-buffer range above, not on a table page.
- With passthrough, writes inside the area no longer reach `FakeFb::write`. So a guest table page
  that is reachable through the area would go stale in the cache.

  The prototype checks for exactly that (§6), and the risk is correctness only:
  - every host channel runs on a shadow VA space whose leaves the device validated
    (mapping.md §5(b));
  - `resolve_leaf` refuses compressible kinds and anything outside the reservation, whatever the
    guest wrote (`shadow.rs:93-103`).

## 2. Cloud Hypervisor v53, vfio-user client, sparse mmap areas

**Region info is read once.**
- The client asks for every region's info once, when it connects (`vendor/vfio-user-rs/src/lib.rs:829-840`).
- It keeps the returned descriptor and `region_info.offset` as the region's `FileOffset` (`:837`).
- The server crate sends the descriptor only together with a capability list, so an mmap-able
  region needs sparse areas (`lib.rs:1753-1790`).

**Mapping: the areas become KVM memory.** Stock v53 is `vendor/cloud-hypervisor/pci/src/vfio_user.rs:121-205`;
with patch 0005 it is `/mnt/vmstore/caraxes/ch-0005/pci/src/vfio_user.rs:299-388`. `map_mmio_regions`
does the following:
- It works only on regions flagged `VFIO_REGION_INFO_FLAG_MMAP`.
- It takes the sparse areas, or one area for the whole region when there are none.
- For each area it calls `MmapRegion::mmap(size, prot, fd, file_offset.start(), area.offset)`.
  - That is `mmap(MAP_SHARED, fd, region_offset + area_offset)`.
  - The sum is computed with `checked_add`, so a wrap-around fails (`pci/src/mmap.rs:207-240`, the
    add at `:217`).
  - The mapping is placed at an aligned address (`mmap.rs:104-160`).
- It then calls `create_user_memory_region` at `BAR base + area.offset` (`vfio_user.rs:364-383`).

**When mapping happens.**
- The device manager calls this once, right after the BAR addresses are allocated, while it creates
  the device (`vmm/src/device_manager.rs:4487`). The guest has not run yet.
- Nothing re-reads the region info or remaps an area later.
- `move_bar` removes each KVM region and inserts it again with the same mapping at the new base
  (`vfio_user.rs:716-756`). An area follows a BAR move and keeps its content.
- `unmap_mmio_regions` drops the mappings only when the device is removed (`:496-518`).

**What the specification allows.**
- A mappable region comes with one file descriptor: "When this flag is set, the reply will include
  a file descriptor" (`vfio-user.rst:861-864`).
- Region `offset` is "the offset that should be given to the mmap() system call ... also used as
  the base offset when mapping a VFIO sparse mmap area" (`:875-877`).
- An area's offset "will be added to the base offset ... to form the offset argument of the
  subsequent mmap() call" (`:941-947`).
- There is no message by which the server changes region info or areas after the client asked
  (Region Information and Sparse Regions, `:100-135`).
- The specification does expect the server to watch mapped memory: "The server is responsible for
  polling for client updates to memory mapped regions" (`:111-112`). That is the P24 model.

**Consequences for P21.**
- The areas and what backs them are fixed before the guest boots.
- Each area sits at `fd offset = region offset + area offset`, and region offset >= 0.
- An area at guest BAR1 offset G therefore needs a descriptor that can be mapped at offset G or
  more.

## 3. The NVIDIA mmap rule for an RM mapping

**How a mapping context is made.**
- `NV_ESC_RM_MAP_MEMORY` maps the object (a host BAR1 mapping for vidmem). It then installs a
  mapping context on the file descriptor that the caller passes in the parameters
  (`src/nvidia/arch/nvalloc/unix/src/escape.c:598-626`, `rm_create_mmap_context(..., pApi->fd)`).
- The context goes into `nv_add_mapping_context_to_file` (`kernel-open/nvidia/nv-usermap.c:29-125`).
  That function allows exactly one context per file: when the list is not empty, it frees the new
  node and returns `NV_ERR_STATE_IN_USE` (`:102-121`).

**What `nvidia_mmap_helper` requires.**
- It refuses a file without a context (`nv-mmap.c:540-550`).
- It refuses any `vm_pgoff != 0`: `if (vma->vm_pgoff != 0) goto done;` (`:554-557`). The fault
  path does the same (`:245-248`).
- It returns `-ENXIO` unless the VMA size equals the context's size exactly (`:583-587`).
- The context lives until the file is closed (`nv.c:1101-1108`). So the same file can be mapped
  again at offset 0, as often as wanted.

**Lifetime.**
- Freeing an RM user mapping does nothing to CPU mappings that still exist:
  `nv_free_user_mapping` is an empty function (`nv-usermap.c:152-159`).
- Revocation exists only for power management (`nv_revoke_gpu_mappings`, `nv-mmap.c:834`, called
  from `dynamic-power.c:2906`).
- So if the device's client goes away (an unmap, or the process dying), the host RM can hand that
  BAR1 range to the desktop, while Cloud Hypervisor still maps it into the guest.

**The claim, checked.**
- It is correct that one descriptor holds one mapping and refuses a non-zero offset.
- What remains possible: one area, at guest BAR1 offset 0, region offset 0, exactly the mapping's
  size.
- That is not safe for the lifetime reason above.

## 4. Candidate designs

Each candidate is judged on four things: the specification (spec), the NVIDIA driver rule
(driver), isolation (only this VM's reservation, D11/D16), and traps per submission.

**A. memfd or hugetlb file as the region descriptor.**
- Spec: yes; it is what vfio-user servers normally hand out (`:111-112`).
- Driver: not involved.
- Isolation: perfect. The memfd is host RAM, and the device copies only into reservation pages the
  guest's BAR1 tables name.
- It cannot alias VRAM. A memfd's pages are shmem page-cache RAM, and an mmap of the file maps those
  pages. A file of RAM pages cannot hold PCI BAR frames (`VM_PFNMAP`), and neither can hugetlbfs.
- It is therefore only a shadow. The guest writes into RAM, and the device must copy to VRAM before
  the GPU reads the data.
  - That works for data ordered by the doorbell: the GPFIFO entries and GPPut, copied at the
    trapped doorbell or by the P24 poller.
  - It is wrong for anything the GPU writes and the CPU reads (USERD GPGet), and for buffers
    consumed without a doorbell.
  - The area's placement must still be guessed in advance.
- Traps per submission: the same as passthrough. The doorbell stays.
- Verdict: the P24 route, not P21.

**B. dma-buf of the reservation** (`kernel-open/nvidia/nv-dmabuf.c`).
- Spec: yes.
- Driver:
  - The export (`NV_ESC_EXPORT_TO_DMABUF_FD`, `kernel-open/nvidia/nv.c:2806-2814`,
    `nv_dma_buf_export` `nv-dmabuf.c:1825-1870`) accepts any vidmem handle of the caller's client:
    - the same GPU;
    - OS-page aligned;
    - inside the object;
    - vidmem only on a dGPU (`src/nvidia/arch/nvalloc/unix/src/osapi.c:1414-1495`).

    No privilege check was found on this path.
  - mmap is allowed for dGPU vidmem (`osapi.c:5700-5703`) when the exporter asks for it
    (`bAllowMmap`, `nv-dmabuf.c:1604` and `:1641-1646`).
  - `nv_dma_buf_mmap` accepts any page offset inside the buffer (`:1331-1352`).
  - It maps the whole buffer to the host's BAR1 first (`:1359-1370`). So the buffer size is also its
    host BAR1 cost.
  - There can be up to 128 objects, each with its own offset and size (`nv-ioctl.h:131-150`).
- Lifetime: each VMA holds the buffer's BAR1 mapping through a refcount (`nv_dma_buf_vma_open` and
  `_release`, `:1246-1283`). The buffer holds a duplicate of the memory handle under the RM's
  internal client (`osapi.c:1563-1565`, `DupObject` `:5653`).

  So the host can neither reuse the BAR1 range nor free the VRAM while Cloud Hypervisor still maps
  it, even if the device process dies first. A restarted device then fails its fixed-address
  reservation and refuses to start: fail-closed.
- CUDA: `cuMemGetHandleForAddressRange(..., DMA_BUF_FD)` has no source. The only vidmem dma-buf
  exporter in the open module is this escape, so CUDA must go through it. That is an inference, not
  a citation.
- Verdict: **the descriptor to use.**

**C. One RM mapping per sparse area, each with its own descriptor.**
- Not in the specification: one descriptor per region, and the area offsets are relative to it
  (`:861-864`, `:875-877`, `:941-947`).
- Stop.
- Also rejected: the single-area-at-0 case with an RM descriptor, for the lifetime hazard of §3.

**D. One large host BAR1 mapping of the whole reservation as one area over all of guest BAR1, host
offsets equal to guest offsets.**
- The size alone rules it out. The host has 256 MiB of BAR1, shared with the desktop (`fb.rs:280-288`,
  parallel.md §2). The reservation is 2-4 GiB. Every byte of a mapping or dma-buf occupies host
  BAR1.
- The offsets cannot be made equal from either side:
  - host side: `NVOS33` has no BAR1 placement flag (`src/common/sdk/nvidia/inc/nvos.h:1724-1842`;
    `MAP_FIXED` is the CPU virtual address, `:1768-1782`);
  - guest side: guest BAR1 offsets are chosen by the guest RM (§1).
- Forcing static BAR1 in the guest would need these:
  - a guest BAR1 larger than the claimed 8 GiB FB address space;
  - the reservation at guest BAR1 offsets from about 3.6 GiB up, which needs an fd offset of at least
    3.6 GiB and a negative region offset. `mmap.rs:217` refuses that.
- Verdict: no.

**E. sysfs `resource1` of the host card as the descriptor** (raw host BAR1, any offset).
- It needs root.
- It hands Cloud Hypervisor all of the host's BAR1, including the desktop's surfaces.
- It bypasses the RM and has the lifetime hazard of §3.
- Verdict: no (see also the memory note that host GPU state is off limits).

**F. Re-point a fixed host BAR1 range by unmap and map, hoping the RM reuses the address.**
- The desktop allocates from the same BAR1 heap. A lost race puts guest writes into host memory.
- Verdict: no.

**G. Layout-matched window: the prototype.**
- The mechanism is B:
  - a dma-buf of the reservation piece `[base + 0x980000, + len)`, with `len` 1 MiB by default;
  - offered as region 1 with offset 0 and one sparse area `[0, len)`.
- Spec: yes. Driver: yes. Isolation: yes; the range is checked to lie inside the usable part,
  never the carve-out.
- Correctness holds only while the guest maps each BAR1 page g of the area to `base + 0x980000 + g`.
  Every traced run did. The device re-checks after every guest change of the BAR1 tables over the
  area (§6).
- A mismatch cannot be repaired, because the area cannot be withdrawn (§2). The guest's CPU accesses
  of that page then hit another page of its own reservation.
- Verdict: good for measuring the gain, not for general use.

**H. Page remap into a private pool: the target design.**
- The area is backed by a pool P of `len` bytes at the top of the reservation. P is taken out of the
  guest's usable region (`fbRegionInfo`), like the carve-out.
- When the guest maps BAR1 page g (inside the area) to its page T, the device does the following:
  - It remaps guest address T to host page `P + g`. It copies T's content first; T is normally
    fresh, because libcuda maps the buffer before it creates the channel.
  - Every device path that turns a guest FB address into host VRAM applies the remap:
    - the FB store (`fb.rs` `VramStore`, which also serves BAR2, PRAMIN and walks);
    - the shadow's leaf offsets and the splitting of coalesced runs (`shadow.rs`);
    - the mirror's GPFIFO, USERD and instance addresses (`mirror.rs`, `channel.rs`).
  - When the guest later maps another page T' at g, the device swaps back: it copies `P + g` to T
    and remaps T' in the same way.
- Correct for any guest layout. Spec, driver and isolation as in G (P is inside the reservation).
- Cost:
  - a pool page per area page, out of the guest's FB;
  - one extra table lookup per FB access (a small map, empty for most pages);
  - quiescing the guest's channels for a late remap of a page already mapped on the GPU. That is
    rare: the channel buffers are CPU-mapped before any channel uses them.
- Verdict: **recommended.** It needs the files this package may not touch (§7).

**I. A Cloud Hypervisor change: re-query region info at run time, or KVM slots managed per page.**
- The specification has no such message. That would be invented protocol (patches/REVIEW-vhost-user.md).
- Stop.

## 5. Recommendation

**Build H on B.**
- Offer guest BAR1 `[0, len)` as one sparse mmap area of region 1: region offset 0, area offset 0.
- Back the area with a dma-buf of a pool piece of the reservation.
- Keep the area correct for any guest layout by remapping, in the device, each guest page mapped
  there to its pool page.
- Until H exists, the prototype G measures the gain on the observed layout and checks it at every
  change.

**Traps per operation.** Today's figures are from mapping.md §6: about 25 us per trap, device
share 1-2 us.

| Operation | Today | P21 (G or H) | P21 plus patch 0005 doorbell ioeventfd |
|---|---|---|---|
| submission (GPFIFO lo, GPFIFO hi, GPPut, doorbell) | 4 traps, about 100 us | 1 trap (the doorbell), about 25-30 us | 0 socket round trips; 1 KVM exit ending in an eventfd |
| small CPU copy through a BAR1 mapping, per 4/8-byte access | 1 trap each (P21's "0.4 MB/s") | direct at write-combined speed if the buffer's BAR1 pages lie in the area (H: any buffer mapped there; G: only if it matches the layout, else reported); otherwise 1 trap each | same |
| context creation (BAR2 table writes) | about 96k traps | unchanged | unchanged |

**What stays trapped:**
- all of BAR0, including the doorbell, which patch 0005 turns into an ioeventfd;
- BAR1 outside the area;
- all of BAR2 (region 3), that is page tables and instance memory: the separate "BAR2 selective
  passthrough" item of the roadmap;
- PRAMIN.

**Isolation argument:**
1. The area maps only `[fb0, fb0 + len)`. `bar1_passthrough_area` (`fb.rs:343`) checks that this
   lies inside the usable reservation and outside the device-owned carve-out with the BAR root
   directories.
2. The dma-buf holds that memory and its host BAR1 mapping for as long as any mapping exists (§4 B).
   The host cannot give either to the desktop or another VM while the guest can reach it, whatever
   order the processes die in.
3. No hardware walks guest BAR1 tables. A guest write through the area reaches only its own VRAM,
   as a trapped write does today.
4. A guest page-table page reached through the area can only make the device's cached walk stale.
   The shadow still validates every leaf: reservation or registered sysmem only, no compressible
   kind (`shadow.rs:93-103`). The prototype counts such pages. `CARAXES_SHADOW_ALWAYS_RESYNC=1` is
   the safe setting when one appears.

**Host BAR1 budget.** The area adds `len` (1 MiB by default) of host BAR1 per device to the P20
window budget (2 x 32 MiB).

## 6. The prototype (G), as built

**Switch and settings.**
- `CARAXES_BAR1_PASSTHROUGH=1` turns it on (`fb.rs:311` `bar1_passthrough_config`).
- `CARAXES_BAR1_PT_MIB` sets the area size: a power of two, 1-64, default 1.
- `CARAXES_BAR1_PT_FB_OFFSET` sets the backing's offset from the reservation base, default
  `0x980000` (`BAR1_PT_FB_OFFSET`, `fb.rs:330`).
- It needs `--reserve-vram` and host execution; otherwise the device refuses to start
  (`main.rs:407`).

**Code.**
- `hostmem.rs`:
  - `export_dmabuf` (`:287`) and `HostMemory::export_bar1_passthrough` (`:1141`) do the
    `NV_ESC_EXPORT_TO_DMABUF_FD` of the reservation piece with `bAllowMmap`.
  - `Bar1Export` (`:270`) keeps the descriptor for the process's life.
- `device.rs`: `regions_with` / `Bar1Area` (`:206`, `:216`) mark region 1
  `VFIO_REGION_INFO_FLAG_MMAP` with offset 0, one sparse area `[0, len)` and the dma-buf as
  `mmap_fd`. All other regions are unchanged.
- `fb.rs`:
  - the check state `Bar1Pt` (`:618`);
  - `set_bar1_passthrough` (`:1238`), `area_walk` (`:1279`, one read per table per 2 MiB unit) and
    `poll_bar1_passthrough` (`:1370`);
  - `note_write` re-arms the walk when a BAR1 table page over the area is written;
  - `walk_update` re-arms the page-table hazard check when the shadow's set of table pages changes.
- `device.rs:646`: the device polls after every trapped BAR0 write. After changing a BAR1 mapping
  the guest flushes and invalidates through BAR0, before it uses the mapping. When nothing changed,
  the poll costs two flag reads.
- `boot.rs:400`: the check state survives a function-level reset.
- `boot.rs` `exec_summary`: one exit-summary line
  `bar1 passthrough: guest BAR1 [0, len) on FB fb0; N check(s), last: M page(s) mapped, K matched, J elsewhere; X mismatched page(s) and Y page-table hazard(s) over the run`.
- Findings are logged as `ERROR: bar1 passthrough: ...`, the first 16.

**Tests.**
- `fb::tests::bar1_passthrough_area_stays_in_the_usable_reservation`.
- `fb::tests::bar1_passthrough_check_follows_the_guest_bar1_tables`: 4 KiB and 64 KiB pages,
  mismatch, re-arm on table writes only, the table hazard, carry across reset.
- `device::tests::bar1_area_makes_region_1_mmapable_at_offset_zero`.
- `cargo build --release -p vfio-user-nvgpu` is clean. `cargo test -p vfio-user-nvgpu` passes (101
  unit tests plus 2). `cargo clippy --all-targets [--all-features] -- -D warnings -A
  clippy::unnecessary_cast -A clippy::field_reassign_with_default` is clean.

**Not verified until the coordinator's run:**
- that the host RM exports and mmaps the dma-buf for the device's client. If it refuses, the
  device prints the status and does not start;
- that KVM maps the dma-buf's `VM_PFNMAP` frames. It does this for VFIO BARs the same way;
- the effective memory type in the guest. It is expected to be write-combined: the guest maps BAR1
  WC, and on this AMD host the nested table does not force UC for MMIO. The copy number will show
  it.

`caraxes/tools/bench-trap.c` now takes a write offset of `none` and a read offset, so BAR1 can be
timed with reads only. A write in the area would land in the guest's own VRAM.

## 7. Plan

| Step | What | Estimate |
|---|---|---|
| 1 | Coordinator run of §8 with G: does the export work, is the area mapped, is the layout matched, what are the trap numbers | 1 h of VM time |
| 2 | If step 1 shows mismatches, or for the general case: H, the pool and the remap table. `fb.rs` `VramStore` and the walks translate through it; `rpc.rs` shrinks the usable region by the pool; `shadow.rs` resolves leaves through it and splits coalesced runs at remapped pages; `mirror.rs` / `channel.rs` take GPFIFO, USERD and instance addresses through it; the remap is installed in the BAR0 poll of §6 and completed before the guest's invalidate returns | 2-3 days (files outside this package's allowance: shadow.rs, mirror.rs, channel.rs) |
| 3 | Late remap: a page already mapped on the GPU. Park the guest's channels (the preempt path of the mirror), copy, update the shadow, invalidate, resume | 1 day |
| 4 | Area size and budget: 1 MiB covers about 170 channel buffers; P20 accounting of the extra host BAR1; the exit summary into p20-check | 0.5 day |
| 5 | With patch 0005 merged: the submission path is GPFIFO and GPPut direct plus the doorbell ioeventfd. Then the token pool of the roadmap, and P24 for the last KVM exit | see roadmap |
| 6 | BAR2 selective passthrough (context creation): the same B mechanism does not fit, because every BAR2 write can be a page-table write the device must see. It needs a design of its own | - |

## 8. Measurement commands (coordinator, vm2 as in mapping.md, nothing run here)

Build the device from this tree, then run the same series without and with the switch.

```sh
R=/home/silas/git/Caraxes          # or the merged tree
cargo build --release -p vfio-user-nvgpu
M=$R/caraxes/runs/p21-bar1-$(date +%F); mkdir -p "$M"
source "$R/caraxes/runs/mapping-2026-09-24/env.sh"
export CARAXES_SERVER=$R/target/release/vfio-user-nvgpu

# A. Reference (switch off)
unset CARAXES_BAR1_PASSTHROUGH
"$R/caraxes/tools/bench-run.sh" v2-restart > "$M/a-up.txt" 2>&1
"$R/tools/caraxes-vm.sh" ssh 'cat > ~/bench/bench-trap.c' < "$R/caraxes/tools/bench-trap.c"
"$R/tools/caraxes-vm.sh" ssh 'cd ~/bench && gcc -O2 -o bench-trap bench-trap.c &&
  sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource1 20000 none 0x80000 &&
  sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource1 20000 none 0x400000 &&
  sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource0 20000' | tee "$M/a-trap.txt"
"$R/caraxes/tools/bench-run.sh" series --targets v2 --reps 3 --metrics "short-auto rl copy" --out "$M/a" 2>&1 | tail -5

# B. P21 prototype (switch on, 1 MiB area on FB base + 0x980000)
export CARAXES_BAR1_PASSTHROUGH=1
"$R/caraxes/tools/bench-run.sh" v2-restart > "$M/b-up.txt" 2>&1
grep -E 'bar1 passthrough' /mnt/vmstore/caraxes/vm2/gpu.log | tee "$M/b-device-start.txt"
"$R/tools/caraxes-vm.sh" ssh 'cd ~/bench &&
  sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource1 20000 none 0x80000 &&
  sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource1 20000 none 0x400000 &&
  sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource0 20000' | tee "$M/b-trap.txt"
"$R/caraxes/tools/bench-run.sh" series --targets v2 --reps 3 --metrics "short-auto rl copy" --out "$M/b" 2>&1 | tail -5
"$R/tools/caraxes-vm.sh" down
grep -E 'bar1 passthrough|ERROR: bar1' /mnt/vmstore/caraxes/vm2/gpu.log | tee "$M/b-device-summary.txt"
grep -c ' bar1 0x000[0-9a-f]' /mnt/vmstore/caraxes/vm2/gpu-trace.log | tee "$M/b-bar1-trapped-in-first-MiB.txt"
```

The trace grep counts trapped `bar1` lines whose offset is below 0x100000.

**What to read from the results:**
- `b-device-start.txt` shows the export line. `b-up.txt` shows Cloud Hypervisor starting; an
  `Could not mmap sparse area` there means the dma-buf mmap was refused.
- `b-trap.txt`:
  - the read at `0x80000`, inside the area, should drop from about 25 us (`a-trap.txt`) to a few
    microseconds;
  - the read at `0x400000` stays trapped.
- `b-device-summary.txt` must show `0 mismatched page(s) and 0 page-table hazard(s)`. A run with
  either above 0 is not a valid P21 number.
- `b-bar1-trapped-in-first-MiB.txt` should be 0: no GPFIFO or GPPut write reaches the device any
  more.
- short-auto: per-launch time against a0 and c0 (0.12-0.19 ms); expected about 75 us less per
  launch. rl: wall time. copy: unchanged, because cuMemcpy runs on copy engines. The copy metric
  is a regression check here, not the P21 small-copy case.
- Isolation, as in P20:
  - `caraxes/tools/bench-hostwatch.sh` during B;
  - host `nvidia-smi -q -d MEMORY`, the BAR1 used before, during and after: B should show 1 MiB
    more while vm2 is up, and the same value as before once Cloud Hypervisor is gone;
  - `p20-check.py` on the device log.

## 9. Prototype G withdrawn (2026-09-24, 17:40)

On the first run against the 615 host (`caraxes/runs/cutover-615-2026-09-24/v2-610guest/`)
the 610 guest laid its BAR1 out 2 MiB away from the traced layout: 128 pages of the area
mapped FB pages the area did not back, and the check could only log it after the guest had
already read and written the wrong pages of its own reservation (the area is mapped once by
the client and cannot be withdrawn). The cuda gate's robustness stage failed 1 of 3 runs in
that session. `CARAXES_BAR1_PASSTHROUGH=1` is therefore refused unless
`CARAXES_BAR1_PASSTHROUGH_UNSAFE_LAYOUT=1` names the experiment; the measurements that used
it (P21, the token pool, the reactor, the closing series) stay valid as measurements of the
trap counts, their layout matched. The pool of [bar1-pool.md](bar1-pool.md) is the mode to use.
