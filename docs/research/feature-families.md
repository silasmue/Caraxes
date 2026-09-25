<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Feature families

Scope: the 28 feature families of 615.71.09 that TU106 lacks, which chips have them, what the device answers, and where an honest answer would come from.

## Facts

### The mechanism

- The 28 families hold 302 controls: 200 in the 12 families of category E and 102 in the 16 families of the D rows moved to E [docs/caraxes/gsp-plan/e-families.md intro].
- `chips/<chip>/chip.toml` `[features]` names every family of the version `absent` or `present`, and an unknown name refuses the chip file [docs/caraxes/gsp-surface.md "After E and A"].
- The controls of an absent family answer `NV_ERR_NOT_SUPPORTED`, request echoed, label `refused (feature absent on this chip)`, and never reach the host [docs/caraxes/gsp-surface.md "After E and A"].
- A present family without a model falls through every handler to the dispatcher's "unanswered" `NV_ERR_NOT_SUPPORTED` [docs/caraxes/gsp-plan/e-families.md §4].
- TU106 names all 28 families absent; GB206 names `frl`, `dp2x`, `display_later`, `ampere_later`, `row_remap` and `blackwell` present [docs/caraxes/gsp-surface.md "After D"; docs/caraxes/gsp-plan/d-device-model.md §7].
- Of the 302 controls, 199 have a call site in the 615 kernel; the other 103 are user-space only or uncalled [docs/caraxes/gsp-plan/e-families.md Summary].
- The tree decides "which chip has it" by the per-chip child lists, the NVOC property HALs and the nvkms display HAL table, and some features by the board (links, ECC, sync connector, notebook ACPI) [docs/caraxes/gsp-plan/e-families.md §1, §2].

### The families

| # | Family | Ctl | What | Chips with the feature (as the docs give it) | TU106 answer | GB206 | Truth source |
|---:|---|---:|---|---|---|---|---|
| 1 | `rm_test` | 13 | RM self-tests, error and ucode injection | none in production | refused | absent | the `RM_TEST_ONLY_CODE` flag |
| 2 | `nvlink` | 93 | NVLink and INTERNAL_NVLINK | HAL: TU102, TU104, GA100, GA10x, AD10x, GH100, GB10x, GB20x; links only on SXM and bridge boards | refused | absent | tier 0 kernel logic; tier 1 capture |
| 3 | `hshub` | 8 | high-speed hub behind NVLink and C2C | GA100 and later with NVLink or EGM | refused | absent | kernel logic; HSHUB count by capture |
| 4 | `fabric` | 9 | FLA, fabric probe, fabric memory | FLA GA100 and later; probe on NVLink-capable chips | refused | absent | kernel logic (probe state machine) |
| 5 | `c2c` | 7 | chip-to-chip links | GH100, GB100 on Grace | refused | absent | capture of a Grace system |
| 6 | `gsync` | 9 | G-SYNC / Quadro Sync board | boards with a sync connector | refused | absent | closed board protocol |
| 7 | `vgpu` | 27 | vGPU host, SR-IOV VF | vGPU hosts | refused; `NVENC_SW_SESSION_STATS` NV_OK zeros | absent | isolation rule |
| 8 | `mig` | 12 | MIG and SMC | GA100, GH100, GB100, GB102, GB110, GB112, GB202, GB203 | refused | absent | capture with MIG on |
| 9 | `cc` | 9 | Confidential Computing | CC: GH100, GB10x, GB20x; APM: GA10x, AD10x | refused | absent, sent at boot | kernel logic for CC off |
| 10 | `spdm` | 1 | SPDM | GH100, GB10x, GB20x with CC on | refused | absent | firmware |
| 11 | `ccu` | 6 | clock-counter unit | GH100, GB100, GB102, GB20x | refused | absent, sent at boot | kernel logic; sample size by trace |
| 12 | `internal_uncalled` | 6 | INTERNAL without a kernel call site | none (one Tegra caller) | refused | absent | none |
| 13 | `frl` | 4 | HDMI 2.1 FRL | display HALs from C6: GA10x, AD10x, GB20x | refused | present, unanswered | kernel logic plus an HDMI 2.1 sink model |
| 14 | `dp2x` | 6 | DP 2.x UHBR, cable id | chips whose `DP_GET_CAPS` reports DP 2.0 (GB20x) | refused | present, unanswered | capture of `DP_GET_CAPS`, DP 2.1 spec |
| 15 | `display_later` | 4 | shared generic packets, SOR flush mode | display HALs C8, C9, CA and later | refused | present, unanswered | kernel logic |
| 16 | `display_embedded` | 6 | DSI, serializer, LVDS | Tegra display, LVDS boards | refused | absent | none |
| 17 | `notebook_power` | 18 | platform request handler, Dynamic Boost, AUX power, GCX | notebook boards with ACPI GPS or NVPCF | refused; 3 ACK | absent | platform ACPI |
| 18 | `ecc` | 4 | ECC configuration, reset, injection | ECC boards (HBM parts, pro GDDR boards) | refused | absent | capture |
| 19 | `row_remap` | 2 | DRAM row and bank remapping | HBM parts from GA100, GB20x | refused | present, unanswered | header, category B getters |
| 20 | `mig_partitioning` | 4 | MIG partitioning internals | as `mig` | refused | absent | as `mig` |
| 21 | `nvlink_sli` | 13 | NVLink peers, SLI bridge, boost sync | TU102, TU104, GA102 bridges; GA100, GH100 multi-GPU | refused | absent | capture of a bridged pair |
| 22 | `ampere_later` | 3 | faulted-bit clear, raw compression mode, access-bit dump | GA10x and later | refused | present, unanswered | kernel logic |
| 23 | `ga10x_pcie_ro` | 1 | PCIe relaxed ordering | GA100 to GA107 | refused | absent | kernel logic |
| 24 | `hopper_platform` | 2 | C2C probe filter, CC memory transfer | GH100, GB100 with Grace or CC | refused | absent | kernel logic |
| 25 | `blackwell` | 5 | CMC, DRAM encryption, MMU GCC debug, HES | GB10x, GB20x | refused | present, unanswered | kernel logic, header |
| 26 | `vgpu_paths` | 7 | vGPU host or guest only | vGPU host or guest | refused | absent | isolation rule |
| 27 | `windows_only` | 5 | Windows-only notifications, debugger contexts | Windows clients | refused; 2 ACK | absent | none |
| 28 | `legacy_uncalled` | 18 | verification-only, superseded, uncalled | none | refused; 1 ACK | absent | none |

