#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Workload gate ("verrücktere Kernels", docs/caraxes/workload-gate.md): real workloads on top of
# the microbenchmarks, native against v2 (and v1 while the host runs 610.57.04), interleaved,
# restart per rep, host watch and Xid capture, summary.md at the end.
#
#   run.sh [--targets "native v2"] [--reps 3] [--full] [--metrics "..."] [--out DIR]
#
# Metrics (default): short-auto rl ppo-full llama llama-nograph transformer convnet. --full sets the
# 60 s workloads (transformer, convnet) to 420 s and llama-bench to 20 reps; ppo-full is 420 s
# either way (PPO_FULL_SECONDS). Duration, native + v2, 3 reps: about 70 min default, about
# 2 h 30 min with --full.
# v2: the closing configuration (caraxes/runs/mapping-2026-09-24/env.sh: vm2, slot 0, 2 GiB
# profile; the P21 area, the doorbell token pool, the pinned spin reactor on Cloud Hypervisor
# 0001-0006). The device binary is the newest frozen
# /mnt/vmstore/caraxes/vfio-user-nvgpu-closing-610-* unless CARAXES_SERVER is set (for example
# to target/release/vfio-user-nvgpu, a fresh build from main). The device selects its host
# layouts from the running driver (hostabi.rs, in the closing build since e36f582), so the
# frozen binary serves a 615 host as well (cutover-615.md (c)).
# v1 forwards ioctls verbatim and needs a guest on the host's driver version: it is dropped
# unless the host runs 610.57.04 (FORCE_V1=1 overrides; cutover-615.md (e)).
# llama needs caraxes/tools/build-llama.sh all beforehand; without its build it is dropped.
# Nothing here changes host modules or their parameters; host GPU access is as an RM client
# (the workloads, nvidia-smi queries of the host watch).
set -uo pipefail
R=$(cd "$(dirname "$(readlink -f "$0")")/../../.." && pwd -P)
cd "$R"
targets="native v2" reps=3 full=0 out="" metrics="short-auto rl ppo-full llama llama-nograph transformer convnet"
while [[ $# -gt 0 ]]; do
    case $1 in
        --targets) targets=$2; shift 2 ;; --reps) reps=$2; shift 2 ;; --full) full=1; shift ;;
        --metrics) metrics=$2; shift 2 ;; --out) out=$2; shift 2 ;;
        *) sed -n '2,/^set -uo/p' "$0" | sed '$d'; exit 2 ;;
    esac
done
F=${out:-$R/caraxes/runs/workload-2026-09-24/$([[ $full == 1 ]] && echo full || echo gate)-$(date +%H%M)}
mkdir -p "$F"
T0=$(date '+%Y-%m-%d %H:%M:%S')
note() { echo "# $*" | tee -a "$F/setup.txt"; }

export PPO_FULL_SECONDS=${PPO_FULL_SECONDS:-420}
if [[ $full == 1 ]]; then
    export WORKLOAD_SECONDS=${WORKLOAD_SECONDS:-420} LLAMA_REPS=${LLAMA_REPS:-20}
else
    export WORKLOAD_SECONDS=${WORKLOAD_SECONDS:-60} LLAMA_REPS=${LLAMA_REPS:-5}
fi
# The longest run (ppo-full or a --full workload) plus torch import and cuDNN autotuning.
export BENCH_TIMEOUT=${BENCH_TIMEOUT:-$(( (PPO_FULL_SECONDS > WORKLOAD_SECONDS ? PPO_FULL_SECONDS : WORKLOAD_SECONDS) + 480 ))}

hostdrv=$(awk '/NVRM version/ {for (i=1;i<=NF;i++) if ($i ~ /^[0-9]+\.[0-9]+\.[0-9]+$/) {print $i; exit}}' /proc/driver/nvidia/version)
{ echo "# workload gate $(date -Iseconds) targets=[$targets] reps=$reps full=$full metrics=[$metrics]"
  echo "# host driver $hostdrv"; head -1 /proc/driver/nvidia/version; } > "$F/setup.txt"

# v1 only on a 610 host (its guest image and backend are 610.57.04).
if [[ " $targets " == *" v1 "* && $hostdrv != 610.57.04 && ${FORCE_V1:-0} != 1 ]]; then
    note "v1 dropped: host driver $hostdrv, the v1 guest and vhost-user-nvrm are 610.57.04 (cutover-615.md (e))"
    targets=$(tr ' ' '\n' <<< "$targets" | grep -vx v1 | tr '\n' ' ')
fi
# llama only with its build and model.
LLAMA_DIST=${LLAMA_DIST:-/mnt/vmstore/caraxes/llama/dist} LLAMA_GGUF=${LLAMA_GGUF:-/mnt/vmstore/caraxes/models/llama-gate.gguf}
export LLAMA_DIST LLAMA_GGUF
if [[ " $metrics " == *" llama"* && ! ( -x $LLAMA_DIST/llama-bench && -s $LLAMA_GGUF ) ]]; then
    note "llama and llama-nograph dropped: no $LLAMA_DIST/llama-bench or $LLAMA_GGUF (caraxes/tools/build-llama.sh all)"
    metrics=$(tr ' ' '\n' <<< "$metrics" | grep -vE '^llama(-nograph)?$' | tr '\n' ' ')
fi
[[ -f $LLAMA_DIST/BUILD-INFO.txt ]] && sed 's/^/# llama /' "$LLAMA_DIST/BUILD-INFO.txt" >> "$F/setup.txt"
[[ -f $LLAMA_GGUF ]] && echo "# model $(readlink -f "$LLAMA_GGUF") sha256 $(cut -d' ' -f1 "$(readlink -f "$LLAMA_GGUF").sha256" 2>/dev/null)" >> "$F/setup.txt"

