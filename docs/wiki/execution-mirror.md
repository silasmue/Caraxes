<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Execution on the host: mirror channels

The guest's channels never touch the physical GPU. For each eligible guest channel the
device creates a host channel of its own, as an ordinary client of the host RM, bound to a
shadow VA space that names only admitted mappings, and rings it when the guest rings. The
host driver schedules the guest's work like any other process's.

## The mirror objects

| Guest object | Host mirror | Notes |
|---|---|---|
| VA space (a guest root) | one `ShadowSpace` with its own host VA space | filtered leaves only ([page-tables-and-shadow](page-tables-and-shadow.md)); one shadow per guest root, after Xid 31 showed one shadow mirroring the wrong context |
| TSG | one host TSG (`KEPLER_CHANNEL_GROUP_A`) | `NVA06C` preempt and timeslice controls act on it (TRAP class) |
| GPFIFO channel | one host channel with the same engine objects, its USERD in the guest's slice or pool page | UVM SW channels and channels with refused classes stay in software (`Mirror::eligible`) |
| CE objects | `TURING_DMA_COPY_A` on the mirror | CE channels run on the filtered shadow like GR; 400 of 400 in the isolation run |
| GR compute and 3D | `TURING_COMPUTE_A`, `TURING_A` on a host-managed shadow VA space | the GR context buffers the host RM builds are promoted into the shadow (`GPU_PROMOTE_CTX`), never into the guest's root; a host-built context on a guest root raised Xid 38 |
| error notifier | the mirror's notifier | becomes `RC_TRIGGERED` and `OS_ERROR_LOG` for the guest ([gsp-rpc-and-events](gsp-rpc-and-events.md)) |
| TLB invalidate | a privileged device channel issuing `MEM_OP MMU_TLB_INVALIDATE` of the guest PDB | the guest's register invalidate never reaches the physical TLB (Xid 31 CE2 FAULT_PDE, fixed) |

- Doorbell: the token pool names the mirror; the reactor waits for pending shadow plans,
  flushes the write-combined BAR1 window and rings the host doorbell
  ([channels-and-submission](channels-and-submission.md)).
- Page directory: `DMA_SET_PAGE_DIRECTORY` points the host channel at the shadow root, a
  privileged control; the device raises `CAP_SYS_ADMIN` on one thread around it
  ([isolation-rules](isolation-rules.md)).
- The host RM objects belong to the device process, so the host's `nvidia-smi` shows the
  VM's VRAM slice as used memory and the device process as the context owner.

## What runs where

| Work | Where it executes | Cost |
|---|---|---|
| kernels, 3D, copies on GR and CE | the physical GPU on the mirror | native for long kernels and bulk copies |
| the software copy engine for channels that stay in software | the device | only where a mirror is refused |
| UVM managed memory | not yet: the executor raises no faults; `cuda-managed` is the open CUDA test | fault forwarding to the guest is roadmap work |
| host TSG preempts | the host RM, from the pool's warm-move barrier and idles | 3.9 s in 1112 calls over the coverage map; one preempt at a kill -9 took 3.2 s with Xid 109 |

## Faults and containment

- A GSP-reported Xid is printed by the host kernel RM with the pid and name of the
  channel's owner (`kernel_gsp.c`); every host Xid of the project names the device process
  (`vfio-user-nvgpu`) and landed on that VM's own mirrors, with the host desktop unaffected.
- The device's own RC watchdog raises Xid 45 on stalled guest GR channels, a device choice
  with no host engine reset; guest-side Xids stay in the guest's journal.
- Teardown is fail closed: mirrors, descriptors, reservation, client, stopping at the first
  failure; the host's scrub on free clears the slice before the next owner.

## In VirtIO Leandro

- The guest's own RM objects are created on the host by the backend under mirrored file
  descriptors; the guest's channels are the host's channels, with the guest's page tables
  built by the host RM at the guest's request. There is no shadow: isolation depends on the
  objects and mappings created during setup ([ARCHITECTURE.md](../ARCHITECTURE.md)).

## Depth

- [caraxes/execution.md](../caraxes/execution.md): the mirror objects, the two execution walls,
  the contained Xids.
- [caraxes/host-exec.md](../caraxes/host-exec.md): the standalone primitive that proved a host
  channel walks a guest-built page directory.
- [caraxes/exec-formats.md](../caraxes/exec-formats.md), [caraxes/host-gr.md](../caraxes/host-gr.md):
  PBDMA faults, GMMU entries, the GR context on a host channel.
- [research/isolation-and-xids.md](../research/isolation-and-xids.md): the Xid catalogue.
