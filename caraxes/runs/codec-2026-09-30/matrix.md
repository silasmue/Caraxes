# Codec matrix (night 2026-09-29/30, codec agent)

Binary lf24 (win-lf18-2026-09-29 991e3d86, sha a8e754b7), one binary for every guest.
ffmpeg: BtbN master N-126965-gd85cdd2597 (2026-09-29), the same build on both guests
(win64/linux64 GPL static, downloaded anonymously on the host, copied in).
Clips: 30 s 1080p60 testsrc2 from the host's software encoders (libx264 veryfast, libx265
ultrafast, libvpx-vp9 realtime, libsvtav1 preset 12; 20 Mbit/s). Reference: the host's software
decode as framemd5 (yuv420p). Scripts: linux/codec-matrix.sh, win/codec-matrix.ps1.

- Encode "gpu": `-hwaccel cuda -hwaccel_output_format cuda -i clip-h264.mp4 -c:v X_nvenc -preset p4
  -tune ll -b:v 20M` (NVDEC -> CUDA -> NVENC), 1800 frames; the output is counted (ffprobe) and
  decoded in software without an error.
- Encode "cpusrc": testsrc2 generated on the guest CPU, uploaded by ffmpeg.
- Decode speed: `-hwaccel cuda -hwaccel_output_format cuda -f null` (frames stay in VRAM).
- Decode bit-exact: `-hwaccel cuda -pix_fmt yuv420p -f framemd5` against the host's software decode.

## Linux B (desktop-b, 615 open, CH, slot 5; l1-lf24 2026-09-30 00:00)

| test | result | fps |
|---|---|---|
| NVENC H.264 gpu / cpusrc | PASS / PASS | 419.9 / 406.2 |
| NVENC HEVC gpu / cpusrc | PASS / PASS | 333.3 / 325.3 |
| NVENC AV1 | clean NEG: "Codec not supported", exit 187 | - |
| NVDEC H.264 cuda / h264_cuvid | PASS, framemd5 1800/1800 | 650.1 / 637.6 |
| NVDEC HEVC cuda / hevc_cuvid | PASS, framemd5 1800/1800 | 1310.0 / 1088.3 |
| NVDEC VP9 cuda / vp9_cuvid | PASS, framemd5 1800/1800 | 900.0 / 836.8 |
| NVDEC AV1 cuda / av1_cuvid | clean NEG: "hwaccel initialisation returned error" / "not supported" | - |
| nvenc-probe (gate) | PASS | ENC 180.1, DEC 157.3 |

- 0 guest Xid, 0 host Xid (device pid 1637908).
- Interrupt vectors over the session (device diag): NVENC0 (9) 7800 raises, 7794 acks, mean
  54 us, max 5.0 ms; NVDEC0 (8) 20143 raises, mean 37 us; GR0 (4) mean 45 us.

## Windows (windows-plain, 617.14, CH, slot 0, BAR1 pool 32 MiB, 8 vCPUs; w4-lf24 02:17-02:43)

