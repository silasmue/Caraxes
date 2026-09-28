#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The launch path series on vm5 (env.sh): per metric its own bench-run.sh series with the VM and
# its device restarted before every rep, so each kept device log (<metric>/v2-device-rep<r>.txt,
# with the exit summary's `launch timing:` lines) holds one metric. Default: short-auto,
# launch1k, rl and copy with 3 reps (OL_METRICS, OL_REPS), then ppo (60 s) with 2 (OL_PPO=0
# skips it). Host watch around the part, down at the end, host Xid lines.
# Usage: bench.sh LABEL BINARY [VAR=value ...]   (the extra variables go to the device)
set -uo pipefail
source "$(dirname "$(readlink -f "$0")")/env.sh"
LABEL=${1:?usage: bench.sh LABEL BINARY [VAR=value ...]}
export CARAXES_SERVER=${2:?binary}
shift 2
for kv in "$@"; do export "${kv?}"; done
C=$OL_HERE/$LABEL
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
[[ -x $CARAXES_SERVER ]] || { echo "ERROR: no binary $CARAXES_SERVER" >&2; exit 2; }
ol_free || exit 2
mkdir -p "$C"
cd "$OL_ROOT" || exit 1
T0=$(date '+%F %T')
ol_setup "$C"
BR=caraxes/tools/bench-run.sh
caraxes/tools/bench-hostwatch.sh start "$C"
$BR v2-restart > "$C/v2-up.txt" 2>&1 || { echo "v2 did not come up"; tail -5 "$C/v2-up.txt"; }
grep -iE 'intr page|bar2 memfd|bar1 pool|msi|lazy|doorbell tokens|doorbell poll|launch|host ABI|guest ABI' "$OL_W/gpu.log" | cut -c1-400 > "$C/device-start.txt"
$BR stage v2 >> "$C/v2-up.txt" 2>&1
series() {  # METRIC REPS [VAR=value ...]: its own series, then a restart that keeps the last rep's log
    local m=$1 reps=$2
    shift 2
    env "$@" V2_RESTART=1 $BR series --targets v2 --reps "$reps" --metrics "$m" --out "$C/$m" 2>&1 | tail -2
    { echo "# $(date +%T) v2 restart after the $m series"
      V2_LOG_KEEP="$C/$m/v2-device-rep$reps.txt" $BR v2-restart; } >> "$C/$m/series.txt" 2>&1
}
for m in ${OL_METRICS:-short-auto launch1k rl copy}; do series "$m" "${OL_REPS:-3}"; done
[[ ${OL_PPO:-1} == 1 ]] && series ppo 2 PPO_SECONDS=60
caraxes/tools/bench-hostwatch.sh stop "$C"
ol_finish "$C" "$T0"
