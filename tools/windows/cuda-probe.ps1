# SPDX-License-Identifier: MIT
# CUDA on the stock driver alone (nvcuda.dll, no toolkit): a deviceQuery, a vector add JIT-compiled
# from PTX and verified, kernel launch cost and pinned copy bandwidth (the numbers for the v1 and
# native comparison). C# 5 for PowerShell 5.1. -Managed adds managed memory: deviceQuery's
# managedMemory/concurrentManagedAccess, cuMemAllocManaged 64 MiB, a CPU write, a kernel increment,
# a synchronize and a CPU check (the GSP_GET_FEATURES UVM bit A/B of 2026-09-28).
# Every result line is printed as it is measured ("result: ..."), and again as a summary.
# Usage: cuda-probe.ps1 [-Mib 64] [-Managed]
param([int]$Mib = 64, [switch]$Managed)
$ErrorActionPreference = "Stop"
$src = @'
using System; using System.Runtime.InteropServices;
namespace CaraxesCuda {
public static class Probe {
  public static bool Passed = false;
  [DllImport("nvcuda.dll")] static extern int cuInit(uint flags);
  [DllImport("nvcuda.dll")] static extern int cuDriverGetVersion(out int v);
  [DllImport("nvcuda.dll")] static extern int cuDeviceGetCount(out int n);
  [DllImport("nvcuda.dll")] static extern int cuDeviceGet(out int dev, int ordinal);
  [DllImport("nvcuda.dll")] static extern int cuDeviceGetName(byte[] name, int len, int dev);
  [DllImport("nvcuda.dll", EntryPoint = "cuDeviceTotalMem_v2")] static extern int cuDeviceTotalMem(out UIntPtr bytes, int dev);
  [DllImport("nvcuda.dll")] static extern int cuDeviceGetAttribute(out int v, int attrib, int dev);
  [DllImport("nvcuda.dll")] static extern int cuDevicePrimaryCtxRetain(out IntPtr ctx, int dev);
  [DllImport("nvcuda.dll", EntryPoint = "cuDevicePrimaryCtxRelease_v2")] static extern int cuDevicePrimaryCtxRelease(int dev);
  [DllImport("nvcuda.dll")] static extern int cuCtxSetCurrent(IntPtr ctx);
  [DllImport("nvcuda.dll")] static extern int cuCtxSynchronize();
  [DllImport("nvcuda.dll", EntryPoint = "cuMemAlloc_v2")] static extern int cuMemAlloc(out ulong dptr, UIntPtr bytes);
  [DllImport("nvcuda.dll", EntryPoint = "cuMemFree_v2")] static extern int cuMemFree(ulong dptr);
  [DllImport("nvcuda.dll", EntryPoint = "cuMemAllocHost_v2")] static extern int cuMemAllocHost(out IntPtr p, UIntPtr bytes);
  [DllImport("nvcuda.dll")] static extern int cuMemFreeHost(IntPtr p);
  [DllImport("nvcuda.dll", EntryPoint = "cuMemcpyHtoD_v2")] static extern int cuMemcpyHtoD(ulong dst, IntPtr src, UIntPtr bytes);
  [DllImport("nvcuda.dll", EntryPoint = "cuMemcpyDtoH_v2")] static extern int cuMemcpyDtoH(IntPtr dst, ulong src, UIntPtr bytes);
  [DllImport("nvcuda.dll")] static extern int cuModuleLoadData(out IntPtr module, byte[] image);
  [DllImport("nvcuda.dll")] static extern int cuModuleUnload(IntPtr module);
  [DllImport("nvcuda.dll", CharSet = CharSet.Ansi)] static extern int cuModuleGetFunction(out IntPtr f, IntPtr module, string name);
  [DllImport("nvcuda.dll")] static extern int cuLaunchKernel(IntPtr f, uint gx, uint gy, uint gz, uint bx, uint by, uint bz, uint shared, IntPtr stream, IntPtr[] args, IntPtr extra);
  [DllImport("nvcuda.dll")] static extern int cuGetErrorName(int err, out IntPtr name);
  [DllImport("nvcuda.dll")] static extern int cuMemAllocManaged(out ulong dptr, UIntPtr bytes, uint flags);

