<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Video engines: NVENC and NVDEC

A streamed desktop or game encodes every frame. On the vCPUs (x264) that costs about two
guest cores at 1080p. The GPU's NVENC encodes from VRAM for almost no CPU. NVDEC does the
same for video playback. Both are falcon engines with their own runlists, reached through a
channel and one class each (TU106: `NVC4B7_VIDEO_ENCODER`, `NVC4B0_VIDEO_DECODER`).

## How the stock driver uses them

- User space (`libnvidia-encode`, `libnvcuvid`, on top of a CUDA context) makes the calls:
  - legacy GPU and PERF controls around each session, which reach GSP;
  - the caps controls;
  - a bare channel on the device with engineType NVENC0 or NVDEC0 (no TSG, the device's
    default VA space), bound and scheduled, then a first push it waits for;
  - the engine object.
- At the object's allocation the kernel RM:
  - allocates the falcon context and maps it into the channel's VA space, first fit
    (`kflcnAllocContext`, `falcon/kernel_falcon.c:279-299`; `kchannelMapEngineCtxBuf`,
    `kernel_channel.c:3801-3870`);
  - promotes it to GSP by VA alone (`_kflcnPromoteContext`, `kernel_falcon.c:184-276`).
- Completions are non-stall interrupts that the kernel RM services itself
  (`gkflcnRegisterIntrService`, `kernel_falcon.c:362-400`) and turns into events on
  subdevice notifiers 14 (NVDEC0) and 38 (NVENC0).

## VirtIO Leandro

- The guest's channel and object are host objects created by the backend, so NVENC works as
  on the host. Sunshine `capture=kms` with NVENC passed a client-frame check (2026-09-19).

## VFIO Leandro

- The session controls are forwarded to the host as GSS legacy controls.
- The channel is mirrored on the shadow VA space of the device's default VA space. It runs in
  a host TSG of its own and is bound to its engine at its first doorbell.
- The guest's video object is deferred. The guest's context page first leaves the shadow,
  and only then is the host object allocated, so the host's first-fit context lands in that
  hole.
- Only the chip's own video classes are served; anything else is refused (fail closed).
- The completion vectors are NVDEC0 8 and NVENC0 9 on TU106.
- Two designs failed and were removed:
  - Reserving free VA to steer the host's context: an RM allocation storm that froze the host.
  - Holding the first doorbell until the object: a deadlock, because the guest waits for
    that push.
- Status: the session opens; no guest frame has been encoded yet.

| Measure | Value | Where |
|---|---|---|
| native probe, 300 frames 1080p H.264 | 160.7 fps, 2.9 ms median latency, decode PSNR ≥ 63.4 dB | host |
| guest probe | pending | vm2, 610 guest |

## Depth

- [caraxes/video-engines.md](../caraxes/video-engines.md): what the guest sends, the design,
  the failure modes, what the next probe must show.
- [display-and-kms.md](display-and-kms.md): capture and streaming.
- [execution-mirror.md](execution-mirror.md): mirror channels and shadow VA spaces.
