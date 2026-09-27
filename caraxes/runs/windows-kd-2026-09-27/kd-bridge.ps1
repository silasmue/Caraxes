# SPDX-License-Identifier: MIT
$ErrorActionPreference = 'Stop'
$root = 'C:\Temp'
$inbox = Join-Path $root 'kd-cmd'
$sent = Join-Path $root 'kd-cmd-sent'
$log = Join-Path $root 'kd-console.log'
$state = Join-Path $root 'kd-bridge.state'
$kd = 'C:\Program Files (x86)\Windows Kits\10\Debuggers\x64\kd.exe'

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class KdConsoleControl {
    private delegate bool HandlerRoutine(uint controlType);
    private static HandlerRoutine handler = delegate(uint controlType) { return true; };
    [DllImport("kernel32.dll", SetLastError=true)] private static extern bool AllocConsole();
    [DllImport("kernel32.dll", SetLastError=true)] private static extern bool SetConsoleCtrlHandler(HandlerRoutine callback, bool add);
    [DllImport("kernel32.dll", SetLastError=true)] private static extern bool GenerateConsoleCtrlEvent(uint controlEvent, uint processGroupId);
    public static void Initialize() {
        if (!AllocConsole() && Marshal.GetLastWin32Error() != 5) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        if (!SetConsoleCtrlHandler(handler, true)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    }
    public static void Break() {
        const uint CTRL_BREAK_EVENT = 1;
        if (!GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, 0)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    }
}
'@
[KdConsoleControl]::Initialize()

New-Item -ItemType Directory -Force $inbox, $sent | Out-Null
Set-Content $log "[$(Get-Date -Format o)] starting KD bridge"

$si = [Diagnostics.ProcessStartInfo]::new()
$si.FileName = $kd
$si.Arguments = '-bonc -k com:port=com1,baud=115200 -cf C:\Temp\nv-start-failures.kds'
$si.UseShellExecute = $false
# KD must inherit the private console allocated above. GenerateConsoleCtrlEvent
# addresses console process groups; CREATE_NO_WINDOW would detach KD from that
# console and make @BREAK a no-op.
$si.CreateNoWindow = $false
$si.RedirectStandardInput = $true
$si.RedirectStandardOutput = $true
$si.RedirectStandardError = $true
$p = [Diagnostics.Process]::new()
$p.StartInfo = $si
if (-not $p.Start()) { throw 'failed to start kd.exe' }
Set-Content $state "running kd_pid=$($p.Id) started=$(Get-Date -Format o)"

$stdout = $p.StandardOutput.ReadLineAsync()
$stderr = $p.StandardError.ReadLineAsync()
try {
    while (-not $p.HasExited) {
        if ($stdout.IsCompleted) {
            $line = $stdout.GetAwaiter().GetResult()
            if ($null -ne $line) { Add-Content $log $line }
            $stdout = $p.StandardOutput.ReadLineAsync()
        }
        if ($stderr.IsCompleted) {
            $line = $stderr.GetAwaiter().GetResult()
            if ($null -ne $line) { Add-Content $log "STDERR: $line" }
            $stderr = $p.StandardError.ReadLineAsync()
        }
        foreach ($file in Get-ChildItem $inbox -Filter '*.cmd' | Sort-Object Name) {
            foreach ($line in Get-Content $file.FullName) {
                Add-Content $log "COMMAND[$($file.Name)]: $line"
                if ($line -eq '@BREAK') {
                    [KdConsoleControl]::Break()
                    Add-Content $log "CONTROL[$($file.Name)]: CTRL_BREAK_EVENT sent"
                } else {
                    $p.StandardInput.WriteLine($line)
                }
            }
            $p.StandardInput.Flush()
            Move-Item $file.FullName (Join-Path $sent $file.Name)
        }
        Start-Sleep -Milliseconds 100
    }
    Set-Content $state "exited code=$($p.ExitCode) time=$(Get-Date -Format o)"
} finally {
    if (-not $p.HasExited) { $p.Kill() }
}
