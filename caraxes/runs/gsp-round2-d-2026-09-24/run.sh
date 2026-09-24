#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Round 2 of the category D work (d-device-model.md R2, the open MODEL rows, the ZBC model):
# the nvml and cuda gates on vm3, before and after, with the environment of
# caraxes/runs/gsp-rpc-2026-09-24/run.sh gate610/gate615 (1 GiB profile at reservation
# slot 5, token pool, hybrid reactor, no BAR1 area). The tools run from the main checkout
# R; the device binary is named by SERVER.
# Usage: SERVER=<binary> run.sh gate610|gate615 <label> [zbc-probe-binary]
#   gate610: vm3 (610 guest); gate615: vm3-615 (615 guest).
#   With a third argument the guest runs caraxes/tools/zbc-probe.rs (built statically on
#   the host) after the gates: GF100_ZBC_CLEAR getters, one SET_ZBC_COLOR_CLEAR, readback.
set -uo pipefail
MODE=${1:?usage: SERVER=<binary> run.sh gate610|gate615 <label> [zbc-probe]}
LABEL=${2:?label}
PROBE=${3:-}
: "${SERVER:?SERVER=<device binary>}"
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
R=/home/silas/git/Caraxes
C=$HERE/$MODE-$LABEL
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
if pgrep -f '/mnt/vmstore/caraxes/vm[3]' >/dev/null; then
    echo "ERROR: a vm3 process runs; refusing to start" >&2; pgrep -af '/mnt/vmstore/caraxes/vm[3]' >&2; exit 2
fi
mkdir -p "$C"; cd "$R" || exit 1
export CARAXES_IP=192.168.103.23 CARAXES_TAP=caraxes3 CARAXES_MAC=02:00:00:00:00:23
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_GATE_RUNS=$C
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH CARAXES_DISPLAY
export CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1 CARAXES_MIRROR_GR_SHADOW=1
export CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024 CARAXES_RESERVE_SLOT=5
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
case $MODE in
    gate610) export VM=vm3 CARAXES_SERVER=$SERVER ;;
    gate615) export VM=vm3-615 CARAXES_DRIVER=615 CARAXES_SERVER=$SERVER ;;
    *) echo "ERROR: unknown mode $MODE" >&2; exit 2 ;;
esac
W=/mnt/vmstore/caraxes/$VM; T0=$(date '+%F %T')
{ head -1 /proc/driver/nvidia/version; sha256sum "$CARAXES_SERVER"; env | grep -E '^CARAXES_|^LEA_CH|^VM=' | sort; } > "$C/setup.txt"
caraxes/tools/bench-hostwatch.sh start "$C"
tools/caraxes-vm.sh up > "$C/up.txt" 2>&1 || tail -5 "$C/up.txt"
pgrep -af "vfio-user-nvgpu.*$W/" > "$C/device-pid.txt"
tools/caraxes-vm.sh wait >> "$C/up.txt" 2>&1
tools/caraxes-vm.sh probe round2d >> "$C/up.txt" 2>&1; tail -3 "$C/up.txt" | cut -c1-200
tools/caraxes-vm.sh ssh 'nvidia-smi; sudo dmesg | grep -E "NVRM|nvidia" | grep -vE "ioctl|RmMsg" | tail -60' > "$C/guest.txt" 2>&1
tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
TL0=$(wc -l < "$W/gpu-trace.log")
tools/caraxes-gate.sh nvml > "$C/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?"; tail -1 "$C/gate-nvml.txt" | cut -c1-300
tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -1 "$C/gate-cuda.txt" | cut -c1-300
TL1=$(wc -l < "$W/gpu-trace.log")
if [[ -n $PROBE ]]; then
    tools/caraxes-vm.sh ssh 'cat > /tmp/zbc-probe && chmod +x /tmp/zbc-probe' < "$PROBE"
    tools/caraxes-vm.sh ssh 'sudo /tmp/zbc-probe --set; echo "zbc-probe rc=$?"' > "$C/zbc-probe.txt" 2>&1
    tail -2 "$C/zbc-probe.txt"
fi
TL2=$(wc -l < "$W/gpu-trace.log")
caraxes/tools/bench-hostwatch.sh stop "$C"
T=$W/gpu-trace.log
# The D controls and the ZBC controls the gates and the probe sent, with their answers.
sed -n "$((TL0 + 1)),${TL1}p" "$T" | grep -aE 'ctrl 0x(9096|2080110b|20800177|20800a6a|20800a56|20800130|b01|20800198|801805|20800a6d|20800ac2|2080206f|83de0307|83de0308|20802207)' | cut -c1-400 > "$C/device-dcontrols-gates.txt"
sed -n "$((TL1 + 1)),${TL2}p" "$T" | grep -aE 'ctrl 0x9096|alloc.*0x9096' | cut -c1-400 > "$C/device-zbc-probe.txt"
grep -aE 'RC_TRIGGERED|OS_ERROR_LOG|unsolicited|dmodel' "$T" | cut -c1-400 > "$C/device-rc.txt"
grep -aE 'unanswered|NOT_SUPPORTED|status 0x56|refused' "$T" | sed -E 's/^.*(ctrl 0x[0-9a-f]+|alloc: |rx seq [0-9]+ )/\1/' | sort | uniq -c | sort -rn | head -25 | cut -c1-300 > "$C/device-trace-issues.txt"
grep -E 'unanswered|ERROR|refused|NOT_SUPPORTED' "$W/gpu.log" | sort | uniq -c | sort -rn | head -15 | cut -c1-300 > "$C/device-log-issues.txt"
tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
# Other VMs run on the card concurrently: the host Xid lines of this device's pid.
DP=$(grep -oE 'server pid [0-9]+' "$C/up.txt" | head -1 | awk '{print $3}')
grep -E "pid=${DP:-none}," "$C/host-xid.txt" > "$C/host-xid-device.txt"
sleep 3
echo "$MODE-$LABEL done $(date -Iseconds); device pid $DP; host Xid lines $(grep -c Xid "$C/host-xid.txt"), of this device $(grep -c Xid "$C/host-xid-device.txt"); vm3 left: $(pgrep -f '/mnt/vmstore/caraxes/vm[3]' | wc -l)"
