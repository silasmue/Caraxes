<!-- SPDX-License-Identifier: MIT -->
# Synthetic VBIOS for the TU106 device

This note covers what the 610.57.04 kernel RM (GSP-client mode, TU10x) reads from the
VBIOS image. It then specifies a minimal synthetic image that passes every check,
so the device can serve the PROM window without a real VBIOS capture. The generator and
the Python oracle live in `caraxes/tools/synth-vbios.py`.

Background: experiment E0 ([experiments.md](../history/caraxes-experiments-2026-09-29.md), "E0 result") failed in
`s_locateExpansionRoms` with `NV_ERR_INVALID_DATA` (0x25). The sysfs capture
`caraxes/reference/rtx2070/rom.bin` (130048 bytes) ends at 0x1FC00. The NPDE of the EFI image
says a third image follows there.

Path abbreviations (all under `vendor/open-gpu-kernel-modules/src/`):

| Short | Path |
|---|---|
| `vbios.c` | `nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_vbios_tu102.c` |
| `fwsec.c` | `nvidia/src/kernel/gpu/gsp/kernel_gsp_fwsec.c` |
| `frts.c` | `nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_frts_tu102.c` |
| `gsp_tu.c` | `nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_tu102.c` |
| `falcon_tu.c` | `nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_falcon_tu102.c` |
| `kernel_gsp.c` | `nvidia/src/kernel/gpu/gsp/kernel_gsp.c` |
| `pcie.h` | `nvidia/inc/kernel/platform/pci_exp_table.h` |
| `nvoc.c` / `nvoc.h` | `nvidia/generated/g_kernel_gsp_nvoc.c` / `.h` |
| `tu102/` | `common/inc/swref/published/turing/tu102/` |

## Summary

- The kernel RM reads the VBIOS for one purpose only: to extract the FWSEC falcon ucode
  and patch an FRTS command into its DMEM image. The image is freed right after parsing
  (`kernel_gsp.c:4931`). Nothing from it is forwarded to GSP-RM.
- The synthetic image is the capture, unchanged (x86 image at 0x0 and EFI image at 0xEC00),
  with a 0x600-byte NVIDIA extension image (code type 0xE0) appended at 0x1FC00. The
  extension image holds the PCI headers, a one-entry falcon ucode table at the spot where
  the captured BIT already points, a V2 FWSEC descriptor, 0x200 bytes of dummy code, and a
  0x100-byte DMEM image with the application interface and DMEM mapper. Total size is
  0x20200 (131584) bytes.
- The oracle's results: `biosSize` 0x20200, `expansionRomOffset` 0x11000, BIT at 0x1B0,
  ucode table at 0x1FD24, FWSEC descriptor V2 at 0x1FE00, and the FRTS command at offset
  0x80 of the FWSEC data buffer (`frtsRegionOffset4K` at data+0xA0).
- Registers the parse needs: only the PROM window (0x300000 + i) and, when the
  application id is 0x85 or 0x45, the debug fuse at 0x2174C. The default application id
  0x05 is accepted under either fuse value. TU10x has no ucode fuse-version read.

## a. Consumers of the VBIOS image

Call order, with every function that reads the image or anything derived from it:

| Step | Function (file:line) | What it consumes |
|---|---|---|
| 1 | `kgspInitRm_IMPL` (`kernel_gsp.c:4878`), block at 4914-4958 | Runs only while `pFwsecUcode` is NULL. Calls extract, then parse, then frees the image (4931). The version string starts as "unknown" (4919) and is set from BIOSDATA (4933-4936). A parse error fails the boot (4938-4943). An extract error other than `NV_ERR_NOT_SUPPORTED` also fails it (4947-4957) |
| 1a | HAL: `kgspExtractVbiosFromRom_TU102` for TU102..AD107 (`nvoc.c:1396-1398`) | |
| 2 | `kgspExtractVbiosFromRom_TU102` (`vbios.c:442-569`) | PROM window reads. Returns `KernelGspVbiosImg {pImage, biosSize, expansionRomOffset}` (`nvoc.h:92-97`) |
| 2a | `kbifPreOsGlobalErotGrantRequest_HAL` (`vbios.c:480-489`) | On TU it is the stub `_ac1694` that returns `NV_OK` (`g_kernel_bif_nvoc.c:1068-1077`, `g_kernel_bif_nvoc.h:1754-1756`). No register access |
| 2b | `s_romImgFindPciHeader_TU102` (`vbios.c:202-273`) | IFR ("NVGI") header. Called only when offset 0 has no PCI ROM signature (`vbios.c:494-503`). Our image never takes this path |
| 2c | `s_locateExpansionRoms` (`vbios.c:276-416`) | Image chain, see section b |
| 3 | `kgspParseFwsecUcodeFromVbiosImg_IMPL` (`fwsec.c:1110-1158`) | BIT, falcon ucode table, FWSEC descriptor, see section c |
| 3a | `kgspIsDebugModeEnabled_TU102` (`gsp_tu.c:93-104`, HAL `nvoc.c:620-622`) | Reads fuse 0x2174C and chooses application id 0x45 or 0x85 |
| 3b | `s_vbiosFindBitHeader` (`fwsec.c:404-455`), `s_vbiosParseFwsecUcodeDescFromBit` (`fwsec.c:467-693`) | BIT header, BIOSDATA token (version), FALCON_DATA token, table, entries, descriptor |
| 3c | `s_vbiosNewFlcnUcodeFromDesc` (`fwsec.c:1035-1095`), then `s_vbiosFillFlcnUcodeFromDescV2` (`fwsec.c:706-865`) or `...V3` (`fwsec.c:878-1021`) | Copies code and data (V2) or the whole ucode and signatures (V3) into sysmem buffers |
| 4 | `kgspFreeVbiosImg` (`kernel_gsp_booter.c:36-50`) | Frees the image. Only `pFwsecUcode` lives on |
| 5 | `kgspPrepareForBootstrap_TU102` (`gsp_tu.c:425-469`), called from `kernel_gsp.c:4822` | Calls `kgspPrepareForFwsecFrts_HAL` with `pWprMeta->frtsOffset` (456-459) |
| 5a | `kgspPrepareForFwsecFrts_TU102` (`frts.c:577-589`), then `s_prepareForFwsec_TU102` (`frts.c:274-451`), then `s_vbiosPatchInterfaceData` (`frts.c:157-261`) | The DMEM image of the parsed FWSEC: application interface, DMEM mapper and command buffer (section d) |
| 6 | `kgspBootstrap_TU102` (`gsp_tu.c:522-...`), FWSEC at 545-557 | Resets the GSP falcon, then calls `kgspExecuteFwsec_HAL` |
| 6a | `kgspExecuteFwsec_TU102` (`frts.c:463-565`), then `kgspExecuteHsFalcon_TU102` (`falcon_tu.c:380-435`), then `s_prepareHsFalconWithLoader` (`falcon_tu.c:322-361`) | `imemNsPa`/`imemNsSize`/`imemSecPa`/`imemSecSize`/`dmemSize` and the buffer physical addresses go into the bootloader DMEM descriptor. After the halt, the kernel checks the VBIOS scratch and WPR2 registers |
| 7 | `kgspTeardown_TU102` (`gsp_tu.c:660-705`) | FWSEC-SB at unload (`CMD_SB` 0x19) through the same DMEM mapper, then checks the PGC6 registers and scratch 0x15 |
| 8 | Unload (`kernel_gsp.c:5300-5303`) | Resets the version string and frees `pFwsecUcode` |

