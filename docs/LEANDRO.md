<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Leandro: one NVIDIA GPU, several virtual machines, two architectures

## Abstract

Leandro shares one consumer NVIDIA GPU among Linux virtual machines on a host that keeps
using the card itself. Two architectures exist. **VirtIO Leandro** (v1) runs a modified guest
driver whose Resource Manager (RM) calls travel over vhost-user to a per-VM host backend that
replays them on the host driver. **VFIO Leandro** (v2, code name Caraxes) presents a synthetic
NVIDIA PCI device over vfio-user to an unmodified stock guest driver; the device plays the
role of the GPU System Processor firmware (GSP-RM), models the PCI BARs, and executes the
guest's work on the physical GPU as a client of the host RM through mirror channels and
shadow page tables. This document describes both, explains the driver structures they rest
on (the RM, the GSP, the BARs and the memory model), states what each architecture can and
cannot do, and closes with the ranked weaknesses and the work that would remove them by
Christmas 2026. Every concept named here has a page in [the wiki](wiki/README.md); every
number comes from a record cited in [the research notes](research/README.md) or the working
documents under [docs/caraxes](caraxes/README.md). Driver references name the open kernel
modules 615.71.09 by path and function.

## 1. The problem

- One GPU, several guests. A desktop or a small lab has one card; NVIDIA's vGPU product
  needs a data-center part, a licence and a host driver package the open modules do not ship.
- Stock drivers. A guest should install its distribution's NVIDIA packages and nothing else,
  so that CUDA, Vulkan, NVENC and a Wayland desktop work as on bare metal and driver updates
  come from the distribution.
- Isolation. A guest may control every byte it writes; the host desktop and the other
  guests must not see its memory, and a guest fault must stay a guest fault.
- Performance near native. GPU-bound work should run at the card's speed; the cost of the
  virtualisation layer should sit in setup, not in the submission path.
- The card is shared, not partitioned: the host's own desktop keeps running on it during
  every measurement in this project.

## 2. The two architectures

### 2.1 VirtIO Leandro (v1)

- The guest runs the NVIDIA user-space libraries unchanged and a Leandro kernel module,
  `virtio_nvrm.ko`, in place of `nvidia.ko`. The module owns `/dev/nvidia*` and forwards
  each supported ioctl as a request on a virtio queue.
- A per-VM host process, `vhost-user-nvrm`, validates each request against the host
  driver's ABI, translates pointers and file descriptors, and calls the host RM.
- GPU command buffers and submission registers are mapped into the guest through a
  shared-memory window, so submissions cost no forwarding; RM calls, mappings and
  synchronisation do.
- The guest's display is NVIDIA's displayless class with a generated EDID and guest
  timers for vblank ([DISPLAY.md](DISPLAY.md)).
- Details: [ARCHITECTURE.md](ARCHITECTURE.md), [SECURITY.md](SECURITY.md).

### 2.2 VFIO Leandro (v2, Caraxes)

- The guest runs the stock open kernel modules and user space of its distribution, with
  three registry keys in a modprobe file and, until sparse registration exists, one small
  DMA-range module ([wiki: sysmem and DMA](wiki/sysmem-and-dma.md)).
- A per-VM host process, `vfio-user-nvgpu`, is the whole device: PCI configuration space,
  BAR0 registers, the GSP boot exchange, the GSP message queue, every RPC, control and
  event the guest's kernel RM sends to what it believes is firmware.
- The device owns a fixed slice of the card's VRAM for the VM, registers the guest's RAM
  with the host RM, shadows the guest's GPU page tables, and runs the guest's channels on
  host channels it creates as an ordinary RM client.
- The guest's display is an emulated DisplayPort monitor served in the GSP role: EDID over
  AUX, DPCD, link training, vblank and flips on the device's clock.
- Details: [caraxes/README.md](caraxes/README.md) (decisions D1 to D20), the research notes,
  the wiki.

### 2.3 Side by side

```
VirtIO Leandro                          VFIO Leandro
guest app -> NVIDIA user space          guest app -> NVIDIA user space
  -> virtio_nvrm.ko (Leandro)             -> nvidia.ko, nvidia-uvm, nvidia-modeset (stock)
  -> vhost-user queue                     -> PCI config, BAR0/1/2, GSP message queue
  -> vhost-user-nvrm (host)               -> vfio-user-nvgpu (host): GSP-RM role, BARs,
  -> host RM ioctls                          shadow tables, mirror channels -> host RM
  -> host driver -> GPU                   -> host driver -> GPU
```

| Aspect | VirtIO Leandro (v1) | VFIO Leandro (v2) |
|---|---|---|
| Trust boundary | the VM; the backend validates ioctl envelopes, pointer spans and FD tokens; hostile guests not established ([SECURITY.md](SECURITY.md)) | the VM; the device validates every GPU address through shadow tables (D17, D18), every VRAM address against the slice (D11) and refuses compression state (D16); malicious guests untested ([wiki: isolation rules](wiki/isolation-rules.md)) |
| Guest changes | a Leandro kernel module replaces `nvidia.ko`; matching NVIDIA user space staged by hand | none to the driver; three registry keys; a temporary DMA-range module until D18 |
| Driver versions | host and guest user space must match one compiled ABI (580, 595, 610, 615 entries) | guest version independent of the host; the device carries per-version generated tables (615 baseline, 610 kept); host ABI selected at run time |
| What crosses the boundary | about 290 ioctl shapes; every RM call is a round trip | 14 RPC functions and 2 GMCAPI commands carrying 844 routed controls; RM calls stay in the guest kernel where the stock driver keeps them |
| Performance shape | native on GPU-bound work; 13 to 35 us per forwarded RM call; 100 ms per blocking sync; startup 0.19 s | native on GPU-bound work; 0 traps per submission with the shared-memory windows; setup paths 1.7x (context create) to 12.5x (free) of native; startup about 3.4 s |
| Isolation of memory | host-side accounting of registrations and a VRAM ledger; no partition of the card | a fixed VRAM slice per VM, registered guest RAM only, shadowed and filtered page tables, host scrub on free proven, cross-VM pattern test 0 hits |
| Display | displayless class, generated EDID, guest hrtimer vblank | emulated DisplayPort SST connector, device-paced vblank and flips, hotplug, cursor |
| What breaks | a driver whose ioctl layouts changed needs a new ABI entry and re-validation; refused controls stop debugging, P2P, MIG, profiling paths | a driver whose stable transport layer changed (baseline break); user space that stops on a refused control; a chip identity outside the Turing family for execution |
| What scales | VMs up to the host's pin budget and VRAM ledger | VMs up to the VRAM slots (2 GiB slots on an 8 GiB card beside the desktop), the 256 MiB BAR1 budget and one device process per VM |
| Host driver changes | none | none |
| Hypervisor | Cloud Hypervisor with patches 0001 to 0003 (vhost-user shared memory) | Cloud Hypervisor with patches 0005 and 0006 (vfio-user region I/O fds), 0004 optional |

