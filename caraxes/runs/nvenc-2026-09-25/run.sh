#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# NVENC and NVDEC through the mirror (docs/caraxes/video-engines.md), 2026-09-25. vm2 only,
# 610 guest, 2 GiB guest RAM, 1 GiB profile at reservation slot 3, in the environment of
# caraxes/runs/night-2026-09-24/env.sh plus the recommended configuration (interrupt page,
# token pool 64, hybrid reactor, BAR1 pool 32 MiB, BAR2 memfd, MSI rearm ioeventfd, lazy
# allocation). The probe is caraxes/tools/nvenc-probe.c, built in the guest against FFmpeg's
# nv-codec-headers (MIT; fetched without any identifier into target/nvenc/ffnvcodec).
#   run.sh probe <label> [probe args...]   vm2 up, build the probe, run it (default -n 300),
#                                         host nvidia-smi encoder/decoder reads while it runs,
#                                         the device's video-engine lines, vm2 down
#   run.sh cuda <label>                   vm2 up, the cuda gate (tools/caraxes-gate.sh cuda), down
# CARAXES_SERVER names the device binary (required; a frozen copy in /mnt/vmstore/caraxes).
set -uo pipefail
MODE=${1:?usage: run.sh probe|cuda <label> [probe args]}
LABEL=${2:?label}
shift 2
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
ROOT=$(cd "$HERE/../../.." && pwd -P)
C=$HERE/$MODE-$LABEL
[[ -e $C ]] && { echo "ERROR: $C exists; never overwrite a run" >&2; exit 2; }
HDR=$ROOT/target/nvenc/ffnvcodec
[[ -f $HDR/nvEncodeAPI.h ]] || { echo "ERROR: $HDR/nvEncodeAPI.h missing" >&2; exit 2; }
SERVER=${CARAXES_SERVER:?CARAXES_SERVER: the device binary}
cd "$ROOT" || exit 1
. caraxes/runs/night-2026-09-24/env.sh
night_guest "${NVENC_GUEST:-610}"  # NVENC_GUEST=615: vm2-615
LEA_ROOT=$ROOT . tools/lib/driver.sh && LEA_ROOT=$ROOT caraxes_driver_select || exit 2
GLIB=$CARAXES_GUEST_LIBDIR  # the selected driver's userspace (610: /opt/nvrm/lib, 615: the system path)
export CARAXES_SERVER=$SERVER
export CARAXES_MEM=2G
export CARAXES_BAR1_POOL_MIB=32 CARAXES_BAR2_MEMFD=1 CARAXES_MSI_REARM_IOEVENTFD=1 CARAXES_ALLOC_LAZY=1
# vm2 free: its device or its Cloud Hypervisor, matched by the command line's start (a host
# guard's shell may carry the vm2 path in its own text).
VM2_RE='^/mnt/vmstore/caraxes/(vm2(-615)?/|vm1/cloud-hypervisor[^ ]* .*caraxes/vm2(-615)?/)'
if pgrep -f "$VM2_RE" >/dev/null; then
    echo "ERROR: a vm2 process runs; refusing to start" >&2; pgrep -af "$VM2_RE" >&2; exit 2
fi
mkdir -p "$C"
W=$NIGHT_W; T0=$(date '+%F %T')
night_setup "$C"
caraxes/tools/bench-hostwatch.sh start "$C"
# Host BAR1 used, every 0.5 s for the whole run (a read-only NVML query): probe-defer and
# probe-defer2 were stopped at host BAR1 exhaustion (docs/caraxes/video-engines.md §4).
( echo "# SPDX-License-Identifier: MIT"; echo "# time bar1_used_mib"
  while :; do
    echo "$(date +%T.%N | cut -c1-12) $(timeout 10 nvidia-smi -q -d MEMORY 2>/dev/null | grep -A3 'BAR1 Memory' | awk '/Used/ {print $3}')"
    sleep 0.5
  done ) > "$C/host-bar1.txt" 2>&1 &
