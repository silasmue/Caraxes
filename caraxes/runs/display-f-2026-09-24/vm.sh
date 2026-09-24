#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# tools/caraxes-vm.sh with the series-F environment (env.sh beside this file); run from the
# worktree root: caraxes/runs/display-f-2026-09-24/vm.sh up|wait|ssh|probe|down ...
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd -P)
# shellcheck disable=SC1091
source "$here/env.sh"
exec tools/caraxes-vm.sh "$@"
