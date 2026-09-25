#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
# Does the Windows driver switch to GSP-RM mode by registry? Sets EnableGpuFirmware and
# RMEnableGpuFirmware (DWORD 1) under the nvlddmkm service key, its Parameters subkey and the
# card's class key (night-2026-09-25/etw-driver-start/registry-before.txt: class instance 0007),
# restarts the driver with pnputil, and looks in the device trace for the GSP path (SEC2 load
# 0x840180/0x8401c0, GSP RISC-V 0x111000+, GSP queues 0x110c00+) instead of the PMU stall poll
# at 0x10a0a4. Then once more after a full reboot, because RM keys are read at driver load.
# Needs the VM up with the guest agent (run-qemu-gpu.sh) and gpu-up.sh's trace. Usage:
#   regkey-test.sh <results dir>
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
W=/mnt/vmstore/caraxes/win11
OUT=${1:?results dir}; mkdir -p "$OUT"
T=$W/gpu-trace.log
log() { printf '%s %s\n' "$(date +%T)" "$*" | tee -a "$OUT/regkey-test.log"; }
ps() { timeout 300 "$HERE/qga.py" ps "$1" 2>&1; }

verdict() {
    local from=$1
    local sec2 gsprv gspq pmu
    sec2=$(tail -n +"$from" "$T" | grep -ac ' W bar0 0x008401[8c]')
    gsprv=$(tail -n +"$from" "$T" | grep -ac ' bar0 0x00111')
    gspq=$(tail -n +"$from" "$T" | grep -ac ' bar0 0x00110c')
    pmu=$(tail -n +"$from" "$T" | grep -ac ' R bar0 0x0010a0a4')
    echo "sec2 loads $sec2, gsp riscv $gsprv, gsp queue $gspq, pmu engctl polls $pmu"
}

log "== registry state before"
ps 'reg query HKLM\SYSTEM\CurrentControlSet\Services\nvlddmkm /s /f GpuFirmware 2>&1 | Out-String; reg query "HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0007" /v DriverDesc 2>&1 | Out-String' | tee -a "$OUT/regkey-test.log"
log "== set the keys"
ps 'foreach ($k in @("HKLM\SYSTEM\CurrentControlSet\Services\nvlddmkm","HKLM\SYSTEM\CurrentControlSet\Services\nvlddmkm\Parameters","HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0007")) { reg add $k /f | Out-Null; foreach ($v in @("EnableGpuFirmware","RMEnableGpuFirmware")) { reg add $k /v $v /t REG_DWORD /d 1 /f | Out-String } }; reg query HKLM\SYSTEM\CurrentControlSet\Services\nvlddmkm /s /f GpuFirmware | Out-String; reg query "HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0007" /f GpuFirmware | Out-String' | tee -a "$OUT/regkey-test.log"
inst=$(ps 'Get-PnpDevice -Class Display | Where-Object { $_.InstanceId -like "PCI\VEN_10DE*" -and $_.Status -ne "Unknown" } | Select-Object -First 1 -ExpandProperty InstanceId' | grep -a 'PCI' | head -1 | tr -d '\r')
log "card instance: $inst"
from=$(( $(wc -l < "$T") + 1 ))
log "== restart the driver"
ps "pnputil /restart-device \"$inst\" 2>&1 | Out-String" | tee -a "$OUT/regkey-test.log"
sleep 100
log "after restart: $(verdict "$from")"
ps 'Get-PnpDevice -Class Display | Where-Object { $_.InstanceId -like "PCI\VEN_10DE*" } | Format-List InstanceId,Status,Problem | Out-String' | tee -a "$OUT/regkey-test.log"
log "== reboot the guest (RM keys are read at driver load)"
ps 'shutdown /r /t 3' >/dev/null 2>&1
sleep 60
from=$(( $(wc -l < "$T") + 1 ))
for _ in $(seq 1 30); do timeout 40 "$HERE/qga.py" ping >/dev/null 2>&1 && break; sleep 10; done
sleep 120
log "after reboot: $(verdict "$from")"
ps 'Get-PnpDevice -Class Display | Where-Object { $_.InstanceId -like "PCI\VEN_10DE*" } | Format-List InstanceId,Status,Problem | Out-String' | tee -a "$OUT/regkey-test.log"
tail -n +"$from" "$T" | grep -av NV_PROM | grep -a ' bar0 ' | awk '{print $2, $4, $NF}' | sort | uniq -c | sort -rn | head -40 > "$OUT/regkey-test-registers-after-reboot.txt"
log "done"
