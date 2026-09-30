#!/usr/bin/env bash
# Rerun of the gate's nova and vm10 legs: the first run found no rom.bin in wt-flagsrb (a git-ignored
# capture, now a symlink as in the other worktrees). Same commands as legs-stack.sh, one VM at a time.
R=/mnt/vmstore/caraxes/wt-flagsrb G=/mnt/vmstore/caraxes/runs-2026-09-30/gate-stack
B=/mnt/vmstore/caraxes/vfio-user-nvgpu-stack-ffec4d49
L=/mnt/vmstore/caraxes/locks/vmlock.sh
cd "$R" || exit 1
say() { echo "$(date +%T) $*" | tee -a "$G/legs.txt"; }
mv $G/nova $G/nova-norom 2>/dev/null; mv $G/nova.log $G/nova-norom.log 2>/dev/null
mv $G/vm10 $G/vm10-norom 2>/dev/null; mv $G/vm10.log $G/vm10-norom.log 2>/dev/null
say "rerun nova/vm10 with rom.bin linked"
bash $L vm9 7200 timeout 1200 caraxes/runs/nova-2026-09-25/nova-boot.sh $G/nova CARAXES_SERVER=$B CARAXES_KERNEL=/mnt/vmstore/caraxes/nova-kernel/build/arch/x86/boot/bzImage CARAXES_VBIOS=/mnt/vmstore/caraxes/night-2026-09-28/synth-vbios-nova.bin CARAXES_GUEST_ABI= > $G/nova.log 2>&1
say "nova rc=$? probes $(grep -c 'CARAXES-CHECK \(probe\|cycle\|reload\)' $G/nova/guest-nova.txt 2>/dev/null)"
bash $L vm10 7200 timeout 900 caraxes/runs/openrender-2026-09-29/openrender-boot.sh $G/vm10 CARAXES_SERVER=$B CARAXES_RESERVE_SLOT=5 CARAXES_DOORBELL_TOKENS=64 CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 > $G/vm10.log 2>&1
say "vm10 rc=$? $(cat $G/vm10/results.txt 2>/dev/null | tr '\n' ' ') job-timeouts $(grep -c 'job timeout' $G/vm10/guest.txt 2>/dev/null)"
say "rerun done"
