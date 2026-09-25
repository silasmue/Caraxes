# SPDX-License-Identifier: MIT
# The workload gate's environment for the night part (WORKLOAD_ENV of ../run.sh): the 615
# guest on vm2-615 at 1 GiB slot 3 with the night configuration
# (caraxes/runs/night-2026-09-24/env.sh: pool, token pool 64, hybrid reactor, interrupt page).
# The caller exports CARAXES_SERVER (run.sh would otherwise take the newest closing-610 build).
source "$(dirname "${BASH_SOURCE[0]}")/../../night-2026-09-24/env.sh"
night_guest 615
