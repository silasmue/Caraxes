<!-- SPDX-License-Identifier: MIT -->
# rmlog-610-dec: why NVDEC failed (CUDA error 2), and the fix

- `probe-dec610-2g` (2 GiB profile, slot 1) failed the same way as on 1 GiB: NVENC
  123.8 fps, `cuvidCreateDecoder` CUDA 2. So VRAM is not the cause.
- `rmfull.txt`: `caraxes/tools/rmioctl-log.c` in the 610 guest (1 GiB).
  - `NVC4B0` under the NVDEC0 channel: status 0.
  - `NVC4B0` under the NVDEC1 channel: `NV_ERR_INVALID_CLASS` (0x22), answered by the guest
    kernel, so the device never saw it.
- The device told the guest that NVDEC1 exists: `engineCaps` of chips/tu106/chip.toml
  listed NVDEC0/1. So the guest's NVDEC caps showed 2 engines, and cuvid placed its decoder
  on NVDEC1.
- The host (615.71.09 on this RTX 2070) lists GR0, CE0-4, NVDEC0, NVENC0 and SW in
  `GET_ENGINES_V2` (`../native-rmlog/host-rm-engines.log`): no NVDEC1.
- Fix: NVDEC1 dropped from the TU106 `engine_caps`.
  - `probe-nvdec0-610`: 1 NVDEC engine, 300 frames decoded at 118.5 fps, PSNR min 63.37 dB,
    RESULT PASS.
  - `probe-nvdec0-615`: the same, 74.9 fps, PSNR min 63.37 dB, PASS.