## 3. The NVIDIA driver as seen from a virtual GPU

Wiki: [rm-resource-model](wiki/rm-resource-model.md), [rm-controls](wiki/rm-controls.md),
[kernel-rm-hal-and-registry](wiki/kernel-rm-hal-and-registry.md). Evidence:
[research/driver-structure.md](research/driver-structure.md).

- **Two halves.** The open kernel modules hold the kernel RM (`src/nvidia`), the OS layer
  (`kernel-open/nvidia`), UVM (`kernel-open/nvidia-uvm`), modeset and DRM. On every chip
  from Turing on, the physical half of the RM runs as firmware on the GPU System Processor
  (GSP-RM), and the kernel RM is its client. The open tree has no physical engine code for
  a non-GSP mode; `osinit.c` only prints a "Falling back to monolithic RM" notice.
- **The resource model.** Everything is an object under a client: `NV01_ROOT` (client),
  `NV01_DEVICE_0`, `NV20_SUBDEVICE_0`, memory objects, VA spaces, TSGs, channels, engine
  objects, events. Objects are allocated by class id with a parameter struct
  (`NV_ESC_RM_ALLOC`), driven by controls (`NV_ESC_RM_CONTROL`, a 32-bit command id plus a
  parameter struct), duplicated across clients (`DUP_OBJECT`) and freed. The resource server
  in `src/nvidia/src/libraries/resserv` keeps the tree.
- **Controls.** 1397 exported control ids in 615.71.09. Each carries flags in the generated
  export tables (`src/nvidia/generated/g_*_nvoc.c`): privilege, `ROUTE_TO_PHYSICAL` (answered
  by GSP-RM), `INTERNAL` (sent only by the kernel RM itself), `CACHEABLE`. 844 route to GSP.
  The kernel RM answers the rest itself, which is why a virtual device in the GSP role never
  sees a `GET_WORK_SUBMIT_TOKEN` or a `GET_NAME_STRING`.
- **HAL selection by boot registers.** `gpumgrGetGpuHalFactor` (`gpu_mgr.c`) and
  `rm_is_supported_device` (`osapi.c`) read `NV_PMC_BOOT_0`, `NV_PMC_BOOT_42` and
  `NV_PMC_BOOT_1` through BAR0 and pick the chip HAL and the runtime variant from them. No
  PCI device id enters that choice. `NV_PMC_BOOT_1` bits 17:16 select the vGPU guest variant,
  which is dead on Turing because its RPC infrastructure needs the SR-IOV plugin offload
  (`initRpcInfrastructure_VGPU`, `rpc.c`), so the only variant a Turing identity can run is
  the GSP client (`PF_KERNEL_ONLY`). `osHandleGpuLost` rereads `BOOT_0` and declares the GPU
  lost when it changes, so an emulated value is fixed for the life of the device.
- **The registry.** Module options (`NVreg_*`) and `NVreg_RegistryDwords` keys reach the
  kernel RM through `nvrm_registry.h`; in 615 they also travel to GSP-RM inside
  `GMCAPI_CMD_GSP_INIT` (44 entries in the captured request). Three keys make the stock
  driver skip work a virtual device cannot serve yet: `RmDisableGlobalCeUtils=1` (no copy
  engine self-test on a channel the device never learns the VA space of),
  `RMDisableScrubOnFree=1`, `RmRcWatchdog=0` (no watchdog GR channel).
- **Class list and engines.** The per-chip class database (`g_gpu_class_list.c`) names what
  a chip can allocate; the engine list (`ENGINE_MASK` in the static configuration) and the
  FIFO device-info table (`kfifoGetHostDeviceInfoTable_KERNEL`) name the engines and their
  fault ids. A stock TU106 Linux stack allocates 36 of the 48 GSP-bound classes the chip has.
- **NVML on top.** `nvidia-smi` and NVML use the same controls as any client; the `nvml` gate
  compares the guest's `nvidia-smi -q` with the host's over 11 checks. Identity answers are
  device-owned, telemetry (temperature, power, clocks, utilisation) is forwarded from the card.
- **What v1 forwards and v2 does not.** v1 replays the ioctl layer (about 290 verified
  signatures) and must translate every embedded pointer and FD; v2 sits one layer lower, where
  the kernel RM has already resolved handles and the payload is inline, but where the kernel
  RM also sends internal controls that an unprivileged host client can never forward as they
  are. The GSP boundary is not smaller than the ioctl boundary; it wraps the same alloc and
  control vocabulary ([caraxes/README.md](caraxes/README.md), idea P1).

## 4. The GSP and its emulation

Wiki: [gsp-rm](wiki/gsp-rm.md), [gsp-boot-exchange](wiki/gsp-boot-exchange.md),
[gsp-message-queue](wiki/gsp-message-queue.md), [gsp-rpc-and-events](wiki/gsp-rpc-and-events.md),
[gsp-control-categories](wiki/gsp-control-categories.md),
[gsp-transport-classes](wiki/gsp-transport-classes.md), [gsp-abi-615](wiki/gsp-abi-615.md).
Evidence: [research/gsp-transport.md](research/gsp-transport.md),
[research/gsp-surface.md](research/gsp-surface.md),
[research/gsp-rpc-events-classes.md](research/gsp-rpc-events-classes.md),
[research/abi-baseline.md](research/abi-baseline.md).

### 4.1 What GSP-RM does on real hardware

- GSP-RM is the physical RM compiled for the GSP's RISC-V core: it owns the engines, the MMU
  fault buffers, the BAR1 and BAR2 bind registers, clocks, power, display hardware and the
  compression policy. The kernel RM never writes `NV_PBUS_BAR1_BLOCK` or `NV_PBUS_BAR2_BLOCK`
  and never programs fault buffers; those HALs are stubs in the kernel-only variant.
- The kernel RM boots it (FWSEC from the VBIOS into the GSP falcon, Booter into SEC2, then
  the RISC-V image), links a message queue and from then on sends every physical operation
  as an RPC. A stand-in must produce the CPU-visible side effects of the boot and answer the
  RPCs; it never executes firmware.

### 4.2 The boot exchange (Turing, 615)

1. The CPU creates the command queue in guest RAM (`msgqTxCreate`, `msgq.c`) and writes
   `NV_PGSP_QUEUE_HEAD(0) = 0` while the falcon is halted.
2. FWSEC-FRTS runs in the model, the falcon is "reset into RISC-V", and the libos-argument
   page address lands in the falcon mailboxes.
3. Booter Load on SEC2 carries the WPR meta; the device links its status queue here, because
   the next step comes without a doorbell.
