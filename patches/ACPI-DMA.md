<!-- SPDX-License-Identifier: MIT -->
<!-- Written 2026-09-23 for backlog item N1 (docs/caraxes/README.md, parked idea P8). Citations:
     ACPI = ACPI 6.5 (uefi.org/specs/ACPI/6.5, read through the Wayback Machine because uefi.org
     answered 403); "6.8" = the guest kernel, Ubuntu tag Ubuntu-6.8.0-136.136 (the files cited are
     identical to upstream v6.8 except kernel/dma/direct.c, which carries a stable fix); "6.12" =
     upstream v6.12; "mainline" = torvalds/master at v7.3-rc4; Cloud Hypervisor paths are relative
     to vendor/cloud-hypervisor/ (v53.0); NVIDIA paths are relative to
     vendor/open-gpu-kernel-modules/ (610.57.04). "Inference" marks what was not read in code or
     run. -->
# ACPI `_DMA` for the PCI host bridge

Two patches replace the guest module
[`caraxes_dma_range`](../guest-module/caraxes_dma_range/) (decision D13) with the firmware path:

| Patch | Project | State |
|---|---|---|
| [0004-acpi-dma-host-bridge.patch](0004-acpi-dma-host-bridge.patch) | Cloud Hypervisor v53.0 | Applies to pristine v53.0 and on top of 0001-0003; builds (release); unit tests pass |
| [linux/0001-x86-acpi-honour-dma-ranges.patch](linux/0001-x86-acpi-honour-dma-ranges.patch) | Linux, guest kernel Ubuntu 6.8.0-136 | RFC draft; compile-tested, not booted |

With both patches, the guest kernel installs the device's `dma_range_map` itself, from a table
that Cloud Hypervisor writes into the DSDT. For every DMA address it computes for the device, the
guest then gives the host IOVA: guest physical address + the offset of that range.

## 1. The problem

The facts come from [sysmem-iova.md](../docs/caraxes/sysmem-iova.md):

- The guest has no vIOMMU, so it writes guest physical addresses (GPAs) into GPU page tables.
- The physical GPU sits in a translated host IOMMU domain (DMA-FQ), so it can reach only the IOVAs
  that the host driver has mapped.
- **Measured:** one host OS descriptor (`NV01_MEMORY_SYSTEM_OS_DESCRIPTOR`) gives one contiguous
  IOVA range:
  - The range is aligned to the descriptor's size rounded up to a power of two.
  - Test values: 256 MiB always landed at 0xa0000000, and 1 GiB always at 0x40000000.
  - The allocator works top-down: a second descriptor held at the same time landed directly below
    the first.
  - The base is reproducible while the host's own IOVA use stays the same, but the host does not
    guarantee it (sysmem-iova.md §1.3).
- **Consequence:** the guest has to use DMA address = GPA + a per-range offset. There is one range
  per descriptor, so one per GiB of guest RAM under the sysmem-iova.md §4 plan.
- **Why the table works in the guest:** Linux holds exactly such a table per device,
  `dev->dma_range_map`, an array of `struct bus_dma_region {cpu_start, dma_start, size}` ending in
  a zero-size entry (6.8 `include/linux/dma-direct.h:20-24`).
  - dma-direct consults the table on x86 too (`dma-direct.h:26-40`, `:63-68`).
  - A physical address that no entry covers translates to `DMA_MAPPING_ERROR`.
- **What was missing:** on x86, nothing fills the table from firmware, which is why the guest module
  exists.

## 2. Mechanism

### 2.1 The ACPI `_DMA` object (ACPI 6.5 §6.2.4, §6.4.3.5.1)

- `_DMA` "returns a byte stream in the same format as a `_CRS` object". It is "only defined under
  devices that represent buses" and "is only valid if a `_CRS` object is also defined" (§6.2.4).
- It lists "the ranges the bus controller (bridge) decodes on the child-side of its interface". In
  the spec's example, bus-master addresses "will have 0x200000000 added as they cross the bridge",
  and the example writes that offset as `_TRA` of a `QWORDMemory (ResourceProducer, …)` descriptor
  (§6.2.4).
- In the QWord Address Space Descriptor (§6.4.3.5.1, type 0x8A):
  - `_MIN`/`_MAX` are "the address space on the secondary side of the bridge".
  - `_TRA` is "the offset that must be added to the address on the secondary side to obtain the
    address on the primary side".
  - Bit 0 of the general flags is "Ignored" by the spec.
- **So for DMA:** `_MIN` = `dma_start` (the bus address), `_LEN` = `size`, and
  **`_TRA = cpu_start − dma_start`**, modulo 2^64 when negative.
  - The N1 brief wrote the offset as `dma_start − cpu_start`. That is the other sign. The spec and
    Linux (§2.2) agree on the sign used here.
  - The unit test and the iasl round trip (§3.3) confirm it.
- **Where the object goes.** A host bridge's `_DMA` applies to every device below it. It cannot sit
  on a slot device: the spec allows it only on buses, and Linux also requires a `_CRS` next to it
  (§2.2). One segment's host bridge is therefore the finest scope the spec allows.

### 2.2 How Linux consumes it

1. **Configure DMA for the device.** `pci_dma_configure` runs at every probe. On a host bridge with
   an ACPI companion, it calls `acpi_dma_configure(dev, acpi_get_dma_attr(adev))`
   (6.8 `drivers/pci/pci-driver.c:1655-1669`).
   - On x86, `acpi_get_dma_attr` returns COHERENT or NON_COHERENT, never NOT_SUPPORTED:
     `CONFIG_ACPI_CCA_REQUIRED` is not set (`drivers/acpi/scan.c:1437-1470`), and Cloud Hypervisor
     emits `_CCA` = 1 anyway (`vmm/src/pci_segment.rs:442`).
