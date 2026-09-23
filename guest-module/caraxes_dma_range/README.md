<!-- SPDX-License-Identifier: GPL-2.0-only -->
# caraxes_dma_range

- Installs a DMA range map (`dev->dma_range_map`) on the NVIDIA GPU's PCI device
  in a Caraxes guest **before `nvidia.ko` binds**. From then on, every DMA
  address the guest kernel computes for that device is the guest physical
  address plus the offset of the range that holds it.
- Stock kernel API only, no NVIDIA symbols, no kernel or driver patch. Built and
  tested on the guest's Ubuntu 6.8.0-136. Results:
  [caraxes/runs/dma-range-2026-09-23](../../caraxes/runs/dma-range-2026-09-23).

> **Status (2026-09-23): deprecated as the long-term mechanism.** The replacement is the
> ACPI `_DMA` route: Cloud Hypervisor patch `patches/0004-acpi-dma-host-bridge.patch` plus the
> x86 Linux change `patches/linux/0001-x86-acpi-honour-dma-ranges.patch`, described in
> [patches/ACPI-DMA.md](../../patches/ACPI-DMA.md). This module stays in the tree as the working
> aid until that route has passed its VM tests, and as a small self-contained example of Linux
> kernel programming (see `WALKTHROUGH.md`). Do not build new features on it.

## Why it exists, and why it is temporary

- The guest has no vIOMMU, so without the map the addresses it gives the GPU are
  guest physical addresses. The physical GPU sits in a translated host IOMMU
  domain and reaches only the IOVAs the host driver mapped. The measurement and
  the options are in [sysmem-iova.md](../../docs/caraxes/sysmem-iova.md) §1-3.
- The device registers guest RAM with the host RM in 1 GiB chunks. Each chunk
  gets one contiguous IOVA range. This module makes the guest use those IOVAs
  (sysmem-iova.md §4, option (b)).
- This is **decision D13** in [docs/caraxes/README.md](../../docs/caraxes/README.md):
  a temporary guest aid with a removal plan, and the only guest-side patch in
  the plan.
- **Removal** (sysmem-iova.md §5). Delete the module, its modprobe.d lines and
  the launcher step once one of these holds:
  1. Upstream Linux applies ACPI `_DMA` on x86 and Cloud Hypervisor emits
     `_DMA` from the device's table.
  2. A host component maps guest RAM at IOVA = GPA in the GPU's domain
     (cuda-path.md §3.2 S4). The table and the device's reverse translation go
     away as well.
  3. The execution model changes so that no guest-written sysmem address
     reaches the GPU.
- Isolation is not improved by this module (parked idea P7). The trusted-guest
  assumption stays.

## Files

| File | Responsibility |
|---|---|
| `caraxes_dma_range.c` | The module: parsing, validation, install, bus notifier, readback |
| `caraxes_dma_range_test.c` | Check of the DMA API against the map (load only with no driver bound) |
| `caraxes_dma_range_pci.h` | Device lookup shared by both |

## Parameters

| Parameter | Meaning |
|---|---|
| `ranges=` | `cpu_start:dma_start:size[,...]` in hex (`0x` optional), at most 64 entries. Required |
| `bdf=` | `[DDDD:]BB:DD.F`. Default: the first `10de` VGA controller, else the first `10de` 3D controller |
| `force=1` | Replace a DMA range map the device already has, with a warning. The replaced map is restored on unload |

`ranges` has its own parameter handler: a `charp` parameter stops at 1024
bytes, and 64 entries of 64-bit IOVAs need about 2.5 KiB.

**Validation.** The load is refused (`EINVAL`) if any of these fails:

- `size` must be a non-zero multiple of 2 MiB.
- Both starts must be page aligned.
- No entry may wrap around the address space.
- Entries must not overlap, in CPU space or in DMA space.
- The table must **cover every System RAM page** (`/proc/iomem`). A page it
  does not cover gets `DMA_MAPPING_ERROR`. There is also a v6.8 bug:
  `check_ram_in_range_map()` loops forever on a RAM range that is covered at
  its start but not to its end.