4. `GspStatusQueueInit` polls `msgqRxLink` for the device's queue header, then writes
   `NV_PGSP_MSGQ_TAIL(0) = 0`.
5. `GMCAPI_CMD_GSP_INIT` goes out with the system information and every registry entry as
   NVKV words (296 words, 142 keys in the capture); its response is the static configuration
   (GPU name, GID, FB length and regions, BAR PDE bases, engine mask, feature words) and the
   init completion. In 610 this was four RPCs and a `GSP_INIT_DONE` event.
6. `INIT_GSP_TRACE_CRASH_BUFFER`, the internal client allocations, the first
   `GSP_RM_CONTROL`s, `UPDATE_BAR_PDE` and the event import's `ALLOC_MEMORY` follow;
   `RmInitAdapter succeeded` ends the boot. A 615 boot carries 272 RPCs.

### 4.3 The message queue and its elements

- msgq v2: each side owns one 32-byte `msgqTxHeader` at the start of its transmit queue
  (`msgq.c`); the four pointers live in BAR0 registers `NV_PGSP_QUEUE_HEAD/TAIL(0)` and
  `NV_PGSP_MSGQ_HEAD/TAIL(0)` (`dev_gsp.h`), monotonic counters modulo 63 slots of 4 KiB.
- An element is 24 bytes of MCTP framing (magic `MCTP`, payload size, MCTP header, NVDM type
  0x2510DE7E for RM RPC or 0x2610DE7E for GMCAPI) followed by the RPC header and payload;
  no checksum, no sequence number in the element (610 had both). A consumed element's magic
  is overwritten with `SEEN`. An RPC above 65512 bytes travels as `CONTINUATION_RECORD`s.
- The doorbell: the sender fences and writes `QUEUE_HEAD`, which raises the GSP interrupt on
  real hardware and is the one trapped register that carries work in the device. One 615 RPC
  is four trapped accesses, 108 us median, against 3 us of device work.

### 4.4 RPC functions, events, controls

- A 615 GSP client on TU106 uses 14 of the 232 RPC functions (`rpc_global_enums.h`) plus the
  GMCAPI commands `GSP_INIT` and `GSP_SUSPEND`; the device answers all of them.
  `GSP_RM_CONTROL` and `GSP_RM_ALLOC` carry the RM vocabulary; `ALLOC_MEMORY` registers
  guest pages (the operational-event import sends it at every 615 boot); `UPDATE_BAR_PDE`
  hands the kernel's BAR2 root; `DUP_OBJECT`, `FREE`, `IDLE_CHANNELS`, `GPU_EXEC_REG_OPS`
  (never forwarded), `DUMP_PROTOBUF_COMPONENT` (empty), `ECC_NOTIFIER_WRITE_ACK`.
