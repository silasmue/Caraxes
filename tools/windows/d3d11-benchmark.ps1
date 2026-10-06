# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
<#
.SYNOPSIS
Original, bounded NVIDIA D3D11 offscreen microbenchmark; emits JSONL on stdout.
.DESCRIPTION
Run in 64-bit Windows PowerShell 5.1 or PowerShell 7. Requires feature level 11.0,
a hardware NVIDIA adapter and d3dcompiler_47.dll. No window, Present, vsync or WARP.
Each sample measures CPU encoding, explicit Flush, and DONOTFLUSH query polling
separately. GPU timestamps require a valid disjoint query; invalid samples are
retained and make the run unsuccessful. Readback validation is outside timing.

Fill uses an R32_UINT target at 256x256, 512x512, 1024x1024 and 1920x1080. ALU
compares simple and 64-round dependent integer pixel shaders at 512x512. Depth
compares visible versus rejected draws against precleared D32_FLOAT, with the
same early-depth integer shader and pipeline/occlusion queries. Memory uses two
DEFAULT structured buffers, together 32 or 128 MiB, and one read plus one write
per uint per pass. This is effective shader traffic, not physical VRAM bandwidth
or a claim about residency/compression. Repeated passes may benefit from caches.

There are at most 4 fullscreen draws/sample by default, 16 MiB per compute
Dispatch by default (selectable 1 MiB), and less than 256 MiB of explicitly allocated resource/input storage.
The hard limit is below 512 MiB; driver allocations are outside this accounting.
A slow GPU batch aborts before another sample. Query/map deadlines cannot interrupt
a hung native driver call; the launcher must impose an external process deadline.
Polling uses SpinWait(64) and occasional Yield; CPU wait is observation latency,
not GPU execution. Depth cases emit SKIP if statistics/occlusion queries are unavailable.
GPU intervals may include submission starvation and scheduling by other contexts.
Raw warmup samples are retained. No native-Windows reference is available.
.EXAMPLE
powershell.exe -NoProfile -File .\d3d11-benchmark.ps1 -Suite all -Warmups 2 -Repetitions 5
.EXAMPLE
powershell.exe -NoProfile -File .\d3d11-benchmark.ps1 -Suite memory -WorkingSetMiB 128
.EXAMPLE
pwsh -NoProfile -File ./d3d11-benchmark.ps1 -ValidateOnly
# ValidateOnly checks C#/helpers/ABI sizes and, on Windows, compiles HLSL on the CPU;
# it never creates a D3D device.
.NOTES
Query contracts: https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ne-d3d11-d3d11_query
https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-getdata
https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-flush
Vtable order: Microsoft Windows SDK d3d11.h (IUnknown and DeviceChild included).
Only interop patterns are adapted from our d3d11-render.ps1; workloads are original.
#>
[CmdletBinding()]
param(
    [ValidateSet('all','fill','alu','depth','memory')][string]$Suite = 'all',
    [ValidateSet('all','32','128')][string]$WorkingSetMiB = 'all',
    [ValidateRange(0,5)][int]$Warmups = 2,
    [ValidateRange(1,20)][int]$Repetitions = 5,
    [ValidateRange(1,16)][int]$Draws = 4,
    [ValidateRange(1,4)][int]$ComputePasses = 2,
    [ValidateSet(1,16)][int]$DispatchMiB = 16,
    [ValidateRange(2,60)][int]$TimeoutSeconds = 10,
    [ValidateRange(10,600)][int]$TotalSeconds = 540,
    [ValidateRange(1,500)][int]$MaxGpuMilliseconds = 250,
    [ValidateRange(-1,15)][int]$AdapterIndex = -1,
    [switch]$ValidateOnly
)
$ErrorActionPreference = 'Stop'
$src = @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

