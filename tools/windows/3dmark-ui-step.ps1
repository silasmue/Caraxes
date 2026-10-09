# SPDX-License-Identifier: MIT
# Run in the signed-in user's interactive desktop. Coordinates are screen pixels.
# ExpectedPid can bind every step to the PID reported by 3dmark-launch.ps1.
param(
    [ValidateSet('snapshot', 'click', 'wheel')][string]$Action = 'snapshot',
    [int]$X = 0, [int]$Y = 0, [int]$Delta = 0,
    [ValidateRange(0, 10)][int]$WaitSeconds = 3,
    [ValidateRange(1, 2147483647)][int]$ExpectedPid = 0
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$self = [Diagnostics.Process]::GetCurrentProcess()
try { $session = $self.SessionId } finally { $self.Dispose() }
if ($identity.IsSystem -or $session -eq 0 -or -not [Environment]::UserInteractive) {
    throw '3DMark UI requires a signed-in interactive desktop; SYSTEM/session 0 is refused'
}

Add-Type -AssemblyName @('System.Drawing', 'System.Windows.Forms')
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class Caraxes3DMarkUi {
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, int data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    private static extern int GetWindowTextW(IntPtr h, StringBuilder text, int count);
    public static uint Owner(IntPtr h) { uint pid; GetWindowThreadProcessId(h, out pid); return pid; }
    public static string Title(IntPtr h) {
        var text = new StringBuilder(256); GetWindowTextW(h, text, text.Capacity); return text.ToString();
    }
}
'@
$shell = New-Object -ComObject WScript.Shell
$outputDir = 'C:\Users\Public\caraxes'
New-Item -ItemType Directory -Force -Path $outputDir | Out-Null

function Get-3DMarkWindows {
    foreach ($process in @(Get-Process -Name 3DMark -ErrorAction SilentlyContinue)) {
        try {
            # MainWindowHandle is cached: refresh on every bounded acquisition pass.
            $process.Refresh()
            [PSCustomObject]@{
                ProcessId = $process.Id; Session = $process.SessionId
                StartedTicks = $process.StartTime.ToUniversalTime().Ticks
                Window = $process.MainWindowHandle; Title = $process.MainWindowTitle
            }
        } catch { } # Exit between enumeration and inspection is retried, never selected.
        finally { $process.Dispose() }
    }
}

function Test-OwnedWindow($target, [switch]$Foreground) {
    $process = $null
    $script:OwnedWindowRejection = ''
    try {
        if ($ExpectedPid -gt 0 -and $target.ProcessId -ne $ExpectedPid) {
            $script:OwnedWindowRejection = 'Expected PID mismatch'; return $false
        }
        $process = Get-Process -Id $target.ProcessId -ErrorAction Stop
        $process.Refresh()
        if ($process.ProcessName -ine '3DMark' -or $process.SessionId -ne $session -or
            $process.StartTime.ToUniversalTime().Ticks -ne $target.StartedTicks) {
            $script:OwnedWindowRejection = 'Process name, session or start time changed'; return $false
        }
        if (-not [Caraxes3DMarkUi]::IsWindow($target.Window)) {
            $script:OwnedWindowRejection = 'Window no longer exists'; return $false
        }
        if ([Caraxes3DMarkUi]::Owner($target.Window) -ne $target.ProcessId) {
            $script:OwnedWindowRejection = 'Window owner changed'; return $false
        }
        # A PID-bound loading window can change its caption. Input needs readiness below.
        if ($ExpectedPid -eq 0 -and [Caraxes3DMarkUi]::Title($target.Window) -cne '3DMark') {
            $script:OwnedWindowRejection = 'Unbound window title changed'; return $false
        }
        if ($Foreground -and [Caraxes3DMarkUi]::GetForegroundWindow() -ne $target.Window) {
            $script:OwnedWindowRejection = 'Foreground window changed'; return $false
        }
        return $true
    } catch {
        $script:OwnedWindowRejection = 'Process inspection failed: ' + $_.Exception.GetType().Name
        return $false
    }
    finally { if ($null -ne $process) { $process.Dispose() } }
}

function Test-InputReadyWindow($target) {
    if (-not (Test-OwnedWindow $target -Foreground)) { return $false }
    $title = [Caraxes3DMarkUi]::Title($target.Window)
    if ($title -cne '3DMark') {
        $caption = $title -replace '[\r\n]', ' '
        $script:OwnedWindowRejection = 'Input title not ready: ' + $caption.Substring(0, [Math]::Min(80, $caption.Length))
        return $false
    }
    return $true
}

function Save-UiScreenshot([string]$name) {
    $screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = New-Object System.Drawing.Bitmap $screen.Width, $screen.Height
    $graphics = $null
    $path = Join-Path $outputDir $name
    try {
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $graphics.CopyFromScreen($screen.Left, $screen.Top, 0, 0, $screen.Size)
        $captured = [DateTime]::UtcNow.ToString('o')
        $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        if ($null -ne $graphics) { $graphics.Dispose() }
        $bitmap.Dispose()
    }
    $file = Get-Item -LiteralPath $path
    if ($file.Length -le 0) { throw 'Screenshot capture produced an empty file' }
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    '3DMARK_UI_SCREENSHOT path={0} sha256={1} bytes={2} size={3}x{4} utc={5}' -f `
        $path, $hash, $file.Length, $screen.Width, $screen.Height, $captured
}

function Stop-UiStep([string]$reason, $seen) {
    # Diagnostic text is bounded; the failure image is a fresh screen capture.
    $seen = @(Get-3DMarkWindows)
    $initial = 'unselected'
    if ($null -ne $target) {
        $initial = 'pid={0}/hwnd={1}/started_ticks={2}' -f $target.ProcessId, $target.Window, $target.StartedTicks
    }
    $observations = (@($seen | Select-Object -First 8) | ForEach-Object {
        $title = ($_.Title -replace '[\r\n]', ' ')
        if ($title.Length -gt 120) { $title = $title.Substring(0, 120) }
        'pid={0}/session={1}/hwnd={2}/title="{3}"' -f $_.ProcessId, $_.Session, $_.Window, $title
    }) -join '; '
    $foreground = [Caraxes3DMarkUi]::GetForegroundWindow()
    try { Save-UiScreenshot '3dmark-ui-step-failure.png' }
    catch { Write-Output ('3DMARK_UI_FAILURE_SCREENSHOT unavailable: ' + $_.Exception.Message) }
    throw "$reason (rejection=$script:OwnedWindowRejection; initial=$initial; session=$session; expected_pid=$ExpectedPid; foreground=$foreground; observed=[$observations])"
}

# One ten-second budget covers window acquisition and foreground activation.
# A selected allocation is pinned by PID/start time; never switch to another process.
$clock = [Diagnostics.Stopwatch]::StartNew()
$selected = $null
$target = $null
$seen = @()
$script:OwnedWindowRejection = ''
while ($clock.Elapsed.TotalSeconds -lt 10) {
    $seen = @(Get-3DMarkWindows)
    $candidates = @($seen | Where-Object {
        $_.Session -eq $session -and $_.Window -ne [IntPtr]::Zero -and
        ($ExpectedPid -gt 0 -or $_.Title -ceq '3DMark') -and
        ($ExpectedPid -eq 0 -or $_.ProcessId -eq $ExpectedPid) -and
        ($null -eq $selected -or ($_.ProcessId -eq $selected.ProcessId -and $_.StartedTicks -eq $selected.StartedTicks))
    })
    if ($candidates.Count -gt 1) { Stop-UiStep 'Ambiguous owned 3DMark windows; no action taken' $seen }
    if ($candidates.Count -eq 1) {
        $candidate = $candidates[0]
        if ($null -eq $selected) { $selected = $candidate }
        if (Test-OwnedWindow $candidate) {
            $null = [Caraxes3DMarkUi]::ShowWindow($candidate.Window, 9)
            $null = $shell.AppActivate($candidate.ProcessId)
            $null = [Caraxes3DMarkUi]::SetForegroundWindow($candidate.Window)
            if ((Test-OwnedWindow $candidate -Foreground) -and $clock.Elapsed.TotalSeconds -lt 10) {
                $target = $candidate; break
            }
        }
    }
    $remaining = 10000 - $clock.Elapsed.TotalMilliseconds
    if ($remaining -gt 0) { Start-Sleep -Milliseconds ([int][Math]::Min(200, [Math]::Ceiling($remaining))) }
}
if ($null -eq $target) { Stop-UiStep '3DMark UI window/foreground unavailable within 10 seconds; no action taken' $seen }

if (-not (Test-OwnedWindow $target -Foreground)) { Stop-UiStep 'Owned 3DMark window changed before action; no action taken' $seen }
if ($Action -ne 'snapshot') {
    if (-not (Test-InputReadyWindow $target)) { Stop-UiStep '3DMark page title not ready for input; no action taken' $seen }
    if (-not [Caraxes3DMarkUi]::SetCursorPos($X, $Y)) { Stop-UiStep 'Could not position the cursor; no action taken' $seen }
    if (-not (Test-InputReadyWindow $target)) { Stop-UiStep 'Owned 3DMark window or page title changed before input; no action taken' $seen }
    if ($Action -eq 'click') {
        [Caraxes3DMarkUi]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
        [Caraxes3DMarkUi]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
    } else { [Caraxes3DMarkUi]::mouse_event(0x800, 0, 0, $Delta, [UIntPtr]::Zero) }
}
Start-Sleep -Seconds $WaitSeconds
if (-not (Test-OwnedWindow $target -Foreground)) { Stop-UiStep 'Owned 3DMark window changed before screenshot' $seen }
Save-UiScreenshot '3dmark-ui-step.png'
"3DMARK_UI_STEP $Action at $X,$Y delta=$Delta pid=$($target.ProcessId) session=$session hwnd=$($target.Window)"
