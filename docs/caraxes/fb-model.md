<!-- SPDX-License-Identifier: MIT -->
<!-- Source survey written 2026-09-23 against vendor/open-gpu-kernel-modules at tag 610.57.04.
     Every claim about the driver cites file:line in that tree. Items marked "derived" are
     arithmetic on cited code; items marked "choice" are design decisions of this note. -->
# Framebuffer model for the synthetic TU106 (milestone 1)

This note settles open questions 5 and 17 ([open-questions.md](open-questions.md)) far enough to build
milestone 1: `RmInitAdapter succeeded`, `/dev/nvidia0` usable, and `nvidia-smi` printing the card.
It specifies:

- every path by which the guest kernel RM (GSP-client mode, `PF_KERNEL_ONLY`) touches FB contents;
- what `kbusVerifyBar2_GM107` checks;
- where the RM puts its structures in FB, and what the synthetic `GspStaticConfigInfo` must say;
- the device-side FB model: backing, BAR1/BAR3 translation, PRAMIN, and registers;
- the three backing options for after milestone 1.

Companion notes:

- [kernel-rm-hw-surface.md](kernel-rm-hw-surface.md): the full register surface.
- [gsp-boot-turing.md](gsp-boot-turing.md) §3.1: the WPR meta layout.
- [identity-profile.md](identity-profile.md): `fb_length` and `LOCAL_MEMORY_RANGE`.

## Source files cited

All paths are relative to `vendor/open-gpu-kernel-modules/`. The short names below are used in the text.

| Short name | Path |
|---|---|
| `kern_bus_gm107.c` | `src/nvidia/src/kernel/gpu/bus/arch/maxwell/kern_bus_gm107.c` |
| `kern_bus.c` | `src/nvidia/src/kernel/gpu/bus/kern_bus.c` |
| `kern_bus_vbar2.c` | `src/nvidia/src/kernel/gpu/bus/kern_bus_vbar2.c` |
| `kern_bus_gv100.c` | `src/nvidia/src/kernel/gpu/bus/arch/volta/kern_bus_gv100.c` |
| `kern_bus_tu102.c` | `src/nvidia/src/kernel/gpu/bus/arch/turing/kern_bus_tu102.c` |
| `bar2_walk.c` | `src/nvidia/src/kernel/gpu/mmu/bar2_walk.c` |
| `gmmu_walk.c` | `src/nvidia/src/kernel/gpu/mmu/gmmu_walk.c` |
| `kern_gmmu.c` | `src/nvidia/src/kernel/gpu/mmu/kern_gmmu.c` |
| `kern_gmmu_gm107.c` | `src/nvidia/src/kernel/gpu/mmu/arch/maxwell/kern_gmmu_gm107.c` |
| `kern_gmmu_gm200.c` | `src/nvidia/src/kernel/gpu/mmu/arch/maxwell/kern_gmmu_gm200.c` |
| `kern_gmmu_tu102.c` | `src/nvidia/src/kernel/gpu/mmu/arch/turing/kern_gmmu_tu102.c` |
| `kern_gmmu_fmt_gp10x.c` | `src/nvidia/src/kernel/gpu/mmu/arch/pascal/kern_gmmu_fmt_gp10x.c` |
| `gmmu_fmt.c` | `src/nvidia/src/libraries/mmu/gmmu_fmt.c` |
| `mem_mgr.c` | `src/nvidia/src/kernel/gpu/mem_mgr/mem_mgr.c` |
| `mem_mgr_gm107.c` | `src/nvidia/src/kernel/gpu/mem_mgr/arch/maxwell/mem_mgr_gm107.c` |
| `mem_mgr_gsp_client.c` | `src/nvidia/src/kernel/gpu/mem_mgr/mem_mgr_gsp_client.c` |
| `mem_utils.c` | `src/nvidia/src/kernel/gpu/mem_mgr/mem_utils.c` |
| `mem_desc.c` | `src/nvidia/src/kernel/gpu/mem_mgr/mem_desc.c` |
| `kern_mem_sys.c` | `src/nvidia/src/kernel/gpu/mem_sys/kern_mem_sys.c` |
| `kern_mem_sys_gm107.c` | `src/nvidia/src/kernel/gpu/mem_sys/arch/maxwell/kern_mem_sys_gm107.c` |
| `kernel_gsp_tu102.c` | `src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_tu102.c` |
| `kernel_gsp.c` | `src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c` |
| `g_kern_bus_nvoc.c/.h`, `g_kern_gmmu_nvoc.c`, `g_mem_mgr_nvoc.c/.h` | `src/nvidia/generated/` |
| `gm107/dev_bus.h`, `gm107/dev_ram.h`, `gm107/dev_fb.h` | `src/common/inc/swref/published/maxwell/gm107/` |
| `gp100/dev_mmu.h` | `src/common/inc/swref/published/pascal/gp100/dev_mmu.h` |
| `tu102/dev_mmu.h`, `tu102/dev_vm.h`, `tu102/dev_bus.h` | `src/common/inc/swref/published/turing/tu102/` |
| `gm107/dev_mmu.h` | `src/common/inc/swref/published/maxwell/gm107/dev_mmu.h` |
| `kern_bus_gp100.c` | `src/nvidia/src/kernel/gpu/bus/arch/pascal/kern_bus_gp100.c` |
| `kern_gmmu_gv100.c` | `src/nvidia/src/kernel/gpu/mmu/arch/volta/kern_gmmu_gv100.c` |
| `kern_gmmu_fmt_gm10x.c` | `src/nvidia/src/kernel/gpu/mmu/arch/maxwell/kern_gmmu_fmt_gm10x.c` |
| `kern_gpu_gm107.c` | `src/nvidia/src/kernel/gpu/arch/maxwell/kern_gpu_gm107.c` |
| `kern_gpu_tu102.c` | `src/nvidia/src/kernel/gpu/arch/turing/kern_gpu_tu102.c` |
| `kern_mem_sys_gp102.c` | `src/nvidia/src/kernel/gpu/mem_sys/arch/pascal/kern_mem_sys_gp102.c` |
| `kernel_gsp_frts_tu102.c` | `src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_frts_tu102.c` |
| `kern_disp_0400.c` | `src/nvidia/src/kernel/gpu/disp/arch/v04/kern_disp_0400.c` |
| `disp_inst_mem.c` | `src/nvidia/src/kernel/gpu/disp/inst_mem/disp_inst_mem.c` |
| `kernel_fifo_tu102.c` | `src/nvidia/src/kernel/gpu/fifo/arch/turing/kernel_fifo_tu102.c` |
| `gpu_vaspace.c` | `src/nvidia/src/kernel/mem_mgr/gpu_vaspace.c` |
| `rpc.c` | `src/nvidia/src/kernel/vgpu/rpc.c` |
| `rpc_headers.h`, `rpc_global_enums.h` | `src/nvidia/inc/kernel/vgpu/` |
| `gsp_static_config.h` | `src/nvidia/inc/kernel/gpu/gsp/gsp_static_config.h` |
| `g_chips2halspec_nvoc.h`, `g_kern_mem_sys_nvoc.h` | `src/nvidia/generated/` |
| `ctrl2080fb.h`, `ctrl2080internal.h` | `src/common/sdk/nvidia/inc/ctrl/ctrl2080/` |
| `ctrl0041.h`, `ctrl90f1.h` | `src/common/sdk/nvidia/inc/ctrl/` |
| `nvos.h` | `src/common/sdk/nvidia/inc/nvos.h` |

