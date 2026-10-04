# SPDX-License-Identifier: MIT
# Report the single present NVIDIA RTX 2070; optionally change its Windows display name.
# PowerShell 5.1 / C# 5. No driver restart, INF change, or hardware-ID change.
[CmdletBinding()]
param(
    [switch]$Apply,
    [switch]$Restore,
    [switch]$DriverDescription,
    [string]$BackupPath = "$env:ProgramData\Leandro\display-name-backup.json"
)
$ErrorActionPreference = 'Stop'
$targetName = 'Leandro VFIO RTX 2070'
$maxBackupBytes = 32768
if ($Apply -and $Restore) { throw 'Choose either -Apply or -Restore.' }
if ($DriverDescription -and -not $Apply) { throw '-DriverDescription requires -Apply.' }

if (-not ('LeandroDisplayName' -as [type])) {
    Add-Type @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
public static class LeandroDisplayName {
    [StructLayout(LayoutKind.Sequential)] struct DeviceInfo {
        public uint Size; public Guid ClassGuid; public uint DevInst; public IntPtr Reserved;
    }
    [StructLayout(LayoutKind.Sequential)] struct PropertyKey {
        public Guid Format; public uint Id;
    }
    public class NameProperty {
        public bool Present; public uint Type; public string Value; public string RawBase64;
    }
    static Guid DisplayClass = new Guid("4d36e968-e325-11ce-bfc1-08002be10318");
    static PropertyKey FriendlyName = new PropertyKey {
        Format = new Guid("a45c254e-df1c-4efd-8020-67d146a850e0"), Id = 14
    };
    [DllImport("setupapi.dll", SetLastError = true)]
    static extern IntPtr SetupDiCreateDeviceInfoList(ref Guid guid, IntPtr parent);
    [DllImport("setupapi.dll", CharSet = CharSet.Unicode, ExactSpelling = true, SetLastError = true)]
    static extern bool SetupDiOpenDeviceInfoW(IntPtr set, string instance, IntPtr parent,
        uint flags, ref DeviceInfo info);
    [DllImport("setupapi.dll", SetLastError = true)]
    static extern bool SetupDiDestroyDeviceInfoList(IntPtr set);
    [DllImport("setupapi.dll", CharSet = CharSet.Unicode, ExactSpelling = true, SetLastError = true)]
    static extern bool SetupDiGetDevicePropertyW(IntPtr set, ref DeviceInfo info,
        ref PropertyKey key, out uint type, [Out] byte[] buffer, uint size,
        out uint required, uint flags);
    [DllImport("setupapi.dll", CharSet = CharSet.Unicode, ExactSpelling = true, SetLastError = true)]
    static extern bool SetupDiSetDevicePropertyW(IntPtr set, ref DeviceInfo info,
        ref PropertyKey key, uint type, byte[] buffer, uint size, uint flags);
    static IntPtr Open(string instance, out DeviceInfo info) {
        info = new DeviceInfo(); info.Size = (uint)Marshal.SizeOf(typeof(DeviceInfo));
        IntPtr set = SetupDiCreateDeviceInfoList(ref DisplayClass, IntPtr.Zero);
        if (set == new IntPtr(-1)) throw new Win32Exception(Marshal.GetLastWin32Error());
        if (!SetupDiOpenDeviceInfoW(set, instance, IntPtr.Zero, 0, ref info)) {
            int error = Marshal.GetLastWin32Error(); SetupDiDestroyDeviceInfoList(set);
            throw new Win32Exception(error);
        }
        return set;
    }
    public static NameProperty Read(string instance) {
        DeviceInfo info; IntPtr set = Open(instance, out info);
        try {
            uint type, size;
            bool ok = SetupDiGetDevicePropertyW(set, ref info, ref FriendlyName,
                out type, null, 0, out size, 0);
            int error = Marshal.GetLastWin32Error();
            if (!ok && error == 1168) return new NameProperty { Present = false, Type = 0 };
            if (!ok && error != 122) throw new Win32Exception(error);
            if (type != 0x12 || size < 2 || size > 4096 || size % 2 != 0)
                throw new InvalidOperationException("Unexpected FriendlyName type or size.");
            byte[] bytes = new byte[size];
            if (!SetupDiGetDevicePropertyW(set, ref info, ref FriendlyName,
                out type, bytes, size, out size, 0)) throw new Win32Exception(Marshal.GetLastWin32Error());
            if (type != 0x12 || size != bytes.Length)
                throw new InvalidOperationException("FriendlyName changed while reading it.");
            return new NameProperty { Present = true, Type = type,
                Value = Encoding.Unicode.GetString(bytes).TrimEnd('\0'),
                RawBase64 = Convert.ToBase64String(bytes) };
        } finally { SetupDiDestroyDeviceInfoList(set); }
    }
    public static void Write(string instance, uint type, byte[] bytes) {
        DeviceInfo info; IntPtr set = Open(instance, out info);
        try {
            if (!SetupDiSetDevicePropertyW(set, ref info, ref FriendlyName, type,
                bytes, bytes == null ? 0 : (uint)bytes.Length, 0)) {
                int error = Marshal.GetLastWin32Error();
                if (type != 0 || bytes != null || error != 1168) throw new Win32Exception(error);
            }
        } finally { SetupDiDestroyDeviceInfoList(set); }
    }
}
'@
}

