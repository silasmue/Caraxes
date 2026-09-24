#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Plumbing, second boot: the device binary of main 3cb33af (env.sh's CARAXES_SERVER since the
# coordinator's switch) with the stage's settings in pure emulation: up (games overlay reused),
# wait, probe (nvidia.ko), display modules, the games disk after a reboot-free restart, down.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
export SOTTR_EMULATION=1
# shellcheck source=caraxes/runs/sottr-2026-09-24/env.sh
source "$here/../env.sh"
export LEA_TOOLS=${PLUMBING_TOOLS:-$LEA_TOOLS}
V=("$LEA_TOOLS/caraxes-vm.sh")
W=${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM
{
    echo "# SPDX-License-Identifier: MIT"
    echo "# $(date -Iseconds) binary check, CARAXES_SERVER=$CARAXES_SERVER"
    sha256sum "$CARAXES_SERVER"
    "${V[@]}" up 2>&1
    "${V[@]}" wait 240 2>&1
    grep -iE 'intr page|display page|doorbell tokens|TU106' "$W/gpu.log" | cut -c1-200
    "${V[@]}" probe sottr-3cb33af 2>&1 | tail -3
    "${V[@]}" ssh 'cd caraxes-guest && sudo bash display-up.sh 2>&1 | tail -2; findmnt -n /games; ls /games/SteamLibrary/steamapps/common | grep Tomb; systemctl get-default; sync'
    "${V[@]}" down 2>&1
    grep -E '^intr page:|^display page:|doorbell poll \(' "$W/gpu.log" | cut -c1-300
} > "$here/binary-check.txt" 2>&1
cat "$here/binary-check.txt"
