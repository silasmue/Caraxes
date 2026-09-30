#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Train A/B session (2026-09-30): the bench mode of caraxes/runs/ctxcreate-2026-09-29/run.sh
# (lean vm2-615, 615 guest, trace off, CARAXES_PROFILE=1) at the 1 GiB profile, plus a root
# copy check for the admin copy channels. Run it inside vmlock.sh vm2-615; it stops its VM.
# Usage: ab.sh BIN LABEL [INIT_REPS]
#   The VRAM slot is chosen at start: the first 1 GiB slot of 4 0 5 1 2 3 that overlaps no
#   running device's reserved range (from its gpu.log); the launcher's own headroom check still
#   decides.
set -uo pipefail
BIN=${1:?binary} LABEL=${2:?label} REPS=${3:-20}
HERE=/mnt/vmstore/caraxes/runs-2026-09-30/train
TOOLS=/home/silas/git/Caraxes/caraxes/tools
MAIN=/home/silas/git/Caraxes
CBIN=/mnt/vmstore/caraxes/ctxcreate-2026-09-29/bin
C=$HERE/ab/$LABEL
[[ -x $BIN ]] || { echo "ERROR: $BIN is not an executable" >&2; exit 2; }
[[ -e $C ]] && { echo "ERROR: $C exists" >&2; exit 2; }
[[ -x $CBIN/census-cu ]] || { echo "ERROR: $CBIN/census-cu missing" >&2; exit 2; }

# Reserved ranges of the running devices.
TOP=$((0x1e9800000)) G=$((1 << 30))
used=()
for p in $(pgrep -f 'vfio-user-nvgpu.* --socket '); do
    sock=$(tr '\0' '\n' < /proc/$p/cmdline 2>/dev/null | grep -A1 -x -- '--socket' | tail -1)
    [[ -n $sock ]] || continue
    log=$(dirname "$sock")/gpu.log
    r=$(grep -a -o 'reserved host VRAM \[0x[0-9a-f]*, 0x[0-9a-f]*)' "$log" 2>/dev/null | tail -1 | grep -o '0x[0-9a-f]*' | tr '\n' ' ')
    [[ -n $r ]] && used+=("$r")
done
SLOT=
for n in 4 0 5 1 2 3; do
    lo=$((TOP - (n + 1) * G)) hi=$((TOP - n * G)) ok=1
    for r in "${used[@]}"; do
        set -- $r
        (( lo < $2 && $1 < hi )) && ok=0
    done
    (( ok )) && { SLOT=$n; break; }
done
[[ -n $SLOT ]] || { echo "ERROR: no free 1 GiB slot (used: ${used[*]})" >&2; exit 75; }

# MX_NOPOOL=1 (default here): no BAR1 pool. Beside the Windows VM the host has 160 MiB of BAR1
# free, and a 32 MiB pool with its windows (96 MiB) would leave less than the 96 MiB floor
# (bar1pool.rs check_host_bar1); both arms of an A/B run the same way.
export MX_NOPOOL=${MX_NOPOOL:-1}
source "$MAIN/caraxes/runs/matrix-2026-09-25/env.sh"
mx_guest wl615
export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=$SLOT
V2_NODES='sudo modprobe nvidia-uvm
    [ -e /dev/nvidiactl ] || sudo mknod -m 666 /dev/nvidiactl c 195 255
    [ -e /dev/nvidia0 ] || sudo mknod -m 666 /dev/nvidia0 c 195 0
    u=$(grep nvidia-uvm /proc/devices | cut -d" " -f1); sudo rm -f /dev/nvidia-uvm /dev/nvidia-uvm-tools
    sudo mknod -m 666 /dev/nvidia-uvm c $u 0; sudo mknod -m 666 /dev/nvidia-uvm-tools c $u 1'
export CARAXES_SERVER=$BIN CARAXES_LAUNCH_TIMING=1 CARAXES_PROFILE=1 CARAXES_PROFILE_OUT=$C/profile
export CARAXES_TRACE=0
mx_free || exit 2
mkdir -p "$C"
echo "slot $SLOT (running devices' ranges: ${used[*]:-none}), MX_NOPOOL=$MX_NOPOOL" > "$C/slot.txt"

