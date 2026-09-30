<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Leandro wiki

One page per concept that [leandro.md](../leandro.md) names, for both architectures,
Leandro VirtIO (v1) and Leandro VFIO (v2): the NVIDIA driver as a
virtual GPU sees it, the GSP and its emulation, the PCI BARs and the memory model,
submission and interrupts, execution, display, identity, performance and verification.
Each page is short, names the driver by path and function, and links to the
[research notes](../research/README.md) as evidence.

## The driver

| Page | Contents |
|---|---|
| [rm-resource-model.md](rm-resource-model.md) | Clients, devices, subdevices, memory, channels and events as a tree; handles; the deferred API object; where each architecture cuts the tree |
| [rm-controls.md](rm-controls.md) | The 32-bit command ids and their flags, privilege, the RPC and handler statuses, who answers a control: the kernel RM, GSP-RM, GSS legacy, the Windows-only ids |
| [kernel-rm-hal-and-registry.md](kernel-rm-hal-and-registry.md) | HAL selection from the boot registers, the runtime variants, the module options a 615 Linux guest carries today, the keys that place memory, what the kernel RM reads from the device |
| [guest-drivers-and-abis.md](guest-drivers-and-abis.md) | The four guests on one device binary (Linux 615, Windows 617, nova-core, nouveau with NVK), the ABI detection at the GSP boot, per-version layouts, what each guest sends and needs |

## The GSP and its emulation

| Page | Contents |
|---|---|
| [gsp-rm.md](gsp-rm.md) | What GSP-RM owns on real hardware and what a stand-in for four guests must provide |
| [gsp-boot-exchange.md](gsp-boot-exchange.md) | The 615 Turing boot in order, the 570 exchange of nova and nouveau, teardown, resume and the TDR reset, the FSP boot of Blackwell |
| [gsp-message-queue.md](gsp-message-queue.md) | msgq v2 and v0, the pointer registers, the MCTP element, the doorbell's cost, GMCAPI and NVKV, the queue-head ioeventfd switch |
| [gsp-rpc-and-events.md](gsp-rpc-and-events.md) | The 14 RPC functions of a 615 client, the 570 and Windows sets, the events and their per-ABI payloads, the classes on the wire |
| [gsp-control-categories.md](gsp-control-categories.md) | Categories A to E, what "refused" means and who stops on a refusal, the D rule table |
| [gsp-transport-classes.md](gsp-transport-classes.md) | ZT-PAGE, ZT-QUEUE, HOST, TRAP, REFUSE; RPC counts and the cost of one RPC; the polled queue |
| [gsp-abi-615.md](gsp-abi-615.md) | `NV_ABI_STABLE` against the unstable layer, the 615, 610 and 570 snapshots, the workflow for a new version, the drift check |

## PCI, BARs and memory

| Page | Contents |
|---|---|
| [pci-config-and-identity.md](pci-config-and-identity.md) | Configuration space, the BAR0 mirror and the extended space, fixed XVE offsets, no MSI-X, TU106 against GB206 |
| [bar0-registers.md](bar0-registers.md) | The pages and registers a GSP-client driver touches, and which are memory, ioeventfds or answered before the device lock |
| [bar1-vram-window.md](bar1-vram-window.md) | The 256 MiB window, the device's two 16 MiB windows and the host BAR1 floor, the BAR1 pool, the ReBAR outlook |
| [bar2-page-tables-and-userd.md](bar2-page-tables-and-userd.md) | PRAMIN bootstrap, `kbusVerifyBar2`, the flush read, USERD placement, the BAR2 memfd and its sync points |
| [sysmem-and-dma.md](sysmem-and-dma.md) | OS descriptors and IOVAs, GPA DMA with whole-RAM registration off the loop, the former D13 helper route, D18's remaining step |
| [page-tables-and-shadow.md](page-tables-and-shadow.md) | Guest GMMU tables, the walk cache, `resolve_leaf`, gathers and the gather cache, held and `MEM_OP` invalidates, kinds and compression, managed memory |
| [traps-and-fast-paths.md](traps-and-fast-paths.md) | The path and cost of one trap, every fast path with its switch, default and sync point, traps per second per workload, the ranked sources and the plan order |

## Submission, execution, display

