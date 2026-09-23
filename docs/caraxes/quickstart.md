<!-- SPDX-License-Identifier: MIT -->
# The stock driver on a synthetic RTX 2070

- Direct commands for the device server and Cloud Hypervisor. `tools/caraxes-vm.sh`
  and `tools/caraxes-gate.sh` wrap the run and the check; nothing else is required.
- Linux x86-64 host with the NVIDIA open kernel modules 610.57.04 running against the
  physical card (GSP firmware active), matching `vendor/open-gpu-kernel-modules`. The
  host driver stays untouched; see Safety below.
- This is Leandro v2 (Caraxes): the guest runs the unmodified stock driver, with no
  guest module. Background: [README.md](README.md), [kickoff.md](kickoff.md).
- Current state: milestone 1 is reached and `nvidia-smi` telemetry is complete. What
  is still missing is listed under "Not yet working". Full ladder: [experiments.md](experiments.md).

## Prerequisites

| What | Where |
|---|---|
| Cloud Hypervisor v53 | vendored at `vendor/cloud-hypervisor`; the launcher expects the built binary at `$LEA_CH` (default `/mnt/vmstore/leandro/cloud-hypervisor/bin/cloud-hypervisor`) |
| Guest image and kernel | Ubuntu 24.04 guest at `$LEA_BASE_IMAGE`; kernel and initrd at `$LEA_VM_DIR/guest-image/vmlinuz` and `initrd` (kernel 6.8.0-136-generic); `$LEA_VM_DIR` defaults to `/mnt/vmstore/leandro/vm` |
| Config space and ROM of the physical card | `caraxes/reference/rtx2070/config-space.txt` (tracked, an `lspci -xxxx` dump) and `rom.bin` (git-ignored sysfs capture, 2 PCI images, 130048 bytes) |
| Synthetic VBIOS | built by `caraxes/tools/synth-vbios.py` from `rom.bin`; the third, missing image the driver needs is constructed, not captured (decision D8) |
| Tap device | `caraxes0`, owned by the user, `192.168.100.1/24` |
| VM state directory | NAS at `/mnt/vmstore/caraxes/<vm>`; the launcher falls back to `target/caraxes/<vm>` when that path is absent |

Create the tap once per boot of the host:

```sh
sudo ip tuntap add dev caraxes0 mode tap user "$USER"
sudo ip addr add 192.168.100.1/24 dev caraxes0
sudo ip link set caraxes0 up
```

Build the synthetic VBIOS once:

```sh
python3 caraxes/tools/synth-vbios.py caraxes/reference/rtx2070/rom.bin caraxes/reference/rtx2070/synth-vbios.bin
```

## 1. Build

```sh
cargo build --release -p vfio-user-nvgpu
```

This produces `target/release/vfio-user-nvgpu`, the path `tools/caraxes-vm.sh` expects.

## 2. Prepare the guest (once per overlay)

Create the overlay disk on top of the baked guest image:

```sh
qemu-img create -f qcow2 -b "$LEA_BASE_IMAGE" -F qcow2 target/caraxes/vm0/rootfs.qcow2
```

`up` also needs a cloud-init seed image, `seed.img`, next to the overlay; build it
from `seed/user-data` (hostname `caraxes-vm0`, address `192.168.100.20/24`).

In the guest:

1. Copy `vendor/open-gpu-kernel-modules` (the same 610.57.04 tag as the host) and
   build the kernel module against the guest's own headers:

   ```sh
   make modules -j8 SYSSRC=/lib/modules/6.8.0-136-generic/build
   ```

   Install only `nvidia.ko`; do not install `nvidia-modeset` or `nvidia-drm` yet (see
   "Not yet working").
2. Copy the firmware from the host and place it where the module expects it:

   ```sh
   cp gsp_tu10x.bin /lib/firmware/nvidia/610.57.04/gsp_tu10x.bin
   ```

   The source on the host is `/usr/lib/firmware/nvidia/610.57.04/gsp_tu10x.bin`.
3. Set the module options in `/etc/modprobe.d/caraxes.conf`:

   ```
   options nvidia NVreg_GpuInitOnProbe=1 NVreg_ResmanDebugLevel=0 NVreg_RegistryDwords="RmDisableGlobalCeUtils=1;RMDisableScrubOnFree=1;RmRcWatchdog=0"
   ```

   The three registry keys turn off the watchdog channel, the copy-engine self-test
   and free-time scrubbing, none of which the device can serve yet.