Other hits for the grep patterns, and why they do not consume the image:

- `_kgspVbiosVersionToStr` (`kernel_gsp.c:4435-4442`) only formats the BIOSDATA version
  for log messages (`frts.c:447, 561`, `gsp_tu.c:1196`).
- `GspStaticConfigInfo.bVbiosValid / vbiosSubVendor / vbiosSubDevice / vbiosRevision /
  vbiosOemRevision` (`nvidia/inc/kernel/gpu/gsp/gsp_static_config.h:133-137`) are
  **filled by GSP-RM** and read by `gpu.c:2557-2570` (CPER firmware version) and
  `gpu_boost_mgr.c:708-746` (sub-vendor matching). They do not come from `pVbiosImg`.
- `NV_PROM_DATA` appears only in `vbios.c` (grep over the whole tree, excluding the
  register headers). `kernel-open/` never maps the PCI expansion ROM: a grep for
  `pci_map_rom`, `pci_platform_rom` and `PCI_ROM_ADDRESS` finds nothing.
- `pkcHashInfo` does not appear in 610.57.04. The only BCRT remnants are the two
  code-type defines at `vbios.c:46-47` and `BCRT30_RSA3K_SIG_SIZE` (`fwsec.c:189`).

## b. The image chain walk

Reads go through `s_romImgReadGeneric` (`vbios.c:99-169`). Each one is an aligned 32-bit
`NV_PROM_DATA` read at BAR0 0x300000 + offset (`tu102/dev_ext_devices.h:27`, `vbios.c:79-86`).
A value that crosses a dword boundary takes a second dword read (150-159). Offsets are
`baseOffset + offset`, and `offset + size` must not exceed `maxOffset` = 0x100000
(`vbios.c:136-145`, 423-429, 477). Otherwise the result is `NV_ERR_INVALID_OFFSET`.

Start (`vbios.c:492-510`): the u16 at PROM 0 must be 0xAA55, 0x4E56 or 0xBB77
(`pcie.h:31-36`). If not, the IFR path runs (2b). The capture has 0xAA55, so
`pciOffset` = 0.

Per image, with `currBlock` = the image start (`vbios.c:296-395`):

| Read | Offset | Size | Use (`vbios.c` line) |
|---|---|---|---|
| ROM header `pciDataStrucPtr` | image + 0x18 (`pcie.h:40, 47`) | 2 | 308. PCIR = image + ptr (311) |
| PCIR `sig` | PCIR + 0x0 | 4 | 313-319. Must be "PCIR" 0x52494350, "NPDS" 0x5344504E or "RGIS" 0x53494752 (`pcie.h:68-73`). Otherwise `NV_ERR_INVALID_DATA` (**this is the E0 failure**) |
| PCIR `lastImage` | PCIR + 0x15 (`pcie.h:84, 99`) | 1 | 321-322. Bit 7 (`PCI_LAST_IMAGE`, `pcie.h:75`) gives the initial "last" |
| PCIR `imageLen` | PCIR + 0x10 (`pcie.h:83, 96`) | 2 | 323. Unit is 512 bytes (`pcie.h:76`). `subImgLen` = `imageLen` |
| PCIR `pciDataStructLen` | PCIR + 0xA (`pcie.h:80, 93`) | 2 | 330. NPDE = `(PCIR + len + 0xF) & ~0xF` (331), aligned in **absolute** PROM offsets |
| NPDE `signature` | NPDE + 0x0 (`pcie.h:108, 120`) | 4 | 338-341. "NPDE" 0x4544504E (`pcie.h:104`). If absent, the PCIR values stand |
| NPDE `nvPciDataExtRev` | NPDE + 0x4 (`pcie.h:110`) | 2 | 343-346. Only 0x100 and 0x101 are used (`pcie.h:105-106`) |
| NPDE `nvPciDataExtLen` | NPDE + 0x6 (`pcie.h:109`) | 2 | 348 |
| NPDE `subimageLen` | NPDE + 0x8 (`pcie.h:111`) | 2 | 351. **Replaces** the image length used for stepping and sizing |
| NPDE `privLastImage` | NPDE + 0xA (`pcie.h:112`) | 1 | 355-359. Read when `len` ≥ 0xB, and then bit 7 **replaces** the PCIR last bit. Otherwise, if `subimageLen` < `imageLen`, the image is not last (360-363) |
| PCIR `codeType` | PCIR + 0x14 (`pcie.h:82, 98`) | 1 | 371 |

Bookkeeping (`vbios.c:374-394`):

