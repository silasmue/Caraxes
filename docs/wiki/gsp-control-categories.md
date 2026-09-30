<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GSP control categories

Of the 1397 controls of 615.71.09, 844 route to GSP-RM. The device sorts them by where the
truth of an answer lives: in the chip's data, on the host card, in the guest's own objects,
in a device model, or nowhere on this chip. The category decides the recipe, the isolation
argument and the truth source of every answer, and every answer carries a provenance label
(decision D9).

## The five categories

| Cat. | Meaning | Answered from | Isolation |
|---|---|---|---|
| A | pure chip data: static getters, caps, tables, anything `RMCTRL_FLAGS_CACHEABLE` | a per-chip table (`chips/<chip>/controls.toml` in `crates/vfio-user-nvgpu`) from source constants, chip files or one capture of the card | none: constants of the identity |
| B | host-forwardable: public getters of live state with a flat struct equal on host and guest (telemetry, clocks, PCIe, ECC status) | the device's own host subdevice or device, through a deny list and a public-control filter; identity fields rewritten to the profile | host-wide telemetry is visible; rate limits and TTL caches bound it |
| C | guest-state derived: controls on objects the guest allocated (channels, TSGs, context shares, ZBC) | the channel registry and the guest's own host mirror objects, never another client's | resolved to this VM's mirrors only |
| D | needs device modelling: engines, links, power, bus, display, engine-only state | a generated rule table: REFUSE, DENY, ACK, MODEL, MIRROR, TABLE | DENY rows refuse for isolation with `NV_ERR_INSUFFICIENT_PERMISSIONS` or `NV_ERR_NOT_SUPPORTED` |
| E | never expected from a stock client on this chip: features the chip lacks | refused per feature family the chip file names absent ([feature-families-and-chips](feature-families-and-chips.md)) | never reaches the host |

- Counts in the 615 snapshot: A 122, B 86, C 39, D 390, E 207 routed controls; answered 777
  (host-forwarded 69, synthesized 80, guest-derived 75, echo 65, refused 488), unhandled 67
  (A 50, C 1, D 16).
- The categories are name-and-flag heuristics of `caraxes/tools/gsp-surface.py`, recorded
  as they are; the device columns come from the crate at the snapshot's commit.

## What "refused" means

| Label | Status | Why |
|---|---|---|
| `refused (feature absent on this chip)` | `NV_ERR_NOT_SUPPORTED`, request echoed | an E family the chip lacks; the answer the card would give |
| `refused (feature absent on this board)` | `NV_ERR_NOT_SUPPORTED` or the header's status | a D REFUSE row (links, sync connector, notebook ACPI) |
| isolation refusal | `NV_ERR_INSUFFICIENT_PERMISSIONS` (9 rows) or `NV_ERR_NOT_SUPPORTED` (bus rows) | a D DENY row whose effect would reach shared hardware |
| unanswered | `NV_ERR_NOT_SUPPORTED` from the dispatcher's unanswered arm | a gap, not a decision |

- `NV_ERR_NOT_SUPPORTED` is what a vGPU guest's RM gives, and the kernel tolerates it on
  most paths: the watchdog init accepts it but fails `RmInitAdapter` on any other error
  (`src/nvidia/arch/nvalloc/unix/src/osinit.c`); the confidential-compute engine logs an
  assertion and continues (`src/nvidia/src/kernel/gpu/conf_compute/conf_compute.c`).
- User space can stop on a refusal: 615 libcuda failed `cuInit` when
  `NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO` was refused, so a refused control seen in a
  user-space trace is re-examined, not assumed harmless.
- A kernel can stop too: the Windows 617 KMD sends `GSP_SUSPEND` and reports Code 43 when
  `GR_GFX_POOL_QUERY_SIZE` is refused ([research: windows-findings](../research/windows-findings.md),
  P39). The Windows build also sends controls outside the export tables; the device answers
  those ahead of the categories ([guest-drivers-and-abis](guest-drivers-and-abis.md)).

## The D rule table

- The 308 D controls of 615 were sorted into 200 REFUSE, 37 DENY, 21 ACK, 42 MODEL, 3
  MIRROR and 5 TABLE, and 102 moved to E, 17 to C, 7 to A.
- In Caraxes runs no CUDA or Wayland workload has reached a D control; the one D control
  seen is `NV0073_CTRL_CMD_SPECIFIC_HDCP_CTRL` on a DisplayPort unplug. The real card's 610
  user-space traces show one more, `NV83DE_CTRL_CMD_DEBUG_SET_MODE_MMU_DEBUG` from cuda-gdb
  ([caraxes/gsp-plan/d-device-model.md](../vfio/gsp-plan/d-device-model.md)).
- The 12 display MODEL rows and the 4 open A rows are the remaining D gaps.

## Depth

- [research/gsp-surface.md](../research/gsp-surface.md): the counts and their evidence.
- [caraxes/gsp-surface.md](../vfio/gsp-surface.md): the inventory and its rounds.
- [caraxes/gsp-plan/b-host-forwardable.md](../vfio/gsp-plan/b-host-forwardable.md),
  [c-guest-state.md](../vfio/gsp-plan/c-guest-state.md),
  [d-device-model.md](../vfio/gsp-plan/d-device-model.md),
  [e-families.md](../vfio/gsp-plan/e-families.md): the per-category plans.
