#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Unattended Windows driver-bind runs (README.md "Night run"): for each configuration, start the
# device, boot the installed Windows under QEMU, wait until the NVIDIA driver has run its
# start sequence (the seven AUX writes at 0xd96c + n*0x50 end it today) or a timeout, record
# what the driver touched, and power the VM off (QEMU is terminated; Windows gets no clean
# shutdown, which its journaled disk tolerates). The driver starts at every boot, so a boot is
# the "driver restart". The last configuration is left running when NIGHT_KEEP_LAST=1.
# Usage: night-win.sh <results dir>   (this copy waits for the trace to stall instead of fixed delays)
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
W=/mnt/vmstore/caraxes/win11
OUT=${1:?results dir}
mkdir -p "$OUT"
EXTCFG=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-extcfg-9df39581
LEGACY_ROM=$W/rom-legacy-only.bin
T4_BOARD=$HERE/../../../crates/vfio-user-nvgpu/boards/tesla-t4-on-tu106.toml
SYNTH_VBIOS=$HERE/../../reference/rtx2070/synth-vbios.bin
PROM_DUMP=${NIGHT_PROM_DUMP:-$HERE/../../reference/rtx2070/prom-window-2026-09-25.bin}
HWCFG=${NIGHT_HWCFG:-$(command ls -t /mnt/vmstore/caraxes/vfio-user-nvgpu-v615-extcfg-hwcfg-* 2>/dev/null | head -1)}
IFR=${NIGHT_IFR:-$(command ls -t /mnt/vmstore/caraxes/vfio-user-nvgpu-v615-extcfg-ifr-* 2>/dev/null | head -1)}
PMU2=${NIGHT_PMU2:-$(command ls -t /mnt/vmstore/caraxes/vfio-user-nvgpu-v615-extcfg-pmu2-* 2>/dev/null | head -1)}
FULL_DUMP=$HERE/../../reference/rtx2070/prom-window-2026-09-25.bin
q='qemu-system-x86'
log() { printf '%s %s\n' "$(date +%T)" "$*" | tee -a "$OUT/night.log"; }

qemu_running() { pgrep -f "${q}_64.*win11" >/dev/null; }
qemu_stop() {
    local p
    p=$(pgrep -f "${q}_64.*win11" | head -1)
    [[ -n $p ]] && kill "$p"
    for _ in $(seq 1 20); do qemu_running || break; sleep 1; done
    qemu_running && { p=$(pgrep -f "${q}_64.*win11" | head -1); kill -9 "$p"; sleep 1; }
    "$HERE/gpu-down.sh" >/dev/null 2>&1 || true
}

# summarise <name>: what the driver touched in this run's trace
summarise() {
    local name=$1 T=$W/gpu-trace.log f=$OUT/$1
    {
        echo "# $name $(date +%T): trace lines $(wc -l < "$T"), non-PROM $(grep -avc NV_PROM "$T")"
        echo "# distinct non-PROM BAR0 offsets: $(grep -a ' bar0 ' "$T" | grep -av NV_PROM | awk '{print $4}' | sort -u | wc -l)"
        echo "# GSP/SEC2/PFIFO writes: $(grep -a ' W bar0 0x0011\| W bar0 0x00084\| W bar0 0x0000[23]' "$T" | wc -l)"
        echo "# PMU ENGCTL polls: $(grep -ac ' R bar0 0x0010a0a4' "$T"); PMU port writes: $(grep -ac ' W bar0 0x0010a1[8c]' "$T"); SEC2 port writes: $(grep -ac ' W bar0 0x008401[8c]' "$T"); GSP riscv accesses: $(grep -ac ' bar0 0x00111' "$T"); GSP queue accesses: $(grep -ac ' bar0 0x00110c' "$T")"
        echo "# reads by register:"
        grep -a ' bar0 ' "$T" | grep -av NV_PROM | awk '$2=="R"{print $4, $NF}' | sort | uniq -c | sort -rn | head -60
        echo "# writes:"
        grep -a ' W bar0' "$T" | sed -E 's/^ *[0-9]+ //' | cut -c1-90 | sort | uniq -c | sort -rn | head -40
        echo "# extended config mirror reads/writes:"
        grep -a ' bar0 0x00088[1-9a-f]' "$T" | sed -E 's/^ *[0-9]+ //' | cut -c1-70 | sort | uniq -c | sort -rn | head -20
        echo "# last 30 non-PROM lines:"
        grep -av NV_PROM "$T" | tail -30 | cut -c1-140
        if [[ -s $OUT/baseline-offsets.txt ]]; then
            echo "# BAR0 offsets not in the 35-register baseline (offset, first answer):"
            grep -a ' bar0 ' "$T" | grep -av NV_PROM | awk '{print $4}' | sort -u | comm -23 - "$OUT/baseline-offsets.txt" | while read -r o; do
                printf '%s %s\n' "$o" "$(grep -a -m1 " bar0 $o " "$T" | sed -E 's/^ *[0-9]+ //' | cut -c1-80)"
            done
            echo "# baseline offsets not touched this run:"
            grep -a ' bar0 ' "$T" | grep -av NV_PROM | awk '{print $4}' | sort -u | comm -13 - "$OUT/baseline-offsets.txt" | tr '\n' ' '; echo
        fi
    } > "$f-summary.txt"
    grep -av NV_PROM "$T" | grep -a ' bar0 \| rom ' | head -3000 > "$f-bar0-head.txt"
    cp "$W/gpu.log" "$f-gpu.log" 2>/dev/null
}

