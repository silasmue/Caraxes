#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The plumbing test's guest checks (pure emulation, SOTTR_EMULATION=1, after up.sh): the games
# disk, the game's files, Steam's preparation, the Steam launcher and the Steam Linux Runtime,
# Sunshine. Usage: plumbing/checks.sh <label> [guest command]   (output to plumbing/<label>.txt);
# without a command, plumbing/<label>.sh is run in the guest with bash -s.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
export SOTTR_EMULATION=1
# shellcheck source=caraxes/runs/sottr-2026-09-24/env.sh
source "$here/../env.sh"
label=$1; shift
{
    echo "# SPDX-License-Identifier: MIT"
    if [[ $# -gt 0 ]]; then
        echo "# $(date -Iseconds) guest command: $*"
        "$LEA_TOOLS/caraxes-vm.sh" ssh "$*" 2>&1
    else
        echo "# $(date -Iseconds) guest script: $label.sh"
        "$LEA_TOOLS/caraxes-vm.sh" ssh bash -s < "$here/$label.sh" 2>&1
    fi
    echo "rc=$?"
} > "$here/$label.txt"
cat "$here/$label.txt"