- `blockOffset = currBlock - pciOffset`, `blockSize = subImgLen * 512`.
- The first image with code type 0xE0 (`NV_BCRT_HASH_INFO_BASE_CODE_TYPE_VBIOS_EXT`,
  `vbios.c:47`) sets `extRomOffset = blockOffset` (377-380).
- Otherwise, the first image with code type 0x00 (`..._VBIOS_BASE`, `vbios.c:46`) sets
  `baseRomSize = blockSize` (381-384). Other code types (EFI = 3) are only stepped over.
- If the image is last, stop. Otherwise `currBlock += subImgLen * 512` (387-394).

Results:

- `biosSize` = `blockOffset + blockSize` **of the last image** (397-401). It must be at most
  0x100000 (519-524). The copy loop then reads exactly that many bytes from
  `pciOffset` (540-550).
- `expansionRomOffset = extRomOffset - baseRomSize`, when both are non-zero. Otherwise it
  is 0 (403-413).

The captured chain as the oracle reads it:

| Image | Start | PCIR | Code type | imageLen | NPDE | subimageLen | NPDE len | last (PCIR / NPDE, effective) |
|---|---|---|---|---|---|---|---|---|
| x86 | 0x00000 | 0x00170 | 0x00 | 0x76 | 0x00190 rev 0x101 | 0x76 | 0x14 | 0 / 0, **not last** |
| EFI | 0x0EC00 | 0x0EC1C | 0x03 | 0x88 | 0x0EC40 rev 0x100 | 0x88 | 0x10 | 1 / 0, **not last** |
| (ext) | 0x1FC00 | missing in the capture | | | | | | |

This gives `baseRomSize` = 0xEC00. With the extension image at 0x1FC00,
`expansionRomOffset` = 0x1FC00 - 0xEC00 = **0x11000**, which is exactly the EFI image's size.
In effect, the pointers that are relative to `expansionRomOffset` address a virtual
layout where the x86 image is followed directly by the extension image.

**Where the BIT is searched, and what each offset is relative to.** `s_vbiosFindBitHeader`
scans the **whole copied image** (all images, offset 0 to `biosSize - 4`) byte by byte and
takes the first checksum-valid hit (`fwsec.c:419-447`). In the capture that is the x86
image at 0x1B0. BIT token `DataPtr` values, including the BIOSDATA and FALCON_DATA
pointers, are **image-absolute** (`fwsec.c:541-542, 563-564`). The falcon ucode table
pointer and each entry's `DescPtr` are **relative to `expansionRomOffset`**
(`fwsec.c:575-576, 606-609, 629, 671`). Ucode code and data follow the descriptor directly
(`fwsec.c:765, 777`) and are therefore image-absolute as well.

## c. FWSEC parse path

All structure reads use `s_vbiosReadStructure` (`fwsec.c:339-362`): packed little-endian
data, with each field widened to NvU32 (`bios_U008/16/32` are all `NvU32`,
`nvidia/inc/kernel/gpu/vbios/bios_types.h:53-55`). The read fails with
`NV_ERR_INVALID_OFFSET` if `offset + packedSize > biosSize` (355-359).

1. **BIT header** (`fwsec.c:47-57`, format `1w1d1w4b`, 12 bytes): Id u16 = 0xB8FF
   (`fwsec.c:43`), Signature u32 = 0x00544942 "BIT\0" (44), BCD_Version u16, HeaderSize u8
   (at +8, `fwsec.c:45`), TokenSize u8, TokenEntries u8, HeaderChksum u8. The byte sum over
   `HeaderSize` bytes must be 0 mod 256 (434-441). If the ID matches but the dword read at
   `addr+2` or a checksum byte read goes past `biosSize`, the whole search fails (439, 450).
   The trailing padding must therefore not contain 0xFF 0xB8 near the end; ours is zeros.
   If no BIT is found: `NV_ERR_GENERIC` (454), logged as "failed to find BIT header"
   (1135).
   - Captured: BIT at 0x1B0, version 0x0100, HeaderSize 12, TokenSize 6, 17 tokens,
     checksum OK.
2. **Tokens** (`fwsec.c:60-73`): with TokenSize ≥ 8 the format is `2b1w1d` (32-bit
   `DataPtr`). With ≥ 6 it is `2b2w` (16-bit `DataPtr`). Anything smaller gives
   `NV_ERR_INVALID_STATE` (495-509). Token *i* sits at `bitAddr + HeaderSize + i*TokenSize`
   (521-524). Unreadable tokens are skipped (525-531).
   - BIOSDATA 0x42 (75), DataVersion 1 or 2, DataSize > 5: reads `Version` u32 and
     `OemVersion` u8 at `DataPtr` (534-552). Captured: 0x90063D00 / 0x76, which gives
     "90.06.3D.00.76" and matches `nvidia-smi-q.txt` and `proc-information.txt`.
   - FALCON_DATA 0x70 (90), DataVersion **2**, DataSize ≥ 4 (555-560): reads
     `FalconUcodeTablePtr` u32 at `DataPtr` (563-571). Captured: token 14, `DataPtr`
     0x3B7, pointer **0xED24**.
3. **Falcon ucode table header** at `expansionRomOffset + FalconUcodeTablePtr`
   (`fwsec.c:100-112, 574-584`), 6 bytes: Version = 1, HeaderSize ≥ 6, EntrySize ≥ 6
   (587-592, otherwise the token is skipped), EntryCount. `DescVersion` and `DescSize` are
   not checked.
4. **Entries** at `table + HeaderSize + i*EntrySize` (`fwsec.c:114-123, 605-610`), format
   `2b1d`: ApplicationID u8, TargetID u8 (unchecked), DescPtr u32. An entry is taken if its
   ApplicationID is 0x05 (`FIRMWARE_SEC_LIC`), or is 0x45 under the debug fuse, or is 0x85
   under the production fuse (`fwsec.c:125-127, 620-625`).
   - The fuse is `NV_FUSE_OPT_SECURE_GSP_DEBUG_DIS` at **0x2174C**, bit 0
     (`tu102/dev_fuse.h:26-29`). Debug mode means bit 0 = 0 (`_DATA_NO`)
     (`gsp_tu.c:101-103`). The device should return 1 (production). Booter image selection
     needs that anyway (see [registers-turing.md](registers-turing.md)). With application
     id 0x05 the parse works under either value.
