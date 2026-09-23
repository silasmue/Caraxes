#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Caraxes gates against a running guest (tools/caraxes-vm.sh up; wait). Conventions of
# docs/TESTING.md: one JSON verdict on stdout, diagnostics on stderr, exit 0 pass,
# 1 fail, 2 skip (a missing prerequisite never counts as a pass). Nothing is built
# or started here. Reference: docs/caraxes/nvml.md.
# Usage: tools/caraxes-gate.sh nvml|cuda [test...]|gpu|vdisplay|display
# nvml: runs nvidia-smi and nvidia-smi -q in the guest; checks the device-owned
# identity (name, 4096 MiB total, consistent usage) and that temperature, performance
# state, power draw and utilization are numbers; then compares the forwarded values
# with the host's own nvidia-smi -q (read-only NVML query on the host card).
# CARAXES_GATE_NAME (default "Leandro RTX 2070-4G"), CARAXES_GATE_FB_MIB (4096) and
# CARAXES_GATE_TEMP_TOL (degrees, default 4) set the expectations.
# cuda, gpu, vdisplay, display: the Leandro v1 gates (../Leandro-Test scripts/test.sh)
# against the vfio-user device, in v1's verdict format (scripts/lib/common.sh
# lea_gate_*): one JSON line {"gate","result","dur_s","reason","facts"}, exit 0/1/2.
# cuda runs the v1 CUDA material (nvprobe stages, the compute rows of probe/matrix,
# rlprobe, convburn, robustness; the probe/suites unscored) one test per ssh call and
# prints a per-test table with the failing call, its error code and the device RPCs
# that did not answer NV_OK. gpu runs the guest-side stages of v1's gpu gate (smi,
# memory, kernel, torch, robustness, own, encode). vdisplay and display report "not
# applicable yet" (exit 2) with the reason. Material comes from
# `tools/caraxes-vm.sh sync-tests`; raw output goes to caraxes/runs/gate-<name>-<date>/.
# CARAXES_GATE_NATIVE=1 lets gpu run the native torch reference on the host card
# (LEA_HOSTVENV); otherwise caraxes/reference/rtx2070/torch-native.txt is used.
# CARAXES_GATE_RUNS sets the directory the run directories go to (default caraxes/runs).
set -uo pipefail

LEA_ROOT=$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd -P)
VMSH=$LEA_ROOT/tools/caraxes-vm.sh
SMI_DIR=${CARAXES_GUEST_NVRM:-/opt/nvrm}
WANT_NAME=${CARAXES_GATE_NAME:-Leandro RTX 2070-4G}
WANT_MIB=${CARAXES_GATE_FB_MIB:-4096}
TOL=${CARAXES_GATE_TEMP_TOL:-4}

usage() { sed -n '9,28p' "$0" | sed 's/^# \{0,1\}//' >&"$(( ${1:-0} ? 2 : 1 ))"; exit "${1:-0}"; }
say() { printf 'caraxes-gate: %s\n' "$*" >&2; }
jstr() { local s=${1//\\/\\\\}; s=${s//\"/\\\"}; printf '"%s"' "$s"; }

# verdict <pass|fail|skip> <reason>; prints the JSON and exits with 0/1/2.
verdict() {
    local v=$1 reason=$2 code
    case $v in pass) code=0 ;; fail) code=1 ;; *) code=2 ;; esac
    printf '{"gate":"nvml","verdict":"%s","reason":%s,"guest":{%s},"host":{%s},"checks":[%s]}\n' \
        "$v" "$(jstr "$reason")" "${GJSON:-}" "${HJSON:-}" "${CHECKS:-}"
    exit "$code"
}

# field <file> <section|*> <key>: the value of `key` in nvidia-smi -q output, inside the
# 4-space-indented section `section` (or anywhere for *), first occurrence.
field() {
    awk -v sec="$2" -v key="$3" '
        /^    [^ ]/ && index($0, " : ") == 0 { cur = $0; gsub(/^ +| +$/, "", cur) }
        {
            i = index($0, " : "); if (i == 0) next
            k = substr($0, 1, i - 1); gsub(/^ +| +$/, "", k)
            if (k == key && (sec == "*" || cur == sec)) { v = substr($0, i + 3); gsub(/^ +| +$/, "", v); print v; exit }
        }' "$1"
}

