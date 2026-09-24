#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The interrupt page series (docs/caraxes/intr-page.md §9): vm0 with the series-F environment
# (env.sh), the token pool and the hybrid reactor, CARAXES_INTR_PAGE off (A) and on (B). Per
# configuration: up, probe, display-up, the Mutter and the Weston trap windows (run.sh --full,
# trap-table.py --pages), the display gate (compositor, present), the device's summary lines,
# down. Run from the worktree root: caraxes/runs/intr-page-2026-09-24/series.sh [A B]
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
# shellcheck disable=SC1091
source "$here/env.sh"
W=${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM
VMSH=("$LEA_TOOLS/caraxes-vm.sh")
[[ $# -eq 0 ]] && set -- A B
for cfg in "$@"; do
    case $cfg in
        A) unset CARAXES_INTR_PAGE; name=A-page-off ;;
        B) export CARAXES_INTR_PAGE=1; name=B-page-on ;;
        *) echo "unknown configuration $cfg"; continue ;;
    esac
    D=$here/$name; mkdir -p "$D"
    env | grep -E '^(CARAXES_|LEA_CH)' | sort > "$D/env.txt"
    t0=$(date '+%F %T')
    echo "== $name up at $t0"
    "${VMSH[@]}" up > "$D/up.txt" 2>&1 || { tail -5 "$D/up.txt"; "${VMSH[@]}" down > "$D/down.txt" 2>&1; continue; }
    "${VMSH[@]}" wait 180 >> "$D/up.txt" 2>&1
    grep -iE 'intr page|doorbell tokens|doorbell poll' "$W/gpu.log" | cut -c1-400 > "$D/device-start.txt"
    "${VMSH[@]}" probe "intrpage-$name" > "$D/probe.txt" 2>&1
    for f in "$here"/guest/*.sh; do
        "${VMSH[@]}" ssh "mkdir -p caraxes-f && cat > caraxes-f/$(basename "$f")" < "$f"
    done
    "$here/run.sh" "$name/display-up" 'cd caraxes-guest && sudo bash display-up.sh && sudo bash nvidia-egl-setup.sh on && lsmod | grep nvidia'
    "$here/run.sh" "$name/traps-mutter" --full bash caraxes-f/trap-mutter.sh 14
    "$here/run.sh" "$name/traps-weston" --full bash caraxes-f/trap-weston.sh 14
    for r in traps-mutter traps-weston; do
        python3 caraxes/tools/trap-table.py "$D/$r/device-trace.txt.xz" --pages > "$D/$r/trap-table.txt" 2>&1
        grep -E "FRAMES|PRESENTED" "$D/$r/guest.txt"
    done
    [[ -n ${SKIP_GATE:-} ]] || CARAXES_GATE_RUNS=$D "$LEA_TOOLS/caraxes-gate.sh" display > "$D/gate.json" 2> "$D/gate.err"
    echo "gate rc=$?" >> "$D/gate.json"
    "${VMSH[@]}" down > "$D/down.txt" 2>&1
    grep -E '^intr page:|doorbell poll \(|^vfio-user-nvgpu: intr page' "$W/gpu.log" | cut -c1-1200 > "$D/device-summary.txt"
    journalctl -k --since "$t0" --no-pager -g 'Xid' > "$D/host-xid.txt" 2>&1
    echo "$name: Xid lines $(grep -c Xid "$D/host-xid.txt"), device pid was $(cat "$W/gpu.pid" 2>/dev/null)"
    cat "$D/device-summary.txt"
done