namespace CaraxesD3DBench {
[ComImport, Guid("770aae78-f26f-4dba-a829-253c83d1b387"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface Factory {
    void SetPrivateData(); void SetPrivateDataInterface(); void GetPrivateData(); void GetParent();
    void EnumAdapters(); void MakeWindowAssociation(); void GetWindowAssociation(); void CreateSwapChain(); void CreateSoftwareAdapter();
    [PreserveSig] int EnumAdapters1(uint index, [MarshalAs(UnmanagedType.IUnknown)] out object adapter);
}
[ComImport, Guid("29038f61-3839-4626-91fd-086879011a05"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface Adapter {
    void SetPrivateData(); void SetPrivateDataInterface(); void GetPrivateData(); void GetParent();
    void EnumOutputs(); void GetDesc(); void CheckInterfaceSupport();
    [PreserveSig] int GetDesc1(out AdapterDesc desc);
}
[StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
public struct AdapterDesc {
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=128)] public string Description;
    public uint Vendor, Device, Subsystem, Revision;
    public UIntPtr Dedicated, DedicatedSystem, Shared;
    public uint LuidLo; public int LuidHi; public uint Flags;
}

public sealed class Benchmark : IDisposable {
    [StructLayout(LayoutKind.Sequential)] struct TextureDesc { public uint Width,Height,Mips,Array,Format,Samples,Quality,Usage,Bind,CPU,Misc; }
    [StructLayout(LayoutKind.Sequential)] struct BufferDesc { public uint Bytes,Usage,Bind,CPU,Misc,Stride; }
    [StructLayout(LayoutKind.Sequential)] struct InitialData { public IntPtr Data; public uint Pitch,Slice; }
    [StructLayout(LayoutKind.Sequential)] struct ViewDesc { public uint Format,Dimension,First,Count,Flags,Unused; }
    [StructLayout(LayoutKind.Sequential)] struct QueryDesc { public uint Query,Misc; }
    [StructLayout(LayoutKind.Sequential)] struct Viewport { public float X,Y,Width,Height,Min,Max; }
    [StructLayout(LayoutKind.Sequential)] struct Mapped { public IntPtr Data; public uint Row,Depth; }
    [StructLayout(LayoutKind.Sequential)] struct Box { public uint Left,Top,Front,Right,Bottom,Back; }
    [StructLayout(LayoutKind.Sequential)] struct RasterDesc { public uint Fill,Cull; public int CCW,DepthBias; public float Clamp,Slope; public int DepthClip,Scissor,MSAA,AA; }
    [StructLayout(LayoutKind.Sequential)] struct StencilOp { public uint Fail,DepthFail,Pass,Func; }
    [StructLayout(LayoutKind.Sequential)] struct DepthDesc { public int Enabled; public uint Write,Func; public int Stencil; public byte ReadMask,WriteMask; public StencilOp Front,Back; }

    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateTextureFn(IntPtr self,ref TextureDesc desc,IntPtr data,out IntPtr result);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateBufferFn(IntPtr self,ref BufferDesc desc,IntPtr data,out IntPtr result);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateViewFn(IntPtr self,IntPtr resource,IntPtr desc,out IntPtr result);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateShaderFn(IntPtr self,IntPtr bytes,UIntPtr size,IntPtr linkage,out IntPtr result);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateStateFn(IntPtr self,IntPtr desc,out IntPtr result);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateQueryFn(IntPtr self,ref QueryDesc desc,out IntPtr result);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate IntPtr BlobPtrFn(IntPtr self);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate UIntPtr BlobSizeFn(IntPtr self);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void VoidFn(IntPtr self);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int ReasonFn(IntPtr self);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void ObjectFn(IntPtr self,IntPtr value);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void QueryFn(IntPtr self,IntPtr query);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int GetDataFn(IntPtr self,IntPtr query,IntPtr data,uint size,uint flags);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void ShaderFn(IntPtr self,IntPtr shader,IntPtr instances,uint count);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void BuffersFn(IntPtr self,uint slot,uint count,ref IntPtr buffer);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void UavsFn(IntPtr self,uint slot,uint count,ref IntPtr view,IntPtr counts);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void TargetsFn(IntPtr self,uint count,ref IntPtr view,IntPtr depth);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void ClearFn(IntPtr self,IntPtr view,[In]float[] color);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void ClearDepthFn(IntPtr self,IntPtr view,uint flags,float depth,byte stencil);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void DepthStateFn(IntPtr self,IntPtr state,uint reference);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void ViewportFn(IntPtr self,uint count,ref Viewport viewport);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void TopologyFn(IntPtr self,uint value);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void DrawFn(IntPtr self,uint vertices,uint first);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void DispatchFn(IntPtr self,uint x,uint y,uint z);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void CopyFn(IntPtr self,IntPtr dst,IntPtr src);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void CopyPartFn(IntPtr self,IntPtr dst,uint sub,uint x,uint y,uint z,IntPtr src,uint srcSub,ref Box box);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void UpdateFn(IntPtr self,IntPtr dst,uint sub,IntPtr box,IntPtr data,uint row,uint depth);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int MapFn(IntPtr self,IntPtr resource,uint sub,uint mode,uint flags,out Mapped mapped);
    [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void UnmapFn(IntPtr self,IntPtr resource,uint sub);

    [DllImport("dxgi.dll")] static extern int CreateDXGIFactory1(ref Guid guid,[MarshalAs(UnmanagedType.IUnknown)]out object factory);
    [DllImport("d3d11.dll")] static extern int D3D11CreateDevice(IntPtr adapter,int driver,IntPtr software,uint flags,IntPtr levels,uint levelCount,uint sdk,out IntPtr device,out int featureLevel,out IntPtr context);
    [DllImport("d3dcompiler_47.dll",CharSet=CharSet.Ansi,ExactSpelling=true)]
    static extern int D3DCompile(byte[] src,UIntPtr size,string name,IntPtr defines,IntPtr includes,string entry,string target,uint flags,uint flags2,out IntPtr code,out IntPtr errors);

    // All timed calls use delegates bound once, outside the measured intervals.
    IntPtr dev,ctx,constants,constantBytes,vs,simple,heavy,compute,raster,depthOn,depthOff;
    VoidFn flush,clearState; QueryFn begin,end; GetDataFn getData; ReasonFn removed;
    ShaderFn setVS,setPS,setCS; BuffersFn psCB,csCB,csSRV; UavsFn csUAV;
    TargetsFn targets; ClearFn clear; ClearDepthFn clearDepth; DepthStateFn setDepth;
    ViewportFn viewport; TopologyFn topology; ObjectFn setRaster; DrawFn draw; DispatchFn dispatch;
    CopyFn copy; CopyPartFn copyPart; UpdateFn update; MapFn map; UnmapFn unmap;
    readonly List<IntPtr> owned = new List<IntPtr>();
    readonly Stopwatch lifetime = Stopwatch.StartNew();
    int timeout,total,limitGpu,warmups,repetitions,draws,passes,samples,invalid,skipped;
    bool statsSupported,occlusionSupported;
    uint chunkWords;
    string currentCase="startup";
    int currentSample=-1;
    readonly float[] black = new float[] {0,0,0,0};
    const uint R32_UINT=42, D32_FLOAT=40;
    const string Hlsl = @"
cbuffer Params : register(b0) { uint Seed; uint Count; uint Base; uint Rounds; };
float4 vs(uint i:SV_VertexID):SV_Position {
    float2 p[3]={float2(-1,-1),float2(-1,3),float2(3,-1)};
    return float4(p[i],0.75,1);
}
uint pixel(float4 p) { return uint(p.x)*747796405u + uint(p.y)*2891336453u + Seed; }
uint mix(uint v) { [loop] for(uint k=0;k<Rounds;k++) v=(v^(v>>13))*1664525u+1013904223u; return v; }
uint psSimple(float4 p:SV_Position):SV_Target { return pixel(p); }
[earlydepthstencil] uint psHeavy(float4 p:SV_Position):SV_Target { return mix(pixel(p)); }
StructuredBuffer<uint> Input : register(t0);
RWStructuredBuffer<uint> Output : register(u0);
[numthreads(256,1,1)] void cs(uint3 tid:SV_DispatchThreadID) {
    uint i=Base+tid.x; if(i<Count) Output[i]=Input[i]^Seed;
}";

    static T Fn<T>(IntPtr obj,int slot) { return (T)(object)Marshal.GetDelegateForFunctionPointer(Marshal.ReadIntPtr(Marshal.ReadIntPtr(obj),slot*IntPtr.Size),typeof(T)); }
    static void Check(int hr,string operation) { if(hr<0) throw new Exception(operation+" HRESULT 0x"+hr.ToString("x8")); }
    static void Release(ref IntPtr p) { if(p!=IntPtr.Zero) { Marshal.Release(p); p=IntPtr.Zero; } }
    IntPtr Own(IntPtr p) { owned.Add(p); return p; }
    static double Ms(long ticks) { return ticks*1000.0/Stopwatch.Frequency; }
    static string Quote(string s) {
        var b=new StringBuilder("\"");
        foreach(char c in s) { if(c=='"'||c=='\\') b.Append('\\').Append(c); else if(c<32) b.Append("\\u").Append(((int)c).ToString("x4")); else b.Append(c); }
        return b.Append('"').ToString();
    }
    static string Value(object v) {
        if(v==null) return "null";
        if(v is string) return Quote((string)v);
        if(v is bool) return (bool)v ? "true":"false";
        if(v is double && (Double.IsNaN((double)v)||Double.IsInfinity((double)v))) return "null";
        return Convert.ToString(v,CultureInfo.InvariantCulture);
    }
    static void Emit(string type,params object[] pairs) {
        if((pairs.Length&1)!=0) throw new Exception("invalid JSON fields");
        var b=new StringBuilder("{\"schema\":1,\"type\":").Append(Quote(type));
        for(int i=0;i<pairs.Length;i+=2) b.Append(',').Append(Quote((string)pairs[i])).Append(':').Append(Value(pairs[i+1]));
        Console.WriteLine(b.Append('}').ToString()); Console.Out.Flush();
    }
    void Deadline() { if(lifetime.Elapsed.TotalSeconds>=total) throw new TimeoutException("whole benchmark deadline"); }
    void DeviceAlive() { Check(removed(dev),"GetDeviceRemovedReason"); }
    void PollPause(ref int polls,Stopwatch watch) {
        polls++; Deadline();
        if(watch.Elapsed.TotalSeconds>=timeout) { DeviceAlive(); throw new TimeoutException("query/map deadline in "+currentCase); }
        if((polls&255)==0) DeviceAlive();
        Thread.SpinWait(64);
        if((polls&63)==0) Thread.Yield();
    }
    IntPtr StructPointer(object s) { IntPtr p=Marshal.AllocHGlobal(Marshal.SizeOf(s)); Marshal.StructureToPtr(s,p,false); return p; }
    IntPtr Buffer(BufferDesc desc,IntPtr initial) { IntPtr p; Check(Fn<CreateBufferFn>(dev,3)(dev,ref desc,initial,out p),"CreateBuffer"); return p; }
    IntPtr Texture(TextureDesc desc) { IntPtr p; Check(Fn<CreateTextureFn>(dev,5)(dev,ref desc,IntPtr.Zero,out p),"CreateTexture2D"); return p; }
    IntPtr View(IntPtr resource,int slot,object descriptor) {
        IntPtr p=IntPtr.Zero,d=descriptor==null ? IntPtr.Zero:StructPointer(descriptor);
        try { Check(Fn<CreateViewFn>(dev,slot)(dev,resource,d,out p),"CreateView slot "+slot); return p; }
        finally { if(d!=IntPtr.Zero) Marshal.FreeHGlobal(d); }
    }
    IntPtr State(int slot,object desc) {
        IntPtr d=StructPointer(desc),p=IntPtr.Zero;
        try { Check(Fn<CreateStateFn>(dev,slot)(dev,d,out p),"CreateState slot "+slot); return p; }
        finally { Marshal.FreeHGlobal(d); }
    }
    IntPtr Shader(string entry,string target,int slot) {
        byte[] source=Encoding.ASCII.GetBytes(Hlsl);
        IntPtr code=IntPtr.Zero,errors=IntPtr.Zero,p=IntPtr.Zero;
        try {
            int hr=D3DCompile(source,(UIntPtr)source.Length,"caraxes-d3d11-benchmark",IntPtr.Zero,IntPtr.Zero,entry,target,1u<<15,0,out code,out errors);
            if(hr<0&&errors!=IntPtr.Zero) throw new Exception(Marshal.PtrToStringAnsi(Fn<BlobPtrFn>(errors,3)(errors)));
            Check(hr,"D3DCompile "+entry);
            Check(Fn<CreateShaderFn>(dev,slot)(dev,Fn<BlobPtrFn>(code,3)(code),Fn<BlobSizeFn>(code,4)(code),IntPtr.Zero,out p),"CreateShader "+entry);
            return Own(p);
        } finally { Release(ref errors); Release(ref code); }
    }
    void SetConstants(uint seed,uint count,uint first,uint rounds) {
        Marshal.WriteInt32(constantBytes,0,unchecked((int)seed)); Marshal.WriteInt32(constantBytes,4,unchecked((int)count));
        Marshal.WriteInt32(constantBytes,8,unchecked((int)first)); Marshal.WriteInt32(constantBytes,12,unchecked((int)rounds));
        update(ctx,constants,0,IntPtr.Zero,constantBytes,0,0);
    }
    sealed class Query : IDisposable {
        public IntPtr Obj,Data; public uint Size;
        public Query(Benchmark b,uint kind,uint size) {
            Size=size; QueryDesc desc=new QueryDesc { Query=kind };
            Check(Fn<CreateQueryFn>(b.dev,24)(b.dev,ref desc,out Obj),"CreateQuery "+kind);
            Data=Marshal.AllocHGlobal((int)size);
        }
        public ulong U64(int offset) { return unchecked((ulong)Marshal.ReadInt64(Data,offset)); }
        public void Dispose() { if(Data!=IntPtr.Zero) Marshal.FreeHGlobal(Data); Data=IntPtr.Zero; Release(ref Obj); }
    }
    void Wait(Query q,Stopwatch elapsed,ref int polls) {
        for(;;) {
            int hr=getData(ctx,q.Obj,q.Data,q.Size,1); // D3D11_ASYNC_GETDATA_DONOTFLUSH
            if(hr==0) { polls++; return; }
            if(hr!=1) { Check(hr,"GetData"); throw new Exception("unexpected GetData status "+hr); }
            PollPause(ref polls,elapsed);
        }
    }
    void Fence() {
        using(var q=new Query(this,0,4)) {
            end(ctx,q.Obj); flush(ctx); int polls=0; var w=Stopwatch.StartNew(); Wait(q,w,ref polls);
        }
    }
    Mapped MapReady(IntPtr resource) {
        int polls=0; var w=Stopwatch.StartNew();
        for(;;) {
            Mapped result; int hr=map(ctx,resource,0,1,0x100000,out result); // READ, DO_NOT_WAIT
            if(hr==0) { if(result.Data==IntPtr.Zero) throw new Exception("null mapped resource"); return result; }
            if(hr!=unchecked((int)0x887a000a)) { Check(hr,"Map readback"); throw new Exception("unexpected Map status"); }
            PollPause(ref polls,w);
        }
    }
    void Open(int adapterIndex) {
        if(IntPtr.Size!=8) throw new Exception("64-bit process required");
        if(Environment.OSVersion.Platform!=PlatformID.Win32NT) throw new Exception("Windows required (ValidateOnly is portable)");
        Guid guid=typeof(Factory).GUID; object factory;
        Check(CreateDXGIFactory1(ref guid,out factory),"CreateDXGIFactory1");
        try {
            var f=(Factory)factory;
            for(uint i=0;i<16;i++) {
                object item; int hr=f.EnumAdapters1(i,out item);
                if(hr==unchecked((int)0x887a0002)) break;
                Check(hr,"EnumAdapters1");
                try {
                    AdapterDesc desc; Check(((Adapter)item).GetDesc1(out desc),"GetDesc1");
                    if(adapterIndex>=0&&i!=(uint)adapterIndex) continue;
                    if(desc.Vendor!=0x10de||(desc.Flags&2)!=0) { if(adapterIndex>=0) throw new Exception("selected adapter is not NVIDIA hardware"); continue; }
                    IntPtr a=Marshal.GetComInterfaceForObject(item,typeof(Adapter)); int level;
                    try { Check(D3D11CreateDevice(a,0,IntPtr.Zero,0,IntPtr.Zero,0,7,out dev,out level,out ctx),"D3D11CreateDevice"); }
                    finally { Marshal.Release(a); }
                    if(level<0xb000) throw new Exception("feature level 11.0 required");
                    Emit("adapter","index",i,"description",desc.Description,"vendor",desc.Vendor,"device",desc.Device,"flags",desc.Flags,
                        "dedicated_bytes",desc.Dedicated.ToUInt64(),"feature_level",level,"luid_lo",desc.LuidLo,"luid_hi",desc.LuidHi,"hardware",true);
                    Bind(); return;
                } finally { Marshal.ReleaseComObject(item); }
            }
            throw new Exception("no selected NVIDIA hardware adapter; WARP is never used");
        } finally { Marshal.ReleaseComObject(factory); }
    }
    bool SupportsQuery(uint kind) {
        QueryDesc desc=new QueryDesc { Query=kind };IntPtr q=IntPtr.Zero;
        int hr=Fn<CreateQueryFn>(dev,24)(dev,ref desc,out q);Release(ref q);
        if(hr==unchecked((int)0x80004001)||hr==unchecked((int)0x80070057)||hr==unchecked((int)0x887a0004)) return false;
        Check(hr,"query capability "+kind);return true;
    }
    void Bind() {
        begin=Fn<QueryFn>(ctx,27); end=Fn<QueryFn>(ctx,28); getData=Fn<GetDataFn>(ctx,29);
        clearState=Fn<VoidFn>(ctx,110); flush=Fn<VoidFn>(ctx,111); removed=Fn<ReasonFn>(dev,39);
        setVS=Fn<ShaderFn>(ctx,11); setPS=Fn<ShaderFn>(ctx,9); setCS=Fn<ShaderFn>(ctx,69);
        psCB=Fn<BuffersFn>(ctx,16); csCB=Fn<BuffersFn>(ctx,71); csSRV=Fn<BuffersFn>(ctx,67); csUAV=Fn<UavsFn>(ctx,68);
        targets=Fn<TargetsFn>(ctx,33); clear=Fn<ClearFn>(ctx,50); clearDepth=Fn<ClearDepthFn>(ctx,53);
        setDepth=Fn<DepthStateFn>(ctx,36); viewport=Fn<ViewportFn>(ctx,44); topology=Fn<TopologyFn>(ctx,24); setRaster=Fn<ObjectFn>(ctx,43);
        draw=Fn<DrawFn>(ctx,13); dispatch=Fn<DispatchFn>(ctx,41); copy=Fn<CopyFn>(ctx,47); copyPart=Fn<CopyPartFn>(ctx,46);
        update=Fn<UpdateFn>(ctx,48); map=Fn<MapFn>(ctx,14); unmap=Fn<UnmapFn>(ctx,15);
        constantBytes=Marshal.AllocHGlobal(16); constants=Own(Buffer(new BufferDesc { Bytes=16,Bind=4 },IntPtr.Zero));
        vs=Shader("vs","vs_5_0",12); simple=Shader("psSimple","ps_5_0",15); heavy=Shader("psHeavy","ps_5_0",15); compute=Shader("cs","cs_5_0",18);
        raster=Own(State(22,new RasterDesc { Fill=3,Cull=1,DepthClip=1 }));
        StencilOp stencil=new StencilOp { Fail=1,DepthFail=1,Pass=1,Func=8 };
        depthOn=Own(State(21,new DepthDesc { Enabled=1,Write=0,Func=2,ReadMask=255,WriteMask=255,Front=stencil,Back=stencil }));
        depthOff=Own(State(21,new DepthDesc { Enabled=0,Write=0,Func=8,ReadMask=255,WriteMask=255,Front=stencil,Back=stencil }));
        statsSupported=SupportsQuery(4);occlusionSupported=SupportsQuery(1);
        Emit("capabilities","pipeline_statistics",statsSupported,"occlusion",occlusionSupported);
        Fence();
    }
    sealed class Sample {
        public double Encode,Flush,Wait,Wall; public long CpuBegin,CpuEncoded,CpuFlushed,CpuDone; public ulong Start,End,Frequency,Occlusion,IA,PS,CS;
        public bool Valid,Disjoint,Statistics,HasOcclusion; public string InvalidReason; public int Polls;
        public double Gpu { get { return (End-Start)*1000.0/Frequency; } }
    }
    static string TimestampReason(ulong start,ulong end,ulong frequency,bool disjoint) {
        return disjoint ? "disjoint":frequency==0 ? "zero_frequency":end<=start ? "nonpositive_interval":null;
    }
    Sample Measure(Action workload,bool rasterWork) {
        Deadline(); DeviceAlive();
        using(var disjoint=new Query(this,3,16)) using(var first=new Query(this,2,8)) using(var last=new Query(this,2,8))
        using(var statistics=statsSupported ? new Query(this,4,88):null) using(var occlusion=rasterWork&&occlusionSupported ? new Query(this,1,8):null) {
            long t0=Stopwatch.GetTimestamp();
            begin(ctx,disjoint.Obj); if(statistics!=null) begin(ctx,statistics.Obj); if(occlusion!=null) begin(ctx,occlusion.Obj);
            end(ctx,first.Obj); workload(); end(ctx,last.Obj);
            if(occlusion!=null) end(ctx,occlusion.Obj); if(statistics!=null) end(ctx,statistics.Obj); end(ctx,disjoint.Obj);
            long t1=Stopwatch.GetTimestamp(); flush(ctx); long t2=Stopwatch.GetTimestamp();
            int polls=0; var w=Stopwatch.StartNew();
            Wait(disjoint,w,ref polls); Wait(first,w,ref polls); Wait(last,w,ref polls); if(statistics!=null) Wait(statistics,w,ref polls);
            if(occlusion!=null) Wait(occlusion,w,ref polls);
            long t3=Stopwatch.GetTimestamp();
            var s=new Sample { Encode=Ms(t1-t0),Flush=Ms(t2-t1),Wait=Ms(t3-t2),Wall=Ms(t3-t0),Polls=polls,
                CpuBegin=t0,CpuEncoded=t1,CpuFlushed=t2,CpuDone=t3,
                Start=first.U64(0),End=last.U64(0),Frequency=disjoint.U64(0),Disjoint=Marshal.ReadInt32(disjoint.Data,8)!=0,Statistics=statistics!=null,HasOcclusion=occlusion!=null,
                IA=statistics==null ? 0:statistics.U64(0),PS=statistics==null ? 0:statistics.U64(56),CS=statistics==null ? 0:statistics.U64(80),
                Occlusion=occlusion==null ? 0:occlusion.U64(0) };
            s.InvalidReason=TimestampReason(s.Start,s.End,s.Frequency,s.Disjoint);
            s.Valid=s.InvalidReason==null;
            return s;
        }
    }
    void Record(Sample s,int width,int height,int mib,ulong pixels,ulong bytes,int commands,uint seed,int rounds,int verified,double validationMs) {
        samples++; if(!s.Valid) invalid++;
        Emit("sample","case",currentCase,"sample",currentSample,"warmup",currentSample<warmups,
            "width",width,"height",height,"working_set_mib",mib,"commands",commands,"seed",seed,"integer_rounds",rounds,
            "gpu_valid",s.Valid,"gpu_disjoint",s.Disjoint,"invalid_reason",s.InvalidReason,"gpu_start_ticks",s.Start,"gpu_end_ticks",s.End,"gpu_frequency_hz",s.Frequency,
            "gpu_ms",s.Valid ? (object)s.Gpu:null,"cpu_begin_ticks",s.CpuBegin,"cpu_encoded_ticks",s.CpuEncoded,
            "cpu_flushed_ticks",s.CpuFlushed,"cpu_done_ticks",s.CpuDone,"cpu_frequency_hz",Stopwatch.Frequency,"cpu_encode_ms",s.Encode,"cpu_flush_ms",s.Flush,"cpu_wait_ms",s.Wait,"cpu_total_ms",s.Wall,
            "getdata_calls",s.Polls,"ia_vertices",s.Statistics ? (object)s.IA:null,"ps_invocations",s.Statistics ? (object)s.PS:null,
            "cs_invocations",s.Statistics ? (object)s.CS:null,"occlusion_samples",s.HasOcclusion ? (object)s.Occlusion:null,
            "timing_scope","workload_only_validation_excluded","dispatch_mib",chunkWords*4/1048576,
            "logical_pixels",pixels,"effective_rw_bytes",bytes,"effective_gib_s",s.Valid&&bytes!=0 ? (object)(bytes/(s.Gpu/1000.0)/1073741824.0):null,
            "validated_words",verified,"validation","pass","validation_ms",validationMs,"validation_timed",false);
        if(s.Valid&&s.Gpu>limitGpu) throw new Exception("batch exceeded GPU duration limit; no next batch submitted");
        Deadline();
    }
    static uint Pixel(uint x,uint y,uint seed,int rounds) {
        unchecked { uint v=x*747796405u+y*2891336453u+seed; for(int i=0;i<rounds;i++) v=(v^(v>>13))*1664525u+1013904223u; return v; }
    }
    static uint Input(uint i) { return unchecked(i*747796405u+2891336453u); }
    void RasterCase(string name,int width,int height,int rounds,bool depth,bool occluded) {
        currentCase=name; currentSample=-1;
        Emit("case_begin","case",name,"width",width,"height",height,"draws",draws,"rounds",rounds,"depth_test",depth,"occluded",occluded,"format","R32_UINT");
        if(depth&&(!statsSupported||!occlusionSupported)) {
            skipped++;Emit("case_skip","case",name,"reason","depth comparison requires pipeline statistics and occlusion queries");return;
        }
        IntPtr render=IntPtr.Zero,stage=IntPtr.Zero,rtv=IntPtr.Zero,z=IntPtr.Zero,dsv=IntPtr.Zero;
        try {
            var desc=new TextureDesc { Width=(uint)width,Height=(uint)height,Mips=1,Array=1,Format=R32_UINT,Samples=1,Bind=0x20 };
            render=Texture(desc); rtv=View(render,9,null); desc.Bind=0;desc.Usage=3;desc.CPU=0x20000;stage=Texture(desc);
            if(depth) { desc.Bind=0x40;desc.Usage=0;desc.CPU=0;desc.Format=D32_FLOAT;z=Texture(desc);dsv=View(z,10,null); }
            clearState(ctx); targets(ctx,1,ref rtv,dsv); setVS(ctx,vs,IntPtr.Zero,0); setPS(ctx,rounds==0 ? simple:heavy,IntPtr.Zero,0);
            topology(ctx,4); setRaster(ctx,raster); setDepth(ctx,depth ? depthOn:depthOff,0); psCB(ctx,0,1,ref constants);
            var vp=new Viewport { Width=width,Height=height,Max=1 };viewport(ctx,1,ref vp);
            for(currentSample=0;currentSample<warmups+repetitions;currentSample++) {
                Deadline(); uint seed=unchecked(0x1234567u+(uint)currentSample*104729u);
                SetConstants(seed,0,0,(uint)rounds); clear(ctx,rtv,black); if(depth) clearDepth(ctx,dsv,1,occluded ? 0.0f:1.0f,0);
                Fence(); // precondition and clears are outside both CPU and GPU sample intervals
                Sample s=Measure(delegate { for(int d=0;d<draws;d++) draw(ctx,3,0); },true);
                if(s.Statistics&&s.IA!=(ulong)draws*3) throw new Exception("pipeline IA vertex count mismatch");
                ulong expected=(ulong)width*(ulong)height*(ulong)draws;
                if(s.HasOcclusion&&s.Occlusion!=(occluded ? 0:expected)) throw new Exception("occlusion count mismatch: "+s.Occlusion+" expected "+(occluded ? 0:expected));
                var validation=Stopwatch.StartNew();
                copy(ctx,stage,render); Fence(); Mapped m=MapReady(stage);
                int checkedWords=0;
                try {
                    if(m.Row<(uint)width*4) throw new Exception("short texture row pitch");
                    int[] row=new int[width];
                    for(int y=0;y<height;y++) {
                        Deadline();Marshal.Copy(IntPtr.Add(m.Data,checked((int)m.Row*y)),row,0,width);
                        for(int x=0;x<width;x++) {
                            uint actual=unchecked((uint)row[x]);uint want=occluded ? 0:Pixel((uint)x,(uint)y,seed,rounds);
                            if(actual!=want) throw new Exception("pixel mismatch at "+x+","+y+": "+actual+" expected "+want);
                            checkedWords++;
                        }
                    }
                } finally { unmap(ctx,stage,0); }
                Record(s,width,height,0,expected,0,draws,seed,rounds,checkedWords,validation.Elapsed.TotalMilliseconds);
            }
        } finally { clearState(ctx); Release(ref dsv);Release(ref z);Release(ref rtv);Release(ref stage);Release(ref render);flush(ctx); }
    }
    IntPtr InputBuffer(uint words) {
        uint[] input=new uint[words]; for(uint i=0;i<words;i++) input[i]=Input(i);
        GCHandle pin=GCHandle.Alloc(input,GCHandleType.Pinned); IntPtr initial=IntPtr.Zero;
        try {
            initial=StructPointer(new InitialData { Data=pin.AddrOfPinnedObject() });
            return Buffer(new BufferDesc { Bytes=checked(words*4),Bind=8,Misc=0x40,Stride=4 },initial);
        } finally { if(initial!=IntPtr.Zero) Marshal.FreeHGlobal(initial);pin.Free(); }
    }
    void MemoryCase(int mib) {
        currentCase="memory_rw_"+mib; currentSample=-1;
        uint words=checked((uint)mib*1024u*1024u/8u),bufferBytes=words*4;
        int chunks=checked((int)((words+chunkWords-1)/chunkWords));
        Emit("case_begin","case",currentCase,"working_set_mib",mib,"buffer_bytes",bufferBytes,"buffers",2,"passes",passes,
            "dispatch_bytes",chunkWords*4,"dispatches_per_pass",chunks,"traffic","one uint read plus one uint write per element/pass; effective, not physical DRAM");
        IntPtr input=IntPtr.Zero,output=IntPtr.Zero,srv=IntPtr.Zero,uav=IntPtr.Zero,stage=IntPtr.Zero;
        try {
            input=InputBuffer(words); output=Buffer(new BufferDesc { Bytes=bufferBytes,Bind=0x80,Misc=0x40,Stride=4 },IntPtr.Zero);
            var view=new ViewDesc { Dimension=1,Count=words };srv=View(input,7,view);uav=View(output,8,view);
            stage=Buffer(new BufferDesc { Bytes=4096,Usage=3,CPU=0x20000 },IntPtr.Zero);
            clearState(ctx);setCS(ctx,compute,IntPtr.Zero,0);csCB(ctx,0,1,ref constants);Fence();
            for(currentSample=0;currentSample<warmups+repetitions;currentSample++) {
                Deadline();uint seed=unchecked(0x9e3779b9u+(uint)currentSample*104729u);
                csSRV(ctx,0,1,ref srv);csUAV(ctx,0,1,ref uav,IntPtr.Zero);Fence();
                Sample s=Measure(delegate {
                    for(int pass=0;pass<passes;pass++) for(uint first=0;first<words;first+=chunkWords) {
                        SetConstants(seed,words,first,0);uint n=Math.Min(chunkWords,words-first);dispatch(ctx,(n+255)/256,1,1);
                    }
                },false);
                IntPtr none=IntPtr.Zero;csUAV(ctx,0,1,ref none,IntPtr.Zero);csSRV(ctx,0,1,ref none);
                if(s.Statistics&&s.CS!=(ulong)words*(ulong)passes) throw new Exception("compute invocation count mismatch");
                int checkedWords=0;var validation=Stopwatch.StartNew();
                for(uint k=0;k<16;k++) {
                    // Include both ends and 14 deterministic distributed 4 KiB blocks.
                    uint block=k==0 ? 0:k==15 ? bufferBytes/4096-1:unchecked(k*2654435761u)%(bufferBytes/4096);
                    Box box=new Box { Left=block*4096,Right=(block+1)*4096,Bottom=1,Back=1 };
                    copyPart(ctx,stage,0,0,0,0,output,0,ref box);Fence();Mapped m=MapReady(stage);
                    try {
                        for(uint j=0;j<1024;j++) {
                            uint actual=unchecked((uint)Marshal.ReadInt32(m.Data,(int)j*4));uint want=Input(block*1024+j)^seed;
                            if(actual!=want) throw new Exception("buffer mismatch at uint "+(block*1024+j)+": "+actual+" expected "+want);
                            checkedWords++;
                        }
                    } finally { unmap(ctx,stage,0); }
                }
                Record(s,0,0,mib,0,(ulong)words*8UL*(ulong)passes,chunks*passes,seed,0,checkedWords,validation.Elapsed.TotalMilliseconds);
            }
        } finally { clearState(ctx);Release(ref stage);Release(ref uav);Release(ref srv);Release(ref output);Release(ref input);flush(ctx); }
    }
    public void Dispose() {
        if(ctx!=IntPtr.Zero&&clearState!=null) clearState(ctx);
        for(int i=owned.Count-1;i>=0;i--) { IntPtr p=owned[i];Marshal.Release(p); } owned.Clear();
        if(constantBytes!=IntPtr.Zero) Marshal.FreeHGlobal(constantBytes);constantBytes=IntPtr.Zero;
        Release(ref ctx);Release(ref dev);
    }
    public static int Validate() {
        if(IntPtr.Size!=8) throw new Exception("64-bit process required");
        if(Marshal.SizeOf(typeof(TextureDesc))!=44||Marshal.SizeOf(typeof(BufferDesc))!=24||Marshal.SizeOf(typeof(DepthDesc))!=52||Marshal.SizeOf(typeof(RasterDesc))!=40||Marshal.SizeOf(typeof(Mapped))!=16||Marshal.SizeOf(typeof(AdapterDesc))!=312||Marshal.SizeOf(typeof(InitialData))!=16||Marshal.SizeOf(typeof(ViewDesc))!=24||Marshal.SizeOf(typeof(QueryDesc))!=8)
            throw new Exception("interop structure size mismatch");
        if(Quote("a\n\"\\")!="\"a\\u000a\\\"\\\\\"") throw new Exception("JSON escaping mismatch");
        if(Pixel(0,0,17,0)!=17||Input(0)!=2891336453u||Input(1)!=3639132858u) throw new Exception("reference arithmetic mismatch");
        if(Pixel(511,511,0x1234567u,64)!=2773538987u) throw new Exception("heavy reference vector mismatch");
        for(uint i=0;i<100;i++) if(((Input(i)^123u)^123u)!=Input(i)) throw new Exception("reference transform mismatch");
        if(TimestampReason(1,2,100,false)!=null||TimestampReason(1,2,100,true)!="disjoint"||TimestampReason(1,2,0,false)!="zero_frequency"||TimestampReason(1,1,100,false)!="nonpositive_interval"||TimestampReason(2,1,100,false)!="nonpositive_interval")
            throw new Exception("timestamp validity mismatch");
        bool windows=Environment.OSVersion.Platform==PlatformID.Win32NT;
        if(windows) {
            string[] entries={"vs","psSimple","psHeavy","cs"};string[] targets={"vs_5_0","ps_5_0","ps_5_0","cs_5_0"};
            byte[] source=Encoding.ASCII.GetBytes(Hlsl);
            for(int i=0;i<entries.Length;i++) {
                IntPtr code=IntPtr.Zero,errors=IntPtr.Zero;
                try {
                    int hr=D3DCompile(source,(UIntPtr)source.Length,"caraxes-d3d11-benchmark-selftest",IntPtr.Zero,IntPtr.Zero,entries[i],targets[i],1u<<15,0,out code,out errors);
                    if(hr<0&&errors!=IntPtr.Zero) throw new Exception(Marshal.PtrToStringAnsi(Fn<BlobPtrFn>(errors,3)(errors)));
                    Check(hr,"selftest HLSL "+entries[i]);
                } finally { Release(ref errors);Release(ref code); }
            }
        }
        Emit("self_test","result","pass","gpu_opened",false,"benchmark_pass",false,"hlsl_compile",windows ? "pass":"not_run_non_windows");return 0;
    }
    public static int Run(string suite,string workingSet,int warmup,int repeats,int drawCount,int computePasses,int dispatchMiB,int queryTimeout,int totalTimeout,int maxGpu,int adapter) {
        using(var b=new Benchmark()) {
            b.warmups=warmup;b.repetitions=repeats;b.draws=drawCount;b.passes=computePasses;b.chunkWords=(uint)dispatchMiB*1048576u/4;b.timeout=queryTimeout;b.total=totalTimeout;b.limitGpu=maxGpu;
            try {
                Emit("run_begin","utc_start",DateTime.UtcNow.ToString("o",CultureInfo.InvariantCulture),"process_id",Process.GetCurrentProcess().Id,"suite",suite,"working_set_mib",workingSet,"warmups",warmup,"repetitions",repeats,"draws",drawCount,"compute_passes",computePasses,
                    "dispatch_mib",dispatchMiB,"timeout_seconds",queryTimeout,"total_seconds",totalTimeout,"max_gpu_ms",maxGpu,"native_reference",false,"present",false,
                    "timing","GPU timestamp interval; CPU encode, explicit Flush, DONOTFLUSH wait; validation excluded",
                    "memory_limit_note","explicit live resource/input storage below 256 MiB; driver residency not controlled");
                b.Open(adapter);
                if(suite=="all"||suite=="fill") {
                    b.RasterCase("fill_256",256,256,0,false,false);b.RasterCase("fill_512",512,512,0,false,false);
                    b.RasterCase("fill_1024",1024,1024,0,false,false);b.RasterCase("fill_1080p",1920,1080,0,false,false);
                }
                if(suite=="all"||suite=="alu") { b.RasterCase("alu_simple",512,512,0,false,false);b.RasterCase("alu_integer64",512,512,64,false,false); }
                if(suite=="all"||suite=="depth") { b.RasterCase("depth_visible",512,512,64,true,false);b.RasterCase("depth_occluded",512,512,64,true,true); }
                if(suite=="all"||suite=="memory") { if(workingSet=="all"||workingSet=="32") b.MemoryCase(32);if(workingSet=="all"||workingSet=="128") b.MemoryCase(128); }
                Emit("run_end","result",b.invalid!=0 ? "invalid_samples":b.skipped!=0 ? "partial":"pass","samples",b.samples,"invalid_samples",b.invalid,"skipped_cases",b.skipped,"elapsed_ms",b.lifetime.Elapsed.TotalMilliseconds);
                return b.invalid!=0 ? 2:b.skipped!=0 ? 3:0;
            } catch(Exception e) {
                Emit("error","case",b.currentCase,"sample",b.currentSample,"message",e.Message,"elapsed_ms",b.lifetime.Elapsed.TotalMilliseconds,
                    "samples",b.samples,"invalid_samples",b.invalid,"result","fail");return 1;
            }
        }
    }
}
}
'@
Add-Type -TypeDefinition $src -Language CSharp
if ($ValidateOnly) {
    $result = [CaraxesD3DBench.Benchmark]::Validate()
} else {
    $result = [CaraxesD3DBench.Benchmark]::Run($Suite,$WorkingSetMiB,$Warmups,$Repetitions,$Draws,$ComputePasses,$DispatchMiB,$TimeoutSeconds,$TotalSeconds,$MaxGpuMilliseconds,$AdapterIndex)
}
exit $result
