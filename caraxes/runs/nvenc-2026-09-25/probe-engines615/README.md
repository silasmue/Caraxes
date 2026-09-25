<!-- SPDX-License-Identifier: MIT -->
# probe-engines615: the 615 guest encodes after the ENGINE_MASK fix

- Setup:
  - Binary `vfio-user-nvgpu-v615-nvenc-db2ff5f`: `ENGINE_MASK` now encodes each NV2080
    engine as the GMC id of the same RM type, so the 615 guest keeps NVC4B0 and NVC4B7.
  - vm2-615, 1 GiB profile, NVIDIA's 615.71.09 encode/decode libraries.
- NVENC:
  - The session opened in 18.5 ms and the encoder initialised in 210 ms.
  - 300 frames at 1920x1080 H.264 P1 CBR in 2.345 s: 127.9 fps. Latency: median 4.2 ms,
    p99 14.0 ms, max 18.4 ms. 8.4 Mbit/s at 60 fps.
  - Bitstream: SPS 1920x1080, profile 100, level 4.2, 3 IDR, no bad start code.
- NVDEC: `cuvidCreateDecoder` returned CUDA error 2, so 0 frames. The caps query now
  reports 2 NVDEC engines. The failure is the same as on 610 (`probe-layout610`).
- Host: no NVRM line, no Xid.
