# SPDX-License-Identifier: MIT
# NVENC through the NVENC API directly (nvEncodeAPI64.dll + nvcuda.dll, no ffmpeg), the Windows
# twin of caraxes/tools/nvenc-probe.c: a CUDA context, an NVENC session on it, P1 ultra-low-latency
# CBR, N frames of the same moving test pattern, one ENC line with the same fields.
#   -Mode lock : driver input buffers, locked and filled by the CPU per frame (as nvenc-probe.c)
#   -Mode vram : 8 NV12 frames uploaded once to CUDA device memory, registered, mapped per frame
#                (no per-frame upload: the encoder alone)
# Struct offsets and versions: nv-codec-headers 13.1 (nvEncodeAPI.h), laid out for Windows x64
# with clang -target x86_64-pc-windows-msvc. C# 5 for PowerShell 5.1.
# Usage: nvenc-probe.ps1 [-Mode lock|vram] [-Codec h264|hevc] [-Frames 300] [-Kbit 20000]
param([ValidateSet('lock', 'vram')][string]$Mode = 'lock', [ValidateSet('h264', 'hevc')][string]$Codec = 'h264',
      [int]$Frames = 300, [int]$Kbit = 20000, [int]$Width = 1920, [int]$Height = 1080)
$ErrorActionPreference = "Stop"
$src = @'
using System; using System.Runtime.InteropServices;
namespace CaraxesNvenc {
public static class Probe {
  [DllImport("nvcuda.dll")] static extern int cuInit(uint flags);
  [DllImport("nvcuda.dll")] static extern int cuDeviceGet(out int dev, int ordinal);
  [DllImport("nvcuda.dll", EntryPoint = "cuCtxCreate_v2")] static extern int cuCtxCreate(out IntPtr ctx, uint flags, int dev);
  [DllImport("nvcuda.dll", EntryPoint = "cuCtxDestroy_v2")] static extern int cuCtxDestroy(IntPtr ctx);
  [DllImport("nvcuda.dll", EntryPoint = "cuMemAllocPitch_v2")] static extern int cuMemAllocPitch(out ulong dptr, out UIntPtr pitch, UIntPtr widthBytes, UIntPtr height, uint elementSize);
  [DllImport("nvcuda.dll", EntryPoint = "cuMemFree_v2")] static extern int cuMemFree(ulong dptr);
  [DllImport("nvcuda.dll", EntryPoint = "cuMemcpyHtoD_v2")] static extern int cuMemcpyHtoD(ulong dst, byte[] src, UIntPtr bytes);
  [DllImport("nvEncodeAPI64.dll")] static extern int NvEncodeAPICreateInstance(IntPtr functionList);

  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int FOpen(IntPtr p, out IntPtr enc);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int FPreset(IntPtr enc, Guid codec, Guid preset, int tuning, IntPtr p);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int FP(IntPtr enc, IntPtr p);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int F0(IntPtr enc);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate IntPtr FErr(IntPtr enc);

