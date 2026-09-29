#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Rebuild vk-offscreen's image from a serial log and write it as PNG.

openrender-check.sh prints the PPM gzip-compressed and base64-encoded between the lines
CARAXES-PPM-BEGIN and CARAXES-PPM-END. Usage: extract-ppm.py serial.log out.png
(standard library only)."""
import base64
import gzip
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


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    lines, inside = [], False
    with open(sys.argv[1], "r", errors="replace") as f:
        for line in f:
            line = line.strip().split("\r")[-1]
            if line.endswith("CARAXES-PPM-BEGIN"):
                lines, inside = [], True
            elif line.endswith("CARAXES-PPM-END"):
                inside = False
            elif inside:
                lines.append(line)
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