[docs/caraxes/gsp-plan/e-families.md Summary table, §2, §3.1-3.28; docs/caraxes/gsp-surface.md "After E and A", "After D"; ACK rows: docs/caraxes/gsp-plan/d-controls.tsv column answer, docs/caraxes/gsp-plan/d-device-model.md §6.6]

- "refused" is `NV_ERR_NOT_SUPPORTED` with the request echoed; "ACK" is NV_OK with the request echoed; "present, unanswered" is the dispatcher's `NV_ERR_NOT_SUPPORTED` [docs/caraxes/gsp-surface.md "After E and A"; docs/caraxes/gsp-plan/d-device-model.md §2; docs/caraxes/gsp-plan/e-families.md §4].
- The three `notebook_power` ACKs are `SET_POWERSTATE`, `SET_AUX_POWER_STATE` and `UNSET_DYNAMIC_BOOST_LIMIT` [docs/caraxes/gsp-plan/d-device-model.md §6.6].
- The `legacy_uncalled` ACK is `INTERNAL_MEMSYS_SET_ZBC_REFERENCED`, which the kernel sends on Turing [docs/caraxes/gsp-plan/e-families.md §3.26-3.28].
- The two `windows_only` ACKs are `NV0073_CTRL_CMD_SYSTEM_NOTIFY_DRR_MSCG_WAR` and `NV5070_CTRL_CMD_SET_MEMPOOL_WAR_FOR_BLIT_TEARING` [docs/caraxes/gsp-plan/d-controls.tsv].
- `NVENC_SW_SESSION_STATS` is kept in the `empty_ok` list because the real card answers it NV_OK with 12 zero bytes [docs/caraxes/gsp-surface.md "After E and A"].

### Where the chips stand

- MIG is HAL-supported on GA100, GH100, GB100, GB102, GB10B, GB110, GB112, GB202, GB203, GR100 and GR102, not on GB205 to GB207 [615 tree: src/nvidia/generated/g_gpu_nvoc.c:335].
- NVLink is HAL-enabled from TU102 on except TU106, TU116 and TU117, and whether links exist is decided by the board's IOCTRL entries [615 tree: src/nvidia/generated/g_kernel_nvlink_nvoc.c:243; docs/caraxes/gsp-plan/e-families.md Summary].
- The CC-capable HAL covers GH100, the GB10x and GB202 to GB207, and CC turns on only when a scratch register says so [615 tree: src/nvidia/generated/g_gpu_nvoc.c:536; docs/caraxes/gsp-plan/e-families.md §2].
- Turing's display HAL C5 has no HDMI FRL, the C6 HAL has it [615 tree: src/nvidia-modeset/src/nvkms-evo3.c:7945, :8030].
- GA100, GH100 and GB100 have no display head, so the display families never reach them [docs/caraxes/gsp-plan/e-families.md §2].
- The TU106 child list has no ConfidentialCompute, Spdm, KernelCcu or KernelFsp object [docs/caraxes/gsp-plan/e-families.md §2].

