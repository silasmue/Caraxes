# SPDX-License-Identifier: MIT
# Hardware D3D11 gate: draw a triangle, copy to a staging texture and verify pixels.
# Only an NVIDIA hardware adapter can pass. Uses the stock Windows D3D compiler.
$ErrorActionPreference = "Stop"
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
  public static bool Passed = false;
  static readonly System.Diagnostics.Stopwatch Clock = System.Diagnostics.Stopwatch.StartNew();
  public static void Stage(string name) { Console.WriteLine("stage " + Clock.ElapsedMilliseconds + " ms: " + name); Console.Out.Flush(); }
  [DllImport("dxgi.dll")] static extern int CreateDXGIFactory1(ref Guid riid, [MarshalAs(UnmanagedType.IUnknown)] out object f);
  [DllImport("d3d11.dll")] static extern int D3D11CreateDevice(IntPtr adapter, int type, IntPtr sw, uint flags, IntPtr fl, uint nfl, uint sdk, out IntPtr dev, out int level, out IntPtr ctx);
  public static string Run() {
    var s = new System.Text.StringBuilder(); Guid g = typeof(IDXGIFactory1).GUID; object fo;
    Stage("CreateDXGIFactory1 begin");
    int hr = CreateDXGIFactory1(ref g, out fo); Stage("CreateDXGIFactory1 end"); if (hr != 0) return "CreateDXGIFactory1 " + hr.ToString("x");
    var f = (IDXGIFactory1)fo;
    for (uint i = 0; ; i++) {
      Stage("EnumAdapters1 " + i);
      object ao; if (f.EnumAdapters1(i, out ao) != 0) break;
      var a = (IDXGIAdapter1)ao; DESC1 d; a.GetDesc1(out d);
      s.AppendLine(String.Format("adapter {0}: {1} ven {2:x4} dev {3:x4} dedicated {4} MiB flags {5}", i, d.Description, d.VendorId, d.DeviceId, d.Ded.ToInt64() >> 20, d.Flags));
      if (d.VendorId != 0x10de || (d.Flags & 2) != 0) { Marshal.ReleaseComObject(ao); continue; }
      IntPtr pa = Marshal.GetComInterfaceForObject(a, typeof(IDXGIAdapter1)); IntPtr dev, ctx; int lvl;
      var t0 = DateTime.Now;
      Stage("D3D11CreateDevice begin: " + d.Description);
      hr = D3D11CreateDevice(pa, 0, IntPtr.Zero, 0, IntPtr.Zero, 0, 7, out dev, out lvl, out ctx);
      Stage("D3D11CreateDevice end: 0x" + hr.ToString("x8"));
      s.AppendLine(String.Format("  D3D11CreateDevice hr {0:x8} feature level {1:x} in {2} ms", hr, lvl, (int)(DateTime.Now - t0).TotalMilliseconds));
      try { if (hr == 0) { s.AppendLine(RenderGate.Run(dev, ctx)); Passed = true; } }
      finally { if (ctx != IntPtr.Zero) Marshal.Release(ctx); if (dev != IntPtr.Zero) Marshal.Release(dev); Marshal.Release(pa); Marshal.ReleaseComObject(ao); }
      if (Passed) break;
    }
    Marshal.ReleaseComObject(fo);
    return s.ToString();
  }
}

