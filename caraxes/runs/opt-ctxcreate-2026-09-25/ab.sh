#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The prefetch A/B of docs/caraxes/ctx-create.md §5: the end-state binary a8b3a54, alternating
# CARAXES_SHADOW_PREFETCH=0 and the default, three sessions each (run.sh measure), so that the
# other agents' load on the shared host falls on both sides.
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
export BIN=/mnt/vmstore/caraxes/vfio-user-nvgpu-ctx610-a8b3a54
for i in 1 2 3; do
    CARAXES_SHADOW_PREFETCH=0 "$HERE/run.sh" measure "ab-off-$i"
    "$HERE/run.sh" measure "ab-on-$i"
done