  // nvEncodeAPI.h 13.1, Windows x64 (clang -target x86_64-pc-windows-msvc).
  const int SZ_FL = 2552, FL_OPEN = 240, FL_PRESETEX = 320, FL_INIT = 96, FL_CREATEIN = 104, FL_DESTROYIN = 112,
    FL_CREATEBS = 120, FL_DESTROYBS = 128, FL_ENCODE = 136, FL_LOCKBS = 144, FL_UNLOCKBS = 152, FL_LOCKIN = 160,
    FL_UNLOCKIN = 168, FL_MAP = 208, FL_UNMAP = 216, FL_DESTROY = 224, FL_REGISTER = 248, FL_UNREGISTER = 256,
    FL_LASTERR = 304;
  const uint V_FL = 1895956493, V_OPEN = 1895890957, API_VERSION = 16777229, V_PRESET = 4043636749,
    V_CFG = 4043898893, V_INIT = 4043767821, V_CIN = 1895956493, V_CBS = 1895890957, V_LIN = 1895890957,
    V_PIC = 4043767821, V_LBS = 4043440141, V_REG = 1896153101, V_MAP = 1896087565;
  const int SZ_OPEN = 1552, OPEN_DEVTYPE = 4, OPEN_DEVICE = 8, OPEN_APIVER = 24;
  const int SZ_PRESET = 5128, PRESET_CFG = 8, SZ_CFG = 3584, CFG_GOP = 20, CFG_FRAMEP = 24, CFG_RC = 40,
    CFG_CODEC = 168, H264_IDR = 8, HEVC_IDR = 20, RC_MODE = 4, RC_AVG = 20, RC_MAX = 24;
  const int SZ_INIT = 1800, INIT_GUID = 4, INIT_PRESET = 20, INIT_W = 36, INIT_H = 40, INIT_DARW = 44,
    INIT_DARH = 48, INIT_FNUM = 52, INIT_FDEN = 56, INIT_PTD = 64, INIT_CFG = 88, INIT_MAXW = 96,
    INIT_MAXH = 100, INIT_TUNING = 136;
  const int SZ_CIN = 776, CIN_W = 4, CIN_H = 8, CIN_FMT = 16, CIN_BUF = 24, SZ_CBS = 776, CBS_BUF = 16;
  const int SZ_LIN = 1544, LIN_BUF = 8, LIN_PTR = 16, LIN_PITCH = 24;
  const int SZ_PIC = 3360, PIC_W = 4, PIC_H = 8, PIC_PITCH = 12, PIC_TS = 24, PIC_IN = 40, PIC_OUT = 48,
    PIC_FMT = 64, PIC_STRUCT = 68;
  const int SZ_LBS = 1544, LBS_BUF = 8, LBS_SIZE = 36, LBS_PTR = 56;
  const int SZ_REG = 1536, REG_TYPE = 4, REG_W = 8, REG_H = 12, REG_PITCH = 16, REG_RES = 24, REG_OUT = 32,
    REG_FMT = 40, REG_USAGE = 44;
  const int SZ_MAP = 1544, MAP_RES = 16, MAP_OUT = 24, MAP_FMT = 32;
  const int DEV_CUDA = 1, FMT_NV12 = 1, RC_CBR = 2, TUNING_ULL = 3, PIC_FRAME = 1, RES_CUDAPTR = 1, INPUT_IMAGE = 0;
  static readonly Guid H264 = new Guid(0x6bc82762, 0x4e63, 0x4ca4, 0xaa, 0x85, 0x1e, 0x50, 0xf3, 0x21, 0xf6, 0xbf);
  static readonly Guid HEVC = new Guid(0x790cdc88, 0x4522, 0x4d7b, 0x94, 0x25, 0xbd, 0xa9, 0x97, 0x5f, 0x76, 0x03);
  static readonly Guid P1 = new Guid(0xfc0a8d3e, 0x45f8, 0x4cf8, 0x80, 0xc7, 0x29, 0x88, 0x71, 0x59, 0x0e, 0xbf);

  static IntPtr fl, enc;
  static T Fn<T>(int off) where T : class {
    return Marshal.GetDelegateForFunctionPointer(Marshal.ReadIntPtr(fl, off), typeof(T)) as T;
  }
  static IntPtr Block(int size, uint version) {
    IntPtr p = Marshal.AllocHGlobal(size);
    for (int i = 0; i < size; i++) Marshal.WriteByte(p, i, 0);
    Marshal.WriteInt32(p, 0, (int)version);
    return p;
  }
  static void Cu(int r, string what) { if (r != 0) throw new Exception(what + ": CUresult " + r); }
  static void Nv(int r, string what) {
    if (r == 0) return;
    string last = "";
    if (enc != IntPtr.Zero) { IntPtr s = Fn<FErr>(FL_LASTERR)(enc); if (s != IntPtr.Zero) last = Marshal.PtrToStringAnsi(s); }
    throw new Exception(what + ": NVENCSTATUS " + r + " " + last);
  }
  static int W, H;
  static byte PatY(int x, int y, int i) {
    int bx = (i * 8) % (W > 256 ? W - 256 : 1), by = H / 3;
    if (x >= bx && x < bx + 256 && y >= by && y < by + 256) return 235;
    return (byte)(16 + ((x + y + 4 * i) / 4) % 200);
  }
  // The NV12 test pattern of nvenc-probe.c, frame i, into buf at pitch.
  static void Fill(byte[] buf, int pitch, int i) {
    for (int y = 0; y < H; y++) { int row = y * pitch; for (int x = 0; x < W; x++) buf[row + x] = PatY(x, y, i); }
    int uv = H * pitch;
    for (int y = 0; y < H / 2; y++) {
      int row = uv + y * pitch;
      for (int x = 0; x < W / 2; x++) {
        buf[row + 2 * x] = (byte)(128 + ((x / 8 + i) % 32) - 16);
        buf[row + 2 * x + 1] = (byte)(128 + ((y / 8) % 32) - 16);
      }
    }
  }

