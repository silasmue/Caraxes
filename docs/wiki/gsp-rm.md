<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GSP-RM

The GPU System Processor (GSP) is a RISC-V core on every NVIDIA GPU from Turing on. GSP-RM
is the physical half of the Resource Manager compiled to run on it. The kernel RM in the
open kernel modules is its client, and VFIO Leandro's device plays GSP-RM's role for the
guest: it produces the CPU-visible side effects of the boot and answers the RPC stream.

## Division of labour on real hardware

| GSP-RM owns | The kernel RM does |
|---|---|
| engines, runlists, channel scheduling | allocates channels and TSGs through RPC, writes GPFIFO entries and rings doorbells |
| the MMU fault buffers, `NV_PBUS_BAR1_BLOCK`, `NV_PBUS_BAR2_BLOCK` (the kernel never writes them) | bootstraps the BAR2 page tables through PRAMIN and hands its root over with `UPDATE_BAR_PDE` |
| clocks, power, thermal, the PMU | asks through controls; `nvidia-smi` telemetry is GSP data |
| the display hardware behind the NV0073 controls | runs nvkms on top |
| the compression policy and the comptag store | is told which page kinds are compressible |
| static chip data (`GspStaticConfigInfo`, 1608 bytes in 615) | receives it once in the `GSP_INIT` response |

- The kernel RM's view of its own surface: `src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c`
  (`kgspInitRm`, `kgspRecvPoll`, the event dispatcher), `src/nvidia/src/kernel/vgpu/rpc.c`
  (the RPC encoders).
- The kernel RM waits for every reply by spinning (`kgspRecvPoll`, `osSpinLoop`); all RPCs
  but `ECC_NOTIFIER_WRITE_ACK` and the fire-and-forget GMCAPI commands are synchronous.

## What a stand-in must provide

1. The boot side effects: two falcon register blocks (GSP at BAR0 0x110000, SEC2 at
   0x840000), the WPR2 and GFW status registers, a PROM window with a parseable VBIOS
   ([gsp-boot-exchange](gsp-boot-exchange.md)).
2. The message queue in guest RAM and the four pointer registers
   ([gsp-message-queue](gsp-message-queue.md)).
3. An answer to `GMCAPI_CMD_GSP_INIT` and to the 14 RPC functions a 615 client uses
   ([gsp-rpc-and-events](gsp-rpc-and-events.md)).
4. An answer to every routed control the client sends, sorted by where the truth lives
   ([gsp-control-categories](gsp-control-categories.md)).
5. The events the client needs: display hotplug, channel errors, the operational-event
   buffer; never the ones that stop the guest.
6. Refusals that look like GSP-RM's own: `NV_ERR_NOT_SUPPORTED` for an absent feature,
   `NV_ERR_INVALID_CLASS` under a channel, `NV_ERR_INSUFFICIENT_PERMISSIONS` for a privileged
   path ([feature-families-and-chips](feature-families-and-chips.md)).

## What the stand-in does not do

- It executes no firmware: booter images, the GSP-RM radix3 image and the FWSEC image are
  parsed by the guest and never read by the device.
- It does not own the physical engines; the guest's work runs on host channels the device
  creates as an RM client ([execution-mirror](execution-mirror.md)).
- It does not run GSP-RM's log buffers; a decoded line would need the per-version metadata
  of the guest's `gsp_*.bin`.

## Why the GSP boundary and not the ioctl boundary

- The vGPU-VF transport, which would have given a paravirtual RPC channel without a firmware
  model, is dead on Turing ([kernel-rm-hal-and-registry](kernel-rm-hal-and-registry.md)).
- Everything the guest's user space asks is answered by the guest's own kernel RM, so no
  pointer or file-descriptor translation exists on this boundary; the payloads of the RPCs are
  inline.
- The vocabulary is not smaller: `GSP_RM_ALLOC` and `GSP_RM_CONTROL` wrap the RM class and
  control space, and the kernel RM adds the internal controls it sends on its own
  ([caraxes/README.md](../caraxes/README.md), idea P1).

## Depth

- [research/gsp-surface.md](../research/gsp-surface.md): the surface in numbers.
- [caraxes/gsp-surface.md](../caraxes/gsp-surface.md), [caraxes/gsp-plan/README.md](../caraxes/gsp-plan/README.md):
  the inventory and the plans.
- [caraxes/other-architectures.md](../caraxes/other-architectures.md): the boot path on
  Ampere, Ada, Hopper and Blackwell.
