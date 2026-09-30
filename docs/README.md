<!-- SPDX-License-Identifier: MIT -->
# Documentation

Two architectures share this documentation: VirtIO Leandro (v1), API forwarding through a
guest module, and VFIO Leandro (v2, code name Caraxes), a synthetic NVIDIA GPU for stock
guest drivers. Start with [leandro.md](leandro.md) for both, the [v2 quickstart](vfio/quickstart.md)
or the [v1 quickstart](virtio/quickstart.md) to run a VM, and the [wiki](wiki/README.md) for one
concept at a time. Read [Security](virtio/security.md) and the [isolation rules](wiki/isolation-rules.md)
before interpreting isolation claims.

## Both architectures

| File | Contents |
|---|---|
| [leandro.md](leandro.md) | The overview: the two architectures, the driver, the GSP and its emulation, PCI and memory, submission and interrupts, execution, display, identity, performance, verification, the ranked weaknesses and the road to Christmas 2026 |
| [wiki/](wiki/README.md) | One page per concept, driver references by path and function, a glossary |
| [research/](research/README.md) | The evidence: one short note per topic, 615.71.09 lines and run folders |
| [naming.md](naming.md) | Component names, terminology and the guest-visible GPU name of both architectures |
| [Project README](../README.md) | What each version is, the status, requirements and build commands |
| [Development](../DEVELOPMENT.md) | Build, checks, Nix, tracing and troubleshooting |
| [Licenses](../LICENSES.md) | Licensing and third-party attribution |

## VFIO Leandro (v2)

| File | Contents |
|---|---|
| [vfio/quickstart.md](vfio/quickstart.md) | Build the device, prepare a guest, run a Linux, Windows, nova or nouveau VM, verify |
| [vfio/architecture.md](vfio/architecture.md) | The device process, its data paths, the source map, what is chip-bound and what is version-bound |
| [vfio/known-issues.md](vfio/known-issues.md) | Open defects and limits with their evidence, and the issues closed recently |
| [vfio/decisions.md](vfio/decisions.md) | The design decisions D1 to D20 |
| [vfio/testing.md](vfio/testing.md) | The v1 gates against the device, the verdict rules, the guest-side test material |
| [vfio/benchmark.md](vfio/benchmark.md) | v1, v2 and native on one card: harness, method, the tables |
| [caraxes/](vfio/README.md) | The index of the v2 design documents and studies |

## VirtIO Leandro (v1): setup and use

| File | Contents |
|---|---|
| [virtio/quickstart.md](virtio/quickstart.md) | Two Ubuntu desktops or two NixOS compute guests |
| [virtio/vm-preparation.md](virtio/vm-preparation.md) | Host installation, Cloud Hypervisor patches, networking and images |
| [virtio/ubuntu-desktop.md](virtio/ubuntu-desktop.md) | Guest modules, NVIDIA userspace, GNOME/X11 and Sunshine |
| [virtio/wayland.md](virtio/wayland.md) | GNOME/Wayland, KMS capture and verification |
| [virtio/input.md](virtio/input.md) | Moonlight input and optional upstream evdev forwarding |
| [virtio/guest-userspace.md](virtio/guest-userspace.md) | NVIDIA library installation and version requirements |
| [virtio/showcase.md](virtio/showcase.md) | Demonstration checks and what they establish |

## VirtIO Leandro (v1): design and development

| File | Contents |
|---|---|
| [virtio/architecture.md](virtio/architecture.md) | Request path, ownership, mappings, events and source map |
| [virtio/security.md](virtio/security.md) | Trust boundary, validation, remaining risks and required tests |
| [virtio/display.md](virtio/display.md) | Virtual display, capture, module parameters and limits |
| [virtio/abi-versions.md](virtio/abi-versions.md) | Supported driver ABIs, generation and version changes |
| [virtio/testing.md](virtio/testing.md) | Automated checks, hardware evidence and measurement rules |
| [virtio/open-questions.md](virtio/open-questions.md) | Numbered investigations, findings and unresolved issues |
| [virtio/future.md](virtio/future.md) | Planned correctness, compatibility and research work |
| [virtio/virtio-uapi.md](virtio/virtio-uapi.md) | Proposal for generalizing table-described driver forwarding |
| [virtio/llm.md](virtio/llm.md) | Maintenance notes; filename retained for existing links |

## Crate and module READMEs

Each README describes the component and its source files.

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

## Records and history

Recorded runs, not guarantees for the current revision.

| File or directory | Contents |
|---|---|
| [history](history/README.md) | Dated snapshots of both versions: the v2 working record, briefs, the experiment ladder, superseded surveys |
| [measurements/fence-14](virtio/measurements/fence-14/README.md) | v1: Vulkan fence-wait polling fallback |
| [measurements/vram-68](virtio/measurements/vram-68/README.md) | v1: two-guest VRAM policy comparison |
| [measurements/vram-69](virtio/measurements/vram-69/README.md) | v1: four-guest VRAM policy tests |
| [measurements/vram-69b](virtio/measurements/vram-69b/README.md) | v1: admission limits and vGPU-mode behaviour |
| [measurements/vram-70](virtio/measurements/vram-70/README.md) | v1: workloads under different VRAM limits |
| [measurements/sottr-4q](virtio/measurements/sottr-4q/README.md) | v1: two concurrent Shadow of the Tomb Raider benchmarks |
| `../caraxes/runs/` | v2: every run record, one folder per experiment and date |
