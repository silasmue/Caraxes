<!-- SPDX-License-Identifier: MIT -->
# vm4-615, first start (2026-09-25 11:24)

- Guest driver: the distribution packages (`prepare.txt`): nvidia-driver-open, libnvidia-gl
  (amd64, i386), -encode, -decode, -fbc1 at 615.71.09-2ubuntu1, linux-modules-extra-6.8.0-136.
- Driver: `probe.txt` RmInitAdapter succeeded; `display-up.txt` DP-1 1920x1080;
  `checks-driver.txt` nvidia-smi rc 0, vulkaninfo NVIDIA 615.71.09.
- GNOME: `gnome.txt` Wayland, gbm renderer on card0, 0 page-flip failures.
- Steam runtime: `sniper-system-info-extract.txt`, the NVIDIA Vulkan ICD and EGL vendor in the
  sniper container, no issues.
- vkcube: not run (the script passed `--wsi`, which vulkan-tools 1.3.275 lacks; `checks-session.txt`).
- Sunshine: `sunshine-encoder.txt`, nvenc refused (`OpenEncodeSessionEx`: unsupported device),
  vulkan encode refused, libx264 chosen after about 70 s.
- Stream: Moonlight re-paired through the API and opened at 11:31 (operator testing).
- Host Xid: 11:30:18 NVENC0 FAULT_PTE, pid 142502 (vm2's device, not this session);
  11:32:26 CE0 FAULT_PDE read, pid 126268 (this session's device), during the stream.
  The device log: one `HOST RC EVENT on a mirrored engine ... exceptType 0x1f`, 25 `bar1 pool:
  remap refused (the guest maps it twice)`.
- Benchmark numbers: the operator's, to be added here.
