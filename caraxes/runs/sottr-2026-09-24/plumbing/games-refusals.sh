#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Plumbing: the launcher's games-disk refusals, against a scratch base (never the v1 base):
# a missing base, and an overlay start while another VM holds the writer lock (a live pid).
# Both must stop in `up` before the device or the VM starts.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
export SOTTR_EMULATION=1
# shellcheck source=caraxes/runs/sottr-2026-09-24/env.sh
source "$here/../env.sh"
export LEA_TOOLS=${PLUMBING_TOOLS:-$LEA_TOOLS}
V=("$LEA_TOOLS/caraxes-vm.sh")
S=${1:?scratch dir}
STORE=${CARAXES_STORE:-/mnt/vmstore/caraxes}
{
    echo "# SPDX-License-Identifier: MIT"
    echo "== 1. missing base"
    CARAXES_GAMES_DISK=$S/none.qcow2 "${V[@]}" up; echo "rc=$?"
    echo "== 2. writer lock held by another VM (live pid)"
    qemu-img create -q -f qcow2 "$S/base.qcow2" 16M
    h=$(printf '%s' "$(readlink -f "$S/base.qcow2")" | sha256sum | cut -c1-16)
    mkdir -p "$STORE/locks"
    sleep 60 & holder=$!
    echo "vm-other $holder $S/base.qcow2" > "$STORE/locks/games-$h.writer"
    CARAXES_GAMES_DISK=$S/base.qcow2 "${V[@]}" up; echo "rc=$?"
    kill $holder; wait $holder 2>/dev/null
    rm -f "$STORE/locks/games-$h.writer"
    pgrep -f "$STORE/vm4/gpu.sock" >/dev/null && echo "UNEXPECTED: a vm4 device runs" || echo "no vm4 device started"
} 2>&1 | sed "s|$S|<scratch>|g" > "$here/games-refusals.txt"
cat "$here/games-refusals.txt"