public static class RenderGate {
  // ID3D11Device/ID3D11DeviceContext ABI, including inherited IUnknown/DeviceChild slots.
  [StructLayout(LayoutKind.Sequential)] struct Texture { public uint W,H,Mips,Array,Format,Samples,Quality,Usage,Bind,CPU,Misc; }
  [StructLayout(LayoutKind.Sequential)] struct Viewport { public float X,Y,W,H,Min,Max; }
  [StructLayout(LayoutKind.Sequential)] struct Mapped { public IntPtr Data; public uint Row,Depth; }
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateTexture(IntPtr self, ref Texture desc, IntPtr data, out IntPtr texture);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateView(IntPtr self, IntPtr resource, IntPtr desc, out IntPtr view);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateShader(IntPtr self, IntPtr bytes, UIntPtr size, IntPtr linkage, out IntPtr shader);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate IntPtr BlobPtr(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate UIntPtr BlobSize(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Targets(IntPtr self,uint n,ref IntPtr view,IntPtr depth);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Clear(IntPtr self,IntPtr view,[In] float[] color);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void SetShader(IntPtr self,IntPtr shader,IntPtr instances,uint n);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void SetViewport(IntPtr self,uint n,ref Viewport viewport);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Topology(IntPtr self,uint value);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Draw(IntPtr self,uint n,uint first);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Copy(IntPtr self,IntPtr dst,IntPtr src);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int Map(IntPtr self,IntPtr resource,uint sub,uint mode,uint flags,out Mapped mapped);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Unmap(IntPtr self,IntPtr resource,uint sub);
  [DllImport("d3dcompiler_47.dll",CharSet=CharSet.Ansi,ExactSpelling=true)]
  static extern int D3DCompile(byte[] src,UIntPtr size,string name,IntPtr defs,IntPtr include,string entry,string target,uint flags,uint flags2,out IntPtr code,out IntPtr errors);
  static T Fn<T>(IntPtr obj,int slot) { return (T)(object)Marshal.GetDelegateForFunctionPointer(Marshal.ReadIntPtr(Marshal.ReadIntPtr(obj),slot*IntPtr.Size),typeof(T)); }
  static void Check(int hr,string op) { if(hr<0) throw new Exception(op+" HRESULT 0x"+hr.ToString("x8")); }
  static IntPtr Shader(IntPtr dev,string entry,string target,int slot) {
    byte[] src=System.Text.Encoding.ASCII.GetBytes("float4 vs(uint i:SV_VertexID):SV_Position { float2 p[3]={float2(-0.75,-0.75),float2(0,0.75),float2(0.75,-0.75)}; return float4(p[i],0,1); } float4 ps():SV_Target { return float4(0.25,0.5,0.75,1); }");
    IntPtr code=IntPtr.Zero,errors=IntPtr.Zero,shader=IntPtr.Zero;
    try {
      int hr=D3DCompile(src,(UIntPtr)src.Length,"caraxes-render",IntPtr.Zero,IntPtr.Zero,entry,target,0,0,out code,out errors);
      if(hr<0 && errors!=IntPtr.Zero) throw new Exception(Marshal.PtrToStringAnsi(Fn<BlobPtr>(errors,3)(errors)));
      Check(hr,"D3DCompile");
      Check(Fn<CreateShader>(dev,slot)(dev,Fn<BlobPtr>(code,3)(code),Fn<BlobSize>(code,4)(code),IntPtr.Zero,out shader),"CreateShader");
      return shader;
    } finally { if(errors!=IntPtr.Zero)Marshal.Release(errors); if(code!=IntPtr.Zero)Marshal.Release(code); }
  }
  public static string Run(IntPtr dev,IntPtr ctx) {
    IntPtr render=IntPtr.Zero,staging=IntPtr.Zero,rtv=IntPtr.Zero,vs=IntPtr.Zero,ps=IntPtr.Zero;
    try {
      Probe.Stage("render resources begin");
      Texture d=new Texture { W=64,H=64,Mips=1,Array=1,Format=28,Samples=1,Bind=0x20 };
      Check(Fn<CreateTexture>(dev,5)(dev,ref d,IntPtr.Zero,out render),"CreateTexture2D(render)");
      d.Usage=3;d.Bind=0;d.CPU=0x20000;
      Check(Fn<CreateTexture>(dev,5)(dev,ref d,IntPtr.Zero,out staging),"CreateTexture2D(staging)");
      Check(Fn<CreateView>(dev,9)(dev,render,IntPtr.Zero,out rtv),"CreateRenderTargetView");
      vs=Shader(dev,"vs","vs_4_0",12);ps=Shader(dev,"ps","ps_4_0",15);
      Probe.Stage("render resources ready");
      Fn<Targets>(ctx,33)(ctx,1,ref rtv,IntPtr.Zero);
      Fn<SetShader>(ctx,11)(ctx,vs,IntPtr.Zero,0);Fn<SetShader>(ctx,9)(ctx,ps,IntPtr.Zero,0);
      Fn<Topology>(ctx,24)(ctx,4);
      Viewport vp=new Viewport { W=64,H=64,Max=1 };Fn<SetViewport>(ctx,44)(ctx,1,ref vp);
      var clock=System.Diagnostics.Stopwatch.StartNew();
      for(int frame=0;frame<3;frame++) {
        Probe.Stage("frame " + frame + " submit");
        Fn<Clear>(ctx,50)(ctx,rtv,new float[]{0,0,0,1});
        // Alternate triangle/clear/triangle: stale readback cannot pass later frames.
        if(frame!=1)Fn<Draw>(ctx,13)(ctx,3,0);
        Fn<Copy>(ctx,47)(ctx,staging,render);
        Probe.Stage("frame " + frame + " readback begin");
        Mapped m;Check(Fn<Map>(ctx,14)(ctx,staging,0,1,0,out m),"Map(readback)");
        try {
          if(m.Data==IntPtr.Zero || m.Row<256)throw new Exception("invalid mapped texture");
          byte[] center=new byte[4],corner=new byte[4];
          Marshal.Copy(IntPtr.Add(m.Data,checked((int)m.Row*32+32*4)),center,0,4);Marshal.Copy(m.Data,corner,0,4);
          int[] expected=frame==1 ? new int[]{0,0,0,255} : new int[]{64,128,191,255};
          for(int c=0;c<4;c++)if(Math.Abs(center[c]-expected[c])>1)throw new Exception("triangle pixel mismatch: "+BitConverter.ToString(center));
          if(corner[0]!=0||corner[1]!=0||corner[2]!=0||corner[3]!=255)throw new Exception("clear pixel mismatch: "+BitConverter.ToString(corner));
        } finally { Fn<Unmap>(ctx,15)(ctx,staging,0); }
        Probe.Stage("frame " + frame + " verified");
      }
      clock.Stop();return "PASS: NVIDIA D3D11 triangle + background readback, 3 frames, "+clock.ElapsedMilliseconds+" ms (smoke test, not throughput benchmark)";
    } finally { foreach(IntPtr p in new IntPtr[]{ps,vs,rtv,staging,render})if(p!=IntPtr.Zero)Marshal.Release(p); }
  }
}

'@
Add-Type -TypeDefinition $src -Language CSharp
[Probe]::Run()

if (-not [Probe]::Passed) { throw "No NVIDIA hardware adapter passed the rendering gate" }
