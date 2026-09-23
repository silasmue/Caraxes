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
