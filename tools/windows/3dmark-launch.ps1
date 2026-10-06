# SPDX-License-Identifier: MIT
# Run in the signed-in user's interactive PowerShell desktop, not through SYSTEM.
param(
    [ValidateRange(30, 180)]
    [int]$ReadySeconds = 120
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$self = [Diagnostics.Process]::GetCurrentProcess()
try { $session = $self.SessionId } finally { $self.Dispose() }
if ($identity.IsSystem -or $session -eq 0 -or -not [Environment]::UserInteractive) {
    throw '3DMark launch requires a signed-in interactive desktop; SYSTEM/session 0 is refused'
}

$steam = Join-Path ${env:ProgramFiles(x86)} 'Steam\steam.exe'
if (-not (Test-Path -LiteralPath $steam -PathType Leaf)) {
    throw "Steam executable is missing: $steam"
}

function Get-Session3DMark {
    @(Get-Process -Name 3DMark -ErrorAction SilentlyContinue | Where-Object {
        try { $_.SessionId -eq $session } catch { $false }
    })
}

function Get-SteamErrorWindow {
    @(Get-Process -ErrorAction SilentlyContinue | Where-Object {
        try {
            $_.SessionId -eq $session -and $_.MainWindowTitle -ceq 'Steam error' -and
            $_.MainWindowHandle -ne [IntPtr]::Zero
        } catch { $false }
    })
}

$clock = [Diagnostics.Stopwatch]::StartNew()
$existing = @(Get-Session3DMark)
$launched = $false
if ($existing.Count -eq 0) {
    if (@(Get-SteamErrorWindow).Count -gt 0) {
        throw 'Steam error window is already open; launch refused'
    }
    Start-Process -FilePath $steam -ArgumentList @('-applaunch', '223850') | Out-Null
    $launched = $true
}

while ($clock.Elapsed.TotalSeconds -lt $ReadySeconds) {
    $errors = @(Get-SteamErrorWindow)
    if ($errors.Count -gt 0) {
        throw "Steam error window appeared in session $session (PID $($errors[0].Id)); no window was closed"
    }
    $candidates = @(Get-Session3DMark)
    foreach ($process in $candidates) {
        try {
            $process.Refresh()
            if ($process.MainWindowHandle -ne [IntPtr]::Zero -and
                $process.MainWindowTitle -ceq '3DMark') {
                $utc = [DateTime]::UtcNow.ToString('o')
                'READY pid={0} title=3DMark elapsed_s={1:N3} utc={2}' -f `
                    $process.Id, $clock.Elapsed.TotalSeconds, $utc
                return
            }
        } catch { } # A process may exit between enumeration and window inspection.
    }
    Start-Sleep -Milliseconds 500
}

$pids = (@(Get-Session3DMark) | ForEach-Object { $_.Id }) -join ','
throw "3DMark window was not ready within $ReadySeconds seconds (launched=$launched; session=$session; observed_pids=$pids)"