## 3. Run

```sh
CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=4096 CARAXES_HOST_RM=1 tools/caraxes-vm.sh up
tools/caraxes-vm.sh wait
tools/caraxes-vm.sh probe milestone1
tools/caraxes-vm.sh down
```

- `CARAXES_BOOT_MODEL=1` attaches the falcon and message-queue model; without it the
  device stops at the GFW wait (experiment E1).
- `CARAXES_FB_MIB` sets the framebuffer size reported to the guest.
- `CARAXES_HOST_RM=1` lets the device open the host RM and forward the phase-5 RPCs;
  without it the guest stops at the first unanswered RPC.
- `wait` blocks until SSH answers; `probe` reloads `nvidia.ko` and captures the log of
  that reload; `down` stops the VM and the device.
- Logs and traces live under `$W`, which is `$CARAXES_STORE/<vm>` (`vm0` by default):
  `gpu.log` (device process), `gpu-trace.log` (every BAR and config access), `ch.log`
  (Cloud Hypervisor), `serial.log` (guest console). A `probe` also writes
  `probe-<label>/guest-dmesg.txt` and `probe-<label>/device-trace.txt`, scoped to that
  reload only.

## 4. Verify

```sh
tools/caraxes-gate.sh nvml
```

Passes (exit 0) when the identity, the framebuffer accounting and the forwarded
telemetry all check out against the host's own read-only `nvidia-smi -q`. In the
guest, `nvidia-smi` prints:

```
|   0  Leandro RTX 2070-4G            Off |   00000000:00:05.0 Off |                  N/A |
| 30%   50C    P8             29W /  175W |       0MiB /   4096MiB |     19%      Default |
```

The name, UUID and framebuffer size are device-owned (decision D6, never rewritten
in a reply); temperature, power, clocks and utilization are forwarded from the
physical card ([nvml.md](nvml.md)).

## Not yet working

- **No CUDA.** `nvidia-uvm` and the first CUDA context are the next experiment (E5);
  see [cuda-path.md](cuda-path.md) for the design and [experiments.md](experiments.md)
  for status.
- **No display.** `nvidia-modeset` and `nvidia-drm` stay unloaded; scope is
  `nvidia.ko` only until CUDA is working.
- **Golden channel skipped.** The KernelGraphics floorsweeping reply is deliberately
  all zeros (`gpcMask` 0), which skips the golden-image channel and the Bug 4208224
  workaround; GR stays unusable for clients.

Full status and the experiment-by-experiment record: [experiments.md](experiments.md).

## Safety rules (kickoff.md §2)

- The host GPU is read-only and hands-off: no unbinding, no kernel modules, no
  `/dev/mem` tricks, no writes to host NVIDIA registers, no host driver changes. Reach
  the card only as an unprivileged host RM client, the same way `vhost-user-nvrm`
  does. Report every read of host GPU state.
- No guest driver patches. The guest runs the stock open driver, never the GRID
  driver.
- Stay inside this repository. VM assets under `/mnt/vmstore/leandro` are inputs;
  overlays, sockets and logs go under `target/caraxes/`.
- Leandro v1 keeps working: add code, do not rewrite `vhost-user-nvrm`, `nvrm-abi`,
  `nvrm-wire` or the guest module.
- Passing the real card to a VM (the reference captures R1/R3) is opt-in and needs a
  prepared, scheduled procedure; it is not part of this quickstart.

## Exact commands used on 2026-09-23

The PCI-visible ROM of the card (runs as root, reads only):

```sh
echo 1 | sudo tee /sys/bus/pci/devices/0000:2d:00.0/rom
sudo cat /sys/bus/pci/devices/0000:2d:00.0/rom > caraxes/reference/rtx2070/rom.bin
echo 0 | sudo tee /sys/bus/pci/devices/0000:2d:00.0/rom
```

The cloud-init seed next to the overlay (`user-data`, `meta-data`, `network-config` in `seed/`):

```sh
mkfs.fat -C -n cidata seed.img 1440
mcopy -i seed.img seed/user-data seed/meta-data seed/network-config ::
```

Installing the guest module after the in-guest build (from the tree's root in the guest):

```sh
sudo make modules_install -j8 SYSSRC=/lib/modules/$(uname -r)/build
sudo depmod -a
sudo modprobe nvidia
```
