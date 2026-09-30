<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Identity and costume

Scope: where each guest-visible identity fact comes from, which layers may lie over the executing TU106 and which must stay true, and how far the costume ladder reached per rung.

## Facts

### What the guest derives from what

- D6: the card presents itself as `Leandro <board>-<size>` with the FB size from the profile policy, set at the device level (FB size and GSP static info), never by rewriting RM replies [docs/vfio/README.md §Decisions D6].
- The name is generated from `{transport, personality, profile}` (`Leandro VFIO RTX 2070-4G`); the backend never appears in it [docs/vfio/identity-profile.md "The naming model (P17)"].
- Nothing in the kernel RM compares the PCI device id with the chip from `PMC_BOOT_0/42`: the HAL comes from the boot registers alone [615 tree: src/nvidia/arch/nvalloc/unix/src/osapi.c:3809; docs/vfio/costume-5060.md §3].

| Guest-visible fact | Guest RM source (615) | Device takes it from | Evidence |
|---|---|---|---|
| Product name (NV2080 name string, nvidia-smi, NVML, CUDA, torch) | `NVGMC_SC_GPU_NAME_STRING` 0x2000 in the `GSP_INIT` response | naming model: board personality + profile size | [615 tree: src/nvidia/interface/gmcapi/gmcapi_gsp_config.h:140; docs/vfio/gsp-transport-615.md §5.3] |
| Short name | `NVGMC_SC_GPU_SHORT_NAME_STRING` 0x2001 | chip table (`TU106`) | [615 tree: src/nvidia/interface/gmcapi/gmcapi_gsp_config.h:141; docs/vfio/costume-5060.md §2] |
| /proc "Model" | the guest's own name table, keyed by PCI device id | board PCI id | [615 tree: kernel-open/nvidia/nv-procfs.c:124; 615 tree: src/nvidia/generated/g_nv_name_released.h:841] |
| PCI vendor/device/revision/subsystem, BAR sizes | config space and its BAR0 mirror | board data (`boards/*.toml`, `ids_from` of a capture) | [docs/vfio/costume-5060.md §1, §2] |
| Architecture, chip HAL, "Product Architecture" | `PMC_BOOT_0`/`PMC_BOOT_42` (`MC_GET_ARCH_INFO`) | chip table (`chips/<chip>/chip.toml`, overlays in `src/chip/derived.rs`) | [615 tree: src/nvidia/src/kernel/gpu/mc/kernel_mc.c:47; docs/vfio/costume-ladder.md §1] |
| FB size | `NVGMC_SC_FB_LENGTH` 0x1010 and the FB region keys | profile (`--fb-mib`, `CARAXES_FB_MIB`, `LEA_VRAM_PROFILE_MIB`) | [615 tree: src/nvidia/interface/gmcapi/gmcapi_gsp_config.h:111; docs/vfio/gsp-transport-615.md §5.3] |
| GPU UUID | `NVGMC_SC_GPU_GID_SHA1` 0x2002 | device's synthetic GID, seeded by the legacy-format name and the FB size; no capture UUID | [615 tree: src/nvidia/interface/gmcapi/gmcapi_gsp_config.h:142; docs/vfio/costume-5060.md §2] |
| VBIOS version | `NVGMC_SC_VBIOS_*` 0x50-0x53; on Turing also the PROM image the guest parses | board data (Turing: the synthesized TU106 image) | [docs/vfio/gsp-transport-615.md §5.3; docs/vfio/costume-5060.md §2] |
| Engine list | `NVGMC_SC_ENGINE_MASK` 0x1100 | chip table | [615 tree: src/nvidia/interface/gmcapi/gmcapi_gsp_config.h:124; docs/vfio/gsp-transport-615.md §5.3] |
| Class list, channel classes | class database of the presented chip | chip table; the host side needs `class_map` to the executing chip | [docs/vfio/costume-ladder.md §1] |
| Compute capability | derived by libcuda/NVML from the chip identity, not from GR info | chip table (presented chip) | [docs/vfio/costume-ladder.md §3.4] |
| SMs, L2 (torch) | GR info | host GR info of the physical card; chip `[gr] emulated_info` in pure emulation | [docs/vfio/costume-5060.md §2; docs/vfio/costume-ladder.md §1] |
| GPU Part Number | `GET_CHIP_DETAILS`: SKU plus pciDevId | host SKU + board device id (mixed, e.g. `2D04-400-A1`) | [docs/vfio/costume-5060.md §2] |
| Virtualization mode | `bIsVgx`, `hypervisorType` | device choice, default "not virtual" | [docs/vfio/identity-profile.md "Where the same numbers go under Leandro VFIO"] |

