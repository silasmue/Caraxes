#!/usr/bin/env bash
# Run tools/windows/showcase.ps1 elevated in the signed-in user's session (mmc auto-elevates, and
# UI Automation from a limited token cannot read an elevated window): upload, a scheduled task with
# RunLevel Highest, wait for its exit file, fetch the images (2026-09-30: W2-showcase).
# Usage: showcase-hi.sh [ONLY]   (ONLY: showcase.ps1 -Only, e.g. devmgr,props)
set -uo pipefail
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain S=/mnt/vmstore/caraxes/runs-2026-09-30/win/W2-showcase/shots
ONLY=${1:-devmgr,props,taskmgr,dxdiag}
mkdir -p $S; rm -f $S/caraxes-showcase-*.txt; cd $K || exit 1
r() { timeout 60 python3 CH-build/run44.py "$@" 2>&1 | tr -d '\r'; }
B='C:\Users\Public\caraxes\caraxes-showcase'
W=$(mktemp --suffix=.ps1)
cat > $W <<PS
\$p = Start-Process powershell.exe -ArgumentList "-NoProfile -NonInteractive -ExecutionPolicy Bypass -File $B.ps1 -Only $ONLY" -PassThru -RedirectStandardOutput $B-out.txt -RedirectStandardError $B-err.txt
\$null = \$p.Handle
if (-not \$p.WaitForExit(400000)) { \$p.Kill(); \$r = "PROBE_TIMEOUT" } else { \$r = "PROBE_EXIT=" + \$p.ExitCode }
Set-Content -Encoding ascii $B-exit.txt \$r
PS
r scp /home/silas/git/Caraxes/tools/windows/showcase.ps1 win:C:/Users/Public/caraxes/caraxes-showcase.ps1 > /dev/null
r scp $W win:C:/Users/Public/caraxes/caraxes-showcase-hi.ps1 > /dev/null; rm -f $W
r ssh "Remove-Item -Force -ErrorAction SilentlyContinue $B-exit.txt,$B-out.txt,$B-err.txt,C:\\Users\\Public\\caraxes\\showcase\\*; Stop-Process -Name mmc,dxdiag -Force -ErrorAction SilentlyContinue
\$act = New-ScheduledTaskAction -Execute powershell.exe -Argument '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File $B-hi.ps1'
\$prin = New-ScheduledTaskPrincipal -UserId \$env:USERNAME -LogonType Interactive -RunLevel Highest
\$set = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Seconds 520) -AllowStartIfOnBatteries
Register-ScheduledTask -Force -TaskName caraxes-showcase-hi -Action \$act -Principal \$prin -Settings \$set -ErrorAction Stop | Out-Null
Start-ScheduledTask -TaskName caraxes-showcase-hi; 'started'" | tail -1
for i in $(seq 1 50); do sleep 10; timeout 60 python3 CH-build/run44.py scp "win:C:/Users/Public/caraxes/caraxes-showcase-exit.txt" $S/ > /dev/null 2>&1 && break; done
for f in out err; do r scp "win:C:/Users/Public/caraxes/caraxes-showcase-$f.txt" $S/ > /dev/null; done
cat $S/caraxes-showcase-exit.txt; iconv -f cp850 -t utf-8 $S/caraxes-showcase-out.txt | tr -d '\r'; iconv -f cp850 -t utf-8 $S/caraxes-showcase-err.txt | tr -d '\r' | head -3 | cut -c1-3000
for f in $(r ssh 'Get-ChildItem C:\Users\Public\caraxes\showcase | ForEach-Object { $_.Name }'); do r scp "win:C:/Users/Public/caraxes/showcase/$f" $S/ > /dev/null; done
ls $S
