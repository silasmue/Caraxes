#!/usr/bin/env bash
# main.sh: wait (60 s polls, up to 4 h) until win-busy and linux-busy are gone and no vm4-615 device
# runs and MemAvailable >= 13 GB; claim linux-busy; cell C (combo), then B2 (invhold, cases b-d only).
set -u
SP=$(cd "$(dirname "$0")" && pwd -P); V=/mnt/vmstore/caraxes; B=$V/linux-busy
N=$(dirname "$SP"); LOG=$N/main2.log; ME="tokenfix-test agent"
log() { echo "$(date '+%F %T') $*" >> "$LOG"; }
pre() {
  [[ -e $V/win-busy ]] && { echo "win-busy: $(head -c 80 $V/win-busy)"; return 1; }
  [[ -e $B ]] && { echo "linux-busy: $(head -c 80 $B)"; return 1; }
  pgrep -af 'vfio-user-nvgpu' | grep -q -- "--socket $V/vm4-615" && { echo "vm4-615 device runs"; return 1; }
  a=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo); (( a < 13312 )) && { echo "MemAvailable $a MiB < 13 GB (8 GiB pin + the device's 4 GiB floor)"; return 1; }
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
"$SP/cell.sh" C-combo /mnt/vmstore/caraxes/vfio-user-nvgpu-v615-combo-3111dbf9 pace; log "cell C rc=$?"; chk C-combo; sleep 30
a=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo); (( a < 13312 )) && { log "MemAvailable $a MiB: no B rerun"; exit 0; }
A_MAX=0 "$SP/cell.sh" B2-invhold /mnt/vmstore/caraxes/vfio-user-nvgpu-v615-invhold-3bad9810; log "cell B2 rc=$?"
log "done"