How HALs resolve: in the generated tables, `RmVariantHal` has only two variants, `VF` and `PF_KERNEL_ONLY`. So the `else` branch of every `/* RmVariantHal: VF */` test is the one TU106 takes (`g_chips2halspec_nvoc.h:121`).

---

## 1. Every CPU path into FB, in order

### 1.0 Before state init: no FB access

- **KernelMemorySystem construct.** It only programs a *sysmem* address into `NV_PFB_NISO_FLUSH_SYSMEM_ADDR` 0x100C10 (`kern_mem_sys_gm107.c:281`, `:373`, `gm107/dev_fb.h:26-28`).
- **GSP boot touches no FB bytes from the CPU.** In the kernel build, the only code that uses the PRAMIN window or moves `NV_PBUS_BAR0_WINDOW` is:
  - the bus code (`kern_bus_gm107.c`, `kern_bus_gv100.c:418`, and `kern_bus_gp100.c:995-1085`). The `kern_bus_gp100.c` hits are `kbusInstBlkWriteAddrLimit_GP100` and `kbusInitInstBlk_GP100`, which are bound only for VF (`g_kern_bus_nvoc.c:1695`, `:1710`);
  - `bar2_walk.c`;
  - `mem_utils.c`.

  `kernel_gsp_tu102.c` uses only `DRF_SIZE(NV_PRAMIN)` as a constant (`:790`, `:800`).
- **Firmware images.** Everything the WPR meta describes sits in sysmem ([gsp-boot-turing.md](gsp-boot-turing.md) §3).
- **FB placement is the device's job.** The synthetic GSP (booter) decides what, if anything, it places in FB.

### 1.1 MemoryManager pre-init and init: layout only, no access

1. `memmgrStatePreInitLocked_IMPL` → `memmgrInitFbRegions` → `memmgrInitBaseFbRegions_FWCLIENT` copies `GspStaticConfigInfo.fbRegionInfoParams` (`mem_mgr.c:372`, `mem_mgr_gsp_client.c:81-106`).
   - The VBIOS/VGA reservation hook and the chip region hook are empty in this build (`g_mem_mgr_nvoc.h:1124`, `:1427`).
2. `memmgrPreInitReservedMemory_GM107` sizes the CPU-RM reserved block with offsets relative to its future base (`mem_mgr.c:376`, `mem_mgr_gm107.c:2056-2130`). See §3.
3. `memmgrStateInitLocked_IMPL` places the block (`memmgrInitReservedMemory_GM107`, `mem_mgr.c:629`) and creates the heap (`mem_mgr.c:666`).
4. State init order is KernelMemorySystem, MemoryManager, KernelGmmu, KernelBus (`kern_gpu_gm107.c:480-491`). The heap therefore exists when KernelBus allocates FB.

### 1.2 KernelBus state init: BAR0 window, BAR2 bootstrap, BAR2 self-test

`kbusStateInitLocked_IMPL` (`kern_bus_gm107.c:379-489`) runs these steps.

1. **`memmgrVerifyGspDmaOps`** (`:404-405`). It returns at once unless BAR access is blocked (confidential compute only; `mem_mgr.c:778-800`, `kern_bus_gm107.c:388-398`).
2. **Default BAR0 window.** `kbusSetupDefaultBar0Window` (`:197-261`, called at `:301`):
   - `offsetBar0 = (fbAddrSpaceSizeMb << 20) - 1 MiB` (`:222`). `bReservedMemAtBottom` is never set in the tree, so it stays zero.
   - If the static memsys config says `bFbpaPresent == FALSE` or cache-only mode is on, the offset is 0 or the L2 size instead (`:232-252`). `bFbpaPresent` comes from GSP control `NV2080_CTRL_CMD_INTERNAL_MEMSYS_GET_STATIC_CONFIG` 0x20800a1c (`kern_mem_sys.c:484-498`, `ctrl2080internal.h:119`).
   - `kbusSetBAR0WindowVidOffset_GM107` (`:4744-4776`) asserts 64 KiB alignment (`:4751`).
   - It reads `NV_PBUS_BAR0_WINDOW` once while its cache is 0 (`:4755-4758`).
   - It then does two read-modify-writes, BASE (`:4767`) and TARGET = VID_MEM (`:4768`). It skips both if the cached value already matches (`:4762`).
3. **`kbusInitBar2_GM107`** (`:1346-1395`, called at `:334`):
   - a. `kbusSetupBar2CpuAperture_GM107` maps PCI BAR3 for `rmApertureLimit + 1` bytes, write-combined (`:1525-1532`). The limits are `cpuVisibleLimit = 32 MiB - 1` and `rmApertureLimit = 16 MiB - 1` (`:4364-4365`, `g_kern_bus_nvoc.h:185-186`). No device access happens here.
   - b. `kbusSetupBar2GpuVaSpace_GM107` (`:1629-1976`):
     - It rebases `pdeBase`/`pteBase` by `rsvdMemoryBase` (`:1690-1693`).
     - `bUsePhysicalBar2InitPagetable` is set only for an SR-IOV VF (`kern_bus.c:58-61`). So the kernel takes the PRAMIN path: `kbusSetupBar0WindowBeforeBar2Bootstrap_GM107` (`:1715`, `:2122-2160`).
     - That sets the window to `pdeBase & ~0xffff` (`:2146-2147`) and records `bar2OffsetInBar0Window = pdeBase - windowBase + 0x700000` (`:2151-2153`).
     - It fetches the GMMU format (`:1821`), creates the walker, and reserves and sparsifies the 4 KiB page-table level for the whole CPU-visible VA range [0, 32 MiB) (`:1862-1873`).
     - Every PDE and PTE write in this phase goes through PRAMIN as 32-bit writes (`bar2_walk.c:243-264` for fills, `bar2_walk.c:434-450` for PDEs), each via `kbusMemAccessBar0Window_GM107` (`kern_bus_gm107.c:4813-4903`).
     - Levels are carved out in sequence: page directories at `pdeBase + n * 4 KiB`, page tables at `pteBase + n * 4 KiB` (`bar2_walk.c:620-626`, `:709-717`).
   - c. `kbusPatchBar2Pdb_GSPCLIENT` (`kern_bus.c:849-902`, called at `kern_bus_gm107.c:1925`):
     - It replaces the software PDB with a memdesc at `GspStaticConfigInfo.bar2PdeBase` (`kern_bus.c:866-870`).
     - It **reads the kernel's PDE3[0] through PRAMIN** at `bar2OffsetInBar0Window` (two 32-bit reads, `:891-892`).
     - It sends RPC `UPDATE_BAR_PDE` (function 70) with `{barType = 1 (BAR_2), entryValue, entryLevelShift = 47}` (`:899`, `rpc.c:9709-9731`, `rpc_headers.h:225-229`, `rpc_global_enums.h:80`).
     - The RPC status is ignored; the function returns `NV_OK` (`kern_bus.c:901`).
   - d. The window is restored (`kern_bus_gm107.c:1945-1949`, `:2174-2198`).
   - e. `kbusCommitBar2_KERNEL` (`kern_bus.c:457-473`) runs two steps.
     - **`kbusInitVirtualBar2_VBAR2`** (`kern_bus_vbar2.c:374-420`) maps the page-level block `[pdeBase, pdeBase + dirs + tables)` into BAR2 as a persistent mapping, packed at the top of the RM aperture (`kern_bus_vbar2.c:598-601`). The chain is `memmgrMemDescBeginTransfer` → `memdescMapInternal` → `kbusMapBar2Aperture_VBAR2` → `kbusUpdateRmAperture_GM107`.
       - Because `pPageLevels` is still NULL, `kbusUpdateRmAperture_GM107` moves the window to `pdeBase` again (`kern_bus_gm107.c:2733-2740`).
       - It writes the PTEs through PRAMIN (`:2395-2459`) and reads the last entry back through PRAMIN as a flush (`:2464-2475`), then restores the window (`:2747-2751`).
       - It then either reads the flush page through BAR2 (if `pReadToFlush` is already set) or sends RPC control `NV2080_CTRL_CMD_INTERNAL_BUS_FLUSH_WITH_SYSMEMBAR` 0x20800a70 (`:2761-2770`, `kern_bus.c:437-442`).
       - Finally it issues a **TLB invalidate** for the PDB `bar2PdeBase` (`:2772-2783`).
     - **`kbusSetupCpuPointerForBusFlush_GV100`** (`kern_bus_gv100.c:50-99`) allocates one 4 KiB FB page from the heap and maps it into BAR2 as the permanent `pReadToFlush`. This is the **first time BAR2 PTEs are written through BAR3 itself**: `pPageLevels` is now a BAR3 pointer (`kern_bus_gm107.c:2359-2394`), followed by another invalidate.
