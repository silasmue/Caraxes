#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
#
# Build and run caraxes/tools/host-exec.rs (docs/caraxes/host-exec.md): the host-side
# execution primitive that proves the physical RTX 2070 walks a GMMU v2 page table built
# by "the guest" in "guest" VRAM. It touches the host GPU only through the RM API and
# frees every allocation on exit (SIGINT/SIGTERM too).
#
#   caraxes/tools/host-exec.sh [--max-stage A|B|C|D|E|F] [OUTDIR]
#
#   --max-stage  highest stage to run (default D: the CE copy proof).
#   OUTDIR       evidence directory (default caraxes/runs/host-exec-2026-09-23).
#
# The script captures the host journal (Xid) and nvidia-smi memory before and after the
# run, builds the tool with rustc alone (like host-facts.sh style), and runs it under
# `sudo -n`. CAP_SYS_ADMIN is dropped to the invoking user inside the tool and raised only
# around the two privileged controls (D12).
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)
src="$here/host-exec.rs"
bin="$root/target/host-exec"

max=D
out="$root/caraxes/runs/host-exec-2026-09-23"
while [[ $# -gt 0 ]]; do
    case $1 in
        --max-stage) max=$2; shift 2 ;;
        -h|--help) sed -n '6,20p' "$0"; exit 0 ;;
        *) out=$1; shift ;;
    esac
done
mkdir -p "$out"

echo "== building host-exec (rustc, uid $(id -u)) =="
rustc -O --edition 2021 "$src" -o "$bin"

hdr='<!-- SPDX-License-Identifier: MIT -->'
smi() { nvidia-smi -q -d MEMORY | sed -n '/GPU 0000/,/BAR1 Memory Usage/p'; }
pins() { grep nr_foll_pin /proc/vmstat; }

{
    echo "$hdr"
    echo "host-exec host state BEFORE, $(date -Is)"
    smi; pins
} >"$out/host-before.txt"

echo "== running host-exec run --go --max-stage $max (sudo -n) =="
start=$(date +%s)
# shellcheck disable=SC2024
sudo -n "$bin" run --go --max-stage "$max" 2>&1 | tee "$out/run.txt"

{
    echo "$hdr"
    echo "host-exec host state AFTER, $(date -Is)"
    smi; pins
    echo "--- kernel Xid / NVRM since run start ---"
    sudo -n journalctl -k --since "@$start" 2>/dev/null | grep -iE 'xid|nvrm' || echo "(none, or journal not readable)"
} >"$out/host-after.txt"

# Add the SPDX header to the run log for the licence check.
sed -i "1i $hdr" "$out/run.txt" 2>/dev/null || true
echo "== evidence in $out =="