5. **Descriptor header** u32 `vDesc` at `expansionRomOffset + DescPtr` (`fwsec.c:628-637`):
   - Bit 0 must be 1 ("version available", `fwsec.c:129-131, 640-646`).
   - Bits 15:8 are the version (135-139). Bits 31:16 are the size (140).
   - V2 needs size ≥ 60 (format `15d`). V3 needs size ≥ 44 (format `9d1w2b2w`).
     Anything else is skipped (653-669).
   - The full descriptor is read at `DescPtr + expansionRomOffset` (671-681). If no entry
     qualifies: `NV_ERR_INVALID_DATA` (692), logged as "failed to parse FWSEC ucode desc"
     (1144).
6. **V2 descriptor** (`fwsec.c:148-168`, 60 bytes) becomes `KGSP_FLCN_UCODE_BOOT_WITH_LOADER`
   (`fwsec.c:741`, structure at `nvoc.h:123-142`). Derived values (`fwsec.c:747-762`):
   - `imemSize` = `IMEMLoadSize`; `imemNsSize` = `IMEMLoadSize - IMEMSecSize`;
     `imemNsPa` = `IMEMPhysBase`
   - `imemSecSize` = align256(`IMEMSecSize`);
     `imemSecPa` = `IMEMSecBase - IMEMVirtBase + IMEMPhysBase`
   - `dataOffset` = `DMEMOffset`; `dmemSize` = `DMEMLoadSize`; `dmemPa` = `DMEMPhysBase`;
     `interfaceOffset` = `InterfaceOffset`
   - The code buffer is align256(`imemSize`) and the data buffer is align256(`dmemSize`)

   Checks, each giving `NV_ERR_INVALID_OFFSET`:
   - `descOffset + descSize` < `biosSize` (765-769)
   - code end ≤ `biosSize` (771-775)
   - `code + DMEMOffset` < `biosSize` (777-781)
   - data end ≤ `biosSize` (783-787)
   - `imemNsPa` < code buffer, and `imemNsPa + imemSize` ≤ code buffer (790-799)
   - `dmemPa` < data buffer, and `dmemPa + dmemSize` ≤ data buffer (801-810)

   Then the code bytes go to code buffer + `imemNsPa` and the data bytes to data buffer +
   `dmemPa` (849-855). The sizes copied are `IMEMLoadSize` and `DMEMLoadSize`.
   `StoredSize`, `UncompressedSize` and the `alt*` sizes are not used.
7. **V3 descriptor** (`fwsec.c:170-188`) becomes `KGSP_FLCN_UCODE_BOOT_FROM_HS` (908):
   - The ucode size is align256(`StoredSize`). The signatures are the `descSize - 44`
     bytes after the descriptor (981-994). Each one is `BCRT30_RSA3K_SIG_SIZE` = 384
     bytes (930). The range checks are at 937-978.
   - Signature selection happens later in `s_prepareForFwsec_TU102` (`frts.c:357-384`).
     It uses `kgspReadUcodeFuseVersion_HAL`, which on TU10x is `_b2b553` and returns 0
     **without a register read** (`nvoc.c:1092-1094`, `nvoc.h:1897-1899`). So bit 0 of
     `SignatureVersions` would select the first signature.
   - **A V3 FWSEC cannot run on TU10x**: `kgspExecuteHsFalcon_TU102` accepts only
     WITH_LOADER and DIRECT and returns `NV_ERR_NOT_SUPPORTED` for BOOT_FROM_HS
     (`falcon_tu.c:398-409`, HAL `nvoc.c:1536-1538`). The synthetic image uses V2. There is
     no ucode fuse-version register for the device to answer on TU10x.
8. **Failure masking**: if the V2/V3 fill fails, `s_vbiosNewFlcnUcodeFromDesc` logs the
   error, sets `*ppFlcnUcode = NULL` and still returns `NV_OK` (`fwsec.c:1080-1094`). The
   failure then shows up later as the assertion that `pFwsecUcode` is not NULL in
   `s_prepareForFwsec_TU102` (`frts.c:295`, `NV_ERR_INVALID_ARGUMENT`).

## d. FRTS command construction and what the CPU checks afterwards

Structures (`frts.c:64-131`). These have natural C alignment, no `pragma pack`; the sizes
below follow from the x86-64 ABI:

| Structure | Layout | Size |
|---|---|---|
| `FALCON_APPLICATION_INTERFACE_HEADER_V1` (64-70) | version u8, headerSize u8, entrySize u8, entryCount u8 | 4 |
| `FALCON_APPLICATION_INTERFACE_ENTRY_V1` (72-76) | id u32, dmemOffset u32 | 8 |
| `FALCON_APPLICATION_INTERFACE_DMEM_MAPPER_V3` (80-99) | +0x00 signature, +0x04 version u16, +0x06 size u16, +0x08 cmd_in_buffer_offset, +0x0C cmd_in_buffer_size, +0x10 cmd_out_buffer_offset, +0x14 cmd_out_buffer_size, +0x18 nvf_img_data_buffer_offset, +0x1C .._size, +0x20 printfBufferHdr, +0x24 ucode_build_time_stamp, +0x28 ucode_signature, **+0x2C init_cmd**, +0x30 ucode_feature, +0x34 ucode_cmd_mask0, +0x38 ucode_cmd_mask1, +0x3C multiTgtTbl | 64 |
| `FWSECLIC_READ_VBIOS_DESC` (104-111) | +0x00 version, +0x04 size, +0x08 gfwImageOffset u64, +0x10 gfwImageSize, +0x14 flags | 24 |
| `FWSECLIC_FRTS_REGION_DESC` (115-122) | +0x00 version, +0x04 size, +0x08 frtsRegionOffset4K, +0x0C frtsRegionSize, +0x10 frtsRegionMediaType | 20 |
| `FWSECLIC_FRTS_CMD` (127-131) | readVbiosDesc at +0x00, frtsRegionDesc at +0x18, 4 bytes of tail padding (8-byte alignment from the u64) | 48 |

