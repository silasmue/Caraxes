#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Build vk-offscreen on the host for the Ubuntu 24.04 guest: glslc compiles the two shaders
# to SPIR-V word lists that the C file includes, gcc links against libvulkan.so.1 only, and
# the check below refuses a binary that needs a glibc symbol newer than the guest's 2.39.
# Output: vk-offscreen next to this script (git-ignored build products: *.inc, the binary).
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
glslc -mfmt=c -O -o tri.vert.inc tri.vert
glslc -mfmt=c -O -o tri.frag.inc tri.frag
gcc -std=c11 -D_POSIX_C_SOURCE=200809L -O2 -Wall -Wextra -Werror -U_FORTIFY_SOURCE \
    -fno-stack-protector -o vk-offscreen vk-offscreen.c -lvulkan
newest=$(objdump -T vk-offscreen | grep -oE 'GLIBC_[0-9.]+' | sort -uV | tail -1)
echo "newest glibc symbol version: $newest"
[[ $(printf '%s\n' "$newest" GLIBC_2.39 | sort -V | tail -1) == GLIBC_2.39 ]] ||
    { echo "ERROR: needs $newest, the guest has GLIBC_2.39" >&2; exit 1; }
sha256sum vk-offscreen
