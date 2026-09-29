# SPDX-License-Identifier: MIT
# The media part of the display soak, in the signed-in user's session (run44.py upack):
# Microsoft Edge plays a local 1080p60 H.264 clip and a local VP9 clip in loops (pages with a
# dropped-frame counter drawn over the video, visible in the device's scanout dumps), a
# YouTube video (muted autoplay; the player picks the codec, Turing has no AV1 decode), and a
# WebGL page in an Edge instance on ANGLE's Vulkan backend (--use-angle=vulkan: Vulkan
# presents). Every -SampleSeconds it logs the GPU engines' utilization (video decode, 3D,
# copy, video encode; the performance counters of the GPU engines, not localized) and
# nvidia-smi's decoder utilization; at the end a summary with the decode samples and the
# display driver's TDR events. The clips come from the host (CH-build/soak-*.{mp4,webm}).
param([int]$Minutes = 60, [int]$SampleSeconds = 10,
      [string]$YouTube = 'https://www.youtube.com/embed/LXb3EKWsInQ?autoplay=1&mute=1&loop=1&playlist=LXb3EKWsInQ',
      [switch]$NoYouTube, [switch]$NoVulkan)
$ErrorActionPreference = "Continue"
$dir = 'C:\Users\Public\caraxes'
$log = Join-Path $dir 'soak-media.csv'
$edge = @("${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe", "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe") |
    Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { "FAIL: no msedge.exe"; exit 1 }
$page = @'
<!doctype html><html><body style="margin:0;background:#000">
<video id="v" src="CLIP" autoplay muted loop playsinline style="width:100vw;height:100vh"></video>
<div id="s" style="position:fixed;left:8px;top:8px;font:24px monospace;color:#0f0;background:#000a"></div>
<script>
const v = document.getElementById('v'), s = document.getElementById('s');
setInterval(() => { const q = v.getVideoPlaybackQuality();
  s.textContent = 'CLIP ' + q.totalVideoFrames + ' frames, ' + q.droppedVideoFrames + ' dropped, t=' + v.currentTime.toFixed(1); }, 1000);
</script></body></html>
'@
foreach ($c in 'soak-h264-1080p60.mp4', 'soak-vp9-1080p60.webm') {
    if (-not (Test-Path (Join-Path $dir $c))) { "FAIL: $c missing in $dir"; exit 1 }
    Set-Content -Encoding utf8 (Join-Path $dir "$c.html") ($page -replace 'CLIP', $c)
}
Set-Content -Encoding utf8 (Join-Path $dir 'soak-webgl.html') @'
<!doctype html><html><body style="margin:0;background:#000"><canvas id="c" style="width:100vw;height:100vh"></canvas>
<div id="s" style="position:fixed;left:8px;top:8px;font:24px monospace;color:#ff0"></div><script>
const c = document.getElementById('c'), gl = c.getContext('webgl2'), s = document.getElementById('s');
s.textContent = gl ? gl.getParameter(gl.getExtension('WEBGL_debug_renderer_info').UNMASKED_RENDERER_WEBGL) : 'no WebGL2';
let n = 0, t0 = performance.now();
function frame(t) { c.width = c.clientWidth; c.height = c.clientHeight; const k = (t / 1000) % 1;
  gl.viewport(0, 0, c.width, c.height); gl.clearColor(k, 1 - k, 0.5, 1); gl.clear(gl.COLOR_BUFFER_BIT);
  if (++n % 60 == 0) { s.textContent = s.textContent.split(' | ')[0] + ' | ' + (n * 1000 / (t - t0)).toFixed(1) + ' fps'; }
  requestAnimationFrame(frame); }
requestAnimationFrame(frame);
</script></body></html>
'@
$common = @('--no-first-run', '--no-default-browser-check', '--autoplay-policy=no-user-gesture-required', '--disable-sync')
$procs = @()
$procs += Start-Process $edge -PassThru -ArgumentList ($common + @("--user-data-dir=$dir\edge-a", '--window-position=0,0', '--window-size=960,540', "--app=file:///$($dir -replace '\\','/')/soak-h264-1080p60.mp4.html"))
$procs += Start-Process $edge -PassThru -ArgumentList ($common + @("--user-data-dir=$dir\edge-b", '--window-position=960,0', '--window-size=960,540', "--app=file:///$($dir -replace '\\','/')/soak-vp9-1080p60.webm.html"))
if (-not $NoYouTube) {
    $procs += Start-Process $edge -PassThru -ArgumentList ($common + @("--user-data-dir=$dir\edge-c", '--window-position=0,540', '--window-size=960,540', "--app=$YouTube"))
}
if (-not $NoVulkan) {
    $procs += Start-Process $edge -PassThru -ArgumentList ($common + @("--user-data-dir=$dir\edge-vk", '--use-angle=vulkan', '--window-position=960,540', '--window-size=960,540', "--app=file:///$($dir -replace '\\','/')/soak-webgl.html"))
}
"started $($procs.Count) Edge instance(s) at $(Get-Date -Format HH:mm:ss); sampling every $SampleSeconds s for $Minutes min into $log"
$smi = Get-Command nvidia-smi.exe -ErrorAction SilentlyContinue
Set-Content -Encoding ascii $log 'time,decode_pct,3d_pct,copy_pct,encode_pct,smi_dec_pct,smi_gpu_pct,edge_procs'
$end = (Get-Date).AddMinutes($Minutes); $samples = 0; $decoding = 0
while ((Get-Date) -lt $end) {
    Start-Sleep $SampleSeconds
    $eng = Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine -ErrorAction SilentlyContinue
    $sum = { param($t) [int](($eng | Where-Object { $_.Name -like "*engtype_$t*" } | Measure-Object UtilizationPercentage -Sum).Sum) }
    $dec = & $sum 'VideoDecode'; $g3d = & $sum '3D'; $cp = & $sum 'Copy'; $enc = & $sum 'VideoEncode'
    $sd = ''; $sg = ''
    if ($smi) { $q = (& $smi.Source --query-gpu=utilization.decoder,utilization.gpu --format=csv,noheader,nounits 2>$null) -split ','; if ($q.Count -ge 2) { $sd = $q[0].Trim(); $sg = $q[1].Trim() } }
    $alive = @(Get-Process msedge -ErrorAction SilentlyContinue).Count
    Add-Content -Encoding ascii $log ("{0:HH:mm:ss},{1},{2},{3},{4},{5},{6},{7}" -f (Get-Date), $dec, $g3d, $cp, $enc, $sd, $sg, $alive)
    $samples++; if ($dec -gt 0 -or ($sd -match '^\d+$' -and [int]$sd -gt 0)) { $decoding++ }
}
$start = (Get-Date).AddMinutes(-$Minutes - 1)
$tdr = @(Get-WinEvent -FilterHashtable @{ LogName = 'System'; StartTime = $start } -ErrorAction SilentlyContinue |
    Where-Object { $_.ProviderName -match 'nvlddmkm|Display' -or $_.Id -in 4101, 4097 })
"samples $samples, with video decode activity $decoding; display driver events since the start: $($tdr.Count)"
$tdr | Select-Object -First 10 | ForEach-Object { "  {0:HH:mm:ss} {1} id {2}: {3}" -f $_.TimeCreated, $_.ProviderName, $_.Id, (($_.Properties | ForEach-Object { $_.Value }) -join ' ; ') }
if ($decoding -gt 0 -and $tdr.Count -eq 0) { "PASS: media soak, hardware decode seen, no TDR" } else { "FAIL: media soak (decode samples $decoding, events $($tdr.Count))"; exit 1 }
