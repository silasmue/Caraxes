#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# cuCtxCreate, cuCtxDestroy, cuMemAlloc and cuMemFree on today's code (2026-09-29,
# docs/research/ctxcreate-2026-09-29.md). The lean compute VM of the night of 2026-09-28
# (night-2026-09-28/lean-run.sh: vm2-615, the matrix environment `wl615`, slot 0, 2 GiB) with a
# frozen device binary; lean-run.sh's steps plus a profile dump before the VM goes down.
#
# Modes:
#   census BIN LABEL [VAR=VALUE ...]  device trace on (one line per trapped access), CARAXES_PROFILE=1;
#       census-cu init x3, ctx 5 and ops 10 with the NV_PBUS_SW_SCRATCH(30) markers, then bench-cu
#       init x5 and churn 20 x3; afterwards census-table.py, rpc-cost.py and profile-rank.py on the
#       records (census-table.md, census-rpc-cost.md, profile-*.md).
#   bench BIN LABEL [VAR=VALUE ...]   device trace off, CARAXES_PROFILE=1: the clean times. bench-cu
#       init x10 and churn 20 x3, census-cu ctx 5 and ops 10 without markers.
#   native LABEL                      the same programs on the host (a CUDA client of the host RM):
#       bench-cu init x10, churn 20 x3 (target/bench/bin/bench-cu), census-cu init x3, ctx 5, ops 10.
# VAR=VALUE pass to the device and the launcher, after the lean VM's settings (for another slot:
# CARAXES_RESERVE_SLOT=5 CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024).
# The run folder is LABEL beside this script; an existing one is never overwritten.
# census-cu is built on the host from caraxes/tools/census-cu.c (bin/ beside this script's
# records, GLIBC 2.34 at most) and copied into the guest's ~/ctx; the guest's own ~/bench/bench-cu
# and ~/bench/kernels.ptx are used as they are.
set -uo pipefail
MODE=${1:?usage: run.sh census|bench BIN LABEL [VAR=VALUE ...] | native LABEL}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
TOOLS=$(cd "$HERE/../../tools" && pwd -P)
MAIN=/home/silas/git/Caraxes
CBIN=/mnt/vmstore/caraxes/ctxcreate-2026-09-29/bin
[[ -x $CBIN/census-cu ]] || { echo "ERROR: $CBIN/census-cu missing" >&2; exit 2; }

if [[ $MODE == native ]]; then
    LABEL=${2:?label}; C=$HERE/$LABEL
    [[ -e $C ]] && { echo "ERROR: $C exists" >&2; exit 2; }
    mkdir -p "$C"
    NB=$MAIN/target/bench/bin
    { head -1 /proc/driver/nvidia/version; sha256sum "$NB/bench-cu" "$CBIN/census-cu"; date -Iseconds
      nvidia-smi --query-gpu=temperature.gpu,clocks.sm,pstate,utilization.gpu,memory.used --format=csv,noheader
      pgrep -af 'vfio-user-nvgpu --socket|cloud-hypervisor' | grep -v pgrep | cut -c1-160; cut -d' ' -f1-3 /proc/loadavg; } > "$C/setup.txt" 2>&1
    W=$C/work; mkdir -p "$W"; cp "$NB/kernels.ptx" "$W/"
    ( cd "$W" && export CENSUS_PTX=$W/kernels.ptx CENSUS_CUBIN=$W/census.cubin
      "$CBIN/census-cu" ptx2cubin kernels.ptx census.cubin
      echo "=== bench-cu init"; for i in $(seq 10); do "$NB/bench-cu" init; done
      echo "=== bench-cu churn"; for i in 1 2 3; do "$NB/bench-cu" churn 20; done
      echo "=== census-cu init"; for i in 1 2 3; do "$CBIN/census-cu" init; sleep 1; done
      echo "=== census-cu ctx"; "$CBIN/census-cu" ctx 5
      echo "=== census-cu ops"; "$CBIN/census-cu" ops 10 ) > "$C/native.txt" 2>&1
    echo "native rc=$? $(grep -c '^OP' "$C/native.txt") OP lines, $(grep -c '^INIT' "$C/native.txt") INIT, $(grep -c '^CHURN' "$C/native.txt") CHURN"
    grep -hE '^(INIT|CHURN|OP) ' "$C/native.txt" > "$C/results.txt"
    exit 0
