#!/usr/bin/env bash
# main.sh: wait (60 s polls, up to 4 h) until win-busy and linux-busy are gone and no vm4-615 device
# runs; claim linux-busy; cell A (tokenfix), cell B (invhold baseline), cell C (combo) if one exists.
set -u
SP=$(cd "$(dirname "$0")" && pwd -P); V=/mnt/vmstore/caraxes; B=$V/linux-busy
N=$(dirname "$SP"); LOG=$N/main.log; ME="tokenfix-test agent"
log() { echo "$(date '+%F %T') $*" >> "$LOG"; }
pre() {
  [[ -e $V/win-busy ]] && { echo "win-busy: $(head -c 80 $V/win-busy)"; return 1; }
  [[ -e $B ]] && { echo "linux-busy: $(head -c 80 $B)"; return 1; }
  pgrep -af 'vfio-user-nvgpu' | grep -q -- "--socket $V/vm4-615" && { echo "vm4-615 device runs"; return 1; }
  return 0
}
log "waiting"
ok=0
for i in $(seq 240); do
  why=$(pre) && { ok=1; break; }
  (( i % 5 == 1 )) && log "not yet: $why"
  sleep 60
done
(( ok )) || { log "not free in 4 h: $why"; exit 1; }
echo "$ME $(date -Iseconds)" > "$B"
trap 'grep -q "^tokenfix-test agent" /mnt/vmstore/caraxes/linux-busy 2>/dev/null && rm -f /mnt/vmstore/caraxes/linux-busy' EXIT
log "linux-busy claimed; MemAvailable $(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo) MiB"
chk() { local a; a=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo); (( a < 3072 )) && { log "MemAvailable $a MiB < 3 GB: stop"; exit 1; }
  journalctl -k --since "$(cat $N/$1/t0)" --no-pager -g 'Xid' 2>/dev/null | grep -vq '^-- ' && { log "Xid in $1: stop"; exit 1; }; return 0; }
"$SP/cell.sh" A-tokenfix /mnt/vmstore/caraxes/vfio-user-nvgpu-v615-tokenfix-d1084c0a; log "cell A rc=$?"; chk A-tokenfix; sleep 30
A_RUNS=2 A_MAX=2 A_STOP_ON_FAIL=1 "$SP/cell.sh" B-invhold /mnt/vmstore/caraxes/vfio-user-nvgpu-v615-invhold-3bad9810; log "cell B rc=$?"; chk B-invhold
C=$(ls -t $V/vfio-user-nvgpu-*combo-* 2>/dev/null | head -1)
if [[ -n $C ]]; then sleep 30; log "combo binary $C"; "$SP/cell.sh" C-combo "$C" pace; log "cell C rc=$?"; else log "no combo binary"; fi
log "done"
