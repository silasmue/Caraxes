<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Caraxes research notes

Rule: short and precise; bullets; every fact with its evidence; long form lives in [docs/caraxes](../caraxes/README.md); records under `caraxes/runs`.

Target: driver 615.71.09 only (D19, D20); 610.57.04 appears as one "610 delta" bullet at most.

## Notes

| Note | Scope |
|---|---|
| [driver-structure.md](driver-structure.md) | How the 615 kernel RM picks its HAL, the module options and registry keys the device needs, what the driver reads from where, the unstable-interface opt-in |
| [gsp-surface.md](gsp-surface.md) | The GSP surface in numbers: controls, RPCs, events, classes, boot types; categories A to E; families; what "refused" means |
| [gsp-transport.md](gsp-transport.md) | Message queue v2, GMCAPI, element framing, the doorbell trap, the boot exchange in order, teardown, suspend, `NV_ABI_STABLE` against unstable |
| [gsp-rpc-events-classes.md](gsp-rpc-events-classes.md) | The 14 RPCs a 615 client uses and their answers, the events, the classes on TU106 and the refusal rule, `ALLOC_MEMORY` |
| [abi-baseline.md](abi-baseline.md) | The 615.71.09 snapshot under `caraxes/abi/`, the diff tool, the 610-to-615 delta, the workflow for the next driver |
| [identity-and-costume.md](identity-and-costume.md) | What the guest derives its identity from, the costume rules, the costume ladder and its limits |
| [feature-families.md](feature-families.md) | The 28 control families, which chips have them, refused or answered, sent at boot anyway, truth sources |
| [memory-model.md](memory-model.md) | Guest RAM registration, the swiotlb alias, D16 to D18, shadow page tables, gathers and region pools, fail-closed rules |
| [windows-and-traps.md](windows-and-traps.md) | Every shared-memory window, what it removes, what still traps per operation and per frame, the cost per trap |
| [isolation-and-xids.md](isolation-and-xids.md) | The isolation rules and the catalogue of every Xid seen with cause and fix |
| [performance-facts.md](performance-facts.md) | Ratios to native per operation and workload, where the remaining time goes, the ReBAR note |
| [methods.md](methods.md) | Gates, census, profile mode, coverage map, guest sweep, lab kit, emu-boot check, ABI drift check, how a new workload is measured |
| [blackwell-and-hopper.md](blackwell-and-hopper.md) | GB206 identity, FSP boot, GMMU v3, the N gate, GH100 in emulation, what a capture or cloud validation adds |
| [windows-driver-code43.md](windows-driver-code43.md) | The Windows driver's register sequence before Code 43 by name, devinit on Turing, the VBIOS scan, ranked causes and the next experiments |
| [astra-review-2026-09-26.md](astra-review-2026-09-26.md) | External review of the Windows X0 RPC phase and the Linux CUDA/Vulkan coexistence: the empty-GPU KGR static info as the INSUFFICIENT_RESOURCES gate, control layouts, the CUDA-only ladder, instrumentation and audit targets |
| [compression-isolation-2026-09-28.md](compression-isolation-2026-09-28.md) | How vGPU and MIG isolate compression (1:1 or raw comptag lines follow the physical page), and the route for real compression per VM slice |
| [compression-guest-view-2026-09-28.md](compression-guest-view-2026-09-28.md) | What the 615 guest decides compression by, and every answer the device changes with `CARAXES_COMPRESSION=slice`; the Linux scrub-on-free precondition |
| [zbc-host-table-2026-09-28.md](zbc-host-table-2026-09-28.md) | ZBC with real compression: who may add to the host's table, placement and release, what the device forwards, the cap, what a guest sees and can learn |
| [shadow-fault-race-2026-09-28.md](shadow-fault-race-2026-09-28.md) | The Xid 31 FAULT_PDE of desktop-b: nvidia-uvm maps through `MEM_OP` invalidates the software executor ran without a shadow plan; the fix (the push stops at the invalidate until the shadow is applied) and the TSG fault-record lookup |
| [uvm-faults-2026-09-28.md](uvm-faults-2026-09-28.md) | Managed memory and UVM faults: why `cudaMallocManaged` ends in a host Xid 31 (the `GSP_GET_FEATURES` UVM bit), the guest's replayable-fault path on TU10x, UVM without faults, vGPU, what an unprivileged host process can do, ranked options |
| [video-path-2026-09-28.md](video-path-2026-09-28.md) | NVENC/NVDEC per frame in a 615 guest: channels, doorbells, interrupts and idle gaps; the gap to native is host engine time (likely the host card's power state, with the guest's PERF_BOOST not forwarded); the 610 delta is run-to-run spread; three fixes ranked |
| [uvm-guest-ram-2026-09-28.md](uvm-guest-ram-2026-09-28.md) | Managed memory pinned in guest RAM without faults: when UVM maps a GPU up front, module parameters, the preload-shim recipe and what breaks, what the device can do, a no-fault nvidia-uvm mode for upstream, the `GSP_GET_FEATURES` answer per guest OS (`HOST_OS_TYPE`) |
| [host-boost-2026-09-28.md](host-boost-2026-09-28.md) | The host card ran VM work at P8; what `PERF_BOOST` does in the RM (params, privilege, per-client hints, no cleanup on free); the device's activity-driven 3 s boost renewed at most every second, `CARAXES_HOST_BOOST=0`, and the A/B to measure |
| [one-abi-binary-2026-09-28.md](one-abi-binary-2026-09-28.md) | One device binary for every guest ABI: what was compile time, host-side against guest-side layouts, the detection (RMARGS magic, first element's frame), the optional pin, the tests and the gate commands for nova, Linux 615 and Windows |
| [vkmark-frame-2026-09-28.md](vkmark-frame-2026-09-28.md) | vkmark cube and clear per frame in a 615 guest: rings, interrupts and traps per frame; the 60 Hz window (vblank ISR holds every completion, the flip path's traps block the app) costs 20-28 %; the steady loop is interrupt-released; budget against native; five fixes ranked, and why the ratio is not SotTR's |
| [device-ce-2026-09-29.md](device-ce-2026-09-29.md) | The software CE's data movement on a device-owned host CE: the Windows paging channel's 19.7 GiB of fills (75 s of CPU in P32), why not split the channel, a device VA space that maps only the VM's memory |
| [display-balloon-2026-09-29.md](display-balloon-2026-09-29.md) | SotTR's NVKMS refusal in a full 4 GiB guest: v1's display balloon, where a 615 guest's sizes and Vulkan budget come from (all guest-kernel, PMA-derived except `fb_length`), why no device answer can reserve room for NVKMS, alternatives ranked, the recommended memacct application cap with its automatic size and test plan |
| [dmem-cap-2026-09-29.md](dmem-cap-2026-09-29.md) | The display reserve on the upstream dmem cgroup controller: verified kernel, NVIDIA 615 and systemd facts (charges go to the calling task, a limit below usage is a silent no-op, NVKMS stays uncharged, NVIDIA userspace reads `dmem.max`, systemd has no dmem support), the two guest units and their deviation from systemd's delegation rules, Linux B's HWE 7.0 variant, the test plan, a host-side design |
| [display-reserve-2026-09-29.md](display-reserve-2026-09-29.md) | v1's display balloon as a 615 RM patch: why RM and not NVKMS (the default API write lock makes free-and-retry atomic), how kernel ISO scanout requests are identified, why the reserve is sized from the largest live display allocation (IMP, bandwidth and head state checked and rejected), the design, the guest build and loader for game tests only |
| [windows-traps-exit-2026-09-29.md](windows-traps-exit-2026-09-29.md) | Why a Windows CUDA short kernel costs 54-60 us: the trap census of one CUDA process (BAR2 byte stores, 28 traps per interrupt, the doorbell, no TIME reads), and the ways out ranked: BAR2 memfd, interrupt page, usermode page with an O(changed) scan under QEMU, QEMU io fds, Windows under Cloud Hypervisor and its blockers |
| [gr-preemption-2026-09-29.md](gr-preemption-2026-09-29.md) | SotTR's host Xid 109 (a GR switch to host TSG 4 never acknowledged in 4.29 s): the host context runs graphics WFI, and the driver's compute CTA/CILP request is dropped; native Linux graphics never asks for GFXP; verdicts per candidate; the fix (read-only mode log, compute parity, owned GFXP in the VM's slice at 6.63 MiB per GR TSG) with its isolation argument and tests |
| [open-render-2026-09-29.md](open-render-2026-09-29.md) | Rendering through an open driver: nova has no channels or submission in any tree (7.3-rc5, drm-rust-next, NVK MR !43352 draft); nouveau of 7.2.7 loads GSP-RM 570.144 like nova and Mesa 25.2.8 NVK is in the 615 image; what nouveau's r570 path needs from the device (control frame, channel and VA space alloc layouts, Fermi GR masks, ZCULL, the 570 WAR id, NVDEC1/2 vectors, compression slice), what the branch adds, the vm10 ladder R1-R5 and the open bare-GR-channel mirror |
| [sottr-invgate-2026-09-29.md](sottr-invgate-2026-09-29.md) | SotTR on Linux stuck on its loading screen: MMU invalidates held for other roots' shadow work; the scoped hold and urgent-first worker, the benchmark (15.4 FPS average, 106.7 max), where the frame time goes (BAR2 syncs at invalidates, VRAM window churn, pool polls), the BAR2 victim cache, the four-guest gate and the ran-ahead detector |
| [lockfree-2026-09-29.md](lockfree-2026-09-29.md) | The device lock's long holders: held rings, mirror builds on the worker, per-holder and per-work attribution; Windows' holds are software doorbells of the paging channel (per-page table walks through the BAR1 windows); a SotTR loading crawl from full walks of a root with sysmem page tables and its fix; the invalidate that syncs only tables |
| [vblank-path-2026-09-29.md](vblank-path-2026-09-29.md) | Why a display-page write cost 147-180 µs (every trapped ack ran the BAR1 pool's replica merge, two uncached 4 KiB VRAM reads, three per flip) and the invalidate trigger 267-279 µs (the BAR2 memfd sync and the merge, both needed before the shadow planner); the fixes, where each is on the train branch, the expected gain |
| [ctxcreate-2026-09-29.md](ctxcreate-2026-09-29.md) | cuCtxCreate, cuCtxDestroy, cuMemAlloc and cuMemFree against native on the production candidate: the census and profile per operation, where the time goes (the first plan's gather descriptors, BAR2 memfd syncs, held invalidates, host channel builds and frees), a root process's copy channels in the software executor; fixes ranked; the shadow prefetch and mirrored admin copy channels with their A/B |
| [trap-census-2026-09-30.md](trap-census-2026-09-30.md) | Every trapped guest access per workload on Linux 615 and Windows (rates, cost, vCPU and device share), the fast paths and when they are on, the profile's routed-trap gap, the top ten sources with their spec-true elimination and the plan order |

## Format of a note

- Title, one line "Scope:", then "Facts" (one sentence each, evidence in brackets), "Consequence for Caraxes", "Open".
- Evidence is a line in the 615 tree (`/home/silas/git/Leandro/vendor/open-gpu-kernel-modules`, cited as `615 tree: path:line`), a run folder under `caraxes/runs`, or a `docs/caraxes` document and section.
- No narrative, no history, no status; numbers that compare go in tables.

## Maintenance

- A fact changes: fix the bullet in its note and cite the new evidence (new run folder, new line, new section); do not append a second, contradicting bullet.
- A new finding goes into the note of its topic; the working record and the status log stay in [docs/caraxes/README.md](../caraxes/README.md).
- A new driver version: rerun the ABI diff ([abi-baseline.md](abi-baseline.md)), then recheck every `615 tree:` citation the diff touches.
