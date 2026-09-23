<!-- SPDX-License-Identifier: MIT -->
<!-- Written 2026-09-23. Driver citations are relative to vendor/open-gpu-kernel-modules/ (tag
     610.57.04). Linux citations name the release: v7.2 is the host kernel (7.2.4-arch1-2), v6.8 the
     guest kernel (Ubuntu 6.8.0-136; upstream v6.8 was read). Cloud Hypervisor citations are
     relative to vendor/cloud-hypervisor/. "Inference" marks conclusions not read in code or
     measured. -->
# Guest RAM and host IOVAs: requirement S for E5b

This note settles [cuda-path.md](cuda-path.md) §3 (requirement S, fix S1) and §7.1 items 3 and 4. It
also covers item 1 of [uvm.md](uvm.md) "What blocks E5b": the sysmem PTEs carry guest physical
addresses.

**The problem.** The guest has no vIOMMU, so the addresses it writes into GPU page tables for
system memory are guest physical addresses (GPAs) ([vfio-user.md](vfio-user.md)). The physical GPU
sits in a translated host IOMMU domain (DMA-FQ, [host-facts.md](host-facts.md) H1). The GPU
therefore reads and writes only at IOVAs the host driver has mapped.

## Summary

- **Measured.** One OS descriptor gives **one contiguous IOVA range**, whose base is aligned to
  the descriptor's size rounded up to a power of two.
  - 256 MiB: always 0xa0000000.
  - 1 GiB: always 0x40000000.
  - The base did not change over three fresh buffers, nor over two re-registrations of the same
    buffer.
  - Both ranges lie below 4 GiB, and both are 2 MiB aligned (and more).
  - Two descriptors held at once got adjacent ranges, the second directly below the first.
  - After RM_FREE, a second query finds no descriptor (NV_ERR_OBJECT_NOT_FOUND), and every page
    pin was released.
- **Why.** Each descriptor becomes a single `dma_map_sg` call, and the kernel allocates one IOVA
  range for the whole list (§2). A descriptor over 4 GiB − 4 KiB is split into several calls, which
  give independent ranges.
- **Option (a) is not available on x86.** A per-device DMA offset set by firmware without a guest
  change does not exist there. Linux applies ACPI `_DMA` only on arm64 and LoongArch, a device tree
  does not describe the guest's PCI devices, and Cloud Hypervisor emits no `_DMA`.
- **The guest's DMA layer honours `dev->dma_range_map` on x86 (dma-direct).** Every path the NVIDIA
  modules use to get a sysmem DMA address goes through the DMA API: RM memdescs, UVM, and
  `dma_alloc_coherent`.
- **Recommendation for E5b: option (b).**
  - *Device.* It registers guest RAM in 1 GiB chunks (one OS descriptor each) and reads each
    chunk's IOVA base with `GET_SURFACE_PHYS_ATTR`.
  - *Guest.* A small temporary module installs the resulting table as the NVIDIA device's
    `dma_range_map` before `nvidia.ko` binds. The table has one entry per GiB of guest RAM.
  - *Device, reverse direction.* It translates IOVA → GPA for its own accesses to guest memory.
- **Operator's word needed:**
  1. Guest RAM above about 1-2 GiB switches the host GPU to 64-bit IOVAs for the rest of the boot
     (§1.4).
  2. All guest RAM stays pinned while the VM runs.
  3. The trusted-guest assumption stays.

## 1. Measurement

### 1.1 Procedure

The tool is [caraxes/tools/sysmem-iova.rs](../../caraxes/tools/sysmem-iova.rs) (rustc only, like
`host-facts.rs`). The evidence is in `caraxes/runs/sysmem-iova-2026-09-23/`:
- `run1-256m.txt`, `run2-1g.txt`: the two runs;
- `host-before.txt`, `host-after.txt`: host state around the runs;
- `commands.txt`: the exact commands.

**Buffer.** A memfd of the given size, mapped `MAP_SHARED | MAP_POPULATE`, with every page
written. This is the kind of memory Cloud Hypervisor gives a guest with `--memory shared=on`, and
the kind Leandro v1 maps into its arenas (`crates/vhost-user-nvrm/src/host_pool.rs:268-373`).

**Registration.** As v1 does it (`host_pool.rs:486-504`):
- `NV_ESC_RM_ALLOC_MEMORY` of class 0x71 (`NV01_MEMORY_SYSTEM_OS_DESCRIPTOR`) under the device;
- sent on a `/dev/nvidia0` fd registered to the client's ctl fd;
- flags NONCONTIGUOUS | PCI | CACHED | NO_MAP (`host_pool.rs:170`).

