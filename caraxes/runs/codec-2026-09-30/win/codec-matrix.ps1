# SPDX-License-Identifier: MIT
# Codec matrix on the Windows guest (ffmpeg in C:\caraxes\codec): NVENC H.264/HEVC encode of a
# 30 s 1080p60 clip (frames decoded by NVDEC into CUDA memory, and a CPU-generated source), NVDEC
# decode of H.264/HEVC/VP9/AV1 with -hwaccel d3d11va and cuda (fps without download; framemd5
# against the host's software decode), the cuvid decoders, AV1 as the negative case.
# One line per test: "MATRIX <test> PASS|FAIL|NEG <detail>".
param([int]$Seconds = 30, [string]$Only = "")
$ErrorActionPreference = "Stop"
$d = "C:\caraxes\codec"
$ff = "$d\ffmpeg.exe"; $fp = "$d\ffprobe.exe"
$frames = 60 * $Seconds

function Run([string]$exe, [string]$arguments, [string]$tag, [int]$timeout = 300) {
    $out = "$d\log-$tag.out.txt"; $err = "$d\log-$tag.err.txt"
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $exe -ArgumentList $arguments -NoNewWindow -PassThru `
        -RedirectStandardOutput $out -RedirectStandardError $err
    $null = $p.Handle
    if (-not $p.WaitForExit($timeout * 1000)) { $p.Kill(); return @(-1, "TIMEOUT after $timeout s", "", $timeout) }
    # [string] keeps an empty file as "" (a $null element would vanish from the array).
    return @($p.ExitCode, [string](Get-Content $err -Raw), [string](Get-Content $out -Raw), $sw.Elapsed.TotalSeconds)
}
function Frames([string]$log) {
    $f = [regex]::Matches("" + $log, 'frame=\s*(\d+)') | Select-Object -Last 1
    if ($f) { return [int]$f.Groups[1].Value } else { return 0 }
}
function Fps([string]$log, [double]$wall) {
    $n = Frames $log
    $t = [regex]::Match("" + $log, 'rtime=([\d.]+)s')
    $s = if ($t.Success) { [double]$t.Groups[1].Value } else { $wall }
    if ($s -gt 0) { return ("{0} frames {1:F2} s {2:F1} fps" -f $n, $s, ($n / $s)) } else { return "$n frames" }
}
function Fallback([string]$log) {
    return ([string]$log) -match 'Failed setup for format|hwaccel initialisation returned error|not supported|No device available for decoder|Unsupported|Your platform doesn''t support|Failed to create|Error while decoding'
}
function Say([string]$st, [string]$what) { Write-Output "MATRIX $what $st" }
function Hashes([string]$file) {
    return @(Get-Content $file | Where-Object { $_ -notmatch '^#' } | ForEach-Object { ($_ -split ',')[-1].Trim() })
}
function Want([string]$t) { return ($Only -eq "") -or ($t -match $Only) }

Write-Output ("ffmpeg: " + ((& $ff -hide_banner -version 2>&1 | Select-Object -First 1)))
$gpu = (Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name + " " + $_.DriverVersion }) -join "; "
Write-Output "gpu: $gpu"

# --- encode
foreach ($c in @("h264", "hevc")) {
    if (-not (Want "enc-$c")) { continue }
    $o = "$d\enc-$c.mp4"
    Remove-Item -Force -ErrorAction SilentlyContinue $o
    $r = Run $ff "-hide_banner -benchmark -y -hwaccel cuda -hwaccel_output_format cuda -i $d\clip-h264.mp4 -t $Seconds -c:v ${c}_nvenc -preset p4 -tune ll -b:v 20M $o" "enc-$c-gpu"
    $ok = $r[0] -eq 0 -and (Test-Path $o)
    $n = ""
    if ($ok) {
        $q = Run $fp "-v error -count_frames -select_streams v:0 -show_entries stream=nb_read_frames -of csv=p=0 $o" "enc-$c-count" 120
        $n = ([string]$q[2]).Trim()
        $v = Run $ff "-hide_banner -v error -i $o -f null -" "enc-$c-verify" 300
        $ok = $ok -and ($n -eq "$frames") -and ([string]::IsNullOrWhiteSpace($v[1]))
    }
    Say $(if ($ok) { "PASS" } else { "FAIL" }) "enc-$c-nvenc (NVDEC->CUDA->NVENC, 1080p60 ${Seconds}s): exit $($r[0]), $(Fps $r[1] $r[3]), out frames $n"
    if (-not $ok) { Write-Output ((([string]$r[1]) -split "`n") | Select-Object -Last 6) }

    $o = "$d\enc-$c-cpu.mp4"
    Remove-Item -Force -ErrorAction SilentlyContinue $o
    $r = Run $ff "-hide_banner -benchmark -y -f lavfi -i testsrc2=size=1920x1080:rate=60:duration=$Seconds -c:v ${c}_nvenc -preset p4 -tune ll -b:v 20M $o" "enc-$c-cpu"
    $ok = $r[0] -eq 0 -and (Test-Path $o) -and ((Frames $r[1]) -eq $frames)
    Say $(if ($ok) { "PASS" } else { "FAIL" }) "enc-$c-nvenc-cpusrc (testsrc2 on the CPU, uploaded): exit $($r[0]), $(Fps $r[1] $r[3])"
    if (-not $ok) { Write-Output ((([string]$r[1]) -split "`n") | Select-Object -Last 6) }
}
if (Want "enc-av1") {
    $r = Run $ff "-hide_banner -y -f lavfi -i testsrc2=size=1920x1080:rate=60:duration=1 -c:v av1_nvenc $d\enc-av1.mp4" "enc-av1" 60
    $clean = $r[0] -ne 0 -and ([string]$r[1]) -match 'not supported|No capable devices|unsupported|Codec not supported'
    $why = (([string]$r[1]) -split "`n" | Where-Object { $_ -match 'nvenc|support|capab|Error' } | Select-Object -First 2) -join ' | '
    $st = if ($clean) { "NEG" } elseif ($r[0] -eq 0) { "FAIL(unexpected success)" } else { "FAIL" }
    Say $st ("enc-av1-nvenc (TU106 has no AV1 encode): exit " + $r[0] + ", " + $why)
}

