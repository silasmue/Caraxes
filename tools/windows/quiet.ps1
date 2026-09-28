# SPDX-License-Identifier: MIT
# Windows Update state of the test image, and (-Apply) a quiet test week: quality and feature
# updates paused for -Days days, no drivers from Windows Update (the guest driver stays the
# stock 617.14 the tests name). Run P26 found an LCU (KB5129195) installing during the workload
# stages and pending at shutdown; the next boot powered off after 66 s.
# Usage: quiet.ps1 [-Apply] [-Days 7]      (reversible: delete the values it prints)
param([switch]$Apply, [int]$Days = 7)
$ErrorActionPreference = 'Stop'
$cv = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
"OS: $($cv.ProductName) $($cv.DisplayVersion) build $($cv.CurrentBuild).$($cv.UBR)"
Get-HotFix | Sort-Object InstalledOn -Descending | Select-Object -First 5 HotFixID, InstalledOn | Format-Table -AutoSize | Out-String
$cbs = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing'
"CBS RebootPending: $(Test-Path "$cbs\RebootPending"); PackagesPending: $(Test-Path "$cbs\PackagesPending")"
"WU RebootRequired: $(Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired')"
Get-Service wuauserv, TrustedInstaller, UsoSvc | Format-Table Name, Status, StartType -AutoSize | Out-String
$ux = 'HKLM:\SOFTWARE\Microsoft\WindowsUpdate\UX\Settings'
$pol = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate'
$ds = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\DriverSearching'
if ($Apply) {
    $now = (Get-Date).ToUniversalTime()
    $start = $now.ToString('yyyy-MM-ddTHH:mm:ssZ')
    $end = $now.AddDays($Days).ToString('yyyy-MM-ddTHH:mm:ssZ')
    New-Item -Force $ux | Out-Null
    foreach ($n in 'PauseFeatureUpdatesStartTime', 'PauseQualityUpdatesStartTime') { Set-ItemProperty $ux $n $start }
    foreach ($n in 'PauseFeatureUpdatesEndTime', 'PauseQualityUpdatesEndTime', 'PauseUpdatesExpiryTime') { Set-ItemProperty $ux $n $end }
    if (-not (Test-Path $pol)) { New-Item -Force $pol | Out-Null }
    New-ItemProperty -Force $pol ExcludeWUDriversInQualityUpdate -PropertyType DWord -Value 1 | Out-Null
    New-ItemProperty -Force $ds SearchOrderConfig -PropertyType DWord -Value 0 | Out-Null
    "APPLIED: updates paused until $end; no Windows Update drivers"
}
"UX\Settings:"; Get-ItemProperty $ux -ErrorAction SilentlyContinue | Select-Object Pause* | Format-List | Out-String
"Policy ExcludeWUDriversInQualityUpdate: $((Get-ItemProperty $pol -ErrorAction SilentlyContinue).ExcludeWUDriversInQualityUpdate)"
"DriverSearching SearchOrderConfig: $((Get-ItemProperty $ds -ErrorAction SilentlyContinue).SearchOrderConfig)"
$nv = Get-CimInstance Win32_VideoController | Where-Object Name -match 'NVIDIA'
"NVIDIA driver: $($nv.DriverVersion) ($($nv.Name))"
