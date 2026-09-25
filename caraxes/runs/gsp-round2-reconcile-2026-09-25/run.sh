#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The GPU runs of the reconciliation of the C table and the D models (2026-09-25). The tools
# run from the main checkout R; the device binary is given.
# Usage: run.sh <phase> <device binary> 610|615 [gates|probe]
#   610: vm3 (610.57.04 guest), the environment of caraxes/runs/gsp-rpc-2026-09-24/run.sh
#        gate610: 1 GiB at reservation slot 5, tap caraxes3.
#   615: vm5-615 (615.71.09 guest, the v615 device), the environment of
#        caraxes/runs/gsp-round2-c-2026-09-24/run.sh: 1 GiB at reservation slot 2, tap caraxes5.
#   Both: the host-execution configuration of caraxes/runs/doorbell-2026-09-24/env-vm1.sh,
#   token pool (64), hybrid reactor, no BAR1 area.
#   gates (default): the nvml and cuda gates.
#   probe: no gates; the device runs with CARAXES_RC_SELFTEST=20 (a SIMULATED RC record of
#          no channel every 20 s) and, on 615, CARAXES_GOE_SELFTEST=5; the RC journal probe
#          (caraxes/tools/rc-journal-probe.sh) runs, and on 615 the operational-event probe
#          (caraxes/tools/goe-probe.sh) first.
# While a process of the VM runs (a census or a ladder of another agent), waits five minutes
# and retries, up to an hour; likewise when the device refuses the reservation for lack of
# headroom. Never overwrites a phase; brings the VM down at its end.
set -uo pipefail
PHASE=${1:?phase}; BIN=${2:?device binary}; GUEST=${3:?610|615}; KIND=${4:-gates}
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
SRC=$(cd "$HERE/../../.." && pwd -P)
R=/home/silas/git/Caraxes
C=$HERE/$PHASE
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
export CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1
export CARAXES_MIRROR_GR_SHADOW=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register
export CARAXES_FB_MIB=1024 CARAXES_GATE_FB_MIB=1024
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh
export CARAXES_DOORBELL_TOKENS=64 CARAXES_DOORBELL_POLL=hybrid
export CARAXES_SERVER=$BIN
unset CARAXES_DOORBELL_IOEVENTFD CARAXES_INTR_PAGE CARAXES_BAR1_PASSTHROUGH CARAXES_DISPLAY \
    CARAXES_GUEST_ABI CARAXES_DRIVER CARAXES_RC_SELFTEST CARAXES_GOE_SELFTEST
case $GUEST in
    610) export VM=vm3 CARAXES_RESERVE_SLOT=5
         export CARAXES_IP=192.168.103.23 CARAXES_TAP=caraxes3 CARAXES_MAC=02:00:00:00:00:23
         PAT=/mnt/vmstore/caraxes/vm3 ;;
    615) export VM=vm5-615 CARAXES_DRIVER=615 CARAXES_RESERVE_SLOT=2
         export CARAXES_IP=192.168.105.26 CARAXES_TAP=caraxes5 CARAXES_MAC=02:00:00:00:00:26
         PAT=/mnt/vmstore/caraxes/vm5 ;;
    *) echo "ERROR: guest $GUEST" >&2; exit 2 ;;
esac
case $KIND in
    gates) ;;
    probe) export CARAXES_RC_SELFTEST=20
           [[ $GUEST == 615 ]] && export CARAXES_GOE_SELFTEST=5 ;;
    *) echo "ERROR: kind $KIND" >&2; exit 2 ;;
esac
W=/mnt/vmstore/caraxes/$VM
mkdir -p "$C"; cd "$R" || exit 1
export CARAXES_GATE_RUNS=$C
for wait in $(seq 0 12); do
    pgrep -f "$PAT" >/dev/null || break
    [[ $wait == 12 ]] && { echo "ERROR: $PAT still busy after an hour" | tee -a "$C/busy.txt" >&2; exit 2; }
    { date -Iseconds; pgrep -af "$PAT"; echo "busy; retry in 5 min"; } >> "$C/busy.txt"
    sleep 300