**Query.** `NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR` 0x410103 on the descriptor:
- sent at every 4 KiB offset, 65,536 queries for 256 MiB and 262,144 for 1 GiB;
- plus samples at 0, every 64 MiB, the middle, the last page and the last byte.

`memOffset` returns `memdescGetPhysAddr(AT_GPU, offset)` (`src/nvidia/src/kernel/gpu/mem_mgr/mem_desc.c:4649-4726`).
For sysmem that address is the IOMMU mapping's `iovaArray` entry (`mem_desc.c:3171-3186`).

`contigSegmentSize` is useless for this question:
- For a NONCONTIGUOUS memdesc it is clamped to the end of the current 4 KiB page
  (`mem_desc.c:4711-4720`, "XXX overly conservative").
- It read 0x1000 at every page-aligned offset (0x1 at the last byte).
- Contiguity has to be established by querying every page, as done here.

**Cleanup check** (per registration):
1. RM_FREE.
2. The same query again on the freed handle.
3. The kernel's `nr_foll_pin_acquired`/`nr_foll_pin_released` counters, read before and after. The
   host driver pins pages with `FOLL_LONGTERM` (`kernel-open/nvidia/os-mlock.c:216-311`).

**Safety.**
- *RAM guard.* MemAvailable must stay at least 4 GiB after the buffer is allocated. It never came
  close: at least 9.7 GiB were left.
- *Size cap.* Sizes over 1 GiB are refused unless `--allow-over-1g` is given (§1.4).
- *Nothing touches VRAM.* No VRAM is allocated, and no register or mapping of the card is used.

### 1.2 Privilege: exactly what ran as root

`GET_SURFACE_PHYS_ATTR` is PRIVILEGED:
- It carries flags 0x4 (`src/nvidia/generated/g_mem_nvoc.c:166`).
- A PRIVILEGED control is refused unless `secInfo.privLevel >= USER_ROOT`
  (`src/nvidia/src/kernel/rmapi/control.c:683-696`).
- That level is taken **per ioctl** from `osIsAdministrator()`, which is `capable(CAP_SYS_ADMIN)`
  (`src/nvidia/arch/nvalloc/unix/src/escape.c:381`, `os.c:648`,
  `kernel-open/nvidia/os-interface.c:389-392`, `kernel-open/common/inc/nv-linux.h:490`).

The tool uses this as follows:
1. It is started with `sudo -n`.
2. Before opening any device node it switches to uid/gid 1000 with `PR_SET_KEEPCAPS`. It keeps
   CAP_SYS_ADMIN in the permitted set only; the effective set is empty.
3. It raises CAP_SYS_ADMIN into the effective set only around the `GET_SURFACE_PHYS_ATTR` ioctls:
   17 windows in run 1 and 16 in run 2.

The client, the device, REGISTER_FD, RM_ALLOC_MEMORY, RM_FREE and all buffers ran as uid 1000.
Each run also sent the query once without the capability, as a control; the answer was 0x1b
`NV_ERR_INSUFFICIENT_PERMISSIONS`.

Commands run as root:
- `sudo -n target/sysmem-iova measure --go --sizes 256 --reps 3 --pair` (14:30:13-14:30:20);
- `sudo -n target/sysmem-iova measure --go --sizes 1024 --reps 3` (14:30:56-14:31:25).

Nothing else ran as root. The same per-call privilege scoping works for the device process under
D12 (§4).

### 1.3 Results

| Size | Registrations | Segments per descriptor | IOVA base (every registration) | Base alignment | 2 MiB aligned | Below 4 GiB | RM_ALLOC_MEMORY | Query cost |
|---|---|---|---|---|---|---|---|---|
| 4 MiB (control) | 2 (one per run) | – | 0xc0000000 (run 1), 0xbbc00000 (run 2) | 1 GiB / 4 MiB | yes | yes | – | – |
| 256 MiB | 3 fresh buffers + the same buffer twice | **1** | **0xa0000000** ×5 | 512 MiB | yes | yes | 19-27 ms | about 16 µs per query (1.0-1.14 s per 65,536) |
| 256 MiB pair, held together | 2 | 1 each | A 0xa0000000, B 0x90000000 | 256 MiB | yes | yes | – | – |
| 1 GiB | 3 fresh buffers + the same buffer twice | **1** | **0x40000000** ×5 | 1 GiB | yes | yes | 140-192 ms | 16-21 µs per query (4.1-5.5 s per 262,144) |

