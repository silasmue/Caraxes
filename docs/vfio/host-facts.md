<!-- SPDX-License-Identifier: MIT -->
# Host facts H1-H8 and the VRAM reservation experiment (D11)

This file records the host facts listed in [cuda-path.md](cuda-path.md) §7.2 and the result of
the VRAM reservation experiment behind decision D11 ([README.md](decisions.md)). Both were measured
on the RTX 2070 host (TU106, 8 GiB, driver 610.57.04) on 2026-09-23.

**Tool.** [caraxes/tools/host-facts.rs](../../caraxes/tools/host-facts.rs) builds with rustc
alone, like `fifo-host-query.rs`:
- `host-facts facts` sends only public controls whose flags carry NON_PRIVILEGED (0x8) and not
  PRIVILEGED (0x4).
- `host-facts reserve --go` runs the allocation experiment.

It ran as uid 1000 (an unprivileged RM client). It made no register access and no mapping.

**Evidence.** Everything is in `caraxes/runs/host-facts-2026-09-23/`:
- `sysfs.txt`: H1 and H2 from sysfs and lspci;
- `facts-controls.txt`: H2-H7 from RM controls;
- `h8-link-caps.txt`: H8, decoded from the captured config space;
- `reserve-run1.txt` and `reserve-run2.txt`: the allocation experiment (§2);
- `host-after.txt`: the journal and nvidia-smi after the runs.

**Paths.** Source paths are relative to `vendor/open-gpu-kernel-modules/src/nvidia`, except those
starting with `sdk/`, which stands for `vendor/open-gpu-kernel-modules/src/common/sdk/nvidia/inc`.

**Touches of the host card.** The runs touched the card only through the RM API:
- `facts`, at 13:55:31 and 13:55:47: RM_ALLOC of a client, device and subdevice, the controls
  in §1, then RM_FREE.
- `reserve`, at 13:56:30 and 13:57:19: eleven vidmem allocations in each run. Each was freed
  right after it was measured. Nothing was left allocated.

## 1. Facts H1-H8

