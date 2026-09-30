<!-- SPDX-License-Identifier: MIT -->
# Leandro VFIO

Leandro VFIO (v2) gives the guest a PCI GPU, a synthetic TU106 (RTX 2070), for its stock
driver; the per-VM host process `vfio-user-nvgpu` serves it over vfio-user, plays the GSP
firmware and runs the guest's work on the host card. This directory holds its designs,
source surveys and measured studies, one document per topic, dated where their numbers are.
[leandro.md](../leandro.md) compares it with Leandro VirtIO; superseded documents live in
[history](../history/README.md#leandro-vfio-v2).

## Start here

| File | Contents |
|---|---|
| [guest-support.md](guest-support.md) | The status per guest driver: Linux 615, Windows 617, nova-core, nouveau with NVK, each cell with its record |
| [showcase.md](showcase.md) | Screenshots and outputs: the stock driver on one synthetic TU106 under Windows 11 and Ubuntu 24.04 |
| [quickstart.md](quickstart.md) | Build the device, prepare a guest, run a Linux, Windows, nova or nouveau VM, verify |
| [architecture.md](architecture.md) | The device process, its data paths, the source map, what is chip-bound and what is version-bound |
| [known-issues.md](known-issues.md) | Open defects and limits with their evidence; the issues closed recently |
| [decisions.md](decisions.md) | The design decisions D1 to D20 with their reasons |
| [testing.md](testing.md) | The v1 gates against the device: what `sync-tests` copies, the gates of `tools/caraxes-gate.sh`, the verdict rules, the per-test results |
| [benchmark.md](benchmark.md) | v1, v2 and native on one card (P13): harness, method, the closing series, the findings |
| [workload-gate.md](workload-gate.md) | Real workloads against native: PPO, llama.cpp, a transformer training loop, ResNet-18; the runner and the table |

## The driver and the GSP

| File | Contents |
|---|---|
| [driver-modes.md](driver-modes.md) | How the stock driver picks its runtime variant; why the vGPU-VF shortcut is dead on Turing; why GSP-client is the only path |
| [init-map.md](init-map.md) | From PCI enumeration to the first meaningful GSP RPC, with source anchors |
| [kernel-rm-hw-surface.md](kernel-rm-hw-surface.md) | What the kernel RM touches directly besides the GSP boot: config mirror, BAR2 and PRAMIN, interrupt tree, TLB invalidate, timer, MSI, self-tests |
| [registers-turing.md](registers-turing.md) | The BAR0 register map the device answers, by phase |
| [gsp-boot-turing.md](gsp-boot-turing.md) | Every register the driver touches to boot GSP-RM on TU106, in order, with expected values |
| [gsp-boot-blackwell.md](gsp-boot-blackwell.md) | The FSP Chain-of-Trust boot of GB206 and what `src/chip/boot_fsp.rs` answers |
| [cpu-rm-acr-turing.md](cpu-rm-acr-turing.md) | The CPU-RM boot chain on Turing without GSP-RM: what the device must answer |
| [gsp-transport-615.md](gsp-transport-615.md) | The CPU-GSP transport of 615.71.09: message queue v2, MCTP-framed elements, GMCAPI and `GSP_INIT` key by key, the device's `msgq::v2` |
| [gsp-surface.md](gsp-surface.md) | The GSP surface from source: which controls reach GSP, how the device sorts the unhandled ones into categories A to E (P23) |
| [gsp-plan/](gsp-plan/README.md) | The plans for the 615 GSP surface: RPC functions, events, classes, the boot exchange, categories B, C and D, the feature families, the lab kit |
| [gsp-abi-615.md](gsp-abi-615.md) | D20, the GSP ABI baseline 615.71.09: what `NV_ABI_STABLE` covers, the snapshot under `caraxes/abi/`, the counts, the 610-to-615 delta, the workflow and the drift check |
| [gsp-abi-615-tables.md](gsp-abi-615-tables.md) | The generated, human-readable form of the 615 snapshot (checked by `tools/check.sh`) |
| [gsp-transport-classes.md](gsp-transport-classes.md) | Every routed control by transport class: ZT-PAGE, ZT-QUEUE, HOST, TRAP, REFUSE; the cost of one RPC; what a polled queue would save |
| [fifo-device-info.md](fifo-device-info.md) | The FIFO device-info table for TU106: field semantics, engine descriptors, host-derived and synthesized values |
| [vbios-synthesis.md](vbios-synthesis.md) | What the kernel RM needs from the VBIOS and the layout of the synthetic image `caraxes/tools/synth-vbios.py` builds |
| [nvml.md](nvml.md) | The controls `nvidia-smi` and `nvidia-smi -q` send, forwarded or synthesized; the `nvml` gate |
| [cuda-lookahead.md](cuda-lookahead.md) | Every RPC, control and allocation the CUDA path sends, per phase from `cuInit` to teardown, classified |
| [other-architectures.md](other-architectures.md) | GSP boot path and RPC API on Ampere, Ada, Hopper and Blackwell; the VF front end; API stability |

## Memory, BARs and the fast paths

| File | Contents |
|---|---|
| [fb-model.md](fb-model.md) | The framebuffer model: PRAMIN and BAR2 paths, the GMMU walk for BAR3, the FB layout from the WPR meta |
| [memory-model.md](memory-model.md) | Guest FB addresses inside a fixed host VRAM reservation, guest RAM as host OS descriptors, translation, privilege and teardown; the shadow's host RM calls (gathers, region frees, the fail-closed argument) |
| [mapping.md](mapping.md) | The mapping package: incremental shadow walk with write-through copies, coalesced RM maps with a budget, the worker and the doorbell barrier, the BAR aperture TLB |
| [sysmem-iova.md](sysmem-iova.md) | One host OS descriptor of guest RAM gets one contiguous IOVA range; the measurements behind the DMA design |
| [dma-routes.md](dma-routes.md) | Guest DMA addressing: every route (module, ACPI `_DMA`, shadow page tables, host-side tricks) and why shadowing needs no guest change |
| [host-facts.md](host-facts.md) | Host facts H1 to H8 and the VRAM reservation experiment behind D11 |
| [host-resources.md](host-resources.md) | The host VRAM floor, no retry storms on NO_MEMORY, the leak audit, the soak line; figure `figures/host-resources.svg` |
| [bar1-passthrough.md](bar1-passthrough.md) | What the guest does with BAR1, Cloud Hypervisor's sparse-area mechanics, why an RM mapping fd is unsafe, the dma-buf export, prototype G |
| [bar1-pool.md](bar1-pool.md) | The BAR1 pool: a private pool below the carve-out as one dma-buf area, page moves behind the idle-and-preempt barrier, the isolation argument; figure `figures/bar1-pool-table-pages.svg` |
| [bar2-memfd.md](bar2-memfd.md) | BAR2 in host RAM: the memfd area around the trapped flush page, the sync points, why dirty tracking needs a VMM change, the hazards and tests |
| [intr-page.md](intr-page.md) | The interrupt page: BAR0 page 0 and the VF interrupt tree as memory, the reactor's tagged application, the per-frame census |
| [doorbell-token-pool.md](doorbell-token-pool.md) | The doorbell token pool: a submission with no trap through datamatch ioeventfds; the 253-token bound |
| [doorbell-poll.md](doorbell-poll.md) | The doorbell reactor: blocking, spin and hybrid polling of the token eventfds, measured |
| [zero-based-fb.md](zero-based-fb.md) | The zero-based framebuffer for Windows: the owned FB at guest address zero over the physical reservation |
| [uvm.md](uvm.md) | nvidia-uvm on the synthetic device: GPU registration, the channel and copy-engine emulation, the HMM condition |
| [alloc-path.md](alloc-path.md) | Where `cuMemAlloc` and `cuMemFree` spend their time: the guest RM's trapped traffic per mapping change and the four steps against it |
| [ctx-create.md](ctx-create.md) | `cuCtxCreate`: the guest's setup, the host channel builds and the tail; the hold refinement and the shadow prefetch |
| [launch-path.md](launch-path.md) | Where a short kernel's microseconds go from the guest's doorbell to the host's |
| [optimization-backlog.md](optimization-backlog.md) | The trap census of 2026-09-25 per CUDA operation and the ranked backlog it produced (the current census is [research/trap-census-2026-09-30.md](../research/trap-census-2026-09-30.md)) |
| [coverage-map.md](coverage-map.md) | The device's profile mode, 29 workload windows with function coverage, the guest control sweep, the untested surface |

## Execution

| File | Contents |
|---|---|
| [cuda-path.md](cuda-path.md) | The design for CUDA on the stock driver: what nvidia-uvm needs, the first context's objects, identity-mapped VRAM plus host-created channels, the E5 ladder |
| [host-exec.md](host-exec.md) | The standalone host-execution primitive: a host channel bound to a page directory the client built in reserved VRAM |
| [execution.md](execution.md) | Execution on the physical GPU in the device: the VRAM-backed store, the mirror objects, teardown, the execution walls and their contained Xids, the invalidate gate and the device lock |
| [host-gr.md](host-gr.md) | The execution walls analysed: PBENTRY, the CE stall over uncompressed backing, server-reserved PDEs for a host-built GR context |
| [exec-formats.md](exec-formats.md) | PBDMA error causes, GMMU v2 entry layouts and the walker's divergences, the RM flush and invalidate sequence, who maps the GR context buffers |
| [gr-launch-diag.md](gr-launch-diag.md) | Silence after the first GR doorbell: the diagnosis from user space |
| [gr-first-push.md](gr-first-push.md) | The first GR push of `cuCtxCreate` on the shadow VA space: content, wait, completion |
| [gr-completion.md](gr-completion.md) | How a compute push's completion reaches libcuda through the mirror |
| [video-engines.md](video-engines.md) | NVENC and NVDEC through the mirror: protocol and layout, the deferred video object, the probes; figure `figures/video-engine-mirror.svg` |
| [parallel.md](parallel.md) | Two VMs on one card at the same time (P20): slots, isolation by construction and by pattern test, scrub on free, fail-closed teardown, the refusal of a third VM |

## Display

| File | Contents |
|---|---|
| [display-path.md](display-path.md) | What the display gates need, what a display engine costs on Turing, the routes compared |
| [display-connector.md](display-connector.md) | One emulated connector and head, Wayland first: the NV0073 answers, how vblank and flip completion reach the guest, the device timer that paces flips |
| [display-scaling.md](display-scaling.md) | The connectors compared for an emulated link, 4K144 arithmetic, multi-head, the product view |
| [display-adapter.md](display-adapter.md) | Where DP AUX and every display control terminates in the guest driver, the EDID lifecycle, the channel and method basics |
| [display-ab.md](display-ab.md) | Phases A and B: the display in the GSP role, EDID over AUX, link training, gate D3 |
| [display-cd.md](display-cd.md) | Phases C and D: flips on the device clock, Wayland, GNOME, the display gate on vm0 |
| [display-e.md](display-e.md) | Phase E: the stock EGL, GL and Vulkan stack rendering on the physical GPU, Weston and GNOME, the per-frame trap table, the stream stage |
| [display-615.md](display-615.md) | The 615 guest's display path: the overlay, the 615 display layouts, the gate, the per-frame tables |
| [display-pacing.md](display-pacing.md) | Why GNOME flipped at 15 to 35 Hz on a 60 Hz clock: trapped BAR2 flush reads per frame; the deferred sync |
| [display-own-clock.md](display-own-clock.md) | The display on its own clock and lock (plan, 2026-09-29) |
| [sottr-stage.md](sottr-stage.md) | The Shadow of the Tomb Raider stage: the game VM, the Steam library overlay and its writer lock, Sunshine and Moonlight, what is recorded |

## Identity, other chips, other guests

| File | Contents |
|---|---|
| [identity-profile.md](identity-profile.md) | How `Leandro VFIO RTX 2070-4G` is presented from the VRAM profile policy |
| [versatility.md](versatility.md) | The three axes after D14 (chip, board, driver version), where each fact lives, how to add a chip or a driver version |
| [costume-5060.md](costume-5060.md) | The RTX 5060 Ti's ids and name on the TU106: what stays true, where the RM could catch it |
| [costume-ladder.md](costume-ladder.md) | The costume ladder: chips presented over the executing TU106, per rung what is lied, what stays true, the first failure |
| [limits-mig-ecc-vgpu.md](limits-mig-ecc-vgpu.md) | ECC, MIG and vGPU modelled from the kernel source and driven by nvidia-smi in pure emulation |
| [blackwell-gb206.md](blackwell-gb206.md) | The Blackwell lab capture read item by item against TU106; the D14 work list for a GB206 identity |
| [blackwell-gate.md](blackwell-gate.md) | The Blackwell N gate in pure emulation: stages N0 to N4, the walls, the launcher command |
| [lab-blackwell.md](lab-blackwell.md) | The operator's checklist for a lab capture with `caraxes/tools/lab-capture.sh` |
| [passthrough-procedure.md](passthrough-procedure.md) | The prepared passthrough procedure for R1/R3 (SSH/tmux, unbind, vfio-pci, QEMU trace, recovery), written, not run |
| [guest-driver-versions.md](guest-driver-versions.md) | Other guest driver versions against the device: 595, 610, 615; the 615 base image layout (section 8) |
| [v1-615-image.md](v1-615-image.md) | The Leandro VirtIO guest image for 615.71.09 and how the v1 gates run on it |
| [nova.md](nova.md) | nova-core against the synthetic TU106: what the probe reads and waits for, the differences, the ABI diff, gates N0 to N4 |
| [nova-guest.md](nova-guest.md) | nova-core as a guest driver of the device: the 570 transport, the re-probe, the five-probe run |
| [nova-vgpu-upstream.md](nova-vgpu-upstream.md) | The upstream nova-core vGPU manager series read against the device: what transfers, what is VF-only |
| [vfio-user.md](vfio-user.md) | The vfio-user facts that matter: Cloud Hypervisor and QEMU client behaviour, the rust-vmm server API, memory sharing |
| [leandro-reuse.md](leandro-reuse.md) | What of v1's host backend and translation tables the device reuses as libraries, and what breaks |

## Figures

`figures/` holds the SVG figures the documents embed: the v2 data path
([v2-data-path.svg](figures/v2-data-path.svg)), the BAR1 pool's table pages, the host
resources and the VRAM floor, the video engine mirror, the capture re-arm lock. The
reference dumps of the RTX 2070 live in `caraxes/reference/rtx2070/`; run records in
`caraxes/runs/<experiment>-<date>/`.
