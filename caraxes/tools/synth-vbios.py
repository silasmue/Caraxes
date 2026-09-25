#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Synthetic TU10x VBIOS image for the Caraxes vfio-user device.

The NVIDIA open kernel modules 610.57.04 (GSP-client mode) read the VBIOS
through the BAR0 PROM window (NV_PROM_DATA = 0x300000), walk the PCI ROM image
chain, look for the BIT header, and extract the FWSEC falcon ucode together with
its DMEM "application interface" so they can patch an FRTS command into it.
Nothing from the image ever executes on our device.  This tool appends a
minimal NVIDIA extension image (PCI code type 0xE0) to a sysfs ROM capture
(x86 + EFI images, left byte-for-byte untouched) so that every check in the
driver passes.

Usage:
  synth-vbios.py ROM_IN IMAGE_OUT        generate, then run the oracle on the result
  synth-vbios.py --verify IMAGE          run the oracle only
  synth-vbios.py --ext-sig 0x4E56 --appid 0x85 ROM_IN IMAGE_OUT
                                         the image both OpenRM and nova-core accept
                                         (docs/caraxes/nova-guest.md)
  synth-vbios.py --desc v3 ROM_IN IMAGE_OUT
                                         the costume ladder's Ampere/Ada image
                                         (docs/caraxes/costume-ladder.md): the FWSEC
                                         entry carries a FALCON_UCODE_DESC_V3 (boot from
                                         HS, what kgspExecuteHsFalcon_GA102 runs) instead
                                         of the Turing V2 (boot with loader)

All "file:line" references are relative to
vendor/open-gpu-kernel-modules/src/nvidia/ and describe the 610.57.04 sources:
  vbios.c = src/kernel/gpu/gsp/arch/turing/kernel_gsp_vbios_tu102.c
  fwsec.c = src/kernel/gpu/gsp/kernel_gsp_fwsec.c
  frts.c  = src/kernel/gpu/gsp/arch/turing/kernel_gsp_frts_tu102.c
  pcie.h  = inc/kernel/platform/pci_exp_table.h
