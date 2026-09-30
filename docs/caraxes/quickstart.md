<!-- SPDX-License-Identifier: MIT -->
# The stock driver on a synthetic RTX 2070

- Cloud Hypervisor is the preferred VMM for the current Linux path. The tested
  desktop uses Ubuntu 24.04, stock NVIDIA 615.71.09 packages, GNOME Wayland and
  Sunshine KMS capture with NVENC.
- Linux x86-64 host with NVIDIA open kernel modules and GSP firmware active. The
  current measured desktop uses host and guest driver 615.71.09. The host driver
  stays untouched; see Safety below.
- This is Leandro v2 (Caraxes). The current helper-free path uses the unmodified
  stock driver and no Caraxes guest kernel module. Background: [README.md](../history/caraxes-record-2026-09-28.md).

## Current 615 Wayland path

The maintained desktop harness provisions the stock 615 packages, starts GNOME
Wayland and Sunshine, records evidence, and preserves the legacy helper profile.
Use a current release build of `vfio-user-nvgpu`:

```sh
CARAXES_GUEST_DMA_GPA=1 \
SOTTR_SERVER=/path/to/current/vfio-user-nvgpu \
caraxes/runs/sottr-2026-09-24/up-615.sh

caraxes/runs/sottr-2026-09-24/stream-615.sh
caraxes/runs/sottr-2026-09-24/down-615.sh
```

- `CARAXES_GUEST_DMA_GPA=1` registers guest RAM and lets the stock guest use GPA
  DMA addresses. Provisioning removes only the old display-module dependency on
  `caraxes_dma_range`; it keeps the boot ordering and unrelated NVIDIA settings.
- Omit the flag to retain the older helper and probe workflow.
- The stage checks VRAM reservation overlap, tap ownership and image compatibility.
  Read its [operator page](../../caraxes/runs/sottr-2026-09-24/README.md) before use.
- The 2026-09-27 trials passed short single-guest and two-guest Wayland, NVENC,
  NVDEC and Vulkan checks. See the
  [measured report](../research/wayland-multiguest-2026-09-27.md) for the exact
  profiles, rates and remaining gates.

The direct 610 commands below document the original milestone path. They remain
useful for low-level experiments, but they are not the current desktop recipe.

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

### The guest driver: `CARAXES_DRIVER`

`CARAXES_DRIVER=610|615` selects the guest's driver for one run of `tools/caraxes-vm.sh`
and `tools/caraxes-gate.sh`. The default is the repository's `DRIVER_VERSION` (610.57.04).
The mapping lives in `tools/lib/driver.sh`; any other value is refused.

| | `610` (default) | `615` |
|---|---|---|
| Version | 610.57.04 | 615.71.09 |
| `LEA_BASE_IMAGE` | `$LEA_VM_DIR/guest-baked-20260801-610.57.04-20260820-105733.qcow2` | `/mnt/vmstore/caraxes/images/guest-baked-2026-09-24-615.71.09.qcow2` ([guest-driver-versions.md](guest-driver-versions.md) section 8); set before `local.env`, so its 610 default does not apply |
| Guest nvidia-smi and libraries | `/opt/nvrm/bin/nvidia-smi`, `/opt/nvrm/lib` | `/usr/bin/nvidia-smi`, `/usr/lib/x86_64-linux-gnu` |
| `CARAXES_GUEST_ABI` (for the device) | left as the caller has it (unset, or `595` for the experiment) | exported as `615`; another value is refused |

- A 615 VM needs its own overlay on the 615 image, in a new state directory:
  `qemu-img create -f qcow2 -F qcow2 -b /mnt/vmstore/caraxes/images/guest-baked-2026-09-24-615.71.09.qcow2 <dir>/rootfs.qcow2`,
  plus a `seed.img`.
- With `CARAXES_DRIVER` set, or with a driver other than 610, `up` checks that
  `LEA_BASE_IMAGE` carries the version in its name and is in the overlay's backing chain,
  and refuses otherwise.
- The device has a 615 ABI implementation. The status-queue limitation recorded in
  [guest-driver-versions.md](guest-driver-versions.md) section 5 is dated bring-up
  history and no longer describes the current 615 path.
- `sync-tests` writes the selected version into the guest's `leandro-test/DRIVER_VERSION`
  and the userspace paths into `caraxes-tests/caraxes/driver.env`. After a switch, run it
  again before the gates.

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

## Limits of the original 610 milestone path

- **No CUDA.** `nvidia-uvm` and the first CUDA context are the next experiment (E5);
  see [cuda-path.md](cuda-path.md) for the design and [experiments.md](../history/caraxes-experiments-2026-09-29.md)
  for status.
- **No display.** `nvidia-modeset` and `nvidia-drm` stay unloaded; scope is
  `nvidia.ko` only until CUDA is working.
- **Golden channel skipped.** The KernelGraphics floorsweeping reply is deliberately
  all zeros (`gpcMask` 0), which skips the golden-image channel and the Bug 4208224
  workaround; GR stays unusable for clients.

Full status and the experiment-by-experiment record: [experiments.md](../history/caraxes-experiments-2026-09-29.md).

Those three historical limits do not describe the tested 615 desktop configuration.
The current result does not imply full native parity. Long-duration stream soak,
hardware-decoded clients, resource-exhaustion recovery, full game runs and stronger
multi-guest isolation evidence remain open. Windows is still in Code 43 debugging;
no Windows rendering result exists.

## Host access and VM state

- Keep the host NVIDIA driver bound. The backend submits work through host RM;
  do not unbind, reset the GPU, reload host modules or write host registers directly.
- Registered guest RAM requires `CAP_SYS_ADMIN` for the privileged RM query.
  The launcher prepares a binary with `cap_sys_admin+p`; the backend enables it
  on the calling thread only around the required operation. See
  [memory-model.md](memory-model.md#4-host-resources-privilege-teardown).
- Use stock guest drivers. The current 615 GPA path needs no Caraxes guest module.
- VM overlays, sockets and logs live under `$CARAXES_STORE/$VM`, normally
  `/mnt/vmstore/caraxes/$VM`, with a repository-local fallback. Keep backing
  images frozen while their overlays are in use.
- Preserve Leandro v1 compatibility when changing shared RM or ABI code.
- Physical GPU passthrough is a separate workflow and is not used here.

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
