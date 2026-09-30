# SPDX-License-Identifier: MIT
# A Chromium GPU client beside a benchmark: Edge shows a local page (file://, written here) with
# a WebGL animation driven by requestAnimationFrame and a CSS transform animation, as the Steam
# client's steamwebhelper does (D3D11 through ANGLE, DirectComposition). Nothing is fetched: the
# page has no external reference, and Edge starts with a throwaway profile, no sync, no first-run
# pages and no background networking.
# Usage: edge-anim.ps1 [-Seconds 330]
# Runs in the signed-in user's session (upack), so that Edge has a desktop.
param([int]$Seconds = 330)
$ErrorActionPreference = "Stop"
$dir = "C:\Users\Public\caraxes\edge-anim"
New-Item -ItemType Directory -Force $dir | Out-Null
$page = Join-Path $dir "anim.html"
Set-Content -Encoding utf8 $page @'
<!doctype html><html><head><meta charset="utf-8"><title>caraxes edge-anim</title>
<style>body{margin:0;background:#202020;overflow:hidden}
#spin{position:absolute;left:40px;top:40px;width:120px;height:120px;background:#3a8;animation:r 2s linear infinite}
@keyframes r{from{transform:rotate(0)}to{transform:rotate(360deg)}}
canvas{position:absolute;left:200px;top:40px}</style></head><body>
<div id="spin"></div><canvas id="c" width="640" height="480"></canvas><pre id="s" style="color:#ccc;position:absolute;left:40px;top:540px"></pre>
<script>
const gl = document.getElementById('c').getContext('webgl');
const vs = gl.createShader(gl.VERTEX_SHADER);
gl.shaderSource(vs, 'attribute vec2 p; uniform float t; void main(){ float c=cos(t), s=sin(t); gl_Position=vec4(c*p.x-s*p.y, s*p.x+c*p.y, 0.0, 1.0); }');
gl.compileShader(vs);
const fs = gl.createShader(gl.FRAGMENT_SHADER);
gl.shaderSource(fs, 'precision mediump float; uniform float t; void main(){ gl_FragColor=vec4(0.5+0.5*sin(t), 0.4, 0.5+0.5*cos(t), 1.0); }');
gl.compileShader(fs);
const pr = gl.createProgram(); gl.attachShader(pr, vs); gl.attachShader(pr, fs); gl.linkProgram(pr); gl.useProgram(pr);
const b = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, b);
gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([0,0.8, -0.7,-0.5, 0.7,-0.5]), gl.STATIC_DRAW);
const loc = gl.getAttribLocation(pr, 'p'); gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
const ut = gl.getUniformLocation(pr, 't');
let n = 0; const t0 = performance.now();
function frame(now) {
  gl.clearColor(0.1, 0.1, 0.15, 1); gl.clear(gl.COLOR_BUFFER_BIT);
  gl.uniform1f(ut, now / 500); gl.drawArrays(gl.TRIANGLES, 0, 3);
  if (++n % 60 == 0) document.getElementById('s').textContent = n + ' frames, ' + (n * 1000 / (now - t0)).toFixed(1) + ' fps';
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
</script></body></html>
'@
$edge = @("${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe", "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { "FAIL: msedge.exe not found"; exit 1 }
$profile = Join-Path $dir "profile"
$a = @("--user-data-dir=$profile", "--no-first-run", "--no-default-browser-check", "--disable-sync",
       "--disable-background-networking", "--disable-component-update", "--disable-features=msEdgeStartupBoost",
       "--new-window", "--window-size=900,640", "--window-position=40,40", "file:///$($page.Replace('\', '/'))")
$t0 = Get-Date
$p = Start-Process -FilePath $edge -ArgumentList $a -PassThru
"start $($t0.ToString('HH:mm:ss')) edge pid $($p.Id), page $page"
Start-Sleep -Seconds $Seconds
$gpu = Get-Process msedge -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $edge } | Measure-Object
"edge processes at the end: $($gpu.Count)"
# Only the Edge processes of this throwaway profile (their command line names it).
Get-CimInstance Win32_Process -Filter "Name = 'msedge.exe'" | Where-Object { $_.CommandLine -like "*$profile*" } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
"end $((Get-Date).ToString('HH:mm:ss')) after $([int]((Get-Date) - $t0).TotalSeconds) s"
"PASS: edge-anim ran $Seconds s"