done
T0=$(date '+%F %T')
{ head -1 /proc/driver/nvidia/version; sha256sum "$CARAXES_SERVER"; git -C "$SRC" log --oneline -1; env | grep -E '^CARAXES_|^LEA_CH|^VM=' | sort; } > "$C/setup.txt"
caraxes/tools/bench-hostwatch.sh start "$C"
up_ok=0
for try in $(seq 1 13); do
    tools/caraxes-vm.sh up > "$C/up.txt" 2>&1 || tail -5 "$C/up.txt"
    pgrep -af "vfio-user-nvgpu.*$W/" > "$C/device-pid.txt"
    if tools/caraxes-vm.sh wait >> "$C/up.txt" 2>&1; then up_ok=1; break; fi
    if grep -q 'below the .* MiB headroom' "$W/gpu.log" 2>/dev/null; then
        echo "try $try $(date -Iseconds): reservation refused for headroom; down, retry in 5 min" | tee -a "$C/retries.txt"
        tools/caraxes-vm.sh down >> "$C/retries.txt" 2>&1
        [[ $try -lt 13 ]] && sleep 300
        continue
    fi
    break
done
if [[ $up_ok == 1 ]]; then
    tools/caraxes-vm.sh probe gsprpc >> "$C/up.txt" 2>&1; tail -3 "$C/up.txt" | cut -c1-200
    tools/caraxes-vm.sh ssh 'nvidia-smi; sudo dmesg | grep -E "NVRM|goeimport|nvidia" | grep -vE "ioctl|RmMsg" | tail -60' > "$C/guest.txt" 2>&1
    if [[ $KIND == probe ]]; then
        if [[ $GUEST == 615 ]]; then
            "$SRC/caraxes/tools/goe-probe.sh" 45 > "$C/goe-probe.txt" 2>&1; echo "goe probe rc=$?"; tail -2 "$C/goe-probe.txt"
        fi
        "$SRC/caraxes/tools/rc-journal-probe.sh" 60 > "$C/rc-journal-probe.txt" 2>&1; echo "rc journal probe rc=$?"; tail -4 "$C/rc-journal-probe.txt"
    else
        tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
        tools/caraxes-gate.sh nvml > "$C/gate-nvml.txt" 2>&1; echo "nvml gate rc=$?"; tail -1 "$C/gate-nvml.txt" | cut -c1-300
        tools/caraxes-gate.sh cuda > "$C/gate-cuda.txt" 2>&1; echo "cuda gate rc=$?"; tail -1 "$C/gate-cuda.txt" | cut -c1-300
    fi
else
    echo "VM did not come up (see up.txt, retries.txt)"
fi
caraxes/tools/bench-hostwatch.sh stop "$C"
T=$W/gpu-trace.log
grep -aE 'rc self-test|RC_TRIGGERED|OS_ERROR_LOG|operational event|op-event|goe record|UPDATE_GET|update get|held until enabled|held doorbell|unsolicited|refused' "$T" | cut -c1-400 > "$C/device-rc.txt"
sed -nE 's/.* E boot: ctrl (0x[0-9a-f]{8}): .* -> status (0x[0-9a-f]+) \[(.*)\]$/\1 \2 [\3]/p' "$T" | sort | uniq -c | sort -k2 > "$C/device-controls.txt"
grep -aE 'unanswered|NOT_SUPPORTED|status 0x56|refused' "$T" | sed -E 's/^.*(ctrl 0x[0-9a-f]+|alloc: |rx seq [0-9]+ )/\1/' | sort | uniq -c | sort -rn | head -25 | cut -c1-300 > "$C/device-trace-issues.txt"
grep -E 'unanswered|ERROR|refused|NOT_SUPPORTED' "$W/gpu.log" | sort | uniq -c | sort -rn | head -15 | cut -c1-300 > "$C/device-log-issues.txt"
tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
echo "$PHASE done $(date -Iseconds); host Xid lines $(grep -c Xid "$C/host-xid.txt"); $VM left: $(pgrep -f "$PAT" | wc -l)"