4. **`kbusVerifyBar2_GM107`** (`kern_bus_gm107.c:3986-4281`, called at `:360-363` because `PDB_PROP_GPU_COHERENT_CPU_MAPPING` is false). See §2.
5. The BAR1 instance block memset (`:471-477`) is skipped: `bar1.pInstBlkMemDesc` is only created in the non-GSP-client branch (`mem_mgr_gm107.c:813-829`).

### 1.3 After BAR2 is up: all FB transfers go through BAR3

- **Transfer type.** `memmgrGetMemTransferType` returns `TRANSFER_TYPE_PROCESSOR` for FB surfaces (`mem_utils.c:60-127`). The exceptions are:
  - a caller that passes `TRANSFER_FLAGS_PREFER_CE` while global CeUtils exists (`:94-106`);
  - `TRANSFER_TYPE_BAR0`, which is only chosen in simulation (`:84-92`).
- **Mapping.** A processor transfer maps the surface with `memdescMapInternal` (`mem_utils.c:1290-1372`, `mem_desc.c:2072-2140`). For FB on TU this is `MEMDESC_MAP_INTERNAL_TYPE_BAR2` (`mem_desc.c:1990-2022`).
  - The mapping is `kbusMapBar2ApertureCached_VBAR2`, which allocates BAR2 VA (`kern_bus_vbar2.c:528-719`) and writes 4 KiB PTEs (`kern_bus_gm107.c:2582`, `:2675`).
  - The CPU pointer is `BAR3 base + VA + PteAdjust` (`kern_bus_vbar2.c:703`). So **BAR3 offset equals BAR2 virtual address**.
- **Page tables stay fixed.** After bootstrap the BAR2 page-table *structure* does not change; only PTEs are rewritten. All 16 page tables for the 32 MiB were reserved up front (`kern_bus_gm107.c:1862-1873`).
- **Page-table writes.** PTE and PDE writes for any GMMU address space (BAR1, user VA spaces) go through the same memmgr transfers, i.e. through BAR3. Examples: `gmmu_walk.c:958-1010` copy, and fills at `gmmu_walk.c:782`.

### 1.4 BAR1 (state post-load and later)

- **BAR1 init runs.** `kbusStatePostLoad_GM107` calls `kbusInitBar1_GM107` (`kern_bus_gm107.c:682-700`, `:879-1235`); it is **not** stubbed for the kernel variant (`g_kern_bus_nvoc.c:1859-1863`).
  - It creates a sparsified BAR1 VA space starting at 0 (`:925-928`, `:990-992`).
  - Its page levels are allocated in FB (`kern_gmmu.c:125`, `:131`, `gmmu_walk.c:141-147`) and written through BAR2 (§1.3).
- **Stubbed pieces.**
  - `kbusBar1InstBlkVasUpdate` → `_ac1694` (`g_kern_bus_nvoc.c:1895`).
  - `kbusBar1InstBlkBind` → `_92bfc3` (`g_kern_bus_nvoc.c:2003`).
  - So the kernel never writes a BAR1 instance block or `NV_PBUS_BAR1_BLOCK`.
- **`kbusPatchBar1Pdb_GSPCLIENT`** (`kern_bus.c:766-827`):
  - It describes `GspStaticConfigInfo.bar1PdeBase` as the new root (`:794`).
  - It moves the walker's root level there with `bCopyEntries = TRUE` (`:805-812`). The kernel therefore **copies its BAR1 root entries into FB at `bar1PdeBase`**: a processor copy through BAR2 (`gmmu_walk.c:958-1003`, `mem_utils.c:1080-1104`). All later root updates land there.
  - It then invalidates with PDB = `bar1PdeBase` (`kern_bus.c:823`).
- **BAR1 page size.** With PCI BAR1 ≤ 256 MiB, BAR1 mappings are forced to 64 KiB (`kern_bus.c:514-541`).
- **Static BAR1** (all of FB mapped once) is only enabled when BAR1 can hold FB plus extras (`kern_bus_tu102.c:367-470`); otherwise BAR1 is dynamic.
- **CPU accesses through BAR1 during init.**
  - `kfifoPreAllocUserD_TU102` maps preallocated USERD into BAR1 and CPU-maps the BAR1 range (`kernel_fifo_tu102.c:489-721`).
  - The RC watchdog writes GPFIFO/GPPut through that mapping ([kernel-rm-hw-surface.md](kernel-rm-hw-surface.md), KernelRc).

### 1.5 GMMU format used for BAR1/BAR2 on TU106

- **Format selection.**
  - `kgmmuFmtGet(VERSION_DEFAULT, 0)` picks the newest registered family (`kern_gmmu.c:663-685`) and the 64 KiB big-page format (`kern_gmmu.c:716-733`; `defaultBigPageSize = 64 KiB` for TU, `g_kern_gmmu_nvoc.c:286-291`).
  - Levels come from `kgmmuFmtInitLevels_GP10X` (TU binding at `g_kern_gmmu_nvoc.c:714-717`). Entries come from `kgmmuFmtInitPde/PdeMulti/Pte_GP10X` (TU binding at `g_kern_gmmu_nvoc.c:668-735`).
- **Levels** (`kern_gmmu_fmt_gp10x.c:31-105`), for a 49-bit VA:

| Level | VA bits | Entry size | Entries per page used by BAR2 |
|---|---|---|---|
| PD3 (root) | 48:47 | 8 B | 4 |
| PD2 | 46:38 | 8 B | 512 |
| PD1 | 37:29 | 8 B | 512 |
| PD0 | 28:21 | 16 B (dual PDE), or 8 B PTE for a 2 MiB page | 256 |
| PT big (64 KiB pages) | 20:16 | 8 B | 32 |
| PT small (4 KiB pages) | 20:12 | 8 B | 512 |

  - PD0 is both a directory and a page table (`bPageTable = TRUE`, 2 sublevels, `kern_gmmu_fmt_gp10x.c:83-89`).
  - A PD0 entry is a PTE (a 2 MiB page) exactly when its PTE VALID bit 0 is set (`gmmu_fmt.c:63-85`).
  - A 128 KiB big page is possible only through regkey (`kern_gmmu_fmt_gp10x.c:56`, `kern_gmmu.c:534-568`).

