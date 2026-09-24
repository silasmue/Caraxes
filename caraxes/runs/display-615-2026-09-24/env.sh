# SPDX-License-Identifier: MIT
# The 615 display series on vm0 (docs/caraxes/display-615.md): the environment of series C of
# the display page (display-page-2026-09-24/env.sh over intr-page-2026-09-24/env.sh: host
# execution, the display engine, Cloud Hypervisor 0006, the token pool and the hybrid reactor)
# at slot 0 with the 2 GiB profile, the interrupt page and the display page on, with the 615
# guest: state folder vm0-615 (prepare-vm0-615.sh), CARAXES_DRIVER=615 (the launcher takes the
# 615 base image and passes --guest-abi 615), and the v615 build of this branch
# (CARGO_TARGET_DIR=target/disp615 cargo build --release -p vfio-user-nvgpu --features v615).
# Source it from the worktree root.
here_d615=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
root_d615=$(cd "$here_d615/../../.." && pwd -P)
# shellcheck disable=SC1091
source "$here_d615/../display-page-2026-09-24/env.sh"
export VM=vm0-615 CARAXES_DRIVER=615
export CARAXES_RESERVE_SLOT=0 CARAXES_FB_MIB=2048 CARAXES_GATE_FB_MIB=2048
export CARAXES_INTR_PAGE=1 CARAXES_DISPLAY_PAGE=1
export CARAXES_SERVER=${D615_SERVER:-$root_d615/target/disp615/release/vfio-user-nvgpu}
