<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Channels and submission

GPU work is submitted through channels: a ring of pushbuffer pointers (the GPFIFO), a
per-channel page holding the put and get pointers (USERD), and a doorbell register that
tells the hardware a channel has work. Leandro VFIO makes a submission cost no trap and no
message; Leandro VirtIO maps the submission registers into the guest for the same reason.
Which channels run on a host channel and which in the device is on
[software-executor](software-executor.md); a ring that moves while it runs is on
[channel-migration-and-deferred-api](channel-migration-and-deferred-api.md).

## The objects

| Object | Class on TU106 | Role |
|---|---|---|
| TSG | `KEPLER_CHANNEL_GROUP_A` 0xA06C | a time-slice group: channels that share a context and a runlist slot; `NVA06C_CTRL_CMD_GPFIFO_SCHEDULE`, `PREEMPT`, `SET_TIMESLICE` |
| channel | `TURING_CHANNEL_GPFIFO_A` 0xC46F (GB206: `BLACKWELL_CHANNEL_GPFIFO_B` 0xCA6F) | GPFIFO ring, USERD, an instance block, an error notifier; `NVA06F_CTRL_CMD_GPFIFO_SCHEDULE`, `BIND` |
| engine objects | `TURING_COMPUTE_A` 0xC5C0, `TURING_A` 0xC597 (3D), `TURING_DMA_COPY_A` 0xC5B5 | set on a subchannel with `SET_OBJECT` in the method stream |
| deferred API | `NV50_DEFERRED_API_CLASS` 0x5080 | Windows: controls run at a software method in the channel's stream |
| UVM SW | `GP100_UVM_SW` | a software class on subchannel 5; such channels stay in the device |

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
- The Windows KMD computes its tokens in its own RM, caches them, and gives most channels
  no error notifier ([vfio/doorbell-token-pool.md](../vfio/doorbell-token-pool.md) (g)).

## Zero-trap submission in Leandro VFIO

- The BAR1 pool (32 MiB by default with `--host-exec`, `CARAXES_BAR1_POOL_MIB`) turns writes
  1 and 2 into plain memory writes ([bar1-vram-window](bar1-vram-window.md); the knobs and
  open switches named here are on [device-configuration](device-configuration.md)).
- The doorbell token pool (`CARAXES_DOORBELL_TOKENS=N`, an open switch, off by default; the
  Linux B and nouveau launchers set 64; at most 253): the device writes its own tokens into
  the notifier entry, from a namespace outside every RM encoding, one per channel, assigned
  at the channel's first doorbell, which traps. Each token is one KVM datamatch ioeventfd on
  the doorbell register, declared to the VMM through `VFIO_USER_DEVICE_GET_REGION_IO_FDS`
  (Cloud Hypervisor patches 0005 and 0006, [patches/README.md](../../patches/README.md)).
- Guest-token ioeventfds, automatic with the boot model: one datamatch per guest token of the
  busiest runlists (240 values on TU106: runlists 0 and 1 interleaved, then 7 (NVENC), 2 and
  3; only 0, 1 and 7 beside a pool of 64). They cover drivers that never read the pool token:
  the Windows KMD, the NVENC library. Pool tokens (bits 31:23) and guest tokens never collide.
- A ring is then one KVM exit ending in an eventfd: 1.24 to 1.43 us for slots 0 to 63; KVM
  scans datamatch entries at about 25 ns each, so a value outside the set pays the whole scan
  and then the trap, 36.8 us ([research: windows-and-traps](../research/windows-and-traps.md)).
- The doorbell reactor (`CARAXES_DOORBELL_POLL`: `hybrid` by default with `--host-exec`,
  `spin`, `off`) polls the eventfds instead of sleeping in `epoll_wait`; hybrid spins 200 us
  after work, then blocks
  ([research: linux-graphics-optimization](../research/linux-graphics-optimization-2026-09-27.md)).
  The eventfd names the channel, so the device reads no GPPut.
- For a mirror channel the device waits for the root's pending shadow jobs, flushes the
  write-combined window and rings the host doorbell: p50 2.4 us from the ring to the host
  doorbell on Linux B ([research: vkmark-frame](../research/vkmark-frame-2026-09-28.md)). A
  ring whose root still has jobs queued is held and rung by the completer thread without the
  device lock ([vfio/execution.md](../vfio/execution.md) §9.1).
- A doorbell that still traps is posted: the vfio-user loop queues it without the device lock
  and answers at once; every later guest access rings the queue first, as a PCIe read does
  not pass a posted write (execution.md §9.9). Always on since 2026-09-30.
- Still trapped: the first doorbell of each pool channel, nvidia-uvm's rings, any value
  outside the declared set.

## Measured

| Measure | Value | Record |
|---|---|---|
| traps per submission, copy, event and synchronise (the shared-memory windows on) | 0 net of background; PPO submits 28000 times per second | [research: performance-facts](../research/performance-facts.md) |
| short kernel launch, native / Linux B / Windows with guest tokens | 6 / 13 to 20 / 28.6 us median (C48, a VM booting beside it) | [research: windows-findings](../research/windows-findings.md) P32, step 4 |
| Windows, trapped ring to its host doorbell, p50 | 7.0 us | windows-findings P38 |
| a launch after 30 ms idle, guest / native | 680 us / 155 to 177 us, no trap in its window | [vfio/optimization-backlog.md](../vfio/optimization-backlog.md) |
| reactor CPU, hybrid | 3.1 % of a core over a CUDA VM's life; spin takes a whole core | [vfio/doorbell-poll.md](../vfio/doorbell-poll.md) |

- Windows' guest-token run (C48): 9143 guest writes in 3169 eventfd wakeups; queued launch
  65.6 us, launch plus synchronize 104.8 us. The tail (p90 about 900 us) is the interrupt path.

## In Leandro VirtIO

- Channel and mapping setup use the forwarded control path; the backend maps the GPFIFO and
  USERD pages into an 8 GiB VIRTIO shared-memory region through `SHMEM_MAP` (patch 0001),
  so subsequent submissions are guest memory writes and the doorbell is never forwarded
  ([architecture.md](../virtio/architecture.md)).

## Depth

- [vfio/doorbell-token-pool.md](../vfio/doorbell-token-pool.md): tokens, KVM limits,
  guest tokens (g), the measurement.
- [vfio/doorbell-poll.md](../vfio/doorbell-poll.md): the reactor modes and host tuning.
- [vfio/execution.md](../vfio/execution.md) §9: the held ring, the posted doorbell.
- [research/windows-and-traps.md](../research/windows-and-traps.md): what still traps.
