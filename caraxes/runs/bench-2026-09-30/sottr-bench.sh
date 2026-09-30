#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# One SotTR 1080p benchmark run for a device A/B (2026-09-29): sottr-bench-6g.sh with the display
# reserve on (-1), 4 GiB, 12 GB guest RAM and CARAXES_PROFILE=1, plus benchmark-window evidence:
# when the guest monitor shows the benchmark rendering (the game runs, VRAM >= 3500 MiB and
# gpu >= 3 % in three samples in a row),
# a SIGUSR2 profile dump, a 20 s `perf record -e cpu-clock -a -g` in the guest, and a second
# dump; at the end a graceful `down` (the exit summary) and gpu.log copied into OUT.
# The benchmark's copy of caraxes/runs/dmem-2026-09-29/sottr-ab.sh: CARAXES_PROFILE=0, so no
# profile dumps and no guest perf record (the loop sees no profile-on line).
# Usage: sottr-bench.sh OUT BINARY [VAR=VALUE ...]   (the VARs go to the device, e.g.
#        NIGHT_BAR1_WINDOWS=32 NIGHT_BAR1_WINDOW_MIB=1)
set -uo pipefail
OUT=$1 BIN=$2; shift 2
for kv in "$@"; do export "${kv?}"; done
H=$(cd "$(dirname "$(readlink -f "$0")")/../dmem-2026-09-29" && pwd -P)
W=$H/desktop-b-dmem-6g.sh
G=/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b-dmem
export DMEM_KERNEL=stock
mkdir -p "$OUT"
log() { echo "$(date +%T) $*" | tee -a "$OUT/ab.txt"; }
gssh() { timeout "${T:-60}" "$W" ssh "$@" 2>&1 | grep -v "unable to resolve"; }

SOTTR_MEM=12G SOTTR_FB_MIB=4096 SOTTR_VRAM_FLOOR_MIB=1024 DMEM_CAP=off SOTTR_DISPLAY_RESERVE_MIB=-1 \
    CARAXES_PROFILE=0 \
    timeout 2900 "$H/sottr-bench-6g.sh" "$OUT/bench" "$BIN" > "$OUT/bench.log" 2>&1 &
BP=$!
log "bench pid $BP, binary $BIN, extra: $*"

hits=0 done_dumps=0
while kill -0 "$BP" 2>/dev/null; do
    sleep 10
    (( done_dumps )) && continue
    m=$(T=20 gssh 'tail -1 /tmp/sottr-mon.log 2>/dev/null')
    g=$(grep -o 'gpu=[0-9]*' <<< "$m" | cut -d= -f2); v=$(grep -o 'vram=[0-9]*' <<< "$m" | cut -d= -f2)
    # the benchmark: the game runs, its VRAM is loaded and the GPU works (the desktop and
    # Sunshine alone show ~7 % before the game; loading shows 0-1 %)
    if [[ $m == *game=[0-9]* && ${v:-0} -ge 3500 && ${g:-0} -ge 3 ]]; then hits=$((hits + 1)); else hits=0; fi
    (( hits < 3 )) && continue
    if ! grep -q "profile on (CARAXES_PROFILE=1)" "$G/gpu.log"; then log "no profile-on line: no dumps"; done_dumps=1; continue; fi
    kill -USR2 "$(cat "$G/gpu.pid")" && log "dump A (benchmark rendering, gpu=$g)"
    T=60 gssh 'sudo timeout 25 perf record -e cpu-clock -a -g -o /tmp/ab-perf.data -- sleep 20 >/dev/null 2>&1; sudo perf report -i /tmp/ab-perf.data --stdio --sort comm,dso --no-children 2>/dev/null | head -60' > "$OUT/guest-perf-comm.txt"
    kill -USR2 "$(cat "$G/gpu.pid")" && log "dump B"
    done_dumps=1
done
wait "$BP"; log "bench rc=$?"
timeout 240 "$W" down > /dev/null 2>&1; log "down"
cp "$G/gpu.log" "$OUT/gpu-exit.log"
grep -iE "average fps|max fps|min fps" "$OUT/bench/benchmark-result.txt" 2>/dev/null | head -3 | tee -a "$OUT/ab.txt"
