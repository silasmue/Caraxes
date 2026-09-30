#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The whole Leandro-Test material on one binary (docs/research/ctxcreate-2026-09-29.md):
# every suite and probe of lt-all.sh (the Caraxes runner's CUDA, suite and gpu tests, every
# probe/matrix probe, the C and Python probes, bench-cu), timed, next to the same on the host.
# Modes:
#   vm BIN LABEL [VAR=VALUE ...]  the lean VM (matrix-2026-09-25/env.sh wl615: vm2-615, slot 0)
#       at LT_FB_MIB (default 4096) with the device trace off (LT_TRACE=1: on); up, the
#       launcher's probe, sync-tests, the nvml and gpu gates (tools/caraxes-gate.sh, which
#       read the host's nvidia-smi -q and compare with caraxes/reference), then lt-all.sh in
#       the guest; the guest's output comes back as LABEL/lt-all.tgz, rows in LABEL/rows.tsv.
#   native LABEL  lt-all.sh on the host over a copy of Leandro-Test (probe/bin as built on
#       the host) with the host venv; the tests that take VRAM to the refusal are skipped.
#   table VMLABEL NATIVELABEL  the joined table (VMLABEL/table.md): result and seconds per
#       test, the ratio to native.
# VAR=VALUE pass to the device and the launcher after the lean VM's settings. A run folder is
# never overwritten.
set -uo pipefail
MODE=${1:?usage: lt-suite.sh vm BIN LABEL [VAR=VALUE ...] | native LABEL | table VMLABEL NATIVELABEL}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
MAIN=/home/silas/git/Caraxes
LTSRC=${CARAXES_LEANDRO_TEST:-/home/silas/git/Leandro-Test}
HOSTVENV=${LEA_HOSTVENV:-/home/silas/git/Leandro/vendor/hostvenv}

table() {
    local vm=$HERE/$1 nat=$HERE/$2
    python3 - "$vm/rows.tsv" "$nat/rows.tsv" > "$vm/table.md" <<'EOF'
import sys
def rows(p):
    out = {}
    try:
        for l in open(p):
            f = l.rstrip("\n").split("\t")
            if len(f) >= 5 and f[0] == "LTROW":
                out[f[1]] = (f[2], float(f[3] or 0), f[4])
    except FileNotFoundError:
        pass
    return out
vm, nat = rows(sys.argv[1]), rows(sys.argv[2])
print("| test | VM | VM s | native | native s | VM / native | VM detail |")
print("|---|---|---|---|---|---|---|")
for t, (r, s, d) in vm.items():
    nr, ns, _ = nat.get(t, ("-", 0.0, ""))
    ratio = f"{s / ns:.2f}" if ns > 0.05 and r == "pass" and nr == "pass" else "-"
    print(f"| {t} | {r} | {s:.1f} | {nr} | {ns:.1f} | {ratio} | {d[:160].replace('|', '/')} |")
n = lambda rs, v: sum(1 for x in rs.values() if x[0] == v)
print(f"\nVM: {n(vm, 'pass')} pass, {n(vm, 'fail')} fail, {n(vm, 'skip')} skip; native: {n(nat, 'pass')} pass, {n(nat, 'fail')} fail, {n(nat, 'skip')} skip")
EOF
    cat "$vm/table.md"
}

case $MODE in
table) table "${2:?VMLABEL}" "${3:?NATIVELABEL}"; exit 0 ;;
native)
    LABEL=${2:?label}; C=$HERE/$LABEL
    [[ -e $C ]] && { echo "ERROR: $C exists" >&2; exit 2; }
    R=$C/root; mkdir -p "$R/caraxes" "$R/leandro-test"
    rsync -a --exclude traces --exclude __pycache__ "$LTSRC/probe" "$LTSRC/scripts" "$R/leandro-test/"
    head -1 /proc/driver/nvidia/version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1 > "$R/leandro-test/DRIVER_VERSION"
    cp "$MAIN/tools/caraxes-guest-tests.sh" "$R/caraxes/run-tests.sh"
    cp "$HERE/lt-all.sh" "$R/caraxes/lt-all.sh"
    printf '# SPDX-License-Identifier: MIT\nCARAXES_GUEST_LIBDIR=%s\nCARAXES_GUEST_SMI=%s\n' /usr/lib /usr/bin/nvidia-smi > "$R/caraxes/driver.env"
    { head -1 /proc/driver/nvidia/version; git -C "$LTSRC" describe --always --dirty --abbrev=12; date -Iseconds
      pgrep -af 'vfio-user-nvgpu --socket|cloud-hypervisor' | grep -v pgrep | cut -c1-160; cut -d' ' -f1-3 /proc/loadavg; } > "$C/setup.txt" 2>&1
    LT_ROOT=$R LT_NATIVE=1 LT_BENCH=$MAIN/target/bench/bin CARAXES_GUEST_VENV=$HOSTVENV \
        "$R/caraxes/lt-all.sh" "$C/out" > "$C/lt-all.txt" 2>&1
    grep '^LTROW' "$C/lt-all.txt" > "$C/rows.tsv"
    tail -1 "$C/lt-all.txt"
    exit 0 ;;
