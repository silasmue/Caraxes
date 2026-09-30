# SPDX-License-Identifier: MIT
# The CUDA part of the display soak: cuda-probe.ps1 (deviceQuery, PTX JIT vector add, launch
# cost, pinned bandwidth) again and again for -Minutes; one line per round with its result.
# Its sibling must sit next to it as caraxes-cuda-probe.ps1 (the pack uploads both).
param([int]$Minutes = 60)
$ErrorActionPreference = "Continue"
$probe = Join-Path $PSScriptRoot 'caraxes-cuda-probe.ps1'
if (-not (Test-Path $probe)) { "FAIL: $probe missing"; exit 1 }
$end = (Get-Date).AddMinutes($Minutes); $n = 0; $failed = 0
while ((Get-Date) -lt $end) {
    $n++
    $out = & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $probe 2>&1 | Out-String
    $short = ($out -split "`n" | Where-Object { $_ -match '^short kernel' } | Select-Object -Last 1)
    $ok = $out -match 'PASS: CUDA'
    if (-not $ok) { $failed++ }
    "{0:HH:mm:ss} round {1}: {2} {3}" -f (Get-Date), $n, $(if ($ok) { 'PASS' } else { 'FAIL' }), $short.Trim()
}
if ($failed -eq 0) { "PASS: $n CUDA rounds in $Minutes min" } else { "FAIL: $failed of $n CUDA rounds"; exit 1 }
