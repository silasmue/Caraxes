<!-- SPDX-License-Identifier: MIT -->
# vfio-user-nvgpu

Leandro VFIO: the device.

- One host process per VM. Cloud Hypervisor connects to its vfio-user socket, and the guest
  sees a PCI GPU, a synthetic TU106 (RTX 2070), for its stock driver.
- Models PCI config space, BAR0/1/2 and the VBIOS, plays GSP-RM on the message queues in
  guest RAM, shadows the guest's page tables and runs the guest's channels on host channels
  as a client of the host RM (through `nvrm-client` and `nvrm-sys`); the display uses
  `dp-sink`.
- Entry points: the binary (`src/main.rs`), the library (`src/lib.rs`), every variable it
  reads in `src/settings.rs`; chip and board data in `chips/` and `boards/`; guest-side
  helpers in `guest/`.
- Build and test: `cargo build --release -p vfio-user-nvgpu`, `cargo test -p vfio-user-nvgpu`.
  `tools/caraxes-vm.sh` starts a VM with it; `tools/emu-boot-check.sh` boots it without a GPU.
- Docs: [architecture and source map](../../docs/vfio/architecture.md),
  [quickstart](../../docs/vfio/quickstart.md),
  [device configuration](../../docs/wiki/device-configuration.md).
