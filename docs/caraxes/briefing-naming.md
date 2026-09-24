<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Briefing (operator, 2026-09-23): synthetic GPU naming and configuration model

Low-priority refactor handed over by the operator on 2026-09-23 evening. Kept verbatim.

Refactor the synthetic GPU naming/configuration model so that platform, transport,
personality, profile, and backend are cleanly separated.

Target naming format: `Leandro <Transport> <Personality>-<Profile>`

Examples: `Leandro VFIO RTX 2070-4G`, `Leandro VirtIO RTX 2070-4G`, `Leandro VFIO A100-16G`.

Model the fields independently: platform = Leandro; transport = VFIO | VirtIO;
personality = RTX 2070 | A100 | ...; profile = 4G | 8G | ...; backend = OpenRM | nova-core | ...

Rules: backend is an implementation detail and must not appear in the guest-visible GPU name.
VFIO means the Caraxes/vfio-user synthetic PCI GPU path. VirtIO means the existing Leandro v1
/ vhost-user-style path. Do not hardcode complete GPU names throughout the codebase; generate
them from the structured configuration. Preserve current behavior and compatibility wherever
possible. Avoid changing protocol semantics, RM behavior, GPU execution, or unrelated code.
Keep existing user configuration working if practical; add compatibility parsing where cheap.
Search the entire repository for hardcoded "Leandro RTX...", "RTX 2070-4G", profile names,
transport assumptions, and duplicate naming logic. Consolidate naming/profile generation into
one obvious module/API. Add small tests for name generation and parsing. Update relevant
docs/examples/configs. Make incremental commits/checkpoints and do not combine unrelated
cleanup into this refactor.

Long-term goal: Meisterstack should eventually be able to supply something equivalent to
`VirtualGpuSpec { transport, personality, profile, backend }` and Leandro/Caraxes derives the
guest-visible device identity from that spec.

First inspect the existing architecture and produce a short refactoring plan. Then implement
it incrementally without disturbing the currently working GPU paths.

Operator's addition: do not forget the existing `4Q` and `1333MiB` profiles (Leandro v1).
Operator's addition (20:05): the brief covers the v1 project too, i.e. `crates/vhost-user-nvrm`
(grid.rs naming and profiles), `guest-module/virtio_nvrm`, `scripts/`, the v1 docs and configs, not
only Caraxes. Note for the plan: the v1 gates in `../Leandro-Test` compare the guest-visible name
(`Leandro RTX 2070-4G`); a new default with the transport word changes that string, so the plan
must say how compatibility is kept (default format unchanged until the gates are updated, or a
gate mask), and `../Leandro-Test` itself is never modified by us.

## Plan (coordinator, 2026-09-23)

Run as an Opus agent in an isolated git worktree with incremental commits there, merged into
main as soon as it lands (one merge per landing, not a big bang). Start after the E5b part 2
hand-back, because `crates/vfio-user-nvgpu` is uncommitted and being edited until then: a
checkpoint commit of the working tree comes first so the worktree carries both devices.

## Refactoring plan (agent, 2026-09-23)

What exists today. Two copies of one naming rule: `guest_card_name` in
`crates/vhost-user-nvrm/src/vram.rs` (v1, rewrites the host's `GET_NAME_STRING` reply:
strip `NVIDIA `/`GeForce `, prefix `Leandro `, append `-<n>G` or `-<n>M` from the
profile's `fb_length`, drop the suffix and then the whole name when it would not fit the
64-byte field) and `StaticIdentity::guest_name` in `crates/vfio-user-nvgpu/src/rpc.rs`
(Caraxes, same shape, board name from `boards/*.toml`, no length bound). The host-side
catalogue names in `crates/nvrm-abi/src/vgpu.rs` (`RTX2070-4Q`, `RTX2070-130M`) are
launcher labels, never guest-visible. The only hard-coded complete name outside tests
and historical records is the gate expectation in `tools/caraxes-gate.sh`.
`nvml::Profile` in the Caraxes crate carries no name, so it stays as it is. There is no
`scripts/` directory in this repository; the v1 rig scripts live in `../Leandro-Test`.
`guest-module/virtio_nvrm` has no GPU name site (its only name, `Leandro vDisp`, is the
EDID monitor name of the virtual display, not the GPU).

The single source. A new module `nvrm_abi::naming` (file
`crates/nvrm-abi/src/naming.rs`). `nvrm-abi` is the right home: both devices already
depend on it, it holds the related `vgpu` catalogue and the name-field limits
(`mediate::name_max`), and it has no device or transport dependency. It provides:

