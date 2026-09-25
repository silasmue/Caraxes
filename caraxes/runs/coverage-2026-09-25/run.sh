#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# The coverage map of 2026-09-25 (docs/caraxes/coverage-map.md): the device's profile
# (CARAXES_PROFILE=1, src/profile.rs) over workloads the gates do not cover or cover thinly,
# one profile per workload (a SIGUSR2 dump after each; a workload is the difference of two
# dumps). vm2 only, the night environment (caraxes/runs/night-2026-09-24/env.sh: 610 guest,
# 1 GiB at reservation slot 3, host RM and execution, token pool 64, hybrid reactor, interrupt
# page, BAR1 pool) plus the recommended configuration (bench-run.sh header): BAR1 pool 32 MiB,
# CARAXES_BAR2_MEMFD=1, CARAXES_MSI_REARM_IOEVENTFD=1, CARAXES_ALLOC_LAZY=1.
#   run.sh overhead-off|overhead-on [label]   short-auto x5 with the profile off or on
#                                             (the plain build, target/cov/release)
#   run.sh map [label]                        the workload series with the profile on (the
#                                             build with -C instrument-coverage,
#                                             target/cov-instr/release: LLVM counters too)
#   run.sh extra [label]                      as map, for what map could not run: streams and
#                                             multiproc without the refused cuMemAllocAsync,
#                                             NVRTC with its builtins on the library path, and
#                                             the control sweep (ctlsweep-table.h beside this
#                                             script, from map's untouched controls)
# REPS (default 5) sets the short-auto repetitions of the overhead modes.
# SERVER=path overrides the binary. Results go to <mode>[-label]/ next to this script.
set -uo pipefail
MODE=${1:?usage: run.sh overhead-off|overhead-on|map|extra [label]}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
C=$HERE/$MODE${2:+-$2}
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
cd "$ROOT" || exit 1
# shellcheck source=../night-2026-09-24/env.sh
source "$ROOT/caraxes/runs/night-2026-09-24/env.sh"
night_guest 610
export CARAXES_BAR1_POOL_MIB=32 CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 CARAXES_ALLOC_LAZY=1
unset CARAXES_PROFILE CARAXES_PROFILE_OUT LLVM_PROFILE_FILE CARAXES_TRACE_ACCESSES CARAXES_TRACE_FLUSH
STORE=/mnt/vmstore/caraxes/coverage-2026-09-25
case $MODE in
    overhead-off) SERVER=${SERVER:-$ROOT/target/cov/release/vfio-user-nvgpu} ;;
    overhead-on) SERVER=${SERVER:-$ROOT/target/cov/release/vfio-user-nvgpu}; export CARAXES_PROFILE=1 ;;
    map|extra) SERVER=${SERVER:-$ROOT/target/cov-instr/release/vfio-user-nvgpu}; export CARAXES_PROFILE=1
         mkdir -p "$STORE/llvm-$(basename "$C")"
         export LLVM_PROFILE_FILE=$STORE/llvm-$(basename "$C")/device-%p.profraw ;;
    *) echo "ERROR: unknown mode $MODE" >&2; exit 2 ;;
esac
export CARAXES_SERVER=$SERVER
[[ -n ${CARAXES_PROFILE:-} ]] && export CARAXES_PROFILE_OUT=$C/profile/dev
night_vm2_free || exit 2
mkdir -p "$C/work" "$C/profile"
W=$NIGHT_W; T0=$(date '+%F %T')
night_setup "$C"
caraxes/tools/bench-hostwatch.sh start "$C"

up_ok=0
for attempt in 1 2 3 4 5 6; do
    tools/caraxes-vm.sh up >> "$C/up.txt" 2>&1 && tools/caraxes-vm.sh wait >> "$C/up.txt" 2>&1 && { up_ok=1; break; }
    tail -3 "$C/up.txt"
    tools/caraxes-vm.sh down >> "$C/up.txt" 2>&1
    if grep -qE 'NO_MEMORY|did not give|headroom' "$C/up.txt"; then
        echo "reservation refused (attempt $attempt), waiting 5 min"; sleep 300
    else
        break
    fi
