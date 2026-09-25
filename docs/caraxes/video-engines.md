<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Video engines: NVENC and NVDEC through the mirror

Status (2026-09-25): the NVENC session opens in the guest. The mirror builds the guest's
NVENC channel on the host. No guest frame has been encoded yet. The deferred-object design
(§3) is unit-tested, but it has had no GPU run: the only run of it was stopped by an
unrelated host BAR1 exhaustion.

- Records: `caraxes/runs/nvenc-2026-09-25/`.
- Probe: `caraxes/tools/nvenc-probe.c`.
- Citations: the 615.71.09 tree.

![Video engine mirror](figures/video-engine-mirror.svg)

## 1. The probe

- It uses the NVENC API and cuvid directly, through FFmpeg's nv-codec-headers (MIT, SDK 13.1).
  The headers are fetched without any identifier into `target/nvenc/ffnvcodec`. The
  libraries are loaded at run time, so the guest needs no CUDA toolkit.
- Steps:
  1. Open a CUDA context and an NVENC session.
  2. Encode N frames of a moving pattern to H.264 (P1, ultra-low latency, CBR).
  3. Check the Annex-B stream: SPS, PPS and IDR are present, and the SPS picture size is
     the one requested.
  4. Decode the stream with NVDEC and compare the luma (PSNR).
- Output: an `ENC` line (fps, latency), `BITSTREAM`, `DEC`, `RESULT PASS|FAIL`.
- Native run on the host (`native/`): 300 frames at 1080p, 160.7 fps, 2.9 ms median
  latency, 8.4 Mbit/s at 60 fps. Decode minimum 63.4 dB. Result: PASS.
- The 610 image has `libnvidia-encode.so.1` and `libnvcuvid.so.1` in `/opt/nvrm/lib`.

## 2. What the guest sends (610 guest)

| Step | Guest RPC | Device before | Device now |
|---|---|---|---|
| open session | GSS legacy `0x20808165`, `0x2080A028`, `0x20808163`; `0x20808164` at close; NV_OK natively (`matrix/traces/610.57.04/nvenc.jsonl:2059-2205`, `:6325`) | unanswered: `NV_ENC_ERR_INCOMPATIBLE_CLIENT_KEY` (21) | forwarded to the host (`ver/v610.rs` `LEGACY`) |
| caps | `NVENC_GET_CAPS_V2`, `NVDEC_GET_CAPS_V2` | chip table | unchanged |
| VAS | `FERMI_VASPACE_A` with index GPU_DEVICE, `COPY_SERVER_RESERVED_PDES`, FREE (`gpu_vaspace.c:3877-3921`) | root forgotten with the handle | root kept under `(hClient, hDevice)` |
| channel | `TURING_CHANNEL_GPFIFO_A` under the device, engineType 0x1b, no TSG, no VAS; BIND; SCHEDULE | software executor; `nvEncInitializeEncoder` status 20 | host channel in its own TSG on the shadow VAS, bound to NVENC0 |
| first push | a doorbell before any engine object; the guest waits for it | — | runs on the host engine |
| context | `GPU_PROMOTE_CTX` with entryCount 0 and a top-level `virtAddress`/`size`: 0x12002d000 + 0x1000 (`falcon/kernel_falcon.c:184-276`) | ignored | recorded; kept out of set A |
| object | `NVC4B7_VIDEO_ENCODER` under the channel, no params | recorded | deferred, then allocated on the host (§3) |

## 3. The deferred-object design

- Eligibility:
  - A video channel (engineType NVENCn or NVDECn) is mirrored without a TSG, in a host TSG
    of its own.
  - Its objects must be video classes of the chip's class database (`..B7`, `..B0`).
    Anything else is refused and the channel stays in software.
- Build: at the first doorbell the host channel is bound to its engine (BIND) and
  scheduled, so the first push runs.
