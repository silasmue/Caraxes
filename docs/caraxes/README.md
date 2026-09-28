<!-- SPDX-License-Identifier: MIT -->
# Caraxes: a synthetic NVIDIA PCI device for stock guest drivers

The working record of VFIO Leandro (Leandro v2, code name Caraxes), started
2026-09-23: what was found in source, what was measured on the host, the decisions
taken, the experiment ladder and the status log. The entry points are the
[repository README](../../README.md), the overview of both architectures in
[LEANDRO.md](../LEANDRO.md), the [wiki](../wiki/README.md) and the
[research notes](../research/README.md); this directory holds the long form behind them.

## Goal

```
unmodified NVIDIA guest driver (tested Linux desktop: 615.71.09)
  → synthetic NVIDIA PCI device served over vfio-user (Cloud Hypervisor client)
  → synthetic GSP-RM endpoint (message queue + RPC, emulated in userspace)
  → existing Leandro RM backend (crates/vhost-user-nvrm) → host driver → RTX 2070
```

Constraints (from the project brief): do not modify the host driver; no guest
driver patches except as a temporary aid; the stock (non-GRID) driver; the
frontend lives in userspace via vfio-user; existing Leandro must keep working;
reuse the RM backend. Current priority: Windows driver initialization, followed
by desktop stability and latency on Linux and Windows.

## Current verified configuration

As of 2026-09-27, the preferred Linux path is Cloud Hypervisor with Ubuntu 24.04,
the distribution-packaged NVIDIA 615.71.09 driver, GNOME Wayland and Sunshine KMS
capture with NVENC. The guest uses stock NVIDIA modules and does not load
`caraxes_dma_range`; `CARAXES_GUEST_DMA_GPA=1` selects registered guest RAM with
GPA addresses. The older helper-based profile remains available.

Short measured trials established the following:

- One helper-free guest ran GNOME Wayland, native NVIDIA Vulkan, Sunshine NVENC
  and an NVENC-to-NVDEC round trip together.
- Two guests with 2 GiB reserved VRAM each streamed concurrently and completed overlapping NVENC,
  NVDEC and Vulkan checks. Both Moonlight sessions received about 58 FPS against a
  60 FPS request.
- These are functional and short concurrency checks. They do not establish native
  performance, long-term stability, hostile-tenant isolation or supported-alpha status.
- Windows currently reaches Code 43 diagnostics. Windows rendering has not passed.

Commands and limits are in [quickstart.md](quickstart.md). Measurements and exact
test conditions are in
[wayland-multiguest-2026-09-27.md](../research/wayland-multiguest-2026-09-27.md).

## Files

