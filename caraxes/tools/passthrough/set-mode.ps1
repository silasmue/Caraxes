# SPDX-License-Identifier: MIT
# R4: the primary display's mode, persistently (ChangeDisplaySettingsEx with CDS_UPDATEREGISTRY):
# -Width x -Height at -Hz. R4 uses 1920x1080@60, the raster of the synthetic device's EDID
# (CEA timing, 148.5 MHz), so flip cadence and blank lines compare. -List prints the modes
# EnumDisplaySettings offers. Runs in the signed-in user's session (session 0 has no console
# display). Stock Windows components only (PowerShell 5.1, C# 5).
param([int]$Width = 1920, [int]$Height = 1080, [int]$Hz = 60, [switch]$List)
$ErrorActionPreference = "Stop"
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class R4Mode {
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  public struct DEVMODE {
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
    public short dmSpecVersion, dmDriverVersion, dmSize, dmDriverExtra; public int dmFields;
    public int dmPositionX, dmPositionY, dmDisplayOrientation, dmDisplayFixedOutput;
    public short dmColor, dmDuplex, dmYResolution, dmTTOption, dmCollate;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
    public short dmLogPixels; public int dmBitsPerPel, dmPelsWidth, dmPelsHeight, dmDisplayFlags, dmDisplayFrequency;
    public int dmICMMethod, dmICMIntent, dmMediaType, dmDitherType, dmReserved1, dmReserved2, dmPanningWidth, dmPanningHeight;
  }
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool EnumDisplaySettings(string dev, int mode, ref DEVMODE dm);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int ChangeDisplaySettingsEx(string dev, ref DEVMODE dm, IntPtr hwnd, int flags, IntPtr param);
}
'@
function Cur { $d = New-Object R4Mode+DEVMODE; $d.dmSize = [Runtime.InteropServices.Marshal]::SizeOf($d)
    [void][R4Mode]::EnumDisplaySettings($null, -1, [ref]$d); $d }
$d = Cur
"current: {0}x{1} @ {2} Hz, {3} bpp" -f $d.dmPelsWidth, $d.dmPelsHeight, $d.dmDisplayFrequency, $d.dmBitsPerPel
if ($List) {
    $m = New-Object R4Mode+DEVMODE; $m.dmSize = [Runtime.InteropServices.Marshal]::SizeOf($m); $i = 0; $seen = @{}
    while ([R4Mode]::EnumDisplaySettings($null, $i, [ref]$m)) {
        $k = "{0}x{1} @ {2} Hz" -f $m.dmPelsWidth, $m.dmPelsHeight, $m.dmDisplayFrequency
        if (-not $seen.ContainsKey($k)) { $seen[$k] = 1; $k }
        $i++
    }
    exit 0
}
$d.dmPelsWidth = $Width; $d.dmPelsHeight = $Height; $d.dmDisplayFrequency = $Hz
$d.dmFields = 0x80000 -bor 0x100000 -bor 0x400000   # DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY
$r = [R4Mode]::ChangeDisplaySettingsEx($null, [ref]$d, [IntPtr]::Zero, 0x1, [IntPtr]::Zero)   # CDS_UPDATEREGISTRY
"ChangeDisplaySettingsEx: $r (0 = DISP_CHANGE_SUCCESSFUL)"
$d = Cur
"now: {0}x{1} @ {2} Hz" -f $d.dmPelsWidth, $d.dmPelsHeight, $d.dmDisplayFrequency
if ($r -ne 0 -or $d.dmPelsWidth -ne $Width -or $d.dmPelsHeight -ne $Height -or $d.dmDisplayFrequency -ne $Hz) { "FAIL"; exit 1 }
"PASS"
