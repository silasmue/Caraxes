<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Research notes

The evidence layer of VFIO Leandro: one short note per topic, bullets, every fact with its
evidence. Concepts are explained in the [wiki](../wiki/README.md); designs and studies live
in [docs/vfio](../vfio/README.md); records under `caraxes/runs`. The target driver is
615.71.09; 610.57.04 appears as one "610 delta" bullet at most.

## Concept notes

| Note | Scope |
|---|---|
| [driver-structure.md](driver-structure.md) | How the 615 kernel RM picks its HAL, the module options and registry keys the device needs, what the driver reads from where, the unstable-interface opt-in |
| [gsp-surface.md](gsp-surface.md) | The GSP surface in numbers: controls, RPCs, events, classes, boot types; categories A to E; families; what "refused" means |
| [gsp-transport.md](gsp-transport.md) | Message queue v2, GMCAPI, element framing, the doorbell trap, the boot exchange in order, teardown, suspend, `NV_ABI_STABLE` against unstable |
| [gsp-rpc-events-classes.md](gsp-rpc-events-classes.md) | The 14 RPCs a 615 client uses and their answers, the events, the classes on TU106 and the refusal rule, `ALLOC_MEMORY` |
| [abi-baseline.md](abi-baseline.md) | The 615.71.09 snapshot under `caraxes/abi/`, the diff tool, the 610-to-615 delta, the workflow for the next driver |
| [one-abi-binary-2026-09-28.md](one-abi-binary-2026-09-28.md) | One device binary for every guest ABI: host-side against guest-side layouts, the detection at the GSP boot, the optional pin, the gate commands for nova, Linux 615 and Windows |
| [identity-and-costume.md](identity-and-costume.md) | What the guest derives its identity from, the costume rules, the costume ladder and its limits |
| [feature-families.md](feature-families.md) | The 28 control families, which chips have them, refused or answered, sent at boot anyway, truth sources |
| [blackwell-and-hopper.md](blackwell-and-hopper.md) | GB206 identity, FSP boot, GMMU v3, the N gate, GH100 in emulation, what a capture or cloud validation adds |
| [memory-model.md](memory-model.md) | Guest RAM registration, the swiotlb alias, D16 to D18, shadow page tables, gathers and region pools, fail-closed rules |
| [windows-and-traps.md](windows-and-traps.md) | Every shared-memory window, what it removes, what still traps per operation and per frame, the cost per trap |
| [isolation-and-xids.md](isolation-and-xids.md) | The isolation rules and the catalogue of every Xid seen with cause and fix |
| [performance-facts.md](performance-facts.md) | Ratios to native per operation and workload, where the remaining time goes, the ReBAR note |
| [methods.md](methods.md) | Gates, census, profile mode, coverage map, guest sweep, lab kit, emu-boot check, ABI drift check, how a new workload is measured |
| [windows-driver-code43.md](windows-driver-code43.md) | The Windows driver's register sequence before Code 43, devinit on Turing, the VBIOS scan, ranked causes |
| [windows-findings.md](windows-findings.md) | The running list of Windows findings P17 to step 23 (2026-09-28 to 2026-09-30), each with evidence, fix and memory-model dependency: the flip notifier (step 16), the disk-full repair (22), channel migration and the reconnect (23) |
| [windows-deferred-context-2026-09-28.md](windows-deferred-context-2026-09-28.md) | WDDM's deferred contexts: the NV5080 registration, INITIALIZE_CTX, PROMOTE_CTX and EVICT_CTX, the contract, what was missing |

## Dated notes

