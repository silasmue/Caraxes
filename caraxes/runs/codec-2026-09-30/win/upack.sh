#!/usr/bin/env bash
# run44.py upack for a script outside the kit's tools/windows (same mechanism: a one-shot
# scheduled task in the signed-in user's interactive session, no password stored).
# Usage: upack.sh start SCRIPT.ps1 TAG WAIT_S [args...] | upack.sh status TAG
set -uo pipefail
K=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain
cd $K || exit 1
USER_=$(cat CH-build/guest-user 2>/dev/null || echo caraxes)
D='C:\Users\Public\caraxes'; DF=C:/Users/Public/caraxes
s() { timeout 60 python3 CH-build/run44.py ssh "$@" 2>&1 | tr -d '\r'; }
case $1 in
start)
    SCRIPT=$2 TAG=$3 WAIT=$4; shift 4
    base="$D\\caraxes-$TAG"
    s "New-Item -ItemType Directory -Force '$D' | Out-Null" > /dev/null
    timeout 60 python3 CH-build/run44.py scp -q "$SCRIPT" "win:$DF/caraxes-$TAG.ps1"
    args=""; for a in "$@"; do args+=",\"$a\""; done
    w=$(mktemp --suffix=.ps1)
    cat > $w <<EOF
\$ErrorActionPreference = "Stop"
Remove-Item -Force -ErrorAction SilentlyContinue $base-exit.txt
\$a = @("-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-File","$base.ps1"$args)
\$p = Start-Process powershell.exe -ArgumentList \$a -PassThru -RedirectStandardOutput $base-out.txt -RedirectStandardError $base-err.txt
\$null = \$p.Handle
if (-not \$p.WaitForExit($((WAIT * 1000)))) { \$p.Kill(); \$r = "PROBE_TIMEOUT $WAIT s" } else { \$r = "PROBE_EXIT=" + \$p.ExitCode }
Set-Content -Encoding ascii $base-exit.txt \$r
EOF
    timeout 60 python3 CH-build/run44.py scp -q $w "win:$DF/caraxes-$TAG-run.ps1"; rm -f $w
    s "\$act = New-ScheduledTaskAction -Execute powershell.exe -Argument '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File $base-run.ps1'
\$prin = New-ScheduledTaskPrincipal -UserId '$USER_' -LogonType Interactive -RunLevel Limited
\$set = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Seconds $((WAIT + 120))) -AllowStartIfOnBatteries
Register-ScheduledTask -Force -TaskName caraxes-$TAG -Action \$act -Principal \$prin -Settings \$set | Out-Null
Remove-Item -Force -ErrorAction SilentlyContinue $base-exit.txt,$base-out.txt,$base-err.txt
Start-ScheduledTask -TaskName caraxes-$TAG
'started caraxes-$TAG'" ;;
status)
    base="$D\\caraxes-$2"
    s "if (Test-Path $base-exit.txt) { Get-Content $base-exit.txt } else { 'RUNNING: ' + (Get-ScheduledTask -TaskName caraxes-$2).State }
foreach (\$f in '$base-out.txt','$base-err.txt') { if (Test-Path \$f) { Get-Content \$f } }" ;;
esac
