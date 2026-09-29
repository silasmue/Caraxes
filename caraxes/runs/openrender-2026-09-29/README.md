<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Open-driver rendering: nouveau (GSP-RM 570.144) and NVK on the device, 2026-09-29

The runs behind [docs/research/open-render-2026-09-29.md](../../../docs/research/open-render-2026-09-29.md).
vm10 is an overlay on the 615 image (Mesa 25.2.8 with NVK) booting the Linux 7.2.7 kernel of
`build-kernel.sh` directly; the device detects guest ABI 570 from nouveau's GSP boot.

| File | What |
|---|---|
| `build-kernel.sh` | the nova kernel plus `DRM_NOUVEAU=m`, in `/mnt/vmstore/caraxes/nova-kernel/{build,modules}-openrender`; `kernel.config` is its config |
| `prepare-vm10.sh` | vm10's overlay: modules, the 570.144 firmware, blacklists, `vk-offscreen`, the check service (guestfish, no boot) |
| `openrender-check.sh` | the guest's check service: R1 probe, R2 kernel channels, R3 NVK device, R4 clear+copy, R5 triangle, the image as gzip+base64 on the serial console, unbind, power off |
| `openrender-boot.sh` | one boot of vm10 with host execution; keeps `setup.txt`, `guest.txt`, `device.txt`, `vk-offscreen.png` |
| `vk-offscreen/` | the offscreen Vulkan check (`build.sh`: glslc, gcc, needs at most GLIBC_2.39) |
| `extract-ppm.py` | the image from `serial.log` as PNG; `--png` for a PNG printed as is (weston's screenshot) |
| `fetch-weston-debs.sh`, `apt-closure.py` | weston 13 and its dependency closure from Ubuntu noble as a flat local apt repository in `/mnt/vmstore/caraxes/openrender-debs` (InRelease signatures checked against the pinned archive key, every index and .deb against its SHA256); `prepare-vm10.sh` copies it into the image, the check installs weston from it once (R8) |
| `vm10-boot1` .. `vm10-boot5` | the ladder runs: see the note's "The vm10 runs" |
| `nova-regress/` | the nova-core regression of this branch's device (PASS, `RESULT.md`) |

## Inputs (2026-09-29)

- Device: `/mnt/vmstore/caraxes/vfio-user-nvgpu-openrender-86d0e71f` (release build of
  86d0e71f), sha256 `5420bd2f9fc69edbc7deccb5c3b35ae38358382e440256f056e518ded30fd8ba`.
- Kernel: `/mnt/vmstore/caraxes/nova-kernel/build-openrender/arch/x86/boot/bzImage`, sha256
  `98908e0833b2d10ee6018529eaa3217e82794c73a6e31d0acac01fc1165febf8`.
- VBIOS: `/mnt/vmstore/caraxes/night-2026-09-28/synth-vbios-nova.bin` (FWSEC application id
  0x85, which nouveau requires too).
- vk-offscreen: sha256 `2db0a8331aad8d8546711e5f713c1e175d82918fb625b15a078577a14b2e6cd1`.

## Commands (run by the operator; nothing here was started by an agent)

```sh
cd /mnt/vmstore/caraxes/wt-openrender
caraxes/runs/openrender-2026-09-29/vk-offscreen/build.sh        # already built; rebuild is harmless
caraxes/runs/openrender-2026-09-29/prepare-vm10.sh              # guestfish, offline; once per image change
# R1-R5 in one boot: pick a free reservation slot (desktop-b uses 4); 1 GiB profile, 4 GiB RAM.
caraxes/runs/openrender-2026-09-29/openrender-boot.sh caraxes/runs/openrender-2026-09-29/vm10-boot1 \
    CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-openrender-86d0e71f CARAXES_RESERVE_SLOT=5
```

- The boot ends by itself (the check powers the guest off; watchdog 7 min). `down` runs at the end.
- What to look at: the last lines the script prints (rungs, `step A`/`step B`, rc), then
  `vm10-boot1/guest.txt` and `vm10-boot1/device.txt`; `grep 'guest ABI' vm10-boot1/gpu.log`
  must say 570.

## Regression (nova-core, pure emulation, no slot)

```sh
cd /mnt/vmstore/caraxes/wt-openrender
caraxes/runs/nova-2026-09-25/nova-boot.sh caraxes/runs/openrender-2026-09-29/nova-regress \
    CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-openrender-f7e9f4c7 \
    CARAXES_KERNEL=/mnt/vmstore/caraxes/nova-kernel/build/arch/x86/boot/bzImage \
    CARAXES_VBIOS=/mnt/vmstore/caraxes/night-2026-09-28/synth-vbios-nova.bin CARAXES_GUEST_ABI=
```

## Expected at this commit

- R1-R5, R7 (vm10-boot5): nouveau binds with GSP-RM 570.144, its kernel channels come up,
  `renderD128` appears, NVK lists the device, vk-offscreen passes, GL runs as zink on NVK.
- R8 (from vm10-boot6): weston headless with its GL renderer, vkcube-wayland FPS, a screenshot
  (`weston-vkcube.png`), glmark2-wayland and vkmark (wayland) scores.
- nouveau makes no TSG (every channel's parent is the device; the RM wraps such a channel in
  a TSG of its own, kernel_channel.c:365-410 of 615). Where each channel runs:
  - NVK's GR channels carry `TURING_COMPUTE_A`, so the GR shadow path takes them
    (`mirror.rs` `is_gr_shadow` does not ask for a guest TSG) and builds a host TSG of their
    own on GR0, keyed by the channel's handle (`tsg_key_of`): R4/R5 run on the host.
  - The kernel's GR channel (fences) and its privileged TTM copy channel stay on the
    software executor (`eligible`: bare GR without the compute class, privileged CE).
- If R4/R5 fail, `device.txt` names the channel's path (`shadow:` lines or
  `mirror: chid N in software: <why>`).
