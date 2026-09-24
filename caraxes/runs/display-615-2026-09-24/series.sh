#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# One vm0-615 session of the 615 display series (docs/caraxes/display-615.md), with series C of
# the display page (display-page-2026-09-24/series.sh) and the Q1 window of the category D runs
# (gsp-d-2026-09-24/series.sh) as the model:
#   up (a refused VRAM reservation is retried every 5 minutes for up to an hour), wait, probe,
#   then per stage
#     setup    the first boot's guest setup (guest/setup-guest.sh), then down: the next
#              session boots with it
#     nvdrm    build nvidia-drm.ko without the HDCP property (guest/build-nvdrm-615.sh)
#     display  display-up.sh and nvidia-egl-setup.sh on (every session after setup needs it)
#     gnome    three fresh GNOME starts (display-page-2026-09-24/guest/gnome-restart.sh)
#     traps    the Mutter and Weston trap windows (run.sh --full, trap-table.py --pages)
#     q1       DP unplug and plug through the device's control file while the guest watches
#              the connector (crates/vfio-user-nvgpu/guest/hotplug-watch.sh), full trace kept,
#              the D controls of the window (gsp-d-2026-09-24/dscan.py)
#     gate     the display gate (tools/caraxes-gate.sh display), its trace window and dscan
#   and down right after, with the host's Xid lines of the session and the device's pid.
# Usage (from the worktree root): series.sh <session name> <stage...>
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
ROOT_WT=$(cd "$here/../../.." && pwd -P)
name=$1; shift
# shellcheck disable=SC1091
source "$here/env.sh"
W=${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM
VMSH=("$LEA_TOOLS/caraxes-vm.sh")
DSCAN=$ROOT_WT/caraxes/runs/gsp-d-2026-09-24/dscan.py
D=$here/$name
[[ -e $D ]] && { echo "ERROR: $D exists; never overwrite a session" >&2; exit 2; }
# The device and Cloud Hypervisor of vm0 or vm0-615 name their sockets on the command line.
VM0_PAT='/mnt/vmstore/caraxes/vm0[^ ]*/(gpu|ch)[.]sock'
if pgrep -f "$VM0_PAT" > /dev/null; then
    echo "ERROR: a vm0 process runs; not starting" >&2; pgrep -af "$VM0_PAT" >&2; exit 2
fi
mkdir -p "$D"
env | grep -E '^(CARAXES_|LEA_CH|VM=)' | sort > "$D/env.txt"
sha256sum "$CARAXES_SERVER" > "$D/binary.txt"
git -C "$ROOT_WT" log --oneline -1 >> "$D/binary.txt"
t0=$(date '+%F %T')
echo "== $name up at $t0"

finish() {
    "${VMSH[@]}" down > "$D/down.txt" 2>&1
    grep -E '^intr page:|^display page:|doorbell poll \(|ERROR|panic' "$W/gpu.log" | cut -c1-600 | tail -20 > "$D/device-summary.txt"
    {
        echo '# SPDX-License-Identifier: MIT'
        echo "# host kernel log, Xid lines since $t0 ($(cat "$D/device-pid.txt" 2>/dev/null))"
        journalctl -k --since "$t0" --no-pager -g 'Xid' 2>/dev/null | grep -v '^-- No entries'
    } > "$D/host-xid.txt"
    echo "$name: host Xid lines $(grep -vc '^#' "$D/host-xid.txt"); vm0 processes after down: $(pgrep -fc "$VM0_PAT")"
}

up_ok=0
for try in $(seq 13); do
    "${VMSH[@]}" up > "$D/up.txt" 2>&1 && { up_ok=1; break; }
    tail -3 "$D/up.txt"
    "${VMSH[@]}" down >> "$D/up-retries.txt" 2>&1
    if grep -q 'refused: reserving' "$W/gpu.log" "$D/up.txt" 2>/dev/null && ((try < 13)); then
        echo "try $try at $(date -Iseconds): reservation refused, next try in 5 minutes" | tee -a "$D/up-retries.txt"
        cp "$D/up.txt" "$D/up-refused-$try.txt"
        sleep 300
        continue
    fi
    break
done
((up_ok)) || { echo "up failed"; finish; exit 1; }
echo "device pid $(cat "$W/gpu.pid" 2>/dev/null)" | tee "$D/device-pid.txt"
"${VMSH[@]}" wait 240 >> "$D/up.txt" 2>&1 || { tail -3 "$D/up.txt"; finish; exit 1; }
grep -iE 'guest ABI|host ABI|display engine|intr page|display page|doorbell tokens|doorbell poll' "$W/gpu.log" | cut -c1-400 > "$D/device-start.txt"
"${VMSH[@]}" probe "d615-$name" > "$D/probe.txt" 2>&1
cp "$W/probe-d615-$name/guest-dmesg.txt" "$D/probe-guest-dmesg.txt" 2>/dev/null
tail -4 "$D/probe.txt"

for stage in "$@"; do
    case $stage in
        setup)
            "${VMSH[@]}" ssh 'cat > setup-guest.sh' < "$here/guest/setup-guest.sh"
            "${VMSH[@]}" ssh 'sudo bash setup-guest.sh' > "$D/setup-guest.txt" 2>&1
            echo "setup rc=$?"; head -12 "$D/setup-guest.txt"
            ;;
        nvdrm)
            "${VMSH[@]}" ssh 'cat > build-nvdrm-615.sh' < "$here/guest/build-nvdrm-615.sh"
            "${VMSH[@]}" ssh 'sudo bash build-nvdrm-615.sh' > "$D/build-nvdrm-615.txt" 2>&1
            echo "nvdrm rc=$?"; tail -6 "$D/build-nvdrm-615.txt"
            ;;
        display)
            for f in "$ROOT_WT"/caraxes/runs/display-page-2026-09-24/guest/*.sh; do
                "${VMSH[@]}" ssh "mkdir -p caraxes-f && cat > caraxes-f/$(basename "$f")" < "$f"
            done
            "$here/run.sh" "$name/display-up" 'cd caraxes-guest && sudo bash display-up.sh && sudo bash nvidia-egl-setup.sh on && lsmod | grep nvidia'
            cat "$D/display-up/guest.txt" | head -12
            ;;
        gnome)
            for i in 1 2 3; do
                "$here/run.sh" "$name/gnome-start-$i" bash caraxes-f/gnome-restart.sh 20
                grep -E 'Type=|gnome-shell pid|eglcore' "$D/gnome-start-$i/guest.txt"
            done
            ;;
        traps)
            "$here/run.sh" "$name/traps-mutter" --full bash caraxes-f/trap-mutter.sh 14
            "$here/run.sh" "$name/traps-weston" --full bash caraxes-f/trap-weston.sh 14
            for r in traps-mutter traps-weston; do
                python3 "$ROOT_WT/caraxes/tools/trap-table.py" "$D/$r/device-trace.txt.xz" --pages > "$D/$r/trap-table.txt" 2>&1
                grep -E "FRAMES|PRESENTED" "$D/$r/guest.txt"
                grep -E "^window|all trapped accesses" "$D/$r/trap-table.txt"
            done
            ;;
        q1)
            "${VMSH[@]}" ssh "mkdir -p d615 && cat > d615/hotplug-watch.sh" < "$ROOT_WT/crates/vfio-user-nvgpu/guest/hotplug-watch.sh"
            ( sleep 10; echo unplug > "$W/display-ctl"; echo "host: unplug at $(date -Iseconds)" >> "$D/q1-host.txt"
              sleep 14; echo plug > "$W/display-ctl"; echo "host: plug at $(date -Iseconds)" >> "$D/q1-host.txt" ) &
            "$here/run.sh" "$name/q1-unplug-plug" --full 'sudo bash d615/hotplug-watch.sh 40'
            wait
            python3 "$DSCAN" "$D/q1-unplug-plug/device-trace.txt.xz" > "$D/q1-unplug-plug/dscan.txt" 2>&1
            head -30 "$D/q1-unplug-plug/dscan.txt"
            ;;
        gate)
            b0=$(stat -c %s "$W/gpu-trace.log")
            CARAXES_GATE_RUNS=$D "$LEA_TOOLS/caraxes-gate.sh" display > "$D/gate.json" 2> "$D/gate.err"
            echo "gate rc=$?" >> "$D/gate.json"
            tail -c +$((b0 + 1)) "$W/gpu-trace.log" | xz -T0 -3 > "$D/gate-trace.txt.xz"
            python3 "$DSCAN" "$D/gate-trace.txt.xz" > "$D/gate-dscan.txt" 2>&1
            rm -f "$D/gate-trace.txt.xz"   # 20 MB; the dscan summary is kept
            grep -o '"result": *"[^"]*"' "$D/gate.json" | head -1
            grep -o '"detail": *"[^"]*"' "$D/gate.json" | head -1
            head -20 "$D/gate-dscan.txt"
            ;;
        *) echo "unknown stage $stage" ;;
    esac
done
# The controls the device did not answer, and the 615-only display controls, in the session.
grep -aE 'unanswered|NOT_SUPPORTED|status 0x56' "$W/gpu-trace.log" | sed -E 's/^.*(ctrl 0x[0-9a-f]+)/\1/' \
    | sort | uniq -c | sort -rn | head -25 | cut -c1-250 > "$D/device-trace-issues.txt"
grep -aE 'ctrl 0x(731391|7302b2|730282)' "$W/gpu-trace.log" | cut -c1-300 > "$D/device-trace-615-display.txt"
finish
