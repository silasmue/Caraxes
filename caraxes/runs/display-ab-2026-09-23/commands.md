<!-- SPDX-License-Identifier: MIT -->
# Display phases A and B: commands, word for word

Run 2026-09-23 on vm0 (tap caraxes0, guest 192.168.100.20) from the worktree root of
branch `worktree-agent-a6f9005032084a629`. Host GPU access: only the unprivileged host RM
client the device opens with `CARAXES_HOST_RM=1` (nvml forwarding) and the read-only
`nvidia-smi -q` of `tools/caraxes-gate.sh nvml`. Nothing of the display touches the host.

## Build and unit checks

```sh
cargo build --release -p vfio-user-nvgpu
cargo test -q -p vfio-user-nvgpu
cargo test -q -p dp-sink
cargo fmt --all
cargo clippy -q --locked --workspace --all-targets -- -D warnings -A clippy::unnecessary_cast -A clippy::field_reassign_with_default
cargo clippy -q --locked --workspace --all-targets --all-features -- -D warnings -A clippy::unnecessary_cast -A clippy::field_reassign_with_default
```

Struct layouts: `python3 abi/gen.py` (reads `abi/names.txt`, compiles a sizeof/offsetof
probe against the 610.57.04 SDK headers with gcc, writes the table that became
`crates/vfio-user-nvgpu/src/ver/v610/display-layouts.txt`).

## D0 baseline (display off, before phase A)

```sh
VM=vm0 tools/caraxes-vm.sh down
VM=vm0 CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=4096 CARAXES_HOST_RM=1 tools/caraxes-vm.sh up
VM=vm0 tools/caraxes-vm.sh wait
VM=vm0 tools/caraxes-vm.sh probe display-ab-baseline
VM=vm0 tools/caraxes-gate.sh nvml > caraxes/runs/display-ab-2026-09-23/gate-nvml-baseline.json 2> caraxes/runs/display-ab-2026-09-23/gate-nvml-baseline.stderr
VM=vm0 tools/caraxes-vm.sh uvm display-ab-baseline
```

## Guest: full four-module build (quickstart.md step 2)

```sh
VM=vm0 tools/caraxes-vm.sh ssh 'make -C /home/leandro/ogkm/open-gpu-kernel-modules clean > /home/leandro/clean-display-ab.log 2>&1; echo clean-exit=$?'
VM=vm0 tools/caraxes-vm.sh ssh 'nohup make -C /home/leandro/ogkm/open-gpu-kernel-modules modules -j8 SYSSRC=/lib/modules/6.8.0-136-generic/build > /home/leandro/build-display-ab.log 2>&1 < /dev/null & echo started'
VM=vm0 tools/caraxes-vm.sh ssh 'sudo make -C /home/leandro/ogkm/open-gpu-kernel-modules modules_install -j8 SYSSRC=/lib/modules/6.8.0-136-generic/build > /home/leandro/install-display-ab.log 2>&1; echo install-exit=$?; sudo depmod -a; modinfo -n nvidia nvidia-modeset nvidia-drm; modinfo -F version nvidia-drm'
VM=vm0 tools/caraxes-vm.sh ssh 'printf "%s\n" "# Caraxes display phases A/B (docs/caraxes/display-ab.md): never autoload, load by hand." "blacklist nvidia-drm" "blacklist nvidia-modeset" "options nvidia-drm modeset=1 fbdev=0" | sudo tee /etc/modprobe.d/caraxes-display.conf'
```

Lesson: `tools/caraxes-vm.sh down` kills Cloud Hypervisor without a guest shutdown. The
build products and the installed modules had not been written back yet: after the restart
every `.ko` of the build read as all zeros and the install and the modprobe.d file were
gone. Rebuilt after `make clean`, with a `sync` in the guest before any `down`.

## Phase A (display on)

```sh
VM=vm0 tools/caraxes-vm.sh down
VM=vm0 CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=4096 CARAXES_HOST_RM=1 CARAXES_DISPLAY=1 tools/caraxes-vm.sh up
VM=vm0 tools/caraxes-vm.sh wait
VM=vm0 tools/caraxes-vm.sh probe display-a1
python3 caraxes/runs/display-ab-2026-09-23/route.py /mnt/vmstore/caraxes/vm0/probe-display-a1/device-trace.txt
VM=vm0 tools/caraxes-gate.sh nvml > caraxes/runs/display-ab-2026-09-23/gate-nvml-phase-a.json 2> caraxes/runs/display-ab-2026-09-23/gate-nvml-phase-a.stderr
VM=vm0 tools/caraxes-vm.sh uvm display-a1
tools/check.sh > caraxes/runs/display-ab-2026-09-23/check-phase-a.txt 2>&1
```

## Phase B (nvidia-modeset and nvidia-drm)

Run b1 (gdm3 still enabled in the guest image; its Xorg took card0 and set a mode, so
this run is phase-C evidence, not the D3 run):