- The guest's video object, in order (`Mirror::place_video_object`, called after the
  allocation):
  1. The guest's context range leaves set A. The guest promotes it just before the
     allocation (`falcon/kernel_falcon.c:294-298`).
  2. The shadow is re-synced, and the unmap is waited for.
  3. The host object is allocated. The host RM maps the host's context first fit
     (`kchannelMapEngineCtxBuf`, `kernel_channel.c:3860-3870`), so it lands in the hole the
     guest's context left.
- Refusals: an object with no promoted context is refused, and so is a second video object
  on the same channel.
- Completion: NVDEC0 and NVENC0 have non-stall vectors 8 and 9 (`chips/tu106/interrupts.toml`).
  Host OS events on notifiers 14 and 38 raise them, as for GR and CE. The NVENC path
  was seen raising vector 9 in `probe-mirror1`.
- Cleanup: the guest-page descriptors of a mirrored channel (USERD, error notifier) are
  freed with the channel.

## 4. Failure modes found

| Run | Design | What happened |
|---|---|---|
| `probe-mirror1` | object allocated when the guest allocates it, before the guest's context left the shadow | the host context took a free VA that the guest filled later (a 2 MiB leaf at 0x120400000 was refused in the shadow); Xid 31 FAULT_PTE on NVENC0 at 0x1205fe000, pid 121037 |
| `probe-mirror2`: the VA reservation storm | object at doorbell 1, after reserving every free VA of [4.5 GiB, 1 TiB), split recursively down to 64 KiB | 3576 fixed reservations per build (48 s); 11435 host `virtmemAllocResources ... NO_MEMORY` lines; host VRAM down to 431 MiB; NVENC0 FAULT_PDE at VA 0 (twice, pid 196394); the desktop froze and the host was rebooted at 11:44. The code is removed, and no mirror path reserves VA speculatively or retries by splitting |
| `probe-hold`: the doorbell-hold deadlock | doorbell 1 held until the object | the guest waits for its first push before it allocates the object; it stayed held for 10 s, then `nvEncInitializeEncoder` status 20. No Xid, no host NVRM line, host VRAM 3749 MiB free after |
| `probe-defer`, `probe-defer2` | the deferred-object design | both stopped at host BAR1 exhaustion during the probe step's driver reload, before any CUDA or NVENC call; no result. The second time the game VM had no pool, and more than 130 MiB were taken within a second, which exceeds vm2's 96 MiB budget; the source is open (`probe-defer2/README.md`). `run.sh` now samples host BAR1 every 0.5 s |

## 5. What the next probe must show

- 610 guest on vm2, alone on the host (the coordinator's decision after probe-defer2):
  - [ ] `nvEncInitializeEncoder` succeeds.
  - [ ] 300 frames at 1080p encode, and the `ENC` line gives the fps and latency.
  - [ ] The stream passes the header check: SPS 1920x1080, IDR present.
  - [ ] NVDEC decodes all 300 frames, PSNR ≥ 30 dB.
- Device log:
  - [ ] `guest context [0x12002d000, ...) leaves set A`, then `host video object class 0xc4b7`.
  - [ ] No `shadow: map va ... refused` for the channel's root. A refusal means the host's
    context is larger than the guest's hole and landed elsewhere.
- Host:
  - [ ] No NVRM line.
  - [ ] `nvidia-smi encodersessions` lists the session under the device pid.
  - [ ] VRAM free is unchanged after vm2 goes down.
- Then the 615 guest on vm2-615, the NVDEC-only path, and Sunshine with `encoder = nvenc`
  on the game VM (1080p60 at 20 Mbit/s against the x264 baseline of 2.09 guest cores).

## 6. Limits

- One video object per channel, and only TU106's NVC4B7 and NVC4B0 so far.
- The design relies on the host's video context fitting the guest's hole: same chip, and the
  same context size in both drivers.
- The NVENC engine is shared with the host desktop's encoder sessions and the game VM's.
