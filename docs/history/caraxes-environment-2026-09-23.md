<!-- SPDX-License-Identifier: MIT -->
# Environment (measured 2026-09-23)

## Host

| Item | Value | Source |
|---|---|---|
| GPU | NVIDIA GeForce RTX 2070, TU106, PCI `0000:2d:00.0`, `10de:1f02` rev a1, subsystem `1458:4011` (Gigabyte) | `lspci -nn`, `caraxes/reference/rtx2070/` |
| Functions on the card | `.0` VGA, `.1` HD audio `10de:10f9`, `.2` USB 3.1 `10de:1ada`, `.3` UCSI `10de:1adb` | `lspci -nn` |
| BARs | BAR0 16 MiB 32-bit non-prefetch at `fb000000`; BAR1 256 MiB 64-bit prefetch at `7fe0000000`; BAR3 32 MiB 64-bit prefetch at `7ff0000000`; BAR5 I/O 128 B at `f000`; expansion ROM 512 KiB at `fc000000` `[virtual] [disabled]` | `lspci -vvv` |
| Capabilities | PM v3 @60, MSI 64-bit 1 vector @68 (enabled), PCIe v2 legacy endpoint @78, VC @100, Power Budgeting @128, LTR @250, L1SS @258, AER @420, VSEC @600, Secondary PCIe @900, Resizable BAR @bb0 | `lspci -vvv` |
| Interrupt | MSI, IRQ 101 | `/proc/driver/nvidia/gpus/.../information` |
| Host driver | open kernel modules 610.57.04 (`license: Dual MIT/GPL`), GSP firmware 610.57.04 active; firmware files `gsp_tu10x.bin` (29 MB), `gsp_ga10x.bin`, `ucodes_tu10x.bin`, `ucodes_ga10x.bin` in `/usr/lib/firmware/nvidia/610.57.04/` | `modinfo nvidia`, `nvidia-smi -q` |
| VBIOS | 90.06.3d.00.76 | `nvidia-smi -q` |
| Kernel | 7.2.4-arch1-2; `CONFIG_KVM=m`, `CONFIG_VFIO_PCI=m`, IOMMU on (33 groups, GPU in group 28); no `CONFIG_MMIOTRACE` | `uname`, `/proc/config.gz`, sysfs |
| Kernel command line | `... nvidia.NVreg_ResmanDebugLevel=0` — this is why the host `nvidia.ko` logs every ioctl (`NVRM: ioctl(...)`) and pool-shrinker line to dmesg (`kernel-open/nvidia/nv.c:2486`, `os-interface.c:841-860`); present since 2026-09-22 18:11 | `/proc/cmdline`, journal |
| Display | The RTX 2070 is the only display device and drives the Wayland session (`DISPLAY=:0`, `WAYLAND_DISPLAY=wayland-1`). Passthrough experiments take the desktop down. | `lspci`, `loginctl` |
| QEMU | 11.1.1 with `vfio-user-pci` (socket, romfile/rombar, x-msg-timeout, x-no-posted-writes, x-send-queued, x-max-bounce-buffer-size, x-pci-*-id overrides) and `vfio-pci` (`x-no-mmap` usable for MMIO tracing) | `qemu-system-x86_64 -device help` |
| Rust | rustc/cargo 1.89.0 (pinned by `rust-toolchain.toml`) | |
| Other | clang, gcc, meson, ninja, python3, nix, `qemu-nbd`, `guestfish`; `sudo -n` works without password | |

Access limits found: mapping BAR0 through `/dev/mem` fails with EPERM and through
`/sys/.../resource0` with EINVAL (see [open-questions.md](caraxes-open-questions-2026-09-23.md) Q1), so
the real `NV_PMC_BOOT_0/1/42` values were not read directly. Expected from the
header encodings (`src/common/inc/swref/published/nv_ref.h:108-175`): BOOT_0 =
`0x166000A1` (architecture 0x16 in bits 28:24, implementation 6 in 23:20, major A,
minor 1), BOOT_1 = `0x00000000` (VGPU field 17:16 = REAL), BOOT_42 chip id `0x166`
in bits 29:20 with major/minor revision in 19:12. Verify in experiment R2.

## Reference dumps (`caraxes/reference/rtx2070/`)

| File | Contents |
|---|---|
| `config-space.txt` | Full 4 KiB PCI config space (`lspci -xxxx`) |
| `lspci-vvv.txt` | Decoded capabilities |
| `rom.bin` (git-ignored) | The PCI-visible ROM chain read through sysfs: 2 images, x86 (60416 B) and EFI (69632 B, marked last), 130048 B total. **This is not the full VBIOS**: the driver reads the ROM through the BAR0 PROM window (`NV_PROM_DATA`, 0x300000) and follows NVIDIA's own image chain past the PCI "last image" flag to find the FWSEC extension image (`kernel_gsp_vbios_tu102.c:276-416`). The FWSEC image is needed to build the FRTS command. See experiment R1. |
| `nvidia-smi-q.txt`, `proc-information.txt`, `proc-version.txt` | Board and driver identification |
| `boot-registers.txt` | Failed attempts (kept as evidence of the access limits) |

