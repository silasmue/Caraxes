<!-- SPDX-License-Identifier: MIT -->
# probe-dec610-2g: NVDEC on a 2 GiB profile, still CUDA error 2

- Binary `vfio-user-nvgpu-main-nvenc-1c4c848`, vm2, 2 GiB profile at slot 1 (`NVENC_FB_MIB=2048 NVENC_SLOT=1`).
- NVENC: 123.8 fps, median 5.4 ms. NVDEC: `cuvidCreateDecoder` CUDA 2, 0 frames.
- So VRAM is not the cause; see `../rmlog-610-dec/`.
