#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The closing series on driver 610.57.04 before the host moves to 615.71.09 (D19): native,
# v1 and v2 interleaved, N=5, restart per rep on both paths, quiet host (no other VM). v2 =
# main 81054f0 (frozen as /mnt/vmstore/caraxes/vfio-user-nvgpu-closing-610-81054f0) with the
# mapping package, the P21 area, the doorbell token pool and the pinned reactor reader, on
# Cloud Hypervisor 0001-0006;
# vm2 at slot 0 with the 2 GiB profile (caraxes/runs/mapping-2026-09-24/env.sh).
set -uo pipefail
R=/home/silas/git/Caraxes; F=$R/caraxes/runs/closing-610-2026-09-24
cd "$R"
source "$R/caraxes/runs/mapping-2026-09-24/env.sh"
export CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-closing-610-81054f0
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_BAR1_PASSTHROUGH=1 CARAXES_DOORBELL_TOKENS=64
export CARAXES_DOORBELL_POLL=${CARAXES_DOORBELL_POLL:-spin}   # pinned reactor reader (doorbell-poll.md), core chosen automatically
unset CARAXES_DOORBELL_IOEVENTFD
{ cat /proc/driver/nvidia/version | head -1; sha256sum "$CARAXES_SERVER" target/bench/release/vhost-user-nvrm /mnt/vmstore/caraxes/vm1/cloud-hypervisor-0006; git -C "$R" rev-parse HEAD; env | grep -E '^CARAXES_|^LEA_CH' | sort; } > "$F/setup.txt"
pgrep -af 'vfio-user-nvgpu|vhost-user-nvrm' | grep -v pgrep > "$F/other-vms-at-start.txt"
caraxes/tools/bench-hostwatch.sh start "$F"
caraxes/tools/bench-run.sh v2-restart > "$F/v2-up.txt" 2>&1
grep -iE 'bar1 passthrough|doorbell tokens|doorbell poll|host ABI' /mnt/vmstore/caraxes/vm2/gpu.log | cut -c1-300 > "$F/v2-device-start.txt"
caraxes/tools/bench-run.sh stage v2 >> "$F/v2-up.txt" 2>&1
caraxes/tools/bench-v1vm.sh up > "$F/v1-up.txt" 2>&1; caraxes/tools/bench-v1vm.sh modules >> "$F/v1-up.txt" 2>&1
caraxes/tools/bench-run.sh stage v1 >> "$F/v1-up.txt" 2>&1
caraxes/tools/bench-run.sh startup v1 --out "$F"
caraxes/tools/bench-run.sh startup v2 --out "$F"
V1_RESTART=1 V2_RESTART=1 caraxes/tools/bench-run.sh series --targets "native v1 v2" --reps "${REPS:-5}" --out "$F"
caraxes/tools/bench-hostwatch.sh stop "$F"
grep -E 'bar1 passthrough|doorbell tokens' /mnt/vmstore/caraxes/vm2/gpu.log | cut -c1-600 | tail -3 > "$F/v2-device-summary.txt"
journalctl -k --since "-4h" --no-pager -g Xid > "$F/host-xid.txt" 2>/dev/null
python3 caraxes/tools/bench-summary.py "$F/native" "$F/v1" "$F/v2" > "$F/summary.md"
echo "closing series done $(date -Iseconds)"
