#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Design H, the BAR1 pool (docs/caraxes/bar1-pool.md §6): vm2 at slot 0 with the 2 GiB profile,
# main d3c15ad frozen, the 1 MiB area (mode 1) against the 64 MiB pool (mode pool); token pool
# and spin reactor on in both; copies, rlprobe, short kernel with 5 reps and PPO with 2.
set -uo pipefail
R=/home/silas/git/Caraxes; M=$R/caraxes/runs/bar1-pool-2026-09-24; cd "$R"
source "$R/caraxes/runs/mapping-2026-09-24/env.sh"
export CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-poolh-d3c15ad LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=spin CARAXES_BAR1_POOL_MIB=64
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_BAR1_POOL_WARM
for mode in ${MODES:-1 pool}; do
  export CARAXES_BAR1_PASSTHROUGH=$mode; D=$M/$mode; mkdir -p "$D"
  caraxes/tools/bench-hostwatch.sh start "$D"
  nvidia-smi -q -d MEMORY | grep -A3 BAR1 > "$D/host-bar1-before.txt"
  caraxes/tools/bench-run.sh v2-restart > "$D/up.txt" 2>&1 || { tail -3 "$D/up.txt"; caraxes/tools/bench-hostwatch.sh stop "$D"; continue; }
  grep -E 'bar1 (passthrough|pool)' /mnt/vmstore/caraxes/vm2/gpu.log | cut -c1-400 > "$D/device-start.txt"
  nvidia-smi -q -d MEMORY | grep -A3 BAR1 > "$D/host-bar1-up.txt"
  caraxes/tools/bench-run.sh stage v2 >> "$D/up.txt" 2>&1
  tools/caraxes-vm.sh ssh 'cat > ~/bench/bench-trap.c' < caraxes/tools/bench-trap.c
  tools/caraxes-vm.sh ssh 'cd ~/bench && gcc -O2 -o bench-trap bench-trap.c && sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource1 20000 none 0x80000 && sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource1 20000 none 0x2000000 && sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource1 20000 none 0x8000000' > "$D/trap.txt"
  V2_RESTART=1 caraxes/tools/bench-run.sh series --targets v2 --reps "${REPS:-5}" --metrics "copy rl short-auto" --out "$D" 2>&1 | tail -3
  PPO_SECONDS=60 caraxes/tools/bench-run.sh series --targets v2 --reps 2 --metrics ppo --out "$D/ppo" 2>&1 | tail -2
  caraxes/tools/bench-hostwatch.sh stop "$D"
  tools/caraxes-vm.sh down > "$D/down.txt" 2>&1
  grep -E 'bar1 (passthrough|pool)|ERROR: bar1' /mnt/vmstore/caraxes/vm2/gpu.log | cut -c1-600 > "$D/device-summary.txt"
  grep -c ' bar1 0x0[0-3][0-9a-f]\{6\} ' /mnt/vmstore/caraxes/vm2/gpu-trace.log > "$D/bar1-trapped-in-first-64MiB.txt"
  nvidia-smi -q -d MEMORY | grep -A3 BAR1 > "$D/host-bar1-after.txt"
  echo "$mode: $(grep TRAP "$D/trap.txt" | grep -o 'roff=[0-9a-fx]* .*read_p50_us=[0-9.]*' | sed 's/value=[^ ]* write_p50_us=[^ ]* write_p99_us=[^ ]* //' | tr '\n' ';') trapped<64MiB $(cat "$D/bar1-trapped-in-first-64MiB.txt"); $(tail -1 "$D/device-summary.txt" | cut -c1-200)"
done
journalctl -k --since "-3h" --no-pager -g Xid > "$M/host-xid.txt" 2>/dev/null
python3 caraxes/tools/bench-summary.py "$M/1/v2" "$M/pool/v2" > "$M/summary.md"
python3 caraxes/tools/bench-summary.py "$M/1/ppo/v2" "$M/pool/ppo/v2" > "$M/summary-ppo.md"
echo "bar1 pool series done $(date -Iseconds)"
