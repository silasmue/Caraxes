#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Rebuild vk-offscreen's image from a serial log and write it as PNG.

openrender-check.sh prints the PPM gzip-compressed and base64-encoded between the lines
CARAXES-PPM-BEGIN and CARAXES-PPM-END. Usage: extract-ppm.py serial.log out.png
(standard library only). `extract-ppm.py --png serial.log out.png` writes the PNG printed
between CARAXES-PNG-BEGIN and CARAXES-PNG-END as it is (weston's screenshot, R8);
`extract-ppm.py --ppm file.ppm out.png` converts a PPM copied out of the guest.
Lines inside a block that are not base64 (another console writer) are skipped."""
import base64
import gzip
import re
import struct
import sys
import zlib


def png(width: int, height: int, rgb: bytes) -> bytes:
    def chunk(kind: bytes, data: bytes) -> bytes:
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    rows = b"".join(b"\0" + rgb[y * width * 3:(y + 1) * width * 3] for y in range(height))
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(rows, 9))
            + chunk(b"IEND", b""))


BASE64 = re.compile(r"^[A-Za-z0-9+/]+=*$")


def block(path: str, tag: str) -> list:
    """The base64 lines between CARAXES-<tag>-BEGIN and -END (the last such block)."""
    lines, inside = [], False
    with open(path, "r", errors="replace") as f:
        for line in f:
            line = line.strip().split("\r")[-1]
            if line.endswith(f"CARAXES-{tag}-BEGIN"):
                lines, inside = [], True
            elif line.endswith(f"CARAXES-{tag}-END"):
                inside = False
            elif inside and BASE64.match(line):
                # Another console writer (systemd's status line) can land inside a block;
                # only base64 lines belong to it.
                lines.append(line)
    return lines


def main() -> int:
    if len(sys.argv) == 4 and sys.argv[1] == "--png":
        # A PNG the guest printed as is (openrender-check.sh R8: weston's screenshot).
        lines = block(sys.argv[2], "PNG")
        if not lines:
            print("no CARAXES-PNG block in", sys.argv[2], file=sys.stderr)
            return 1
        with open(sys.argv[3], "wb") as f:
            f.write(base64.b64decode("".join(lines)))
        print(f"{sys.argv[3]}: PNG from {sys.argv[2]}")
        return 0
    if len(sys.argv) == 4 and sys.argv[1] == "--ppm":
        # A PPM file copied out of the guest (openrender-boot.sh results/).
        ppm = open(sys.argv[2], "rb").read()
        sys.argv = [sys.argv[0], sys.argv[2], sys.argv[3]]
    elif len(sys.argv) != 3:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    else:
        lines = block(sys.argv[1], "PPM")
        if not lines:
            print("no CARAXES-PPM block in", sys.argv[1], file=sys.stderr)
            return 1
        ppm = gzip.decompress(base64.b64decode("".join(lines)))
    magic, dims, maxval, rest = ppm.split(b"\n", 3)
    width, height = map(int, dims.split())
    if magic != b"P6" or maxval != b"255" or len(rest) != width * height * 3:
        print("not a 8-bit binary PPM", file=sys.stderr)
        return 1
    with open(sys.argv[2], "wb") as f:
        f.write(png(width, height, rest))
    print(f"{sys.argv[2]}: {width}x{height}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
