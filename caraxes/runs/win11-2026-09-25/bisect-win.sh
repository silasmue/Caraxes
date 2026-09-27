#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# One headless pure-emulation boot per argument: the Y31 set with CARAXES_WIN_EXCEPT=<list> (and, for
# "nofalcon", without CARAXES_PREFER_RECORDED); prints started/failed per run from night.log.
# Usage: bisect-win.sh <binary> <name>=<except list|nofalcon> ...
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P); D=$HERE/night-2026-09-25; BIN=$1; shift
for spec in "$@"; do
    name=${spec%%=*}; ex=${spec#*=}
    envs="CARAXES_LPWR_2806_OK=1 CARAXES_EMU_GR=1 CARAXES_RUSD=1 CARAXES_WIN_COMPBIT=1 CARAXES_WIN_DISPLAY_QUERIES=1 CARAXES_WIN_ACKS=1 CARAXES_ACK_UNANSWERED=1 WIN_GPU_DISPLAY=0"
    [[ $ex == nofalcon ]] && ex=0 || envs+=" CARAXES_PREFER_RECORDED=1"
    envs+=" CARAXES_WIN_EXCEPT=$ex"
    WIN_MEM=4G NIGHT_MAX=240 NIGHT_RUNS="$name" NIGHT_SERVER=$BIN NIGHT_ENV="$envs" "$HERE/night-win4.sh" "$D" > /dev/null 2>&1 < /dev/null
    r=$(grep -a "$name: guest Problem" "$D/night.log" | tail -1 | awk '{print $NF}')
    s=$(grep -ac "GMCAPI_GSP_SUSPEND" /mnt/vmstore/caraxes/win11/gpu-trace.log)
    echo "$(date +%T) $name except=[$ex] -> ${r:-no guest answer}, GSP_SUSPEND x$s" | tee -a "$D/bisect.log"
done