2. **Architecture hook.** `acpi_dma_configure_id` calls `acpi_arch_dma_setup(dev)` first
   (6.8 `scan.c:1628-1651`, call at `:1638`; 6.12 `:1678`; mainline `:1668`).
3. **Parse `_DMA`.** `acpi_dma_get_range(dev, &map)` (6.8 `scan.c:1484-1540`) does this:
   - It walks from the device up through `dev->parent` to the first ACPI companion with `_DMA`.
     For a PCI function that walk reaches the host bridge. x86 sets the companion in
     `arch/x86/pci/acpi.c:601`.
   - It returns -EINVAL without a `_CRS`.
   - It evaluates `_DMA` through `acpi_dev_get_dma_resources` (`drivers/acpi/resource.c:1058`,
     memory resources only), then allocates `ret + 1` regions with
     `cpu_start = res.start`, `dma_start = res.start − offset` and `size = resource_size(res)`.
   - `acpi_decode_space` sets `res.start = _MIN + _TRA` and `offset = _TRA`, **but only for
     `ResourceProducer`** descriptors, meaning general-flags bit 0 clear
     (`resource.c:206-248`, check at `:232`; ACPICA `rsaddr.c:185`, `ACPI_PRODUCER` = 0).
     Although the spec ignores that bit, Linux does not: a `ResourceConsumer` `_DMA` silently loses
     its offset.
4. **Install the table.** This step exists only on arm64 and LoongArch:
   - arm64 `drivers/acpi/arm64/dma.c:7-56` (6.8), since v5.14 (db59e1b6e492);
   - LoongArch `arch/loongarch/kernel/dma.c:8-30`, since v6.1 (bf2ee8d0c385, c78c43fe7d42).

   Both install `map` as `dev->dma_range_map`, set `bus_dma_limit` to the highest bus address and
   clamp the default masks. On x86, `include/linux/acpi.h:256-261` (6.8) makes
   `acpi_arch_dma_setup` an empty inline for everything other than `CONFIG_ARM64 ||
   CONFIG_LOONGARCH`. **On x86 nothing calls `acpi_dma_get_range`**, so `_DMA` is never evaluated.
   This is still true in 6.12 (`acpi.h:251`) and in mainline (`acpi.h:270`).
5. **Lifetime.** The driver core frees the table: `device_unbind_cleanup` does
   `kfree(dev->dma_range_map)` on unbind and on a failed probe (6.8 `drivers/base/dd.c:607-611`).
   `device_release` frees it as well. A table from `acpi_dma_get_range` is therefore rebuilt at each
   probe and never leaks. The guest module had to rebuild this lifecycle by hand with a bus
   notifier (`caraxes_dma_range.c:12-19`).
6. **Precedent on x86.** The x86 DMA layer already honours a per-device table: the STA2X11 fixup
   sets one with `dma_direct_set_offset` (`arch/x86/pci/sta2x11-fixup.c:144`). x86 does not select
   `ARCH_HAS_PHYS_TO_DMA`, so it takes the generic `phys_to_dma`.

### 2.3 Why x86 never had it

- x86 platforms describe DMA translation through the IOMMU tables (DMAR, IVRS), not through
  bridge offsets.
- The hook was created for arm64's IORT/`_DMA` handling and generalised when LoongArch needed
  multiple windows. x86 had no firmware producing `_DMA` either time, so nobody wired it.

This is a gap, not a decision against x86. There is no mailing-list statement either way
(inference from the commit history: db59e1b6e492, bf2ee8d0c385).

### 2.4 The NVIDIA open module goes through the DMA API on the PCI device

Every sysmem DMA address that the driver gives the GPU comes from the DMA API, called on
`&pci_dev->dev` (`nvl->dma_dev.dev = nvl->dev`, `kernel-open/nvidia/nv-pci.c:2056`):

| Path | Call | Translated by `dma_range_map` |
|---|---|---|
| RM memdesc, contiguous | `dma_map_page_attrs` (`kernel-open/nvidia/nv-dma.c:85`) | yes (dma-direct `phys_to_dma`) |
| RM memdesc, scatter list (OS descriptors, the sysmem-iova case) | `dma_map_sg` (`nv-dma.c:246`) | yes |
| RM coherent allocations | `dma_alloc_coherent` (`kernel-open/nvidia/nv-vm.c:340`) | yes |
| UVM sysmem chunks | `dma_map_page` (`kernel-open/nvidia-uvm/uvm_gpu.c:3979`) | yes |
| UVM coherent | `dma_alloc_coherent` (`uvm_gpu.c:3936`) | yes |
| Peer MMIO (P2P) | `dma_map_resource` (`nv-dma.c:839`) | **no**: `dma_direct_map_resource` returns the address unchanged and only checks it against the mask and `bus_dma_limit` (6.8 `kernel/dma/direct.c:509-523`). Not used by Caraxes (one GPU) |
| UVM page-tree root in sysmem | `page_to_phys` (`uvm_gpu.c:1342-1345`) | **no**: keep the root in vidmem (default; sysmem-iova.md §3(a)) |

- **The raw path is not taken.** The raw `nv_phys_to_dma` path runs only when no remap is required,
  and on 5.0+ kernels remap is required (`nv.c:5243-5250`, `nv-linux.h:957-1020`;
  sysmem-iova.md §3(a)).