- 610 delta: the static identity travels as the `GspStaticConfigInfo` struct of `GET_GSP_STATIC_INFO`, and the costume's chip ids also appear in `INTERNAL_GPU_GET_CHIP_INFO` 0x20800A36, an id 615 reuses for the fabric-probe sync [docs/vfio/identity-profile.md "Where the same numbers go under Leandro VFIO"; docs/vfio/gsp-transport-615.md §7].

### Costume rules

| Layer | Lie over the TU106? | Evidence |
|---|---|---|
| PCI ids, subsystem, name, BAR sizes | yes, freely | [docs/vfio/costume-ladder.md §4] |
| Config-space layout | no: the Turing RM reads link registers at fixed XVE offsets 0x84/0x88 | [615 tree: src/common/inc/swref/published/maxwell/gm107/dev_nv_xve.h:104-105; 615 tree: src/nvidia/src/kernel/gpu/bif/arch/maxwell/kernel_bif_gm107.c:994] |
| Boot identity within a HAL family (TU104 on TU106) | yes | [docs/vfio/costume-ladder.md §3.1] |
| Boot identity across families (Ampere, Ada, Hopper) | yes, if VBIOS and boot model lie consistently where the new HAL looks | [docs/vfio/costume-ladder.md §3.2, §3.3] |
| Class database | yes toward the guest; the host needs a class map | [docs/vfio/costume-ladder.md §4] |
| Class ids in the method stream | no: libcuda writes them (SET_OBJECT) and the TU106 PBDMA/GR rejects a foreign class | [docs/vfio/costume-ladder.md §3.2; run: caraxes/runs/costume-ladder-2026-09-24/r2-ga106/gpu] |
| GMMU format | no, not without a v3-to-v2 shadow walker | [docs/vfio/costume-ladder.md §3] |
| Compute capability | not independently: it follows the chip identity | [docs/vfio/costume-ladder.md §3.4] |

- The whole GB206 config space on the TU106 made nvidia-smi report "Link Width Max 25x", because 0x84 is Device Capabilities 2 there; the 5060 costume therefore takes only the ids and keeps the TU106 layout [run: caraxes/runs/costume-2026-09-24/gb206-layout; docs/vfio/costume-5060.md §3].
- `--sm-version 7.0` put SM 0x0700 into every `INTERNAL_STATIC_KGR_GET_INFO` answer, yet nvidia-smi and torch kept 7.5 [run: caraxes/runs/costume-ladder-2026-09-24/sm70-tu106/gpu].
- With the GA106 identity and SM 7.5 in GR info, nvidia-smi reported 8.6, so an SM-version lie is a chip lie and the reverse [run: caraxes/runs/costume-ladder-2026-09-24/r2-ga106/gpu; docs/vfio/costume-ladder.md §3.4].
- The GA106 break comes before any QMD: `SET_OBJECT 0xC7C0` and `0xC7B5` in libcuda's first pushes raised host Xid 13 "Class 0xc7c0 Subchannel 0x1 Mismatch" and Xid 32 on the device's channels [run: caraxes/runs/costume-ladder-2026-09-24/r2-ga106/gpu].
- Cross-family boots needed the costume, never the guest, to change: GA106 a V3 boot-from-HS FWSEC descriptor, `NV_USABLE_FB_SIZE_IN_MB`, `RISCV_CPUCTL.ACTIVE_STAT` and libos heartbeat mailboxes; AD106 also the scrubber handoff; GH100 Hopper's FSP boot-complete scratch at 0x200BC [615 tree: src/nvidia/src/kernel/gpu/gsp/arch/ampere/kernel_gsp_falcon_ga102.c:204; 615 tree: src/common/inc/swref/published/ampere/ga102/dev_gc6_island_addendum.h:33; 615 tree: src/nvidia/src/kernel/gpu/gsp/arch/ada/kernel_gsp_ad102.c:81; 615 tree: src/common/inc/swref/published/hopper/gh100/dev_therm.h:26].
- The Turing family is the costume's reach for execution: TU104 on TU106 selects the same TU102 implementations and the same class ids [docs/vfio/costume-ladder.md §3.1].

