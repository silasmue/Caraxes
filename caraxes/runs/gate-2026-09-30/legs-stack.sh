#!/usr/bin/env bash
# The final four-guest gate of 2026-09-30 on the stack head flags-rb-2026-09-30 (ffec4d49), one VM
# at a time, with the scripts of the stack itself (repo-relative launchers). Usage: legs-stack.sh
R=/mnt/vmstore/caraxes/wt-flagsrb G=/mnt/vmstore/caraxes/runs-2026-09-30/gate-stack
B=/mnt/vmstore/caraxes/vfio-user-nvgpu-stack-ffec4d49 K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-stack-cap
L=/mnt/vmstore/caraxes/locks/vmlock.sh
cd "$R" || exit 1
say() { echo "$(date +%T) $*" | tee -a "$G/legs.txt"; }
say "start: $(sha256sum $B | cut -c1-16) $B"
# 1. Linux B 615 kick-smoke: the smoke suite and a Moonlight stream (Sunshine on nvenc).
bash $L desktop-b 7200 timeout 1800 caraxes/runs/night-2026-09-28/kick-smoke.sh $G/kick-smoke $B > $G/kick-smoke.log 2>&1
say "kick-smoke rc=$? $(grep -h 'RESULT' $G/kick-smoke/smoke.txt 2>/dev/null | tr '\n' ' ') $(grep -oiE 'video (format|codec)[^,]*|HEVC|H\.?265|H\.?264' $G/kick-smoke/moonlight.log 2>/dev/null | sort -u | tr '\n' ' ')"
# 2. nova 5-probe (pure emulation, the synthetic VBIOS).
bash $L vm9 7200 timeout 1200 caraxes/runs/nova-2026-09-25/nova-boot.sh $G/nova CARAXES_SERVER=$B CARAXES_KERNEL=/mnt/vmstore/caraxes/nova-kernel/build/arch/x86/boot/bzImage CARAXES_VBIOS=/mnt/vmstore/caraxes/night-2026-09-28/synth-vbios-nova.bin CARAXES_GUEST_ABI= > $G/nova.log 2>&1
say "nova rc=$? probes $(grep -c 'CARAXES-CHECK \(probe\|cycle\|reload\)' $G/nova/guest-nova.txt 2>/dev/null)"
# 3. nouveau + NVK vm10 R1-R8: the flags branch's minimal env (the rest are defaults now).
bash $L vm10 7200 timeout 900 caraxes/runs/openrender-2026-09-29/openrender-boot.sh $G/vm10 CARAXES_SERVER=$B CARAXES_RESERVE_SLOT=5 CARAXES_DOORBELL_TOKENS=64 CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 > $G/vm10.log 2>&1
say "vm10 rc=$? $(cat $G/vm10/results.txt 2>/dev/null | tr '\n' ' ') job-timeouts $(grep -c 'job timeout' $G/vm10/guest.txt 2>/dev/null)"
# 4. Windows: stages, Heaven, Superposition, one reboot (the reconnect), the minimal env.
while [ ! -x "$K" ]; do sleep 20; done
bash $L windows 7200 /mnt/vmstore/caraxes/runs-2026-09-30/win/session3.sh W1-gate-stack "$K" sleep=120 config arm stages heaven superposition reboot clients refused events
say "windows rc=$?"
# 5. Linux SotTR once, frame check as information.
bash $L desktop-b-dmem 7200 timeout 2400 "$R/caraxes/runs/dmem-2026-09-29/sottr-ab.sh" $G/sottr $B > $G/sottr.log 2>&1
say "sottr rc=$? $(grep -iE 'average fps' $G/sottr/bench/benchmark-result.txt 2>/dev/null | head -1 | tr -d '\r\t')"
for f in $G/sottr/bench/guest-shots/*.png; do [ -f "$f" ] && "$R/caraxes/runs/gate-2026-09-30/frame-check.sh" "$f" | tail -1 | sed "s|^|frame $(basename $f): |" | tee -a "$G/legs.txt"; done
say "done"
