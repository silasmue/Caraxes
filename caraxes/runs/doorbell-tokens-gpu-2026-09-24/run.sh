#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Zero-trap submission on the GPU path: P21 area plus the datamatch token pool (docs/caraxes/
# doorbell-token-pool.md, "GPU measurement"), vm1 with the 1 GiB profile at slot 4, Cloud
# Hypervisor 0001-0006. A: P21 with the trapped doorbell; C: P21 with the pool (64 tokens).
set -uo pipefail
R=/home/silas/git/Caraxes; O=$R/caraxes/runs/doorbell-tokens-gpu-2026-09-24
source "$R/caraxes/runs/doorbell-2026-09-24/env-vm1.sh"
export CARAXES_SERVER=/mnt/vmstore/caraxes/vm1/vfio-user-nvgpu-tokens
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_BAR1_PASSTHROUGH=1; unset CARAXES_DOORBELL_IOEVENTFD
for cfg in ${CONFIGS:-A-p21-trapped C-p21-tokens}; do
  D=$O/$cfg; mkdir -p "$D"
  if [[ $cfg == C-* ]]; then export CARAXES_DOORBELL_TOKENS=64; else unset CARAXES_DOORBELL_TOKENS; fi
  t0=$(date '+%Y-%m-%d %H:%M:%S')
  "$R/caraxes/tools/bench-hostwatch.sh" start "$D"
  "$R/caraxes/tools/bench-run.sh" v2-restart > "$D/up.txt" 2>&1 || { tail "$D/up.txt"; "$R/caraxes/tools/bench-hostwatch.sh" stop "$D"; continue; }
  grep -iE 'bar1 passthrough|doorbell tokens|ioeventfd' /mnt/vmstore/caraxes/$VM/gpu.log /mnt/vmstore/caraxes/$VM/ch.log | cut -c1-240 > "$D/device-start.txt"
  "$R/caraxes/tools/bench-run.sh" series --targets v2 --reps "${REPS:-3}" --metrics "${METRICS:-short-auto rl}" --out "$D" 2>&1 | cut -c1-200 | tail -4
  "$R/tools/caraxes-vm.sh" ssh 'mkdir -p ~/bench; cat > ~/bench/bench-trap.c' < "$R/caraxes/tools/bench-trap.c"
  "$R/tools/caraxes-vm.sh" ssh 'cd ~/bench && gcc -O2 -o bench-trap bench-trap.c && sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource0 20000 0xbb0090 0 0x12345 && sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource0 20000 0xbb0090 0 0xca70003f' | tee "$D/trap.txt"
  "$R/caraxes/tools/bench-hostwatch.sh" stop "$D"
  "$R/tools/caraxes-vm.sh" down > "$D/down.txt" 2>&1
  grep -E 'bar1 passthrough|ERROR: bar1|doorbell' /mnt/vmstore/caraxes/$VM/gpu.log | cut -c1-600 > "$D/device-summary.txt"
  grep -c ' bar1 0x000[0-9a-f]' /mnt/vmstore/caraxes/$VM/gpu-trace.log > "$D/bar1-trapped-in-first-MiB.txt"
  grep 'W bar0 0x00bb0090' /mnt/vmstore/caraxes/$VM/gpu-trace.log | grep -vc ' 0x12345 ' > "$D/doorbell-trapped-except-bench.txt"
  journalctl -k --since "$t0" --no-pager -g 'Xid' > "$D/host-xid.txt"
  echo "$cfg: bar1 first MiB trapped $(cat "$D/bar1-trapped-in-first-MiB.txt"), doorbells trapped (except bench) $(cat "$D/doorbell-trapped-except-bench.txt"), Xid $(grep -c Xid "$D/host-xid.txt"); $(grep -h MSPERLAUNCH "$D"/v2/short-auto-*.txt | tr '\n' ' '); $(grep -h stage4 "$D"/v2/rl-*.txt | tr '\n' ' ')"
done
python3 "$R/caraxes/tools/bench-summary.py" "$O/A-p21-trapped/v2" "$O/C-p21-tokens/v2" > "$O/summary.md" 2>&1