**Contiguity.** In every registration, every page's IOVA equalled base + offset, from offset 0 to
the last byte. Every sample line reads "(= base + offset)" in the run files.

**Attributes.** Aperture 1 (SYSMEM), `memFormat` 0x6, `gpuCacheAttr` 2 (UNCACHED; the descriptor
did not ask for GPU_CACHEABLE).

**Base behaviour.** The base is reproducible while the host's own IOVA use is unchanged. It is not
a constant of the host:
- The 4 MiB control landed at 0xc0000000 in run 1 and at 0xbbc00000 36 s later. The host driver's
  own allocations had moved the allocator's state in between.
- Freed ranges are handed out again: the same base came back after each RM_FREE, including for a
  re-registration of the same pages.
- A second descriptor held at the same time is placed directly below the first. The allocator
  works top-down (§2).

**Cleanup.**
- Every RM_FREE returned 0.
- The query on the freed handle returned 0x57 `NV_ERR_OBJECT_NOT_FOUND`.
- The pin counters rose by one page per 4 KiB page on registration and fell by the same amount
  on free. The counters are batched per CPU, so single readings show ±16 or ±64.
- Over both runs, acquired − released returned exactly to its value before run 1, 3249
  (`host-before.txt`, `host-after.txt`).
- The host driver logged a matching `nv_register_user_pages`/`nv_unregister_user_pages` pair for
  each registration.

**IOMMU page size.**
- The IOVA granule is 4 KiB: the smallest size in the domain's page-size bitmap (Linux v7.2
  `drivers/iommu/dma-iommu.c:684`). The AMD v1 table offers 4 KiB-2^51 without 512 GiB
  (`drivers/iommu/amd/amd_iommu_types.h:312`).
- The leaf size of a mapping follows the physical contiguity of the pages. memfd pages without
  `MADV_HUGEPAGE` (shmem THP is `advise` here) are 4 KiB leaves (inference).
- For the GPU only the IOVA layout matters: a 2 MiB-aligned, 2 MiB-contiguous IOVA run is what a
  GPU big-page PTE needs. Every range here is at least 256 MiB aligned.

**Host side effects.**
- No Xid, no IO_PAGE_FAULT, and no NVRM error in the kept log. nvidia-smi was normal before and
  after.
- No "Using N-bit DMA addresses" notice: the GPU still takes 32-bit IOVAs first.
- **Log flood.** This host's driver logs every ioctl (`NVreg_ResmanDebugLevel=0`). The two full
  scans therefore wrote about 1.6 million NVRM lines to the kernel log in two minutes, and journald
  reported three `/dev/kmsg` overruns (`host-after.txt`). Lines lost in the overruns cannot be
  checked.
- The tool now queries every 2 MiB by default; `--full-scan` restores the per-page scan.

### 1.4 Sizes above 1 GiB: not run, and why

`iommu_dma_alloc_iova` behaves as follows (Linux v7.2 `drivers/iommu/dma-iommu.c:760-804`):
- It first tries below 4 GiB for every PCI device whose `pci_32bit_workaround` is set. That is
  the default without `iommu.forcedac` (`dma-iommu.c:2155`).
- On the first failure it clears the flag **for the rest of the boot** and prints "Using N-bit DMA
  addresses" (`dma-iommu.c:791-799`).
- The allocation is size-aligned: its alignment is the size rounded up to a power of two
  (`drivers/iommu/iova.c:175-176`; `alloc_iova_fast` passes `size_aligned = true`,
  `iova.c:377-413`).

The free IOVA space below 4 GiB for group 28 is limited by three reservations:
- the host bridge windows are reserved in the IOVA domain: [0xe0000000, 0xfcffffff] and
  [0x820000000, 0x7fffffffff] (`host-before.txt`; `dma-iommu.c:507-551`);
- the MSI window [0xfee00000, 0xfeefffff] is reserved as well (`amd/iommu.c:53-54`,
  `dma-iommu.c:553-588`);
- pfn 0 is never allocated (`base_pfn = 1`, `dma-iommu.c:685`).

**The consequence for 2 GiB.** A 2 GiB descriptor needs a 2 GiB-aligned slot:
- [2, 4) GiB holds the PCI window and MSI;
- [0, 2) GiB contains pfn 0.

