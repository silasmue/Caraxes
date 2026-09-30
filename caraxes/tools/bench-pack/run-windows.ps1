# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
#
# Runs the render-benchmark pack inside the Windows 11 guest (NVIDIA 617).
# This script runs IN the guest, in the signed-in interactive session — Heaven
# and Superposition need a real desktop to render into, same as on the Linux
# side. Deploy it as a scheduled task with an interactive principal (the
# operator's `run44.py upack` helper does this); this script does not attempt
# to create that task itself, since that lives outside this pack.
#
# What this script needs already sitting on the guest, under -BenchPackDir
# (default C:\bench-pack — stage it there however the guest normally receives
# files, e.g. the same channel run44.py upack uses):
#   blender\benchmark-launcher-cli-3.3.0-windows.zip
#   unigine\Unigine_Heaven-4.0.exe
#   unigine\Unigine_Superposition-1.1.exe
#   cache-seed\blender-benchmark-launcher\...   (copy of cache-seed/windows/blender-benchmark-launcher
#                                                 from fetch-and-seed.sh, so Blender/scenes are not
#                                                 re-downloaded — see README)
#
# Writes one RESULT line per test to stdout and to $OutDir\results.tsv (same
# tab-separated shape as run-linux.sh / run-native.sh), raw logs under
# $OutDir\logs\.

param(
  [string]$BenchPackDir = "C:\bench-pack",
  [string]$OutDir = ".\bench-pack-out\windows",
  [string[]]$BlenderScenes = @("monster", "junkshop", "classroom"),
  [string[]]$BlenderFullDevices = @("CUDA", "OPTIX"),
  [string]$BlenderSanityDevice = "CPU",
  [string]$BlenderSanityScene = "monster",
  [string[]]$HeavenPresets = @("basic", "extreme"),
  [string[]]$SuperpositionPresets = @("1080p_medium", "1080p_high")
)

$ErrorActionPreference = "Continue"
New-Item -ItemType Directory -Force -Path "$OutDir\logs" | Out-Null
$ResultsFile = Join-Path $OutDir "results.tsv"
if (-not (Test-Path $ResultsFile)) {
  "# ts host benchmark preset device_api score fps_avg fps_min duration_s log" | Out-File -Encoding utf8 $ResultsFile
}

function Write-Result {
  param($Benchmark, $Preset, $DeviceApi, $Score = "NA", $FpsAvg = "NA", $FpsMin = "NA", $DurationS = "NA", $Log = "NA")
  $ts = (Get-Date).ToString("o")
  $line = "RESULT`tts=$ts`thost=windows-guest`tbenchmark=$Benchmark`tpreset=$Preset`tdevice_api=$DeviceApi`tscore=$Score`tfps_avg=$FpsAvg`tfps_min=$FpsMin`tduration_s=$DurationS`tlog=$Log"
  Write-Output $line
  Add-Content -Encoding utf8 -Path $ResultsFile -Value $line
}

function Get-LogField {
  param([string]$Path, [string]$Field)
  if (-not (Test-Path $Path)) { return "NA" }
  $m = Select-String -Path $Path -Pattern "^$Field`:\s*([0-9.]+)" | Select-Object -Last 1
  if ($m) { return $m.Matches[0].Groups[1].Value } else { return "NA" }
}

function Parse-BlenderResult {
  param([string]$JsonPath, [string]$Scene)
  if (-not (Test-Path $JsonPath)) { return @("NA", "NA") }
  try {
    $data = Get-Content $JsonPath -Raw | ConvertFrom-Json
    foreach ($entry in $data) {
      if ($entry.scene.label -eq $Scene) {
        return @($entry.stats.samples_per_minute, $entry.stats.total_render_time)
      }
    }
  } catch { }
  return @("NA", "NA")
}

# ---------------------------------------------------------------------------
# Stage: unzip the CLI, install Heaven/Superposition (free Basic edition,
# /SILENT is the Inno-Setup-style switch Unigine's own installer.sh uses for
# Superposition on Windows; Heaven's installer accepts the same switch in
# testing — if it ever pops a wizard instead, click through ONCE interactively
# in the signed-in session, then re-runs are unattended), and copy the
# pre-seeded Blender cache into %LOCALAPPDATA%.
# ---------------------------------------------------------------------------
function Stage {
  $cliDir = Join-Path $BenchPackDir "blender-cli"
  if (-not (Test-Path "$cliDir\benchmark-launcher-cli.exe")) {
    Expand-Archive -Path (Join-Path $BenchPackDir "blender\benchmark-launcher-cli-3.3.0-windows.zip") -DestinationPath $cliDir -Force
  }
  $heavenDir = "C:\Program Files (x86)\Unigine\Heaven Benchmark 4.0"
  if (-not (Test-Path "$heavenDir\bin\Heaven.exe")) {
    Start-Process -FilePath (Join-Path $BenchPackDir "unigine\Unigine_Heaven-4.0.exe") -ArgumentList "/SILENT" -Wait
  }
  $superDir = "C:\Program Files\Unigine\Superposition Benchmark"
  if (-not (Test-Path "$superDir\bin\Superposition.exe")) {
    Start-Process -FilePath (Join-Path $BenchPackDir "unigine\Unigine_Superposition-1.1.exe") -ArgumentList "/SILENT" -Wait
  }
  $cacheSrc = Join-Path $BenchPackDir "cache-seed\blender-benchmark-launcher"
  $cacheDst = Join-Path $env:LOCALAPPDATA "blender-benchmark-launcher"
  if ((Test-Path $cacheSrc) -and -not (Test-Path $cacheDst)) {
    Copy-Item -Recurse -Force $cacheSrc $cacheDst
  }
}

