#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Guest half of tools/caraxes-gate.sh cuda|gpu. `tools/caraxes-vm.sh sync-tests` installs
# it as ~/caraxes-tests/caraxes/run-tests.sh beside the Leandro-Test material in
# ~/caraxes-tests/leandro-test (docs/caraxes/testing.md). It runs inside the guest for
# the reason probe/run/matrix-guest.sh gives: quoting a workload through ssh and sudo
# is where runs silently change.
# Usage: run-tests.sh OUTDIR TEST...   |   run-tests.sh --list
# Every test writes OUTDIR/<test>.log and prints one row on stdout:
#   CXROW <tab> test <tab> pass|fail|skip <tab> scored 1|0 <tab> detail <tab> rm
# where rm is, for a failed test re-run under the v1 tracer, the last RM ioctl whose
# status was not NV_OK (OUTDIR/<test>.rm.tsv), else "-".
# Unscored rows (the Python suites) are information, as in v1 (probe/suites/README.md).
# A failing row's detail is the first failing call with its error code, taken from
# the program's own output (nvprobe and managedprobe print "ERROR <call>: <code> <name>").
# This side measures and applies each test's own v1 criterion; the gate on the host
# computes the verdict. OUTDIR/<test>.dmesg keeps the guest kernel log of the test's
# window. CARAXES_TEST_TIMEOUT (seconds, default 120) bounds each run.
set -uo pipefail

ROOT=$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd -P)
LT=$ROOT/leandro-test
VENV=${CARAXES_GUEST_VENV:-$HOME/gpu/venv}
PY=$VENV/bin/python
TMO=${CARAXES_TEST_TIMEOUT:-120}
SMI="sudo LD_LIBRARY_PATH=/opt/nvrm/lib /opt/nvrm/bin/nvidia-smi"
# What the v1 matrix scripts and the probes read (probe/matrix/*.sh, probe/Makefile).
export LEA_TEST_ROOT=$LT LEA_ROOT=$LT LEANDRO=$LT LEA_PROBE_BIN=$LT/probe/bin
export NVPROBE_PTX=$LT/probe/kernels/kernels.ptx NVTORCH_PY=$PY LEA_HOSTVENV=$VENV

CUDA_TESTS=(cuda-init cuda-context cuda-core cuda-jit cuda-launch cuda-managed cuda-hostreg
    cuda-torch cuda-gdb rlprobe convburn robustness)
SUITES=(test_async_streams test_cuda_graphs test_high_freq_event_polling test_multi_process_cuda
    test_nvrtc test_uvm_migration test_vram_churn rapids_cuml_cudf)
GPU_TESTS=(gpu-memory gpu-kernel gpu-torch gpu-robustness gpu-own gpu-encode)