### The ladder per rung

| Rung (`CARAXES_COSTUME`) | Lie | Pure emulation (nvidia-smi: compute cap, FB) | GPU check (vm1, host execution) | Broke at | Evidence |
|---|---|---|---|---|---|
| `rtx2080` TU104 | BOOT_0 0x164000a1, 10de:1e82 | 7.5, 1024 MiB | boots; nvml gate all but power-number; cuda gate 10 of 12 scored; 0 Xid | nothing | [run: caraxes/runs/costume-ladder-2026-09-24/r1-tu104] |
| `rtx5060ti` (ids only on TU106) | 10de:2d04, sub 1043:8a08, BAR0 64 MiB, name | not recorded | boots; 7.5, Turing; nvml all but power-number; cuda 10 of 12 scored; 0 Xid | the whole GB206 config space (XVE offsets) | [run: caraxes/runs/costume-2026-09-24/final] |
| `rtx3060` GA106 | Ampere HAL 0x176000a1, 10de:2503, GA106 classes | 8.6, 1024 MiB | boots; 8.6; cuda-init passes; cuda-jit `cuCtxCreate: 719`; host Xid 13 and 32 | class ids in the pushbuffer | [run: caraxes/runs/costume-ladder-2026-09-24/r2-ga106] |
| `rtx4060ti` AD106 | Ada HAL 0x196000a1, 10de:2803 | 8.9, 1024 MiB | not run | (emulation only) | [run: caraxes/runs/costume-ladder-2026-09-24/r3-ad106] |
| `h100` GH100 | Hopper HAL 0x180000a1 on the GB206 FSP model and GMMU v3, 10de:2331 | 9.0, 4096 MiB | not run | (emulation only) | [run: caraxes/runs/costume-ladder-2026-09-24/r4-gh100] |
| `rtx5060ti-gb206` | whole GB206 identity, `host_chip = "tu106"` | 12.0, 4096 MiB | not attempted | GMMU v3 tables vs the v2-only shadow walker | [run: caraxes/runs/costume-ladder-2026-09-24/r5-gb206] |

[Pure-emulation column: run: caraxes/runs/emu-regress-2026-09-25/ladder-conf, with the guest module options of docs/vfio/quickstart.md §2]

- Blackwell and Hopper identities in pure emulation (no costume over the TU106): see [blackwell-and-hopper.md](blackwell-and-hopper.md).

## Consequence for Leandro VFIO

- A presented chip is the base chip's tables plus a TOML overlay, so the overlay file is the complete list of lies.
- Execution on the RTX 2070 is honest only for Turing identities; a non-Turing identity is for pure emulation or for a host card of that family.
- Compute capability cannot be chosen separately from the chip identity; the operator's "fake the chip, keep the true SM" holds only inside the Turing family.
- Per-card values of a capture (UUID, PDI, DSN, serials) never enter a board file; the GID stays the device's own.

## Open

- Which of the two chip-identity inputs, the architecture from `MC_GET_ARCH_INFO` or the compute class, decides libcuda's compute capability is not measured (closed libraries).
- The SM 8.6 and 12.0 rungs and the GPU checks of AD106 and GH100 were not run; the ladder stopped after the GA106 Xids.
- The costume and ladder runs used the 610.57.04 guest image; no 615 guest has run a costume.
- nvml `power-number` reports `N/A` on every costume, the documented 610-on-615 limitation.
- Q33: whether a display-less identity (GA100, GH100) keeps a display when static info and fuses claim one.
