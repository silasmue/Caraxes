#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Cut the EFI (GOP) image out of a PCI option ROM so the expansion ROM BAR can stay on for the
Windows guest without the firmware running the GOP driver (which spins on NV_PTIMER_TIME in pure
emulation). Keeps the x86 legacy image, marks it as the last image, and pads to the original size.
Usage: make-rom-legacy.py <rom.bin> <out.bin>"""
import struct, sys
src, dst = sys.argv[1], sys.argv[2]
rom = bytearray(open(src, "rb").read())
off, kept, images = 0, None, []
while off + 0x1a <= len(rom) and rom[off] == 0x55 and rom[off + 1] == 0xAA:
    pcir = off + struct.unpack_from("<H", rom, off + 0x18)[0]
    assert rom[pcir:pcir + 4] == b"PCIR", f"no PCIR at {pcir:#x}"
    length = struct.unpack_from("<H", rom, pcir + 0x10)[0] * 512
    code_type, indicator = rom[pcir + 0x14], rom[pcir + 0x15]
    images.append((off, length, code_type, indicator))
    if code_type == 0 and kept is None:
        kept = (off, length, pcir)
    if indicator & 0x80:
        break
    off += length
print("images:", [(hex(o), hex(l), t, hex(i)) for o, l, t, i in images])
assert kept, "no x86 legacy image"
o, l, pcir = kept
out = bytearray(rom[o:o + l])
out[pcir - o + 0x15] |= 0x80          # last image
out += b"\xff" * (len(rom) - len(out))  # keep the size the device expects
open(dst, "wb").write(out)
print(f"kept legacy image {o:#x}+{l:#x}, wrote {len(out)} bytes to {dst}")