fi

BIN=${2:?device binary}; LABEL=${3:?label}; shift 3
C=$HERE/$LABEL
[[ -x $BIN ]] || { echo "ERROR: $BIN is not an executable" >&2; exit 2; }
[[ -e $C ]] && { echo "ERROR: $C exists" >&2; exit 2; }
source "$MAIN/caraxes/runs/matrix-2026-09-25/env.sh"
mx_guest wl615
FB=${LEAN_FB_MIB:-2048}; export CARAXES_FB_MIB=$FB CARAXES_GATE_FB_MIB=$FB
V2_NODES='sudo modprobe nvidia-uvm
    [ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
    [ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
    u=$(grep nvidia-uvm /proc/devices | cut -d" " -f1); sudo rm -f /dev/nvidia-uvm /dev/nvidia-uvm-tools
    sudo mknod -m 666 /dev/nvidia-uvm c $u 0; sudo mknod -m 666 /dev/nvidia-uvm-tools c $u 1'
export CARAXES_SERVER=$BIN CARAXES_LAUNCH_TIMING=1 CARAXES_PROFILE=1 CARAXES_PROFILE_OUT=$C/profile
case $MODE in
    census) export CARAXES_TRACE=1; unset CARAXES_TRACE_ACCESSES CARAXES_TRACE_FLUSH ;;
    bench) export CARAXES_TRACE=0 ;;
    *) echo "ERROR: mode census|bench|native" >&2; exit 2 ;;
esac
for kv in "$@"; do export "${kv?}"; done
mx_free || exit 2
mkdir -p "$C"

# The guest script: census-cu from the host (base64), then the mode's programs.
GS=$C/guest-script.sh
{ echo 'set +e'
  echo 'mkdir -p ~/ctx && cd ~/ctx || exit 1'
  echo "base64 -d > census-cu <<'B64'"; base64 "$CBIN/census-cu"; echo 'B64'
  cat <<'EOF'
chmod +x census-cu; cp ~/bench/kernels.ptx . || exit 1
echo "# census-cu $(sha256sum census-cu | cut -c1-16), bench-cu $(sha256sum ~/bench/bench-cu | cut -c1-16)"
./census-cu ptx2cubin kernels.ptx census.cubin || echo "ptx2cubin failed"
x() { echo "--- xid: $(sudo dmesg | grep -c 'NVRM: Xid')"; }
EOF
  case $MODE in
  census) cat <<'EOF'
