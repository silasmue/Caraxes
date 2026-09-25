# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The desktop series of 2026-09-25 (docs/caraxes/intr-page.md §11): the environment of the display
# page series (display-page-2026-09-24/env.sh: vm0, 2 GiB profile, host execution, the display
# engine, Cloud Hypervisor 0001-0006, token pool, hybrid reactor) at reservation slot 0, with the
# recommended configuration of matrix-2026-09-25 (interrupt page, display page, BAR1 pool 32 MiB,
# BAR2 memfd, MSI re-arm ioeventfd, lazy plans) and the binary frozen from this branch
# (CARAXES_SERVER_DESK). The switches of the new windows are set per configuration by series.sh.
# Source it from the worktree root.
here_env25=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck disable=SC1091
source "$here_env25/../display-page-2026-09-24/env.sh"
export CARAXES_SERVER=${CARAXES_SERVER_DESK:-/mnt/vmstore/caraxes/vm0/vfio-user-nvgpu-desk-d96cc59}
export CARAXES_RESERVE_SLOT=0
export CARAXES_INTR_PAGE=1 CARAXES_DISPLAY_PAGE=1
export CARAXES_BAR1_PASSTHROUGH=pool CARAXES_BAR1_POOL_MIB=32
export CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 CARAXES_ALLOC_LAZY=1
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_BAR2_MEMFD_SYNC_DOORBELL CARAXES_TRACE_FLUSH
unset CARAXES_GUEST_ABI CARAXES_COSTUME CARAXES_DRIVER