- **The masks are compatible.** `dma_direct_supported` accepts every mask of at least 32 bits, so
  the driver's 47-bit `dma_set_mask` succeeds after the ACPI clamp (6.8 `direct.c`). Its new mask
  replaces the clamped default, and `bus_dma_limit` stays at the table's top.

**Result.** One `dma_range_map` on the GPU's PCI function covers every address the driver hands
the GPU. This is the sysmem-iova.md finding, and it holds whether the table comes from the module
or from `_DMA`.

## 3. Cloud Hypervisor patch 0004

### 3.1 Option

```
--pci-segment pci_segment=<id>,dma_ranges=[<cpu_start>:<dma_start>:<size>,...]
```

- Each entry means: a device on segment `<id>` reaches guest physical address `cpu_start + n` at
  bus address `dma_start + n`, for `n < size`. This is the same triple, in the same order, as the
  guest module's `ranges=` parameter.
- Numbers are decimal or `0x`-prefixed hex. The brackets keep v53's `OptionParser` from splitting
  the list at its commas, as in `topology=[…]` (`option_parser/src/lib.rs:88-125`).
- **Validation** (`PciSegmentConfig::validate`):
  - every range is non-empty, 4 KiB aligned in all three fields, and does not wrap;
  - no two ranges overlap on the CPU side or on the bus side, so the table can be read
    unambiguously in both directions (`translate_dma_to_phys` takes the first match).
- **Warning only:** a bus-side range that overlaps the segment's own MMIO windows. The spec treats
  such child-side accesses as peer-to-peer; Linux does not check (§6).
- **API.** `PciSegmentConfig` gains `dma_ranges: Option<Vec<DmaRangeConfig>>`, with `serde(default)`
  so existing configs deserialize unchanged. The OpenAPI schema is extended to match.

**Why `--pci-segment`, and not `--platform` or `--user-device`.**
- `_DMA` belongs to a bus (§2.1), and v53 already has one per-segment option group
  (`--pci-segment`, `vmm/src/config.rs:810-865`), which reaches `DeviceManager` next to the segment
  constructors (`vmm/src/device_manager.rs:1344-1361`).
- A VM-wide `--platform dma_ranges=` would force the translation onto every device of every
  segment, and it would still need a segment id.
- A per-device option on `--user-device` would promise a granularity that ACPI cannot express.
- **Isolation.** A device that needs a translation others must not see gets a segment of its own:
  `--platform num_pci_segments=2` and `--user-device …,pci_segment=1`.

### 3.2 What is emitted

`PciSegment::to_aml_bytes` (`vmm/src/pci_segment.rs`, the `PNP0A08` device `_SB_.PCxx`) adds one
object after `_CRS`, and only when ranges are configured:

```
Name (_DMA, ResourceTemplate () {
    QWordMemory (ResourceProducer, PosDecode, MinFixed, MaxFixed, Cacheable, ReadWrite,
                 0, dma_start, dma_start + size - 1, cpu_start - dma_start, size)
    ...                                   // one per range
})
```

**Why a `Name` and not a `Method`.**
- The table is static, and a `Name` returns the same buffer as a `Method` would. It is how the same
  device already carries `_CRS`.
- Linux only tests that the name exists (`acpi_has_method` → `acpi_get_handle`) and then evaluates
  it.
- iasl accepts it as the predefined `_DMA` with 0 errors and 0 warnings (§3.3).

**Descriptor fields.**
- `ResourceProducer` (bit 0 clear) is required for Linux to apply `_TRA` (§2.2).
- `Cacheable`/`ReadWrite` describe RAM. Linux ignores both for `_DMA`.

**No option, no change.** Without the option, nothing is emitted and the DSDT is byte-identical to
v53.0.

### 3.3 Byte-level example: a 4 GiB guest, two ranges

**RAM layout.** Cloud Hypervisor places 4 GiB of guest RAM at GPA [0, 3 GiB) and
[4 GiB, 5 GiB) (`arch/src/x86_64/layout.rs:86`, `:125`).

**Example table.** Assume the device registered each RAM region as one descriptor after the host
switched to 64-bit IOVAs (sysmem-iova.md §1.4). Top-down allocation gives:

| # | GPA (cpu_start) | IOVA (dma_start) | size | `_TRA` = cpu − dma |
|---|---|---|---|---|
| 0 | 0x0 | 0x7ffe00000000 | 0xc0000000 (3 GiB) | 0xffff800200000000 (= −0x7ffe00000000) |
| 1 | 0x100000000 | 0x7ffdc0000000 | 0x40000000 (1 GiB) | 0xffff800340000000 (= −0x7ffcc0000000) |

Option: `--pci-segment pci_segment=1,dma_ranges=[0:0x7ffe00000000:0xc0000000,0x100000000:0x7ffdc0000000:0x40000000]`

**AML inside `Device (_SB_.PC01)`.** The listing is 104 bytes; the unit test
`pci_segment::unit_tests::dma_ranges_emit_dma_object` asserts these exact bytes:

