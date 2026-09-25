<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Leandro wiki

One page per concept that [LEANDRO.md](../LEANDRO.md) names: the NVIDIA driver as a virtual
GPU sees it, the GSP and its emulation, the PCI BARs and the memory model, submission,
execution, display, identity, performance and verification, for both architectures,
VirtIO Leandro (v1) and VFIO Leandro (v2, code name Caraxes). Each page is short, names the
driver by path and function, and links to the evidence.

## The driver

| Page | Contents |
|---|---|
| [rm-resource-model.md](rm-resource-model.md) | Clients, devices, subdevices, memory, channels and events as a tree; where each architecture cuts it |
| [rm-controls.md](rm-controls.md) | The 32-bit command ids, their flags, and who answers a control: the kernel RM, GSP-RM or the host |
| [kernel-rm-hal-and-registry.md](kernel-rm-hal-and-registry.md) | HAL selection from the three boot registers, the runtime variants, the registry keys a guest needs, what the kernel RM reads from the device |

## The GSP and its emulation

| Page | Contents |
|---|---|
| [gsp-rm.md](gsp-rm.md) | What GSP-RM owns on real hardware and what a stand-in must provide |
| [gsp-boot-exchange.md](gsp-boot-exchange.md) | The Turing boot in order, teardown and resume, the FSP boot of Blackwell and Hopper |
| [gsp-message-queue.md](gsp-message-queue.md) | msgq v2, the four pointer registers, the MCTP element, the doorbell, GMCAPI and NVKV |
| [gsp-rpc-and-events.md](gsp-rpc-and-events.md) | The 14 RPC functions a 615 client uses, the events, the classes on the wire |
| [gsp-control-categories.md](gsp-control-categories.md) | Categories A to E, what "refused" means, the D rule table |
| [gsp-transport-classes.md](gsp-transport-classes.md) | ZT-PAGE, ZT-QUEUE, HOST, TRAP, REFUSE; the cost of one RPC; the polled queue |
| [gsp-abi-615.md](gsp-abi-615.md) | `NV_ABI_STABLE` against the unstable layer, the snapshot, the workflow for the next driver, the drift check |

## PCI, BARs and memory

| Page | Contents |
|---|---|
| [pci-config-and-identity.md](pci-config-and-identity.md) | Configuration space, the BAR0 mirror, fixed XVE offsets, TU106 against GB206 |
| [bar0-registers.md](bar0-registers.md) | The pages and registers a GSP-client driver really touches, and which are memory in the device |
| [bar1-vram-window.md](bar1-vram-window.md) | The 256 MiB window, its host budget, the BAR1 pool, the ReBAR outlook |
| [bar2-page-tables-and-userd.md](bar2-page-tables-and-userd.md) | PRAMIN bootstrap, `kbusVerifyBar2`, USERD placement, the BAR2 memfd and its sync points |
| [sysmem-and-dma.md](sysmem-and-dma.md) | OS descriptors and IOVAs, guest RAM registration, the swiotlb alias, the DMA module and its removal (D18) |
| [page-tables-and-shadow.md](page-tables-and-shadow.md) | GMMU tables in the guest, the shadow walk cache, `resolve_leaf`, gathers and regions, the fail-closed rules |

## Submission, execution, display

| Page | Contents |
|---|---|
| [channels-and-submission.md](channels-and-submission.md) | TSGs, channels, USERD, GPPut, the work-submit token, the doorbell token pool and the reactor |
| [interrupts.md](interrupts.md) | The interrupt tree, the RM lock toggles, MSI and its re-arm, the interrupt and display pages, host completions |
| [execution-mirror.md](execution-mirror.md) | Mirror channels, TSGs and shadow VA spaces on the host RM; faults and containment |
| [isolation-rules.md](isolation-rules.md) | D11 to D18 with their evidence, what a guest can never do, what the rules do not give, v1's boundary |
| [display-and-kms.md](display-and-kms.md) | The stock display stack, v1's displayless class, v2's DisplayPort connector, Wayland, capture and streaming, encoding |
| [video-engines.md](video-engines.md) | NVENC and NVDEC: what the stock driver sends, v1's host objects, v2's mirrored video channel and its deferred object |

## Identity, performance, verification

| Page | Contents |
|---|---|
| [identity-and-costume.md](identity-and-costume.md) | The naming model, where every guest-visible fact comes from, the costume rules and the ladder |
| [feature-families-and-chips.md](feature-families-and-chips.md) | The 28 families, which chips have them, what a second chip (GB206) needs |
| [performance-shape.md](performance-shape.md) | The unit costs, per-operation and per-workload ratios, v1 against v2, where the time goes |
| [verification-methods.md](verification-methods.md) | Gates, benchmarks, census, profile, coverage, the checks without a GPU, the recipe for a new workload |
| [captured-cards.md](captured-cards.md) | One row per captured card (RTX 2070, RTX 5060 Ti), what each fed and lacks, a row template |
| [contributing-a-capture.md](contributing-a-capture.md) | The lab kit: run, root items, runtime, redaction, sending the tarball, what happens at home |
| [glossary.md](glossary.md) | Every term, one line |

## Where the depth is

- [docs/research](../research/README.md): one short evidence note per topic, every fact with a
  615 line or a run folder.
- [docs/caraxes](../caraxes/README.md): the working record of VFIO Leandro: decisions, studies,
  designs, measurements.
- [ARCHITECTURE.md](../ARCHITECTURE.md), [SECURITY.md](../SECURITY.md), [DISPLAY.md](../DISPLAY.md),
  [TESTING.md](../TESTING.md): VirtIO Leandro's design documents.
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