# --- decode
$clips = [ordered]@{ h264 = "clip-h264.mp4"; hevc = "clip-hevc.mp4"; vp9 = "clip-vp9.webm"; av1 = "clip-av1.mp4" }
foreach ($c in $clips.Keys) {
    $clip = "$d\" + $clips[$c]
    $ref = Hashes "$clip.sw.framemd5"
    foreach ($hw in @("d3d11va", "cuda")) {
        if (-not (Want "dec-$c-$hw")) { continue }
        $ofmt = if ($hw -eq "cuda") { "cuda" } else { "d3d11" }
        # speed: frames stay in video memory
        $r = Run $ff "-hide_banner -benchmark -hwaccel $hw -hwaccel_output_format $ofmt -i $clip -f null -" "dec-$c-$hw-speed"
        $fb = Fallback $r[1]
        $speed = Fps $r[1] $r[3]
        # correctness: downloaded, converted to yuv420p, framemd5 against the host's software decode
        $m = "$d\dec-$c-$hw.framemd5"
        Remove-Item -Force -ErrorAction SilentlyContinue $m
        $r2 = Run $ff "-hide_banner -hwaccel $hw -i $clip -pix_fmt yuv420p -f framemd5 $m" "dec-$c-$hw-md5"
        $fb = $fb -or (Fallback $r2[1])
        $got = if (Test-Path $m) { Hashes $m } else { @() }
        $same = 0
        for ($i = 0; $i -lt [Math]::Min($got.Count, $ref.Count); $i++) { if ($got[$i] -eq $ref[$i]) { $same++ } }
        $exact = ($got.Count -eq $ref.Count) -and ($same -eq $ref.Count)
        if ($c -eq "av1") {
            $st = if ($fb -or $r[0] -ne 0) { "NEG" } else { "FAIL(av1 claimed by hw?)" }
        } else {
            $st = if ($r[0] -eq 0 -and -not $fb -and $exact) { "PASS" } else { "FAIL" }
        }
        Say $st "dec-$c-${hw}: exit $($r[0])/$($r2[0]), speed $speed, framemd5 $same/$($ref.Count) equal, fallback=$fb"
        if ($st -ne "PASS") { Write-Output ((([string]$r[1]) -split "`n") | Where-Object { $_ -match 'hwaccel|Failed|support|Error|error' } | Select-Object -First 4) }
    }
    if (Want "dec-$c-cuvid") {
        $r = Run $ff "-hide_banner -benchmark -c:v ${c}_cuvid -i $clip -f null -" "dec-$c-cuvid" 300
        $n = Frames $r[1]
        if ($c -eq "av1") {
            $st = if ($r[0] -ne 0) { "NEG" } else { "FAIL(unexpected success)" }
        } else {
            $st = if ($r[0] -eq 0 -and $n -eq $frames) { "PASS" } else { "FAIL" }
        }
        Say $st "dec-$c-cuvid: exit $($r[0]), $(Fps $r[1] $r[3])"
        if ($st -ne "PASS") { Write-Output ((([string]$r[1]) -split "`n") | Where-Object { $_ -match 'cuvid|CUDA|support|Error|error' } | Select-Object -First 3) }
    }
}
Write-Output "done $(Get-Date -Format HH:mm:ss)"
