#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The benchmark behind docs/performance.md: native records against Leandro VirtIO (v1) and
# Leandro VFIO (v2) on one card, one VM at a time under the VM store's vmlock.sh. Each area
# is a thin wrapper around the scripts it names. Usage:
#   run.sh all|compute|v2-linux|open|v2-windows|v1|summary OUTDIR [BINARY]
#   compute     v2 lean VM: the workload series and the P13 metrics (v2-compute.sh), then the
#               Leandro-Test suite (caraxes/runs/ctxcreate-2026-09-29/lt-suite.sh, run from a
#               copy in OUTDIR/lt, where it writes)
#   v2-linux    Linux B: the kick-smoke with vkmark and glmark2 (bench-kick.sh), the codec
#               matrix and Sunshine with nvenc and x264 (codec-linux.sh), SotTR once
#               (sottr-bench.sh, then gate-2026-09-30/frame-check.sh on its guest shots)
#   open        nova-core 5-probe (vm9) and nouveau with NVK, R1-R8 (vm10)
#   v2-windows  D3D11 stages, Heaven, Superposition, the codec matrix, Sunshine with nvenc and
#               x264, one reboot (win-bench.sh)
#   v1          P13 metrics, workload series, Leandro-Test, the smi and cuda gates, glmark2
#               off-screen (v1-bench.sh)
#   summary     OUTDIR/summary.md from the records (also written after every other area)
# BINARY: the v2 device (default: the gate binary in the VM store). Windows takes WIN_BINARY
# (default: the kit's vfio-user-nvgpu-stack-cap, the same bytes with cap_sys_admin).
set -uo pipefail
AREA=${1:?usage: run.sh all|compute|v2-linux|open|v2-windows|v1|summary OUTDIR [BINARY]}
O=$(realpath -m "${2:?usage: run.sh AREA OUTDIR [BINARY]}")
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
REPO=$(cd "$HERE/../../.." && pwd -P)
STORE=/mnt/vmstore/caraxes
BIN=${3:-$STORE/vfio-user-nvgpu-stack-ffec4d49}
WIN_BIN=${WIN_BINARY:-$STORE/alpha-2026-09-27/windows-plain/CH-build/vfio-user-nvgpu-stack-cap}
L=$STORE/locks/vmlock.sh
cd "$REPO" || exit 1
mkdir -p "$O"
say() { echo "$(date +%T) $*" | tee -a "$O/timeline.txt"; }
# Before a start: the previous VM's device, hypervisor and backend must be gone (a device that
# is still exiting makes the Windows kit refuse), and 45 GB must stay free on the store.
settle() {
    local n=0
    while ps -eo args | grep -qE '^(/[^ ]*/)?(vfio-user-nvgpu[^ ]* |cloud-hypervisor |vhost-user-nvrm )' && (( n++ < 60 )); do sleep 5; done
    (( $(df --output=avail -BG "$STORE" | tail -1 | tr -dc 0-9) >= 45 )) || { say "STOP: under 45 GB free on $STORE"; exit 1; }
}
run() { local g=$1 t=$2; shift 2; settle; say "start $g: $*"; bash "$L" "$g" 7200 timeout "$t" "$@"; say "end $g rc=$?"; }

