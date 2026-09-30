# SPDX-License-Identifier: MIT
# Unigine Heaven 4.0 (free Basic edition) benchmark in the signed-in user's session (run44.py
# upack): DirectX 11, 1920x1080 windowed, the bench pack's engine flags (PHORONIX runs the
# benchmark and prints its FPS/Score, then exits; caraxes/tools/bench-pack/run-windows.ps1).
# -Quality MEDIUM | HIGH | ULTRA, -Tessellation NORMAL | EXTREME. Install it first over SSH.
param([string]$Quality = "HIGH", [string]$Tessellation = "NORMAL")
$ErrorActionPreference = "Continue"
$bin = "C:\Program Files (x86)\Unigine\Heaven Benchmark 4.0\bin"
if (-not (Test-Path "$bin\Heaven.exe")) { "FAIL: $bin\Heaven.exe missing"; exit 1 }
$out = Join-Path $env:TEMP "caraxes-heaven-out.txt"
$a = @("-video_app", "direct3d11", "-data_path", "../", "-sound_app", "null",
       "-engine_config", "../data/heaven_4.0.cfg", "-system_script", "heaven/unigine.cpp",
       "-video_mode", "-1", "-video_fullscreen", "0", "-video_width", "1920", "-video_height", "1080",
       "-extern_define", "PHORONIX,RELEASE,QUALITY_$Quality,TESSELLATION_$Tessellation")
$t0 = Get-Date
"start $($t0.ToString('HH:mm:ss')) quality $Quality tessellation $Tessellation"
$p = Start-Process -FilePath "$bin\Heaven.exe" -ArgumentList $a -WorkingDirectory $bin -RedirectStandardOutput $out -PassThru
if (-not $p.WaitForExit(900000)) { $p.Kill(); "FAIL: Heaven did not finish in 15 min" }
$t1 = Get-Date
"end $($t1.ToString('HH:mm:ss')) after $([int]($t1 - $t0).TotalSeconds) s, exit $($p.ExitCode)"
Get-Content $out -ErrorAction SilentlyContinue | Select-String -Pattern "FPS|Score|Min|Max|Render|Direct3D|Error|error" | ForEach-Object { $_.Line }
# The PHORONIX define prints the run's mean as "FPS:" at the end, and no Score line.
if (Select-String -Path $out -Pattern "^(FPS|Score):" -Quiet -ErrorAction SilentlyContinue) { "PASS: Heaven $Quality/$Tessellation" } else { "FAIL: no FPS or score in the output (the run did not finish)" }
