<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# dmem display reserve in Linux B's variant (2026-09-29)

Design, facts and test plan: [docs/research/dmem-cap-2026-09-29.md](../../../docs/research/dmem-cap-2026-09-29.md).
Guest files: [crates/vfio-user-nvgpu/guest/dmem-cap](../../../crates/vfio-user-nvgpu/guest/dmem-cap).

## Pieces

- **VM.** `desktop-b-dmem`: a copy of Linux B's disks (06:46; `alpha-2026-09-27/desktop-b-dmem/copy-verify.log`: identical).
  - The wrapper [desktop-b-dmem.sh](desktop-b-dmem.sh) has Linux B's settings, tap, address and slot, so it never runs beside Linux B.
  - It needs 8 GiB of host RAM, beside the 8 GiB Windows VM.
- **Host build check.**
  - `nvidia-kernel-source-open 615.71.09-2ubuntu1` builds against Ubuntu's 7.0.0-34 headers with `NV_DMEM_CGROUP_PRESENT`.
  - Files: `/mnt/vmstore/caraxes/dmem-2026-09-29/{dl,build,build.log}`; `dl/SHA256SUMS` holds the checked sums.
- **Scripts.**
  - [prepare-hwe.sh](prepare-hwe.sh) with [guest-hwe-kernel.sh](guest-hwe-kernel.sh): the HWE kernel, the boot files and the units;
  - [guest-dmem-check.sh](guest-dmem-check.sh): the checks before the game;
  - [sottr-bench-dmem.sh](sottr-bench-dmem.sh): the benchmark with the cap on or off.

## Commands (the operator runs them; host NAT as for Linux B, `desktop-b/sottr-nat`)

```bash
B=/mnt/vmstore/caraxes/wt-dmem/caraxes/runs/dmem-2026-09-29
BIN=<device binary, e.g. the one of the last Linux B SotTR run>

# A. Kernel and units (stock 6.8 boot; about 15 min, mostly apt and DKMS)
$B/desktop-b-dmem.sh up && $B/desktop-b-dmem.sh wait 200
$B/prepare-hwe.sh                      # log $B/prepare/prepare.txt; ends with the VM down

# B. First boot of 7.0.0-34 and the checks before the game
NIGHT_SERVER=$BIN NIGHT_FB_MIB=4096 NIGHT_SLOT=0 CARAXES_NO_TRACE=1 $B/desktop-b-dmem.sh up
$B/desktop-b-dmem.sh wait 200
$B/desktop-b-dmem.sh ssh 'sudo modprobe nvidia && sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm modeset=1 && echo modules-ok; bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1; sudo systemctl start gdm3; sleep 15; XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus systemctl --user start app-dev.lizardbyte.app.Sunshine.service; sleep 6'
$B/desktop-b-dmem.sh ssh 'bash -s' < $B/guest-dmem-check.sh > $B/check-1.txt 2>&1
$B/desktop-b-dmem.sh down

# C. SotTR 1080p: five runs with the cap, one control without (each run starts the VM)
for i in 1 2 3 4 5; do DMEM_CAP=on $B/sottr-bench-dmem.sh $B/cap-on-$i $BIN; $B/desktop-b-dmem.sh down; done
DMEM_CAP=off $B/sottr-bench-dmem.sh $B/cap-off-1 $BIN; $B/desktop-b-dmem.sh down
```

- Linux B (`desktop-b`) stays bootable with its own wrapper.
- `DMEM_KERNEL=stock desktop-b-dmem.sh up` boots the copy on 6.8 again. Both halves then log "no dmem controller" and cap nothing.

## Pass criteria (per run of C)

- `guest-monitor.txt`: `nvkms_fail=0`, `xid=0` at the end.
- `benchmark-result.txt` written.
- `timeline.txt`: `host-rmse` non-zero until the result.
- `game-start.txt`: the game's cgroup in app.slice, with no WARNING.
- `guest-dmem.txt`: one "read back ok" line for cap on.
- Control: the refusal reproduces.

## Results

- **A, B (operator, 2026-09-29).**
  - `prepare/prepare.txt`: 7.0.0-34 installed, DKMS 615.71.09 built, boot files copied (sha256 equal), units installed.
  - `check-1.txt` (1 GiB profile): both halves ran; NVML inside app.slice reports app.slice's `dmem.max` as total.
- **C, cap-on-1** (4 GiB): NVKMS refused at 648 s.
  - Free VRAM was 2 MiB, app.slice 3461 of 3555 MiB; usage outside app.slice had grown by about 175 MiB against R = 84 MiB.
  - Steam hung at its shader-cache prompt, so the game was started directly (`systemd-run --user --scope --slice=app.slice`, `SteamAppId=750920`).
  - Only the text evidence is committed (`.log` files are git-ignored); the shots stay in the run directory on the NAS.
- **Parked:** the next step is a 6 GiB run without a cap (leak or reserve). Analysis: [dmem-cap-2026-09-29.md](../../../docs/research/dmem-cap-2026-09-29.md) "Status".

## Display reserve (RM patch, 2026-09-29)

Design and evidence: [docs/research/display-reserve-2026-09-29.md](../../../docs/research/display-reserve-2026-09-29.md). The patch is `patches/nvidia/0001`. The build is loaded only by the bench with `SOTTR_DISPLAY_RESERVE`; the stock modules stay the default.

```bash
B=/mnt/vmstore/caraxes/wt-dmem/caraxes/runs/dmem-2026-09-29
BIN=<device binary>
export DMEM_KERNEL=stock                       # 6.8.0-136; the build is per kernel

# 1. Build the patched nvidia.ko in the guest (once per kernel; about 5 min)
NIGHT_SERVER=$BIN NIGHT_FB_MIB=4096 NIGHT_SLOT=0 CARAXES_NO_TRACE=1 $B/desktop-b-dmem-6g.sh up
$B/desktop-b-dmem-6g.sh wait 200
$B/display-reserve-prepare.sh $B/drsv-build
$B/desktop-b-dmem-6g.sh down

# 2. SotTR 1080p with the reserve, then the control (each run starts and stops the VM)
SOTTR_FB_MIB=4096 DMEM_CAP=off SOTTR_DISPLAY_RESERVE=RmDisplayReserve=1 $B/sottr-bench-6g.sh $B/drsv-on-1 $BIN; $B/desktop-b-dmem-6g.sh down
SOTTR_FB_MIB=4096 DMEM_CAP=off SOTTR_DISPLAY_RESERVE=RmDisplayReserve=0 $B/sottr-bench-6g.sh $B/drsv-off-1 $BIN; $B/desktop-b-dmem-6g.sh down
```

Pass for the reserve run:
- `guest-monitor.txt` ends with `nvkms_fail=0 xid=0`;
- `benchmark-result.txt` is written;
- `timeline.txt` shows `host-rmse` non-zero until the result;
- `guest-prep.txt` shows `RegistryDwords` with `RmDisplayReserve=1`.

The control must reproduce the NVKMS refusal.
