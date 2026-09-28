<!-- SPDX-License-Identifier: MIT -->
# Compression: the guest's view (2026-09-28)

Scope: what a 615 guest decides compression by, and what the device answers with `CARAXES_COMPRESSION=slice` (workstream B, `crates/vfio-user-nvgpu/src/compression.rs`). Host side: workstream A; ZBC: workstream C; vGPU/MIG background: [compression-isolation-2026-09-28.md](compression-isolation-2026-09-28.md). `K` = `615 tree: src/nvidia/src/kernel/`.

## Facts

### The card

- The RTX 2070 runs 1:1 comptag lines, not raw mode: host RM 615.71.09 answers `FB_GET_INFO_V2` `1TO1_COMPTAG_ENABLED` 1 and `FB_GET_COMPBIT_STORE_INFO` `comptaglineAllocationPolicy` 1 = `POLICY_1TO1` [chips/tu106/recorded-controls.tsv:49, :76; 615 tree: src/common/sdk/nvidia/inc/ctrl/ctrl0080/ctrl0080fb.h:186].
- Post-L2 compression is on: the card's `FB_GET_CAPS_V2` is `0b 81 12`, `DISABLE_PLC_GLOBALLY` (2:0x04) clear [matrix/traces/610.57.04/control/vk-enum.tsv, reply of 0x801307; K/gpu/mem_mgr/mem_mgr_ctrl.c:122-125].
- The card's compressible top is 0x1_f7d0_0000: `MaxCompbitLine` 0x1f7d0 and `COMPRESSION_SIZE` 0xf7d0_0000 (the same bytes cut to 32 bits) [recorded-controls.tsv:10, :76].
- Its backing store is host FB at 0x16a_0000, 31.5 MiB, below every reservation [recorded-controls.tsv:76].
- A page's comptag line is `1 + SPA[35:16]`, written to PTE bits 55:36 (20 bits) [K/gpu/mem_mgr/arch/turing/mem_mgr_tu102.c:505-512; 615 tree: src/common/inc/swref/published/turing/tu102/dev_fb_addendum.h:27, dev_mmu.h:117].

### Decision chain of the Linux guest

1. The guest caches `INTERNAL_MEMSYS_GET_STATIC_CONFIG` once [K/gpu/mem_sys/kern_mem_sys.c:517-529].
2. The FB region flags come from the NVKV key `FB_REGION_FLAGS` [K/gpu/gsp/kernel_gsp.c:7832; K/gpu/mem_mgr/mem_mgr_gsp_client.c:89].
3. Scrub on free is on for TU106 unless `RMDisableScrubOnFree` is set or the OS is Windows WDDM [615 tree: src/nvidia/generated/g_mem_mgr_nvoc.c:411-416; K/gpu/mem_mgr/mem_mgr.c:221-225; K/gpu/mem_mgr/arch/maxwell/mem_mgr_gm107.c:1506-1515].
4. 1:1 sets `bUseVasForCeMemoryOps` [mem_mgr_gm107.c:1523-1527]: the scrubber's CeUtils runs in virtual mode [K/gpu/mem_mgr/mem_scrub.c:152-155] over an alias of the whole heap with kind 0x09 [K/gpu/mem_mgr/arch/maxwell/mem_utils_gm107.c:456-480, 831-850; mem_mgr_tu102.c:568-577]; the memmgr CeUtils too [mem_mgr.c:4074-4079].
5. An allocation that asks for compression loses it when `bDisableCompbitBacking` is set [mem_mgr_gm107.c:428-447].
6. The kind: generic 0x08 (0x09 with PLC off), depth S8Z24 0x0C; without compression 0x06 and 0x03 [mem_mgr_tu102.c:245-272; tu102/dev_mmu.h:97-111].
7. `kmemsysAllocComprResources_KERNEL` needs 1:1 or raw [kern_mem_sys.c:473-474], scrub on free unless Windows WDDM, vGPU host, VF or SLI [:480-495], and in 1:1 the CE VA space [:496-499]; it marks the surface `ctagId` -1 and `COMPR_REQUIRED` [:502-503].
8. A ZBC-capable kind takes a ZBC reference, `INTERNAL_MEMSYS_SET_ZBC_REFERENCED` on the first one [mem_mgr_gm107.c:539-557].
9. A non-PMA heap allocation with `COMPR_REQUIRED` is placed only in a region with `supportCompressed` [K/gpu/mem_mgr/heap.c:1068-1081].
10. At map time a compressible 4 KiB mapping is widened to the physical page size [K/gpu/mem_mgr/arch/maxwell/virt_mem_allocator_gm107.c:742-756]; a PTE in a region without `supportCompressed` gets the uncompressed kind and line 0 [:1866-1881, 1931-1943].
11. `ctagId` -1 makes the comptags physically based [mem_mgr.c:2601-2614]; the PTE line is `memmgrDetermineComptag_TU102(PA)` [K/gpu/mmu/kern_gmmu.c:974-991]; UVM's external mappings take the same path [K/rmapi/nv_gpu_ops.c:4349-4389].
12. `1TO1_COMPTAG_ENABLED` is answered from the static config inside the guest [K/gpu/mem_sys/kern_mem_sys_ctrl.c:660-665]; `COMPRESSION_SIZE` goes to GSP [:871-918].
13. The compbit store is read by the kernel only to save it over a suspend, which no chip asks for [K/gpu/mem_mgr/mem_mgr_pwr_mgmt.c:365-380; generated/g_kern_mem_sys_nvoc.c:289-290].
14. `HEAP_START` does not move: the CBC branch needs region 0 reserved, and it is not [mem_mgr_tu102.c:779-830; kern_mem_sys_ctrl.c:450-495].

