#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# One short vm0 session of the category D runs (docs/caraxes/gsp-plan/d-device-model.md R0,
# R1, §8 Q1 and Q3), guest side only, no host reads:
#   up, wait, probe; then per stage
#     gate  the display gate (tools/caraxes-gate.sh display) as series C of the display page
#     q1    DP unplug and plug through the device's control file while the guest watches the
#           connector (crates/vfio-user-nvgpu/guest/hotplug-watch.sh), full trace window kept
#     q3    the cuda gate, then kill -9 of a running CUDA process (bench-cu churn) and a new
#           CUDA process after it, full trace window kept
#   and down right after. Every window's D controls: dscan.py.
# Usage (from the worktree root): series.sh <name> <device binary> <stage...>
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
ROOT_WT=$(cd "$here/../../.." && pwd -P)
name=$1 bin=$2; shift 2
export GSPD_SERVER=$bin
# shellcheck disable=SC1091
source "$here/env.sh"
W=${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM
VMSH=("$LEA_TOOLS/caraxes-vm.sh")
D=$here/$name
mkdir -p "$D"
if pgrep -f "$VM/gpu.sock|$VM/ch.sock" > /dev/null; then
    echo "$VM is in use; not starting"
    exit 1
fi
env | grep -E '^(CARAXES_|LEA_CH)' | sort > "$D/env.txt"
sha256sum "$bin" > "$D/binary.txt"
t0=$(date '+%F %T')
echo "== $name up at $t0 ($bin)"
"${VMSH[@]}" up > "$D/up.txt" 2>&1 || { tail -5 "$D/up.txt"; "${VMSH[@]}" down > "$D/down.txt" 2>&1; exit 1; }
"${VMSH[@]}" wait 180 >> "$D/up.txt" 2>&1
"${VMSH[@]}" probe "gspd-$name" > "$D/probe.txt" 2>&1
echo "device pid $(cat "$W/gpu.pid" 2>/dev/null)" | tee "$D/device-pid.txt"

# window <label> <guest command>: run it and keep the device trace window of it.
window() {
    local label=$1; shift
    local out=$D/$label b0 rc
    mkdir -p "$out"
    b0=$(stat -c %s "$W/gpu-trace.log")
    printf '# SPDX-License-Identifier: MIT\n# %s guest command:\n%s\n' "$(date '+%F %T')" "$*" > "$out/command.txt"
    "${VMSH[@]}" ssh "$*" > "$out/guest.txt" 2>&1
    rc=$?
    echo "rc=$rc" >> "$out/command.txt"
    sleep 1
    tail -c +$((b0 + 1)) "$W/gpu-trace.log" | xz -T0 -3 > "$out/device-trace.txt.xz"
    python3 "$here/dscan.py" "$out/device-trace.txt.xz" > "$out/dscan.txt" 2>&1
    head -30 "$out/dscan.txt"
}

for stage in "$@"; do
    case $stage in
        gate)
            b0=$(stat -c %s "$W/gpu-trace.log")
            CARAXES_GATE_RUNS=$D "$LEA_TOOLS/caraxes-gate.sh" display > "$D/gate.json" 2> "$D/gate.err"
            echo "gate rc=$?" >> "$D/gate.json"
            tail -c +$((b0 + 1)) "$W/gpu-trace.log" | xz -T0 -3 > "$D/gate-trace.txt.xz"
            python3 "$here/dscan.py" "$D/gate-trace.txt.xz" > "$D/gate-dscan.txt" 2>&1
            grep -o '"summary": *"[^"]*"' "$D/gate.json" | head -1
            head -20 "$D/gate-dscan.txt"
            ;;
        q1)
            "${VMSH[@]}" ssh "mkdir -p gspd && cat > gspd/hotplug-watch.sh" < "$ROOT_WT/crates/vfio-user-nvgpu/guest/hotplug-watch.sh"
            "${VMSH[@]}" ssh 'sudo modprobe nvidia-drm modeset=1 2>/dev/null; ls /sys/class/drm/' > "$D/q1-drm.txt" 2>&1
            ( sleep 10; echo unplug > "$W/display-ctl"; echo "host: unplug at $(date -Iseconds)" >> "$D/q1-host.txt"
              sleep 14; echo plug > "$W/display-ctl"; echo "host: plug at $(date -Iseconds)" >> "$D/q1-host.txt" ) &
            window q1-unplug-plug 'sudo bash gspd/hotplug-watch.sh 40'
            wait
            ;;
        q3)
            # The device nodes and the guest test runner the cuda gate needs (bench-run.sh
            # V2_NODES; tools/caraxes-vm.sh sync-tests).
            "${VMSH[@]}" ssh 'sudo modprobe nvidia-uvm
                [ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
                [ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
                u=$(grep nvidia-uvm /proc/devices | cut -d" " -f1); sudo rm -f /dev/nvidia-uvm /dev/nvidia-uvm-tools
                sudo mknod -m 666 /dev/nvidia-uvm c $u 0; sudo mknod -m 666 /dev/nvidia-uvm-tools c $u 1' > "$D/q3-nodes.txt" 2>&1
            "${VMSH[@]}" sync-tests > "$D/q3-sync-tests.txt" 2>&1
            b0=$(stat -c %s "$W/gpu-trace.log")
            CARAXES_GATE_RUNS=$D "$LEA_TOOLS/caraxes-gate.sh" cuda > "$D/cuda-gate.json" 2> "$D/cuda-gate.err"
            echo "cuda gate rc=$?" >> "$D/cuda-gate.json"
            tail -c +$((b0 + 1)) "$W/gpu-trace.log" | xz -T0 -3 > "$D/cuda-gate-trace.txt.xz"
            python3 "$here/dscan.py" "$D/cuda-gate-trace.txt.xz" > "$D/cuda-gate-dscan.txt" 2>&1
            grep -o '"summary": *"[^"]*"' "$D/cuda-gate.json" | head -1
            head -20 "$D/cuda-gate-dscan.txt"
            window q3-kill9 'cd ~/bench && (./bench-cu churn 100000 > /tmp/churn.txt 2>&1 & p=$!; sleep 8; echo "kill -9 $p after 8 s"; kill -9 $p; wait $p; echo "churn exit $?"; tail -2 /tmp/churn.txt; sleep 3; ./bench-cu init; echo "init exit $?"; sudo dmesg | grep -E "NVRM|Xid" | grep -v "NVRM: VM:" | tail -12)'
            ;;
        *) echo "unknown stage $stage" ;;
    esac
done

"${VMSH[@]}" down > "$D/down.txt" 2>&1
grep -E '^intr page:|^display page:|doorbell poll \(|ERROR|panic' "$W/gpu.log" | cut -c1-600 | tail -20 > "$D/device-summary.txt"
{
    echo '# SPDX-License-Identifier: MIT'
    echo "# host kernel log, Xid lines since $t0 ($(cat "$D/device-pid.txt"))"
    journalctl -k --since "$t0" --no-pager -g 'Xid' 2>/dev/null | grep -v '^-- No entries'
} > "$D/host-xid.txt"
echo "$name: host Xid lines $(grep -vc '^#' "$D/host-xid.txt"); vm0 processes after down: $(pgrep -fc "$VM/gpu.sock|$VM/ch.sock")"
