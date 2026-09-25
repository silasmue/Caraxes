#!/usr/bin/env bash
# main.sh: wait for the preconditions (every 30 s, up to 2 h), then cell A and cell B.
set -u
SP=$(cd "$(dirname "$0")" && pwd -P); V=/mnt/vmstore/caraxes
N=$(dirname "$SP"); LOG=$N/main.log
log() { echo "$(date '+%F %T') $*" >> "$LOG"; }
pre() {
  [[ -e $V/linux-go ]] || { echo "no linux-go"; return 1; }
  [[ -e $V/win-busy ]] && { echo "win-busy"; return 1; }
  [[ -e $V/linux-busy ]] && { echo "linux-busy"; return 1; }
  pgrep -af 'vfio-user-nvgpu' | grep -- "--socket $V/win11" | grep -q -- '--host-rm' && { echo "win11 device with --host-rm"; return 1; }
  pgrep -af 'vfio-user-nvgpu' | grep -q -- "--socket $V/vm4-615" && { echo "vm4-615 device runs"; return 1; }
  return 0
}
log "waiting for preconditions"
ok=0
for i in $(seq 240); do
  why=$(pre) && { ok=1; break; }
  (( i % 10 == 1 )) && log "not yet: $why"
  sleep 30
done
(( ok )) || { log "preconditions not met in 2 h: $why"; exit 1; }
touch $V/linux-busy
trap 'rm -f /mnt/vmstore/caraxes/linux-busy' EXIT
log "linux-busy set; MemAvailable $(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo) MiB"
"$SP/run.sh" A-invhold "$SP/cfgA.env" A; log "cell A rc=$?"
sleep 30
avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
if (( avail < 3072 )); then log "MemAvailable $avail MiB < 3 GB: no cell B"; exit 1; fi
if journalctl -k --since "$(cat $N/A-invhold/t0)" --no-pager -g 'Xid.*: 31,' 2>/dev/null | grep -vq '^-- '; then log "Xid 31 in cell A: no cell B"; exit 1; fi
"$SP/run.sh" B-hold "$SP/cfgB.env" B; log "cell B rc=$?"
log "done"