$devices = @(Get-PnpDevice -Class Display -PresentOnly | Where-Object {
    $_.InstanceId -match '^PCI\\VEN_10DE&DEV_1F02(?:&|\\)'
})
if ($devices.Count -ne 1) { throw "Expected one present PCI VEN_10DE DEV_1F02 Display adapter; found $($devices.Count)." }
$instance = [string]$devices[0].InstanceId
$driverKey = [string](Get-PnpDeviceProperty -InstanceId $instance -KeyName 'DEVPKEY_Device_Driver').Data
if ($driverKey -notmatch '^\{4d36e968-e325-11ce-bfc1-08002be10318\}\\[0-9]{4}$') {
    throw "Unexpected display adapter software key: $driverKey"
}
$softwarePath = "SYSTEM\CurrentControlSet\Control\Class\$driverKey"
$BackupPath = [IO.Path]::GetFullPath($BackupPath)

function Open-SoftwareKey([bool]$writable) {
    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey(
        [Microsoft.Win32.RegistryHive]::LocalMachine, [Microsoft.Win32.RegistryView]::Registry64)
    try { $key = $base.OpenSubKey($softwarePath, $writable) } finally { $base.Dispose() }
    if ($null -eq $key) { throw "Adapter software key is missing: $softwarePath" }
    return $key
}
function Read-DriverDescription($key) {
    $present = @($key.GetValueNames()) -contains 'DriverDesc'
    $kind = $null; $value = $null
    if ($present) {
        $kind = [string]$key.GetValueKind('DriverDesc')
        $value = $key.GetValue('DriverDesc', $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
    }
    return [pscustomobject]@{ Present = $present; Kind = $kind; Value = $value }
}
function Read-State {
    $key = Open-SoftwareKey $false
    try {
        $driver = Read-DriverDescription $key
        $adapter = $key.GetValue('HardwareInformation.AdapterString')
        if ($adapter -is [byte[]]) { $adapter = [Text.Encoding]::Unicode.GetString($adapter).TrimEnd([char]0) }
    } finally { $key.Dispose() }
    $video = @(Get-CimInstance Win32_VideoController | Where-Object { $_.PNPDeviceID -eq $instance })
    return [pscustomobject]@{
        FriendlyName = [LeandroDisplayName]::Read($instance); DriverDesc = $driver
        AdapterString = $adapter
        VideoController = @($video | Select-Object -Property @('Name', 'VideoProcessor'))
    }
}
function Show-State([string]$label, $state) {
    $friendly = $state.FriendlyName
    "$label FriendlyName: present=$($friendly.Present) type=$($friendly.Type) value='$($friendly.Value)'"
    "$label DriverDesc: present=$($state.DriverDesc.Present) kind=$($state.DriverDesc.Kind) value='$($state.DriverDesc.Value)'"
    "$label AdapterString: '$($state.AdapterString)'"
    foreach ($video in $state.VideoController) {
        "$label Win32_VideoController: Name='$($video.Name)' VideoProcessor='$($video.VideoProcessor)'"
    }
    if ($state.VideoController.Count -eq 0) { "$label Win32_VideoController: no matching instance" }
}
function Validate-Backup($backup) {
    if ($backup.SchemaVersion -ne 1 -or $backup.InstanceId -cne $instance) {
        throw 'Backup schema or device instance does not match; refusing mutation.'
    }
    if ($backup.DriverKey -cne $driverKey) { throw 'Backup software key does not match; refusing mutation.' }
    $name = $backup.FriendlyName
    if ($name.Present -isnot [bool] -or $backup.DriverDescriptionTracked -isnot [bool]) { throw 'Invalid backup presence flags.' }
    if ($name.Present) {
        $bytes = [Convert]::FromBase64String($name.RawBase64)
        if ($name.Type -ne 18 -or $bytes.Length -lt 2 -or $bytes.Length -gt 4096 -or $bytes.Length % 2 -ne 0 -or
            [Text.Encoding]::Unicode.GetString($bytes).TrimEnd([char]0) -cne $name.Value) { throw 'Invalid FriendlyName backup.' }
    } elseif ($name.Type -ne 0 -or $null -ne $name.Value -or $null -ne $name.RawBase64) {
        throw 'Invalid absent FriendlyName backup.'
    }
    $driver = $backup.DriverDesc
    if ($driver.Present -isnot [bool]) { throw 'Invalid DriverDesc presence flag.' }
    if ($driver.Present -and ($driver.Kind -notin @('String', 'ExpandString') -or
        $driver.Value -isnot [string] -or $driver.Value.Length -gt 2048)) { throw 'Unexpected DriverDesc backup type or size.' }
    if (-not $driver.Present -and ($null -ne $driver.Kind -or $null -ne $driver.Value)) { throw 'Invalid absent DriverDesc backup.' }
}
function Load-Backup {
    $file = Get-Item -LiteralPath $BackupPath
    if ($file.PSIsContainer -or $file.Length -gt $maxBackupBytes) { throw 'Backup is not a bounded JSON file.' }
    $backup = [IO.File]::ReadAllText($BackupPath) | ConvertFrom-Json
    Validate-Backup $backup
    return $backup
}
function Save-Backup($backup, [bool]$replaceExisting) {
    Validate-Backup $backup
    $bytes = [Text.Encoding]::UTF8.GetBytes(($backup | ConvertTo-Json -Depth 6))
    if ($bytes.Length -gt $maxBackupBytes) { throw 'Backup exceeds 32 KiB.' }
    $directory = [IO.Path]::GetDirectoryName($BackupPath)
    [IO.Directory]::CreateDirectory($directory) | Out-Null
    $temporary = "$BackupPath.$([Guid]::NewGuid().ToString('N')).tmp"
    try {
        $stream = [IO.File]::Open($temporary, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        try { $stream.Write($bytes, 0, $bytes.Length); $stream.Flush($true) } finally { $stream.Dispose() }
        if ($replaceExisting) { [IO.File]::Replace($temporary, $BackupPath, $null) }
        else { [IO.File]::Move($temporary, $BackupPath) }
    } finally { if ([IO.File]::Exists($temporary)) { [IO.File]::Delete($temporary) } }
}

"Adapter instance: $instance"
"Adapter software key: HKLM\$softwarePath"
$before = Read-State
Show-State 'Original' $before
if (-not ($Apply -or $Restore)) {
    "REPORT ONLY: target '$targetName'; backup '$BackupPath'"
    return
}
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Apply and Restore require elevation.' }

if ($Apply) {
    $backupExists = Test-Path -LiteralPath $BackupPath
    if ($backupExists) { $backup = Load-Backup }
    else {
        $backup = [pscustomobject]@{
            SchemaVersion = 1; InstanceId = $instance; DriverKey = $driverKey
            CreatedUtc = [DateTime]::UtcNow.ToString('o')
            FriendlyName = $before.FriendlyName; DriverDesc = $before.DriverDesc
            DriverDescriptionTracked = [bool]$DriverDescription
        }
    }
    if ($DriverDescription -and -not $backup.DriverDescriptionTracked) {
        $backup.DriverDesc = $before.DriverDesc
        $backup.DriverDescriptionTracked = $true
    }
    Save-Backup $backup $backupExists
    "Backup saved before mutation: $BackupPath"
    if (-not $before.FriendlyName.Present -or $before.FriendlyName.Value -cne $targetName) {
        [LeandroDisplayName]::Write($instance, 18, [Text.Encoding]::Unicode.GetBytes($targetName + [char]0))
    }
    if ($DriverDescription) {
        $key = Open-SoftwareKey $true
        try { $key.SetValue('DriverDesc', $targetName, [Microsoft.Win32.RegistryValueKind]::String) }
        finally { $key.Dispose() }
    }
} else {
    $backup = Load-Backup
    $saved = $backup.FriendlyName
    if ($before.FriendlyName.Present -ne $saved.Present -or $before.FriendlyName.RawBase64 -cne $saved.RawBase64) {
        if ($saved.Present) { [LeandroDisplayName]::Write($instance, [uint32]$saved.Type, [Convert]::FromBase64String($saved.RawBase64)) }
        else { [LeandroDisplayName]::Write($instance, 0, $null) }
    }
    if ($backup.DriverDescriptionTracked) {
        $key = Open-SoftwareKey $true
        try {
            if ($backup.DriverDesc.Present) {
                $kind = [Microsoft.Win32.RegistryValueKind]$backup.DriverDesc.Kind
                $key.SetValue('DriverDesc', [string]$backup.DriverDesc.Value, $kind)
            } else { $key.DeleteValue('DriverDesc', $false) }
        } finally { $key.Dispose() }
    }
}
$after = Read-State
Show-State 'Current' $after
if ($Apply) {
    if (-not $after.FriendlyName.Present -or $after.FriendlyName.Type -ne 18 -or
        $after.FriendlyName.Value -cne $targetName) { throw 'FriendlyName readback did not match.' }
    if ($DriverDescription -and (-not $after.DriverDesc.Present -or $after.DriverDesc.Kind -ne 'String' -or
        $after.DriverDesc.Value -cne $targetName)) { throw 'DriverDesc readback did not match.' }
    'PASS: FriendlyName applied; DriverDesc applied only with -DriverDescription.'
} else {
    if ($after.FriendlyName.Present -ne $backup.FriendlyName.Present -or
        $after.FriendlyName.Type -ne $backup.FriendlyName.Type -or
        $after.FriendlyName.RawBase64 -cne $backup.FriendlyName.RawBase64) { throw 'FriendlyName restore readback did not match.' }
    if ($backup.DriverDescriptionTracked -and ($after.DriverDesc.Present -ne $backup.DriverDesc.Present -or
        $after.DriverDesc.Kind -cne $backup.DriverDesc.Kind -or
        $after.DriverDesc.Value -cne $backup.DriverDesc.Value)) { throw 'DriverDesc restore readback did not match.' }
    'PASS: original properties restored; backup retained for repeat restores.'
}
'Reopen Device Manager and Task Manager to inspect the name. Task Manager/DXGI propagation is unverified.'
