<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Feature families and chips

Much of the RM's control surface belongs to features a given chip does not have: NVLink,
MIG, confidential computing, HDMI 2.1, DisplayPort 2.x. The device groups those controls
into feature families, names each family present or absent per chip, and refuses an absent
family the way the card itself would. The chip tables also hold what a second chip such as
GB206 needs beyond Turing.

## The mechanism

- 28 families hold 302 controls of 615.71.09 (200 from category E, 102 from the D rows
  moved to E); 199 of them have a call site in the 615 kernel, 103 are user-space only or
  uncalled.
- `chips/<chip>/chip.toml` `[features]` names every family of the version `absent` or
  `present`; an unknown name refuses the chip file. TU106 names all 28 absent; GB206 names
  `frl`, `dp2x`, `display_later`, `ampere_later`, `row_remap` and `blackwell` present.
- An absent family answers `NV_ERR_NOT_SUPPORTED` with the request echoed, labelled
  `refused (feature absent on this chip)`, and never reaches the host. A present family
  without a model falls through to the dispatcher's unanswered `NV_ERR_NOT_SUPPORTED`; until a
  model exists, present and absent differ only in the label.
- The tree decides "which chip has it" by the per-chip child lists
  (`src/nvidia/src/kernel/gpu/arch/*/kern_gpu_*.c`), the NVOC property HALs
  (`src/nvidia/generated/g_gpu_nvoc.c`, `g_kernel_nvlink_nvoc.c`) and the nvkms display HAL
  table (`nvkms-evo3.c`), and some features by the board (links, ECC, a sync connector,
  notebook ACPI).

## The families

| Family | Controls | Chips with the feature | Truth source |
|---|---:|---|---|
| `nvlink`, `nvlink_sli`, `hshub`, `fabric`, `c2c` | 93, 13, 8, 9, 7 | HAL from TU102 on except TU106, TU116, TU117; links only on SXM and bridge boards; C2C on Grace systems | kernel logic for tier 0; a capture of a bridged pair or an HGX node for tier 1 |
| `mig`, `mig_partitioning` | 12, 4 | GA100, GH100, GB100, GB102, GB110, GB112, GB202, GB203, not GB205 to GB207 | a capture with MIG on |
| `cc`, `spdm`, `ccu`, `hopper_platform`, `blackwell` | 9, 1, 6, 2, 5 | CC on GH100, GB10x, GB20x; CCU on GH100, GB100, GB20x; the Blackwell set (CMC, DRAM encryption, HES) on GB10x and GB20x | kernel logic for CC off (`bIsBar1Trusted`, `bIsPcieTrusted` false); `ccuSampleSize` only from firmware |
| `frl`, `dp2x`, `display_later`, `display_embedded` | 4, 6, 4, 6 | HDMI FRL from display HAL C6 (GA10x on); DP 2.x where `DP_GET_CAPS` says so (GB20x); DSI and LVDS on Tegra | kernel logic plus an HDMI 2.1 sink model; a capture of `DP_GET_CAPS` |
| `ecc`, `row_remap` | 4, 2 | HBM and pro GDDR boards | capture |
| `vgpu`, `vgpu_paths` | 27, 7 | vGPU hosts and guests | refused everywhere (isolation rule) |
| `notebook_power`, `gsync`, `ga10x_pcie_ro`, `ampere_later` | 18, 9, 1, 3 | notebook ACPI, a sync connector, GA10x relaxed ordering, GA10x-and-later bits | platform ACPI; a closed board protocol; kernel logic |
| `rm_test`, `internal_uncalled`, `windows_only`, `legacy_uncalled` | 13, 6, 5, 18 | none in production Linux | refused everywhere |

- A few rows are acknowledged with `NV_OK` because the card does: three `notebook_power`
  setters, `INTERNAL_MEMSYS_SET_ZBC_REFERENCED`, two Windows-only notifications, and
  `NVENC_SW_SESSION_STATS` with 12 zero bytes.
- Sent at boot anyway: `INTERNAL_CONF_COMPUTE_GET_STATIC_INFO` on every chip whose child
  list has the ConfidentialCompute engine (GA100 to GB20x), and the CCU sample-info and unmap
  controls on GB100 and GB20x; the synthetic GB206 answers them `NV_ERR_NOT_SUPPORTED` and the
  gate passes.

## What a second chip needs (GB206 from the lab capture)

| Item | GB206 | TU106 |
|---|---|---|
| `MC_GET_ARCH_INFO` | arch 0x1B0, implementation 6, chip id 0x1B6; `GB206-A` | Turing, `TU106` |
| boot | FSP Chain of Trust, `gsp_ga10x.bin`, no ROM parsing ([gsp-boot-exchange](gsp-boot-exchange.md)) | FWSEC and Booter from the VBIOS, `gsp_tu10x.bin` |
| GMMU | v3, 52-bit physical addresses | v2, 47 bits |
| classes | CA6F channel, CAB5 copy, CE97 3D, CEC0 compute | C46F, C5B5, C597, C5C0 |
| engines | GR0, COPY0/4/7, NVDEC, NVENC, NVJPG, OFA, SEC2; GR fault id 384 | GR0, CE0 to CE4, three NVDEC; fault id 64 |
| interrupts | vectors 0x84, 0x40, 0x48, 0x94; RUNLIST and RUNLIST_NOTIFICATION subtrees | the Turing table |
| compute capability | 12.0 (GR SM version 0x0C00) | 7.5 |
| PCI | 64 MiB BAR0, 16 GiB resizable BAR1, MSI-X, SR-IOV | 16 MiB, 256 MiB, MSI |

- The GB206 tables live in `chips/gb206/` (`boot.toml`, `chip.toml`, `controls.toml`,
  `device-info.toml`, `interrupts.toml`, `regmap.toml`); four physical-RM answers come from the
  capture (device-info table, CE physical caps, PCE masks, GR info).
- The N gate in pure emulation passes N0 to N4 (device, PCI identity, driver load,
  `nvidia-smi` with Architecture Blackwell, the first `BLACKWELL_CHANNEL_GPFIFO_B` channel)
  on the 610 guest; past N4 the first push never completes, because no Blackwell method
  runs on the TU106.
- GH100 reuses the FSP model with `chips/gh100/boot.toml` for Hopper's own scratch, WPR2 and
  fuse registers and reports compute capability 9.0 in emulation.

## Depth

- [research/feature-families.md](../research/feature-families.md): all 28 families with lines.
- [research/blackwell-and-hopper.md](../research/blackwell-and-hopper.md): GB206, FSP, GMMU v3, the N gate.
- [caraxes/gsp-plan/e-families.md](../caraxes/gsp-plan/e-families.md), [caraxes/blackwell-gb206.md](../caraxes/blackwell-gb206.md),
  [caraxes/blackwell-gate.md](../caraxes/blackwell-gate.md).
