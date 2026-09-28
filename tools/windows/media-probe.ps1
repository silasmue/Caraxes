# SPDX-License-Identifier: MIT
# NVENC and NVDEC through ffmpeg: H.264 and HEVC encode of a generated 1080p60 clip, decode of both
# with -hwaccel cuda and with the cuvid decoders, frame counts checked, fps reported.
# ffmpeg is not on the image: -Fetch downloads the BtbN win64 GPL build (anonymous GitHub request,
# needs guest internet; QEMU user networking gives it), or place it at C:\caraxes\ffmpeg\bin first.
# Usage: media-probe.ps1 [-Fetch] [-Seconds 10]
param([switch]$Fetch, [int]$Seconds = 10)
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$dir = "C:\caraxes"
$bin = "$dir\ffmpeg\bin"
New-Item -ItemType Directory -Force $dir | Out-Null
if (-not (Test-Path "$bin\ffmpeg.exe")) {
    if (-not $Fetch) { throw "no $bin\ffmpeg.exe: run with -Fetch or copy an ffmpeg build there" }
    $zip = "$dir\ffmpeg.zip"
    $url = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
    & curl.exe -sSL --fail -o $zip $url
    if ($LASTEXITCODE -ne 0) { throw "download failed (curl exit $LASTEXITCODE): $url" }
    Expand-Archive $zip -DestinationPath "$dir\ffmpeg-x" -Force
    $exe = Get-ChildItem "$dir\ffmpeg-x" -Recurse -Filter ffmpeg.exe | Select-Object -First 1
    New-Item -ItemType Directory -Force "$dir\ffmpeg" | Out-Null
    Copy-Item (Split-Path $exe.FullName) "$dir\ffmpeg\" -Recurse -Force
    Remove-Item $zip, "$dir\ffmpeg-x" -Recurse -Force
}
$frames = 60 * $Seconds
$results = New-Object System.Collections.Generic.List[string]

# One bounded ffmpeg/ffprobe run; stdout and stderr to files (PowerShell 5.1 turns native stderr
# into errors otherwise). Returns exit code and the stderr text.
function Invoke-Tool([string]$exe, [string]$arguments, [string]$tag, [int]$timeout = 300) {
    $out = "$dir\$tag.out.txt"; $err = "$dir\$tag.err.txt"
    $p = Start-Process -FilePath "$bin\$exe" -ArgumentList $arguments -NoNewWindow -PassThru `
        -RedirectStandardOutput $out -RedirectStandardError $err
    if (-not $p.WaitForExit($timeout * 1000)) { $p.Kill(); return @(-1, "TIMEOUT after $timeout s", "") }
    return @($p.ExitCode, (Get-Content $err -Raw), (Get-Content $out -Raw))
}
function Fps([string]$log) {
    # -benchmark prints "bench: utime=..s stime=..s rtime=..s"; the last progress line has frame=N.
    $f = [regex]::Matches($log, 'frame=\s*(\d+)') | Select-Object -Last 1
    $t = [regex]::Match($log, 'rtime=([\d.]+)s')
    if ($f -and $t.Success -and [double]$t.Groups[1].Value -gt 0) {
        return "{0} frames in {1} s = {2:F0} fps" -f $f.Groups[1].Value, $t.Groups[1].Value, ([double]$f.Groups[1].Value / [double]$t.Groups[1].Value)
    }
    return "fps unknown"
}
function Say($ok, $what) {
    $line = "MEDIA " + $(if ($ok) { "PASS" } else { "FAIL" }) + " " + $what
    $results.Add($line); Write-Output $line
}

$r = Invoke-Tool "ffmpeg.exe" "-hide_banner -encoders" "encoders" 60
Write-Output ("encoders: " + ((($r[2] -split "`n") | Where-Object { $_ -match 'nvenc' } | ForEach-Object { ($_ -split '\s+')[2] }) -join ' '))
$r = Invoke-Tool "ffmpeg.exe" "-hide_banner -decoders" "decoders" 60
Write-Output ("decoders: " + ((($r[2] -split "`n") | Where-Object { $_ -match 'cuvid' } | ForEach-Object { ($_ -split '\s+')[2] }) -join ' '))

$src = "-f lavfi -i testsrc2=size=1920x1080:rate=60:duration=$Seconds"
foreach ($codec in @("h264", "hevc")) {
    $file = "$dir\nvenc-$codec.mp4"
    $r = Invoke-Tool "ffmpeg.exe" "-hide_banner -benchmark -y $src -c:v ${codec}_nvenc -preset p4 -tune ll -b:v 10M $file" "enc-$codec"
    Say ($r[0] -eq 0 -and (Test-Path $file)) "NVENC $codec encode: exit $($r[0]), $(Fps $r[1])"
    if ($r[0] -ne 0) { Write-Output (($r[1] -split "`n") | Select-Object -Last 8) }
}
foreach ($codec in @("h264", "hevc")) {
    $file = "$dir\nvenc-$codec.mp4"
    if (-not (Test-Path $file)) { Say $false "NVDEC ${codec}: no encoded input"; continue }
    $p = Invoke-Tool "ffprobe.exe" "-v error -count_frames -select_streams v:0 -show_entries stream=nb_read_frames -of csv=p=0 $file" "probe-$codec" 120
    $n = ("" + $p[2]).Trim()
    Say ($n -eq "$frames") "encoded $codec file holds $n of $frames frames"
    $r = Invoke-Tool "ffmpeg.exe" "-hide_banner -benchmark -hwaccel cuda -hwaccel_output_format cuda -i $file -f null -" "dec-cuda-$codec"
    $got = [regex]::Matches($r[1], 'frame=\s*(\d+)') | Select-Object -Last 1
    Say ($r[0] -eq 0 -and $got -and $got.Groups[1].Value -eq "$frames") "NVDEC $codec (-hwaccel cuda): exit $($r[0]), $(Fps $r[1])"
    $r = Invoke-Tool "ffmpeg.exe" "-hide_banner -benchmark -c:v ${codec}_cuvid -i $file -f null -" "dec-cuvid-$codec"
    $got = [regex]::Matches($r[1], 'frame=\s*(\d+)') | Select-Object -Last 1
    Say ($r[0] -eq 0 -and $got -and $got.Groups[1].Value -eq "$frames") "NVDEC $codec (${codec}_cuvid): exit $($r[0]), $(Fps $r[1])"
}
Write-Output "--- summary"
$results
if ($results | Where-Object { $_ -like "MEDIA FAIL*" }) { exit 1 }