```
08 5f 44 4d 41                  NameOp "_DMA"
11 42 06                        BufferOp, PkgLength 0x62 (98 = 2 length bytes + 2 size bytes + 94)
0a 5e                           BufferSize: BytePrefix 94
                                -- descriptor 0 (46 bytes) --
8a 2b 00                        QWord Address Space Descriptor, length 43
00                              resource type: memory range
0c                              general flags: MaxFixed | MinFixed, PosDecode, bit 0 = 0 (producer)
03                              type-specific: _MEM = 1 (cacheable), _RW = 1
00 00 00 00 00 00 00 00         _GRA 0
00 00 00 00 fe 7f 00 00         _MIN 0x00007ffe00000000
ff ff ff bf fe 7f 00 00         _MAX 0x00007ffebfffffff
00 00 00 00 02 80 ff ff         _TRA 0xffff800200000000
00 00 00 c0 00 00 00 00         _LEN 0x00000000c0000000
                                -- descriptor 1 --
8a 2b 00 00 0c 03
00 00 00 00 00 00 00 00         _GRA 0
00 00 00 c0 fd 7f 00 00         _MIN 0x00007ffdc0000000
ff ff ff ff fd 7f 00 00         _MAX 0x00007ffdffffffff
00 00 00 40 03 80 ff ff         _TRA 0xffff800340000000
00 00 00 40 00 00 00 00         _LEN 0x0000000040000000
79 00                           EndTag, checksum 0
```

**Round trip through ACPICA's iasl** (20260408, built from source in the scratch space):
- Disassembling these bytes, wrapped in an SSDT with `_HID PNP0A08`, gives
  `Name (_DMA, ResourceTemplate () { QWordMemory (ResourceProducer, PosDecode, MinFixed, MaxFixed,
  Cacheable, ReadWrite, 0x0, 0x7FFE00000000, 0x7FFEBFFFFFFF, 0xFFFF800200000000, 0xC0000000, …) … })`.
- Recompiling that ASL reports "0 Errors, 0 Warnings, 0 Remarks" and produces **byte-identical**
  AML.

**What Linux derives** (§2.2):
- entry 0: `cpu_start` = 0x7ffe00000000 + 0xffff800200000000 = 0x0 (mod 2^64), `dma_start` =
  0x7ffe00000000, `size` = 0xc0000000;
- entry 1: `cpu_start` = 0x100000000, `dma_start` = 0x7ffdc0000000, `size` = 0x40000000;
- `bus_dma_limit` = 0x7ffebfffffff.

The table is exactly the one given on the command line.

**The same RAM under the §4 plan.** With 1 GiB descriptors (sysmem-iova.md §4) the same 4 GiB are
four entries, 4 × 46 + 2 = 186 bytes of template. A single 3 GiB descriptor is also valid: it is
below the 4 GiB − 4 KiB submap limit, so it is one IOVA range (sysmem-iova.md §2).

### 3.4 Status

**Patch file.**
- `git format-patch` of one commit on top of v53.0 (`9ed824d6d`).
- 5 files, +315/−3; about 170 of those lines are tests: `vmm/src/{config.rs, vm_config.rs,
  pci_segment.rs, device_manager.rs}` and `vmm/src/api/openapi/cloud-hypervisor.yaml`.

**Applies:**
- cleanly with `git apply` to pristine v53.0;
- cleanly after 0001-0003, which is how `tools/build.sh ch` applies the series (0001 also touches
  `device_manager.rs`, in other hunks).

**Builds:** `cargo build --release --bin cloud-hypervisor`, the profile `tools/build.sh` uses, for
both v53.0 + 0004 and v53.0 + 0001-0004. The only warning is the existing `MmapRegion` import
warning from 0001. `--help` shows the new syntax.

**Tests:**
- `cargo test -p vmm --features kvm --lib`: 89 passed on both trees, including
  `test_pci_segment_parsing`, `test_config_validation` (six new cases) and
  `dma_ranges_emit_dma_object`.
- `cargo test -p vmm` without `--features kvm` does not compile at v53.0 itself (`try_clone` on
  `VfioDeviceFd`), so the hypervisor feature is required.
- `cargo clippy -p vmm --features kvm --all-targets`: no findings in the touched code.

**Not done:** no VM was booted.

**Trailers.**
- The commit message ends with `SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause` (the files'
  own header).
- It has **no `Signed-off-by`**: Cloud Hypervisor requires a DCO sign-off, and only the submitter
  can give it.

**Side effect.** `tools/build.sh ch` globs `patches/[0-9][0-9][0-9][0-9]-*.patch`. The next run
therefore applies 0004 as well: it finds 3 of 4 patches applied and reapplies the series. Without
the option the built binary behaves as before.

## 4. Guest kernel requirement: Linux draft

The change is [linux/0001-x86-acpi-honour-dma-ranges.patch](linux/0001-x86-acpi-honour-dma-ranges.patch),
`[RFC PATCH] ACPI: x86: honour _DMA translation for device DMA setup`:

- a new file `drivers/acpi/x86/dma.c` with `acpi_arch_dma_setup()` for x86, modelled on the
  LoongArch one:
  - it leaves an already installed `dma_range_map` alone (as mainline arm64 does);
  - it calls `acpi_dma_get_range()` and returns quietly on -ENODEV;
  - otherwise it installs the table, sets `bus_dma_limit` to the highest bus address, and clamps
    the default coherent and streaming masks;
- `include/linux/acpi.h`: `|| defined(CONFIG_X86)` on the prototype guard;
- `drivers/acpi/Makefile`: `acpi-$(CONFIG_X86) += x86/dma.o`.

**Effect.** Only devices below an ACPI node with `_DMA` are touched; every other device gets
-ENODEV and no change.
- No IOMMU (the Caraxes guest): dma-direct and swiotlb translate. An uncovered physical address
  fails to map rather than passing through.
- Behind an IOMMU DMA domain: iommu-dma ignores the table but bounds IOVAs by `bus_dma_limit`
  (6.8 `drivers/iommu/dma-iommu.c:778`), as on arm64.

