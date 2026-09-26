#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Capture the guest kernel's DbgPrint output (NT Kernel Logger, flag 0x40000 = DBGPRINT) around the
# NVIDIA adapter start: start as soon as the guest agent answers, stop after $2 seconds, decode with
# tracerpt and keep the lines. Needs the Debug Print Filter values (DEFAULT, IHVVIDEO) set before boot.
# Usage: dbgprint-capture.sh <out file> [seconds]
set -uo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
OUT=${1:?out}; SECS=${2:-40}
until timeout 10 "$HERE/qga.py" ping >/dev/null 2>&1; do sleep 1; done
{
echo "# $(date +%T) agent up; starting the NT Kernel Logger with DBGPRINT"
timeout 60 "$HERE/qga.py" ps 'logman stop "NT Kernel Logger" -ets 2>&1 | Out-Null; Remove-Item C:\etw\dbgp.* -ErrorAction SilentlyContinue; logman start "NT Kernel Logger" -p "Windows Kernel Trace" 0x40000 -o C:\etw\dbgp.etl -ets 2>&1 | Out-String; (Get-Date).ToString("HH:mm:ss.fff")'
sleep "$SECS"
echo "# $(date +%T) stopping"
timeout 120 "$HERE/qga.py" ps 'logman stop "NT Kernel Logger" -ets 2>&1 | Out-String; tracerpt C:\etw\dbgp.etl -o C:\etw\dbgp.csv -of CSV -y 2>&1 | Out-Null; $l = Get-Content C:\etw\dbgp.csv; "lines: " + $l.Count; $l | Select-String -Pattern "NVRM|nvlddmkm|NVIDIA|nv[A-Z]" | Select-Object -First 300 | Out-String -Width 400'
} > "$OUT" 2>&1
