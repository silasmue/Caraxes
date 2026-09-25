#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Morning matrix, workload part (docs/caraxes/workload-gate.md): caraxes/runs/workload-2026-09-24/run.sh
# with the default metrics and 3 reps on the 615 guest at the 2 GiB profile (workload-env.sh:
# vm2-615, slot 0, the matrix windows), so that llama, transformer and ResNet-18 run like for like
# with native (the night's 1 GiB run could not load the llama model). Target v2 only: the native
# column is the night's (caraxes/runs/workload-2026-09-24/night/gate/native/, same binaries and
# model). Down at the end, the device summary (the pool's page movements), the guest's
# dma-ranges, host Xid lines with the device pid. Usage: workload.sh [label]
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
source "$HERE/workload-env.sh"
C=$HERE/workload-615${1:+-$1}
[[ -e $C ]] && { echo "ERROR: $C exists" >&2; exit 2; }
mx_free || exit 2
T0=$(date '+%F %T')
export WORKLOAD_ENV=$HERE/workload-env.sh
"$MX_ROOT/caraxes/runs/workload-2026-09-24/run.sh" --targets "v2" --out "$C"
rc=$?
"$LEA_TOOLS/caraxes-vm.sh" ssh 'cat /sys/kernel/debug/swiotlb/io_tlb_nslabs 2>/dev/null; sudo dmesg | grep -iE "swiotlb|dma|xid|nvrm.*fault" | tail -30' > "$C/guest-dma.txt" 2>&1
mx_finish "$C" "$T0"
echo "workload part done rc=$rc $(date -Iseconds)"
