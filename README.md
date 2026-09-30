<!-- SPDX-License-Identifier: MIT -->
# Leandro

GPU sharing for virtual machines on one NVIDIA card that the host keeps using. Two
architectures live in this repository:

- **VirtIO Leandro (v1)**: API forwarding. A Leandro kernel module in the guest,
  `virtio_nvrm.ko`, forwards the NVIDIA driver calls over vhost-user to a per-VM host
  backend, `vhost-user-nvrm`, which replays them on the host driver.
- **VFIO Leandro (v2, code name Caraxes)**: a synthetic NVIDIA TU106, reported as an RTX
  2070, served to the guest over vfio-user by a per-VM host process, `vfio-user-nvgpu`. The
  guest runs its stock driver. The device plays the GSP firmware, models the PCI BARs,
  shadows the guest's page tables and runs the guest's channels on host channels as a
  client of the host driver.

![The two architectures side by side](docs/img/leandro-two-architectures.svg)

[docs/LEANDRO.md](docs/LEANDRO.md) describes both in one document; the
[documentation index](docs/README.md) lists everything else.

**Use trusted guests.** Both architectures confine a guest by validation in the host
process; v2 adds a fixed VRAM slice, registered guest RAM and validated shadow page tables.
Every isolation run used a stock driver; no adversarial guest has been tested
([SECURITY.md](docs/SECURITY.md), [isolation rules](docs/wiki/isolation-rules.md)).

## The two architectures

| | VirtIO Leandro (v1) | VFIO Leandro (v2) |
|---|---|---|
| Guest driver | Leandro modules `virtio_nvrm.ko` and `nvrm_nodes.ko` in place of `nvidia.ko`; NVIDIA user space staged to match the host driver | the guest's own stock driver: Linux 615.71.09 open modules, Windows 617.14, nova-core, nouveau with NVK |
| Host process | `vhost-user-nvrm`, one per VM | `vfio-user-nvgpu`, one per VM |
| What crosses the boundary | about 290 ioctl shapes; every RM call is a round trip; submissions through a shared window | PCI config space, BAR0/1/2 accesses (trapped or mapped), the GSP message queues in guest RAM; RM calls stay in the guest kernel |
| Isolation | validated ioctl envelopes, pointers and FDs; a VRAM ledger; no partition of the card | a fixed VRAM slice, registered guest RAM only, shadow page tables that admit validated leaves, no guest compression state |
| Display | NVIDIA's displayless class with a generated EDID and guest timers | an emulated DisplayPort monitor with vblank and flips on the device's clock |
| Hypervisor | Cloud Hypervisor v53.0 with patches 0001 to 0003 | Cloud Hypervisor v53.0 with patches 0005 to 0008 |
| Shared code | the RM translation tables and bindings (`nvrm-abi`, `nvrm-sys`), the host RM client (`nvrm-client`), the tracer, the naming model, the test gates and their verdict rules | |

## Status (2026-09-30)

- v2, Linux guest (Ubuntu 24.04, stock 615.71.09): CUDA, Vulkan, GL, a GNOME desktop on
  Wayland, NVENC and NVDEC bit-exact against software decode (H.264, HEVC, VP9), Sunshine
  streaming with NVENC at 60 fps and 5.9 ms host latency. Gates: `nvml` pass, `cuda` 10 of
  12 (managed memory and cuda-gdb open), `display` 11 of 11.
- v2, Windows 11 guest (stock 617.14) under Cloud Hypervisor: a stable desktop, the D3D11
  stages 12 of 12, Unigine Heaven 1080p High 152 to 158 fps, Superposition 1080p Medium up
  to 12247 points, Sunshine with NVENC or x264, NVENC and NVDEC bit-exact. The crashes
  beside a second GPU client are fixed (channel migration, 11 of 11 runs).
- v2, open drivers: nova-core probes to completion in pure emulation; nouveau with GSP-RM
  570.144 and Mesa NVK renders on the device (Weston, vkcube, glmark2, vkmark).
- v2 performance: GPU-bound work runs at the card's speed. Setup paths cost more than
  native (cuCtxCreate 240 to 256 ms beside another VM against 130 ms native). 3D workloads
  still trap 3200 to 8600 times per second, 14 to 24 % of a vCPU; the plan to remove them
  is in the [trap census](docs/research/trap-census-2026-09-30.md).
- v2 open defects: [known issues](docs/caraxes/KNOWN-ISSUES.md) (DX12 3DMark, Shadow of the
  Tomb Raider on Linux, the NVENC stream latency on Windows, host VRAM with two VMs).
- One device binary serves every guest ABI; one configuration with five knobs (VRAM size,
  display, compression, doorbell poll, BAR1 pool); the device logs its effective
  configuration at start ([device configuration](docs/wiki/device-configuration.md)).
- v1: unchanged since 2026-09-24. Its gates on the 615.71.09 image: `smi` pass, `cuda` 14 of
  14. It stays the reference path and the other side of the v1-against-v2 benchmark.

## Start here

- [Documentation index](docs/README.md): guides, design notes, the wiki, the research notes.
- [LEANDRO.md](docs/LEANDRO.md): both architectures, the driver, the GSP, memory, submission,
  execution, display, identity, performance, verification, the ranked weaknesses.
- v2: [quickstart](docs/caraxes/quickstart.md), [architecture](docs/caraxes/architecture.md),
  [known issues](docs/caraxes/KNOWN-ISSUES.md), [decisions](docs/caraxes/decisions.md).