- Events: 45 defined, 36 dispatched by the kernel, 4 sent by the device (`POST_EVENT` for
  display hotplug and the event import, `RC_TRIGGERED` and `OS_ERROR_LOG` from a guest
  channel's error notifier, `GSP_INIT_DONE` on the 610 transport only). Events that panic or
  stop the guest (`TRIGGER_BUGCHECK`, `GSP_LOCKDOWN_NOTICE`, `FORCED_DRIVER_SHUTDOWN`) are
  never sent.
- Controls: of 844 routed, the device answers 777 (host-forwarded 69, synthesized 80,
  guest-derived 75, echo 65, refused 488) and 67 are unhandled, most of them static chip data
  waiting for a capture. Categories: A pure chip data (per-chip tables), B host-forwardable
  public getters (filtered by a deny list), C guest-state derived (channel registry), D device
  model (rule table REFUSE, DENY, ACK, MODEL, MIRROR, TABLE), E feature families the chip lacks
  (28 families, 302 controls).
- Classes: 143 GSP-bound classes a client can put on the wire, 48 in TU106's database, 36
  allocated by a stock stack; a class outside the database is refused with GSP-RM's own codes
  (`NV_ERR_NOT_SUPPORTED`, under a channel `NV_ERR_INVALID_CLASS`), so no phantom object exists.

### 4.5 What a stand-in must answer, what it must refuse, and why

- Answer: everything a stock client sends at boot and in the CUDA, NVML, Vulkan and display
  paths; a refusal there is an assertion the kernel tolerates (`NV_ERR_NOT_SUPPORTED` is what
  a vGPU guest gets) or a wall in user space (615 `cuInit` failed on a refused
  `CE_GET_LCE_SHIM_INFO`).
- Refuse because the feature is absent: NVLink, MIG, confidential computing, C2C, HDMI FRL, DP
  2.x and 22 more families on TU106, labelled `refused (feature absent on this chip)`; the
  same answer the card would give.
- Refuse because of isolation: profiler classes, register access, bus and power setters,
  anything whose effect reaches shared host hardware or another VM's state, labelled as an
  isolation refusal with `NV_ERR_INSUFFICIENT_PERMISSIONS` or `NV_ERR_NOT_SUPPORTED`.
- A gap (unhandled) also answers `NV_ERR_NOT_SUPPORTED`, from the dispatcher's unanswered arm;
  a refusal differs from a gap by being deliberate and labelled (decision D9: every answer
  carries its provenance).

### 4.6 Stability: the 615 ABI and later versions as deltas

- From 615.71.09 NVIDIA tags the transport and boot layer `NV_ABI_STABLE` (`nvtypes.h`,
  `gspifpub.h`): 12 structs (`msgqTxHeader`, `GSP_MSG_QUEUE_ELEMENT`, `GMCAPI_HEADER`, the boot
  arguments, the `GSP_FMC_*` parameters) and 5 enums, growing only into reserved space.
- The RM RPC layer is an opt-in "unstable interface" (`NVGMC_SI_UNSTABLE_INTERFACE_SUPPORT`,
  `gmcapi_system_info.h`); the 615 kernel opts in with both bits. Every control and alloc
  layout, RPC number and internal handle stays per version. The kernel still accepts only a
  GSP-RM image whose `.fwversion` equals its own.
- Decision D20: 615.71.09 is the recorded baseline (`caraxes/abi/gsp-615.71.09/`, nine TSVs
  and a MANIFEST); a new driver is snapshotted and diffed (`gsp-abi.py diff`), and the
  per-version tables are regenerated from the delta. The 610-to-615 delta was 185 driver-side
  changes in 34 groups, 26 touching what the device implements. A change in the stable layer
  is a baseline break that needs a decision.

### 4.7 Transport classes

Every routed control was classified by its structure (call, data, pointers, handles, side
effects) into what could leave the trapped path: ZT-PAGE 101 (static getters a page could
publish), ZT-QUEUE 155 (device-only answers on a polled queue), HOST 103 (a host RM round
trip: static, telemetry, GSS-legacy), TRAP 32 (addresses, handles, window order, host TSG
state), REFUSE 490. Controls are lifecycle traffic: 187 RPCs per CUDA process, 101 per
`nvidia-smi -q`, 1352 per GNOME start, 0 per frame and 0 per submission. The largest saving
is not a transport change but caching in the HOST class (1.85 s of GSS-legacy forwards over
the profiles); the polled queue saves about 130 us per RPC
([caraxes/gsp-transport-classes.md](caraxes/gsp-transport-classes.md)).

## 5. PCI, BARs and memory

Wiki: [pci-config-and-identity](wiki/pci-config-and-identity.md),
[bar0-registers](wiki/bar0-registers.md), [bar1-vram-window](wiki/bar1-vram-window.md),
[bar2-page-tables-and-userd](wiki/bar2-page-tables-and-userd.md),
[sysmem-and-dma](wiki/sysmem-and-dma.md), [page-tables-and-shadow](wiki/page-tables-and-shadow.md).
Evidence: [research/memory-model.md](research/memory-model.md),
[research/windows-and-traps.md](research/windows-and-traps.md).

| BAR | TU106 | Role for the driver | In the device |
|---|---|---|---|
| config space | 4 KiB, PM, MSI, PCIe capabilities | ids, BAR sizes, MSI enable; the RM reads it through the BAR0 mirror `NV_PCFG` at 0x88000 | seeded from the captured `lspci -xxxx`; the Turing RM reads link registers at fixed XVE offsets, so the layout must stay Turing's |
| BAR0 | 16 MiB registers | boot ids, config mirror, PROM window (VBIOS at 0x300000), the GSP and SEC2 falcons, interrupt tree at 0xb80000, MMU invalidate, usermode doorbell 0xbb0090, display FE | trapped, every access logged; two interrupt pages and the display page offered as memory; the doorbell as ioeventfds |
| BAR1 | 256 MiB, fixed | the CPU's window onto VRAM through GMMU tables: GPFIFO entries, USERD GPPut, Vulkan host-visible memory, the console | a private pool `[0, pool)` as a dma-buf area (default 64 MiB) so submissions trap 0 times; the rest trapped through the device's own two 32 MiB host BAR1 windows |
| BAR2 (PCI BAR3) | 32 MiB | the RM's virtual aperture for page tables, instance blocks and scrubs; bootstrapped through PRAMIN, verified by `kbusVerifyBar2_GM107` | a sealed memfd with one trapped flush page; synced at the RM's own flush reads, MMU invalidates, BAR2 binds and GSP doorbells |
| ROM | 512 KiB | the PCI image chain to the FWSEC image | captured two images plus a constructed third one (D8); nothing executes |

- **Which BAR0 registers a driver really touches.** Besides the GSP boot: `NV_PMC_BOOT_*`,
  `NV_PCFG` (config reads, MSI re-arm at `NV_XVE_CYA_2`), `NV_PBUS_BAR0_WINDOW` and PRAMIN,
  the interrupt tree (leaves, enables, top, trigger), `NV_PFB_PRI_MMU_INVALIDATE`, the flush
  registers, `NV_PTIMER`, the usermode page (`TIME_0/1` reads, the doorbell), the display FE
  interrupt page. The kernel RM's own interrupt-enable toggles at every RM lock acquire and
  release are 183 writes per Mutter frame, which is why the interrupt page exists.
- **VRAM: a fixed slice.** Guest FB address X is host VRAM address X. The device reserves
  `[B, E)` at the top of the host FB heap as one `NV01_MEMORY_LOCAL_USER` allocation,
  FIXED_ADDRESS, CONTIGUOUS, never compressible; slot n lies n profile sizes below the top,
  and a reservation that would leave the host under its headroom is refused (a third 2 GiB
  VM was refused with a 455 MiB shortfall while two ran). The guest is told the card's full
  address space so that its fake WPR lands in the host's reserved top where no engine uses it.
- **Sysmem: registered guest RAM.** Each 1 GiB chunk of guest RAM is one
  `NV01_MEMORY_SYSTEM_OS_DESCRIPTOR` over the device's mapping of the guest memfd, pinned
  with `FOLL_LONGTERM` (`os-mlock.c`), and gets one contiguous IOVA range. Registration is
  refused when the host would be left under 4 GiB available. Guest DMA address = GPA + one
  offset (`caraxes_dma_range`, the one guest module, D13) until D18 makes the device translate
  every entry itself.
- **The swiotlb alias.** A GSP-client guest refuses to initialise when its sysmem flush
  buffer's DMA address lies at or above 2^40 (`kern_mem_sys_gm107.c`); once the host GPU has
  switched to 64-bit IOVAs the launcher aliases the guest's swiotlb pool below that bound in
  the guest's table only, and the device follows the alias from the registered descriptor.
- **Page tables and the shadow.** The guest writes GMMU v2 tables in its VRAM slice as on
  bare metal (`kern_gmmu.c` places PDEs and PTEs in `ADDR_FBMEM`; UVM in `UVM_APERTURE_VID`).
  The device keeps a walk cache per guest root, patches it write-through on trapped or synced
  table writes, and admits a leaf only if it lies in this VM's reservation or in one
  registered guest-RAM descriptor, is not compressible and is not in a device-reserved range
  (`resolve_leaf`). Admitted leaves become host RM mappings (coalesced, gathered for
  scattered sysmem, freed by region) on a shadow VA space the host channels run on. A guest
  entry the filter refuses is counted and faults on the guest's own channel; the GPU never
  walks an unfiltered guest table. Shadowing costs no VRAM beyond the guest's own tables.
- **Windows.** Interrupt page, display page, doorbell token pool, BAR1 pool, BAR2 memfd, MSI
  re-arm ioeventfd: each opt-in, each falling back to the trapped path, each with a stated
  sync point at the last trapped access before a guest store can reach the GPU.

## 6. Submission and interrupts

Wiki: [channels-and-submission](wiki/channels-and-submission.md),
[interrupts](wiki/interrupts.md). Evidence: [research/windows-and-traps.md](research/windows-and-traps.md).

- **Channels.** A GPFIFO channel (`TURING_CHANNEL_GPFIFO_A` 0xC46F on TU106) has a GPFIFO ring
  of pushbuffer pointers and a USERD page holding GPPut; a TSG groups channels on one
  runlist. The guest's kernel RM allocates them through `GSP_RM_ALLOC`; the device constructs
  them under the guest's handles and mirrors each eligible one on a host channel in the
  guest's slice ([wiki: execution mirror](wiki/execution-mirror.md)).
- **A submission is four writes.** Two dwords of the GPFIFO entry and GPPut through BAR1, then
  the work-submit token to the usermode doorbell `NVC361_NOTIFY_CHANNEL_PENDING` at BAR0
  0xbb0090. The token is `runlist << 16 | ChID`, generated by
  `kfifoGenerateWorkSubmitTokenHal_TU102` (`kernel_fifo_tu102.c`) and written into the
  channel's notifier entry by the kernel RM itself; GSP-RM never sees
  `GET_WORK_SUBMIT_TOKEN`.
- **Zero-trap submission.** The BAR1 pool makes the three BAR1 writes plain memory writes. The
  device writes its own tokens into the notifier, from a pool outside every RM encoding, one
  per channel, handed out at the channel's first (trapped) doorbell; each token is one KVM
  datamatch ioeventfd on the doorbell register (Cloud Hypervisor patches 0005 and 0006, up to
  253 tokens per reply, 64 by default). A ring is then one KVM exit ending in an eventfd
  (1.24 to 1.43 us) with no VMM and no vfio-user message. A reactor thread polls the eventfds
  (spin, or hybrid at 3.1 % of a core) and rings the host doorbell after the pending shadow
  plans are applied (3.6 to 4.1 us inside the device).
- **Interrupts.** The Turing interrupt tree (`NV_CTRL` at 0xb80000: leaves, leaf enables, top,
  top enables, trigger; `intr_tu102.c`) plus `NV_PMC_INTR_EN_SET/CLEAR` on page 0 take about
  60 trapped accesses per interrupt in the trapped model. The interrupt page offers both pages
  as memory areas of a memfd; the device writes pending bits before it signals the MSI, and the
  reactor applies the guest's stores (write-1-to-clear, enables, trigger) with tag bits and a
  retrigger watchdog so that no store is lost. The display FE interrupt status page (0x611000)
  is the third such page.
- **MSI.** One vector; the RM re-arms it by writing `NV_XVE_CYA_2` through the config mirror
  once per interrupt (`kernel_bif_gm107.c`), 140 to 160 times per second in every CUDA
  workload; an ioeventfd without datamatch takes that trap to 0. The host RM's non-stall
  completion notifier is per engine and host-wide, so an idle guest received the desktop's
  completions as MSIs (331 per second in the coverage map); progress-gated forwarding is in
  the backlog.
- **The cost of a trap.** A BAR0 access answered by the model: 23 to 27 us p50 on a quiet
  host, 28 to 35 us busy, of which the device's own work is 1 to 2 us; the rest is the KVM exit,
  the VMM and the vfio-user socket round trip. A doorbell outside the pool: 36 us; inside: 4 to
  6 us. In the recommended configuration a submission, a copy, an event and a synchronise
  trap 0 times, and PPO submits 28000 times per second with no trap per submission.

## 7. Execution on the host

Wiki: [execution-mirror](wiki/execution-mirror.md), [isolation-rules](wiki/isolation-rules.md).
Evidence: [research/isolation-and-xids.md](research/isolation-and-xids.md),
[caraxes/execution.md](caraxes/execution.md), [caraxes/parallel.md](caraxes/parallel.md).

- **The host RM as a client.** The device opens `/dev/nvidiactl` as an unprivileged client
  (D10) and builds, per guest root, one shadow VA space; per guest TSG one host TSG; per
  eligible guest channel one host channel whose USERD lies in the guest's slice or pool page,
  with the same engine objects (CE, GR compute, 3D). The physical GPU walks the shadow tables,
  which name only admitted leaves; GR context buffers the host RM builds are mapped into the
  shadow, never into the guest's root.
- **Privilege (D12).** The process runs as an ordinary user with `CAP_SYS_ADMIN` permitted but
  not effective and raises it on one thread only around the controls the RM guards with
  `osIsAdministrator()` (`escape.c`, `control.c`): `GET_SURFACE_PHYS_ATTR`,
  `DMA_SET_PAGE_DIRECTORY`, `GPU_PROMOTE_CTX`, and the privileged `MEM_OP` of the TLB
  invalidator channel. Narrower than v1's `LEA_ADMIN_PRIV=1`, which keeps it effective.
- **The isolation rules.** D11: a fixed slice, refused when the host would run short. D16: no
  guest comptag line reaches the host, by policy in every answer (no compression), by a PTE
  filter forcing uncompressed kinds with COMPTAGLINE 0, and by an audit after each gate; a
  violation would corrupt the desktop's own compressed surfaces and needs a host reboot. D17:
  shadow tables are the DMA route and the guest stays bare metal. D18: the GPU may reach only
  this VM's slice and registered guest RAM through validated shadows.
- **What a guest can never do.** Map VRAM outside its slice or RAM it did not get registered;
  choose a compressible page kind; ring a channel that is not its own; read the host's
  registers (`GPU_EXEC_REG_OPS` answers `NOACCESS` for everything but the three boot ids);
  reach GSP-RM's bus, power, clock, ECC, NVLink or profiler paths; change the boot identity;
  see another VM's pattern (0 hits in both directions in the pattern test).
- **Faults stay with the VM.** Every host Xid line names the device process's pid and lands on
  that VM's own mirror channels; the catalogue of every Xid recorded (32 PBENTRY, 38 on a
  host-built GR context, 31 FAULT_PDE/PTE from mapping order, 13 kind and class mismatches,
  109 on a kill -9) records cause and fix, and the fixes went into the mapping and ordering
  paths, never into a host reset. The host desktop was judged unaffected by a host watch
  (`nvidia-smi` at most 0.087 s over 70 samples with two VMs).
