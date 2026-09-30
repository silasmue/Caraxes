# SPDX-License-Identifier: MIT
# Showcase screenshots of the stock driver on the device (docs/vfio/showcase.md): Device Manager
# with the display adapters expanded, the GPU's driver tab, Task Manager's GPU page, dxdiag's
# display tab and, with -Only nvcpl, the NVIDIA Control Panel's System Information (on the 617.14
# guest of 2026-09-30 the Control Panel exits at start without a window). Each image is the
# window's visible frame (DWM extended frame bounds), taken from the screen after the other
# windows are minimised. English and German user interfaces.
# Run it elevated in the signed-in user's session (caraxes/runs/win-2026-09-30/showcase-hi.sh):
# mmc auto-elevates, and UI Automation from a limited token (run44.py upack) cannot read it.
# Usage: showcase.ps1 [-Only devmgr,props,taskmgr,dxdiag,nvcpl]   ->  C:\Users\Public\caraxes\showcase
param([string]$Only = "devmgr,props,taskmgr,dxdiag")
$ErrorActionPreference = "Stop"
# ASCII only: Windows PowerShell 5.1 reads a script without a BOM as ANSI.
$out = "C:\Users\Public\caraxes\showcase"
New-Item -ItemType Directory -Force $out | Out-Null
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Drawing, System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class W {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int hh, bool repaint);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(int flags, int dx, int dy, int data, IntPtr extra);
    public static void Click(int x, int y) { SetCursorPos(x, y); mouse_event(2, 0, 0, 0, IntPtr.Zero); mouse_event(4, 0, 0, 0, IntPtr.Zero); }
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int size);
}
'@
[W]::SetProcessDPIAware() | Out-Null
$A = [System.Windows.Automation.AutomationElement]
$T = [System.Windows.Automation.TreeScope]

function say($m) { "{0:HH:mm:ss} {1}" -f (Get-Date), $m }

# Names are lists: the English and the German user interface.
function Match([string]$n, [string[]]$names, [bool]$like) {
    foreach ($x in $names) { if (($like -and $n -like $x) -or ($n -eq $x)) { return $true } }
    return $false
}

function Wait-Window([string[]]$name, [int]$seconds = 30, [switch]$Like, [string]$Class = "") {
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) {
        foreach ($w in $A::RootElement.FindAll($T::Children, [System.Windows.Automation.Condition]::TrueCondition)) {
            if ($Class -and $w.Current.ClassName -ne $Class) { continue }
            if (Match $w.Current.Name $name $Like) { return $w }
        }
        Start-Sleep -Milliseconds 500
    }
    throw "no window '$name' after $seconds s"
}

function Find([System.Windows.Automation.AutomationElement]$root, [string[]]$name, [int]$seconds = 20, [switch]$Like, [string]$Type = "") {
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) {
        foreach ($e in $root.FindAll($T::Descendants, [System.Windows.Automation.Condition]::TrueCondition)) {
            if ($Type -and $e.Current.ControlType.ProgrammaticName -ne "ControlType.$Type") { continue }
            if (Match $e.Current.Name $name $Like) { return $e }
        }
        Start-Sleep -Milliseconds 500
    }
    $seen = ($root.FindAll($T::Descendants, [System.Windows.Automation.Condition]::TrueCondition) | Select-Object -First 80 |
        ForEach-Object { $_.Current.ControlType.ProgrammaticName.Replace("ControlType.", "") + ":" + $_.Current.Name }) -join " | "
    throw "no element '$name' after $seconds s; seen: $seen"
}

function Act($e) {
    foreach ($pat in [System.Windows.Automation.InvokePattern]::Pattern, [System.Windows.Automation.SelectionItemPattern]::Pattern,
                     [System.Windows.Automation.ExpandCollapsePattern]::Pattern) {
        $o = $null
        if ($e.TryGetCurrentPattern($pat, [ref]$o)) {
            if ($o -is [System.Windows.Automation.InvokePattern]) { $o.Invoke() }
            elseif ($o -is [System.Windows.Automation.SelectionItemPattern]) { $o.Select() }
            else { $o.Expand() }
            return
        }
    }
    throw "no pattern to act on '$($e.Current.Name)'"
}

