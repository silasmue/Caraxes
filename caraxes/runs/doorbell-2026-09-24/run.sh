#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Doorbell ioeventfd on the GPU path (patch 0005 plus the token-less pending scan, device
# 3bcfb35): vm2 at reservation slot 0 in the mapping package's configuration, the patched
# Cloud Hypervisor (v53.0 + 0001-0005), A = flag off (trapped doorbell), B = flag on.
# Per configuration: restart, bench-trap on the doorbell register, then the harness metrics.
set -uo pipefail
R=/home/silas/git/Caraxes; M=$R/caraxes/runs/mapping-2026-09-24; O=$R/caraxes/runs/doorbell-2026-09-24
source "${DOORBELL_ENV:-$M/env.sh}"
export CARAXES_SERVER=/mnt/vmstore/caraxes/vm2/vfio-user-nvgpu-doorbell
O=$O${DOORBELL_SUFFIX:-}
export LEA_CH=/mnt/vmstore/caraxes/vm3/ch-0005-v.sh
METRICS=${METRICS:-"short-auto launch-auto rl"}
for cfg in ${CONFIGS:-A-off B-on}; do
  D=$O/$cfg; mkdir -p "$D"
  if [[ $cfg == B-on ]]; then export CARAXES_DOORBELL_IOEVENTFD=1; else unset CARAXES_DOORBELL_IOEVENTFD; fi
  t0=$(date '+%Y-%m-%d %H:%M:%S')
  "$R/caraxes/tools/bench-hostwatch.sh" start "$D"
  "$R/caraxes/tools/bench-run.sh" v2-restart > "$D/up.txt" 2>&1 || { tail "$D/up.txt"; exit 1; }
  [[ ${STAGE:-0} == 1 ]] && "$R/caraxes/tools/bench-run.sh" stage v2 > "$D/stage.txt" 2>&1
  grep -i 'ioeventfd\|region I/O' /mnt/vmstore/caraxes/$VM/ch.log > "$D/ch-ioeventfd.txt" 2>/dev/null
  "$R/tools/caraxes-vm.sh" ssh 'mkdir -p ~/bench; cat > ~/bench/bench-trap.c' < "$R/caraxes/tools/bench-trap.c"
  "$R/tools/caraxes-vm.sh" ssh 'cd ~/bench && gcc -O2 -o bench-trap bench-trap.c && sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource0 20000 0xbb0090' | tee "$D/trap.txt"
  "$R/caraxes/tools/bench-run.sh" series --targets v2 --reps "${REPS:-3}" --metrics "$METRICS" --out "$D" 2>&1 | cut -c1-200 | tail -5
  "$R/caraxes/tools/bench-hostwatch.sh" stop "$D"
  T=/mnt/vmstore/caraxes/$VM/gpu-trace.log
  grep -o 'doorbell (ioeventfd, no token): [0-9]* of [0-9]* recorded channel(s) pending [^,]*, scan [0-9]* us' "$T" \
    | awk '{p+=$5; n+=$7; s+=$(NF-1); c++} END {if (c) printf "wakeups %d, pending per wakeup %.2f, channels %.1f, scan us mean %.2f\n", c, p/c, n/c, s/c; else print "no ioeventfd wakeups"}' > "$D/scan.txt"
  grep -c ' W bar0 0x00bb0090' "$T" > "$D/trapped-doorbells.txt" 2>/dev/null
  journalctl -k --since "$t0" --no-pager -g 'Xid' > "$D/host-xid.txt" 2>/dev/null
  echo "$cfg: $(cat "$D/scan.txt"), trapped doorbell writes $(cat "$D/trapped-doorbells.txt"), host Xid lines $(wc -l < "$D/host-xid.txt")"
done
python3 "$R/caraxes/tools/bench-summary.py" "$O/A-off/v2" "$O/B-on/v2" > "$O/summary.md" 2>&1
tail -30 "$O/summary.md"