A range whose CPU and DMA starts differ modulo 2 MiB loads, but with a warning:
GPU big pages would lose their alignment.

## Behaviour

- **Refuses if a driver is bound** to the device (`EBUSY`), with or without
  `force`. Changing the translation under a driver that already holds mappings
  would make them translate back wrongly. Unload nvidia first.
- **Refuses if a map exists** (`EEXIST`) unless `force=1` is given.
- **Installs** a `kmalloc` copy of the table with a zero-size terminator, the
  layout `translate_phys_to_dma()` walks. It logs one line per range.
- **The driver core owns the installed copy.** `device_unbind_cleanup()`
  (drivers/base/dd.c) runs whenever a driver unbinds or a probe fails. It
  `kfree`s `dma_range_map` and clears it, and `device_release()` frees it
  with the device.
- **Re-installs at every bind.** A PCI bus notifier puts a fresh copy in place
  at `BUS_NOTIFY_BIND_DRIVER` whenever the device has no map. That event comes
  after the bus's `dma_configure` and before the driver's probe. So
  `rmmod nvidia; modprobe nvidia` keeps the map without reloading this module.
- **Pins itself while a driver is bound.** It takes a module reference at
  `BUS_NOTIFY_BOUND_DRIVER` and drops it at `UNBOUND_DRIVER`. Until nvidia
  unbinds, `rmmod caraxes_dma_range` fails with "Module is in use".
- **Unload** (only possible with no driver bound):
  - It frees its map if that map is still in place, and restores the map that
    `force=1` replaced (or leaves none).
  - It never frees a map it did not install.

## Readback

Everything is under `/sys/module/caraxes_dma_range/parameters/`, read-only:

| File | Content |
|---|---|
| `table` | The table the module installs, one `0xcpu:0xdma:0xsize` line per range. Joined with `,` it is a valid `ranges=` again |
| `live` | The map the device carries right now, in the same format. It is read under the device lock, and is empty when there is none |
| `status` | `device= driver= map=ours\|none\|foreign ranges= installs= pinned= previous=kept\|none` |
| `ranges` | The string given at load |

A userspace check that the table is in place:

```sh
P=/sys/module/caraxes_dma_range/parameters
diff "$P/table" "$P/live" && grep -q 'map=ours' "$P/status"
```

## Build, load order, use

Build inside the guest, which needs `linux-headers-$(uname -r)`:

```sh
make -C guest-module/caraxes_dma_range      # or: make -C /lib/modules/$(uname -r)/build M=$PWD
```

The module must be loaded **before `nvidia`**. There are two ways to do that:

- **Launcher.** The table changes with every VM start, so this is the
  preferred way. Keep nvidia from loading at boot, then have
  `tools/caraxes-vm.sh` run, in the guest:

  ```sh
  sudo insmod caraxes_dma_range.ko ranges="$(cat table)"
  echo 1 | sudo tee /sys/bus/pci/devices/0000:00:05.0/reset   # as `probe` does
  sudo modprobe nvidia
  ```

- **modprobe.d**, for a table that is written before nvidia loads. Install the
  module with `make -C /lib/modules/$(uname -r)/build M=$PWD modules_install`
  and run `depmod -a`, then add, for example in
  `/etc/modprobe.d/caraxes-dma-range.conf`:

  ```
  softdep nvidia pre: caraxes_dma_range
  options caraxes_dma_range ranges=0:40000000:40000000,...
  ```

  The softdep also applies when udev loads nvidia by modalias. If nvidia is in
  the initramfs, this module and its options must be there too.

Because of the bind notifier, loading this module at any time before nvidia's
next probe is enough. Changing the table means:
`rmmod nvidia_uvm nvidia; rmmod caraxes_dma_range; insmod ... ranges=<new>; modprobe nvidia`.