### Sent at boot anyway

- The GB20x child list has ConfidentialCompute and KernelCcu [615 tree: src/nvidia/src/kernel/gpu/arch/blackwell/kern_gpu_gb202.c:87, :93].
- The ConfidentialCompute engine sends `INTERNAL_CONF_COMPUTE_GET_STATIC_INFO` 0x20800AF3 at state init and post-load on every chip whose child list has the engine, GA100 to GB20x [615 tree: src/nvidia/src/kernel/gpu/conf_compute/conf_compute.c:486, :591; docs/caraxes/gsp-plan/e-families.md §3.9].
- `kccuStateLoad` sends `INTERNAL_CCU_GET_SAMPLE_INFO` 0x20800AB2 on GB100 and GB20x, and `INTERNAL_CCU_UNMAP` 0x20800AB4 runs on unload [615 tree: src/nvidia/src/kernel/gpu/ccu/kernel_ccu.c:410-450; src/nvidia/src/kernel/gpu/ccu/arch/blackwell/kernel_ccu_gb100.c:43-58].
- The synthetic GB206 runs show these `cc` and `ccu` controls answered `NV_ERR_NOT_SUPPORTED`, and the gate passes anyway [run: caraxes/runs/blackwell-2026-09-24/n2c/device-events.txt; docs/caraxes/gsp-plan/e-families.md Summary].
- On the GH100 costume the guest asserts twice on the refused CC static info and logs a failed CCU memdesc map, and continues to the TU106 baseline wall [docs/caraxes/costume-ladder.md §3.3].
- The honest CC-off answer from source is NV_OK with `bIsBar1Trusted` and `bIsPcieTrusted` false [615 tree: src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080internal.h:3654-3657].
- `ccuSampleSize` is a GSP-RM number the kernel only multiplies into the buffer size, and no native trace can see it because the control is INTERNAL [docs/caraxes/gsp-plan/e-families.md §3.11].

### Truth sources by tier

| Tier | Families |
|---|---|
| refused on every chip, no hardware needed | `rm_test`, `vgpu`, `internal_uncalled`, `spdm`, `gsync`, `display_embedded`, `notebook_power`, `vgpu_paths`, `windows_only`, `legacy_uncalled` |
| answerable from kernel logic alone | `nvlink` tier 0, `hshub`, `fabric` single GPU, `cc` static info, `ccu`, `display_later`, `ampere_later`, `ga10x_pcie_ro`, `hopper_platform`, `row_remap` |
| need a capture or trace as truth or check | `mig` with `mig_partitioning`, `dp2x`, `frl`, `ecc`, `c2c`, `blackwell`, `nvlink` tier 1 with `nvlink_sli` |

[docs/caraxes/gsp-plan/e-families.md Summary, §5]

## Consequence for Caraxes

- TU106 stays the reference with every family absent, held by `chipctl::tests` and the 615 guest gates, and needs no family work.
- A chip that names a family present needs a model module before the flag means anything; until then present and absent differ only in the label.
- The GB206 identity's first family work is `cc` static info (NV_OK, two false booleans) and `ccu` (a chip-table sample size, recorded map and unmap), the only family controls its stock guest sends at boot.
- Captures for the capture tier come from the lab kit, not from the host card (see [methods.md](methods.md)); Blackwell and Hopper specifics are in [blackwell-and-hopper.md](blackwell-and-hopper.md); the surface counts are in [gsp-surface.md](gsp-surface.md).

## Open

- The value of `ccuSampleSize` for GB20x and GH100 is unmeasured [docs/caraxes/gsp-plan/e-families.md §3.11].
- The MIG profile tables need a capture of a MIG-capable part with MIG on [docs/caraxes/gsp-plan/e-families.md §3.8].
- Which chips report DP 2.0 in `DP_GET_CAPS` is a GSP-RM answer that needs a capture [docs/caraxes/gsp-plan/e-families.md §3.14].
- The HSHUB count per chip needs a capture to confirm the source-derived design [docs/caraxes/gsp-plan/e-families.md §3.3].
- NVLink tier 1, `nvlink_sli` and fabric memory need an HGX capture with two or more GPUs [docs/caraxes/gsp-plan/e-families.md §6].