`s_prepareForFwsec_TU102` (`frts.c:274-451`) builds the command:
- `readVbiosDesc` = {1, 24, 0, 0, flags 2} (311-315).
- For FRTS (`CMD_FRTS` = 0x15, `frts.c:101`), `frtsRegionDesc` = {1, 20, `frtsOffset >> 12`,
  0x100 (1 MiB in 4K units, `tu102/dev_gc6_island_addendum.h:29`), media 2 = FB}
  (317-331).
- For SB (`CMD_SB` = 0x19) the buffer is just the 24-byte `readVbiosDesc` (334-339).
- `frtsOffset` = `gspFwWprEnd - frtsSize` (`gsp_tu.c:820`). It must be greater than 0
  (`frts.c:297-298`).

For a WITH_LOADER ucode, the kernel maps the **data buffer** and patches it (`frts.c:411-438`).
`s_vbiosPatchInterfaceData` (`frts.c:157-261`) works on offsets from the start of the data
buffer, **not** from `dmemPa`, and with `mappedDataSize = dmemSize`:

1. `interfaceOffset` < size, and `interfaceOffset + 4` ≤ size (177-186).
2. `entryCount` ≥ 2, otherwise `NV_ERR_INVALID_DATA` "too few interface entires"
   (189-193). The header's `headerSize` and `entrySize` are **ignored**: entries always
   start at `interfaceOffset + 4` with an 8-byte stride (195, 203-210).
3. Each entry must fit (198-207). The entry with id 4 (`DMEMMAPPER`, `frts.c:78`) must have
   `dmemOffset` < size and `dmemOffset + 64` ≤ size (212-230). If there is none:
   `NV_ERR_INVALID_DATA` (233-237).
4. `init_cmd` = cmd (239). A too-small `cmd_in_buffer_size` only logs a message
   (241-244). `cmd_in_buffer_offset` must be < size (246-249).
5. **Quirk**: the next bounds check uses `pIntFaceEntry->dmemOffset + cmdBufferSize`
   (251-255). `pIntFaceEntry` is the **last entry walked**, not the mapper. The command is
   then copied to `cmd_in_buffer_offset` without checking its end (257-258). The synthetic
   image puts the mapper entry last and sizes the input buffer at 0x40, so both are safe.

Execution (`gsp_tu.c:545-557`, `frts.c:463-565`, `falcon_tu.c:322-435`):

- The kernel resets the GSP falcon.
- It writes `RM_FLCN_BL_DMEM_DESC` (`nvidia/arch/nvalloc/common/inc/rmflcnbl.h:45-102`,
  84 bytes) to DMEM offset 0 through `DMEMC(0)`/`DMEMD(0)` (0x1101C0/0x1101C4,
  `tu102/dev_falcon_v4.h:108, 117`; `falcon_tu.c:51-90, 291-296`). The layout:
  - +0x00 reserved[4] (left uninitialised)
  - +0x10 signature[4] = 0
  - +0x20 ctxDma = 4
  - +0x24 codeDmaBase lo/hi
  - +0x2C nonSecureCodeOff = `imemNsPa`, +0x30 nonSecureCodeSize = `imemNsSize`
  - +0x34 secureCodeOff = `imemSecPa`, +0x38 secureCodeSize = `imemSecSize`
  - +0x3C codeEntryPoint = 0
  - **+0x40 dataDmaBase lo/hi**, +0x48 dataSize = `dmemSize`
  - +0x4C argc, +0x50 argv (`falcon_tu.c:340-356`)
- It loads the generic bootloader from the driver's SEC2 ucode, not from the VBIOS, to the
  top of IMEM (`falcon_tu.c:283-313`, using HWCFG 0x110108), then sets BOOTVEC and starts
  the CPU. It waits for the halt with no mailboxes (`frts.c:480-481`).

After the halt, the CPU checks:

| Register | Address | Required value | Check |
|---|---|---|---|
| `NV_PBUS_VBIOS_SCRATCH(0x0E)` | 0x1438 (`tu102/dev_bus.h:27`) | bits 31:16 = 0 (`frts.c:133-135`) | `frts.c:497-504`, otherwise `NV_ERR_GENERIC` |
| `NV_PFB_PRI_MMU_WPR2_ADDR_HI` | 0x1FA828 (`tu102/dev_fb.h:42-44`) | VAL 31:4 not 0 | `frts.c:506-513` |
| `NV_PFB_PRI_MMU_WPR2_ADDR_LO` | 0x1FA824 (`tu102/dev_fb.h:39-41`) | VAL 31:4 = `frtsOffset >> 12`, so the register reads `(frtsOffset >> 12) << 4` | `frts.c:515-525` |

At teardown, FWSEC-SB (`gsp_tu.c:675-699`, `frts.c:527-556`) requires:
- `NV_PGC6_AON_SECURE_SCRATCH_GROUP_05_PRIV_LEVEL_MASK` 0x118128 bit 0 = 1
- `..._GROUP_05(0)` GFW_BOOT progress 0x118234 bits 7:0 = 0xFF
- `NV_PBUS_VBIOS_SCRATCH(0x15)` 0x1454 bits 15:0 = 0

A failure there only asserts; unload continues.

**How the device can learn `frtsOffset`.** The FWSEC data buffer is in guest sysmem.
`dataDmaBase` arrives in the bootloader descriptor at DMEM +0x40/+0x44. With the
synthetic image:
- the DMEM mapper is at `dataDmaBase + 0x20`, with `init_cmd` (0x15 or 0x19) at +0x4C
- the command is at `dataDmaBase + 0x80`
- `frtsRegionOffset4K` is at `dataDmaBase + 0xA0`