| Note | Scope |
|---|---|
| [astra-review-2026-09-26.md](astra-review-2026-09-26.md) | External review of the Windows X0 RPC phase and the Linux CUDA/Vulkan coexistence: the empty-GPU KGR static info gate, control layouts, the CUDA-only ladder |
| [va-alloc-bursts-2026-09-26.md](va-alloc-bursts-2026-09-26.md) | The `VA Space alloc failed` bursts at a multi-client teardown: the registry freed the owning client first; the fix |
| [alpha-gates-2026-09-27.md](alpha-gates-2026-09-27.md) | The alpha acceptance order of 2026-09-27: the v1/v2 fixes, Windows, the Linux desktop, latency; prerequisite checks |
| [stability-results-2026-09-27.md](stability-results-2026-09-27.md) | Stability checks across Windows B11, Linux L6 and the v1 import trials: no new host Xid, confirmed improvements, open items |
| [linux-graphics-optimization-2026-09-27.md](linux-graphics-optimization-2026-09-27.md) | Linux desktop B alone: stability and the hybrid-poll comparison |
| [wayland-multiguest-2026-09-27.md](wayland-multiguest-2026-09-27.md) | The stock 615 Wayland desktop and two guests with concurrent NVENC, NVDEC and Vulkan: configuration, rates, limits |
| [windows-2026-09-27.md](windows-2026-09-27.md) | Windows bring-up under QEMU: the driver's path to Code 43 and the first fixes |
| [nova-transport-compatibility-2026-09-27.md](nova-transport-compatibility-2026-09-27.md) | nova-core's transport and RPC separation series against the device's GSP model |
| [virtio-nvgpu-comparison-2026-09-27.md](virtio-nvgpu-comparison-2026-09-27.md) | Source review of nestrilabs/virtio-nvgpu, an API-forwarding relative of v1: what it does, what it reports, what that implies for v2 |
| [compression-isolation-2026-09-28.md](compression-isolation-2026-09-28.md) | How vGPU and MIG isolate compression (comptag lines follow the physical page), and the route for real compression per VM slice |
| [compression-guest-view-2026-09-28.md](compression-guest-view-2026-09-28.md) | What the 615 guest decides compression by, and every answer the device changes with `CARAXES_COMPRESSION=slice`; the Linux scrub-on-free precondition |
| [zbc-host-table-2026-09-28.md](zbc-host-table-2026-09-28.md) | ZBC with real compression: who may add to the host's table, placement and release, what the device forwards, the cap |
| [shadow-fault-race-2026-09-28.md](shadow-fault-race-2026-09-28.md) | The Xid 31 FAULT_PDE of desktop-b: nvidia-uvm maps through `MEM_OP` invalidates the software executor ran without a shadow plan; the fix |
| [uvm-faults-2026-09-28.md](uvm-faults-2026-09-28.md) | Managed memory and UVM faults: why `cudaMallocManaged` ends in a host Xid 31, the guest's replayable-fault path on TU10x, ranked options |
| [uvm-guest-ram-2026-09-28.md](uvm-guest-ram-2026-09-28.md) | Managed memory pinned in guest RAM without faults: module parameters, the preload-shim recipe, a no-fault nvidia-uvm mode for upstream |
| [video-path-2026-09-28.md](video-path-2026-09-28.md) | NVENC and NVDEC per frame in a 615 guest: channels, doorbells, interrupts and idle gaps; the gap to native is host engine time |
| [host-boost-2026-09-28.md](host-boost-2026-09-28.md) | The host card ran VM work at P8; what `PERF_BOOST` does; the device's activity-driven boost |
| [vkmark-frame-2026-09-28.md](vkmark-frame-2026-09-28.md) | vkmark per frame in a 615 guest: rings, interrupts and traps; the 60 Hz window costs 20 to 28 %; fixes ranked |
| [device-ce-2026-09-29.md](device-ce-2026-09-29.md) | The software CE's data movement on a device-owned host CE: the Windows paging channel's fills, a device VA space that maps only the VM's memory |
| [display-balloon-2026-09-29.md](display-balloon-2026-09-29.md) | SotTR's NVKMS refusal in a full 4 GiB guest: v1's display balloon, where a 615 guest's sizes come from, alternatives ranked |
| [dmem-cap-2026-09-29.md](dmem-cap-2026-09-29.md) | The display reserve on the upstream dmem cgroup controller: verified kernel, NVIDIA and systemd facts, the guest units, the test plan |
| [display-reserve-2026-09-29.md](display-reserve-2026-09-29.md) | v1's display balloon as a 615 RM patch: why RM and not NVKMS, how ISO scanout requests are identified, the design, the loader for game tests |
| [windows-traps-exit-2026-09-29.md](windows-traps-exit-2026-09-29.md) | Why a Windows CUDA short kernel costs 54 to 60 us: the trap census of one process, and the ways out ranked |
| [gr-preemption-2026-09-29.md](gr-preemption-2026-09-29.md) | SotTR's host Xid 109: the host context runs graphics WFI, the compute CTA/CILP request is dropped; the fix with its isolation argument |
| [gr-build-2026-09-29.md](gr-build-2026-09-29.md) | A Windows GR channel's host mirror scheduled 238 to 397 ms after its first doorbell: the host RM calls cost 10 ms, the fence carving the rest; the fix |
| [flr-host-events-2026-09-29.md](flr-host-events-2026-09-29.md) | After a KMD reset (FLR) no host event reached the guest: the host subdevice kept the old mirror's notifier action; the fix |
| [open-render-2026-09-29.md](open-render-2026-09-29.md) | Rendering through an open driver: nova has no channels in any tree; nouveau with GSP-RM 570.144 and Mesa NVK; what the device lacked; the vm10 ladder R1 to R8 |
| [sottr-invgate-2026-09-29.md](sottr-invgate-2026-09-29.md) | SotTR on Linux stuck on its loading screen: MMU invalidates held for other roots' shadow work; the scoped hold, the benchmark, the rendering check |
| [lockfree-2026-09-29.md](lockfree-2026-09-29.md) | The device lock's long holders: held rings, mirror builds on the worker, Windows' software doorbells, a SotTR loading crawl and its fix |
| [vblank-path-2026-09-29.md](vblank-path-2026-09-29.md) | Why a display-page write cost 147 to 180 us and an invalidate trigger 267 to 279 us; the fixes and where each is |
| [ctxcreate-2026-09-29.md](ctxcreate-2026-09-29.md) | cuCtxCreate, cuCtxDestroy, cuMemAlloc and cuMemFree against native: where the time goes, fixes ranked, the gather cache and mirrored admin copy channels with their A/B |
| [trap-census-2026-09-30.md](trap-census-2026-09-30.md) | Every trapped guest access per workload on Linux 615 and Windows (rates, cost, vCPU share), the fast paths and when they are on, the top ten sources with their spec-true elimination and the plan order |

## Format of a note

- Title, one line "Scope:", then "Facts" (one sentence each, evidence in brackets),
  "Consequence for Caraxes", "Open".
- Evidence is a line in the 615 tree (`/home/silas/git/Leandro/vendor/open-gpu-kernel-modules`,
  cited as `615 tree: path:line`), a run folder under `caraxes/runs`, or a `docs/vfio`
  document and section.
- No narrative, no history, no status; numbers that compare go in tables.

## Maintenance

- A fact changes: fix the bullet in its note and cite the new evidence; do not append a
  second, contradicting bullet.
- A new finding goes into the note of its topic; the concept goes to the wiki page that
  explains it.
- A new driver version: rerun the ABI diff ([abi-baseline.md](abi-baseline.md)), then
  recheck every `615 tree:` citation the diff touches.