- v1: [quickstart](docs/QUICKSTART.md), [architecture](docs/ARCHITECTURE.md),
  [testing](docs/TESTING.md), [open questions](docs/OPEN-QUESTIONS.md).
- [Wiki](docs/wiki/README.md): one page per concept; [research notes](docs/research/README.md):
  the evidence.
- [Development](DEVELOPMENT.md): build, checks, tracing and troubleshooting.

## Requirements

- Linux x86-64 host with KVM and the NVIDIA open kernel modules with GSP firmware active. The
  recorded host runs 615.71.09 on an RTX 2070; v2 selects the host ABI at run time and
  serves the guest's driver version independently of it. v1's guest user space must match a
  compiled host ABI entry ([abi-versions.md](docs/abi-versions.md)).
- Cloud Hypervisor [v53.0](CH_VERSION) with the [patches](patches/README.md): 0001 to 0003
  (vhost-user shared memory) for v1; 0005 to 0008 (vfio-user region I/O fds and read-only
  mmap areas) for v2; 0004 (ACPI `_DMA`) is optional evidence.
- The pinned [Rust toolchain](rust-toolchain.toml); Python 3, a C compiler and clang for the
  tools and the ABI checks. [DRIVER_VERSION](DRIVER_VERSION) names the build default of the
  shared crates.
- Guests: v2 an Ubuntu 24.04 image with the distribution's NVIDIA packages, a Windows 11
  image with the 617 driver, or the nova and nouveau kernels of the run folders; v1 the
  baked image with the Leandro modules ([VM-PREPARATION.md](docs/VM-PREPARATION.md)).
- Builds and software checks need no GPU and no Leandro-Test checkout. The hardware gates
  read the private Leandro-Test repository.

## Build and check

```sh
./tools/build.sh all                         # v1: vendor sources, patched Cloud Hypervisor, workspace
cargo build --release -p vfio-user-nvgpu     # v2: the device
./tools/check.sh                             # formatting, clippy, tests, licence headers, ABI drift
CARAXES_CHECK_EMU_BOOT=1 ./tools/check.sh    # also boots the synthetic GPU without a GPU
```

- v1 VMs start with the commands of the [v1 quickstart](docs/QUICKSTART.md).
- v2 VMs start with `tools/caraxes-vm.sh up` and are checked with `tools/caraxes-gate.sh`
  ([v2 quickstart](docs/caraxes/quickstart.md)). Give each VM its own device process,
  socket, VRAM slot, writable disk and network identity.
- Every change to v2 passes the four-guest gate before a merge: Linux 615 kick-smoke,
  nova-core, nouveau with NVK, and a Windows session
  ([verification methods](docs/wiki/verification-methods.md)).

## Repository layout

| Path | Contents |
|---|---|
| `crates/vfio-user-nvgpu` | v2: the device (PCI and BAR0 model, GSP boot and message queue, RPC dispatcher, memory model, shadow page tables, mirror channels, display); `chips/` and `boards/` data |
| `crates/dp-sink` | v2: the DisplayPort sink (EDID, DPCD, AUX, link training) |
| `crates/vhost-user-nvrm`, `guest-module/virtio_nvrm`, `guest-module/nvrm_nodes` | v1: the host backend and the guest modules |
| `crates/nvrm-abi`, `crates/nvrm-sys`, `crates/nvrm-client`, `crates/nvrm-trace`, `crates/nvrm-wire` | shared: translation tables and the naming model, generated driver bindings, the host RM client, the `LD_PRELOAD` tracer, the v1 wire schema |
| `guest-module/caraxes_dma_range` | v2: the former guest DMA-range module (decision D13), no longer loaded |
| `patches/` | Cloud Hypervisor patches for both, the ACPI `_DMA` draft, the vendored vfio-user crate change |
| `caraxes/abi/` | the GSP ABI snapshots of 615.71.09 and 610.57.04 and their delta |
| `caraxes/reference/` | captures of the RTX 2070 and the RTX 5060 Ti |
| `caraxes/runs/` | the run records every number in the documents refers to |
| `caraxes/tools/` | benchmark, census, profile, coverage, ABI and lab-kit tools |
| `tools/` | build, checks, the v2 launcher and gates, the Windows test pack |
| `matrix/` | v1's recorded ioctl coverage |
| `docs/` | the documentation of both ([index](docs/README.md)) |

## Contributing and attribution

- Keep changes focused. Include a reproducer and validation commands; for hardware results
  record the commit, GPU, host and guest driver, guest kernel and workload.
- A new card enters through a capture ([contributing a capture](docs/wiki/contributing-a-capture.md)).
- Most original code was AI-assisted; human review is ongoing. This is experimental software.
- Developed at University of Stuttgart, IKR, alongside
  [MeisterStack](https://github.com/UniStuttgart-IKR/MeisterStack). v1's original
  repository is [Leandro](https://github.com/UniStuttgart-IKR/Leandro).
- Driver definitions derive from [NVIDIA open-gpu-kernel-modules](https://github.com/NVIDIA/open-gpu-kernel-modules);
  the vfio-user server builds on the rust-vmm `vfio-user` crate; RM ABI and ownership work
  also draws on [gVisor nvproxy](https://github.com/google/gvisor).

## Licence

Rust crates, tools and documentation: MIT. Guest kernel modules: GPL-2.0-only. Every file
carries its SPDX identifier; [LICENSES.md](LICENSES.md) lists third-party terms.
