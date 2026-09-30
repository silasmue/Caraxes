# SPDX-License-Identifier: MIT
# A DxgKrnl ETW trace across a TDR: -Action start opens a real-time-buffered session of the
# Microsoft-Windows-DxgKrnl provider (all keywords, verbose) into C:\etw\dxgk.etl; -Action
# stop closes it and converts it with tracerpt into C:\etw\dxgk.csv (and a summary of event
# names by count, plus the events whose name mentions a TDR, a timeout or a reset). Admin
# session (SSH) or SYSTEM. Stock Windows tools only (logman, tracerpt).
param([ValidateSet('start', 'stop')][string]$Action = 'start', [string]$Name = 'caraxes-dxgk')
$ErrorActionPreference = "Continue"
$dir = 'C:\etw'
New-Item -ItemType Directory -Force $dir | Out-Null
$etl = Join-Path $dir 'dxgk.etl'
if ($Action -eq 'start') {
    logman stop $Name -ets 2>$null | Out-Null
    Remove-Item -Force -ErrorAction SilentlyContinue $etl
    logman create trace $Name -o $etl -ets -bs 1024 -nb 64 256 -p Microsoft-Windows-DxgKrnl 0xffffffffffffffff 0xff
    "started $Name at $(Get-Date -Format HH:mm:ss.fff)"
    exit $LASTEXITCODE
}
logman stop $Name -ets
$csv = Join-Path $dir 'dxgk.csv'
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
