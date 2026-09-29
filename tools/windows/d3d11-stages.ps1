# SPDX-License-Identifier: MIT
# D3D11 render stages on the NVIDIA hardware device, each read back and checked: a clear and a
# triangle, a depth-tested pair of triangles (D32_FLOAT), a texture sampled with Load, a
# constant buffer, a compute shader writing a UAV, a 4x MSAA target resolved, an event query and
# an occlusion query read back by the CPU (GetData), and a tessellated triangle. Each stage
# prints PASS or FAIL with the pixels it read, so a scene that renders black can be narrowed
# to the stage that fails. Stock Windows components only (d3d11, d3dcompiler_47).
$ErrorActionPreference = "Stop"
$src = @'
using System; using System.Runtime.InteropServices;
public static class Stages {
  [StructLayout(LayoutKind.Sequential)] struct Texture { public uint W,H,Mips,Array,Format,Samples,Quality,Usage,Bind,CPU,Misc; }
  [StructLayout(LayoutKind.Sequential)] struct BufferDesc { public uint Bytes,Usage,Bind,CPU,Misc,Stride; }
  [StructLayout(LayoutKind.Sequential)] struct Init { public IntPtr Mem; public uint Pitch,Slice; }
  [StructLayout(LayoutKind.Sequential)] struct Viewport { public float X,Y,W,H,Min,Max; }
  [StructLayout(LayoutKind.Sequential)] struct Mapped { public IntPtr Data; public uint Row,Depth; }
  [StructLayout(LayoutKind.Sequential)] struct QueryDesc { public uint Query, Misc; }
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateQuery(IntPtr self, ref QueryDesc desc, out IntPtr query);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Async(IntPtr self, IntPtr query);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int GetData(IntPtr self, IntPtr query, IntPtr data, uint size, uint flags);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateTexture(IntPtr self, ref Texture desc, IntPtr data, out IntPtr texture);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateTextureInit(IntPtr self, ref Texture desc, ref Init data, out IntPtr texture);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateBuffer(IntPtr self, ref BufferDesc desc, ref Init data, out IntPtr buffer);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateView(IntPtr self, IntPtr resource, IntPtr desc, out IntPtr view);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateShader(IntPtr self, IntPtr bytes, UIntPtr size, IntPtr linkage, out IntPtr shader);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate IntPtr BlobPtr(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate UIntPtr BlobSize(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Targets(IntPtr self, uint n, ref IntPtr view, IntPtr depth);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Clear(IntPtr self, IntPtr view, [In] float[] color);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void ClearDepth(IntPtr self, IntPtr view, uint flags, float depth, byte stencil);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void SetShader(IntPtr self, IntPtr shader, IntPtr instances, uint n);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void SetViews(IntPtr self, uint start, uint n, ref IntPtr views);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void SetUavs(IntPtr self, uint start, uint n, ref IntPtr uav, IntPtr counts);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void SetViewport(IntPtr self, uint n, ref Viewport viewport);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Topology(IntPtr self, uint value);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Draw(IntPtr self, uint n, uint first);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Dispatch(IntPtr self, uint x, uint y, uint z);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Copy(IntPtr self, IntPtr dst, IntPtr src);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Resolve(IntPtr self, IntPtr dst, uint dsub, IntPtr src, uint ssub, uint format);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int Map(IntPtr self, IntPtr resource, uint sub, uint mode, uint flags, out Mapped mapped);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Unmap(IntPtr self, IntPtr resource, uint sub);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Flush(IntPtr self);
  [DllImport("d3d11.dll")] static extern int D3D11CreateDevice(IntPtr adapter, int type, IntPtr sw, uint flags, IntPtr fl, uint nfl, uint sdk, out IntPtr dev, out int level, out IntPtr ctx);
  [DllImport("d3dcompiler_47.dll", CharSet = CharSet.Ansi, ExactSpelling = true)]
  static extern int D3DCompile(byte[] src, UIntPtr size, string name, IntPtr defs, IntPtr include, string entry, string target, uint flags, uint flags2, out IntPtr code, out IntPtr errors);
  static T Fn<T>(IntPtr obj, int slot) { return (T)(object)Marshal.GetDelegateForFunctionPointer(Marshal.ReadIntPtr(Marshal.ReadIntPtr(obj), slot * IntPtr.Size), typeof(T)); }
  static void Check(int hr, string op) { if (hr < 0) throw new Exception(op + " HRESULT 0x" + hr.ToString("x8")); }
  const string Hlsl = @"
cbuffer C : register(b0) { float4 cbcolor; };
Texture2D<float4> T : register(t0);
RWTexture2D<unorm float4> U : register(u0);
float4 vs(uint i : SV_VertexID) : SV_Position { float2 q[3] = { float2(-0.75,-0.75), float2(0,0.75), float2(0.75,-0.75) }; return float4(q[i], 0.5, 1); }
float4 vs_far(uint i : SV_VertexID) : SV_Position { float2 q[3] = { float2(-1,-1), float2(0,1), float2(1,-1) }; return float4(q[i], 0.75, 1); }
float4 ps() : SV_Target { return float4(0.25, 0.5, 0.75, 1); }
float4 ps_red() : SV_Target { return float4(1, 0, 0, 1); }
float4 ps_tex() : SV_Target { return T.Load(int3(1, 1, 0)); }
float4 ps_cb() : SV_Target { return cbcolor; }
[numthreads(8, 8, 1)] void cs(uint3 id : SV_DispatchThreadID) { U[id.xy] = float4(0.5, 0.25, 1, 1); }
struct VO { float4 p : SV_Position; };
VO vs_patch(uint i : SV_VertexID) { float2 q[3] = { float2(-0.75,-0.75), float2(0,0.75), float2(0.75,-0.75) }; VO o; o.p = float4(q[i], 0.5, 1); return o; }
struct HC { float e[3] : SV_TessFactor; float i : SV_InsideTessFactor; };
HC hsc(InputPatch<VO, 3> p) { HC c; c.e[0] = 4; c.e[1] = 4; c.e[2] = 4; c.i = 4; return c; }
[domain(""tri"")] [partitioning(""integer"")] [outputtopology(""triangle_cw"")] [outputcontrolpoints(3)] [patchconstantfunc(""hsc"")]
VO hs(InputPatch<VO, 3> p, uint i : SV_OutputControlPointID) { return p[i]; }
[domain(""tri"")] VO ds(HC c, float3 b : SV_DomainLocation, const OutputPatch<VO, 3> p) { VO o; o.p = p[0].p * b.x + p[1].p * b.y + p[2].p * b.z; return o; }
";
  static IntPtr dev, ctx, staging;
  static IntPtr Shader(string entry, string target, int slot) {
    byte[] s = System.Text.Encoding.ASCII.GetBytes(Hlsl);
    IntPtr code = IntPtr.Zero, errors = IntPtr.Zero, shader = IntPtr.Zero;
    try {
      int hr = D3DCompile(s, (UIntPtr)s.Length, "caraxes-stages", IntPtr.Zero, IntPtr.Zero, entry, target, 0, 0, out code, out errors);
      if (hr < 0 && errors != IntPtr.Zero) throw new Exception(Marshal.PtrToStringAnsi(Fn<BlobPtr>(errors, 3)(errors)));
      Check(hr, "D3DCompile " + entry);
      Check(Fn<CreateShader>(dev, slot)(dev, Fn<BlobPtr>(code, 3)(code), Fn<BlobSize>(code, 4)(code), IntPtr.Zero, out shader), "Create " + entry);
      return shader;
    } finally { if (errors != IntPtr.Zero) Marshal.Release(errors); if (code != IntPtr.Zero) Marshal.Release(code); }
  }
  static IntPtr Tex(uint w, uint h, uint format, uint bind, uint samples) {
    var d = new Texture { W = w, H = h, Mips = 1, Array = 1, Format = format, Samples = samples, Bind = bind }; IntPtr t;
    Check(Fn<CreateTexture>(dev, 5)(dev, ref d, IntPtr.Zero, out t), "CreateTexture2D"); return t;
  }
  static IntPtr View(int slot, IntPtr res, string what) { IntPtr v; Check(Fn<CreateView>(dev, slot)(dev, res, IntPtr.Zero, out v), what); return v; }
  static byte[] Pixel(IntPtr tex, int x, int y) {
    Fn<Copy>(ctx, 47)(ctx, staging, tex);
    Mapped m; Check(Fn<Map>(ctx, 14)(ctx, staging, 0, 1, 0, out m), "Map");
    try { var b = new byte[4]; Marshal.Copy(IntPtr.Add(m.Data, y * (int)m.Row + x * 4), b, 0, 4); return b; }
    finally { Fn<Unmap>(ctx, 15)(ctx, staging, 0); }
  }
  static string Expect(string name, byte[] got, int[] want) {
    bool ok = true; for (int c = 0; c < 4; c++) if (Math.Abs(got[c] - want[c]) > 2) ok = false;
    return (ok ? "PASS " : "FAIL ") + name + ": read " + BitConverter.ToString(got) + ", want " + String.Join("-", Array.ConvertAll(want, v => v.ToString("X2")));
  }
  static void Frame(IntPtr rtv, IntPtr dsv) {
    Fn<Targets>(ctx, 33)(ctx, 1, ref rtv, dsv);
    var vp = new Viewport { W = 256, H = 256, Max = 1 }; Fn<SetViewport>(ctx, 44)(ctx, 1, ref vp);
    Fn<Topology>(ctx, 24)(ctx, 4);
  }
  public static string Run() {
    var s = new System.Text.StringBuilder(); int level;
    Check(D3D11CreateDevice(IntPtr.Zero, 1, IntPtr.Zero, 0, IntPtr.Zero, 0, 7, out dev, out level, out ctx), "D3D11CreateDevice");
    s.AppendLine("device feature level " + level.ToString("x"));
    staging = IntPtr.Zero;
    { var d = new Texture { W = 256, H = 256, Mips = 1, Array = 1, Format = 28, Samples = 1, Usage = 3, CPU = 0x20000 };
      Check(Fn<CreateTexture>(dev, 5)(dev, ref d, IntPtr.Zero, out staging), "staging"); }
    IntPtr vs = Shader("vs", "vs_5_0", 12), vsFar = Shader("vs_far", "vs_5_0", 12), ps = Shader("ps", "ps_5_0", 15),
           psRed = Shader("ps_red", "ps_5_0", 15), psTex = Shader("ps_tex", "ps_5_0", 15), psCb = Shader("ps_cb", "ps_5_0", 15),
           cs = Shader("cs", "cs_5_0", 18);
    int[] blue = { 64, 128, 191, 255 }, black = { 0, 0, 0, 255 }, red = { 255, 0, 0, 255 };
    IntPtr rt = Tex(256, 256, 28, 0x20, 1), rtv = View(9, rt, "RTV");
    // 1. Clear and triangle.
    try {
      Frame(rtv, IntPtr.Zero); Fn<Clear>(ctx, 50)(ctx, rtv, new float[] { 0, 0, 0, 1 });
      Fn<SetShader>(ctx, 11)(ctx, vs, IntPtr.Zero, 0); Fn<SetShader>(ctx, 9)(ctx, ps, IntPtr.Zero, 0); Fn<Draw>(ctx, 13)(ctx, 3, 0);
      s.AppendLine(Expect("triangle (center)", Pixel(rt, 128, 128), blue)); s.AppendLine(Expect("clear (corner)", Pixel(rt, 2, 2), black));
    } catch (Exception e) { s.AppendLine("FAIL triangle: " + e.Message); }
    // 2. Depth: the near triangle stays in front of the far one drawn after it.
    try {
      IntPtr ds = Tex(256, 256, 40, 0x40, 1), dsv = View(10, ds, "DSV");
      Frame(rtv, dsv); Fn<Clear>(ctx, 50)(ctx, rtv, new float[] { 0, 0, 0, 1 }); Fn<ClearDepth>(ctx, 53)(ctx, dsv, 1, 1.0f, 0);
      Fn<SetShader>(ctx, 11)(ctx, vs, IntPtr.Zero, 0); Fn<SetShader>(ctx, 9)(ctx, ps, IntPtr.Zero, 0); Fn<Draw>(ctx, 13)(ctx, 3, 0);
      Fn<SetShader>(ctx, 11)(ctx, vsFar, IntPtr.Zero, 0); Fn<SetShader>(ctx, 9)(ctx, psRed, IntPtr.Zero, 0); Fn<Draw>(ctx, 13)(ctx, 3, 0);
      s.AppendLine(Expect("depth: near triangle kept (center)", Pixel(rt, 128, 128), blue));
      s.AppendLine(Expect("depth: far triangle where nothing is nearer", Pixel(rt, 128, 250), red));
      Frame(rtv, IntPtr.Zero); Marshal.Release(dsv); Marshal.Release(ds);
    } catch (Exception e) { s.AppendLine("FAIL depth: " + e.Message); }
    // 3. A texture read with Load.
    try {
      var px = new byte[16 * 4]; for (int i = 0; i < 16; i++) { px[i*4] = 200; px[i*4+1] = 100; px[i*4+2] = 50; px[i*4+3] = 255; }
      var h = GCHandle.Alloc(px, GCHandleType.Pinned); IntPtr tex, srv;
      try { var init = new Init { Mem = h.AddrOfPinnedObject(), Pitch = 16 }; var d = new Texture { W = 4, H = 4, Mips = 1, Array = 1, Format = 28, Samples = 1, Bind = 0x8 };
            Check(Fn<CreateTextureInit>(dev, 5)(dev, ref d, ref init, out tex), "texture"); } finally { h.Free(); }
      srv = View(7, tex, "SRV");
      Frame(rtv, IntPtr.Zero); Fn<Clear>(ctx, 50)(ctx, rtv, new float[] { 0, 0, 0, 1 });
      Fn<SetViews>(ctx, 8)(ctx, 0, 1, ref srv);
      Fn<SetShader>(ctx, 11)(ctx, vs, IntPtr.Zero, 0); Fn<SetShader>(ctx, 9)(ctx, psTex, IntPtr.Zero, 0); Fn<Draw>(ctx, 13)(ctx, 3, 0);
      s.AppendLine(Expect("texture Load", Pixel(rt, 128, 128), new int[] { 200, 100, 50, 255 }));
      Marshal.Release(srv); Marshal.Release(tex);
    } catch (Exception e) { s.AppendLine("FAIL texture: " + e.Message); }
    // 4. A constant buffer.
    try {
      var f = new float[] { 0.1f, 0.9f, 0.3f, 1f }; var h = GCHandle.Alloc(f, GCHandleType.Pinned); IntPtr cb;
      try { var init = new Init { Mem = h.AddrOfPinnedObject() }; var d = new BufferDesc { Bytes = 16, Bind = 0x4 };
            Check(Fn<CreateBuffer>(dev, 3)(dev, ref d, ref init, out cb), "constant buffer"); } finally { h.Free(); }
      Frame(rtv, IntPtr.Zero); Fn<Clear>(ctx, 50)(ctx, rtv, new float[] { 0, 0, 0, 1 });
      Fn<SetViews>(ctx, 16)(ctx, 0, 1, ref cb);
      Fn<SetShader>(ctx, 11)(ctx, vs, IntPtr.Zero, 0); Fn<SetShader>(ctx, 9)(ctx, psCb, IntPtr.Zero, 0); Fn<Draw>(ctx, 13)(ctx, 3, 0);
      s.AppendLine(Expect("constant buffer", Pixel(rt, 128, 128), new int[] { 26, 230, 77, 255 }));
      Marshal.Release(cb);
    } catch (Exception e) { s.AppendLine("FAIL constant buffer: " + e.Message); }
    // 5. A compute shader writing a UAV.
    try {
      IntPtr ut = Tex(256, 256, 28, 0x80, 1), uav = View(8, ut, "UAV");
      Fn<SetShader>(ctx, 69)(ctx, cs, IntPtr.Zero, 0); Fn<SetUavs>(ctx, 68)(ctx, 0, 1, ref uav, IntPtr.Zero); Fn<Dispatch>(ctx, 41)(ctx, 32, 32, 1);
      IntPtr none = IntPtr.Zero; Fn<SetUavs>(ctx, 68)(ctx, 0, 1, ref none, IntPtr.Zero);
      s.AppendLine(Expect("compute UAV", Pixel(ut, 128, 128), new int[] { 128, 64, 255, 255 }));
      Marshal.Release(uav); Marshal.Release(ut);
    } catch (Exception e) { s.AppendLine("FAIL compute: " + e.Message); }
    // 6. 4x MSAA resolved.
    try {
      IntPtr ms = Tex(256, 256, 28, 0x20, 4), msv = View(9, ms, "MSAA RTV");
      Frame(msv, IntPtr.Zero); Fn<Clear>(ctx, 50)(ctx, msv, new float[] { 0, 0, 0, 1 });
      Fn<SetShader>(ctx, 11)(ctx, vs, IntPtr.Zero, 0); Fn<SetShader>(ctx, 9)(ctx, ps, IntPtr.Zero, 0); Fn<Draw>(ctx, 13)(ctx, 3, 0);
      Fn<Resolve>(ctx, 57)(ctx, rt, 0, ms, 0, 28);
      s.AppendLine(Expect("MSAA 4x resolved", Pixel(rt, 128, 128), blue));
      Marshal.Release(msv); Marshal.Release(ms);
    } catch (Exception e) { s.AppendLine("FAIL MSAA: " + e.Message); }
    // 7. Event and occlusion queries: the GPU's completion read back by the CPU.
    try {
      var qd = new QueryDesc { Query = 1 }; IntPtr occ, ev; Check(Fn<CreateQuery>(dev, 24)(dev, ref qd, out occ), "occlusion query");
      qd.Query = 0; Check(Fn<CreateQuery>(dev, 24)(dev, ref qd, out ev), "event query");
      Frame(rtv, IntPtr.Zero); Fn<Clear>(ctx, 50)(ctx, rtv, new float[] { 0, 0, 0, 1 });
      Fn<Async>(ctx, 27)(ctx, occ);
      Fn<SetShader>(ctx, 11)(ctx, vs, IntPtr.Zero, 0); Fn<SetShader>(ctx, 9)(ctx, ps, IntPtr.Zero, 0); Fn<Draw>(ctx, 13)(ctx, 3, 0);
      Fn<Async>(ctx, 28)(ctx, occ); Fn<Async>(ctx, 28)(ctx, ev);
      var t0 = System.Diagnostics.Stopwatch.StartNew(); IntPtr buf = Marshal.AllocHGlobal(8);
      try {
        int hr; do { hr = Fn<GetData>(ctx, 29)(ctx, ev, buf, 4, 0); } while (hr == 1 && t0.ElapsedMilliseconds < 3000);
        s.AppendLine((hr == 0 ? "PASS" : "FAIL") + " event query: GetData hr 0x" + hr.ToString("x8") + " after " + t0.ElapsedMilliseconds + " ms");
        t0.Restart(); do { hr = Fn<GetData>(ctx, 29)(ctx, occ, buf, 8, 0); } while (hr == 1 && t0.ElapsedMilliseconds < 3000);
        long samples = hr == 0 ? Marshal.ReadInt64(buf) : -1;
        s.AppendLine((hr == 0 && samples > 10000 && samples < 30000 ? "PASS" : "FAIL") + " occlusion query: " + samples + " samples (the triangle covers about 18000) after " + t0.ElapsedMilliseconds + " ms, hr 0x" + hr.ToString("x8"));
      } finally { Marshal.FreeHGlobal(buf); }
      Marshal.Release(ev); Marshal.Release(occ);
    } catch (Exception e) { s.AppendLine("FAIL queries: " + e.Message); }
    // 8. A tessellated triangle (hull and domain shaders, a 3-point patch).
    try {
      IntPtr vsp = Shader("vs_patch", "vs_5_0", 12), hs = Shader("hs", "hs_5_0", 16), ds = Shader("ds", "ds_5_0", 17);
      Frame(rtv, IntPtr.Zero); Fn<Topology>(ctx, 24)(ctx, 35); Fn<Clear>(ctx, 50)(ctx, rtv, new float[] { 0, 0, 0, 1 });
      Fn<SetShader>(ctx, 11)(ctx, vsp, IntPtr.Zero, 0); Fn<SetShader>(ctx, 60)(ctx, hs, IntPtr.Zero, 0); Fn<SetShader>(ctx, 64)(ctx, ds, IntPtr.Zero, 0);
      Fn<SetShader>(ctx, 9)(ctx, ps, IntPtr.Zero, 0); Fn<Draw>(ctx, 13)(ctx, 3, 0);
      Fn<SetShader>(ctx, 60)(ctx, IntPtr.Zero, IntPtr.Zero, 0); Fn<SetShader>(ctx, 64)(ctx, IntPtr.Zero, IntPtr.Zero, 0);
      s.AppendLine(Expect("tessellated triangle (center)", Pixel(rt, 128, 128), blue));
      s.AppendLine(Expect("tessellation: clear outside (corner)", Pixel(rt, 2, 2), black));
      Marshal.Release(ds); Marshal.Release(hs); Marshal.Release(vsp);
    } catch (Exception e) { s.AppendLine("FAIL tessellation: " + e.Message); }
    return s.ToString();
  }
}
'@
Add-Type -TypeDefinition $src -Language CSharp
$r = [Stages]::Run()
$r
if ($r -match "FAIL") { "FAIL: D3D11 stages" ; exit 1 } else { "PASS: D3D11 stages" }