- **Teardown.** Fail closed: mirror objects, descriptors, reservation, client, stopping at the
  first failure and leaving the reservation pinned rather than reused; the host's scrub on
  free (`PMA` without `SKIP_SCRUB`, `phys_mem_allocator.c`) clears the slice before the next
  owner, measured at 23 to 38 ms for 2 GiB and 0 pattern records after.
- **What has no quota.** BAR1 (256 MiB host-wide) is budgeted per device (2 x 32 MiB windows
  plus the pool); GPU time, channel count and RM work are not; a runaway VA teardown once
  stalled the host's `nvidia-smi` for 20 s without any Xid.

## 8. Display

Wiki: [display-and-kms](wiki/display-and-kms.md). Evidence:
[caraxes/display-e.md](caraxes/display-e.md), [caraxes/display-615.md](caraxes/display-615.md),
[WAYLAND.md](WAYLAND.md).

- **v1.** The displayless class (`NVA083_GRID_DISPLAYLESS`) with a generated EDID, guest
  hrtimers for vblank, RM events for fence wake-ups; GNOME on Wayland with KMS capture and
  NVENC through Sunshine passed a client-frame check on 2026-09-19. Two concurrent Shadow of
  the Tomb Raider guests at 720p completed on one card
  ([measurements/sottr-4q](measurements/sottr-4q/README.md)).
- **v2.** The device declares heads, windows, SORs and one DisplayPort SST connector per head
  (D15), answers the NV0073 display controls, serves EDID over AUX and a DPCD model from the
  `dp-sink` crate, completes link training, paces vblank and flip completion on its own timer
  (`AWAKEN` and `LAST_DATA` on the core channel, `NV9010_VBLANK_CALLBACK` in nvkms), reports
  hotplug through `POST_EVENT`, and scans out nothing physically. The stock `nvidia-modeset`
  and `nvidia-drm` see an ordinary monitor.
