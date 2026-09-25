# Windows 11 guest (2026-09-25)

Goal: a Windows 11 guest under Cloud Hypervisor, first without the GPU, then with the
vfio-user device (`nova-guest.md` section 7, P23: which RM path the Windows driver takes).

## Why two hypervisors

- Cloud Hypervisor has no VGA and no GOP, so the Windows installer cannot be driven under it.
- The install runs once under QEMU (OVMF with Secure Boot, swtpm TPM 2.0, virtio disk and
  network, `install-qemu.sh`). The operator clicks through the setup and loads the virtio
  storage driver from the virtio-win ISO.
- The installed disk then boots under Cloud Hypervisor (`up-ch.sh`): EDK2 `CLOUDHV.fd`
  (built with `SECURE_BOOT_ENABLE` and `TPM2_ENABLE`), `--tpm socket=` to the same swtpm
  state, virtio-blk and virtio-net. Headless: serial log, RDP or Sunshine over the tap.

## Files

| Path | What |
|---|---|
| `/mnt/vmstore/caraxes/win11/` | state: `win11.qcow2` (80 GiB, thin), `swtpm/` (TPM state, shared by both), `OVMF_VARS.4m.fd` (QEMU vars), `CLOUDHV.fd` |
| `/mnt/vmstore/caraxes/win11/iso/` | `virtio-win.iso` (public download), the Windows ISO stays in `~/Downloads` |
| `target/win11/edk2/` | the EDK2 tree and build (`edk2.log`, `build.log`) |
| `install-qemu.sh` | the one-time install under QEMU (window on the operator's desktop) |
| `up-ch.sh`, `down-ch.sh` | the Cloud Hypervisor run (tap `caraxes4`, 192.168.104.0/24) |

## Setup notes

- Windows 11 checks TPM 2.0 and a Secure-Boot-capable UEFI at setup. QEMU with
  `OVMF_CODE.secboot.4m.fd` and swtpm satisfies both; the `LabConfig` registry bypass
  (`Shift+F10`, `reg add HKLM\SYSTEM\Setup\LabConfig /v BypassTPMCheck ...`) is the fallback.
- During setup, "Load driver" from the virtio-win CD: `viostor\w11\amd64` (virtio-blk), later
  `NetKVM\w11\amd64`, `Balloon`, `vioserial`, `viorng` from the same ISO.
- OOBE without a Microsoft account: `Shift+F10`, `OOBE\BYPASSNRO` (25H2 may need
  `start ms-cxh:localonly`).
- Under Cloud Hypervisor the disk must be virtio-blk and the firmware `CLOUDHV.fd`; OVMF does
  not run there. The TPM state directory is the same, so BitLocker or TPM-bound state survives
  the move. Windows may ask for re-activation after the hardware change.
- The GPU comes later: `--user-device socket=...` as in `tools/caraxes-vm.sh`, first in pure
  emulation to see which driver path Windows takes.

## Firmware build (done 2026-09-25 14:41)

- edk2-stable202508, `OvmfPkg/CloudHv/CloudHvX64.dsc`, RELEASE, GCC5, `SECURE_BOOT_ENABLE=TRUE`,
  `TPM2_ENABLE=TRUE`, four jobs, two minutes; result `/mnt/vmstore/caraxes/win11/CLOUDHV.fd`
  (4.2 MiB).
- BaseTools do not build with GCC 16 as shipped (`-Werror` on an unused variable in
  `StringFuncs.c`): the three `-Werror` in `BaseTools/Source/C/Makefiles/header.makefile` were
  removed in the build tree before `make -C BaseTools`.
- Host packages added for this: `swtpm`, `nasm`, `acpica`.
