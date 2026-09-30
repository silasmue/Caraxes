# Codec agent: merge-train note (2026-09-30 02:55)

Branch `codec-2026-09-30` (worktree /mnt/vmstore/caraxes/wt-codec), based on win-lf18-2026-09-29
991e3d86 (lf24).

## Commits

- 052f8611 tools/windows: present-probe references System.Drawing
  - Why: 73e971b0 reads `Screen.PrimaryScreen.Bounds` (System.Drawing.Rectangle), but Add-Type
    referenced only System.Windows.Forms. The C# failed to compile in the guest (w4-lf24,
    present-status: AddTypeCompilerError at line 55), so no 60 fps present load ran during
    Sunshine streams.
  - Effect: tools only; no device change, no cargo test needed (device crate untouched:
    baseline `cargo test -p vfio-user-nvgpu` on 991e3d86: 844 passed).

## No device fix was needed for NVENC/NVDEC correctness on lf24

- Windows: Sunshine encoder=nvenc streams (w3, w4); the NVENC async completion event works
  (D3D11 + enableEncodeAsync probe: wait median 3.8 ms, 0 timeouts). NVDEC is bit-exact with
  d3d11va and cuda. AV1 fails cleanly.
- Linux B: every codec test passes and is bit-exact; AV1 fails cleanly.
- Evidence: matrix.md in this directory; logs win-w3-lf24/, win-w4-lf24/, linux-l1-lf24/, and
  the kit run dirs windows-plain/w1-lf24, w3-lf24, w4-lf24 (traces zstd).

## Open (not in the train)

- Windows NVENC host processing latency in Sunshine: avg 31-70 ms against x264 12-14 ms (static
  desktop, 21 fps). A 60 fps measurement is owed (needs the present-probe fix).
- The Windows-only FIFO control 0x20801111 on class 0xB297 objects comes twice per frame in the
  NVENC stream. The device only acknowledges it; its semantics are unknown.
- Linux Sunshine nvenc/x264 comparison: session l2 is queued behind locks/priority.
