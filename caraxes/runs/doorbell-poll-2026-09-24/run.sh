#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The doorbell reactor on the GPU path (docs/caraxes/doorbell-poll.md): P21 area plus the
# datamatch token pool (64 tokens), vm1 with the 1 GiB profile at slot 4, Cloud Hypervisor
# 0001-0006, as caraxes/runs/doorbell-tokens-gpu-2026-09-24/run.sh. Configurations:
#   C   the reader blocks in epoll_wait (CARAXES_DOORBELL_POLL=off, as there)
#   D0  spin, not pinned (CARAXES_DOORBELL_POLL_CPU=none)
#   D   spin, pinned (CARAXES_DOORBELL_POLL_CPU=${POLL_CPU:-auto})
#   E   hybrid, 200 us idle, pinned as D
# No bench-trap here: its 20000 pool-token writes would be rings of the reactor too.
set -uo pipefail
R=${R:-/home/silas/git/Caraxes}; W=$(cd "$(dirname "$0")" && pwd); O=${O:-$W}
source "$R/caraxes/runs/doorbell-2026-09-24/env-vm1.sh"
export CARAXES_SERVER=/mnt/vmstore/caraxes/vm1/vfio-user-nvgpu-poll
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_BAR1_PASSTHROUGH=1 CARAXES_DOORBELL_TOKENS=64; unset CARAXES_DOORBELL_IOEVENTFD
for cfg in ${CONFIGS:-C-off D0-spin-unpinned D-spin-pinned E-hybrid-200us}; do
  D=$O/$cfg; mkdir -p "$D"
  unset CARAXES_DOORBELL_POLL CARAXES_DOORBELL_POLL_CPU CARAXES_DOORBELL_POLL_IDLE_US
  case $cfg in
    C-*) ;;
    D0-*) export CARAXES_DOORBELL_POLL=spin CARAXES_DOORBELL_POLL_CPU=none ;;
    D-*) export CARAXES_DOORBELL_POLL=spin CARAXES_DOORBELL_POLL_CPU=${POLL_CPU:-auto} ;;
    E-*) export CARAXES_DOORBELL_POLL=hybrid CARAXES_DOORBELL_POLL_CPU=${POLL_CPU:-auto} CARAXES_DOORBELL_POLL_IDLE_US=200 ;;
  esac
  env | grep -E '^CARAXES_DOORBELL' | sort > "$D/env.txt"
  t0=$(date '+%Y-%m-%d %H:%M:%S')
  "$R/caraxes/tools/bench-hostwatch.sh" start "$D"
  if ! "$R/caraxes/tools/bench-run.sh" v2-restart > "$D/up.txt" 2>&1; then
    tail "$D/up.txt"; "$R/caraxes/tools/bench-hostwatch.sh" stop "$D"
    "$R/tools/caraxes-vm.sh" down > "$D/down.txt" 2>&1; continue
  fi
  grep -iE 'bar1 passthrough: guest|doorbell tokens|doorbell poll|ioeventfd' /mnt/vmstore/caraxes/$VM/gpu.log | cut -c1-300 > "$D/device-start.txt"
  # Where the reader thread runs, and the host's per-CPU load over 5 s before the series.
  pid=$(cat /mnt/vmstore/caraxes/$VM/gpu.pid)
  for t in /proc/"$pid"/task/*; do
    [[ $(cat "$t/comm") == doorbell-* ]] &&
      echo "$(cat "$t/comm") tid ${t##*/} on CPU $(awk '{print $39}' "$t/stat") allowed $(grep Cpus_allowed_list "$t/status" | cut -f2)"
  done > "$D/reader-thread.txt"
  a=$(grep '^cpu[0-9]' /proc/stat); sleep 5; b=$(grep '^cpu[0-9]' /proc/stat)
  paste <(echo "$a") <(echo "$b") | awk '{n=NF/2; t0=0; t1=0; for (i=2;i<=9;i++) {t0+=$i; t1+=$(i+n)}; printf "%s %.0f%%\n", $1, 100*(1-(($(5+n)+$(6+n))-($5+$6))/(t1-t0))}' > "$D/host-cpu-busy.txt"
  "$R/caraxes/tools/bench-run.sh" series --targets v2 --reps "${REPS:-3}" --metrics "${METRICS:-short-auto rl}" --out "$D" 2>&1 | cut -c1-200 | tail -4
  "$R/caraxes/tools/bench-hostwatch.sh" stop "$D"
  "$R/tools/caraxes-vm.sh" down > "$D/down.txt" 2>&1
  grep -E 'bar1 passthrough: guest|ERROR: bar1|doorbell|^mirror: [0-9]' /mnt/vmstore/caraxes/$VM/gpu.log | grep -v ': check [0-9]' | cut -c1-900 > "$D/device-summary.txt"
  grep -c 'W bar0 0x00bb0090' /mnt/vmstore/caraxes/$VM/gpu-trace.log > "$D/doorbell-trapped.txt"
  journalctl -k --since "$t0" --no-pager -g 'Xid' > "$D/host-xid.txt"
  echo "$cfg: doorbells trapped $(cat "$D/doorbell-trapped.txt"), Xid $(grep -c Xid "$D/host-xid.txt"); $(grep -h MSPERLAUNCH= "$D"/v2/short-auto-*.txt | tr '\n' ' ')"
  grep -h 'doorbell poll (' "$D/device-summary.txt"
done
python3 "$R/caraxes/tools/bench-summary.py" "$O"/C-off/v2 "$O"/D0-spin-unpinned/v2 "$O"/D-spin-pinned/v2 "$O"/E-hybrid-200us/v2 > "$O/summary.md" 2>&1