## VM assets already on this machine

`local.env` points at `/mnt/vmstore/leandro/vm`:

- `guest-baked-20260801-610.57.04-20260820-105733.qcow2`: Ubuntu 24.04 server, kernel
  6.8.0-136-generic, NVIDIA userspace 610.57.04 under `/opt/nvrm/lib`, headers and
  build tools installed, no NVIDIA kernel module (Leandro uses `virtio_nvrm.ko`). The
  stock open module can be built in-guest from `vendor/open-gpu-kernel-modules` (same
  tag) and `gsp_tu10x.bin` copied to `/lib/firmware/nvidia/610.57.04/`.
- NixOS guest images (`guest-nixos-610.43.03-*`) with kernel 6.12; the recipe
  (`nix/guest-image.nix`) can add the open driver via `nvidiaPackages.mkDriver`.
- Cloud Hypervisor binary at `/mnt/vmstore/leandro/cloud-hypervisor/bin/cloud-hypervisor`.

## Vendor checkouts (state after the 2026-09-23 update pass)

| Path | Upstream | Before | Now | Note |
|---|---|---|---|---|
| `vendor/cloud-hypervisor` | cloud-hypervisor/cloud-hypervisor | v53.0 + 3 local patches applied | unchanged; v53.0 is the newest tag (fetched tags) | The patch series is documented in `patches/README.md`; the user considers it incomplete for upstreaming |
| `vendor/vhost-device` | rust-vmm/vhost-device | main @ 93f867e | unchanged; equals `origin/main` after fetch | |
| `vendor/open-gpu-kernel-modules` | NVIDIA/open-gpu-kernel-modules | 610.57.04 (blob:none) | unchanged, tags fetched; newest tag is **615.71.09** | Kept at the running host driver version on purpose (`tools/build.sh check-driver`); the ABI crate already carries 615 layouts (`crates/nvrm-sys/src/v615.rs`) |
| `vendor/nvidia-rm-headers` | (tracked in this repo) | 580/595/610/615 header closures | unchanged | |
| `vendor/vfio` | rust-vmm/vfio (new, git-ignored) | – | main, shallow | `vfio-user` crate 0.1.6: client + server, `examples/` |
| `vendor/libvfio-user` | nutanix/libvfio-user (new, git-ignored) | – | master, shallow | C reference implementation; the protocol spec moved to QEMU |
| `vendor/qemu-vfio-user` | qemu/qemu v11.1.1 (new, git-ignored) | – | `hw/vfio-user/*`, `hw/vfio/pci.c`, `docs/interop/vfio-user.rst` | Fetched as single files for reading, not a checkout |
| `vendor/hostvenv` | – | Python venv | unchanged | Not a repository |

Graphify: the repository graph in `graphify-out/` covers the Leandro tree (built from
`e4134ded`). A separate AST-only graph of the NVIDIA RM tree was built with
`graphify update vendor/open-gpu-kernel-modules/src/nvidia --no-cluster` (30 s,
`vendor/open-gpu-kernel-modules/src/nvidia/graphify-out/graph.json`, git-ignored);
use it with `--graph` for `path`/`explain`/`query`.

## Host-side setup for the experiments (2026-09-23)

- VM state (overlays, seeds, sockets, logs) lives on the NAS at `/mnt/vmstore/caraxes/<vm>`
  (NVMe over a 25 Gbit RDMA link, created by the operator on 2026-09-23); the launcher
  `tools/caraxes-vm.sh` falls back to `target/caraxes/<vm>` when that directory is absent.
  The first E0 run used the fallback.
- Tap for the guest network, created on the host on 2026-09-23 (not persistent across reboots):

  ```sh
  sudo ip tuntap add dev caraxes0 mode tap user silas
  sudo ip addr add 192.168.100.1/24 dev caraxes0 && sudo ip link set caraxes0 up
  ```

  The Leandro guest image already has `192.168.100.11/24` in its netplan and the
  `leandro-vm` SSH key authorized; the cloud-init seed the launcher expects sets the
  hostname `caraxes-vm0` and `192.168.100.20/24` instead (`seed/user-data` next to the overlay).
- Guest preparation (done once per overlay): `vendor/open-gpu-kernel-modules` packed as
  `ogkm-610.57.04.tar.zst` (26 MB) and `gsp_tu10x.bin` copied by scp; in-guest build with
  `make modules -j8 SYSSRC=/lib/modules/6.8.0-136-generic/build`.