So a 2 GiB registration **must** fail below 4 GiB, and it would switch the host GPU
(0000:2d:00.0, the desktop's only display device) to 64-bit IOVAs until reboot. The same happens as
soon as the 32-bit space runs out for any other reason:
- run 2 already had to use [1, 2) GiB, because the host's own IOVAs occupy part of [2, 3) GiB;
- a second 1 GiB chunk would therefore trigger the switch too.

That is a lasting change to how the host maps the display GPU's DMA. The task's RAM guard did not
anticipate it, so the 2 GiB step was left for the operator (§6).

**What the switch means.** The GPU's DMA mask is 47 bits:
- `nv_set_dma_address_size` (`kernel-open/nvidia/nv.c:3234-3247`);
- `NV_CHIP_EXTENDED_SYSTEM_PHYSICAL_ADDRESS_BITS` 47 (`src/common/inc/swref/published/turing/tu102/hwproject.h:27`).

After the switch, new host IOVAs come top-down from just below 2^47 (inference). Mappings that
already exist keep their addresses. Only 0000:2d:00.0 is affected: the flag is per device, so
xHCI, audio and UCSI keep theirs. Without an IOMMU the GPU uses host physical addresses above
4 GiB routinely, so the hardware path is not new. The switch is still a change of host state that
only the operator should allow.

**What a 2 GiB descriptor would give (inference from the code in §2).** One range, 2 GiB aligned,
placed high. Descriptors up to 4 GiB − 4 KiB still form one range. Larger descriptors split at
multiples of 4 GiB − 4 KiB, which is not 2 MiB aligned (§2). This is why §4 uses 1 GiB chunks.

## 2. How the host maps an OS descriptor (why one range)

The call chain, with the driver and kernel code for each step:

1. `RmAllocOsDescriptor` → `RmCreateOsDescriptor`. `os_lock_user_pages` pins the user range with
   `FOLL_LONGTERM` (`src/nvidia/arch/nvalloc/unix/src/escape.c:134-203`, `:206-290`,
   `kernel-open/nvidia/os-mlock.c:216-311`). The result is an `OS_PAGE_ARRAY` descriptor
   (`src/nvidia/arch/nvalloc/unix/src/osmemdesc.c:151-153`).
2. `osCreateMemdescFromPages` builds a NONCONTIGUOUS ADDR_SYSMEM memdesc. It registers the pages
   and calls `memdescMapIommu` **at allocation time** (`osmemdesc.c:175-320`, `:296`).
3. `memdescMapIommu` → `iovaspaceAcquireMapping` → `osIovaMap` → `nv_dma_map_alloc`
   (`mem_desc.c:4294-4316`, `src/nvidia/arch/nvalloc/unix/src/os.c:3800`).
4. In `nv_dma_map_pages`, a multi-page non-contiguous map takes the scatterlist path
   (`kernel-open/nvidia/nv-dma.c:466-537`, `:511`). The pages are split into submaps of at most
   `NV_DMA_SUBMAP_MAX_PAGES` = `U32_MAX >> PAGE_SHIFT` pages (4 GiB − 4 KiB) on x86
   (`kernel-open/common/inc/nv-linux.h:1091-1096`, `nv-dma.c:148-232`).
5. Each submap is **one `dma_map_sg` call** (`nv-dma.c:234-263`). The per-page IOVAs are read
   back from the mapped scatterlist (`nv-dma.c:308-335`).
6. In the DMA-FQ domain, `iommu_dma_map_sg` sums all segments into one length and allocates
   **one** IOVA range for it (Linux v7.2 `dma-iommu.c:1412-1522`, allocation at `:1507`). It maps
   the segments back to back (`iommu_map_sg`, `:1517`). Padding is inserted only at a segment
   boundary; the boundary mask is 4 GiB for PCI, and page-sized segments never cross it inside a
   submap.
7. The IOVA allocation is size-aligned and top-down, tried below 4 GiB first (§1.4).

**Therefore:** one descriptor of at most 4 GiB − 4 KiB gives one IOVA range, aligned to its
power-of-two size. Two descriptors are independent allocations: adjacency is an accident of the
allocator's state, not a guarantee.

Two facts matter for teardown:
- **Unmapping is lazy in DMA-FQ.** IOTLB entries of an unmapped range can stay valid until the
  flush queue drains: 10 ms per-CPU, or 1 s single-queue (`dma-iommu.c:103-104`).
- **Freed IOVAs are reused at once** (measured, §1.3).

Consequence: the GPU must be idle on a guest's channels before its descriptors are freed
(host-facts.md §5 item 7).

## 3. Options

### (a) Per-device DMA offset in the guest, set by firmware, no guest change

**Mechanism.** Linux's DMA offset per device is `dev->dma_range_map`, an array of
`struct bus_dma_region {cpu_start, dma_start, size}` with a zero-size terminator:
- v6.8 `include/linux/dma-direct.h:20-55`;
- v6.8 `include/linux/device.h:745`.

`phys_to_dma`/`dma_to_phys` walk the table; an address not covered translates to
`DMA_MAPPING_ERROR`, so `dma_capable` fails (`dma-direct.h:26-40`). It is a table, not a single
constant, so the guest RAM ranges do not need one common offset.

**Who can set it:**

| Source | Status on x86 |
|---|---|
| ACPI `_DMA` on the host bridge | **Not used on x86.** `acpi_dma_configure_id` calls `acpi_arch_dma_setup` (v7.2 `drivers/acpi/scan.c:1659-1680`, call at `:1669`). That function is real only on arm64 and LoongArch: v7.2 `include/linux/acpi.h:263-267`, v6.8 `:257-261`. The arm64 version is the one that parses `_DMA` and sets `dma_range_map` (`drivers/acpi/arm64/dma.c:29-37`); everywhere else it is an empty inline. `acpi_dma_get_range` exists (`scan.c:1541-1600`), but nothing calls it on x86. Patch set written 2026-09-23 (Cloud Hypervisor 0004 plus an x86 Linux draft that adds the missing `acpi_arch_dma_setup`): [../../patches/ACPI-DMA.md](../../patches/ACPI-DMA.md) |
| Device tree `dma-ranges` | Only when the PCI host bridge's parent has an `of_node` (v7.2 `drivers/pci/pci-driver.c:1676-1682`). An x86 Cloud Hypervisor guest boots from ACPI, and its host bridge has no DT node |
| Cloud Hypervisor | Emits `_CCA` and `_CRS` for each PCI segment (`vmm/src/pci_segment.rs:442-490`) but **no `_DMA`** (no match in the tree). Its RAM layout is [0, 3 GiB) and [4 GiB, …) (`arch/src/x86_64/layout.rs:86-125`, `arch/src/x86_64/mod.rs:1036-1058`) |
| Kernel command line | No x86 parameter sets a per-device DMA offset |

**Does the NVIDIA stack honour `dma_range_map` in the guest?** Yes, on every path that yields a
sysmem DMA address:
- **RM memdescs.** Always IOMMU-mapped through `nv_dma_map_alloc` when the iovaspace is set
  (`mem_desc.c:4294-4316`). On kernels 5.0 and later `nv_dma_maps_swiotlb` is false, so
  `nv_requires_dma_remap` is true (`kernel-open/nvidia/nv.c:5243-5250`,
  `kernel-open/common/inc/nv-linux.h:957-1020`). The address therefore comes from
  `dma_map_sg`/`dma_map_page_attrs` → dma-direct → `phys_to_dma` → `dma_range_map`.
- **The raw path is not taken.** `nv_phys_to_dma` (`nv-linux.h:537-546`) is used only when no
  remap is required (`nv.c:3940-3966`). It may even skip `phys_to_dma`: the conftest looks for the
  function in `<linux/dma-mapping.h>` (`kernel-open/conftest.sh:1496-1508`).
- **UVM.** `dma_map_page` on the GPU's PCI device (`kernel-open/nvidia-uvm/uvm_gpu.c:3970-3998`),
  and `dma_alloc_coherent` (`uvm_gpu.c:3936`).
- **Exception: the sysmem page-tree root.** If UVM's page tree root is in sysmem, UVM passes
  `page_to_phys` to `SetPageDirectory` (`uvm_gpu.c:1342-1345`, `uvm_va_space.c:1422`). Today the
  root is in vidmem (E5a: FB 0x200000). Keep `uvm_page_table_location` at its default and do not
  select sysmem page tables.
- **dma-direct accepts the NVIDIA mask** (47 bits): `dma_direct_supported` returns 1 for any mask
  of at least 32 bits (v6.8 `kernel/dma/direct.c:567-578`).

**Could one constant work?** Only for guest RAM of at most 4 GiB − 4 KiB in a single region, and
the hole [3, 4) GiB would have to be filled with pinned dummy pages. Cloud Hypervisor's RAM is split
around that hole, and larger guests split into several ranges anyway (§2). A single offset is
therefore not a useful target.

**Verdict.** The guest *consumer* exists and the NVIDIA stack uses it, but on x86 no *stock
producer* can fill it. (a) is not realizable without changing the guest kernel and the VMM.

### (b) A tiny temporary guest aid (D13) that installs the table

**Guest side.** Implemented 2026-09-23 as `guest-module/caraxes_dma_range/` (about 600 lines
with validation, a bus notifier that re-installs the map before every probe, and sysfs
readback; see its README for the binding rules). The design was:
1. It finds the synthetic GPU (10de:1f02).
2. It **refuses if a driver is already bound**.
3. It builds a `bus_dma_region` array from the table the device publishes (§4) and assigns it to
   `pdev->dev.dma_range_map`.
4. It stays loaded while `nvidia` is bound. On unload it clears the pointer only if no driver is
   bound.

It needs no symbol export, because `dma_range_map` is a plain field of `struct device`. It patches
nothing in the NVIDIA modules or the kernel.

**Ordering.** It must run before `nvidia.ko` probes, for example with
`softdep nvidia pre: caraxes_dma_range` in the guest's modprobe.d. The Caraxes harness can also
load it first.

**Coverage.** The table must cover all guest RAM from GPA 0:
- swiotlb bounce buffers and every allocation the driver makes must translate;
- an uncovered address yields `DMA_MAPPING_ERROR`, and the mapping fails or bounces.

**What changes in the guest.** Only the device-visible DMA addresses of this one device. The CPU
side, other devices and the NVIDIA code are unchanged.

**What the device must then do.**
- **RPC addresses become IOVAs.** Every sysmem address the guest sends in RPCs is now an IOVA:
  GSP queue PTE arrays, fault and access-counter buffers, USERD and method-buffer addresses.
- **The device translates back.** For its own reads and writes of guest memory it maps IOVA → GPA
  with the same table: one lookup in the guest-memory accessor (`crates/vfio-user-nvgpu/src/guestmem.rs`,
  used by `msgq.rs`, `fb.rs`, `channel.rs`).

**Verdict.** It works with the stock NVIDIA driver on the stock guest kernel, and it is the only
guest-side change. **Recommended** (§4).

### (c) Identity IOVAs on the host (`iommu=pt`, or an identity domain for group 28)

**How it could be set:**
- *At runtime.* `echo identity > /sys/kernel/iommu_groups/28/type` needs **no driver bound** to
  any function in the group (v7.2 `drivers/iommu/iommu.c:3289-3292`, `owner_cnt`). That means
  unbinding `nvidia`, `snd_hda_intel`, `xhci_hcd` and `nvidia-gpu`: the desktop goes down, and the
  rules forbid it.
- *At boot.* `iommu=pt` / `iommu.passthrough=1` is global: every device on the host loses DMA
  translation and protection.

**What it gives.** With identity, the GPU's DMA address is the host physical address. Guest RAM is
memfd pages scattered over host RAM, so GPA ≠ HPA, and there is no contiguity to offset:
- A 4 GiB guest would need about a million table entries.
- Alternatively, 1 GiB hugetlbfs backing for the VM would give one entry per GiB. The guest would
  still need the same aid as in (b).

**Verdict.** Identity removes nothing from (b). It takes away IOMMU protection for the whole host,
and it requires a reboot or the display GPU unbound. **Not acceptable** for this host.

**A related fact.** IOVA = GPA can only be had by a host component that controls the domain:
- VFIO/iommufd does that for passthrough, but it needs the group, which means unbinding `nvidia`.
- S4 in cuda-path.md §3.2 would do it with a host kernel helper doing `iommu_map` in the nvidia
  domain.
- The DMA API offers no way to request a specific IOVA.

### (d) Device-side translation of every sysmem address the guest writes

**What the device sees.** RPC payloads (`GSP_RM_ALLOC`/`CONTROL` parameters), BAR0 writes,
doorbells, and guest memory that it chooses to read.

**What the physical GPU acts on that the device cannot see or intercept under execution model (a):**
1. **PTEs written by the guest CPU into guest-RAM page tables.** These are plain memory stores,
   with no trap and no notification. The device learns of them only by reading, and a later
   rewrite goes unnoticed.
2. **PTEs written by the GPU itself.** UVM updates its page tree with CE pushes: memsets and
   inline-data copies (`uvm_mmu.c`; the E5a trace shows the page-tree init push). Under E5b the
   physical GPU executes them, so the device never sees the values.
3. **Physical sysmem addresses inside pushbuffers:**
   - CE `OFFSET_IN/OUT` with `SET_SRC/DST_PHYS_MODE` targeting sysmem, as UVM uses for chunk
     copies;
   - host `SEM_ADDR` methods.
   The physical GPU fetches them directly from GPFIFO and pushbuffer memory. The device forwards
   only the doorbell, and a pushbuffer may be rewritten or generated by the GPU after that.
4. **Instance blocks and RAMFC contents.** In model (a) the physical GPU uses host-created
   channels, but the page-directory base comes from the guest's tree, and its sysmem PDEs carry
   guest addresses.

**Verdict.** Translation at the device would need shadow page tables and pushbuffer rewriting,
that is the rejected shadow model (cuda-path.md §3.2 S5 and §4.1 (b)). **Rejected.**

## 4. Recommendation for E5b

**Mechanism: S1 with the guest aid of option (b).**

1. **Register guest RAM at VM start** (device, host side).
   - When Cloud Hypervisor sends `DMA_MAP` for each memory zone ([vfio-user.md](vfio-user.md)), the
     device maps the zone's fd.
   - It registers the zone in **1 GiB chunks**, one `NV01_MEMORY_SYSTEM_OS_DESCRIPTOR` each, with
     the flags of `host_pool.rs:170`.
   - Chunks that are 1 GiB in size and 1 GiB aligned in GPA get 1 GiB-aligned IOVA bases (§2). This
     keeps GPA mod 1 GiB = IOVA mod 1 GiB, so every 2 MiB and 64 KiB page the guest maps keeps its
     alignment for GPU big-page PTEs.
   - Cloud Hypervisor's zones start at 0 and 4 GiB. A trailing piece smaller than 1 GiB is its own
     descriptor, aligned to its power-of-two size.
   - Chunks also stay under the 4 GiB − 4 KiB submap limit, so each descriptor is exactly one range.
2. **Read each chunk's base.** Use one privileged `GET_SURFACE_PHYS_ATTR` at offset 0. Optionally
   check the last page (base + size − 4 KiB) and every 2 MiB. Refuse to start if a chunk is not
   one range.
   - The device holds CAP_SYS_ADMIN only in its permitted set and raises it around this call and
     the other D12 controls (§1.2).
   - This is narrower than v1's `LEA_ADMIN_PRIV=1`, which keeps the capability effective for the
     whole process (`crates/vhost-user-nvrm/src/main.rs:50-96`).
3. **Publish the table** (GPA base, IOVA base, size), one entry per chunk: 4 entries for today's
   4 GiB guest.
   - For E5b the simplest channel is a file that the device writes and `tools/caraxes-vm.sh`
     passes to the module as a parameter.
   - A vendor-specific capability in the synthetic config space would avoid the side channel
     later.
4. **Guest.** Load `caraxes_dma_range` before `nvidia`. It installs the table and refuses if
   `nvidia` is already bound.
5. **Device, reverse direction.** Every guest-memory access by the device that takes an address
   from the guest translates IOVA → GPA through the table. Addresses outside it are errors, never
   passed through.
6. **Teardown.** Free the guest's host channels and wait for idle before freeing the descriptors
   (§2, DMA-FQ).
7. **Budget.** All guest RAM is pinned for the VM's life, as with VFIO passthrough:
   - 4 GiB takes about 0.6-0.8 s to register (measured 140-192 ms per GiB);
   - the device's pin limit must cover the guest (v1's `PinBudget` defaults to 1 GiB,
     `host_pool.rs:40-70`).

