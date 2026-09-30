<!-- SPDX-License-Identifier: MIT -->
# Roadmap

The lines of work carry dragon code names ([naming](naming.md)). The previous roadmap, as of
2026-09-27, is in [history](history/caraxes-roadmap-2026-09-27.md).

## Caraxes: NVIDIA

- **Leandro VFIO (v2), today's work.** A synthetic TU106 over vfio-user for stock drivers:
  Linux 615, Windows 11, nova-core and nouveau with NVK on one device binary. State in the
  [README](../README.md#status-2026-09-30), defects in [known issues](vfio/known-issues.md).
- **Leandro VirtIO (v1), the NVIDIA VirtIO path.** API forwarding over vhost-user, unchanged
  since 2026-09-24; the reference for allocation-heavy work and the other side of the
  [v1-against-v2 benchmark](vfio/benchmark.md).
- **Goal.** The ranked weaknesses of [leandro.md](leandro.md#122-weaknesses-ranked-by-what-they-cost-the-goals)
  removed by Christmas 2026 ([the road](leandro.md#123-the-road-to-christmas-2026)).

## Meleys: AMD

- Planned: a VirtIO and a VFIO variant. Nothing exists yet; `virtio-kfd` is a name proposal.
- Reference for the VirtIO variant: virtio-gpu DRM native context, upstream for amdgpu.

## Dreamfyre: Intel

- Planned: a VirtIO and a VFIO variant. Nothing exists yet; `virtio-xe` is a name proposal.
- Reference for the VirtIO variant: virtio-gpu DRM native context, in review for Intel.

## Sunfyre: the common protocol

- Planned: one GPU virtualisation protocol that every variant speaks.
- Groundwork: the [table-described forwarding proposal](virtio/virtio-uapi.md) from
  Leandro VirtIO. Shared code is extracted only after a second backend shows common
  requirements.

## Next

Next week, in this order:

1. **Slim both projects down** to fewer features.
2. **Rename the code** together with it, a pure rename with the same knobs and defaults:
   - `caraxes/` becomes `vfio/`; every path in scripts, tools, crates, CI, checks and docs
     follows, including the doc paths code still names from before the docs split;
   - `CARAXES_*` becomes `LVFIO_*`, v1-only variables become `LVIRTIO_*`, `LEA_*` stays where
     both versions or Leandro-Test share it (for example `LEA_BASE_IMAGE`, `LEA_VM_DIR`);
   - the device prints one warning line for each old variable still set, naming its
     replacement, and does not honour it; tests cover both;
   - "Caraxes" in comments, log messages, help and script texts becomes "Leandro VFIO";
   - unchanged: the names listed as kept in [naming](naming.md#prefixes-and-directories-today).
3. **Then the open items** of the [known issues](vfio/known-issues.md#open), in their order
   there.
