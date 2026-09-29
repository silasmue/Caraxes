<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Identity and costume

The identity is everything the guest believes about its GPU: the PCI ids, the chip, the
name, the framebuffer size, the engines, the classes, the compute capability. In VFIO
Leandro it is a property of the synthetic device, set before any guest software runs, on
three axes: chip, board and driver version (decision D14). A costume is a presented identity
that differs from the executing chip; the ladder measured how far it can go.

## The naming model

- `Leandro <Transport> <Personality>-<Profile>`: `Leandro VFIO RTX 2070-4G` from v2,
  `Leandro VirtIO RTX 2070-4G` from v1; the legacy form `Leandro RTX 2070-4G` is an opt-in.
  One module builds it (`nvrm_abi::naming`, `crates/nvrm-abi/src/naming.rs`); the backend
  never appears in the name ([NAMING.md](../NAMING.md)).
- The profile is the guest framebuffer size, from a plain size or v1's vGPU-style catalogue
  (`RTX2070-4Q` resolves through the card's VMMU segment size to a whole-segment size).
- The GID is the device's own, seeded by the legacy name and the size; a capture's UUID
  never enters a board file.

## Where each guest-visible fact comes from (615)

| Fact | Guest RM source | Device takes it from |
|---|---|---|
| product name (`nvidia-smi`, NVML, CUDA, torch) | `NVGMC_SC_GPU_NAME_STRING` in the `GSP_INIT` response (`gmcapi_gsp_config.h`) | the naming model |
| short name | `NVGMC_SC_GPU_SHORT_NAME_STRING` | the chip table (`TU106`) |
| `/proc` "Model" | the driver's own name table by PCI device id (`g_nv_name_released.h`) | the board's PCI id |
| PCI ids, subsystem, BAR sizes | config space and its BAR0 mirror | `boards/*.toml` |
| architecture, chip HAL, "Product Architecture" | `NV_PMC_BOOT_0/42` (`MC_GET_ARCH_INFO`, `kernel_mc.c`) | `chips/<chip>/chip.toml` |
| FB size | `NVGMC_SC_FB_LENGTH` and the region keys | the profile |
| GPU UUID | `NVGMC_SC_GPU_GID_SHA1` | the synthetic GID |
| VBIOS version | `NVGMC_SC_VBIOS_*`; on Turing also the PROM image the guest parses | board data (Turing: the synthesized image) |
| engine list | `NVGMC_SC_ENGINE_MASK` | the chip table |
| class list | the class database of the presented chip | the chip table; the host side needs a class map to the executing chip |
| compute capability | derived by libcuda and NVML from the chip identity | the chip table |
| SMs, L2 | GR info | the host card's GR info; `[gr] emulated_info` in pure emulation |
| GPU part number | SKU plus PCI device id | host SKU plus board id |
| virtualisation mode | `bIsVgx`, `hypervisorType` | "not virtual" by default |

- In v1 the same numbers were produced by rewriting selected replies after the fact
  (`rewrite_fb_info`, `rewrite_gpu_name` in `vram.rs`); under the GSP role nothing is
  rewritten.

## Costume rules

| Layer | May lie over the executing TU106? |
|---|---|
| PCI ids, subsystem, name, BAR sizes | yes, freely |
| configuration-space layout | no: the Turing RM reads link registers at fixed XVE offsets |
| boot identity within the HAL family (TU104 on TU106) | yes: same TU102 implementations, same class ids |
| boot identity across families (Ampere, Ada, Hopper) | only in pure emulation, with the VBIOS and boot model lying consistently where the new HAL looks |
| class database | toward the guest yes; the host needs a class map |
| class ids in the method stream | no: libcuda writes `SET_OBJECT`, and the TU106 PBDMA and GR reject a foreign class (host Xid 13 "Class Mismatch") |
| GMMU format | no, not without a v3-to-v2 shadow walker |
| compute capability | not independently: it follows the chip identity |

## The ladder

| Rung (`CARAXES_COSTUME`) | Lie | Pure emulation | With host execution | Broke at |
|---|---|---|---|---|
| `rtx2080` (TU104) | BOOT_0 0x164000a1, 10de:1e82 | 7.5, boots | nvml gate, cuda 10 of 12, 0 Xid | nothing |
| `rtx5060ti` (ids only) | 10de:2d04, sub 1043:8a08, BAR0 64 MiB, the name | boots | 7.5, Turing, nvml and cuda as on the 2070, 0 Xid | the whole GB206 config space (XVE offsets) |
| `rtx3060` (GA106) | Ampere HAL, 10de:2503, GA106 classes | 8.6 | cuda-init passes, `cuCtxCreate` 719, host Xid 13 and 32 | class ids in the pushbuffer |
| `rtx4060ti` (AD106) | Ada HAL, 10de:2803 | 8.9 | not run | emulation only |
| `h100` (GH100) | Hopper HAL on the GB206 FSP model and GMMU v3, 10de:2331 | 9.0 | not run | emulation only |
| `rtx5060ti-gb206` | the whole GB206 identity | 12.0 | not attempted | GMMU v3 against the v2-only shadow |

- Cross-family boots needed the costume, never the guest, to change: GA106 a V3 FWSEC
  descriptor and the Ampere FB-size and heartbeat registers (`kernel_gsp_falcon_ga102.c`),
  AD106 the scrubber handoff (`kernel_gsp_ad102.c`), GH100 Hopper's FSP boot-complete scratch
  (`dev_therm.h` for gh100).
- Consequence: a chip executes only on a host card of its family; "fake the chip, keep the
  true SM" holds only inside the Turing family. The path to more chips is more host cards and
  the chip tables ([feature-families-and-chips](feature-families-and-chips.md)).

## Depth

- [research/identity-and-costume.md](../research/identity-and-costume.md): the cited lines and runs.
- [caraxes/identity-profile.md](../caraxes/identity-profile.md): the profile policy and the naming model.
- [caraxes/costume-5060.md](../caraxes/costume-5060.md), [caraxes/costume-ladder.md](../caraxes/costume-ladder.md),
  [caraxes/versatility.md](../caraxes/versatility.md): the ladder and the three axes.
