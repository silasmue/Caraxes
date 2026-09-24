<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Caraxes roadmap: done, in progress, next, later

Snapshot of 2026-09-24, 10:45. The decisions and ideas it refers to are in [README.md](README.md).

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
  rt and events in the display gate, Sunshine with software x264 and Moonlight on the host, a
  pointer device.

## Next (this week)

1. P21: BAR1 window passthrough (operator's order).
2. BAR2 selective passthrough: the 96k trapped 8-byte writes of a context create.
3. gpu gate leftovers: the own holder, ffmpeg in the guest image, the native torch reference,
   robustness inside the gpu gate, the smi name masking.
4. UVM managed memory: fault forwarding to the guest, physical UVM channels, copy-engine written
   page tables (the pushbuffer scanner), cuda-managed and the gpu gate's kernel stage.
5. D18: sparse demand-driven registration with the cached mapping object, then drop the guest
   DMA module and the `_DMA` patches: the vanilla gate.
6. v1 upkeep if v1 stays the reference: the pinned-page leak and the 100 ms blocking sync.
7. NVENC through a mirror for Sunshine hardware encoding; the Moonlight stream stage.
8. Multi-VM beyond the proof: timeslices and QoS, reservation policy as configuration, the host
   journal flood.
9. The P13 decision after the benchmark rerun.

## Later

- D14 versatility: a second driver version's ABI module, host and guest versions split, the
  nova-core guest (r000 ABI recheck, N gates).
- A Blackwell identity from the lab capture (new boot model, GMMU, classes, display IP v05).
- Cleanup and user documentation (quickstart, architecture, testing), `up` runs sync-tests,
  `sync` before `down`.
- Meisterstack integration through the VirtualGpuSpec; profiles (P22 parked).
- Phase E: a host-side sink and viewer, timed flips, monitor power.
- P18: the SR-IOV middleman on Ada and later; a Windows guest after the nova tests; nested
  virtualisation.
- Two-path maintenance and the Cloud Hypervisor benchmark; the ACPI `_DMA` patches kept posted.
