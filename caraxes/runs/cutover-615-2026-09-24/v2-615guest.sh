#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# cutover-615.md step (d): the 615 guest on the 615 host. State folder vm2-615 (an overlay on
# the 615 base image with its own cloud-init seed, vm2's tap, IP and MAC), CARAXES_DRIVER=615
# (guest ABI 615, the v615-feature binary from target/v615), token pool and hybrid reactor,
# no BAR1 area. nvml gate, cuda gate, init and short kernel.
set -uo pipefail
R=/home/silas/git/Caraxes; C=$R/caraxes/runs/cutover-615-2026-09-24/v2-615guest; mkdir -p "$C"; cd "$R"
export VM=vm2-615 CARAXES_IP=192.168.102.22 CARAXES_TAP=caraxes2 CARAXES_MAC=02:00:00:00:00:22
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
export CARAXES_MIRROR_GR_SHADOW=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
export CARAXES_FB_MIB=2048 CARAXES_GATE_FB_MIB=2048 CARAXES_RESERVE_SLOT=0
export CARAXES_DRIVER=615 CARAXES_SERVER=$R/target/v615/release/vfio-user-nvgpu LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH
W=/mnt/vmstore/caraxes/vm2-615; T0=$(date '+%F %T')
{ cat /proc/driver/nvidia/version | head -1; sha256sum "$CARAXES_SERVER"; env | grep -E '^CARAXES_|^LEA_CH' | sort; } > "$C/setup.txt"
caraxes/tools/bench-hostwatch.sh start "$C"
tools/caraxes-vm.sh up > "$C/up.txt" 2>&1 || { tail -5 "$C/up.txt"; }
tools/caraxes-vm.sh wait >> "$C/up.txt" 2>&1
tools/caraxes-vm.sh probe cut615 >> "$C/up.txt" 2>&1; tail -3 "$C/up.txt" | cut -c1-200
grep -E 'host ABI|guest ABI|msgq|GMCAPI|transport|GSS legacy|ERROR' "$W/gpu.log" | cut -c1-300 | head -8 > "$C/device-log-first.txt"; head -4 "$C/device-log-first.txt" | cut -c1-260
tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
tools/caraxes-gate.sh nvml > "$C/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?"; tail -3 "$C/gate-nvml.txt" | cut -c1-200
tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -3 "$C/gate-cuda.txt" | cut -c1-200
caraxes/tools/bench-run.sh stage v2 > "$C/stage.txt" 2>&1
caraxes/tools/bench-run.sh series --targets v2 --reps 2 --metrics "init short-auto" --out "$C" 2>&1 | tail -2
caraxes/tools/bench-hostwatch.sh stop "$C"
grep -E 'unanswered|ERROR|refused|NOT_SUPPORTED' "$W/gpu.log" | sort | uniq -c | sort -rn | head -12 | cut -c1-250 > "$C/device-log-issues.txt"
tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
echo "615 guest on 615 host done $(date -Iseconds); Xid $(grep -c Xid "$C/host-xid.txt")"
