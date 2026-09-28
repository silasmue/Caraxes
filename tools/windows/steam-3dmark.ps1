# SPDX-License-Identifier: MIT
# Steam and 3DMark on the Windows guest, as SYSTEM through the guest agent. The Steam login and
# the 3DMark run itself are the user's, at the guest's console (QEMU window or RDP); this script
# never handles credentials.
#   -Install   Steam, silently (SteamSetup.exe /S from the Steam CDN; guest internet comes from
#              QEMU user networking, no host NAT rule)
#   -Prefer    after 3DMark is installed: every 3DMark executable prefers the high-performance GPU
#              (the NVIDIA adapter; the desktop runs on QEMU's VGA), for each logged-on user
#   -Collect   the scores in the 3DMark result files (*.3dmark-result: a zip of XML files)
param([switch]$Install, [switch]$Prefer, [switch]$Collect)
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$dir = "C:\caraxes"
New-Item -ItemType Directory -Force $dir | Out-Null
$steamDir = "${env:ProgramFiles(x86)}\Steam"

if ($Install) {
    if (Test-Path "$steamDir\steam.exe") {
        Write-Output "STEAM PASS already installed: $steamDir"
    } else {
        $setup = "$dir\SteamSetup.exe"
        & curl.exe -sSL --fail -o $setup "https://cdn.cloudflare.steamstatic.com/client/installer/SteamSetup.exe"
        if ($LASTEXITCODE -ne 0) { throw "SteamSetup download failed (curl exit $LASTEXITCODE)" }
        Start-Process $setup -ArgumentList "/S" -Wait
        if (Test-Path "$steamDir\steam.exe") { Write-Output "STEAM PASS installed: $steamDir" } else { throw "STEAM FAIL: no $steamDir\steam.exe after SteamSetup /S" }
    }
    $free = [math]::Round((Get-PSDrive C).Free / 1GB, 1)
    Write-Output "C: free $free GiB (3DMark Demo 231350: 6.7 GB on disk in the v1 library)"
    Write-Output "next (user, at the console): start Steam, log in, install 3DMark or 3DMark Demo, then run this script with -Prefer"
}

if ($Prefer) {
    $exes = @(foreach ($name in @("3DMark", "3DMark Demo")) {
        $root = "$steamDir\steamapps\common\$name"
        if (Test-Path $root) { Get-ChildItem $root -Recurse -Filter *.exe | Select-Object -ExpandProperty FullName }
    })
    if ($exes.Count -eq 0) { throw "no 3DMark installation under $steamDir\steamapps\common" }
    # Only the hives of logged-on users are loaded under HKEY_USERS.
    $users = @(Get-ChildItem Registry::HKEY_USERS | Where-Object { $_.PSChildName -match '^S-1-5-21-[\d-]+$' })
    if ($users.Count -eq 0) { throw "no logged-on user: log in at the console first" }
    foreach ($u in $users) {
        $key = "Registry::HKEY_USERS\$($u.PSChildName)\Software\Microsoft\DirectX\UserGpuPreferences"
        if (-not (Test-Path $key)) { New-Item -Path $key | Out-Null }
        foreach ($e in $exes) { New-ItemProperty -Path $key -Name $e -Value "GpuPreference=2;" -PropertyType String -Force | Out-Null }
    }
    Write-Output "STEAM PASS GPU preference 'high performance' for $($exes.Count) 3DMark executable(s), $($users.Count) user(s)"
    $exes | ForEach-Object { Write-Output "  $_" }
}

if ($Collect) {
    $files = @(Get-ChildItem C:\Users -Recurse -Depth 6 -Filter *.3dmark-result -ErrorAction SilentlyContinue)
    if ($files.Count -eq 0) { Write-Output "no *.3dmark-result under C:\Users (read the score from the 3DMark window)" }
    foreach ($f in $files) {
        $x = "$dir\3dmark\" + $f.BaseName
        New-Item -ItemType Directory -Force $x | Out-Null
        Copy-Item $f.FullName "$x.zip" -Force
        Expand-Archive "$x.zip" $x -Force
        Write-Output "=== $($f.FullName) ($($f.LastWriteTime))"
        Get-ChildItem $x -Recurse -Filter *.xml | ForEach-Object {
            [xml]$doc = Get-Content $_.FullName -Raw
            $doc.SelectNodes("//*[contains(translate(local-name(),'SCORE','score'),'score') or contains(local-name(),'Fps') or contains(local-name(),'fps')]") |
                Where-Object { $_.InnerText -match '^\s*[\d.]+\s*$' } |
                ForEach-Object { Write-Output ("  {0} = {1}" -f $_.LocalName, $_.InnerText.Trim()) }
        }
    }
}
if (-not ($Install -or $Prefer -or $Collect)) { Write-Output "usage: steam-3dmark.ps1 -Install | -Prefer | -Collect" }