| File | Contents |
|---|---|
| [../LEANDRO.md](../LEANDRO.md) | The overview of VirtIO Leandro (v1) and VFIO Leandro (v2): the problem, the two architectures side by side, the driver as a virtual GPU sees it, the GSP and its emulation, PCI and BARs and memory, submission and interrupts, execution, display, identity, performance, verification, and the ranked weaknesses with their mitigation paths |
| [../wiki/](../wiki/README.md) | One page per concept LEANDRO.md names (29 pages and an index: the RM, the GSP, the BARs, the memory model, submission, execution, display, video engines, identity, performance, verification, captured cards, contributing a capture, glossary), driver references by path and function, links to the research notes and to this directory |
| [../research/](../research/README.md) | The research knowledge base: one short note per topic (driver structure, GSP surface, transport, RPCs/events/classes, ABI baseline, identity and costume, feature families, memory model, windows and traps, isolation and Xids, performance, methods, Blackwell and Hopper), bullets with evidence, 615.71.09 only; this directory stays the detailed working record |
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
| [memory-model.md](memory-model.md) | E5b part 1: guest FB addresses inside a fixed host VRAM reservation, guest RAM registered as host OS descriptors, the IOVA table for `caraxes_dma_range`, the device-side translation, privilege and teardown; §7 the shadow's host RM calls (gather descriptors for scattered sysmem, whole-region frees, the region free list, the fail-closed argument, measurements) |
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
| [display-615.md](display-615.md) | The 615.71.09 guest's display path on vm0-615 (2026-09-24/25): the overlay with the host's 615 GL/EGL/Vulkan userspace and a rebuilt nvidia-drm.ko (the image's DKMS module needs drm_display_helper.ko, which the image lacks), the 615 display layouts in `ver/v615/display.rs`, the display gate 11/11, the per-frame trap tables against series C (Mutter 138.92 against 139.86, Weston 29.25 against 31.08), Q1's 615 half (DP_SET_WAR_FLAGS at plug and unplug, HDCP_CTRL at unplug, nothing unanswered) |
| [display-ab.md](display-ab.md) | Display phases A and B on vm0 (D15): the display in the GSP role (`--display`; `src/display/`, `chips/tu106/display.toml`, the board's `[display]` connector, the `crates/dp-sink` sink behind an NV0073 AUX adapter); gate D3 passed (`card0-DP-1` connected, EDID byte-identical and edid-decode clean, 1080p60/720p60/1080p50, the over-budget 8K120 VIC pruned, no modeset); every display RPC/control in order with its answer and label (D2); the recorded AUX capture replaced the hand-written transcript; what phase C needs (core notifier, window completion, four modeset-path controls) |
| [experiments.md](experiments.md) | The experiment ladder, starting with the smallest one, each with a falsifiable expectation |
| [kickoff.md](kickoff.md) | The brief for the implementation sessions: rules, reading order, E0 step by step, run recipe, parked ideas |
| [parallel.md](parallel.md) | P20, the parallelism proof: two reserving VMs in fixed slots run the cuda gate concurrently beside the desktop; isolation by construction and by pattern test, the host's scrub on free proven for the reservation, fail-closed teardown, the D11 refusal of a third VM; the BAR1 leak and deadlock found and fixed, and two open sysmem-path findings (mapping scale, the chunk-crossing hole) |
| [mapping.md](mapping.md) | The mapping package (2026-09-24): incremental shadow walk with write-through table copies, coalesced RM maps with a per-device budget, resync on a worker thread with a doorbell barrier, IOVA = GPA + one offset and CE channels on the filtered shadow (the two P20 §8 fixes), the BAR aperture TLB, the doorbell and context-creation accounting (96k trapped BAR2 writes per cuCtxCreate), the CeUtils experiment (§9: the guest driver does not load without `RmDisableGlobalCeUtils=1`, keep the key) |
| [nova-vgpu-upstream.md](nova-vgpu-upstream.md) | The upstream nova-core vGPU manager series (v2, 32 patches, 2026-09-14) read patch by patch against the device: plugin ABI and RPC discipline, VRAM/channel allocation, CeUtils scrubbing and fail-closed teardown, VMMU segment, FIFO engine table, VF routing, migration, logging; ranked actionable differences and a cross-check of the GSP message queues (subagent report, P19) |
| [blackwell-gb206.md](blackwell-gb206.md) | The Blackwell lab capture (RTX 5060 Ti, GB206, driver 615.71.09, korngold, 2026-09-24, filed under `caraxes/reference/gb206/`) read item by item against TU106; the D14 work list for a GB206 identity with five draft chip tables under `chips/gb206/`; the 610.57.04 driver supports GB206 (gsp_ga10x firmware, boots GSP through FSP, no ROM parsing); 615.71.09 redesigns the CPU-GSP transport (queue v2 in registers, MCTP-framed elements, one GMCAPI init instead of the four init RPCs) and changes seven control structs; the 615 gsp-abi-sizes record; a nine-step N-gate plan of 27 to 39 days that stops at "driver up" without a Blackwell card here (subagent report) |
| [guest-driver-versions.md](guest-driver-versions.md) | Other guest driver versions against the 610.57.04 device (2026-09-24, vm3): Ubuntu's recommended 595.91.07 stops at the first queue element (pre-610 element header) and comes up with three `CARAXES_GUEST_ABI=595` switches; 610.43.02 works as is; 615.71.09 stops at the status-queue link, with the captured GMCAPI_CMD_GSP_INIT; size table, version detection, D14 estimate (subagent report) |
| [bar1-passthrough.md](bar1-passthrough.md) | P21: what the guest does with BAR1 (4-byte GPFIFO and GPPut writes at one traced layout, tables through BAR2), Cloud Hypervisor v53's sparse-area mechanics, the NVIDIA mmap rule and why an RM mapping fd is unsafe, the dma-buf export as the region descriptor, rejected candidates, target design H (private pool, guest pages remapped), prototype G behind `CARAXES_BAR1_PASSTHROUGH=1` with its checks, measurement plan and results (subagent report) |
| [host-resources.md](host-resources.md) | The host VRAM floor (`CARAXES_HOST_VRAM_FLOOR_MIB`, default 768, fail closed, one gate for every host allocation and map), no retry storms on NO_MEMORY (no split, one attempt, back-off), the leak audit table (allocation, owner, freed when, counter) with five leaks fixed, and the soak line with `caraxes/tools/soak-watch.sh` (2026-09-25) |
| [bar1-pool.md](bar1-pool.md) | P21 design H behind `CARAXES_BAR1_PASSTHROUGH=pool` (`CARAXES_BAR1_POOL_MIB`, default 64): a private pool below the carve-out, taken out of the guest's usable FB, backs guest BAR1 `[0, pool)` as one dma-buf sparse area; every page the guest maps there lives at its pool page in every device translation (FB store, shadow leaves, host USERD; `FakeFb::remap`), copied in at map and back at unmap; pages the GPU maps move only behind the idle-and-preempt barrier with a shadow re-sync, USERD pages and failed barriers are refused and retried (fail closed); isolation argument, measurement plan against the 1 MiB area (copy, rl, ppo, short-auto, P21 checks), unit tests (subagent report, not yet run on a VM) |
| [bar2-memfd.md](bar2-memfd.md) | D18's BAR2 line behind `CARAXES_BAR2_MEMFD=1`: region 3 as a sealed memfd with sparse areas around the trapped flush page; FB pages mapped through BAR2 live in the memfd (`FakeFb::backing`), synced at the RM's flush reads, MMU invalidates, BAR2 binds and GSP doorbells by a snapshot compare that writes back only the changed bytes and marks the shadow's walk-cache pages dirty; why soft-dirty, userfaultfd write-protect and mprotect on the device's mapping cannot see guest writes (KVM dirty logging would, with a VMM change); what stays trapped, the hazards, fail-closed setup, tests, measurement plan; measured 2026-09-25: 0 trapped BAR2 writes, cuCtxCreate 0.53 s, hangs together with the BAR1 pool (subagent report) |
| [display-pacing.md](display-pacing.md) | Why GNOME flipped at 15-35 Hz on the 60 Hz display clock (2026-09-26): per-frame timeline from the invhold traces (tick exact, flips latch at the first tick, latch to next kick 20-54 ms), cause: about 23 trapped BAR2 flush reads per frame in the compositor commit path, each a full memfd sync (0.2-0.5 ms); fix: flush reads defer their sync to the next hand-over (invalidate, RPC or channel doorbell, 1 ms flush clock), `CARAXES_BAR2_MEMFD_DEFER_FLUSH`; the Sunshine side; what the GPU run must show |
| [gsp-transport-615.md](gsp-transport-615.md) | The CPU-GSP transport of driver 615.71.09 from source (NV_ABI_STABLE): message queue v2 with the pointers in NV_PGSP_QUEUE_HEAD/TAIL(0) and MSGQ_HEAD/TAIL(0), the 24-byte MCTP-framed element without checksum or sequence number and what replaces them, GMCAPI and GMCAPI_CMD_GSP_INIT with its NVKV request and response key by key, the 615 boot arguments, what stays as in 610, the seven changed control structs and the reused id 0x20800A36, how the device tells the version, a 610 against 615 table for every structure the device touches, the device's v2 transport (`msgq::v2`, `gmcapi`, `ver::v615`, `CARAXES_GUEST_ABI=615`) and its tests, with the generated 615 gsp-abi-sizes tables (subagent report) |
| [repo-split.md](repo-split.md) | The plan for separating v1 (Leandro) from Caraxes after the operator's 615 decision: inventory of every path and crate (v1-only, Caraxes-only, shared, with a corrections table for paths the fork no longer has), the port-back commits Leandro must receive (the rewritten vhost-user series, the P17 naming model, one nvrm-abi hunk), the shared-crate options with the recommendation B (shared crates stay in Leandro, Caraxes pins a Leandro commit as a git dependency), the step order that keeps both repositories building, the 615 switch placed in it, and eleven silent-failure risks with a command checklist (subagent report) |
| [blackwell-gate.md](blackwell-gate.md) | The Blackwell N gate in pure emulation: the GB206 identity (`--board-data rtx5060ti`, `CARAXES_CHIP=gb206`), stages N0 (device) to N4 (first channel allocation), the predicted walls, preconditions and the exact vm3 launcher command; defined 2026-09-24, N0 to N4 passed on vm1 the same day (its section 4, experiments.md b4 to b8) and again on vm7 after the rig fix of 2026-09-25 (its section 5) |
| [costume-5060.md](costume-5060.md) | The 5060 costume (`--board-data rtx5060ti-on-tu106`, `CARAXES_COSTUME=rtx5060ti`): the RTX 5060 Ti's PCI ids, BAR0 size and name on the TU106, whose boot registers, config layout, GSP boot, GMMU, classes, VBIOS and compute capability 7.5 stay true; the costume-or-truth table, every place in the 610 RM where the costume could be caught (the fixed XVE link offsets did catch the whole GB206 config space), and the vm1 check: nvml and cuda gates as on the 2070 identity, short kernel and rlprobe |
| [costume-ladder.md](costume-ladder.md) | The costume ladder (`CARAXES_COSTUME=rtx2080|rtx3060|rtx4060ti|h100|rtx5060ti-gb206`, `CARAXES_SM_VERSION`): chips presented over the executing TU106 as overlays on its (or GB206's) tables (`src/chip/derived.rs`, `chips/{tu104,ga106,ad106,gh100}`), ladder boards with `host_chip` and a class map for the mirror; per rung what is lied, what stays true, the first failure with its source line and the verdict; the pure-emulation baseline wall; the lesson which layers the identity can lie on |
| [limits-mig-ecc-vgpu.md](limits-mig-ecc-vgpu.md) | The limits runs of 2026-09-25: ECC, MIG and vGPU modelled from the kernel source (`src/families/`, the GH100 rung and the new GA100 rung `chips/ga100`, `CARAXES_COSTUME=a100`) and driven by nvidia-smi in pure emulation; per command works / caught by X / breaks at Y with the source line; what each model needs for real execution; what stays impossible on this rig |
| [cutover-615.md](cutover-615.md) | The checklist for the host driver upgrade 610.57.04 to 615.71.09 (2026-09-24): the device's run-time host-ABI selection (`hostabi.rs`, `CARAXES_HOST_ABI`, the log line to look for), then in order the 610 closing benchmark and frozen binaries, the operator's upgrade checks, the 610 guest on the 615 host (nvml, cuda, display gates), the 615 guest once `ver/v615` lands (`CARAXES_DRIVER=615`), v1's wait for L615, rollback, open points |
| [v1-615-image.md](v1-615-image.md) | The Leandro v1 guest image for 615.71.09 (`guest-v1-2026-09-24-615.71.09.qcow2`, made from the first v1 615 run's overlay without booting): what differs from the Caraxes 615 base image (guest nvidia modules blacklisted, `nvidia-modprobe` diverted, `nvidia-persistenced` masked, `/opt/nvrm` on the 615 system libraries), why not nix offline, `bench-v1vm.sh` taking v1's material from Leandro (`LEA_V1_ROOT`, `LEA_V1_BIN`, `NVRM`, driver selection, dry run), `LEA_MANAGED_COMPAT=1` as in the gpu gate, `v1-gates.sh`, the boot command |
| [video-engines.md](video-engines.md) | NVENC and NVDEC through the mirror: protocol and layout, dated failure analysis, passing single-guest probes, and the 2026-09-27 two-guest Wayland concurrency checks; figure `figures/video-engine-mirror.svg` |
| [sottr-stage.md](sottr-stage.md) | The SotTR stage (2026-09-24, roadmap item 2): vm4 as the game VM (3 GiB profile at slot 0, tap `caraxes3`, display configuration C, main 3cb33af device), the v1 Steam library as a thin overlay (`CARAXES_GAMES_DISK`, writer lock in the Caraxes store), the guest preparation of a fresh 610 overlay, Steam library registration and autostart, Sunshine kms, automatic Moonlight pairing and window, what is recorded and how it compares with v1's sottr-4q; plumbing test in pure emulation |
| [gsp-surface.md](gsp-surface.md) | P23, the GSP surface of 610.57.04 from source: 829 of 1377 exported controls reach GSP, the device handles 171 with D9 provenance, the 658 unhandled sorted into five categories (pure chip data, host-forwardable, guest-state derived, needs modelling, never expected on TU106) with a recipe and estimate each, the 20 RPC functions and 41 events a GSP client uses, the top ten a Linux CUDA or display client hits next, what Windows and nova-core share; produced by `caraxes/tools/gsp-surface.py` (subagent report, 2026-09-24) |
| [gsp-plan/](gsp-plan/README.md) | P23 implementation plans for the GSP surface on 615.71.09, one document per part of gsp-surface.md (index in gsp-plan/README.md): RPC functions (14 of 232 used by a 615 GSP client on TU106, 7 answered, `ALLOC_MEMORY` sent at every boot and refused), events (45, the device raises only POST_EVENT for hotplug), classes reaching GSP (143 on the wire, 48 on TU106), the boot exchange with teardown, suspend/resume and what `NV_ABI_STABLE` covers; category B (70 host-forwardable controls: 30 real forwards, 11 device-owned, 12 guest-object, 13 refused, 4 never sent) and C (31 guest-state controls plus 12 from B); category D (308 controls needing a device model, grouped by subsystem with estimate and rank: no CUDA or Wayland workload has reached D beyond `HDCP_CTRL` on unplug and one cuda-gdb control; 126 belong to A, C or E; roadmap R0 rule table, R1 display, R2 CUDA error paths, R3 games, R4 suspend, R5 tools); per item recipe, truth source, isolation and open questions (subagent reports, 2026-09-24) |
| [gsp-abi-615.md](gsp-abi-615.md), [gsp-abi-615-tables.md](gsp-abi-615-tables.md), `caraxes/abi/` | D20, the GSP ABI baseline 615.71.09: what NVIDIA tags `NV_ABI_STABLE` (12 structs, 5 enums, file and line) and what the tree calls an opt-in unstable interface (the RM RPC layer, the internal handles, unpromoted NVKV keys); the snapshot `caraxes/abi/gsp-615.71.09/` (controls, RPCs, events, classes, boot types, layouts, constants, enums, bit ranges as TSV, MANIFEST) and `gsp-610.57.04/`, made by `caraxes/tools/gsp-abi.py`; the counts (844 routed controls, 777 answered, 67 unhandled, as counted from the snapshot's `controls.tsv`, which is the source for these numbers; 232 RPC functions, 14 sent by a 615 client, all answered; 45 events; 149 GSP-bound classes); the 610-to-615 delta as the worked example (every change the plans found is reproduced; what the diff cannot see and why); the workflow for the next driver and the `tools/check.sh` drift step; the tables file is the generated human-readable form of the snapshot |
| [doorbell-poll.md](doorbell-poll.md) | The doorbell reactor (`CARAXES_DOORBELL_POLL=off\|spin\|hybrid`, `_POLL_CPU`, `_POLL_IDLE_US`): one thread polls the token pool's eventfds instead of sleeping in `epoll_wait`, pinned to an automatically chosen core; the vm1 measurement of blocking, spin unpinned, spin pinned and hybrid (short kernel 0.026 to 0.014 ms, rlprobe stage 4 2.7 to 2.1 s, hybrid at 3 % of a core), host tuning recommendations for the reactor core, and what P24 would change |
| [workload-gate.md](workload-gate.md) | The workload gate (2026-09-24, prepared; first run 2026-09-25 on the 615 guest at 1 GiB, §7): real workloads beside the micro rows, native against v2 (v1 on a 610 host only): PPO CartPole at 420 s, llama.cpp pp512/tg128 with and without CUDA graphs (host-built llama-bench for sm_75, Llama 3.2 1B Q4_K_M), a GPT-style training loop, ResNet-18 training; the one-command runner `caraxes/runs/workload-2026-09-24/run.sh`, how to read the ratios, the table to fill |
| [intr-page.md](intr-page.md) | The interrupt page (`CARAXES_INTR_PAGE=1`, `_SLEEP_MS`, `_RETRIGGER_MS`): BAR0 page 0 and the VF CPU interrupt tree page offered as sparse mmap areas of region 0 from a memfd, the reactor applying the guest's acks and enable writes by polling (tag bits and sentinels so every store is visible, a retrigger watchdog for what stays ambiguous); per-frame register census of series F (437 of 662 Mutter traps are these two pages), the expected 662 to about 225, what stays trapped |
| [alloc-path.md](alloc-path.md) | Where cuMemAlloc/cuMemFree time goes (830/903 us, 6.7 ms for 64 MiB): no memory RPC reaches the device, the cost is the guest RM's trapped traffic (24 traps per mapping change: 18 RM-lock interrupt-enable writes, 5 MMU invalidate, 1 flush; plus BAR2 PTE writes), the device's host calls sit in the worker; the four steps (existing intr page + BAR2 memfd, lazy shadow plans, invalidate and flush page as memory, region-object free list) with expected cost and isolation argument; `CARAXES_ALLOC_LAZY=1` built (`lazy.rs`), `caraxes/tools/rpc-cost.py`, `BENCH_CU_PHASE_GAP_MS`, measurement plan (subagent report) |
| [optimization-backlog.md](optimization-backlog.md) | Trap census of 2026-09-25 (vm3, main 1ea2529): every CUDA driver-API operation alone (`caraxes/tools/census-cu.c` with BAR0 scratch markers, `census-table.py`), traps by BAR, page and register, RPC and host RM work, guest against native, with no window, with interrupt page + BAR1 pool + token pool, and with the BAR2 memfd; findings (memfd plus pool hangs the first cuCtxCreate, memfd alone takes cuCtxCreate 4.1 s to 0.87 s, submissions and copies trap 0 times with the windows, 140-160 MSI re-arm traps per second, non-trap stalls); the ranked backlog with mechanism, saving, risk and estimate; the ReBAR note; the order for the next three days |
| [coverage-map.md](coverage-map.md) | The coverage map of 2026-09-25 (vm2, 610 guest): the device's profile (`CARAXES_PROFILE=1`, src/profile.rs: traps by BAR/page/register, controls, allocs, RPCs, events, host RM calls and window hits per device and per guest client, dumped on SIGUSR2 and at exit, 84 ns per trapped access) and `caraxes/tools/profile-rank.py`; 29 workload windows beyond the gates (nvidia-smi loops, graphs, streams, texture/surface, torch cuBLAS/cuDNN/attention, two processes, OOM, kill -9, illegal address, NVRTC, Vulkan, idle) with LLVM function coverage; the union by provenance and the untested surface; the guest control sweep (`caraxes/tools/ctlsweep.c`); ten ranked candidates outside the benchmarks; the recipe for the next workload |
| [gsp-transport-classes.md](gsp-transport-classes.md) | The GSP controls of 615.71.09 by transport class (2026-09-25): per routed control the caller kind, frequency from the profiles and 615 device logs, sizes, embedded pointers/handles/addresses, answer source, side effects, a verdict ZT-PAGE/ZT-QUEUE/HOST/TRAP/REFUSE with a reason code and an isolation note (`caraxes/abi/gsp-615.71.09/transport.tsv` from `caraxes/tools/gsp-transport.py`); one 615 RPC is four trapped accesses (108 us median) against 3 us of device work; controls are lifecycle traffic (0 per frame); the answer page is not worth a driver change, but static GSS-legacy answers can be cached by a stock guest through the reply's `rmctrlFlags`; the polled queue needs page 0x110000 as a read-only area plus write ioeventfds (about 130 us per RPC); what cannot be optimised (490 REFUSE, 32 TRAP) |
| [alloc-path.md](alloc-path.md) | Where cuMemAlloc/cuMemFree time goes (830/903 us, 6.7 ms for 64 MiB): no memory RPC reaches the device, the cost is the guest RM's trapped traffic (24 traps per mapping change: 18 RM-lock interrupt-enable writes, 5 MMU invalidate, 1 flush; plus BAR2 PTE writes), the device's host calls sit in the worker; the four steps (existing intr page + BAR2 memfd, lazy shadow plans, invalidate and flush page as memory, region-object free list) with expected cost and isolation argument; `CARAXES_ALLOC_LAZY=1` built (`lazy.rs`), `caraxes/tools/rpc-cost.py`, `BENCH_CU_PHASE_GAP_MS`, measurement plan; measured 2026-09-25 (§7): 45.5 to 4.3 traps, cuMemAlloc 1 MiB 972 to 175 us (subagent report) |
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
| D20 | **615.71.09 is the recorded GSP ABI baseline; later driver versions are handled as diffed deltas against `caraxes/abi/`.** The baseline is a generated, diffable snapshot of the 615.71.09 tree (`caraxes/abi/gsp-615.71.09/`: every control with its params size, layout hash, routing, privilege, category and the device's answer; every RPC function, event and class; the `NV_ABI_STABLE` and boot types with sizes, field offsets and version constants; the GMCAPI keys and stable enumerators; the control flag-word bit ranges), made by `caraxes/tools/gsp-abi.py snapshot`; a new driver is snapshotted and diffed against it (`gsp-abi.py diff`), and its per-version tables, `hostabi.rs` row and gates follow from the delta ([gsp-abi-615.md](gsp-abi-615.md) §6). A change in the stable layer (an `NV_ABI_STABLE` struct, the queue element framing, the boot arguments, the GMCAPI header or a promoted key) is a baseline break that needs a decision, not a routine delta. `tools/check.sh` fails when the snapshot drifts from the 615 tree or the tables document from the snapshot. | 615 is where NVIDIA starts tagging the transport and boot layer stable (D19); from here the boundary should change only incrementally, like a stable ABI with hardware additions, and a recorded, machine-readable baseline makes each increment a reviewed diff instead of a trace-driven discovery | taken 2026-09-25 (operator) |
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
| P23 | **Complete GSP surface (operator, 2026-09-24)**: emulate as many device answers as possible from source, so that another client of the same RM core (the Windows driver of the same generation, later nova-core) runs without trace-driven bring-up. [gsp-surface.md](gsp-surface.md) is the inventory and work list: of 829 controls that reach GSP the device answers 171; the rest splits into 62 pure chip data (one table-driven handler plus one capture per chip), 71 host-forwardable (a forward rule with a D9 deny list), 30 guest-state derived (registry and mirror), 306 needing device modelling (mostly display and profiler/debugger) and 189 never expected on TU106 (one NOT_SUPPORTED rule per feature family); roughly 40 to 60 agent-days, of which A plus E remove about 250 items in about 5. Missing RPC functions today: ALLOC_MEMORY, UNLOADING_GUEST_DRIVER, DUMP_PROTOBUF_COMPONENT, ECC_NOTIFIER_WRITE_ACK; the device never sends the RC_TRIGGERED, MMU_FAULT_QUEUED and OS_ERROR_LOG events. Open: whether the Windows 610 driver boots a TU106 with GSP-RM at all. | 2026-09-24, operator's direction After E and A (2026-09-24, [gsp-surface.md](gsp-surface.md) "After E and A"): the device answers 373 of the routed controls on 610 and 383 on 615; category E is refused per chip as feature families (`[features]` in `chips/*/chip.toml`, `src/chipctl.rs`), A comes from `chips/tu106/controls.toml` (8 filled, 52 listed with their source), the ids and field offsets are generated (`caraxes/tools/gsp-tables.py`, `ctrl-layouts.py`). Unhandled: A 52, B 70, C 29 (31 on 615), D 305 (308), E 0. The plans for B, C, D and the RPC surface are being written under `docs/caraxes/gsp-plan/`. Current (2026-09-25, the source is the ABI snapshot `caraxes/abi/gsp-615.71.09/controls.tsv`, [gsp-abi-615.md](gsp-abi-615.md) section 4): of 844 routed controls on 615 the device answers 777 and 67 are unhandled (A 50, C 1, D 16); on 610 763 of 829, 66 unhandled. |
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
- 2026-09-25: **The pure-emulation regressions of 2026-09-24 were the rig**
  ([blackwell-gate.md](blackwell-gate.md) section 5, evidence
  `caraxes/runs/emu-regress-2026-09-25/`): the ladder's vm6 lacked the guest module options of
  quickstart.md section 2 (`RmDisableGlobalCeUtils=1;RMDisableScrubOnFree=1;RmRcWatchdog=0`), so
  the TU106 met the global CeUtils self-test (memmgrMemSet timeout, mem_mgr.c:445) and the GB206
  the watchdog's GR object (`numGpcs > 0`, kernel_graphics_object.c:192). Without the options
  the declared good commit 61e0b6a fails too; with them the declared bad 7621cfb and main boot,
  so no commit was at fault and none was moved or re-categorised. With the options, the TU106
  (610 and 615 guest) reaches RmInitAdapter and nvidia-smi, the N gate passes N0 to N4, and all
  six costume-ladder rungs boot in pure emulation. The device now reads the 610 guest's
  SET_REGISTRY table and warns about a missing key; `tools/emu-boot-check.sh` (check.sh step
  `emu-boot`, `CARAXES_CHECK_EMU_BOOT=1`) boots the TU106 in pure emulation without a GPU.
- 2026-09-25: **The costume ladder** ([costume-ladder.md](costume-ladder.md), evidence
  `caraxes/runs/costume-ladder-2026-09-24/`): TU104 over the TU106 works on the GPU (nvml gate
  but power-number, cuda gate 10 of 12 (the gate script's count: the declared-unsupported cuda-gdb row is among the 12; documents that count only the 11 scored rows write the same result as 10 of 11, testing.md section 3, cuda) scored, 0 Xid). GA106 and AD106 take the Ampere/Ada HAL
  from BOOT_42 and were caught first by the VBIOS (the GA102 HAL runs FWSEC only from a V3
  boot-from-HS descriptor, kernel_gsp_falcon_ga102.c:204), then by four boot registers the new
  HAL reads; with `synth-vbios.py --desc v3` and those in the boot data both reach the point
  TU106 itself reaches in pure emulation. On the GPU the GA106 costume boots, nvidia-smi says
  compute capability 8.6 (the GR info says 7.5), and libcuda's first push breaks it: SET_OBJECT
  0xC7C0/0xC7B5 in the method stream against the mapped TU106 objects, host Xid 13 "Class
  Mismatch" and Xid 32 on the device's channels, with detail lines printed under other pids,
  which stopped the ladder there. GH100 on the FSP model needed Hopper's FSP-boot
  registers; Confidential Computing (present), the CCU shared buffer and the HUB PCE mask are
  asked but not fatal (which families GH100 declares present or absent is decided by
  `crates/vfio-user-nvgpu/chips/gh100/chip.toml` `[features]`, which lists `cc` present and
  `ccu` absent). GB206 over the TU106
  stops at the GMMU v3 shadow. The SM version in GR info does not reach CUDA (nvidia-smi and
  torch keep 7.5 with `--sm-version 7.0`). Pure emulation of the TU106 itself stops at the CE
  scrubber (the CeUtils VA space is freed before its channel runs), and the GB206 N gate's
  pure emulation regressed at main (`numGpcs > 0`, also with 7621cfb).
  Correction (2026-09-25): both pure-emulation walls were the rig, not the device, since the
  ladder's vm6 lacked the guest module options of quickstart.md section 2; with them the TU106
  reaches nvidia-smi, the GB206 passes N0 to N4 and all six rungs boot
  ([blackwell-gate.md](blackwell-gate.md) section 5).
- 2026-09-24: **The 5060 costume on the GPU** ([costume-5060.md](costume-5060.md), evidence
  `caraxes/runs/costume-2026-09-24/`): vm1 with the 1 GiB profile presents `10de:2d04` rev a1,
  subsystem 1043:8a08, BAR0 64 MiB and the name "Leandro VFIO RTX 5060 Ti-1G" (/proc Model
  "NVIDIA GeForce RTX 5060 Ti" from the guest's name table) on the TU106 with host execution;
  nvidia-smi and torch report compute capability 7.5, 36 SMs, architecture Turing. nvml gate:
  all but power-number (610 on 615); cuda gate 10 of 12 scored (cuda-managed as on the 2070
  identity); 0 Xid. The first run with the whole GB206 config space was caught by the Turing
  RM's fixed XVE link offsets (link width "25x"); the costume now takes only the ids
  (`ids_from`) and keeps the TU106 layout.
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
  offsets are generated per driver version from the SDK headers. One correction:
  `CE_GET_LCE_SHIM_INFO` was in no E family but counted as unanswered category A; the 615 user
  space calls it during cuInit (cutover-615.md, run 4), and it is answered now (entry below).
- 2026-09-24: **615 guest on the 615 host: cuInit passes, cuda gate at the 610 guest's result**
  ([cutover-615.md](cutover-615.md) "The 615 guest on the 615 host", records
  `caraxes/runs/cutover-615-2026-09-24/v2-615guest-r5/` and `-r6/`). Run 4 failed every cuda stage
  with `cuInit: 100`: the 615 libcuda asks `NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO` per CE, a control
  610 exports PRIVILEGED and 615 NON_PRIVILEGED (same 12-byte struct), and the device refused it.
  It is now forwarded to a 615 host and answered from the chip's shim table otherwise (`cuda.rs`,
  `hostabi::HostLayouts::lce_shim_info_public`, `chips/tu106/chip.toml` `[ce] lce_shim`); the
  host's answers equal the table (shim 0, local index = LCE). `GPU_QUERY_ECC_STATUS` stays
  forwarded; its 0x56 is the card's (no ECC), as for the 610 guest. Run 5 on a 1 GiB profile:
  nvml pass, cuda 10 of 12 scored pass (cuda-managed 801 as on 610, cuda-gdb unsupported), the
  610 guest had 9; 0 Xid. Open: the bench series still fails with cuInit 803 through
  `LD_LIBRARY_PATH=/opt/nvrm/lib`, a harness setting.
- 2026-09-24: **SotTR stage prepared** ([sottr-stage.md](sottr-stage.md), scripts and plumbing
  evidence in `caraxes/runs/sottr-2026-09-24/`). `tools/caraxes-vm.sh` attaches a Steam library
  disk (`CARAXES_GAMES_DISK`: a thin overlay per VM; `CARAXES_GAMES_DISK_RW=1`: the base under a
  writer lock in `/mnt/vmstore/caraxes/locks/`). vm4 (3 GiB, slot 0, `caraxes3`, 8 GiB, 8 vCPUs)
  mounts v1's `LEAGAMES` disk at `/games` and has `/games/SteamLibrary` registered in Steam's
  `libraryfolders.vdf`; the game is the native Feral port. `up.sh`, `stream.sh` (pairing through
  Sunshine's API only when needed, Moonlight window on the host desktop) and `down.sh` are ready
  for the operator. They need vm0 and vm2 down. Plumbing in pure emulation passed: guest
  preparation, `RmInitAdapter succeeded`, games disk from fstab, Steam launcher at its installer,
  sniper runtime, pairing and a 57 FPS Moonlight stream from a stand-in x11 Sunshine, down with
  the recorded pid. Not testable there: KMS, GNOME and the kms capture, because nvidia-drm needs
  the host RM. The GPU run is the operator's.
  offsets are generated per driver version from the SDK headers. One correction is already known:
  `CE_GET_LCE_SHIM_INFO` sits in the `internal_uncalled` family, but the 615 user space calls it
  during cuInit (cutover-615.md, run 4); it moves out with the 615 cuInit work.
- 2026-09-24: **GSP surface, categories B and C** ([gsp-surface.md](gsp-surface.md) "After B
  and C", [gsp-plan/b-host-forwardable.md](gsp-plan/b-host-forwardable.md) §8,
  [gsp-plan/c-guest-state.md](gsp-plan/c-guest-state.md) §8). Routed controls answered: 373 to
  462 on the 610 ABI, 383 to 473 on 615.
  - **The isolation fix first.** `FB_GET_OFFLINED_PAGES` and `FB_GET_REMAPPED_ROWS` handed the
    guest host FB pages; they now pass on only pages inside the VM's reservation (rows: none).
    `HostRmClient` no longer sends a control of another class to the host root client.
  - **B.** 26 plain forwards, 3 filtered forwards (scrub range, dynamic offlined pages, GPUMON
    process fields), 11 device-owned answers (5 of them chip data in `controls.toml`) and 17
    refusals. The 10 guest-object rows stay open.
  - **C.** Resets, the M-group settings and the interleave levels go only to the guest
    object's own host mirror. `MAKE_REALTIME` and `RESTART_RUNLIST` are recorded and never
    forwarded (decision Q-C7). 15 controls are refused. `NV_EVENT_BUFFER UPDATE_GET` waits for
    `ALLOC_MEMORY`.
  - **Decision under D9 (Q-B6).** The GSP heap statistics are the device's own, all zero; the
    host GSP's numbers are not forwarded.
  - **Q-B1.** The read-only sweep of the 30 forward candidates on the device's own host
    subdevice gave 15 NV_OK, 11 NOT_SUPPORTED, 3 INVALID_ARGUMENT for a zeroed request and 1
    GENERIC; no Xid (caraxes/runs/gsp-bc-2026-09-24/qb1-sweep).
  - **GPU gates.** They could not run: the host RM refused vm5's reservation at slot 6 and the
    device failed closed. In pure emulation, the nvml and cuda rows were unchanged before and
    after.
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
  610 guest had 9; 0 Xid. The bench series' cuInit 803 was a harness setting, closed in the
  next entry.
- 2026-09-24: **First 615 benchmark series; bench-run.sh picks the guest libcuda by driver**
  ([benchmark.md](benchmark.md) "First 615 series", [cutover-615.md](cutover-615.md) "Closed:
  the bench series", records `caraxes/runs/bench-615-2026-09-24/`). The 615 base image keeps the
  610.57.04 userspace in `/opt/nvrm/lib` with its linker entry disabled; the gate's probes find
  the 615 libcuda through the linker cache, bench-run.sh forced `/opt/nvrm/lib` and got cuInit
  803. bench-run.sh now takes the v2 directory from `tools/lib/driver.sh` like the gates (610
  unchanged, v1 unchanged; `bench-run.sh libdir`, checked in `tools/check.sh`). On vm2 with the
  1 GiB profile at slot 3, no P21 area, the 615 guest equals the 610 guest (short kernel 0.144
  against 0.307 ms/launch on a busier host, rlprobe stage 4 15.2 against 20.4 s, cuCtxCreate
  4349 against 4144 ms, PPO 6226 against 6718 steps/s); the gap to the closing series is the
  missing P21 area. 0 Xid. Next: the like-for-like with the area and the spin reactor.
- 2026-09-24: **GSP surface, category D rule table and display models** (gsp-plan R0 and R1,
  [gsp-plan/d-device-model.md](gsp-plan/d-device-model.md) §10, [gsp-surface.md](gsp-surface.md)
  "After D"). `src/dmodel.rs` gives the D controls the plan's labelled answer from a table
  generated per driver version out of `d-controls.tsv` (`caraxes/tools/gsp-dtable.py`): on 615,
  101 REFUSE, 37 DENY and 14 ACK answered, the display rows through the display's board table;
  the 102 rows that belong to E are 16 new families in `[features]` (GB206 declares six present),
  the 7 A rows are controls.toml rows or open rows naming their source, the 17 C rows are listed
  for the C work; the profiler classes are refused at allocation (INSUFFICIENT_PERMISSIONS), and
  no D id may enter a forward list (guard test). R1: `HDCP_CTRL` per cmd, `DP_CONFIGURE_FEC`,
  `GET_RG_STATUS`, `DP_SET_WAR_FLAGS` and `PERF_NOTIFY_VIDEOEVENT` ACK. Routed controls answered:
  373 to 630 on 610, 383 to 643 on 615; D unanswered 305/308 to 48. On vm0 (610 guest,
  `caraxes/runs/gsp-d-2026-09-24/`): display gate 11/11 before and after (one rerun: the first
  after-run failed only the fence-timing stage under two other VMs, with identical control
  answers); the unplug sends exactly `HDCP_CTRL` DISABLE_AUTHENTICATION, now answered; a kill -9
  of a CUDA process sends no D control (no 0x2080110B, no 0x20800177); the display gate itself
  sends `SET_ZBC_COLOR_CLEAR` three times (ZBC model, R3). No host Xid on the device's pid (one
  Xid 31 on vm1's device in the same hour).
- 2026-09-24: **The Xid 31 in cuda-hostreg was the swiotlb alias, fixed**
  ([memory-model.md](memory-model.md) §2 "The alias in the device", records in
  `caraxes/runs/xid31-hostreg-2026-09-24/`).
  - **Case.** vm1's cuda gate with main 7621cfb failed cuda-hostreg (719), with one Xid 31 on
    the device pid 508533: `CE2 ... FAULT_PTE` at a GPU VA inside the pinned buffer.
  - **Cause.** The guest aliases its swiotlb pool below 2^40, rounded out to 2 MiB, so
    ordinary guest pages next to the pool get alias IOVAs. The device's table had no alias.
    The shadow filter refused those leaves: the faulting VA was the first refused leaf. The
    shadow had no PTE and the copy engine faulted on the VM's own channel.
  - **Not a race, and not B and C.** Whether a buffer gets margin pages depends on the
    guest's free lists, which is why the rerun passed.
  - **Fix.** The device reads the guest's table (`<dma-ranges>.guest`) at every reset and
    accepts only aliases of its own registered guest RAM (`DmaTable::aliases_of`). It
    translates them for its own accesses. The shadow maps an alias leaf from the registered
    descriptor of the same page. Three unit tests cover it.
  - **Reproducer.** The new probe `caraxes/tools/hostreg-margin.c` registers exactly those
    pages.
  - **Runs.**
    - Before, device pid 1131576: 719 and two Xid 31 on the device pid.
    - After, device pid 1151433: hostreg-margin 20 of 20, hostregprobe 20 of 20, full cuda
      gate 10 of 11 (only the known cuda-managed 801), 0 host Xid.
- 2026-09-24: **GSP surface, category D round 2: R2, the open MODEL rows, the ZBC table**
  ([gsp-plan/d-device-model.md](gsp-plan/d-device-model.md) §10 "Round 2",
  [gsp-surface.md](gsp-surface.md) "After round 2 of D", records
  `caraxes/runs/gsp-round2-d-2026-09-24/`). R2 in `src/rc.rs` and a per-VM `DState`:
  `FIFO_DISABLE_CHANNELS` only on the guest's own channels (mirrors by their own channel and
  TSG objects, never a runlist action), `GPU_REPORT_NON_REPLAYABLE_FAULT` and the RC watchdog
  timeout RC the guest's channel through the existing RC_TRIGGERED path, `LOG_OOB_XID` and the
  RC journal, compute mode and GPU accounting per VM, the BAR0 map policy deny by default. The
  open MODEL rows outside the display got the plan's small models (FECS setters, the two L2
  flushes on the device's own subdevice, FBSR_INIT, RATED_TDP, the MMU debug mode); the timed
  semaphore is refused until Q4; the 12 display rows stay with the display branch. The ZBC
  table is guest-local per VM, seeded from one read-only host sweep (Q5: 48 GET controls on an
  unprivileged client's own GF100_ZBC_CLEAR, all NV_OK, no host kernel line; only GSP-RM's seed
  kept, the host desktop's clear colours left out), and the getters answer from it. Rule table
  615: REFUSE 202, DENY 37, ACK 20, TABLE 5, MODEL 41, MIRROR 3; routed controls answered 734 to
  757 on 615 (720 to 743 on 610), D unanswered 48 to 28. vm3 gates before and after on both
  guests unchanged (610: nvml power N/A, cuda 10 of 12; 615: nvml pass, cuda 10 of 12, both
  failing only `cuda-managed`); the gates send no D control, a ZBC probe in the guest reads the
  seeded table and its own three entries back after (all 0x56 before). No host Xid on the
  device's pid (two Xid 31 of another VM's device during the 615 before run). What the guest
  watchdog needs before `RmRcWatchdog=0` can go: 2D-class NOTIFY in the software executor, the
  BAR1 USERD path (Q7).
- 2026-09-25: **The GSP ABI baseline is recorded (D20)** ([gsp-abi-615.md](gsp-abi-615.md),
  `caraxes/abi/`). `caraxes/tools/gsp-abi.py snapshot` writes the 615.71.09 surface as TSV
  (1397 controls, 844 routed, 734 answered by the device: host-forwarded 69, synthesized 79,
  guest-derived 40, echo 67, refused 479; 232 RPC functions, the 14 a 615 client sends all
  answered; 45 events, 4 sent by the device; 224 classes, 149 reaching GSP; 41 boot and transport
  types, 17 of them `NV_ABI_STABLE`), and the same for 610.57.04. `gsp-abi.py diff` of the two
  reproduces every 610-to-615 change of the plans (TSG params 20 to 28, `rpc_rc_triggered` 48 to
  64, the `INTERNAL_CONTROL_GSP_TRACE` field insert at equal size, `PERMISSIONS_INIT` 7 to 6, the
  `FB_FLUSH_GPU_CACHE` flag words, `GR_GET_GLOBAL_SM_ORDER` 10264, `GPU_GET_PES_INFO` 76, the msgq
  v2 element and boot arguments, the reused id 0x20800A36) and finds seven equal-size layout changes
  the plans had not recorded, among them `DP_GET_CAPS` (synthesized by the device) gaining
  `bIsInternalDpTunnelingSupported`; not visible to it: the GSS legacy sizes, the new meaning of
  the channel and TSG `internalFlags`, the register-held queue pointers. `gsp-surface.py` now
  reads the RPC arms of `gsprpc.rs`, `gspmem.rs` and `events.rs` (19 functions and 4 events
  handled, not 11 and 2) and takes the 615 client's sends for a 615 tree. The tables are
  [gsp-abi-615-tables.md](gsp-abi-615-tables.md); `tools/check.sh` step `gsp-abi` fails when the
  snapshot drifts from the 615 tree or the tables from the snapshot (passes here: 0 changes).
- 2026-09-25: **The 615 guest's display path** ([display-615.md](display-615.md),
  `caraxes/runs/display-615-2026-09-24/`). vm0-615 is an overlay on the 615 base image with vm0's
  seed. Its GL, EGL and Vulkan userspace at 615.71.09 is copied from the host's nvidia-utils,
  because the image has no libnvidia-gl-615. Its nvidia-drm.ko is rebuilt from the image's DKMS
  source without the new HDCP property, because the DKMS module needs drm_display_helper.ko from
  linux-modules-extra, which the image lacks. With the display page environment of series C (slot
  0, 2 GiB), the display gate passes 11/11 twice and three GNOME starts come up on the NVIDIA EGL.
  The per-frame traps match series C row for row: Mutter 138.92 (610: 139.86) and Weston 29.25 at
  60 Hz (610: 31.08 at 57.4 Hz). Q1's 615 half: the unplug sends DP_SET_WAR_FLAGS, then HDCP_CTRL
  DISABLE_AUTHENTICATION, and the plug sends DP_SET_WAR_FLAGS; they are the only D controls, and
  none is unanswered. The device needed no fix. `ver/v615/display.rs` now makes the 615 layouts
  explicit (GET_ACTIVE 20 bytes, staleSorIndex) and records the WAR per connector, with tests.
  Found: the 615 GL driver asks GET_ZBC_CLEAR_TABLE_ENTRY 45 times per context (unanswered, A).
  0 host Xid on the device's pid.
- 2026-09-25: **GSP plan round 2, category C** ([gsp-plan/c-guest-state.md](gsp-plan/c-guest-state.md)
  §8 "Round 2", [gsp-plan/b-host-forwardable.md](gsp-plan/b-host-forwardable.md) §8,
  [gsp-surface.md](gsp-surface.md) "After the GSP plan round 2"; runs
  `caraxes/runs/gsp-round2-c-2026-09-24/`). Routed controls answered: 720 to 746 on the 610
  ABI, 734 to 760 on 615.
  - **The 17 C rows of the D plan** are rows of `PUBLIC_C`: 9 done, 8 partial (the three
    scheduling switches act on the own mirrors, but the software executor does not consult the
    flags; `PREEMPTION_BIND` recorded, not forwarded until the D17 VA check; the two DP
    readbacks wait for display-engine state; watchdog and non-replayable fault go to the
    device's RC journal only). Nothing reaches a host object other than the guest channel's own
    mirror.
  - **Nine of the ten open B rows** answer from channel and context state; the debugger
    getters without a session model are refused (MMU debug, per-SM status), ERRBAR is disabled,
    GCC is absent on TU106. The ZBC entry stays with the ZBC model.
  - **TSG id.** The kernel's grpID keys the host TSG map on 615 (the guest handle on 610); a
    bare channel's internal TSG id keys its own host TSG.
  - **IDLE_CHANNELS** idles mirrored channels through the host RM's `FIFO_IDLE_CHANNELS` on
    the device's own mirror objects (timeout capped at 50 ms), unknown channels 0x33.
  - **Operational events.** Every RC the device raises becomes a GOE record in the 615
    import's registered buffer plus `POST_EVENT`; the kernel drains and answers `UPDATE_GET`
    (71 of 71 self-test records in the probe run), and a guest tool bound to the kernel's log
    (`caraxes/tools/goe-probe.sh`) receives them. `MMU_FAULT_QUEUED` stays unsent, per source
    (no executor faults, mirror faults are RCs of non-UVM channels, the host fault buffer is
    host-global).
  - **Gates on vm5 (1 GiB, slot 2), before and after identical:** 615 guest nvml pass, cuda 10
    pass with cuda-managed failing (801); 610 guest nvml power N/A as before, cuda: torch,
    rlprobe and convburn pass, the nvprobe-based tests print nothing on vm5's 610 image both
    times. No Xid on the device's pid (two Xid 31 in one window belonged to another run's
    device).
- 2026-09-25: **Trap census and optimisation backlog** ([optimization-backlog.md](optimization-backlog.md),
  records `caraxes/runs/trap-census-2026-09-25/`). Every CUDA operation alone on vm3 (main
  1ea2529, 610 guest, 1 GiB slot 5) with the device trace and marker writes, in four
  configurations plus native. With the interrupt page, BAR1 pool (32 MiB) and token pool, a
  launch, a copy, an event and a synchronise trap 0 times net, and PPO submits 28000 times per second
  without a trap. What traps is the context lifetime (75k and 21.5k BAR2-dominated traps for
  cuCtxCreate and cuCtxDestroy), the host registrations (807-1049 per 1 MiB), and one MSI re-arm per
  interrupt (140-160/s). `CARAXES_BAR2_MEMFD=1` together with the BAR1 pool hangs the first
  cuCtxCreate, also with `_SYNC_DOORBELL=1`: the shadow's first walk finds 15617 of 16128 leaves.
  Without the pool the memfd takes cuInit 177 to 20 ms, cuCtxCreate 4.1 s to 0.87 s and cuCtxDestroy
  1.24 s to 0.18 s, and 1 MiB host registration from 39 to 1.7 ms. What is left of the context
  creation is about 6000 host RM map calls in the shadow worker. Stalls without traps: D2D 16 MiB
  3.85 ms against 0.25, H2D 16 MiB 1.9-5.2 ms against 1.3, GR operations 10-30 ms while another VM
  holds the engine. Backlog top five: memfd with the pool, the shadow's map work at context
  creation, the invalidate and flush page as memory with a cheaper sync, the MSI re-arm as an
  ioeventfd, and progress-gated interrupt forwarding. 0 host Xid.
- 2026-09-25: **Backlog items 1 and 4: the BAR2 memfd works beside the BAR1 pool, and the MSI
  re-arm no longer traps** ([bar2-memfd.md](bar2-memfd.md) §8, [intr-page.md](intr-page.md) §4;
  records `caraxes/runs/opt-memfd-pool-2026-09-25/`, `caraxes/runs/opt-msi-ioeventfd-2026-09-25/`).
  - **Why memfd plus pool hung.** The pool follows the guest's BAR1 tables through trapped writes.
    With the memfd the guest RM writes them through BAR2, and the sync's report
    (`note_pages_changed`) did not reach the pool. The pool never moved a page, and the shadow
    kept a 2 MiB leaf whole: the 511 missing leaves.
  - **The fix.** The report now reaches the pool. The two translations also have one order: the
    memfd by guest address first, the store through the pool's remap, and the same for the
    write-back and the loads. Two unit tests fail without it.
  - **Numbers on vm3** (interrupt page, pool 32 MiB, token pool, hybrid, memfd):
    - cuCtxCreate 0.61 s with 1228 traps (pool alone 4.09 s, memfd alone 0.87 s);
    - cuCtxDestroy 0.20 s, cuInit 28 ms, host alloc and register 1.7 and 2.3 ms;
    - rlprobe process 15.5 s against 21.3 s;
    - the device's CPU per copy process 3.5 s against 11 s.
  - **Copy rows.** They do not regress. The pinned rows' 6.5 and 12.9 GB/s modes follow the
    card's PCIe link state (P8/P5 against P0) in both configurations.
  - **The MSI re-arm.** `CARAXES_MSI_REARM_IOEVENTFD=1` makes the write to `NV_XVE_CYA_2` (0x88704,
    value ignored) an ioeventfd without datamatch, read by the token pool's reactor, which runs
    what the trapped write ran. Re-arm traps go from 141 to 0 per second in rlprobe and from 153
    to 0 in PPO. All traps per second go from 508 to 456 and from 423 to 314.
  - **Gates and Xid.** The cuda gate is 10 of 12 in both (cuda-managed), and there was 0 host
    Xid.
  - **Recommended configuration.** Memfd, pool, token pool, interrupt page and the re-arm switch
    (bench-run.sh header).
- 2026-09-25: **The shadow's host RM calls at context creation, backlog item 2**
  ([memory-model.md](memory-model.md) §7, records `caraxes/runs/opt-shadow-maps-2026-09-25/`).
  - **Setup.** vm0, 1 GiB at slot 0, 610 guest, BAR2 memfd without the BAR1 pool, census probe.
  - **Cause.** Five context creations in a row showed it: the guest's sysmem pages come back
    scattered. That cost about 15000 one-page map calls per context (29 us each) and as many
    unmaps at the next destroy (81 us each), and a queued destroy delayed the next creation by
    up to 1.4 s.
  - **Built:**
    - gather descriptors (`hostmem.rs` `gather`): one OS descriptor over a scattered stretch's
      guest pages in VA order, placed with `mremap` of the device's shared guest-RAM mapping, so
      one call maps up to 512 pages;
    - whole-region frees of emptied regions;
    - a per-VAS region free list;
    - host RM call counters by kind on every sync line (`rpc-cost.py` prints the histogram).
  - **Measured without gain, left off:** deferred TLB invalidation (`CARAXES_SHADOW_DEFER_TLB=1`).
  - **Results:**
    - host RM calls per cuCtxCreate 13171 to 228, worker apply 770 to 67 ms;
    - cuCtxCreate 815 to 309 ms (native 146), cuCtxDestroy 170 to 107 ms;
    - `bench-cu init` cuCtxCreate 1719 to 353 ms, churn unchanged;
    - the whole census's shadow host time 4.8 s to 0.72 s.
  - **Isolation.** Every gathered page is a page of a registered guest-RAM descriptor that the
    filter resolved; nothing is pinned beyond D13. The switches are `CARAXES_SHADOW_GATHER`,
    `_BULK_FREE` and `_REGION_POOL` (0 turns each off).
  - **Gates.** cuda 11 of 12 (cuda-managed) in every part, 0 host Xid.
- 2026-09-25: **The C table and the D models reconciled** ([gsp-plan/c-guest-state.md](gsp-plan/c-guest-state.md)
  §8 "Reconciliation", [gsp-plan/d-device-model.md](gsp-plan/d-device-model.md) §10,
  [gsp-surface.md](gsp-surface.md) "After the reconciliation"; runs
  `caraxes/runs/gsp-round2-reconcile-2026-09-25/`). The two round-2 merges answered five controls
  twice, the C table's partial rows first: `FIFO_DISABLE_CHANNELS`,
  `GPU_REPORT_NON_REPLAYABLE_FAULT`, `INTERNAL_RC_WATCHDOG_TIMEOUT`,
  `RC_SET_CLEAN_ERROR_HISTORY` and `GPU_VALIDATE_MEM_MAP_REQUEST` (and the B getter
  `DEBUG_GET_MODE_MMU_DEBUG`) are now the D models' only. One RC journal (rc.rs), read by the C
  rows `RC_GET_ERROR_COUNT` and `RC_GET_ERROR_V2`; every RC the device raises lands in it once
  and, on 615, in one operational-event record. One set of scheduling flags, which the software
  executor now consults (a disabled channel's doorbell is held and replayed). A guard test keeps
  public.rs, chipctl.rs and dmodel.rs/rc.rs disjoint per ABI. Routed controls answered
  unchanged (763 on 610, 777 on 615). Gates: 615 on vm5-615 nvml pass, cuda all scored stages
  but cuda-managed; 610 on vm3 nvml power N/A as always, cuda the same as 615; the journal
  round trip (`CARAXES_RC_SELFTEST`, caraxes/tools/rc-journal-probe.sh) and the goe-probe (11
  records) pass. No host Xid.
- 2026-09-25: **Night series on vm2** ([benchmark.md](benchmark.md) "Night series",
  [bar2-memfd.md](bar2-memfd.md) §8, [alloc-path.md](alloc-path.md) §7,
  [workload-gate.md](workload-gate.md) §7; records in `caraxes/runs/{bar2-memfd,alloc-path,bench-615,workload}-2026-09-24/night/`).
  vm2 at 1 GiB slot 3, main 57693d8 frozen, host 615.71.09.
  - **BAR2 memfd measured.** Trapped BAR2 writes per context creation 96269 to 0; cuCtxCreate
    3.5-3.9 s to 0.53 s (2.7x native), cuCtxDestroy to 0.15 s. With the BAR1 pool every context
    creation hangs at the first GR doorbell (also with the doorbell sync and without the
    interrupt page); without the pool it runs, cuda gate 10 of 11 (known cuda-managed 801).
  - **Allocation path measured.** 45.5 traps per 1 MiB cuMemAlloc at the baseline, 4.3 with
    interrupt page, memfd and `CARAXES_ALLOC_LAZY=1`; cuMemAlloc 1 MiB 972 to 175 us (4.9x
    native, 1.9x v1), 64 MiB 7.3 to 0.54 ms; cuMemFree 1 MiB 401 us (15x) is the next target.
  - **Like-for-like 615 series.** The binary refuses the fixed P21 area, so the pool stood in;
    615 and 610 guest match each other and the closing series (short kernel 0.016 ms, rlprobe
    stage 4 2.0 s on the quiet 610 part); cuCtxCreate 4.0-4.4 s without the memfd.
  - **Workload gate, first run** (615 guest, quiet host): PPO 420 s solves on all reps at 0.64
    of native; llama does not load in 1 GiB, transformer and ResNet-18 run at halved batches
    (0.85 and 0.92 per sample); one Xid 31 (GPC FAULT_PDE) on vm2's device in transformer rep 2.
  - Harness: `bench-run.sh` retries a restart the D11 headroom rule refused; the workload runner
    takes `WORKLOAD_ENV`. Other host Xids of the night (two Xid 31 at 23:32) were vm1's device.
- 2026-09-25: **Morning matrix on main fbe4dfb** ([benchmark.md](benchmark.md) "Morning matrix
  (2026-09-25, main fbe4dfb)", records `caraxes/runs/matrix-2026-09-25/`). The frozen fbe4dfb
  builds with all three optimisations together (BAR2 memfd beside the BAR1 pool, batched shadow
  maps, MSI re-arm ioeventfd) plus the lazy plans, host 615.71.09.
  - **Gates.** nvml: 615 guest pass, 610 guest and costume pass but the known power-number;
    cuda: 10 of 11 on the 610 guest, the costume and the 615 rerun (the known cuda-managed 801);
    display 11 of 11 on the 610 guest (vm0, alone) and the 615 guest (vm0-615); v1 on the 615
    image: smi pass, cuda 14 of 14.
  - **Benchmarks.** cuCtxCreate 319/323 ms (1.6x native; closing series 19x), cuCtxDestroy 1.3x,
    cuInit 14 ms, cuMemAlloc 1 MiB 198/201 us (5.5x), cuMemFree 1 MiB 12x, short kernel 2.5x,
    rlprobe stage 4 1.7x, PPO 0.67 of native on both guests; copies at native speed on two of
    the 610 guest's three device instances, at the old half on the 615 guest's. Census: cuCtxCreate
    437 ms, cuMemAllocHost 1.9 ms, cuMemHostRegister 2.1 ms.
  - **Workload gate at 2 GiB** (615 guest): like for like with native now; llama pp512/tg128 0.96
    and 0.92, transformer 1.01, ResNet-18 1.04, PPO 420 s 0.64.
  - **The Xid 31 recurs on fbe4dfb** (with the swiotlb-alias fix): three host Xid 31, GPC
    FAULT_PDE on a write, all on a 615 guest's device in a cuDNN training step (cuda gate
    convburn on vm5-615, pid 2362730; workload convnet rep 1 on vm2-615, pid 2545558);
    intermittent (the reruns with and without the pool passed); the faulting gate run's pool
    summary is the only one with refused barriers. No Xid on the 610 guest.
  - Harness: `bench-run.sh` keeps each rep's device log before a restart (`V2_LOG_KEEP`).
- 2026-09-25: **The morning matrix's Xid 31: cause found and fixed** ([bar1-pool.md](bar1-pool.md)
  §9, records `caraxes/runs/xid31-pool-2026-09-25/`). A mirrored channel's USERD lives in the
  BAR1 pool (libcuda maps it into BAR1 before it creates the channel), so the guest's GPPut store
  reaches the host channel without a trap; a loaded host channel runs the new work before the
  shadow has the invalidate's mappings (with `CARAXES_ALLOC_LAZY=1` before the plan is made):
  FAULT_PDE on a GR write into a fresh 2 MiB region, on the device's pid. The refused barriers
  were a consequence (a faulted context's teardown), not the cause; the pool never moves a page
  whose barrier failed. Fix, fail closed (`untrapped.rs`): an invalidate of a root whose channel
  has an untrapped USERD is planned and applied before the guest continues; the trapped GPPut
  store plans lazily deferred invalidates too; counters for ran-ahead doorbells and held
  invalidates. Reproduced with `allocburn.py` (fresh allocations written while the GR channel is
  busy): Xid 31 at the first iteration on both guests without the rule (615 pid 2861668, 610 pid
  2951267); with it 20 of 20 on each guest, the cuda gate on both guests 10 of 11 (cuda-managed
  801), 0 Xid. Cost (the rule off then on, vm5-615): cuMemAlloc 1 MiB 175 to 200 us, 64 MiB 511 to
  2277 us, cuMemFree 64 MiB 791 to 2402 us, cuCtxCreate 395 to 486 ms, launches unchanged; holding
  only plans that map something new would recover the frees (not built).
- 2026-09-25: **Coverage map and the profile** ([coverage-map.md](coverage-map.md), records
  `caraxes/runs/coverage-2026-09-25/`).
  - **The profile.** `CARAXES_PROFILE=1` aggregates, per device and per guest client:
    - trapped accesses, with device and guest-seen time;
    - controls with a status histogram and label, allocs, RPCs and events;
    - every host RM ioctl, through a new nvrm-abi observer, and the shadow's call kinds;
    - window hits.

    It dumps TSV on SIGUSR2 and at exit and costs 84 ns per trapped access; short-auto with it
    on is within noise. `caraxes/tools/profile-rank.py` ranks the rows and joins them with
    controls.tsv and the handler module.
  - **The map.** 29 workload windows on vm2 with the recommended configuration and an
    instrumented build:
    - 208 of 829 GSP-routed controls reached, all 70 host-forwarded ones;
    - untouched and answered by the device: 42 synthesized, 59 guest-derived, 48 echo, 427
      refused;
    - 931 of 1464 device functions ran under a workload, 392 only in unit tests, 141 nowhere.
  - **The sweep.** The guest control sweep (`ctlsweep.c`, generated from controls.tsv)
    reached 111 untouched getters and found 21 unanswered user-callable ones.
  - **Top candidates outside the benchmarks.**
    - BAR1 pool moves: 1024 pages per CUDA process, 363 ms of device time.
    - BAR2 memfd syncs.
    - Host TSG preempts, mostly the pool barrier's: 3.9 s; one took 3.2 s at a kill -9, with
      host Xid 109.
    - Shadow gathers.
    - The device's BAR1 window remaps.
    - GSS-legacy PERF 0x79: 21-678 ms per nvidia-smi call.
    - 331 idle MSIs per second from other VMs' work.
  - **Findings.** `cuMemAllocAsync` is refused (801). An illegal address reaches the guest as
    LAUNCH_FAILED (719), with no RC event and host Xid 31 on the device's pid.
- 2026-09-25: **Coverage follow-up: the sweep getters and the forward cache**
  ([coverage-map.md](coverage-map.md) "Status", [gsp-surface.md](gsp-surface.md) "After the
  sweep getters of the coverage map", records `caraxes/runs/coverage-followup-2026-09-25/`).
  - **The 21 unanswered sweep getters are answered.**
    - 7 are plain host forwards (`FORWARD`): the latency buffer, IP version, scheduler caps,
      GPC tile map, FB floorsweeping, PMGR module and falcon architecture.
    - 5 are chip-table rows: the TPC partition mode NONE, the RC policy, the FBHUB PCE mask
      0xF from the card's traces, an empty OEM object (D6) and an empty I2C device table.
    - 9 are refused as absent features: compression (2), NVJPG, VPR (2), P2P caps (vGPU only),
      FB carveouts, WPR offsets and dielets.

    Routed controls unhandled: 45 on 610, 46 on 615. The sweep on vm2 finds no `unanswered`
    control.
  - **The forward cache** (`src/fwdcache.rs`, `CARAXES_FORWARD_CACHE=0` turns it off).
    - It keeps the first host answer of 22 static per-card getters for the device's lifetime.
      Live telemetry, host-settable state and the GSS-legacy commands stay live.
    - Their repeat host calls go away: 311 hits and 21 fills in one session. That saves 2.8 ms
      of about 83 ms of device time per `nvidia-smi -q` on a quiet host.
    - nvidia-smi's wall time follows the host's load and the uncached GSS-legacy perf and clock
      commands.
  - **Gates.** cuda 10 of 11 (cuda-managed 801); nvml fails on power N/A only (610 guest).
    0 host Xid.
  - The first start was refused at the BAR1 pool mmap: the host BAR1 was full with the other
    VMs. `run.sh` retries it.
- 2026-09-25: **SotTR on the 615 guest with the distribution's driver** ([sottr-stage.md](sottr-stage.md)
  section 8, records `caraxes/runs/sottr-2026-09-24/session-615-20260925-112443/`).
  - vm4-615: an overlay on the 615 image, `apt` installs `nvidia-driver-open` 615.71.09 with
    GL, EGL, Vulkan, encode and decode from NVIDIA's CUDA repository, and
    `linux-modules-extra` (the packaged nvidia-drm then loads). No `/opt/nvrm-gl`, no ld.so or
    ICD edits. `up-615.sh`, `stream-615.sh`, `down-615.sh`.
  - Driver up, `nvidia-smi` and `vulkaninfo` on the NVIDIA ICD, GNOME on Wayland with the gbm
    renderer, Steam's sniper container imports the NVIDIA Vulkan ICD without issues; the
    operator's Moonlight stream ran on the first start.
  - Sunshine prefers NVENC: `OpenEncodeSessionEx` answers "unsupported device" (no mirror),
    Vulkan encode fails, libx264 is chosen.
  - Host Xid 31 (CE0 `FAULT_PDE`) once on this session's device during the stream; one on
    vm2's device (NVENC0).
  - Weekend goal: the guest driver must not matter ([roadmap.md](roadmap.md) item 2); what the
    device still needs from the guest is sottr-stage.md section 8.1 (the three registry keys).
- 2026-09-25: **NVENC and NVDEC through the mirror, in progress** ([video-engines.md](video-engines.md),
  records `caraxes/runs/nvenc-2026-09-25/`, probe `caraxes/tools/nvenc-probe.c`).
  - Native probe: 300 frames of 1080p H.264 at 160.7 fps, 2.9 ms median latency, decode
    PSNR at least 63.4 dB.
  - The session opens in the guest: 4 GSS legacy controls forwarded.
  - The NVENC channel is mirrored on the shadow VAS and bound to NVENC0 at its first
    doorbell. The host object is allocated once the guest's context page has left the
    shadow.
  - Two failure modes found and removed:
    - VA reservation storm: host RM NO_MEMORY, the desktop froze, host reboot at 11:44.
    - Doorbell-hold deadlock: the guest waits for its first push.
  - No guest frame has been encoded yet; the next probe waits for a host BAR1 budget.
- 2026-09-25: **Limits runs: ECC, MIG and vGPU** ([limits-mig-ecc-vgpu.md](limits-mig-ecc-vgpu.md),
  evidence `caraxes/runs/limits-2026-09-25/`). All runs are pure emulation on vm8 (`CARAXES_TAP=none`).
  - **ECC** (`src/families/ecc.rs`, GH100 rung). Everything works:
    - `nvidia-smi -q -d ECC`, the ECC query fields, `-q -d ROW_REMAPPER`,
      `--query-remapped-rows` and `-q -d PAGE_RETIREMENT`.
    - `-e 0/1` stays pending until the next boot or `-r`, and `-p 0/1` works.
    - `-r` does not fail closed. The PCI FLR re-boots the modelled GSP, the second
      RmInitAdapter succeeds and the pending mode is in force.
  - **MIG** (`src/families/mig.rs`).
    - Works: `-mig 1` (resetless on GH100), `-lgip`, `-lgipp`, `-cgi` for every profile,
      `-lgi`, `-lcip` and `-mig 0`.
    - Breaks: the compute instance, and the destroy of the last GPU instance. Both stop at the
      kernel's golden-image channel (`numGpcs > 0`, kernel_graphics_object.c:192), because pure
      emulation has no GR. So there is no MIG device and no CUDA on one.
  - **vGPU.** The open driver builds neither a vGPU host nor a vGPU guest
    (os-interface.c:1517-1552 of 615). 0 vGPU controls reached the device. The refusals now
    name that reason.
  - **Pending.** The GA100 rung (A100 identity over tu106, MIG pending until a reset through
    the devinit scratch bit), the TU106-as-VF run and the TU106 emu-boot check. The operator
    stopped VM boots for host memory.