**Test module.** Load it only while no driver is bound:
`sudo insmod caraxes_dma_range_test.ko; sudo rmmod caraxes_dma_range_test`.

- It holds the device lock and sets the 47-bit mask nvidia uses (restoring the
  old mask afterwards).
- On the GPU's device it calls:
  - `dma_map_page` on 256 fresh pages;
  - `dma_alloc_coherent` 8 times;
  - `dma_map_page(DMA_TO_DEVICE)` at the first, middle and last page of every
    range, with sub-page offsets.
- It checks every result against guest physical address + range offset, and
  `dma_to_phys()` back. It logs one line per range and `PASS` or `FAIL`; the
  load fails with `EIO` on `FAIL`.
- The device is never touched: without a vIOMMU a mapping only computes an
  address (dma-direct).

## How the device produces the table

This summarizes sysmem-iova.md §4.

1. When Cloud Hypervisor sends `DMA_MAP` for a memory zone, the device
   registers the zone with the host RM in **1 GiB chunks**, one
   `NV01_MEMORY_SYSTEM_OS_DESCRIPTOR` each. A trailing piece is its own chunk,
   and its size must be a multiple of 2 MiB.
2. For each chunk it reads the IOVA base with one privileged
   `GET_SURFACE_PHYS_ATTR` at offset 0 (D12). It checks the last page and every
   2 MiB, and refuses to start if a chunk is not one range.
3. It writes one `gpa:iova:size` entry per chunk: 4 entries for a 4 GiB guest,
   whose zones are [0, 3 GiB) and [4 GiB, 5 GiB). It publishes them as a file
   that the launcher passes as `ranges=`. A vendor capability in config space
   could replace the file later.
4. The table must cover all guest RAM, including swiotlb, which lies in low
   memory. Entries may also cover non-RAM, such as the first page and the
   legacy hole.

## What the device must then do

- **Treat every guest sysmem address as an IOVA.** This covers GSP queue PTE
  arrays, fault and access-counter buffers, USERD, method buffers and page
  tables.
- **Translate back for its own accesses.** Every guest-memory access the device
  makes with an address from the guest maps IOVA to GPA through the same table,
  in the guest-memory accessor. An address outside the table is an error, never
  passed through.
- **Keep the table fixed while nvidia is bound in the guest.** Re-registering a
  chunk can give a different IOVA; that needs the reload sequence above.
- **Free in this order:** guest driver quiesced or VM stopped, then the host
  channels freed and the GPU idle, then the descriptors freed. Unmapping in
  DMA-FQ is lazy and freed IOVAs are reused at once (sysmem-iova.md §2).
  The guest's copy of the map is freed by the guest driver core on unbind and
  needs nothing from the device.
- **Keep UVM's page tree in vidmem.** A sysmem root is passed to the GPU as
  `page_to_phys`, which bypasses this map (sysmem-iova.md §3(a)).

## Kernel versions

| Version | What matters |
|---|---|
| up to 6.7 | `struct bus_dma_region` has a `u64 offset` field. It is filled under a `LINUX_VERSION_CODE` guard |
| 6.8 (guest) | The `offset` field is gone. `walk_system_ram_range` is not exported, so `walk_iomem_res_desc` is used. `dma_direct_set_offset()` exists but is not exported, takes a single range, and says calling it from a driver is a bug. `check_ram_in_range_map()` has the endless-loop bug on partial coverage |
| 6.12 | Same struct. `dma_range_map_min/max()` helpers appear, and OF/ACPI use them to set `bus_dma_limit`; this module leaves `bus_dma_limit` alone. The loop bug is still there. `device_unbind_cleanup()` still frees the map |
| 7.x | The loop bug is fixed (`dma_find_range()`). The OF and arm64 ACPI configure paths skip devices that already have a map ("dma_range_map already set"). The core sets the driver with `device_set_driver()`. The unbind cleanup and `device_release()` still `kfree` the map. The module compiles against 7.2 headers but is untested there |
