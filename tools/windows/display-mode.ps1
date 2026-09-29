# SPDX-License-Identifier: MIT
# The primary display's modes and a mode change: lists what EnumDisplaySettings offers, sets
# -Width x -Height (at the current refresh rate) with ChangeDisplaySettingsEx, waits -Hold
# seconds, then restores the registry mode. Runs in the signed-in user's session (the display
# settings of session 0 are not the console's). Stock Windows components only.
param([int]$Width = 1280, [int]$Height = 720, [int]$Hold = 10)
$ErrorActionPreference = "Stop"
$src = @'
using System; using System.Runtime.InteropServices;
public static class Mode {
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  public struct DEVMODE {
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string DeviceName;
    public ushort SpecVersion, DriverVersion, Size, DriverExtra; public uint Fields;
    public int PositionX, PositionY; public uint DisplayOrientation, DisplayFixedOutput;
    public short Color, Duplex, YResolution, TTOption, Collate;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string FormName;
    public ushort LogPixels; public uint BitsPerPel, PelsWidth, PelsHeight, DisplayFlags, DisplayFrequency;
    public uint ICMMethod, ICMIntent, MediaType, DitherType, Reserved1, Reserved2, PanningWidth, PanningHeight;
  }
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern bool EnumDisplaySettingsW(string dev, int mode, ref DEVMODE dm);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int ChangeDisplaySettingsExW(string dev, ref DEVMODE dm, IntPtr hwnd, uint flags, IntPtr param);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int ChangeDisplaySettingsExW(string dev, IntPtr dm, IntPtr hwnd, uint flags, IntPtr param);
  static DEVMODE New() { var d = new DEVMODE(); d.Size = (ushort)Marshal.SizeOf(typeof(DEVMODE)); return d; }
  public static string Current() {
    var d = New(); EnumDisplaySettingsW(null, -1, ref d);
    return String.Format("{0}x{1} {2} Hz {3} bpp", d.PelsWidth, d.PelsHeight, d.DisplayFrequency, d.BitsPerPel);
  }
  public static string List() {
    var seen = new System.Collections.Generic.SortedSet<string>();
    for (int i = 0; ; i++) {
      var d = New(); if (!EnumDisplaySettingsW(null, i, ref d)) break;
      if (d.BitsPerPel == 32) seen.Add(String.Format("{0,5}x{1,-5} {2} Hz", d.PelsWidth, d.PelsHeight, d.DisplayFrequency));
    }
    return String.Join("; ", seen);
  }
  public static int Set(uint w, uint h) {
    var d = New(); EnumDisplaySettingsW(null, -1, ref d);
    d.PelsWidth = w; d.PelsHeight = h; d.Fields = 0x80000 | 0x100000;   // DM_PELSWIDTH | DM_PELSHEIGHT
    return ChangeDisplaySettingsExW(null, ref d, IntPtr.Zero, 0, IntPtr.Zero);
  }
  public static int Restore() { return ChangeDisplaySettingsExW(null, IntPtr.Zero, IntPtr.Zero, 0, IntPtr.Zero); }
}
'@
Add-Type -TypeDefinition $src -Language CSharp
"current: " + [Mode]::Current()
"modes (32 bpp): " + [Mode]::List()
$t = Get-Date
$r = [Mode]::Set($Width, $Height)
"set ${Width}x${Height}: DISP_CHANGE $r in $([int]((Get-Date) - $t).TotalMilliseconds) ms; now " + [Mode]::Current()
Start-Sleep $Hold
$t = Get-Date
$r2 = [Mode]::Restore()
"restore: DISP_CHANGE $r2 in $([int]((Get-Date) - $t).TotalMilliseconds) ms; now " + [Mode]::Current()
if ($r -eq 0 -and $r2 -eq 0) { "PASS: mode change to ${Width}x${Height} and back" } else { "FAIL: mode change (0 is DISP_CHANGE_SUCCESSFUL)"; exit 1 }