function Frame($w) {
    $h = [IntPtr]$w.Current.NativeWindowHandle
    $r = New-Object W+RECT
    [W]::DwmGetWindowAttribute($h, 9, [ref]$r, 16) | Out-Null
    return $r
}

function Shot($w, [string]$file, [int]$width = 0, [int]$height = 0) {
    $h = [IntPtr]$w.Current.NativeWindowHandle
    [W]::ShowWindow($h, 9) | Out-Null
    if ($width -gt 0) { [W]::MoveWindow($h, 60, 40, $width, $height, $true) | Out-Null }
    [W]::SetForegroundWindow($h) | Out-Null
    Start-Sleep -Seconds 2
    $r = Frame $w
    $bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
    $bmp.Save("$out\$file", [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    say "$file $($r.R - $r.L)x$($r.B - $r.T) at $($r.L),$($r.T)"
    return $r
}

# Windows left from an earlier run: the GPU's property sheet.
foreach ($w in $A::RootElement.FindAll($T::Children, [System.Windows.Automation.Condition]::TrueCondition)) {
    if (Match $w.Current.Name "*Properties", "Eigenschaften von*" $true) {
        try { ([System.Windows.Automation.WindowPattern]$w.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)).Close() } catch {}
    }
}
(New-Object -ComObject Shell.Application).MinimizeAll()
Start-Sleep -Seconds 2
$gpu = Get-PnpDevice -Class Display -PresentOnly | Where-Object { $_.InstanceId -like "PCI\VEN_10DE*" } | Select-Object -First 1
say "GPU: $($gpu.FriendlyName) $($gpu.InstanceId)"
$want = $Only.Split(",")

if ($want -contains "devmgr") {
    Start-Process mmc.exe -ArgumentList "devmgmt.msc"
    $w = Wait-Window "Device Manager", "Ger$([char]0xE4)te-Manager" 60 -Class MMCMainFrame
    Start-Sleep -Seconds 2
    # The device tree is devmgr.dll's own control, which UI Automation does not see: keys. The
    # root row is selected; type-ahead to the display adapters, expand, select the GPU below.
    $h = [IntPtr]$w.Current.NativeWindowHandle
    [W]::ShowWindow($h, 9) | Out-Null
    [W]::MoveWindow($h, 60, 40, 900, 700, $true) | Out-Null
    [W]::SetForegroundWindow($h) | Out-Null
    Start-Sleep -Seconds 1
    $keys = if ($w.Current.Name -eq "Device Manager") { "Display a" } else { "Grafik" }
    # A click on the root row (below the toolbar) gives the tree the focus.
    $f = Frame $w
    [W]::Click($f.L + 90, $f.T + 93)
    Start-Sleep -Milliseconds 500
    [System.Windows.Forms.SendKeys]::SendWait("{HOME}")
    Start-Sleep -Milliseconds 300
    [System.Windows.Forms.SendKeys]::SendWait($keys)
    Start-Sleep -Milliseconds 800
    [System.Windows.Forms.SendKeys]::SendWait("{RIGHT}")
    Start-Sleep -Milliseconds 800
    [System.Windows.Forms.SendKeys]::SendWait("{DOWN}")
    Start-Sleep -Seconds 1
    Shot $w "devmgr.png" | Out-Null
    ([System.Windows.Automation.WindowPattern]$w.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)).Close()
}

