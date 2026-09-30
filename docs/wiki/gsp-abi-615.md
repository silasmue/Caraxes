<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# The GSP ABI baseline: 615.71.09 and later versions as deltas

From 615.71.09 NVIDIA marks the transport and boot layer of the GSP interface as stable and
calls the RM RPC layer above it an opt-in unstable interface. VFIO Leandro records that
version as a machine-readable baseline and treats every later driver as a reviewed diff
against it (decision D20).

## Stable and unstable in the tree

- `NV_ABI_STABLE` (`src/common/sdk/nvidia/inc/nvtypes.h`) is a no-op tag "for human benefit
  only" that static checkers may use "to verify no changes across releases". It tags 12
  structs and 5 enums: `msgqTxHeader`, `GSP_MSG_QUEUE_ELEMENT`, `MESSAGE_QUEUE_INIT_ARGUMENTS`,
  `GSP_SR_INIT_ARGUMENTS`, `GSP_ARGUMENTS_CACHED`, `GMCAPI_HEADER`, the `GSP_FMC_*` and
  `GSP_*_PARAMS` boot structs, and the GMCAPI enums (`src/nvidia/arch/nvalloc/common/inc/gsp/gspifpub.h`).
  Stable structs grow only into reserved space.
- The RM RPC layer is opted into with `NVGMC_SI_UNSTABLE_INTERFACE_SUPPORT` (bits RPC and
  EVENTS, `src/nvidia/interface/gmcapi/gmcapi_system_info.h`); the 615 kernel sets both.
  Everything that layer carries (`GSP_RM_CONTROL`, `GSP_RM_ALLOC`, every control and alloc
  layout, events, RPC numbers, the internal handles, the NVKV keys still in
  `gmcapi_system_info_unstable.h`) is outside the promise. Queue encryption is explicitly
  not stable.
- The kernel still accepts only a GSP-RM image whose `.fwversion` equals its own.
- The device honours the opt-in: without the RPC bit RM RPCs are refused, without the
  EVENTS bit no RM event is sent.

## The snapshot

`caraxes/abi/gsp-615.71.09/`, made by `caraxes/tools/gsp-abi.py snapshot`, nine sorted TSVs
with SPDX headers and a MANIFEST (driver, `version.mk` hash, commit, compilers, rows and
sha256 per file):

| File | One row per | Rows |
|---|---|---:|
| `controls.tsv` | exported control: params struct, size, layout hash, route, privilege, category, device answer | 1397 |
| `rpc.tsv` | RPC function: payload struct, size, hash, sent by a GSP client, device | 232 |
| `events.tsv` | event: payload, size, dispatched by the CPU side, sent by the device | 45 |
| `classes.tsv` | `RS_ENTRY` class: route, reaches GSP, alloc params, in TU106's database, device | 224 |
| `boot.tsv` | stable or boot type the device implements: size, hash, version constants | 41 |
| `layouts.tsv`, `constants.tsv`, `enums.tsv`, `bitfields.tsv` | field offsets, transport constants, stable enumerators, DRF bit ranges | 666, 242, 58, 924 |

- Snapshots of 610.57.04 and 570.144 sit beside it in `caraxes/abi/`, with the deltas
  `delta-610.57.04-615.71.09.md` and `delta-615.71.09-570.144.md`.
- Sizes are compiler `sizeof`; the layout hash covers clang's record layout, so a field
  moved at equal size changes it. Driver-side columns depend only on the tree; the device
  columns are read from the crate. `transport.tsv` adds the transport class
  ([gsp-transport-classes](gsp-transport-classes.md)).

## Workflow for driver N

1. `gsp-abi.py snapshot --ogkm <tree-N> --out caraxes/abi/gsp-N/`, committed on its own.
2. `gsp-abi.py diff caraxes/abi/gsp-615.71.09 caraxes/abi/gsp-N`; exit 1 means a struct
   the device implements changed size or offsets; `--strict` fails on any driver-side change.
3. Classify each change. A change in the stable layer (a stable struct, the element
   framing, the boot arguments, the GMCAPI header, a promoted key, the opt-in) is a baseline
   break and stops for a decision; the sender survey is redone.
4. Regenerate the per-version tables into `src/ver/vN/` (`gsp-tables.py`, `ctrl-layouts.py`,
   `gsp-dtable.py`) after the version's `nvrm-sys` bindings exist; `ver/vN.rs` overrides
   what moved. For the one binary add a `ver::GuestAbi` variant, a `guest_layouts!` table,
   the private stand-ins in `src/ver/common.rs` and, for a new transport or element frame, a
   rule in `BootModel::detect_guest`
   ([research: one-abi-binary](../research/one-abi-binary-2026-09-28.md),
   [guest-drivers-and-abis](guest-drivers-and-abis.md)).
5. If the host moves to N, add a `hostabi.rs` row per changed host call, GSS-legacy sizes
   from an `nvidia-smi -q` trace included.
6. `tools/check.sh`, the unit tests, the guest gates with an image of version N.
7. Record the delta; a new baseline replaces D20's row.

- The 610-to-615 delta: 185 driver-side changes in 34 groups, 26 touching what the device
  implements (the element grew from 16 to 24 bytes, `msgqRxHeader` went, `GSP_ARGUMENTS_CACHED`
  152 to 184, `rpc_rc_triggered` 48 to 64, seven control structs, one reused id 0x20800A36).
- What the diff cannot see: GSS-legacy sizes (no open header), the new meaning of the
  channel and TSG `internalFlags`, the register-held queue pointers, and which RPCs a client
  stopped sending; those are the hand-kept survey column. Controls compiled only into the
  Windows build (0x20801111) are in no export table and in no snapshot.
- The 610-to-570 diff lists 57 changes that affect the device. nouveau reaches six: the
  control frame, two allocation structs, the OS error log and the kernel interrupt table, all
  translated, and `GPFIFO_SCHEDULE`, echoed at the guest's size
  ([research: open-render](../research/open-render-2026-09-29.md)).

## The drift check

- `tools/check.sh` step `gsp-abi` renders the snapshot against
  `docs/vfio/gsp-abi-615-tables.md` and runs `diff --strict` against a found 615 tree
  (`CARAXES_OGKM_615`, the Leandro checkout beside the repository, or a NAS path); without a
  tree the drift half is skipped with a note. On 2026-09-25: 0 driver-side and 0 device-side
  changes.

## Depth

- [research/abi-baseline.md](../research/abi-baseline.md): the cited lines and the delta table.
- [caraxes/gsp-abi-615.md](../vfio/gsp-abi-615.md): the baseline document;
  [caraxes/gsp-abi-615-tables.md](../vfio/gsp-abi-615-tables.md): the rendered snapshot.
- [caraxes/guest-driver-versions.md](../vfio/guest-driver-versions.md): 595, 610 and 615
  guests against a 610 device.