```sh
VM=vm0 tools/caraxes-vm.sh probe display-b0
VM=vm0 tools/caraxes-vm.sh ssh 'sudo dmesg -C; sudo modprobe video; sudo modprobe nvidia-modeset; echo modeset=$?; sudo modprobe nvidia-drm modeset=1 fbdev=0; echo drm=$?; sleep 5; sudo dmesg | grep -v "NVRM: VM:"; ls /sys/class/drm/; for c in /sys/class/drm/card*-*; do echo $c $(cat $c/status); cat $c/modes; done'
scp -q -i /mnt/vmstore/leandro/vm/id_leandro -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR crates/vfio-user-nvgpu/guest/drm-modes.c leandro@192.168.100.20:drm-modes.c
VM=vm0 tools/caraxes-vm.sh ssh 'cc -O2 -I/usr/include/drm -o drm-modes drm-modes.c && sudo ./drm-modes'
```

Guest change after b1 (recorded in docs/caraxes/experiments.md):

```sh
VM=vm0 tools/caraxes-vm.sh ssh 'sudo systemctl disable gdm3; sudo systemctl set-default multi-user.target; systemctl get-default; sync'
VM=vm0 tools/caraxes-vm.sh ssh 'sync; sudo systemctl stop gdm3; sync'
```

Run b2 used the same load commands (drm-modes ran inside the window). Run b3 is the D3
run (server rebuilt with the raster-method count in the kick note):

```sh
VM=vm0 tools/caraxes-vm.sh ssh 'sync'
VM=vm0 tools/caraxes-vm.sh down
VM=vm0 CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=4096 CARAXES_HOST_RM=1 CARAXES_DISPLAY=1 tools/caraxes-vm.sh up
VM=vm0 tools/caraxes-vm.sh wait
VM=vm0 tools/caraxes-vm.sh probe display-b3
VM=vm0 tools/caraxes-vm.sh ssh 'sudo dmesg -C; sudo modprobe video; sudo modprobe nvidia-modeset; sudo modprobe nvidia-drm modeset=1 fbdev=0; sleep 20; for f in status enabled dpms modes; do echo "== $f"; cat /sys/class/drm/card0-DP-1/$f; done; echo "== dmesg"; sudo dmesg | grep -v "NVRM: VM:"' > caraxes/runs/display-ab-2026-09-23/b3-guest-sysfs-dmesg.txt
sed -n 372083,398792p /mnt/vmstore/caraxes/vm0/gpu-trace.log > /mnt/vmstore/caraxes/vm0/b3-device-trace.txt
python3 caraxes/runs/display-ab-2026-09-23/route.py /mnt/vmstore/caraxes/vm0/b3-device-trace.txt > caraxes/runs/display-ab-2026-09-23/b3-route.txt
VM=vm0 tools/caraxes-vm.sh ssh 'sudo ./drm-modes; sleep 8; sudo dmesg | grep -v "NVRM: VM:" | tail -3' > caraxes/runs/display-ab-2026-09-23/b3-guest-drm-modes.txt
sed -n 398793,413259p /mnt/vmstore/caraxes/vm0/gpu-trace.log > /mnt/vmstore/caraxes/vm0/b3-drm-modes-trace.txt
python3 caraxes/runs/display-ab-2026-09-23/route.py /mnt/vmstore/caraxes/vm0/b3-drm-modes-trace.txt > caraxes/runs/display-ab-2026-09-23/b3-drm-master-close-route.txt
scp -q -i /mnt/vmstore/leandro/vm/id_leandro -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR leandro@192.168.100.20:/sys/class/drm/card0-DP-1/edid caraxes/runs/display-ab-2026-09-23/guest-card0-DP-1.edid.bin
edid-decode --check caraxes/runs/display-ab-2026-09-23/guest-card0-DP-1.edid.bin > caraxes/runs/display-ab-2026-09-23/guest-edid-decode.txt 2>&1
VM=vm0 tools/caraxes-gate.sh nvml > caraxes/runs/display-ab-2026-09-23/gate-nvml-phase-b.json 2> caraxes/runs/display-ab-2026-09-23/gate-nvml-phase-b.stderr
VM=vm0 tools/caraxes-vm.sh uvm display-b3
python3 caraxes/runs/display-ab-2026-09-23/transcript.py /mnt/vmstore/caraxes/vm0/b3-device-trace.txt 100
cp caraxes/runs/display-ab-2026-09-23/guest-card0-DP-1.edid.bin crates/dp-sink/tests/transcripts/caraxes-dp-monitor.edid.bin
cargo test -q -p dp-sink
```

The transcript output became `crates/dp-sink/tests/transcripts/nvidia-610-guest-capture.txt`
(with a header) and `b3-aux-transcript.txt`. The empty `*.stderr` files of the gate runs
were removed (nothing was written to stderr). The text evidence files got an SPDX line and
a one-line description at the top when they were collected.
