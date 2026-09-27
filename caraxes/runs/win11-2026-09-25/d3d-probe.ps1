# SPDX-License-Identifier: MIT
# D3D11 probe of every DXGI adapter (win-loop): enumerate the adapters, create a D3D11 device on each
# (hardware driver on that adapter), and report the feature level. A device on the NVIDIA adapter makes
# the UMD and the KMD create their contexts and channels; the device trace shows them.
$src = @'
using System; using System.Runtime.InteropServices;
[ComImport, Guid("770aae78-f26f-4dba-a829-253c83d1b387"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IDXGIFactory1 {
  void SetPrivateData(); void SetPrivateDataInterface(); void GetPrivateData(); void GetParent();
  void EnumAdapters(); void MakeWindowAssociation(); void GetWindowAssociation(); void CreateSwapChain(); void CreateSoftwareAdapter();
  [PreserveSig] int EnumAdapters1(uint i, [MarshalAs(UnmanagedType.IUnknown)] out object a);
}
[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
public struct DESC1 { [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string Description; public uint VendorId, DeviceId, SubSysId, Revision; public IntPtr Ded, DedSys, Shared; public uint LuidLo; public int LuidHi; public uint Flags; }
[ComImport, Guid("29038f61-3839-4626-91fd-086879011a05"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IDXGIAdapter1 {
  void SetPrivateData(); void SetPrivateDataInterface(); void GetPrivateData(); void GetParent();
  void EnumOutputs(); void GetDesc(); void CheckInterfaceSupport();
  [PreserveSig] int GetDesc1(out DESC1 d);
}
public static class Probe {
  [DllImport("dxgi.dll")] static extern int CreateDXGIFactory1(ref Guid riid, [MarshalAs(UnmanagedType.IUnknown)] out object f);
  [DllImport("d3d11.dll")] static extern int D3D11CreateDevice(IntPtr adapter, int type, IntPtr sw, uint flags, IntPtr fl, uint nfl, uint sdk, out IntPtr dev, out int level, out IntPtr ctx);
  public static string Run() {
    var s = new System.Text.StringBuilder(); Guid g = typeof(IDXGIFactory1).GUID; object fo;
    int hr = CreateDXGIFactory1(ref g, out fo); if (hr != 0) return "CreateDXGIFactory1 " + hr.ToString("x");
    var f = (IDXGIFactory1)fo;
    for (uint i = 0; ; i++) {
      object ao; if (f.EnumAdapters1(i, out ao) != 0) break;
      var a = (IDXGIAdapter1)ao; DESC1 d; a.GetDesc1(out d);
      s.AppendLine(String.Format("adapter {0}: {1} ven {2:x4} dev {3:x4} dedicated {4} MiB flags {5}", i, d.Description, d.VendorId, d.DeviceId, d.Ded.ToInt64() >> 20, d.Flags));
      IntPtr pa = Marshal.GetComInterfaceForObject(a, typeof(IDXGIAdapter1)); IntPtr dev, ctx; int lvl;
      var t0 = DateTime.Now;
      hr = D3D11CreateDevice(pa, 0, IntPtr.Zero, 0, IntPtr.Zero, 0, 7, out dev, out lvl, out ctx);
      s.AppendLine(String.Format("  D3D11CreateDevice hr {0:x8} feature level {1:x} in {2} ms", hr, lvl, (int)(DateTime.Now - t0).TotalMilliseconds));
      if (ctx != IntPtr.Zero) Marshal.Release(ctx); if (dev != IntPtr.Zero) Marshal.Release(dev); Marshal.Release(pa);
    }
    return s.ToString();
  }
}
'@
Add-Type -TypeDefinition $src -Language CSharp
[Probe]::Run()