A DMA read of those 4 bytes gives the value that WPR2_LO must reflect. The other option is
to recompute `frtsOffset` from the FB size the way `kgspPopulateWprMeta` does
(`gsp_tu.c:767-824`), but that duplicates driver logic.

Registers the device must answer consistently with the synthetic image:

| Register | Value | Why |
|---|---|---|
| 0x300000 + i, i < 0x20200 | byte i of `synth-vbios.bin`, served as 32-bit reads (8-bit reads also work, `vbios.c:64-71`) | image chain, copy, parse |
| 0x300000 + i, i ≥ 0x20200 | anything; 0xFF as now | never read after a successful walk (the copy stops at `biosSize`) |
| 0x2174C | bit 0 = 1 | production. Needed for booter PROD images. Irrelevant to the FWSEC choice with application id 0x05 |
| 0x1438 | bits 31:16 = 0 after the FWSEC halt | FRTS error code |
| 0x1FA824 / 0x1FA828 | LO = `frtsRegionOffset4K << 4`, HI non-zero after the halt. **Both 0 before FRTS** | `frts.c:506-525`; `kernel_gsp.c:4805-4811` refuses to boot if WPR2 is already up |
| 0x118128, 0x118234, 0x1454 | as above | FWSEC-SB at unload |

## e. Byte layout of the synthetic ROM

The captured x86 and EFI images stay **byte-for-byte unchanged** (the generator checks
this with `cmp`). They can stay as they are because:
- the EFI NPDE already says "not last" and points at 0x1FC00
- the captured BIT (x86 image) already carries a FALCON_DATA v2 token
- its pointer (0xED24) lands at ext+0x124, past the extension image's headers

No checksum or pointer in the captured images has to change. If a different capture put
the table inside the header area, the generator would refuse (`build_ext_image`) instead of
editing the BIT.

Alignment rules that apply:
- images are multiples of 512 bytes (`pcie.h:76`, `vbios.c:375, 393`)
- the NPDE sits at the next 16-byte boundary after the PCIR, in absolute terms
  (`vbios.c:331`)
- the code buffer and the data buffer are rounded up to 256 bytes (`fwsec.c:761-762`)
- the imem placement checks are against those rounded sizes

The code and data **image offsets** need no alignment: they are copied with `portMemCopy`.

| Region | Offset | Size |
|---|---|---|
| x86 image (captured) | 0x00000 | 0xEC00 |
| EFI image (captured) | 0x0EC00 | 0x11000 |
| Extension image (synthetic) | 0x1FC00 | 0x600 (3 blocks) |
| **Total** | | **0x20200 = 131584** |

Extension image, field by field. "Checked by" means read by the driver; "not checked"
means the value is ours and the driver ignores it.

| Offset | Size | Field | Value | Checked by |
|---|---|---|---|---|
| 0x1FC00 | 2 | ROM signature | 0xBB77 (`PCI_EXP_ROM_SIGNATURE_NV2`) | not checked for images after the first. Only offset 0 is checked (`vbios.c:492-510`) |
| 0x1FC02 | 0x16 | reserved | 0 | not checked |
| 0x1FC18 | 2 | pciDataStrucPtr | 0x0020 | `vbios.c:308, 311` |
| 0x1FC20 | 4 | PCIR sig | "NPDS" 0x5344504E | `vbios.c:313-319` |
| 0x1FC24 | 2 | vendorID | 0x10DE | not checked |
| 0x1FC26 | 2 | deviceID | 0x1F02 (copied from the x86 PCIR) | not checked |
| 0x1FC28 | 2 | deviceListPtr | 0 | not checked |
| 0x1FC2A | 2 | pciDataStructLen | 0x0018 | `vbios.c:330-331`, giving an NPDE at 0x1FC40 |
| 0x1FC2C | 1 | revision | 0 | not checked |
| 0x1FC2D | 3 | class code | 00 00 03 | not checked |
| 0x1FC30 | 2 | imageLen | 0x0003 | `vbios.c:323`. Superseded by the NPDE |
| 0x1FC32 | 2 | vendorRomRev | 0 | not checked |
| 0x1FC34 | 1 | codeType | **0xE0** | `vbios.c:371, 377-380`, which sets `extRomOffset` = 0x1FC00 |
| 0x1FC35 | 1 | lastImage | 0x80 | `vbios.c:321-322`. Superseded by the NPDE |
| 0x1FC36 | 2 | maxRunTimeImageLen | 0 | not checked |
| 0x1FC40 | 4 | NPDE sig | "NPDE" 0x4544504E | `vbios.c:338-341` |
| 0x1FC44 | 2 | nvPciDataExtRev | 0x0101 | `vbios.c:343-346` |
| 0x1FC46 | 2 | nvPciDataExtLen | 0x0010 | `vbios.c:348, 355` (≥ 0xB, so `privLastImage` is used) |
| 0x1FC48 | 2 | subimageLen | 0x0003 | `vbios.c:351, 375`, giving `biosSize` 0x1FC00 + 0x600 |
| 0x1FC4A | 1 | privLastImage | **0x80** | `vbios.c:355-359`, which ends the walk |
| 0x1FC4B | 1 | flags | 0 | not checked |
| 0x1FC4C | 0xD8 | padding | 0 | |
| 0x1FD24 | 1 | table Version | 1 | `fwsec.c:587` |
| 0x1FD25 | 1 | table HeaderSize | 6 | `fwsec.c:588, 608` |
| 0x1FD26 | 1 | table EntrySize | 6 | `fwsec.c:589, 609` |
| 0x1FD27 | 1 | table EntryCount | 1 | `fwsec.c:595` |
| 0x1FD28 | 1 | table DescVersion | 2 | not checked |
| 0x1FD29 | 1 | table DescSize | 60 | not checked |
| 0x1FD2A | 1 | entry ApplicationID | 0x05 (the `--appid` option can set 0x85 or 0x45) | `fwsec.c:620-625` |
| 0x1FD2B | 1 | entry TargetID | 0 | not checked |
| 0x1FD2C | 4 | entry DescPtr | 0xEE00 (= 0x1FE00 - 0x11000) | `fwsec.c:629, 671` |
| 0x1FD30 | 0xD0 | padding | 0 | |
| 0x1FE00 | 4 | vDesc | 0x003C0201 (available, V2, size 60) | `fwsec.c:640-660` |
| 0x1FE04 | 4 | StoredSize | 0x300 | not checked for V2 |
| 0x1FE08 | 4 | UncompressedSize | 0x300 | not checked |
| 0x1FE0C | 4 | VirtualEntry | 0 | copied to `codeEntry` (753). The loader uses 0 anyway (`falcon_tu.c:353`) |
| 0x1FE10 | 4 | InterfaceOffset | 0x000 | `fwsec.c:759`, `frts.c:177-186` |
| 0x1FE14 | 4 | IMEMPhysBase | 0 | `fwsec.c:750, 790-799` |
| 0x1FE18 | 4 | IMEMLoadSize | 0x200 | `fwsec.c:748, 771-775, 795-799` |
| 0x1FE1C | 4 | IMEMVirtBase | 0 | `fwsec.c:752` |
| 0x1FE20 | 4 | IMEMSecBase | 0x100 | `fwsec.c:752`, giving `imemSecPa` 0x100 |
| 0x1FE24 | 4 | IMEMSecSize | 0x100 | `fwsec.c:749, 751`, giving `imemNsSize` 0x100 |
| 0x1FE28 | 4 | DMEMOffset | 0x200 (from code start) | `fwsec.c:755, 777-781` |
| 0x1FE2C | 4 | DMEMPhysBase | 0 | `fwsec.c:757, 801-810` |
| 0x1FE30 | 4 | DMEMLoadSize | 0x100 | `fwsec.c:756, 783-787, 806-810`. It is also `mappedDataSize` in `frts.c` |
| 0x1FE34 | 4 | altIMEMLoadSize | 0x200 | not used |
| 0x1FE38 | 4 | altDMEMLoadSize | 0x100 | not used |
| 0x1FE3C | 0x100 | non-secure code | zeros | copied only (`fwsec.c:850-851`). Never executed |
| 0x1FF3C | 0x100 | secure code | zeros | same |
| 0x2003C | 0x100 | DMEM image | see the next table | `frts.c:157-261` |
| 0x2013C | 0xC3 | padding | 0 | must not hold 0xFF 0xB8 (`fwsec.c:419-450`) |
| 0x201FF | 1 | image checksum | 0x67 (byte sum of the extension image = 0) | not checked. PCI convention only |

