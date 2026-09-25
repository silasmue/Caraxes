<!-- SPDX-License-Identifier: MIT -->
# probe-nvdec0-615: NVENC and NVDEC pass on the 615 guest

- Binary `vfio-user-nvgpu-v615-nvenc-dec1`, vm2-615, 1 GiB, NVIDIA 615.71.09 encode/decode libraries, host alone.
- NVENC: 300 frames of 1080p H.264 in 3.153 s, 95.1 fps. Latency: median 7.0 ms, p99 17.4 ms, max 30.2 ms. (probe-engines615, the same device without the NVDEC change: 127.9 fps, median 4.2 ms.)
- Bitstream: SPS 1920x1080, 3 IDR, no bad start code.
- NVDEC: 1 engine, 300 frames in 4.008 s, 74.9 fps. PSNR min 63.37 dB, avg 71.46 dB.
- RESULT PASS. No host NVRM line, no Xid.
