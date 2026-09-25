<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Video engines: NVENC and NVDEC through the mirror

Status 2026-09-25: work in progress. The session opens. The mirror builds the guest's
NVENC channel on the host. No frame has been encoded in a guest yet. The deferred-object
design in §3 is unit-tested only; the hold design before it deadlocked (`probe-hold`).
Records: `caraxes/runs/nvenc-2026-09-25/`. Probe: `caraxes/tools/nvenc-probe.c`.
Citations are to the 615.71.09 tree.

## 1. The probe

- It uses the NVENC API and cuvid directly, through FFmpeg's nv-codec-headers (MIT, SDK 13.1).
  It opens a CUDA context and an NVENC session, then encodes N frames of a moving pattern
  to H.264 (P1, ultra-low latency, CBR). It checks the Annex-B stream (SPS, PPS, IDR, and
  the SPS picture size). It then decodes the stream with NVDEC and compares the luma
  (PSNR).
- Native run on the host (`native/`): 300 frames at 1080p, 160.7 fps, 2.9 ms median
  latency, 8.4 Mbit/s produced at 60 fps. Decode minimum 63.4 dB. Result: PASS.
- The 610 image has `libnvidia-encode.so.1` and `libnvcuvid.so.1` in `/opt/nvrm/lib`.

## 2. What the guest sends (610 guest, vm2)

| Step | Guest RPC | Device before | Now |
|---|---|---|---|
| open session | GSS legacy `0x20808165`, `0x2080A028`, `0x20808163` (open), `0x20808164` (close); NV_OK natively (`matrix/traces/610.57.04/nvenc.jsonl:2059-2205`, `:6325`) | unanswered: `NV_ENC_ERR_INCOMPATIBLE_CLIENT_KEY` (21) | forwarded to the host like the other GSS legacy controls (`ver/v610.rs` `LEGACY`) |
| caps | `NVENC_GET_CAPS_V2`, `NVDEC_GET_CAPS_V2` | chip table | unchanged |
| VAS | `FERMI_VASPACE_A` index GPU_DEVICE, `COPY_SERVER_RESERVED_PDES`, FREE: a transient handle for the device's default VAS (`gpu_vaspace.c:3877-3921`) | root forgotten with the handle | root kept under `(hClient, hDevice)` (`channel.rs` `device_vas`) |
| channel | `TURING_CHANNEL_GPFIFO_A` under the device, engineType 0x1b (NVENC0), no TSG, no VAS; BIND 0x1b; SCHEDULE | bare channel: software executor, `nvEncInitializeEncoder` fails with `NV_ENC_ERR_GENERIC` (20) | mirrored on the shadow VAS of the default VAS, in a host TSG of its own |
| first doorbell | before the video object exists | — | held (§3) |
| context | `GPU_PROMOTE_CTX` with entryCount 0 and the top-level `virtAddress`/`size` (`falcon/kernel_falcon.c:217-271`), 0x12002d000 + 0x1000 | ignored (entries only) | recorded as the falcon context and kept out of set A |
| object | `NVC4B7_VIDEO_ENCODER` under the channel, no params | recorded | the mirror is built now; the host allocates the same class |

## 3. The mirror

- A video channel (engineType NVENCn/NVDECn) is eligible without a TSG. Its objects must
  be the chip's video classes (`..B7`/`..B0` in the class database). Any other class is
  refused and the channel stays in software (fail closed).
- The host RM maps the host's video context into the shadow VAS at the first free VA
  (`kchannelMapEngineCtxBuf`, `kernel_channel.c:3860-3870`). The guest's later buffers
  may land at that VA:
  - Built at the first doorbell, the host context took a VA that the guest filled later.
  - The guest's 2 MiB leaf at 0x120400000 was then refused in the shadow.
  - Result: Xid 31 FAULT_PTE on NVENC0 at 0x1205fe000 (`probe-mirror1`).
- Holding the first doorbell until the object arrives deadlocks: the guest waits for its
  first push before it allocates NVC4B7. `probe-hold` (binary 872743e): the channel stayed
  held for 10 s, then `nvEncInitializeEncoder` failed with status 20. The run had no Xid,
  no host NVRM line, and host VRAM was 3749 MiB free afterwards.
- Now: the channel is built at its first doorbell and bound to its engine (BIND
  engineType), so the first push runs. The guest's NVC4B7 is deferred
  (`Mirror::place_video_object`, called after the allocation):
  - The guest's context range, promoted just before the allocation
    (`falcon/kernel_falcon.c:294-298`), leaves set A.
  - The worker unmaps it and is waited for.
  - Then the host object is allocated, so the host's first-fit context lands in that hole.
  - With no promoted context, the object is refused.
- Completion: NVDEC0 and NVENC0 have non-stall vectors 8 and 9 (`chips/tu106/interrupts.toml`).
  Host OS events on notifiers 14 and 38 raise them, as for GR and CE.
- The guest-page descriptors of a mirrored channel (USERD, error notifier) are freed with
  the channel. Before this they were freed only on a failed build.

## 4. The rejected attempt (probe-mirror2)

- Allocating the object at the first doorbell after reserving every free VA of
  [4.5 GiB, 1 TiB) failed:
  - It made 3576 fixed VA reservations per build, split recursively down to 64 KiB.
    The build took 48 s.
  - The host logged 11,435 `virtmemAllocResources ... NO_MEMORY` lines.
  - Host VRAM fell to 431 MiB free, and the desktop froze. The host was rebooted at 11:44.
  - NVENC0 then faulted at VA 0 (FAULT_PDE, twice, pid 196394).
- The code is gone. No path of the mirror now reserves VA speculatively or retries by
  splitting.

## 5. Open

- The hold design has no GPU run yet (encode, decode, 615 guest, Sunshine).
- If the host's context is larger than the guest's hole, it still lands elsewhere. A
  refused shadow leaf then shows the collision.
