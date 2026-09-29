# SPDX-License-Identifier: MIT
# Unigine Superposition 1.1 (free Basic edition) in the signed-in user's session (run44.py
# upack): installs it once from C:\bench-pack\unigine (silent Inno Setup), runs one preset in
# benchmark mode on DirectX 11 at 1920x1080 and prints the automation log's Score, FPS, Min FPS
# and Max FPS; -Seconds N runs it as a fixed-length load instead. The engine flags are the bench pack's (caraxes/tools/bench-pack/run-windows.ps1,
# after the Phoronix Test Suite's pts/unigine-super). -Preset 1080p_medium | 1080p_high.
param([string]$Preset = "1080p_medium", [string]$BenchPackDir = "C:\bench-pack", [int]$Seconds = 0)
$ErrorActionPreference = "Continue"
$bin = "C:\Program Files\Unigine\Superposition Benchmark\bin"
if (-not (Test-Path "$bin\Superposition.exe")) {
    $exe = Join-Path $BenchPackDir "unigine\Unigine_Superposition-1.1.exe"
    if (-not (Test-Path $exe)) { "FAIL: $exe missing"; exit 1 }
    "installing $(Get-Date -Format HH:mm:ss)"
    Start-Process -FilePath $exe -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/SP-" -Wait
    if (-not (Test-Path "$bin\Superposition.exe")) { "FAIL: the installer left no $bin\Superposition.exe"; exit 1 }
}
$q = switch ($Preset) { "1080p_high" { "2" } default { "1" } }
$autoDir = Join-Path $env:USERPROFILE "Superposition\automation"
Remove-Item -Recurse -Force $autoDir -ErrorAction SilentlyContinue
$a = @("-sound_app", "openal", "-system_script", "superposition/system_script.cpp",
       "-data_path", "../", "-engine_config", "../data/superposition/unigine.cfg",
       "-video_mode", "-1", "-project_name", "Superposition", "-video_resizable", "1",
       # Start-Process joins the list with spaces and quotes nothing (Windows PowerShell 5.1): an
       # unquoted command is split, the engine then runs only "config_readonly" and loads no world
       # (a black scene at 500-870 fps, C87/C90).
       "-console_command", '"config_readonly 1 && world_load superposition/superposition"',
       "-mode", "2", "-preset", "0", "-video_width", "1920", "-video_height", "1080",
       "-video_fullscreen", "0", "-video_app", "direct3d11",
       "-shaders_quality", $q, "-textures_quality", $q)
$t0 = Get-Date
"start $($t0.ToString('HH:mm:ss')) preset $Preset"
$p = Start-Process -FilePath "$bin\Superposition.exe" -ArgumentList $a -WorkingDirectory $bin -PassThru
# -Seconds N: a fixed-length load (the render keeps going in this mode); stopped after N s.
if ($Seconds -gt 0) {
    if (-not $p.WaitForExit($Seconds * 1000)) { $p.Kill(); "PASS: Superposition $Preset rendered for $Seconds s (stopped)"; exit 0 }
} elseif (-not $p.WaitForExit(900000)) { $p.Kill(); "FAIL: Superposition did not finish in 15 min"; exit 1 }
$t1 = Get-Date
"end $($t1.ToString('HH:mm:ss')) after $([int]($t1 - $t0).TotalSeconds) s, exit $($p.ExitCode)"
$logs = Get-ChildItem $autoDir -Filter "log*.txt" -ErrorAction SilentlyContinue
if (-not $logs) { "FAIL: no automation log in $autoDir"; exit 1 }
$text = $logs | Get-Content
$text | Select-String -Pattern "Score|FPS|Render|API|Resolution|Preset|Quality" | ForEach-Object { $_.Line }
$score = ($text | Select-String -Pattern "^Score:\s*([0-9.]+)" | Select-Object -Last 1)
if ($score) { "PASS: Superposition $Preset" } else { "FAIL: no score in the log"; exit 1 }