  static readonly System.Diagnostics.Stopwatch Clock = System.Diagnostics.Stopwatch.StartNew();
  public static void Stage(string name) { Console.WriteLine("stage " + Clock.ElapsedMilliseconds + " ms: " + name); Console.Out.Flush(); }
  static void Check(int r, string op) {
    if (r == 0) return;
    IntPtr n; string name = cuGetErrorName(r, out n) == 0 ? Marshal.PtrToStringAnsi(n) : "?";
    throw new Exception(op + ": CUresult " + r + " " + name);
  }
  static void Say(System.Text.StringBuilder s, string line) { s.AppendLine(line); Console.WriteLine("result: " + line); Console.Out.Flush(); }
  static string Name(int r) { IntPtr n; return cuGetErrorName(r, out n) == 0 ? Marshal.PtrToStringAnsi(n) : "?"; }
  static string Attr(int dev, int a) { int v; return cuDeviceGetAttribute(out v, a, dev) == 0 ? v.ToString() : "n/a"; }

  const string Ptx = @"
.version 7.0
.target sm_75
.address_size 64

.visible .entry empty()
{
	ret;
}

.visible .entry spin(.param .u32 pcycles)
{
	.reg .pred %p<2>;
	.reg .b32 %r<2>;
	.reg .b64 %rd<5>;
	ld.param.u32 %r1, [pcycles];
	cvt.u64.u32 %rd4, %r1;
	mov.u64 %rd1, %clock64;
SPIN:
	mov.u64 %rd2, %clock64;
	sub.s64 %rd3, %rd2, %rd1;
	setp.lt.s64 %p1, %rd3, %rd4;
	@%p1 bra SPIN;
	ret;
}

.visible .entry inc(.param .u64 pp, .param .u32 pn)
{
	.reg .pred %p<2>;
	.reg .b32 %r<8>;
	.reg .b64 %rd<5>;
	ld.param.u64 %rd1, [pp];
	ld.param.u32 %r1, [pn];
	mov.u32 %r2, %ctaid.x;
	mov.u32 %r3, %ntid.x;
	mov.u32 %r4, %tid.x;
	mad.lo.s32 %r5, %r2, %r3, %r4;
	setp.ge.u32 %p1, %r5, %r1;
	@%p1 bra DONE;
	cvta.to.global.u64 %rd2, %rd1;
	mul.wide.u32 %rd3, %r5, 4;
	add.s64 %rd4, %rd2, %rd3;
	ld.global.u32 %r6, [%rd4];
	add.s32 %r7, %r6, 1;
	st.global.u32 [%rd4], %r7;
DONE:
	ret;
}

.visible .entry vadd(.param .u64 pa, .param .u64 pb, .param .u64 pc, .param .u32 pn)
{
	.reg .pred %p<2>;
	.reg .b32 %r<6>;
	.reg .f32 %f<4>;
	.reg .b64 %rd<11>;
	ld.param.u64 %rd1, [pa];
	ld.param.u64 %rd2, [pb];
	ld.param.u64 %rd3, [pc];
	ld.param.u32 %r1, [pn];
	mov.u32 %r2, %ctaid.x;
	mov.u32 %r3, %ntid.x;
	mov.u32 %r4, %tid.x;
	mad.lo.s32 %r5, %r2, %r3, %r4;
	setp.ge.u32 %p1, %r5, %r1;
	@%p1 bra DONE;
	cvta.to.global.u64 %rd4, %rd1;
	cvta.to.global.u64 %rd5, %rd2;
	cvta.to.global.u64 %rd6, %rd3;
	mul.wide.u32 %rd7, %r5, 4;
	add.s64 %rd8, %rd4, %rd7;
	add.s64 %rd9, %rd5, %rd7;
	add.s64 %rd10, %rd6, %rd7;
	ld.global.f32 %f1, [%rd8];
	ld.global.f32 %f2, [%rd9];
	add.f32 %f3, %f1, %f2;
	st.global.f32 [%rd10], %f3;
DONE:
	ret;
}
";

