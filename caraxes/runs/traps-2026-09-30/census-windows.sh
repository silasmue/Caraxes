#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Trap census session on the Windows guest (windows-plain kit, Cloud Hypervisor), 2026-09-30.
# Run inside `vmlock.sh windows ...`, only after the Windows agent wrote traps/windows-ready.
# Workloads, each between two SIGUSR2 profile dumps and /proc thread CPU snapshots: desktop
# idle, a Moonlight stream with the present probe as load, Heaven (a steady window after the
# world load). The stop follows the codec agent's session.sh: only this session's own VM.
# Usage: census-windows.sh OUT BINARY-cap [TAG]
set -uo pipefail
OUT=$1 BIN=$2 TAG=${3:-T1-census}
H=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain
IDLE_S=${IDLE_S:-60} STREAM_S=${STREAM_S:-60} HEAVEN_S=${HEAVEN_S:-60}
mkdir -p "$OUT"; cd "$K" || exit 1
log() { echo "$(date +%T) $*" | tee -a "$OUT/timeline.txt"; }
s() { timeout "${T:-60}" python3 CH-build/run44.py ssh "$@" 2>&1 | tr -d '\r'; }
MYCH="" MYDEV="" n=0
mark() {
    n=$((n + 1))
    kill -USR2 "$MYDEV" 2>/dev/null
    python3 "$H/taskstat.py" snap "$MYCH" "$OUT/ch.$n.json"
    python3 "$H/taskstat.py" snap "$MYDEV" "$OUT/dev.$n.json"
    echo "$n $(date +%s.%N) $*" >> "$OUT/marks.txt"
    log "mark $n: $*"
}
stop() {
    local cur_ch cur_dev; cur_ch=$(cat $K/ch.pid 2>/dev/null) cur_dev=$(cat $K/gpu.pid 2>/dev/null)
    if [[ -z $MYCH && -z $MYDEV ]] || [[ $(cat $K/current) != "$TAG" ]] ||
       ! [[ ( -n $MYCH && $cur_ch == "$MYCH" ) || ( -n $MYDEV && $cur_dev == "$MYDEV" ) ]]; then
        log "no VM of mine to stop (mine: ch ${MYCH:-none} dev ${MYDEV:-none}; kit: ch ${cur_ch:-none} dev ${cur_dev:-none}, current $(cat $K/current))"
    else
        log "stop ch $MYCH dev $MYDEV"; timeout 400 python3 CH-build/run44.py stop > "$OUT/stop.txt" 2>&1; tail -5 "$OUT/stop.txt"
    fi
    cp "$K/gpu.log" "$OUT/gpu.log" 2>/dev/null
    [[ -n $MYDEV ]] && echo "host Xid lines for device pid $MYDEV: $(sudo -n dmesg | grep -c "pid=$MYDEV,")" >> "$OUT/timeline.txt"
    for f in "$OUT"/profile.*.tsv; do [[ -f $f ]] && xz -T2 -f "$f"; done
}
trap stop EXIT
log "start $TAG $BIN"
CH_BIN=/mnt/vmstore/caraxes/ch-readonly-mmap/target-0008/release/cloud-hypervisor CH_SERVER=$BIN CH_SLOT=0 \
  CH_DEV_EXTRA="CARAXES_INTR_PAGE=1 CARAXES_DOORBELL_POLL=hybrid WIN_BAR1_POOL_MIB=32 WIN_CPUS=8 CARAXES_PROFILE=1 CARAXES_PROFILE_OUT=$OUT/profile ${DEV_EXTRA:-}" \
  timeout 300 python3 CH-build/run44.py start "$TAG" > "$OUT/start.txt" 2>&1; rc=$?
if [[ $(cat $K/current) == "$TAG" ]]; then
    MYCH=$(python3 -c "import json;print(json.load(open('$K/$TAG/start.json')).get('ch_pid',''))" 2>/dev/null)
    MYDEV=$(python3 -c "import json;print(json.load(open('$K/$TAG/start.json')).get('win_device_pid',''))" 2>/dev/null)
