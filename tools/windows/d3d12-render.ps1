# SPDX-License-Identifier: MIT
# Diagnostic, not a benchmark: draw an offscreen triangle using a real constant buffer,
# change that buffer after GPU completion, and verify both RGBA readbacks on NVIDIA hardware.
# C# 5 / 64-bit Windows PowerShell 5.1; stock dxgi, d3d12 and d3dcompiler_47 DLLs only.
# ABI source: Microsoft's DirectX-Headers include/directx/d3d12.h, including its _WIN32
# descriptor-handle return-buffer signatures. No external sample implementation is used.
# https://github.com/microsoft/DirectX-Headers/blob/main/include/directx/d3d12.h
# https://learn.microsoft.com/en-us/windows/win32/api/d3d12/ns-d3d12-d3d12_graphics_pipeline_state_desc
param(
    [ValidateSet('root', 'heap', 'both')][string]$Binding = 'both',
    [ValidateRange(1, 60)][int]$FenceTimeoutSeconds = 20
)
$ErrorActionPreference = 'Stop'
$src = @'
using System;
using System.Runtime.InteropServices;
namespace CaraxesD3D12Render {
[ComImport, Guid("770aae78-f26f-4dba-a829-253c83d1b387"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IDXGIFactory1 {
  void SetPrivateData(); void SetPrivateDataInterface(); void GetPrivateData(); void GetParent();
  void EnumAdapters(); void MakeWindowAssociation(); void GetWindowAssociation(); void CreateSwapChain(); void CreateSoftwareAdapter();
  [PreserveSig] int EnumAdapters1(uint i, [MarshalAs(UnmanagedType.IUnknown)] out object a);
}
[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
public struct AdapterDesc {
  [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string Description;
  public uint VendorId, DeviceId, SubSysId, Revision;
  public IntPtr Dedicated, DedicatedSystem, Shared;
  public uint LuidLo; public int LuidHi; public uint Flags;
}
[ComImport, Guid("29038f61-3839-4626-91fd-086879011a05"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IDXGIAdapter1 {
  void SetPrivateData(); void SetPrivateDataInterface(); void GetPrivateData(); void GetParent();
  void EnumOutputs(); void GetDesc(); void CheckInterfaceSupport();
  [PreserveSig] int GetDesc1(out AdapterDesc d);
}
[StructLayout(LayoutKind.Sequential)] struct QueueDesc { public int Type, Priority, Flags; public uint NodeMask; }
[StructLayout(LayoutKind.Sequential)] struct HeapProps { public int Type, CpuPage, Pool; public uint CreationMask, VisibleMask; }
[StructLayout(LayoutKind.Sequential)] struct ResourceDesc {
  public int Dimension; public ulong Alignment, Width; public uint Height;
  public ushort Depth, Mips; public int Format; public uint SampleCount, SampleQuality; public int Layout, Flags;
}
[StructLayout(LayoutKind.Sequential)] struct HeapDesc { public int Type; public uint Count; public int Flags; public uint NodeMask; }
[StructLayout(LayoutKind.Sequential)] struct Bytecode { public IntPtr Data; public UIntPtr Size; }
[StructLayout(LayoutKind.Sequential)] struct StreamOutput { public IntPtr Entries; public uint Count; public IntPtr Strides; public uint StrideCount, RasterizedStream; }
[StructLayout(LayoutKind.Sequential)] struct TargetBlend {
  public int Enable, LogicEnable, Src, Dst, Op, SrcAlpha, DstAlpha, AlphaOp, LogicOp;
  public byte WriteMask;
}
[StructLayout(LayoutKind.Sequential)] struct Blend {
  public int AlphaToCoverage, Independent;
  [MarshalAs(UnmanagedType.ByValArray, SizeConst = 8)] public TargetBlend[] Targets;
}
[StructLayout(LayoutKind.Sequential)] struct Rasterizer {
  public int Fill, Cull, FrontCCW, DepthBias; public float DepthBiasClamp, SlopeBias;
  public int DepthClip, Multisample, AntialiasedLine; public uint ForcedSamples; public int Conservative;
}
[StructLayout(LayoutKind.Sequential)] struct StencilFace { public int Fail, DepthFail, Pass, Func; }
[StructLayout(LayoutKind.Sequential)] struct DepthStencil {
  public int DepthEnable, WriteMask, DepthFunc, StencilEnable; public byte ReadMask, StencilMask;
  public StencilFace Front, Back;
}
[StructLayout(LayoutKind.Sequential)] struct InputLayout { public IntPtr Elements; public uint Count; }
[StructLayout(LayoutKind.Sequential)] struct GraphicsPso {
  public IntPtr Root;
  public Bytecode VS, PS, DS, HS, GS;
  public StreamOutput Stream;
  public Blend Blend;
  public uint SampleMask;
  public Rasterizer Rasterizer;
  public DepthStencil Depth;
  public InputLayout Input;
  public int StripCut, Topology; public uint TargetCount;
  [MarshalAs(UnmanagedType.ByValArray, SizeConst = 8)] public int[] Formats;
  public int DepthFormat; public uint Samples, Quality, NodeMask;
  public Bytecode Cached; public int Flags;
}
[StructLayout(LayoutKind.Explicit, Size = 32)] struct RootParameter {
  [FieldOffset(0)] public int Type;
  [FieldOffset(8)] public uint RegisterOrRangeCount;
  [FieldOffset(12)] public uint Space;
  [FieldOffset(16)] public IntPtr Ranges;
  [FieldOffset(24)] public int Visibility;
}
[StructLayout(LayoutKind.Sequential)] struct RootDesc { public uint Count; public IntPtr Parameters; public uint SamplerCount; public IntPtr Samplers; public int Flags; }
[StructLayout(LayoutKind.Sequential)] struct DescriptorRange { public int Type; public uint Count, Register, Space, Offset; }
[StructLayout(LayoutKind.Sequential)] struct CbvDesc { public ulong Address; public uint Size; }
[StructLayout(LayoutKind.Sequential)] struct Barrier { public int Type, Flags; public IntPtr Resource; public uint Subresource; public int Before, After; }
[StructLayout(LayoutKind.Sequential)] struct Footprint { public ulong Offset; public int Format; public uint Width, Height, Depth, RowPitch; }
[StructLayout(LayoutKind.Explicit, Size = 48)] struct CopyLocation {
  [FieldOffset(0)] public IntPtr Resource;
  [FieldOffset(8)] public int Type;
  [FieldOffset(16)] public Footprint Placed;
  [FieldOffset(16)] public uint Subresource;
}
[StructLayout(LayoutKind.Sequential)] struct Viewport { public float X, Y, W, H, Min, Max; }
[StructLayout(LayoutKind.Sequential)] struct Rect { public int Left, Top, Right, Bottom; }
[StructLayout(LayoutKind.Sequential)] struct Range { public UIntPtr Begin, End; }

public static class Probe {
  public static bool Passed;
  static readonly System.Diagnostics.Stopwatch Clock = System.Diagnostics.Stopwatch.StartNew();
  static void Stage(string name) { Console.WriteLine("stage " + Clock.ElapsedMilliseconds + " ms: " + name); Console.Out.Flush(); }
  [DllImport("dxgi.dll")] static extern int CreateDXGIFactory1(ref Guid iid, [MarshalAs(UnmanagedType.IUnknown)] out object f);
  [DllImport("d3d12.dll")] static extern int D3D12CreateDevice(IntPtr adapter, int level, ref Guid iid, out IntPtr device);
  [DllImport("d3d12.dll")] static extern int D3D12SerializeRootSignature(ref RootDesc desc, int version, out IntPtr blob, out IntPtr error);
  [DllImport("d3dcompiler_47.dll", CharSet = CharSet.Ansi, ExactSpelling = true)]
  static extern int D3DCompile(byte[] source, UIntPtr size, string name, IntPtr defines, IntPtr include, string entry, string target, uint flags, uint flags2, out IntPtr code, out IntPtr errors);
  [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr CreateEventW(IntPtr attrs, bool manual, bool initial, IntPtr name);
  [DllImport("kernel32.dll", SetLastError = true)] static extern uint WaitForSingleObject(IntPtr handle, uint ms);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateQueueFn(IntPtr self, ref QueueDesc d, ref Guid iid, out IntPtr q);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateAllocatorFn(IntPtr self, int type, ref Guid iid, out IntPtr a);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreatePsoFn(IntPtr self, ref GraphicsPso d, ref Guid iid, out IntPtr p);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateListFn(IntPtr self, uint mask, int type, IntPtr a, IntPtr p, ref Guid iid, out IntPtr l);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateRootFn(IntPtr self, uint mask, IntPtr data, UIntPtr size, ref Guid iid, out IntPtr r);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateResourceFn(IntPtr self, ref HeapProps h, int flags, ref ResourceDesc d, int state, IntPtr clear, ref Guid iid, out IntPtr r);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateHeapFn(IntPtr self, ref HeapDesc d, ref Guid iid, out IntPtr h);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateFenceFn(IntPtr self, ulong initial, int flags, ref Guid iid, out IntPtr f);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void CreateRtvFn(IntPtr self, IntPtr resource, IntPtr desc, ulong cpuHandle);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void CreateCbvFn(IntPtr self, ref CbvDesc desc, ulong cpuHandle);
  // Windows C-style d3d12.h explicitly supplies a return-buffer argument for these getters.
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate IntPtr HeapHandleFn(IntPtr self, out ulong handle);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void FootprintsFn(IntPtr self, ref ResourceDesc d, uint first, uint count, ulong offset, out Footprint layout, out uint rows, out ulong rowBytes, out ulong total);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void ExecuteFn(IntPtr self, uint count, [In] IntPtr[] lists);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int SignalFn(IntPtr self, IntPtr fence, ulong value);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate ulong CompletedFn(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int OnCompletionFn(IntPtr self, ulong value, IntPtr ev);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int RemovedFn(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CloseFn(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int AllocatorResetFn(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int ListResetFn(IntPtr self, IntPtr allocator, IntPtr pso);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void SetPtrFn(IntPtr self, IntPtr value);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void SetRootFn(IntPtr self, uint index, ulong address);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void HeapsFn(IntPtr self, uint count, [In] IntPtr[] heaps);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void ViewportsFn(IntPtr self, uint count, ref Viewport v);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void ScissorsFn(IntPtr self, uint count, ref Rect r);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void TargetsFn(IntPtr self, uint count, ref ulong handle, int contiguous, IntPtr depth);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void ClearRtvFn(IntPtr self, ulong handle, [In] float[] color, uint count, IntPtr rects);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void TopologyFn(IntPtr self, int topology);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void DrawFn(IntPtr self, uint vertices, uint instances, uint start, uint firstInstance);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void BarrierFn(IntPtr self, uint count, ref Barrier b);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void CopyTextureFn(IntPtr self, ref CopyLocation dst, uint x, uint y, uint z, ref CopyLocation src, IntPtr box);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int MapFn(IntPtr self, uint subresource, ref Range range, out IntPtr data);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void UnmapFn(IntPtr self, uint subresource, ref Range range);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate ulong GpuVaFn(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate IntPtr BlobPtrFn(IntPtr self);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate UIntPtr BlobSizeFn(IntPtr self);
  static T Fn<T>(IntPtr obj, int slot) { return (T)(object)Marshal.GetDelegateForFunctionPointer(Marshal.ReadIntPtr(Marshal.ReadIntPtr(obj), slot * IntPtr.Size), typeof(T)); }
  static void Check(int hr, string op) { if (hr < 0) throw new Exception(op + " HRESULT 0x" + hr.ToString("x8")); }
  static void Size(Type type, int expected) { if (Marshal.SizeOf(type) != expected) throw new Exception("ABI size " + type.Name + " = " + Marshal.SizeOf(type) + ", expected " + expected); }
  static void Offset(Type type, string field, int expected) { if (Marshal.OffsetOf(type, field).ToInt32() != expected) throw new Exception("ABI offset " + type.Name + "." + field); }
  static void VerifyAbi() {
    if (IntPtr.Size != 8) throw new Exception("Run in 64-bit Windows PowerShell; this gate uses the Windows x64 ABI");
    Size(typeof(ResourceDesc), 56); Size(typeof(HeapProps), 20); Size(typeof(HeapDesc), 16);
    Size(typeof(TargetBlend), 40); Size(typeof(Blend), 328); Size(typeof(Rasterizer), 44); Size(typeof(DepthStencil), 52);
    Size(typeof(GraphicsPso), 656); Size(typeof(RootParameter), 32); Size(typeof(RootDesc), 40);
    Size(typeof(DescriptorRange), 20); Size(typeof(CbvDesc), 16); Size(typeof(Barrier), 32);
    Size(typeof(Footprint), 32); Size(typeof(CopyLocation), 48); Size(typeof(Range), 16);
    Offset(typeof(GraphicsPso), "Blend", 120); Offset(typeof(GraphicsPso), "Rasterizer", 452);
    Offset(typeof(GraphicsPso), "Depth", 496); Offset(typeof(GraphicsPso), "Input", 552);
    Offset(typeof(GraphicsPso), "Formats", 580); Offset(typeof(GraphicsPso), "Cached", 632);
  }
  static IntPtr Compile(string entry, string target) {
    // Both stages consume b0: transform in VS, tint in PS. Color cannot be baked into DXBC.
    byte[] source = System.Text.Encoding.ASCII.GetBytes(
      "cbuffer Data : register(b0) { float4 tint; float4 transform; }; " +
      "float4 vs(uint i:SV_VertexID):SV_Position { " +
      "float2 p = i == 0 ? float2(-0.75,-0.75) : (i == 1 ? float2(0,0.75) : float2(0.75,-0.75)); " +
      "return float4(p * transform.xy + transform.zw, 0, 1); } " +
      "float4 ps():SV_Target { return tint; }");
    IntPtr code = IntPtr.Zero, errors = IntPtr.Zero;
    try {
      int hr = D3DCompile(source, (UIntPtr)source.Length, "caraxes-d3d12-render", IntPtr.Zero, IntPtr.Zero, entry, target, 0x800, 0, out code, out errors);
      if (hr < 0 && errors != IntPtr.Zero) throw new Exception("D3DCompile " + entry + ": " + Marshal.PtrToStringAnsi(Fn<BlobPtrFn>(errors, 3)(errors)));
      Check(hr, "D3DCompile " + entry);
      IntPtr result = code; code = IntPtr.Zero; return result;
    } finally { if (errors != IntPtr.Zero) Marshal.Release(errors); if (code != IntPtr.Zero) Marshal.Release(code); }
  }
  static IntPtr Resource(IntPtr device, int heap, ResourceDesc desc, int state) {
    HeapProps props = new HeapProps { Type = heap, CreationMask = 1, VisibleMask = 1 };
    Guid iid = new Guid("696442be-a72e-4059-bc79-5b5c98040fad"); IntPtr resource;
    Check(Fn<CreateResourceFn>(device, 27)(device, ref props, 0, ref desc, state, IntPtr.Zero, ref iid, out resource), "CreateCommittedResource heap " + heap);
    return resource;
  }
  static IntPtr DescriptorHeap(IntPtr device, int type, bool visible) {
    HeapDesc desc = new HeapDesc { Type = type, Count = 1, Flags = visible ? 1 : 0 };
    Guid iid = new Guid("8efb471d-616c-4f49-90f7-127bb763fa51"); IntPtr heap;
    Check(Fn<CreateHeapFn>(device, 14)(device, ref desc, ref iid, out heap), "CreateDescriptorHeap type " + type); return heap;
  }
  static IntPtr RootSignature(IntPtr device, bool table) {
    IntPtr parameter = IntPtr.Zero, range = IntPtr.Zero, blob = IntPtr.Zero, errors = IntPtr.Zero;
    try {
      RootParameter p = new RootParameter { Type = table ? 0 : 2, Visibility = 0 };
      if (table) {
        DescriptorRange r = new DescriptorRange { Type = 2, Count = 1 }; // one CBV at b0, table offset 0
        range = Marshal.AllocHGlobal(20); Marshal.StructureToPtr(r, range, false);
        p.RegisterOrRangeCount = 1; p.Ranges = range;
      }
      parameter = Marshal.AllocHGlobal(32); Marshal.StructureToPtr(p, parameter, false);
      RootDesc desc = new RootDesc { Count = 1, Parameters = parameter, Flags = 1 };
      int hr = D3D12SerializeRootSignature(ref desc, 1, out blob, out errors);
      if (hr < 0 && errors != IntPtr.Zero) throw new Exception("SerializeRootSignature: " + Marshal.PtrToStringAnsi(Fn<BlobPtrFn>(errors, 3)(errors)));
      Check(hr, "D3D12SerializeRootSignature");
      Guid iid = new Guid("c54a6b66-72df-4ee8-8be5-a946a1429214"); IntPtr root;
      Check(Fn<CreateRootFn>(device, 16)(device, 0, Fn<BlobPtrFn>(blob, 3)(blob), Fn<BlobSizeFn>(blob, 4)(blob), ref iid, out root), "CreateRootSignature"); return root;
    } finally {
      if (blob != IntPtr.Zero) Marshal.Release(blob); if (errors != IntPtr.Zero) Marshal.Release(errors);
      if (parameter != IntPtr.Zero) Marshal.FreeHGlobal(parameter); if (range != IntPtr.Zero) Marshal.FreeHGlobal(range);
    }
  }
  static void Wait(IntPtr fence, ulong value, IntPtr device, uint timeout, string label) {
    ulong completed = Fn<CompletedFn>(fence, 8)(fence);
    // UINT64_MAX means device removal, not successful completion.
    if (completed == ulong.MaxValue) throw new Exception(label + ": device removed 0x" + Fn<RemovedFn>(device, 37)(device).ToString("x8"));
    if (completed >= value) return;
    IntPtr ev = CreateEventW(IntPtr.Zero, false, false, IntPtr.Zero);
    if (ev == IntPtr.Zero) throw new Exception("CreateEventW error " + Marshal.GetLastWin32Error());
    try {
      Check(Fn<OnCompletionFn>(fence, 9)(fence, value, ev), "SetEventOnCompletion");
      uint result = WaitForSingleObject(ev, timeout);
      completed = Fn<CompletedFn>(fence, 8)(fence);
      int removed = Fn<RemovedFn>(device, 37)(device);
      if (result != 0 || completed < value || completed == ulong.MaxValue || removed < 0)
        throw new Exception(label + ": fence " + value + " failed after bounded wait " + timeout + " ms; wait 0x" + result.ToString("x8") + ", completed " + completed + ", removal 0x" + removed.ToString("x8"));
    } finally { CloseHandle(ev); }
  }
  static bool Pixel(byte[] pixels, int offset, int r, int g, int b) {
    return Math.Abs(pixels[offset] - r) <= 1 && Math.Abs(pixels[offset + 1] - g) <= 1 && Math.Abs(pixels[offset + 2] - b) <= 1 && pixels[offset + 3] == 255;
  }
  static int VerifyPixels(byte[] pixels, int pitch, int r, int g, int b, string label) {
    int colored = 0;
    for (int y = 0; y < 64; y++) for (int x = 0; x < 64; x++) {
      int at = y * pitch + x * 4;
      if (Pixel(pixels, at, r, g, b)) colored++;
      else if (!Pixel(pixels, at, 0, 0, 0))
        throw new Exception(label + ": unexpected RGBA at " + x + "," + y + " = " + pixels[at] + "," + pixels[at + 1] + "," + pixels[at + 2] + "," + pixels[at + 3]);
    }
    if (colored < 800 || colored > 1400 || !Pixel(pixels, 32 * pitch + 32 * 4, r, g, b) || !Pixel(pixels, 0, 0, 0, 0))
      throw new Exception(label + ": triangle coverage/center/background invalid; colored pixels " + colored);
    return colored;
  }
  static void Render(IntPtr device, IntPtr queue, IntPtr fence, ref ulong fenceValue, bool table, uint timeout) {
    string mode = table ? "descriptor-heap CBV" : "root CBV";
    IntPtr root = IntPtr.Zero, vs = IntPtr.Zero, ps = IntPtr.Zero, pso = IntPtr.Zero, target = IntPtr.Zero;
    IntPtr readback = IntPtr.Zero, cb = IntPtr.Zero, rtvHeap = IntPtr.Zero, cbHeap = IntPtr.Zero, allocator = IntPtr.Zero, list = IntPtr.Zero;
    try {
      Stage(mode + " resources");
      root = RootSignature(device, table); vs = Compile("vs", "vs_5_0"); ps = Compile("ps", "ps_5_0");
      TargetBlend blend = new TargetBlend { Src = 2, Dst = 1, Op = 1, SrcAlpha = 2, DstAlpha = 1, AlphaOp = 1, LogicOp = 5, WriteMask = 15 };
      StencilFace stencil = new StencilFace { Fail = 1, DepthFail = 1, Pass = 1, Func = 8 };
      GraphicsPso desc = new GraphicsPso {
        Root = root, VS = new Bytecode { Data = Fn<BlobPtrFn>(vs, 3)(vs), Size = Fn<BlobSizeFn>(vs, 4)(vs) },
        PS = new Bytecode { Data = Fn<BlobPtrFn>(ps, 3)(ps), Size = Fn<BlobSizeFn>(ps, 4)(ps) },
        Blend = new Blend { Targets = new TargetBlend[8] }, SampleMask = uint.MaxValue,
        Rasterizer = new Rasterizer { Fill = 3, Cull = 1, DepthClip = 1 },
        Depth = new DepthStencil { DepthFunc = 2, ReadMask = 255, StencilMask = 255, Front = stencil, Back = stencil },
        Topology = 3, TargetCount = 1, Formats = new int[8], Samples = 1
      };
      for (int i = 0; i < 8; i++) desc.Blend.Targets[i] = blend;
      desc.Formats[0] = 28; // R8G8B8A8_UNORM
      Guid iid = new Guid("765a30f3-f624-4c6f-a828-ace948622445");
      Check(Fn<CreatePsoFn>(device, 10)(device, ref desc, ref iid, out pso), "CreateGraphicsPipelineState " + mode);
      ResourceDesc texture = new ResourceDesc { Dimension = 3, Width = 64, Height = 64, Depth = 1, Mips = 1, Format = 28, SampleCount = 1, Flags = 1 };
      target = Resource(device, 1, texture, 4); // DEFAULT texture, ALLOW_RENDER_TARGET, RENDER_TARGET
      Footprint footprint; uint rows; ulong rowBytes, total;
      Fn<FootprintsFn>(device, 38)(device, ref texture, 0, 1, 0, out footprint, out rows, out rowBytes, out total);
      if (rows != 64 || rowBytes != 256 || footprint.Offset != 0 || footprint.RowPitch < 256 || footprint.RowPitch % 256 != 0 || total < (ulong)footprint.RowPitch * 64 || total > 1048576)
        throw new Exception("Unexpected copy footprint: rows " + rows + ", row bytes " + rowBytes + ", pitch " + footprint.RowPitch + ", total " + total);
      ResourceDesc buffer = new ResourceDesc { Dimension = 1, Width = total, Height = 1, Depth = 1, Mips = 1, SampleCount = 1, Layout = 1 };
      readback = Resource(device, 3, buffer, 0x400); // READBACK buffer, COPY_DEST
      buffer.Width = 256; cb = Resource(device, 2, buffer, 0xac3); // UPLOAD, GENERIC_READ, one aligned CB
      ulong cbAddress = Fn<GpuVaFn>(cb, 11)(cb);
      if (cbAddress == 0 || cbAddress % 256 != 0) throw new Exception("Invalid constant-buffer GPU VA 0x" + cbAddress.ToString("x"));
      Console.WriteLine(mode + ": constant buffer GPU VA 0x" + cbAddress.ToString("x") + ", 256 bytes; VS and PS read b0");
      rtvHeap = DescriptorHeap(device, 2, false); ulong rtv;
      Fn<HeapHandleFn>(rtvHeap, 9)(rtvHeap, out rtv);
      if (rtv == 0) throw new Exception("Null RTV CPU descriptor handle");
      Fn<CreateRtvFn>(device, 20)(device, target, IntPtr.Zero, rtv);
      ulong gpuHandle = 0;
      if (table) {
        cbHeap = DescriptorHeap(device, 0, true); ulong cpuHandle;
        Fn<HeapHandleFn>(cbHeap, 9)(cbHeap, out cpuHandle); Fn<HeapHandleFn>(cbHeap, 10)(cbHeap, out gpuHandle);
        if (cpuHandle == 0 || gpuHandle == 0) throw new Exception("Null CBV descriptor handle");
        CbvDesc view = new CbvDesc { Address = cbAddress, Size = 256 };
        Fn<CreateCbvFn>(device, 17)(device, ref view, cpuHandle);
      }
      iid = new Guid("6102dee4-af59-4b09-b999-b44d73f09b24");
      Check(Fn<CreateAllocatorFn>(device, 9)(device, 0, ref iid, out allocator), "CreateCommandAllocator");
      iid = new Guid("5b160d0f-ac1b-4185-8ba8-b3ae42a5a455");
      Check(Fn<CreateListFn>(device, 12)(device, 0, 0, allocator, pso, ref iid, out list), "CreateCommandList");
      byte[] previous = null;
      for (int frame = 0; frame < 2; frame++) {
        string label = mode + " frame " + frame;
        Stage(label + " record");
        if (frame != 0) {
          // The preceding fence completed before allocator reset and CPU buffer modification.
          Check(Fn<AllocatorResetFn>(allocator, 8)(allocator), "Reset allocator");
          Check(Fn<ListResetFn>(list, 10)(list, allocator, pso), "Reset list");
        }
        Range none = new Range(); IntPtr mapped;
        Check(Fn<MapFn>(cb, 8)(cb, 0, ref none, out mapped), "Map constant buffer");
        Range written = new Range { End = (UIntPtr)32 };
        try { Marshal.Copy(new float[] { frame == 0 ? 0.8f : 0.2f, frame == 0 ? 0.2f : 0.8f, 0.4f, 1, 1, 1, 0, 0 }, 0, mapped, 8); }
        finally { Fn<UnmapFn>(cb, 9)(cb, 0, ref written); }
        if (frame != 0) {
          Barrier toRender = new Barrier { Resource = target, Subresource = uint.MaxValue, Before = 0x800, After = 4 };
          Fn<BarrierFn>(list, 26)(list, 1, ref toRender);
        }
        Fn<SetPtrFn>(list, 30)(list, root);
        if (table) { Fn<HeapsFn>(list, 28)(list, 1, new IntPtr[] { cbHeap }); Fn<SetRootFn>(list, 32)(list, 0, gpuHandle); }
        else Fn<SetRootFn>(list, 38)(list, 0, cbAddress);
        Viewport viewport = new Viewport { W = 64, H = 64, Max = 1 }; Rect scissor = new Rect { Right = 64, Bottom = 64 };
        Fn<ViewportsFn>(list, 21)(list, 1, ref viewport); Fn<ScissorsFn>(list, 22)(list, 1, ref scissor);
        Fn<TargetsFn>(list, 46)(list, 1, ref rtv, 1, IntPtr.Zero);
        Fn<ClearRtvFn>(list, 48)(list, rtv, new float[] { 0, 0, 0, 1 }, 0, IntPtr.Zero);
        Fn<TopologyFn>(list, 20)(list, 4); // TRIANGLELIST
        Fn<DrawFn>(list, 12)(list, 3, 1, 0, 0);
        Barrier toCopy = new Barrier { Resource = target, Subresource = uint.MaxValue, Before = 4, After = 0x800 };
        Fn<BarrierFn>(list, 26)(list, 1, ref toCopy);
        CopyLocation dst = new CopyLocation { Resource = readback, Type = 1, Placed = footprint };
        CopyLocation src = new CopyLocation { Resource = target, Type = 0, Subresource = 0 };
        Fn<CopyTextureFn>(list, 16)(list, ref dst, 0, 0, 0, ref src, IntPtr.Zero);
        Check(Fn<CloseFn>(list, 9)(list), "Close " + label);
        Stage(label + " submit");
        Fn<ExecuteFn>(queue, 10)(queue, 1, new IntPtr[] { list });
        Check(Fn<SignalFn>(queue, 14)(queue, fence, ++fenceValue), "Signal " + label);
        Wait(fence, fenceValue, device, timeout, label);
        Stage(label + " fence reached");
        Range read = new Range { End = (UIntPtr)total };
        Check(Fn<MapFn>(readback, 8)(readback, 0, ref read, out mapped), "Map readback");
        byte[] pixels = new byte[(int)total];
        try { Marshal.Copy(mapped, pixels, 0, pixels.Length); } finally { Fn<UnmapFn>(readback, 9)(readback, 0, ref none); }
        int colored = VerifyPixels(pixels, (int)footprint.RowPitch, frame == 0 ? 204 : 51, frame == 0 ? 51 : 204, 102, label);
        if (previous != null) {
          int changed = 0;
          for (int y = 0; y < 64; y++) for (int x = 0; x < 64; x++) {
            int at = y * (int)footprint.RowPitch + x * 4;
            if (pixels[at] != previous[at] || pixels[at + 1] != previous[at + 1]) changed++;
          }
          if (changed != colored) throw new Exception(label + ": changed pixels " + changed + ", expected " + colored);
          Console.WriteLine(label + ": " + changed + " pixels changed after constant-buffer update");
        }
        previous = pixels;
        Console.WriteLine(label + ": verified " + colored + " triangle pixels and " + (4096 - colored) + " clear pixels");
      }
      Console.WriteLine("PASS: " + mode + ", two hardware draws with verified constant-buffer color change");
    } finally {
      foreach (IntPtr p in new IntPtr[] { list, allocator, pso, root, vs, ps, cbHeap, rtvHeap, readback, cb, target }) if (p != IntPtr.Zero) Marshal.Release(p);
    }
  }
  public static string Run(string binding, uint timeout) {
    Passed = false; object factory = null;
    try {
      VerifyAbi(); Guid iid = typeof(IDXGIFactory1).GUID;
      Check(CreateDXGIFactory1(ref iid, out factory), "CreateDXGIFactory1");
      for (uint i = 0; ; i++) {
        object adapter; int hr = ((IDXGIFactory1)factory).EnumAdapters1(i, out adapter);
        if (hr == unchecked((int)0x887a0002)) break;
        Check(hr, "EnumAdapters1");
        IntPtr pa = IntPtr.Zero, device = IntPtr.Zero, queue = IntPtr.Zero, fence = IntPtr.Zero;
        try {
          AdapterDesc desc; Check(((IDXGIAdapter1)adapter).GetDesc1(out desc), "GetDesc1");
          Console.WriteLine("adapter " + i + ": " + desc.Description + ", vendor 0x" + desc.VendorId.ToString("x4") + ", flags " + desc.Flags);
          if (desc.VendorId != 0x10de || (desc.Flags & 2) != 0) continue;
          pa = Marshal.GetComInterfaceForObject(adapter, typeof(IDXGIAdapter1));
          Stage("NVIDIA D3D12CreateDevice begin"); iid = new Guid("189819f1-1db6-4b57-be54-1821339b85f7");
          Check(D3D12CreateDevice(pa, 0xb000, ref iid, out device), "D3D12CreateDevice hardware 11_0");
          Stage("NVIDIA D3D12CreateDevice end");
          QueueDesc q = new QueueDesc(); iid = new Guid("0ec870a6-5d7e-4c22-8cfc-5baae07616ed");
          Check(Fn<CreateQueueFn>(device, 8)(device, ref q, ref iid, out queue), "CreateCommandQueue DIRECT");
          iid = new Guid("0a753dcf-c4d8-4b91-adf6-be5a60d95a76");
          Check(Fn<CreateFenceFn>(device, 36)(device, 0, 0, ref iid, out fence), "CreateFence");
          ulong value = 0;
          // Root CBV runs first; a failed mode stops the gate and never becomes a partial pass.
          if (binding == "root" || binding == "both") Render(device, queue, fence, ref value, false, timeout);
          if (binding == "heap" || binding == "both") Render(device, queue, fence, ref value, true, timeout);
          Passed = true;
          return "PASS: NVIDIA hardware D3D12 graphics/readback, binding=" + binding + "; no WARP, no swapchain";
        } finally {
          foreach (IntPtr p in new IntPtr[] { fence, queue, device, pa }) if (p != IntPtr.Zero) Marshal.Release(p);
          Marshal.ReleaseComObject(adapter);
        }
      }
      throw new Exception("No NVIDIA hardware adapter found; software fallback is forbidden");
    } catch (Exception e) { return "FAIL: " + e.Message; }
    finally { if (factory != null) Marshal.ReleaseComObject(factory); }
  }
}
}
'@
Add-Type -TypeDefinition $src -Language CSharp
[CaraxesD3D12Render.Probe]::Run($Binding, [uint32]($FenceTimeoutSeconds * 1000))
if (-not [CaraxesD3D12Render.Probe]::Passed) { throw 'D3D12 graphics/readback gate failed' }
