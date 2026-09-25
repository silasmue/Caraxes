<!-- SPDX-License-Identifier: MIT -->
# probe-layout610: the video layout. NVENC encodes; NVDEC fails in the guest

- Setup:
  - Binary `vfio-user-nvgpu-main-nvenc-050ddf5`: the video layout,
    docs/caraxes/video-engines.md §3.
  - 610 guest on vm2, host alone.
- Device:
  - `shadow: host-managed VAS ... video layout: base 0x11c000000, host area [0x11c000000,
    0x120000000)`.
  - `host video object class 0xc4b7 0x800000b8: its context at 0x11c000000, in the host-only
    area`.
  - The NVDEC0 object (0xc4b0) was placed the same way.
  - 0 leaves refused by the layout.
- NVENC, from `probe.txt`:
  - The encoder initialised in 216 ms.
  - 300 frames at 1920x1080 H.264 P1 CBR in 2.862 s: 104.8 fps. Latency: median 5.5 ms,
    p99 25.1 ms, max 30.5 ms. 8.4 Mbit/s at 60 fps.
  - Bitstream: SPS 1920x1080, profile 100, level 4.2, 3 IDR, 297 slices, no bad start code.
- NVDEC: `cuvidCreateDecoder` returned CUDA error 2, so 0 frames and no PSNR.
  - The guest opened NVDEC0 and NVDEC1 channels and freed them 0.3 ms after NVDEC1's first
    doorbell.
  - No device answer in that window was an error. Open.
- Host:
  - No NVRM line, no Xid.
  - At the exit: 0 channels, 0 engine objects, 0 TSGs; reservation and guest RAM freed.
  - `nvidia-smi encodersessions` listed no session: its only sample in the 3 s encode found
    none.
