<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Guest DMA addressing: every route and what each one has to patch

Written 2026-09-23 on the operator's question: "think about a method that needs no patches,
no kernel modules, or maybe a small Cloud Hypervisor patch; or give me a matrix of what has
to be patched per route". Standing constraint: in the end everything must work on a vanilla
guest (stock NVIDIA driver, stock distribution kernel); a Cloud Hypervisor patch may be
acceptable; an upstream Linux change is hard.

## The problem in one paragraph

The guest driver writes system-memory addresses that the physical GPU dereferences on its
own: page-table entries with the SYSMEM aperture, and a few physical addresses in method
streams. The guest obtains those addresses from its kernel's DMA API on the synthetic PCI
device, so without help they are guest physical addresses (GPAs). The physical GPU reaches
host memory through the host IOMMU at I/O virtual addresses (IOVAs) that the host RM's DMA
mappings receive from the host kernel's allocator ([sysmem-iova.md](sysmem-iova.md)). The
device can translate every address it consumes itself (RPC parameters, queue pages, the
mirror channel's GPFIFO/USERD, promoted context buffers: [memory-model.md](memory-model.md)),
but not the ones the GPU reads straight out of guest-written structures. So either the guest
writes IOVAs (guest-side translation), or the host IOVAs equal the GPAs (host-side control),
or the device rewrites the structures before the GPU sees them (shadowing).

## The matrix

"Patch" means a source change that has to be carried. "Config" means a module option,
registry key, kernel command line or launcher flag, which the stock-guest rule allows.

| # | Route | Guest NVIDIA driver | Guest kernel | Extra guest module | Guest config | Cloud Hypervisor | Host NVIDIA driver | Host kernel / config | Device work | Isolation (P7) | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | Guest module installs `dma_range_map` (today, D13) | none | none | **yes** (`caraxes_dma_range`) | insmod before `nvidia` | none | none | none | done (E5b part 1) | no: the GPU walks guest-written IOVAs | works; deprecated; bring-up aid until 4 lands |
| 2 | ACPI `_DMA`: CH patch 0004 + x86 Linux change | none | **patch** until upstream | none | own PCI segment | **small patch** | none | none | pre-registration flow (RAM as a file registered before CH starts) | no | weakest: the kernel half is the hard one; vm3 test is evidence only |
| 3 | Same after the kernel change is upstream and in the distro kernel | none | none (distro ≥ N) | none | own PCI segment | small patch | none | none | pre-registration flow | no | vanilla in years, not now; keep the patch posted |
| 4 | **Shadow page tables in the device** (what NVIDIA's pre-SR-IOV vGPU host does on Turing) | none | none | none | 2 options to keep page tables and instance memory in VRAM (`uvm_page_table_location=vid`, RM `RmInstLoc*`; names to verify) | none | none | none | **large**: PTE/PDE shadowing, invalidate hooks, method-stream rewrite, sysmem-table sync (§4) | **yes**: the device controls every address the GPU uses | the only vanilla route today; recommended target after CUDA runs |
| 5 | CH places high RAM zones at the observed IOVAs (pre-registration + zone placement) | none | none | none | none | small patch | none | none | pre-registration + zone plan | no | covers only zones above 4 GiB; the low 3 GiB still need route 4, so not worth it alone; a later optimization of 4 at most |
| 6 | Host IOMMU passthrough + host physical reservation backing guest RAM (`memmap=`, `iommu=pt`) | none | none | none | none | patch (map a physical range as RAM) | none | **host command line**, no isolation | register physical ranges | destroys host isolation | no |
| 7 | vIOMMU in the guest (virtio-iommu) | none | none | none | `iommu=on` | none | none | none | consume vfio-user IOVA maps | no | only moves the problem: the host still chooses its IOVAs; dropped |
| 8 | GPU ATS / shared virtual addressing with the device process | none | none | none | none | none | **not on x86 GeForce** | platform | – | – | no |
| 9 | NVIDIA's vGPU manager on the host (mdev) | none | none | none | none | vfio-mdev | **closed, GRID only** | – | – | yes | not available for this card; no |
| 10 | VRAM-only guest (no sysmem DMA) | none | none | none | none | none | none | none | refuse sysmem | – | no CUDA (pushbuffers, semaphores, host-pinned memory are sysmem); no |

Routes 1-3 are the same mechanism delivered three ways. Route 4 is the different mechanism.

## Why the host side cannot be made to match (routes 5-9)

- The host kernel's IOVA allocator picks the IOVA for every RM mapping (32-bit range first,
  then top-down from the 47-bit limit after the switch). Neither the RM API nor the device
  can request an IOVA ([sysmem-iova.md](sysmem-iova.md) §2).
- A guest needs RAM at GPA 0 to boot. The host's own memory lives at host physical address 0.
  So even with the IOMMU in passthrough (IOVA = host physical), GPA ≠ IOVA for the low zone.
- The only host mechanisms that let a userspace process choose IOVAs are VFIO and iommufd, and
  they need the device bound to vfio-pci, which the host RM driving the card excludes.

## Route 4 in detail: shadowing

**Principle.** The guest keeps writing GPAs. The device keeps two views of the framebuffer:
the *guest view* (what the guest reads back: today's `HostRamStore`) and the *device view*
(real VRAM, what the GPU walks: the VRAM store of E5b part 2). Every write that lands in a
page-table page is translated on the way to the device view: entries with the SYSMEM
aperture get GPA → IOVA from the same table E5b part 1 builds. Everything else is copied
unchanged.

**How the device knows which pages are page tables.**
1. Every VA space's root comes to the device: the kernel RM's `DMA_SET_PAGE_DIRECTORY`
   control is ROUTE_TO_PHYSICAL and the UVM VA spaces register the same way
   (`channel::Registry` already maps address spaces to roots).
2. From a root, PDEs point at lower tables; a write to a known PDE page registers the new
   table page. The GMMU v2 format is per chip ([versatility.md](versatility.md)).
3. The guest signals every table update with an MMU invalidate: the kernel RM writes
   `NV_PFB_PRI_MMU_INVALIDATE` through BAR0, UVM uses `MEM_OP` host methods in its
   pushbuffers. Both are visible to the device and are the points where a lazy shadow is
   brought up to date.

**Where the writes come from, and that all of them are visible.**
- CPU writes through PRAMIN (BAR0 window) and the BAR2/BAR3 aperture: vfio-user delivers
  them as messages; the device already routes them through the FB model.
- Copy-engine writes by UVM in physical addressing mode: the device already decodes the
  pushbuffers ([uvm.md](uvm.md)); with physical execution it scans each submission before
  ringing the mirror doorbell, rewrites physical SYSMEM addresses, and executes the copies
  that target page-table pages itself so the guest view stays right.
- Page tables placed in system memory by the guest are invisible to the device while the
  guest writes them. Two guest options move them to VRAM (`uvm_page_table_location=vid`;
  the RM's instance-location registry keys for PDE/PTE, instance blocks, USERD, runlists).
  Where an entry still points at a sysmem table, the device shadows that table into
  device-owned VRAM at invalidate time.

**Structures that are not page tables.** Everything the device programs for the mirror
channel is translated already (GPFIFO base, USERD, fault buffers, promoted context
buffers, semaphore/notifier addresses in RPC parameters). Method streams reach data through
virtual addresses, so the rewritten PTEs cover them.

**Cost and risk.**
- Effort: 4-8 agent-days after E5b part 2, most of it in the write filter and its tests.
- Every page-table write costs a second write and a translation; invalidates cost a walk of
  the dirty tables. UVM-heavy workloads will feel it; a per-page dirty bitmap keeps it linear.
- Correctness depends on catching every table page. A missed one shows as an IOMMU fault on
  the host (a clear signal, not silent corruption), and the device can verify shadows against
  the guest view in a debug mode.
- Isolation falls out: a guest can only name sysmem the device registered for it.

**Phasing.** E5b part 2 is designed with the two views from the start, but keeps route 1 for
bring-up so CUDA runs first. Then: the write filter with page-table tracking; invalidate
hooks; the pushbuffer scanner; switch the launcher to identity DMA (no module) and rerun the
gates; delete the module.

## What this changes for the current work

- The guest module and the `_DMA` patches stay as they are (deprecated, evidence).
- E5b part 2 gets one design requirement: the VRAM-backed `FbStore` sits behind a write
  filter, and the guest view stays readable.
- Q32: verify the registry key names for instance-memory location in
  `vendor/open-gpu-kernel-modules` and which of them the GSP-client kernel RM honours.
  Partial answer (2026-09-23): the kernel RM reads `NV_REG_STR_RM_INST_LOC` fields `PDE`, `PTE`,
  `BAR_PDE`, `BAR_PTE` from `pGpu->instLocOverrides` in `_kgmmuInitRegistryOverrides`
  (`src/nvidia/src/kernel/gpu/mmu/kern_gmmu.c`); the defaults are `ADDR_FBMEM`. Whether the
  GSP side honours the same keys for its own tables is still open.
- Decision D17 ([README](README.md)): route 4 is the target, built in two stages after CUDA runs, a
  validating PTE filter first (isolation, guest module kept), the translating filter second (vanilla,
  guest module dropped). The guest stays bare metal: page tables in VRAM are the driver's native
  default, so the only configuration deviation is disabling UVM's sysmem fallback. VRAM cost of
  shadowing: none beyond the guest's own tables; a host-RAM shadow is the fallback for sysmem tables only.
