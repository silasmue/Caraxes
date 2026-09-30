<!-- SPDX-License-Identifier: MIT -->
# Documentation

Two architectures share this documentation: VirtIO Leandro (v1), API forwarding through a
guest module, and VFIO Leandro (v2, code name Caraxes), a synthetic NVIDIA GPU for stock
guest drivers. Start with [LEANDRO.md](LEANDRO.md) for both, the [v2 quickstart](caraxes/quickstart.md)
or the [v1 quickstart](QUICKSTART.md) to run a VM, and the [wiki](wiki/README.md) for one
concept at a time. Read [Security](SECURITY.md) and the [isolation rules](wiki/isolation-rules.md)
before interpreting isolation claims.

## Both architectures

| File | Contents |
|---|---|
| [LEANDRO.md](LEANDRO.md) | The overview: the two architectures, the driver, the GSP and its emulation, PCI and memory, submission and interrupts, execution, display, identity, performance, verification, the ranked weaknesses and the road to Christmas 2026 |
| [wiki/](wiki/README.md) | One page per concept, driver references by path and function, a glossary |
| [research/](research/README.md) | The evidence: one short note per topic, 615.71.09 lines and run folders |
| [NAMING.md](NAMING.md) | Component names, terminology and the guest-visible GPU name of both architectures |
| [Project README](../README.md) | What each version is, the status, requirements and build commands |
| [Development](../DEVELOPMENT.md) | Build, checks, Nix, tracing and troubleshooting |
| [Licenses](../LICENSES.md) | Licensing and third-party attribution |

## VFIO Leandro (v2)

| File | Contents |
|---|---|
| [caraxes/quickstart.md](caraxes/quickstart.md) | Build the device, prepare a guest, run a Linux, Windows, nova or nouveau VM, verify |
| [caraxes/architecture.md](caraxes/architecture.md) | The device process, its data paths, the source map, what is chip-bound and what is version-bound |
| [caraxes/KNOWN-ISSUES.md](caraxes/KNOWN-ISSUES.md) | Open defects and limits with their evidence, and the issues closed recently |
| [caraxes/decisions.md](caraxes/decisions.md) | The design decisions D1 to D20 |
| [caraxes/testing.md](caraxes/testing.md) | The v1 gates against the device, the verdict rules, the guest-side test material |
| [caraxes/benchmark.md](caraxes/benchmark.md) | v1, v2 and native on one card: harness, method, the tables |
| [caraxes/](caraxes/README.md) | The index of the v2 design documents and studies |

## VirtIO Leandro (v1): setup and use

| File | Contents |
|---|---|
| [QUICKSTART.md](QUICKSTART.md) | Two Ubuntu desktops or two NixOS compute guests |
| [VM-PREPARATION.md](VM-PREPARATION.md) | Host installation, Cloud Hypervisor patches, networking and images |
| [UBUNTU-DESKTOP.md](UBUNTU-DESKTOP.md) | Guest modules, NVIDIA userspace, GNOME/X11 and Sunshine |
| [WAYLAND.md](WAYLAND.md) | GNOME/Wayland, KMS capture and verification |
| [INPUT.md](INPUT.md) | Moonlight input and optional upstream evdev forwarding |
| [GUEST-USERSPACE.md](GUEST-USERSPACE.md) | NVIDIA library installation and version requirements |
| [SHOWCASE.md](SHOWCASE.md) | Demonstration checks and what they establish |

## VirtIO Leandro (v1): design and development

| File | Contents |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Request path, ownership, mappings, events and source map |
| [SECURITY.md](SECURITY.md) | Trust boundary, validation, remaining risks and required tests |
| [DISPLAY.md](DISPLAY.md) | Virtual display, capture, module parameters and limits |
| [abi-versions.md](abi-versions.md) | Supported driver ABIs, generation and version changes |
| [TESTING.md](TESTING.md) | Automated checks, hardware evidence and measurement rules |
| [OPEN-QUESTIONS.md](OPEN-QUESTIONS.md) | Numbered investigations, findings and unresolved issues |
| [FUTURE.md](FUTURE.md) | Planned correctness, compatibility and research work |
| [VIRTIO-UAPI.md](VIRTIO-UAPI.md) | Proposal for generalizing table-described driver forwarding |
| [llm.md](llm.md) | Maintenance notes; filename retained for existing links |

## Crate and module READMEs

Each README describes the component and its source files.

| README | Component |
|---|---|
| [crates/vfio-user-nvgpu](../crates/vfio-user-nvgpu/) | v2: the device (no separate README; [caraxes/architecture.md](caraxes/architecture.md) is its source map) |
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
commands are in [abi-versions.md](abi-versions.md).

## Records and history

Recorded runs, not guarantees for the current revision.

| File or directory | Contents |
|---|---|
| [history](history/README.md) | Dated snapshots of both versions: the v2 working record, briefs, the experiment ladder, superseded surveys |
| [measurements/fence-14](measurements/fence-14/README.md) | v1: Vulkan fence-wait polling fallback |
| [measurements/vram-68](measurements/vram-68/README.md) | v1: two-guest VRAM policy comparison |
| [measurements/vram-69](measurements/vram-69/README.md) | v1: four-guest VRAM policy tests |
| [measurements/vram-69b](measurements/vram-69b/README.md) | v1: admission limits and vGPU-mode behaviour |
| [measurements/vram-70](measurements/vram-70/README.md) | v1: workloads under different VRAM limits |
| [measurements/sottr-4q](measurements/sottr-4q/README.md) | v1: two concurrent Shadow of the Tomb Raider benchmarks |
| `../caraxes/runs/` | v2: every run record, one folder per experiment and date |
