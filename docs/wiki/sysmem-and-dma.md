<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# System memory and DMA

A GPU reads guest RAM through the host's IOMMU at addresses the host driver chooses
(IOVAs), while the guest's driver writes the addresses it knows (guest physical addresses,
GPAs) into its page tables. Making the two agree without changing the guest driver is the
DMA problem of VFIO Leandro; VirtIO Leandro never has it, because the host backend
registers memory on the guest's behalf.

## How the host RM registers memory

- Each descriptor is one `NV01_MEMORY_SYSTEM_OS_DESCRIPTOR` over a host virtual range, pinned
  with `FOLL_LONGTERM` (`kernel-open/nvidia/os-mlock.c`) and mapped for the device with one
  `dma_map_sg` call, which gives one contiguous IOVA range aligned to the descriptor's
  power-of-two size, allocated top-down (measured: `caraxes/sysmem-iova.md`).
- A descriptor is at most 4 GiB minus 4 KiB. The first one that does not fit below 4 GiB
  switches the host GPU to 64-bit IOVAs until the host reboots; chunks then lie near 2^47.
- The RM holds the pages by page array, so the window the descriptor was made over can be
  unmapped afterwards (`os-mlock.c`), which the gather descriptors use.

## The two architectures

| | VirtIO Leandro | VFIO Leandro |
|---|---|---|
| who registers | the backend, per guest request, in a per-VM pin budget (`LEA_MAX_PIN_TOTAL_MIB`, 1024 by default) | the device, the guest's whole RAM in 1 GiB chunks at start (today) |
| addresses the guest writes | none: the backend maps and the guest gets a handle | GPAs, as on bare metal |
| translation | in the backend's request translation | in the shadow page tables ([page-tables-and-shadow](page-tables-and-shadow.md)) |
| guest change | the Leandro module | `caraxes_dma_range` (decision D13): a small module that makes the guest's DMA addresses equal to the host IOVAs (IOVA = GPA + one offset) until the device translates every entry itself |

## Today's model in VFIO Leandro

- Guest RAM is registered as IOVA = GPA + one offset: 1 GiB chunks from the highest GPA
  down plus a filler over the [3, 4) GiB hole, restarting below any host allocation in the
  way (`CARAXES_LINEAR_IOVA`). The linear layout closed the chunk-crossing hole (128 refused
  crossing leaves before, 0 after).
- Registration is refused when pinning would leave the host under 4 GiB of `MemAvailable`;
  every chunk's IOVA is checked with `GET_SURFACE_PHYS_ATTR`, a privileged control raised
  on one thread only (decision D12).
- `GuestMemory::translate` is the device's only path to guest RAM; an address outside the
  table is refused and counted, never passed through.
- The swiotlb alias: a GSP-client guest refuses to initialise when its one-page sysmem flush
  buffer has a DMA address at or above 2^40
  (`src/nvidia/src/kernel/gpu/mem_sys/arch/maxwell/kern_mem_sys_gm107.c`). With 64-bit
  IOVAs the launcher aliases the guest's swiotlb pool, rounded out to 2 MiB, at IOVA
  0xf000000000 in the guest's table only; the device accepts an alias only when the table
  describes exactly the registered RAM and maps alias leaves from the registered descriptor.
  A missing alias caused a host Xid 31 on pinned buffers in the rounding margins.

## The target model (decision D18)

- Sparse, demand-driven registration: when a validated guest entry first points into a
  2 MiB guest-RAM region, the device pins and registers that region (a cached, refcounted
  mapping object: GPA, length, host VA, RM handle, IOVA, permissions), reuses it, and
  releases it with hysteresis when the last entry disappears.
- The guest writes GPAs and the device translates; GPA is never equal to the GPU's DMA
  address. Then the guest module and the ACPI `_DMA` patches go: the vanilla gate.
- Why not the alternatives: an ACPI `_DMA` method needs a Cloud Hypervisor patch and an
  x86 Linux change ([patches/ACPI-DMA.md](../../patches/ACPI-DMA.md)); a vIOMMU cannot line
  up two allocators; host-side tricks cannot see the guest's tables
  ([caraxes/dma-routes.md](../caraxes/dma-routes.md)).

## Depth

- [research/memory-model.md](../research/memory-model.md): the cited lines and the rules.
- [caraxes/memory-model.md](../caraxes/memory-model.md) §2, [caraxes/sysmem-iova.md](../caraxes/sysmem-iova.md),
  [guest-module/caraxes_dma_range/README.md](../../guest-module/caraxes_dma_range/README.md).