# run <name> <max seconds> [env assignments...]: one boot with the device
run() {
    local name=$1 max=$2; shift 2
    log "== $name: $*"
    qemu_stop
    ( export "$@"; "$HERE/gpu-up.sh" ) >> "$OUT/night.log" 2>&1 || { log "$name: gpu-up failed"; return 1; }
    ( export "$@"; setsid nohup "$HERE/run-qemu-gpu.sh" > "$OUT/$name-qemu.log" 2>&1 < /dev/null & )
    sleep 20
    qemu_running || { log "$name: QEMU did not start"; tail -3 "$OUT/$name-qemu.log" >> "$OUT/night.log"; return 1; }
    local t0=$SECONDS marker=0
    while (( SECONDS - t0 < max )); do
        sleep 15
        qemu_running || { log "$name: QEMU exited"; break; }
        if (( marker == 0 )) && grep -aq ' W bar0 0x0000db4c' "$W/gpu-trace.log"; then
            marker=1; log "$name: driver sequence reached the AUX writes after $((SECONDS - t0)) s; waiting until the trace stalls"
        fi
        if (( marker == 1 )); then
            # the driver may go on (GSP path): wait until the trace has not grown for 90 s
            n1=$(wc -l < "$W/gpu-trace.log"); sleep 90; n2=$(wc -l < "$W/gpu-trace.log")
            if (( n2 == n1 )); then log "$name: trace stalled at $n2 lines after $((SECONDS - t0)) s"; break; fi
        fi
    done
    (( marker == 0 )) && log "$name: no AUX-write marker within $((SECONDS - t0)) s"
    summarise "$name"
    log "$name: $(sed -n 2,3p "$OUT/$name-summary.txt" | tr '\n' ' ')"
}

