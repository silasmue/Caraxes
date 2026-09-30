<!-- SPDX-License-Identifier: MIT -->
# Leandro VirtIO

Leandro VirtIO (v1) forwards the NVIDIA driver's calls. The guest sees a VirtIO device and
runs Leandro's module `virtio_nvrm.ko` in place of `nvidia.ko`; the per-VM host backend
`vhost-user-nvrm` serves it over vhost-user and replays each RM call on the host driver.
[leandro.md](../leandro.md) compares it with Leandro VFIO.

## Setup and use

| File | Contents |
|---|---|
| [quickstart.md](quickstart.md) | Two Ubuntu desktops or two NixOS compute guests |
| [vm-preparation.md](vm-preparation.md) | Host installation, Cloud Hypervisor patches, networking and images |
| [ubuntu-desktop.md](ubuntu-desktop.md) | Guest modules, NVIDIA userspace, GNOME/X11 and Sunshine |
| [wayland.md](wayland.md) | GNOME/Wayland, KMS capture and verification |
| [input.md](input.md) | Moonlight input and optional upstream evdev forwarding |
| [guest-userspace.md](guest-userspace.md) | NVIDIA library installation and version requirements |
| [showcase.md](showcase.md) | Demonstration checks and what they establish |

## Design and development

| File | Contents |
|---|---|
| [architecture.md](architecture.md) | Request path, ownership, mappings, events and source map |
| [security.md](security.md) | Trust boundary, validation, remaining risks and required tests |
| [display.md](display.md) | Virtual display, capture, module parameters and limits |
| [abi-versions.md](abi-versions.md) | Supported driver ABIs, generation and version changes |
| [testing.md](testing.md) | Automated checks, hardware evidence and measurement rules |
| [open-questions.md](open-questions.md) | Numbered investigations, findings and unresolved issues |
| [future.md](future.md) | Planned correctness, compatibility and research work |
| [virtio-uapi.md](virtio-uapi.md) | Proposal for generalizing table-described driver forwarding |
| [llm.md](llm.md) | Maintenance notes |

## Measurements

Recorded runs, not guarantees for the current revision. Older snapshots are in
[history](../history/README.md#leandro-virtio-v1).

| Directory | Contents |
|---|---|
| [fence-14](measurements/fence-14/README.md) | Vulkan fence-wait polling fallback |
| [vram-68](measurements/vram-68/README.md) | Two-guest VRAM policy comparison |
| [vram-69](measurements/vram-69/README.md) | Four-guest VRAM policy tests |
| [vram-69b](measurements/vram-69b/README.md) | Admission limits and vGPU-mode behaviour |
| [vram-70](measurements/vram-70/README.md) | Workloads under different VRAM limits |
| [sottr-4q](measurements/sottr-4q/README.md) | Two concurrent Shadow of the Tomb Raider benchmarks |
