#!/usr/bin/env bash
# vm10 again: its VRAM slot 5 [0x69800000, 0xa9800000) was refused at 09:53 (the host held
# [0x69800000, 0x6ad20000) with no other VM running). Slot 4, which Linux B takes alone, instead;
# the same command otherwise. Waits for the gate chain (SotTR) to end.
R=/mnt/vmstore/caraxes/wt-flagsrb G=/mnt/vmstore/caraxes/runs-2026-09-30/gate-stack
B=/mnt/vmstore/caraxes/vfio-user-nvgpu-stack-ffec4d49
L=/mnt/vmstore/caraxes/locks/vmlock.sh
cd "$R" || exit 1
say() { echo "$(date +%T) $*" | tee -a "$G/legs.txt"; }
while kill -0 3251455 2>/dev/null; do sleep 20; done
mv $G/vm10 $G/vm10-slot5-refused 2>/dev/null; mv $G/vm10.log $G/vm10-slot5-refused.log 2>/dev/null
bash $L vm10 7200 timeout 900 caraxes/runs/openrender-2026-09-29/openrender-boot.sh $G/vm10 CARAXES_SERVER=$B CARAXES_RESERVE_SLOT=4 CARAXES_DOORBELL_TOKENS=64 CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 > $G/vm10.log 2>&1
say "vm10 (slot 4) rc=$? $(cat $G/vm10/results.txt 2>/dev/null | tr '\n' ' ') job-timeouts $(grep -c 'job timeout' $G/vm10/guest.txt 2>/dev/null)"
