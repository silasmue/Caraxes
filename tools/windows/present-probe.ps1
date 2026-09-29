# SPDX-License-Identifier: MIT
# D3D11 presents to a window on the synthetic GPU's display: a flip-model swap chain
# (DXGI_SWAP_EFFECT_FLIP_DISCARD) on a WinForms window, windowed, then exclusive fullscreen,
# then windowed again; each phase clears every frame to a new colour and presents with sync
# interval 1. Prints frames, time, rate and the swap chain's frame statistics per phase.
# -Seconds N: windowed only, phases of -Frames frames for N seconds (a soak load).
# -Width/-Height: the window's size (960x540); the screen's size or more makes it a borderless
# window over the whole screen (a game's borderless mode).
# Runs in the signed-in user's session (a scheduled task with an interactive principal), not
# as SYSTEM: session 0 has no desktop to present to. Stock Windows components only.
param([int]$Frames = 300, [int]$Seconds = 0, [int]$Width = 960, [int]$Height = 540)
$ErrorActionPreference = "Stop"
$src = @'
using System; using System.Runtime.InteropServices; using System.Windows.Forms;
public static class Present {
  [StructLayout(LayoutKind.Sequential)] public struct ModeDesc { public uint W, H, RefNum, RefDen, Format, Scanline, Scaling; }
  [StructLayout(LayoutKind.Sequential)] public struct SwapDesc {
    public ModeDesc Buffer; public uint SampleCount, SampleQuality, Usage, Count;
    public IntPtr Window; public int Windowed; public uint Effect, Flags;
  }
  [StructLayout(LayoutKind.Sequential)] public struct FrameStats { public uint PresentCount, PresentRefreshCount, SyncRefreshCount; public long SyncQPCTime, SyncGPUTime; }
  [DllImport("d3d11.dll")] static extern int D3D11CreateDeviceAndSwapChain(IntPtr adapter, int type, IntPtr sw, uint flags, IntPtr fl, uint nfl, uint sdk, ref SwapDesc desc, out IntPtr swap, out IntPtr dev, out int level, out IntPtr ctx);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int PresentFn(IntPtr self, uint sync, uint flags);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int GetBuffer(IntPtr self, uint i, ref Guid iid, out IntPtr surface);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int SetFullscreen(IntPtr self, int on, IntPtr output);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int ResizeBuffers(IntPtr self, uint n, uint w, uint h, uint format, uint flags);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int Stats(IntPtr self, out FrameStats s);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int CreateView(IntPtr self, IntPtr resource, IntPtr desc, out IntPtr view);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Targets(IntPtr self, uint n, ref IntPtr view, IntPtr depth);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void ClearFn(IntPtr self, IntPtr view, [In] float[] color);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate void Unbind(IntPtr self, uint n, IntPtr views, IntPtr depth);
  static T Fn<T>(IntPtr obj, int slot) { return (T)(object)Marshal.GetDelegateForFunctionPointer(Marshal.ReadIntPtr(Marshal.ReadIntPtr(obj), slot * IntPtr.Size), typeof(T)); }
  static void Check(int hr, string op) { if (hr < 0) throw new Exception(op + " HRESULT 0x" + hr.ToString("x8")); }
  static readonly Guid Texture2D = new Guid("6f15aaf2-d208-4e89-9ab4-489535d34f9c");
  static IntPtr View(IntPtr swap, IntPtr dev) {
    IntPtr tex, rtv; Guid g = Texture2D;
    Check(Fn<GetBuffer>(swap, 9)(swap, 0, ref g, out tex), "GetBuffer");
    try { Check(Fn<CreateView>(dev, 9)(dev, tex, IntPtr.Zero, out rtv), "CreateRenderTargetView"); }
    finally { Marshal.Release(tex); }
    return rtv;
  }
  static string Phase(string name, Form form, IntPtr swap, IntPtr dev, IntPtr ctx, int frames) {
    IntPtr rtv = View(swap, dev);
    var clock = System.Diagnostics.Stopwatch.StartNew(); int failed = 0; int hrLast = 0;
    try {
      for (int i = 0; i < frames; i++) {
        Application.DoEvents();
        float t = (i % 120) / 120f;
        Fn<Targets>(ctx, 33)(ctx, 1, ref rtv, IntPtr.Zero);
        Fn<ClearFn>(ctx, 50)(ctx, rtv, new float[] { t, 0.5f * (1 - t), 1 - t, 1 });
        int hr = Fn<PresentFn>(swap, 8)(swap, 1, 0);
        if (hr < 0) { failed++; hrLast = hr; }
      }
    } finally {
      Fn<Unbind>(ctx, 33)(ctx, 0, IntPtr.Zero, IntPtr.Zero);
      Marshal.Release(rtv);
    }
    clock.Stop();
    FrameStats s; int sh = Fn<Stats>(swap, 16)(swap, out s);
    return String.Format("{0}: {1} frames in {2} ms = {3:F1} fps, {4} failed present(s){5}; frame statistics {6}",
      name, frames, clock.ElapsedMilliseconds, frames * 1000.0 / Math.Max(1, clock.ElapsedMilliseconds), failed,
      failed > 0 ? " (last 0x" + hrLast.ToString("x8") + ")" : "",
      sh < 0 ? "hr 0x" + sh.ToString("x8") : String.Format("present {0}, present refresh {1}, sync refresh {2}", s.PresentCount, s.PresentRefreshCount, s.SyncRefreshCount));
  }
  public static string Run(int frames, int seconds, int w, int h) {
    var sb = new System.Text.StringBuilder();
    var form = new Form { Text = "caraxes present probe", Width = w, Height = h, StartPosition = FormStartPosition.CenterScreen };
    var screen = Screen.PrimaryScreen.Bounds;
    string kind = "windowed " + w + "x" + h;
    if (w >= screen.Width && h >= screen.Height) {
      form.FormBorderStyle = FormBorderStyle.None; form.StartPosition = FormStartPosition.Manual; form.Bounds = screen;
      kind = "borderless " + screen.Width + "x" + screen.Height;
    }
    form.Show(); Application.DoEvents();
    var d = new SwapDesc { Buffer = new ModeDesc { W = 0, H = 0, Format = 87 }, SampleCount = 1, Usage = 0x20, Count = 2,
                           Window = form.Handle, Windowed = 1, Effect = 4, Flags = 2 };
    IntPtr swap, dev, ctx; int level;
    Check(D3D11CreateDeviceAndSwapChain(IntPtr.Zero, 1, IntPtr.Zero, 0, IntPtr.Zero, 0, 7, ref d, out swap, out dev, out level, out ctx), "D3D11CreateDeviceAndSwapChain");
    try {
      sb.AppendLine(String.Format("device feature level {0:x}, flip-discard swap chain, 2 buffers, B8G8R8A8", level));
      if (seconds > 0) {
        var end = DateTime.Now.AddSeconds(seconds); int n = 0;
        while (DateTime.Now < end) { Console.WriteLine(Phase(kind + " #" + (++n), form, swap, dev, ctx, frames)); Console.Out.Flush(); }
        sb.Append("PASS: D3D11 flip-model windowed presents for " + seconds + " s, " + n + " phase(s)");
        return sb.ToString();
      }
      sb.AppendLine(Phase(kind, form, swap, dev, ctx, frames));
      Check(Fn<SetFullscreen>(swap, 10)(swap, 1, IntPtr.Zero), "SetFullscreenState(TRUE)");
      Check(Fn<ResizeBuffers>(swap, 13)(swap, 0, 0, 0, 0, 2), "ResizeBuffers(fullscreen)");
      sb.AppendLine(Phase("exclusive fullscreen", form, swap, dev, ctx, frames));
      Check(Fn<SetFullscreen>(swap, 10)(swap, 0, IntPtr.Zero), "SetFullscreenState(FALSE)");
      Check(Fn<ResizeBuffers>(swap, 13)(swap, 0, 0, 0, 0, 2), "ResizeBuffers(windowed)");
      sb.AppendLine(Phase("windowed again", form, swap, dev, ctx, frames));
      sb.Append("PASS: D3D11 flip-model presents windowed, exclusive fullscreen and windowed");
    } finally {
      Marshal.Release(ctx); Marshal.Release(dev); Marshal.Release(swap); form.Close();
    }
    return sb.ToString();
  }
}
'@
Add-Type -TypeDefinition $src -Language CSharp -ReferencedAssemblies System.Windows.Forms
[Present]::Run($Frames, $Seconds, $Width, $Height)
