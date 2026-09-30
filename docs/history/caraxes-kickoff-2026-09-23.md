<!-- SPDX-License-Identifier: MIT -->
# Kickoff brief for the implementation sessions

Written 2026-09-23 for whoever starts the Caraxes implementation: a fresh agent
session in this repository, or the same session after a context reset. Everything
below is grounded in the notes in this directory. When the notes and this brief
disagree, the notes win and this brief gets corrected.

## 1. What Caraxes is

Leandro v2. The chain to build:

```
unmodified NVIDIA guest driver (open kernel modules 610.57.04, stock, not GRID)
  → synthetic NVIDIA PCI device, served from a Rust process over vfio-user
  → synthetic GSP-RM endpoint (falcon boot side effects + message queue + RPC)
  → existing Leandro RM backend (crates/vhost-user-nvrm, crates/nvrm-abi)
  → host driver → physical RTX 2070 (TU106)
```

Leandro v1 (this tree, minus `docs/caraxes/` and `caraxes/`) forwards RM ioctls
from a patched guest module over vhost-user and works. It stays untouched and
working. Caraxes adds a second front end that needs no guest module at all.

First milestone: the stock Linux driver recognizes and initializes the synthetic
Turing device (`RmInitAdapter succeeded!`, `/dev/nvidia0` usable, `nvidia-smi`
prints `Leandro RTX 2070-4G` with the profile's memory size). Windows comes later.

## 2. Rules that do not bend

1. **The host GPU state is read-only and hands-off.** No unbinding, no kernel
   modules, no `/dev/mem` tricks, no writes to host NVIDIA registers, no host
   driver changes. The physical card is reached only through the host RM API as an
   unprivileged client, exactly as `vhost-user-nvrm` does today. Report every read
   of host GPU state to the operator. The desktop runs on this card, and so does the
   agent session (VSCode), so passthrough (R1/R3) is not allowed until a prepared
   procedure exists (see the safety envelope in `experiments.md`).
2. **No guest driver patches.** Temporary aids are allowed only when documented in
   `experiments.md` with the reason and the plan to remove them. The guest runs
   the stock open driver, never the GRID driver.
3. **Stay inside `/home/silas/git/Caraxes`** (its `target/` symlink included). VM
   assets under `/mnt/vmstore/leandro` are inputs; overlays, sockets and logs go
   under `target/caraxes/`. Never touch sibling repositories.
4. **Leandro v1 keeps working.** Add code; do not rewrite `vhost-user-nvrm`,
   `nvrm-abi`, `nvrm-wire` or the guest module for Caraxes' sake. Making a private
   function public (the profile decision in `vram.rs`, see open question Q22) is
   fine when the existing tests still pass.
5. **Explain before significant architectural changes.** Write the reasoning into
   `README.md` (decisions table) before the change, not after.
6. **`tools/check.sh` passes** before a session ends: SPDX header on every tracked
   file (MIT outside `guest-module/`), no marker glyphs, clippy clean, formatted.
7. **Notes are part of the deliverable.** Every experiment result goes into
   `experiments.md` (expected, observed, log excerpts, commits of both trees). New
   unknowns get a stable number in `open-questions.md`. Do not commit unless the
   operator asks.
8. **Differential testing.** The real card's behaviour is the ground truth; the
   emulator is corrected from observed guest behaviour one failure at a time.
9. **Label provenance, pass through first, minimize later (D9).** Every value the
   device hands the guest is tagged with where it came from (host RM reply, profile,
   synthesized constant, guest-written). The first implementation of E3/E4 forwards host
   information generously and documents it; the cut to the necessary set follows from the
   labels. That is the basis for real per-VM isolation on the GPU.

## 3. Read in this order

1. [README.md](caraxes-record-2026-09-28.md): findings 1–9, decisions D1–D6.
2. [experiments.md](caraxes-experiments-2026-09-29.md): the ladder E0–E4 and the reference captures R1–R3.
3. [vfio-user.md](../caraxes/vfio-user.md): protocol, the Cloud Hypervisor client, the E0 device model.
4. [init-map.md](../caraxes/init-map.md) phases 0–2: what the driver does before it needs GSP.
5. [registers-turing.md](../caraxes/registers-turing.md): the BAR0 map by phase.
6. [identity-profile.md](../caraxes/identity-profile.md): where the name and FB size come from.
7. When E1–E3 arrive: [gsp-boot-turing.md](../caraxes/gsp-boot-turing.md),
   [gsp-rpc-protocol.md](caraxes-gsp-rpc-protocol-610.md), [kernel-rm-hw-surface.md](../caraxes/kernel-rm-hw-surface.md),
   [leandro-reuse.md](../caraxes/leandro-reuse.md).
8. [open-questions.md](caraxes-open-questions-2026-09-23.md) and [environment.md](caraxes-environment-2026-09-23.md).

## 4. State when this brief was written

- Exploration complete; no device code exists. Reference dumps of the real card are
  in `caraxes/reference/rtx2070/` (config space, capabilities, PCI-visible ROM
  chain, `nvidia-smi -q`). The ROM chain lacks the FWSEC image (experiment R1).
- Vendor: `vendor/open-gpu-kernel-modules` at 610.57.04 (matches the host driver),
  `vendor/cloud-hypervisor` v53 with the v1 patches, `vendor/vfio` (rust-vmm, git-ignored;
  `vfio_user` 0.1.6 is also on crates.io), `vendor/libvfio-user` and
  `vendor/qemu-vfio-user` for reading.
- Decisions: GSP-client mode with a TU106 identity (D1); no Ada identity (D2); Rust
  server on rust-vmm `vfio_user`, Cloud Hypervisor v53 `--user-device` as the client,
  QEMU only for the passthrough MMIO capture (D3); vendor driver stays at 610.57.04
  (D4); first experiment is a logging-only device (D5); identity `Leandro <board>-<size>`
  from the existing profile policy at the device level (D6).
- The operator has given the go-ahead for E0.

## 5. First deliverable: experiment E0, step by step

Each step has a check. Do not skip to the next step while a check fails.

1. **Crate.** `crates/vfio-user-nvgpu` as a workspace member (the name follows
   `vhost-user-nvrm`: transport, then what is served). Binary `vfio-user-nvgpu`.
   Dependencies: `vfio_user = "0.1.6"` and `vfio-bindings` from crates.io (same
   lineage as Cloud Hypervisor's client 0.1.3), `nvrm-abi` for the profile catalogue.
   Modules: `config.rs` (PCI config space model), `bar0.rs` (register map and PROM
   window), `trace.rs` (access log), `identity.rs` (profile to PCI ids, name, FB size),
   `server.rs` (the `ServerBackend` implementation). Check: `cargo build` and
   `tools/check.sh` pass with the empty backend.
2. **Config space.** Parse `caraxes/reference/rtx2070/config-space.txt` (the
   `lspci -xxxx` hex dump) into the 4 KiB region. Writable per PCI rules: command
   register, BAR sizing (mask writes return the size mask), ROM BAR enable bit, PM
   control, MSI control/address/data; everything else read-only. Keep PM (0x60),
   MSI (0x68, 64-bit, one vector, no masking) and PCIe (0x78) capabilities as on
   the card; return zeros for the extended capability list at 0x100. Check: a unit
   test reads back vendor/device/subsystem/class/revision and the BAR size masks.
3. **Regions and IRQs** as in the table in [vfio-user.md](../caraxes/vfio-user.md): BAR0 16 MiB
   trapped, BAR1 256 MiB trapped, BAR3 32 MiB trapped, BAR5 128 B I/O, ROM 512 KiB
   from `rom.bin` (git-ignored; fail with a clear message if missing), config 4 KiB.
   IRQ info INTx 1, MSI 1, MSI-X 0. `set_irqs` keeps the eventfds and never fires.
   `dma_map`/`dma_unmap` log address, size, offset and fd and keep the fds (E3 mmaps
   them). `reset` logs and does nothing.
4. **BAR0 answers.** `NV_PMC_BOOT_0` (0x0) = 0x166000A1 (predicted; R2 confirms),
   `NV_PMC_BOOT_1` (0x4) = 0, `NV_PMC_BOOT_42` (0xA00) = chip id 0x166 in bits 29:20
   with revision A1 in 19:12; PROM window 0x300000 onward served from `rom.bin`;
   zero elsewhere. Every access to every region is logged as one line:
   monotonic microseconds, direction, region, offset, size, value. The log goes to
   the file given by `--trace`, the console shows a summary.
5. **Self-test without a VM.** A `--selftest <socket>` mode (or an integration
   test) uses `vfio_user::Client` from the same crate: connect, read device, region
   and IRQ info, read config bytes 0x0–0x3F and BAR0 0x0/0x4/0xA00, and compare with
   the expected values. Check: the self-test passes against a running server.
6. **Guest preparation (once).** Overlay disk on the Ubuntu 610.57.04 image:
   `qemu-img create -f qcow2 -b <base> -F qcow2 target/caraxes/vm0/rootfs.qcow2`.
   Kernel and initrd are in `/mnt/vmstore/leandro/vm/guest-image/`. Guest user
   `leandro`, key `/mnt/vmstore/leandro/vm/id_leandro`. Network: a tap owned by the
   user (create with `sudo ip tuntap add … user silas`, record the command in
   `environment.md`); no Leandro rig is required. Copy `vendor/open-gpu-kernel-modules`
   into the guest, build `make modules -j$(nproc)` against `linux-headers-6.8.0-136-generic`,
   install only `nvidia.ko` (no modeset/drm for E0–E3), copy
   `/usr/lib/firmware/nvidia/610.57.04/gsp_tu10x.bin` from the host to
   `/lib/firmware/nvidia/610.57.04/`. Check: `modinfo nvidia` in the guest shows
   610.57.04 and the module loads without a device (no probe).
7. **Run.** The launcher `tools/caraxes-vm.sh` (`up`, `wait`, `probe <label>`, `down`)
   does all of the following; `probe` reloads the module in the guest and saves the guest
   log and the device trace of that window. Server first, then Cloud Hypervisor (`/mnt/vmstore/leandro/cloud-hypervisor/bin/cloud-hypervisor`,
   v53):

   ```sh
   target/release/vfio-user-nvgpu --socket target/caraxes/vm0/gpu.sock \
       --trace target/caraxes/vm0/gpu-trace.log &
   "$LEA_CH" --cpus boot=4 --memory size=4G,shared=on \
     --kernel /mnt/vmstore/leandro/vm/guest-image/vmlinuz \
     --initramfs /mnt/vmstore/leandro/vm/guest-image/initrd \
     --cmdline 'root=/dev/vda1 rw console=ttyS0 net.ifnames=0' \
     --disk path=target/caraxes/vm0/rootfs.qcow2,image_type=qcow2,backing_files=on \
     --net tap=<tap>,mac=02:00:00:00:00:20 \
     --user-device socket=target/caraxes/vm0/gpu.sock,id=gpu0 \
     --serial file=target/caraxes/vm0/serial.log --console off
   ```

   No `--platform iommu`, no virtio-mem. In the guest: `lspci -nn` shows
   `10de:1f02`; then `modprobe nvidia NVreg_GpuInitOnProbe=1 NVreg_ResmanDebugLevel=0`.
8. **Expect** (from [experiments.md](caraxes-experiments-2026-09-29.md) E0): probe binds; the trace
   shows reads at BAR0 0x4, 0x0, 0xA00 (three rounds), then the PROM window from
   0x300000, then `RmInitAdapter failed` with the VBIOS error path in the guest log.
   Falsifiers: probe refusal (config model wrong), no PROM reads (init-map order
   wrong), any read at an offset absent from [registers-turing.md](../caraxes/registers-turing.md)
   (map incomplete; add it with the phase it belongs to).
9. **Record.** Guest `dmesg`, the trace file, the Cloud Hypervisor log, the commit
   ids of this tree and of `vendor/open-gpu-kernel-modules`, into
   `experiments.md` under E0. Update `registers-turing.md` with every new offset and
   `open-questions.md` with what the run resolved or raised. Then stop and report.

## 6. After E0

Done on 2026-09-23: E0, E1 and E2 (results in `experiments.md`, evidence in
`caraxes/runs/`). E1 uses the synthetic VBIOS from `caraxes/tools/synth-vbios.py`
(decision D8); E2 is `--boot-model` (`boot.rs`, `falcon.rs`, `guestmem.rs`), and
`tools/caraxes-vm.sh probe` resets the device first so runs repeat. Milestone 1 was reached on 2026-09-23 (`RmInitAdapter succeeded!`, `nvidia-smi` prints
`Leandro RTX 2070-4G`); see "Milestone 1 reached" in `experiments.md` for what is open.
E3 is done as well
(`msgq.rs`, `rpc.rs`: queues, `GSP_INIT_DONE`, the two sync replies, labelled static info).
The next experiment is E4: answer the phase-5 RPCs in the order of `phase5-rpcs.md`, a
dispatcher table keyed by function and control id, each answer labelled with its source;
synthesized rows first, then the host-forwarded ones; probe after every few rows. The
success check is `nvidia-smi` in the guest printing the Leandro name. Host RM access lives
in the device process itself (decision D10): open `/dev/nvidiactl` through `nvrm-client`,
allocate the device's own root client, device and subdevice on the host, and issue the
public controls the inventory names; do not attach the vhost-user device to the VM.
Subagents: use Opus (or Sonnet for surveys); Fable-class agents only when necessary.

E1 (full VBIOS through the PROM window; needs the R1 capture, which takes the
desktop down and is scheduled with the operator), E2 (falcon state machines and
boot status registers), E3 (message queue, `GSP_INIT_DONE`, static info from the
profile; success is `nvidia-smi` printing the Leandro name and size), E4 (phase-5
RPCs answered by the host RM through the Leandro backend). Details and expectations
are in [experiments.md](caraxes-experiments-2026-09-29.md). Scope stays `nvidia.ko` and `nvidia-uvm`
until E4 passes; `nvidia-modeset` and `nvidia-drm` come after.

Use the tracer. `crates/nvrm-trace` decodes the guest's RM ioctls with the `nvrm-abi`
tables; from E3 on, decode the device-side RPCs with the same tables and the same line
format, and run the tracer on `nvidia-smi` in the guest to compare the two streams
(open question Q23). Extend the tracer rather than writing a second decoder.

## 7. Ideas recorded, not planned

- **P3, spatial slicing.** Slice the GPU by SMs rather than in time (as NVIDIA's vGPU
  scheduler does), and virtualize the device for real behind the synthetic front end.
  Recorded 2026-09-23 at the operator's request; future work after milestone 1.

- **P1, vendor-neutral transport.** Once the GSP-RPC front end works, the calls
  crossing the device boundary could be packed into a vendor-neutral protocol, so
  that a backend on a different GPU (an AMD card, for instance) could serve an
  NVIDIA guest in principle. Recorded on 2026-09-23 at the operator's request; not
  to be worked on until milestone 1 is reached, and then only after a written design.
  Related: the v1 generalization proposal in [../VIRTIO-UAPI.md](../VIRTIO-UAPI.md).
  Measured on 2026-09-23 (numbers in [README.md](caraxes-record-2026-09-28.md), parked ideas): the GSP
  boundary carries the whole RM alloc/control vocabulary inside two RPCs, so it is not
  smaller than the ioctl boundary; a small vendor-neutral protocol would have to sit
  above RM.

## 8. How to report

Lead with the outcome of the last check that ran. Quote guest log lines and trace
lines verbatim in code blocks. Name the file and line for every claim about the
driver. Say what was not verified. Keep the operator informed before any action
that touches host state outside the repository (network taps included).
