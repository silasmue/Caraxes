#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The costume ladder's pure-emulation rungs (docs/caraxes/costume-ladder.md section 3, the
# environments of caraxes/runs/costume-ladder-2026-09-24/*/emu/env.txt) again, on vm7 with the
# guest module options every other Caraxes guest carries (prepare-vm7.sh CARAXES_GUEST_CONF=1),
# with this checkout's device. One record per rung under ladder-conf/<rung>/.
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
run() { local d=$1; shift; echo "== $d"; "$HERE/emu-check.sh" "$HERE/ladder-conf/$d" "$@" 2>&1 | grep -E "RmInitAdapter|rc=|^[^,]+, [0-9.]+,|Check failed|Assertion" | head -n 8; }
run baseline-tu106
run r1-tu104 CARAXES_COSTUME=rtx2080
run r2-ga106 CARAXES_COSTUME=rtx3060
run r3-ad106 CARAXES_COSTUME=rtx4060ti
run r4-gh100 CARAXES_COSTUME=h100 CARAXES_FB_MIB=4096
run r5-gb206 CARAXES_COSTUME=rtx5060ti-gb206 CARAXES_FB_MIB=4096
