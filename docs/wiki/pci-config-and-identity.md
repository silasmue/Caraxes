<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# PCI configuration space and the PCI identity

The PCI configuration space is the first thing a guest sees of the synthetic GPU: vendor and
device ids, the BAR sizes, the capabilities. The kernel RM reads it twice, once through real
configuration cycles at probe and afterwards through a mirror in BAR0, and it reads some
offsets at fixed positions that a costume must not move.

## What the driver reads

| Item | Where | Function |
|---|---|---|
| vendor, device, revision, subsystem ids | config header and the BAR0 mirror `NV_PCFG` at 0x88000 | `gpuGetIdInfo_GM107` (`src/nvidia/src/kernel/gpu/arch/maxwell/kern_gpu_gm107.c`); `dev_nv_xve.h` names the mirror |
| BAR sizes | the config header; later `NV2080_CTRL_CMD_BUS_GET_PCI_BAR_INFO` | `kernel-open/nvidia/nv.c` at probe, `kern_bus.c` |
| MSI enable | mirror 0x88000 + 0x68 bit 16, kept in sync with the real capability | KernelBif (`src/nvidia/src/kernel/gpu/bif/`) |
| MSI re-arm | `NV_XVE_CYA_2` at mirror offset 0x704 (BAR0 0x88704), written once per interrupt; the value is ignored, hardware looks at the offset | `kernel_bif_gm107.c` |
| PCIe link registers | fixed XVE offsets 0x84 and 0x88 (`dev_nv_xve.h` for gm107) | `kernel_bif_gm107.c` |
| the option ROM | the PCI image chain through the BAR0 PROM window at 0x300000, not the ROM BAR | `kgspExtractVbiosFromRom_HAL` |
| the "Model" line in `/proc/driver/nvidia/gpus/*/information` | the driver's own name table keyed by PCI device id (`src/nvidia/generated/g_nv_name_released.h`) | `kernel-open/nvidia/nv-procfs.c` |

- No PCI id enters the chip choice; the HAL comes from the boot registers alone
  ([kernel-rm-hal-and-registry](kernel-rm-hal-and-registry.md)).
- The guest's PCI configuration capability walk (offsets 0x06, 0x34, 0x60, 0x68, 0x78) still
  costs 0 to 280 trapped reads per second; the caller is unidentified.

## TU106 against GB206

| | RTX 2070 (TU106) | RTX 5060 Ti (GB206) |
|---|---|---|
| ids | 10de:1f02 rev a1, subsystem 1458:4011 | 10de:2d04 rev a1, subsystem 1043:8a08 |
| BAR0 / BAR1 / BAR3 | 16 MiB / 256 MiB / 32 MiB | 64 MiB / 16 GiB (resizable 64 MiB to 16 GiB) / 32 MiB |
| capabilities | PM, MSI, PCIe | PM, MSI-X (9 vectors), PCIe, SR-IOV, ARI, resizable BAR, DSN, PBI mailbox VSEC |
| link | 8 GT/s x16 | 32 GT/s x16 |

- The synthetic `rtx5060ti` board keeps the ids and drops MSI-X, the extended space and the
  16 GiB BAR1; the PBI mailbox capability carries four UUID bytes, so it is removed and
  zeroed for every board.
- The whole GB206 configuration space on a TU106 identity made `nvidia-smi` report
  "Link Width Max 25x", because offset 0x84 is Device Capabilities 2 there: the layout stays
  the executing chip's ([identity-and-costume](identity-and-costume.md)).

## In the device

- `config.rs` seeds the space from the captured `lspci -xxxx` dump
  (`caraxes/reference/rtx2070/config-space.txt`); BARs, the ROM BAR, command and MSI
  registers are writable per PCI rules; the rest is read-only.
- The BAR0 mirror answers the same bytes; MSI enable there tracks the capability.
- Cloud Hypervisor's vfio-user client reads the space over the socket; it is not mappable,
  so a cache of read-only dwords in the VMM would need a patch.

## In VirtIO Leandro

- No PCI GPU exists in the guest: `virtio_nvrm.ko` is a virtio device; the NVIDIA libraries
  never see a PCI identity, and `nvrm_nodes.ko` supplies the proc files they read
  ([ARCHITECTURE.md](../ARCHITECTURE.md)).

## Depth

- [research/driver-structure.md](../research/driver-structure.md), [research/identity-and-costume.md](../research/identity-and-costume.md).
- [caraxes/kernel-rm-hw-surface.md](../caraxes/kernel-rm-hw-surface.md) §E, [caraxes/costume-5060.md](../caraxes/costume-5060.md),
  [caraxes/blackwell-gb206.md](../caraxes/blackwell-gb206.md) §1.
