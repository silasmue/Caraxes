<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Execution on the host: mirror channels

The guest's channels never touch the physical GPU directly. For each eligible guest channel
the device creates a host channel of its own, as an ordinary client of the host RM, bound to
a shadow VA space that names only admitted mappings, and rings it when the guest rings. The
host driver schedules the guest's work like any other process's. This is the default with
`--host-exec` since 2026-09-30 ([device-configuration](device-configuration.md)); which
channels stay in the device is on [software-executor](software-executor.md).

## The mirror objects

| Guest object | Host mirror | Notes |
|---|---|---|
| VA space (a guest root) | one `ShadowSpace` with its own host VA space | filtered leaves only ([page-tables-and-shadow](page-tables-and-shadow.md)); one shadow per guest root, after Xid 31 showed one shadow mirroring the wrong context |
| TSG | one host TSG (`KEPLER_CHANNEL_GROUP_A`) | `NVA06C` preempt and timeslice controls act on it; the builds of one guest TSG share it: the first makes it, the others join (`mbuild.rs` `TsgCell`) |
| GPFIFO channel | one host channel with the same engine objects, allocated with `NVOS04_FLAGS_CHANNEL_DENY_PHYSICAL_MODE_CE` | its USERD in the guest's slice or pool page; on Windows a private host USERD mirrors the guest's sysmem USERD (`CARAXES_SHADOW_USERD`) |
| CE objects | `TURING_DMA_COPY_A` on the mirror | CE channels run on the filtered shadow like GR; 400 of 400 in the isolation run |
| GR compute and 3D | `TURING_COMPUTE_A`, `TURING_A` on a host-managed shadow VA space | the GR context buffers the host RM builds are promoted into the shadow (`GPU_PROMOTE_CTX`), never into the guest's root; a host-built context on a guest root raised Xid 38 |
| preemption modes | bound at the build, before the first schedule | the compute mode the guest asked for (CTA or CILP), checked by GET and never lowered; graphics stays WFI ([research: gr-preemption](../research/gr-preemption-2026-09-29.md)) |
| error notifier | the mirror's notifier | becomes `RC_TRIGGERED` and `OS_ERROR_LOG` for the guest ([gsp-rpc-and-events](gsp-rpc-and-events.md)) |
| non-stall completions | host OS events on the engines' notifiers, armed REPEAT | raise the guest's vectors; disarmed before the event is freed ([research: flr-host-events](../research/flr-host-events-2026-09-29.md)) |
| TLB invalidate | a privileged device channel issuing `MEM_OP MMU_TLB_INVALIDATE` of the guest PDB | the guest's register invalidate never reaches the physical TLB (Xid 31 CE2 FAULT_PDE, fixed) |

- Doorbell: the token pool or a guest token names the channel; the reactor rings the host
  doorbell ([channels-and-submission](channels-and-submission.md)).
- Page directory: `DMA_SET_PAGE_DIRECTORY` points the host channel at the shadow root, a
  privileged control; the device raises `CAP_SYS_ADMIN` on one thread around it
  ([isolation-rules](isolation-rules.md)).
- The host RM objects belong to the device process, so the host's `nvidia-smi` shows the
  VM's VRAM slice as used memory and the device process as the context owner.

## The build

- A channel's first doorbell takes the build's inputs under the device lock and hands the
  host RM calls to the shadow worker as one job (`mirror.rs` `shadow_build`,
  `build_on_host`): host TSG, USERD and notifier objects, the channel, the engine objects,
  the preemption bind. The job runs after the root's shadow plans, so the set-A mirror
  exists before the host RM validates the GPFIFO VA.
- The commit (`commit_build`) takes the device lock briefly: schedule, token, the GPPut the
  host RM cleared written back, the ring ([vfio/execution.md](../vfio/execution.md) §9.2).
- A Windows D3D device's GR build took 238 to 397 ms, almost all of it the new VA space's
  first plan carving the shadow fence per leaf; the fence is now carved once per region
  ([research: gr-build](../research/gr-build-2026-09-29.md)). Build costs per engine are on
  [software-executor](software-executor.md).

## What runs where

| Work | Where it executes | Cost |
|---|---|---|
| kernels, 3D, copies on GR and CE | the physical GPU on the mirror | native for long kernels (17.3 ms against 16.3) and bulk copies (h2d 256 MiB 13.1 GB/s, as native) ([performance-facts](../research/performance-facts.md)) |
| NVENC and NVDEC | the physical GPU on video mirrors | [video-engines](video-engines.md) |
| the guest kernel's privileged copy channels, UVM channels, bare GR channels | the device's software executor; large transfers on the device CE | [software-executor](software-executor.md) |
| managed memory | Linux: the device reports UVM off, so `cudaMallocManaged` fails cleanly instead of faulting on the host; Windows: on, without concurrent access | [uvm-faults](../research/uvm-faults-2026-09-28.md); [windows-findings](../research/windows-findings.md) P31 |

## Faults and containment

- A GSP-reported Xid is printed by the host kernel RM with the pid and name of the
  channel's owner (`kernel_gsp.c`); every host Xid of the project names the device process
  (`vfio-user-nvgpu`) and landed on that VM's own mirrors, with the host desktop unaffected.
- The device's own RC watchdog raises Xid 45 on stalled guest GR channels, a device choice
  with no host engine reset; guest-side Xids stay in the guest's journal.
- With graphics WFI a guest context that does not drain holds GR until the host's 4.29 s
  context-switch timeout, then Xid 109 RCs that TSG only (SotTR on Linux B,
  [research: gr-preemption](../research/gr-preemption-2026-09-29.md)).
- A root whose GPPut store does not trap has each invalidate applied before the guest
  continues, scoped to the invalidated root since f2af77c6
  ([research: sottr-invgate](../research/sottr-invgate-2026-09-29.md)).
- Teardown is fail closed: mirrors, descriptors, reservation, client, stopping at the first
  failure; the host's scrub on free clears the slice before the next owner.

## Windows

- The shadow fence (`CARAXES_SHADOW_FENCE`, Windows launcher): a host placeholder over
  [0x1_2000_0000, 0x1_4000_0000) in every GR and CE shadow VA space, so the host's own
  buffers land above the range where Windows keeps kernel data
  ([research: windows-findings](../research/windows-findings.md), P24 to P29).
- Deferred contexts and ring migrations run through the 0x5080 object
  ([channel-migration-and-deferred-api](channel-migration-and-deferred-api.md)).

## In Leandro VirtIO

- The guest's own RM objects are created on the host by the backend under mirrored file
  descriptors; the guest's channels are the host's channels, with the guest's page tables
  built by the host RM at the guest's request. There is no shadow: isolation depends on the
  objects and mappings created during setup ([architecture.md](../virtio/architecture.md)).

## Depth

- [vfio/execution.md](../vfio/execution.md): the mirror objects, the two execution walls,
  the held invalidate (§8), the device lock (§9).
- [vfio/host-exec.md](../vfio/host-exec.md): the standalone primitive that proved a host
  channel walks a guest-built page directory.
- [vfio/exec-formats.md](../vfio/exec-formats.md), [vfio/host-gr.md](../vfio/host-gr.md):
  PBDMA faults, GMMU entries, the GR context on a host channel.
- [research/isolation-and-xids.md](../research/isolation-and-xids.md): the Xid catalogue.