GS=$C/guest-script.sh
{ echo 'set +e'
  echo 'mkdir -p ~/ctx && cd ~/ctx || exit 1'
  echo "base64 -d > census-cu <<'B64'"; base64 "$CBIN/census-cu"; echo 'B64'
  cat <<'EOF'
chmod +x census-cu; cp ~/bench/kernels.ptx . || exit 1
echo "# census-cu $(sha256sum census-cu | cut -c1-16), bench-cu $(sha256sum ~/bench/bench-cu | cut -c1-16)"
./census-cu ptx2cubin kernels.ptx census.cubin || echo "ptx2cubin failed"
x() { echo "--- xid: $(sudo dmesg | grep -c 'NVRM: Xid')"; }
export CENSUS_PTX=$PWD/kernels.ptx CENSUS_CUBIN=$PWD/census.cubin
EOF
  echo "echo '=== bench-cu init'; for i in \$(seq $REPS); do ~/bench/bench-cu init; done"
  cat <<'EOF'
echo "=== bench-cu churn"; for i in 1 2 3; do ~/bench/bench-cu churn 20; done; x
echo "=== census-cu ctx"; timeout 240 ./census-cu ctx 5
echo "=== census-cu ops"; timeout 240 ./census-cu ops 10; x
echo "=== user bench-cu copy"; timeout 200 ~/bench/bench-cu copy 3 2>&1 | grep -E "^BW|^CHECK|ERROR"; x
echo "=== root bench-cu copy"; sudo timeout 200 ~/bench/bench-cu copy 3 2>&1 | grep -E "^BW|^CHECK|ERROR" | sed 's/^/ROOT /'; x
echo "=== root bench-cu init"; for i in 1 2 3; do sudo timeout 60 ~/bench/bench-cu init 2>&1 | sed 's/^/ROOT /'; done; x
EOF
  echo 'sudo dmesg | grep -E "NVRM: Xid" | tail -5 | cut -c1-200'
} > "$GS"

up=0
cleanup() { (( up )) && { "$LEA_TOOLS/caraxes-vm.sh" down >> "$C/down.txt" 2>&1; up=0; }; }
trap cleanup EXIT INT TERM
T0=$(date '+%F %T')
mx_setup "$C"
if up=1 && mx_up "$C" && : > "$MX_W/gpu-trace.log" && "$LEA_TOOLS/caraxes-vm.sh" probe "train-$LABEL" | tail -n 2 >> "$C/up.txt" &&
    "$LEA_TOOLS/caraxes-vm.sh" ssh "$V2_NODES" >> "$C/up.txt" 2>&1; then
    G0=$(wc -l < "$MX_W/gpu.log")
    timeout 900 "$LEA_TOOLS/caraxes-vm.sh" ssh 'bash -s' < "$GS" > "$C/guest.txt" 2>&1
    echo "guest rc=$? $(grep -c '^OP' "$C/guest.txt") OP, $(grep -c '^INIT' "$C/guest.txt") INIT, $(grep -c '^CHURN' "$C/guest.txt") CHURN lines"
    kill -USR2 "$(cat "$MX_W/gpu.pid")" 2>/dev/null; sleep 2
    tail -n +"$((G0 + 1))" "$MX_W/gpu.log" > "$C/gpu-during.log"
else
    echo "up failed" | tee -a "$C/guest.txt"
fi
mx_finish "$C" "$T0"; up=0
cp "$MX_W/gpu.log" "$C/gpu.log"
grep -a "launch timing" "$MX_W/gpu.log" > "$C/launch-timing.txt"
P=$(ls "$C"/profile.*.tsv 2>/dev/null | grep -v exit | tail -1)
[[ -n $P ]] && python3 "$TOOLS/profile-rank.py" "$P" --top 60 > "$C/profile-device-rank.md" 2>&1
for f in "$C"/profile.*.tsv; do [[ -f $f ]] && xz -T2 "$f"; done
grep -hE '^(ROOT )?(INIT|CHURN|OP|BW|CHECK) ' "$C/guest.txt" > "$C/results.txt"
echo "$(basename "$C"): slot $SLOT, results.txt $(wc -l < "$C/results.txt") lines; host Xid lines $(grep -vc '^#' "$C/host-xid.txt")"
