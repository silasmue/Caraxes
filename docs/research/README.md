<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Caraxes research notes

Rule: short and precise; bullets; every fact with its evidence; long form lives in [docs/caraxes](../caraxes/README.md); records under `caraxes/runs`.

Target: driver 615.71.09 only (D19, D20); 610.57.04 appears as one "610 delta" bullet at most.

## Notes

| Note | Scope |
|---|---|
| [driver-structure.md](driver-structure.md) | How the 615 kernel RM picks its HAL, the module options and registry keys the device needs, what the driver reads from where, the unstable-interface opt-in |
| [gsp-surface.md](gsp-surface.md) | The GSP surface in numbers: controls, RPCs, events, classes, boot types; categories A to E; families; what "refused" means |
| [gsp-transport.md](gsp-transport.md) | Message queue v2, GMCAPI, element framing, the doorbell trap, the boot exchange in order, teardown, suspend, `NV_ABI_STABLE` against unstable |
| [gsp-rpc-events-classes.md](gsp-rpc-events-classes.md) | The 14 RPCs a 615 client uses and their answers, the events, the classes on TU106 and the refusal rule, `ALLOC_MEMORY` |
| [abi-baseline.md](abi-baseline.md) | The 615.71.09 snapshot under `caraxes/abi/`, the diff tool, the 610-to-615 delta, the workflow for the next driver |
| [identity-and-costume.md](identity-and-costume.md) | What the guest derives its identity from, the costume rules, the costume ladder and its limits |
| [feature-families.md](feature-families.md) | The 28 control families, which chips have them, refused or answered, sent at boot anyway, truth sources |
| [memory-model.md](memory-model.md) | Guest RAM registration, the swiotlb alias, D16 to D18, shadow page tables, gathers and region pools, fail-closed rules |
| [windows-and-traps.md](windows-and-traps.md) | Every shared-memory window, what it removes, what still traps per operation and per frame, the cost per trap |
| [isolation-and-xids.md](isolation-and-xids.md) | The isolation rules and the catalogue of every Xid seen with cause and fix |
| [performance-facts.md](performance-facts.md) | Ratios to native per operation and workload, where the remaining time goes, the ReBAR note |
| [methods.md](methods.md) | Gates, census, profile mode, coverage map, guest sweep, lab kit, emu-boot check, ABI drift check, how a new workload is measured |
| [blackwell-and-hopper.md](blackwell-and-hopper.md) | GB206 identity, FSP boot, GMMU v3, the N gate, GH100 in emulation, what a capture or cloud validation adds |
| [windows-driver-code43.md](windows-driver-code43.md) | The Windows driver's register sequence before Code 43 by name, devinit on Turing, the VBIOS scan, ranked causes and the next experiments |

## Format of a note

- Title, one line "Scope:", then "Facts" (one sentence each, evidence in brackets), "Consequence for Caraxes", "Open".
- Evidence is a line in the 615 tree (`/home/silas/git/Leandro/vendor/open-gpu-kernel-modules`, cited as `615 tree: path:line`), a run folder under `caraxes/runs`, or a `docs/caraxes` document and section.
- No narrative, no history, no status; numbers that compare go in tables.

## Maintenance

- A fact changes: fix the bullet in its note and cite the new evidence (new run folder, new line, new section); do not append a second, contradicting bullet.
- A new finding goes into the note of its topic; the working record and the status log stay in [docs/caraxes/README.md](../caraxes/README.md).
- A new driver version: rerun the ABI diff ([abi-baseline.md](abi-baseline.md)), then recheck every `615 tree:` citation the diff touches.
