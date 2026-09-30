#!/usr/bin/env bash
# vm10's vkmark cube ends in a DeviceLost (nouveau "job timeout, channel 7 killed") on dv2 (b6d01293)
# and on the stack, not on the lf16 gate (winlf-1bdf9089, 2026-09-29 19:04). Existing binaries
# between them, with the dv2 gate's vm10 command (legs-fix2.sh: the lf16 flags, dev's scripts),
# slot 4. Usage: vm10-bisect.sh NAME...   (NAME: the part after vfio-user-nvgpu-)
cd /home/silas/git/Caraxes || exit 1
G=/mnt/vmstore/caraxes/runs-2026-09-30/gate-stack/bisect; mkdir -p $G
L=/mnt/vmstore/caraxes/locks/vmlock.sh
for n in "$@"; do
    B=/mnt/vmstore/caraxes/vfio-user-nvgpu-$n
    bash $L vm10 3600 timeout 900 caraxes/runs/openrender-2026-09-29/openrender-boot.sh $G/$n CARAXES_SERVER=$B CARAXES_RESERVE_SLOT=4 CARAXES_INTR_PAGE=1 CARAXES_BAR1_PASSTHROUGH=pool CARAXES_BAR1_POOL_MIB=32 CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 > $G/$n.log 2>&1
    rc=$?
    echo "$(date +%T) $n rc=$rc $(grep -a -h -E '^\[cube\]' $G/$n/serial.log 2>/dev/null | head -1 | cut -c1-70) job-timeouts $(grep -a -c 'job timeout' $G/$n/guest.txt 2>/dev/null) pool-errors $(grep -a -c 'ERROR: bar1 pool' $G/$n/gpu.log 2>/dev/null)" | tee -a $G/results.txt
done