fi
(( rc == 0 )) || { tail -5 "$OUT/start.txt"; exit 1; }
cp "$K/$TAG/env.json" "$OUT/env.json" 2>/dev/null
log "started: ch $MYCH dev $MYDEV"
# Host Xid watchdog, as the Windows agent's session.sh: a host line of this device ends the run.
( while kill -0 $$ 2>/dev/null; do sleep 5; if [ "$(sudo -n dmesg | grep -c "pid=$MYDEV,")" != 0 ]; then echo "HOST LINE OF pid $MYDEV at $(date +%T): stopping" >> "$OUT/timeline.txt"; kill -TERM $$; break; fi; done ) &
for i in $(seq 1 90); do s '(Get-Process explorer -ErrorAction SilentlyContinue | Measure-Object).Count' 2>/dev/null | grep -q '^[1-9]' && break; sleep 5; done
log "desktop: explorer $(s '(Get-Process explorer -ErrorAction SilentlyContinue | Measure-Object).Count' | tail -1)"
sleep 30

mark "idle desktop start"
sleep "$IDLE_S"
mark "idle desktop end"

# The stream: Sunshine as installed (its encoder unchanged), the present probe (60 fps
# windowed) as load, Moonlight on the host.
if s 'Get-Process sunshine -ErrorAction SilentlyContinue | Measure-Object | ForEach-Object Count' | grep -q '^[1-9]'; then
    T=120 timeout 120 python3 CH-build/run44.py upack present-probe.ps1 -Seconds $((STREAM_S + 90)) > "$OUT/present-start.txt" 2>&1
    sleep 5
    setsid nohup env WAYLAND_DISPLAY=wayland-1 DISPLAY=:0 XDG_RUNTIME_DIR=/run/user/1000 LANG=C.UTF-8 /usr/bin/moonlight stream 192.168.107.2 Desktop --resolution 1920x1080 --fps 60 --bitrate 20000 --display-mode windowed --absolute-mouse --video-decoder auto > "$OUT/moonlight.log" 2>&1 < /dev/null &
    MP=$!
    for i in $(seq 1 40); do grep -q "Received first video packet" "$OUT/moonlight.log" 2>/dev/null && break; kill -0 $MP 2>/dev/null || break; sleep 1; done
    log "moonlight pid $MP first video: $(grep -c 'Received first video packet' "$OUT/moonlight.log")"
    sleep 5
    mark "stream start"
    sleep "$STREAM_S"
    mark "stream end"
    kill "$MP" 2>/dev/null; sleep 3
    grep -E "Received first video packet|Video stream is|frame rate|Host processing latency|Average network latency|Frames dropped" "$OUT/moonlight.log" | tail -8 >> "$OUT/timeline.txt"
    T=60 timeout 60 python3 CH-build/run44.py upack present-probe.ps1 status -Seconds $((STREAM_S + 90)) 2>&1 | tail -6 >> "$OUT/timeline.txt"
else
    log "no sunshine.exe: stream skipped"
fi
sleep 20

# Heaven 4.0 (the Windows agent's heaven.ps1): a steady window after the world load.
PACK_WAIT_S=1200 timeout 300 python3 CH-build/run44.py upack heaven.ps1 -Quality HIGH -Tessellation NORMAL > "$OUT/heaven-start.txt" 2>&1
log "heaven started: $(tail -1 "$OUT/heaven-start.txt")"
sleep 90
if ! grep -q 'flr: ' gpu-trace.log 2>/dev/null; then
    mark "heaven start"
    sleep "$HEAVEN_S"
    mark "heaven end"
fi
for i in $(seq 1 40); do
    st=$(timeout 30 python3 CH-build/run44.py upack heaven.ps1 status -Quality HIGH -Tessellation NORMAL 2>&1)
    echo "$st" | grep -qE 'PROBE_EXIT|PROBE_TIMEOUT' && break
    grep -q 'flr: ' gpu-trace.log 2>/dev/null && { log "FLR during heaven"; break; }
    sleep 10
done
echo "$st" | grep -E 'FPS|Frames|Time|PASS|FAIL' >> "$OUT/timeline.txt"
mark "after heaven"
log "FLRs: $(grep -c 'flr: ' gpu-trace.log 2>/dev/null)"
