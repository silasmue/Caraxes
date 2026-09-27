<!-- SPDX-License-Identifier: MIT -->
# Memory model for real execution (E5b part 1)

Written 2026-09-23 with the implementation in `crates/vfio-user-nvgpu` (`fb.rs`
`FbLayout`, `hostmem.rs`, `dmamap.rs`, `guestmem.rs`, `privilege.rs`) and
`tools/caraxes-vm.sh`. Results: [experiments.md](experiments.md), "E5b part 1".
Evidence: `caraxes/runs/e5b-2026-09-23/`.

This document describes the default identity-addressed path. The experimental
[zero-based framebuffer](zero-based-fb.md) uses separate guest and host coordinates.

Part 1 makes every address final while the device still executes pushes itself:

- guest FB addresses are host VRAM addresses inside a reservation the device holds (D11);
- guest sysmem addresses are host IOVAs of guest RAM the device registered (D13).

The fake FB in host RAM is still the backing store. Part 2 swaps it for the reserved
VRAM and hands the pushes to host channels.

## 1. FB address model

**Rule.** Guest FB address X is host VRAM address X. Every FB address the guest can
write lies in the reservation `[B, E)`.

**Inputs.**
- The host FB heap region comes from `FB_GET_FB_REGION_INFO`.
  - On this card it is `[0x3600000, 0x1e9190000)`.
  - `E` is the region end aligned down to 2 MiB: 0x1e9000000.
  - `B = E - S`, where `S` is the profile size rounded up to 2 MiB.
- The host card's FB size comes from `FB_GET_INFO_V2 TOTAL_RAM_SIZE`: 8 GiB.

**What the guest is told.**

| Item | Value | Why |
|---|---|---|
| `NV_PFB_PRI_MMU_LOCAL_MEMORY_RANGE` (FB address space) | the host card's size, 8 GiB (0x8A) | `E` itself cannot be encoded (mag ≤ 63 × 2^scale MiB). With the card's own size, the guest computes its fake WPR at the same place where the real card keeps its own WPR: above `E`, in the host's reserved top 366 MiB |
| WPR layout (the guest computes it, `kgspPopulateWprMeta_TU102`) | measured: `gspFwRsvdStart` 0x1f7600000, non-WPR heap 0x1f7600000, WPR 0x1f7700000, FRTS 0x1ffe00000, VGA workspace 0x1fff00000 | all above `E`. The device checks this at the WPR-meta event (`fb layout: ... ok`). These addresses never reach an engine |
| `fbRegionInfoParams` | region 0 usable `[B, E - 2 MiB)`; region 1 reserved `[E - 2 MiB, 8 GiB)` | nothing below `B` is described, as on the host (its one region starts at 54 MiB). The CPU-RM reserved block lands at the top of region 0, inside the reservation |
| carve-out `[E - 2 MiB, E)` | `bar2PdeBase` = E - 2 MiB, `bar1PdeBase` = E - 2 MiB + 4 KiB | device-owned root pages inside the reservation. 2 MiB keeps the usable end 2 MiB aligned |
| `GspStaticConfigInfo.fb_length` | the profile, 4 GiB | `nvidia-smi` Total comes from it (`mem_mgr_gsp_client.c:118`) |
| PRAMIN reset window (`NV_PBUS_BAR0_WINDOW`) | 8 GiB - 1 MiB | the kernel moves the window before every access. The default is only a register value; no access went through it |

**Numbers for the 4 GiB profile on this host.**

| | Address |
|---|---|
| reservation | `[0xe9000000, 0x1e9000000)` |
| usable (heap) | `[0xe9000000, 0x1e8e00000)`, 4094 MiB |
| BAR2 / BAR1 root | 0x1e8e00000 / 0x1e8e01000 |
| guest WPR (fake) | `[0x1f7600000, 0x200000000)` |

**Observed in the guest.**
- The kernel VAS root is at 0xe9000000, UVM's root at 0xe9200000.
- BAR1/BAR3 walk targets lie in 0x1e8da0000..0x1e8e01018.
- `nvidia-smi` shows `0MiB / 4096MiB`, Reserved 175 MiB.

**Check.** The fake FB counts every access outside `[B, E)`. The count is in the exit
summary of `gpu.log` (`fb: reservation [...]; accesses outside it: N reads, M writes`).
It was 0 and 0 over boot, `nvidia-smi` and the UVM registration.

**Without `--reserve-vram`** the layout is the milestone-1 one: FB `[0, profile)`,
roots in the non-WPR heap.

## 2. Guest RAM and the DMA table

**Registration** (`hostmem.rs` `register`, called from `Device::dma_map`). For each
memory zone Cloud Hypervisor maps (`[0, 3 GiB)` and `[4 GiB, 5 GiB)` for 4 GiB), the
device:
1. Checks that pinning leaves the host at least 4 GiB of `MemAvailable`; otherwise it
   refuses the DMA_MAP, and Cloud Hypervisor fails to start.
2. Registers the zone in 1 GiB chunks aligned in GPA. Each chunk is one
   `NV01_MEMORY_SYSTEM_OS_DESCRIPTOR` over the device's own mapping of the memfd.