  public static string Run(string mode, string codecName, int n, int kbit, int w, int h) {
    W = w; H = h;
    var s = new System.Text.StringBuilder();
    Guid codec = codecName == "hevc" ? HEVC : H264;
    var clock = System.Diagnostics.Stopwatch.StartNew();
    Cu(cuInit(0), "cuInit");
    int dev; Cu(cuDeviceGet(out dev, 0), "cuDeviceGet");
    IntPtr ctx; Cu(cuCtxCreate(out ctx, 0, dev), "cuCtxCreate");
    fl = Block(SZ_FL, V_FL);
    Nv(NvEncodeAPICreateInstance(fl), "NvEncodeAPICreateInstance");
    IntPtr op = Block(SZ_OPEN, V_OPEN);
    Marshal.WriteInt32(op, OPEN_DEVTYPE, DEV_CUDA);
    Marshal.WriteIntPtr(op, OPEN_DEVICE, ctx);
    Marshal.WriteInt32(op, OPEN_APIVER, (int)API_VERSION);
    Nv(Fn<FOpen>(FL_OPEN)(op, out enc), "nvEncOpenEncodeSessionEx");
    s.AppendLine(String.Format("INFO NVENC session opened in {0:F1} ms", clock.Elapsed.TotalMilliseconds));

    IntPtr pc = Block(SZ_PRESET, V_PRESET);
    Marshal.WriteInt32(pc, PRESET_CFG, (int)V_CFG);
    Nv(Fn<FPreset>(FL_PRESETEX)(enc, codec, P1, TUNING_ULL, pc), "nvEncGetEncodePresetConfigEx");
    IntPtr cfg = Marshal.AllocHGlobal(SZ_CFG);
    byte[] tmp = new byte[SZ_CFG]; Marshal.Copy(pc + PRESET_CFG, tmp, 0, SZ_CFG); Marshal.Copy(tmp, 0, cfg, SZ_CFG);
    Marshal.WriteInt32(cfg, CFG_RC + RC_MODE, RC_CBR);
    Marshal.WriteInt32(cfg, CFG_RC + RC_AVG, kbit * 1000);
    Marshal.WriteInt32(cfg, CFG_RC + RC_MAX, kbit * 1000);
    Marshal.WriteInt32(cfg, CFG_GOP, 120);
    Marshal.WriteInt32(cfg, CFG_FRAMEP, 1);
    Marshal.WriteInt32(cfg, CFG_CODEC + (codecName == "hevc" ? HEVC_IDR : H264_IDR), 120);

    IntPtr ip = Block(SZ_INIT, V_INIT);
    Marshal.StructureToPtr(codec, ip + INIT_GUID, false);
    Marshal.StructureToPtr(P1, ip + INIT_PRESET, false);
    Marshal.WriteInt32(ip, INIT_TUNING, TUNING_ULL);
    Marshal.WriteInt32(ip, INIT_W, w); Marshal.WriteInt32(ip, INIT_H, h);
    Marshal.WriteInt32(ip, INIT_DARW, w); Marshal.WriteInt32(ip, INIT_DARH, h);
    Marshal.WriteInt32(ip, INIT_FNUM, 60); Marshal.WriteInt32(ip, INIT_FDEN, 1);
    Marshal.WriteInt32(ip, INIT_PTD, 1);
    Marshal.WriteIntPtr(ip, INIT_CFG, cfg);
    Marshal.WriteInt32(ip, INIT_MAXW, w); Marshal.WriteInt32(ip, INIT_MAXH, h);
    double t1 = clock.Elapsed.TotalMilliseconds;
    Nv(Fn<FP>(FL_INIT)(enc, ip), "nvEncInitializeEncoder");
    s.AppendLine(String.Format("INFO NVENC encoder initialised {0}x{1} {2} P1 ultra-low-latency CBR {3} kbit/s in {4:F1} ms (mode {5})",
      w, h, codecName == "hevc" ? "HEVC" : "H.264", kbit, clock.Elapsed.TotalMilliseconds - t1, mode));

    IntPtr cbs = Block(SZ_CBS, V_CBS);
    Nv(Fn<FP>(FL_CREATEBS)(enc, cbs), "nvEncCreateBitstreamBuffer");
    IntPtr bs = Marshal.ReadIntPtr(cbs, CBS_BUF);

    // Inputs: one locked driver buffer (lock), or 8 registered CUDA frames (vram).
    IntPtr inbuf = IntPtr.Zero; int k = 8;
    ulong[] dptr = new ulong[k]; IntPtr[] reg = new IntPtr[k]; int pitch = 0;
    if (mode == "lock") {
      IntPtr cin = Block(SZ_CIN, V_CIN);
      Marshal.WriteInt32(cin, CIN_W, w); Marshal.WriteInt32(cin, CIN_H, h); Marshal.WriteInt32(cin, CIN_FMT, FMT_NV12);
      Nv(Fn<FP>(FL_CREATEIN)(enc, cin), "nvEncCreateInputBuffer");
      inbuf = Marshal.ReadIntPtr(cin, CIN_BUF);
    } else {
      double tu = clock.Elapsed.TotalMilliseconds;
      for (int j = 0; j < k; j++) {
        UIntPtr p;
        Cu(cuMemAllocPitch(out dptr[j], out p, (UIntPtr)w, (UIntPtr)(h * 3 / 2), 16), "cuMemAllocPitch");
        pitch = (int)p;
        byte[] frame = new byte[pitch * h * 3 / 2];
        Fill(frame, pitch, j);
        Cu(cuMemcpyHtoD(dptr[j], frame, (UIntPtr)frame.Length), "cuMemcpyHtoD");
        IntPtr rr = Block(SZ_REG, V_REG);
        Marshal.WriteInt32(rr, REG_TYPE, RES_CUDAPTR);
        Marshal.WriteInt32(rr, REG_W, w); Marshal.WriteInt32(rr, REG_H, h); Marshal.WriteInt32(rr, REG_PITCH, pitch);
        Marshal.WriteInt64(rr, REG_RES, (long)dptr[j]);
        Marshal.WriteInt32(rr, REG_FMT, FMT_NV12); Marshal.WriteInt32(rr, REG_USAGE, INPUT_IMAGE);
        Nv(Fn<FP>(FL_REGISTER)(enc, rr), "nvEncRegisterResource");
        reg[j] = Marshal.ReadIntPtr(rr, REG_OUT);
      }
      s.AppendLine(String.Format("INFO {0} NV12 frames in CUDA device memory (pitch {1}), uploaded and registered in {2:F0} ms",
        k, pitch, clock.Elapsed.TotalMilliseconds - tu));
    }

    byte[] host = mode == "lock" ? new byte[1] : null;
    double[] lat = new double[n]; long total = 0; double fill = 0;
    bool startCode = false, sps = false;
    IntPtr pp = Block(SZ_PIC, V_PIC), lb = Block(SZ_LBS, V_LBS), li = Block(SZ_LIN, V_LIN), mp = Block(SZ_MAP, V_MAP);
    var te = System.Diagnostics.Stopwatch.StartNew();
    for (int i = 0; i < n; i++) {
      IntPtr input; int inPitch; int fmt = FMT_NV12;
      if (mode == "lock") {
        double tf = te.Elapsed.TotalMilliseconds;
        Marshal.WriteInt32(li, 0, (int)V_LIN);
        Marshal.WriteIntPtr(li, LIN_BUF, inbuf);
        Nv(Fn<FP>(FL_LOCKIN)(enc, li), "nvEncLockInputBuffer");
        IntPtr data = Marshal.ReadIntPtr(li, LIN_PTR); inPitch = Marshal.ReadInt32(li, LIN_PITCH);
        int bytes = inPitch * h * 3 / 2;
        if (host.Length != bytes) host = new byte[bytes];
        Fill(host, inPitch, i);
        Marshal.Copy(host, 0, data, bytes);
        Nv(Fn<FP>(FL_UNLOCKIN)(enc, inbuf), "nvEncUnlockInputBuffer");
        fill += te.Elapsed.TotalMilliseconds - tf;
        input = inbuf;
      } else {
        Marshal.WriteIntPtr(mp, MAP_RES, reg[i % k]);
        Nv(Fn<FP>(FL_MAP)(enc, mp), "nvEncMapInputResource");
        input = Marshal.ReadIntPtr(mp, MAP_OUT); fmt = Marshal.ReadInt32(mp, MAP_FMT); inPitch = pitch;
      }
      double ts = te.Elapsed.TotalMilliseconds;
      for (int b = 4; b < SZ_PIC; b += 4) Marshal.WriteInt32(pp, b, 0);
      Marshal.WriteInt32(pp, PIC_W, w); Marshal.WriteInt32(pp, PIC_H, h); Marshal.WriteInt32(pp, PIC_PITCH, inPitch);
      Marshal.WriteInt64(pp, PIC_TS, i);
      Marshal.WriteIntPtr(pp, PIC_IN, input); Marshal.WriteIntPtr(pp, PIC_OUT, bs);
      Marshal.WriteInt32(pp, PIC_FMT, fmt); Marshal.WriteInt32(pp, PIC_STRUCT, PIC_FRAME);
      Nv(Fn<FP>(FL_ENCODE)(enc, pp), "nvEncEncodePicture");
      for (int b = 4; b < SZ_LBS; b += 4) Marshal.WriteInt32(lb, b, 0);
      Marshal.WriteIntPtr(lb, LBS_BUF, bs);
      Nv(Fn<FP>(FL_LOCKBS)(enc, lb), "nvEncLockBitstream");
      lat[i] = (te.Elapsed.TotalMilliseconds - ts) * 1000.0;
      int size = Marshal.ReadInt32(lb, LBS_SIZE);
      total += size;
      if (i == 0 && size > 8) {
        byte[] head = new byte[Math.Min(size, 256)];
        Marshal.Copy(Marshal.ReadIntPtr(lb, LBS_PTR), head, 0, head.Length);
        startCode = head[0] == 0 && head[1] == 0 && (head[2] == 1 || (head[2] == 0 && head[3] == 1));
        for (int b = 0; b + 4 < head.Length; b++)
          if (head[b] == 0 && head[b + 1] == 0 && head[b + 2] == 1) {
            int t = codecName == "hevc" ? (head[b + 3] >> 1) & 0x3f : head[b + 3] & 0x1f;
            if (t == (codecName == "hevc" ? 33 : 7)) sps = true;
          }
      }
      Nv(Fn<FP>(FL_UNLOCKBS)(enc, bs), "nvEncUnlockBitstream");
      if (mode == "vram") Nv(Fn<FP>(FL_UNMAP)(enc, input), "nvEncUnmapInputResource");
    }
    double secs = te.Elapsed.TotalSeconds;
    double[] ls = (double[])lat.Clone(); Array.Sort(ls);
    s.AppendLine(String.Format("ENC frames={0} bytes={1} seconds={2:F3} fps={3:F1} fill_ms_total={4:F0} lat_us_med={5:F0} lat_us_p99={6:F0} lat_us_max={7:F0} kbit_per_s_at_60fps={8:F0} mode={9}",
      n, total, secs, n / secs, fill, ls[n / 2], ls[(n * 99) / 100], ls[n - 1], total * 8.0 / 1000 / (n / 60.0), mode));
    s.AppendLine(String.Format("BITSTREAM start_code={0} sps={1}", startCode ? 1 : 0, sps ? 1 : 0));
    if (mode == "lock") Nv(Fn<FP>(FL_DESTROYIN)(enc, inbuf), "nvEncDestroyInputBuffer");
    else for (int j = 0; j < k; j++) { Nv(Fn<FP>(FL_UNREGISTER)(enc, reg[j]), "nvEncUnregisterResource"); cuMemFree(dptr[j]); }
    Nv(Fn<FP>(FL_DESTROYBS)(enc, bs), "nvEncDestroyBitstreamBuffer");
    Nv(Fn<F0>(FL_DESTROY)(enc), "nvEncDestroyEncoder");
    enc = IntPtr.Zero;
    cuCtxDestroy(ctx);
    s.AppendLine(startCode && sps && total > 0 ? "RESULT PASS" : "RESULT FAIL bitstream has no start code or SPS");
    return s.ToString();
  }
}
}
'@
Add-Type -TypeDefinition $src -Language CSharp
try { [CaraxesNvenc.Probe]::Run($Mode, $Codec, $Frames, $Kbit, $Width, $Height) }
catch { "RESULT FAIL " + $_.Exception.GetBaseException().Message; exit 1 }
