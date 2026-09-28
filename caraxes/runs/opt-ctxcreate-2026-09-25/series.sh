#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The closing series of docs/caraxes/ctx-create.md, one vm3 session after the other (run.sh):
#   after            9f712dc, census, init and churn, the cuda gate (GATE=1)
#   after-allocburn  9f712dc, 20 iterations of convburn plus allocburn.py
#   before-3         d96cc59 (the start of the day), census, init and churn
#   after-2          9f712dc, the same, right after before-3
#   after-noprefetch 9f712dc with CARAXES_SHADOW_PREFETCH=0
# The host is shared with other agents' VMs; each session records the other device processes
# at its start (other-vms-at-start.txt), and the A/B pairs run back to back.
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
B=/mnt/vmstore/caraxes/vfio-user-nvgpu-ctx610
BIN=$B-9f712dc GATE=1 "$HERE/run.sh" measure after
BIN=$B-9f712dc "$HERE/run.sh" allocburn after-allocburn
BIN=$B-d96cc59 "$HERE/run.sh" measure before-3
BIN=$B-9f712dc "$HERE/run.sh" measure after-2
CARAXES_SHADOW_PREFETCH=0 BIN=$B-9f712dc "$HERE/run.sh" measure after-noprefetch
