#!/usr/bin/env bash
# The cleaned Linux B launchers (flagclean-2026-09-30 ef97efb1) on the stack binary: one kick-smoke
# after the A/B; the device must log no "no longer read" warning.
R=/mnt/vmstore/caraxes/wt-flagclean G=/mnt/vmstore/caraxes/runs-2026-09-30/gate-stack
B=/mnt/vmstore/caraxes/vfio-user-nvgpu-stack-ffec4d49
L=/mnt/vmstore/caraxes/locks/vmlock.sh
cd "$R" || exit 1
say() { echo "$(date +%T) $*" | tee -a "$G/legs.txt"; }
while kill -0 3280653 2>/dev/null; do sleep 20; done
bash $L desktop-b 3600 timeout 1800 caraxes/runs/night-2026-09-28/kick-smoke.sh $G/clean-kick $B > $G/clean-kick.log 2>&1
rc=$?
grep -a 'config: ' /mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b/gpu.log | cut -c1-400 > $G/clean-kick/config.txt
say "clean-kick rc=$rc $(grep -h 'glmark2\|RESULT' $G/clean-kick/smoke.txt 2>/dev/null | tr '\n' ' ') warnings $(grep -c 'no longer read' $G/clean-kick/config.txt)"