**State.**
- Base: `Ubuntu-6.8.0-136.136` (`9e2939f04a28`). Also applies to upstream v6.8.
- Compile-tested with GCC 16.2.1 and `W=1`, with no warnings in the new objects:
  - `drivers/acpi/x86/dma.o` and `drivers/acpi/scan.o` with `x86_64_defconfig`;
  - the same objects with Ubuntu's generic amd64 config, exported from
    `debian.master/config/annotations`;
  - a full `vmlinux` link with the defconfig succeeded (`CONFIG_WERROR` off, because GCC 16 trips
    unrelated `-Werror` checks in v6.8), and `acpi_arch_dma_setup` is a `T` symbol in it.
- **Not booted.**
- checkpatch: the code is clean. It asks for a `Signed-off-by` (the submitter's) and flags the
  `Co-Authored-By` spelling and the SPDX trailer. Adjust or drop both before posting.

**6.12 and mainline.** The same change with two differences, both written out in the patch notes:
- the object goes into `drivers/acpi/x86/Makefile` as `acpi-x86-y += dma.o`;
- the highest-address loop becomes `dma_range_map_max(map)` (since v6.10).

That variant compiled on v6.12 (defconfig, W=1). It was not built on mainline.

**Dynamic debug.** `dyndbg="file drivers/acpi/x86/dma.c +p"` on the guest command line prints
`ACPI _DMA: N range(s), bus limit …` at each probe of an affected device. Ubuntu builds with
`CONFIG_DYNAMIC_DEBUG=y`.

**Relation to the guest module.** The module installs its table at `BUS_NOTIFY_BIND_DRIVER`, which
comes after `dma_configure`, and only if the device has none. With the kernel patch and a `_DMA`
present, the module sees a foreign table. Run one or the other, never both.

## 5. Delivering the table before the DSDT exists (ordering)

**The conflict.** The IOVAs exist only after the device has registered guest RAM with the host RM,
but the DSDT is built from the VM config:
- The device currently registers when Cloud Hypervisor sends `DMA_MAP`, during device creation in
  `vm_boot` (`vmm/src/device_manager.rs:4473-4479`).
- Cloud Hypervisor builds the ACPI tables later in the same boot (`vmm/src/vm.rs:2724-2843`), from
  the config fixed at `vm.create`.

A static `dma_ranges=` therefore cannot carry IOVAs that are first allocated during that boot.
Today's harness writes `$W/dma-ranges` only after `DMA_MAP`, which suits the module
(`tools/caraxes-vm.sh:82-115`, `:134-149`), not `_DMA`.

**The fix within the static option is to register first:**
1. The harness creates the guest RAM backing itself, for example a 4 GiB file in `/dev/shm` or on
   hugetlbfs.
2. The device server maps that file and registers it in chunks (1 GiB, or one per RAM region). It
   writes the table and **holds the registrations**.
   - It computes GPAs from Cloud Hypervisor's rule: zone file offsets run sequentially across the
     RAM regions. File [0, 3 GiB) → GPA [0, 3 GiB), and [3 GiB, 4 GiB) → GPA [4 GiB, 5 GiB)
     (`vmm/src/memory_manager.rs:645-700`).
3. The harness starts Cloud Hypervisor with the table:

   ```
   --memory size=0
   --memory-zone id=ram,size=4G,file=<file>,shared=on
   --platform num_pci_segments=2
   --pci-segment pci_segment=1,dma_ranges=[...]
   --user-device socket=...,pci_segment=1
   ```
4. At `DMA_MAP`, the device checks that fd, offset and size match what it pre-registered, and
   refuses otherwise.

The IOVAs are then fixed before boot and stay valid for the VM's life.

**Later alternatives** (not implemented):
- A `_DMA` **Method** that builds its buffer from an OperationRegion the VMM serves at evaluation
  time. Cloud Hypervisor's memory hotplug already uses this pattern. Linux evaluates `_DMA` at each
  probe, so a table published after `DMA_MAP` would be seen.
- A vfio-user extension that returns the device's bus address per `DMA_MAP`, from which the VMM
  builds `_DMA` itself.

Both are larger and less generic than 0004.

## 6. Test plan in the Caraxes VM

These steps need the operator (VM use and host GPU, §8). Record the results in
`caraxes/runs/acpi-dma-<date>/`.

**Setup:**
- Cloud Hypervisor from `tools/build.sh ch` (now 0001-0004).
- The guest kernel: Ubuntu 6.8.0-136 source with the Linux patch, built as a `-caraxes` flavour and
  installed in the guest image, or passed with `CARAXES_KERNEL` and `CARAXES_INITRD`.
- The guest command line adds `dyndbg="file drivers/acpi/x86/dma.c +p"`.

**Steps:**

1. **T0: firmware only, stock guest kernel.**
   - Boot with `--platform num_pci_segments=2`, the GPU on `pci_segment=1`, and a harmless table
     with a non-zero offset, e.g. `dma_ranges=[0:0x1000000000:0xc0000000,0x100000000:0x10c0000000:0x40000000]`.
   - Expect `sudo acpidump -n DSDT | acpixtract; iasl -d dsdt.dat` to show `_DMA` under `PC01`
     with the §3.3 layout, and none under `PC00`.
   - The stock kernel ignores it: no dyndbg line, and `nvidia` behaves as without the option.
   - Nothing reaches the GPU yet.
2. **T1: patched kernel, identity table.** Use `CARAXES_DMA=identity` or
   `dma_ranges=[0:0:0xc0000000,0x100000000:0x100000000:0x40000000]`.
   - Expect the dyndbg line for `0001:00:xx.0` only.
   - Expect nvidia-smi and the E5a UVM registration to pass unchanged.
   - This isolates the kernel change from any offset.
3. **T2: patched kernel, real IOVAs (non-zero offset).** Use the pre-registration flow of §5.
   - Expect the dyndbg line with the bus limit at the top IOVA.
   - Expect `nvidia-smi` to be normal.
   - Run the same UVM checks as with the module (`tools/caraxes-vm.sh uvm`,
     `crates/vfio-user-nvgpu/guest/uvm-register.c`):
     - The semaphore payloads appear in guest RAM, and GPGet advances.
     - The device log shows only IOVAs from the table in RPC addresses.
     - The guest's `/sys/kernel/debug/swiotlb` shows no bounces for the GPU.
   - **Falsifiers:**
     - an AMD-Vi `IO_PAGE_FAULT` for 0000:2d:00.0 or an Xid on the host (read-only
       `journalctl -k`, filtered);
     - a `DMA addr … overflow` or `swiotlb` error in the guest.
4. **T3: fallback comparison.**
   - Run the same T2 flow on the **stock** kernel with `caraxes_dma_range` loaded with the same
     table (`tools/caraxes-vm.sh dma`).
   - The module's `live` parameter and the kernel's dyndbg line must describe the same
     `{cpu_start, dma_start, size}` entries.
   - The UVM and nvidia-smi results must match T2.
   - Only after T2 = T3 can the module be retired (sysmem-iova.md §5 item 1).
5. **T4: negative checks.**
   - Validation errors, each expected to be refused:
     - `dma_ranges` with an overlapping entry;
     - `dma_ranges` with a misaligned entry;
     - a `pci_segment` beyond `num_pci_segments`.
   - Without `--pci-segment … dma_ranges`, the DSDT must be byte-identical to an unpatched build
     (compare `acpidump` output).

## 7. Upstreaming

**Cloud Hypervisor.**
- Rebase 0004 on current main. Add the DCO sign-off and a `docs/` note next to the existing
  `--pci-segment` documentation.
- Submit it on its own. It is independent of 0001-0003 and of NVIDIA.
- The generic motivation is a vfio-user or vhost-user backend whose hardware DMA runs through a host
  IOMMU domain. aarch64 guests honour `_DMA` today; x86 guests do once the kernel change lands.
- Expect questions on:
  - the option's home (`--pci-segment`, §3.1);
  - `Name` versus `Method`;
  - the MMIO-window overlap policy (warn or refuse);
  - a runtime path (§5) for tables that are known only after `DMA_MAP`.

**Linux.**
- Post the 6.12/mainline variant as an RFC to linux-acpi (Rafael J. Wysocki), with x86 and iommu
  in Cc.
- Include a reproducer: a Cloud Hypervisor guest with 0004, or QEMU with a hand-written SSDT.
- Expect the review questions in the patch notes:
  1. x86 firmware that already ships a `_DMA` and would now be honoured; gate if needed.
  2. A generic helper in `drivers/acpi/scan.c` shared with arm64 and LoongArch instead of a third
     copy.
  3. dma-direct's zone heuristic with non-monotonic tables (§8).
- A stable backport to the Ubuntu 6.8 guest is not realistic. The guest keeps a custom kernel until
  a distribution kernel carries the change.

## 8. Open

1. **Ordering (§5).** The pre-registration flow, meaning a file-backed RAM zone, the device server
   registering before Cloud Hypervisor starts, and a `DMA_MAP` consistency check, is designed here
   but not implemented. Owner: the harness and device side.
2. **MMIO window overlap.** After the host's 64-bit IOVA switch, IOVAs come from just below 2^47,
   and those can fall inside the guest segment's 64-bit MMIO window. Measured 32-bit IOVAs such as
   0xc0000000 fall inside the 32-bit window [0xc0000000, 0xe8000000)
   (`arch/src/x86_64/layout.rs:86-93`).
   - 0004 warns about this.
   - Linux does not check, and the physical GPU never sees the guest's windows. It is a
     spec-conformance blemish, not a functional fault (inference).
3. **Zone heuristic.** With `bus_dma_limit` set, `dma_direct_optimal_gfp_mask` uses
   `dma_to_phys(bus_dma_limit)` (6.8 `kernel/dma/direct.c:47-67`).
   - If the highest-IOVA range maps low guest RAM, coherent allocations are steered to
     `ZONE_DMA32` (or even `ZONE_DMA` if that range ends below 16 MiB).
   - This is correct but conservative, and the same on arm64. The guest module does not set
     `bus_dma_limit`, so it does not show this. Watch for coherent-allocation failures in T2.
4. **Firmware `_DMA` on real x86 machines.** Whether any shipping x86 firmware carries a `_DMA`
   that Linux has so far ignored is unknown. It decides whether the kernel change needs a gate.
5. **Not run:** no VM boot, no guest kernel boot, no GPU contact.
6. **Not edited (the task allowed new files only):**
   - `patches/README.md` should gain a row for 0004;
   - `docs/caraxes/README.md` should mark N1 and P8 as having patches;
   - `docs/caraxes/sysmem-iova.md` §3(a) and §5 should link here.

## Results (2026-09-23)

Run in vm3 (4 GiB, 8 vCPUs, the quickstart's device settings, stock nvidia 610.57.04). Evidence, with every
command: [caraxes/runs/acpi-dma-2026-09-23/](../caraxes/runs/acpi-dma-2026-09-23/) (`commands.txt` first).
Raw DSDTs and their `iasl -d` output: `/mnt/vmstore/caraxes/vm3/acpi/`.

**Verdict.** Patch 0004 does its work. Without the option the DSDT is byte-identical to today's binary. With it,
the `_DMA` object sits on the chosen host bridge only, in the §3.3 byte layout, and the stock guest kernel ignores
it. The Linux draft boots, and it turns the object into exactly the table on the command line, at every probe of
the device. What is still missing is the §5 pre-registration flow; without it real IOVAs cannot reach the table (T3).

### How the table was observed in the guest

`caraxes_dma_range_test.ko` cannot see a firmware map. It refuses to run while a driver is bound, and a map from
`_DMA` exists only between `dma_configure` in `really_probe()` and `device_unbind_cleanup()`. A new checker,
`dmacheck` (in the run directory, MIT), is a PCI driver with no id table, bound through `driver_override`. From
inside its probe it:
- logs `dma_range_map`, `bus_dma_limit` and the masks;
- runs the test module's checks: 256 pages, 8 coherent 64 KiB buffers, the edge pages of every range, and
  `dma_to_phys` above the table;
- returns `-ENODEV`, so the driver core frees the map again.

`caraxes_dma_range` was never loaded in vm3.

### T0: firmware only, stock guest kernel 6.8.0-136-generic

**Pass.**
- **DSDT without the option.** With the patched binary and no option, the DSDT is byte-identical to the one from
  the stock binary the launcher uses (both sha256 `245e1c34…`).
- **`_DMA` with the option.** Run with `--platform num_pci_segments=2`, `--user-device …,pci_segment=1` and the
  identity table `dma_ranges=[0:0:0xc0000000,0x100000000:0x100000000:0x40000000]` (System RAM per `/proc/iomem`:
  `1000-9ffff`, `100000-bfffffff`, `100000000-13fffffff`):
  - `iasl -d` shows exactly one `Name (_DMA, ResourceTemplate ())`, under `_SB.PC01` after its `_CRS`, and none
    under `PC00`;
  - the two descriptors are `QWordMemory (ResourceProducer, …, Cacheable, ReadWrite, …)`;
  - the 104 raw bytes at DSDT offset 0xe82 have the §3.3 layout (`08 5f 44 4d 41 11 42 06 0a 5e 8a 2b 00 00 0c 03 …
    79 00`).
- **The stock kernel ignores it.** On `0001:00:01.0`, `dmacheck` reports `dma_range_map absent`, and all 264
  mappings are identity.
- **nvidia is unaffected.** `tools/caraxes-vm.sh probe` (FLR, then modprobe) gives `RmInitAdapter succeeded!` on
  `0001:00:01.0`, and `nvidia-smi` shows `Leandro RTX 2070-4G`, bus id `00000001:00:01.0`, 4096 MiB.
- **One difference from segment 0.** The I/O BAR 5 gets no address on segment 1: `PC01`'s `_CRS` has no I/O window,
  and the driver enables memory decoding only (`0000 -> 0002`). This is harmless for nvidia.

### T1: patched kernel 6.8.12-caraxesdma, identity table

**Pass.**
- At every probe the kernel logs `ACPI _DMA: 2 range(s), bus limit 0x13fffffff` (dynamic debug).
- `dmacheck` sees the two ranges as installed from firmware and gets 269 of 269 correct mappings, with
  `dma_to_phys(0x140000000) = ~0`.
- nvidia gets a fresh table at each probe: `nvidia 0001:00:01.0: ACPI _DMA: 2 range(s)…`, then `RmInitAdapter
  succeeded!` and `nvidia-smi`. This holds again after `rmmod nvidia_uvm nvidia` and a second probe, which shows
  that the driver core frees the table at unbind and the probe rebuilds it.
- "dma_range_map already set" never appeared.
- **Control.** The same kernel without the option: DSDT `245e1c34…`, no `ACPI _DMA` line in the whole boot journal,
  `dmacheck` identity with no map, and nvidia normal on `0000:00:05.0`.

### T2: patched kernel, +5 GiB on every range, nvidia not loaded

**Pass.**
- **Table:** `dma_ranges=[0:0x140000000:0xc0000000,0x100000000:0x240000000:0x40000000]`. The kernel reports bus
  limit `0x27fffffff`.
- **47-bit mask.** Every one of 269 mappings is GPA + 5 GiB (for example `0x1080000 -> 0x141080000`) and
  translates back. All 8 coherent allocations succeed, none of them below 4 GiB. swiotlb:
  `software IO TLB: mapped [mem 0x00000000ba7ff000-0x00000000be7ff000] (64MB)`, with no bounce and no error.
  `/sys/kernel/debug/swiotlb` does not exist in this build.
- **Default 32-bit mask** (the mask `dma_configure` leaves). Every mapping fails through
  `swiotlb addr 0x00000001fdfff000+4096 overflow (mask ffffffff, bus limit 27fffffff)`, which comes with one
  `WARN` from `swiotlb_map`, and the coherent allocations fail as well. A driver that never raises its mask cannot
  use an offset table. nvidia sets 47 bits.
- **T2b, the real table.** This table was produced by the device in vm2's register mode: 4 × 1 GiB, top-down, GPA 0
  at IOVA `0x7fff80000000`. Here too all 275 mappings are correct, and the kernel reports bus limit
  `0x7fffbfffffff`. All 8 coherent buffers now land below 4 GiB (none did in T1 and T2). That is §8 item 3 as
  predicted: `dma_to_phys(bus_dma_limit)` = `0x3fffffff`, so dma-direct adds `GFP_DMA32`. It is correct and
  harmless at 4 GiB, but it gives ZONE_DMA32 all coherent memory. No MMIO-overlap warning: `PC01`'s windows are
  `[0xd4000000, 0xe7ffffff]` and `[0x200040000000, 0x3fffffeeffff]`, below the IOVAs.

### T3: real IOVAs, not run as instructed

Timestamps from one boot, aligned on `SET_IRQS`, which both sides log:
- In Cloud Hypervisor time, the two `DMA_MAP`s arrive at 21.853 and 21.887 ms.
- The ACPI tables are generated at 22.41–22.60 ms, in the same `vm_boot`.
- In vm2's register mode, `DMA_MAP` is answered only after host registration, about 620 ms per GiB. The boot
  waits about 2.5 s, and every IOVA is known before the DSDT is built.

So the information exists in time; the static `dma_ranges` option just cannot receive it, because it is fixed at
`VmCreate`.

Two more findings:
- With `--memory size=4G,shared=on`, each RAM region has its own anonymous memfd, so nothing can be registered
  before Cloud Hypervisor starts.
- The device writes its table in bare hex (the module's format). `dma_ranges=` refuses that
  (`unable to convert`) or reads it as decimal.

The exact additions for the device and the launcher are in the run's `t3-boot-order.txt` §4-§5:
- a `--preregister` mode on a harness-owned file;
- a `DMA_MAP` check by inode and offset;
- registrations kept across guest reboots;
- `0x`-prefixed table output;
- a `CARAXES_DMA=acpi` launcher mode that orders file, device, table and Cloud Hypervisor, and skips the module.

**T4, validation part (extra, no guest).** Cloud Hypervisor refuses each bad table before it creates a VM:
- overlap on the CPU side;
- overlap on the bus side;
- a misaligned entry;
- an empty entry;
- a wrapping entry;
- `pci_segment=2` with two segments.

A valid table that overlaps the 32-bit window only warns. Not done: the `acpidump` byte compare of T4, which T0
already covers.

### Guest kernel build recipe

The exact commands are in `commands.txt`.
- **Source:** tag `Ubuntu-6.8.0-136.136` plus `linux/0001`. The release is `6.8.12-caraxesdma`: the Ubuntu tree's
  Makefile says 6.8.12, and `LOCALVERSION=-caraxesdma`.
- **Config:**
  - the guest's `/boot/config-6.8.0-136-generic`;
  - `localmodconfig` against vm3's `lsmod`, taken with nvidia loaded;
  - `WERROR` off, the Canonical key files emptied, debug info and BTF off.
- **Compiler:** gcc 13.4.0 from nixpkgs (unwrapped) for the kernel, the system gcc for host programs.
- **Build:** `nice -n 19 make -j12`, 6 minutes, output in `target/caraxes/kbuild-caraxesdma/`.
- **Install:**
  - `modules_install INSTALL_MOD_PATH=` into a staging directory;
  - the build tree from `scripts/package/install-extmod-build`;
  - both copied by rsync (`install-kernel.sh`);
  - in the guest: `depmod`, `update-initramfs -c` and `update-grub`, which keeps the stock kernel;
  - nvidia rebuilt in the guest from `~/ogkm` against `/lib/modules/6.8.12-caraxesdma/build`.
- **Boot:** Cloud Hypervisor boots the kernel directly (`--kernel`/`--initramfs`), so the grub entries are not used.

### Deviations from §6

- **The Cloud Hypervisor binary** was the one built earlier today (v53.0 + 0001-0004, sha256 `9ceeeff1…`), not
  `tools/build.sh ch`.
- **The launcher was not changed.** `LEA_CH` already selects the binary. The two allowed hooks (binary, extra
  arguments) would still not put `--user-device` on a segment or add kernel command-line words. The run used a
  run-local script (`vm3-up.sh`) with the launcher's device and Cloud Hypervisor lines, plus `wait`, `ssh`,
  `probe` and `down` from the launcher unchanged.
- **The compiler** was gcc 13.4.0 rather than the host's GCC 16. GCC 16 defaults to C23, where `bool` is a keyword,
  and 6.8's boot-code Makefiles set no `-std`; gcc-13 also matches the guest's module compiler.
- **Verification** used `dmacheck` instead of the test module and the module's `live` readback (see above).
- **Kernel and flavour name:** `-caraxesdma` instead of a `-caraxes` flavour.
- **Launcher gap found:** once `nvidia-smi` has loaded `nvidia_uvm`, `tools/caraxes-vm.sh probe`'s `rmmod nvidia`
  fails and the probe does not reload (probe `t1-identity-2`). `rmmod nvidia_uvm` first fixed it.

### Before the Linux patch can be posted

1. **Update the draft notes.** They say "not boot-tested". It is now boot-tested on the Ubuntu 6.8 base in a
   Cloud Hypervisor guest with 0004: the identity, offset and top-down tables, rebuilt per probe, and a no-`_DMA`
   control.
2. **Port and test the posting base.** Port to mainline (`drivers/acpi/x86/Makefile`, `dma_range_map_max()`),
   then build and boot it there. The 6.12 variant has only been compiled, and nothing was built for 32-bit x86
   (`CONFIG_X86` includes it).
3. **Fix the trailers.** Add the submitter's `Signed-off-by`, and drop or adjust the SPDX trailer and the
   `Co-Authored-By` spelling.
4. **Add the evidence to the notes:**
   - the ZONE_DMA32 steering with top-down tables (T2b);
   - the swiotlb overflow `WARN` for drivers that keep the 32-bit default mask under an offset table (T2).

   Both are the same on arm64 today, and both belong in the review answers.
5. **Provide a reproducer that needs no out-of-tree VMM,** for example QEMU with an SSDT that adds `_DMA` to a
   PCI expander bridge. Otherwise reviewers must first carry 0004.
6. **Settle the gating question** (review question 1): whether shipping x86 firmware already carries a `_DMA`.
   It is unanswered.
