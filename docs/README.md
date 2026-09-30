<!-- SPDX-License-Identifier: MIT -->
# Documentation

Two versions live in this repository, each named after what the guest sees:

- **Leandro VirtIO (v1):** the guest sees a VirtIO device and runs Leandro's module
  `virtio_nvrm`; the host backend `vhost-user-nvrm` serves it over vhost-user and forwards
  the RM API. Documents in [virtio/](virtio/README.md).
- **Leandro VFIO (v2):** the guest sees a PCI GPU, a synthetic TU106 (RTX 2070), and runs its
  stock driver; the host process `vfio-user-nvgpu` serves it over vfio-user with GSP
  emulation. Documents in [vfio/](vfio/README.md).

Read [security](virtio/security.md) and the [isolation rules](wiki/isolation-rules.md) before
interpreting isolation claims.

## Shared

| File | Contents |
|---|---|
| [leandro.md](leandro.md) | Both versions in one document: the driver, the GSP and its emulation, memory, submission, execution, display, identity, performance, verification, the ranked weaknesses and the road to Christmas 2026 |
| [performance.md](performance.md) | Measured performance of both versions against native |
| [roadmap.md](roadmap.md) | What exists and what is planned, by code name; the next step |
| [naming.md](naming.md) | Official names, code names, prefixes, directories and components |
| [wiki/](wiki/README.md) | One page per concept, driver references by path and function, a glossary |
| [research/](research/README.md) | The evidence: one short note per topic, 615.71.09 lines and run folders |
| [history/](history/README.md) | Dated snapshots of both versions |
| [Project README](../README.md) | Status, requirements and build commands |
| [Development](../DEVELOPMENT.md) | Build, checks, Nix, tracing and troubleshooting |
| [Licenses](../LICENSES.md) | Licensing and third-party attribution |

## Per version

- Leandro VirtIO: [quickstart](virtio/quickstart.md), [architecture](virtio/architecture.md),
  [testing](virtio/testing.md), [open questions](virtio/open-questions.md); everything in
  [virtio/](virtio/README.md), measurements included.
- Leandro VFIO: [quickstart](vfio/quickstart.md), [architecture](vfio/architecture.md),
  [known issues](vfio/known-issues.md), [decisions](vfio/decisions.md); everything in
  [vfio/](vfio/README.md). Run records: `../caraxes/runs/`, one folder per experiment and date.

## Crate and module READMEs

| README | Component |
|---|---|
| [crates/vfio-user-nvgpu](../crates/vfio-user-nvgpu/) | v2: the device (no separate README; [vfio/architecture.md](vfio/architecture.md) is its source map) |
| [crates/dp-sink](../crates/dp-sink/README.md) | v2: DisplayPort sink model |
| [guest-module/caraxes_dma_range](../guest-module/caraxes_dma_range/README.md) | v2: the former guest DMA-range module (decision D13) |
| [guest-module/virtio_nvrm](../guest-module/virtio_nvrm/README.md) | v1: guest RM/UVM forwarding, mappings, events and display |
| [guest-module/nvrm_nodes](../guest-module/nvrm_nodes/README.md) | v1: guest NVIDIA proc data, optional placeholder nodes and pinning diagnostics |
| [crates/vhost-user-nvrm](../crates/vhost-user-nvrm/README.md) | v1: per-VM host backend, sessions and resource ownership |
| [crates/nvrm-wire](../crates/nvrm-wire/README.md) | v1: request/reply schema and generated C header |
| [crates/nvrm-sys](../crates/nvrm-sys/README.md) | shared: generated NVIDIA bindings and layout metadata |
| [crates/nvrm-abi](../crates/nvrm-abi/README.md) | shared: ioctl descriptors, embedded pointers, FDs, mediation layouts, the naming model |
| [crates/nvrm-client](../crates/nvrm-client/README.md) | shared: RM ownership wrappers and diagnostic tools |
| [crates/nvrm-trace](../crates/nvrm-trace/README.md) | shared: `LD_PRELOAD` call tracing |
| [patches](../patches/README.md) | Cloud Hypervisor changes, validation and upstreaming |
| [matrix](../matrix/README.md) | v1: NVIDIA userspace API coverage and recorded probes |
| [tools/windows](../tools/windows/README.md) | v2: the Windows test pack |

[crates/xtask](../crates/xtask/src/main.rs) has no separate README; its ABI generation
commands are in [virtio/abi-versions.md](virtio/abi-versions.md).
