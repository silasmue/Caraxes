#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# P21 prototype G (docs/caraxes/bar1-passthrough.md §8): BAR1 passthrough area of 1 MiB backed
# by a dma-buf of the reservation, measured on vm1 (1 GiB profile, slot 4) with the doorbell
# ioeventfd on in both configurations; A = no passthrough, B = CARAXES_BAR1_PASSTHROUGH=1.
set -uo pipefail
R=/home/silas/git/Caraxes; O=$R/caraxes/runs/p21-bar1-2026-09-24
source "$R/caraxes/runs/doorbell-2026-09-24/env-vm1.sh"
export CARAXES_SERVER=/mnt/vmstore/caraxes/vm2/vfio-user-nvgpu-p21
export LEA_CH=/mnt/vmstore/caraxes/vm3/ch-0005-v.sh
# P21_DOORBELL=0 keeps the doorbell trapped: with the passthrough area the device no longer
# sees the guest's GPPut writes, so the token-less ioeventfd path (watched GPPut copy) cannot
# tell which channel to ring and the guest waits forever (run B-on-with-ioeventfd-hung, rc=124
# after 900 s). Until the datamatch token pool exists, P21 needs the trapped doorbell, which
# carries the token.
if [[ ${P21_DOORBELL:-1} == 1 ]]; then export CARAXES_DOORBELL_IOEVENTFD=1; else unset CARAXES_DOORBELL_IOEVENTFD; fi
nvidia-smi -q -d MEMORY | grep -A3 'BAR1' | tr '\n' ' ' > "$O/host-bar1-before.txt"; echo >> "$O/host-bar1-before.txt"
for cfg in ${CONFIGS:-A-off B-on}; do
  D=$O/$cfg; mkdir -p "$D"
  if [[ $cfg == B-on ]]; then export CARAXES_BAR1_PASSTHROUGH=1; else unset CARAXES_BAR1_PASSTHROUGH; fi
  t0=$(date '+%Y-%m-%d %H:%M:%S')
  "$R/caraxes/tools/bench-hostwatch.sh" start "$D"
  "$R/caraxes/tools/bench-run.sh" v2-restart > "$D/up.txt" 2>&1 || { tail "$D/up.txt"; "$R/caraxes/tools/bench-hostwatch.sh" stop "$D"; continue; }
  grep -iE 'bar1 passthrough|ioeventfd|region 1' /mnt/vmstore/caraxes/$VM/gpu.log /mnt/vmstore/caraxes/$VM/ch.log 2>/dev/null | cut -c1-240 > "$D/device-start.txt"
  nvidia-smi -q -d MEMORY | grep -A3 'BAR1' | tr '\n' ' ' > "$D/host-bar1-up.txt"; echo >> "$D/host-bar1-up.txt"
  "$R/tools/caraxes-vm.sh" ssh 'mkdir -p ~/bench; cat > ~/bench/bench-trap.c' < "$R/caraxes/tools/bench-trap.c"
  "$R/tools/caraxes-vm.sh" ssh 'cd ~/bench && gcc -O2 -o bench-trap bench-trap.c && sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource1 20000 none 0x80000 && sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource1 20000 none 0x400000 && sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource0 20000 0xbb0090' | tee "$D/trap.txt"
  "$R/caraxes/tools/bench-run.sh" series --targets v2 --reps "${REPS:-2}" --metrics "${METRICS:-short-auto rl}" --out "$D" 2>&1 | cut -c1-200 | tail -4
  "$R/caraxes/tools/bench-hostwatch.sh" stop "$D"
  "$R/tools/caraxes-vm.sh" down > "$D/down.txt" 2>&1
  grep -E 'bar1 passthrough|ERROR: bar1|doorbell' /mnt/vmstore/caraxes/$VM/gpu.log | cut -c1-240 > "$D/device-summary.txt"
  grep -c ' bar1 0x000[0-9a-f]' /mnt/vmstore/caraxes/$VM/gpu-trace.log > "$D/bar1-trapped-in-first-MiB.txt" 2>/dev/null
  nvidia-smi -q -d MEMORY | grep -A3 'BAR1' | tr '\n' ' ' > "$D/host-bar1-after.txt"; echo >> "$D/host-bar1-after.txt"
  journalctl -k --since "$t0" --no-pager -g 'Xid' > "$D/host-xid.txt" 2>/dev/null
  echo "$cfg: trapped in first MiB $(cat "$D/bar1-trapped-in-first-MiB.txt"), Xid lines $(grep -c Xid "$D/host-xid.txt"); $(grep -h MSPERLAUNCH "$D"/v2/short-auto-*.txt | tr '\n' ' '); $(grep -h stage4 "$D"/v2/rl-*.txt | tr '\n' ' ')"
  cat "$D/device-summary.txt" | tail -4
done
python3 "$R/caraxes/tools/bench-summary.py" "$O/A-off/v2" "$O/B-on/v2" > "$O/summary.md" 2>&1
