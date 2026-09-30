<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# System memory and DMA

A GPU reads guest RAM through the host's IOMMU at addresses the host driver chooses
(IOVAs), while the guest's driver writes the addresses it knows into its page tables. In
VFIO Leandro the guest writes its guest physical addresses (GPAs), as on bare metal, and the
device translates them to IOVAs of registered guest RAM when it builds the shadow page tables.
VirtIO Leandro never has the problem, because the host backend registers memory on the
guest's behalf.

## How the host RM registers memory

- Each descriptor is one `NV01_MEMORY_SYSTEM_OS_DESCRIPTOR` over a host virtual range, pinned
  with `FOLL_LONGTERM` (`kernel-open/nvidia/os-mlock.c`) and mapped for the device with one
  `dma_map_sg` call, which gives one contiguous IOVA range aligned to the descriptor's
  power-of-two size, allocated top-down ([caraxes/sysmem-iova.md](../vfio/sysmem-iova.md)).
- A descriptor is at most 4 GiB minus 4 KiB. The first one that does not fit below 4 GiB
  switches the host GPU to 64-bit IOVAs until the host reboots; chunks then lie near 2^47.
- The RM holds the pages by page array, so the window the descriptor was made over can be
  unmapped afterwards (`os-mlock.c`), which the gather descriptors use.

## The two architectures

| | VirtIO Leandro | VFIO Leandro |
|---|---|---|
| who registers | the backend, per guest request, in a per-VM pin budget (`LEA_MAX_PIN_TOTAL_MIB`, 1024 by default) | the device, the guest's whole RAM, at `DMA_MAP` |
| addresses the guest writes | none: the backend maps and the guest gets a handle | GPAs, as on bare metal |
| translation | in the backend's request translation | in the shadow page tables ([page-tables-and-shadow](page-tables-and-shadow.md)) |
| guest change | the Leandro module | none |

## Registration in VFIO Leandro

- With `--host-exec` the guest's DMA addresses are its GPAs and no guest module is loaded
  (`crates/vfio-user-nvgpu/src/device.rs` `with_guest_dma_gpa`; the default since
  2026-09-30, [run: flags-2026-09-30](../../caraxes/runs/flags-2026-09-30/NOTE.md)).
- The device answers a `DMA_MAP` once the region is mapped and queues it; a worker
  (`registrar.rs`) registers it in 1 GiB pieces, each faulted in first (`MADV_POPULATE_WRITE`),
  so the vfio-user loop's RM calls wait at most for one pin. Until a piece is registered the
  device's own accesses go through and the shadow's descriptor snapshot waits for the worker; a
  `DMA_UNMAP` cancels the queued pieces of its region
  ([research: windows-findings](../research/windows-findings.md), "Guest RAM registration off
  the loop").
- The IOVA layout is linear, IOVA = GPA + one offset: 1 GiB chunks from the highest GPA down
  plus a filler over the [3, 4) GiB hole, restarting below any host allocation in the way. It
  closed the chunk-crossing hole (128 refused crossing leaves before, 0 after).
- Registration is refused when pinning would leave the host under 4 GiB of `MemAvailable`,
  counting what is still queued; every chunk's IOVA is checked with `GET_SURFACE_PHYS_ATTR`, a
  privileged control raised on one thread only (decision D12).
- `GuestMemory::translate` is the device's only path to guest RAM; an address outside the
  table is refused and counted, never passed through.
- A GSP-client guest refuses to initialise when its one-page sysmem flush buffer has a DMA
  address at or above 2^40
  (`src/nvidia/src/kernel/gpu/mem_sys/arch/maxwell/kern_mem_sys_gm107.c`); with GPA DMA that
  address is a GPA.
- When the VMM reconnects (a guest reboot under Cloud Hypervisor), the device resets and
  releases the old client's registrations (windows-findings, Cloud Hypervisor step 23).

## The helper route

- Decision D13 made the guest's DMA addresses equal to the host IOVAs with a guest module,
  `caraxes_dma_range`, and aliased the guest's swiotlb pool below 2^40 once the host used
  64-bit IOVAs. With `--host-exec` the device no longer uses it; the module and its README
  remain in `guest-module/`.

## The target model (decision D18)

- Done: the guest writes GPAs and the device translates; the guest needs no module.
- Left: sparse, demand-driven registration instead of the whole-RAM pin. When a validated
  guest entry first points into a 2 MiB guest-RAM region, the device pins and registers that
  region (a cached, refcounted mapping object: GPA, length, host VA, RM handle, IOVA,
  permissions), reuses it, and releases it with hysteresis when the last entry disappears.
- Why not the alternatives: an ACPI `_DMA` method needs a Cloud Hypervisor patch and an
  x86 Linux change ([patches/ACPI-DMA.md](../../patches/ACPI-DMA.md)); a vIOMMU cannot line
  up two allocators; host-side tricks cannot see the guest's tables
  ([caraxes/dma-routes.md](../vfio/dma-routes.md)).

## Depth

- [research: memory-model](../research/memory-model.md): the cited lines and the rules.
- [caraxes/memory-model.md](../vfio/memory-model.md) §2, [caraxes/sysmem-iova.md](../vfio/sysmem-iova.md),
  [guest-module/caraxes_dma_range/README.md](../../guest-module/caraxes_dma_range/README.md).