**First checks in E5b:**
- After the module loads, the guest's `/sys/kernel/debug/swiotlb` shows no bounces for the GPU
  (inference: all RAM covered).
- The first UVM registration pushes are executed by the physical GPU:
  - the semaphore payloads appear in guest RAM;
  - GPGet advances.
- **Falsifier:** an AMD-Vi `IO_PAGE_FAULT` for 0000:2d:00.0 in the host log, or an Xid. Check with
  a read-only `journalctl -k`, filtered: the host driver's ioctl logging floods the log.

## 5. Removal plan for the guest aid

> Update 2026-09-23: the guest module exists (`guest-module/caraxes_dma_range/`), and the
> replacement route is written up with its patches in
> [../../patches/ACPI-DMA.md](../../patches/ACPI-DMA.md); the ordering problem (IOVAs exist
> only after `DMA_MAP`, the DSDT is static) and its fix are in that file's §5.

**What the aid is.**
- One guest module, one modprobe.d line, one table-passing step in `tools/caraxes-vm.sh`.
- On the device side, the table publication.
- The reverse translation in the device stays in every variant below (except the last), because
  guest addresses remain IOVAs.

**It is removed when one of these is in place:**
1. **Stock firmware path.** Upstream Linux calls `acpi_dma_get_range` on x86 (an x86
   `acpi_arch_dma_setup`), *and* Cloud Hypervisor emits `_DMA` under the PCI segment
   (`vmm/src/pci_segment.rs`) from a table the device supplies.
   - Removal: delete the module and its loader lines; the table then travels through the VMM.
   - This is two upstream changes and a guest kernel carrying them, so it is not near-term.
