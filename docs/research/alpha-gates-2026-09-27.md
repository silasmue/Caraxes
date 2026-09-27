<!-- SPDX-License-Identifier: MIT -->
# Alpha acceptance work, 2026-09-27

Order agreed with the user: fix the V1/V2 findings, Windows, Linux desktop,
then latency optimization. All Caraxes VM trials execute against the host GPU.
Do not copy source code or benchmarks from nvkvm/Kayfabe into Leandro.
External projects may inform comparisons and ideas; implement and validate locally.

## Fixes and prerequisite checks

- V1: bounded external host-memory import, including pin-budget and lifetime checks.
- V2: remove repeated engine-context property queries from steady doorbells while
  preserving live exclusion refresh and shadow ordering.
- V2: fix latency histogram quantiles for durations above 200 us.
- Windows: execute internal physical CE operations within VM-owned memory without
  granting guest requests physical host-channel privilege. Check completion ordering.
- Linux: test `CARAXES_GUEST_DMA_GPA=1` without `caraxes_dma_range`, using registered
  guest RAM and host-managed GR/CE shadows. Keep the working helper profile until
  CUDA, graphics, driver reload and memory ownership tests pass without it.

## Windows gates

1. Driver initialization with host execution and no new host Xids.
2. NVIDIA D3D device creation and a rendered image checked by readback; exclude
   Microsoft software adapters from success.
3. Display presentation, resize and repeated application start/stop; desktop input.
4. Available Windows graphics benchmarks and games, including 3DMark when installed
   and usable. Record frame times and result correctness, not only successful launch.
5. Personality matrix: native TU106 baseline; `Leandro VFIO RTX 2070` naming with
   assigned VRAM; then selected alternate device identities. Record PnP, driver name,
   DXGI name, actual capabilities, rendering and workloads separately for each.
   A newer name does not imply newer executable instructions or graphics features.

## Linux desktop gates

- Wayland compositor, verified Vulkan/GL frames, cursor and frame pacing.
- Repeat with capture/encoding and new graphics work while streaming; delayed
  BAR2 flush must not trade correct ordering for a higher display kick rate.
- SotTR benchmark once graphics and capture coexist reliably; record render,
  capture/encode and client latency separately.

## Measurement discipline

- One GPU test VM at a time; host NVIDIA driver remains bound.
- Record source revision/diff, binary hash, flags, driver, memory budgets and workload.
- Reproduce the project's existing short test exactly before comparing old 14–16 us
  numbers: `NVPROBE_ITERS=100 NVPROBE_CYCLES=1000 NVPROBE_SCHED=auto ./nvprobe 4`.
- Observe host VRAM/BAR1, process RSS and Xids before/after; keep the existing floors.
- A gate is passed only by captured evidence. Boot, rendering, performance and soak
  stability are separate outcomes; this checklist is not an alpha support claim.

## Windows naming evidence

The RM identity already supports `Leandro VFIO RTX 2070-<profile>`. Existing
Windows captures show this in `Win32_VideoController.VideoProcessor`, while PnP
`Name` and DXGI description retain `NVIDIA GeForce RTX 2070` from the installed
driver description for PCI 10de:1f02. Capture these separately. Changing RM
`gpuNameString` is not evidence that the Windows INF description changed, nor
that another GPU's instructions are executable. Use transport/legacy name-format
A/B with identical board data, memory budget and execution settings first.
