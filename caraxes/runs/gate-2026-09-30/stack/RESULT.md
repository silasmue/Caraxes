<!-- SPDX-License-Identifier: MIT -->
# Four-guest gate of the stack flags-rb-2026-09-30, 2026-09-30

The binary is vfio-user-nvgpu-stack-ffec4d49 (sha256 32b1c1f37b3be2e8…), built from ffec4d49. The
commits after it change scripts only. One VM ran at a time under vmlock.sh; `legs.txt` has the
timeline.

- **Linux B 615 kick-smoke: PASS** (`kick-smoke/`).
  - vkcube on Wayland and X11 ran.
  - NVENC 196.6 fps, NVDEC PSNR 71.46 dB, the NVENC holder check passed.
  - Moonlight streamed over HEVC and the picture changed. Guest Xid 0.
  - glmark2 [build] was 432 fps in this run. It did not reproduce on a quiet card:
    - `ab-kick-*`: dv2 b6d01293 4756, stack 4501;
    - `clean-kick`: the cleaned launchers 4558, with no "no longer read" warning.
- **nova 5-probe: PASS** (`nova/`): probe, three unbind/bind cycles, reload, 0 refused. The first
  try had no rom.bin in the worktree (`nova-norom.txt`).
- **nouveau + NVK vm10 R1-R8: the checks pass, but vkmark fails** (`vm10/`, slot 4; the host RM
  refused slot 5, see `vm10-slot5-refused.txt`).
  - vk-offscreen and weston-vkcube hashes OK; vkcube-wayland 39 fps; glmark2-wayland 4440.
  - vkmark cube under weston ends in `vk::DeviceLostError`: nouveau logs "job timeout, channel 7
    killed". The device logs 16 "bar1 pool: … remap refused (… the barrier failed: chid busy)".
  - The lf16 gate (winlf-1bdf9089, 2026-09-29) passed with vkmark cube 4790 and 0 device errors.
  - `bisect/`: final1-424a43e8 and diag1-58e5c6ac fail the same way.
  - So the regression lies in lf16..final1 (1bdf9089..424a43e8, 72 commits under crates/).
  - The next existing binary to test is train0-991e3d86 (= lf24).
- **Windows 11 617.14: PASS** (`windows/`, `chain.txt`).
  - D3D11 stages 12/12. Heaven 140.4 fps, Superposition 1080p Medium 11622 (86.9 fps).
  - One guest reboot, served as client 2.
  - 0 host Xid, 0 FLR, no display-driver event.
  - The one FAIL line in `checks.txt` (no d3d-probe-status.json) is the probe the session does
    not run.
- **Linux SotTR (information only)** (`sottr/`): rc 0, 34.7 fps average (2026-09-29: 12.1-15.4), Xid
  0.
  - The frame check had no frame to judge: the shots came after the benchmark (the host shot is
    black, the guest xwd is empty).
  - The "neon" question stays open.
