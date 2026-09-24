# SPDX-License-Identifier: MIT
# The category D runs on vm0 (docs/caraxes/gsp-plan/d-device-model.md R0, R1, §8 Q1 and Q3):
# the display page series' environment (display-page-2026-09-24/env.sh over
# intr-page-2026-09-24/env.sh: host execution, the display engine, Cloud Hypervisor 0006, the
# token pool and the hybrid reactor) at slot 0 with the 2 GiB profile, the interrupt page
# and the display page on as in series C, and the binary GSPD_SERVER (this branch's build,
# or its base for the run before the change). Source it from the worktree root.
here_gspd=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck disable=SC1091
source "$here_gspd/../display-page-2026-09-24/env.sh"
export CARAXES_RESERVE_SLOT=0
export CARAXES_INTR_PAGE=1 CARAXES_DISPLAY_PAGE=1
export CARAXES_SERVER=${GSPD_SERVER:?set GSPD_SERVER to the device binary}
