<!-- SPDX-License-Identifier: MIT -->
# Display phases C and D: commands, word for word

Run 2026-09-24 00:00-02:00 on vm0 (tap caraxes0, guest 192.168.100.20) from the worktree root of
branch `worktree-agent-adbfba499c7110889` (reset to main `5de9b40` first). Host GPU access: only
the unprivileged host RM client the device opens with `CARAXES_HOST_RM=1` (nvml forwarding). No
host execution (`--host-exec` not used), nothing of the display touches the host GPU. Each
experiment directory holds `command.txt` (the guest command), `guest.txt` (its output) and
`device-trace.txt` (the events of the device trace for that window); the full trace windows (every
register and BAR1 access) are kept in `/mnt/vmstore/caraxes/vm0/display-cd/<label>/`.

## Build and unit checks

```sh
cargo build --release -p vfio-user-nvgpu
cargo test -q -p vfio-user-nvgpu
cargo fmt --all
cargo clippy -q --locked -p vfio-user-nvgpu --all-targets -- -D warnings -A clippy::unnecessary_cast -A clippy::field_reassign_with_default
sh crates/vfio-user-nvgpu/guest/build-host.sh          # nvkms-vblank, static, into target/guest/
```

The worktree lacks the git-ignored inputs of the main checkout: `caraxes/reference/rtx2070/rom.bin`
and `synth-vbios.bin` were copied from the main checkout, `vendor/open-gpu-kernel-modules` is a
symlink to the main checkout's copy.

## VM

```sh
VM=vm0 tools/caraxes-vm.sh ssh 'sync'; VM=vm0 tools/caraxes-vm.sh down
VM=vm0 CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=4096 CARAXES_HOST_RM=1 CARAXES_DISPLAY=1 tools/caraxes-vm.sh up
VM=vm0 tools/caraxes-vm.sh wait
VM=vm0 tools/caraxes-vm.sh probe display-c<N>        # nvidia.ko
```