compute() {
    run vm2-615 3600 "$HERE/v2-compute.sh" "$O/v2-compute" "$BIN" 4096 > "$O/v2-compute.log" 2>&1
    mkdir -p "$O/lt" && cp caraxes/runs/ctxcreate-2026-09-29/lt-suite.sh caraxes/runs/ctxcreate-2026-09-29/lt-all.sh "$O/lt/"
    run vm2-615 5400 "$O/lt/lt-suite.sh" vm "$BIN" v2-lt > "$O/v2-lt.log" 2>&1
}
open_drivers() {
    run vm9 1200 caraxes/runs/nova-2026-09-25/nova-boot.sh "$O/nova" CARAXES_SERVER="$BIN" \
        CARAXES_KERNEL=$STORE/nova-kernel/build/arch/x86/boot/bzImage \
        CARAXES_VBIOS=$STORE/night-2026-09-28/synth-vbios-nova.bin CARAXES_GUEST_ABI= > "$O/nova.log" 2>&1
    run vm10 900 caraxes/runs/openrender-2026-09-29/openrender-boot.sh "$O/vm10" CARAXES_SERVER="$BIN" \
        CARAXES_RESERVE_SLOT=4 CARAXES_DOORBELL_TOKENS=64 CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 > "$O/vm10.log" 2>&1
}
v2_linux() {
    run desktop-b 2400 "$HERE/bench-kick.sh" "$O/kick" "$BIN" > "$O/kick.log" 2>&1
    run desktop-b 3600 "$HERE/codec-linux.sh" "$O/codec-linux" "$BIN" copy matrix probe desk software nvenc > "$O/codec-linux.log" 2>&1
    # sottr-bench-6g.sh bounds itself at 2900 s and the wrapper then stops the VM: more here.
    run desktop-b-dmem 3400 "$HERE/sottr-bench.sh" "$O/sottr" "$BIN" > "$O/sottr.log" 2>&1
    for f in "$O"/sottr/bench/guest-shots/*.png; do
        [[ -f $f ]] && caraxes/runs/gate-2026-09-30/frame-check.sh "$f" 2>&1 | tail -1 | sed "s|^|$(basename "$f"): |" >> "$O/sottr/frame-check.txt"
    done
}
v2_windows() {
    run windows 9000 "$HERE/win-bench.sh" "$O/win-$(date +%Y%m%d-%H%M)" "$WIN_BIN" sleep=120 config arm \
        stages heaven superposition copy matrix software nvenc reboot clients refused events > "$O/win.log" 2>&1
}
v1() { run v1 9000 "$HERE/v1-bench.sh" "$O/v1" > "$O/v1.log" 2>&1; }

# The summary: the P13 table against the native closing series of 2026-09-24, then the key
# lines of every record that exists.
summary() {
    local d
    {
        echo "# Benchmark summary $(date -Iseconds)"
        echo; echo "Device: $(sha256sum < "$BIN" | cut -c1-16) $BIN; host $(head -1 /proc/driver/nvidia/version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)"
        local p13=()
        for d in "$O/v1/p13/v1" "$O/v2-compute/p13/v2"; do [[ -d $d ]] && p13+=("$d"); done
        if (( ${#p13[@]} )); then
            echo; echo "## P13 metrics"; echo
            python3 caraxes/tools/bench-summary.py caraxes/runs/closing-610-2026-09-24/native "${p13[@]}" | grep -vE 'device CPU|VMM CPU'
        fi
        echo; echo "## Workload series (short kernel x1000, llama.cpp, transformer, ResNet-18, nvenc-probe)"
        for d in v2-compute v1; do
            [[ -f $O/$d/guest-series.txt ]] || continue
            echo; echo "$d:"; echo '```'
            grep -E "MSPERLAUNCH|\| llama|# TRANSFORMER|# CONVNET|^ENC|^DEC|RESULT" "$O/$d/guest-series.txt" | cut -c1-160
            echo '```'
        done
        echo; echo "## Leandro-Test (lt-all.sh)"; echo
        [[ -f $O/lt/v2-lt/rows.tsv ]] && echo "- v2: $(tail -1 "$O/v2-lt.log" 2>/dev/null)"
        [[ -f $O/v1/rows.tsv ]] && echo "- v1: $(grep -c $'\tpass\t' "$O/v1/rows.tsv") pass, $(grep -c $'\tfail\t' "$O/v1/rows.tsv") fail, $(grep -c $'\tskip\t' "$O/v1/rows.tsv") skip"
        for f in v2:"$O"/lt/v2-lt/rows.tsv v1:"$O"/v1/rows.tsv; do
            [[ -f ${f#*:} ]] && awk -F'\t' -v w="${f%%:*}" '$3 == "fail" { printf "  - fail %s (%s): %s\n", $2, w, substr($5, 1, 140) }' "${f#*:}"
        done
        echo; echo "## Linux graphics, video, streaming"; echo; echo '```'
        grep -hE "glmark2|Score|RESULT|vkcube" "$O/kick/smoke.txt" 2>/dev/null
        grep -h "^MATRIX" "$O/codec-linux/matrix.txt" 2>/dev/null | cut -c1-150
        for s in software nvenc; do
            grep -hE "^window|Host processing latency|Rendering frame rate" "$O/codec-linux/stream-$s/stream.txt" 2>/dev/null | sed "s/^/$s: /"
        done
        grep -hiE "average fps|bench rc" "$O/sottr/ab.txt" 2>/dev/null; cat "$O/sottr/frame-check.txt" 2>/dev/null
        cat "$O/vm10/results.txt" 2>/dev/null; echo "nova checks: $(grep -c 'CARAXES-CHECK \(probe\|cycle\|reload\)' "$O/nova/guest-nova.txt" 2>/dev/null)"
        echo '```'
        echo; echo "## Windows"; echo; echo '```'
        grep -hE "^(PASS|FAIL):|FPS:|Score:|MATRIX|^window|Host processing latency|Rendering frame rate|back up|FLRs|HOST LINE" "$O/win.log" 2>/dev/null | cut -c1-160
        echo '```'
        echo; echo "## v1 gates"; echo
        grep -hE "PASS|FAIL" "$O/v1/gates/smi/verdict.txt" 2>/dev/null | sed 's/^/- /'
        [[ -f $O/v1/gates.txt ]] && echo "- $(tail -1 "$O/v1/gates.txt")"
    } > "$O/summary.md"
    echo "summary: $O/summary.md"
}

case $AREA in
compute) compute ;;
v2-linux) v2_linux ;;
open) open_drivers ;;
v2-windows) v2_windows ;;
v1) v1 ;;
all) compute; open_drivers; v2_linux; v2_windows; v1 ;;
summary) ;;
*) echo "ERROR: area all|compute|v2-linux|open|v2-windows|v1|summary" >&2; exit 2 ;;
esac
summary
