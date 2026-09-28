# SPDX-License-Identifier: MIT
# Windows smoke checks for the synthetic GPU, as SYSTEM through the guest agent (P18-build/run18.py
# pack smoke.ps1). Names (nvidia-smi, Device Manager, DXGI, registry), Vulkan enumeration, the D3D11
# triangle gate, the D3D12 compute gate, display-driver events since the start. One
# "SMOKE PASS|FAIL|INFO <check>" line each; the siblings are in C:\Windows\Temp as caraxes-*.ps1.
param([int]$GateSeconds = 90)
$ErrorActionPreference = "Continue"
$ProgressPreference = "SilentlyContinue"
$start = Get-Date
$dir = "C:\caraxes"
New-Item -ItemType Directory -Force $dir | Out-Null
$results = New-Object System.Collections.Generic.List[string]
function Say([string]$verdict, [string]$what) {
    $line = "SMOKE $verdict $what"; $results.Add($line); Write-Output $line
}
function Verdict($ok) { if ($ok) { "PASS" } else { "FAIL" } }
# A bounded child process; returns exit code (-1 on timeout) and stdout + stderr.
function Invoke-Bounded([string]$exe, [string]$arguments, [int]$seconds, [string]$tag) {
    $out = "$dir\smoke-$tag.out.txt"; $err = "$dir\smoke-$tag.err.txt"
    $p = Start-Process -FilePath $exe -ArgumentList $arguments -NoNewWindow -PassThru `
        -RedirectStandardOutput $out -RedirectStandardError $err
    if (-not $p.WaitForExit($seconds * 1000)) { $p.Kill(); return @(-1, "TIMEOUT after $seconds s") }
    return @($p.ExitCode, ((Get-Content $out -Raw) + (Get-Content $err -Raw)))
}
function Invoke-Gate([string]$script, [string]$tag) {
    Invoke-Bounded "powershell.exe" "-NoProfile -NonInteractive -ExecutionPolicy Bypass -File C:\Windows\Temp\caraxes-$script" $GateSeconds $tag
}

Write-Output "=== Device Manager"
$devs = @(Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like 'PCI\VEN_10DE*' -and $_.Class -eq 'Display' })
foreach ($d in $devs) {
    $prop = { param($k) (Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName $k -ErrorAction SilentlyContinue).Data }
    $problem = & $prop 'DEVPKEY_Device_ProblemCode'
    Write-Output ("name '{0}', DeviceDesc '{1}', status {2}, problem {3}" -f $d.FriendlyName, (& $prop 'DEVPKEY_Device_DeviceDesc'), $d.Status, $problem)
    Write-Output ("  hardware ids: " + ((& $prop 'DEVPKEY_Device_HardwareIds') -join ', '))
    Write-Output ("  driver: " + (& $prop 'DEVPKEY_Device_DriverVersion') + ", inf " + (& $prop 'DEVPKEY_Device_DriverInfPath'))
    Say (Verdict ($d.Status -eq 'OK' -and $problem -eq 0)) "Device Manager: '$($d.FriendlyName)' status $($d.Status), problem code $problem"
    Say "INFO" ("own name in Device Manager: " + $(if ($d.FriendlyName -like '*Leandro*') { "yes" } else { "no (INF DriverDesc)" }))
}
if ($devs.Count -eq 0) { Say "FAIL" "no NVIDIA display device present" }
Get-CimInstance Win32_VideoController | ForEach-Object {
    Write-Output ("Win32_VideoController: '{0}' RAM {1} MiB driver {2} error {3} processor '{4}'" -f $_.Name, ($_.AdapterRAM / 1MB), $_.DriverVersion, $_.ConfigManagerErrorCode, $_.VideoProcessor)
}
# The adapter strings the KMD wrote (display class key; binary UTF-16 values).
Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}' -ErrorAction SilentlyContinue |
    ForEach-Object { Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue } |
    Where-Object { $_.ProviderName -eq 'NVIDIA' } | ForEach-Object {
        $k = $_
        $str = { param($v) if ($v -is [byte[]]) { [Text.Encoding]::Unicode.GetString($v).TrimEnd([char]0) } else { "$v" } }
        Write-Output ("registry: DriverDesc '{0}', AdapterString '{1}', ChipType '{2}', BiosString '{3}', MemorySize {4}" -f $k.DriverDesc,
            (& $str $k.'HardwareInformation.AdapterString'), (& $str $k.'HardwareInformation.ChipType'),
            (& $str $k.'HardwareInformation.BiosString'), $k.'HardwareInformation.qwMemorySize')
    }

Write-Output "=== nvidia-smi"
$smi = "$env:SystemRoot\System32\nvidia-smi.exe"
$r = Invoke-Bounded $smi "-L" 60 "smi-L"
Write-Output $r[1]
$name = [regex]::Match("" + $r[1], 'GPU 0: (.+?) \(UUID').Groups[1].Value
Say (Verdict ($r[0] -eq 0 -and $name)) "nvidia-smi -L: exit $($r[0]), name '$name'"
Say "INFO" ("own name in nvidia-smi: " + $(if ($name -like '*Leandro*') { "yes" } else { "no" }))
$r = Invoke-Bounded $smi "--query-gpu=name,pci.bus_id,driver_version,vbios_version,memory.total,memory.used,utilization.gpu,temperature.gpu,power.draw --format=csv" 60 "smi-query"
Write-Output $r[1]
$r = Invoke-Bounded $smi "-q" 120 "smi-q"
Say (Verdict ($r[0] -eq 0)) "nvidia-smi -q: exit $($r[0]), $((("" + $r[1]) -split "`n").Count) lines in $dir\smoke-smi-q.out.txt"

Write-Output "=== Vulkan"
$r = Invoke-Bounded "$env:SystemRoot\System32\vulkaninfo.exe" "--summary" 60 "vulkan"
$gpus = (("" + $r[1]) -split "`n") | Where-Object { $_ -match 'deviceName|deviceType|driverName|driverInfo|apiVersion' }
Write-Output $gpus
Say (Verdict ($r[0] -eq 0 -and ("" + $r[1]) -match 'NVIDIA|Leandro')) "vulkaninfo --summary: exit $($r[0]) (enumeration only)"

Write-Output "=== D3D11 triangle gate"
$r = Invoke-Gate "d3d11-render.ps1" "d3d11"
Write-Output $r[1]
Say (Verdict ("" + $r[1] -match 'PASS: NVIDIA D3D11')) "D3D11 triangle + readback: exit $($r[0])"

Write-Output "=== D3D12 compute gate"
$r = Invoke-Gate "d3d12-probe.ps1" "d3d12"
Write-Output $r[1]
Say (Verdict ("" + $r[1] -match 'PASS: NVIDIA D3D12')) "D3D12 fence + compute + readback: exit $($r[0])"

Write-Output "=== Display driver events since the start"
$ev = @(Get-WinEvent -FilterHashtable @{ LogName = 'System'; StartTime = $start } -ErrorAction SilentlyContinue |
    Where-Object { $_.ProviderName -match 'nvlddmkm|Display|DxgKrnl' -and $_.Level -le 3 })
$ev | Select-Object -First 20 | ForEach-Object { Write-Output ("{0} {1} id {2} level {3}: {4}" -f $_.TimeCreated, $_.ProviderName, $_.Id, $_.Level, ($_.Message -split "`n")[0]) }
$tdr = @($ev | Where-Object { $_.Id -eq 4101 -or $_.ProviderName -eq 'nvlddmkm' })
Say (Verdict ($tdr.Count -eq 0)) "no TDR (4101) or nvlddmkm error since the start ($($ev.Count) warning/error event(s) in total)"

Write-Output "=== summary"
$results
if ($results | Where-Object { $_ -like "SMOKE FAIL*" }) { exit 1 }
