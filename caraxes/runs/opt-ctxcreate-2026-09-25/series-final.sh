#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The final series of docs/caraxes/ctx-create.md with the branch head's build (a8f66e3, the
# code of a8b3a54), one vm3 session after the other (run.sh):
#   final-allocburn  20 iterations of convburn plus allocburn.py (bar1-pool.md §9 reproduction)
#   final            census, init and churn, the cuda gate (GATE=1)
#   before-4         d96cc59 (the start of the day), census, init and churn
#   final-2          a8f66e3, the same, right after before-4
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
B=/mnt/vmstore/caraxes/vfio-user-nvgpu-ctx610
BIN=$B-a8f66e3 "$HERE/run.sh" allocburn final-allocburn
BIN=$B-a8f66e3 GATE=1 "$HERE/run.sh" measure final
BIN=$B-d96cc59 "$HERE/run.sh" measure before-4
BIN=$B-a8f66e3 "$HERE/run.sh" measure final-2
