<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Captured cards

One entry per physical card the project holds a capture of: what was captured, with which
driver and when, where it lives, what it fed, and what is still missing. A capture is
read-only evidence taken from a real card; the chip and board tables of the device are
built from it by hand. New entries come from [contributing-a-capture](contributing-a-capture.md).

## The cards

| Card | Chip, board | Driver | Date | Where it lives |
|---|---|---|---|---|
| NVIDIA GeForce RTX 2070 (home, the host card) | TU106, 10de:1f02 rev a1, subsystem 1458:4011 (Gigabyte), 8 GiB, BAR1 256 MiB | 610.57.04 at the first capture; 615.71.09 since 2026-09-24 | 2026-09-23 to 2026-09-27 | `caraxes/reference/rtx2070/`; native traces in `matrix/traces/610.57.04/` (outside git) |
| NVIDIA GeForce RTX 5060 Ti (workplace) | GB206, 10de:2d04 rev a1, subsystem 1043:8a08 (ASUS), 16 GiB, BAR1 16 GiB resizable | 615.71.09 on a 7.0.0-31-generic kernel | 2026-09-24 (`lab-capture` 1, 2 s, with sudo) | `caraxes/reference/gb206/` (`MANIFEST.txt`, `SHA256SUMS`) |

### RTX 2070 (TU106)

- Contains: config space (`lspci -xxxx`, `lspci -vvv`), `nvidia-smi -q`, `/proc` information and
  version; `rom.bin` and the synthesized VBIOS (outside git).
- BAR0 through the RM, not through `devmem` or `resource0` (both refused, `boot-registers.txt`):
  `GPU_EXEC_REG_OPS` READ_32, read-only (`caraxes/tools/hostreg-read.c`).
  - 7941 offsets (PMC, PBUS, FUSE, PTOP and its device-info table, PDISP, the Windows driver's
    early reads), 2026-09-25: `bar0-registers-2026-09-25.txt`.
  - The 1 MiB PROM window with the whole image chain, both FWSEC images and the falcon ucode
    table, 2026-09-25: `prom-window-2026-09-25.bin` and `.md`.
  - PMU and SEC2 falcon registers (224 dwords, 2026-09-25) and display registers
    0x640000 to 0x641000 (1024 reads, 2026-09-27).
- The Windows 617.14 KMD's adapter-start controls asked of the card by an unprivileged RM client:
  184 controls, 160 `NV_OK`, 2026-09-26 (`kmd-controls-2026-09-26.md`, `.tsv`).
- Fed: the PCI identity and config space, the two PCI ROM images, the `nvidia-smi` reference
  values, the native traces every gate compares against, the captured table for unmodelled BAR0
  reads (`crates/vfio-user-nvgpu/src/bar0.rs` `CapturedRegisters`), the recorded Windows answers
  (`recorded.rs`, `chips/tu106/recorded-controls.tsv`).
- Missing: a passthrough capture of the GSP boot and the real `GSP_INIT` response (planned as R4
  in [experiments](../history/caraxes-experiments-2026-09-29.md), not run); the category-A
  controls that need the card's answers; a lab-kit run with `--priv`.

### RTX 5060 Ti (GB206)

- Contains: `host/` (uname, os-release, cmdline, modules, firmware files with sums, a kernel log
  with 0 Xid, IOMMU groups); `gpu-0000_01_00.0/` (`lspci -vvv`, the 4096-byte config space, BAR
  and resizable-BAR sizes, sysfs attributes, `/proc` information, `nvidia-smi -q` as text, XML
  and a 237-field CSV, the sysfs ROM of 162304 bytes without a FWSEC image, 4 DRM connectors);
  `rm-tools/` (`MC_GET_ARCH_INFO`, PCI info, names, the 122-class list, engines, fault ids, PCE
  masks, CE caps, BAR and FB info, GR masks, the interrupt table).
- Fed: `chips/gb206/` (boot, chip, controls, device-info, interrupts, regmap) and
  `boards/rtx5060ti.toml`; the four physical-RM answers of the N gate; the `rtx5060ti` costume.
- Missing: the lab kit's sweep of 164 getters (the capture predates the kit), the `--priv` rows
  (`DP_GET_CAPS`), native traces, EDIDs, the device-info table and PES info in 615 layouts (the
  RM tools ran with unverified 610 headers), L2 and LTC counts, the linux-firmware `gb206/` list,
  the GSP RPC stream and firmware state (no capture can give them).

## Rules

- Serial numbers, UUIDs, the PBI mailbox and DSN bytes and host names are redacted in every filed
  capture; the device's GID is its own, never a card's.
- The chip is verified on the machine from `MC_GET_ARCH_INFO` (`rm-tools/lab-rm-query.txt`),
  never assumed from the product name. A second capture of a card is a new entry with its date.
- The kit's self-test on the RTX 2070 without sudo (`caraxes/runs/lab-selftest-2026-09-24/`)
  recorded 33 items, 30 ok, 2 reduced, 1 skipped, and a verified 53-file tarball; it tests the
  kit and is not a filed capture.

## Cards the project has no capture of

| Chip | Would validate | Where a capture could come from |
|---|---|---|
| TU104 (T4) | the Turing sibling with the NVLink object | a cloud T4 |
| GA10x (A10; L4 is AD104) | `ampere_later`, `ga10x_pcie_ro`, ECC on a pro board | cloud |
| GA100 (A100) | `mig`, `ecc`, `row_remap`, NVLink tier 0 or 1 | cloud, with MIG on |
| GH100 (H100, H200) | Hopper CUDA, `cc`, `ccu`, `mig`, `hshub`, `fabric` | cloud |
| GB100 (B200) | Blackwell data-center, `ccu`, `blackwell`, NVLE | cloud |
| GB202 (RTX PRO 6000 Server) | the closest GB20x in a cloud, with MIG | cloud |

## Entry template

```
| <product name> (<where>) | <chip>, <vendor:device> rev <r>, subsystem <ids> (<board vendor>), <VRAM>, BAR1 <size> | <driver> on <kernel> | <YYYY-MM-DD> (<kit version>, <runtime>, with or without sudo) | `caraxes/reference/<chip>/<lab-kit-stamp>/`; traces in `matrix/traces/<driver>/<chip>/` |
```

- Below the table, one section per card: Contains (the manifest's steps), Fed (chip and board
  tables, physical-RM answers, confirmed features), Missing (skipped, reduced, impossible).

## Depth

- [history/caraxes-environment-2026-09-23.md](../history/caraxes-environment-2026-09-23.md): the reference dumps of the RTX 2070.
- [vfio/blackwell-gb206.md](../vfio/blackwell-gb206.md): the GB206 capture read item by item.
- [vfio/gsp-plan/lab-kit.md](../vfio/gsp-plan/lab-kit.md) §6 to §9: what a tarball feeds,
  what a capture cannot give, which cloud part covers which chip.
