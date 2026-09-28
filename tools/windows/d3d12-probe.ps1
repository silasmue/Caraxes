# SPDX-License-Identifier: MIT
# Hardware D3D12 gate on the NVIDIA adapter: device, a queue fence signal (100 round trips timed),
# and a compute dispatch that writes 256 words into a UAV buffer, copied to a readback buffer and
# verified. Stock Windows components only (d3d12.dll, d3dcompiler_47.dll); C# 5 for PowerShell 5.1.
# Vtable slots follow d3d12.h (IUnknown 0-2, ID3D12Object 3-6, ID3D12DeviceChild 7).
$ErrorActionPreference = "Stop"
$src = @'
using System; using System.Runtime.InteropServices;
namespace CaraxesD3D12 {
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
[StructLayout(LayoutKind.Sequential)] public struct QueueDesc { public int Type; public int Priority; public int Flags; public uint NodeMask; }
[StructLayout(LayoutKind.Sequential)] public struct HeapProps { public int Type; public int CpuPage; public int Pool; public uint CreationMask; public uint VisibleMask; }
[StructLayout(LayoutKind.Sequential)] public struct ResDesc { public int Dimension; public ulong Alignment; public ulong Width; public uint Height; public ushort Depth; public ushort Mips; public int Format; public uint SampleCount; public uint SampleQuality; public int Layout; public int Flags; }
[StructLayout(LayoutKind.Sequential)] public struct PsoDesc { public IntPtr RootSig; public IntPtr Cs; public UIntPtr CsSize; public uint NodeMask; public IntPtr Cached; public UIntPtr CachedSize; public int Flags; }
[StructLayout(LayoutKind.Sequential)] public struct Barrier { public int Type; public int Flags; public IntPtr Resource; public uint Subresource; public int Before; public int After; }

public static class Probe {
  public static bool Passed = false;
  static readonly System.Diagnostics.Stopwatch Clock = System.Diagnostics.Stopwatch.StartNew();
  public static void Stage(string name) { Console.WriteLine("stage " + Clock.ElapsedMilliseconds + " ms: " + name); Console.Out.Flush(); }
  [DllImport("dxgi.dll")] static extern int CreateDXGIFactory1(ref Guid riid, [MarshalAs(UnmanagedType.IUnknown)] out object f);
  [DllImport("d3d12.dll")] static extern int D3D12CreateDevice(IntPtr adapter, int level, ref Guid riid, out IntPtr dev);
  [DllImport("d3d12.dll")] static extern int D3D12SerializeRootSignature(IntPtr desc, int version, out IntPtr blob, out IntPtr error);
  [DllImport("d3dcompiler_47.dll", CharSet = CharSet.Ansi, ExactSpelling = true)]
  static extern int D3DCompile(byte[] src, UIntPtr size, string name, IntPtr defs, IntPtr include, string entry, string target, uint flags, uint flags2, out IntPtr code, out IntPtr errors);
  [DllImport("kernel32.dll")] static extern IntPtr CreateEventW(IntPtr attrs, bool manual, bool initial, IntPtr name);
  [DllImport("kernel32.dll")] static extern uint WaitForSingleObject(IntPtr h, uint ms);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);

  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateQueueFn(IntPtr self, ref QueueDesc d, ref Guid riid, out IntPtr q);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateAllocatorFn(IntPtr self, int type, ref Guid riid, out IntPtr a);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreatePsoFn(IntPtr self, ref PsoDesc d, ref Guid riid, out IntPtr p);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateListFn(IntPtr self, uint mask, int type, IntPtr alloc, IntPtr pso, ref Guid riid, out IntPtr l);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateRootSigFn(IntPtr self, uint mask, IntPtr blob, UIntPtr size, ref Guid riid, out IntPtr r);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateResourceFn(IntPtr self, ref HeapProps h, int heapFlags, ref ResDesc d, int state, IntPtr clear, ref Guid riid, out IntPtr r);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateFenceFn(IntPtr self, ulong initial, int flags, ref Guid riid, out IntPtr f);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int RemovedReasonFn(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void ExecuteFn(IntPtr self, uint n, IntPtr[] lists);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int SignalFn(IntPtr self, IntPtr fence, ulong value);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate ulong CompletedFn(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int OnCompletionFn(IntPtr self, ulong value, IntPtr ev);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CloseFn(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void DispatchFn(IntPtr self, uint x, uint y, uint z);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void CopyBufferFn(IntPtr self, IntPtr dst, ulong dstOff, IntPtr src, ulong srcOff, ulong n);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void SetPtrFn(IntPtr self, IntPtr p);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void BarrierFn(IntPtr self, uint n, ref Barrier b);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void RootUavFn(IntPtr self, uint index, ulong va);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int MapFn(IntPtr self, uint sub, IntPtr range, out IntPtr data);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void UnmapFn(IntPtr self, uint sub, IntPtr range);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate ulong GpuVaFn(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate IntPtr BlobPtr(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate UIntPtr BlobSize(IntPtr self);

  static T Fn<T>(IntPtr obj, int slot) { return (T)(object)Marshal.GetDelegateForFunctionPointer(Marshal.ReadIntPtr(Marshal.ReadIntPtr(obj), slot * IntPtr.Size), typeof(T)); }
  static void Check(int hr, string op) { if (hr < 0) throw new Exception(op + " HRESULT 0x" + hr.ToString("x8")); }

  // ID3D12Fence 8 GetCompletedValue, 9 SetEventOnCompletion; ID3D12Device 37 GetDeviceRemovedReason.
  static void Wait(IntPtr fence, ulong value, IntPtr dev, string what) {
    if (Fn<CompletedFn>(fence, 8)(fence) >= value) return;
    IntPtr ev = CreateEventW(IntPtr.Zero, false, false, IntPtr.Zero);
    try {
      Check(Fn<OnCompletionFn>(fence, 9)(fence, value, ev), "SetEventOnCompletion");
      if (WaitForSingleObject(ev, 20000) != 0)
        throw new Exception(what + ": fence value " + value + " not reached in 20 s (completed " + Fn<CompletedFn>(fence, 8)(fence)
          + ", device removed reason 0x" + Fn<RemovedReasonFn>(dev, 37)(dev).ToString("x8") + ")");
    } finally { CloseHandle(ev); }
  }

  public static string Run() {
    var s = new System.Text.StringBuilder(); Guid g = typeof(IDXGIFactory1).GUID; object fo;
    Check(CreateDXGIFactory1(ref g, out fo), "CreateDXGIFactory1");
    var f = (IDXGIFactory1)fo;
    for (uint i = 0; ; i++) {
      object ao; if (f.EnumAdapters1(i, out ao) != 0) break;
      var a = (IDXGIAdapter1)ao; DESC1 d; a.GetDesc1(out d);
      s.AppendLine(String.Format("adapter {0}: {1} ven {2:x4} dev {3:x4} subsys {4:x8} dedicated {5} MiB", i, d.Description, d.VendorId, d.DeviceId, d.SubSysId, d.Ded.ToInt64() >> 20));
      if (d.VendorId != 0x10de || (d.Flags & 2) != 0) continue;
      IntPtr pa = Marshal.GetComInterfaceForObject(a, typeof(IDXGIAdapter1));
      try { s.AppendLine(Gate(pa)); Passed = true; }
      catch (Exception e) { s.AppendLine("FAIL: " + e.Message); }
      finally { Marshal.Release(pa); }
      break;
    }
    return s.ToString();
  }

  static string Gate(IntPtr adapter) {
    IntPtr dev = IntPtr.Zero, queue = IntPtr.Zero, fence = IntPtr.Zero, alloc = IntPtr.Zero, list = IntPtr.Zero;
    IntPtr rs = IntPtr.Zero, pso = IntPtr.Zero, uav = IntPtr.Zero, rb = IntPtr.Zero, code = IntPtr.Zero, err = IntPtr.Zero;
    IntPtr sig = IntPtr.Zero, sigErr = IntPtr.Zero, param = IntPtr.Zero, desc = IntPtr.Zero;
    try {
      Guid iid = new Guid("189819f1-1db6-4b57-be54-1821339b85f7");
      Stage("D3D12CreateDevice begin");
      Check(D3D12CreateDevice(adapter, 0xb000, ref iid, out dev), "D3D12CreateDevice(11_0)");
      Stage("D3D12CreateDevice end");
      // ID3D12Device 8 CreateCommandQueue (DIRECT), 36 CreateFence.
      QueueDesc qd = new QueueDesc();
      iid = new Guid("0ec870a6-5d7e-4c22-8cfc-5baae07616ed");
      Check(Fn<CreateQueueFn>(dev, 8)(dev, ref qd, ref iid, out queue), "CreateCommandQueue");
      iid = new Guid("0a753dcf-c4d8-4b91-adf6-be5a60d95a76");
      Check(Fn<CreateFenceFn>(dev, 36)(dev, 0, 0, ref iid, out fence), "CreateFence");
      // 1. An empty queue signal: the GPU writes the fence value; no shader. Queue 14 Signal.
      Stage("fence signal 1");
      Check(Fn<SignalFn>(queue, 14)(queue, fence, 1), "Signal");
      Wait(fence, 1, dev, "queue signal");
      Stage("fence 1 reached");
      var clock = System.Diagnostics.Stopwatch.StartNew();
      for (ulong v = 2; v < 102; v++) { Check(Fn<SignalFn>(queue, 14)(queue, fence, v), "Signal"); Wait(fence, v, dev, "signal round trip"); }
      double roundTripUs = clock.Elapsed.TotalMilliseconds * 1000.0 / 100;
      // 2. Compute: 4 groups of 64 threads store 3i+1 at word i of a root UAV.
      byte[] hlsl = System.Text.Encoding.ASCII.GetBytes("RWByteAddressBuffer o : register(u0); [numthreads(64,1,1)] void cs(uint3 i : SV_DispatchThreadID) { o.Store(i.x * 4, i.x * 3 + 1); }");
      int hr = D3DCompile(hlsl, (UIntPtr)hlsl.Length, "caraxes-d3d12", IntPtr.Zero, IntPtr.Zero, "cs", "cs_5_0", 0, 0, out code, out err);
      if (hr < 0 && err != IntPtr.Zero) throw new Exception("D3DCompile: " + Marshal.PtrToStringAnsi(Fn<BlobPtr>(err, 3)(err)));
      Check(hr, "D3DCompile");
      // D3D12_ROOT_PARAMETER (32 bytes): type 4 (UAV) @0, register u0 space 0 @8, visibility ALL @24.
      param = Marshal.AllocHGlobal(32); for (int k = 0; k < 32; k += 4) Marshal.WriteInt32(param, k, 0);
      Marshal.WriteInt32(param, 0, 4);
      // D3D12_ROOT_SIGNATURE_DESC (40 bytes): 1 parameter @0, pParameters @8, no samplers, flags 0.
      desc = Marshal.AllocHGlobal(40); for (int k = 0; k < 40; k += 4) Marshal.WriteInt32(desc, k, 0);
      Marshal.WriteInt32(desc, 0, 1); Marshal.WriteIntPtr(desc, 8, param);
      hr = D3D12SerializeRootSignature(desc, 1, out sig, out sigErr);
      if (hr < 0 && sigErr != IntPtr.Zero) throw new Exception("D3D12SerializeRootSignature: " + Marshal.PtrToStringAnsi(Fn<BlobPtr>(sigErr, 3)(sigErr)));
      Check(hr, "D3D12SerializeRootSignature");
      // ID3D12Device 16 CreateRootSignature, 11 CreateComputePipelineState, 27 CreateCommittedResource,
      // 9 CreateCommandAllocator, 12 CreateCommandList.
      iid = new Guid("c54a6b66-72df-4ee8-8be5-a946a1429214");
      Check(Fn<CreateRootSigFn>(dev, 16)(dev, 0, Fn<BlobPtr>(sig, 3)(sig), Fn<BlobSize>(sig, 4)(sig), ref iid, out rs), "CreateRootSignature");
      PsoDesc pd = new PsoDesc(); pd.RootSig = rs; pd.Cs = Fn<BlobPtr>(code, 3)(code); pd.CsSize = Fn<BlobSize>(code, 4)(code);
      iid = new Guid("765a30f3-f624-4c6f-a828-ace948622445");
      Check(Fn<CreatePsoFn>(dev, 11)(dev, ref pd, ref iid, out pso), "CreateComputePipelineState");
      HeapProps hp = new HeapProps(); hp.Type = 1;                  // DEFAULT
      ResDesc rd = new ResDesc(); rd.Dimension = 1; rd.Width = 1024; rd.Height = 1; rd.Depth = 1; rd.Mips = 1;
      rd.SampleCount = 1; rd.Layout = 1; rd.Flags = 4;              // BUFFER, ROW_MAJOR, ALLOW_UNORDERED_ACCESS
      iid = new Guid("696442be-a72e-4059-bc79-5b5c98040fad");
      Check(Fn<CreateResourceFn>(dev, 27)(dev, ref hp, 0, ref rd, 0, IntPtr.Zero, ref iid, out uav), "CreateCommittedResource(uav)");
      hp.Type = 3; rd.Flags = 0;                                    // READBACK, created in COPY_DEST
      Check(Fn<CreateResourceFn>(dev, 27)(dev, ref hp, 0, ref rd, 0x400, IntPtr.Zero, ref iid, out rb), "CreateCommittedResource(readback)");
      iid = new Guid("6102dee4-af59-4b09-b999-b44d73f09b24");
      Check(Fn<CreateAllocatorFn>(dev, 9)(dev, 0, ref iid, out alloc), "CreateCommandAllocator");
      iid = new Guid("5b160d0f-ac1b-4185-8ba8-b3ae42a5a455");
      Check(Fn<CreateListFn>(dev, 12)(dev, 0, 0, alloc, pso, ref iid, out list), "CreateCommandList");
      // ID3D12GraphicsCommandList 29 SetComputeRootSignature, 41 SetComputeRootUnorderedAccessView,
      // 14 Dispatch, 26 ResourceBarrier, 15 CopyBufferRegion, 9 Close; ID3D12Resource 11 GetGPUVirtualAddress.
      Fn<SetPtrFn>(list, 29)(list, rs);
      Fn<RootUavFn>(list, 41)(list, 0, Fn<GpuVaFn>(uav, 11)(uav));
      Fn<DispatchFn>(list, 14)(list, 4, 1, 1);
      Barrier b = new Barrier(); b.Resource = uav; b.Subresource = 0xffffffff; b.Before = 0x8; b.After = 0x800;
      Fn<BarrierFn>(list, 26)(list, 1, ref b);
      Fn<CopyBufferFn>(list, 15)(list, rb, 0, uav, 0, 1024);
      Check(Fn<CloseFn>(list, 9)(list), "Close");
      Stage("dispatch submit");
      clock.Restart();
      Fn<ExecuteFn>(queue, 10)(queue, 1, new IntPtr[] { list });
      Check(Fn<SignalFn>(queue, 14)(queue, fence, 200), "Signal");
      Wait(fence, 200, dev, "compute dispatch");
      double dispatchMs = clock.Elapsed.TotalMilliseconds;
      Stage("dispatch complete");
      // ID3D12Resource 8 Map, 9 Unmap.
      IntPtr data; Check(Fn<MapFn>(rb, 8)(rb, 0, IntPtr.Zero, out data), "Map(readback)");
      int[] words = new int[256];
      try { Marshal.Copy(data, words, 0, 256); } finally { Fn<UnmapFn>(rb, 9)(rb, 0, IntPtr.Zero); }
      for (int k = 0; k < 256; k++)
        if (words[k] != k * 3 + 1) throw new Exception("readback word " + k + " = " + words[k] + ", expected " + (k * 3 + 1));
      Stage("readback verified");
      return String.Format("PASS: NVIDIA D3D12 device, fence signal round trip {0:F0} us (avg of 100), compute dispatch of 256 words verified in {1:F1} ms", roundTripUs, dispatchMs);
    } finally {
      foreach (IntPtr p in new IntPtr[] { list, alloc, rb, uav, pso, rs, sig, sigErr, code, err, fence, queue, dev })
        if (p != IntPtr.Zero) Marshal.Release(p);
      if (param != IntPtr.Zero) Marshal.FreeHGlobal(param);
      if (desc != IntPtr.Zero) Marshal.FreeHGlobal(desc);
    }
  }
}
}
'@
Add-Type -TypeDefinition $src -Language CSharp
[CaraxesD3D12.Probe]::Run()
if (-not [CaraxesD3D12.Probe]::Passed) { throw "No NVIDIA hardware adapter passed the D3D12 gate" }
