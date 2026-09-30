<!-- SPDX-License-Identifier: MIT -->
# Naming

## Official names

Project: **Leandro**. Each version is named after what the guest sees; v1 and v2 are short
forms.

| Name | The guest sees | Host process | Transport |
|---|---|---|---|
| Leandro VirtIO (v1) | a VirtIO device, driven by Leandro's module `virtio_nvrm.ko` | `vhost-user-nvrm`, forwards the RM API | vhost-user |
| Leandro VFIO (v2) | a PCI GPU, a synthetic TU106 (RTX 2070), driven by its stock driver | `vfio-user-nvgpu`, emulates the GSP | vfio-user |

- The GPU name inside a guest is `Leandro <Transport> <Personality>-<Profile>`, for example
  `Leandro VFIO RTX 2070-4G` ([identity profile](vfio/identity-profile.md)).
- Leandro is not affiliated with NVIDIA.

## Code names

Dragons name the lines of work. They appear only here and in the [roadmap](roadmap.md).

| Code name | Line of work |
|---|---|
| Caraxes | NVIDIA: Leandro VFIO today; the NVIDIA VirtIO path is Leandro VirtIO |
| Meleys | AMD, a VirtIO and a VFIO variant |
| Dreamfyre | Intel, a VirtIO and a VFIO variant |
| Sunfyre | the common GPU virtualisation protocol shared by all |

## Prefixes and directories today

| Scope | Environment | Directories |
|---|---|---|
| both, and Leandro-Test | `LEA_*` | `docs/`, `crates/nvrm-{sys,abi,client,trace}`, `crates/xtask`, `patches/`, `tools/` |
| Leandro VirtIO | `LEA_*` | `docs/virtio/`, `crates/vhost-user-nvrm`, `crates/nvrm-wire`, `guest-module/virtio_nvrm`, `guest-module/nvrm_nodes`, `matrix/` |
| Leandro VFIO | `CARAXES_*` | `docs/vfio/`, `crates/vfio-user-nvgpu`, `crates/dp-sink`, `caraxes/`, `tools/caraxes-*.sh`, `tools/windows/` |

- `caraxes/`, `CARAXES_*` and `tools/caraxes-*.sh` still carry the code name; the planned
  rename is in the [roadmap](roadmap.md#next).
- These keep their names because data or other systems depend on them:
  `/mnt/vmstore/caraxes` (qcow2 backing chains embed it), the host tap devices
  `caraxes6`/`caraxes7`, names inside guest images (the `caraxes-dmem-cap` unit,
  `/opt/caraxes`), the checkout directory `~/git/Caraxes` and the git history.

## Components

| Role | Name | Version |
|---|---|---|
| Protocol and device | `virtio-nvrm` | v1 |
| Guest frontend | `virtio_nvrm.ko` | v1 |
| Guest proc and pinning helper | `nvrm_nodes.ko` | v1 |
| Host backend | `vhost-user-nvrm` | v1 |
| Wire schema | `nvrm-wire` | v1 |
| Input backend | upstream `vhost-device-input` | v1 |
| Synthetic GPU | `vfio-user-nvgpu` | v2 |
| DisplayPort sink model | `dp-sink` | v2 |
| Driver bindings | `nvrm-sys` | both |
| Ioctl descriptions and translation, the naming model | `nvrm-abi` | both |
| Diagnostic RM ownership | `nvrm-client` | both |
| Userspace tracer | `nvrm-trace` | both |

- `nvrm` denotes NVIDIA Resource Manager; UVM uses the same transport.
- Component names describe their role and interface.
- The old LD_PRELOAD forwarding shim is retired; `nvrm-trace` observes calls only.
- Avoid names already used elsewhere: `nvshim`, `nvproxy`, `nvhost`, `nvvm`.
- New vendor backends use separate ABI namespaces. `virtio-kfd` and `virtio-xe` are
  proposals, not implemented devices.
- Extract shared libraries only after a second backend demonstrates common requirements.
