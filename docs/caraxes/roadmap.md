<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Caraxes roadmap: done, in progress, next, later

Snapshot of 2026-09-24, 10:30. The decisions and ideas it refers to are in [README.md](README.md).

## Done

- Synthetic vfio-user GPU on stock Cloud Hypervisor v53: PCI identity, BARs, VBIOS, GSP boot
  emulation, the RPC dispatcher (about 150 controls, a dozen RPC types), the naming model with the
  transport word as default (`Leandro VFIO RTX 2070-4G`, P17).
- nvidia-smi and NVML: gate 11/11 with real telemetry from the host card.
- Memory model: fixed VRAM reservation with slots (D11, D18 target), guest RAM registered with the
  host RM, IOVA table, guest DMA module as the alpha bootstrap (D13).
- Execution: mirror channels, copy engines on the guest's tables, GR on a host-managed shadow VA
  space (D17 stage 1), privileged MEM_OP TLB invalidate, USERD and work-submit token handling,
  L2 cache-op registers forwarded, one shadow per guest root, one host TSG per guest TSG.
- CUDA: nvprobe 0-4 green, the cuda gate 4/4 (jit, launch, torch with cuDNN, rlprobe),
  robustness, two VMs concurrently (P20).
- Isolation: no comptag reaches the host (D16), the shadow filter (D17), the parallelism proof
  (slots, checker, pattern test, host scrub on free, fail-closed teardown, refusal case), the BAR1
  window budget and leak fix.
- Display: DisplayPort SST connector with EDID and link training, modeset, flips paced at the
  mode's rate, vblank, Weston and GNOME on Wayland, hotplug, hardware cursor; the vdisplay and
  display gates pass on Wayland; the first frame rendered on the physical GPU (display-e).
- Benchmark: harness and the v1 / v2 / native table (P13, [benchmark.md](benchmark.md)).
- Hardening: GSP message queue (ordering, wedges, checksum), flush registers, fail-closed
  teardown.
- The GSP surface inventory (P23, [gsp-surface.md](gsp-surface.md)): 829 controls reach GSP, 171
  answered today, the rest sorted into five categories with a recipe each.
- Research: display and DisplayPort studies, nova, execution formats, DMA routes, GR completion
  and diagnosis, the upstream vGPU manager comparison, the Blackwell lab capture kit.

## In progress (today)

- Mapping package on vm2 ([mapping.md](mapping.md) when it lands): write-through shadow,
  coalesced maps, mapping budget and batched teardown are in (allocations 20 ms to 0.9 ms,
  context create 5.4 s to 2.7 s); next the doorbell passthrough (target 30 us per submission),
  then context create and destroy, resync off the server thread, the two P20 isolation fixes
  (all channels on the filtered shadow, IOVA = GPA + offset), and the benchmark rerun.
- GPU-rendered desktop on vm0 ([display-e.md](display-e.md) when it lands): Weston on the NVIDIA
  EGL stack at 60 Hz, next Mutter with a measured frame rate, the v1 stages swapchain, present,
  rt and events in the display gate (9 of 11 on the first run, present held at 48 FPS by the
  synchronous resync), the classic demos (vkcube, glxgears, eglgears, glmark2), Sunshine with
  software x264 and Moonlight on the host, a pointer device.
- Cloud Hypervisor patch 0005 ([patches/](../../patches/)): VFIO_USER_DEVICE_GET_REGION_IO_FDS
  in the vfio-user client with KVM ioeventfds, written against the published specification
  (the libvfio-user layout deviation marked and refused), a vendored server-side crate change,
  the doorbell page declared by the device behind a switch; due 2026-09-25 09:00.

## Next (this week)

1. P21: BAR1 window passthrough (operator's order). Prototype G measured 2026-09-24 (three of
   four submission traps gone, rlprobe stage 4 halved); next the datamatch token pool (design
   agent running: device-chosen work-submit tokens through the notifier page, one eventfd per
   token declared at start, the vendored client accepting several descriptors per reply, a CH
   patch on top of 0005), which makes a submission exit-free, then target design H (a private
   pool in the reservation with guest BAR1 pages remapped in every translation, so the area
   holds for any guest layout and can grow for asset streaming in a game).
2. The game benchmark: Shadow of the Tomb Raider in the Caraxes guest at 720p and 1080p,
   streamed through Sunshine to Moonlight on the host with the v1 stream measurement, one guest
   and then two concurrently against [sottr-4q](../measurements/sottr-4q/README.md); the guest
   needs the game installed (v1 image or a Steam login by the operator).
3. NVENC through a mirror for Sunshine hardware encoding (pulled forward from later: the game
   comparison with v1 needs it); the Moonlight stream stage.
4. P23, the GSP surface week: E (refusal per family, 1 day), A (table handler plus one sweep
   of the real card, operator's go-ahead), B (forward rule with the D9 deny list), C (channel
   registry and mirror), D rules 1 and 4 (acknowledge setters, refuse profiling), the nine RPC
   functions and the RC and fault events; display and mirror parts of D with their packages.
5. BAR2 selective passthrough: the 96k trapped 8-byte writes of a context create.
6. gpu gate leftovers: the own holder, ffmpeg in the guest image, the native torch reference,
   robustness inside the gpu gate, the smi name masking.
7. UVM managed memory: fault forwarding to the guest, physical UVM channels, copy-engine written
   page tables (the pushbuffer scanner), cuda-managed and the gpu gate's kernel stage.
8. D18: sparse demand-driven registration with the cached mapping object, then drop the guest
   DMA module and the `_DMA` patches: the vanilla gate.
9. v1 upkeep if v1 stays the reference: the pinned-page leak and the 100 ms blocking sync.
10. Multi-VM beyond the proof: timeslices and QoS, reservation policy as configuration, the host
    journal flood (gone with the next host reboot).
11. The P13 decision after the benchmark rerun.

## Later

- D14 versatility: a second driver version's ABI module, host and guest versions split, the
  nova-core guest (r000 ABI recheck, N gates).
- A Blackwell identity from the lab capture (new boot model, GMMU, classes, display IP v05).
- Cleanup and user documentation (quickstart, architecture, testing), `up` runs sync-tests,
  `sync` before `down`.
- Meisterstack integration through the VirtualGpuSpec; profiles (P22 parked).
- Phase E: a host-side sink and viewer, timed flips, monitor power.
- P24 poll-mode GSP: one core serving several devices, the submission path without a trap.
- P18: the SR-IOV middleman on Ada and later; a Windows guest after the nova tests (first the
  open question whether the Windows 610 driver boots a TU106 with GSP-RM); nested
  virtualisation; NVLink as a peer model resolved by the shadow, inside one VM only.
- Two-path maintenance and the Cloud Hypervisor benchmark; the ACPI `_DMA` patches kept posted.
