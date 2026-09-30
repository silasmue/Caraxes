<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Video engines: NVENC and NVDEC

A streamed desktop or game encodes every frame. With x264 on the vCPUs Sunshine took 1.4
guest cores at 1080p60; with NVENC it encodes from VRAM for a quarter of a core. NVDEC does
the same for video playback. Both are falcon engines with their own runlists, reached
through a channel and one class each (TU106: `NVC4B7_VIDEO_ENCODER`, `NVC4B0_VIDEO_DECODER`).
In VFIO Leandro the guest's video channels run on host mirror channels; every codec TU106
has works on Linux and Windows, and NVDEC's output is bit-exact.

## How the stock driver uses them

- User space (`libnvidia-encode`, `libnvcuvid`, on top of a CUDA context) makes the calls:
  - legacy GPU and PERF controls around each session, which reach GSP;
  - the caps controls (`NVENC_GET_CAPS_V2`, `NVDEC_GET_CAPS_V2`);
  - a bare channel on the device with engineType NVENC0 or NVDEC0 (no TSG, the device's
    default VA space), bound and scheduled, then a first push it waits for;
  - the engine object.
- At the object's allocation the kernel RM allocates the falcon context and maps it into the
  channel's VA space first fit (`kflcnAllocContext`, `falcon/kernel_falcon.c`;
  `kchannelMapEngineCtxBuf`, `kernel_channel.c`), then promotes it to GSP by VA alone
  (`_kflcnPromoteContext`).
- Completions are non-stall interrupts that the kernel RM services itself
  (`gkflcnRegisterIntrService`) and turns into events on subdevice notifiers 14 (NVDEC0)
  and 38 (NVENC0).

## VirtIO Leandro

- The guest's channel and object are host objects created by the backend, so NVENC works as
  on the host. Sunshine `capture=kms` with NVENC passed a client-frame check (2026-09-19).

## VFIO Leandro

- The session controls are forwarded to the host as GSS legacy controls.
- The channel runs on a host mirror in a host TSG of its own, bound to its engine and
  scheduled at its first doorbell, so the guest's first push runs before the object exists.
- The video layout (`shadow.rs` `Layout::video`): a root first built for a video channel gets
  a shadow VA space with `vaBase` 0x11C000000, so the host's first fit for the falcon
  context lands in the host-only area [0x11C000000, 0x120000000), inside the guest's own
  GSP-RM range, which the guest never maps.
- The host object is placed by `Mirror::place_video_object`: a 4 KiB probe allocation
  before and after it shows that the context took the host area; otherwise the object is
  freed and the guest gets `NV_ERR_NOT_SUPPORTED`.
- Only the chip's own video classes are served; a channel carrying another class stays in
  the software executor (fail closed).
- The Windows KMD's own video channels are PRIVILEGED, like every kernel client's; they are
  mirrored unprivileged (bac4f5fa), and their 0x5080 object stays an inert host object
  ([research: windows-findings](../research/windows-findings.md), P29b, P31).
- Completion: vectors NVDEC0 8 and NVENC0 9 on TU106, raised from host OS events.
- Host clocks: without a boost the card ran the guest's video at P8 (ENC 90 to 98 fps
  against 165 native). The device holds `PERF_BOOST` {BOOST_TO_MAX, 3 s}, renewed at most
  once a second while the VM is active; on with `--host-exec`
  ([research: host-boost](../research/host-boost-2026-09-28.md)).
- Doorbells: the video libraries ring their own guest token and never read the pool token
  ([research: video-path](../research/video-path-2026-09-28.md)); the automatic guest-token
  ioeventfds include NVENC's runlist 7 on TU106
  ([caraxes/doorbell-token-pool.md](../caraxes/doorbell-token-pool.md) (g)).
- Two designs failed and were removed:
  - Reserving free VA to steer the host's context: an RM allocation storm that froze the host.
  - Holding the first doorbell until the object: a deadlock, because the guest waits for
    that push.

## Measured (codec matrix, 2026-09-30)

One binary, BtbN ffmpeg on both guests, 30 s 1080p60 clips; bit-exact means `framemd5` of
all 1800 frames against the host's software decode
([codec matrix](../../caraxes/runs/codec-2026-09-30/matrix.md)).

| Test, fps | Linux B (615) | Windows (617.14) |
|---|---|---|
| NVENC H.264, NVDEC to CUDA to NVENC | 419.9 | 315.1 (frame count check owed) |
| NVENC HEVC, same | 333.3 | 183.9 (same) |
| NVDEC H.264, `-hwaccel cuda` | 650.1, bit-exact | 587, bit-exact |
| NVDEC HEVC | 1310.0, bit-exact | 636, bit-exact |
| NVDEC VP9 | 900.0, bit-exact | 584, bit-exact |
| AV1 encode and decode | clean refusal | clean refusal |
| nvenc-probe, frames in VRAM | ENC 180.1, DEC 157.3 | ENC 164.2 |
| NVENC0 interrupt (vector 9), mean in the device's diagnostics | 54 us | 257 us |

- TU106 has no AV1 engine, natively too. A browser that gets AV1 decodes it in software.
- 0 host Xid on either guest.

| Sunshine, Linux B, 150 s at 60 fps (l3) | NVENC (HEVC) | x264 |
|---|---|---|
| Moonlight fps | 59.96 | 59.87 |
| host processing latency, mean | 5.9 ms | 18.3 ms |
| guest Sunshine CPU | 23.7 % of a core | 142.3 % |
| host Cloud Hypervisor CPU | 56.1 % of a core | 175.4 % |

- Windows: Sunshine `encoder = nvenc` streams, and the async completion event works (wait
  median 3.76 ms, none over 100 ms in 300 frames). On a static desktop at 21 fps its host latency
  was 70.2 ms mean against 12.3 ms for x264 (w4): open. The KMD sends the Windows-only FIFO
  control 0x20801111 twice per frame in NVENC mode; the device only acknowledges it.

## Limits

- One video object per channel, only the chip's NVC4B7 and NVC4B0.
- A root whose shadow VA space was built for GR or CE first has no host area; video objects
  on it are refused. The host area is 64 MiB.
- The NVENC engine is shared with the host desktop's encoder sessions and other VMs.

## Depth

- [caraxes/video-engines.md](../caraxes/video-engines.md): what the guest sends, the layout,
  the failure modes.
- [research/video-path-2026-09-28.md](../research/video-path-2026-09-28.md): one frame's time.
- [display-and-kms.md](display-and-kms.md): capture and streaming.
- [execution-mirror.md](execution-mirror.md): mirror channels and shadow VA spaces.
