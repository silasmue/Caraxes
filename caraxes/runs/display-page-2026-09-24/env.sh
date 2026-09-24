# SPDX-License-Identifier: MIT
# The display page series on vm0 (docs/caraxes/intr-page.md §6a and §10): the environment of
# the interrupt page series (series F's vm0 at slot 1, 2 GiB profile, host execution, the
# display engine, Cloud Hypervisor 0001-0006, the token pool, the hybrid reactor), with the
# binary of this branch. CARAXES_INTR_PAGE and CARAXES_DISPLAY_PAGE are set per configuration
# by series.sh. Source it from the worktree root.
here_env=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck disable=SC1091
source "$here_env/../intr-page-2026-09-24/env.sh"
# Frozen from target/fepage/release/vfio-user-nvgpu of this branch before the series.
export CARAXES_SERVER=${CARAXES_SERVER_FEPAGE:-/mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-fepage}
unset CARAXES_INTR_PAGE CARAXES_DISPLAY_PAGE
