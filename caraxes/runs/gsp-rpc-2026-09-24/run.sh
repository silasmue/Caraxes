#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The GSP-plan runs of 2026-09-24 (README.md next to this script). The tools run from the
# main checkout R; the device binaries are this worktree's (target/rpc for the 610 guest
# ABI, target/rpc615 for 615, built with CARAXES_TARGET_DIR as tools/check.sh builds).
# Usage: run.sh emu615|gate610|gate615 [label]
#   emu615:  vm3-615, pure emulation (boot model and host RM for forwarded controls, no
#            reservation, no host execution): the 615 boot to init done, ALLOC_MEMORY.
#   gate610: vm3 (610 guest), the host-execution configuration of
#            caraxes/runs/doorbell-2026-09-24/env-vm1.sh at a 1 GiB profile in reservation
#            slot 5, token pool and hybrid reactor, no BAR1 area; nvml and cuda gates.
#   gate615: vm3-615 (615 guest), the same configuration with the 615 device.
set -uo pipefail
MODE=${1:?usage: run.sh emu615|gate610|gate615 [label]}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
SRC=$(cd "$HERE/../../.." && pwd -P)
R=/home/silas/git/Caraxes
C=$HERE/$MODE${2:+-$2}
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
if pgrep -f /mnt/vmstore/caraxes/vm3 >/dev/null; then
    echo "ERROR: a vm3 process runs; refusing to start" >&2; pgrep -af /mnt/vmstore/caraxes/vm3 >&2; exit 2
fi
mkdir -p "$C"; cd "$R" || exit 1
export CARAXES_IP=192.168.103.23 CARAXES_TAP=caraxes3 CARAXES_MAC=02:00:00:00:00:23
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_GATE_RUNS=$C
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH CARAXES_DISPLAY
case $MODE in
    emu615)
        export VM=vm3-615 CARAXES_DRIVER=615 CARAXES_SERVER=$SRC/target/rpc615/release/vfio-user-nvgpu ;;
    gate610|gate615)
        export CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1 CARAXES_MIRROR_GR_SHADOW=1
        export CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
        export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=5
        export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
        if [[ $MODE == gate610 ]]; then
            export VM=vm3 CARAXES_SERVER=$SRC/target/rpc/release/vfio-user-nvgpu
        else
            export VM=vm3-615 CARAXES_DRIVER=615 CARAXES_SERVER=$SRC/target/rpc615/release/vfio-user-nvgpu
        fi ;;
    *) echo "ERROR: unknown mode $MODE" >&2; exit 2 ;;
esac
W=/mnt/vmstore/caraxes/$VM; T0=$(date '+%F %T')
{ head -1 /proc/driver/nvidia/version; sha256sum "$CARAXES_SERVER"; git -C "$SRC" log --oneline -1; env | grep -E '^CARAXES_|^LEA_CH|^VM=' | sort; } > "$C/setup.txt"
caraxes/tools/bench-hostwatch.sh start "$C"
tools/caraxes-vm.sh up > "$C/up.txt" 2>&1 || tail -5 "$C/up.txt"
pgrep -af "vfio-user-nvgpu.*$W/" > "$C/device-pid.txt"
tools/caraxes-vm.sh wait >> "$C/up.txt" 2>&1
tools/caraxes-vm.sh probe gsprpc >> "$C/up.txt" 2>&1; tail -3 "$C/up.txt" | cut -c1-200
tools/caraxes-vm.sh ssh 'nvidia-smi; sudo dmesg | grep -E "NVRM|goeimport|nvidia" | grep -vE "ioctl|RmMsg" | tail -60' > "$C/guest.txt" 2>&1
if [[ $MODE != emu615 ]]; then
    tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
    tools/caraxes-gate.sh nvml > "$C/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?"; tail -1 "$C/gate-nvml.txt" | cut -c1-300
    tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -1 "$C/gate-cuda.txt" | cut -c1-300
fi
caraxes/tools/bench-hostwatch.sh stop "$C"
T=$W/gpu-trace.log
grep -aE 'ALLOC_MEMORY|alloc memory|event buffer|operational event|gmcapi GSP_INIT|unstable interfaces|registry (RmLogonRC|RMSplitVas|EnableGpuFirmwareLogs).*honoured|static-info key (EMULATION|WPR_LAYOUT|CHIP_SKU)|refused|RC_TRIGGERED|OS_ERROR_LOG|unsolicited' "$T" | cut -c1-400 > "$C/device-gsp-plan.txt"
grep -aE 'unanswered|NOT_SUPPORTED|status 0x56|refused' "$T" | sed -E 's/^.*(ctrl 0x[0-9a-f]+|alloc: |rx seq [0-9]+ )/\1/' | sort | uniq -c | sort -rn | head -25 | cut -c1-300 > "$C/device-trace-issues.txt"
grep -E 'unanswered|ERROR|refused|NOT_SUPPORTED' "$W/gpu.log" | sort | uniq -c | sort -rn | head -15 | cut -c1-300 > "$C/device-log-issues.txt"
tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
echo "$MODE done $(date -Iseconds); host Xid lines $(grep -c Xid "$C/host-xid.txt"); vm3 left: $(pgrep -f /mnt/vmstore/caraxes/vm3 | wc -l)"