- **PDE (PD3/PD2/PD1)** (`gp100/dev_mmu.h:97`, `:113-119`; used by `kern_gmmu_fmt_gp10x.c:138-153`):
  - bits 2:1 aperture: 0 invalid, 1 video, 2 sys coherent, 3 sys non-coherent (`gm107/dev_mmu.h:26-30` values via `kern_gmmu_fmt_gm10x.c:165-182`);
  - bit 3 volatile;
  - vidmem address bits 32:8, shifted left 12;
  - sysmem address bits 53:8, shifted left 12.
- **Dual PDE (PD0, 16 bytes)** (`gp100/dev_mmu.h:98-112`; used by `kern_gmmu_fmt_gp10x.c:107-136`):
  - Big half, low qword: aperture bits 2:1, volatile bit 3, vid address bits 32:4 shifted left **8**, sys address bits 53:4 shifted left 8.
  - Small half, high qword: aperture bits 66:65, volatile bit 67, vid address bits 96:72 shifted left 12, sys address bits 117:72 shifted left 12.
- **PTE** (`gp100/dev_mmu.h:120-157`, Turing overrides in `tu102/dev_mmu.h:116-118`; used by `kern_gmmu_fmt_gp10x.c:155-195`):

| Bits | Field | Values |
|---|---|---|
| 0 | VALID | |
| 2:1 | APERTURE | 0 video, 1 peer, 2 sys coherent, 3 sys non-coherent (`gm107/dev_mmu.h:79-83`) |
| 3 | VOL | |
| 4 | ENCRYPTED | |
| 5 | PRIVILEGE | |
| 6 | READ_ONLY | |
| 7 | ATOMIC_DISABLE | |
| 32:8 | ADDRESS_VID | shifted left 12 |
| 35:33 | PEER index | |
| 53:8 | ADDRESS_SYS | shifted left 12 |
| 55:36 | COMPTAGLINE | Turing |
| 63:56 | KIND | |

- **Sparse and "4K" encodings.**
  - Sparse PTE is VALID = 0, VOL = 1 (0x8).
  - Sparse PDE is aperture INVALID with VOL = 1.
  - The source says "GM20X supports sparse directly in HW" (`kern_gmmu_gm200.c:41-68`).
  - The "nv4k" PTE encoding is VALID = 0, VOL = 1, PRIV = 1 (`kern_gmmu_gv100.c:67-77`). BAR2 never uses it (`bar2_walk.c:185-187`).
- **BAR2 mapping PTEs.** The template sets VALID, VOL from the memdesc, APERTURE (video for FB), and KIND from the memdesc (`kern_bus_gm107.c:2675-2723`). All BAR2 mappings use 4 KiB pages (`:2582`).

---

## 2. `kbusVerifyBar2_GM107` step by step

- **Constants.** `SAMPLEDATA = 0xABCDABCD`, `FBSIZETESTED = 0x10` (`kern_bus_gm107.c:4008-4009`).
- **L2 flags.** `flagsClean = ALL | CLEAN`, plus `WAIT_FB_PULL` if `kmemsysIsL2CleanFbPull` (`:4034-4039`).
- **When it is skipped.** Only for inst-in-sysmem, broken-FB, cache-only mode, or the confidential-compute skip flag (`:4018-4030`, `:395`). None applies here.

The steps:

1. **Allocate and map the test surface.**
   - A 16-byte FB surface, uncached, from the heap (`:4066-4076`).
   - It is mapped into BAR2 with `kbusMapRmAperture_HAL` (= `memdescMapInternal`, `g_kern_bus_nvoc.h:3219`; `:4079`). That writes one 4 KiB PTE through BAR3, then flushes by reading `pReadToFlush` through BAR3 (`kern_bus_gv100.c:388-400`), then invalidates the TLB (§4d).
   - `testMemoryOffset` = the surface's FB address (`:4087`).
   - `testAddrSpace` = `NV_MMU_PTE_APERTURE_VIDEO_MEMORY` = 0 (`:4089`, `kern_gmmu_gm107.c:379-380`).
2. **Save and move the window.**
   - Save the cached window base, and read `0x1700` for its TARGET (`:4097-4098`).
   - RMW BASE = `testAddr >> 16`, then RMW TARGET = 0 (`:4100-4101`).
3. **PRAMIN round trip.**
   - Read the dword at `0x700000 + (testAddr & 0xFFFF)` and keep it (`:4102`).
   - Write `0xABCDABCD` there and read it back; it must match, else `NV_ERR_MEMORY_ERROR` (`:4104-4120`).
4. **First L2 evict.** RPC control `NV2080_CTRL_CMD_INTERNAL_MEMSYS_L2_INVALIDATE_EVICT` 0x20800a6c (`:4126`, `kern_mem_sys.c:1122-1136`, `ctrl2080internal.h:2148`). A non-OK reply fails the test.
5. **PRAMIN read after evict.** Read the same dword again; it must still be `0xABCDABCD` (`:4133-4150`).
6. **Restore.** Write the saved dword back, then restore BASE and TARGET with two RMWs (`:4154-4156`). The sysmem-only flush at `:4158-4163` does not apply.
7. **BAR2 write.** Write `0xABCDABCD` to the four dwords at the BAR2 CPU pointer, i.e. BAR3 offset = surface VA (`:4178-4181`).
   - Then `osFlushCpuWriteCombineBuffer` (`:4189`) and a second L2-evict RPC (`:4191-4196`).
8. **Point the window at the surface.**
   - Save the window again and read `0x1700` (`:4199-4200`).
   - RMW BASE = `testMemoryOffset >> 16`, RMW TARGET = 0 (`:4201-4202`).
9. **PRAMIN compare and rewrite.** For each of the four dwords at `0x700000 + (testMemoryOffset & 0xFFFF) + 4i`:
   - the read must be `0xABCDABCD`, i.e. the BAR3 write must have landed in FB (`:4209-4224`);
   - then write `0xABCDABDD` (`SAMPLEDATA + 0x10`) through PRAMIN (`:4226`).
10. **Restore and flush.**
    - Restore BASE and TARGET (`:4229-4230`).
    - `kbusFlush_HAL(BUS_FLUSH_VIDEO_MEMORY)` → `kbusFlushSingle_GV100` reads one dword of `pReadToFlush` through BAR3 (`:4232`, `kern_bus_gv100.c:380-400`).
    - A third L2-evict RPC (`:4240-4244`).
11. **BAR2 compare.** Read the four dwords through BAR3; each must be `0xABCDABDD` (`:4247-4259`).
12. **Unmap and free** (`:4262-4273`). An unmap without sparsify writes no PTEs (`kern_bus_gm107.c:2614-2616`).

**What a host-RAM FB with a small walker needs to pass:**

- PRAMIN at `(BASE << 16) + off` and BAR3 at VA `o` must hit the same host byte: `walk(bar2PdeBase, o) == (BASE << 16) + off`.
- The three L2-evict RPCs and the sysmembar RPC must return success.
- `MMU_INVALIDATE` TRIGGER must read back as 0.
- No caching between the paths, or caches dropped on invalidate.
- Nothing else is compared.

