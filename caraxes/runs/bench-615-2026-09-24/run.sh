#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The first 615 benchmark series (docs/caraxes/benchmark.md "First 615 series"): the 615
# guest and then the 610 guest on the 615 host (615.71.09), both on vm2 with the 1 GiB
# profile at reservation slot 3, the environment of
# caraxes/runs/cutover-615-2026-09-24/v2-615guest-run.sh. The device is built from the
# checkout this script lives in: the 615 guest gets the v615-feature build
# (CARGO_TARGET_DIR=target/b615), the 610 guest the default build (target/b610); both
# select the 615.71.09 host ABI at run time. The tools run from the same checkout, so bench-run.sh picks the guest's
# libcuda directory by driver (the fix of this run: 615 /usr/lib/x86_64-linux-gnu, 610
# /opt/nvrm/lib). Metrics as the closing series (caraxes/runs/closing-610-2026-09-24/run.sh):
# short-auto rl copy init with 3 reps, ppo (PPO_SECONDS=60) with 2, restart per rep.
# Usage: run.sh 615|610   (results in <part>/ next to this script, never overwritten)
set -uo pipefail
PART=${1:?usage: run.sh 615|610}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
SRC=$(cd "$HERE/../../.." && pwd -P); C=$HERE/$PART
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
export CARAXES_IP=192.168.102.22 CARAXES_TAP=caraxes2 CARAXES_MAC=02:00:00:00:00:22
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
export CARAXES_MIRROR_GR_SHADOW=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=3
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
# A worktree has no sibling Leandro-Test; name the checkout (read only).
export LT=${LT:-/home/silas/git/Leandro-Test} CARAXES_LEANDRO_TEST=${CARAXES_LEANDRO_TEST:-/home/silas/git/Leandro-Test}
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH
case $PART in
    615) export VM=vm2-615 CARAXES_DRIVER=615
         export CARAXES_SERVER=${CARAXES_SERVER:-$SRC/target/b615/release/vfio-user-nvgpu} ;;
    # The default image and driver. A v615 build serves guest ABI 615 unless told otherwise
    # and refuses 610 (vfio-user-nvgpu main.rs --guest-abi); the 610 guest needs the default
    # build of the same checkout (the first attempt, 610-v615build/, got cuInit 999).
    610) export VM=vm2; unset CARAXES_DRIVER CARAXES_GUEST_ABI
         export CARAXES_SERVER=${CARAXES_SERVER:-$SRC/target/b610/release/vfio-user-nvgpu} ;;
    *) echo "ERROR: part is 615 or 610" >&2; exit 2 ;;
esac
W=/mnt/vmstore/caraxes/$VM; T0=$(date '+%F %T')
if pgrep -f /mnt/vmstore/caraxes/vm2 >/dev/null; then
    echo "ERROR: a vm2 process runs; refusing to start" >&2; pgrep -af /mnt/vmstore/caraxes/vm2 >&2; exit 2
fi
mkdir -p "$C"; cd "$SRC" || exit 1
{ head -1 /proc/driver/nvidia/version; sha256sum "$CARAXES_SERVER"; git -C "$SRC" log --oneline -1
  echo "bench-run.sh libdir v2: $(caraxes/tools/bench-run.sh libdir v2)"
  env | grep -E '^CARAXES_|^LEA_CH|^VM=' | sort; } > "$C/setup.txt"
pgrep -af 'vfio-user-nvgpu|vhost-user-nvrm' | grep -v pgrep > "$C/other-vms-at-start.txt"
caraxes/tools/bench-hostwatch.sh start "$C"
caraxes/tools/bench-run.sh v2-restart > "$C/up.txt" 2>&1; tail -2 "$C/up.txt" | cut -c1-200
grep -E 'host ABI|guest ABI|doorbell tokens|doorbell poll|ERROR' "$W/gpu.log" | cut -c1-300 | head -8 > "$C/device-log-first.txt"
tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
caraxes/tools/bench-run.sh stage v2 > "$C/stage.txt" 2>&1
# Which libcuda the bench binaries resolve with the harness's LD_LIBRARY_PATH, and which the
# linker cache gives the gate's probes (run without it).
tools/caraxes-vm.sh ssh "cd ~/bench; echo harness: \$(LD_LIBRARY_PATH=$(caraxes/tools/bench-run.sh libdir v2) ldd ./bench-cu | grep libcuda); echo cache: \$(ldconfig -p | grep 'libcuda.so.1 '); readlink -f \$(ldconfig -p | sed -n 's/.*libcuda.so.1 .*=> //p' | head -1)" > "$C/libcuda.txt" 2>&1
cat "$C/libcuda.txt"
V2_RESTART=1 caraxes/tools/bench-run.sh series --targets v2 --reps 3 --metrics "short-auto rl copy init" --out "$C"
V2_RESTART=1 PPO_SECONDS=60 caraxes/tools/bench-run.sh series --targets v2 --reps 2 --metrics ppo --out "$C"
caraxes/tools/bench-hostwatch.sh stop "$C"
grep -E 'unanswered|ERROR|refused|NOT_SUPPORTED' "$W/gpu.log" | sort | uniq -c | sort -rn | head -12 | cut -c1-250 > "$C/device-log-issues.txt"
tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
python3 caraxes/tools/bench-summary.py "$C/v2" > "$C/summary.md" 2>&1
echo "bench-615 part $PART done $(date -Iseconds); Xid lines $(grep -c Xid "$C/host-xid.txt")"