done
finish() {
    caraxes/tools/bench-hostwatch.sh stop "$C"
    tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
    sleep 2
    grep -E 'profile|intr page|bar2 memfd|bar1 pool|doorbell|mirror: |shadow: |ERROR|WARN' "$W/gpu.log" | tail -60 | cut -c1-600 > "$C/device-summary.txt"
    journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
    local pid; pid=$(cat "$C/device-pid.txt" 2>/dev/null)
    echo "host Xid lines $(grep -c Xid "$C/host-xid.txt"), of the device pid ${pid:-?}: $(grep -c "pid=${pid:-none}\b" "$C/host-xid.txt")" | tee "$C/xid-summary.txt"
    echo "$MODE done $(date -Iseconds); vm2 left: $(pgrep -f /mnt/vmstore/caraxes/vm2 | wc -l)"
}
if [[ $up_ok != 1 ]]; then echo "ERROR: vm2 did not come up"; finish; exit 1; fi
cat "$W/gpu.pid" > "$C/device-pid.txt"
tools/caraxes-vm.sh probe cov >> "$C/up.txt" 2>&1; tail -1 "$C/up.txt" | cut -c1-200
caraxes/tools/bench-run.sh v2-nodes >> "$C/up.txt" 2>&1
grep -E 'profile|intr page|bar2 memfd|bar1 pool|doorbell tokens|doorbell poll|msi re-arm|lazy|reserved host VRAM|host ABI' "$W/gpu.log" | cut -c1-400 > "$C/device-start.txt"
caraxes/tools/bench-run.sh stage v2 >> "$C/up.txt" 2>&1

if [[ $MODE == overhead-* ]]; then
    BENCH_TIMEOUT=300 caraxes/tools/bench-run.sh series --targets v2 --reps "${REPS:-5}" --metrics short-auto --out "$C/work" 2>&1 | tail -2
    finish; exit 0
fi

# The map: one dump per workload boundary. marks.tsv: dump number, the workload that ended there.
DEVPID=$(cat "$C/device-pid.txt"); P=$CARAXES_PROFILE_OUT; n=0
mark() {
    n=$((n + 1)); kill -USR2 "$DEVPID"
    for _ in $(seq 100); do [[ -f $P.$n.tsv ]] && break; sleep 0.1; done
    printf '%s\t%s\t%s\t%s\n' "$n" "$1" "$(date +%s)" "$([[ -f $P.$n.tsv ]] && echo dumped || echo MISSING)" >> "$C/marks.tsv"
}
wl() {  # LABEL CMD...: run one workload, then dump
    local label=$1; shift
    echo "== $label $(date +%T)"
    timeout -k 10 900 "$@" > "$C/work/$label.txt" 2>&1; echo "# rc=$?" >> "$C/work/$label.txt"
    grep -E '^COV|GATE:|CUDA-GATE|NVML-GATE|^# rc' "$C/work/$label.txt" | tail -2 | cut -c1-220
    mark "$label"
}
g() { tools/caraxes-vm.sh ssh "bash ~/cov/covguest.sh $1"; }
GW=(tools/caraxes-vm.sh ssh)
mark boot
tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
tools/caraxes-vm.sh ssh 'mkdir -p ~/cov'
for f in caraxes/tools/covwork.c caraxes/tools/covtorch.py caraxes/runs/coverage-2026-09-25/covguest.sh; do
    tools/caraxes-vm.sh ssh "cat > ~/cov/$(basename "$f")" < "$f"
done
g build > "$C/build.txt" 2>&1; tail -1 "$C/build.txt"
g inventory > "$C/inventory.txt" 2>&1
if [[ $MODE == extra ]]; then
    for f in caraxes/tools/ctlsweep.c caraxes/runs/coverage-2026-09-25/ctlsweep-table.h; do
        tools/caraxes-vm.sh ssh "cat > ~/cov/$(basename "$f")" < "$f"
    done
    g build-sweep >> "$C/build.txt" 2>&1; tail -1 "$C/build.txt"
    mark setup
    for w in streams multiproc nvrtc ctlsweep; do
        wl "$w" "${GW[@]}" "bash ~/cov/covguest.sh $w"
    done
    tools/caraxes-vm.sh ssh 'cat ~/cov/ctlsweep.txt' > "$C/work/ctlsweep-table-out.txt" 2>&1
    finish; exit 0
fi
mark setup
wl gate-nvml tools/caraxes-gate.sh nvml
wl gate-cuda tools/caraxes-gate.sh cuda
wl bench-init env BENCH_TIMEOUT=300 caraxes/tools/bench-run.sh series --targets v2 --reps 1 --metrics init --out "$C/bench"
wl bench-short-auto env BENCH_TIMEOUT=300 caraxes/tools/bench-run.sh series --targets v2 --reps 1 --metrics short-auto --out "$C/bench"
wl bench-rl env BENCH_TIMEOUT=300 caraxes/tools/bench-run.sh series --targets v2 --reps 1 --metrics rl --out "$C/bench"
wl bench-ppo env BENCH_TIMEOUT=300 PPO_SECONDS=30 caraxes/tools/bench-run.sh series --targets v2 --reps 1 --metrics ppo --out "$C/bench"
for w in smi-q smi-dmon graphs streams managed texsurf zerocopy query torch-matmul torch-conv \
         torch-attention multiproc oom nvrtc vulkan idle kill9 illegal; do
    wl "$w" "${GW[@]}" "bash ~/cov/covguest.sh $w"
    # A guest error path must not end the series: the next workload shows whether it did.
done
wl after-errors "${GW[@]}" "bash ~/cov/covguest.sh query"
finish