vm) ;;
*) echo "ERROR: mode vm|native|table" >&2; exit 2 ;;
esac

BIN=${2:?device binary}; LABEL=${3:?label}; shift 3
C=$HERE/$LABEL
[[ -x $BIN ]] || { echo "ERROR: $BIN is not an executable" >&2; exit 2; }
[[ -e $C ]] && { echo "ERROR: $C exists" >&2; exit 2; }
source "$MAIN/caraxes/runs/matrix-2026-09-25/env.sh"
mx_guest wl615
FB=${LT_FB_MIB:-4096}; export CARAXES_FB_MIB=$FB CARAXES_GATE_FB_MIB=$FB
export CARAXES_SERVER=$BIN CARAXES_TRACE=${LT_TRACE:-0} CARAXES_GATE_RUNS=$C/gates
for kv in "$@"; do export "${kv?}"; done
V2_NODES='sudo modprobe nvidia-uvm
    [ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
    [ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
    u=$(grep nvidia-uvm /proc/devices | cut -d" " -f1); sudo rm -f /dev/nvidia-uvm /dev/nvidia-uvm-tools
    sudo mknod -m 666 /dev/nvidia-uvm c $u 0; sudo mknod -m 666 /dev/nvidia-uvm-tools c $u 1'
mx_free || exit 2
mkdir -p "$C/gates"
VMSH=$LEA_TOOLS/caraxes-vm.sh
T0=$(date '+%F %T')
mx_setup "$C"
"$MAIN/caraxes/tools/bench-hostwatch.sh" start "$C"
# The launcher's probe wants a trace file; with the trace off it gets an empty one.
if mx_up "$C" && { [[ $CARAXES_TRACE == 1 ]] || : > "$MX_W/gpu-trace.log"; } &&
    "$VMSH" probe "lt-$LABEL" | tail -n 2 >> "$C/up.txt" && "$VMSH" ssh "$V2_NODES" >> "$C/up.txt" 2>&1; then
    "$VMSH" sync-tests > "$C/sync-tests.txt" 2>&1; echo "sync-tests rc=$? $(tail -1 "$C/sync-tests.txt" | cut -c1-150)"
    "$VMSH" ssh 'cat > ~/caraxes-tests/caraxes/lt-all.sh && chmod +x ~/caraxes-tests/caraxes/lt-all.sh' < "$HERE/lt-all.sh"
    (cd "$MAIN" && tools/caraxes-gate.sh nvml) > "$C/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?"
    (cd "$MAIN" && tools/caraxes-gate.sh gpu) > "$C/gate-gpu.txt" 2>&1; echo "gpu gate rc=$?"
    "$VMSH" ssh '~/caraxes-tests/caraxes/lt-all.sh ~/caraxes-tests/out/lt-all' > "$C/lt-all.txt" 2>&1
    echo "lt-all rc=$? $(tail -1 "$C/lt-all.txt")"
    "$VMSH" ssh 'cd ~/caraxes-tests/out && tar czf - lt-all' > "$C/lt-all.tgz" 2>/dev/null
    "$VMSH" ssh 'sudo dmesg | grep -iE "nvrm|xid" | tail -60' > "$C/guest-dmesg.txt" 2>&1
else
    echo "up failed" | tee -a "$C/lt-all.txt"
fi
"$MAIN/caraxes/tools/bench-hostwatch.sh" stop "$C"
mx_finish "$C" "$T0"
cp "$MX_W/gpu.log" "$C/gpu.log"
grep '^LTROW' "$C/lt-all.txt" > "$C/rows.tsv"
echo "$(basename "$C"): $(grep -c $'\tpass\t' "$C/rows.tsv") pass, $(grep -c $'\tfail\t' "$C/rows.tsv") fail, $(grep -c $'\tskip\t' "$C/rows.tsv") skip; host Xid lines $(grep -vc '^#' "$C/host-xid.txt")"