---

## 3. FB addresses

### 3.1 What the kernel computes, from where

- **WPR meta (from `LOCAL_MEMORY_RANGE` 0x100CE0).**
  - `fbSize = MAG << (SCALE + 20)`, times 15/16 if the ECC bit is set (`kern_mem_sys_gp102.c:36-58`).
  - `kgspPopulateWprMeta_TU102` then lays out, top down (`kernel_gsp_tu102.c:755-861`):
    1. VGA workspace: `fbSize - 1 MiB` unless `NV_PDISP_VGA_WORKSPACE_BASE` 0x625F04 reports STATUS_VALID (`:785-802`, `kern_disp_0400.c:52-61`);
    2. `gspFwWprEnd` (128 KiB aligned);
    3. FRTS (1 MiB);
    4. boot binary;
    5. GSP ELF (64 KiB aligned);
    6. firmware heap (1 MiB aligned);
    7. `gspFwWprStart = heap - 1 MiB`;
    8. `nonWprHeapOffset = gspFwWprStart - 1 MiB`;
    9. `gspFwRsvdStart = nonWprHeapOffset` (`:859-861`).
  - Heap sizing: `kernel_gsp.c:6417-6550`; see [gsp-boot-turing.md](gsp-boot-turing.md) §3.1.
  - After FWSEC, the kernel only checks that `WPR2_ADDR_HI` is non-zero and `WPR2_ADDR_LO == frtsOffset >> 12` (`kernel_gsp_frts_tu102.c:506-525`).
- **Region table (from `GspStaticConfigInfo`)** (`mem_mgr_gsp_client.c:39-126`):
  - `numFBRegions` must be at least 1 (`:60-65`) and at most 16 (`:68-75`, `g_mem_mgr_nvoc.h:432`).
  - A region with a non-zero `reserved` field is a reserved region (`:96-105`).
  - `fbTotalMemSizeMb = ceil(fb_length / 1 MiB)` (`:118`).
  - `fbAddrSpaceSizeMb = ceil(last region's limit / 1 MiB)`, taken from the *last* region whether reserved or not (`:119-120`).
  - It asserts address space ≥ total (`:122`).
- **CPU-RM reserved block** (sizes: `mem_mgr_gm107.c:2056-2130`, `mem_mgr.c:2051-2087`):

  | Relative offset | Contents | Source |
  |---|---|---|
  | +0x0 | 64 KiB display instance memory, if KernelDisplay exists | `mem_mgr_gm107.c:2074-2083`; placed at `rsvdMemoryBase + 0` by `disp_inst_mem.c:249-275` |
  | next 4 KiB | "BAR1 instance block" | `:2098-2101` |
  | next 4 KiB | "BAR2 instance block" | `:2102-2105` |
  | 4 KiB aligned | BAR2 page directories: 4 levels × 4 KiB = 0x4000 | derived from `_kbusGetSizeOfBar2PageDir_GM107` (`kern_bus_gm107.c:4381-4404`, `:4417-4505`) with VA limit 32 MiB - 1 (`kern_bus.c:686-701`) |
  | next | BAR2 page tables: 16 × 4 KiB = 0x10000 | derived from `kbusGetSizeOfBar2PageTables_GM107`, `kern_bus_gm107.c:3846-3910` |

  - On a GSP client the two instance-block offsets are **never rebased or written**. `memmgrStateInitAdjustReservedMemory` skips GSP clients (`mem_mgr_gm107.c:813`), and `kbusInitInstBlk` is `_ac1694` (`g_kern_bus_nvoc.c:1714`).
  - The size is rounded up to 64 KiB (`mem_mgr_gm107.c:965`) and **must stay below 1 MiB**, or state init fails with `NV_ERR_INSUFFICIENT_RESOURCES` (`:973-976`, `:916-923`; GK104 check bound for TU at `g_mem_mgr_nvoc.c:1254-1262`).
  - The block goes at the top of the **last non-reserved, non-protected region**: `top = min(region.limit + 1, fbAddrSpaceSizeMb << 20)`, `rsvdMemoryBase = align_down(top - size, 4 KiB)` (`:990-1062`).
  - It is then cut out of that region as `NV2080_FB_REGION_TAG_CPU_RM_RESERVED` (`:1084-1116`).
- **BAR2 addresses.** `pdeBase` and `pteBase` become absolute at `kern_bus_gm107.c:1690-1693`.
- **Default BAR0 window** is `fbAddrSpaceSize - 1 MiB` (`:222`).

### 3.2 Worked example: 4 GiB, no ECC, display present, VGA workspace not valid (derived)

Two inputs come from outside the source:

- `.fwimage` size 0x1C01DC8: read from the host's `/lib/firmware/nvidia/610.57.04/gsp_tu10x.bin` with `readelf -S`. The guest must carry the identical file.
- `sizeOfBootloader` 0x1000: from [gsp-boot-turing.md](gsp-boot-turing.md).