| Page | Contents |
|---|---|
| [channels-and-submission.md](channels-and-submission.md) | TSGs, channels, the work-submit token, the BAR1 pool, the token pool and guest-token ioeventfds, the reactor, held and posted doorbells |
| [channel-migration-and-deferred-api.md](channel-migration-and-deferred-api.md) | WDDM's deferred API objects and contexts, the ring migration to 65536 entries and a new USERD, how the host RM migrates the mirror in-stream, the bug it fixed |
| [interrupts.md](interrupts.md) | The interrupt tree, MSI and its re-arm, the interrupt page in tagged and exact mode, the display page, host completions and re-arming after an FLR |
| [execution-mirror.md](execution-mirror.md) | Mirror objects on the host RM, the build off the device lock, preemption modes, what runs where, faults and containment, Windows' shadow fence |
| [software-executor.md](software-executor.md) | Which channels run in the device and which on a host mirror, what the executor runs, the device CE, the cost of each side |
| [isolation-rules.md](isolation-rules.md) | D11 to D18, private pages, channel privilege, budgets and floors; what a guest can never do; what the rules do not give; v1's boundary |
| [display-and-kms.md](display-and-kms.md) | The stock display stack and the Windows KMD's use of it, v1's displayless class, v2's DisplayPort model, measurements, capture and encoding |
| [display-flips-and-reserve.md](display-flips-and-reserve.md) | The flip notifier model, the latch at the blank's start, the vblank path's costs, the modeset, FLR and host-event fixes, the NVKMS display reserve |
| [video-engines.md](video-engines.md) | NVENC and NVDEC: the stock driver's path, v1's host objects, v2's video mirror, the codec matrix, Sunshine NVENC against x264 |

## Identity, performance, verification

| Page | Contents |
|---|---|
| [identity-and-costume.md](identity-and-costume.md) | The naming model, the guest-ABI axis, where every guest-visible fact comes from, the costume rules and the ladder |
| [feature-families-and-chips.md](feature-families-and-chips.md) | The 28 families, the Windows-only answers, what a second chip (GB206) needs |
| [device-configuration.md](device-configuration.md) | The one device configuration since 2026-09-30: what `--host-exec` implies, the five knobs, removed variables and their warning, the `config:` block, the open switches per guest |
| [performance-shape.md](performance-shape.md) | Unit costs of v1 and v2, CUDA per operation, compute workloads, graphics on Linux, nouveau and Windows, NVENC and NVDEC, trap rates, where the time goes |
| [verification-methods.md](verification-methods.md) | The four-guest gate, the rendering check, the Windows test pack, the per-area gates, census and profile, the checks without a GPU |
| [captured-cards.md](captured-cards.md) | One entry per captured card (RTX 2070, RTX 5060 Ti), what each fed and lacks |
| [contributing-a-capture.md](contributing-a-capture.md) | The lab kit: run, root items, runtime, redaction, sending the tarball, what happens at home |
| [glossary.md](glossary.md) | Every term, one line |

## Where the depth is

- [docs/research](../research/README.md): one short evidence note per topic, every fact with a
  615 line or a run folder.
- [docs/vfio](../vfio/README.md): the design documents and studies of Leandro VFIO;
  [decisions](../vfio/decisions.md) D1 to D20; [known issues](../vfio/known-issues.md).
- [architecture.md](../virtio/architecture.md), [security.md](../virtio/security.md), [display.md](../virtio/display.md),
  [testing.md](../virtio/testing.md): Leandro VirtIO's design documents.
- The driver: the open kernel modules tree the citations name (615.71.09), referenced by path
  and function.

## Maintenance

- Principles and references live here; exact evidence lives in [docs/research](../research/README.md)
  and the run folders under `caraxes/runs`. A wiki page states a mechanism and names the
  function; a research note cites the line.
- A driver reference is a path and a function name, so it survives a version change. A line
  number is a 615.71.09 line and says so.
- When the driver version changes: run the ABI diff ([gsp-abi-615.md](gsp-abi-615.md)),
  recheck every cited line the diff touches in the research notes, and change a wiki page
  only where a mechanism changed.
- A number on a page comes from a record; when the record changes, the page changes with
  it, and the old number is not kept beside the new one.
- Website rules: one H1 per page, plain Markdown, relative links, no HTML beyond the SPDX
  comment, a first paragraph that works as the page's summary.
