<!-- SPDX-License-Identifier: MIT -->
# Memory model for real execution (E5b part 1)

Written 2026-09-23 with the implementation in `crates/vfio-user-nvgpu` (`fb.rs`
`FbLayout`, `hostmem.rs`, `dmamap.rs`, `guestmem.rs`, `privilege.rs`) and
`tools/caraxes-vm.sh`. Results: [experiments.md](experiments.md), "E5b part 1".
Evidence: `caraxes/runs/e5b-2026-09-23/`.

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
  2 MiB rounding) shares the alias. A DMA buffer the guest places there would fault on the
  physical GPU. The device's own table does not carry the alias: it never reads the flush
  buffer, and an access would show as a refused address.
- **The clean fix.** A registered chunk below 2^40. That is possible only before the
  host's 64-bit switch, or with a host-side IOVA mapping (S4).

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