3. Reads each chunk's IOVA with `GET_SURFACE_PHYS_ATTR` at offset 0, every 128 MiB and
   the last page, all in one privileged window. The chunk must read base + offset
   everywhere and have aperture SYSMEM; otherwise registration is refused.

**Measured.**
- GPA 0 → IOVA 0x40000000. Then the host switched the GPU to 64-bit IOVAs (allowed by
  the operator): GPA 1 GiB → 0x7fffc0000000, 2 GiB → 0x7fff80000000,
  4 GiB → 0x7fff40000000.
- 0.7-1.5 s per GiB.
- 4 privileged windows in total.

**Table format.** `<state dir>/dma-ranges` holds one line in the `ranges=` format of
`caraxes_dma_range`, hex, sorted by GPA. It is rewritten atomically after each zone:

```
0:40000000:40000000,40000000:7fffc0000000:40000000,80000000:7fff80000000:40000000,100000000:7fff40000000:40000000
```

`DmaTable::new` enforces the module's rules:
- at most 64 entries;
- sizes are 2 MiB multiples;
- starts are page aligned;
- no overlap in either space.

The launcher waits until the sizes add up to the VM's RAM.

**Translation.** `GuestMemory::translate(addr, len)` is the only way to guest RAM. Every
read and write the device makes goes through it:
- queue pages, RMARGS, libos args;
- the WPR meta, the FWSEC command;
- fault buffers, USERD, semaphores, pushbuffers;
- sysmem page-table levels and PTE targets;
- the crash buffer.

With a table, an address outside it is refused and counted (`dma table: N DMA
address(es) outside the table refused` in the summary). It is never passed through.
Without a table the translation is the identity.

`--identity-dma-table` publishes IOVA = GPA without host registration, which exercises
the module path.

**The 40-bit flush buffer and the swiotlb alias.**
- **The constraint.** A GSP-client guest allocates its sysmem flush buffer (one page)
  early, under a temporary 40-bit DMA mask. It refuses to initialize when that page's DMA
  address is 2^40 or above (`kmemsysInitFlushSysmemBuffer_GM107`,
  `kern_mem_sys_gm107.c:227-340`: `osInitNvMapping` fails, `RmInitAdapter failed!
  (0x22:0x59:894)`). With the mask lowered, dma-direct bounces the page through swiotlb.
- **When it bites.** The first registration of this host boot got a 32-bit IOVA for
  GPA 0 (0x40000000), and the guest worked. After the host GPU switched to 64-bit IOVAs
  (the switch lasts until the host reboots), every chunk lies near 2^47, and the guest
  cannot boot.
- **The workaround.** `tools/caraxes-vm.sh dma` does the following whenever an IOVA is
  2^40 or above:
  1. reads the guest's pool from its kernel log (`software IO TLB: mapped [mem A-B]`);
  2. asks the device binary for a table with that pool, rounded out to 2 MiB, aliased at
     IOVA 0xf000000000 (`vfio-user-nvgpu --dma-table-alias`, `DmaTable::with_alias`);
  3. installs that table, which it also saves as `$W/dma-ranges.guest`.
- **Result.** The guest's flush register then reads `NV_PFB_NISO_FLUSH_SYSMEM_ADDR`
  0xf0038b70, that is IOVA 0xf0038b7000.
- **Why the alias is acceptable.** The physical GPU never uses the guest's flush buffer:
  the host owns the real flush register. 960 GiB lies above every host 32-bit IOVA and
  below the 64-bit ones, so a stray DMA there faults instead of reaching host memory.
- **The risk (part 2).** Up to about 2 MiB of ordinary guest RAM around the pool (the
  2 MiB rounding) shares the alias. Until 2026-09-24 the device's own table did not carry
  the alias, and a DMA buffer the guest placed there faulted on the physical GPU. That
  happened (Xid 31 in cuda-hostreg); the device now follows the alias, see "The alias in
  the device" below.
- **The clean fix.** A registered chunk below 2^40. That is possible only before the
  host's 64-bit switch, or with a host-side IOVA mapping (S4).

### The alias in the device (Xid 31 in cuda-hostreg, 2026-09-24)

**What happened.** The cuda gate of the 610 guest on vm1 (main 7621cfb, 1 GiB profile at
slot 4; `caraxes/runs/gsp-bc-2026-09-24/gpu-vm1/`) failed cuda-hostreg with `D Memcpy HtoD
(reg): 719 unspecified launch failure`. The host logged one Xid 31 on the device pid 508533:
`MMU Fault: ENGINE CE2 HUBCLIENT_CE0 faulted @ 0x100_0bb0f000, FAULT_PTE
ACCESS_TYPE_VIRT_READ`. The rerun of the same script and binary passed, with 0 Xid.

