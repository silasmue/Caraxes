<!-- SPDX-License-Identifier: MIT -->
# VFIO Leandro (Caraxes)

A synthetic NVIDIA PCI device for Linux VMs sharing one NVIDIA GPU with the host. The
guest runs its distribution's stock NVIDIA driver; a per-VM host process,
`vfio-user-nvgpu`, serves the device over vfio-user, plays the GPU System Processor
firmware (GSP-RM) for the guest, models the PCI BARs, and executes the guest's work on the
physical GPU as a client of the host driver through mirror channels and shadow page tables.
This is Leandro v2. VirtIO Leandro (v1), the API-forwarding architecture with a guest
module, lives in the [Leandro repository](https://github.com/UniStuttgart-IKR/Leandro);
[docs/LEANDRO.md](docs/LEANDRO.md) describes both.

**Use trusted guests.** The device confines a guest to its own VRAM slice and registered
RAM through validated shadow page tables, refuses compression state and privileged paths,
and keeps every recorded fault on its own process ([isolation rules](docs/wiki/isolation-rules.md));
every isolation run used the stock driver, and no adversarial guest has been tested.

## Start here

- [LEANDRO.md](docs/LEANDRO.md): the overview of both architectures: the driver, the GSP,
  the BARs and memory, submission, execution, display, identity, performance, verification,
  and the ranked weaknesses with their mitigation paths.
- [Wiki](docs/wiki/README.md): one page per concept, driver references by path and function.
- [Research notes](docs/research/README.md): the evidence, one short note per topic with
  615.71.09 lines and run folders.
- [Working record](docs/history/caraxes-record-2026-09-28.md): decisions D1 to D20, studies, designs, measurements
  and the status log of VFIO Leandro.
- [Quickstart](docs/caraxes/quickstart.md): build the device, prepare a guest, run and verify.
- [Development](DEVELOPMENT.md): build, checks, tracing and troubleshooting.
- [Testing](docs/caraxes/testing.md), [benchmark](docs/caraxes/benchmark.md),
  [roadmap](docs/history/caraxes-roadmap-2026-09-27.md), [open questions](docs/history/caraxes-open-questions-2026-09-23.md).

## Requirements

- Linux x86-64 host with KVM and the NVIDIA open kernel modules on the host card with GSP
  firmware active. The host runs 615.71.09; the device serves a 615.71.09 or 610.57.04
  guest and selects the host ABI at run time. [DRIVER_VERSION](DRIVER_VERSION) still names
  the 610.57.04 build default for the shared crates ([cutover-615.md](docs/history/caraxes-cutover-615-2026-09-24.md)).
- Cloud Hypervisor [v53.0](CH_VERSION) with [patches 0005 and 0006](patches/README.md)
  (vfio-user region I/O fds and datamatch sub-regions); 0004 is optional. Patches 0001 to
  0003 belong to VirtIO Leandro.
- Pinned [Rust toolchain](rust-toolchain.toml); Python 3, a C compiler and clang for the
  tools and the ABI checks.
- A guest image with the distribution's stock NVIDIA driver, `/etc/modprobe.d/caraxes.conf`
  with the three registry keys, and the temporary `caraxes_dma_range` module
  ([kernel-rm-hal-and-registry](docs/wiki/kernel-rm-hal-and-registry.md),
  [sysmem-and-dma](docs/wiki/sysmem-and-dma.md)). Recorded guests: Ubuntu 24.04 with kernel
  6.8.
- Core builds and software checks need no GPU and no Leandro-Test checkout.

## Build and check

```sh
cargo build --release -p vfio-user-nvgpu
./tools/check.sh
CARAXES_CHECK_EMU_BOOT=1 ./tools/check.sh      # boots the synthetic GPU without a GPU
tools/caraxes-vm.sh up                          # one VM with the device (VM=vm0 by default)
VM=vm0 tools/caraxes-gate.sh nvml               # then cuda, gpu, vdisplay, display
```

- `tools/check.sh` runs formatting, clippy, the unit tests, the licence headers, the ABI
  generation and the GSP ABI drift check against a 615.71.09 tree when one is found.
- `tools/caraxes-vm.sh` launches Cloud Hypervisor with the device; `tools/caraxes-gate.sh`
  runs the gates with VirtIO Leandro's verdict rules
  ([verification-methods](docs/wiki/verification-methods.md)).
- The recommended device configuration (interrupt page, BAR1 pool, doorbell token pool,
  hybrid reactor, BAR2 memfd, MSI re-arm ioeventfd) is the header of `caraxes/tools/bench-run.sh`.
- Give each VM its own device process, socket, VRAM slot, writable disk and network identity.

## Hardware evidence

| GPU / driver | Recorded coverage |
|---|---|
| RTX 2070 (TU106) / host 615.71.09, guest 615.71.09 | `nvml` pass, `cuda` 10 of 12 scored (cuda-managed and cuda-gdb open), `display` 11 of 11 on Wayland; workload gate at 2 GiB (llama.cpp 0.92 to 0.96, transformer 1.01, ResNet-18 1.04, PPO 0.64 of native); 2026-09-25 |
| RTX 2070 / host 615.71.09, guest 610.57.04 | the same gates; two VMs concurrently with isolation evidence (2026-09-24) |
| RTX 5060 Ti (GB206) / 615.71.09 | a read-only capture (2026-09-24); the GB206 identity boots in pure emulation to the first channel allocation with a 610 guest; no execution on it |

- Results cover the recorded revisions and workloads, not every configuration; every number
  in the documents names its run under `caraxes/runs/`.
- The VRAM slice is a fixed share of the card; there is no QoS for GPU time, channels or RM
  work between VMs ([performance-shape](docs/wiki/performance-shape.md)).
- [Captured cards](docs/wiki/captured-cards.md) lists what the project holds;
  [contributing a capture](docs/wiki/contributing-a-capture.md) how to add a card.

## Components

| Path | Role |
|---|---|
| `crates/vfio-user-nvgpu` | The device: PCI and BAR0 model, GSP boot and message queue, the RPC dispatcher, the memory model, shadow page tables, mirror channels, display, the shared-memory windows; `chips/` and `boards/` data |
| `crates/dp-sink` | DisplayPort sink: EDID, DPCD, AUX, link training |
| `crates/nvrm-abi`, `crates/nvrm-sys`, `crates/nvrm-client`, `crates/nvrm-trace` | Shared with VirtIO Leandro: translation tables and the naming model, generated driver bindings, the host RM client, the `LD_PRELOAD` tracer |
| `crates/vhost-user-nvrm`, `guest-module/virtio_nvrm`, `guest-module/nvrm_nodes` | VirtIO Leandro's backend and guest modules, kept for the v1-against-v2 benchmark ([repo-split.md](docs/history/caraxes-repo-split-2026-09-24.md)) |
| `guest-module/caraxes_dma_range` | The temporary guest DMA-range module (decision D13) |
| `patches/` | Cloud Hypervisor patches: 0001 to 0003 vhost-user shared memory (v1), 0004 ACPI `_DMA`, 0005 and 0006 vfio-user region I/O fds |
| `caraxes/abi/` | The GSP ABI snapshots of 615.71.09 and 610.57.04 and their delta |
| `caraxes/reference/` | The captures of the RTX 2070 and the RTX 5060 Ti |
| `caraxes/runs/` | The records every number refers to |
| `caraxes/tools/` | Benchmark, census, profile, coverage, ABI, lab-kit and host tools |
| `tools/` | Launcher, gates, checks, emu-boot |
| `matrix/` | VirtIO Leandro's recorded ioctl coverage |

## Contributing and attribution

- Keep changes focused. Include a reproducer and validation commands; for hardware
  results record the commit, GPU, host and guest driver, guest kernel and workload.
- A new card enters through a capture ([contributing a capture](docs/wiki/contributing-a-capture.md)).
- Most original code was AI-assisted; human review is ongoing.
- Developed at University of Stuttgart, IKR, alongside
  [MeisterStack](https://github.com/UniStuttgart-IKR/MeisterStack).
- Driver definitions derive from [NVIDIA open-gpu-kernel-modules](https://github.com/NVIDIA/open-gpu-kernel-modules);
  the vfio-user server builds on the rust-vmm `vfio-user` crate; RM ABI and ownership work
  also draws on [gVisor nvproxy](https://github.com/google/gvisor).
- Rust/tools/docs: MIT. Guest modules: GPL-2.0-only. See [LICENSES.md](LICENSES.md).