TRACER=$LT/lib/libnvrm_trace.so
RM_LAST=""
row() { printf 'CXROW\t%s\t%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "${4//$'\t'/ }" "${RM_LAST:--}"; }

# retrace NAME CMD...: after a failure, run CMD once more under the v1 tracer
# (crates/nvrm-trace, LD_PRELOAD) and keep the last RM call that did not return NV_OK:
# the guest-side name of what the device must answer next. v1 wraps only the workload
# (LEA_MATRIX_WRAP), never a shell, and so does this.
retrace() {
    local name=$1 tsv=$OUT/$1.rm.tsv; shift
    [[ -f $TRACER ]] || { RM_LAST="no tracer"; return; }
    rm -f "$tsv"
    local -a wrap=(env LEA_TRACE_FORMAT=tsv LEA_TRACE_DUMP=0 "LEA_TRACE_FILE=$tsv" "LD_PRELOAD=$TRACER")
    if [[ $1 == */probe/matrix/* ]]; then
        LEA_MATRIX_WRAP="${wrap[*]}" timeout -k 5 "$TMO" "$@" > "$OUT/$name.rm.out" 2>&1
    else
        timeout -k 5 "$TMO" "${wrap[@]}" "$@" > "$OUT/$name.rm.out" 2>&1
    fi
    # ioctl lines: ioctl node esc cmd size paramsize rc status fd (crates/nvrm-trace/src/log.rs).
    RM_LAST=$(awk -F'\t' '$1 == "ioctl" && $8 != "-" && $8 != "0x0" { l = $2 " " $3 " " $4 " status " $8 } END { print l }' "$tsv" 2>/dev/null)
    [[ -z $RM_LAST && ! -s $tsv ]] && RM_LAST="no RM call traced"
    [[ -n $RM_LAST ]] || RM_LAST="all RM calls NV_OK"
}

# first_error LOG: the first failing call with its code, else the last exception, else
# the last line.
first_error() {
    local l
    l=$(grep -m1 -E '^ERROR [A-Za-z_]+: -?[0-9]+' "$1") ||
    l=$(grep -m1 -E '^[A-E] [A-Za-z(). ]+: [1-9][0-9]* ' "$1" | sed 's/  */ /g') ||
    l=$(grep -E '^[A-Za-z_.]*(Error|Exception)(:|$)' "$1" | tail -1) ||
    l=$(grep -m1 -E '^ERROR' "$1") ||
    l=$(grep -vE '^[[:space:]]*$' "$1" | tail -1)
    printf '%s' "${l:0:300}"
}

# run LOG SECONDS CMD...: run with a hard timeout, output into LOG, return the status.
run() {
    local log=$1 t=$2; shift 2
    timeout -k 5 "$t" "$@" > "$log" 2>&1
}

verdict_rc() { # verdict_rc NAME LOG RC: after a timeout say so, else name the error.
    if [[ $3 -eq 124 || $3 -eq 137 ]]; then row "$1" fail 1 "timed out after ${TMO}s; last line: $(first_error "$2")"
    else row "$1" fail 1 "exit $3: $(first_error "$2")"; fi
}

# nvprobe STAGE NAME WANT: one staged driver-API run (probe/c/nvprobe.c).
t_nvprobe() {
    local log=$OUT/$2.log rc
    run "$log" "$TMO" "$LT/probe/bin/nvprobe" "$1"; rc=$?
    if [[ $rc -eq 0 ]] && grep -q "$3" "$log"; then row "$2" pass 1 "$(grep -m1 "$3" "$log")"
    else retrace "$2" "$LT/probe/bin/nvprobe" "$1"; verdict_rc "$2" "$log" "$rc"; fi
}

# matrix NAME: a v1 matrix probe, unmodified, with its own criterion (exit 0 and a
# CRITERION line pass, exit 2 is its declared reason not to run, anything else fails).
t_matrix() {
    local log=$OUT/$1.log rc crit
    run "$log" "$TMO" "$LT/probe/matrix/$1.sh"; rc=$?
    crit=$(sed -n 's/^CRITERION: //p' "$log" | tail -1)
    if [[ $rc -eq 0 && -n $crit ]]; then row "$1" pass 1 "$crit"
    elif [[ $rc -eq 2 ]]; then row "$1" skip 1 "$(grep -m1 -E '^(declared-unsupported|blocked)' "$log" || first_error "$log")"
    elif [[ $rc -eq 0 ]]; then row "$1" fail 1 "no CRITERION line"
    else retrace "$1" "$LT/probe/matrix/$1.sh"; verdict_rc "$1" "$log" "$rc"; fi
}

t_rlprobe() {
    local log=$OUT/rlprobe.log rc v
    NVRL_PIN=1 run "$log" "$TMO" "$PY" "$LT/probe/python/rlprobe.py"; rc=$?
    v=$(grep -o 'mean10=[0-9.]*' "$log" | tail -1)
    if [[ $rc -eq 0 && -n $v ]]; then row rlprobe pass 1 "$v (pinned)"
    else NVRL_PIN=1 retrace rlprobe "$PY" "$LT/probe/python/rlprobe.py"; verdict_rc rlprobe "$log" "$rc"; fi
}

t_convburn() {
    local log=$OUT/convburn.log rc v
    run "$log" "$((TMO * 3))" "$PY" "$LT/probe/python/convburn.py"; rc=$?
    v=$(grep -o 'acc=[0-9.e+-]*' "$log" | tail -1)
    if [[ $rc -eq 0 && -n $v ]]; then row convburn pass 1 "$v"
    else retrace convburn "$PY" "$LT/probe/python/convburn.py"; verdict_rc convburn "$log" "$rc"; fi
}

# v1 gpu gate "robustness" (scripts/test.sh): two processes at once, SIGKILL mid-run
# and a clean next run, and the module pinned while a client holds an FD. v1 tried
# rmmod virtio_nvrm; here nvidia.ko's refcount is read instead, because an rmmod that
# succeeded would unload the guest driver.
t_robustness() {
    local name=$1 log=$OUT/$1.log n rc0 rc1
    {
        echo "(a) two CUDA processes at once"
        ( timeout -k 5 "$TMO" "$LT/probe/bin/nvprobe" 3 > "$OUT/$name-p1.txt" 2>&1 &
          timeout -k 5 "$TMO" "$LT/probe/bin/nvprobe" 3 > "$OUT/$name-p2.txt" 2>&1 & wait )
        echo "  P1: $(tail -1 "$OUT/$name-p1.txt")"; echo "  P2: $(tail -1 "$OUT/$name-p2.txt")"
        echo "(b) kill -9 mid-run"
        timeout -k 5 "$TMO" "$LT/probe/bin/nvprobe" 3 > /dev/null 2>&1 & local bg=$!
        sleep 2; kill -9 "$bg" 2>/dev/null; wait "$bg" 2>/dev/null; sleep 2
        echo "  next run: $(timeout -k 5 "$TMO" "$LT/probe/bin/nvprobe" 3 2>&1 | tail -1)"
        echo "(c) module pinned by an open FD"
        rc0=$(cat /sys/module/nvidia/refcnt)
        python3 -c 'import time; f=open("/dev/nvidiactl"); time.sleep(4)' & local holder=$!
        sleep 1; rc1=$(cat /sys/module/nvidia/refcnt); wait "$holder" 2>/dev/null
        echo "  nvidia refcnt $rc0 -> $rc1 while /dev/nvidiactl is open"
    } > "$log" 2>&1
    n=$(grep -c 'stage 3 ok (kernel, result correct)' "$log")
    if [[ $n -eq 3 ]] && (( rc1 > rc0 )); then row "$name" pass 1 "2 in parallel ok, next run after SIGKILL ok, nvidia.ko pinned by an open FD"
    else row "$name" fail 1 "$n of 3 runs correct ($(first_error "$OUT/$name-p1.txt")); refcnt $rc0 -> $rc1"; fi
}

# v1 gpu gate "memory": nvprobe 2 through the RM mmap path.
t_gpu_memory() { t_nvprobe 2 gpu-memory 'stage 2 ok'; }

# v1 gpu gate "kernel": nvprobe 3 correct and managedprobe (default stage) twice to
# stage 2; v1's pin and pool accounting belongs to virtio_nvrm and has no counterpart.
t_gpu_kernel() {
    local log=$OUT/gpu-kernel.log mp i
    {
        timeout -k 5 "$TMO" "$LT/probe/bin/nvprobe" 3 2>&1 | tail -2
        for i in 1 2; do
            echo "managedprobe run $i:"
            timeout -k 5 "$TMO" stdbuf -o0 "$LT/probe/bin/managedprobe" 2>&1 | grep -E 'stage[123] |ERROR'
        done
    } > "$log" 2>&1
    mp=$(grep -c 'stage2 ok' "$log")
    if grep -q 'stage 3 ok (kernel, result correct)' "$log" && [[ $mp -eq 2 ]]; then
        row gpu-kernel pass 1 "result correct, managedprobe 2x to stage 2"
    else row gpu-kernel fail 1 "managedprobe-stage2=$mp/2; $(first_error "$log")"; fi
}

# v1 gpu gate "torch": rlprobe mean10 and convburn acc; the host compares them with the
# native reference (bit-identical), this side only reports them.
t_gpu_torch() {
    local log=$OUT/gpu-torch.log m a
    {
        echo "### MODULE (no LD_PRELOAD)"
        timeout -k 5 "$((TMO * 3))" "$PY" "$LT/probe/python/rlprobe.py" 2>&1 | tail -2
        timeout -k 5 "$((TMO * 8))" "$PY" "$LT/probe/python/convburn.py" 2>&1 | tail -3
    } > "$log" 2>&1
    m=$(grep -o 'mean10=[0-9.]*' "$log" | tail -1); a=$(grep -o 'acc=[0-9.e+-]*' "$log" | tail -1)
    if [[ -n $m && -n $a ]]; then row gpu-torch pass 1 "$m $a"; else row gpu-torch fail 1 "${m:-no mean10} ${a:-no acc}: $(first_error "$log")"; fi
}

# v1 gpu gate "own": the guest's own CUDA process is in the guest's nvidia-smi list.
t_gpu_own() {
    local log=$OUT/gpu-own.log holder
    {
        "$PY" -c '
import torch, time, os
t = torch.empty(64 * 1024 * 1024 // 4, dtype=torch.float32, device="cuda")
t.fill_(1.0); torch.cuda.synchronize()
print("holder pid", os.getpid(), flush=True)
time.sleep(20)' > "$OUT/gpu-own-holder.txt" 2>&1 &
        sleep 12
        echo "holder=$(grep -oP 'holder pid \K[0-9]+' "$OUT/gpu-own-holder.txt")"
        $SMI | sed -n '/Processes:/,$p'
        wait 2>/dev/null
    } > "$log" 2>&1
    holder=$(grep -oP 'holder=\K[0-9]+' "$log")
    if [[ -n $holder ]] && grep -qE "^\|[^|]*[[:space:]]${holder}[[:space:]]" "$log"; then
        row gpu-own pass 1 "guest PID $holder listed in the VM's own nvidia-smi"
    else row gpu-own fail 1 "holder did not start: $(first_error "$OUT/gpu-own-holder.txt")"; fi
}

# v1 gpu gate "encode", guest half: NVENC 1080p, then a hardware decode of 30 frames.
t_gpu_encode() {
    local log=$OUT/gpu-encode.log bytes
    command -v ffmpeg > /dev/null || { echo "no ffmpeg" > "$log"; row gpu-encode fail 1 "no ffmpeg in the guest (not in the baked image)"; return; }
    {
        timeout -k 5 "$TMO" ffmpeg -hide_banner -loglevel error -f lavfi -i testsrc=size=1920x1080:rate=30 \
            -frames:v 30 -c:v h264_nvenc -pix_fmt yuv420p -y /tmp/gate-enc.mp4 2>&1
        echo "bytes=$(stat -c %s /tmp/gate-enc.mp4 2>/dev/null || echo 0)"
        echo "--- decode ---"
        timeout -k 5 "$TMO" ffmpeg -hide_banner -loglevel error -stats -hwaccel cuda -i /tmp/gate-enc.mp4 -f null - 2>&1 | tail -1
    } > "$log" 2>&1
    bytes=$(grep -oP 'bytes=\K[0-9]+' "$log"); bytes=${bytes:-0}
    if [[ $bytes -le 0 ]]; then row gpu-encode fail 1 "no bitstream: $(first_error "$log")"
    elif ! grep -qE 'frame=[[:space:]]*30' "$log"; then row gpu-encode fail 1 "decode did not reach 30 frames"
    else row gpu-encode pass 1 "bytes=$bytes, hardware decode 30 frames"; fi
}

# A v1 Python suite (probe/suites): unscored, missing modules are a skip.
t_suite() {
    local log=$OUT/suite-$1.log rc
    ( cd "$LT/probe/suites" && run "$log" "$((TMO * 5))" "$PY" "$1.py" ); rc=$?
    if grep -qE '^ModuleNotFoundError|No module named' "$log"; then
        row "suite-$1" skip 0 "missing module: $(grep -oP "No module named '\K[^']+" "$log" | head -1)"
    elif [[ $rc -eq 0 ]]; then row "suite-$1" pass 0 "$(grep -vE '^[[:space:]]*$' "$log" | tail -1)"
    else verdict_rc "suite-$1" "$log" "$rc" | sed 's/\t1\t/\t0\t/'; fi
}

one() {
    case $1 in
        cuda-init) t_nvprobe 0 cuda-init 'stage 0 ok' ;;
        cuda-context) t_nvprobe 1 cuda-context 'stage 1 ok' ;;
        cuda-*) t_matrix "$1" ;;
        rlprobe) t_rlprobe ;;
        convburn) t_convburn ;;
        robustness|gpu-robustness) t_robustness "$1" ;;
        gpu-memory) t_gpu_memory ;;
        gpu-kernel) t_gpu_kernel ;;
        gpu-torch) t_gpu_torch ;;
        gpu-own) t_gpu_own ;;
        gpu-encode) t_gpu_encode ;;
        suite-*) t_suite "${1#suite-}" ;;
        *) row "$1" fail 1 "unknown test" ;;
    esac
}

case ${1:-} in
    --list) printf '%s\n' "${CUDA_TESTS[@]}" "${SUITES[@]/#/suite-}" "${GPU_TESTS[@]}"; exit 0 ;;
    ""|-h|--help) sed -n '5,17p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
OUT=$1; shift
mkdir -p "$OUT" || exit 2
[[ -x $LT/probe/bin/nvprobe ]] || { echo "no $LT/probe/bin/nvprobe; run tools/caraxes-vm.sh sync-tests" >&2; exit 2; }
[[ $# -gt 0 ]] || set -- "${CUDA_TESTS[@]}" "${SUITES[@]/#/suite-}"
for t in "$@"; do
    # The guest kernel log of this test's window, without the per-ioctl debug lines.
    d0=$(sudo dmesg 2>/dev/null | wc -l)
    RM_LAST=""
    one "$t"
    sudo dmesg 2>/dev/null | tail -n +"$((d0 + 1))" | grep -vE 'NVRM: (ioctl|VM:)|nvidia_(open|close|ctl_)|Opening GPU' > "$OUT/$t.dmesg"
done
