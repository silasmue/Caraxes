<!-- SPDX-License-Identifier: MIT -->
# Leandro

GPU sharing for virtual machines on one NVIDIA card that the host keeps using. Two
architectures live in this repository, each named after what the guest sees:

- **Leandro VirtIO (v1)**: API forwarding. The guest sees a VirtIO device; a Leandro kernel
  module in the guest, `virtio_nvrm.ko`, forwards the NVIDIA driver calls over vhost-user
  to a per-VM host backend, `vhost-user-nvrm`, which replays them on the host driver.
- **Leandro VFIO (v2)**: the guest sees a PCI GPU, a synthetic NVIDIA TU106 reported as an
  RTX 2070, and runs its stock driver. A per-VM host process, `vfio-user-nvgpu`, serves it
  over vfio-user: it plays the GSP firmware, models the PCI BARs, shadows the guest's page
  tables and runs the guest's channels on host channels as a client of the host driver.

![The two architectures side by side](docs/img/leandro-two-architectures.svg)

[docs/leandro.md](docs/leandro.md) describes both in one document, the
[roadmap](docs/roadmap.md) what comes next; the [documentation index](docs/README.md) lists
everything else.

**Use trusted guests.** Both architectures confine a guest by validation in the host
process; v2 adds a fixed VRAM slice, registered guest RAM and validated shadow page tables.
Every isolation run used a stock driver; no adversarial guest has been tested
([security.md](docs/virtio/security.md), [isolation rules](docs/wiki/isolation-rules.md)).

## The two architectures

| | Leandro VirtIO (v1) | Leandro VFIO (v2) |
|---|---|---|
| Guest driver | Leandro modules `virtio_nvrm.ko` and `nvrm_nodes.ko` in place of `nvidia.ko`; NVIDIA user space staged to match the host driver | the guest's own stock driver: Linux 615.71.09 open modules, Windows 617.14, nova-core, nouveau with NVK |
| Host process | `vhost-user-nvrm`, one per VM | `vfio-user-nvgpu`, one per VM |
| What crosses the boundary | about 290 ioctl shapes; every RM call is a round trip; submissions through a shared window | PCI config space, BAR0/1/2 accesses (trapped or mapped), the GSP message queues in guest RAM; RM calls stay in the guest kernel |
| Isolation | validated ioctl envelopes, pointers and FDs; a VRAM ledger; no partition of the card | a fixed VRAM slice, registered guest RAM only, shadow page tables that admit validated leaves, no guest compression state |
| Display | NVIDIA's displayless class with a generated EDID and guest timers | an emulated DisplayPort monitor with vblank and flips on the device's clock |
| Hypervisor | Cloud Hypervisor v53.0 with patches 0001 to 0003 | Cloud Hypervisor v53.0 with patches 0005 to 0008 |
| Shared code | the RM translation tables and bindings (`nvrm-abi`, `nvrm-sys`), the host RM client (`nvrm-client`), the tracer, the naming model, the test gates and their verdict rules | |

## Status (2026-09-30)

One synthetic TU106 and one device binary serve four driver stacks, the stock ones
unmodified ([guest support](docs/vfio/guest-support.md), numbers in
[performance](docs/performance.md)):

| Guest driver | Boot, rebind | Graphics | CUDA, video |
|---|---|---|---|
| NVIDIA open 615.71.09, Linux | boots | GNOME on Wayland, Vulkan, GL | CUDA, NVENC, NVDEC |
| NVIDIA 617.14, Windows 11 | boots, reboots | WDDM desktop, D3D11 | CUDA, NVENC, NVDEC |
| nova-core | 5 probes, 3 rebinds, a reload | none in the driver | none in the driver |
| nouveau with Mesa NVK | boots, unbinds | headless Weston, Vulkan, GL | no CUDA; video not tested |

- v2, Linux guest (Ubuntu 24.04, stock 615.71.09): CUDA, Vulkan, GL, a GNOME desktop on
  Wayland, NVENC and NVDEC bit-exact against software decode (H.264, HEVC, VP9), Sunshine
  streaming with NVENC at 60 fps and 5.9 ms host latency. Gates: `nvml` pass, `cuda` 10 of
  12 (managed memory and cuda-gdb open), `display` 11 of 11.
- v2, Windows 11 guest (stock 617.14) under Cloud Hypervisor: a stable desktop, the D3D11
  stages 12 of 12, Unigine Heaven 1080p High 152 to 158 fps, Superposition 1080p Medium up
  to 12247 points, Sunshine with NVENC or x264, NVENC and NVDEC bit-exact. The crashes
  beside a second GPU client are fixed (channel migration, 11 of 11 runs); guest reboots
  survive the vfio-user reconnect.