- **Rendering.** The guest's EGL, GL and Vulkan stack renders on the physical GPU through the
  shadow GR path; the display gate passes 11 of 11 with host execution; Weston at 60.00 Hz
  presented, Mutter at 59.40 Hz on the 615 guest; per presented frame 29 (Weston) and 139
  (Mutter) trapped accesses with the interrupt and display pages, against 157 and 662 without.
- **Capture and streaming.** KMS capture reads the block-linear scanout correctly; Sunshine
  `capture=kms` with software x264 streamed to Moonlight at 25.85 FPS with 101 ms host latency
  and 2.09 guest cores; NVENC through a mirror channel is the missing piece for a hardware
  encoder. No Caraxes SotTR frame rate exists yet.
- **Wayland is the goal.** X11 was a stepping stone in v1; in v2 the compositor runs on KMS
  with a stable vblank and page-flip cadence that the device controls, the issue v1's guest
  timers could not settle (v1 open question 27).

## 9. Identity

Wiki: [identity-and-costume](wiki/identity-and-costume.md),
[feature-families-and-chips](wiki/feature-families-and-chips.md). Evidence:
[research/identity-and-costume.md](research/identity-and-costume.md),
[research/feature-families.md](research/feature-families.md),
[research/blackwell-and-hopper.md](research/blackwell-and-hopper.md).

- **Profiles and names.** The guest sees `Leandro <Transport> <Personality>-<Profile>`
  (`Leandro VFIO RTX 2070-4G`, `Leandro VirtIO RTX 2070-4G`) from one naming module
  (`nvrm_abi::naming`, [NAMING.md](NAMING.md)); the profile is the guest framebuffer size,
  resolved from v1's vGPU-style catalogue (`RTX2070-4Q`) or a plain size. In v2 the name, FB
  length, regions, GID and engine mask are keys of the `GSP_INIT` response, so nothing is
  rewritten on the way back from the host; in v1 selected replies are rewritten.
- **Three axes (D14).** Chip (boot model, registers, engines, classes, families, in
  `chips/<chip>/`), board (PCI ids, BAR sizes, name, display connector, in `boards/*.toml`),
  driver version (generated RPC, control and static-info layouts per version). TU106 with the
  RTX 2070 board is the first implementation; GB206 with the RTX 5060 Ti board is the second
  chip, in pure emulation.
- **Costumes.** A presented chip is a base chip's tables plus a TOML overlay, so the overlay
  is the complete list of lies. PCI ids, subsystem, name and BAR sizes lie freely; the config
  space layout cannot (the whole GB206 space on the TU106 made nvidia-smi report link width
  25x, because the Turing RM reads `dev_nv_xve.h` offsets 0x84 and 0x88); boot identity can
  lie within the HAL family (TU104 on TU106 boots, passes the nvml gate and 10 of 12 CUDA
  tests) and across families only in emulation.
- **The limits.** Compute capability follows the chip identity, not GR info (`--sm-version
  7.0` on TU106 kept 7.5; the GA106 identity reported 8.6 with SM 7.5 in GR info). Class ids
  live in the pushbuffer: libcuda's `SET_OBJECT 0xC7C0` against TU106 engines raised host Xid
  13 "Class Mismatch", so a costume from another family cannot execute on the RTX 2070 without
  rewriting method streams. The GMMU format is per family (v3 from Hopper on). The Turing
  family is the costume's reach for execution.
- **Blackwell and Hopper in emulation.** The GB206 identity from the lab capture boots the
  610 guest to `RmInitAdapter succeeded` and the first channel allocation (N4) on the FSP
  Chain-of-Trust boot model (`kern_fsp_gb202.c`: EMEM queues, NVDM COT payload,
  `GSP_FMC_BOOT_PARAMS`) with a GMMU v3 walker for BAR2, and nvidia-smi reports compute
  capability 12.0; GH100 reuses the FSP model with a boot overlay and reports 9.0. Neither can
  execute on the TU106.

## 10. Performance

Wiki: [performance-shape](wiki/performance-shape.md). Evidence:
[research/performance-facts.md](research/performance-facts.md),
[caraxes/benchmark.md](caraxes/benchmark.md), [caraxes/optimization-backlog.md](caraxes/optimization-backlog.md).

Ratios are Caraxes over native on the RTX 2070 (host 615.71.09, 615 guest, quiet host,
n=5 unless stated); above 1 is slower for times, below 1 is slower for rates.

| Metric | native | v1 (610 closing) | v2 (matrix 2026-09-25) | v2 / native |
|---|---|---|---|---|
| short kernel, ms per launch | 0.006 | 0.009 | 0.015 | 2.5 |
| 30M-cycle kernel, ms | 16.3 | 16.0 | 17.3 (night) | 1.06 |
| rlprobe stage 4, s | 1.20 | 1.50 | 2.00 | 1.67 |
| h2d pinned 256 MiB, GB/s | 13.1 | 13.1 | 13.1 | 1.00 |
| h2d pinned 1 MiB, GB/s | 12.1 | 12.2 | 6.08 | 0.50 |
| cuInit, ms | 106 | 35.2 | 14.8 | 0.14 |
| cuCtxCreate, ms | 195 | 165 | 323 | 1.7 |
| cuMemAlloc 1 MiB, us | 35.8 | 93.4 | 201 | 5.6 |
| cuMemFree 1 MiB, us | 27.1 | 52.5 | 339 | 12.5 |
| PPO CartPole, steps/s | 107663 | - | 75070 | 0.67 |
| llama.cpp pp512 / tg128, tokens/s (2 GiB) | 8475 / 295 | - | 8150 / 272 | 0.96 / 0.92 |
| transformer training, tokens/s | 103760 | - | 104754 | 1.01 |
| ResNet-18 training, images/s | 506 | - | 526 | 1.04 |

- **What runs at native speed.** Long kernels, bulk transfers, device-bound training and
  inference: the GPU does the work on the guest's own tables and channels, and the device is
  not on that path.
