#!/usr/bin/env bash
# The four-guest gate of final-2026-09-30 (424a43e8): the Linux legs, each under vmlock.sh.
cd /home/silas/git/Caraxes || exit 1
G=/mnt/vmstore/caraxes/runs-2026-09-30/gate-final1 B=/mnt/vmstore/caraxes/vfio-user-nvgpu-final1-424a43e8
leg=$1
case $leg in
nova) bash /mnt/vmstore/caraxes/locks/vmlock.sh vm9 14400 timeout 1200 caraxes/runs/nova-2026-09-25/nova-boot.sh $G/nova CARAXES_SERVER=$B CARAXES_KERNEL=/mnt/vmstore/caraxes/nova-kernel/build/arch/x86/boot/bzImage CARAXES_VBIOS=/mnt/vmstore/caraxes/night-2026-09-28/synth-vbios-nova.bin CARAXES_GUEST_ABI= > $G/nova.log 2>&1; echo "nova rc=$? $(date +%T)" >> $G/legs.txt ;;
vm10) bash /mnt/vmstore/caraxes/locks/vmlock.sh vm10 14400 timeout 900 caraxes/runs/openrender-2026-09-29/openrender-boot.sh $G/vm10 CARAXES_SERVER=$B CARAXES_RESERVE_SLOT=5 CARAXES_INTR_PAGE=1 CARAXES_BAR1_PASSTHROUGH=pool CARAXES_BAR1_POOL_MIB=32 CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 > $G/vm10.log 2>&1; echo "vm10 rc=$? $(date +%T)" >> $G/legs.txt ;;
kick) bash /mnt/vmstore/caraxes/locks/vmlock.sh desktop-b 14400 timeout 1800 caraxes/runs/night-2026-09-28/kick-smoke.sh $G/kick-smoke $B > $G/kick-smoke.log 2>&1; echo "kick-smoke rc=$? $(date +%T)" >> $G/legs.txt ;;
esac