**Cause.** The address is a GPU VA, not a sysmem address: hostregprobe's registered 2 MiB
buffer was mapped at 0x1000ba00000 (`guest/cuda-hostreg.log` of the gate run), and
0x1000bb0f000 is offset 0x10f000 into it. The device trace of the stage
(`gate-cuda-2026-09-24-10/device/cuda-hostreg.trace:2998-3021` in the record) has 24
lines `shadow: REFUSED leaf va 0x1000bb0f000 -> Sys(f0040b7000) ... sysmem outside the
registered guest-RAM ranges`. The first refused leaf is the faulting VA. The refused IOVAs
are all in the alias window, at offsets 0xa0000-0xb6000 and 0x40b7000-0x40bf000. That run's
guest pool was `[0xba2b7000, 0xbe2b7000)`, aliased from GPA 0xba200000
(`/mnt/vmstore/caraxes/vm1/probe-gspbc/dma.txt`). So the refused pages are exactly the two
rounding margins `[0xba200000, 0xba2b7000)` and `[0xbe2b7000, 0xbe400000)`: ordinary guest
pages that the kernel handed to the probe's `posix_memalign`. The guest translated them
through its table to the alias, as it must.

The device's own table has no alias. The shadow filter (`resolve_leaf`, shadow.rs:94)
refused those leaves, as D16/D17 require for an address it cannot place. The shadow VA
space then had no PTE there, and the copy engine faulted on the VM's own channel. The
fault stayed on the device pid, so isolation held. Whether the probe gets margin pages
depends on the guest's free lists, which is why the earlier gates and the rerun passed.
The B and C changes (hostrm.rs, the forward table, the withheld resets) play no part:
no control of the probe was refused, and the stage's only non-zero status is the usual
`GPU_QUERY_ECC_STATUS -> 0x56`.

**Fix** (commit "vfio-user-nvgpu: follow the guest's swiotlb alias").
- The launcher already records the table it installs in the guest,
  `$W/dma-ranges.guest`. It now writes the file atomically (tools/caraxes-vm.sh:371), and
  `up` removes the previous boot's file (:186).
- At every reset the device reads that file (device.rs:591 `load_guest_aliases`, called
  from the function-level reset at :925 and the vfio-user reset at :986). The launcher's
  `dma` step always resets the function after `insmod`.
- `DmaTable::aliases_of` (dmamap.rs:228) accepts the guest's table only under three
  conditions:
  - it describes exactly the registered guest RAM;
  - every entry either translates as the device's table does or lies wholly outside the
    device's IOVAs;
  - every alias names registered GPAs only.

  Anything else gives an `ERROR dma table: guest table ... refused` line, and the aliases
  stay off (the old behaviour). A missing file means no aliases.
- `GuestMemory::translate` (guestmem.rs:100) also accepts an alias address. So the device's
  own accesses follow it too: USERD, error notifiers, semaphores and sysmem page tables.
- `GuestMemory::host_iova` (guestmem.rs:156) gives the registered host IOVA of the same
  guest page. The shadow plan resolves every sysmem leaf through it (shadow.rs:133
  `host_target`, used at :480), so an alias leaf maps from the registered descriptor at its
  page's offset. The leaf keeps the guest's address as its identity for the diff.

**Isolation.** An alias can only name pages of this VM's own registered guest RAM.
`aliases_of` checks that each alias's GPA range is covered by the device's table and that
no alias reuses an IOVA the device's table gives another meaning. The physical GPU still
never sees an alias IOVA: the shadow maps the leaf from the guest-RAM descriptor. What
changes for the GPU is that the shadow now maps pages the filter used to refuse. The pool
itself is included, which also makes bounced DMA work.

**Tests** (`cargo test -p vfio-user-nvgpu`, 287 unit tests pass):
- `dmamap::tests::the_guest_alias_is_accepted_and_nothing_else`: the alias is accepted.
  Refused are unregistered RAM, a missing chunk, and swapped chunks.
- `guestmem::tests::the_guest_alias_reaches_the_same_pages`: reads through the alias, the
  host IOVA and the entry bounds.
- `shadow::tests::a_leaf_at_the_guest_alias_maps_from_the_registered_descriptor`: a plan
  on the fake FB. Without the aliases the alias leaf is refused (the Xid case). With them
  it maps from descriptor 7 at offset 0x20_1000, and a leaf past the entry stays refused.

**Reproduction on the GPU.** The new probe `caraxes/tools/hostreg-margin.c` makes the case
deterministic:
1. It allocates guest RAM in 2 MiB chunks without THP.
2. It reads the frames from `/proc/self/pagemap` and keeps the chunks that own margin pages.
3. It registers each kept chunk (DEVICEMAP) and checks an HtoD/DtoH round trip byte by byte.

Runs: `caraxes/runs/xid31-hostreg-2026-09-24/` (run.sh, `before` with the 7621cfb binary,
`after` with the fix; record.md there):
- **Before**, main 7621cfb, device pid 1131576: hostreg-margin failed with the case's own
  numbers, `719 unspecified launch failure`, `bad=2088960`. The host logged two Xid 31 on
  the device pid, `CE2 HUBCLIENT_CE0 ... FAULT_PTE`.
- **After**, the fix, device pid 1151433:
  - hostreg-margin passed 20 of 20, with 256-512 margin pages per iteration;
  - hostregprobe passed 20 of 20;
  - the full cuda gate passed 10 of 11 (10 of the 11 scored rows; the gate script counts the same result as 10 of 12, with the declared-unsupported cuda-gdb row in the 12, testing.md section 3, cuda) (cuda-managed has the 801 it has in every gate
    that day);
  - 0 host Xid.