2. **IOVA = GPA on the host** (S4, cuda-path.md §3.2). A host-side component maps each guest page
   at IOVA = GPA in the GPU's domain.
   - Removal: delete the module and the table, and drop the reverse translation.
   - This touches the nvidia driver's domain on the host and needs the operator's explicit
     go-ahead.
3. **The model changes** so that no guest-written sysmem address reaches the GPU. S2 (vidmem-only
   GPU work) is not enough for CUDA (cuda-path.md §3.2).

**Until then** the aid is marked temporary in the harness and in the device docs, as D13 requires.
It is the only guest-side patch in the plan.

## 6. Needs the operator's word

1. **Host GPU leaves 32-bit IOVAs.** Registering more than about 1-2 GiB of guest RAM switches
   0000:2d:00.0 to 64-bit IOVAs until reboot (§1.4). A 4 GiB guest will trigger it. Two choices:
   - accept the per-device runtime switch (recommended: narrow, only the GPU function);
   - boot the host with `iommu.forcedac=1`, which applies to every PCI device.

   The 2 GiB measurement of the task was withheld for this reason. Once allowed, run
   `sudo -n target/sysmem-iova measure --go --sizes 2048 --reps 2 --allow-over-1g`, which reads
   every 2 MiB by default.
2. **Pinning.** All guest RAM stays pinned while the VM runs (4 GiB today). The host keeps its
   4 GiB floor only if the VM size is chosen with that in mind.
