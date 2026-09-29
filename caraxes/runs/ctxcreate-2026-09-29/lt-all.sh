#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Every Leandro-Test suite and probe once, timed: one row per test
#   LTROW <tab> test <tab> pass|fail|skip <tab> seconds <tab> detail
# on stdout and in OUT/rows.tsv, each test's output in OUT/<test>.log.
# Runs in the guest (ROOT = ~/caraxes-tests after `tools/caraxes-vm.sh sync-tests`) and
# natively on the host (lt-suite.sh native builds the same layout over the host's
# Leandro-Test and venv). The Caraxes runner (tools/caraxes-guest-tests.sh, installed as
# ROOT/caraxes/run-tests.sh) judges the tests it knows with its own criteria; the others
# are judged here the same way (a matrix probe: exit 0 with a CRITERION line pass, exit 2
# skip; a probe: exit 0 and its result line).
# Usage: lt-all.sh OUT [test...]   (default: the whole list below); LT_NATIVE=1 skips the
# tests that drive the card's VRAM to the refusal (on the host card they would starve the
# desktop, docs: the host VRAM floor).
set -uo pipefail
OUT=${1:?usage: lt-all.sh OUT [test...]}; shift
ROOT=${LT_ROOT:-$HOME/caraxes-tests}
LT=$ROOT/leandro-test
RUNNER=$ROOT/caraxes/run-tests.sh
VENV=${CARAXES_GUEST_VENV:-$HOME/gpu/venv}
PY=$VENV/bin/python
BENCH=${LT_BENCH:-$HOME/bench}
TMO=${CARAXES_TEST_TIMEOUT:-300}
NATIVE=${LT_NATIVE:-0}
export CARAXES_GUEST_VENV=$VENV CARAXES_TEST_TIMEOUT=$TMO
export LEA_TEST_ROOT=$LT LEA_ROOT=$LT LEANDRO=$LT LEA_PROBE_BIN=$LT/probe/bin
export NVPROBE_PTX=$LT/probe/kernels/kernels.ptx NVTORCH_PY=$PY LEA_HOSTVENV=$VENV
# shellcheck disable=SC1091
[[ -r $ROOT/caraxes/driver.env ]] && source "$ROOT/caraxes/driver.env"
[[ -n ${CARAXES_GUEST_SMI:-} ]] && export NVIDIA_SMI=$CARAXES_GUEST_SMI
mkdir -p "$OUT" || exit 2
: > "$OUT/rows.tsv"

RUNNER_TESTS=(cuda-init cuda-context cuda-core cuda-jit cuda-launch cuda-managed cuda-hostreg
    cuda-torch cuda-gdb rlprobe convburn robustness
    suite-test_async_streams suite-test_cuda_graphs suite-test_high_freq_event_polling
    suite-test_multi_process_cuda suite-test_nvrtc suite-test_uvm_migration suite-test_vram_churn
    suite-rapids_cuml_cudf gpu-memory gpu-kernel gpu-torch gpu-own gpu-encode)
MATRIX_TESTS=(nvml nvenc nvdec nvofa nvfbc opencl rm-direct pkcs11 kms-nvkms vk-enum vk-offscreen
    vk-rt vk-sc gl-enum gl-render gl-32bit gles egl-gbm egl-wayland egl-xcb egl-xlib)
PROBES=(nvprobe-4 managedprobe hostregprobe oomprobe vrampress ctrlping ioctlping
    torchprobe torchprobe-conv mmsweep mtprobe streamprobe pinwin convoom vramcap
    bench-init bench-churn bench-copy bench-launch)
# Tests that take the card's VRAM to the refusal: not natively. Nor the ones that touch the
# host's display (modesetting, frame capture) or need sudo nvidia-smi (gpu-own).
VRAM_BOUND=(oomprobe vrampress convoom vramcap)
NATIVE_SKIP=(matrix-kms-nvkms matrix-nvfbc gpu-own)

