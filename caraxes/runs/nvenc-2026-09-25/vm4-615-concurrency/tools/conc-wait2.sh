#!/bin/bash
# conc-wait2.sh: cuda-vulkan NOTES.md, linux-go, no vm4-615 device for 3 min, no win-busy / linux-busy, no win11 host-rm device.
N=/home/silas/git/Caraxes/caraxes/runs/nvenc-2026-09-25/vm4-615-cuda-vulkan/NOTES.md
a=vfio-user-nvgpu; P="$a --socket /mnt/vmstore/caraxes/vm4-615"; PW="$a --socket /mnt/vmstore/caraxes/win11"
free_since=0; end=$(( $(date +%s) + 4*3600 ))
while (( $(date +%s) < end )); do
  if pgrep -f -- "$P" >/dev/null; then free_since=0; else (( free_since == 0 )) && free_since=$(date +%s); fi
  win=0; pgrep -af -- "$PW" | grep -q -- '--host-rm' && win=1
  if [[ -f $N && -f /home/silas/git/Caraxes/caraxes/runs/nvenc-2026-09-25/vm4-615-flippace/NOTES.md && -e /mnt/vmstore/caraxes/linux-go && ! -e /mnt/vmstore/caraxes/win-busy && ! -e /mnt/vmstore/caraxes/linux-busy ]] && (( win == 0 && free_since > 0 )) && (( $(date +%s) - free_since >= 180 )); then echo "READY $(date +%T)"; exit 0; fi
  sleep 60
done
echo TIMEOUT; exit 1