3. **Privilege.** The device runs as root-started with CAP_SYS_ADMIN kept in its permitted set only
   and raised per privileged call (D12). This is narrower than `LEA_ADMIN_PRIV=1`.
4. **Trust.** Requirement S's second half does not hold:
   - The host domain also contains the host driver's own IOVAs, including those of its GSP
     queues. The data place the host's 32-bit IOVAs in [2, 3.5) GiB (inference, §7 item 5).
   - Nothing stops a guest from writing such an address into a PTE.
   - The trusted-guest assumption of v1 (`docs/SECURITY.md`) stays.
5. **Host log volume.** With `NVreg_ResmanDebugLevel=0` the host driver logs every ioctl. The
   E5b device will issue many RM calls, and this run already overran `/dev/kmsg`. The setting is
   the `nvidia.NVreg_ResmanDebugLevel=0` boot parameter; lowering the level is the operator's choice.

## 7. Open

1. **2 GiB and larger descriptors** were not measured (§1.4). The predictions to check:
   - one range, 2 GiB aligned, placed high after the switch;
   - a 5 GiB descriptor as two ranges split at 4 GiB − 4 KiB.
2. **Anonymous memory and THP** were not measured. Cloud Hypervisor uses memfd; the tool supports
   `--anon`.
3. **The guest aid is not written or tested.** Nor is the table channel (module parameter versus
   a config-space capability). Owner: the device/VM side.
4. **Guest kernel config.** Check that Ubuntu 6.8's `swiotlb` settings and `CONFIG_DMA_*` options
   do not route the GPU through a path that ignores `dma_range_map`. None does upstream
   (`dma-direct.h:57-90`).
5. **Where the host's 32-bit IOVAs sit.** The data suggest [2, 3.5) GiB: run 2's 1 GiB chunk had
   to go to [1, 2) GiB. Their exact layout needs root debugfs, which was not used.
6. **Per-query cost.** 16-21 µs per `GET_SURFACE_PHYS_ATTR` with per-ioctl logging on. The device
   needs only a few queries per chunk, so this does not matter for E5b.