## 3. Launcher

State for `VM=vm2` lives in `/mnt/vmstore/caraxes/vm2`.

```sh
VM=vm2 CARAXES_TAP=caraxes2 CARAXES_IP=192.168.102.22 CARAXES_MAC=02:00:00:00:00:22 \
CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=4096 CARAXES_HOST_RM=1 \
CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register tools/caraxes-vm.sh up
tools/caraxes-vm.sh wait; tools/caraxes-vm.sh probe <label>     # or: dma, then modprobe yourself
```

- `up` adds `--reserve-vram`, and `--register-guest-ram --dma-ranges $W/dma-ranges`.
  `CARAXES_RESERVE_SLOT=<n>` adds `--reserve-slot n`: the range moves n profile sizes below
  the top, `[E-(n+1)S, E-nS)` (P20, one slot per VM; 0, the default, is the top). The 1 GiB
  headroom rule is unchanged, and a slot below the heap base is refused.
  It then waits for the complete table.
- `dma` runs inside `probe` and `uvm`, before `modprobe`. When the loaded table
  differs, it does `rmmod nvidia_uvm nvidia caraxes_dma_range`, then `insmod
  caraxes_dma_range.ko ranges=<table>`, then a function-level reset. On first use it
  builds the module in the guest from `guest-module/caraxes_dma_range`.
- The guest keeps `blacklist nvidia` in `/etc/modprobe.d/caraxes.conf`, so nvidia
  never binds without the table.

## 4. Host resources, privilege, teardown

**Held for the device's life, by its own host RM client.** This is a second client in
the same process, next to the forwarding client of `hostrm.rs` (D10).
- The reservation (`NV01_MEMORY_LOCAL_USER`, FIXED_ADDRESS, CONTIGUOUS, 2 MiB pages,
  owner `cara`).
- The four descriptors, which keep 4 GiB of guest RAM pinned.

**Reservation rule** (host-facts.md §3):
1. `F = min(HEAP_FREE, nvidia-smi free)`.
2. Refuse when `S + 1 GiB > F`.
3. On NO_MEMORY, refuse. There is no floating fallback.
4. Check the reply's offset and CONTIGUOUS.

A refused reservation stops the device before it creates its socket, so `up` fails
cleanly.

**Privilege** (D12, `privilege.rs`).
- The launcher copies the binary to `$W/vfio-user-nvgpu` and runs `sudo -n setcap
  cap_sys_admin+p` on the copy. This is the only root action.
- The process runs as uid 1000 throughout. CAP_SYS_ADMIN is permitted but not
  effective, and is raised on the calling thread only around the `GET_SURFACE_PHYS_ATTR`
  ioctls.
- It is the least of the two options, because the process never holds root. The
  alternative, `sudo -n` with a drop to `SUDO_UID` (as `caraxes/tools/sysmem-iova.rs`
  does), is also implemented.
- Without registration the device keeps no capability at all.

**Teardown.** The device frees, in this order:
1. the descriptors, which unpins guest RAM;
2. the reservation;
3. the client.

It does this when the client disconnects (`tools/caraxes-vm.sh down`) and in a
signal thread on SIGTERM, SIGINT or SIGHUP (exit 128 + signal). If the process dies
anyway, closing `/dev/nvidiactl` frees the client with everything under it.

**Why the free order is safe in part 1.** No host channel exists, so the GPU cannot be
using the IOVAs. From part 2 on, the order is: host channels, then wait for idle, then
the descriptors (DMA-FQ unmaps lazily and reuses IOVAs at once, sysmem-iova.md §2).

## 5. What part 2 builds on

- **Backing.** `fb.rs` `FbStore` is `read`/`write` of a byte range at an FB address.
  `HostRamStore` is today's implementation. A VRAM store is a `FakeFb::with_store(...)`
  away: windows of the reservation through host BAR1 (256 MiB, H2), or forwarded
  writes. Addresses are already host VRAM addresses.
- **Channels.** `channel::Channel` holds each guest channel's ChID, engine type,
  GPFIFO VA and entries, USERD, error notifier and `(hClient, hVASpace)`. `Registry`
  maps each VAS to its root PDB, which comes from `COPY_SERVER_RESERVED_PDES` and
  `SET_PAGE_DIRECTORY`.
  - All of these are guest addresses that are valid on the physical GPU: FB inside the
    reservation, sysmem as IOVAs.
  - PROMOTE_CTX buffers are not kept yet.
- **Host allocations.** `HostMemory` owns a device and a subdevice on the host. It is
  the natural owner of the host channels, TSGs and the externally owned VAS
  (`SET_PAGE_DIRECTORY` to the guest root). Its `Privilege` also covers the other D12
  controls.
- **Bounds.** `FbLayout::inside` is the check that the physical GPU is never pointed at
  a guest address outside `[B, E)` (fb-model.md §5, the isolation row).

## 6. No FB compression (E5b part 2, 2026-09-23)

