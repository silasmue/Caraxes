<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Captured cards

One row per physical card the project holds a capture of: what was captured, with which
driver and when, where it lives, what it fed, and what is still missing. A capture is
read-only evidence taken from a real card; the chip and board tables of the device are
built from it by hand. New rows come from [contributing-a-capture](contributing-a-capture.md).

## The cards

| Card | Chip, board | Driver of the capture | Date | Where it lives | What it contains | What it fed | What is missing |
|---|---|---|---|---|---|---|---|
| NVIDIA GeForce RTX 2070 (home, the host card) | TU106, 10de:1f02 rev a1, subsystem 1458:4011 (Gigabyte), 8 GiB, BAR1 256 MiB | 610.57.04 (host at capture); the host runs 615.71.09 since 2026-09-24 | 2026-09-23 | `caraxes/reference/rtx2070/` (config space as `lspci -xxxx`, `lspci -vvv`, `nvidia-smi -q`, `/proc` information and version; `rom.bin` and the synthesized VBIOS outside git); native user-space traces in `matrix/traces/610.57.04/` (95 files, outside git); the ioctl catalogue in `matrix/` of the VirtIO Leandro repository | PCI identity and configuration space, the two PCI ROM images, `nvidia-smi` reference values, the native traces every gate compares against | the direct boot-register read (`devmem` and `resource0` refused); the FWSEC image (the third ROM image is constructed, decision D8); a passthrough capture of the GSP boot and the real `GSP_INIT` response (`POWER_FEATURES`, `CHIP_SUB_REV`, SKU, ECID); the 50 category-A controls that need the card's answers; a lab-kit run with `--priv` |
| NVIDIA GeForce RTX 5060 Ti (workplace) | GB206, 10de:2d04 rev a1, subsystem 1043:8a08 (ASUS), 16 GiB, BAR1 16 GiB resizable | 615.71.09 on a 7.0.0-31-generic kernel | 2026-09-24 (`lab-capture` 1, 2 s runtime, with sudo) | `caraxes/reference/gb206/` (`MANIFEST.txt`, `SHA256SUMS`; `host/`: uname, os-release, cmdline, modules, firmware files with sums, kernel log with 0 Xid, IOMMU groups; `gpu-0000_01_00.0/`: `lspci -vvv` and 4096-byte config space, BAR and resizable-BAR sizes, sysfs attributes, `/proc` information, `nvidia-smi -q` as text, XML and 237-field CSV, the sysfs ROM (162304 bytes, PCI chain complete, no FWSEC image, expected), 4 DRM connectors; `rm-tools/`: `MC_GET_ARCH_INFO`, PCI info, names, the 122-class list, engines, fault ids, PCE masks, CE caps, BAR and FB info, GR masks, the interrupt table) | `chips/gb206/` (boot, chip, controls, device-info, interrupts, regmap) and `boards/rtx5060ti.toml`; the four physical-RM answers of the N gate (device-info table, CE physical caps, PCE masks, GR info); the `rtx5060ti` costume on the TU106 | the lab kit's sweep of 164 getters (the capture predates the kit), the `--priv` rows (`DP_GET_CAPS` for DP 2.x), native traces under the tracer, EDIDs (no monitor attached), the device-info table and PES info in 615 layouts (the RM tools ran with unverified 610 headers), L2 and LTC counts, the linux-firmware `gb206/` list, the GSP RPC stream and firmware state (no capture can give them) |

- Serial numbers, UUIDs, the PBI mailbox and DSN bytes and host names are redacted in every
  filed capture; the device's GID is its own, never a card's.
- The kit's own self-test on the RTX 2070 without sudo (2026-09-24, `caraxes/runs/lab-selftest-2026-09-24/`)
  recorded 33 items, 30 ok, 2 reduced, 1 skipped, and a verified 53-file tarball; it is a
  test of the kit, not a filed capture.

## Cards the project has no capture of

| Chip | Would validate | Where a capture could come from |
|---|---|---|
| TU104 (T4) | the Turing sibling with the NVLink object | a cloud T4 |
| GA10x (A10, L4 is AD104) | `ampere_later`, `ga10x_pcie_ro`, ECC on a pro board | cloud |
| GA100 (A100) | `mig`, `ecc`, `row_remap`, NVLink tier 0 or 1 | cloud, with MIG on |
| GH100 (H100, H200) | Hopper CUDA, `cc`, `ccu`, `mig`, `hshub`, `fabric` | cloud |
| GB100 (B200) | Blackwell data-center, `ccu`, `blackwell`, NVLE | cloud |
| GB202 (RTX PRO 6000 Server) | the closest GB20x in a cloud, with MIG | cloud |

## Row template

```
| <product name> (<where>) | <chip>, <vendor:device> rev <r>, subsystem <ids> (<board vendor>), <VRAM>, BAR1 <size> | <driver> on <kernel> | <YYYY-MM-DD> (<kit or capture version>, <runtime>, with or without sudo) | `caraxes/reference/<chip>/<lab-kit-stamp>/`; traces in `matrix/traces/<driver>/<chip>/` | <what the manifest lists: capture, rm-tools, sweep, traces, display, extras> | <which chip and board tables, which physical-RM answers, which features confirmed> | <what was skipped or reduced, what the card cannot give> |
```

- The chip column is verified on the machine from `MC_GET_ARCH_INFO` in
  `rm-tools/lab-rm-query.txt`, never assumed from the product name.
- A second capture of the same card is a new row with its own date; the older row stays.

## Depth

- [caraxes/environment.md](../caraxes/environment.md): the reference dumps of the RTX 2070.
- [caraxes/blackwell-gb206.md](../caraxes/blackwell-gb206.md): the GB206 capture read item by item.
- [caraxes/gsp-plan/lab-kit.md](../caraxes/gsp-plan/lab-kit.md) §6 to §9: what a tarball feeds,
  what a capture cannot give, which cloud part covers which chip.