R0=$(for d in /sys/bus/pci/devices/*; do [ "$(cat $d/vendor)" = 0x10de ] && echo $d/resource0; done | head -1)
echo "# marker resource $R0"
E="sudo timeout 240 env CENSUS_PTX=$PWD/kernels.ptx CENSUS_CUBIN=$PWD/census.cubin CENSUS_MARK=$R0"
echo "=== census-cu init"; for i in 1 2 3; do $E ./census-cu init; sleep 1; done
echo "=== census-cu ctx"; $E ./census-cu ctx 5
echo "=== census-cu ops"; $E ./census-cu ops 10; x
echo "=== bench-cu init"; for i in 1 2 3 4 5; do ~/bench/bench-cu init; done
echo "=== bench-cu churn"; for i in 1 2 3; do ~/bench/bench-cu churn 20; done; x
EOF
  ;;
  bench) cat <<'EOF'
export CENSUS_PTX=$PWD/kernels.ptx CENSUS_CUBIN=$PWD/census.cubin
echo "=== bench-cu init"; for i in $(seq 10); do ~/bench/bench-cu init; done
echo "=== bench-cu churn"; for i in 1 2 3; do ~/bench/bench-cu churn 20; done; x
echo "=== census-cu ctx"; timeout 240 ./census-cu ctx 5
echo "=== census-cu ops"; timeout 240 ./census-cu ops 10; x
EOF
  ;;
  esac
  echo 'sudo dmesg | grep -E "NVRM: Xid" | tail -5 | cut -c1-200'
} > "$GS"

T0=$(date '+%F %T')
mx_setup "$C"
sha256sum "$CBIN/census-cu" >> "$C/setup.txt"
# The launcher's probe (the guest's DMA table, a reset and the module load) wants a trace file;
# with the trace off it gets an empty one.
if mx_up "$C" && { [[ $MODE == census ]] || : > "$MX_W/gpu-trace.log"; } && "$LEA_TOOLS/caraxes-vm.sh" probe "ctx-$LABEL" | tail -n 2 >> "$C/up.txt" &&
    "$LEA_TOOLS/caraxes-vm.sh" ssh "$V2_NODES" >> "$C/up.txt" 2>&1; then
    L0=0; [[ $MODE == census ]] && L0=$(wc -l < "$MX_W/gpu-trace.log")
    G0=$(wc -l < "$MX_W/gpu.log")
    "$LEA_TOOLS/caraxes-vm.sh" ssh 'bash -s' < "$GS" > "$C/guest.txt" 2>&1
    echo "guest rc=$? $(grep -c '^OP' "$C/guest.txt") OP, $(grep -c '^INIT' "$C/guest.txt") INIT, $(grep -c '^CHURN' "$C/guest.txt") CHURN lines"
    # the profile of the device lifetime up to here (the exit dump follows at the down)
    kill -USR2 "$(cat "$MX_W/gpu.pid")" 2>/dev/null; sleep 2
    tail -n +"$((G0 + 1))" "$MX_W/gpu.log" > "$C/gpu-during.log"
    [[ $MODE == census ]] && tail -n +"$((L0 + 1))" "$MX_W/gpu-trace.log" > "$C/census-trace.txt"
else
    echo "up failed" | tee -a "$C/guest.txt"
fi
mx_finish "$C" "$T0"
cp "$MX_W/gpu.log" "$C/gpu.log"
grep -a "launch timing" "$MX_W/gpu.log" > "$C/launch-timing.txt"

# Analysis (host only, no VM): the census tables and the profile ranking.
if [[ -s $C/census-trace.txt ]]; then
    python3 "$TOOLS/census-table.py" "$C/census-trace.txt" --names "$C/guest.txt" --pool-mib "${CARAXES_BAR1_POOL_MIB:-32}" \
        --json "$C/census.json" --detail 20 > "$C/census-table.md" 2>&1
    python3 "$TOOLS/rpc-cost.py" "$C/census-trace.txt" > "$C/census-rpc-cost.md" 2>&1
    xz -T2 "$C/census-trace.txt"
fi
P=$(ls "$C"/profile.*.tsv 2>/dev/null | grep -v exit | tail -1)
if [[ -n $P ]]; then
    python3 "$TOOLS/profile-rank.py" "$P" --clients > "$C/profile-clients.md" 2>&1
    python3 "$TOOLS/profile-rank.py" "$P" --scope clients --top 60 > "$C/profile-clients-rank.md" 2>&1
    python3 "$TOOLS/profile-rank.py" "$P" --top 80 > "$C/profile-device-rank.md" 2>&1
    python3 "$TOOLS/profile-rank.py" "$P" --kind host,shadow --top 60 > "$C/profile-host.md" 2>&1
fi
grep -hE '^(INIT|CHURN|OP) ' "$C/guest.txt" > "$C/results.txt"
echo "$(basename "$C"): results.txt $(wc -l < "$C/results.txt") lines; host Xid lines $(grep -vc '^#' "$C/host-xid.txt")"