if ($want -contains "props") {
    Start-Process rundll32.exe -ArgumentList "devmgr.dll,DeviceProperties_RunDLL /DeviceID `"$($gpu.InstanceId)`""
    $d = Wait-Window "*Properties", "Eigenschaften von*" 30 -Like
    # The property sheet's tabs are not UI Automation items either: Ctrl+Tab to the driver tab.
    [W]::SetForegroundWindow([IntPtr]$d.Current.NativeWindowHandle) | Out-Null
    Start-Sleep -Milliseconds 500
    [System.Windows.Forms.SendKeys]::SendWait("^{TAB}")
    Start-Sleep -Seconds 1
    Shot $d "gpu-driver.png" | Out-Null
    ([System.Windows.Automation.WindowPattern]$d.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)).Close()
}

if ($want -contains "nvcpl") {
    $pkg = Get-AppxPackage -Name "NVIDIACorp.NVIDIAControlPanel" -ErrorAction SilentlyContinue
    if ($pkg) {
        # Through Explorer: a packaged app does not start from an elevated process.
        Start-Process explorer.exe -ArgumentList "shell:AppsFolder\$($pkg.PackageFamilyName)!NVIDIACorp.NVIDIAControlPanel"
        $w = Wait-Window "NVIDIA Control Panel", "NVIDIA Systemsteuerung" 60
        Start-Sleep -Seconds 3
        Act (Find $w "System Information", "Systeminformationen" 20)
        $d = Wait-Window "System Information", "Systeminformationen" 30
        Shot $d "nvcpl-sysinfo.png" | Out-Null
        ([System.Windows.Automation.WindowPattern]$d.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)).Close()
        ([System.Windows.Automation.WindowPattern]$w.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)).Close()
    } else { say "SKIP nvcpl: no NVIDIA Control Panel installed" }
}

if ($want -contains "taskmgr") {
    Start-Process taskmgr.exe
    $w = Wait-Window "Task Manager", "Task-Manager" 30
    Start-Sleep -Seconds 2
    Act (Find $w "Performance", "Leistung")
    Start-Sleep -Seconds 2
    Act (Find $w "GPU 0*" 20 -Like)
    Start-Sleep -Seconds 5
    Shot $w "taskmgr-gpu.png" 1180 760 | Out-Null
    ([System.Windows.Automation.WindowPattern]$w.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)).Close()
}

if ($want -contains "dxdiag") {
    Start-Process dxdiag.exe
    Start-Sleep -Seconds 4
    # First start asks whether to check the drivers' signatures (which may go online): no. The
    # question and the main window share the title; neither shows its buttons or tabs to UI
    # Automation, so keys.
    $names = "DirectX Diagnostic Tool", "DirectX-Diagnoseprogramm"
    $q = Wait-Window $names 30
    if ($q.Current.BoundingRectangle.Width -lt 600) {
        [W]::SetForegroundWindow([IntPtr]$q.Current.NativeWindowHandle) | Out-Null
        Start-Sleep -Milliseconds 500
        [System.Windows.Forms.SendKeys]::SendWait("n")
        say "signature check: no"
        Start-Sleep -Seconds 3
    }
    $w = Wait-Window $names 30
    # Until the "collecting information" box has come and gone (Direct3D takes a while).
    $seen = $false
    foreach ($i in 1..90) {
        Start-Sleep -Seconds 2
        $box = $true
        try { Wait-Window "Collecting Information*", "Informationen werden gesammelt*" 1 -Like | Out-Null } catch { $box = $false }
        if ($box) { $seen = $true } elseif ($seen -or $i -gt 20) { break }
    }
    say "dxdiag collected after about $(2 * $i) s"
    Start-Sleep -Seconds 2
    [W]::SetForegroundWindow([IntPtr]$w.Current.NativeWindowHandle) | Out-Null
    Start-Sleep -Milliseconds 500
    [System.Windows.Forms.SendKeys]::SendWait("^{TAB}")
    Start-Sleep -Seconds 2
    Shot $w "dxdiag-display.png" | Out-Null
    ([System.Windows.Automation.WindowPattern]$w.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)).Close()
}

say "done"
