#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Per-test results of the cuda gate, the nvml verdict and the host Xid count of every
# phase of this directory, side by side (compare.txt).
cd "$(dirname "$(readlink -f "$0")")" || exit 1
for d in before-610 after-610 before-615 after-615 probe-615; do
    [[ -d $d ]] || continue
    echo "== $d"
    sed -nE 's/.*"verdict":"([a-z]+)","reason":"([^"]*)".*/  nvml: \1 (\2)/p' "$d/gate-nvml.txt"
    for t in "$d"/gate-cuda-*/transcript.log; do
        grep -E '^caraxes-gate:|^  (pass|fail|skip)' "$t" | paste - - |
            sed -E 's/caraxes-gate: /  /; s/\t +/ /' | cut -c1-90
    done
    echo "  host Xid lines: $(grep -c Xid "$d/host-xid.txt")"
done