- **What costs.** Submission-bound loops (PPO at 0.64 to 0.67: 407 s of device CPU in a 424 s
  run, the per-ring work of the reactor and the shadow barrier), allocation and free (5.6x and
  12.5x: the guest RM's MMU invalidate and flush traps, the memfd sync of 126 us mean carried
  by the flush read, and the fail-closed hold that applies invalidates of untrapped-USERD
  roots before the guest continues), context creation (the GPU's first work on 20 channels
  after the batched shadow maps took host calls from 13171 to 228 per creation), small pinned
  copies (bimodal with the card's PCIe link state).
- **Where the remaining time goes.** Per CUDA process about 0.55 s of device time outside every
  benchmark row: 1024 BAR1 pool page moves (363 ms) and 834 BAR2 memfd syncs (180 ms); host
  TSG preempts from the pool's warm-move barrier (3.9 s in 1112 calls over the coverage map);
  per RPC 108 us of trap protocol against 3 us of device work; idle interrupt forwarding.
- **v1's shape.** Native on GPU-bound work, 13 to 35 us per forwarded RM call, a flat 100 ms per
  blocking synchronisation, a pinned-page retention until guest restart; on today's builds v1
  is faster on allocation and context paths, v2 on submission latency and startup of a
  process, and both are native on bulk work.
- **The ReBAR outlook.** TU106's BAR1 is 256 MiB and its resizable-BAR capability offers only
  64, 128 and 256 MiB, so the pool copies pages in and out and the device reaches its slice
  through two 32 MiB windows (2.5 ms per window miss). On a card whose BAR1 covers its VRAM,
  the pool can cover all of guest BAR1 and the device maps its slice once; zero-copy pool
  moves still need a VMM change that re-points one 4 KiB page of a sparse area.

## 11. Verification

Wiki: [verification-methods](wiki/verification-methods.md),
[captured-cards](wiki/captured-cards.md), [contributing-a-capture](wiki/contributing-a-capture.md).
Evidence: [research/methods.md](research/methods.md), [caraxes/testing.md](caraxes/testing.md),
[TESTING.md](TESTING.md).

- **Gates.** v1's gates (`nvml`, `cuda`, `gpu`, `vdisplay`, `display`) run unchanged against v2
  with the same verdict rules (one JSON line, exit 0/1/2, skip only for a missing
  precondition, binary verdicts), so a green gate means what it meant in v1. Current: nvml
  pass, cuda 10 of 12 scored (cuda-managed and cuda-gdb open), display 11 of 11, on both the
  610 and the 615 guest; v1 on the 615 image: smi pass, cuda 14 of 14.
- **Census.** One driver-API operation at a time, bracketed by scratch-register markers,
  gives the traps per operation by page and register, the RPCs, the shadow plans and the
  doorbells; numbers count only with 0 host Xid on the device's pid.
- **Profile.** `CARAXES_PROFILE=1` aggregates count, device time and status per trapped
  access, RPC, control, host RM call and window hit, per device and per guest client, at 84 ns
  per trapped access; `profile-rank.py` ranks rows by count times mean latency.
- **Coverage.** 29 workload windows with LLVM function coverage reached 208 of 829 routed
  controls and 931 of 1464 device functions; a guest control sweep issues every user-callable
  getter once through the stock ioctl path.
- **ABI drift.** `tools/check.sh` renders the snapshot against the tables document and diffs
  it against a found 615 tree; 0 driver-side and 0 device-side changes on 2026-09-25. An
  emu-boot step boots the synthetic GPU without a host RM or a GPU and checks
  `RmInitAdapter succeeded` and `nvidia-smi`.
- **The lab kit and captured cards.** A self-contained pack captures a card the project has
  no host for (ids, config space, ROM, RM queries, a sweep of 164 getters, native traces,
  display, extras), redacts every identifier and refuses to pack when one remains; the
  operator runs it, the agent never does. Two cards are captured: the RTX 2070 (TU106, home)
  and the RTX 5060 Ti (GB206, workplace, 615.71.09, 2026-09-24).

## 12. Strengths, weaknesses and the road to Christmas 2026

### 12.1 Per architecture

| | VirtIO Leandro (v1) | VFIO Leandro (v2) |
|---|---|---|
| Strengths | native on GPU-bound work; allocation and context paths near native; startup 0.19 s; 14 of 14 CUDA tests on the 615 image; two SotTR guests ran on one card; a small, reviewable wire schema; no firmware model | stock driver and user space, distribution updates; guest driver version independent of the host; isolation by construction (slice, registered RAM, filtered shadows, no comptags) with a pattern test and an Xid catalogue; 0 traps per submission; display with a device-paced cadence on Wayland; the GSP surface recorded as a diffable ABI baseline; three-axis identity for other chips and versions |
| Weaknesses | a guest kernel module in place of the driver, staged user space; every RM call a round trip (13 to 35 us), 100 ms per blocking sync, a pinned-page retention; refused controls for debugging, P2P, MIG, profiling; hostile-guest isolation not established; display through the displayless class with guest timers | a 3 to 4 s startup; allocation and free 5.6x and 12.5x, PPO at 0.64 to 0.67; 4 GiB of pinned host RAM per VM and a guest DMA module until D18; a 256 MiB BAR1 shared with the desktop; no QoS for time, channels or RM work; execution honest only for Turing identities on the RTX 2070; malicious guests untested; 67 controls unhandled, 50 of them waiting for a capture |

### 12.2 Weaknesses ranked by what they cost the goals

Goals: isolation, stock drivers, performance near native, more chips, more guests per card.
Each row: the mechanism, the evidence, the mitigation and what it takes.

| # | Weakness | Goal hit | Mechanism | Evidence | Mitigation and what it takes |
|---|---|---|---|---|---|
| 1 | Whole-RAM registration and the guest DMA module (v2) | stock drivers, more guests, isolation depth | guest RAM is pinned in 1 GiB chunks at start and guest DMA addresses are made equal to host IOVAs by a guest-side module; a filter bug exposes all of guest RAM, and each VM pins its whole RAM on the host | 4 GiB pinned per VM; registration refused under 4 GiB host headroom ([research/memory-model.md](research/memory-model.md)) | D18 sparse, demand-driven registration: the device pins 2 MiB regions when a validated entry first names them and translates GPA to IOVA itself; then the module and the `_DMA` patches go (the vanilla gate). A model change in the device, no driver change; roadmap item 9 |
| 2 | Allocation and free cost (v2) | performance | the guest RM's MMU invalidate and flush accesses trap, the memfd sync rides on the flush read, and the fail-closed hold applies invalidates before the guest continues | cuMemAlloc 1 MiB 5.6x, cuMemFree 1 MiB 12.5x, 64 MiB free 791 to 2402 us with the hold ([research/performance-facts.md](research/performance-facts.md)) | the invalidate and flush page as memory scanned by the reactor with the sync moved to the doorbells (backlog item 3, 2 to 3 days); a hold limited to plans that map something new; a cheaper sync (KVM dirty log needs a VMM change) |
| 3 | Submission-bound loops (v2) | performance | every ring is a reactor wake-up, a shadow barrier and a host doorbell; the device's per-ring work bounds PPO | PPO 0.64 to 0.67 of native with 407 s of device CPU in 424 s | the poll-mode GSP and reactor (P24): one core serving several devices, batching rings; the GSP queue as a polled page (about 130 us per RPC, needs a read-only sparse area and ioeventfds in Cloud Hypervisor); a device-side timeline for the non-trap stalls first (backlog item 10) |
| 4 | No QoS between guests (both) | more guests, isolation | budgets exist for BAR1 only; GPU time, channel count and RM work of one VM can stall the host's `nvidia-smi` without any Xid | a VA teardown timed out host `nvidia-smi` for 20 s; 732088 refused BAR1 mappings from a window leak ([research/isolation-and-xids.md](research/isolation-and-xids.md)) | per-device budgets for mappings, RM calls per second and channel count; host TSG timeslice and priority for mirrored TSGs; a structural limit remains: Turing has no per-VM interrupt routing or SM partitioning, so time-slicing the card is the host RM's scheduler |
| 5 | Malicious guests untested (both) | isolation | every isolation run used the stock driver; the rules are enforced by the device, but no adversarial guest has tried to break them | parallel.md §6; v1 SECURITY.md "remaining risks" | a fuzzing guest against the GSP dispatcher and the shadow filter (the `vfio-user-nvgpu` fuzz targets exist for the queue), a hostile page-table writer, a doorbell storm; a test-suite item, no design change expected; v1 needs the source-object ownership policy across DUP and UVM imports it never got |
| 6 | Execution only for Turing identities on the RTX 2070 (v2) | more chips | class ids live in the pushbuffer and compute capability follows the chip identity; the GMMU format is per family | GA106 costume: host Xid 13 "Class Mismatch" on libcuda's first push; GB206 stops past N4 because no Blackwell method runs on the TU106 | a structural limit for one host card: a chip runs only on a host card of its family. The path to more chips is more host cards (the workplace RTX 5060 Ti, rented H100 or L4 for captures) and the three-axis tables; the GB206 device needs the CA6F/CAB5 method subset and full KGR static info, then a host of its own |
| 7 | 67 unhandled controls, 50 waiting for a capture (v2) | stock drivers | static chip data the open tree does not hold (`POWER_FEATURES`, `CHIP_SUB_REV`, SKU, ECID, the 50 A rows) can only come from the card | [research/gsp-surface.md](research/gsp-surface.md); the real TU106 `GSP_INIT` response is unmeasured | one passthrough capture of the RTX 2070 with a prepared procedure (the desktop must come down), or the lab kit with `--priv` on the workplace card for its chip; a capture, not a design change |
| 8 | BAR1 of 256 MiB shared with the desktop (v2) | performance, more guests | the pool copies pages in and out behind a barrier and the device reaches its slice through two 32 MiB windows | 1024 pool moves per CUDA process (363 ms); 2.5 ms per window miss; the pool ran at 32 MiB beside two VMs | a hardware feature: resizable BAR on a newer card lets each VM own its window and the device map its slice once; zero-copy moves need a VMM patch that re-points one page of a sparse area; on Turing a structural limit |
| 9 | Startup 3 to 4 s (v2) | performance | the stock driver's boot exchange (272 RPCs), the BAR2 page-table bootstrap and the first context's 20 channels all run through trapped paths | module load 3236 ms, cuCtxCreate 323 ms against 195 native | the polled GSP queue (35 ms per boot), the BAR2 memfd from the first access, lazy shadow plans; the boot will stay slower than v1's 0.19 s because the stock driver boots firmware it believes in |
| 10 | Host driver coupling (both) | stock drivers | v1's ABI entries must match the running host driver exactly; v2's host-side layouts (`hostabi.rs`, GSS-legacy sizes) are hand-kept per host version | the 610-to-615 delta needed one `hostabi.rs` row per changed host call | v2: the ABI drift check and the per-version workflow keep it a reviewed diff (a day per driver); v1: an `abi.toml` entry plus validation per version; a maintenance cost, not a limit |
| 11 | No hardware encoder on the display path (v2) | performance | Sunshine encodes in software until NVENC runs through a mirror channel | 25.85 FPS stream at 2.09 guest cores | an NVENC mirror (roadmap item 3); the encode stage of the gpu gate follows |
| 12 | Blackwell and Hopper unverified on a 615 guest (v2) | more chips | every Blackwell gate run used the 610 guest; the FSP timing, the register surface before `kgspInitRm` and the ACR layout are composed from headers | [research/blackwell-and-hopper.md](research/blackwell-and-hopper.md) Open | a 615 guest run of the N gate (a day); a passthrough trace on a Blackwell host or a rented GB202 to settle the composed values |
| 13 | Idle interrupt forwarding (v2) | performance, isolation | the host's non-stall notifier is per engine and host-wide; every desktop completion wakes an idle guest, a timing side channel | 331 MSIs per second idle in the coverage map | progress-gated forwarding judged from the guest's own USERD (backlog item 5, 1 to 2 days); full per-VM routing exists only on SR-IOV parts |
| 14 | v1's per-call and sync cost and pinned-page retention | performance | every RM call crosses the VM boundary; a blocking wait falls back to a 100 ms poll; forwarded OS descriptors stay charged after release | 13 to 35 us per call, 100 ms per blocking sync, restart per rep needed | a structural property of API forwarding for the per-call cost; the sync and the retention are v1 bugs (roadmap item 10) if v1 stays a reference path |

### 12.3 The road to Christmas 2026

- **Isolation.** D18 sparse registration (row 1), then the vanilla gate: stock Cloud
  Hypervisor patches only, no guest module, no `_DMA` patch. Adversarial guest tests (row 5).
  Progress-gated interrupts (row 13). A capture of the real `GSP_INIT` response (row 7).
- **Stock drivers.** The 50 A rows from a capture; the 12 display MODEL rows; `cuda-managed`
  (UVM fault forwarding, roadmap item 8) and `cuda-gdb`; NVENC (row 11); a second driver
  version through the delta workflow to prove D20 (row 10).
- **Performance near native.** Rows 2, 3 and 9 in that order: invalidate and flush page as
  memory, the polled GSP queue, the HOST caches (T1 to T3, up to 1.85 s over the profiles),
  the poll-mode reactor; the target is allocation under 100 us, PPO above 0.8 of native and a
  SotTR frame rate beside v1's.
- **More chips.** The GB206 device past N4 on the workplace card's captures; a 615 N gate; the
  Hopper overlay; the SR-IOV middleman (P18) on Ada and later where a VF gives hardware
  isolation and the device keeps the GSP role.
- **More guests per card.** QoS budgets (row 4), the reservation policy as configuration,
  timeslices for mirrored TSGs, and the ReBAR path on a newer card (row 8).
- **The decision.** v1 stays the reference for allocation-heavy and setup-heavy work until rows
  2 and 9 close; v2 is the product path for stock drivers, isolation and display. The SotTR
  benchmark over Sunshine and Moonlight, one guest and then two, decides the product question
  (P13) once v2 has an NVENC mirror.
