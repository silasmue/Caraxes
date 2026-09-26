#!/bin/bash
# setup.sh: vm4-615 up with the poollock build, the guest workloads built in ~/conc.
set -u
C=/tmp/claude-1000/-home-silas-git-Caraxes/43784b5f-fc25-49bd-8e67-5dbab7e19aca/scratchpad/conc
R=/home/silas/git/Caraxes; ST=$R/caraxes/runs/sottr-2026-09-24; W=/mnt/vmstore/caraxes/vm4-615
N=$R/caraxes/runs/nvenc-2026-09-25/vm4-615-concurrency; mkdir -p "$N/tools"
g() { (cd $R && VM=vm4-615 CARAXES_IP=192.168.103.24 CARAXES_DRIVER=615 timeout ${GT:-400} tools/caraxes-vm.sh ssh "$@"); }
log() { echo "$(date +%T) $*" | tee -a "$N/run.log"; }
{ date -Iseconds; nvidia-smi --query-gpu=memory.used,memory.free --format=csv; nvidia-smi -q -d MEMORY | sed -n '/BAR1/,/Free/p'; grep MemAvailable /proc/meminfo; } > "$N/host-baseline.txt"
log "up: SOTTR_SERVER=$(cat $C/server) SOTTR_NAT=0 SOTTR_SUN_ENCODER=nvenc CARAXES_MEM=8G"
(cd "$ST" && SOTTR_SERVER=$(cat $C/server) SOTTR_NAT=0 SOTTR_SUN_ENCODER=nvenc CARAXES_MEM=8G ./up-615.sh) > "$N/up-console.txt" 2>&1; rc=$?
log "up rc=$rc"
OUT=$(cat "$W/sottr-session"); ln -sfn "$OUT" "$N/session"; cp "$OUT/t0" "$N/t0"
setsid nohup bash $C/guard.sh "$OUT" > /dev/null 2>&1 < /dev/null &
[[ $rc == 0 ]] || exit 1
tar -C $C -cf - hold.sh ladder2.sh cuwork.c vkcomp.c eglwork.c comp_spv.h grun.sh rmioctl-log.c cuhold.c ladder.sh nvenc-hold.c | g 'mkdir -p ~/conc && tar -C ~/conc -xf -'
tar -C /usr/include -chf - vulkan vk_video EGL KHR GLES2 | g 'mkdir -p ~/conc/inc && tar -C ~/conc/inc -xf -'
GT=600 g 'cd ~/conc; ls ~; ls ~/nvenc 2>/dev/null | head; I=-I$HOME/conc/inc;
  gcc -O2 -o cuwork cuwork.c -ldl -lm && gcc -O2 $I -o vkcomp vkcomp.c -l:libvulkan.so.1 && gcc -O2 $I -o eglwork eglwork.c -l:libEGL.so.1 -l:libGLESv2.so.2 && gcc -O2 -shared -fPIC -o rmioctl-log.so rmioctl-log.c -ldl && gcc -O2 -o cuhold cuhold.c -ldl; echo build rc=$?;
  F=$(dirname "$(find ~ /usr/include /usr/local/include -name dynlink_loader.h 2>/dev/null | head -1)"); echo "ffnvcodec: $F"; gcc -O2 -o nvenc-hold nvenc-hold.c -I"$F" -ldl -lm; echo nvenc build rc=$?; [ -x nvenc-hold ] || { cp ~/nvenc/nvenc-probe nvenc-hold && echo "FALLBACK nvenc-hold = ~/nvenc/nvenc-probe (no idle hold: C4 invalid)"; }; ls -la' > "$N/guest-setup.txt" 2>&1
log "guest setup: $(grep -E 'rc=' "$N/guest-setup.txt" | tr '\n' ' ')"
