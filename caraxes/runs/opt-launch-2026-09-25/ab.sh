#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The launch path A/B series on vm5 (env.sh), interleaved because other agents' VMs load the host
# unevenly: per rep, A (the frozen before binary, 7095b3c, the recommended configuration) and B
# (the after binary with the cuts on) each get two VM lives: `launch` (short-auto, launch1k, copy)
# and `rl`; then ppo (60 s) A and B per rep. Every life is a fresh VM and device; its device log
# with the `launch timing:` lines is kept as <cfg>/<life>/v2-device-rep<r>.txt, the guest results
# under <cfg>/<life>/rep<r>/. Host watch around the part, down at the end, host Xid lines.
# Usage: ab.sh LABEL [REPS] [PPO_REPS]
set -uo pipefail
source "$(dirname "$(readlink -f "$0")")/env.sh"
LABEL=${1:?usage: ab.sh LABEL [REPS] [PPO_REPS]}
REPS=${2:-3} PPO_REPS=${3:-2}
export PPO_SECONDS=60
BIN_A=${OL_BIN_A:-/mnt/vmstore/caraxes/vfio-user-nvgpu-launch-before-7095b3c}
BIN_B=${OL_BIN_B:-/mnt/vmstore/caraxes/vfio-user-nvgpu-launch-after-25c164b}
# B's switches: the fast ring path, the adaptive hybrid window capped at 2 ms, the interrupt gate
# with a 50 ms hold (hot for 500 ms after a host doorbell).
B_ENV=(CARAXES_LAUNCH_FAST=1 CARAXES_DOORBELL_POLL_ADAPT_US=2000 CARAXES_INTR_GATE_MS=50)
# OL_B_ENV="VAR=value ..." replaces them; OL_LIVES="launch rl" picks the lives (ppo runs when PPO_REPS > 0);
# OL_BIN_A / OL_BIN_B replace the binaries, OL_LAUNCH_METRICS / OL_RL_METRICS the lives' metrics.
[[ -n ${OL_B_ENV:-} ]] && read -r -a B_ENV <<< "$OL_B_ENV"
C=$OL_HERE/$LABEL
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
ol_free || exit 2
mkdir -p "$C"
cd "$OL_ROOT" || exit 1
T0=$(date '+%F %T')
export CARAXES_SERVER=$BIN_A
ol_setup "$C"
{ echo "# A: $BIN_A $(sha256sum < "$BIN_A" | cut -c1-16)"
  echo "# B: $BIN_B $(sha256sum < "$BIN_B" | cut -c1-16) ${B_ENV[*]}"; } >> "$C/setup.txt"
BR=caraxes/tools/bench-run.sh
caraxes/tools/bench-hostwatch.sh start "$C"
prev=""
cfg_env() {  # A|B: the device's binary and switches
    unset CARAXES_LAUNCH_FAST CARAXES_DOORBELL_POLL_ADAPT_US CARAXES_INTR_GATE_MS CARAXES_INTR_DIRECT
    if [[ $1 == A ]]; then export CARAXES_SERVER=$BIN_A; else export CARAXES_SERVER=$BIN_B "${B_ENV[@]}"; fi
}
life() {  # CFG LIFE REP METRICS...: a fresh VM and device, then the metrics once
    local cfg=$1 name=$2 r=$3
    shift 3
    local d=$C/$cfg/$name
    mkdir -p "$d/rep$r"
    cfg_env "$cfg"
    { echo "# $(date +%T) $cfg $name rep $r: restart (keeps the previous life's log as ${prev:-nothing})"
      V2_LOG_KEEP=$prev $BR v2-restart; } >> "$C/lives.txt" 2>&1
    pgrep -af "vfio-user-nvgpu.*$OL_W/" | cut -c1-200 >> "$d/device-pid.txt"
    $BR series --targets v2 --reps 1 --metrics "$*" --out "$d/rep$r" 2>&1 | tail -1
    prev=$d/v2-device-rep$r.txt
}
cfg_env A
$BR v2-restart > "$C/v2-up.txt" 2>&1 || { echo "v2 did not come up"; tail -5 "$C/v2-up.txt"; }
$BR stage v2 >> "$C/v2-up.txt" 2>&1
for r in $(seq 1 "$REPS"); do
    for cfg in A B; do
        [[ " ${OL_LIVES:-launch rl} " == *" launch "* ]] && life "$cfg" launch "$r" ${OL_LAUNCH_METRICS:-short-auto launch1k copy}
        [[ " ${OL_LIVES:-launch rl} " == *" rl "* ]] && life "$cfg" rl "$r" ${OL_RL_METRICS:-rl}
    done
done
for r in $(seq 1 "$PPO_REPS"); do
    for cfg in A B; do life "$cfg" ppo "$r" ppo; done
done
"$LEA_TOOLS/caraxes-vm.sh" down > "$C/down.txt" 2>&1
[[ -n $prev ]] && cp "$OL_W/gpu.log" "$prev"
caraxes/tools/bench-hostwatch.sh stop "$C"
{ echo "# host kernel log, Xid lines since $T0 (device pids in */*/device-pid.txt)"
  journalctl -k --since "$T0" --no-pager -g Xid 2>/dev/null | grep -v '^-- No entries'; } > "$C/host-xid.txt"
echo "$LABEL done $(date -Iseconds): host Xid lines $(grep -vc '^#' "$C/host-xid.txt"), $VM processes left $(pgrep -fc "/mnt/vmstore/caraxes/$VM/")"