# night-win.sh <out> summarise <name>: only record the running device's trace under <name>.
if [[ ${2:-} == summarise ]]; then summarise "${3:?name}"; log "$3: $(sed -n 2,3p "$OUT/$3-summary.txt" | tr '\n' ' ')"; exit 0; fi
MAX=${NIGHT_MAX:-600}          # seconds per run (a Windows update at boot needs more)
RUNS=${NIGHT_RUNS:-A B C}      # which configurations
log "night run start; results in $OUT; runs: $RUNS; max ${MAX}s each"
for r in $RUNS; do case $r in
    A) run A-extcfg-mirror "$MAX" WIN_GPU_SERVER="$EXTCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 ;;
    B) run B-extcfg-region "$MAX" WIN_GPU_SERVER="$EXTCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 CARAXES_EXT_CONFIG=1 ;;
    C) run C-extcfg-t4 "$MAX" WIN_GPU_SERVER="$EXTCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_BOARD="$T4_BOARD" WIN_GPU_REGS="$HERE/bar0-registers-t4-costume.txt" ;;
    D) run D-extcfg-region-t4 "$MAX" WIN_GPU_SERVER="$EXTCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 CARAXES_EXT_CONFIG=1 WIN_GPU_BOARD="$T4_BOARD" WIN_GPU_REGS="$HERE/bar0-registers-t4-costume.txt" ;;
    F) run F-extcfg-3dctrl "$MAX" WIN_GPU_SERVER="$EXTCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_CONFIG="$HERE/config-space-3d.txt" ;;
    G) run G-extcfg-rootport-region "$MAX" WIN_GPU_SERVER="$EXTCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 CARAXES_EXT_CONFIG=1 WIN_GPU_ROOTPORT=1 ;;
    H) run H-extcfg-synthvbios "$MAX" WIN_GPU_SERVER="$EXTCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_VBIOS="$SYNTH_VBIOS" ;;
    I) [[ -x ${HWCFG:-/nonexistent} ]] && run I-hwcfg-synthvbios "$MAX" WIN_GPU_SERVER="$HWCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_VBIOS="$SYNTH_VBIOS" || log "I skipped: no hwcfg binary" ;;
    J) [[ -x ${HWCFG:-/nonexistent} ]] && run J-hwcfg-only "$MAX" WIN_GPU_SERVER="$HWCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 || log "J skipped: no hwcfg binary" ;;
    K) run K-extcfg-synthvbios-t4 "$MAX" WIN_GPU_SERVER="$EXTCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_VBIOS="$SYNTH_VBIOS" WIN_GPU_BOARD="$T4_BOARD" WIN_GPU_REGS="$HERE/bar0-registers-t4-costume.txt" ;;
    L) [[ -s $PROM_DUMP ]] && run L-extcfg-promdump "$MAX" WIN_GPU_SERVER="$EXTCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_VBIOS="$PROM_DUMP" || log "L skipped: no PROM dump" ;;
    M) [[ -s $PROM_DUMP && -x ${HWCFG:-/nonexistent} ]] && run M-hwcfg-promdump "$MAX" WIN_GPU_SERVER="$HWCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_VBIOS="$PROM_DUMP" || log "M skipped: no PROM dump or hwcfg binary" ;;
    N) [[ -s $PROM_DUMP ]] && run N-extcfg-promdump-t4 "$MAX" WIN_GPU_SERVER="$EXTCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_VBIOS="$PROM_DUMP" WIN_GPU_BOARD="$T4_BOARD" WIN_GPU_REGS="$HERE/bar0-registers-t4-costume.txt" || log "N skipped: no PROM dump" ;;
    P) [[ -s $FULL_DUMP && -x ${IFR:-/nonexistent} ]] && run P-ifr-fullwindow "$MAX" WIN_GPU_SERVER="$IFR" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_VBIOS="$FULL_DUMP" || log "P skipped: no full dump or ifr binary" ;;
    Q) [[ -s $FULL_DUMP && -x ${IFR:-/nonexistent} ]] && run Q-ifr-fullwindow-t4 "$MAX" WIN_GPU_SERVER="$IFR" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_VBIOS="$FULL_DUMP" WIN_GPU_BOARD="$T4_BOARD" WIN_GPU_REGS="$HERE/bar0-registers-t4-costume.txt" || log "Q skipped: no full dump or ifr binary" ;;
    R) [[ -x ${IFR:-/nonexistent} ]] && run R-ifr-fullwindow-region-rootport "$MAX" WIN_GPU_SERVER="$IFR" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_VBIOS="$FULL_DUMP" CARAXES_EXT_CONFIG=1 WIN_GPU_ROOTPORT=1 || log "R skipped: no ifr binary" ;;
    X0) [[ -x ${PMU2:-/nonexistent} ]] && run X0-pmu2-stallack200 "$MAX" WIN_GPU_SERVER="$PMU2" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_VBIOS="$FULL_DUMP" || log "X0 skipped: no pmu2 binary" ;;
    X1) [[ -x ${PMU2:-/nonexistent} ]] && run X1-pmu2-stallack0 "$MAX" WIN_GPU_SERVER="$PMU2" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_VBIOS="$FULL_DUMP" CARAXES_FALCON_STALLACK=0 || log "X1 skipped: no pmu2 binary" ;;
    X2) [[ -x ${PMU2:-/nonexistent} ]] && run X2-pmu2-stallack600 "$MAX" WIN_GPU_SERVER="$PMU2" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 WIN_GPU_VBIOS="$FULL_DUMP" CARAXES_FALCON_STALLACK=600 || log "X2 skipped: no pmu2 binary" ;;
    A2) run A2-extcfg-mirror-repeat "$MAX" WIN_GPU_SERVER="$EXTCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 ;;
    B2) run B2-extcfg-region-repeat "$MAX" WIN_GPU_SERVER="$EXTCFG" WIN_GPU_ROM="$LEGACY_ROM" WIN_GPU_ROMBAR=1 CARAXES_EXT_CONFIG=1 ;;
    *) log "unknown run $r" ;;
esac; done
if [[ ${NIGHT_KEEP_LAST:-0} == 1 ]]; then
    log "leaving the last configuration running"
else
    qemu_stop; log "windows off"
fi
log "night run done"
