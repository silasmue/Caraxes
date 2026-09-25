<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Channels and submission

GPU work is submitted through channels: a ring of pushbuffer pointers (the GPFIFO), a
per-channel page holding the put and get pointers (USERD), and a doorbell register that
tells the hardware a channel has work. VFIO Leandro makes a submission cost no trap and no
message; VirtIO Leandro maps the submission registers into the guest for the same reason.

## The objects

| Object | Class on TU106 | Role |
|---|---|---|
| TSG | `KEPLER_CHANNEL_GROUP_A` 0xA06C | a time-slice group: channels that share a context and a runlist slot; `NVA06C_CTRL_CMD_GPFIFO_SCHEDULE`, `PREEMPT`, `SET_TIMESLICE` |
| channel | `TURING_CHANNEL_GPFIFO_A` 0xC46F (GB206: `BLACKWELL_CHANNEL_GPFIFO_B` 0xCA6F) | GPFIFO ring, USERD, an instance block, an error notifier; `NVA06F_CTRL_CMD_GPFIFO_SCHEDULE`, `BIND` |
| engine objects | `TURING_COMPUTE_A` 0xC5C0, `TURING_A` 0xC597 (3D), `TURING_DMA_COPY_A` 0xC5B5 | set on a subchannel with `SET_OBJECT` in the method stream |
| UVM SW | `GP100_UVM_SW` | a software class on subchannel 5; such channels stay in software in the device |

- The kernel RM allocates them through `GSP_RM_ALLOC`; the FIFO device-info table names the
  engines, runlists and fault ids (`kfifoGetHostDeviceInfoTable_KERNEL`,
  `src/nvidia/src/kernel/gpu/fifo/kernel_fifo.c`).
- Class ids are written into the pushbuffer by user space (libcuda's `SET_OBJECT`), which is
  why a costume of another family cannot execute on the physical chip
  ([identity-and-costume](identity-and-costume.md)).

## A submission is four writes

1. Two dwords of the GPFIFO entry (pushbuffer address and length) through BAR1.
2. USERD GPPut through BAR1.
3. The work-submit token to the usermode doorbell `NVC361_NOTIFY_CHANNEL_PENDING`
   (`src/common/sdk/nvidia/inc/class/clc361.h`) at BAR0 0xbb0090 on TU106.

- The token is `runlist << 16 | ChID`, built by `kfifoGenerateWorkSubmitTokenHal_TU102`
  (`src/nvidia/src/kernel/gpu/fifo/arch/turing/kernel_fifo_tu102.c`) from the `RUNLIST_ID`
  and `VECTOR` fields of `NV_CTRL_VF_DOORBELL` (`dev_ctrl.h`), bits 22:16 and 11:0.
- The kernel RM answers `NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN` itself
  (`kchannelCtrlCmdGpfifoGetWorkSubmitToken_IMPL`, `kernel_channel.c`; not routed to GSP)
  and writes the token into the channel's notifier entry
  (`kchannelNotifyWorkSubmitToken_IMPL`), from where libcuda reads what it rings.

## Zero-trap submission in VFIO Leandro

- The BAR1 pool turns writes 1 and 2 into plain memory writes
  ([bar1-vram-window](bar1-vram-window.md)).
- The doorbell token pool (`CARAXES_DOORBELL_TOKENS=N`, default 64, at most 253): the device
  writes its own tokens into the notifier entry, from a namespace outside every RM encoding
  (bits the RM never sets), one per channel, assigned at the channel's first doorbell, which
  traps and carries the guest's token. Each token is one KVM datamatch ioeventfd on the
  doorbell register, declared to the VMM through `VFIO_USER_DEVICE_GET_REGION_IO_FDS`
  (Cloud Hypervisor patches 0005 and 0006, [patches/README.md](../../patches/README.md)).
- A ring is then one KVM exit ending in an eventfd (1.24 to 1.43 us for slots 0 to 63; KVM
  scans datamatch entries at about 25 ns each, so a value outside the pool pays the whole
  scan and then the trap, 36 us).
- The doorbell reactor (`CARAXES_DOORBELL_POLL=spin|hybrid`) polls the eventfds on one core
  instead of sleeping in `epoll_wait`; the eventfd names the channel, so the device reads no
  GPPut. It waits for pending shadow plans (`wait_all`), flushes the write-combined window and
  rings the host doorbell of the mirror channel: 3.6 to 4.1 us inside the device. Hybrid
  costs 3.1 % of a core, spin a whole one.
- The first doorbell of each channel, nvidia-uvm's rings and any value outside the pool
  still trap.

## Measured

- With the recommended windows a submission, a copy, an event and a synchronise trap 0
  times net of background; PPO submits 28000 times per second with no trap per submission.
- Short kernel: 0.015 ms per launch against 0.006 native (2.5x); a launch after 30 ms idle
  680 us against 155 to 177 us, with no trap in its window (a non-trap stall, unattributed).

## In VirtIO Leandro

- Channel and mapping setup use the forwarded control path; the backend maps the GPFIFO and
  USERD pages into an 8 GiB VIRTIO shared-memory region through `SHMEM_MAP` (patch 0001),
  so subsequent submissions are guest memory writes and the doorbell is never forwarded
  ([ARCHITECTURE.md](../ARCHITECTURE.md)).

## Depth

- [caraxes/doorbell-token-pool.md](../caraxes/doorbell-token-pool.md): tokens, KVM limits, the measurement.
- [caraxes/doorbell-poll.md](../caraxes/doorbell-poll.md): the reactor modes and host tuning.
- [research/windows-and-traps.md](../research/windows-and-traps.md): what still traps.