function Run-Blender {
  param([string]$Device, [string]$Scene)
  Write-Host "== Blender $Device / $Scene =="
  $json = Join-Path $OutDir "logs\blender-$Device-$Scene.json"
  $log = Join-Path $OutDir "logs\blender-$Device-$Scene.log"
  $cli = Join-Path $BenchPackDir "blender-cli\benchmark-launcher-cli.exe"
  $t0 = Get-Date
  & $cli benchmark $Scene --blender-version 5.2.0 --device-type $Device --json -v 2 1>$json 2>$log
  $t1 = Get-Date
  $vals = Parse-BlenderResult -JsonPath $json -Scene $Scene
  Write-Result -Benchmark "blender" -Preset $Scene -DeviceApi $Device -Score $vals[0] `
    -DurationS ([int]($t1 - $t0).TotalSeconds) -Log $log
}

# ---------------------------------------------------------------------------
# Heaven / Superposition: same raw-engine-binary technique as run-linux.sh
# (the free Basic edition has no licensed CLI automation; this is the
# Phoronix-Test-Suite-proven invocation, translated to the Windows binary
# layout from pts/unigine-heaven and pts/unigine-super's install_windows.sh,
# both fetched 2026-09-29). Requires the signed-in interactive session so
# there is a real desktop for the D3D11/OpenGL window to render into.
# ---------------------------------------------------------------------------
function Heaven-Flags {
  param([string]$Preset)
  switch ($Preset) {
    "basic"   { return "PHORONIX,RELEASE,QUALITY_HIGH,TESSELLATION_NORMAL" }
    "extreme" { return "PHORONIX,RELEASE,QUALITY_ULTRA,TESSELLATION_EXTREME" }
    default   { return "PHORONIX,RELEASE" }
  }
}

function Run-Heaven {
  param([string]$Preset)
  Write-Host "== Heaven $Preset =="
  $log = Join-Path $OutDir "logs\heaven-$Preset.log"
  $bin = "C:\Program Files (x86)\Unigine\Heaven Benchmark 4.0\bin"
  $heavenArgs = @(
    "-data_path", "../", "-sound_app", "null", "-engine_config", "../data/heaven_4.0.cfg",
    "-system_script", "heaven/unigine.cpp", "-video_mode", "-1",
    "-video_fullscreen", "0", "-video_width", "1920", "-video_height", "1080",
    "-extern_define", (Heaven-Flags $Preset)
  )
  $t0 = Get-Date
  Start-Process -FilePath "$bin\Heaven.exe" -ArgumentList $heavenArgs -WorkingDirectory $bin `
    -RedirectStandardOutput $log -Wait
  $t1 = Get-Date
  $score = Get-LogField $log "Score"; $fps = Get-LogField $log "FPS"; $minfps = Get-LogField $log "Min FPS"
  Write-Result -Benchmark "heaven" -Preset $Preset -DeviceApi "Direct3D11" -Score $score `
    -FpsAvg $fps -FpsMin $minfps -DurationS ([int]($t1 - $t0).TotalSeconds) -Log $log
}

function Superposition-Flags {
  param([string]$Preset)
  switch ($Preset) {
    "1080p_medium" { return @("-shaders_quality", "1", "-textures_quality", "1") }
    "1080p_high"   { return @("-shaders_quality", "2", "-textures_quality", "2") }
    default        { return @("-shaders_quality", "1", "-textures_quality", "1") }
  }
}

function Run-Superposition {
  param([string]$Preset)
  Write-Host "== Superposition $Preset =="
  $log = Join-Path $OutDir "logs\superposition-$Preset.log"
  $bin = "C:\Program Files\Unigine\Superposition Benchmark\bin"
  $autoDir = Join-Path $env:USERPROFILE "Superposition\automation"
  Remove-Item -Recurse -Force $autoDir -ErrorAction SilentlyContinue
  $superArgs = @(
    "-sound_app", "openal", "-system_script", "superposition/system_script.cpp",
    "-data_path", "../", "-engine_config", "../data/superposition/unigine.cfg",
    "-video_mode", "-1", "-project_name", "Superposition", "-video_resizable", "1",
    "-console_command", "config_readonly 1 && world_load superposition/superposition",
    "-mode", "2", "-preset", "0", "-video_width", "1920", "-video_height", "1080"
  ) + (Superposition-Flags $Preset)
  $t0 = Get-Date
  Start-Process -FilePath "$bin\Superposition.exe" -ArgumentList $superArgs -WorkingDirectory $bin -Wait
  $t1 = Get-Date
  if (Test-Path $autoDir) {
    Get-ChildItem $autoDir -Filter "log*.txt" | Get-Content | Out-File -Encoding utf8 $log
  }
  $score = Get-LogField $log "Score"; $fps = Get-LogField $log "FPS"; $minfps = Get-LogField $log "Min FPS"
  Write-Result -Benchmark "superposition" -Preset $Preset -DeviceApi "Direct3D11" -Score $score `
    -FpsAvg $fps -FpsMin $minfps -DurationS ([int]($t1 - $t0).TotalSeconds) -Log $log
}

# ---------------------------------------------------------------------------
Stage

foreach ($scene in $BlenderScenes) {
  foreach ($device in $BlenderFullDevices) { Run-Blender -Device $device -Scene $scene }
}
Run-Blender -Device $BlenderSanityDevice -Scene $BlenderSanityScene

foreach ($preset in $HeavenPresets) { Run-Heaven -Preset $preset }
foreach ($preset in $SuperpositionPresets) { Run-Superposition -Preset $preset }

Write-Host "== done: $ResultsFile =="