DMEM image. Offsets are from 0x2003C, which is also data-buffer offset 0 because
`dmemPa` = 0:

| DMEM offset | Size | Field | Value | Checked by |
|---|---|---|---|---|
| 0x00 | 1 | interface version | 1 | not checked |
| 0x01 | 1 | headerSize | 4 | ignored (the stride is hard-wired, `frts.c:195`) |
| 0x02 | 1 | entrySize | 8 | ignored (`frts.c:203`) |
| 0x03 | 1 | entryCount | 2 | `frts.c:189-193` (≥ 2) |
| 0x04 | 8 | entry 0 {id, dmemOffset} | {0x1, 0xC0} | fit check only (`frts.c:198-207`). The id is ignored |
| 0x0C | 8 | entry 1 {id, dmemOffset} | {0x4 DMEMMAPPER, 0x20} | `frts.c:212-230`. It is last on purpose, for `frts.c:251` |
| 0x20 | 4 | mapper signature | "DMAP" 0x50414D44 | not checked |
| 0x24 | 2 | mapper version | 3 | not checked |
| 0x26 | 2 | mapper size | 0x40 | not checked |
| 0x28 | 4 | cmd_in_buffer_offset | 0x80 | `frts.c:246-249, 257` |
| 0x2C | 4 | cmd_in_buffer_size | 0x40 (≥ 48) | `frts.c:241-244` (message only) |
| 0x30 | 4 | cmd_out_buffer_offset | 0xC0 | not checked |
| 0x34 | 4 | cmd_out_buffer_size | 0x40 | not checked |
| 0x38..0x47 | 16 | nvf offset/size, printfBufferHdr, build stamp | 0 | not checked |
| 0x48 | 4 | ucode_signature | 0 | not checked |
| 0x4C | 4 | init_cmd | 0 in the image; the kernel writes 0x15 or 0x19 | written at `frts.c:239` |
| 0x50 | 4 | ucode_feature | 0 | not checked |
| 0x54 | 4 | ucode_cmd_mask0 | 0x02200000 (bits 0x15 and 0x19) | not checked |
| 0x58 | 8 | cmd_mask1, multiTgtTbl | 0 | not checked |
| 0x80 | 48 | command input | zeros; the kernel writes `FWSECLIC_FRTS_CMD` or `READ_VBIOS_DESC` here | `frts.c:257-258` |
| 0xA0 | 4 | (after patching) `frtsRegionOffset4K` | `frtsOffset >> 12` | read by the device |
| 0xC0 | 0x40 | command output | zeros | not checked |

Signature blobs: none. V2 descriptors carry no separate signatures that the kernel handles
(`fwsec.c:706-865`). The V3 signature area (`descSize - 44` bytes after the descriptor) does
not apply on TU10x (section c.7).

## f. Limits of a synthetic image

What a real VBIOS provides that this one does not:

- **A real, HS-signed FWSEC.** On hardware the falcon runs it: it carves WPR2, writes FRTS
  data, and reports through scratch 0xE. Here the device has to fake exactly those effects
  (section d). The dummy code is never looked at.
- **Pointers the READ_VBIOS_DESC implies.** FWSEC is told to read the VBIOS itself
  (`gfwImageOffset` 0, flags 2, `frts.c:311-315`). The real FWSEC therefore reads flash
  directly, and the kernel copy does not matter to it.
- **The rest of the extension image(s).** On a real board this includes pre-OS apps, PMU/SEC2
  ucodes, other falcon ucode table entries (FWSEC-DBG, DEVINIT, etc.), and any BIT-referenced
  tables that live beyond 0x1FC00. None of them is read by the kernel RM (section a). The
  x86 image with the full BIT and its devinit/perf/memory tables **is** real, because it
  is our capture.