The static-info FB region flags (`rpc.rs`, `fbRegionInfoParams`) now report
`supportCompressed = 0`. Reason: told a region is compressible, the guest's memory
manager gives its vidmem surfaces a compressible KIND (`GENERIC_MEMORY_COMPRESSIBLE`
0x08/0x09) and a `COMPTAGLINE` in their GMMU PTEs. The reservation is allocated
`COMPR-NONE`, so a copy-engine write through such a PTE drives the L2 comptag path into a
compression resource that does not exist and hangs cleanly, with no Xid (this is the
CE-write-into-reservation stall of host-exec.md open item 1; analysis in host-gr.md).

It is also an isolation hazard independent of the stall: the `COMPTAGLINE` (PTE bits
55:36) indexes the host's **global** comptag store, the same one the desktop's own
compressed surfaces use, so a guest value there is host state, not contained to the VM.
The reservation must therefore never be allocated with `NVOS32_ATTR_COMPR` (that would let
guest comptag lines index the host store), and the guest must be steered to uncompressible
kinds by reporting no compression. With `supportCompressed = 0` the memmgr picks
`GENERIC_MEMORY` 0x06 / `PITCH` 0x00 with `COMPTAGLINE` 0. Verifying a fresh vidmem PTE
through BAR1 (kind 0x06/0x00, COMPTAGLINE 0) is the gate before any mirrored CE/GR
execution is re-enabled (execution.md §7, the safety hold).

## 7. Fewer host RM calls for the shadow (2026-09-25, backlog item 2)

This is item 2 of [optimization-backlog.md](optimization-backlog.md). With the BAR2 memfd, a
context creation is bound by the shadow worker's host RM calls, not by traps.
- Code: `shadow.rs` and `hostmem.rs`, commits 4fec5bd, f0edbb8 and ac950e9.
- Records: `caraxes/runs/opt-shadow-maps-2026-09-25/` (README there).
- RM sources cited: the open kernel modules 615.71.09, the host's driver.

Result on vm0 (1 GiB, 610 guest, BAR2 memfd, no BAR1 pool, census probe):

| | before | after (gather-2) |
|---|---|---|
| cuCtxCreate, guest median | 815 ms | 309 ms |
| host RM calls per cuCtxCreate | 13171 | 228 |
| shadow apply per cuCtxCreate | 770 ms | 67 ms |
| cuCtxDestroy, guest median | 170 ms | 107 ms |
| host RM calls per cuCtxDestroy | 1145 | 79 |

Native is 146 ms and 86 ms. What the guest sees and what the GPU may reach are unchanged. §7.4
gives the argument.

### 7.1 Where the calls came from

