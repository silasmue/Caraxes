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
# Keep the writer open so a reader cannot interrupt logging between lines.
$logStream = [IO.FileStream]::new($log, [IO.FileMode]::Create,
    [IO.FileAccess]::Write, [IO.FileShare]::ReadWrite)
$logWriter = [IO.StreamWriter]::new($logStream, [Text.UTF8Encoding]::new($false))
$logWriter.AutoFlush = $true
$logWriter.WriteLine("[$(Get-Date -Format o)] starting KD bridge")

$si = [Diagnostics.ProcessStartInfo]::new()
$si.FileName = $kd
$si.Arguments = '-bonc -k com:port=com1,baud=115200 -cf C:\Temp\nv-start-failures.kds'
$si.UseShellExecute = $false
# Inherit the console so @BREAK reaches KD.
$si.CreateNoWindow = $false
$si.RedirectStandardInput = $true
$si.RedirectStandardOutput = $true
$si.RedirectStandardError = $true
$p = [Diagnostics.Process]::new()
$p.StartInfo = $si
$started = $false
try {
    $started = $p.Start()
    if (-not $started) { throw 'failed to start kd.exe' }
    Set-Content $state "running kd_pid=$($p.Id) started=$(Get-Date -Format o)"
    $stdout = $p.StandardOutput.ReadLineAsync()
    $stderr = $p.StandardError.ReadLineAsync()
    while (-not $p.HasExited) {
        # Drain bursts without delaying command processing indefinitely.
        for ($n = 0; $n -lt 256 -and $null -ne $stdout -and $stdout.IsCompleted; $n++) {
            $line = $stdout.GetAwaiter().GetResult()
            if ($null -eq $line) { $stdout = $null; break }
            $logWriter.WriteLine($line)
            $stdout = $p.StandardOutput.ReadLineAsync()
        }
        for ($n = 0; $n -lt 256 -and $null -ne $stderr -and $stderr.IsCompleted; $n++) {
            $line = $stderr.GetAwaiter().GetResult()
            if ($null -eq $line) { $stderr = $null; break }
            $logWriter.WriteLine("STDERR: $line")
            $stderr = $p.StandardError.ReadLineAsync()
        }
        foreach ($file in Get-ChildItem $inbox -Filter '*.cmd' | Sort-Object Name) {
            foreach ($line in Get-Content $file.FullName) {
                $logWriter.WriteLine("COMMAND[$($file.Name)]: $line")
                if ($line -eq '@BREAK') {
                    [KdConsoleControl]::Break()
                    $logWriter.WriteLine("CONTROL[$($file.Name)]: CTRL_BREAK_EVENT sent")
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
} catch {
    Set-Content $state "failed time=$(Get-Date -Format o) error=$($_.Exception.Message)"
    throw
} finally {
    if ($started -and -not $p.HasExited) { $p.Kill() }
    $logWriter.Dispose()
}
