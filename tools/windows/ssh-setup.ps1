# SPDX-License-Identifier: MIT
# The guest side of the Cloud Hypervisor tooling: the OpenSSH server with key authentication only,
# and a static address on the Caraxes tap NIC. Cloud Hypervisor has no virtio-serial port, so the
# QEMU guest agent is gone there; the run kits reach the guest over SSH on the tap instead.
# The NIC is a new device instance under each hypervisor, so the address is not stored once: a
# SYSTEM task sets it at every boot on the adapter that carries the tap MAC.
# Run once from a QEMU maintenance boot (guest agent, SYSTEM). Idempotent.
# Usage: ssh-setup.ps1 -Keys <file with public keys> [-Mac 02-00-00-00-00-47]
#                      [-Address 192.168.107.2] [-Prefix 24] [-Subnet 192.168.107.0/24]
param([Parameter(Mandatory)][string]$Keys, [string]$Mac = '02-00-00-00-00-47',
      [string]$Address = '192.168.107.2', [int]$Prefix = 24, [string]$Subnet = '192.168.107.0/24')
$ErrorActionPreference = 'Stop'
$cv = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
"OS: $($cv.ProductName) $($cv.DisplayVersion) build $($cv.CurrentBuild).$($cv.UBR)"

# 1. The OpenSSH server: the Windows capability (Features on Demand).
$cap = Get-WindowsCapability -Online -Name 'OpenSSH.Server*' | Select-Object -First 1
"capability $($cap.Name): $($cap.State)"
if ($cap.State -ne 'Installed') {
    Add-WindowsCapability -Online -Name $cap.Name | Out-String
    $cap = Get-WindowsCapability -Online -Name $cap.Name
    "capability $($cap.Name): $($cap.State)"
    if ($cap.State -ne 'Installed') { throw "OpenSSH.Server not installed ($($cap.State))" }
}
# The first start writes the host keys and the default sshd_config.
Set-Service sshd -StartupType Automatic
Start-Service sshd
Stop-Service sshd

# 2. Key authentication only. Members of Administrators read their keys from
#    administrators_authorized_keys (the default config's Match block), which only
#    Administrators and SYSTEM may hold (by SID, independent of the display language).
$pd = "$env:ProgramData\ssh"
$conf = Get-Content "$pd\sshd_config"
$set = @{ 'PubkeyAuthentication' = 'yes'; 'PasswordAuthentication' = 'no'; 'KbdInteractiveAuthentication' = 'no' }
foreach ($k in $set.Keys) {
    $conf = $conf | Where-Object { $_ -notmatch "^\s*#?\s*$k\s" }
}
# Global options go before the first Match block.
$m = ($conf | Select-String -Pattern '^\s*Match\s' | Select-Object -First 1).LineNumber
$head = if ($m -gt 1) { $conf[0..($m - 2)] } elseif ($m) { @() } else { $conf }
$tail = if ($m) { $conf[($m - 1)..($conf.Count - 1)] } else { @() }
$conf = @($head) + ($set.Keys | ForEach-Object { "$_ $($set[$_])" }) + @($tail)
Set-Content -Encoding ascii "$pd\sshd_config" $conf
$ak = "$pd\administrators_authorized_keys"
Get-Content $Keys | Where-Object { $_ -match '^ssh-' } | Set-Content -Encoding ascii $ak
icacls.exe $ak /inheritance:r /grant '*S-1-5-32-544:F' /grant '*S-1-5-18:F' | Out-String
"authorized keys: $((Get-Content $ak).Count)"
# PowerShell as the login shell (the run kits send PowerShell).
New-Item -Force 'HKLM:\SOFTWARE\OpenSSH' | Out-Null
New-ItemProperty -Force 'HKLM:\SOFTWARE\OpenSSH' DefaultShell -PropertyType String `
    -Value "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" | Out-Null
& "$env:SystemRoot\System32\OpenSSH\sshd.exe" -t
if ($LASTEXITCODE -ne 0) { throw "sshd -t refused the config ($LASTEXITCODE)" }
Start-Service sshd

# 3. Firewall: SSH and ping from the tap subnet only, in every network profile (the tap has
#    no gateway, so Windows files it as an unidentified public network).
foreach ($r in @(
        @{ Name = 'caraxes-sshd'; DisplayName = 'Caraxes sshd (tap)'; Protocol = 'TCP'; LocalPort = 22 },
        @{ Name = 'caraxes-ping'; DisplayName = 'Caraxes ping (tap)'; Protocol = 'ICMPv4'; IcmpType = 8 })) {
    Remove-NetFirewallRule -Name $r.Name -ErrorAction SilentlyContinue
    New-NetFirewallRule @r -Direction Inbound -Action Allow -Profile Any -RemoteAddress $Subnet | Out-Null
    "firewall rule $($r.Name): $Subnet"
}

# 4. The tap address at every boot, on the adapter with the tap MAC.
New-Item -Force -ItemType Directory C:\caraxes | Out-Null
@"
# Written by ssh-setup.ps1: the static tap address on the adapter with MAC $Mac.
`$log = 'C:\caraxes\net.log'
for (`$i = 0; `$i -lt 180; `$i++) {
    `$a = Get-NetAdapter -ErrorAction SilentlyContinue | Where-Object MacAddress -eq '$Mac'
    if (`$a) { break }
    Start-Sleep 1
}
if (-not `$a) { "`$(Get-Date -Format s) no adapter with MAC $Mac" | Add-Content `$log; exit 1 }
`$have = Get-NetIPAddress -InterfaceIndex `$a.ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue
if (-not (`$have | Where-Object IPAddress -eq '$Address')) {
    Set-NetIPInterface -InterfaceIndex `$a.ifIndex -Dhcp Disabled
    `$have | Remove-NetIPAddress -Confirm:`$false -ErrorAction SilentlyContinue
    New-NetIPAddress -InterfaceIndex `$a.ifIndex -IPAddress '$Address' -PrefixLength $Prefix | Out-Null
}
Set-NetConnectionProfile -InterfaceIndex `$a.ifIndex -NetworkCategory Private -ErrorAction SilentlyContinue
"`$(Get-Date -Format s) `$(`$a.Name) (`$(`$a.InterfaceDescription)) `$(`$a.MacAddress): $Address/$Prefix" | Add-Content `$log
"@ | Set-Content -Encoding ascii C:\caraxes\net.ps1
$act = New-ScheduledTaskAction -Execute powershell.exe -Argument '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File C:\caraxes\net.ps1'
$prin = New-ScheduledTaskPrincipal -UserId SYSTEM -LogonType ServiceAccount -RunLevel Highest
$sets = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Minutes 5) -AllowStartIfOnBatteries
Register-ScheduledTask -Force -TaskName caraxes-net -Action $act -Trigger (New-ScheduledTaskTrigger -AtStartup) `
    -Principal $prin -Settings $sets | Out-Null
"task caraxes-net registered (at startup, SYSTEM)"
& powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File C:\caraxes\net.ps1
if (Test-Path C:\caraxes\net.log) { Get-Content C:\caraxes\net.log -Tail 2 }
Get-Service sshd | Format-Table Name, Status, StartType -AutoSize | Out-String
Get-NetTCPConnection -LocalPort 22 -State Listen -ErrorAction SilentlyContinue |
    Format-Table LocalAddress, LocalPort -AutoSize | Out-String
