#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Morning matrix, display part: the display gate (tools/caraxes-gate.sh display, 11 stages) on vm0
# at the 2 GiB profile in reservation slot 0.
#   610: vm0 in the environment of caraxes/runs/display-page-2026-09-24/env.sh (series C: interrupt
#        page and display page) at slot 0 with the main fbe4dfb 610 build;
#   615: vm0-615 in the environment of caraxes/runs/display-615-2026-09-24/env.sh with the v615
#        fbe4dfb build (the overlay carries nvidia-drm from session s3, so `display gate` as s4).
# Both with the matrix windows of env.sh (BAR1 pool 32 MiB, BAR2 memfd, MSI re-arm ioeventfd,
# lazy plans) on top. Per session: up (refused reservations retried), probe, display-up through
# the series' own run.sh (its window records land here), the gate with its trace window's D-scan,
# down, host Xid lines with the device pid.
# Usage (from the worktree root): display.sh 610|615 [label]
set -uo pipefail
source "$(dirname "$(readlink -f "$0")")/env.sh"
G=${1:?usage: display.sh 610|615 [label]}
case $G in
    610) S=$MX_ROOT/caraxes/runs/display-page-2026-09-24
         source "$S/env.sh"
         export CARAXES_SERVER=$MX_BIN610 CARAXES_RESERVE_SLOT=0 CARAXES_INTR_PAGE=1 CARAXES_DISPLAY_PAGE=1 ;;
    615) S=$MX_ROOT/caraxes/runs/display-615-2026-09-24
         D615_SERVER=$MX_BIN615 source "$S/env.sh" ;;
    *) echo "ERROR: guest 610|615" >&2; exit 2 ;;
esac
mx_windows
MX_W=/mnt/vmstore/caraxes/$VM
C=$MX_HERE/display-$G${2:+-$2}
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
VM0_PAT='/mnt/vmstore/caraxes/vm0[^ ]*/(gpu|ch)[.]sock'
if pgrep -f "$VM0_PAT" > /dev/null; then
    echo "ERROR: a vm0 process runs; not starting" >&2; pgrep -af "$VM0_PAT" >&2; exit 2
fi
mkdir -p "$C"; cd "$MX_ROOT" || exit 1
VMSH=$LEA_TOOLS/caraxes-vm.sh
DSCAN=$MX_ROOT/caraxes/runs/gsp-d-2026-09-24/dscan.py
T0=$(date '+%F %T')
mx_setup "$C"
caraxes/tools/bench-hostwatch.sh start "$C"
if mx_up "$C"; then
    grep -iE 'guest ABI|host ABI|display engine|intr page|display page|bar2 memfd|bar1 pool|msi|lazy|doorbell tokens|doorbell poll' "$MX_W/gpu.log" | cut -c1-400 > "$C/device-start.txt"
    "$VMSH" probe "mx-display-$G" > "$C/probe.txt" 2>&1; tail -3 "$C/probe.txt"
    for f in "$MX_ROOT"/caraxes/runs/display-page-2026-09-24/guest/*.sh; do
        "$VMSH" ssh "mkdir -p caraxes-f && cat > caraxes-f/$(basename "$f")" < "$f"
    done
    # The series' run.sh writes to its own folder plus the label; the label points back here.
    "$S/run.sh" "../matrix-2026-09-25/$(basename "$C")/display-up" 'cd caraxes-guest && sudo bash display-up.sh && sudo bash nvidia-egl-setup.sh on && lsmod | grep nvidia'
    head -12 "$C/display-up/guest.txt"
    b0=$(stat -c %s "$MX_W/gpu-trace.log")
    CARAXES_GATE_RUNS=$C "$LEA_TOOLS/caraxes-gate.sh" display > "$C/gate.json" 2> "$C/gate.err"
    echo "gate rc=$?" >> "$C/gate.json"
    tail -c +$((b0 + 1)) "$MX_W/gpu-trace.log" | xz -T0 -3 > "$C/gate-trace.txt.xz"
    python3 "$DSCAN" "$C/gate-trace.txt.xz" > "$C/gate-dscan.txt" 2>&1
    rm -f "$C/gate-trace.txt.xz"   # tens of MB; the D-scan summary is kept
    grep -o '"result": *"[^"]*"' "$C/gate.json" | head -1
    grep -o '"detail": *"[^"]*"' "$C/gate.json" | head -1
    grep -aE 'unanswered|NOT_SUPPORTED|status 0x56' "$MX_W/gpu-trace.log" | sed -E 's/^.*(ctrl 0x[0-9a-f]+)/\1/' \
        | sort | uniq -c | sort -rn | head -25 | cut -c1-250 > "$C/device-trace-issues.txt"
else
    echo "VM did not come up (up.txt, retries.txt)"
fi
caraxes/tools/bench-hostwatch.sh stop "$C"
mx_finish "$C" "$T0"