  public static string Run(int mib, bool managed) {
    var s = new System.Text.StringBuilder();
    int dev = 0; bool ctx = false; IntPtr module = IntPtr.Zero, host = IntPtr.Zero;
    ulong da = 0, db = 0, dc = 0, big = 0, um = 0;
    IntPtr[] cells = new IntPtr[4];
    try {
      Stage("cuInit");
      Check(cuInit(0), "cuInit");
      int ver, count; Check(cuDriverGetVersion(out ver), "cuDriverGetVersion"); Check(cuDeviceGetCount(out count), "cuDeviceGetCount");
      Check(cuDeviceGet(out dev, 0), "cuDeviceGet");
      byte[] nb = new byte[256]; Check(cuDeviceGetName(nb, nb.Length, dev), "cuDeviceGetName");
      string name = System.Text.Encoding.ASCII.GetString(nb).Split('\0')[0];
      UIntPtr mem; Check(cuDeviceTotalMem(out mem, dev), "cuDeviceTotalMem");
      Say(s, String.Format("deviceQuery: driver CUDA {0}.{1}, {2} device(s); device 0 '{3}', {4} MiB, sm_{5}{6}, {7} SMs, clock {8} kHz, memory clock {9} kHz, bus {10} bit, L2 {11} B, PCI {12:x}:{13:x}, TCC {14}",
        ver / 1000, (ver % 1000) / 10, count, name, mem.ToUInt64() >> 20, Attr(dev, 75), Attr(dev, 76), Attr(dev, 16), Attr(dev, 13),
        Attr(dev, 36), Attr(dev, 37), Attr(dev, 38), Attr(dev, 33) == "n/a" ? 0 : int.Parse(Attr(dev, 33)), Attr(dev, 34) == "n/a" ? 0 : int.Parse(Attr(dev, 34)), Attr(dev, 35)));
      IntPtr c; Check(cuDevicePrimaryCtxRetain(out c, dev), "cuDevicePrimaryCtxRetain"); ctx = true;
      Check(cuCtxSetCurrent(c), "cuCtxSetCurrent");
      Stage("context current");
      byte[] image = System.Text.Encoding.ASCII.GetBytes(Ptx + "\0");
      Check(cuModuleLoadData(out module, image), "cuModuleLoadData (PTX JIT)");
      IntPtr fEmpty, fAdd, fSpin, fInc; Check(cuModuleGetFunction(out fEmpty, module, "empty"), "cuModuleGetFunction(empty)");
      Check(cuModuleGetFunction(out fInc, module, "inc"), "cuModuleGetFunction(inc)");
      Check(cuModuleGetFunction(out fSpin, module, "spin"), "cuModuleGetFunction(spin)");
      Check(cuModuleGetFunction(out fAdd, module, "vadd"), "cuModuleGetFunction(vadd)");
      Stage("module loaded");
      // Vector add of 1 Mi floats: c = a + b with a = i, b = 2i.
      int n = 1 << 20; UIntPtr bytes = (UIntPtr)(n * 4);
      Check(cuMemAlloc(out da, bytes), "cuMemAlloc"); Check(cuMemAlloc(out db, bytes), "cuMemAlloc"); Check(cuMemAlloc(out dc, bytes), "cuMemAlloc");
      float[] a = new float[n], b = new float[n], r = new float[n];
      for (int i = 0; i < n; i++) { a[i] = i; b[i] = 2 * i; }
      GCHandle ha = GCHandle.Alloc(a, GCHandleType.Pinned), hb = GCHandle.Alloc(b, GCHandleType.Pinned), hr = GCHandle.Alloc(r, GCHandleType.Pinned);
      try {
        Check(cuMemcpyHtoD(da, ha.AddrOfPinnedObject(), bytes), "cuMemcpyHtoD");
        Check(cuMemcpyHtoD(db, hb.AddrOfPinnedObject(), bytes), "cuMemcpyHtoD");
        for (int k = 0; k < 4; k++) cells[k] = Marshal.AllocHGlobal(8);
        Marshal.WriteInt64(cells[0], (long)da); Marshal.WriteInt64(cells[1], (long)db); Marshal.WriteInt64(cells[2], (long)dc); Marshal.WriteInt32(cells[3], n);
        Stage("vadd launch");
        Check(cuLaunchKernel(fAdd, (uint)((n + 255) / 256), 1, 1, 256, 1, 1, 0, IntPtr.Zero, cells, IntPtr.Zero), "cuLaunchKernel(vadd)");
        Check(cuCtxSynchronize(), "cuCtxSynchronize(vadd)");
        Check(cuMemcpyDtoH(hr.AddrOfPinnedObject(), dc, bytes), "cuMemcpyDtoH");
      } finally { ha.Free(); hb.Free(); hr.Free(); }
      for (int i = 0; i < n; i++) if (r[i] != 3f * i) throw new Exception("vadd mismatch at " + i + ": " + r[i]);
      Say(s, "vadd: 1048576 floats verified");
      Stage("vadd verified");
      // Launch cost: 1000 queued empty launches, then 100 launch + synchronize round trips.
      var clock = System.Diagnostics.Stopwatch.StartNew();
      for (int k = 0; k < 1000; k++) Check(cuLaunchKernel(fEmpty, 1, 1, 1, 1, 1, 1, 0, IntPtr.Zero, null, IntPtr.Zero), "cuLaunchKernel(empty)");
      Check(cuCtxSynchronize(), "cuCtxSynchronize");
      double queued = clock.Elapsed.TotalMilliseconds;
      clock.Restart();
      for (int k = 0; k < 100; k++) { Check(cuLaunchKernel(fEmpty, 1, 1, 1, 1, 1, 1, 0, IntPtr.Zero, null, IntPtr.Zero), "cuLaunchKernel(empty)"); Check(cuCtxSynchronize(), "cuCtxSynchronize"); }
      double sync = clock.Elapsed.TotalMilliseconds;
      Say(s, String.Format("launch: {0:F2} us per queued launch (1000), {1:F1} us per launch + synchronize (100)", queued, sync * 10));
      // NVPROBE-style: a 1000-cycle kernel, synchronize after each launch, 1000 launches.
      IntPtr cyc = Marshal.AllocHGlobal(4); Marshal.WriteInt32(cyc, 1000);
      IntPtr[] spinArgs = new IntPtr[] { cyc };
      double[] each = new double[1000];
      try {
        for (int k = 0; k < 10; k++) { Check(cuLaunchKernel(fSpin, 1, 1, 1, 1, 1, 1, 0, IntPtr.Zero, spinArgs, IntPtr.Zero), "cuLaunchKernel(spin)"); Check(cuCtxSynchronize(), "cuCtxSynchronize"); }
        var one = new System.Diagnostics.Stopwatch();
        for (int k = 0; k < 1000; k++) {
          one.Restart();
          Check(cuLaunchKernel(fSpin, 1, 1, 1, 1, 1, 1, 0, IntPtr.Zero, spinArgs, IntPtr.Zero), "cuLaunchKernel(spin)");
          Check(cuCtxSynchronize(), "cuCtxSynchronize(spin)");
          each[k] = one.Elapsed.TotalMilliseconds * 1000.0;
        }
      } finally { Marshal.FreeHGlobal(cyc); }
      double total = 0; foreach (double e in each) total += e;
      Array.Sort(each);
      Say(s, String.Format("short kernel (1000 cycles, launch + synchronize, 1000x): mean {0:F1} us, median {1:F1} us, p90 {2:F1} us, min {3:F1} us", total / 1000, each[500], each[900], each[0]));
      // Pinned copy bandwidth, 5 copies each way.
      UIntPtr bb = (UIntPtr)((ulong)mib << 20);
      Check(cuMemAllocHost(out host, bb), "cuMemAllocHost"); Check(cuMemAlloc(out big, bb), "cuMemAlloc(big)");
      Check(cuMemcpyHtoD(big, host, bb), "warm-up HtoD");
      clock.Restart(); for (int k = 0; k < 5; k++) Check(cuMemcpyHtoD(big, host, bb), "cuMemcpyHtoD"); double h2d = clock.Elapsed.TotalSeconds;
      clock.Restart(); for (int k = 0; k < 5; k++) Check(cuMemcpyDtoH(host, big, bb), "cuMemcpyDtoH"); double d2h = clock.Elapsed.TotalSeconds;
      double gb = 5.0 * mib * 1048576 / 1e9;
      Say(s, String.Format("bandwidth (pinned, {0} MiB x5): HtoD {1:F2} GB/s, DtoH {2:F2} GB/s", mib, gb / h2d, gb / d2h));
      Say(s, "PASS: CUDA deviceQuery, PTX JIT, vector add, launch and bandwidth");
      if (managed) {
        // Managed memory: the outcome is the A/B's data (a clean refusal is a result too).
        Stage("managed");
        Say(s, String.Format("managed attributes: managedMemory {0}, concurrentManagedAccess {1}, pageableMemoryAccess {2}",
          Attr(dev, 83), Attr(dev, 89), Attr(dev, 88)));
        int un = 16 << 20; UIntPtr ubytes = (UIntPtr)((ulong)un * 4);
        int ra = cuMemAllocManaged(out um, ubytes, 1);
        if (ra != 0) {
          Say(s, "MANAGED RESULT: refused: cuMemAllocManaged(64 MiB, GLOBAL) CUresult " + ra + " " + Name(ra));
        } else {
          int[] w = new int[un];
          for (int i = 0; i < un; i++) w[i] = i;
          Marshal.Copy(w, 0, (IntPtr)(long)um, un);
          Stage("managed CPU write done");
          IntPtr[] ia = new IntPtr[] { Marshal.AllocHGlobal(8), Marshal.AllocHGlobal(4) };
          try {
            Marshal.WriteInt64(ia[0], (long)um); Marshal.WriteInt32(ia[1], un);
            Check(cuLaunchKernel(fInc, (uint)((un + 255) / 256), 1, 1, 256, 1, 1, 0, IntPtr.Zero, ia, IntPtr.Zero), "cuLaunchKernel(inc)");
            int rs = cuCtxSynchronize();
            if (rs != 0) {
              Say(s, "MANAGED RESULT: error: cuCtxSynchronize after the increment CUresult " + rs + " " + Name(rs));
            } else {
              Marshal.Copy((IntPtr)(long)um, w, 0, un);
              int bad = -1;
              for (int i = 0; i < un; i++) if (w[i] != i + 1) { bad = i; break; }
              Say(s, bad < 0 ? "MANAGED RESULT: ok: 64 MiB written by the CPU, incremented by a kernel, verified by the CPU"
                             : "MANAGED RESULT: mismatch at " + bad + ": " + w[bad]);
            }
          } finally { Marshal.FreeHGlobal(ia[0]); Marshal.FreeHGlobal(ia[1]); }
        }
      }
      Passed = true;
    } catch (Exception e) {
      Say(s, "FAIL: " + e.Message);
    } finally {
      foreach (IntPtr p in cells) if (p != IntPtr.Zero) Marshal.FreeHGlobal(p);
      foreach (ulong p in new ulong[] { da, db, dc, big, um }) if (p != 0) cuMemFree(p);
      if (host != IntPtr.Zero) cuMemFreeHost(host);
      if (module != IntPtr.Zero) cuModuleUnload(module);
      if (ctx) cuDevicePrimaryCtxRelease(dev);
    }
    return s.ToString();
  }
}
}
'@
Add-Type -TypeDefinition $src -Language CSharp
$summary = [CaraxesCuda.Probe]::Run($Mib, [bool]$Managed)
"== summary"
$summary
if (-not [CaraxesCuda.Probe]::Passed) { throw "CUDA probe failed" }
