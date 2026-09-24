<!-- SPDX-License-Identifier: MIT -->
# Caraxes: a synthetic NVIDIA PCI device for stock guest drivers

Working notes for Leandro v2 ("Caraxes"). Started 2026-09-23. Nothing here is
implemented yet; this directory records what was found in source, what was
measured on the host, the decisions taken, and the experiment ladder.

## Goal

```
unmodified NVIDIA guest driver (open kernel modules, 610.57.04)
  → synthetic NVIDIA PCI device served over vfio-user (Cloud Hypervisor client)
  → synthetic GSP-RM endpoint (message queue + RPC, emulated in userspace)
  → existing Leandro RM backend (crates/vhost-user-nvrm) → host driver → RTX 2070
```

Constraints (from the project brief): do not modify the host driver; no guest
driver patches except as a temporary aid; the stock (non-GRID) driver; the
frontend lives in userspace via vfio-user; existing Leandro must keep working;
reuse the RM backend rather than rewrite it. First milestone: the stock Linux
driver recognizes and initializes a Turing/RTX 2070-like device. Windows later.

## Files

| File | Contents |
|---|---|
| [quickstart.md](quickstart.md) | Build, prepare the guest, run and verify the current state (user documentation) |
| [architecture.md](architecture.md) | Components, data paths, what is version-bound, differences from Leandro v1, the milestone ladder (user documentation) |
| [environment.md](environment.md) | Host, driver, tooling, vendor checkouts and the reference dumps of the real card |
| [driver-modes.md](driver-modes.md) | How the stock driver picks its runtime variant; why the vGPU-VF shortcut is dead on Turing; why GSP-client is the only path |
| [init-map.md](init-map.md) | The concrete path from PCI enumeration to the first meaningful GSP RPC, with file:line anchors |
| [gsp-boot-turing.md](gsp-boot-turing.md) | Every register the driver touches to boot GSP-RM on TU106, in order, with expected values (source survey) |
| [gsp-boot-blackwell.md](gsp-boot-blackwell.md) | The same for GB206 with a 610.57.04 guest: the FSP Chain-of-Trust boot (FSP boot-complete scratch, NVDM/MCTP messages over the FSP EMEM queue, the COT payload and GSP_FMC_BOOT_PARAMS, the WPR layout ACR fills, target-mask and lockdown release), every register in order with the GB206 HAL and header, and what `src/chip/boot_fsp.rs` answers (source survey) |
| [gsp-rpc-protocol.md](gsp-rpc-protocol.md) | Message-queue memory layout, checksums, sequence rules, RPC headers, init handshake, alloc/control serialization (source survey) |
| [kernel-rm-hw-surface.md](kernel-rm-hw-surface.md) | What the kernel RM touches directly besides GSP boot: config mirror, BAR2/PRAMIN, interrupt tree, TLB invalidate, timer, MSI, self-tests (source survey) |
| [registers-turing.md](registers-turing.md) | Consolidated BAR0 register map the emulator must answer, by phase |
| [leandro-reuse.md](leandro-reuse.md) | What in crates/vhost-user-nvrm and nvrm-abi is reusable for a GSP-RPC front end, and what breaks (source survey) |
| [vfio-user.md](vfio-user.md) | Protocol facts that matter, Cloud Hypervisor and QEMU client behaviour, rust-vmm server API, memory-sharing requirement |
| [identity-profile.md](identity-profile.md) | How "Leandro VFIO RTX 2070-4G" is presented from Leandro's existing VRAM profile policy, and which v1 rewrites it replaces |
| [vbios-synthesis.md](vbios-synthesis.md) | What the kernel RM needs from the VBIOS and the layout of the synthetic image `caraxes/tools/synth-vbios.py` builds (subagent report) |
| [other-architectures.md](other-architectures.md) | GSP boot path and RPC API on Ampere, Ada, Hopper, Blackwell; VF front end; API stability (subagent survey, parked idea P2) |
| [lab-blackwell.md](lab-blackwell.md) | Operator checklist for the Blackwell lab capture (`caraxes/tools/lab-capture.sh`): prerequisites, commands, what needs sudo, what to bring back to `caraxes/reference/<chip>/`, what not to do, the part-2 ABI check per driver version, and what each captured item feeds in the Blackwell identity work |
| [fb-model.md](fb-model.md) | The framebuffer model for milestone 1: PRAMIN and BAR2 paths, the GMMU walk for BAR3, FB layout from the WPR meta, region table, options for later (subagent report) |
| [gsp-abi-610.md](gsp-abi-610.md) | Compiled sizes and offsets of the GSP message-queue and RPC structures for 610.57.04 (`caraxes/tools/gsp-abi-sizes.sh`), what the kernel reads from `GspStaticConfigInfo`, the receive-side checks (subagent report) |
| [phase5-rpcs.md](phase5-rpcs.md) | Ordered inventory of the 98 RPCs after `GET_GSP_STATIC_INFO` up to `RmInitAdapter succeeded!`, plus `nvidia-smi`, with an answer source per RPC and the five hardest (subagent report) |
| [passthrough-procedure.md](passthrough-procedure.md) | The prepared procedure for R1/R3 (SSH/tmux, unbind, vfio-pci, QEMU trace, recovery, watchdog), with the verified host preconditions and blockers (subagent report) |
| [fifo-device-info.md](fifo-device-info.md) | The FIFO device-info table for TU106: field semantics, engine descriptors, host-derived and synthesized values (subagent report) |
| [cuda-path.md](cuda-path.md) | Design for CUDA on the stock driver: what nvidia-uvm needs, the first context's RM objects, the execution model (identity-mapped VRAM plus host-created channels), the guest-DMA versus host-IOVA problem, the E5 ladder (subagent report) |
| [host-facts.md](host-facts.md) | Read-only host facts H1-H8 and the VRAM reservation experiment: IOMMU domain DMA-FQ, BAR1 256 MiB fixed, fixed-address contiguous VRAM up to 5 GiB without privileges, the reservation rule (subagent report) |
| [uvm.md](uvm.md) | What nvidia-uvm sends at registration, the channel and copy-engine emulation, the HMM condition, what blocks E5b (subagent report) |
| [sysmem-iova.md](sysmem-iova.md) | Measured: one host OS descriptor of guest RAM gets one contiguous IOVA range; the guest-side `dma_range_map` module as the temporary aid (D13); what needs the operator's word (subagent report) |
| [../../guest-module/caraxes_dma_range/README.md](../../guest-module/caraxes_dma_range/README.md) | The temporary guest aid (D13): parameters, validation rules, driver-core interplay, load order, what the device must produce and translate back, kernel-version notes (subagent report) |
| [../../patches/ACPI-DMA.md](../../patches/ACPI-DMA.md) | The ACPI `_DMA` route to replace the guest module (P8): mechanism, Cloud Hypervisor patch 0004 with its AML bytes, the x86 Linux draft, the DMA_MAP-before-DSDT ordering problem and its fix, test plan, upstreaming notes (subagent report) |
| [memory-model.md](memory-model.md) | E5b part 1: guest FB addresses inside a fixed host VRAM reservation, guest RAM registered as host OS descriptors, the IOVA table for `caraxes_dma_range`, the device-side translation, privilege and teardown |
| [host-exec.md](host-exec.md) | E5b core: the standalone host-execution primitive ([caraxes/tools/host-exec.rs](../../caraxes/tools/host-exec.rs)) that binds a host channel to a page directory the client built in reserved VRAM and proves the physical GPU walks it (semaphore release and a CE VRAM->sysmem copy verified); the part-2 device wiring spec per guest event; measured status codes and refusals (subagent report) |
| [execution.md](execution.md) | E5b part 2: execution on the physical GPU in the device. The VRAM-backed `FbStore` (option C) with BAR1 windows and the write-combine flush finding; the mirror objects (VA spaces incl. the replayed shared-management form, TSGs, channels, CE objects, doorbell, non-stall events, TLB invalidates); teardown; the two execution walls (the CE first-launch stall, the GR context on a host channel) with the measured contained Xids; the P7 progress-gated interrupt design (subagent report) |
| [dma-routes.md](dma-routes.md) | Guest DMA addressing: the matrix of every route (module, ACPI `_DMA`, shadow page tables, host-side tricks) against what each one patches; why the host side cannot match; the shadowing design that needs no guest change (operator's question 2026-09-23) |
| [host-gr.md](host-gr.md) | Analysis (not measured) for the execution walls: PBENTRY cause (stale write-combined BAR1 data, confirmed on vm2), the CE-write-into-reservation stall as compressible PTE kind over COMPR-NONE backing, the server-reserved PDE mechanism for a host-built GR context, the guest's exact kernel bytes and QMD for a replay; the supervised test matrix (subagent report) |
| [exec-formats.md](exec-formats.md) | Formats behind the execution walls, from the public Turing manuals, RM and nouveau: every PBDMA INTR_0 cause (the recorded a2 Xid 32 is DEVICE, the UVM SW method, not PBENTRY) and the mirror doorbell checklist; GMMU v2 entry layouts, dual-PDE precedence and 20 divergences of the `fb.rs` walker; the RM flush/invalidate sequence after BAR writes; server-reserved PDEs, who maps the GR context buffers in GSP-client mode, and why a host-built context on a replayed VA space cannot be reached through the guest root (subagent report) |
| [briefing-display-nova.md](briefing-display-nova.md) | The operator's side assignment of 2026-09-23 evening: DisplayPort sink, NVIDIA display adapter, Nova-core compatibility and present backend as four packages with gates D0-D6 and N0-N4 (operator's document, condensed verbatim) |
| [nova.md](nova.md) | Package C of the side assignment: nova-core (Linux v7.2 `8d3ae59288f1`, firmware 570.144 from linux-firmware `20260916`) against the device. What the TU106 probe reads and waits for, 15 differences (VBIOS signature and app id, r570 element framing, the CPU sequencer, static-info layout, unload, re-probe), the r570.144-to-610 ABI diff with the size of a `ver/v570.rs`, the guest recipe, gates N0-N4 and 5-7.5 agent-days (subagent report) |
| [briefing-naming.md](briefing-naming.md) | The operator's naming/configuration refactor brief (platform, transport, personality, profile, backend; `Leandro <Transport> <Personality>-<Profile>`; backend never guest-visible; v1's 4Q and 1333MiB profiles kept) and the coordinator's plan for it |
| [versatility.md](versatility.md) | The three axes after the D14 refactor (chip, board, driver version), where each fact lives, what stays hard-coded, how to add a chip or a driver version (subagent report) |
| [nvml.md](nvml.md) | The controls `nvidia-smi` and `nvidia-smi -q` send, forwarded or synthesized, the `nvml` gate, the proposed minimal set |
| [roadmap.md](roadmap.md) | Done, in progress, next and later, as of 2026-09-24 |
| [testing.md](testing.md) | The Leandro v1 gates against the device: what `tools/caraxes-vm.sh sync-tests` copies from ../Leandro-Test into the guest, the `cuda`, `gpu`, `vdisplay` and `display` gates of `tools/caraxes-gate.sh`, the v1 verdict rules, the per-test results and the CUDA-path target list |
| [cuda-lookahead.md](cuda-lookahead.md) | Checklist for the CUDA gate: every RPC, control and allocation the CUDA path sends to the device, per phase (cuInit to teardown), classified A-E (answered, forward, forward with translation, synthesize, needs execution) with trace and source evidence; the bookkeeping gaps in `channel.rs`; the minimal sets for cuInit/cuCtxCreate, for cuMemAlloc/cuMemcpy on the software copy engine, and for the kernel launch (subagent report) |
| [display-path.md](display-path.md) | What the `vdisplay`/`display` gates need, what a display engine in the device would have to serve on Turing (C57x classes, RPCs, BAR0 pages, instance memory, notifiers), the GRID displayless HAL and its catches, the candidate routes with effort and risk, and the first experiments D0/D1 (subagent report) |
| [display-connector.md](display-connector.md) | Route E cut to one emulated DVI-D single-link TMDS connector and one head, Wayland-first: the NV0073/RPC answers and EDID, how vblank and flip completion reach the guest on Turing (AWAKEN and `LAST_DATA`, NV9010, no DRM vblank), the device timer that paces flips, why v1 ran unthrottled under Wayland, scanout and de-tiling cost, phases A-D at 9-14 agent-days (operator's question 2026-09-23) |
| [display-scaling.md](display-scaling.md) | Second display study: the connectors compared for an emulated link (DVI-D, HDMI, DP SST/MST, eDP, virtual), the guest-side limits vs the device-answered ones (IMP, caps page, DPCD), 4K144 arithmetic (DP HBR3 x4 plus DSC), multi-head (4 heads, 8 windows), NVENC and VRAM as the real limits, vGPU comparison, product view, phases E-H to 4 x 4K144 at 17-27 agent-days total (operator's question 2026-09-23) |
| [display-d0.md](display-d0.md) | D0 run on vm0: nvidia-modeset loads headless with zero display RPCs (confirmed on the whole trace, not just the run's own window); nvidia-drm fails to build against the guest kernel's DRM headers; Xorg untested, no NVIDIA X driver in the guest image; what D1 needs first |
| [display-adapter.md](display-adapter.md) | Side package B: where DP AUX and every NV0073/NV5070/NVC370/NVC372 control on the DP connector, EDID, link, head and modeset paths terminates (local, GSP_RM_CONTROL, local plus internal control; none by MMIO), with ids, struct sizes and consumed reply fields; the SST EDID lifecycle and per-displayId state; class list and HAL selection; channel allocation, BAR0 pages, C57D/C57E method basics and the minimal decoder; the ordered phase A/B checklist to the first EDID read; gates D0-D6 per phase; the three riskiest ABI details |
| [display-e.md](display-e.md) | Display phase E on vm0 (2026-09-24): the stock NVIDIA EGL/GL/Vulkan stack rendering on the physical GPU through the shadow GR path, Weston and GNOME on it, the display gate 11/11 with host execution; series E (e1-e31, reconstructed from its evidence) and series F: the classic demos (Weston glmark2 1804, vkcube 44 FPS), the per-frame trap table (Weston 157, Mutter about 670 traps per frame, mostly interrupt servicing; the doorbell ioeventfd removes the 3 doorbell traps and does not move Mutter), the stream stage (Sunshine kms + x264, Moonlight 25.9 FPS), the virtio-input pointer (`CARAXES_INPUT=fifo`), what the NVENC mirror would change |
| [display-ab.md](display-ab.md) | Display phases A and B on vm0 (D15): the display in the GSP role (`--display`; `src/display/`, `chips/tu106/display.toml`, the board's `[display]` connector, the `crates/dp-sink` sink behind an NV0073 AUX adapter); gate D3 passed (`card0-DP-1` connected, EDID byte-identical and edid-decode clean, 1080p60/720p60/1080p50, the over-budget 8K120 VIC pruned, no modeset); every display RPC/control in order with its answer and label (D2); the recorded AUX capture replaced the hand-written transcript; what phase C needs (core notifier, window completion, four modeset-path controls) |
| [experiments.md](experiments.md) | The experiment ladder, starting with the smallest one, each with a falsifiable expectation |
| [kickoff.md](kickoff.md) | The brief for the implementation sessions: rules, reading order, E0 step by step, run recipe, parked ideas |
| [parallel.md](parallel.md) | P20, the parallelism proof: two reserving VMs in fixed slots run the cuda gate concurrently beside the desktop; isolation by construction and by pattern test, the host's scrub on free proven for the reservation, fail-closed teardown, the D11 refusal of a third VM; the BAR1 leak and deadlock found and fixed, and two open sysmem-path findings (mapping scale, the chunk-crossing hole) |
| [mapping.md](mapping.md) | The mapping package (2026-09-24): incremental shadow walk with write-through table copies, coalesced RM maps with a per-device budget, resync on a worker thread with a doorbell barrier, IOVA = GPA + one offset and CE channels on the filtered shadow (the two P20 §8 fixes), the BAR aperture TLB, the doorbell and context-creation accounting (96k trapped BAR2 writes per cuCtxCreate), the CeUtils experiment (§9: the guest driver does not load without `RmDisableGlobalCeUtils=1`, keep the key) |
| [nova-vgpu-upstream.md](nova-vgpu-upstream.md) | The upstream nova-core vGPU manager series (v2, 32 patches, 2026-09-14) read patch by patch against the device: plugin ABI and RPC discipline, VRAM/channel allocation, CeUtils scrubbing and fail-closed teardown, VMMU segment, FIFO engine table, VF routing, migration, logging; ranked actionable differences and a cross-check of the GSP message queues (subagent report, P19) |
| [blackwell-gb206.md](blackwell-gb206.md) | The Blackwell lab capture (RTX 5060 Ti, GB206, driver 615.71.09, korngold, 2026-09-24, filed under `caraxes/reference/gb206/`) read item by item against TU106; the D14 work list for a GB206 identity with five draft chip tables under `chips/gb206/`; the 610.57.04 driver supports GB206 (gsp_ga10x firmware, boots GSP through FSP, no ROM parsing); 615.71.09 redesigns the CPU-GSP transport (queue v2 in registers, MCTP-framed elements, one GMCAPI init instead of the four init RPCs) and changes seven control structs; the 615 gsp-abi-sizes record; a nine-step N-gate plan of 27 to 39 days that stops at "driver up" without a Blackwell card here (subagent report) |
| [guest-driver-versions.md](guest-driver-versions.md) | Other guest driver versions against the 610.57.04 device (2026-09-24, vm3): Ubuntu's recommended 595.91.07 stops at the first queue element (pre-610 element header) and comes up with three `CARAXES_GUEST_ABI=595` switches; 610.43.02 works as is; 615.71.09 stops at the status-queue link, with the captured GMCAPI_CMD_GSP_INIT; size table, version detection, D14 estimate (subagent report) |
| [bar1-passthrough.md](bar1-passthrough.md) | P21: what the guest does with BAR1 (4-byte GPFIFO and GPPut writes at one traced layout, tables through BAR2), Cloud Hypervisor v53's sparse-area mechanics, the NVIDIA mmap rule and why an RM mapping fd is unsafe, the dma-buf export as the region descriptor, rejected candidates, target design H (private pool, guest pages remapped), prototype G behind `CARAXES_BAR1_PASSTHROUGH=1` with its checks, measurement plan and results (subagent report) |
| [bar1-pool.md](bar1-pool.md) | P21 design H behind `CARAXES_BAR1_PASSTHROUGH=pool` (`CARAXES_BAR1_POOL_MIB`, default 64): a private pool below the carve-out, taken out of the guest's usable FB, backs guest BAR1 `[0, pool)` as one dma-buf sparse area; every page the guest maps there lives at its pool page in every device translation (FB store, shadow leaves, host USERD; `FakeFb::remap`), copied in at map and back at unmap; pages the GPU maps move only behind the idle-and-preempt barrier with a shadow re-sync, USERD pages and failed barriers are refused and retried (fail closed); isolation argument, measurement plan against the 1 MiB area (copy, rl, ppo, short-auto, P21 checks), unit tests (subagent report, not yet run on a VM) |
| [bar2-memfd.md](bar2-memfd.md) | D18's BAR2 line behind `CARAXES_BAR2_MEMFD=1`: region 3 as a sealed memfd with sparse areas around the trapped flush page; FB pages mapped through BAR2 live in the memfd (`FakeFb::backing`), synced at the RM's flush reads, MMU invalidates, BAR2 binds and GSP doorbells by a snapshot compare that writes back only the changed bytes and marks the shadow's walk-cache pages dirty; why soft-dirty, userfaultfd write-protect and mprotect on the device's mapping cannot see guest writes (KVM dirty logging would, with a VMM change); what stays trapped, the hazards, fail-closed setup, tests, measurement plan (subagent report) |
| [gsp-transport-615.md](gsp-transport-615.md) | The CPU-GSP transport of driver 615.71.09 from source (NV_ABI_STABLE): message queue v2 with the pointers in NV_PGSP_QUEUE_HEAD/TAIL(0) and MSGQ_HEAD/TAIL(0), the 24-byte MCTP-framed element without checksum or sequence number and what replaces them, GMCAPI and GMCAPI_CMD_GSP_INIT with its NVKV request and response key by key, the 615 boot arguments, what stays as in 610, the seven changed control structs and the reused id 0x20800A36, how the device tells the version, a 610 against 615 table for every structure the device touches, the device's v2 transport (`msgq::v2`, `gmcapi`, `ver::v615`, `CARAXES_GUEST_ABI=615`) and its tests, with the generated 615 gsp-abi-sizes tables (subagent report) |
| [repo-split.md](repo-split.md) | The plan for separating v1 (Leandro) from Caraxes after the operator's 615 decision: inventory of every path and crate (v1-only, Caraxes-only, shared, with a corrections table for paths the fork no longer has), the port-back commits Leandro must receive (the rewritten vhost-user series, the P17 naming model, one nvrm-abi hunk), the shared-crate options with the recommendation B (shared crates stay in Leandro, Caraxes pins a Leandro commit as a git dependency), the step order that keeps both repositories building, the 615 switch placed in it, and eleven silent-failure risks with a command checklist (subagent report) |
| [blackwell-gate.md](blackwell-gate.md) | The Blackwell N gate in pure emulation: the GB206 identity (`--board-data rtx5060ti`, `CARAXES_CHIP=gb206`), stages N0 (device) to N4 (first channel allocation), the predicted walls, preconditions and the exact vm3 launcher command; defined 2026-09-24, not run yet |
| [cutover-615.md](cutover-615.md) | The checklist for the host driver upgrade 610.57.04 to 615.71.09 (2026-09-24): the device's run-time host-ABI selection (`hostabi.rs`, `CARAXES_HOST_ABI`, the log line to look for), then in order the 610 closing benchmark and frozen binaries, the operator's upgrade checks, the 610 guest on the 615 host (nvml, cuda, display gates), the 615 guest once `ver/v615` lands (`CARAXES_DRIVER=615`), v1's wait for L615, rollback, open points |
| [v1-615-image.md](v1-615-image.md) | The Leandro v1 guest image for 615.71.09 (`guest-v1-2026-09-24-615.71.09.qcow2`, made from the first v1 615 run's overlay without booting): what differs from the Caraxes 615 base image (guest nvidia modules blacklisted, `nvidia-modprobe` diverted, `nvidia-persistenced` masked, `/opt/nvrm` on the 615 system libraries), why not nix offline, `bench-v1vm.sh` taking v1's material from Leandro (`LEA_V1_ROOT`, `LEA_V1_BIN`, `NVRM`, driver selection, dry run), `LEA_MANAGED_COMPAT=1` as in the gpu gate, `v1-gates.sh`, the boot command |
| [gsp-surface.md](gsp-surface.md) | P23, the GSP surface of 610.57.04 from source: 829 of 1377 exported controls reach GSP, the device handles 171 with D9 provenance, the 658 unhandled sorted into five categories (pure chip data, host-forwardable, guest-state derived, needs modelling, never expected on TU106) with a recipe and estimate each, the 20 RPC functions and 41 events a GSP client uses, the top ten a Linux CUDA or display client hits next, what Windows and nova-core share; produced by `caraxes/tools/gsp-surface.py` (subagent report, 2026-09-24) |
| [gsp-plan/](gsp-plan/README.md) | The GSP plan on 615.71.09 (one README row per document): RPC functions (14 of 232 used by a 615 GSP client on TU106, 7 answered, `ALLOC_MEMORY` sent at every boot and refused), events (45, the device raises only POST_EVENT for hotplug; contained recipes for host-mirrored errors), classes reaching GSP (143 on the wire, 48 on TU106, only the TSG params changed), the boot exchange with teardown, suspend/resume and what `NV_ABI_STABLE` covers; gaps, recipes, truth sources, isolation, open questions |
| [doorbell-poll.md](doorbell-poll.md) | The doorbell reactor (`CARAXES_DOORBELL_POLL=off\|spin\|hybrid`, `_POLL_CPU`, `_POLL_IDLE_US`): one thread polls the token pool's eventfds instead of sleeping in `epoll_wait`, pinned to an automatically chosen core; the vm1 measurement of blocking, spin unpinned, spin pinned and hybrid (short kernel 0.026 to 0.014 ms, rlprobe stage 4 2.7 to 2.1 s, hybrid at 3 % of a core), host tuning recommendations for the reactor core, and what P24 would change |
| [workload-gate.md](workload-gate.md) | The workload gate (2026-09-24, prepared, not run): real workloads beside the micro rows, native against v2 (v1 on a 610 host only): PPO CartPole at 420 s, llama.cpp pp512/tg128 with and without CUDA graphs (host-built llama-bench for sm_75, Llama 3.2 1B Q4_K_M), a GPT-style training loop, ResNet-18 training; the one-command runner `caraxes/runs/workload-2026-09-24/run.sh`, how to read the ratios, the table to fill |
| [intr-page.md](intr-page.md) | The interrupt page (`CARAXES_INTR_PAGE=1`, `_SLEEP_MS`, `_RETRIGGER_MS`): BAR0 page 0 and the VF CPU interrupt tree page offered as sparse mmap areas of region 0 from a memfd, the reactor applying the guest's acks and enable writes by polling (tag bits and sentinels so every store is visible, a retrigger watchdog for what stays ambiguous); per-frame register census of series F (437 of 662 Mutter traps are these two pages), the expected 662 to about 225, what stays trapped |
| [alloc-path.md](alloc-path.md) | Where cuMemAlloc/cuMemFree time goes (830/903 us, 6.7 ms for 64 MiB): no memory RPC reaches the device, the cost is the guest RM's trapped traffic (24 traps per mapping change: 18 RM-lock interrupt-enable writes, 5 MMU invalidate, 1 flush; plus BAR2 PTE writes), the device's host calls sit in the worker; the four steps (existing intr page + BAR2 memfd, lazy shadow plans, invalidate and flush page as memory, region-object free list) with expected cost and isolation argument; `CARAXES_ALLOC_LAZY=1` built (`lazy.rs`), `caraxes/tools/rpc-cost.py`, `BENCH_CU_PHASE_GAP_MS`, measurement plan (subagent report) |
| [open-questions.md](open-questions.md) | Unresolved questions, numbered |

Reference dumps of the physical RTX 2070 live in `caraxes/reference/rtx2070/`
(config space, capabilities, the PCI-visible ROM images, `nvidia-smi -q`). Experiment
evidence (device traces, guest logs, versions) lives in `caraxes/runs/<experiment>-<date>/`.
The device crate is `crates/vfio-user-nvgpu`; the launcher is `tools/caraxes-vm.sh`.

## Findings that shape the design (2026-09-23)

1. **Host.** RTX 2070 (TU106, `10de:1f02` rev A1, subsystem `1458:4011`), open kernel
   modules 610.57.04 with GSP firmware active, kernel 7.2.4, QEMU 11.1.1 with the
   `vfio-user-pci` client device. The host GPU drives the only display. Details: [environment.md](environment.md).
2. **The stock open driver on a Turing identity has exactly one viable mode: GSP client.**
   The runtime variant is chosen from three BAR0 registers (`NV_PMC_BOOT_0/1/42`).
   The vGPU-VF variant (which would have given us a ready-made paravirtual RPC transport
   with no firmware boot) is compiled in but dead for Turing and Ampere: the VF doorbell
   and scratch-register HAL entries are stubs for TU10X and `PDB_PROP_GPU_VGPU_OFFLOAD_CAPABLE`
   is false below AD10X, so `initRpcInfrastructure_VGPU` fails and GPU construction aborts.
   The monolithic (non-GSP) variant does not exist in the open tree for Turing.
   Evidence: [driver-modes.md](driver-modes.md).
3. **GSP-client mode means the guest expects to boot firmware it cannot run here.**
   The CPU side loads FWSEC (from the VBIOS) into the GSP falcon, Booter into the SEC2
   falcon, and waits for halts, WPR2 registers, scratch values and a message-queue
   header. None of the firmware has to execute: the device only has to produce the
   CPU-visible side effects. The full ordered list is in [gsp-boot-turing.md](gsp-boot-turing.md);
   the emulator needs 2 falcon register-block state machines (GSP at 0x110000, SEC2 at
   0x840000), a PROM window at 0x300000 serving a VBIOS image, and ~10 status registers.
4. **After boot, the transport is a plain shared-memory ring in guest RAM** (command
   queue at +0x1000, status queue at +0x41000 of a 0x81000-byte buffer whose page table
   the guest hands over through MAILBOX0/1 → libos args → RMARGS). The guest polls for
   `GSP_INIT_DONE` (seq 0), then sends `SET_GUEST_SYSTEM_INFO` and `GET_GSP_STATIC_INFO`.
   The reply to the latter (`GspStaticConfigInfo`: FB regions, internal client handles,
   engine caps, GID) is the first RPC that determines everything afterwards. Details:
   [gsp-rpc-protocol.md](gsp-rpc-protocol.md).
5. **The device never needs to run the CPU sequencer.** In 610.57.04 the
   `GSP_RUN_CPU_SEQUENCER` event is unhandled; the only event required for init is
   `GSP_INIT_DONE`.
6. **Leandro's executor is reusable but ioctl-shaped.** `Session::prepare/execute`,
   `host_pool`, `mirror`, `vram` and the `nvrm-abi` translation tables take `nvrm-wire`
   messages carrying NVOS64/NVOS54 envelopes. GSP RPCs carry params inline
   (`rpc_gsp_rm_alloc/control`), a small set of controls arrive FINN-serialized, and the
   guest's kernel RM issues internal (`*_CTRL_CMD_INTERNAL_*`) controls that an
   unprivileged host RM client cannot forward. A shim and a policy for internal controls
   are required. Details: [leandro-reuse.md](leandro-reuse.md).
7. **Guest RAM must be fd-backed (memfd, share=on) for QEMU to pass DMA regions as
   file descriptors.** The rust-vmm `vfio_user::Server` does not implement
   server-initiated `DMA_READ/WRITE`, so the emulated GSP reads the queues by mmap.
   Details: [vfio-user.md](vfio-user.md).

8. **The kernel RM reads PCI config space through a BAR0 mirror at 0x88000** and
   keeps the MSI/MSI-X enable bits there in sync with the real capability; it never
   programs BAR1/BAR2 bind registers or fault buffers (those HALs are stubs in the
   kernel-only variant), but it does bootstrap and verify BAR2 page tables through the
   PRAMIN window, runs an interrupt self-test through the 0xB8xxxx tree, and starts a
   watchdog channel plus a copy-engine self-test that the module parameters
   `RmRcWatchdog=0` and `RmDisableGlobalCeUtils=1` disable. Milestone 1 therefore needs
   a fake FB with a GMMU page-table walker for BAR2, but no real GPU work.
   Details: [kernel-rm-hw-surface.md](kernel-rm-hw-surface.md).

9. **Identity comes from the profile policy Leandro already has.** `vram.rs` decides
   `fb_length`/`size`/`reservation` from `LEA_VGPU_TYPE` and friends, `vgpu.rs` derives the
   catalogue (`RTX2070-4Q`), and `guest_card_name` produces `Leandro RTX 2070-4G`. Under
   GSP the guest's name, FB size and UUID controls are kernel-served from
   `GspStaticConfigInfo`, which the device fills, so the presentation needs no reply
   rewriting. Details: [identity-profile.md](identity-profile.md).

## Decisions

| # | Decision | Why | Status |
|---|---|---|---|
| D1 | Target GSP-client mode (PF_KERNEL_ONLY variant), Turing identity `TU106` | Only variant the stock open driver can run on a Turing identity; matches the physical GPU so userspace compiles for SM 7.5 | taken 2026-09-23 |
| D2 | Do not present an Ada+ identity to reach the vGPU-VF transport | Class lists, MMU/fault formats and SASS targets would mismatch the physical TU106 | taken 2026-09-23 |
| D3 | vfio-user server in Rust on `vendor/vfio/vfio-user` (rust-vmm, v0.1.6); **Cloud Hypervisor v53 (`--user-device`, `--memory shared=on`) is the client**, QEMU only as fallback and for the passthrough reference capture | Keeps the Leandro stack (pinned Cloud Hypervisor); its client supports regions, sparse mmap, INTx/MSI/MSI-X and fd-based DMA maps; QEMU alone can trace MMIO of the real card | taken 2026-09-23 |
| D6 | The card presents itself as `Leandro <board>-<size>` with the framebuffer size from Leandro's existing profile policy (`LEA_VGPU_TYPE`, `LEA_VRAM_PROFILE_MIB`, …), at the device level (FB-size register and GSP static info), not by rewriting RM replies | Same operator interface and names as v1; the guest RM natively believes the smaller card, so the v1 rewrites and the VIDMEM ledger cap become unnecessary | taken 2026-09-23 |
| D4 | Keep `vendor/open-gpu-kernel-modules` at 610.57.04 | Must match the running host driver (`tools/build.sh check-driver`); 615.71.09 exists upstream and can be fetched when the host moves | taken 2026-09-23 |
| D8 | E1 uses a synthetic VBIOS (captured PCI chain plus a constructed FWSEC extension image that passes the driver's parsers) instead of a passthrough capture | Passthrough would take down the desktop and the agent session running on it; nothing from the VBIOS executes on the device, the kernel RM only parses it | taken 2026-09-23 |
| D9 | Everything the device passes to the guest carries a provenance label (host RM reply, profile, synthesized constant, guest-written), and the first E3/E4 implementation passes host information through generously; the cut down to the necessary set comes afterwards, from the documented labels | Gives real per-VM isolation on the GPU a basis: the minimal set is derived from evidence, not guessed; the labels double as documentation | taken 2026-09-23 (operator) |
| D10 | One device process: `vfio-user-nvgpu` talks to the host RM itself (through `/dev/nvidiactl` as an unprivileged client, using Leandro's crates as libraries: `nvrm-client`, `nvrm-abi`, `nvrm-sys`, the `vhost_user_nvrm` library). The vhost-user `virtio-nvrm` device and its guest module are not part of a Caraxes VM; if any step needs the running `vhost-user-nvrm` process, that is temporary and marked as such | The operator wants a single synthetic device, not a Leandro client on the side; the reusable value of v1 is its RM translation and policy code, not its transport | taken 2026-09-23 (operator) |
| D11 | For real execution the device reserves a fixed-address, contiguous range of the card's VRAM for the VM (fb-model option C), sized to the profile; odd sizes (2.5, 3, 5 GiB) are tested; the reservation is refused when the host would be left without enough free VRAM (the host desktop must never run out) | Guest FB addresses equal real VRAM addresses; the guest keeps a fixed share, like a vGPU profile | taken 2026-09-23 (operator) |
| D12 | The device may run as a privileged host RM client (Leandro's `LEA_ADMIN_PRIV` policy) for the admin-only controls real execution needs (`DMA_SET_PAGE_DIRECTORY`, `GPU_PROMOTE_CTX`, `GET_SURFACE_PHYS_ATTR`) | No other way to point a host channel at the guest's page tables | taken 2026-09-23 (operator) |
| D13 | Guest DMA addresses are made equal to host IOVAs through a temporary guest-side aid, documented with a removal plan ([cuda-path.md](cuda-path.md) §3, S1); the only guest-side patch in the plan | Without it the physical GPU would read host memory at guest addresses | taken 2026-09-23 (operator) |
| D14 | Implement for this card (TU106, 610.57.04) but structure everything so it can serve other driver versions and other silicon later: driver-version-bound pieces (RPC and control structures, control ids, static-info layout) generated per vendored driver version behind feature flags like `crates/nvrm-sys`; silicon-bound pieces (boot model, register map, engine and FIFO tables, interrupt table, identity) behind a chip trait with data tables per chip, TU106 being the first implementation; nothing version- or chip-specific hard-coded in the generic paths | The operator wants the Turing device to be the first of several, and a Blackwell lab machine is in reach | taken 2026-09-23 (operator) |
| D15 | The emulated display connector is DisplayPort SST (one per head, no MST, no HDCP, DSC offered), per [display-scaling.md](display-scaling.md); the connector-agnostic phase A and the timer-gated flip cadence of [display-connector.md](display-connector.md) stay, phase B is done directly with DP (EDID over AUX, DPCD model, link-training answers) instead of a DVI-D detour. Heads are synthetic: the device declares the head, window and SOR counts and the connector table itself, independent of the physical card, up to the guest driver's own cap for the emulated display IP (4 heads on Turing's v04.00). | The operator wants DP for its headroom (4K144 with DSC, 10 bpc, VRR) and guest-visible heads decoupled from the silicon; nothing scans out physically, so the physical head count is irrelevant | taken 2026-09-23 (operator) |
| D16 | **No guest comptag line ever reaches the host.** Layers: (1) policy in the guest's view: every FB/memsys/caps answer says no compression (region `supportCompressed=0`, `disable_compbit_backing`, zero comptag lines), so the stock guest never chooses compressible kinds; (2) enforcement in the device: a PTE write filter on the VRAM store that tracks page-table pages from the roots the guest registers and forces every vidmem PTE to an uncompressed kind with COMPTAGLINE 0, plus refusal of compression attributes in mirrored allocations; (3) audit: a sweep of the tracked page-table pages after each gate run asserting no compressible kind and no comptag line; the reservation itself is never allocated with `NVOS32_ATTR_COMPR`. Physical execution of guest work is allowed only under (1) verified by trace until (2) exists, then under (2). Recovery from a violation is a host reboot (comptag store reset). Host-side switch: none found in the open kernel RM (compression policy lives in GSP firmware). | The guest's COMPTAGLINE field indexes the host's global comptag store that the desktop's own compressed surfaces use; a stray write would corrupt host state outside our channels (found 2026-09-23 evening) | taken 2026-09-23 (operator: "wie machen wir das sicher, das ist wichtig") |
| D17 | **Route 4, shadow page tables in the device, is the DMA route, and the guest stays bare metal.** The guest runs the stock driver in its native configuration: page tables in VRAM, where the RM and UVM place them by default (`kern_gmmu.c` PDE/PTE aperture `ADDR_FBMEM`; `uvm_mmu.c` default `UVM_APERTURE_VID` with a sysmem fallback). The only configuration deviation is disabling that fallback (`uvm_page_table_location=vid` and the matching `RmInstLoc` fields, Q32); never vGPU-style driver behaviour (no SR-IOV or paravirtual code paths in the guest). The device keeps the translated tables in place in the VRAM reservation and the untranslated guest view in host RAM, so shadowing costs no VRAM beyond the guest's own tables (8 bytes per PTE: 2 MiB per GiB mapped at 4 KiB pages, 128 KiB at 64 KiB, 4 KiB at 2 MiB; single-digit MiB per context). A host-RAM shadow (the driver's `sys` location, refreshed at each TLB invalidate) is the fallback for tables the guest still places in sysmem, not the design. Built in two stages after CUDA runs: (1) a validating PTE filter (vidmem inside the reservation, sysmem inside the guest's registered IOVA ranges, no compression; the same component as D16 layer 2), guest module kept; (2) the translating filter with invalidate hooks and the UVM pushbuffer scanner, after which the guest module and the `_DMA` patches are dropped. Routes 2 and 3 of [dma-routes.md](dma-routes.md) stay evidence, not targets. | The only route that is both vanilla (stock Cloud Hypervisor, stock guest kernel) and isolated (the device controls every address the GPU uses). The VRAM question is settled: tables in VRAM are the native default and NVIDIA's own SR-IOV-heavy vGPU asserts it. Bare-metal behaviour in the guest is the point: with it Caraxes is better than vGPU, not a copy of it. | taken 2026-09-23 (operator: stay as close to bare metal as possible, not to the vGPU driver; with that we are better than vGPU) |
| D18 | **Target memory model: VRAM slice + sparse sysmem registration + validated shadow tables.** The physical GPU may reach only this VM's VRAM reservation and guest-RAM pages the device has explicitly registered; guest GPU page tables are shadowed and validated, and the guest writes guest physical addresses (GPA), the device translates (GPA is never equal to the GPU's DMA address). Registration becomes sparse and demand-driven: when a validated guest entry first points into a 2 MiB guest-RAM region, the device pins and registers that region (a cached, refcounted mapping object: guest GPA, length, host VA, host RM handle, GPU DMA address, permissions), reuses it across submissions, and releases it with hysteresis when the last entry disappears; the same path serves UVM (GPU fault forwarded to the guest, guest decides residency, device registers the range, updates the shadow, invalidates, replays). Today's registration of the whole guest RAM in 1 GiB chunks (D13) stays as the alpha bootstrap and is not refactored while it risks the working CUDA path; order: the mapping package (write-through shadow, coalesced maps, budgets), then all channels on the filtered shadow and IOVA = GPA + offset (P20 section 8), then sparse registration, after which the guest DMA module and the `_DMA` patches are dropped (the vanilla gate of D17). | The security boundary is the same in both models (this VM's RAM and slice); sparse registration adds defence in depth (a filter bug exposes only registered pages), removes the 4 GiB of pinned host memory per VM, removes the chunk-crossing class of bugs by construction, and makes the guest module unnecessary. Operator's direction of 2026-09-24. | taken 2026-09-24 (operator) |
| D19 | **Baseline driver 615.71.09 before the GSP-surface work (operator, 2026-09-24)**: from 615 on NVIDIA tags the GSP transport and boot structures `NV_ABI_STABLE` (nvtypes.h: a marker for static checkers to verify no changes across releases; carriers: `msgqTxHeader`, `GSP_MSG_QUEUE_ELEMENT`, `GMCAPI_HEADER`, the `GSP_FMC_*` and `GSP_RM_*` boot parameters), so the transport layer the device emulates is meant to stay fixed from there. Order: (1) the device speaks the 615 transport (queue v2 with register-held pointers, MCTP-framed elements, one `GMCAPI_CMD_GSP_INIT` with NVKV data instead of the four init RPCs; `ver/v615` from the generated `nvrm-sys` v615; behind a version switch, 610 unchanged), verified with a 615 guest in pure emulation; (2) the guest images move to 615.71.09 (v2 and, because it forwards ioctls verbatim, v1); (3) the operator upgrades the host driver to 615.71.09 in a window with every VM down, then the host-side client moves to `nvrm-sys` v615; (4) only then P23, the GSP-surface work, on the 615 inventory. What "stable" covers: the transport, boot and init layer; the control parameter layouts still differ per version (seven structs and one reused id between 610 and 615, [blackwell-gb206.md](blackwell-gb206.md) §3) and stay with D14's generated modules. | 2026-09-24, operator's decision |
| D5 | First experiment is a logging-only device (PCI identity, BARs, ROM, boot IDs), not a GSP model | Establishes the toolchain and gives the first observed failure for free | taken 2026-09-23 (go-ahead given) |
| D7 | Device crate is `crates/vfio-user-nvgpu` (binary `vfio-user-nvgpu`), depending on `vfio_user` 0.1.6 from crates.io | Follows the `vhost-user-nvrm` naming; the git-ignored `vendor/vfio` checkout is for reading, a path dependency would break clean checkouts | taken 2026-09-23 |

Planned refactor after E5a (from D14): a `chip` module with a trait for the boot model,
register names, FIFO and engine tables and identity defaults; the TU106 tables moved from
code into per-chip data files; the RPC and control layouts moved to a generated per-version
crate (`gsp-abi-610.md` §e). Every new implementation brief cites D14.

## Night-shift backlog (operator, 2026-09-23)

| # | Item | Rule |
|---|---|---|
| N1 | Research first, then write the Cloud Hypervisor patch that emits the ACPI `_DMA` method for the PCI host bridge, from the ACPI specification, plus the matching x86 Linux change that honours `_DMA` (arm64's `acpi_dma_get_range` as the model); this removes the guest DMA-range module (P8) | new files only under `patches/` and `docs/caraxes/`; nothing applied to `vendor/` in place; patches must apply and build |
| N2 | Rewrite Cloud Hypervisor patch 0001 (generic vhost-user shared-memory window) against the vhost-user specification as written, not the current half-correct version, and adapt Leandro core (`crates/vhost-user-nvrm`, the guest module) so v1 keeps working with the corrected patch; same for 0002 and 0003 if the review finds spec deviations | research and design at night, implementation only with the notes explaining every deviation from today; existing Leandro functionality must not break |

## Ideas recorded, not planned

| # | Idea | Recorded |
|---|---|---|
| P1 | After the GSP-RPC front end works, pack the calls crossing the device boundary into a vendor-neutral protocol so a backend on another vendor's GPU (AMD) could serve an NVIDIA guest in principle. Not before milestone 1, and only after a written design. See [kickoff.md](kickoff.md) §7 and the v1 proposal [VIRTIO-UAPI.md](../VIRTIO-UAPI.md). | 2026-09-23, operator's request |

The operator's hypothesis for P1 is that the vocabulary shrinks layer by layer (CUDA
400–500 entry points, about 120 ioctl shapes in v1, hopefully fewer at the GSP boundary,
mostly initialization and memory/compute allocation), and that it may still be too
device-specific. What the 610.57.04 source says about the size of the GSP boundary
(`src/nvidia/inc/kernel/vgpu/rpc_global_enums.h`, `generated/g_subdevice_nvoc.c`):

| Boundary | Vocabulary |
|---|---|
| Leandro v1 (ioctl shapes in `matrix/catalog-610.57.04.json`) | 290 verified signatures |
| GSP RPC functions | 220 `RM` + 4 `GSP` function codes, 43 event codes |
| Inside `GSP_RM_ALLOC`/`GSP_RM_CONTROL` | the full RM class and control space; of the 641 subdevice controls alone, 445 carry `ROUTE_TO_PHYSICAL` (answered by GSP), 24 are kernel-internal |

So the GSP boundary is not smaller than the ioctl boundary: the two RPCs that matter
wrap the same NVOS alloc/control vocabulary and add the controls the guest kernel RM
issues on its own. The vocabulary only gets small above RM (channel submission, memory
mapping, UVM faults), which supports the operator's suspicion. P1 stays parked.

| P2 | Other architectures (Ampere, Ada, Hopper, Blackwell) for the same GSP-client front end, and the vGPU-VF front end on Ada and newer. Only source analysis until a non-Turing test card exists (a Blackwell card at the operator's workplace is a possibility). Survey to land in `other-architectures.md`, including how stable the GSP RPC API is across architectures and driver versions (Q24): if the driver mainly talks to GSP, that API is the device's capability description. | 2026-09-23 |

| P3 | Slice the GPU spatially (by SMs) instead of in time as NVIDIA's vGPU does, and virtualize the device for real behind the synthetic front end. Future work after milestone 1; the GR partitioning and SM-mask controls in the RM are the place to look. | 2026-09-23, operator's request |

| P4 | Test the six synthesized `nvidia-smi` answers against the host's own answers in the `nvml` gate, then extend both the gate and `nvml.rs` to more `nvidia-smi` sub-commands. See [nvml.md](nvml.md), "Future". | 2026-09-23, operator's request |

| P5 | Research what real GRID vGPU and MIG vGPU devices expose to a guest (capabilities, static info, control answers) and use that to cut the information the device hands over (the D9 minimal set), driven by a configuration file or a similar mechanism rather than code changes. Note only, nothing to implement now. | 2026-09-23, operator's request |

| P6 | Host visibility of the guest's share: today the host's `nvidia-smi` shows nothing for a Caraxes VM because the device only issues read-only queries. From E5b on, the VRAM reservation (D11) is a host allocation owned by the device process, so the host sees the share as used memory, and the host channels give the process a context; keep Leandro's "process attribution" gate item and make the mapping guest to host process explicit (one device process per VM). | 2026-09-23, operator's observation |

| P7 | Isolation between guests and from the host on the GPU: today the trusted-guest assumption of v1 stays (shared IOMMU domain, guest page tables can name host IOVAs, no VRAM isolation). Long-term candidates: a vIOMMU whose IOVA space the device controls, per-VM host IOMMU domains once the host driver allows them, real device virtualization (P3). The operator wants this eventually; it is the point of virtualization. **Finding 2026-09-23 (E5b part 2):** the RM's non-stall completion notifier is per engine and host-wide, so the host desktop's own GR completions reached the guest's interrupt path (32k events in 3 minutes while idle). It is a wakeup broadcast, the same the RM does between host processes, not data access, but a timing side channel. Mitigation in the device (being built): forward an event only when one of the guest's mirrored channels on that engine has work in flight or made progress, judged from the USERD in the reservation; an idle guest receives nothing. Alternative without host events at all: poll the guest's channels at ~1 kHz while work is in flight. Full per-VM interrupt routing exists only on SR-IOV parts (Ampere+ vGPU); Turing cannot do better than device-side filtering. | 2026-09-23, operator's request |

| P8 | The proper removal of the guest DMA-range module: a Cloud Hypervisor patch that emits the ACPI `_DMA` method for its PCI host bridge (written from the ACPI specification), paired with an upstreamable x86 Linux change that honours `_DMA` the way arm64 does. The vIOMMU route is dropped: the guest allocates its I/O addresses and the host driver allocates the IOVAs, so neither side can line them up without the same table. **Status 2026-09-23:** written as `patches/0004-acpi-dma-host-bridge.patch` (`--pci-segment ...,dma_ranges=[cpu:dma:size,...]`, emits a static `_DMA` Name with `_TRA = cpu − dma`, unit test on the exact AML bytes, applies and builds on v53.0 alone and after 0001-0003) plus the x86 Linux draft `patches/linux/0001-x86-acpi-honour-dma-ranges.patch` (compiles on Ubuntu 6.8.0-136 and v6.8, not booted); design and test plan in [../../patches/ACPI-DMA.md](../../patches/ACPI-DMA.md). Not VM-tested. Blocker found: the DSDT is built from the static config but the IOVAs exist only after `DMA_MAP`, so the route needs the pre-registration flow (guest RAM as a file the device registers before Cloud Hypervisor starts, ACPI-DMA.md §5). Sign-offs are the operator's. **Operator's constraint (2026-09-23 16:00):** in the end everything must work on a vanilla guest; a Cloud Hypervisor patch may be acceptable, an upstream kernel change is "hard hard hard", so the `_DMA` route is the weakest of the three; the vm3 test finishes as evidence only. Candidate that needs neither a guest change nor a hypervisor patch: **shadow page tables in the device**, the way NVIDIA's pre-SR-IOV vGPU host plugin works on Turing: the guest writes its GMMU tables with guest physical addresses, the device (which owns the FB backing and sees BAR2/PRAMIN writes and the CE pushbuffers that write page-table entries) rewrites sysmem entries to host IOVAs before they reach real VRAM, or mirrors mappings through host RM map calls, which would also give the host-validated isolation of P7. To be worked out after CUDA runs. | 2026-09-23, operator's preference |
| P9 | Guest driver version independent of the host: once divergence works (Q29), the guest installs its distribution's stock NVIDIA packages with no in-guest build; the device carries the RPC ABI module for the guest's version. | 2026-09-23, operator's request |
| P10 | One tracer for both layers ("Leandro tracer"; the operator finds `nv-x`-style names awful): today `crates/nvrm-trace` taps the RM API above the RM (`LD_PRELOAD` on any process, host or guest) and the device logs the GSP RPC stream under the RM in its own format. Unify: one decoder library on `nvrm-abi` used by both taps, one log format with timestamps so a guest ioctl and the RPCs it caused line up, and a third tap for the device's mirror calls to the host RM. Useful for the CUDA path and for the version-divergence work (Q29). | 2026-09-23, operator's suggestion |
| P11 | Nested Caraxes: a Caraxes guest runs Caraxes itself and serves a synthetic GPU to a nested VM, backed by the synthetic GPU it sees; CUDA and display working there is the end-goal proof that the virtualization layer is complete (the device must only need what it provides). Prerequisites: the CUDA path (E5), the display path ([display-path.md](display-path.md)), nested virtualization in the guest kernel, and the host RM client role played by the inner device against the outer device's guest driver. | 2026-09-23, operator's end goal (for later) |
| P12 | Display: the operator's word 2026-09-23: the GRID displayless route is known to work from v1 and is the practical path ([display-path.md](display-path.md) route C2, experiment D1 with a `GRID_BUILD=1` guest module as the temporary aid; D0 headless first). The future target is route E: the device, in its GSP role, inserts a display by emulating a connector (DVI/DP-style head with a synthesized EDID) so the stock driver sees an ordinary monitor and no build flag is needed. Operator 17:20: explore what emulating a display through the GSP role means with the simplest connector, DVI-D (single-link TMDS, no HDCP, no DP AUX; HDMI/DP are complex protocols), with the refresh rate produced by the device's own timers (vblank cadence), which would also fix the Wayland refresh-rate issue of Leandro v1. **Wayland is the goal** (operator 17:50: X at most a stepping stone): a Wayland compositor on the guest's nvidia-drm KMS with stable vblank and page-flip cadence is the display milestone. After CUDA. | 2026-09-23, operator's direction |
| P13 | Two paths long-term: keep v1 (`vhost-user-nvrm`, API forwarding through a guest module) and v2 (`vfio-user-nvgpu`, synthetic device, stock guest) in one workspace on the shared crates (`nvrm-sys`, `nvrm-abi`, `nvrm-client`, `nvrm-trace`), one `tools/check.sh`, one gate suite with a path dimension so both run the same tests on the same guest image, per-path docs under `docs/` and `docs/caraxes/` with one component index, driver-version bumps through the generated ABI on both sides. When Caraxes is done: benchmark both against native (RM-call latency, H2D/D2H/D2D bandwidth, kernel-launch latency, torch throughput, VRAM churn, startup time, device-process CPU) and decide which path is the product and which the reference. | 2026-09-23, operator's request |
| P14 | Display scaling study (second study, [display-scaling.md](display-scaling.md)): which emulated connector is easiest and gives the most headroom (DVI-D vs HDMI vs DisplayPort vs NVIDIA's virtual heads), what 4K 144 Hz per head takes on TU106 (link and pixel-clock arithmetic, IMP, DSC, NVENC capture limits, VRAM display reserve), multiple heads, and what "display" must mean for the target audience: homelabs and small businesses that cannot afford vGPU, plus the operator's curiosity. Implementation to be discussed after the study. | 2026-09-23, operator's request |
| P16 | Side assignment ([briefing-display-nova.md](briefing-display-nova.md)), lower priority than the test suite, run in parallel where it touches neither the device crate nor the VMs: A a transport-neutral DP sink library with unit tests (EDID, DPCD, AUX and I2C-over-AUX, link training state machine), B an inventory of the display control dispatch in the guest driver (local vs GSP RPC vs MMIO per NV0073/NV5070 control, the EDID ping-pong, the class list), C a Nova-core compatibility study and probe plan (pinned kernel commit, TU106 boot expectations, r570.144 GSP ABI vs our 610 ABI, gates N0-N4), D the present backend (later, needs the display model). Started 2026-09-23 20:40 as three research/library packages. | 2026-09-23, operator's side assignment |
| P17 | Naming/configuration refactor ([briefing-naming.md](briefing-naming.md)): one module generates the guest-visible name from `{transport, personality, profile}`, backend stays internal, both devices use it, v1's `4Q` and `1333MiB` profiles survive, compatibility parsing for existing configs, tests for generation and parsing. Opus in a worktree with incremental commits, merged on landing; starts after the E5b part 2 hand-back and a checkpoint commit of the working tree. **Status: merged into main as d45bf4f (2026-09-23); default switched 2026-09-24** to `Leandro <Transport> <Personality>-<Profile>` (`Leandro VFIO RTX 2070-4G`, `Leandro VirtIO RTX 2070-4G`), legacy `Leandro RTX 2070-4G` opt-in via `LEA_GPU_NAME_FORMAT=legacy` / `--name-format legacy` / `CARAXES_NAME_FORMAT=legacy`, `../Leandro-Test` gates accept both (plan in [briefing-naming.md](briefing-naming.md), model in [identity-profile.md](identity-profile.md)). | 2026-09-23, operator's brief |
| P18 | SR-IOV middleman on Ada and later (operator, 2026-09-23 22:15): on silicon whose host driver exposes SR-IOV virtual functions (the vGPU-capable data-center and workstation parts from Ampere on; GeForce has none), Caraxes should consume a VF on the host through VFIO and present its own synthetic device to the VM: the guest owns its page tables with hardware isolation (VF GMMU), and the device keeps the identity, profile, display model and the GSP role, the part NVIDIA's vGPU manager plugin plays today, without the guest running in vGPU mode. The D17 shadow route remains the path for cards without SR-IOV (Turing GeForce). Design seam to keep: the execution backend behind the channel/memory model is one of {shadow tables on the PF, VF passthrough}, selectable per host card (D14 chip data; the naming model's backend field). Open checks: whether the guest driver can be kept in bare-metal mode on a VF (the VF's PCI identity and the missing display/privileged engines are what make the stock driver switch to `IS_VIRTUAL`), and what the open host modules need for VF creation without the GRID package. | 2026-09-23, operator's direction |
| P20 | Parallelism proof (operator, 2026-09-24): after the sequential benchmark, two Caraxes VMs run the cuda gate concurrently on the one RTX 2070 with the desktop on it. Needs: (1) a reservation placement policy, per-VM slots stacked downward from the top of the host FB, size from the profile, the D11 refusal rule kept (today the reservation is one fixed range, so a second reserving device gets NV_ERR_NO_MEMORY, found by the benchmark agent); (2) isolation evidence, not only function: each device's filter refuses the other VM's addresses and logs it, the comptag audit is clean for both, every Xid stays with its own pid, the host desktop is unaffected, and a pattern written into VM A's VRAM is not readable from VM B; (3) scrubbing of the reservation at teardown and a fail-closed teardown ([nova-vgpu-upstream.md](nova-vgpu-upstream.md) items 1 and 2); (4) a third VM as the refusal case. | 2026-09-24, operator's order |
| P21 | **BAR1 window passthrough (operator's order, 2026-09-24: "das Fenster brauchen wir")**: after the doorbell page, the guest's BAR1 windows onto its VRAM reservation are handed to the VMM as vfio-user sparse-mmap areas backed by the host RM's own BAR1 mappings, so guest CPU access to VRAM runs at native BAR speed instead of one socket message per access (today 0.4 MB/s). Constraints: the guest reprograms its BAR1 page tables, so the mapped areas must follow those changes; the host card has only 256 MiB of BAR1 shared with the desktop, so the per-device window budget of P20 stays and the mapping is windowed, never the whole reservation; the device loses visibility of BAR1 writes, which is acceptable because page tables go through BAR2, PRAMIN and the copy engine, not BAR1 (verify per D17 stage 1 before switching). Priority: right after the doorbell passthrough of the mapping package. | 2026-09-24, operator |
| P22 | Parked: a sysmem-backed VRAM profile. The guest's whole framebuffer lives in its own RAM, the shadow translates vidmem entries into sysmem entries, BAR1 becomes a direct mapping of that RAM, no reservation and no slot limit, no comptag question. Costs PCIe bandwidth (13 GB/s instead of 448 GB/s on TU106), so a profile for many light desktops, never the default. Operator, 2026-09-24: not now, first the window; the whole memory model may be secured better later. | 2026-09-24, parked |
| P23 | **Complete GSP surface (operator, 2026-09-24)**: emulate as many device answers as possible from source, so that another client of the same RM core (the Windows driver of the same generation, later nova-core) runs without trace-driven bring-up. [gsp-surface.md](gsp-surface.md) is the inventory and work list: of 829 controls that reach GSP the device answers 171; the rest splits into 62 pure chip data (one table-driven handler plus one capture per chip), 71 host-forwardable (a forward rule with a D9 deny list), 30 guest-state derived (registry and mirror), 306 needing device modelling (mostly display and profiler/debugger) and 189 never expected on TU106 (one NOT_SUPPORTED rule per feature family); roughly 40 to 60 agent-days, of which A plus E remove about 250 items in about 5. Missing RPC functions today: ALLOC_MEMORY, UNLOADING_GUEST_DRIVER, DUMP_PROTOBUF_COMPONENT, ECC_NOTIFIER_WRITE_ACK; the device never sends the RC_TRIGGERED, MMU_FAULT_QUEUED and OS_ERROR_LOG events. Open: whether the Windows 610 driver boots a TU106 with GSP-RM at all. | 2026-09-24, operator's direction After E and A (2026-09-24, [gsp-surface.md](gsp-surface.md) "After E and A"): the device answers 373 of the routed controls on 610 and 383 on 615; category E is refused per chip as feature families (`[features]` in `chips/*/chip.toml`, `src/chipctl.rs`), A comes from `chips/tu106/controls.toml` (8 filled, 52 listed with their source), the ids and field offsets are generated (`caraxes/tools/gsp-tables.py`, `ctrl-layouts.py`). Unhandled: A 52, B 70, C 29 (31 on 615), D 305 (308), E 0. The plans for B, C, D and the RPC surface are being written under `docs/caraxes/gsp-plan/`. |
| P24 | **Poll-mode GSP (operator, 2026-09-24)**: at the end, run the emulated GSP like an SPDK poller: one dedicated host core serves 5-10 devices by polling instead of trapping, so that every guest-to-device signal is a memory write the guest never exits for: the GSP command queue write pointer (already in shared guest RAM), USERD GPPut of every mirrored channel (USERD in guest sysmem via the RM's instance-location keys, or the host channel's USERD placed in the guest's own page through the channel-alloc USERD parameters), and the usermode doorbell page presented to the guest as a memfd-backed sparse-mmap area of BAR0 that the poller watches (a memfd, unlike an RM mapping, may sit at any region offset). The poller rings the physical doorbell within a poll period, so a submission costs no KVM exit and no socket round trip; with batching across devices this can land below the native launch overhead for submission-heavy workloads (RL loops, small kernels), at the price of one spinning core. Prerequisites: the mapping package (write-through shadow at invalidate time, so the doorbell path carries no consistency work), the USERD-in-sysmem configuration, P21 for BAR1. No Cloud Hypervisor change needed for the doorbell in this form. | 2026-09-24, operator's idea |
| P15 | Synthetic heads on any chip: since scanout never touches the physical display engine, the device can present N heads and connectors on any host card, including display-less data-center parts, as long as the emulated chip identity has a display HAL in the guest driver and its SM architecture matches the physical silicon (CUDA binaries are built for the identity's SM). Open check Q33: whether the guest driver's chip configuration allows a display for display-less identities such as GA100/GH100 when the fuse and static info claim one, or whether such hosts need a sibling identity with the same SM version. | 2026-09-23, operator's question |
| P19 | Learn from the upstream NVIDIA vGPU manager (nova-core, "[PATCH v2 00/32] Introduce NVIDIA vGPU manager") without copying its SR-IOV architecture: [nova-vgpu-upstream.md](nova-vgpu-upstream.md) ranks what transfers (reservation scrubbing and fail-closed teardown, progress-gated interrupts, queue fences and wedge handling, wrap-safe semaphore waits) and what is VF-only; it is also the host stack a P18 middleman would sit on. | 2026-09-23, operator's brief |

## Status

**Standing goal (operator, 2026-09-23 16:40):** every gate of the Leandro v1 test suite passes on
Caraxes (`tools/caraxes-gate.sh cuda|gpu|vdisplay|display`, [testing.md](testing.md)); implementation
choices and cleanup come after that. Order: E5b part 2 (kernel launch through mirror channels) →
E5d (the whole cuda and gpu gates: managed memory needs replayable faults from the host, encode
needs NVENC through a mirror channel plus ffmpeg in the guest, torch and robustness stages) →
display gates via the DisplayPort route (D15, [display-connector.md](display-connector.md)).
Operator 2026-09-23 21:00: proceed like the original Leandro project: its helper tools (ioctlping,
ctrlping, rmdirect, oomprobe, vrampress, the probes) are all runnable; every VM has the test
material mounted and compiled at startup (`up` runs `sync-tests`); then the torch and RL tests in
the VM; then display, expected to be much easier than in v1.

- Exploration done. Vendor checkouts updated (see environment.md).
- Client decided: Cloud Hypervisor. Scope order: `nvidia.ko` + `nvidia-uvm` first, DRM/modeset later.
- 2026-09-23: go-ahead for E0 given; implementation brief in [kickoff.md](kickoff.md).
- 2026-09-23: **E0 done, expectation met** (`crates/vfio-user-nvgpu`, `tools/caraxes-vm.sh`,
  evidence in `caraxes/runs/e0-2026-09-23/`): the stock driver binds, reads the boot
  registers and the config mirror, walks the PROM and fails at the missing third ROM image
  (`RmInitAdapter failed! (0x62:0x25:2119)`).
- 2026-09-23: **E1 done, expectation met** with the synthetic VBIOS (D8): the parser accepts
  the image, prints `VBIOS version 90.06.3D.00.76`, and the driver times out in the GFW boot
  wait (`RmInitAdapter failed! (0x62:0x65:2119)`). Evidence in `caraxes/runs/e1-2026-09-23/`.
  Passthrough captures stay deferred until a prepared procedure exists; that procedure is
  now written ([passthrough-procedure.md](passthrough-procedure.md)) and found three
  blockers to fix first: `sshd` is inactive on the host, there is no display manager
  (Hyprland is started by hand on tty1), and the framebuffer console holds the card.
- 2026-09-23: **E2 done, expectation met** (`--boot-model`): the full FWSEC, Booter,
  mailbox and doorbell sequence runs; the FRTS offset is read from the FWSEC command in
  guest RAM; the driver stops at the status-queue timeout (`GspStatusQueueInit: msgqRxLink
  failed: -7`) and its boot retry refuses because WPR2 is up. A PCI function-level reset
  restores the model, so probes repeat. Evidence in `caraxes/runs/e2-2026-09-23/`.
- 2026-09-23: **E3 done, expectation met**: the device serves the GSP message queues
  (`msgq.rs`, `rpc.rs`), the driver accepts `GSP_INIT_DONE`, the guest-info echo and the
  static info carrying `Leandro RTX 2070-4G` with 4 GiB (every field labelled with its
  source, D9), and stops at the first phase-5 RPC. Evidence in `caraxes/runs/e3-2026-09-23/`.
  Next: E4 along [phase5-rpcs.md](phase5-rpcs.md) (98 RPCs; `nvidia-smi` is the check),
  go-ahead given 2026-09-23 with decision D10 (host RM access from the device process itself).
- 2026-09-23: **E4 in progress**: rows 1-38 answered (`ctrl.rs`, host forwarding in
  `hostrm.rs`), the fake framebuffer with the BAR2 page-table walk passes `kbusVerifyBar2`
  (`fb.rs`); the driver reaches the KGR static-info rows (39-52, `kgr.rs` in progress) and
  the FIFO post-scheduling callbacks. Details under "E4 progress" in
  [experiments.md](experiments.md).
- 2026-09-23: **Milestone 1 reached.** `RmInitAdapter succeeded!` on the stock driver
  and `nvidia-smi` in the guest prints `Leandro RTX 2070-4G` with 4096 MiB. Full account
  under "Milestone 1 reached" in [experiments.md](experiments.md); evidence in
  `caraxes/runs/e4-2026-09-23/milestone1-*`.
- 2026-09-23: **nvidia-smi complete, `nvml` gate passes** (`tools/caraxes-gate.sh nvml`):
  temperature, power, clocks, utilization and PCIe values are forwarded from the host card
  (`src/nvml.rs`, [nvml.md](nvml.md)), identity stays device-owned.
- 2026-09-23: [cuda-path.md](cuda-path.md) landed. E5a (UVM registration on the host-RAM
  framebuffer, no physical GPU) started. E5b and later need operator decisions: a
  fixed-address VRAM reservation on the host, a privileged host RM client in the device,
  and guest DMA addresses equal to host IOVAs (a temporary guest aid at first).
- 2026-09-23: decisions D11-D13 taken; [host-facts.md](host-facts.md) landed: the IOMMU
  domain is translated (DMA-FQ, so the guest-DMA fix S1 applies), fixed-address contiguous
  VRAM allocations of 2.5, 3 and 5 GiB succeed as an unprivileged client at the top of the
  host FB region, no Xid, no host effect; reservation rule: top of the region, 2 MiB aligned,
  refuse unless size + 1 GiB is free, no floating fallback.
- 2026-09-23: the four parallel packages landed: [gsp-abi-610.md](gsp-abi-610.md)
  (compiled sizes; Q4 answered), [fb-model.md](fb-model.md) (BAR2/PRAMIN model and FB
  layout), [phase5-rpcs.md](phase5-rpcs.md) (E4 inventory), and
  [passthrough-procedure.md](passthrough-procedure.md) (three host blockers).
- 2026-09-23: survey of the other architectures and of GSP API stability landed in
  [other-architectures.md](other-architectures.md): the RPC vocabulary and structs are one
  driver-version-wide ABI shared by all chips; only the boot register sequence is
  per-architecture; Ampere GA10x would be the least-work second identity; the VF path
  needs a booted GSP-RM behind it and is a dead end for an emulator.
- 2026-09-23: **E5a done**: `nvidia-uvm` registers the GPU (`UVM_REGISTER_GPU` returns
  NV_OK) with the device executing UVM's seven init pushes on the fake framebuffer
  (`channel.rs`, `uvm.rs`, [uvm.md](uvm.md)); condition in the guest: `uvm_disable_hmm=1`
  or `UVM_INITIALIZE` flags 1, because the Cloud Hypervisor 64-bit PCI window leaves no
  free physical range for HMM device memory. A latent E4 bug (allocation parameters echoed
  as zeros) was fixed on the way. Next: the D14 refactor, then E5b.
- 2026-09-23: [sysmem-iova.md](sysmem-iova.md) landed: registering a memfd buffer with the
  host RM as an OS descriptor yields exactly one contiguous IOVA range (256 MiB at
  0xa0000000, 1 GiB at 0x40000000, reproducible; top-down allocator, bases not constant).
  Recommended E5b mechanism: register guest RAM in 1 GiB chunks, read each base with the
  admin-only query, publish a table, and a small temporary guest module installs it as the
  GPU's `dma_range_map` (x86 Linux ignores ACPI `_DMA`, so no firmware route exists).
  Pending the operator's word: the host GPU switching to 64-bit IOVAs once a 4 GiB guest is
  registered (per device, until reboot), guest RAM pinned for the VM's life, the trusted-guest
  assumption, and the host log volume under `NVreg_ResmanDebugLevel=0`.
- 2026-09-23: **D14 refactor done**: `src/chip/` (trait, Turing boot model, TU106 loader),
  `chips/tu106/*.toml` (chip data, register map, FIFO table, interrupt table, boot registers),
  `boards/rtx2070.toml`, `src/ver/v610.rs` behind the `v610` feature. Behaviour unchanged:
  same boot event sequence, nvml gate and UVM check pass, 38 unit tests, checks green
  ([versatility.md](versatility.md)). Next: E5b part 1 (memory model with the reservation,
  the registered guest RAM and the DMA table) and the test-suite integration.
- 2026-09-23: **guest DMA-range module done** (`guest-module/caraxes_dma_range/`, D13 aid):
  installs `dev->dma_range_map` on the synthetic GPU from `ranges=gpa:iova:size,...` (hex,
  64 entries, 2 MiB multiples, must cover all guest RAM), refuses while a driver is bound,
  re-installs the map before every `nvidia` probe through a PCI bus notifier (the driver
  core frees the map on unbind), readback in sysfs. Tested in vm1: 13 refusals, identity
  table with `RmInitAdapter succeeded!` twice across a driver reload, offset tables
  (+5 GiB, 64-bit top-down, negative) mapping to GPA + offset. Evidence
  `caraxes/runs/dma-range-2026-09-23/`. In flight: E5b part 1 (vm2), test-suite
  integration (vm0), N1 ACPI `_DMA` patch, N2 vhost-user patch rewrite.
- 2026-09-23: **N1 done**: ACPI `_DMA` patch set (`patches/0004-*.patch`, `patches/linux/0001-*.patch`,
  `patches/ACPI-DMA.md`); applies and builds on v53.0, 89/89 vmm tests, AML verified with iasl;
  the Linux side is an unbooted draft; VM tests T0-T4 and the pre-registration flow are open
  (P8). `tools/build.sh ch` will apply 0004 automatically on its next run; without the option
  the DSDT is byte-identical. Hard goal of the day: CUDA in the guest (host execution primitive
  `caraxes/tools/host-exec.rs` in flight).
- 2026-09-23: guest module marked deprecated in favour of the `_DMA` route (kept in tree);
  `guest-module/caraxes_dma_range/WALKTHROUGH.md` written as the operator's kernel-programming
  reading guide (398 lines: each mechanism with kernel source pointers, five experiments for
  vm1, the pitfalls; it pins down the `check_ram_in_range_map` infinite loop in v6.8: the match
  pointer is declared outside the loop and never reset). Test harness landed
  ([testing.md](testing.md)): nvml 11/11, cuda 0/12, all at cuInit on control 0x2080182a.
- 2026-09-23: **N2 done** in worktree branch `worktree-agent-a45f20dcd340b4776` (4 commits on
  `e2f1aec`): patches 0001-0003 rewritten against QEMU's vhost-user.rst (2026-09-11) and VIRTIO
  1.4: SHMEM is bit 22 not 21 (vhost 0.16 had it wrong, both ends shared the bug), shmid-indexed
  regions, per-region offsets, overlap/alignment/partial-unmap refused, mappings dropped on
  reset; Cloud Hypervisor frontend moved to vhost 0.17; `vhost-user-nvrm` switched to match
  (`nregions=1`, BACKEND_SEND_FD); `patches/README.md` specifies the series, review in
  `patches/REVIEW-vhost-user.md`. Independently verified: the series applies to pristine v53.0
  and 0004 applies after it. No compatibility mode: both ends must change together. Merge into
  the main tree deferred until E5b part 1 hands back (the branch bumps workspace deps); the
  v1 VM double-check (patches/README.md "Manual VM check") runs from the worktree first.
- 2026-09-23: **E5b part 1 done on vm2** ([memory-model.md](memory-model.md)): 4 GiB host VRAM
  reserved at `[0xe9000000, 0x1e9000000)`, guest FB address = host VRAM address, guest RAM
  registered in 1 GiB chunks with a real non-identity IOVA table through the guest module,
  `RmInitAdapter succeeded!`, nvml 11/11, `uvm verify 1` through the translation, clean
  teardown and refusal paths; host GPU now in 64-bit IOVA mode until the host reboots; the
  guest's 40-bit flush buffer needs a swiotlb alias in the guest table (Q31 risk noted).
  cuInit still 100 (two more unanswered controls, libcuda 610.43.03 vs module 610.57.04 in
  the guest image to check). E5c agent now iterating on vm2.
- 2026-09-23: **E5b part 2 (execution on the physical GPU) built and partly verified**
  ([execution.md](execution.md), `caraxes/runs/e5b2-2026-09-23/`): the framebuffer is backed by
  the reserved VRAM through BAR1 windows (nvml, `uvm verify 1`, cuda-init/context/core/hostreg
  all pass on real VRAM, no Xid); the full mirror-object wiring works (per guest VA space a host
  externally-owned or shared-management `FERMI_VASPACE_A` with `DMA_SET_PAGE_DIRECTORY`, per TSG
  a host TSG, per channel a host `TURING_CHANNEL_GPFIFO_A` with the guest's GPFIFO/USERD/CE
  object, `GPFIFO_SCHEDULE`, the doorbell forwarded guest-token → host-token, GR0/CE non-stall
  events → guest MSI, TLB invalidates forwarded, teardown in order — all measured on the card,
  freeing cleanly). Finding: BAR1 is write-combined, so the device must flush its windows before
  the host doorbell or the PBDMA fetches stale VRAM (PBENTRY); with the flush, CUDA's CE channels
  run on the physical copy engines with **no Xid**. Two walls remain (host channel execution is
  behind `CARAXES_MIRROR_CHANNELS=1` until then): the CE first-launch stall (host-exec open item
  3) and the GR context on a host channel (c1 faults with a contained Xid 38, c2 unestablished),
  so the kernel launch has not landed and GR/UVM channels run in software on the VRAM store
  meanwhile. Every Xid was contained to the device process; the host desktop was unaffected.
- 2026-09-23: display research done ([display-path.md](display-path.md)): no NVIDIA display
  engine emulation exists anywhere; a real one is 15-25 agent-days at high risk; v1 never had
  a device display (its guest module faked GRID displayless); cheap experiments D0 (headless
  nvidia-drm, no change) and D1 (needs a `GRID_BUILD=1` guest module: operator's decision).
- 2026-09-23: [dma-routes.md](dma-routes.md) answers the operator's "no patches" question:
  shadow page tables in the device are the only vanilla route; recommended after CUDA runs.
- 2026-09-23: [cuda-lookahead.md](cuda-lookahead.md) done from the tracked 610.57.04 native
  traces: the CUDA path phase by phase with every RPC/control classified; set 1 (cuInit +
  cuCtxCreate without execution: 12 items incl. row 41 from host GR masks, DUP_OBJECT VA-space
  alias, context-share lookup, compute-class stub), set 2 (cuMemAlloc/cuMemcpy on the software
  CE: non-stall vectors for GR0/CE2-4), set 3 (kernel launch = part 2). Corrects cuda-path.md:
  GPFIFO and USERD live in one 2 MiB vidmem block written through BAR1.
- 2026-09-23: P16 package B done ([display-adapter.md](display-adapter.md)): AUX terminates at
  the device (DP_AUXCH_CTRL is PRIVILEGED+ROUTE_TO_PHYSICAL, forwarded raw); for one SST DP
  connector the device answers 36+4 NV0073, 3 NV5070, 4 NVC370, 2 NVC372 controls, nothing by
  MMIO; corrections to the earlier notes (GET_HEAD_ROUTING_MAP must echo the mask, NVC573 is
  guest-local, the class list is the guest's own table, armed core state mirrored at 0x688000,
  cursor Free register non-zero, PUT is a byte offset); phase A/B checklist with GET_IP_VERSION
  0x20800a4b as the first missing behaviour; gates D0-D6 with evidence; three riskiest ABI
  details (AUX size/status, the EDID round trip through SET/GET_EDID_V2, link-training agreement
  across five answers, with four of them cached until driver reload).
- 2026-09-23: P16 package A done: `caraxes/dp-sink/` standalone crate (std only, MIT), DPCD model
  with profiles (DP 1.1a HBR x4 default, DP 1.4 HBR3 x4 DSC headroom), link-training state
  machine, AUX and I2C-over-AUX engine with real byte counts, EDID 1.4 generator checked by
  `edid-decode` and an independent test parser, HPD events, transcript replay; 52 tests, fmt and
  clippy clean; moves into `crates/` after the part-2 hand-back. Eleven spec ambiguities listed
  in its README §8.
- 2026-09-23: [nova.md](nova.md) done (P16 package C): only Linux v7.2 nova-core loads stock
  firmware (r570.144 files in linux-firmware 20260916; v7.3/drm-rust-next want `.tlv` files that
  do not exist); today's device cannot talk to nova at all: synthetic VBIOS signature/app id
  (0xBB77/0x05 vs 0x4E56/0x85), 48-byte r570 message header vs 16-byte 610 header, the CPU
  sequencer event 610 dropped, static-info layout (1656 B, name at 1260), unload mailbox bit,
  falcon-start counting on re-probe, queue re-linking; 15 items to work, 14 compatible; a
  `ver/v570.rs` overlay is 150-200 lines; runtime version selection from the firmware's
  `.fwversion` section; total 5-7.5 agent-days; gates N0-N4 with evidence.
- 2026-09-23: [display-scaling.md](display-scaling.md) done (P14): recommended connector for
  headroom is DisplayPort SST (one per head, no MST/HDCP, DSC offered), +3-5 agent-days over DVI-D
  for a DPCD model, an AUX engine serving the EDID, and link-training answers; DVI-D stays for
  first light. 4K144 (1306 MHz, 31.4 Gbit/s at 8 bpc) exceeds HBR3 x4 and needs DSC, like a
  real RTX 20 card; 4K120 fits without. HDMI tops out at 4K60 on Turing (no FRL). Real limits
  are outside the display: NVENC ~214-233 4K frames/s on Turing (scaled, not measured), VRAM
  display reserve 161 MiB per 4K head, a 144 Hz timer needing p99 jitter under 0.5 ms. Four
  heads, eight windows, four SORs. Total to four DP heads at 4K144: 17-27 agent-days.
- 2026-09-23: [display-connector.md](display-connector.md) done (Wayland-first): on Turing with
  610.57.04 a compositor's timing comes only from page-flip completion (nvidia-drm `vblank=0`
  default; the vblank callback needs an Ampere+ HAL entry), so a device timer gating flip
  completion plus the head's vblank interrupt is the whole cadence mechanism; v1 ran Wayland at
  1520 FPS because the GRID displayless HAL completes flips immediately, ignores the present
  interval and reports no output LUT. DVI-D = SOR with SINGLE_TMDS_A, connector type 0x31, EDID
  base block with the DVI interface byte, no DP/HDMI/SCDC/link-training traffic. The stock
  non-GRID driver asks for a display by itself once the display fuse reads 0: no build flag, no
  guest aid. Phases A-D 9-14 agent-days (vs 15-25), first Wayland milestone in phase C.
- 2026-09-23: display D0 on vm0 ([display-d0.md](display-d0.md)): `nvidia-modeset` loads headless
  with zero device traffic (needs `modprobe video` first); across all 267k device-trace lines of
  the day there is no display class, RPC or control, and the display fuse always reads 1 (no
  display). Blockers for D1: `nvidia-drm` did not build in the guest against the 6.8.0-136
  headers when built after the fact (conftest mismatches; a full four-module build is the
  likely fix, v1 guests had it), and the guest image has no NVIDIA X driver (`nvidia_drv.so`,
  `libglxserver_nvidia.so`), which needs the operator's OK to stage from the host's 610.57.04
  files as v1's harness did. Guest restored to baseline.
- 2026-09-23: **display phases A and B done, gate D3 passed** ([display-ab.md](display-ab.md)): with
  `--display` the fuse reads 0, `RmInitAdapter succeeded!` with KernelDisplay, and the stock
  `nvidia-modeset` + `nvidia-drm modeset=1 fbdev=0` show one connected `card0-DP-1` with the
  device's EDID (byte-identical, edid-decode PASS) and modes 1080p60/59.94/50, 720p60/59.94,
  640x480; the 8K120 VIC above the HBR3 x4 budget is pruned; no core UPDATE with raster
  methods; zero unanswered display controls; nvml 11/11 and UVM unchanged before A, after A
  and after B. Headless (fuse 1) remains the default without `--display`. The guest image's
  gdm3 modesets any new card0, so it is disabled on vm0. Next: phase C (core notifier through
  RAMHT, window completion paced by the device timer).
- 2026-09-23: **N2 verified on a real v1 VM** (worktree commit `79b7d0b`, evidence
  `caraxes/runs/vhost-user-check-2026-09-23/` in the worktree): window 0x200000000 negotiated,
  guest sees the 8 GiB window on BAR 2, nvidia-smi and a driver-API vector add pass, the injected
  out-of-range SHMEM_MAP is refused with the device staying up, module reload (device reset)
  works, two VMs at once work; stock CH + new backend comes up without a window and without a
  crash. New limitation found: a device reset while a guest process still holds window
  mappings kills the VM (KVM_RUN EFAULT) because CH now drops mappings on reset as the spec
  requires; fix later in the guest module (zap mappings on device removal) or CH (non-faulting
  backing). Merge into main scheduled for the gap after the E5c hand-back.
- 2026-09-23: **v1 test material in the guest; `cuda` and `gpu` gates run** ([testing.md](testing.md)):
  `tools/caraxes-vm.sh sync-tests` copies the guest-side parts of ../Leandro-Test into
  `~/caraxes-tests` and builds the probes there; `tools/caraxes-gate.sh cuda|gpu|vdisplay|display`
  report in v1's verdict format. On vm0: nvml passes; cuda fails (0 of 12 scored tests), every
  program at `cuInit` 100 because the device answers `BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS`
  (0x2080182a) NOT_SUPPORTED; gpu fails (7 stages); vdisplay and display are not applicable yet
  (exit 2). Evidence in `caraxes/runs/gate-*-2026-09-23*/`.
- 2026-09-23: **ACPI `_DMA` route tested in vm3** ([../../patches/ACPI-DMA.md](../../patches/ACPI-DMA.md)
  "Results", evidence in `caraxes/runs/acpi-dma-2026-09-23/`). Patch 0004 does its work:
  - **T0 pass.** The DSDT is byte-identical without the option. With it, `_DMA` sits on `PC01` only, in the §3.3
    layout. The stock 6.8 kernel ignores it, and nvidia initializes on `0001:00:01.0` with `nvidia-smi` working.
  - **T1 pass.** The patched guest kernel `6.8.12-caraxesdma` (Ubuntu-6.8.0-136.136 + `linux/0001`, built on the
    host) installs the identity table from firmware at every probe. nvidia and `nvidia-smi` work, and the guest
    module is not loaded.
  - **T2 pass.** With a +5 GiB table, nvidia not loaded, every mapped address is GPA + 5 GiB and coherent
    allocations succeed. With the real top-down IOVA table, coherent allocations are steered to ZONE_DMA32, as §8
    predicted.
  - **T3 not run.** The pre-registration flow is still missing. `DMA_MAP` precedes the ACPI build by about 0.5 ms
    in the same boot, but the static option is fixed at `VmCreate`. The device and launcher additions are listed.

  The guest module stays until T3 passes.
- 2026-09-23: **E5b core proven: the physical GPU walks a guest-built VRAM page table**
  ([host-exec.md](host-exec.md); tool [caraxes/tools/host-exec.rs](../../caraxes/tools/host-exec.rs)
  + [caraxes/tools/host-exec.sh](../../caraxes/tools/host-exec.sh); evidence
  `caraxes/runs/host-exec-2026-09-23/`). As an unprivileged RM client (CAP_SYS_ADMIN scoped to
  three controls, D12), staged A-F:
  - **A/B:** a 64 MiB contiguous VRAM reservation (the fixed-top address was `NV_ERR_NO_MEMORY`
    with the three vfio-user VMs running, so a floating fallback was used for the proof); a
    64 MiB BAR1 window plus a second concurrent window (multiple windows can be held); a sysmem
    OS descriptor pinned, its IOVA read (admin) - 64-bit today because the host is on 64-bit DAC;
    a GMMU v2 PD3->PD0->PT built in the reservation and validated.
  - **C:** an externally-owned `FERMI_VASPACE_A`, then **`DMA_SET_PAGE_DIRECTORY` to a PDB in
    VRAM returned `NV_OK`** (the key privileged call), a `TURING_CHANNEL_GPFIFO_A` with USERD in
    sysmem, a `TURING_DMA_COPY_A` (version 1), `GPFIFO_SCHEDULE`, the work-submit token, and the
    `VOLTA_USERMODE_A` doorbell - all `NV_OK`.
  - **D (the proof):** ringing the doorbell made the GPU release a semaphore at a **VA** it could
    only reach by walking the guest-built tables (`0xcafe`, ~9-17 us), and a CE `LAUNCH_DMA` read
    the reserved VRAM at a VA and wrote sysmem, **bytes verified** (~22 us). The first CE launch
    on a fresh channel RC-once (absorbed by a warm-up); a CE write into the reservation stalled
    (open item - CPU/BAR1 writes it instead).
  - **E/F (findings):** the non-stall OS event registers and arms `NV_OK` but a
    `NON_STALL_INTERRUPT` method did not wake the fd (use the engine notifier); `GPU_PROMOTE_CTX`
    for a reservation buffer returned `NV_ERR_OBJECT_NOT_FOUND` (record-only under (c1), do not
    replay bare).
  - Teardown freed every object; `nvidia-smi` memory returned; no `Xid`. host-exec.md carries the
    part-2 device wiring spec (host RM call per guest event, pass-through vs translated addresses,
    what to send back) and the measured numbers.
- 2026-09-23: **P16 package A, DP sink library: unit gate D1 passes** ([crates/dp-sink](../../crates/dp-sink/README.md), moved from `caraxes/dp-sink` on 2026-09-23,
  a standalone crate outside the workspace for now, `std` only). It covers the DPCD for a
  DPCD 1.1a HBR x4 SST profile (the HBR3/TPS4/DSC headroom is a profile value), the training
  state machine that refuses invalid configurations, native AUX and I2C-over-AUX (MOT,
  address-only, E-DDC segment pointer, short replies, NACK/DEFER kept apart from transport
  errors), the EDID generator (1080p60 CVT-RB, `edid-decode --check` PASS plus an independent
  parser), HPD long and short pulses, and a transcript replayer. 52 tests; clippy clean. The
  golden transcript follows the NVIDIA DP library's request order as read in source, not a
  guest capture. The NV0073 mapping and the open spec ambiguities are in the crate README.
- 2026-09-23: **P17 naming refactor landed in a worktree, awaiting merge**: `nvrm_abi::naming`
  generates the guest-visible name from `VirtualGpuSpec { transport, personality, profile,
  backend }`; v1's `guest_card_name` and Caraxes' `StaticIdentity` both call it; the default
  stays `Leandro RTX 2070-4G` for the v1 gates, the transport word (`Leandro VFIO RTX 2070-4G`)
  is opt-in through `LEA_GPU_NAME_FORMAT`, `--name-format` or `CARAXES_NAME_FORMAT`; the
  Caraxes gate asks the device for the expected name (`tools/caraxes-vm.sh name`).
- 2026-09-24: **display phases C and D done** ([display-cd.md](display-cd.md)): the device
  latches flips on a per-head `CLOCK_MONOTONIC` clock (core/window method decoder, RAMHT and
  context DMAs, notifiers, semaphores, AWAKEN as nvidia-drm models it, `LAST_DATA` vblank,
  `RG_DPCA`/LOADV, `FE_CORE_HEAD_STATE`, a real `IS_MODE_POSSIBLE`); 120 flips at 60.00 Hz,
  NVKMS vblank at 60.01 Hz, Weston frame callbacks at 59.86 Hz (1080p60) and 49.87 Hz (1080p50),
  GNOME 46 on Wayland (Mutter atomic KMS, Mesa kms_swrast, 0 page-flip failures), EDID-change
  hotplug through `POST_EVENT`, the hardware cursor through NVKMS's cursor channel; the adapted
  `vdisplay` and `display` gates pass (GPU-execution stages not run: no host execution on vm0;
  the compositor stage missed 60 +- 0.5 Hz by 0.01 Hz once under host load). Open: GPU-rendered compositors (host-exec), BAR1
  direct mapping (CPU access to scanout buffers runs at 0.4 MB/s), cursor, Moonlight reception.
- 2026-09-24: **Mapping package closed** ([mapping.md](mapping.md), parallel.md §8.3 and §8.4,
  `caraxes/runs/mapping-2026-09-24/`). The CeUtils experiment went red: without
  `RmDisableGlobalCeUtils=1` the guest RM's CeUtils channel (bare, no TSG, no VA space handle)
  runs on neither path, and `RmInitAdapter` fails in `memmgrTestCeUtils`. 0 Xid. The key stays
  in the guest default. The concurrency check on the device of 0d00b26 passed with vm1 (2 GiB,
  slot 0) and vm2 (1 GiB, slot 4) next to vm0. Two 2 GiB VMs would have broken the 1 GiB
  desktop floor. The cuda gate ran on both at once with the single-VM result, and
  async_streams passed on vm1 (556k leaves in 24k maps) with host `nvidia-smi` at most
  0.087 s. p20-check PASS, the pattern test was clean in both directions, 0 Xid, and the
  injected teardown failure failed closed.
- 2026-09-24: **P20, the parallelism proof** ([parallel.md](parallel.md), evidence in
  `caraxes/runs/parallel-2026-09-24/`). vm1 on slot 0 and vm2 on slot 1 (2 GiB each) run
  concurrently beside the desktop.
  - The nvml gate passes 11/11 on both, and the scored cuda gate passes on both at once with the
    single-VM green result.
  - A 1 GiB pattern in one VM is invisible to the other in both directions.
  - The host RM's scrub on free clears a freed slot before its next owner sees it (2 GiB costs
    23-38 ms).
  - The teardown fails closed (exit 3, registrations and reservation kept).
  - A third 2 GiB VM is refused by the D11 headroom rule.
  - The host logged 0 Xid, and the comptag audit is clean on both devices.
  - Fixed on the way: a BAR1 leak (windows were never `RM_UNMAP_MEMORY`ed), a lock re-entry
    deadlock in the FB store, and silently dropped guest writes; the per-device BAR1 budget is now
    2 x 32 MiB.
  - Found and not fixed: the unscored `suite-test_async_streams` mirrors about 545k leaves per
    device and stalled the host RM for minutes at teardown (the desktop's electron GPU process
    restarted); and a guest buffer crossing a 1 GiB GPA chunk makes CE page tables point past the
    VM's registered guest RAM, which the host IOMMU blocked (parallel.md §8).
- 2026-09-24: **Blackwell capture filed and read** ([blackwell-gb206.md](blackwell-gb206.md),
  `caraxes/reference/gb206/`): the operator's lab run on an RTX 5060 Ti with driver 615.71.09
  came back complete; 610.57.04 already supports GB206 and boots its GSP through FSP, so a
  first Blackwell identity needs new names and an FSP model, not a new ABI version; 615.71.09
  is a new GSP transport altogether. Open with the operator: MSI-X and the BAR1 size of the
  synthetic GB206, a second capture, the guest driver version for Blackwell.
- 2026-09-24: **Doorbell reactor measured** ([doorbell-poll.md](doorbell-poll.md), benchmark.md,
  `caraxes/runs/doorbell-poll-2026-09-24/`): `CARAXES_DOORBELL_POLL=spin|hybrid` polls the pool's
  eventfds on a pinned core instead of waking a blocked thread. On vm1 with P21 and 64 tokens,
  in the clean second pass:
  - short kernel 0.024-0.027 ms (blocking reader) to 0.014-0.015 ms per launch;
  - rlprobe stage 4 2.7 s to 2.1 s (spin) and 2.3 s (hybrid);
  - ring-to-host-doorbell p99 9.8 us to 4.9 us pinned, 7.1 us unpinned;
  - reader CPU 96 % of a core in spin and 3 % in hybrid with 200 us idle;
  - no contention on the Device mutex (p99 wait under 1 us), 0 Xid.

  About 3.6 us per ring remain inside the device (shadow re-sync, flush, mirror bookkeeping).
  Removing the KVM exit is P24.
- 2026-09-24: **Closing series on 610.57.04** (benchmark.md, `caraxes/runs/closing-610-2026-09-24/`):
  native, v1, v2 interleaved, N=5, quiet host, v2 with the mapping package, the P21 area, the
  token pool and the pinned spin reactor: short kernel 0.015 ms against native 0.006 (this
  morning 0.257), rlprobe stage 4 2.10 s against native 1.20 and v1 1.50 (this morning 20 s),
  launch 30M-cycle 1.06x, cuCtxCreate 3.8 s (19x, the BAR2 page-table writes), allocations 23x,
  copies below 16 MiB at half of native, 0 Xid. The last table before the host moves to 615.
- 2026-09-24: **Zero-trap submission measured** ([doorbell-token-pool.md](doorbell-token-pool.md),
  benchmark.md, `caraxes/runs/doorbell-tokens-gpu-2026-09-24/`): the P21 area plus the datamatch
  token pool (64 tokens, one eventfd each, handed to a channel at its first trapped ring through
  the notifier entry) on Cloud Hypervisor 0001-0006: 120 trapped doorbells in a run of 284612
  guest doorbell writes, 0 trapped BAR1 accesses, short kernel 0.050 to 0.018 ms per launch,
  rlprobe stage 4 11.2 s to 3.0 s (native 1.3 s, v1 3.4 s), transfers 257 to 970 per second,
  0 Xid. In pure emulation before that: 253 datamatch ioeventfds registered, nvprobe 2 with 20
  channels each trapping exactly once.
- 2026-09-24: **P21 BAR1 passthrough, prototype on the GPU path** ([bar1-passthrough.md](bar1-passthrough.md),
  benchmark.md, `caraxes/runs/p21-bar1-2026-09-24/`): guest BAR1's first MiB as one sparse
  mmap area of region 1 backed by an NVIDIA dma-buf of the reservation piece the traces showed,
  spec-conformant, mapped once by Cloud Hypervisor v53; BAR1 reads inside the area 47.9 us to
  1.4 us, no trapped GPFIFO or GPPut write in 2 reps of short kernel and rlprobe, rlprobe stage 4
  19.6 s to 9.0 s, 0 mismatched pages, 0 page-table hazards, 0 Xid, the host BAR1 use released
  at exit. Not combinable with the ioeventfd doorbell until the datamatch token pool exists
  (the device refuses both switches together); the target design H with a private pool and
  guest-page remapping is the next step after the pool.
- 2026-09-24: **Doorbell over KVM ioeventfd on the GPU path** (patch 0005, device 3bcfb35 plus
  the watched GPPut copy; vm1 under load, `caraxes/runs/doorbell-2026-09-24-vm1-watch/`): the
  doorbell write falls from 29.3 us to 3.9 us p50, no trapped doorbell in 196k submissions, the
  token-less scan costs 1.07 us per wakeup on 19 channels after the watched copy (31 us before
  it, through uncached BAR1 reads), the short kernel goes from 0.159-0.176 to 0.144-0.153 ms per
  launch; the remaining three BAR1 traps per submission are P21's.
- 2026-09-24: **P17 default switched**: both devices now name the card
  `Leandro <Transport> <Personality>-<Profile>` by default (`Leandro VFIO RTX 2070-4G`,
  `Leandro VirtIO RTX 2070-4G`); the legacy `Leandro RTX 2070-4G` stays an explicit opt-in
  (`LEA_GPU_NAME_FORMAT=legacy`, `--name-format legacy`, `CARAXES_NAME_FORMAT=legacy`) and
  `naming::parse_name` reads both. The v1 gates in `../Leandro-Test` accept either format
  through one regex helper (`lea_mediated_name_re`, operator's permission of 2026-09-24).
  The Caraxes GID is still seeded from the legacy name, so the guest UUID is unchanged.
- 2026-09-24: **Blackwell N gate passed in pure emulation** ([blackwell-gate.md](blackwell-gate.md),
  [gsp-boot-blackwell.md](gsp-boot-blackwell.md), experiments.md rows b0-b8, evidence in
  `caraxes/runs/blackwell-2026-09-24/`): the device presents the RTX 5060 Ti of the lab capture
  (`--board-data rtx5060ti`, `CARAXES_CHIP=gb206`; MSI-X unlinked, BAR1 256 MiB, PBI mailbox
  and DSN bytes redacted) and emulates the FSP Chain-of-Trust boot (`src/chip/boot_fsp.rs`),
  the GMMU v3 page tables (`src/gmmu_v3.rs`, selected additively in fb.rs) and four
  physical-RM answers from the capture (`src/chip/physical.rs`). A stock 610.57.04 guest on
  vm1 reached RmInitAdapter, `nvidia-smi -q` ("Leandro VFIO RTX 5060 Ti-4G", 4096 MiB, VBIOS
  98.06.1F.40.4A) and UVM's first BLACKWELL_CHANNEL_GPFIFO_B allocation; no host GPU was
  touched. Next: the CA6F/CAB5 method subsets (the first push hangs), full KGR static info.
- 2026-09-24: **Display phase E: a GPU-rendered Wayland desktop** ([display-e.md](display-e.md),
  experiments.md "Display phase E", evidence `caraxes/runs/display-e-2026-09-24/` and
  `caraxes/runs/display-f-2026-09-24/`). The stock NVIDIA EGL/GL/Vulkan stack renders on the
  RTX 2070 through the shadow GR path (bare CE channels mirrored, size-0 context buffers
  excluded, depth kinds carried); Weston GL at 58-60 Hz, GNOME on the NVIDIA EGL, the display
  gate passes 11/11 with host execution (`gate-display-2026-09-24-7`). Series F on main
  `8cca8e6`: demos on Weston vkcube-wayland 44.1 FPS (33.9), glxgears 57-60, glmark2 1804 (1216);
  on GNOME 31.1 (22.8), 33-37 (14-21), glmark2 34. Per presented frame Weston traps 157 times,
  Mutter about 670: 2-3 doorbells and GPPut writes, no GPFIFO BAR1 write, no shadow plan; the
  rest is the guest RM servicing interrupts (about 70 traps per MSI) and, for Mutter, 71 BAR2 PTE
  writes, about Mutter's whole frame budget at 23-27 us per trap. The doorbell ioeventfd removes
  the doorbell traps and leaves Mutter at 35-38 Hz (30-41 without). Stream stage (Sunshine
  `capture=kms` + libx264, Moonlight on the host, bench.sh's summary): valid, 25.85 FPS, host
  latency avg 101 ms, 0 % network drops, 9.8 Mbit/s, 2.09 guest cores. Pointer:
  `CARAXES_INPUT=fifo` adds virtio-input through `--generic-vhost-user` without a hypervisor
  patch. 1 Xid 31 (device pid) at one GNOME start, open (display-e.md §9).
- 2026-09-24: **Display front-end page measured on vm0** ([intr-page.md](intr-page.md) §10.6,
  `caraxes/runs/display-page-2026-09-24/`). With the interrupt page and the display page
  (`CARAXES_DISPLAY_PAGE=1`) Mutter goes from 230 to 140 trapped accesses per frame at 59.9 Hz
  and Weston from 106 to 31 at 57.4 Hz; five GNOME starts per series, 7615 awaken markers all
  acked through the page, `inconsistencies: none`, 0 host Xid. Weston's remaining eleven traps
  per frame and its wider frame spread are the next look.
- 2026-09-24: **GSP surface, categories E and A** ([gsp-surface.md](gsp-surface.md) "After E and
  A", merge e0ce42f). Routed controls answered: 176 to 373 on the 610 ABI, 175 to 383 on 615. The
  189 (200) controls of category E are refused with NOT_SUPPORTED as feature families a chip
  declares absent (`chips/tu106/chip.toml` `[features]`, `src/chipctl.rs`); 8 of category A are
  filled from the card's native traces and the chip files (`chips/tu106/controls.toml`); ids and
  offsets are generated per driver version from the SDK headers. One correction is already known:
  `CE_GET_LCE_SHIM_INFO` sits in the `internal_uncalled` family, but the 615 user space calls it
  during cuInit (cutover-615.md, run 4); it moves out with the 615 cuInit work.
- 2026-09-24: **GSP plan implemented, RPCs, events, classes, boot exchange**
  ([gsp-plan/](gsp-plan/README.md), a "Status" section per document; runs in
  `caraxes/runs/gsp-rpc-2026-09-24/`). `ALLOC_MEMORY` is answered from a registry of the
  guest's pages, so the 615 operational-event import completes (0x90CD, 0x90D0 and their
  controls answered); the six conditional RPCs and `UNLOADING_GUEST_DRIVER` get the plan's
  answers; the v2 transport sends GMCAPI requests, `RC_TRIGGERED` (64 bytes on 615, 48 on 610)
  and `OS_ERROR_LOG` come from the error notifiers of host-mirrored channels (never from the
  host-global RC subscription); classes outside the TU106 class database are refused as GSP-RM
  refuses them; the kernel's TSG ids are recorded; `GSP_INIT` carries the static keys with a
  source, three registry keys are honoured, Booter Load is told from Unload by its own signal,
  the unstable-interface opt-in is checked, and a PM suspend resumes with `GSP_RESUME_DONE`
  (unit-tested only). Pure-emulation 615 boot: init done, no new refusal. GPU gates on vm3:
  610 guest nvml as before (power N/A), cuda 10 of 11 (managed memory); 615 guest nvml pass,
  cuda blocked by the known `CE_GET_LCE_SHIM_INFO` gap of another branch. 0 host Xid. Open: B6
  (GSP-RM log lines), MMU_FAULT_QUEUED and the model-dependent events.