The census's `on-nopool` run and the `before` run of this series behave the same way. Five
context creations in a row alternate between three cases (`before/calls.md`, "Every context
window"):

| case | window | shadow apply | RM calls | what happens |
|---|---|---|---|---|
| contiguous (rep 1) | 479 ms | 33 ms | 749 | the context's 15617 leaves coalesce into 244 runs |
| scattered (reps 2, 4) | 808-815 ms | 499-509 ms | 16946-17210 | the same leaves take about 15360 map calls. The guest's sysmem pages come back from its page pool one by one, in no order (mapping.md §2) |
| queued (reps 3, 5) | 1409-1649 ms | 1372-1436 ms | 15396-15552 | the first plan waits behind the previous destroy's plan in the worker's queue. That plan unmaps the 14337 scattered leaves one mapping at a time |

The census's "about 6000 map calls and 761 ms per context" is the mean of these cases. Per call,
measured over the whole census, a map costs 29 us and an unmap 81 us (`before/census-rpc-cost.md`,
histogram). The cost is the number of calls, and it comes from the scattered sysmem pages.

### 7.2 What one host call covers, and its limits

| Call | Covers | Limit |
|---|---|---|
| `RM_MAP_MEMORY_DMA` (NVOS46, `NV_ESC_RM_MAP_MEMORY_DMA`; hostmem.rs:940 `map_dma_fixed`) | one range `[offset, offset + length)` of one memory object, at one fixed VA | See the list below |
| `RM_UNMAP_MEMORY_DMA` (NVOS47; hostmem.rs:912 `unmap_dma`) | one mapping, or a piece of one (the RM splits it, `virtmemUnmapFrom_IMPL`, virtual_mem.c:1636) | one mapping per call |
| `NV50_MEMORY_VIRTUAL` alloc and `RM_FREE` | one region object; a free also unmaps every mapping made into the object (the resource server's auto-unmap, `_clientUnmapInterBackRefMappings`, rs_client.c:1347-1400) | the free's cost grows with the mappings it takes along |
| `NV01_MEMORY_SYSTEM_OS_DESCRIPTOR` alloc (`NV_ESC_RM_ALLOC_MEMORY`; hostmem.rs `os_descriptor`) | the pages of one user VA range, pinned by their page array (`os_lock_user_pages`, os-mlock.c:216) | pages that are mapped in the calling process |
| `NV0080_CTRL_CMD_DMA_INVALIDATE_TLB` (ctrl0080dma.h:467-495; hostmem.rs:996 `invalidate_vas`) | the TLB of one whole VA space | NON_PRIVILEGED (g_device_nvoc.c:1144) |

A map call's limits:
- one memory handle: the reservation, or one registered guest-RAM descriptor;
- contiguous in VA and in that object's offsets;
- one page size, one kind override, one snoop setting;
- inside the one virtual object it names (dma.c:158-159, `NV_ERR_INVALID_OFFSET` outside);
- no page list.

No call takes a page list. Contiguous leaves were therefore already coalesced (`coalesce`,
shadow.rs:413; mapping.md §2), and scattered guest pages could not be: a call covers a range
of one memory object's offsets. The way around this is a memory object whose offsets are the
scattered pages in VA order.

### 7.3 What changed

**Gather descriptors** (`CARAXES_SHADOW_GATHER`, default on; 0 turns them off).
- **When** (`gather_stretches`, shadow.rs:1094, called at shadow.rs:1233). A plan's new leaves
  in one 2 MiB unit (sorted by VA) are scanned for maximal stretches of 4 KiB sysmem leaves with
  one kind and one snoop setting, contiguous in VA. A stretch is gathered when both hold:
  - it has at least 16 leaves (`GATHER_MIN_LEAVES`);
  - coalescing would still split it into at least 8 runs (`GATHER_MIN_RUNS`).
- **How** (`HostMemory::gather`, hostmem.rs:1473).
  1. Every page must be a page of a registered guest-RAM descriptor, named by the handle and
     offset the filter resolved. Anything else is refused before any system call.
  2. The pages are placed back to back in a fresh VA window of the device with `mremap(old_size
     0)`. That call makes a second mapping of the device's own `MAP_SHARED` mapping of guest RAM
     (guestmem.rs `map`), and consecutive pages go in one call.
  3. One OS descriptor is made over the window. The RM pins the pages by page array and maps
     them for the GPU, as for a registered chunk.
  4. The window is unmapped again. The RM holds the pages by their page array, not by the VA
     (`os_unlock_user_pages`, os-mlock.c:311, needs only the array), so no mapping piles up.
- The stretch's leaves are rewritten to the gather at offsets 0, 4 KiB, and so on. The existing
  coalescing then maps them with one call. The leaves keep the guest's addresses as their
  identity for the diff.
- **Lifetime.** A gather counts the runs mapped from it (`run_gone`, shadow.rs:1063). It is
  freed (`free_idle_gathers`, shadow.rs:1074) after its last mapping is gone:
  - by an unmap (a split keeps it);
  - with its region object (`free_region_whole`);
  - by the teardown.

  Gathers the shadow still holds at device shutdown are freed after the mirror objects and
  before the guest-RAM descriptors (hostmem.rs `free_mirrors`, the "leftover gather" line). By
  then the worker has already finished every teardown (`wait_idle`, mirror.rs:2326).
- **If the host refuses a gather** (`gather of N leaves ... refused` on the sync line), the
  stretch maps run by run from the registered descriptor, as before. If a map from a gather is
  refused, the leaves map one by one from it. A gather no map used is freed again.
- **Cost.** About 2.8 ms per gather over the whole census (135 gathers, 377 ms): pinning and
  IOMMU mapping of the pages, plus one system call per run of consecutive pages. A context
  creation takes about 17 gathers and 45 ms (`gather-2/calls.md`).

**Whole-region frees** (`CARAXES_SHADOW_BULK_FREE`, default on; 0 unmaps run by run).
- When the guest empties a 2 MiB region that holds at least 4 runs (`BULK_MIN_RUNS`), one
  `RM_FREE` of the region object replaces one unmap per run (`free_region_whole`,
  shadow.rs:887; the branch at shadow.rs:1194). The RM unmaps the object's mappings itself
  inside the free.
- The teardown frees whole regions too, about 64 mappings per step (`teardown_step`,
  shadow.rs:1578).
- Measured without gathers (`final`), this took 5737 unmap calls per context creation to 0. It
  bought only 20 % of their time, though: the RM's own unmap costs about 80 us per mapping
  inside the free (12.2 frees, 450 ms per context creation). It pays off with the gathers, which leave few mappings
  per region.

**Region free list** (`CARAXES_SHADOW_REGION_POOL`, default on; 0 frees at once).
- When the guest empties a region, its object goes on a per-VAS list of at most 64
  (`REGION_POOL_MAX`) instead of being freed, but only if all of these hold (`let clean`,
  shadow.rs:1212):
  - the guest wants nothing in the region;
  - no unmap in it failed in this plan;
  - no run and no mirrored leaf is left in it;
  - it does not touch an excluded set-B range.
- The next need of the same region with the same page class takes the object back. A need with
  the other class, or a region that becomes set B (`purge_pool`, shadow.rs:911), frees it.
- The list lives inside one shadow VAS. It saves an allocation and a free per churned region,
  about 0.1 ms each (the census's 64 MiB allocations). It does not carry objects from one
  context to the next, because each guest root gets a new host VAS. A context's 87 region
  objects cost 6-12 ms, and reusing whole shadow VA spaces would also have to keep the host
  RM's own set-B buffers off pooled VAs. That was not worth it.

**Deferred TLB invalidation** (`CARAXES_SHADOW_DEFER_TLB=1`, off by default).
- Every map and unmap carries `DEFER_TLB_INVALIDATION` (NVOS46 bit 31, nvos.h:2151; NVOS47 bit
  0, nvos.h:2192). The shadow then owes the VAS one `DMA_INVALIDATE_TLB`, which `settle`
  (shadow.rs:853) pays:
  - before any virtual object is freed;
  - at the end of every plan, before the doorbell barrier can let a channel run;
  - at the end of every teardown step.
- Measured (`defer-pool`), it saved nothing. In the context windows a map took 28 us and an
  unmap 94 us per call without the deferral, and 32 us and 110 us with it. So the RM's per-call flush and invalidate (virt_mem_allocator_gm107.c:2567-2577) are not
  where the time goes.
- It stays in the code, off, because the control does not repeat the flush the per-call path
  makes (`kbusFlush_HAL`, virt_mem_allocator_gm107.c:2573-2574). Without a gain, that
  difference is not worth carrying.

**Counters.** Every host call of the shadow is counted by kind with its time (`RmCalls`,
shadow.rs:217):
- on every sync line: `RM calls N: map a (x us), unmap b (y us), object alloc, object free,
  invalidate, gather alloc, gather free; regions reused R, pooled P`;
- on the teardown line (its own calls) and in the VAS summary, with the gathers and the
  whole-region frees.

`rpc-cost.py` prints the histogram for a trace window (section 3). The run directory's `calls.py`
splits it per census operation.

### 7.4 Fail-closed argument

**What the physical GPU can reach is unchanged.** Every mapped leaf passed `resolve_leaf`
(shadow.rs:292) in the same plan:
- the whole leaf lies in this VM's reservation or in one registered guest-RAM descriptor;
- its kind is not compressible (D16);
- it is not in a set-B range.

A gather is built only from the (descriptor, offset) pairs of such leaves, and `gather` checks
each pair against the registered descriptors again. The GPU therefore reaches through a gather
exactly the guest pages it would reach through the registered descriptor, through IOVAs of
their own. The gather adds a second IOMMU mapping and a second pin of pages that are already
pinned for the VM's life. It adds no page, and no host memory is pinned beyond what D13 already
pins. Kind and snoop are those of the leaves (one per stretch). The compression rule is untouched
because no compressible leaf reaches a plan.

**Coalescing proves contiguity; it does not assume it.** A run is only leaves that are
contiguous in VA and in one backing's offsets, with the same page size, kind, backing handle
and snoop, inside one region object (`coalesce`, unchanged). For a gathered stretch the backing
offsets are contiguous by construction, and VA contiguity is checked leaf by leaf before the
gather is made. Anything that does not qualify maps the old way, down to one leaf per call. The
unit tests (shadow.rs, against a fake host that records every call):
- `coalescing_keeps_holes_permissions_and_order_apart`: holes, reversed pages and snoop changes
  each split a run;
- `a_compressible_leaf_splits_the_run_and_is_never_mapped`: a compressible leaf in the middle of
  a contiguous stretch is refused by the filter (D16), never reaches a map call, and the run
  splits around it;
- `a_scattered_sysmem_stretch_is_mapped_through_one_gather`: 32 reversed pages give one gather
  with the pages in VA order and one map;
- `contiguous_short_or_mixed_stretches_are_not_gathered`: a contiguous stretch, a short one and
  a stretch across a VA hole;
- `a_refused_gather_maps_run_by_run_from_the_registered_descriptor`.

**No stale leaf survives, and no memory object is freed under a mapping.**
- A gather is freed only after its last mapping. The fake host fails the test if a gather is
  freed while mapped (`a_gather_lives_until_its_last_mapping_is_gone`, with and without
  whole-region frees; `the_teardown_frees_gathers_after_their_mappings`).
- A whole-region free forgets exactly the runs of that region. The RM takes every mapping with
  the object, and a later map of the region gets a fresh object
  (`an_emptied_region_with_many_runs_is_freed_whole`).
- A region goes on the free list only when the RM confirmed every unmap in it. A region with a
  failed unmap is freed instead, and the RM takes what is left with the object
  (`a_region_with_a_failed_unmap_is_freed_not_pooled`). A pooled object holds no mapping
  (`an_emptied_region_is_pooled_and_taken_back_without_new_objects`). It is never taken back
  under set B (`a_pooled_region_under_a_new_exclusion_is_freed`).
- The budget counts mappings. A gather run is one mapping, as a coalesced run was.

**Ordering.**
- The doorbell barrier is unchanged: a plan, including its gathers, is complete before the
  worker reports it done.
- The teardown order is unchanged:
  1. unmaps or whole-region frees, about 64 mappings per step, with the host RM lock released
     between steps. Each gather is freed right after its last mapping;
  2. the remaining objects;
  3. the VAS.

  The guest-RAM descriptors are freed only after all of this (`wait_idle`).
- A whole-region free holds the host RM lock for one object's mappings: at most 512 for 4 KiB
  pages, and with gathers usually a handful.
- With deferral on, `free_virtual` pays the owed invalidate before any object is freed
  (`a_whole_region_free_pays_the_owed_invalidate_first`,
  `no_object_is_freed_before_the_owed_invalidate`). A refused invalidate turns the deferral
  off for the VAS (`a_refused_invalidate_turns_the_deferral_off`).
- With all four switches at 0 (`GATHER`, `BULK_FREE` and `REGION_POOL` at 0, `DEFER_TLB`
  unset), the calls are the earlier sequence (`switches_off_make_the_earlier_calls`).

### 7.5 Measurements

vm0 only, 1 GiB at slot 0, 610 guest. The environment is `caraxes/runs/intr-page-2026-09-24/env.sh`
without the display: host RM and execution, token pool 64, hybrid reactor, interrupt page,
`CARAXES_BAR2_MEMFD=1`, no BAR1 pool. Each run is one VM session with the census probe (cuInit
x3, cuCtxCreate/Destroy x5, the ops x10), `bench-cu` init and churn with 3 reps, and the cuda
gate (not in `before`).

| run | binary, switches | what it isolates |
|---|---|---|
| `before` | 4fec5bd, deferral and pool off | the calls of main d466e9d, counted |
| `defer-pool` | 4fec5bd, defaults | deferred invalidation and the free list |
| `final` | f0edbb8, defaults | plus whole-region frees, deferral off |
| `gather`, `gather-2` | ac950e9, defaults | plus gather descriptors. In `gather`, vm3 was active during the bench runs, which is why `gather-2` was run |

Guest medians (census-cu, us) and shadow work per repetition. Full tables in `compare.md`:

| operation | before | final | gather | gather-2 | native | RM calls per rep, before, then gather-2 | apply ms, before, then gather-2 |
|---|---|---|---|---|---|---|---|
| cuInit | 13847 | 22811 | 12144 | 28269 | 85053 | 0, 0 | 0, 0 |
| cuCtxCreate | 815038 | 837913 | 352252 | 309027 | 146463 | 13171, 228 | 770, 67 |
| cuCtxDestroy | 170081 | 154186 | 132913 | 107017 | 86190 | 1145, 79 | 38, 7 |
| cuMemAlloc 64 MiB | 907 | 850 | 945 | 822 | 136 | 58, 58 | 7.5, 3.9 |
| cuMemAllocHost 1 MiB | 2046 | 1500 | 1470 | 1605 | 758 | 20, 8 | 1.7, 0.8 |
| cuMemHostRegister 1 MiB | 1750 | 1698 | 1660 | 1763 | 273 | 43, 4 | 4.4, 5.1 |

- cuInit makes no shadow plan. Its spread, 12-28 ms, is run to run.
- The allocations were already off the critical path (alloc-path.md §1.1). Their guest time is
  the traps of their episode.

The host RM call histogram of the whole census (`census-rpc-cost.md`, section 3), calls with
total time:

| kind | before | gather-2 |
|---|---|---|
| map | 55025 (1577 ms, 29 us each) | 1102 (87 ms) |
| unmap | 37923 (3075 ms, 81 us each) | 682 (47 ms) |
| object alloc | 894 (113 ms) | 889 (83 ms) |
| object free | 813 (46 ms) | 895 (111 ms, the whole-region frees) |
| gather alloc / free | - | 135 (377 ms) / 135 (19 ms) |
| total | 94655 (4811 ms) | 3838 (724 ms) |

Every context window, in ms (window, apply; RM calls). Five creations in a row:

| run | create 1 | create 2 | create 3 | create 4 | create 5 |
|---|---|---|---|---|---|
| before | 479, 33; 749 | 808, 509; 16946 | 1409, 1372; 15396 | 815, 499; 17210 | 1649, 1436; 15552 |
| gather-2 | 525, 20; 221 | 312, 117; 223 | 217, 26; 237 | 309, 128; 219 | 224, 43; 238 |

`bench-cu` in the same sessions (`bench-summary.md`, median of 3 processes):

| | before | final | gather | gather-2 |
|---|---|---|---|---|
| cuCtxCreate in `init` | 1719 ms | 675 ms | 307 ms | 353 ms |
| cuCtxDestroy in `init` | 193 ms | 172 ms | 125 ms | 151 ms |
| device CPU, `init` | 1970 ms | 1710 ms | 400 ms | 480 ms |
| churn: 1 MiB alloc / free | 180 / 323 us | 182 / 296 us | 342 / 349 us | 177 / 303 us |
| churn: 64 MiB alloc / free | 583 / 848 us | 553 / 830 us | 567 / 896 us | 594 / 887 us |

The `gather` churn row ran while vm3's device was busy (`bench-summary.md`, "6/6"). `gather-2`,
with no other VM active, has churn back at `before`: the churn allocates vidmem, which the
gathers do not touch.

What is left of a context creation (about 220-310 ms against 146 native) is not the shadow.
In the census trace, the window holds 20 first doorbells, each followed by 20-35 ms before the
guest's next access. That is the GPU's first work on the channel: host channel setup and the
first submission.

**Gates.** The cuda gate gave 11 of 12 in `defer-pool`, `final`, `gather` and `gather-2`, with
cuda-managed at its known 801. There were 0 host Xid in every part (device pids 1821188,
1841816, 1873527, 1913470, 1932846) and no host watch alert. `nvidia-smi` took at most 1.77 s
in `before`, 0.81 s in `final` and 0.13 s in the gather runs.