num() { [[ $1 =~ ^([0-9]+(\.[0-9]+)?) ]] && echo "${BASH_REMATCH[1]}"; }
# P0-P2 perform, P3-P7 balanced, P8 and up idle (the RM's pstate groups on Turing).
family() {
    local p=${1#P}
    [[ $p =~ ^[0-9]+$ ]] || { echo unknown; return; }
    if ((p <= 2)); then echo perform; elif ((p <= 7)); then echo balanced; else echo idle; fi
}

CHECKS=""
FAILED=()
check() { # check <name> <ok 0|1> <detail>
    local ok=true
    [[ $2 == 1 ]] || { ok=false; FAILED+=("$1"); say "FAIL $1: $3"; }
    CHECKS+="${CHECKS:+,}{\"name\":$(jstr "$1"),\"ok\":$ok,\"detail\":$(jstr "$3")}"
}

gate_nvml() {
    tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
    [[ -x $VMSH ]] || verdict skip "launcher $VMSH missing"
    command -v nvidia-smi >/dev/null || verdict skip "host nvidia-smi missing (needed for the comparison)"
    "$VMSH" ssh true 2>/dev/null || verdict skip "guest not reachable; start it with tools/caraxes-vm.sh up && tools/caraxes-vm.sh wait"
    local smi="sudo LD_LIBRARY_PATH=$SMI_DIR/lib $SMI_DIR/bin/nvidia-smi"
    "$VMSH" ssh "test -x $SMI_DIR/bin/nvidia-smi" || verdict skip "guest has no $SMI_DIR/bin/nvidia-smi"

    nvidia-smi -q > "$tmp/host-before.txt" 2>"$tmp/host.err" || verdict skip "host nvidia-smi -q failed: $(head -1 "$tmp/host.err")"
    if ! "$VMSH" ssh "$smi" > "$tmp/guest-smi.txt" 2>&1; then
        say "guest nvidia-smi:"; sed 's/^/  /' "$tmp/guest-smi.txt" >&2
        verdict fail "guest nvidia-smi exited non-zero"
    fi
    if ! "$VMSH" ssh "$smi -q" > "$tmp/guest-q.txt" 2>&1; then
        verdict fail "guest nvidia-smi -q exited non-zero"
    fi
    nvidia-smi -q > "$tmp/host-after.txt" 2>/dev/null || cp "$tmp/host-before.txt" "$tmp/host-after.txt"

    local g=$tmp/guest-q.txt name total reserved used free temp pstate power util fan
    name=$(field "$g" '*' 'Product Name')
    total=$(num "$(field "$g" 'FB Memory Usage' Total)")
    reserved=$(num "$(field "$g" 'FB Memory Usage' Reserved)")
    used=$(num "$(field "$g" 'FB Memory Usage' Used)")
    free=$(num "$(field "$g" 'FB Memory Usage' Free)")
    temp=$(field "$g" Temperature 'GPU Current Temp')
    pstate=$(field "$g" '*' 'Performance State')
    power=$(field "$g" 'GPU Power Readings' 'Instantaneous Power Draw')
    util=$(field "$g" Utilization GPU)
    fan=$(field "$g" '*' 'Fan Speed')
    GJSON="\"name\":$(jstr "$name"),\"total_mib\":$(jstr "$total"),\"reserved_mib\":$(jstr "$reserved"),\"used_mib\":$(jstr "$used"),\"free_mib\":$(jstr "$free"),\"temp\":$(jstr "$temp"),\"pstate\":$(jstr "$pstate"),\"power\":$(jstr "$power"),\"util\":$(jstr "$util"),\"fan\":$(jstr "$fan")"

    # Identity: device-owned (decision D6), never the host's.
    check name "$([[ $name == "$WANT_NAME" ]] && echo 1)" "product name '$name', want '$WANT_NAME'"
    check smi-row "$(grep -q "$WANT_NAME" "$tmp/guest-smi.txt" && grep -q "/ *${WANT_MIB}MiB" "$tmp/guest-smi.txt" && echo 1)" \
        "nvidia-smi table shows the name and ${WANT_MIB}MiB"
    check total "$([[ $total == "$WANT_MIB" ]] && echo 1)" "FB total ${total:-N/A} MiB, want $WANT_MIB"
    local sum=$(( ${reserved:-0} + ${used:-0} + ${free:-0} ))
    check usage "$([[ -n $reserved && -n $used && -n $free ]] && (( used <= ${total:-0} && sum >= ${total:-0} - 2 && sum <= ${total:-0} + 2 )) && echo 1)" \
        "reserved $reserved + used $used + free $free = $sum MiB of ${total:-?}"
    # Telemetry: numbers, not N/A.
    check temp-number "$([[ -n $(num "$temp") ]] && echo 1)" "GPU Current Temp '$temp'"
    check pstate-number "$([[ $pstate =~ ^P[0-9]+$ ]] && echo 1)" "Performance State '$pstate'"
    check power-number "$([[ -n $(num "$power") ]] && echo 1)" "Instantaneous Power Draw '$power'"
    check util-number "$([[ -n $(num "$util") ]] && echo 1)" "Utilization GPU '$util'"

    # Pass-through is real: the same card seen from the host, sampled around the guest run.
    local h1=$tmp/host-before.txt h2=$tmp/host-after.txt ht1 ht2 hp1 hp2 hname hpower
    ht1=$(num "$(field "$h1" Temperature 'GPU Current Temp')"); ht2=$(num "$(field "$h2" Temperature 'GPU Current Temp')")
    hp1=$(field "$h1" '*' 'Performance State'); hp2=$(field "$h2" '*' 'Performance State')
    hname=$(field "$h1" '*' 'Product Name'); hpower=$(field "$h2" 'GPU Power Readings' 'Instantaneous Power Draw')
    HJSON="\"name\":$(jstr "$hname"),\"temp_before\":$(jstr "$ht1"),\"temp_after\":$(jstr "$ht2"),\"pstate_before\":$(jstr "$hp1"),\"pstate_after\":$(jstr "$hp2"),\"power\":$(jstr "$hpower")"
    local gt lo hi; gt=$(num "$temp")
    if [[ -n $gt && -n $ht1 && -n $ht2 ]]; then
        lo=$(( (ht1 < ht2 ? ht1 : ht2) - TOL )); hi=$(( (ht1 > ht2 ? ht1 : ht2) + TOL ))
        check temp-matches-host "$(( ${gt%.*} >= lo && ${gt%.*} <= hi ))" "guest $gt C, host $ht1..$ht2 C, tolerance $TOL"
    else
        check temp-matches-host 0 "guest '$temp', host '$ht1'/'$ht2'"
    fi
    local gf; gf=$(family "$pstate")
    check pstate-family "$([[ $gf != unknown && ( $gf == "$(family "$hp1")" || $gf == "$(family "$hp2")" ) ]] && echo 1)" \
        "guest $pstate ($gf), host $hp1/$hp2"
    check host-name-differs "$([[ -n $hname && $hname != "$name" ]] && echo 1)" "host '$hname' vs guest '$name' (identity not forwarded)"

    if ((${#FAILED[@]})); then verdict fail "failed: ${FAILED[*]}"; fi
    verdict pass "identity device-owned, telemetry forwarded and consistent with the host"
}

# ---- the v1 gates: cuda, gpu, vdisplay, display ------------------------------------
# The verdict contract of Leandro v1 (../Leandro-Test scripts/lib/common.sh lea_gate_*),
# restated here because that library needs the v1 rig configuration: stdout carries only
# the one JSON line (fd 3), human text goes to stderr, a stage is pass or fail, skip is
# reserved for a missing precondition, and a gate that dies before its verdict emits a
# fail through the EXIT trap.
VM=${VM:-vm0}
if [[ -z ${CARAXES_STORE:-} ]]; then
    if [[ -d /mnt/vmstore/caraxes ]]; then CARAXES_STORE=/mnt/vmstore/caraxes; else CARAXES_STORE=$LEA_ROOT/target/caraxes; fi
fi
TRACE=$CARAXES_STORE/$VM/gpu-trace.log
GUEST_TESTS=${CARAXES_GUEST_TESTS:-/home/leandro/caraxes-tests}
LEA_TEST=${CARAXES_LEANDRO_TEST:-$(cd "$LEA_ROOT/.." && pwd -P)/Leandro-Test}
CTRL_HDRS=$LEA_ROOT/vendor/open-gpu-kernel-modules/src/common/sdk/nvidia/inc/ctrl
TMO=${CARAXES_TEST_TIMEOUT:-120}
G_NAME="" G_T0=0 G_DONE=0 RUN=""
G_STAGES=() G_FAILED=() G_FKEYS=() G_FVALS=()

jarr() { local first=1 i; printf '['; for i in "$@"; do ((first)) || printf ','; first=0; jstr "$i"; done; printf ']'; }
jval() { if [[ $1 =~ ^-?[0-9]+(\.[0-9]+)?$ ]]; then printf '%s' "$1"; else jstr "$1"; fi; }

g_begin() {
    G_NAME=$1; G_T0=$(date +%s)
    local base n=1
    base=$(realpath -m "${CARAXES_GATE_RUNS:-$LEA_ROOT/caraxes/runs}")/gate-$1-$(date +%Y-%m-%d); RUN=$base
    while [[ -e $RUN ]]; do n=$((n + 1)); RUN=$base-$n; done
    mkdir -p "$RUN"
    { echo "# SPDX-License-Identifier: MIT"; echo "# tools/caraxes-gate.sh $1 transcript, VM $VM, $(date -Iseconds)"; } > "$RUN/transcript.log"
    exec 3>&1 > >(tee -a "$RUN/transcript.log" >&2) 2>&1
    trap g_atexit EXIT
}
g_atexit() {
    local rc=$?
    ((G_DONE)) && return
    g_emit fail "exited before the verdict (pending status $rc)"
    exit 1
}
g_pass() { G_STAGES+=("$1"); echo "  GATE $1: PASS  ($2)"; }
g_fail() { G_STAGES+=("$1"); G_FAILED+=("$1"); echo "  GATE $1: FAIL  ($2)"; }
g_fact() {
    local i
    for i in "${!G_FKEYS[@]}"; do [[ ${G_FKEYS[i]} == "$1" ]] && { G_FVALS[i]=${2:-}; return; }; done
    G_FKEYS+=("$1"); G_FVALS+=("${2:-}")
}
g_emit() {
    local i line
    G_DONE=1
    line=$(printf '{"gate":%s,"result":"%s","dur_s":%d' "$(jstr "$G_NAME")" "$1" "$(( $(date +%s) - G_T0 ))"
        [[ $1 != pass ]] && printf ',"reason":%s' "$(jstr "$2")"
        printf ',"facts":{"stages":%s,"failed":%s' "$(jarr "${G_STAGES[@]}")" "$(jarr "${G_FAILED[@]}")"
        for i in "${!G_FKEYS[@]}"; do printf ',%s:%s' "$(jstr "${G_FKEYS[i]}")" "$(jval "${G_FVALS[i]}")"; done
        printf '}}')
    if [[ -n $RUN ]]; then echo "$line" > "$RUN/verdict.json"; spdx_run; fi
    echo "$line" >&3
}
g_skip() { echo; echo "${G_NAME^^}-GATE: SKIP -- $1"; g_emit skip "$1"; exit 2; }
g_finish() {
    [[ -n ${1:-} ]] && g_fact detail "$1"
    echo
    if ((${#G_FAILED[@]} == 0)); then echo "${G_NAME^^}-GATE: PASS (${G_STAGES[*]:-none})"; g_emit pass; exit 0; fi
    echo "${G_NAME^^}-GATE: FAIL -- ${G_FAILED[*]}"; g_emit fail "failed: ${G_FAILED[*]}"; exit 1
}
gsh() { "$VMSH" ssh "$@"; }

# The preconditions every guest-side v1 gate shares: all skip, never fail.
g_preconditions() {
    [[ -x $VMSH ]] || g_skip "launcher $VMSH missing"
    gsh true 2>/dev/null || g_skip "guest not reachable; start it with tools/caraxes-vm.sh up && tools/caraxes-vm.sh wait"
    gsh "test -x $GUEST_TESTS/caraxes/run-tests.sh && test -x $GUEST_TESTS/leandro-test/probe/bin/nvprobe" \
        || g_skip "no test material in the guest; run tools/caraxes-vm.sh sync-tests"
    # Stale material is a precondition, as a stale binary is in v1 (lea_require_built).
    local want have
    want=$(md5sum < "$LEA_ROOT/tools/caraxes-guest-tests.sh" | cut -d' ' -f1)
    have=$(gsh "md5sum < $GUEST_TESTS/caraxes/run-tests.sh" | cut -d' ' -f1)
    [[ $want == "$have" ]] || g_skip "the guest runner differs from tools/caraxes-guest-tests.sh; run tools/caraxes-vm.sh sync-tests"
    gsh 'test -e /dev/nvidia0 && grep -q "^nvidia " /proc/modules' \
        || g_skip "nvidia.ko not loaded or /dev/nvidia0 missing; run tools/caraxes-vm.sh probe"
    [[ -r $TRACE ]] || g_skip "device trace $TRACE not readable (is the server running with --trace?)"
    gsh "cat $GUEST_TESTS/SYNC.txt" > "$RUN/guest-sync.txt" 2>/dev/null
    g_fact vm "$VM"
    g_fact leandro_test "$(sed -n 's/^leandro-test \([^ ]*\).*/\1/p' "$RUN/guest-sync.txt")"
    g_fact caraxes "$(git -C "$LEA_ROOT" describe --always --dirty --abbrev=12 2>/dev/null)"
    g_fact device "$(grep -m1 -o 'TU106.*FB [0-9]* MiB' "$CARAXES_STORE/$VM/gpu.log" 2>/dev/null)"
}

# ctrl_name 0x2080182a -> NV2080_CTRL_CMD_BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS (vendor headers).
declare -A CTRL_NAME=()
ctrl_name() {
    if [[ -z ${CTRL_NAME[$1]+x} ]]; then
        CTRL_NAME[$1]=$(grep -rhoiE "#define +NV[0-9A-F]+_CTRL_CMD_[A-Z0-9_]+ +\(0x0*${1#0x}U?\)" "$CTRL_HDRS" 2>/dev/null \
            | head -1 | awk '{print $2}')
    fi
    printf '%s' "${CTRL_NAME[$1]:-$1}"
}

# rm_name "ctl 0x2a 0x2080182a status 0x56": the guest tracer's last failed RM call, named.
rm_name() {
    local node esc cmd _ st
    read -r node esc cmd _ st <<< "$1"
    [[ $esc == 0x* && -n $st ]] || { printf '%s' "$1"; return; }
    case $esc in
        0x2a) printf 'RM_CONTROL %s -> %s' "$(ctrl_name "$cmd")" "$st" ;;
        0x2b) printf 'RM_ALLOC class %s -> %s' "$cmd" "$st" ;;
        0x29) printf 'RM_FREE -> %s' "$st" ;;
        *) printf '%s esc %s %s -> %s' "$node" "$esc" "$cmd" "$st" ;;
    esac
}

# device_summary FILE: the RPCs of a test's window that did not come back NV_OK.
device_summary() {
    local line id st out="" n
    while read -r n id st; do
        out+="${out:+; }$(ctrl_name "$id")=$st${n:+ x$n}"
    done < <(grep -oE 'ctrl (0x[0-9a-f]+):.*-> status (0x[0-9a-f]+)' "$1" \
        | sed -E 's/ctrl (0x[0-9a-f]+):.*-> status (0x[0-9a-f]+)/\1 \2/' \
        | awk '$2 != "0x0"' | sort | uniq -c | awk '{print $1, $2, $3}')
    while read -r line; do out+="${out:+; }$line"; done < <(grep -oE 'alloc: .*-> status 0x0*[1-9a-f][0-9a-f]*' "$1" | sort -u | head -3)
    printf '%s' "${out:-none}"
}

# run_tests NAME...: one ssh call per test, each with its window of the device's RPC
# trace (--trace, gpu-trace.log) and of its stderr (gpu.log). The first test's window is
# also kept whole, register accesses included.
declare -A ROW_RES=() ROW_SCORED=() ROW_DETAIL=() ROW_RM=() ROW_DEV=()
ROW_ORDER=()
run_tests() {
    local t n0 n1 e0 e1 w0 line gout full=1 dlog=$CARAXES_STORE/$VM/gpu.log
    gout=$GUEST_TESTS/out/$(basename "$RUN")
    mkdir -p "$RUN/guest" "$RUN/device"
    w0=$(wc -l < "$TRACE"); e0=$(wc -l < "$dlog" 2>/dev/null || echo 0)
    for t in "$@"; do
        n0=$(wc -l < "$TRACE")
        say "$t"
        line=$(timeout $((TMO * 10 + 60)) "$VMSH" ssh "CARAXES_TEST_TIMEOUT=$TMO $GUEST_TESTS/caraxes/run-tests.sh $gout $t" 2>&1 | grep -a '^CXROW' | tail -1)
        n1=$(wc -l < "$TRACE")
        if ((n1 >= n0)); then
            ((full)) && sed -n "$((n0 + 1)),${n1}p" "$TRACE" > "$RUN/device/$t.full.trace"
            sed -n "$((n0 + 1)),${n1}p" "$TRACE" | grep -aEv ' [RW] bar[0-9] ' > "$RUN/device/$t.trace"
            echo "# $((n1 - n0)) device trace lines in this window; register accesses dropped" >> "$RUN/device/$t.trace"
        else
            echo "# the device trace restarted during this test (server restarted?)" > "$RUN/device/$t.trace"
        fi
        full=0
        [[ -n $line ]] || line=$(printf 'CXROW\t%s\tfail\t1\tno result row (guest runner died or ssh timed out)\t-' "$t")
        IFS=$'\t' read -r _ _ ROW_RES[$t] ROW_SCORED[$t] ROW_DETAIL[$t] ROW_RM[$t] <<< "$line"
        ROW_RM[$t]=$(rm_name "${ROW_RM[$t]:--}")
        ROW_DEV[$t]=$(device_summary "$RUN/device/$t.trace")
        ROW_ORDER+=("$t")
        echo "  ${ROW_RES[$t]}: ${ROW_DETAIL[$t]}"
        [[ ${ROW_RES[$t]} == pass ]] || echo "  guest RM: ${ROW_RM[$t]}; device: ${ROW_DEV[$t]}"
    done
    # The whole run's window: RPC events and the server's stderr.
    n1=$(wc -l < "$TRACE"); e1=$(wc -l < "$dlog" 2>/dev/null || echo 0)
    ((n1 >= w0)) && sed -n "$((w0 + 1)),${n1}p" "$TRACE" | grep -aEv ' [RW] bar[0-9] ' > "$RUN/device/window.trace"
    if ((e1 > e0)); then sed -n "$((e0 + 1)),${e1}p" "$dlog" > "$RUN/device/server-stderr.log"
    else echo "# $dlog gained no lines during this run (the RPC trace is in window.trace)" > "$RUN/device/server-stderr.log"; fi
    # Fetch the guest's logs.
    rsync -a -e "ssh -i ${CARAXES_SSH_KEY:-/mnt/vmstore/leandro/vm/id_leandro} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR" \
        "leandro@${CARAXES_IP:-192.168.100.20}:$gout/" "$RUN/guest/" 2>/dev/null || echo "  (could not fetch the guest logs from $gout)"
}

# spdx_run: every text file of the run gets an SPDX header (JSON has no comment syntax).
spdx_run() {
    local f
    while IFS= read -r f; do
        head -1 "$f" | grep -q 'SPDX-License-Identifier' && continue
        { echo "# SPDX-License-Identifier: MIT"; echo "# $G_NAME gate $(basename "$RUN"): ${f#"$RUN"/}"; cat "$f"; } > "$f.tmp" && mv "$f.tmp" "$f"
    done < <(find "$RUN" -type f ! -name '*.json' ! -name '*.tmp' ! -name transcript.log)
}

# write_table: the per-test table, on stderr and as table.txt.
write_table() {
    local t
    {
        echo "# SPDX-License-Identifier: MIT"
        echo "# $G_NAME gate, $(basename "$RUN"), VM $VM: result per test; device = RPCs the device did not answer NV_OK in the test's window"
        printf '%-32s %-5s %-6s %s\n' TEST RESULT SCORED "GUEST ERROR | LAST FAILED RM CALL (guest tracer) | DEVICE"
        for t in "${ROW_ORDER[@]}"; do
            printf '%-32s %-5s %-6s %s | %s | %s\n' "$t" "${ROW_RES[$t]}" "$([[ ${ROW_SCORED[$t]} == 1 ]] && echo yes || echo no)" \
                "${ROW_DETAIL[$t]}" "${ROW_RM[$t]}" "${ROW_DEV[$t]}"
        done
    } > "$RUN/table.txt"
    echo; sed -n '3,$p' "$RUN/table.txt"
}

gate_cuda() {
    g_begin cuda
    g_preconditions
    local -a tests=("$@")
    ((${#tests[@]})) || mapfile -t tests < <(gsh "$GUEST_TESTS/caraxes/run-tests.sh --list" | grep -v '^gpu-')
    g_fact tests_selected "${*:-all}"
    ((${#tests[@]})) || g_skip "the guest runner listed no tests"
    run_tests "${tests[@]}"
    write_table
    local t npass=0 nfail=0 nskip=0 ninfo=0
    for t in "${ROW_ORDER[@]}"; do
        if [[ ${ROW_SCORED[$t]} != 1 ]]; then
            ninfo=$((ninfo + 1)); g_fact "info_$t" "${ROW_RES[$t]}: ${ROW_DETAIL[$t]}"; continue
        fi
        case ${ROW_RES[$t]} in
            pass) npass=$((npass + 1)); g_pass "$t" "${ROW_DETAIL[$t]}" ;;
            skip) nskip=$((nskip + 1)); G_STAGES+=("$t"); g_fact "skipped_$t" "${ROW_DETAIL[$t]}" ;;
            *) nfail=$((nfail + 1)); g_fail "$t" "${ROW_DETAIL[$t]} | rm: ${ROW_RM[$t]} | device: ${ROW_DEV[$t]}"
               g_fact "error_$t" "${ROW_DETAIL[$t]}"; g_fact "rm_$t" "${ROW_RM[$t]}"; g_fact "device_$t" "${ROW_DEV[$t]}" ;;
        esac
    done
    g_fact scored "$npass pass, $nfail fail, $nskip declared-unsupported"
    g_fact suites_unscored "$ninfo"
    g_fact run_dir "${RUN#"$LEA_ROOT"/}"
    g_finish "$npass of $((npass + nfail + nskip)) scored tests pass; table in ${RUN#"$LEA_ROOT"/}/table.txt"
}

# The v1 gpu gate (scripts/test.sh gate_gpu) on the stages a guest can carry. Its
# `tables` stage checks virtio_nvrm's descriptor tables and its counter-check unloads
# virtio_nvrm; neither exists on the stock driver, so both are reported, not run.
gate_gpu() {
    g_begin gpu
    g_preconditions
    g_fact tables "not applicable: virtio_nvrm descriptor-table checksum; the stock driver has none"
    g_fact counter_check "not applicable: unloads virtio_nvrm"

    # smi: v1's masked comparison with the host's nvidia-smi (read-only NVML on the host).
    say "smi -- nvidia-smi against the native host run, masked as v1 masks it"
    local gpuname want mediated
    nvidia-smi > "$RUN/smi-host.txt" 2>&1
    gsh 'sudo LD_LIBRARY_PATH=/opt/nvrm/lib timeout 90 /opt/nvrm/bin/nvidia-smi; echo "module=$?"' > "$RUN/smi-guest.txt" 2>&1
    want=$(tr -d '[:space:]' < "$LEA_ROOT/DRIVER_VERSION")
    gpuname=$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1)
    mediated=${CARAXES_GATE_NAME:-Leandro RTX 2070-4G}
    smask() { sed -e '1d' -e '/| Processes:/,$d' -e '/^module=/d' -e "s/$mediated/CARD/g" -e "s/$gpuname/CARD/g" \
                  -e 's/[0-9][0-9]*/N/g' -e 's/  */ /g' "$1"; }
    diff <(smask "$RUN/smi-host.txt") <(smask "$RUN/smi-guest.txt") > "$RUN/smi.diff"
    g_fact host_gpu "$gpuname"
    if [[ ! -s $RUN/smi.diff ]] && grep -q 'module=0' "$RUN/smi-guest.txt" && grep -q "$want" "$RUN/smi-guest.txt" \
       && grep -qF "$mediated" "$RUN/smi-guest.txt"; then
        g_pass smi "nvidia-smi identical to the native host run (masked), driver $want, guest sees $mediated"
    else
        local rows ln
        rows=$(grep -oE '^[0-9]+' "$RUN/smi.diff" | sort -un | head -5)
        for ln in $rows; do
            echo "  host  $(sed -n "$((ln + 1))p" "$RUN/smi-host.txt")"
            echo "  guest $(sed -n "$((ln + 1))p" "$RUN/smi-guest.txt")"
        done | tee "$RUN/smi-unmasked.txt"
        g_fail smi "masked diff $(grep -c '^[<>]' "$RUN/smi.diff") lines; see smi-unmasked.txt"
        g_fact smi_diff "$(grep -E '^>' "$RUN/smi.diff" | head -3 | tr '\n' ' ')"
    fi

    run_tests gpu-memory gpu-kernel gpu-torch gpu-robustness gpu-own gpu-encode
    write_table
    local t stage
    for t in gpu-memory gpu-kernel gpu-robustness gpu-own gpu-encode; do
        stage=${t#gpu-}
        if [[ ${ROW_RES[$t]} == pass ]]; then g_pass "$stage" "${ROW_DETAIL[$t]}"
        else g_fail "$stage" "${ROW_DETAIL[$t]} | device: ${ROW_DEV[$t]}"; g_fact "error_$stage" "${ROW_DETAIL[$t]}"; fi
    done

    # torch: bit-identical to the native reference, as in v1.
    local ref=$LEA_ROOT/caraxes/reference/rtx2070/torch-native.txt rm ra gm ga
    if [[ ${CARAXES_GATE_NATIVE:-0} == 1 ]]; then
        local hp=${LEA_HOSTVENV:-$LEA_TEST/vendor/hostvenv}/bin/python
        say "torch reference -- native run on the host card (CARAXES_GATE_NATIVE=1)"
        if [[ -x $hp ]]; then
            { echo "# SPDX-License-Identifier: MIT"; echo "# native torch reference on the host card, $(date -Iseconds), $hp"
              NVPROBE_PTX=$LEA_TEST/probe/kernels/kernels.ptx timeout 300 "$hp" "$LEA_TEST/probe/python/rlprobe.py" 2>&1 | tail -2
              timeout 900 "$hp" "$LEA_TEST/probe/python/convburn.py" 2>&1 | tail -1; } > "$ref"
        else echo "  no host venv at $hp"; fi
    fi
    gm=$(grep -o 'mean10=[0-9.]*' <<< "${ROW_DETAIL[gpu-torch]}"); ga=$(grep -o 'acc=[0-9.e+-]*' <<< "${ROW_DETAIL[gpu-torch]}")
    [[ -f $ref ]] && { rm=$(grep -o 'mean10=[0-9.]*' "$ref" | tail -1); ra=$(grep -o 'acc=[0-9.e+-]*' "$ref" | tail -1); }
    if [[ ${ROW_RES[gpu-torch]} != pass ]]; then
        g_fail torch "${ROW_DETAIL[gpu-torch]} | device: ${ROW_DEV[gpu-torch]}"; g_fact error_torch "${ROW_DETAIL[gpu-torch]}"
    elif [[ -z ${rm:-} || -z ${ra:-} ]]; then
        g_fail torch "guest $gm $ga, but no native reference ($ref; CARAXES_GATE_NATIVE=1 runs it on the host card)"
    elif [[ $gm == "$rm" && $ga == "$ra" ]]; then
        g_pass torch "rlprobe $gm and convburn $ga bit-identical to the reference (native)"
    else
        g_fail torch "module($gm,$ga) vs reference-native($rm,$ra)"
    fi
    g_fact run_dir "${RUN#"$LEA_ROOT"/}"
    if ((${#G_FAILED[@]} == 0)); then g_finish "smi..torch + robustness + own + encode (ref=native), details in ${RUN#"$LEA_ROOT"/}/"
    else g_finish "failed: ${G_FAILED[*]}; logs in ${RUN#"$LEA_ROOT"/}/"; fi
}

# vdisplay and display: v1's display gates stand on virtio_nvrm's virtual display and on
# nvidia-modeset/nvidia-drm. Caraxes serves no display engine yet, so both say why they
# do not run, with what the guest has as facts.
gate_display_like() {
    g_begin "$1"
    gsh true 2>/dev/null || g_skip "guest not reachable; start it with tools/caraxes-vm.sh up && tools/caraxes-vm.sh wait"
    g_fact vm "$VM"
    g_fact modeset_module "$(gsh 'modinfo -F filename nvidia-modeset 2>/dev/null || echo none')"
    g_fact drm_module "$(gsh 'modinfo -F filename nvidia-drm 2>/dev/null || echo none')"
    g_fact loaded "$(gsh "grep -oE '^nvidia[a-z_-]*' /proc/modules | tr '\n' ' '")"
    g_fact dri_nodes "$(gsh 'ls /dev/dri 2>/dev/null | tr "\n" " "')"
    case $1 in
        vdisplay) g_skip "not applicable yet: v1 vdisplay tests virtio_nvrm's virtual display (module parameter vdisplay=1, the module's EDID, a frame read back through nvidia-drm on the v1 transport). Caraxes has no display engine in the device (no display class, no connector, no EDID source) and nvidia-modeset/nvidia-drm are out of scope until the compute path works (kickoff.md section 6)" ;;
        *) g_skip "not applicable yet: v1 display needs nvidia-drm with a connector and modeset on the device (plus X, Vulkan presentation and Sunshine/Moonlight in the guest image). Caraxes serves no display engine yet and nvidia-modeset/nvidia-drm are out of scope until the compute path works (kickoff.md section 6)" ;;
    esac
}

case ${1:-} in
    nvml) gate_nvml ;;
    cuda) shift; gate_cuda "$@" ;;
    gpu) gate_gpu ;;
    vdisplay|display) gate_display_like "$1" ;;
    -h|--help) usage 0 ;;
    *) usage 2 ;;
esac