| test | result | fps (Linux B) |
|---|---|---|
| NVENC H.264 gpu / cpusrc | runs, 1800 frames (count check: script bug, rerun owed) / PASS | 315.1 / 178.6 (420 / 406) |
| NVENC HEVC gpu / cpusrc | runs, 1800 frames (same) / PASS | 183.9 / 176.9 (333 / 325) |
| NVENC AV1 | clean NEG: "Codec not supported" | - |
| NVDEC H.264 d3d11va / cuda / cuvid | PASS, framemd5 1800/1800 (both hwaccels) | 329 / 587 / 293 (- / 650 / 638) |
| NVDEC HEVC d3d11va / cuda / cuvid | PASS, framemd5 1800/1800 | 392 / 636 / 261 (- / 1310 / 1088) |
| NVDEC VP9 d3d11va / cuda / cuvid | PASS, framemd5 1800/1800 | 377 / 584 / 227 (- / 900 / 837) |
| NVDEC AV1 d3d11va / cuda / cuvid | clean NEG (hwaccel init error / not supported) | - |
| nvenc-probe vram sync (CUDA) | PASS | 164.2 (w3; Linux probe 180) |
| NVENC async, D3D11 device, completion event (Sunshine's mode) | PASS, 300 frames | wait median 3.76 ms, p99 16.2, max 31.5, 0 over 100 ms |

- 0 host Xid in w3 and w4. Vector 9 (NVENC0) raised 5737 times in w3, 5726 acked, mean 257 us
  (Linux: mean 54 us).
- w1-lf24 (23:28): no user session, from the C: damage of the host disk-full at 22:48 (repaired
  in C95-C99 by the Windows agent).

## Sunshine (Windows): encoder=nvenc works

- C55's "NvEnc: frame 1 encode wait timeout" does not happen on lf24. Sunshine creates
  "h264_nvenc ... async" and streams (w3, w4). The async event path works, as the probe above
  shows.
- Both streams ran on a static desktop: the kit's present-probe.ps1 (73e971b0) fails to compile
  (Screen.PrimaryScreen needs System.Drawing; fixed in codec-2026-09-30 052f8611). Sunshine
  then sends about 21 fps on both encoders, so the CPU numbers below are small and not a real
  offload measurement. A 60 fps run is owed.

| w4 (150 s window from 15 s after the first video) | nvenc | x264 (software) |
|---|---|---|
| Moonlight fps | 21.1 | 21.4 |
| host processing latency min/max/avg | 5.4 / 1006 / 70.2 ms | 8.5 / 24.8 / 12.3 ms |
| guest Sunshine CPU (TotalProcessorTime) | 2.3 % of a core | 0.9 % |
| guest total CPU (8 vCPUs, WMI) | 1.5 % | 1.1 % |
| host Cloud Hypervisor | 28.4 % of a core | 54.1 % |
| host device | 16.5 % | 10.1 % |

- w3 (a window that included the encoder's creation): latency nvenc 6.0/303/31.3 ms against
  x264 9.4/23.3/13.9 ms; guest total CPU 10.9 % against 6.6 %.
- NVENC's host processing latency is higher and more variable than x264's: open. The w3 trace per
  frame: GR/CE doorbells, then NVENC three doorbells, its event about 1 ms later, three more
  doorbells, then about 41 ms of idle until the next frame (static-desktop pacing). The KMD
  sends the Windows-only FIFO control 0x20801111 (on its class 0xB297 objects 0xff008050/0xff008250,
  memory 0xff000100) twice per frame in NVENC mode only. The device only acknowledges it
  (recorded.rs FIFO_WINDOWS_1111). Its semantics are unknown, and so is whether the latency
  depends on it.

## Sunshine (Linux B): l2-lf24 (08:01-08:08, KMS capture, 150 s windows, vkcube load)

| | nvenc (HEVC) | x264 (software, H.264) |
|---|---|---|
| Moonlight fps | 0.16 (stalled after 3 s) | 59.7 |
| host processing latency avg | 20.2 ms | 17.9 ms |
| guest Sunshine CPU | 9.6 % of a core | 141.4 % of a core |
| guest total CPU (8 vCPUs) | 9.8 % | 24.7 % |
| host Cloud Hypervisor | 61.4 % of a core | 171.8 % |
| host device | 52.3 % | 51.4 % |

- The nvenc stall is not an NVENC fault. It comes from host VRAM exhaustion by the Windows VM
  running beside desktop-b (C120, Heaven/Superposition, slot A). The device's soak lines show
  1171-1428 MiB host FB free.
  - Device time 81.317 s: the shadow sync maps guest VA 0x1_2220_0000 (a 2 MiB leaf, FB); the host
    RM answers 0x51 NO_MEMORY; the leaf stays unmapped (fail closed).
  - 21 ms later, chid 44 (Sunshine's GL capture/convert context, an MME draw) reads 0x1_2222_6000
    in that leaf: host Xid 31 GPCCLIENT_T1_0 FAULT_PDE VIRT_READ. The guest gets RC 31, and its
    context dies.
  - Moonlight's IDR requests run from 08:03:00 to 08:03:03, then nothing more.
  - Evidence: linux-l2-lf24/trace-oom-fault-excerpt.txt, gpu-trace.log.zst, host dmesg Xid line
    (pid 3010445).
- The x264 half ran at full rate: 1.4 cores of guest CPU for Sunshine and 1.7 host cores for CH.
- Open (memory model, not codec): the shadow VAS's page tables are host VRAM outside the VM's
  reservation. Under host pressure, a guest mapping can fail where a real card never fails. The
  spec-true direction is to budget the shadow tables inside the VM's slice.
- Mistake of mine: l1 left `encoder = software` in desktop-b's Sunshine config (it was nvenc). The
  Windows agent's gate-fix2 kick-smoke at 07:58 therefore streamed x264 (H.264, 49.9 fps, 37 ms).
  l3 ends with nvenc to restore it.

## Sunshine (Linux B): l3-lf24 (08:16-08:23, card alone, KMS capture, vkcube load, 150 s windows)

| | nvenc (HEVC) | x264 (software, H.264) |
|---|---|---|
| Moonlight fps | 59.96 | 59.87 |
| host processing latency min/max/avg | 3.9 / 29.4 / 5.9 ms | 13.5 / 71.5 / 18.3 ms |
| guest Sunshine CPU | 23.7 % of a core | 142.3 % of a core |
| guest total CPU (8 vCPUs) | 7.8 % | 25.3 % |
| host Cloud Hypervisor | 56.1 % of a core | 175.4 % |
| host device | 50.3 % | 51.9 % |

- 0 host Xid. With nvenc, Sunshine needs 6x less guest CPU, CH needs 3x less host CPU, and the
  latency is 3x lower. desktop-b's Sunshine config ends at encoder = nvenc (restored).

## AV1 and YouTube

- TU106 (native too) has no AV1 encode or decode: NVENC does H.264/HEVC, NVDEC H.264, HEVC, VP8,
  VP9, MPEG-2, VC-1. Both AV1 cases fail cleanly (no hang, no Xid).
- A browser that gets AV1 from YouTube decodes it in software (dav1d) even on a native RTX 2070:
  the high CPU while watching YouTube is that, not the virtual GPU. Keeping the browser off AV1
  moves it to NVDEC: the enhanced-h264ify extension (block AV1), or media.av1.enabled=false in
  Firefox.
