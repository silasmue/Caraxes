# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The workload gate's environment for the morning matrix (WORKLOAD_ENV of
# caraxes/runs/workload-2026-09-24/run.sh): the 615 guest on vm2-615 (the night's workload
# overlay) at the 2 GiB profile in reservation slot 0, with the matrix windows of env.sh.
source "$(dirname "${BASH_SOURCE[0]}")/env.sh"
mx_guest wl615
