#!/usr/bin/env bash
# main.sh: wait (every 30 s, up to 3 h) until linux-busy is gone or names nobody and no vm4-615
# device runs, claim linux-busy, then cell A-defer and cell B-sync (both mode F).
set -u
SP=$(cd "$(dirname "$0")" && pwd -P); V=/mnt/vmstore/caraxes; B=$V/linux-busy
N=$(dirname "$SP"); LOG=$N/main.log; ME="flippace-test agent"
log() { echo "$(date '+%F %T') $*" >> "$LOG"; }
pre() {
  [[ -s $B ]] && grep -q '[^[:space:]]' "$B" && { echo "linux-busy: $(head -c 80 "$B")"; return 1; }
  pgrep -af 'vfio-user-nvgpu' | grep -q -- "--socket $V/vm4-615" && { echo "vm4-615 device runs"; return 1; }
  return 0
}
log "waiting for linux-busy"
ok=0
for i in $(seq 360); do
  why=$(pre) && { ok=1; break; }
  (( i % 10 == 1 )) && log "not yet: $why"
  sleep 30
done
(( ok )) || { log "not free in 3 h: $why"; exit 1; }
echo "$ME $(date -Iseconds)" > "$B"
trap 'grep -q "^flippace-test agent" /mnt/vmstore/caraxes/linux-busy 2>/dev/null && rm -f /mnt/vmstore/caraxes/linux-busy' EXIT
log "linux-busy claimed: $(cat $B); MemAvailable $(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo) MiB"
"$SP/run.sh" A-defer "$SP/cfgA.env" F; log "cell A rc=$?"
sleep 30
avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
if (( avail < 3072 )); then log "MemAvailable $avail MiB < 3 GB: no cell B"; exit 1; fi
if journalctl -k --since "$(cat $N/A-defer/t0)" --no-pager -g 'Xid.*: 31,' 2>/dev/null | grep -vq '^-- '; then log "Xid 31 in cell A: no cell B"; exit 1; fi
"$SP/run.sh" B-sync "$SP/cfgB.env" F; log "cell B rc=$?"
log "done"
