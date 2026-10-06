#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
set -euo pipefail
if [[ $# -ne 1 ]]; then
    echo 'usage: build.sh /absolute/output/vk-unit-timing' >&2
    exit 2
fi
out=$1
[[ $out = /* ]] || { echo 'output path must be absolute' >&2; exit 2; }
src=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
mkdir -p -- "$(dirname -- "$out")"
tmp=$(mktemp -d "$(dirname -- "$out")/.vk-unit-build.XXXXXXXX")
trap 'rm -rf -- "$tmp"' EXIT
glslangValidator -V --target-env vulkan1.2 -S comp -o "$tmp/alu.spv" "$src/alu.comp"
glslangValidator -V --target-env vulkan1.2 -S comp -o "$tmp/stream.spv" "$src/stream.comp"
python3 - "$tmp/alu.spv" "$tmp/stream.spv" "$tmp/shaders.inc" <<'PY'
from pathlib import Path
import struct
import sys
out = []
for name, path in (('alu', Path(sys.argv[1])), ('stream', Path(sys.argv[2]))):
    data = path.read_bytes()
    if len(data) % 4 or len(data) < 20:
        raise SystemExit(f'invalid SPIR-V word stream: {path}')
    words = struct.unpack('<' + 'I' * (len(data) // 4), data)
    out.append(f'static const uint32_t {name}_spv[] = {{')
    out.extend('    ' + ', '.join(f'0x{word:08x}u' for word in words[i:i + 8]) + ','
               for i in range(0, len(words), 8))
    out.append('};')
Path(sys.argv[3]).write_text('\n'.join(out) + '\n')
PY
cc -std=c11 -D_POSIX_C_SOURCE=200809L -O2 -Wall -Wextra -Werror \
    -U_FORTIFY_SOURCE -fno-stack-protector -I "$tmp" \
    -o "$out" "$src/vk-unit-timing.c" -lvulkan -lm
sha256sum "$out"
