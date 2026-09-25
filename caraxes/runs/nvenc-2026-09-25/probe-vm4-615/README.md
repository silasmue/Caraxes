<!-- SPDX-License-Identifier: MIT -->
# probe-vm4-615: NVENC refused on the 615 guest (vm4-615), before any video channel

- Setup:
  - Binary `vfio-user-nvgpu-v615-nvenc-1c4c848`, the same bytes as 050ddf5.
  - vm4-615 brought up by `up-615.sh` with `SOTTR_SUN_ENCODER=nvenc`: the 3 GiB profile,
    NVIDIA's 615.71.09 packages. Host driver 615.71.09.
  - The probe ran inside the running VM (`probe.txt`).
- Result:
  - `nvEncOpenEncodeSessionEx` returned NVENCSTATUS 2 (`NV_ENC_ERR_UNSUPPORTED_DEVICE`).
  - Sunshine's `h264_nvenc` failed the same way (`OpenEncodeSessionEx failed: unsupported
    device (2)`). It fell back to vulkan (failed), then to libx264: session
    `sottr-2026-09-24/session-615-20260925-153236/sunshine-start.txt`.
- Where it differs from the 610 run that encodes (`probe-layout610`, 1 GiB profile, 610
  userspace). The encoder client's RPCs are the same up to `NV90F1` `0x90f10106`. Then:
  - 610 sends `NVDEC_GET_CAPS_V2` (0x801c02), `NVENC_GET_CAPS_V2` (0x801b02),
    `GPU_GET_INFO_V2`, the session open (`0x20808165`, `0x20808163`), then the channel and
    NVC4B7.
  - 615 sends only `GPU_GET_INFO_V2` and stops: no caps query, no session open, no video
    channel. Every answer before that was NV_OK and matched 610 (`GPU_GET_INFO_V2`: the
    same host index 0x11).
- So the 615 library rejects the device from something it reads before the caps. Two
  differences remain untested:
  - the 615 userspace;
  - the 3 GiB profile ("RTX 2070-3G" against "-1G").
- The mirror was not reached: no video channel, no video object, no host NVRM line, no Xid.
