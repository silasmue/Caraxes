#!/usr/bin/env bash
# Copy the gate's small text records into the repository (caraxes/runs/gate-2026-09-30/stack):
# no *.log names (git-ignored), no traces, no images. Usage: collect.sh REPO_WORKTREE
set -uo pipefail
G=/mnt/vmstore/caraxes/runs-2026-09-30/gate-stack D=$1/caraxes/runs/gate-2026-09-30/stack
mkdir -p "$D"
cp $G/legs.txt $G/legs-rerun.sh $G/ab-kick.sh $G/clean-kick.sh $G/vm10-retry.sh $G/vm10-bisect.sh $G/collect.sh "$D/"
for f in nova-norom vm10-norom vm10-slot5-refused; do [ -f $G/$f.log ] && tail -5 $G/$f.log | cut -c1-400 > "$D/$f.txt"; done
if [ -d $G/bisect ]; then mkdir -p "$D/bisect"; cp $G/bisect/results.txt "$D/bisect/"
    for r in $G/bisect/*/; do n=$(basename $r); mkdir -p "$D/bisect/$n"; for f in setup.txt results.txt guest.txt; do [ -f $r/$f ] && cp $r/$f "$D/bisect/$n/"; done; done; fi
tr -d '\000' < $G/chain.log > "$D/chain.txt"
for leg in kick-smoke ab-kick-dv2 ab-kick-stack clean-kick; do
    [ -d $G/$leg ] || continue; mkdir -p "$D/$leg"
    for f in smoke.txt stream.txt device-summary.txt up.txt down.txt config.txt; do [ -f $G/$leg/$f ] && cp $G/$leg/$f "$D/$leg/"; done
done
if [ -d $G/nova ]; then mkdir -p "$D/nova"; for f in guest-nova.txt setup.txt up.txt down.txt device-boot.txt; do [ -f $G/nova/$f ] && cp $G/nova/$f "$D/nova/"; done; fi
if [ -d $G/vm10 ]; then mkdir -p "$D/vm10"; for f in RESULT.md results.txt setup.txt up.txt down.txt guest.txt; do [ -f $G/vm10/$f ] && cp $G/vm10/$f "$D/vm10/"; done
    grep -a -E '^\[(build|texture|shading|cube|clear)\]|Score|DeviceLost|job timeout' $G/vm10/serial.log > "$D/vm10/benchmarks.txt"
    [ -d $G/vm10/results ] && { mkdir -p "$D/vm10/results"; cp $G/vm10/results/*.txt "$D/vm10/results/" 2>/dev/null; }; fi
W=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/W1-gate-stack
if [ -d $W ]; then mkdir -p "$D/windows"; for f in env.json start.json checks.txt d3d11-stages-status.txt superposition-Preset-1080pmedium-status.txt launch.log; do [ -f $W/$f ] && cp $W/$f "$D/windows/"; done
    [ -f $W/launch.log ] && mv "$D/windows/launch.log" "$D/windows/launch.txt"
    grep -a -E "FPS|Score|PASS|FAIL" $W/heaven-Quality-HIGH-Tessellation-NORMAL-status.txt | tail -5 > "$D/windows/heaven.txt"
    grep -a "config: " $W/gpu.log 2>/dev/null | cut -c1-600 > "$D/windows/config.txt"
    ls $W > "$D/windows/files.txt"; fi
if [ -d $G/sottr ]; then mkdir -p "$D/sottr"; find $G/sottr -maxdepth 2 -name '*.txt' -size -200k -exec cp {} "$D/sottr/" \; ; fi
du -sh "$D"
