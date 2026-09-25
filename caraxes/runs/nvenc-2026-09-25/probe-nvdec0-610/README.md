<!-- SPDX-License-Identifier: MIT -->
# probe-nvdec0-610: NVENC and NVDEC pass on the 610 guest

- Binary `vfio-user-nvgpu-main-nvenc-dec1` (the ENGINE_MASK fix and TU106 without NVDEC1), vm2, 1 GiB, host alone.
- NVENC: 300 frames of 1080p H.264 in 1.913 s, 156.8 fps. Latency: median 3.3 ms, p99 7.3 ms, max 8.1 ms. 8.4 Mbit/s at 60 fps.
- Bitstream: SPS 1920x1080, 3 IDR, no bad start code.
- NVDEC: 1 engine, 300 frames in 2.532 s, 118.5 fps. PSNR min 63.37 dB, avg 71.46 dB, the same as the native run.
- RESULT PASS. No host NVRM line, no Xid.