- v2, open drivers: nova-core probes to completion in pure emulation; nouveau with GSP-RM
  570.144 and Mesa NVK renders on the device (Weston, vkcube, glmark2; vkmark open).
- v2 performance: GPU-bound work runs at the card's speed. Setup paths cost more than
  native (cuCtxCreate 240 to 256 ms beside another VM against 130 ms native). 3D workloads
  still trap 3200 to 8600 times per second, 14 to 24 % of a vCPU; the plan to remove them
  is in the [trap census](docs/research/trap-census-2026-09-30.md).
- v2 open defects: [known issues](docs/vfio/known-issues.md) (DX12 3DMark, Shadow of the
  Tomb Raider on Linux, vkmark on nouveau, host VRAM with two VMs) and the Windows 3D
  throughput below.
- One device binary serves every guest ABI; one configuration with five knobs (VRAM size,
  display, compression, doorbell poll, BAR1 pool); the device logs its effective
  configuration at start ([device configuration](docs/wiki/device-configuration.md)).
- v1: unchanged since 2026-09-24. Its gates on the 615.71.09 image: `smi` pass, `cuda` 14 of
  14. It stays the reference path and the other side of the v1-against-v2 benchmark.

## Measured on the RTX 2070 (TU106)

The latest value per workload; native is the same card without a VM. Ranges, older runs and
sources are in [performance](docs/performance.md), the Fire Strike runs in the
[render trials](https://github.com/silasmue/Caraxes/blob/windows-3dmark-2026-10-04/docs/research/windows-render-trials-2026-10-09.md)
on the branch `windows-3dmark-2026-10-04`.

| Workload | Native | Leandro VirtIO | Leandro VFIO |
|---|---|---|---|
| 3DMark Fire Strike 1.1, 1080p, graphics score (Windows 11) | not measured | no Windows guest | 5486 to 5696 |
| Fire Strike graphics test 1 / test 2, fps (Windows 11) | not measured | no Windows guest | 18.9 to 19.6 / 32.2 to 33.7 |
| Unigine Heaven 4.0, 1080p High, fps (Windows 11) | not measured | no Windows guest | 152.6 |
| Unigine Superposition, 1080p Medium, points (Windows 11) | not measured | no Windows guest | 11656 (87.2 fps) |
| Shadow of the Tomb Raider, 1080p, fps (Linux) | 91.7 | 68.2, 70.1 | 34.7 in one run, fails at start in another |
| glmark2-wayland, 1280x720, score (Linux) | 12214 | not measured | 3324 |
| vkmark, 1280x720, score (Linux) | 15973 | not measured | 2388, 2635 |
| NVENC H.264 encode, 1080p, fps (Linux / Windows 11) | not measured | not measured | 414 / 311 |
| Sunshine with NVENC, fps and host latency (Linux / Windows 11) | not measured | 58.8, 6.8 ms | 59.9, 5.3 ms / 59.8, 6.9 ms |
| CUDA kernel of 30M cycles, ms | 16.3 | 16.5 | 17.1 |
| CUDA short kernel, µs per launch | 6 | 8 | 23 |
| cuCtxCreate, ms | 129 | 153 | 210 |
| llama.cpp 1B, token generation, tokens/s | 305 | 304 | 243 |

- Fire Strike: three instrumented runs of 2026-10-09 (C220 to C222); two runs of 2026-10-05
  reached 6015 and 6040. Everything else on Leandro VFIO: 2026-09-30. Native and Leandro
  VirtIO values come from dated records (2026-08-07 to 2026-09-29).
- GPU-bound compute runs at the card's speed. Work made of many small calls is far from
  native: short kernels, context creation and 3D.
- Windows 3D is the open problem. Fire Strike completes, but its graphics tests run at 19 to
  34 fps, and neither server polling nor a larger BAR1 pool raised them. 3DMark Steel Nomad
  (DX12) still faults ([known issue 1](docs/vfio/known-issues.md)).
- The author has little time for the project at present. The [roadmap](docs/roadmap.md), the
  [known issues](docs/vfio/known-issues.md), the
  [trap census](docs/research/trap-census-2026-09-30.md) and the notes on the branch
  `windows-3dmark-2026-10-04` say where to continue. Issues, forks and pull requests are
  welcome.

## Start here

- [Documentation index](docs/README.md): guides, design notes, the wiki, the research notes.
- [leandro.md](docs/leandro.md): both architectures, the driver, the GSP, memory, submission,
  execution, display, identity, performance, verification, the ranked weaknesses.
- v2: [quickstart](docs/vfio/quickstart.md), [architecture](docs/vfio/architecture.md),
  [known issues](docs/vfio/known-issues.md), [decisions](docs/vfio/decisions.md).
- v1: [quickstart](docs/virtio/quickstart.md), [architecture](docs/virtio/architecture.md),
  [testing](docs/virtio/testing.md), [open questions](docs/virtio/open-questions.md).
- [Wiki](docs/wiki/README.md): one page per concept; [research notes](docs/research/README.md):
  the evidence.
- [Development](DEVELOPMENT.md): build, checks, tracing and troubleshooting.

## Requirements

- Linux x86-64 host with KVM and the NVIDIA open kernel modules with GSP firmware active. The
  recorded host runs 615.71.09 on an RTX 2070; v2 selects the host ABI at run time and
  serves the guest's driver version independently of it. v1's guest user space must match a
  compiled host ABI entry ([abi-versions.md](docs/virtio/abi-versions.md)).
- Cloud Hypervisor [v53.0](CH_VERSION) with the [patches](patches/README.md): 0001 to 0003
  (vhost-user shared memory) for v1; 0005 to 0008 (vfio-user region I/O fds and read-only
  mmap areas) for v2; 0004 (ACPI `_DMA`) is optional evidence.
- The pinned [Rust toolchain](rust-toolchain.toml); Python 3, a C compiler and clang for the
  tools and the ABI checks. [DRIVER_VERSION](DRIVER_VERSION) names the build default of the
  shared crates.
- Guests: v2 an Ubuntu 24.04 image with the distribution's NVIDIA packages, a Windows 11
  image with the 617 driver, or the nova and nouveau kernels of the run folders; v1 the
  baked image with the Leandro modules ([vm-preparation.md](docs/virtio/vm-preparation.md)).
- Builds and software checks need no GPU and no Leandro-Test checkout. The hardware gates
  read the private Leandro-Test repository.

## Build and check

```sh
./tools/build.sh all                         # v1: vendor sources, patched Cloud Hypervisor, workspace
cargo build --release -p vfio-user-nvgpu     # v2: the device
./tools/check.sh                             # formatting, clippy, tests, licence headers, ABI drift
CARAXES_CHECK_EMU_BOOT=1 ./tools/check.sh    # also boots the synthetic GPU without a GPU
```

- v1 VMs start with the commands of the [v1 quickstart](docs/virtio/quickstart.md).
- v2 VMs start with `tools/caraxes-vm.sh up` and are checked with `tools/caraxes-gate.sh`
  ([v2 quickstart](docs/vfio/quickstart.md)). Give each VM its own device process,
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
| `docs/` | shared documents; `docs/virtio/` for v1, `docs/vfio/` for v2 ([index](docs/README.md)) |

## Related projects

Same goal, a shared NVIDIA GPU for VMs without passthrough or vGPU:

- [kayfabe](https://github.com/reindertpelsma/kayfabe): a stock NVIDIA driver in the guest
  against emulated hardware with a faked GSP, work forwarded to a host GPU; Rust; research
  stage. The approach closest to Leandro VFIO.
- [nvkvm-pv](https://github.com/reindertpelsma/nvkvm-pv): a paravirtual NVIDIA GPU for KVM
  guests; a small guest kernel module forwards the NVIDIA driver's own API over virtio to the
  host driver, with unmodified user space. The approach closest to Leandro VirtIO.
  - [nvkvm-kata](https://github.com/reindertpelsma/nvkvm-kata): nvkvm inside Kata Containers.
  - [nvkvm-steamos](https://github.com/reindertpelsma/nvkvm-steamos): SteamOS as an nvkvm-pv
    guest.
- [virtio-nvgpu](https://github.com/nestrilabs/virtio-nvgpu): a virtio device that forwards
  NVIDIA kernel-driver ioctls between a Linux guest and the host at the driver ABI level;
  aimed at headless streaming. Also close to Leandro VirtIO.

Other approaches:

- virtio-gpu DRM native context: forwards the native kernel-driver UAPI instead of a graphics
  API; upstream for AMD (amdgpu) and Qualcomm (freedreno), Intel in review
  ([virglrenderer](https://gitlab.freedesktop.org/virgl/virglrenderer),
  [QEMU's virtio-gpu docs](https://www.qemu.org/docs/master/system/devices/virtio/virtio-gpu.html)).
  The reference for the VirtIO variants of Meleys (AMD) and Dreamfyre (Intel).
- [vgpu_unlock](https://github.com/DualCoder/vgpu_unlock): enables NVIDIA's own vGPU on
  consumer cards.

Over the coming months the goal is to learn as much as possible from all of these approaches,
then work out common protocols together (Sunfyre in the [roadmap](docs/roadmap.md)).

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