row() {
    local d=${4//$'\t'/ }
    printf 'LTROW\t%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "${d:0:300}" | tee -a "$OUT/rows.tsv"
}
last() { grep -vE '^[[:space:]]*$' "$1" | tail -1; }
# secs T0: seconds since T0 (date +%s%N), three decimals, without bc (not in the guest image).
secs() { local ms=$(( ($(date +%s%N) - $1) / 1000000 )); printf '%d.%03d' $((ms / 1000)) $((ms % 1000)); }

# probe NAME SECONDS WANT CMD...: pass on exit 0 with a line matching WANT (a regex).
probe() {
    local name=$1 t=$2 want=$3 log=$OUT/$1.log rc t0 dt l; shift 3
    t0=$(date +%s%N)
    timeout -k 5 "$t" "$@" > "$log" 2>&1; rc=$?
    dt=$(secs "$t0")
    l=$(grep -E "$want" "$log" | tail -1)
    if [[ $rc -eq 0 && -n $l ]]; then row "$name" pass "$dt" "$l"
    elif [[ $rc -eq 124 || $rc -eq 137 ]]; then row "$name" fail "$dt" "timed out after ${t}s: $(last "$log")"
    else row "$name" fail "$dt" "exit $rc: $(grep -m1 -E '^ERROR|Error|error' "$log" || last "$log")"; fi
}

matrix() {
    local name=$1 log=$OUT/matrix-$1.log rc t0 dt crit
    t0=$(date +%s%N)
    timeout -k 5 "$TMO" "$LT/probe/matrix/$1.sh" > "$log" 2>&1; rc=$?
    dt=$(secs "$t0")
    crit=$(sed -n 's/^CRITERION: //p' "$log" | tail -1)
    if [[ $rc -eq 0 && -n $crit ]]; then row "matrix-$name" pass "$dt" "$crit"
    elif [[ $rc -eq 2 ]]; then row "matrix-$name" skip "$dt" "$(grep -m1 -E '^(declared-unsupported|blocked)' "$log" || last "$log")"
    elif [[ $rc -eq 0 ]]; then row "matrix-$name" fail "$dt" "no CRITERION line"
    else row "matrix-$name" fail "$dt" "exit $rc: $(grep -m1 -E '^ERROR|error' "$log" || last "$log")"; fi
}

runner() {
    local name=$1 t0 dt r
    t0=$(date +%s%N)
    r=$("$RUNNER" "$OUT/runner" "$name" 2>&1 | grep '^CXROW' | tail -1)
    dt=$(secs "$t0")
    if [[ -z $r ]]; then row "$name" fail "$dt" "no row from the runner"; return; fi
    IFS=$'\t' read -r _ _ res _ detail rm <<< "$r"
    [[ $rm == - ]] && rm=""
    row "$name" "$res" "$dt" "$detail${rm:+ (rm: $rm)}"
}

one() {
    local t=$1
    if [[ $NATIVE == 1 && " ${VRAM_BOUND[*]} " == *" $t "* ]]; then
        row "$t" skip 0 "native: not run, it takes the host card's VRAM to the refusal"; return
    fi
    if [[ $NATIVE == 1 && " ${NATIVE_SKIP[*]} " == *" $t "* ]]; then
        row "$t" skip 0 "native: not run (the host's display, or sudo nvidia-smi)"; return
    fi
    case $t in
        matrix-*) matrix "${t#matrix-}" ;;
        nvprobe-4) probe "$t" "$TMO" 'stage 4 ok' "$LT/probe/bin/nvprobe" 4 ;;
        managedprobe) probe "$t" "$TMO" '^DONE' "$LT/probe/bin/managedprobe" ;;
        hostregprobe) probe "$t" "$TMO" '^DONE' "$LT/probe/bin/hostregprobe" ;;
        oomprobe) probe "$t" "$TMO" 'recovery 1 MiB: 0 ' "$LT/probe/bin/oomprobe" ;;
        vrampress) probe "$t" 120 'vrampress:' "$LT/probe/bin/vrampress" --fill 80 --seconds 30 ;;
        ctrlping) probe "$t" "$TMO" '^ctrlping bytes=16384 .*fails=0 ' "$LT/probe/bin/ctrlping" 2000 ;;
        ioctlping) probe "$t" "$TMO" '^ioctlping .*fails=0 ' "$LT/probe/bin/ioctlping" 10000 ;;
        torchprobe) probe "$t" "$TMO" '^MSPERSTEP=' "$PY" "$LT/probe/python/torchprobe.py" ;;
        torchprobe-conv) NVTORCH_CONV=1 probe "$t" "$TMO" '^MSPERSTEP=' "$PY" "$LT/probe/python/torchprobe.py" ;;
        mmsweep) probe "$t" "$TMO" '^mmsweep: N=8192 .*GFLOP' "$PY" "$LT/probe/python/mmsweep.py" ;;
        mtprobe) probe "$t" "$TMO" '^mtprobe: ' "$PY" "$LT/probe/python/mtprobe.py" ;;
        streamprobe) probe "$t" "$TMO" '^streamprobe: iters=' "$PY" "$LT/probe/python/streamprobe.py" ;;
        pinwin) probe "$t" "$TMO" '^pinwin: recovery 16 MiB correct' "$PY" "$LT/probe/python/pinwin.py" ;;
        convoom) probe "$t" "$TMO" '^convoom: recovery batch=4 [^E]' "$PY" "$LT/probe/python/convoom.py" ;;
        vramcap) probe "$t" "$TMO" '^vramcap: first=' "$PY" "$LT/probe/python/vramcap.py" ;;
        bench-init) probe "$t" "$TMO" '^INIT' bash -c "for i in 1 2 3 4 5; do '$BENCH/bench-cu' init || exit 1; done" ;;
        bench-churn) probe "$t" "$TMO" '^CHURN' "$BENCH/bench-cu" churn 10 ;;
        bench-copy) probe "$t" "$TMO" '^CHECK size_mib=256 bad_words=0' "$BENCH/bench-cu" copy 3 ;;
        bench-launch) (cd "$BENCH" && BENCH_CU_PTX=$BENCH/kernels.ptx probe "$t" "$TMO" '^LAUNCH' ./bench-cu launch 1000) ;;
        *) runner "$t" ;;
    esac
}

[[ $# -gt 0 ]] || set -- "${RUNNER_TESTS[@]}" "${MATRIX_TESTS[@]/#/matrix-}" "${PROBES[@]}"
echo "# lt-all: $# test(s), native=$NATIVE, LT $LT, python $PY, bench $BENCH, $(date -Iseconds)"
for t in "$@"; do
    d0=$(sudo -n dmesg 2>/dev/null | wc -l)
    one "$t"
    sudo -n dmesg 2>/dev/null | tail -n +"$((d0 + 1))" | grep -E 'Xid|NVRM: [A-Z]' > "$OUT/$t.dmesg"
done
echo "# lt-all done $(date -Iseconds): $(grep -c $'\tpass\t' "$OUT/rows.tsv") pass, $(grep -c $'\tfail\t' "$OUT/rows.tsv") fail, $(grep -c $'\tskip\t' "$OUT/rows.tsv") skip"