| # | Fact | Source (control id, flags) | Raw values | Interpretation |
|---|---|---|---|---|
| H1 | IOMMU domain type of group 28 | `/sys/kernel/iommu_groups/28/type` | `DMA-FQ` | **Translated** DMA domain with a flush queue (lazy IOTLB invalidation). Not identity. The group holds the four functions of the card: 2d:00.0 VGA (`nvidia`), 00.1 audio (`snd_hda_intel`), 00.2 xHCI (`xhci_hcd`) and 00.3 UCSI (`nvidia-gpu`). IOMMU: AMD-Vi `ivhd0`. The kernel command line sets no `iommu=`/`amd_iommu=` option |
| H2 | Host BAR1 size | sysfs `resource`, `resource1_resize`; `BUS_GET_PCI_BAR_INFO` 0x20801803 (0x10518); `FB_GET_INFO_V2` index `BAR1_SIZE` | BAR0 16 MiB @0xfb000000; **BAR1 256 MiB** @0x7fe0000000; BAR3 32 MiB @0x7ff0000000. `resource1_resize` = 0x1c0. `BAR1_SIZE` = 262144 KiB; `BAR1_AVAIL_SIZE` = 237184 KiB; `BAR1_MAX_CONTIGUOUS_AVAIL_SIZE` = 235392 KiB. `EnableResizableBar: 0` | The resizable-BAR capability offers only 64, 128 and 256 MiB (mask 0x1c0 = bits 6-8), so **BAR1 cannot grow past 256 MiB on this card**, even with ReBAR enabled. About 230 MiB of it is free. A CPU path to a multi-GiB reservation must use windowed BAR1 mappings |
| H3 | Free VRAM, largest free region | `FB_GET_INFO_V2` 0x20801303 (0x10118) | `TOTAL_RAM_SIZE` 8388608 KiB; `USABLE_RAM_SIZE` 8142912 KiB; `HEAP_SIZE` 7958080 KiB; **`HEAP_FREE` 6616576 KiB** (6461 MiB); **`LARGEST_FREE_REGION_SIZE_KB` 6364736** (6215 MiB); `LARGEST_FREE_REGION_BASE_KB` 1593344; `HEAP_BASE_KB` 55296; `HEAP_START` 0; `MAPPABLE_HEAP_SIZE` 0; `FB_TAX_SIZE_KB` 0; `SUSPEND_RESUME_RSVD_SIZE` 0x20000000 bytes; `HEAP_OFFLINE_SIZE` 0 | **`LARGEST_FREE_REGION_BASE_KB` is relative to the PMA region.** It is `pmaGetLargestFree`'s `largestOffset` inside the region, and `HEAP_BASE_KB` is that region's base (src/kernel/gpu/mem_mgr/phys_mem_allocator/phys_mem_allocator.c:1928-1956; src/kernel/gpu/mem_sys/kern_mem_sys_ctrl.c:674-718). The absolute base is the sum: 0x3600000 + 0x61400000 = **0x64a00000**. At the time of `facts` the free region ran from there to the end of the region (0x1e9190000). nvidia-smi at 13:52 showed Total 8192, Reserved 421, Used 1177, Free 6596 MiB |
| H4 | Host FB region layout | `FB_GET_FB_REGION_INFO` 0x20801320 (0x10108) | 1 region: base 0x3600000, limit 0x1e918ffff (7771 MiB), reserved 0, performance 8, compressed 1, ISO 1, protected 0, tag NONE | The heap is one region, [54 MiB, 7833.6 MiB). The card keeps 54 MiB at the bottom and 366.4 MiB at the top (0x1e9190000..0x200000000: host GSP WPR, FRTS, VGA workspace and the like; 54 + 366.4 = nvidia-smi's "Reserved 421 MiB"). Host allocations fill the region **from the bottom** (§2.2) |
| H5 | GPC/TPC masks | `GR_GET_GPC_MASK` 0x2080122a (0x10118); `GR_GET_TPC_MASK` 0x2080122b (0x10118); `GR_GET_NUM_TPCS_FOR_GPC` 0x20801234 (0x8); `GR_GET_ZCULL_MASK` 0x20801237 (0x10018); `GR_GET_GFX_GPC_AND_TPC_INFO` 0x20801239 (0x8); `GPU_GET_PES_INFO` 0x20800168 (0x8) | gpcMask 0x7; physGfxGpcMask 0x7, numGfxTpc 18. Each of GPC 0-2: tpcMask 0x3f, numTpcs 6, zcullMask 0xf, numPesInGpc 3, activePesMask 0x7, maxTpcPerGpcCount 6, tpcToPesMap [0,0,1,1,2,2] | 3 GPCs × 6 TPCs = 18 TPCs = 36 SMs, no floorsweeping. These are the host-derived inputs for row 41 (`D/kgr.rs` module docs). `GR_GET_PHYS_GPC_MASK` 0x20801232 is PRIVILEGED (0x14) and was not sent. `physGpcMask` = gpcMask is consistent with no floorsweeping |
| H6 | CE capabilities | `CE_GET_ALL_CAPS` 0x20802a0a (0x10108) | present 0x1f. CE0, CE1: caps0 0xe3, caps1 0x03. CE2-CE4: caps0 0xe0, caps1 0x03 | CE0 and CE1 are GRCE and SHARED. All five CEs have SYSMEM, P2P, BL_SIZE_GT_64K, NONPIPELINED_BL and PIPELINED_BL (sdk/ctrl/ctrl2080/ctrl2080ce.h:91-104). None has SYSMEM_READ/WRITE, SCRUB, DECOMP or CC bits. These values are to be mirrored into `CE_GET_PHYSICAL_CAPS`/`CE_GET_ALL_CAPS`. `CE_GET_ALL_PHYSICAL_CAPS` 0x20802a0b is INTERNAL (0x101d0) and was not sent |
| H7 | MC interrupt table | `MC_GET_STATIC_INTR_TABLE` 0x2080170e (0x10048); `MC_GET_INTR_CATEGORY_SUBTREE_MAP` 0x2080170f (0x48); `MC_GET_ENGINE_NOTIFICATION_INTR_VECTORS` 0x2080170d (0x10048) | Static table, 9 entries as type: stall/non-stall vector. NON_REPLAYABLE_FAULT(1): 0x84. NON_REPLAYABLE_FAULT_ERROR(2): 0x85. INFO_FAULT(3): 0x86. REPLAYABLE_FAULT(4): 0x40. REPLAYABLE_FAULT_ERROR(5): 0x83. ACCESS_CNTR(6): 0x48. TMR(7): 0x94 with pmcIntrMask 0x100000. CPU_DOORBELL(8): 0x81. GR0_FECS_LOG(9): 0xffffffff. Subtree map: DEFAULT 0, ESCHED_DRIVEN_ENGINE 0x8, ESCHED_DRIVEN_ENGINE_NOTIFICATION 0x1, RUNLIST 0, RUNLIST_NOTIFICATION 0, UVM_OWNED 0x2, UVM_SHARED 0x4. ENGINE_NOTIFICATION_INTR_VECTORS: status 0x56 (NOT_SUPPORTED) | Subtree 0 (vectors 0-63) is engine non-stall (notification). Subtree 1 (64-127) is UVM_OWNED: REPLAYABLE_FAULT 64 and ACCESS_CNTR 72 share leaf 2. Subtree 2 (128-191) is UVM_SHARED: the fault, doorbell and TMR vectors 129-148, all in leaf 4. Subtree 3 (192-255) is the engine stall range. This matches the rules of cuda-path.md §5.1. The device's current vectors (TMR 194, NON_REPLAYABLE_FAULT 195, ACCESS_CNTR 65) differ from the host's; the guest only needs its own table to be self-consistent. The per-engine non-stall vectors (GR0, CE0-2) are **not** obtainable from this control on this host (NOT_SUPPORTED) |
| H8 | Link capabilities (BAR0 0x88084 = config 0x84) | `caraxes/reference/rtx2070/config-space.txt` (file) | LnkCap 0x00453d03; LnkCtl 0x0140; LnkSta 0x1101; LnkCap2 0x0000000e; DevCap (0x8807c) 0x112c8de1 | Max speed code **3** (8 GT/s), which lies within the 1..6 that `BUS_GET_INFO_V2[PCIE_GPU_LINK_CAPS]` needs (cuda-path.md §1.2 row 2). Width x16; ASPM L0s+L1; L0s exit 256-512 ns, L1 exit 2-4 µs; clock PM 1; ASPM optionality 1; port 0. Link status at capture: 2.5 GT/s x16 (idle downgrade). Supported speeds 2.5/5/8 GT/s. The device can return the captured dword unchanged |

## 2. VRAM reservation experiment (decision D11)

### 2.1 Procedure

The allocation is `NV01_MEMORY_LOCAL_USER` (0x40) under the device, with
`NV_MEMORY_ALLOCATION_PARAMS` (sdk/nvos.h:1600-1634):
- owner `cara`, type IMAGE;
- `attr` = LOCATION_VIDMEM | PHYSICALITY_CONTIGUOUS | PAGE_SIZE_HUGE;
- `attr2` = PAGE_SIZE_HUGE_2MB;
- flags 0 for "contiguous, any address", or `FIXED_ADDRESS_ALLOCATE` (0x10) with `offset`.

**Safety rules.** Before every allocation the tool read two free figures:
- `nvidia-smi -q -d MEMORY` "FB Memory Usage / Free";
- `FB_GET_INFO_V2` `HEAP_FREE`.

It skipped any size above min(both) − 1 GiB; none had to be skipped. Each allocation was then
handled in this order:
1. measured while held;
2. freed;
3. checked: free VRAM had to return within 64 MiB before the next step.

The first unexpected status would have stopped the run. The tool's `Drop` frees any live
memory, and closing `/dev/nvidiactl` frees the client in the kernel if the process dies.

**Where the address comes from.** The physical address comes from the allocation reply.
`vidmemConstruct` writes `memdescGetPhysAddr(AT_GPU, 0)` back into `offset`
(src/kernel/mem_mgr/video_mem.c:836-848, :1102). For a contiguous allocation that is the whole
range, so `NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR` (PRIVILEGED) is not needed.
`NV0041_CTRL_CMD_GET_SURFACE_INFO` returns no address. Every reply's `attr` read back 0x11800000
(CONTIGUOUS, HUGE). Every fixed reply's `offset` equalled the requested offset.

**Run 1.** Run 1 (`reserve-run1.txt`) used `LARGEST_FREE_REGION_BASE_KB` as an absolute address
(see H3). Its three "fixed low" requests at 0x61400000 therefore began 54 MiB inside memory the
host holds. RM refused them cleanly with NV_ERR_NO_MEMORY (0x51). Those refusals serve as extra
occupied-range probes. The tool was corrected to add `HEAP_BASE_KB`, and run 2 repeated the
whole sequence.

### 2.2 Results (run 2, 13:57:19-20)

"Free" is nvidia-smi's FB free in MiB. HEAP_FREE was 1 MiB lower throughout.

| Size | Mode | Status | Physical range obtained | Free before | Free while held | Free after |
|---|---|---|---|---|---|---|
| 2.5 GiB | contiguous, any address | NV_OK | 0x67a00000..0x1079fffff | 6453 | 3893 | 6453 |
| 3 GiB | contiguous, any address | NV_OK | 0x107a00000..0x1c79fffff | 6453 | 3381 | 6453 |
| 5 GiB | contiguous, any address | NV_OK (126 ms) | 0x67a00000..0x1a79fffff | 6453 | 1333 | 6453 |
| 2.5 GiB | fixed, lowest 0x67a00000 | NV_OK | 0x67a00000..0x1079fffff | 6453 | 3893 | 6453 |
| 2.5 GiB | fixed, highest 0x149000000 | NV_OK | 0x149000000..0x1e8ffffff | 6453 | 3893 | 6453 |
| 3 GiB | fixed, lowest 0x67a00000 | NV_OK | 0x67a00000..0x1279fffff | 6453 | 3381 | 6453 |
| 3 GiB | fixed, highest 0x129000000 | NV_OK | 0x129000000..0x1e8ffffff | 6453 | 3381 | 6453 |
| 5 GiB | fixed, lowest 0x67a00000 | NV_OK | 0x67a00000..0x1a79fffff | 6453 | 1333 | 6458 |
| 5 GiB | fixed, highest 0xa9000000 | NV_OK | 0xa9000000..0x1e8ffffff | 6458 | 1338 | 6458 |
| 2.5 GiB | fixed, region bottom 0x3600000 (occupied) | NV_ERR_NO_MEMORY (0x51) | – | 6458 | – | 6458 |
| run 1: 2.5, 3, 5 GiB | fixed 0x61400000 (54 MiB into occupied memory) | NV_ERR_NO_MEMORY (0x51) | – | 6450-6470 | – | same |

**Observations.**
- **Fixed-address, physically contiguous VRAM works for an unprivileged client**, at both ends
  of the free region, up to 5 GiB. Neither `stdmemValidateParams` (src/kernel/mem_mgr/standard_mem.c:34-110)
  nor `vidmemConstruct` (video_mem.c:615-640) checks privilege for FIXED_ADDRESS_ALLOCATE or
  CONTIGUOUS. On this PMA-managed heap the request becomes
  `PMA_ALLOCATE_SPECIFY_ADDRESS_RANGE` [offset, offset + size) (video_mem.c:236-242).
- **The largest contiguous allocation that succeeded is 5 GiB.** No larger size was tried,
  because 5 GiB is the largest size D11 asks for. The rule would admit up to about 5.3 GiB today
  (6453 − 1024 MiB).
- **An occupied range is refused cleanly** with NO_MEMORY in 1-75 ms, and nothing is left
  allocated. RM does not relocate a fixed request.
- **Allocation and free are fast**: 0-177 ms per allocation. Free VRAM was back at the first
  poll after RM_FREE; no scrub delay was visible.
- **Host allocations sit at the bottom and grow.** The first free address moved from
  0x64a00000 (13:55) to 0x67a00000 (13:57): +48 MiB while Firefox grew from 134 to 268 MiB.
  A "contiguous, any address" allocation takes the lowest free address, with one exception: the
  3 GiB request landed 2.5 GiB higher, at 0x107a00000, in both runs (not traced, §5). The top of
  the region, up to 0x1e9190000, stayed free throughout.
- **The desktop kept working.**
  - nvidia-smi answered in 15-66 ms at every step.
  - Xid lines, 13:50-13:58 and `journalctl -k --since -5min`: none.
  - The kernel log shows no NVRM error or OOM from the runs.
  - One `nvAssertFailedNoLog: pSmIssueThrottleCtrl != NULL @ kernel_graphics.c:3425` appeared
    at 13:56:13, before the run. It has recurred since 12:49 (seven times today) and is not
    caused by host-facts, which sends no GR_GET_SM_ISSUE_THROTTLE_CTRL (`host-after.txt`).
  - The Leandro VFIO VM (`vfio-user-nvgpu --host-rm --fb-mib 4096`) was running throughout.

## 3. What this means for option C

**Placement.** Anchor the reservation at the **top** of the host's FB region, 2 MiB aligned. The
bottom holds the host's growing allocations; the top was free in every snapshot.

The region ends at 0x1e9190000, so the aligned end is **E = 0x1e9000000**. For a profile of
size S (the host reservation, `Profile.size` in `crates/vhost-user-nvrm/src/vram.rs:88-104`), the
range is **[E − S, E)**:

| Profile | Fixed range (physical) | Tested | Host free while held (today) |
|---|---|---|---|
| 2.5 GiB | 0x149000000..0x1e8ffffff | yes, NV_OK | 3893 MiB |
| 3 GiB | 0x129000000..0x1e8ffffff | yes, NV_OK | 3381 MiB |
| 4 GiB | 0xe9000000..0x1e8ffffff | no; lies between the tested 3 and 5 GiB ranges, both of which succeeded | ≈ 2357 MiB (derived) |
| 5 GiB | 0xa9000000..0x1e8ffffff | yes, NV_OK | 1338 MiB |

**What the guest sees.** The device presents guest FB addresses equal to these physical
addresses (fb-model.md §5, column C):
- guest regions [0, E − S) reserved;
- [E − S, E) usable, minus the device's own top carve-outs;
- the fake GSP's WPR/VGA area above the range, at E and up.

That area coincides with the real card's own top 366 MiB. The device must keep emulating it:
the physical GPU must never be pointed at guest addresses ≥ E or below E − S. That bounds-check
is fb-model.md §5's isolation row.

**Reservation rule** (to implement in the device's host-RM setup, before the guest boots):
1. Round S up to 2 MiB. Read E from `FB_GET_FB_REGION_INFO` (the end of the non-reserved region,
   aligned down to 2 MiB) rather than hard-coding 0x1e9000000.
2. Read `FB_GET_INFO_V2` `HEAP_FREE` and nvidia-smi's FB free. Let F be the smaller of the two.
   **Refuse when S + 1 GiB > F**, with a message giving S, F and the shortfall.
3. Allocate `NV01_MEMORY_LOCAL_USER` with `attr` = VIDMEM | CONTIGUOUS | PAGE_SIZE_HUGE, `attr2` =
   HUGE_2MB, `FIXED_ADDRESS_ALLOCATE` and `offset` = E − S.
4. On NV_ERR_NO_MEMORY, **refuse**. The host holds part of the range: something large at the top,
   or a heap too fragmented. Do not fall back to a floating address: the guest's FB layout is
   fixed at device start. Report `LARGEST_FREE_REGION` (absolute base = `HEAP_BASE_KB` +
   `LARGEST_FREE_REGION_BASE_KB`).
5. Check that the reply's `offset` equals E − S and that `attr` PHYSICALITY reads CONTIGUOUS.
   Hold the object for the life of the VM and free it on stop. Closing the RM client frees it if
   the device dies.

The rule is checked once, at reservation time. With 5 GiB held the host had 1338 MiB left today,
309 MiB above the rule's floor. Host growth after that point (a browser, a game) is not
prevented.

## 4. What needs admin rights

- **Not needed** for anything in this experiment. That covers:
  - contiguous and fixed-address vidmem allocation;
  - learning the physical address (the allocation reply);
  - all H2-H7 controls.

  Running as admin would change nothing in this path: the only privilege checks in it concern
  `internalflags`, `ALLOCATE_FROM_RESERVED_HEAP` and `PAGE_OFFLINING_OFF`, and those need kernel
  level, not admin (standard_mem.c:52-99, video_mem.c:628-631).
- **Needed later** (decision D12, `LEA_ADMIN_PRIV`):
  - `NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR` 0x410103, flags 0x4 (src/nvidia/generated/g_mem_nvoc.c:166). It
    gives the sysmem IOVA segments for S1 ([cuda-path.md](cuda-path.md) §3.2) and per-page
    addresses of non-contiguous memory.
  - `DMA_SET_PAGE_DIRECTORY` and `GPU_PROMOTE_CTX`, to point host channels at the guest's page
    tables inside the reservation.
- **Not sent** because PRIVILEGED or INTERNAL: `GR_GET_PHYS_GPC_MASK` 0x20801232 (0x14) and
  `CE_GET_ALL_PHYSICAL_CAPS` 0x20802a0b (0x101d0).

## 5. Open

1. **4 GiB at 0xe9000000** was not allocated. It lies between two tested ranges, but confirm it
   with one run before the first 4 GiB VM.
2. **Why the floating 3 GiB request landed 2.5 GiB higher** (0x107a00000, both runs) while
   2.5 GiB and 5 GiB took the lowest free address. PMA's contiguous search for this page count
   was not traced. It does not affect the recommendation, which always uses FIXED.
3. **Host growth while a reservation is held.** The rule protects only the moment of
   reservation. Whether the device should watch `HEAP_FREE` and warn, or whether a headroom larger
   than 1 GiB is needed for gaming on the host, is a policy question for the operator.
4. **Top-of-region allocations by the host.** No host allocation was seen above 0x67a00000.
   Reverse (top-down) PMA allocations exist (`PMA_ALLOCATE_REVERSE_ALLOC`,
   video_mem.c:229-233); which host clients use them was not traced. Reserving early (at VM
   start, before other host GPU work) keeps the chance of a conflict low.
5. **BAR1 windowing.** BAR1 is capped at 256 MiB (H2), with about 230 MiB free. The CPU path of
   option C into a multi-GiB reservation must remap windows. The window size and the cost of
   `RM_MAP_MEMORY` per window were not measured.
6. **Per-engine non-stall vectors** for GR0 and CE0-2: `MC_GET_ENGINE_NOTIFICATION_INTR_VECTORS`
   is NOT_SUPPORTED on this host (H7). The device still has to choose them itself in subtree 0.
7. **DMA-FQ lazy invalidation (H1).** In a flush-queue domain an unmapped IOVA can stay reachable
   briefly. This matters for S1's isolation argument (cuda-path.md §3.1, requirement S) when
   guest RAM descriptors are unregistered.