BAR1W=$!

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
    kill "$BAR1W" 2>/dev/null
    [[ -n ${JWATCH:-} ]] && { pkill -P "$JWATCH" 2>/dev/null; kill "$JWATCH" 2>/dev/null; }
    caraxes/tools/bench-hostwatch.sh stop "$C"
    tools/caraxes-vm.sh down > "$C/down.txt" 2>&1
    grep -E 'mirror: |shadow: |events: |video|NVENC|NVDEC|C4B7|c4b7|C4B0|c4b0|REFUSED|ERROR|WARN' "$W/gpu.log" \
        | cut -c1-1200 | tail -400 > "$C/device-summary.txt"
    journalctl -k --since "$T0" --no-pager -g Xid > "$C/host-xid.txt" 2>/dev/null
    for f in "$C"/*-trace.txt; do [[ -f $f ]] && xz -T0 "$f"; done
    echo "$MODE-$LABEL done $(date -Iseconds); host Xid lines $(grep -c Xid "$C/host-xid.txt"); vm2 left: $(pgrep -f "$VM2_RE" | wc -l)"
}
if [[ $up_ok != 1 ]]; then
    echo "ERROR: vm2 did not come up"; finish; exit 1
fi
pgrep -af "^$W/vfio-user-nvgpu" > "$C/device-pid.txt"
DPID=$(pgrep -f "^$W/vfio-user-nvgpu" | head -1)
# Host kernel NVRM lines of the run; the first NVRM line stops the device (SIGTERM, its own
# teardown), so a fault never repeats.
( journalctl -k -f -n 0 -o short-precise 2>/dev/null | grep --line-buffered -aE 'NVRM|Xid' \
    | grep --line-buffered -av 'ioctl(' | while read -r l; do
        echo "$l" >> "$C/host-kernel.txt"
        kill -TERM "$DPID" 2>/dev/null && echo "STOPPED device $DPID at the first NVRM line" >> "$C/host-kernel.txt"
    done ) &
JWATCH=$!
tools/caraxes-vm.sh probe "nvenc-$LABEL" >> "$C/up.txt" 2>&1; tail -2 "$C/up.txt" | cut -c1-200
caraxes/tools/bench-run.sh v2-nodes >> "$C/up.txt" 2>&1

case $MODE in
probe)
    tools/caraxes-vm.sh ssh 'mkdir -p ~/nvenc/ffnvcodec'
    for f in "$HDR"/*.h; do tools/caraxes-vm.sh ssh "cat > ~/nvenc/ffnvcodec/$(basename "$f")" < "$f"; done
    tools/caraxes-vm.sh ssh 'cat > ~/nvenc/nvenc-probe.c' < caraxes/tools/nvenc-probe.c
    tools/caraxes-vm.sh ssh "cd ~/nvenc && gcc -O2 -Wall -o nvenc-probe nvenc-probe.c -Iffnvcodec -ldl -lm && ls -la nvenc-probe && ls -la $GLIB/libnvidia-encode.so.1 $GLIB/libnvcuvid.so.1 $GLIB/libcuda.so.1" > "$C/build.txt" 2>&1
    cat "$C/build.txt"
    # Host reads while the probe runs (read-only NVML queries of the host RM): encoder and
    # decoder utilisation, the encoder session list with its pids.
    (
        echo "# SPDX-License-Identifier: MIT"
        echo "# host nvidia-smi reads every 0.5 s during the probe; device pid $DPID"
        while :; do
            echo "T $(date +%T.%N | cut -c1-12) $(timeout 10 nvidia-smi --query-gpu=utilization.encoder,utilization.decoder,encoder.stats.sessionCount,encoder.stats.averageFps,encoder.stats.averageLatency --format=csv,noheader 2>&1)"
            timeout 10 nvidia-smi encodersessions 2>&1 | grep -vE '^\s*$' | sed 's/^/S /'
            sleep 0.5
        done
    ) > "$C/host-smi.txt" 2>&1 &
    SMI=$!
    L0=$(wc -l < "$W/gpu-trace.log")
    G0=$(wc -l < "$W/gpu.log")
    ARGS=${*:--n 300}
    tools/caraxes-vm.sh ssh "cd ~/nvenc && timeout 300 env LD_LIBRARY_PATH=$GLIB ./nvenc-probe $ARGS -o /tmp/probe.es; echo rc=\$?; ls -la /tmp/probe.es; top -b -n1 | head -12" > "$C/probe.txt" 2>&1
    sleep 1; kill "$SMI" 2>/dev/null; wait "$SMI" 2>/dev/null
    cat "$C/probe.txt"
    tools/caraxes-vm.sh ssh 'sudo dmesg | grep -iE "NVRM|Xid" | tail -60' > "$C/guest-dmesg.txt" 2>&1
    tail -n +"$((L0 + 1))" "$W/gpu-trace.log" > "$C/probe-trace.txt"
    tail -n +"$((G0 + 1))" "$W/gpu.log" | cut -c1-1500 > "$C/probe-gpulog.txt"
    ;;
cuda)
    tools/caraxes-vm.sh sync-tests > "$C/sync-tests.txt" 2>&1; tail -1 "$C/sync-tests.txt"
    timeout 3600 tools/caraxes-gate.sh cuda > "$C/gate.txt" 2>&1
    tail -20 "$C/gate.txt"
    ;;
*) echo "ERROR: unknown mode $MODE" >&2 ;;
esac
finish
