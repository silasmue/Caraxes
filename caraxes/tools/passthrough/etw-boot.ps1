# SPDX-License-Identifier: MIT
# R4 boot capture (docs/caraxes/experiments.md R4, A1): the DxgKrnl ETW trace of
# tools/windows/dxgk-etw.ps1 (same provider, all keywords, level 0xff, the same buffers) as an
# AutoLogger, so it runs from the next boot through the driver start, the sign-in and the
# desktop's first flips (the window in which our device's runs get the VSync TDR).
#   -Action arm     create autosession\<Name> (C:\etw\dxgk-boot.etl); reboot afterwards
#   -Action stop    stop the running session, delete the AutoLogger, tracerpt -> dxgk-boot.csv,
#                   and summarise like dxgk-etw.ps1
#   -Action disarm  delete the AutoLogger only
# Admin session (SSH) or SYSTEM. Stock Windows tools only (logman, tracerpt).
param([ValidateSet('arm', 'stop', 'disarm')][string]$Action = 'arm', [string]$Name = 'caraxes-dxgk-boot')
$ErrorActionPreference = "Continue"
$dir = 'C:\etw'
New-Item -ItemType Directory -Force $dir | Out-Null
$etl = Join-Path $dir 'dxgk-boot.etl'
if ($Action -eq 'arm') {
    logman delete "autosession\$Name" 2>$null | Out-Null
    Remove-Item -Force -ErrorAction SilentlyContinue $etl
    logman create trace "autosession\$Name" -o $etl -bs 1024 -nb 64 256 -p Microsoft-Windows-DxgKrnl 0xffffffffffffffff 0xff
    if ($LASTEXITCODE -ne 0) { "FAIL: logman create autosession\$Name ($LASTEXITCODE)"; exit 1 }
    logman query "autosession\$Name"
    "armed autosession\$Name at $(Get-Date -Format HH:mm:ss.fff): it starts at the next boot"
    exit 0
}
if ($Action -eq 'disarm') { logman delete "autosession\$Name"; exit $LASTEXITCODE }
logman stop $Name -ets
logman delete "autosession\$Name" | Out-Null
$csv = Join-Path $dir 'dxgk-boot.csv'
Remove-Item -Force -ErrorAction SilentlyContinue $csv
tracerpt $etl -o $csv -of CSV -y | Out-Null
if (-not (Test-Path $csv)) { "FAIL: tracerpt wrote no $csv"; exit 1 }
$rows = Import-Csv $csv
"events: $($rows.Count) in $csv"
$rows | Group-Object 'Event Name' | Sort-Object Count -Descending | Select-Object -First 25 |
    ForEach-Object { "{0,7} {1}" -f $_.Count, $_.Name }
"== TDR, timeout, reset events"
$rows | Where-Object { $_.'Event Name' -match 'Tdr|TDR|Timeout|Reset|Hang' } | Select-Object -First 60 |
    ForEach-Object { ($_.PSObject.Properties | Select-Object -First 12 | ForEach-Object { $_.Value }) -join ' | ' }