The layout is documented in docs/caraxes/vbios-synthesis.md.
"""

import argparse
import struct
import sys

# --------------------------------------------------------------------------
# Constants taken from the driver headers
# --------------------------------------------------------------------------

PCI_EXP_ROM_SIGNATURE = 0xAA55          # pcie.h:31
PCI_EXP_ROM_SIGNATURE_NV = 0x4E56       # pcie.h:32
PCI_EXP_ROM_SIGNATURE_NV2 = 0xBB77      # pcie.h:33
VALID_ROM_SIGS = (PCI_EXP_ROM_SIGNATURE, PCI_EXP_ROM_SIGNATURE_NV, PCI_EXP_ROM_SIGNATURE_NV2)

OFFSETOF_PCI_EXP_ROM_SIG = 0x0          # pcie.h:38
OFFSETOF_PCI_EXP_ROM_PCI_DATA_STRUCT_PTR = 0x18   # pcie.h:40

PCI_DATA_STRUCT_SIGNATURE = 0x52494350      # "PCIR" pcie.h:68
PCI_DATA_STRUCT_SIGNATURE_NV = 0x5344504E   # "NPDS" pcie.h:69
PCI_DATA_STRUCT_SIGNATURE_NV2 = 0x53494752  # "RGIS" pcie.h:70
VALID_PCI_DATA_SIGS = (PCI_DATA_STRUCT_SIGNATURE, PCI_DATA_STRUCT_SIGNATURE_NV,
                       PCI_DATA_STRUCT_SIGNATURE_NV2)

PCI_LAST_IMAGE = 0x80                   # pcie.h:75 NVBIT(7)
PCI_ROM_IMAGE_BLOCK_SIZE = 512          # pcie.h:76

OFFSETOF_PCI_DATA_STRUCT_LEN = 0xA          # pcie.h:80
OFFSETOF_PCI_DATA_STRUCT_CODE_TYPE = 0x14   # pcie.h:82
OFFSETOF_PCI_DATA_STRUCT_IMAGE_LEN = 0x10   # pcie.h:83
OFFSETOF_PCI_DATA_STRUCT_LAST_IMAGE = 0x15  # pcie.h:84

NV_PCI_DATA_EXT_SIG = 0x4544504E        # "NPDE" pcie.h:104
NV_PCI_DATA_EXT_REV_10 = 0x100          # pcie.h:105
NV_PCI_DATA_EXT_REV_11 = 0x101          # pcie.h:106
OFFSETOF_PCI_DATA_EXT_STRUCT_SIG = 0x0          # pcie.h:108
OFFSETOF_PCI_DATA_EXT_STRUCT_LEN = 0x6          # pcie.h:109
OFFSETOF_PCI_DATA_EXT_STRUCT_REV = 0x4          # pcie.h:110
OFFSETOF_PCI_DATA_EXT_STRUCT_SUBIMAGE_LEN = 0x8  # pcie.h:111
OFFSETOF_PCI_DATA_EXT_STRUCT_LAST_IMAGE = 0xA   # pcie.h:112

CODE_TYPE_VBIOS_BASE = 0x00             # vbios.c:46
CODE_TYPE_VBIOS_EXT = 0xE0              # vbios.c:47

BASE_BIOS_MAX_SIZE = 0x100000           # vbios.c:423-429 (1 MiB)

NV_PBUS_IFR_FMT_FIXED0_SIGNATURE_VALUE = 0x4947564E   # "NVGI" dev_bus.h:32
NV_ROM_DIRECTORY_IDENTIFIER = 0x44524652              # "RFRD" vbios.c:44

BIT_HEADER_ID = 0xB8FF                  # fwsec.c:43
BIT_HEADER_SIGNATURE = 0x00544942       # "BIT\0" fwsec.c:44
BIT_HEADER_SIZE_OFFSET = 8              # fwsec.c:45
BIT_TOKEN_BIOSDATA = 0x42               # fwsec.c:75
BIT_TOKEN_FALCON_DATA = 0x70            # fwsec.c:90

FALCON_UCODE_TABLE_HDR_V1_VERSION = 1   # fwsec.c:110
FALCON_UCODE_TABLE_HDR_V1_SIZE_6 = 6    # fwsec.c:111
FALCON_UCODE_TABLE_ENTRY_V1_SIZE_6 = 6  # fwsec.c:122
APPID_FIRMWARE_SEC_LIC = 0x05           # fwsec.c:125
APPID_FWSEC_DBG = 0x45                  # fwsec.c:126
APPID_FWSEC_PROD = 0x85                 # fwsec.c:127

FALCON_UCODE_DESC_V2_SIZE_60 = 60       # fwsec.c:167
FALCON_UCODE_DESC_V3_SIZE_44 = 44       # fwsec.c:187
BCRT30_RSA3K_SIG_SIZE = 384             # fwsec.c:189

APP_IF_ENTRY_ID_DMEMMAPPER = 0x4        # frts.c:78
DMEM_MAPPER_CMD_FRTS = 0x15             # frts.c:101
DMEM_MAPPER_CMD_SB = 0x19               # frts.c:102
SIZEOF_APP_IF_HEADER = 4                # frts.c:64-70 (4 x NvU8)
SIZEOF_APP_IF_ENTRY = 8                 # frts.c:72-76 (2 x NvU32)
SIZEOF_DMEM_MAPPER_V3 = 64              # frts.c:80-99 (u32,u16,u16,14 x u32)
SIZEOF_READ_VBIOS_DESC = 24             # frts.c:104-111 (u32,u32,u64,u32,u32)
SIZEOF_FRTS_CMD = 48                    # frts.c:127-131 (24 + 20, padded to 8 by the u64)
FRTS_REGION_SIZE_1MB_IN_4K = 0x100      # dev_gc6_island_addendum.h:29, frts.c:320

# --------------------------------------------------------------------------
# Synthetic extension image layout (see docs/caraxes/vbios-synthesis.md, 1e)
# --------------------------------------------------------------------------

EXT_PCIR_PTR = 0x20        # PCI data structure right after the 0x1C-byte ROM header
EXT_PCIR_LEN = 0x18        # standard PCI 3.0 data structure length
EXT_NPDE_LEN = 0x10        # covers privLastImage (0xA) and flags (0xB)
EXT_HDR_END = 0x60         # nothing of ours may live below this in the ext image

IMEM_NS_SIZE = 0x100       # dummy non-secure code, one 256-byte falcon block
IMEM_SEC_SIZE = 0x100      # dummy secure code, one 256-byte falcon block
IMEM_LOAD_SIZE = IMEM_NS_SIZE + IMEM_SEC_SIZE
DMEM_LOAD_SIZE = 0x100

# offsets inside the FWSEC DMEM image (the "data" part)
DMEM_IF_HDR = 0x00         # FALCON_APPLICATION_INTERFACE_HEADER_V1
DMEM_MAPPER = 0x20         # FALCON_APPLICATION_INTERFACE_DMEM_MAPPER_V3
DMEM_CMD_IN = 0x80         # command input buffer (FWSECLIC_FRTS_CMD lands here)
DMEM_CMD_IN_SIZE = 0x40
DMEM_CMD_OUT = 0xC0
DMEM_CMD_OUT_SIZE = 0x40
DMEM_MAPPER_SIGNATURE = 0x50414D44   # "DMAP"; not checked by the kernel
# V3 (boot from HS, costume ladder): the RSA3K signature the kernel copies into DMEM at
# PKCDataOffset (frts.c:390-391) needs 384 bytes after the command buffers, so the DMEM
# image grows to 0x300; one signature, for fuse version 0 (the device's
# NV_FUSE_OPT_FPF_GSP_UCODE1_VERSION reads 0, kernel_gsp_ga100.c:59-84, so
# SignatureVersions bit 0 selects it, frts.c:357-372). The signature bytes are zero:
# invented, nothing ever checks them (the device fakes FWSEC's effects).
DMEM_PKC_V3 = 0x100
DMEM_LOAD_SIZE_V3 = 0x300
V3_UCODE_ID = 1            # kgspReadUcodeFuseVersion_GA100 indexes ucodeId - 1
V3_ENGINE_ID_MASK = 0x0400 # written to NV_PFALCON2_FALCON_BROM_ENGIDMASK only; invented

DMEM_MAPPER_VERSION = 3

PROM_ERASED = 0xFF         # what our device returns beyond the image (E0)


class DriverError(Exception):
    """Mirrors a non-NV_OK status in the driver."""

    def __init__(self, status, where, msg):
        super().__init__("%s (status 0x%x at %s)" % (msg, status, where))
        self.status = status


NV_ERR_GENERIC = 0xFFFF
NV_ERR_INVALID_DATA = 0x25
NV_ERR_INVALID_OFFSET = 0x37
NV_ERR_INVALID_STATE = 0x40
NV_ERR_INVALID_ADDRESS = 0x1E
NV_ERR_NOT_SUPPORTED = 0x56


def u8(b, o):
    return b[o]


def u16(b, o):
    return struct.unpack_from("<H", b, o)[0]


def u32(b, o):
    return struct.unpack_from("<I", b, o)[0]


# --------------------------------------------------------------------------
# Oracle, part 1: PROM reads and the image chain (vbios.c)
# --------------------------------------------------------------------------

class Prom:
    """The BAR0 PROM window as our device serves it: bytes of the image,
    0xFF beyond its end (erased flash, as observed in E0)."""

    def __init__(self, data):
        self.data = data

    def read32(self, off):                              # s_promRead32, vbios.c:79-86
        return int.from_bytes(bytes(self.data[off + i] if off + i < len(self.data)
                                    else PROM_ERASED for i in range(4)), "little")

    def read8(self, off):                               # s_promRead08, vbios.c:64-71
        return self.data[off] if off < len(self.data) else PROM_ERASED


def rom_read(prom, base, max_offset, offset, size):
    """s_romImgReadGeneric, vbios.c:99-169: offset is relative to base; the
    sum must stay within max_offset (absolute, 1 MiB); two aligned dword reads."""
    offset += base                                      # vbios.c:129
    if offset > 0xFFFFFFFF:
        raise DriverError(NV_ERR_INVALID_OFFSET, "vbios.c:132", "offset overflow")
    if max_offset > 0 and offset + size > max_offset:   # vbios.c:136-145
        raise DriverError(NV_ERR_INVALID_OFFSET, "vbios.c:142", "read beyond maxOffset")
    byte_index = offset & 3                             # vbios.c:147-150
    aligned = offset - byte_index
    words = prom.read32(aligned).to_bytes(4, "little")
    if byte_index + size > 4:
        words += prom.read32(aligned + 4).to_bytes(4, "little")
    return int.from_bytes(words[byte_index:byte_index + size], "little")


def find_pci_header_ifr(prom):
    """s_romImgFindPciHeader_TU102, vbios.c:202-273 (only reached when the
    first word is not a PCI ROM signature)."""
    fixed0 = rom_read(prom, 0, BASE_BIOS_MAX_SIZE, 0, 4)
    fixed1 = rom_read(prom, 0, BASE_BIOS_MAX_SIZE, 4, 4)
    fixed2 = rom_read(prom, 0, BASE_BIOS_MAX_SIZE, 8, 4)
    image_offset = 0
    if fixed0 == NV_PBUS_IFR_FMT_FIXED0_SIGNATURE_VALUE:
        ver = (fixed1 >> 8) & 0xFF                      # VERSIONSW 15:8
        if ver in (1, 2):
            ext = (fixed1 >> 16) & 0x7FFF               # FIXED_DATA_SIZE 30:16
            image_offset = rom_read(prom, 0, BASE_BIOS_MAX_SIZE, ext + 4, 4)
        elif ver == 3:
            total = fixed2 & 0xFFFFF                    # TOTAL_DATA_SIZE 19:0
            flash_status = rom_read(prom, 0, BASE_BIOS_MAX_SIZE, total, 4)
            rom_dir = flash_status + 4096
            if rom_read(prom, 0, BASE_BIOS_MAX_SIZE, rom_dir, 4) != NV_ROM_DIRECTORY_IDENTIFIER:
                raise DriverError(NV_ERR_INVALID_DATA, "vbios.c:257", "ROM directory not found")
            image_offset = rom_read(prom, 0, BASE_BIOS_MAX_SIZE, rom_dir + 8, 4)
        else:
            raise DriverError(NV_ERR_INVALID_DATA, "vbios.c:264", "IFR version not supported")
    if image_offset & 3:
        raise DriverError(NV_ERR_INVALID_ADDRESS, "vbios.c:269", "IFR image offset unaligned")
    return image_offset


def locate_expansion_roms(prom, pci_offset, max_offset, log):
    """s_locateExpansionRoms, vbios.c:276-416.  Returns (biosSize,
    expansionRomOffset, images)."""
    curr_block = pci_offset
    ext_rom_offset = 0
    base_rom_size = 0
    block_offset = 0
    block_size = 0
    images = []
    while True:                                                         # vbios.c:296
        pci_blck = rom_read(prom, curr_block, max_offset,
                            OFFSETOF_PCI_EXP_ROM_PCI_DATA_STRUCT_PTR, 2)  # vbios.c:308
        pcir = curr_block + pci_blck                                    # vbios.c:311
        sig = rom_read(prom, pcir, max_offset, 0, 4)                    # vbios.c:313
        if sig not in VALID_PCI_DATA_SIGS:                              # vbios.c:316-319
            log("  image @0x%05x: PCIR pointer 0x%04x -> signature 0x%08x invalid"
                % (curr_block, pci_blck, sig))
            raise DriverError(NV_ERR_INVALID_DATA, "vbios.c:318",
                              "failed to locate expansion ROMs")
        last = (rom_read(prom, pcir, max_offset,
                         OFFSETOF_PCI_DATA_STRUCT_LAST_IMAGE, 1) & PCI_LAST_IMAGE) != 0  # vbios.c:321
        img_len = rom_read(prom, pcir, max_offset, OFFSETOF_PCI_DATA_STRUCT_IMAGE_LEN, 2)  # vbios.c:323
        sub_img_len = img_len
        pcir_len = rom_read(prom, pcir, max_offset, OFFSETOF_PCI_DATA_STRUCT_LEN, 2)       # vbios.c:330
        npde = (pcir + pcir_len + 0xF) & ~0xF                                              # vbios.c:331
        npde_note = "no NPDE"
        if rom_read(prom, npde, max_offset, OFFSETOF_PCI_DATA_EXT_STRUCT_SIG, 4) == NV_PCI_DATA_EXT_SIG:  # vbios.c:338-341
            rev = rom_read(prom, npde, max_offset, OFFSETOF_PCI_DATA_EXT_STRUCT_REV, 2)
            if rev in (NV_PCI_DATA_EXT_REV_10, NV_PCI_DATA_EXT_REV_11):                  # vbios.c:346
                ext_len = rom_read(prom, npde, max_offset, OFFSETOF_PCI_DATA_EXT_STRUCT_LEN, 2)
                sub_img_len = rom_read(prom, npde, max_offset,
                                       OFFSETOF_PCI_DATA_EXT_STRUCT_SUBIMAGE_LEN, 2)   # vbios.c:351
                if OFFSETOF_PCI_DATA_EXT_STRUCT_LAST_IMAGE + 1 <= ext_len:             # vbios.c:355
                    last = (rom_read(prom, npde, max_offset,
                                     OFFSETOF_PCI_DATA_EXT_STRUCT_LAST_IMAGE, 1) & PCI_LAST_IMAGE) != 0
                elif sub_img_len < img_len:                                             # vbios.c:360
                    last = False
                npde_note = "NPDE @0x%05x rev 0x%x len 0x%x subimg 0x%x" % (npde, rev, ext_len, sub_img_len)
            else:
                npde_note = "NPDE @0x%05x unknown rev 0x%x (ignored)" % (npde, rev)
        code_type = rom_read(prom, pcir, max_offset, OFFSETOF_PCI_DATA_STRUCT_CODE_TYPE, 1)  # vbios.c:371
        block_offset = curr_block - pci_offset                                  # vbios.c:374
        block_size = sub_img_len * PCI_ROM_IMAGE_BLOCK_SIZE                     # vbios.c:375
        if ext_rom_offset == 0 and code_type == CODE_TYPE_VBIOS_EXT:            # vbios.c:377
            ext_rom_offset = block_offset
        elif base_rom_size == 0 and code_type == CODE_TYPE_VBIOS_BASE:          # vbios.c:381
            base_rom_size = block_size
        images.append((curr_block, pcir, img_len, sub_img_len, code_type, last))
        log("  image @0x%05x: PCIR @0x%05x sig %s len 0x%x blocks, code type 0x%02x, %s, last=%s"
            % (curr_block, pcir, struct.pack("<I", sig).decode("latin1"), img_len,
               code_type, npde_note, last))
        if last:                                                                 # vbios.c:387
            break
        curr_block += sub_img_len * PCI_ROM_IMAGE_BLOCK_SIZE                     # vbios.c:393
    bios_size = block_offset + block_size                                        # vbios.c:400
    if ext_rom_offset > 0 and base_rom_size > 0:                                 # vbios.c:405-412
        expansion_rom_offset = (ext_rom_offset - base_rom_size) & 0xFFFFFFFF
    else:
        expansion_rom_offset = 0
    return bios_size, expansion_rom_offset, images, ext_rom_offset, base_rom_size


def extract_vbios(prom, log):
    """kgspExtractVbiosFromRom_TU102, vbios.c:442-569."""
    max_offset = BASE_BIOS_MAX_SIZE                                  # vbios.c:459, 477
    pci_offset = 0
    sig = rom_read(prom, 0, max_offset, OFFSETOF_PCI_EXP_ROM_SIG, 2)  # vbios.c:492
    if sig not in VALID_ROM_SIGS:                                    # vbios.c:494-503
        pci_offset = find_pci_header_ifr(prom)
        sig = rom_read(prom, pci_offset, max_offset, OFFSETOF_PCI_EXP_ROM_SIG, 2)
    if sig not in VALID_ROM_SIGS:                                    # vbios.c:505-510
        raise DriverError(NV_ERR_INVALID_DATA, "vbios.c:508", "did not find valid ROM signature")
    log("ROM signature 0x%04x at PROM offset 0x%x" % (sig, pci_offset))
    bios_size, exp_off, images, ext_off, base_size = \
        locate_expansion_roms(prom, pci_offset, max_offset, log)     # vbios.c:512
    if bios_size > max_offset:                                       # vbios.c:519-524
        raise DriverError(NV_ERR_INVALID_DATA, "vbios.c:522", "expansion ROM has exceedingly large size")
    # copy loop, vbios.c:540-550 (dword reads, byte reads for the tail)
    image = bytearray()
    for i in range(0, bios_size & ~3, 4):
        image += prom.read32(pci_offset + i).to_bytes(4, "little")
    for i in range(bios_size & ~3, bios_size):
        image.append(prom.read8(pci_offset + i))
    return {"image": bytes(image), "biosSize": bios_size, "expansionRomOffset": exp_off,
            "pciOffset": pci_offset, "images": images, "extRomOffset": ext_off,
            "baseRomSize": base_size}


# --------------------------------------------------------------------------
# Oracle, part 2: BIT and FWSEC descriptor (fwsec.c)
# --------------------------------------------------------------------------

def read_struct(vb, offset, sizes):
    """s_vbiosReadStructure, fwsec.c:339-362: bounds-checked against biosSize,
    little-endian, each field widened to 32 bits."""
    total = sum(sizes)
    if offset + total > len(vb):                     # fwsec.c:355-359
        raise DriverError(NV_ERR_INVALID_OFFSET, "fwsec.c:358", "read beyond biosSize")
    out = []
    for s in sizes:
        out.append(int.from_bytes(vb[offset:offset + s], "little"))
        offset += s
    return out


def find_bit_header(vb):
    """s_vbiosFindBitHeader, fwsec.c:404-455."""
    for addr in range(0, len(vb) - 3):               # fwsec.c:419
        if read_struct(vb, addr, [2])[0] == BIT_HEADER_ID:
            # short-circuit &&: the dword read only happens after an ID match,
            # and an out-of-range read here aborts the whole search (fwsec.c:450)
            if read_struct(vb, addr + 2, [4])[0] == BIT_HEADER_SIGNATURE:
                hsize = read_struct(vb, addr + BIT_HEADER_SIZE_OFFSET, [1])[0]   # fwsec.c:428
                csum = sum(read_struct(vb, addr + j, [1])[0] for j in range(hsize))  # fwsec.c:434-437
                if csum & 0xFF == 0:                  # fwsec.c:441
                    return addr
    raise DriverError(NV_ERR_GENERIC, "fwsec.c:454", "failed to find BIT header")


def parse_fwsec_desc(vb, exp_off, bit_addr, use_debug, log):
    """s_vbiosParseFwsecUcodeDescFromBit, fwsec.c:467-693."""
    hid, hsig, hver, hsize, tsize, tcount, hck = read_struct(vb, bit_addr, [2, 4, 2, 1, 1, 1, 1])  # "1w1d1w4b"
    log("BIT header @0x%05x: version 0x%04x header size %d token size %d tokens %d"
        % (bit_addr, hver, hsize, tsize, tcount))
    if tsize >= 8:                                   # fwsec.c:495-509
        tfmt = [1, 1, 2, 4]                          # "2b1w1d"
    elif tsize >= 6:
        tfmt = [1, 1, 2, 2]                          # "2b2w"
    else:
        raise DriverError(NV_ERR_INVALID_STATE, "fwsec.c:508", "invalid BIT token size")
    version = None
    for tok in range(tcount):                        # fwsec.c:512
        try:
            tid, dver, dsize, dptr = read_struct(vb, bit_addr + hsize + tok * tsize, tfmt)  # fwsec.c:521
        except DriverError:
            continue                                 # fwsec.c:525-531
        if tid == BIT_TOKEN_BIOSDATA and dver in (1, 2) and dsize > 5:   # fwsec.c:534-552
            try:
                ver, oem = read_struct(vb, dptr, [4, 1])
                version = (ver << 8) | oem
            except DriverError:
                pass
        if tid != BIT_TOKEN_FALCON_DATA or dver != 2 or dsize < 4:      # fwsec.c:555-560
            continue
        try:
            table_ptr = read_struct(vb, dptr, [4])[0]                  # fwsec.c:563
        except DriverError:
            continue
        log("BIT token %d: FALCON_DATA v2 @0x%05x -> FalconUcodeTablePtr 0x%x "
            "(+ expansionRomOffset 0x%x = image offset 0x%x)"
            % (tok, dptr, table_ptr, exp_off, exp_off + table_ptr))
        table = exp_off + table_ptr                                    # fwsec.c:575-576
        try:
            tver, thsize, tesize, tecount, tdver, tdsize = read_struct(vb, table, [1] * 6)
        except DriverError:
            continue
        log("ucode table @0x%05x: version %d header %d entry size %d entries %d (desc v%d size %d)"
            % (table, tver, thsize, tesize, tecount, tdver, tdsize))
        if tver != FALCON_UCODE_TABLE_HDR_V1_VERSION or thsize < 6 or tesize < 6:  # fwsec.c:587-592
            continue
        for ent in range(tecount):                                    # fwsec.c:595
            try:
                appid, target, desc_ptr = read_struct(vb, table + thsize + ent * tesize, [1, 1, 4])
            except DriverError:
                continue
            log("  entry %d: application id 0x%02x target 0x%02x DescPtr 0x%x" % (ent, appid, target, desc_ptr))
            if appid != APPID_FIRMWARE_SEC_LIC and (                  # fwsec.c:620-625
                    (use_debug and appid != APPID_FWSEC_DBG) or
                    (not use_debug and appid != APPID_FWSEC_PROD)):
                continue
            try:
                vdesc = read_struct(vb, exp_off + desc_ptr, [4])[0]   # fwsec.c:628-630
            except DriverError:
                continue
            if vdesc & 1 == 0:                                        # fwsec.c:640-646
                continue
            dver_ = (vdesc >> 8) & 0xFF                               # fwsec.c:649-650
            dsize_ = (vdesc >> 16) & 0xFFFF
            if dver_ == 2 and dsize_ >= FALCON_UCODE_DESC_V2_SIZE_60:  # fwsec.c:653-669
                fmt = [4] * 15
            elif dver_ == 3 and dsize_ >= FALCON_UCODE_DESC_V3_SIZE_44:
                fmt = [4] * 9 + [2, 1, 1, 2, 2]
            else:
                log("  entry %d: unexpected desc version 0x%x size 0x%x, skipped" % (ent, dver_, dsize_))
                continue
            desc_off = desc_ptr + exp_off                              # fwsec.c:671
            try:
                fields = read_struct(vb, desc_off, fmt)               # fwsec.c:673
            except DriverError:
                continue
            return {"version": dver_, "offset": desc_off, "size": dsize_,
                    "fields": fields, "vbiosVersion": version, "appid": appid}
    raise DriverError(NV_ERR_INVALID_DATA, "fwsec.c:692", "failed to parse FWSEC ucode desc")


def align_up(v, a):
    return (v + a - 1) & ~(a - 1)


def fill_from_desc_v2(vb, d):
    """s_vbiosFillFlcnUcodeFromDescV2, fwsec.c:706-865 (checks only)."""
    (vdesc, stored, uncompressed, ventry, ifoff, imem_phys, imem_load, imem_virt,
     imem_sec_base, imem_sec_size, dmem_off, dmem_phys, dmem_load, alt_imem, alt_dmem) = d["fields"]
    u = {
        "StoredSize": stored, "UncompressedSize": uncompressed, "VirtualEntry": ventry,
        "InterfaceOffset": ifoff, "IMEMPhysBase": imem_phys, "IMEMLoadSize": imem_load,
        "IMEMVirtBase": imem_virt, "IMEMSecBase": imem_sec_base, "IMEMSecSize": imem_sec_size,
        "DMEMOffset": dmem_off, "DMEMPhysBase": dmem_phys, "DMEMLoadSize": dmem_load,
        "altIMEMLoadSize": alt_imem, "altDMEMLoadSize": alt_dmem,
    }
    imem_size = imem_load                                            # fwsec.c:748
    imem_ns_size = (imem_load - imem_sec_size) & 0xFFFFFFFF          # fwsec.c:749
    imem_ns_pa = imem_phys                                           # fwsec.c:750
    imem_sec_sz = align_up(imem_sec_size, 256)                       # fwsec.c:751
    imem_sec_pa = (imem_sec_base - imem_virt + imem_phys) & 0xFFFFFFFF  # fwsec.c:752
    code_al = align_up(imem_size, 256)                               # fwsec.c:761
    data_al = align_up(dmem_load, 256)                               # fwsec.c:762
    n = len(vb)
    code_off = d["offset"] + d["size"]                               # fwsec.c:765
    if code_off >= n:
        raise DriverError(NV_ERR_INVALID_OFFSET, "fwsec.c:768", "code offset beyond image")
    if code_off + imem_size > n:
        raise DriverError(NV_ERR_INVALID_OFFSET, "fwsec.c:774", "code end beyond image")
    data_off = code_off + dmem_off                                   # fwsec.c:777
    if data_off >= n:
        raise DriverError(NV_ERR_INVALID_OFFSET, "fwsec.c:780", "data offset beyond image")
    if data_off + dmem_load > n:
        raise DriverError(NV_ERR_INVALID_OFFSET, "fwsec.c:786", "data end beyond image")
    if imem_ns_pa >= code_al or imem_ns_pa + imem_size > code_al:     # fwsec.c:790-799
        raise DriverError(NV_ERR_INVALID_OFFSET, "fwsec.c:792/798", "IMEM placement")
    if dmem_phys >= data_al or dmem_phys + dmem_load > data_al:      # fwsec.c:801-810
        raise DriverError(NV_ERR_INVALID_OFFSET, "fwsec.c:803/809", "DMEM placement")
    u.update({"imemSize": imem_size, "imemNsSize": imem_ns_size, "imemNsPa": imem_ns_pa,
              "imemSecSize": imem_sec_sz, "imemSecPa": imem_sec_pa, "codeEntry": ventry,
              "dataOffset": dmem_off, "dmemSize": dmem_load, "dmemPa": dmem_phys,
              "interfaceOffset": ifoff, "codeSizeAligned": code_al, "dataSizeAligned": data_al,
              "imageCodeOffset": code_off, "imageDataOffset": data_off})
    # data buffer as the kernel builds it: zeroed, DMEM image copied at dmemPa (fwsec.c:853-855)
    buf = bytearray(data_al)
    buf[dmem_phys:dmem_phys + dmem_load] = vb[data_off:data_off + dmem_load]
    u["dataBuffer"] = buf
    return u


def patch_interface(buf, mapped_size, interface_offset, cmd, cmd_buf, log):
    """s_vbiosPatchInterfaceData, frts.c:157-261.  Returns the DMEM-mapper and
    command-buffer offsets inside the data buffer."""
    if interface_offset >= mapped_size or interface_offset + SIZEOF_APP_IF_HEADER > mapped_size:  # frts.c:177-186
        raise DriverError(NV_ERR_INVALID_OFFSET, "frts.c:179/185", "interface header")
    ver, hsz, esz, cnt = buf[interface_offset:interface_offset + 4]
    log("app interface @DMEM 0x%x: version %d header %d entry %d count %d" % (interface_offset, ver, hsz, esz, cnt))
    if cnt < 2:                                                          # frts.c:189-193
        raise DriverError(NV_ERR_INVALID_DATA, "frts.c:192", "too few interface entries")
    cur = interface_offset + SIZEOF_APP_IF_HEADER                        # frts.c:195 (headerSize ignored)
    mapper = None
    last_dmem_offset = None
    for i in range(cnt):
        if cur >= mapped_size or cur + SIZEOF_APP_IF_ENTRY > mapped_size:  # frts.c:198-207
            raise DriverError(NV_ERR_INVALID_OFFSET, "frts.c:200/206", "interface entry")
        eid, eoff = struct.unpack_from("<II", buf, cur)
        log("  interface entry %d @DMEM 0x%x: id 0x%x dmemOffset 0x%x" % (i, cur, eid, eoff))
        cur += SIZEOF_APP_IF_ENTRY                                         # frts.c:210 (entrySize ignored)
        last_dmem_offset = eoff
        if eid == APP_IF_ENTRY_ID_DMEMMAPPER:                              # frts.c:212
            if eoff >= mapped_size or eoff + SIZEOF_DMEM_MAPPER_V3 > mapped_size:  # frts.c:216-226
                raise DriverError(NV_ERR_INVALID_OFFSET, "frts.c:218/225", "DMEM mapper")
            mapper = eoff
    if mapper is None:                                                    # frts.c:233-237
        raise DriverError(NV_ERR_INVALID_DATA, "frts.c:236", "no DMEM mapper entry")
    struct.pack_into("<I", buf, mapper + 0x2C, cmd)                       # init_cmd, frts.c:239
    cin_off, cin_size = struct.unpack_from("<II", buf, mapper + 0x08)
    if cin_size < len(cmd_buf):                                           # frts.c:241-244 (message only)
        log("  NOTE: driver would print 'insufficient cmd buffer' (not fatal)")
    if cin_off >= mapped_size:                                            # frts.c:246-249
        raise DriverError(NV_ERR_INVALID_OFFSET, "frts.c:248", "cmd_in_buffer_offset")
    # frts.c:251: bounds check uses the dmemOffset of the LAST entry walked
    if last_dmem_offset + len(cmd_buf) > mapped_size:
        raise DriverError(NV_ERR_INVALID_OFFSET, "frts.c:254", "last entry dmemOffset + cmd size")
    if cin_off + len(cmd_buf) > len(buf):
        raise DriverError(NV_ERR_INVALID_OFFSET, "oracle", "command would overrun the data buffer "
                          "(the driver does not check this, frts.c:257)")
    buf[cin_off:cin_off + len(cmd_buf)] = cmd_buf                         # frts.c:257-258
    return mapper, cin_off, cin_size


def frts_cmd_bytes(frts_offset):
    """FWSECLIC_FRTS_CMD as built by s_prepareForFwsec_TU102, frts.c:311-331."""
    read_vbios = struct.pack("<IIQII", 1, SIZEOF_READ_VBIOS_DESC, 0, 0, 2)          # frts.c:311-315
    region = struct.pack("<IIIII", 1, 20, frts_offset >> 12,
                         FRTS_REGION_SIZE_1MB_IN_4K, 2)                              # frts.c:322-326
    out = read_vbios + region
    return out + bytes(SIZEOF_FRTS_CMD - len(out))                                   # struct padding


def verify(image_bytes, use_debug=False, frts_offset=0x1FF00000, quiet=False):
    lines = []

    def log(s):
        lines.append(s)
        if not quiet:
            print(s)

    prom = Prom(image_bytes)
    log("== chain walk (kgspExtractVbiosFromRom_TU102) on %d bytes ==" % len(image_bytes))
    ex = extract_vbios(prom, log)
    log("extRomOffset 0x%x, baseRomSize 0x%x" % (ex["extRomOffset"], ex["baseRomSize"]))
    log("RESULT biosSize 0x%x (%d), expansionRomOffset 0x%x"
        % (ex["biosSize"], ex["biosSize"], ex["expansionRomOffset"]))
    vb = ex["image"]
    log("== FWSEC parse (kgspParseFwsecUcodeFromVbiosImg), debug fuse -> %s ==" % ("DBG" if use_debug else "PROD"))
    bit = find_bit_header(vb)
    d = parse_fwsec_desc(vb, ex["expansionRomOffset"], bit, use_debug, log)
    if d["vbiosVersion"] is not None:
        v = d["vbiosVersion"]
        log("VBIOS version %2X.%02X.%02X.%02X.%02X (kernel_gsp.c:4435-4442)"
            % ((v >> 32) & 0xFF, (v >> 24) & 0xFF, (v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF))
    log("FWSEC desc: version %d, image offset 0x%x, size %d, application id 0x%02x"
        % (d["version"], d["offset"], d["size"], d["appid"]))
    if d["version"] == 3:
        return verify_v3(vb, d, frts_offset, log)
    u = fill_from_desc_v2(vb, d)
    for k in ("StoredSize", "UncompressedSize", "VirtualEntry", "InterfaceOffset", "IMEMPhysBase",
              "IMEMLoadSize", "IMEMVirtBase", "IMEMSecBase", "IMEMSecSize", "DMEMOffset",
              "DMEMPhysBase", "DMEMLoadSize", "altIMEMLoadSize", "altDMEMLoadSize"):
        log("  desc.%-17s 0x%x" % (k, u[k]))
    log("ucode (boot with loader): imemSize 0x%x imemNsPa 0x%x imemNsSize 0x%x imemSecPa 0x%x "
        "imemSecSize 0x%x; dmemPa 0x%x dmemSize 0x%x; code buffer 0x%x, data buffer 0x%x"
        % (u["imemSize"], u["imemNsPa"], u["imemNsSize"], u["imemSecPa"], u["imemSecSize"],
           u["dmemPa"], u["dmemSize"], u["codeSizeAligned"], u["dataSizeAligned"]))
    log("image offsets: code 0x%x, data 0x%x" % (u["imageCodeOffset"], u["imageDataOffset"]))
    log("== FRTS command (s_prepareForFwsec_TU102), sample frtsOffset 0x%x ==" % frts_offset)
    buf = u["dataBuffer"]
    cmd = frts_cmd_bytes(frts_offset)
    mapper, cin, cin_size = patch_interface(buf, u["dmemSize"], u["interfaceOffset"],
                                            DMEM_MAPPER_CMD_FRTS, cmd, log)
    sig, mver, msize = struct.unpack_from("<IHH", buf, mapper)
    log("DMEM mapper @data+0x%x: signature 0x%08x version %d size %d init_cmd 0x%x"
        % (mapper, sig, mver, msize, u32(buf, mapper + 0x2C)))
    log("RESULT FRTS command @data+0x%x (cmd_in_buffer_size 0x%x, %d bytes written); "
        "frtsRegionOffset4K @data+0x%x = 0x%x"
        % (cin, cin_size, len(cmd), cin + 0x20, u32(buf, cin + 0x20)))
    log("device answer after FWSEC halts: NV_PBUS_VBIOS_SCRATCH(0xE) 0x1438 bits 31:16 = 0; "
        "WPR2_ADDR_LO 0x1FA824 = 0x%08x; WPR2_ADDR_HI 0x1FA828 VAL != 0"
        % ((frts_offset >> 12) << 4))
    # FWSEC-SB at unload patches a READ_VBIOS_DESC through the same mapper
    sb_buf = fill_from_desc_v2(vb, d)["dataBuffer"]
    patch_interface(sb_buf, u["dmemSize"], u["interfaceOffset"], DMEM_MAPPER_CMD_SB,
                    frts_cmd_bytes(frts_offset)[:SIZEOF_READ_VBIOS_DESC], lambda s: None)
    log("FWSEC-SB patch (teardown) also succeeds")
    log("OK: every kernel-side check passes")
    return ex, d, u


def verify_v3(vb, d, frts_offset, log):
    """The GA102-family path: s_vbiosFillFlcnUcodeFromDescV3 (fwsec.c:866-1000) and the
    BOOT_FROM_HS branch of s_prepareForFwsec_TU102 (frts.c:341-395)."""
    (vdesc, stored, pkc, ifoff, imem_phys, imem_load, imem_virt, dmem_phys, dmem_load,
     engmask, ucode_id, sig_count, sig_versions, _res) = d["fields"]
    size = align_up(stored, 256)                                     # fwsec.c:909
    data_off = imem_load                                             # fwsec.c:914
    image_off = d["offset"] + d["size"]                              # fwsec.c:929
    n = len(vb)
    if image_off >= n or image_off + size > n:                       # fwsec.c:930-941
        raise DriverError(NV_ERR_INVALID_OFFSET, "fwsec.c:932/940", "image beyond biosSize")
    if imem_load > size or data_off >= size or data_off + dmem_load > size:  # fwsec.c:944-959
        raise DriverError(NV_ERR_INVALID_OFFSET, "fwsec.c:946-958", "IMEM/DMEM placement")
    if data_off + pkc >= size or data_off + pkc + BCRT30_RSA3K_SIG_SIZE > size:  # fwsec.c:962-973
        raise DriverError(NV_ERR_INVALID_OFFSET, "fwsec.c:964/972", "signature placement")
    sig_total = d["size"] - FALCON_UCODE_DESC_V3_SIZE_44            # fwsec.c:981
    log("V3 desc: StoredSize 0x%x PKCDataOffset 0x%x InterfaceOffset 0x%x IMEM 0x%x@0x%x "
        "DMEM 0x%x@0x%x EngineIdMask 0x%x UcodeId %d SignatureCount %d SignatureVersions 0x%x; "
        "signatures %d bytes" % (stored, pkc, ifoff, imem_load, imem_phys, dmem_load, dmem_phys,
                                 engmask, ucode_id, sig_count, sig_versions, sig_total))
    fuse = 0                                                         # the device reads 0
    want = 1 << fuse                                                 # frts.c:359
    if want & sig_versions == 0:                                     # frts.c:362-365
        raise DriverError(NV_ERR_NOT_SUPPORTED, "frts.c:364", "no signature for fuse version 0")
    if sig_total < BCRT30_RSA3K_SIG_SIZE:                            # frts.c:374-383
        raise DriverError(NV_ERR_INVALID_OFFSET, "frts.c:376", "signature beyond the desc")
    buf = bytearray(vb[image_off + data_off:image_off + data_off + dmem_load])
    cmd = frts_cmd_bytes(frts_offset)
    mapper, cin, cin_size = patch_interface(buf, dmem_load, ifoff, DMEM_MAPPER_CMD_FRTS, cmd, log)
    log("RESULT FRTS command @data+0x%x; frtsRegionOffset4K @data+0x%x = 0x%x"
        % (cin, cin + 0x20, u32(buf, cin + 0x20)))
    log("OK: every kernel-side check of the GA102-family FWSEC path passes")
    return None, d, {"dataOffset": data_off, "dmemSize": dmem_load}


# --------------------------------------------------------------------------
# Generator
# --------------------------------------------------------------------------

def build_ext_image(table_rel, appid, device_id, desc_version=2, ext_sig=PCI_EXP_ROM_SIGNATURE_NV2):
    """Build the NVIDIA extension image.  table_rel is where the unchanged BIT
    FALCON_DATA pointer lands inside this image."""
    if table_rel < EXT_HDR_END:
        raise SystemExit("ucode table would land at ext+0x%x, inside the ROM/PCIR/NPDE headers" % table_rel)
    table_len = 6 + 6                                       # header + one entry
    desc_rel = align_up(table_rel + table_len, 0x100)
    v3 = desc_version == 3
    desc_size = FALCON_UCODE_DESC_V3_SIZE_44 + BCRT30_RSA3K_SIG_SIZE if v3 else FALCON_UCODE_DESC_V2_SIZE_60
    dmem_size = DMEM_LOAD_SIZE_V3 if v3 else DMEM_LOAD_SIZE
    code_rel = desc_rel + desc_size                         # fwsec.c:765/929: code follows desc
    data_rel = code_rel + IMEM_LOAD_SIZE                    # DMEMOffset = IMEMLoadSize
    end_rel = data_rel + dmem_size
    size = align_up(end_rel + 1, PCI_ROM_IMAGE_BLOCK_SIZE)  # +1 keeps a byte for the checksum
    blocks = size // PCI_ROM_IMAGE_BLOCK_SIZE
    img = bytearray(size)

    # ROM header: signature and PCI data structure pointer (pcie.h:43-49)
    struct.pack_into("<H", img, 0x00, ext_sig)
    struct.pack_into("<H", img, OFFSETOF_PCI_EXP_ROM_PCI_DATA_STRUCT_PTR, EXT_PCIR_PTR)
    # PCI data structure (pcie.h:87-101)
    struct.pack_into("<IHHHHB3sHHBBH", img, EXT_PCIR_PTR,
                     PCI_DATA_STRUCT_SIGNATURE_NV, 0x10DE, device_id, 0, EXT_PCIR_LEN, 0,
                     bytes([0x00, 0x00, 0x03]), blocks, 0, CODE_TYPE_VBIOS_EXT, PCI_LAST_IMAGE, 0)
    # NPDE (pcie.h:118-126) at the next 16-byte boundary after the PCIR (vbios.c:331)
    npde = (EXT_PCIR_PTR + EXT_PCIR_LEN + 0xF) & ~0xF
    struct.pack_into("<IHHHBB", img, npde, NV_PCI_DATA_EXT_SIG, NV_PCI_DATA_EXT_REV_11,
                     EXT_NPDE_LEN, blocks, PCI_LAST_IMAGE, 0)
    # Falcon ucode table header + one entry (fwsec.c:100-123)
    struct.pack_into("<6B", img, table_rel, FALCON_UCODE_TABLE_HDR_V1_VERSION, 6, 6, 1,
                     desc_version, FALCON_UCODE_DESC_V3_SIZE_44 if v3 else FALCON_UCODE_DESC_V2_SIZE_60)
    # DescPtr is relative to expansionRomOffset like the table pointer; the
    # caller converts (desc_rel) into that space.
    img_meta = {"table_rel": table_rel, "desc_rel": desc_rel, "code_rel": code_rel,
                "data_rel": data_rel, "end_rel": end_rel, "size": size, "blocks": blocks,
                "npde_rel": npde}
    if v3:
        # FALCON_UCODE_DESC_V3 (fwsec.c:170-185) plus one zero RSA3K signature
        vdesc = 0x1 | (3 << 8) | (desc_size << 16)                  # fwsec.c:129-140
        struct.pack_into("<9IHBBHH", img, desc_rel,
                         vdesc,
                         IMEM_LOAD_SIZE + dmem_size,         # StoredSize -> ucode size
                         DMEM_PKC_V3,                        # PKCDataOffset (DMEM-relative)
                         DMEM_IF_HDR,                        # InterfaceOffset (DMEM-relative)
                         0,                                  # IMEMPhysBase
                         IMEM_LOAD_SIZE,                     # IMEMLoadSize
                         0,                                  # IMEMVirtBase -> BOOTVEC
                         0,                                  # DMEMPhysBase
                         dmem_size,                          # DMEMLoadSize
                         V3_ENGINE_ID_MASK, V3_UCODE_ID,
                         1,                                  # SignatureCount
                         0x1,                                # SignatureVersions: fuse version 0
                         0)
    else:
        # FALCON_UCODE_DESC_V2 (fwsec.c:148-165)
        vdesc = 0x1 | (2 << 8) | (FALCON_UCODE_DESC_V2_SIZE_60 << 16)   # fwsec.c:129-140
        struct.pack_into("<15I", img, desc_rel,
                         vdesc,
                         IMEM_LOAD_SIZE + DMEM_LOAD_SIZE,    # StoredSize (unchecked for V2)
                         IMEM_LOAD_SIZE + DMEM_LOAD_SIZE,    # UncompressedSize (unchecked)
                         0,                                  # VirtualEntry -> codeEntry (unused by loader)
                         DMEM_IF_HDR,                        # InterfaceOffset (DMEM-relative)
                         0,                                  # IMEMPhysBase -> imemNsPa
                         IMEM_LOAD_SIZE,                     # IMEMLoadSize
                         0,                                  # IMEMVirtBase
                         IMEM_NS_SIZE,                       # IMEMSecBase -> imemSecPa = 0x100
                         IMEM_SEC_SIZE,                      # IMEMSecSize
                         IMEM_LOAD_SIZE,                     # DMEMOffset (from code start)
                         0,                                  # DMEMPhysBase -> dmemPa
                         DMEM_LOAD_SIZE,                     # DMEMLoadSize
                         IMEM_LOAD_SIZE,                     # altIMEMLoadSize (unused)
                         DMEM_LOAD_SIZE)                     # altDMEMLoadSize (unused)
    # Dummy code: zeros (never executed; the device fakes FWSEC's effects).
    # DMEM image: application interface, DMEM mapper, command buffers (frts.c:64-99)
    d = data_rel
    struct.pack_into("<4B", img, d + DMEM_IF_HDR, 1, SIZEOF_APP_IF_HEADER, SIZEOF_APP_IF_ENTRY, 2)
    # entry 0: a non-mapper id the kernel skips; entry 1 (last): the DMEM mapper,
    # last on purpose because of the frts.c:251 check on the last entry
    struct.pack_into("<II", img, d + DMEM_IF_HDR + 4, 0x1, DMEM_CMD_OUT)
    struct.pack_into("<II", img, d + DMEM_IF_HDR + 12, APP_IF_ENTRY_ID_DMEMMAPPER, DMEM_MAPPER)
    struct.pack_into("<IHH14I", img, d + DMEM_MAPPER,
                     DMEM_MAPPER_SIGNATURE, DMEM_MAPPER_VERSION, SIZEOF_DMEM_MAPPER_V3,
                     DMEM_CMD_IN, DMEM_CMD_IN_SIZE, DMEM_CMD_OUT, DMEM_CMD_OUT_SIZE,
                     0, 0,          # nvf_img_data_buffer offset/size
                     0,             # printfBufferHdr
                     0,             # ucode_build_time_stamp
                     0,             # ucode_signature
                     0,             # init_cmd (the kernel writes 0x15 or 0x19)
                     0,             # ucode_feature
                     (1 << DMEM_MAPPER_CMD_FRTS) | (1 << DMEM_MAPPER_CMD_SB),  # ucode_cmd_mask0
                     0,             # ucode_cmd_mask1
                     0)             # multiTgtTbl
    # the image checksum byte is set by generate() once the table entry is in
    return img, img_meta, appid


def generate(rom_in, out_path, appid, desc_version=2, ext_sig=PCI_EXP_ROM_SIGNATURE_NV2):
    rom = open(rom_in, "rb").read()
    print("== input %s: %d bytes ==" % (rom_in, len(rom)))
    if u16(rom, 0) not in VALID_ROM_SIGS:
        raise SystemExit("input does not start with a PCI ROM signature")
    # Walk the captured chain the way the driver does, but stop where the
    # capture ends: that is where the extension image must go.
    off, base_size, prev_last_pci = 0, 0, None
    while True:
        pcir = off + u16(rom, off + 0x18)
        if u32(rom, pcir) not in VALID_PCI_DATA_SIGS:
            raise SystemExit("bad PCIR at 0x%x" % pcir)
        img_len = u16(rom, pcir + 0x10)
        code = rom[pcir + 0x14]
        last = bool(rom[pcir + 0x15] & PCI_LAST_IMAGE)
        sub = img_len
        npde = (pcir + u16(rom, pcir + 0xA) + 0xF) & ~0xF
        if u32(rom, npde) == NV_PCI_DATA_EXT_SIG and u16(rom, npde + 4) in (0x100, 0x101):
            sub = u16(rom, npde + 8)
            if u16(rom, npde + 6) >= 0xB:
                last = bool(rom[npde + 0xA] & PCI_LAST_IMAGE)
            elif sub < img_len:
                last = False
        if code == CODE_TYPE_VBIOS_BASE and base_size == 0:
            base_size = sub * PCI_ROM_IMAGE_BLOCK_SIZE
        if code == CODE_TYPE_VBIOS_EXT:
            raise SystemExit("input already contains an extension image at 0x%x" % off)
        if last:
            raise SystemExit("input chain ends at 0x%x without an extension image; "
                             "this tool expects the NPDE of the last captured image to say 'not last'" % off)
        off += sub * PCI_ROM_IMAGE_BLOCK_SIZE
        if off >= len(rom):
            break
    ext_off = off
    if ext_off != len(rom):
        raise SystemExit("capture length 0x%x does not end at the next image 0x%x" % (len(rom), ext_off))
    exp_off = ext_off - base_size                                   # vbios.c:407
    print("next image expected at 0x%x; baseRomSize 0x%x -> expansionRomOffset 0x%x"
          % (ext_off, base_size, exp_off))
    # Keep the captured BIT: its FALCON_DATA pointer decides where the table goes.
    bit = find_bit_header(rom)
    hsize, tsize, tcount = rom[bit + 8], rom[bit + 9], rom[bit + 10]
    table_ptr = None
    for t in range(tcount):
        o = bit + hsize + t * tsize
        tid, dver, dsize = rom[o], rom[o + 1], u16(rom, o + 2)
        dptr = u32(rom, o + 4) if tsize >= 8 else u16(rom, o + 4)
        if tid == BIT_TOKEN_FALCON_DATA and dver == 2 and dsize >= 4:
            table_ptr = u32(rom, dptr)
            print("captured BIT @0x%x, FALCON_DATA token %d @0x%x -> FalconUcodeTablePtr 0x%x"
                  % (bit, t, dptr, table_ptr))
            break
    if table_ptr is None:
        raise SystemExit("captured BIT has no FALCON_DATA v2 token")
    table_abs = exp_off + table_ptr
    table_rel = table_abs - ext_off
    device_id = u16(rom, u16(rom, 0x18) + 6)
    ext, meta, appid = build_ext_image(table_rel, appid, device_id, desc_version, ext_sig)
    # entry: DescPtr is relative to expansionRomOffset (fwsec.c:629, 671)
    desc_ptr = ext_off + meta["desc_rel"] - exp_off
    struct.pack_into("<BBI", ext, table_rel + 6, appid, 0x00, desc_ptr)
    ext[-1] = (-sum(ext[:-1])) & 0xFF      # PCI image checksum; the driver does not check it
    out = rom + bytes(ext)
    with open(out_path, "wb") as f:
        f.write(out)
    print("extension image @0x%x: %d blocks (0x%x bytes); table ext+0x%x, desc ext+0x%x "
          "(DescPtr 0x%x), code ext+0x%x, data ext+0x%x"
          % (ext_off, meta["blocks"], meta["size"], table_rel, meta["desc_rel"], desc_ptr,
             meta["code_rel"], meta["data_rel"]))
    print("wrote %s: %d bytes (0x%x)" % (out_path, len(out), len(out)))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("paths", nargs="+", help="ROM_IN IMAGE_OUT, or IMAGE with --verify")
    ap.add_argument("--verify", action="store_true", help="run the driver oracle on IMAGE only")
    ap.add_argument("--appid", type=lambda s: int(s, 0), default=APPID_FIRMWARE_SEC_LIC,
                    help="FWSEC application id: 0x05 (default, any fuse), 0x85 (prod), 0x45 (debug)")
    ap.add_argument("--debug-fuse", action="store_true",
                    help="oracle: pretend NV_FUSE_OPT_SECURE_GSP_DEBUG_DIS (0x2174C) bit 0 = 0")
    ap.add_argument("--desc", choices=["v2", "v3"], default="v2",
                    help="FWSEC descriptor: v2 (Turing, boot with loader; default) or v3 (GA102/AD10x, "
                         "boot from HS; the costume ladder)")
    ap.add_argument("--ext-sig", type=lambda s: int(s, 0), default=PCI_EXP_ROM_SIGNATURE_NV2,
                    choices=VALID_ROM_SIGS,
                    help="ROM signature of the extension image: 0xBB77 (default, as the card), "
                         "0x4E56 or 0xAA55; nova-core accepts only 0xAA55 and 0x4E56 (Linux 7.2.7 "
                         "drivers/gpu/nova-core/vbios.rs PciRomHeader::new), the nova image "
                         "(docs/caraxes/nova-guest.md) is --ext-sig 0x4E56 --appid 0x85")
    ap.add_argument("--frts-offset", type=lambda s: int(s, 0), default=0x1FF00000,
                    help="oracle: sample frtsOffset for the FRTS command (default 0x1FF00000)")
    a = ap.parse_args()
    try:
        if a.verify:
            if len(a.paths) != 1:
                ap.error("--verify takes one IMAGE")
            data = open(a.paths[0], "rb").read()
            verify(data, a.debug_fuse, a.frts_offset)
        else:
            if len(a.paths) != 2:
                ap.error("generation takes ROM_IN IMAGE_OUT")
            out = generate(a.paths[0], a.paths[1], a.appid, 3 if a.desc == "v3" else 2, a.ext_sig)
            print()
            verify(out, a.debug_fuse, a.frts_offset)
    except DriverError as e:
        print("DRIVER FAILS: %s" % e)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