Guest material: the probes and scripts of `crates/vfio-user-nvgpu/guest/` are copied into
`~/cx/` (scp), then `bash ./cx/build-wl.sh` builds `kmsflip`, `kmsgrab` and `wlframes` in the
guest. Offline packages (Ubuntu 24.04 noble, noble-updates, noble-security; resolved against
the guest's dpkg list, downloaded with plain curl from archive.ubuntu.com, SHA-256 checked):
`weston sway libdrm-tests edid-decode wayland-utils grim wlr-randr` and their 52 missing
dependencies (installed with `sudo dpkg -i ~/caraxes-debs/*.deb`), then `libwayland-dev
libwayland-bin wayland-protocols libffi-dev` (`~/caraxes-debs2`).

## Experiments (bash caraxes/runs/display-cd-2026-09-24/exp.sh <label> <timeout> '<guest command>')

| Label | Guest command |
|---|---|
| c1-kmsflip | `sudo dmesg -C; sudo timeout 90 ./kmsflip --flips 60; ...` (after `modprobe video; modprobe nvidia-modeset; modprobe nvidia-drm modeset=1 fbdev=0`) |
| c2-load | `sudo modprobe video; ...; sudo timeout 60 ./kmsflip --flips 120; ...` |
| c3-weston, c4-weston | `sudo bash ./weston-pace.sh 640x480 10` (first versions, weston-presentation-shm) |
| c5-build | `bash ./cx/build-wl.sh` |
| c6-weston-640 | `sudo SETTLE=8 bash ./cx/weston-pace.sh 640x480 15` |
| c7-weston-1080 | `sudo SETTLE=70 bash ./cx/weston-pace.sh 1920x1080@60 20` (no frame within the window) |
| c8-weston-1080 | `sudo SETTLE=150 bash ./cx/weston-pace.sh 1920x1080@60 60` |
| c9-modeswitch | `for m in 0 3 2 0; do sudo timeout 40 ./cx/kmsflip --mode-index $m --flips 120 ...; done` |
| c10-vblank | `sudo bash ./cx/vblank-check.sh 120 0` (failed: no /dev/nvidia-modeset node) |
| c11-vblank | `sudo bash ./cx/display-up.sh; sudo bash ./cx/vblank-check.sh 120 0; ...` |
| c12-weston-640 | two Weston runs, then the kernel log |
| c13-pixel | `bash ./cx/build-wl.sh; sudo ./cx/kmsgrab pixel 640x480; sudo ./cx/kmsgrab info` |
| c14-sunshine | `sudo bash ./cx/sunshine-kms.sh 640x480 45` |
| d1-gdm-setup | `sudo bash ./cx/gdm-wayland-setup.sh` (first version) |
| d2-gdm-start | `sudo pkill -x weston; sudo dmesg -C; sudo bash ./cx/gnome-check.sh start 240` |
| d3-gnome-x11-status | `sudo bash ./cx/gnome-check.sh status; sudo ./cx/kmsgrab sample ...; ...; session set to ubuntu-wayland` |
| d4-gnome-wayland, d5-gnome-wayland | `sudo bash ./cx/gnome-check.sh start 300; ...` (X11 again; d5 after restarting accounts-daemon: greeter on X11) |
| d6-gdm-debug | gdm `[debug] Enable=true`, start, the journal: gnome-shell on Wayland segfaults in libgallium |
| d7-gnome-kms-swrast | `sudo bash ./cx/gdm-wayland-setup.sh; sudo bash ./cx/gnome-check.sh start 240; sudo ./cx/kmsgrab sample 960 540 100 1070 1900 20` |
| d8-gnome-frames | `sudo modetest -M nvidia-drm -p ...; sudo -u leandro env XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-0 WLFRAMES_WAIT=200 timeout 300 ./wlframes 20 64 8` |
| d9-hotplug-edid | `bash caraxes/runs/display-cd-2026-09-24/hotplug.sh d9-hotplug-edid edid-alt` (host writes the command into `/mnt/vmstore/caraxes/vm0/display-ctl` 10 s into `cx/hotplug-watch.sh 40`) |
| d10-weston-1080p50 | `sudo SETTLE=30 bash ./cx/weston-pace.sh 1920x1080@50 30; sudo SETTLE=20 bash ./cx/weston-pace.sh 1280x720@60 30` |
| d11-unplug, d12-plug-main | `hotplug.sh d11-unplug unplug`, `hotplug.sh d12-plug-main edid-main` |
| d13-cursor | `sudo timeout -s INT 60 modetest -M nvidia-drm -s 127@61:640x480 -C` (modetest dumped core) |
| d14-cursor | `sudo timeout 90 ./cx/kmsflip --mode-index 5 --flips 10 --cursor` |

## Gates

```sh
bash caraxes/runs/display-cd-2026-09-24/gate.sh vdisplay 1   # = CARAXES_LEANDRO_TEST=/home/silas/git/Leandro-Test VM=vm0 tools/caraxes-gate.sh vdisplay
bash caraxes/runs/display-cd-2026-09-24/gate.sh display 1
bash caraxes/runs/display-cd-2026-09-24/gate.sh display 2
# after the rebase onto main 223075d, a fresh device (the first boot fell into emergency mode:
# /boot by-label not found while the host was loaded; the second boot was clean):
bash caraxes/runs/display-cd-2026-09-24/gate.sh vdisplay final
bash caraxes/runs/display-cd-2026-09-24/gate.sh display final
bash caraxes/runs/display-cd-2026-09-24/gate.sh display final2
```

The gate run directories are `caraxes/runs/gate-vdisplay-2026-09-24{,-2,-3}` (the first two are
skips from precondition bugs of the new gate code: `/dev/nvidia0` required, Leandro-Test path)
and `caraxes/runs/gate-display-2026-09-24{,-2}`; on the final build
`gate-vdisplay-2026-09-24-4`, `gate-display-2026-09-24-3` (fail, 59.50 Hz) and `-4` (pass).