- **Values GSP-RM reads from the VBIOS.** On hardware, GSP-RM (`gsp_tu10x.bin`) reads the
  VBIOS itself; this is not visible in the open source. It then reports `bVbiosValid`,
  `vbiosSubVendor`, `vbiosSubDevice`, `vbiosRevision` and `vbiosOemRevision` in
  `GspStaticConfigInfo` (`gsp_static_config.h:133-137`). Our device answers that RPC
  itself. To stay consistent with the BIT BIOSDATA it should report revision 0x90063D00,
  OEM 0x76, and the board's subsystem IDs (1458:4011).

Does the kernel RM copy the VBIOS to GSP? **No**:
- The image is freed at `kernel_gsp.c:4931`, right after parsing.
- Only `pFwsecUcode` survives: sysmem code and data buffers, reached by the falcon
  bootloader through the BL descriptor, not by GSP-RM.
- `GspSystemInfo` (`gsp_static_config.h:170-229`) has no VBIOS fields.
- `GspFwWprMeta` holds only the FRTS and VGA-workspace offsets and sizes
  (`nvidia/arch/nvalloc/common/inc/gsp/gsp_fw_wpr_meta.h:125-137`).

The content of the image therefore matters only for the parse in sections b and c, and
for the DMEM layout the device decodes in section d.

## Verifier output

Run on 2026-09-23:

```
$ caraxes/tools/synth-vbios.py --verify caraxes/reference/rtx2070/rom.bin
...
  image @0x0ec00: PCIR @0x0ec1c sig PCIR len 0x88 blocks, code type 0x03, NPDE @0x0ec40 rev 0x100 len 0x10 subimg 0x88, last=False
  image @0x1fc00: PCIR pointer 0xffff -> signature 0xffffffff invalid
DRIVER FAILS: failed to locate expansion ROMs (status 0x25 at vbios.c:318)

$ caraxes/tools/synth-vbios.py caraxes/reference/rtx2070/rom.bin caraxes/reference/rtx2070/synth-vbios.bin
next image expected at 0x1fc00; baseRomSize 0xec00 -> expansionRomOffset 0x11000
captured BIT @0x1b0, FALCON_DATA token 14 @0x3b7 -> FalconUcodeTablePtr 0xed24
extension image @0x1fc00: 3 blocks (0x600 bytes); table ext+0x124, desc ext+0x200 (DescPtr 0xee00), code ext+0x23c, data ext+0x43c
wrote caraxes/reference/rtx2070/synth-vbios.bin: 131584 bytes (0x20200)
  image @0x1fc00: PCIR @0x1fc20 sig NPDS len 0x3 blocks, code type 0xe0, NPDE @0x1fc40 rev 0x101 len 0x10 subimg 0x3, last=True
RESULT biosSize 0x20200 (131584), expansionRomOffset 0x11000
BIT header @0x001b0: version 0x0100 header size 12 token size 6 tokens 17
ucode table @0x1fd24: version 1 header 6 entry size 6 entries 1 (desc v2 size 60)
VBIOS version 90.06.3D.00.76 (kernel_gsp.c:4435-4442)
FWSEC desc: version 2, image offset 0x1fe00, size 60, application id 0x05
ucode (boot with loader): imemSize 0x200 imemNsPa 0x0 imemNsSize 0x100 imemSecPa 0x100 imemSecSize 0x100; dmemPa 0x0 dmemSize 0x100; code buffer 0x200, data buffer 0x100
image offsets: code 0x1fe3c, data 0x2003c
RESULT FRTS command @data+0x80 (cmd_in_buffer_size 0x40, 48 bytes written); frtsRegionOffset4K @data+0xa0 = 0x1ff00
FWSEC-SB patch (teardown) also succeeds
OK: every kernel-side check passes
```

The oracle reproduces the E0 failure on the raw capture (same status, same 0xFFFF pointer
read at 0x1FC18). This is evidence that its chain walk matches the driver. The output
SHA-256 is `d8993695f1eb93d1602f0c96cfe25f42c1b3cc65a273cd8c34d75d361ead7968`, and the first
130048 bytes are identical to `rom.bin`. With `--debug-fuse`, the default image still
passes (application id 0x05). An image built with `--appid 0x85` fails under
`--debug-fuse` with 0x25 at `fwsec.c:692`, as expected.

## Open

These points cannot be established from the 610.57.04 sources:

1. **What the real TU106 extension image looks like.** This covers its ROM signature
   (0xBB77 or 0xAA55), whether it uses "NPDS" or "PCIR", how many 0xE0 images there are,
   and its size. The driver accepts any of the valid signatures, so our choice is free. The
   captured BIT pointer 0xED24 only proves that the real ucode table sits at ext+0x124.
2. **Which descriptor version and application id the real FWSEC has.** V2 is inferred from
   the TU102 execute path (section c.7). Whether the real table uses 0x85/0x45 or 0x05 is
   unknown. We use 0x05 to avoid depending on the fuse.
3. **The real meaning of application-interface ids other than 4**, and the real values of
   the DMEM mapper fields the kernel does not read (signature, version, masks). Ours are
   placeholders.
4. **The value real hardware leaves in `WPR2_ADDR_HI` after FRTS.** The kernel only checks
   that it is non-zero. `(frtsOffset + 1 MiB - 4 KiB) >> 12 << 4`, the address of the last
   4K page, is a plausible choice, but it is not established.
5. **What GSP-RM firmware reads from the VBIOS on hardware.** It is not visible in the
   source. It does not matter while the device answers the GSP RPCs itself.
6. **The x86-64 layout sizes** (48 bytes for `FWSECLIC_FRTS_CMD`, 64 for the DMEM mapper)
   follow from C alignment rules, not from an explicit size in the source. The command
   buffer (0x40) and DMEM (0x100) leave headroom in case the compiler pads differently.