- `VirtualGpuSpec { transport, personality, profile, backend }` with
  `Transport::{Vfio, Virtio}` (words `VFIO`, `VirtIO`), `Personality` (the board name
  the guest sees, `RTX 2070`; `Personality::from_driver_name` applies v1's prefix
  stripping to the host driver's name), `Option<Profile>` (the guest framebuffer size)
  and `Backend::{OpenRm, NovaCore, Other}`.
- `VirtualGpuSpec::guest_name(format)` and `guest_name_within(format, max)`, the second
  with v1's fallbacks for the 64-byte field. The backend is never read by either.
- `NameFormat::{Legacy, Transport}`: `Legacy` gives `Leandro <Personality>-<Profile>`,
  `Transport` gives `Leandro <Transport> <Personality>-<Profile>`.
- Parsing: `Transport`, `Backend`, `NameFormat` and `Profile` implement `FromStr` with
  the existing spellings as aliases, and `parse_name` reads a guest-visible name of either
  format back into its fields (for gates and tests).

Profiles. The profile field is always the guest-visible framebuffer size, as today:
`<n>G` for whole GiB, `<n>M` otherwise. `Profile::from_str` accepts `4G`, `4GiB`,
`1333M`, `1333MiB`, a bare MiB count and a board-prefixed form (`RTX2070-130M`), through
the existing `vgpu::parse_mib`. The v1 `4Q` style is a catalogue type (what the VM costs
the card), not a guest size: `Profile::from_str("4Q")` is refused with a pointer to
`vgpu::Catalogue::resolve`, and `Profile::from_catalogue(&vgpu::Profile)` maps a resolved
type to its `fb_length`, so `RTX2070-4Q` with 3072 MiB guest FB is named `-3G`, exactly as
v1 names it today. A `1333MiB` size (`LEA_VRAM_LIMIT_MIB=1333`, or `vgpuprofile 1333MiB`)
maps to `Profile { fb_mib: 1333 }` and is named `-1333M`. Nothing in the catalogue
arithmetic or the v1 policy selection (`decide`, `LEA_VGPU_*`, `LEA_VRAM_*`) changes.

Compatibility decision. The v1 gates in `../Leandro-Test` compare the guest-visible
string `Leandro RTX 2070-4G`, and the Caraxes gate and docs use the same string. The
default format is therefore `Legacy`, which emits exactly today's names on both devices.
The transport word is opt-in: `LEA_GPU_NAME_FORMAT=transport` for either device (the
v1 backend reads it once at start), `--name-format transport` on `vfio-user-nvgpu`, and
`CARAXES_NAME_FORMAT=transport` in `tools/caraxes-vm.sh`. Switching the default is a
one-line change in `NameFormat::default` once the gates accept the new names.
`../Leandro-Test` is not modified. The Caraxes GID stays derived from the legacy-format
name so that opting into the new format does not change the guest's UUID.

Steps, one commit each: (1) this plan; (2) `nvrm_abi::naming` with tests; (3) v1
`guest_card_name` generated from a `VirtualGpuSpec` (transport VirtIO), plus
`LEA_GPU_NAME_FORMAT`; (4) Caraxes `StaticIdentity` generated from a spec (transport
VFIO), `--name-format`, `--profile <size>` as a parsed alternative to `--fb-mib`, and
`--print-name`; (5) launcher and gate: `CARAXES_NAME_FORMAT`, `caraxes-vm.sh name`, the
gate's expected name generated by the device instead of hard-coded; (6) docs.
Left alone on purpose: historical records (`docs/measurements/**`, `docs/history/**`,
recorded outputs in `docs/caraxes/*.md` and the recorded criteria in
`matrix/guest-610.57.04.json`), the catalogue's host-side type names, provenance label
strings, and every protocol, RM or execution path.

## Status (2026-09-24): default switched

The operator allowed changes to `../Leandro-Test` for this step (2026-09-24), which lifts
the "never modified by us" condition above. Done in this order:

1. `../Leandro-Test` (commit `3edea73` on its branch `refactor-thesis`): one helper,
   `lea_mediated_name_re` in `scripts/lib/common.sh`, builds an extended regex for either
   format from the host card's name (`Leandro( VirtIO| VFIO)? RTX 2070`). The gpu gate's
   `smi` stage masks and checks with it, `showcase.sh`'s nvidia-smi section accepts it, and
   its README has a short "Guest-visible GPU name" section. Those were the only places
   there that compared the name; the other mentions are comments.
2. `NameFormat::default()` is `Transport`. The legacy name stays an explicit opt-in
   (`LEA_GPU_NAME_FORMAT=legacy`, `--name-format legacy`, `CARAXES_NAME_FORMAT=legacy`),
   `parse_name` reads both formats, an empty format string means the default, and an
   unknown `LEA_GPU_NAME_FORMAT` in v1 falls back to the default. The Caraxes GID is still
   seeded from the legacy-format name; the static-info test pins the recorded UUID
   `GPU-9bd7b93d-3d23-c074-6704-984ef33ab7a7`.
3. Tests, help texts, `identity-profile.md`, `nvml.md`, `crates/vhost-user-nvrm/README.md`
   and the P17 row in `README.md` follow the new default. Historical records keep the names
   they recorded.
