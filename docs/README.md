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
- Leandro VFIO: [guest support](vfio/guest-support.md), [quickstart](vfio/quickstart.md), [architecture](vfio/architecture.md),
  [known issues](vfio/known-issues.md), [decisions](vfio/decisions.md); everything in
  [vfio/](vfio/README.md). Run records: `../caraxes/runs/`, one folder per experiment and date.

## Crates and guest modules

Each directory has a short README; the detail sits beside it in `reference.md`.

| Crate or module | Version | Role |
|---|---|---|
| [vfio-user-nvgpu](../crates/vfio-user-nvgpu/README.md) | VFIO | The device: a synthetic TU106 over vfio-user, GSP emulation, shadow page tables, mirror channels |
| [dp-sink](../crates/dp-sink/README.md) | VFIO | The DisplayPort monitor model the device's display drives |
| [caraxes_dma_range](../guest-module/caraxes_dma_range/README.md) | VFIO | The former guest DMA-range module, no longer loaded (D13) |
| [vhost-user-nvrm](../crates/vhost-user-nvrm/README.md) | VirtIO | The host backend: validates guest RM calls and replays them on the host driver |
| [virtio_nvrm](../guest-module/virtio_nvrm/README.md) | VirtIO | The guest module: owns `/dev/nvidia*` and forwards the calls over vhost-user |
| [nvrm_nodes](../guest-module/nvrm_nodes/README.md) | VirtIO | The guest's `/proc/driver/nvidia` and a pinning self-test |
| [nvrm-wire](../crates/nvrm-wire/README.md) | VirtIO | The request and reply schema between guest module and backend |
| [nvrm-sys](../crates/nvrm-sys/README.md) | shared | Generated NVIDIA bindings, one module per driver version |
| [nvrm-abi](../crates/nvrm-abi/README.md) | shared | The RM ioctl ABI, the translation rules and the naming model |
| [nvrm-client](../crates/nvrm-client/README.md) | shared | The host RM client, and diagnostic tools |
| [nvrm-trace](../crates/nvrm-trace/README.md) | shared | An `LD_PRELOAD` tracer for the NVIDIA ioctl surface |
| [xtask](../crates/xtask/README.md) | shared | `cargo xtask abi`: regenerates and checks the bindings |

Other READMEs: [patches](../patches/README.md) (Cloud Hypervisor changes for both),
[matrix](../matrix/README.md) (Leandro VirtIO's API coverage),
[tools/windows](../tools/windows/README.md) (Leandro VFIO's Windows test pack).