if [[ " $targets " == *" v2 "* ]]; then
    pre=${CARAXES_SERVER:-}
    source "$R/caraxes/runs/mapping-2026-09-24/env.sh"
    # env.sh defaults CARAXES_SERVER to the mapping build; the gate wants the newest closing
    # build unless the caller named one (CARAXES_SERVER set before this script).
    export CARAXES_SERVER=${pre:-$(ls -t /mnt/vmstore/caraxes/vfio-user-nvgpu-closing-610-* 2>/dev/null | head -1)}
    [[ -x $CARAXES_SERVER ]] || { note "no device binary $CARAXES_SERVER"; exit 1; }
    export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
    export CARAXES_BAR1_PASSTHROUGH=1 CARAXES_DOORBELL_TOKENS=64
    export CARAXES_DOORBELL_POLL=${CARAXES_DOORBELL_POLL:-spin}
    unset CARAXES_DOORBELL_IOEVENTFD
    { sha256sum "$CARAXES_SERVER" /mnt/vmstore/caraxes/vm1/cloud-hypervisor-0006; env | grep -E '^CARAXES_|^LEA_CH' | sort; } >> "$F/setup.txt"
fi
{ git -C "$R" rev-parse HEAD; env | grep -E '^(PPO_FULL_SECONDS|WORKLOAD_SECONDS|LLAMA_REPS|LLAMA_ARGS|TRANSFORMER_ARGS|CONVNET_ARGS|BENCH_TIMEOUT)=' | sort; } >> "$F/setup.txt"
note "targets now [$targets] metrics [$metrics]"
pgrep -af 'vfio-user-nvgpu|vhost-user-nvrm|cloud-hypervisor' | grep -v pgrep > "$F/other-vms-at-start.txt"

# Host watch; for v2 a host slower than 5 s per nvidia-smi query stops the device (P20 lesson).
if [[ " $targets " == *" v2 "* ]]; then
    SLOW=${SLOW:-5} KILL=${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM/gpu.pid caraxes/tools/bench-hostwatch.sh start "$F"
else
    caraxes/tools/bench-hostwatch.sh start "$F"
fi

caraxes/tools/bench-run.sh stage native > "$F/native-up.txt" 2>&1
if [[ " $targets " == *" v2 "* ]]; then
    caraxes/tools/bench-run.sh v2-restart > "$F/v2-up.txt" 2>&1 || { note "v2 did not come up (v2-up.txt)"; caraxes/tools/bench-hostwatch.sh stop "$F"; exit 1; }
    grep -iE 'bar1 passthrough|doorbell tokens|doorbell poll|host ABI' "${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM/gpu.log" | cut -c1-300 > "$F/v2-device-start.txt"
    caraxes/tools/bench-run.sh stage v2 >> "$F/v2-up.txt" 2>&1
    [[ " $metrics " == *" llama"* ]] && caraxes/tools/bench-run.sh stage-llama v2 >> "$F/v2-up.txt" 2>&1
fi
if [[ " $targets " == *" v1 "* ]]; then
    { caraxes/tools/bench-v1vm.sh up && caraxes/tools/bench-v1vm.sh modules; } > "$F/v1-up.txt" 2>&1
    caraxes/tools/bench-run.sh stage v1 >> "$F/v1-up.txt" 2>&1
    [[ " $metrics " == *" llama"* ]] && caraxes/tools/bench-run.sh stage-llama v1 >> "$F/v1-up.txt" 2>&1
fi

V1_RESTART=${V1_RESTART:-1} V2_RESTART=${V2_RESTART:-1} \
    caraxes/tools/bench-run.sh series --targets "$targets" --reps "$reps" --metrics "$metrics" --out "$F"

caraxes/tools/bench-hostwatch.sh stop "$F"
if [[ " $targets " == *" v2 "* ]]; then
    grep -E 'bar1 passthrough|doorbell tokens' "${CARAXES_STORE:-/mnt/vmstore/caraxes}/$VM/gpu.log" | cut -c1-600 | tail -3 > "$F/v2-device-summary.txt"
fi
journalctl -k --since "$T0" --no-pager -g Xid > "$F/host-xid.txt" 2>/dev/null
cols=(); for t in $targets; do [[ -d $F/$t ]] && cols+=("$F/$t"); done
{
    echo "<!-- SPDX-License-Identifier: MIT -->"
    echo "# Workload gate $(basename "$F")"
    echo
    echo "Written by caraxes/runs/workload-2026-09-24/run.sh; how to read it: docs/caraxes/workload-gate.md."
    echo
    echo '```'
    cat "$F/setup.txt"
    tail -1 "$F/hostwatch.txt"
    echo "host Xid lines since $T0: $(awk '/Xid/ {n++} END {print n+0}' "$F/host-xid.txt" 2>/dev/null)"
    echo '```'
    echo
    echo "## Per-run result lines"
    echo
    echo '```'
    for c in "${cols[@]}"; do
        for f in "$c"/*.txt; do
            l=$(grep -hE '^# (PPO|LLAMA|TRANSFORMER|CONVNET) |MSPERLAUNCH|stage4:' "$f" | head -1 | cut -c1-400)
            echo "$(basename "$c")/$(basename "$f" .txt): ${l:-no result line, $(grep -h '# WALL' "$f")}"
        done
    done
    echo '```'
    echo
    echo "## Tables"
    echo
    python3 caraxes/tools/bench-summary.py "${cols[@]}"
} > "$F/summary.md"
echo "workload gate done $(date -Iseconds): $F/summary.md"