| Item | FB address | Size |
|---|---|---|
| `fbSize` (0x100CE0) | 0x1_0000_0000 | 4 GiB |
| VGA workspace = `gspFwWprEnd` | 0xFFF0_0000 | 1 MiB |
| FRTS (`frtsOffset`, what WPR2_LO must report) | 0xFFE0_0000 | 1 MiB |
| boot binary | 0xFFDF_F000 | 4 KiB |
| GSP ELF (`gspFwOffset`) | 0xFE1F_0000 | 0x1C01DC8 |
| GSP heap (`gspFwHeapOffset`) | 0xF780_0000 | 105 MiB (8 + 1 + 96, within 64..223 MiB clamps) |
| `gspFwWprStart` (1 MiB WPR meta) | 0xF770_0000 | 1 MiB |
| non-WPR heap = `gspFwRsvdStart` | 0xF760_0000 | 1 MiB |
| **choice:** `bar2PdeBase` (fake GSP's BAR2 root) | 0xF760_0000 | 4 KiB |
| **choice:** `bar1PdeBase` (BAR1 root the kernel fills) | 0xF760_1000 | 4 KiB |
| CPU-RM `rsvdMemoryBase` | 0xF75D_0000 | 0x30000 (0x20000 without display) |
| display instance memory | 0xF75D_0000 | 64 KiB |
| BAR2 PD3 (kernel's root; only the source of PDE3[0]) | 0xF75E_2000 | 4 KiB |
| BAR2 PD2 / PD1 / PD0 | 0xF75E_3000 / 0xF75E_4000 / 0xF75E_5000 | 4 KiB each |
| BAR2 PT0..PT15 (VA 0..32 MiB) | 0xF75E_6000..0xF75F_5FFF | 64 KiB |
| PRAMIN window during bootstrap | BASE 0xF75E (FB 0xF75E_0000); PDE3[0] read at BAR0 0x702000 | 1 MiB |
| default PRAMIN window | BASE 0xFFF0 (FB 0xFFF0_0000) | 1 MiB |
| CPU-RM heap / PMA | 0x0..0xF75C_FFFF, including the VerifyBar2 test page, the flush page, BAR1 page levels and USERD | |

- Without display, the block shrinks to 0x20000 at 0xF75E_0000. The page directories and tables keep the **same absolute addresses** (relative 0x2000 and 0x6000).
- The BAR2 page-level block is self-mapped near the top of the 16 MiB RM aperture: persistent mappings grow down (`kern_bus_vbar2.c:598-601`) from `rmApertureLimit - 4 KiB` (`kern_bus_gm107.c:1729-1735`). The exact VA depends on the eheap; the device must not assume it.

### 3.3 What `GspStaticConfigInfo` (and friends) must say

- **`fb_length`** = the profile's guest FB (4 GiB in the example). It must be ≤ the address space implied by the last region, or the assert at `mem_mgr_gsp_client.c:122` fires.
- **`fbRegionInfoParams`** (`gsp_static_config.h:84`; struct at `ctrl2080fb.h:904-923`):
  - region 0: `[0, gspFwRsvdStart - 1]`, `reserved = 0`, `bProtected = 0`;
  - region 1: `[gspFwRsvdStart, fbSize - 1]`, `reserved = size`.
  - Region 1 makes `fbAddrSpaceSizeMb` equal the `LOCAL_MEMORY_RANGE` size, so the default PRAMIN window lands on the same VGA workspace the WPR meta uses.
  - It keeps the CPU-RM heap and reserved block below the non-WPR heap, and therefore below WPR2 = [`frtsOffset` .. `gspFwWprEnd`) and the later full WPR.
  - The device knows `gspFwRsvdStart` exactly: it receives the WPR meta PA in SEC2 MAILBOX0/1 before it has to answer `GET_GSP_STATIC_INFO` ([gsp-boot-turing.md](gsp-boot-turing.md) §3.1).
- **`bar2PdeBase` and `bar1PdeBase`** (`gsp_static_config.h:130-131`): 4 KiB-aligned FB addresses inside the reserved region. These are the roots the device's BAR3 and BAR1 walkers start from (§4b).
  - The kernel treats both as FB (`kern_bus.c:792-794`, `:865-867`).
  - It writes into `bar1PdeBase` (§1.4) but never into `bar2PdeBase`.
- **Memsys static config** (control 0x20800a1c): `bFbpaPresent = TRUE`, otherwise the default window moves to 0 or the L2 size (`kern_bus_gm107.c:232-252`).
- **Registers.** `NV_PFB_PRI_MMU_LOCAL_MEMORY_RANGE` 0x100CE0 must encode `fbSize` (ECC off). The WPR2 registers must follow `frtsOffset` ([gsp-boot-turing.md](gsp-boot-turing.md) C3).

---

## 4. Device design

### 4a. The FB backing

- **Size.** The fake FB covers the whole advertised address space `[0, fbSize)`, i.e. the `LOCAL_MEMORY_RANGE` size, not just `fb_length`.
- **Backing (choice).** Use one anonymous or memfd mapping with `MAP_NORESERVE`, so pages are zero-filled on first touch. During milestone 1 the kernel touches only:
  - the reserved block (~200 KiB);
  - a few heap pages (test page, flush page, BAR1 page levels, USERD);
  - whatever the fake GSP writes at `bar1PdeBase`/`bar2PdeBase`.
- **Fake-GSP bytes.** WPR, FRTS and the heap regions need no contents unless the fake booter chooses to store something there.
- **Access widths.** 1-, 2-, 4- and 8-byte accesses at any address. The kernel uses 1/2/4-byte PRAMIN accesses (`kern_bus_gm107.c:4847-4895`). BAR3 is mapped write-combined (`:1532`), so BAR3 writes can arrive merged.

### 4b. BAR1 and BAR3 translation

- **VA equals BAR offset.**
  - BAR3 is the BAR2 aperture: the kernel's CPU pointer is `BAR3 base + VA` (`kern_bus_vbar2.c:703`).
  - BAR1's VA space starts at 0 (`kern_bus_gm107.c:926`, `:990-992`).
  - So for a trapped access at offset `o` in PCI BAR3 (or BAR1), **VA = o**.
- **Trapping.** Both BARs must be trapped regions, with no direct mmap, because their bytes do not sit at FB offset `o`.

**Where the root comes from on a GSP client.**

- The kernel programs **no** instance block and **no** bind register:
  - `kbusBindBar2` is `_5baef9` and is not reached (`g_kern_bus_nvoc.c:1687`, `kern_bus.c:58-61`);
  - `kbusInitInstBlk` is `_ac1694` (`g_kern_bus_nvoc.c:1714`), so the walker's PDB update does nothing (`bar2_walk.c:78-99`);
  - BAR1 bind and instance update are stubs (§1.4).
- The BAR2 root is therefore `bar2PdeBase`, which the device itself reported. The device may model an instance block, but nothing in the kernel reads one.
- PDE3 entries at the BAR2 root:
  - The kernel owns PDE3[0]; its value arrives as `UPDATE_BAR_PDE(barType = 1, entryValue, entryLevelShift = 47)`.
  - The device stores `entryValue` at FB `bar2PdeBase + 0`, or keeps it in device state.
  - PDE3[1..3] stay 0 (invalid). Real GSP-RM keeps its own range under PDE3[1] (`kern_bus.c:832-839`); the fake GSP needs none.
  - A later `UPDATE_BAR_PDE` with value 0 clears PDE3[0] at teardown (`kern_bus_gm107.c:2096`).
- The BAR1 root is `bar1PdeBase`, and its entries are written by the kernel itself (§1.4).

**Translation walk** (4 KiB-granular VA, 49-bit format of §1.5). All table reads come from the fake FB when the aperture is 1 (video).

1. `e3 = u64[root + 8 * VA[48:47]]`. If `e3[2:1] == 0`, the address is unmapped.
   - Aperture 1 means `next = e3[32:8] << 12` in FB.
   - Aperture 2/3 means `next = e3[53:8] << 12` in guest DMA space; the kernel does not do this for BAR1/BAR2 on TU (`kern_bus_gm107.c:145-148`, `kern_gmmu.c:125`).
2. `e2 = u64[next + 8 * VA[46:38]]`, decoded the same way.
3. `e1 = u64[next + 8 * VA[37:29]]`, decoded the same way.
4. `lo = u64[next + 16 * VA[28:21]]`, `hi = u64[next + 16 * VA[28:21] + 8]`.
   - If `lo[0] == 1`, the entry is a **2 MiB PTE**; decode `lo` as a PTE with `offset = VA[20:0]`.
   - Otherwise the small PT is `hi[32:8] << 12` if `hi[2:1]` is non-zero, and the big PT is `lo[32:4] << 8` if `lo[2:1]` is non-zero.
5. With a small PT: `pte = u64[smallPT + 8 * VA[20:12]]`, `offset = VA[11:0]`.
   - If that PTE is invalid and a big PT exists: `pte = u64[bigPT + 8 * VA[20:16]]`, `offset = VA[15:0]` (precedence: see Open).
6. PTE decode:
   - `pte[0] == 0`: unmapped. If `pte[3] == 1`, treat it as sparse: read 0, drop writes. Otherwise log a fault and do the same.
   - Aperture 0: `FB[(pte[32:8] << 12) + offset]`.
   - Aperture 2/3: guest DMA at `(pte[53:8] << 12) + offset`.
   - Aperture 1 (peer): log it and treat it as unmapped.
   - `pte[6]` READ_ONLY: drop writes. KIND and comptags are ignored in a host-RAM FB.

**Translation caching.**

- Page tables live in the same fake FB and are rewritten through BAR3 itself (§1.2 step 3e).
- The simplest correct walker keeps no TLB and walks every access (a few hundred accesses during init).
- A walker that caches must drop entries for the PDB written to `INVALIDATE_PDB` whenever `MMU_INVALIDATE` is triggered, or drop everything if `ALL_PDB` is set.

**BAR3 sizing.**

- BAR3 must be at least 16 MiB, since that much is mapped (`kern_bus_gm107.c:1525-1532`). A 32 MiB BAR3, like the real card, matches the 32 MiB of VA the page tables cover.
- BAR1: any size. At ≤ 256 MiB, mappings are 64 KiB (`kern_bus.c:514-541`); a very large BAR1 enables static BAR1, which uses 2 MiB PD0 PTEs (`kern_bus_tu102.c:367-470`).

### 4c. PRAMIN window semantics

- **Window register.** `NV_PBUS_BAR0_WINDOW` 0x1700: BASE bits 23:0 in 64 KiB units (`BASE_SHIFT` 16); TARGET bits 25:24, with 0 = VID_MEM, 2 = SYS_MEM_COHERENT, 3 = SYS_MEM_NONCOHERENT (`gm107/dev_bus.h:43-50`).
- **Window data.** BAR0 0x700000-0x7FFFFF is a 1 MiB window (`gm107/dev_ram.h:26`, `:33`). A BAR0 access at `0x700000 + off` reaches FB `(BASE << 16) + off`.
- **Alignment and limits.**
  - The kernel always programs 64 KiB-aligned bases (`kern_bus_gm107.c:4751`).
  - It moves the window whenever `off + size` would pass 1 MiB (`:4838-4843`).
  - With 24 BASE bits, the window reaches FB addresses below 2^40.
- **Access pattern.**
  - Every BASE or TARGET change is a separate 32-bit read-modify-write of 0x1700, so the register must read back what was last written.
  - The kernel caches the window and reads 0x1700 only while its cache is 0 (`:4755-4758`, `:4788-4791`).
  - Reset value (choice): `(fbSize - 1 MiB) >> 16` with TARGET 0. That is what the VBIOS leaves on real boards, per the comment at `:211-215`.
- **TARGET 2/3.** The kernel does not use them on these paths: VerifyBar2 passes 0 (`:4089`, `:4101`). The device may log and fail such accesses.
- **Out-of-range accesses.** For `(BASE << 16) + off ≥ fbSize`, return 0 and drop writes. This cannot happen on the traced paths.

### 4d. Registers that must respond

| BAR0 offset | Name | Access | Semantics the device must implement | Source |
|---|---|---|---|---|
| 0x001700 | `NV_PBUS_BAR0_WINDOW` | RW | Latch BASE[23:0] and TARGET[25:24]. Reads return the latch. | `gm107/dev_bus.h:43-50`; `kern_bus_gm107.c:4744-4790`, `:4097-4230` |
| 0x700000-0x7FFFFF | PRAMIN | RW 8/16/32 | Access FB at `(BASE << 16) + off` when TARGET = 0 | `gm107/dev_ram.h:26-35`; `kern_bus_gm107.c:4813-4903` |
| 0x001714 | `NV_PBUS_BAR2_BLOCK` | RW | Only written on BAR2 teardown or failure: RMW MODE bit 31 = PHYSICAL, then a read. Latch it; do not act on it. | `tu102/dev_bus.h:51-65`; `kern_bus_gm107.c:1604-1613` |
| 0x001704 | `NV_PBUS_BAR1_BLOCK` | – | Not written by the kernel (bind stubbed) | `g_kern_bus_nvoc.c:2003`; `tu102/dev_bus.h:39` |
| 0xB830A0 | `NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE_PDB` | RW | ADDR[31:4] = PDB >> 12, APERTURE bit 1 (0 = vid). Latch it. | `tu102/dev_vm.h:120-127`; `kern_gmmu_tu102.c:134-148` |
| 0xB830A4 | `..._MMU_INVALIDATE_UPPER_PDB` | RW | ADDR[19:0] = upper PDB bits. Latch it. | `tu102/dev_vm.h:128-130`; `kern_gmmu_tu102.c:149-153` |
| 0xB830B0 | `..._MMU_INVALIDATE` | RW | The kernel writes TRIGGER bit 31, ALL_VA bit 0, ALL_PDB bit 1 = 0, and HUBTLB_ONLY bit 2 for BAR spaces. The device drops cached translations for the latched PDB. **Reads must show TRIGGER (bit 31) = 0**: it is polled before and after every invalidate, with the default timeout (4 s OS timer), and failure is an error. | `tu102/dev_vm.h:131-188`; `kern_gmmu_gm107.c:165-265`; `kern_gmmu_tu102.c:48-117` |
| 0x100C10 | `NV_PFB_NISO_FLUSH_SYSMEM_ADDR` | RW | Latch `dma >> 8` of the sysmem flush page, written at construct and state pre-load. Nothing on the traced paths reads that page back, and the validity check is a stub. | `gm107/dev_fb.h:26-28`; `kern_mem_sys_gm107.c:281`, `:373`, `:393-404`; `g_kern_mem_sys_nvoc.h:501` |
| 0x100CE0 | `NV_PFB_PRI_MMU_LOCAL_MEMORY_RANGE` | R | `fbSize` (§3.3) | `kern_mem_sys_gp102.c:36-58` |
| 0x1FA824 / 0x1FA828 | `WPR2_ADDR_LO/HI` | R | Consistent with `frtsOffset` (§3.1) | `kernel_gsp_frts_tu102.c:506-525` |
| 0x625F04 | `NV_PDISP_VGA_WORKSPACE_BASE` | R | STATUS not valid → VGA workspace at `fbSize - 1 MiB` | `kern_disp_0400.c:52-61` |

- **VREG base.** `GPU_VREG_*` offsets are relative to BAR0 0xB80000 on the PF (`kern_gpu_tu102.c:94-101`, `tu102/dev_vm.h:28`). That is why the invalidate registers sit at 0xB830xx.
- **Status bits.** The only status polled on the FB paths is TRIGGER. PRAMIN, the window register and BAR2_BLOCK are never polled.
- **Non-register obligations (fake GSP RPCs).** Each must return `NV_OK`:

| Request | ID | Action |
|---|---|---|
| RPC `UPDATE_BAR_PDE` | 70 | Store PDE3[0] |
| control `INTERNAL_MEMSYS_L2_INVALIDATE_EVICT` | 0x20800a6c | – |
| control `INTERNAL_BUS_FLUSH_WITH_SYSMEMBAR` | 0x20800a70 | – |
| control `INTERNAL_MEMSYS_GET_STATIC_CONFIG` | 0x20800a1c | Reply with `bFbpaPresent = TRUE` |

### 4e. What stays stubbed on a GSP client

| Piece | Binding for `PF_KERNEL_ONLY` | Consequence for the device |
|---|---|---|
| BAR2 bind / BAR2_BLOCK programming | `kbusBindBar2_5baef9` (`g_kern_bus_nvoc.c:1687`); physical-bootstrap mode only for SR-IOV VF (`kern_bus.c:58-61`) | No bind register to decode; BAR2 root = `bar2PdeBase` |
| BAR1/BAR2 instance block writes | `kbusInitInstBlk_ac1694` (`g_kern_bus_nvoc.c:1714`); reserved offsets never rebased (`mem_mgr_gm107.c:813`) | Nothing written at the "inst block" offsets of the reserved block |
| BAR1 bind, BAR1 inst-block VAS update | `_92bfc3` (`g_kern_bus_nvoc.c:2003`), `_ac1694` (`:1895`) | BAR1 root = `bar1PdeBase` |
| BAR2 PDE commit | `kbusCommitBar2PDEs_22ba1e` (`g_kern_bus_nvoc.c:1841`) | – |
| Physical state init | `kbusStateInitLockedPhysical_ac1694` (`g_kern_bus_nvoc.h:692`) | – |
| MMU fault buffers | `kgmmuFaultBufferInit_ac1694`, `kgmmuFaultBufferLoad_ac1694` (`g_kern_gmmu_nvoc.c:1352`, `:1386`) | No fault-buffer registers; faults need not be reported for milestone 1 |
| Sysmem flush check | `kmemsysAssertSysmemFlushBufferValid_ac1694` (`g_kern_mem_sys_nvoc.h:501`) | 0x100C10 only has to latch |
| L2 evict, sysmembar | RPC controls (`kern_mem_sys.c:1122-1136`, `kern_bus.c:430-445`) | Acknowledge only |

---

## 5. Backing options after milestone 1

**Common constraint.** A GSP-client guest manages its FB itself and hands raw FB physical addresses to "GSP":

- BAR PDEs (§1.2);
- GMMU page-level instances of every VA space, sent with `NV90F1_CTRL_CMD_VASPACE_COPY_SERVER_RESERVED_PDES` (`gpu_vaspace.c:4959`; per-level `physAddress`/`aperture` at `ctrl90f1.h:311-333`);
- page tables whose PTEs contain FB addresses (§1.5).

Any option in which the physical GPU executes guest work has to make those addresses mean the right bytes.

| | A. Fake FB in host RAM | B. Host VIDMEM allocation mapped through host BAR1 | C. Identity-mapped reserved range |
|---|---|---|---|
| Guest FB address X means | host RAM byte X | host vidmem at `alloc + X`, reached through a host BAR1 window | host vidmem at physical X |
| Needs from the host RM | nothing | A host client allocates `profile.size` (`NVOS32`/`NV01_MEMORY_LOCAL_USER`), CPU-maps it through `RM_MAP_MEMORY`, and learns per-page PAs (`NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR`, `ctrl0041.h:113`). Host BAR1 is 256 MiB on TU106 without resizable BAR (`NVreg_EnableResizableBar` default 0, [kernel-rm-hw-surface.md](kernel-rm-hw-surface.md) A), so it needs windowed remapping. | A physically contiguous vidmem allocation at a **fixed** physical offset (`NVOS32_ALLOC_FLAGS_FIXED_ADDRESS_ALLOCATE` `nvos.h:1441`, handled at `mem_utils.c:978-982`, `:1616-1628`; `NVOS32_ATTR_PHYSICALITY_CONTIGUOUS` `nvos.h:1083`), made before host allocations fragment FB. Plus the same BAR1 CPU path as B. |
| Guest `fbRegionInfoParams` | §3.3 | §3.3 | Address space = at least `range end`; regions `[0, base)` reserved, `[base, base + L')` usable, top reserved for the fake GSP. `fb_length` can still be the profile size, because the total comes from `fb_length`, not the regions (`mem_mgr_gsp_client.c:118-122`). |
| Guest page tables, PDBs, PDE copies usable by the physical GPU | no | no: every guest PA differs from the real one, so every PTE and every RPC payload would need rewriting. Writes by the GPU itself (CE, shaders) into page tables bypass the device, so the rewriting cannot be complete. | **yes**: guest PA = host PA |
| Channels and DMA on the physical GPU under Leandro's backend (host RM client via `vhost-user-nvrm`) | impossible; GPU work must be emulated or disabled (`RmDisableGlobalCeUtils=1`, `RmRcWatchdog=0`, open question 19) | only with a software shadow MMU | forwarded RPCs can pass guest PAs straight to the host RM as offsets into the one host allocation (`PA - base`) and as page-directory addresses |
| Isolation | total | host allocation bounds, plus translation | The device must bounds-check PRAMIN and walker results against `[base, base + L)`. The guest's own view of the top of FB (VGA workspace, WPR) must stay emulated, because at those addresses the real card has the host's VBIOS/WPR. |

**Recommendation.**

- **Milestone 1: A.** It needs nothing from the host RM, which fits the rule that the host GPU is read-only for this work. The kernel's own FB use is small and fully described in §1-§2.
- **After milestone 1: C**, with the same region-table mechanism as §3.3 shifted to the reserved range. It is the only option where page tables written by the guest, and the PDE addresses the guest sends to GSP, are valid for the physical GPU without a shadow MMU. It keeps Leandro's existing policy for the reservation size: `Profile.size = fb_length + reservation` (`crates/vhost-user-nvrm/src/vram.rs:88-104`) is what the host reserves.
- **B** is only worth it as a step toward C for measuring real-memory bandwidth. It buys nothing over A for correctness.

---

## 6. Open

1. **Big vs. small PT precedence.** The source does not say how hardware resolves a PD0 dual PDE whose big and small halves are both valid. BAR2 only fills the small half; BAR1 at 64 KiB may fill both. §4b step 5 is an assumption; check it against BAR1 traffic in the first run that reaches BAR1.
2. **Sparse semantics.** "Sparse" (invalid + volatile) is only described as "supported directly in HW" (`kern_gmmu_gm200.c:41-44`). Read-as-zero and write-drop is an assumption.
3. **Real GSP-RM's choices.** What real GSP-RM reports in `fbRegionInfoParams` on TU106, and where it puts `bar1PdeBase`/`bar2PdeBase`, is not visible in the kernel source. The non-WPR-heap placement in §3.2 is a choice. Capture a real `GET_GSP_STATIC_INFO` reply on the host only if that is ever allowed; it touches the host driver.
4. **Compression.** Whether the region flags `supportCompressed`/`supportISO` should be set is open. With a host-RAM FB, compressible kinds cannot behave like real ones. The kernel-side consequences (comptag allocation, `bDisableCompbitBacking` in the memsys static config) were not traced.
5. **Pre-bootstrap window reads.** On first use, `kbusSetBAR0WindowVidOffset_GM107` reads 0x1700 only while its cache is 0 (`kern_bus_gm107.c:4755-4758`). Whether any earlier code reads PRAMIN through the reset window was only checked by grep (§1.0), not by trace.
6. **Access splitting.** BAR3 is mapped write-combined. Whether QEMU/vfio-user delivers merged writes larger than 8 bytes, or splits them, depends on the VMM, not the driver.
7. **Option C host side.**
   - Can a host client obtain a multi-GiB contiguous fixed-address vidmem allocation after the host driver is up? PMA fragmentation and host console allocations decide that; the source does not.
   - Does host TU106 BAR1 support resizing? That decides between B and C and windowed access.
8. **Late FB writes.** CeUtils and the watchdog write FB through the GPU (CE copy, GR channel), not the CPU. Under option A they need emulation or the regkeys (open question 19). Milestone 1 assumes the regkeys.
9. **Exact eheap VAs.** The VA of the BAR2 self-map, the flush page and the test page depend on eheap allocation order. The device must not hard-code them.