### Windows 617.14

- The KMD chooses the kinds [mem_mgr_tu102.c:205-224, `PDB_PROP_GPU_EXTERNAL_HEAP_CONTROL`]; scrub on free is off and exempt [mem_mgr_gm107.c:1506-1515; kern_mem_sys.c:485]; 1:1 is still required [kern_mem_sys.c:473].
- Its adapter start sends `FB_GET_INFO_V2` (with `COMPRESSION_SIZE` and `1TO1_COMPTAG`), then `FB_GET_COMPBIT_STORE_INFO` [caraxes/runs/win11-2026-09-25/night-2026-09-25/X0-rpc-sequence.txt:300-304; caraxes/reference/rtx2070/kmd-controls-2026-09-26.md:34-42].
- The D16 refusal and the card's answer (`CARAXES_WIN_COMPBIT=1`) stopped at the same place [WINDOWS-LOOP.md:211-213]; the KMD still saw `disable_compbit_backing` [caraxes/runs/windows-kd-2026-09-27/README.md:137-145].
- RM refuses `FB_GET_COMPBIT_STORE_INFO` to a vGPU guest [K/rmapi/rmapi_utils.c:212].

### The answers

| Answer | Off (D16, byte for byte) | Slice | Drives |
|---|---|---|---|
| memsys `bOneToOneComptagLineAllocation` | 0 | 1 (card) | chain 4, 7, 11 |
| memsys `bDisableCompbitBacking` | 1 | 0 | chain 5, 14 |
| memsys raw, 1:4, `bDisablePostL2Compression` | 0 | 0 (card) | chain 6 |
| FB region 0 `supportCompressed` | 0 | 1; region 1 (carve-out, pool) 0 | chain 9, 10 |
| `FB_GET_INFO_V2` `COMPRESSION_SIZE` | host or recorded 0xf7d0_0000 | `MaxCompbitLine` × 64 KiB, cut to 32 bits (0xe8e0_0000 for the 4 GiB slot) | Windows KMD |
| `FB_GET_COMPBIT_STORE_INFO` | NOT_SUPPORTED | policy 1TO1, card geometry, `MaxCompbitLine` of this VM; size, addresses, CBC base 0 | Windows KMD |
| `FB_GET_CTAGS_FOR_CBC_EVICTION` | numCompTags 0 | same | none seen |
| `FB_GET_COMPBITCOPY_CONSTRUCT_INFO` | NOT_SUPPORTED | same | none seen |

- The guest's own lines of the 4 GiB slot are 0xe901..=0x1e8e0: its pages, no other VM's [compression.rs tests `comptag_lines_follow_the_page`].
- The model of chain 5-11 in `compression.rs` (`guest615_alloc_compr_resources_preconditions_hold`) holds with these answers: Linux with scrub on free and Windows get kind 0x0C with their own lines; Linux with `RMDisableScrubOnFree=1` gets NV_ERR_INVALID_STATE; off keeps 0x03 and 0x06.

## Consequence for Caraxes

- A Linux guest must drop `RMDisableScrubOnFree=1` under the slice; the device warns when it sees the key [boot.rs `guest_key_warnings`].
- The scrubber then runs a kernel CE channel in virtual mode over a kind-0x09 alias of the heap; the device must execute it. The channel names its own VA space [K/gpu/mem_mgr/channel_utils.c:83-89], which removes item 1 of docs/caraxes/mapping.md §9; item 2, a path for a bare kernel CE channel, remains.
- Workstream A: the grant "reservation allocated compressible" goes to `FbLayout::with_compression` (TODO in main.rs); the shadow takes kinds 0x08-0x0E on slice pages, and the host RM writes the lines.
- Workstream C: ZBC kinds are the compressible kinds, so the guest now sends `SET_ZBC_*` and `SET_ZBC_REFERENCED` (today ACKed).
- A costume presenting another chip gets no compression: its HAL expects another policy (`[compression] chip`).

## Open

- Does the Windows KMD accept a compbit store without a location, or does it need NOT_SUPPORTED as vGPU guests get?
- `cbcCoveragePerSlice` (0xd8_0000) is passed on as the card's; its derivation is unknown.
- Off keeps the host's `COMPRESSION_SIZE` although D16 says no compression.
- `DMA_FLUSH` COMPTAG stays a no-op; the note in dmodel.rs still says D16.
