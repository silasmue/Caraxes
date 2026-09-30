<!-- SPDX-License-Identifier: MIT -->
# Doorbell token pool: a submission with no trap

Written 2026-09-24 for the operator's goal "Submission auf 0 Traps". Spec first: every claim
names a file and line. Sources:

- NVIDIA open kernel modules 610.57.04: `vendor/open-gpu-kernel-modules` (`src/nvidia/...`,
  `src/common/...`, `kernel-open/nvidia-uvm/...`).
- vfio-user specification: `vendor/qemu-vfio-user/vfio-user.rst` (QEMU v11.1.1, sha256
  `cd05c157...`, patches/README.md "Specifications").
- Linux 7.2.4 (the host): `/usr/include/linux/kvm.h` and the build headers
  `/usr/lib/modules/7.2.4-arch1-2/build/include/{linux/kvm_host.h,net/scm.h}`. The KVM sources
  (`virt/kvm/eventfd.c`, `virt/kvm/kvm_main.c`) are not on this host and the rules forbid a
  download; what they decide was measured instead with `caraxes/tools/kvm-ioeventfd-probe.c`
  on the host's own KVM ([probe output](../../caraxes/runs/doorbell-tokens-2026-09-24/kvm-ioeventfd-probe.txt)).
- libvfio-user at `803924493a7c` (`vendor/libvfio-user`), reference server only.
- Cloud Hypervisor v53.0 with patches 0001-0005 in `/mnt/vmstore/caraxes/ch-0005`, with 0006 in
  `/mnt/vmstore/caraxes/ch-0006` (branch `patch0006`); the vendored crate `vendor/vfio-user-rs`.

## Summary

1. A submission of a CUDA channel is four guest writes: the GPFIFO entry (two dwords) and USERD
   GPPut through BAR1, then the work-submit token to the usermode doorbell in BAR0. P21's
   passthrough area makes the three BAR1 writes plain VRAM writes; the doorbell is the fourth.
2. KVM can match an ioeventfd on the written value (`KVM_IOEVENTFD_FLAG_DATAMATCH`), and
   vfio-user can hand the client one sub-region and one eventfd per value. With one eventfd per
   token, the eventfd names the channel, so the device needs no GPPut and no read at all.
3. The guest's tokens are the guest RM's (`runlist << 16 | ChID`), and libcuda rings what it
   reads from the channel's notifier entry. The device writes its own tokens there, as the host
   RM already does for mirrored channels. So it chooses the token namespace: a pool of N
   tokens outside every `NV_CTRL_VF_DOORBELL` encoding, declared at start, one per channel, handed
   out at the channel's first doorbell. That first doorbell traps and carries the guest's token.
4. The limits: one reply carries at most 253 descriptors (`SCM_MAX_FD`), so N <= 253. KVM
   takes 4096 datamatch entries at one address without complaint, but scans them in
   registration order at about 25 ns each. Unmatched writes pay the whole scan, so the default
   is N = 64, assigned lowest slot first.
5. With P21 and the pool, every submission after a channel's first has no exit to the VMM and
   no vfio-user message: one KVM exit that ends in an eventfd, the device woken by epoll.

## (a) Work-submit tokens on Turing in GSP-client mode

**How a token is made.** `kfifoGenerateWorkSubmitTokenHal_TU102`
(`src/nvidia/src/kernel/gpu/fifo/arch/turing/kernel_fifo_tu102.c:75-130`) builds the token
from the channel's runlist and ChID: `RUNLIST_ID` and `VECTOR` fields of `NV_CTRL_VF_DOORBELL`
(`:122-123`). Those fields are bits 22:16 and 11:0 (`src/common/inc/swref/published/turing/tu102/dev_ctrl.h:36-37`).
Bits 31:23 and 15:12 are never set in a token of any RM. The guest writes the token to
`NVC361_NOTIFY_CHANNEL_PENDING` (`src/common/sdk/nvidia/inc/class/clc361.h:33`, BAR0 0xbb0090 on
TU106, the chip's `registers.usermode_doorbell`).

**Who answers `NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN`.**
- The control is 0xc36f0108 (`src/common/sdk/nvidia/inc/ctrl/ctrlc36f.h:60-82`). Its comment:
  "a notification at index NV_CHANNELGPFIFO_NOTIFICATION_TYPE_WORK_SUBMIT_TOKEN will be sent with
  the work submit token value".
- It is exported with flags 0x10008 (`src/nvidia/generated/g_kernel_channel_nvoc.c:727-741`):
  `RMCTRL_FLAGS_NON_PRIVILEGED` (0x8) and `RMCTRL_FLAGS_GSP_PLUGIN_FOR_VGPU_GSP` (0x10000)
  (`src/nvidia/inc/kernel/rmapi/control.h:205`, `:287`). `RMCTRL_FLAGS_ROUTE_TO_PHYSICAL`
  (0x40, `:230`) is not set, so the guest's kernel RM runs it itself and GSP-RM (the device)
  never sees it.
- `kchannelCtrlCmdGpfifoGetWorkSubmitToken_IMPL`
  (`src/nvidia/src/kernel/gpu/fifo/kernel_channel.c:3225-3287`) sends an RPC only under
  `IS_VIRTUAL` (`:3236-3265`), which a GSP client is not. It generates the token (`:3276-3283`)
  and calls `kchannelNotifyWorkSubmitToken` (`:3286`).

**Where the token lands.**
- `kchannelNotifyWorkSubmitToken_IMPL` (`kernel_channel.c:4015-4030`) writes it through
  `kchannelUpdateNotifierMem` (`:1776-1860`) into the channel's error-context memory
  (`pErrContextMemDesc`), entry `notifyIndex[NV_CHANNELGPFIFO_NOTIFICATION_TYPE_WORK_SUBMIT_TOKEN]`.
- That index starts at `NV_CHANNELGPFIFO_NOTIFICATION_TYPE_WORK_SUBMIT_TOKEN` = 1
  (`kernel_channel.c:499-502`, `src/common/sdk/nvidia/inc/nvos.h:2855`).
- The entry is an `NvNotification` of 16 bytes: timeStamp 0-7, info32 8-11, info16 12-13, status
  14-15 (`src/common/sdk/nvidia/inc/nvgputypes.h:57-64`). The token is info32, at notifier + 0x18.
- The index can be moved with `NVC36F_CTRL_CMD_GPFIFO_SET_WORK_SUBMIT_TOKEN_NOTIF_INDEX`
  (`ctrlc36f.h:104-125`, `kernel_channel.c:3290-3331`). That control also runs in the guest
  kernel RM, so the device cannot see a moved index; rule (e) 4 covers it.

**Where the device learns the notifier.**
- In GSP-client mode the kernel RM sends the channel allocation to GSP-RM with
  `errorNotifierMem.base` = the physical address of `pErrContextMemDesc` + `errorContextOffset`
  (`kernel_channel.c:572-591`). The device records it per channel (`channel.rs` `record_alloc`,
  field `error_notifier`).
- `kchannelUpdateNotifierMem` indexes from the start of `pErrContextMemDesc` without
  `errorContextOffset` (`kernel_channel.c:1829`, `:1848`). For a non-zero offset the device's
  address would miss the entry. Rule (e) 4 catches that too.
- In the traces, CUDA's channels have a sysmem notifier, for example
  `errorNotifier Some(Sys(140731116843008))` (`caraxes/runs/mapping-2026-09-24/d1-isolation/gate-cuda-2026-09-24/device/*.trace`).
  nvidia-uvm's own channels have `errorNotifier None`.

**When libcuda reads the entry.** libcuda is closed. The evidence is run d-ce-token
(experiments.md:984, execution.md:140-145, `mirror.rs:736-748`):
- a mirror's `GET_WORK_SUBMIT_TOKEN` on the host channel made the host RM write the *host*
  token into the guest's own notifier page (the host channel's error notifier is that page);
- from the second submission on, the guest rang the host token (`token 0x8003a chid 58`);
- so libcuda reads entry 1 again after its first submission.

The pool needs exactly that: a read after the device's write. Whether libcuda reads before
every submission or caches after a change is not known. The verification counts the rings
that come back with the old token after a hand-out (`own_token_after_handout`).

**nvidia-uvm does not read the entry.** It rings `channel_info.workSubmissionToken`
(`kernel-open/nvidia-uvm/uvm_turing_host.c:235`), taken once from the channel allocation
(`src/nvidia/src/kernel/rmapi/nv_gpu_ops.c:5728`, `:6330`). Its channels keep their own tokens
and trap, with or without the pool.

**The device already owns this namespace.**
- For a mirrored channel the guest rings a value the device did not choose (the host token), and
  `Mirror::chid_of_host_token` (`mirror.rs:744-749`) maps it back.
- Any 32-bit value written to the emulated register reaches the device, trapped or through an
  eventfd. So the device can hand out any token whose value no RM makes.
- Pool tokens are `0xCA70_0000 + slot` (`tokens.rs:31`). Bit 31 is set, and no
  `NV_CTRL_VF_DOORBELL` encoding sets it (test `tokens::tests::pool_tokens_are_never_doorbell_encodings`).

## (b) KVM: datamatch, limits, duplicates, cost

**The call.**
- `struct kvm_ioeventfd` has datamatch, addr, len ("1, 2, 4, or 8 bytes; or 0 to ignore
  length"), fd and flags (`/usr/include/linux/kvm.h:647-654`).
- `KVM_IOEVENTFD_FLAG_DATAMATCH` is bit 0 (`:639`); `KVM_IOEVENTFD` is `:1287`.
- With DATAMATCH, KVM signals the eventfd only for a write of `len` bytes at `addr` whose value
  equals datamatch. The probe's eventfd counters equal the matched writes exactly (probe §3).

**How many.**
- `NR_IOBUS_DEVS` is 1000 (`kvm_host.h:205`), but `struct kvm_io_bus` counts ioeventfds apart
  (`ioeventfd_count`, `:209`).
- The limit check in `kvm_io_bus_register_dev` (`virt/kvm/kvm_main.c`, not on this host)
  leaves ioeventfds out. Measured: 4096 datamatch ioeventfds at one address, all registered, all
  deassigned (probe §1).
- So the roadmap's "KVM holds about a thousand per VM" (roadmap.md:58) is not the limit for
  ioeventfds. What bounds them is the process's descriptors and the cost below.
- Registration copies the bus array: 1.6 us each for the first 16, 33.6 us each at 4096.

**Duplicates and overlaps** (probe §2; the rule is `ioeventfd_check_collision` in
`virt/kvm/eventfd.c`):

| Case at one address | Result |
|---|---|
| same len and same datamatch twice | `EEXIST` |
| wildcard (no datamatch) where datamatch entries of that len exist, or the reverse | `EEXIST` |
| len 0 (any size) where a len 4 entry exists | `EEXIST` |
| len 8 datamatch next to len 4 datamatch | accepted (a different ioeventfd) |
| deassign with a datamatch or fd that was not assigned | `ENOENT` |

So a register holds either one wildcard (patch 0005's `CARAXES_DOORBELL_IOEVENTFD=1`) or a set
of distinct datamatch values (the pool), never both. The device refuses to start with both
(`main.rs:296-301`).

**Cost on the exit path** (probe §3: a guest loop of 32-bit writes to one MMIO address on this
host, AMD, wall clock, median of 3; the host was running other VMs):

| Registered at the address | Written value | ns per write |
|---|---|---|
| none | (exit to userspace, the trapped case) | 4842 |
| 1 | matches | 1244 |
| 64 | matches the first / the last registered | 1289 / 2717 |
| 253 | matches the first / the last registered | 1426 / 7200 |
| 1000 | matches the first / the last registered | 2085 / 27224 |
| 64 / 253 / 1000 | matches none (exits to userspace) | 6288 / 10847 / about 32000 |

- KVM finds the first entry of the address by binary search, then calls each entry of that
  address in registration order until one accepts the value.
- Each entry passed costs about 25 ns. The position of the matching token matters; the total
  number of entries matters only to values that match none.
- The pool therefore registers slot 0 first and hands out the lowest free slot
  (`TokenPool::assign`, `tokens.rs:166`), and keeps N small by default.
- On this host every MMIO write is a nested page fault plus instruction emulation, even with an
  ioeventfd. The ioeventfd removes the trip to the VMM, not the KVM exit (see (f)).

## (c) vfio-user: descriptors per message, several sub-regions at one offset

**What the specification says.**
- `max_msg_fds` is the "Maximum number of file descriptors that can be received by the sender in
  one message. Optional. If not specified then the receiver must assume a value of 1"
  (`vfio-user.rst:515-519`, `VFIO_USER_VERSION` capabilities).
- Each side announces what it can receive, and the other must not send more. The server's reply
  capabilities "must be a subset of those proposed" (`:476`).
- The reply to `VFIO_USER_DEVICE_GET_REGION_IO_FDS` carries "at least one file descriptor in the
  ancillary data. Note that more than one sub-region may share the same file descriptor"
  (`:1025-1027`). Sharing is allowed, not required, so one descriptor per sub-region is in the
  specification.
- Each sub-region has offset, size, fd_index, type, flags (any of `KVM_IOEVENTFD_FLAG_DATAMATCH`,
  `_PIO`, `_VIRTIO_CCW_NOTIFY`) and datamatch (`:1036-1073`). Nothing restricts two sub-regions to
  different offsets, and "it is the client's responsibility to verify the requested values"
  (`:1029`).
- Several ioeventfd sub-regions at one offset with different datamatch values are therefore
  allowed.

**The limit under the specification.** A reply is one message. Linux carries at most
`SCM_MAX_FD` = 253 descriptors in one `SCM_RIGHTS` message (`include/net/scm.h:18`), so no
client can usefully announce more. That bounds N at 253 for the one doorbell register.

**libvfio-user** (reference server):
- `vfu_create_ioeventfd` checks only the bounds of a sub-region (`lib/libvfio-user.c:447-487`),
  so it too allows several at one offset.
- Its reply layout differs from the specification (48-byte entries, patches/README.md "Deviations
  of the reference implementations"), which our client refuses.

**What Cloud Hypervisor with 0005 did with such a reply.**
- `check_sub_region` (`/mnt/vmstore/caraxes/ch-0006/pci/src/vfio_user.rs:139-222`) checks each
  entry on its own.
- The only overlap check is against mmap()ed areas and the MSI-X table and PBA (`:463-467`),
  never between sub-regions.
- `fd_index` must be below the descriptors received (`:197`).
- A duplicate reaches KVM and comes back as `EEXIST`, which is logged, and that entry stays
  trapped (`:490-503`).
- So CH itself would accept a pool reply. What stopped it was the crate beside it:
  - the client left `max_msg_fds` out, so every server had to assume 1;
  - the client received at most 16 descriptors (`MAX_RECEIVED_FDS`). vmm-sys-util closes the
    descriptors of a truncated message and fails the receive (`recv_with_fds`, `MSG_CTRUNC`), and
    CH then fails device creation;
  - the crate's server obeyed the client's 1 and left out every sub-region after the first
    descriptor (patches/README.md, "Use by the Caraxes device").

**What changes.**
- The vendored crate, [patches/vfio-user-rs/0002](../../patches/vfio-user-rs/0002-vfio-user-one-eventfd-per-ioeventfd-sub-region.patch)
  (`vendor/vfio-user-rs/src/lib.rs`):
  - the client announces `max_msg_fds` = `CLIENT_MAX_MSG_FDS` = `SCM_MAX_FD` = 253 (`:301-314`,
    `:660-668`) and receives that many with a reply;
  - the server never sends more than `SCM_MAX_FD` whatever the client announced (`:1542-1548`);
  - `Server::add_ioeventfd` requires a width of 1, 2, 4 or 8 with a datamatch that fits, and
    refuses a sub-region that KVM would refuse as a duplicate of an earlier one
    (`ioeventfds_collide`, `:1412`, `:1498-1535`).
  - Tests: 14 pass, 4 of them new.
- Cloud Hypervisor, [patches/0006](../../patches/0006-vfio-user-region-io-fds-summary.patch):
  - one INFO line per region after registration: registered of offered, descriptors received,
    datamatch range (`region_io_fds_summary`, `vfio_user.rs:228`);
  - a test with a hand-built 253-entry datamatch reply in the specification's bytes through the
    crate's parser and `check_sub_region`.
  - No check changes: none was needed.

**0006, not an amendment of 0005.**
- 0005 is verified and recorded as it stands (patches/README.md "Verification (2026-09-24)").
- The coordinator's P21 runs use its binary (`/mnt/vmstore/caraxes/vm3/cloud-hypervisor-0005`).
- The functional change is in the crate, which 0005 builds against by path, so 0006 is only the
  log line and a test.
- Neither patch is upstreamed. Before upstreaming, 0006 folds into 0005, and the crate patches
  0001 and 0002 into one rust-vmm/vfio change: an upstream reviewer should see one feature, not
  its history.

## (d) The device side

**Switch.** `CARAXES_DOORBELL_TOKENS=N` (`tokens.rs:28`, `parse` `:57`).
- `N` may be 1 to 253, or `on` for the default of 64.
- 254 and more are refused at start with the reason (`SCM_MAX_FD`).
- The operator's 256 is above that limit. The default of 64 follows the cost table of (b):
  - a value that matches none (a channel's first ring, nvidia-uvm's rings) pays about 25 ns per
    entry, so 1.4 us at 64 against 6 us at 253;
  - a CUDA context uses about a dozen channels (GR chids 2-9 plus the CE channels in the traces).
- The pool needs `--boot-model`. It excludes `CARAXES_DOORBELL_IOEVENTFD=1` (KVM `EEXIST`, (b))
  and combines with `CARAXES_BAR1_PASSTHROUGH=1`: the refusal added after run
  B-on-with-ioeventfd-hung now applies to the token-less path only (`main.rs:296-316`).

**Declared at start.**
- `doorbell_token_sub_regions` (`device.rs:124`) makes, for each slot i, one eventfd and one
  sub-region of BAR0: offset = the doorbell, size 4, flags `IOEVENTFD_FLAG_DATAMATCH`, datamatch
  `0xCA70_0000 + i`.
- `main.rs:514-535` adds them in slot order, so KVM registers slot 0 first (CH registers in reply
  order, `vfio_user.rs:469-505`).
- The client asks once per BAR when it maps the device and never again (patches/README.md, "What
  the client does" 1). So the whole pool is declared before the guest runs; nothing is added
  later.

**Assignment at the first doorbell.**
- A channel's first ring traps: the guest writes the token its RM made, which no ioeventfd
  matches. `Dispatcher::doorbell` (`ctrl.rs:178`) rings the channel as before, then calls
  `hand_out_token` (`ctrl.rs:229-320`).
- `hand_out_token` reads info32 of the channel's work-submit-token entry (notifier + 0x18).
  Only if it holds the token just rung, or the mirror's host token for that channel, does it take
  the lowest free slot and write the pool token there.
- For a mirrored channel the host RM writes the host token into that entry while the mirror is
  built inside the same doorbell (`Mirror::ensure_channel` asks for it,
  `mirror.rs:1333-1337`). The device's write comes after it.
- The trapped write returns to the guest only after the device answered, so the guest's next
  read of the entry sees the pool token.
- Every later ring of that channel writes the pool token, matches its ioeventfd and never reaches
  the VMM.
- Per channel exactly one ring traps: the first. nvidia-uvm's channels and channels without a
  notifier trap on every ring, as today. Both are counted.

**Ring by eventfd.**
- One thread and one epoll set over the N eventfds (`spawn_token_reader`, `device.rs:140-206`).
  Each wakeup reads every signalled eventfd, then takes the device lock once.
- For each token it calls `BootModel::doorbell_token`, which calls `Dispatcher::doorbell_pool`
  (`ctrl.rs:201-227`). The pool names the channel, which is rung through the same
  `doorbell_chid` as a trapped doorbell: software executor, shadow GR channel or host mirror.
- A pool token that arrives trapped (its ioeventfd not registered, for example a client that
  left it out) takes the same path.

**Release.**
- A channel's token is freed when the guest frees the channel or its client (`Registry::record_free`,
  `channel.rs:625-632`), and when a ChID is allocated again without a free the device saw
  (`record_alloc`, `channel.rs:597-602`).
- A function-level reset keeps the pool's size and counters and drops every assignment
  (`BootModel::reset`, `boot.rs:410`).
- The eventfds and KVM's registrations live as long as the device process. Cloud Hypervisor
  deassigns them when the device is removed (0005, `unmap_region_io_fds`, `vfio_user.rs:524-532`).

**Exit summary.** One line, `doorbell tokens: ...` (`TokenPool::summary`), with:
- handed out, written again, assigned and peak;
- eventfd wakeups and writes, pool tokens that trapped;
- own-token rings after a hand-out, stale rings;
- channels left on their own token (no notifier, mismatch, exhausted), and the first 16 findings.

## (e) Collisions, stale tokens, fail-closed rules

1. **Disjoint namespaces.** A pool token has bit 31 set; guest and host tokens are below
   1 << 23 (`dev_ctrl.h:36-37`). A pool token can never be read as a ChID, and a guest or host
   token can never select a pool slot. The software executor is always rung with the channel's
   own ChID, never with a pool value (`ctrl.rs:223-225`).
2. **Stale token.** A pool token that no channel holds (freed, reset, or never handed out) rings
   nothing and is counted. A doorbell only makes the engine look at GPPut, so dropping it loses
   nothing a live channel relies on.
3. **Reuse.** A freed slot goes back to the next new channel (lowest first). A late ring of a
   freed channel's token then rings another live channel of the same guest. That is a spurious
   doorbell, which the hardware tolerates too: a channel without new entries runs nothing
   (GPGet == GPPut). Tokens are per device, so never another VM's. The host doorbell is written
   only with the host token of this device's own mirror channel.
4. **Write-back only over a proven value.** The entry is overwritten only when it holds the token
   just rung, or the mirror's host token. The following leave the channel on its own token,
   trapped and correct, and are counted as a mismatch:
   - a moved notifier index (SET_WORK_SUBMIT_TOKEN_NOTIF_INDEX) whose entry the device cannot
     find. Since 2026-09-29 the first 64 entries of the notifier's page are scanned for the RM's
     own token signature (`kchannelNotifyWorkSubmitToken`: info32 the token, info16 0, status
     0xFFFF), and the pool token is written there as well; counted as "written into the token
     entry at another notifier index" (SotTR run E: 43273 rings kept their own token because
     entry 1 held 0, 57274 rang their own token after a hand-out to entry 1; each trapped).
     The scan is exact: only the ringing channel's own notifier page (never past it), only
     its own values (the token it rang, its mirror's host token), and a value only if no
     other channel a ring of it rings keeps its notifier on the same page (a guest token that
     is also another channel's host token could be that channel's entry; tokens.rs
     `own_on_page`). The same values decide the default entry;
   - a non-zero `errorContextOffset` ((a));
   - a notifier the device cannot read or write.
5. **Nothing depends on registration.** A pool token whose ioeventfd KVM or the client did not
   register reaches the device as a trapped write and goes the same way (rule 2 applies). Only
   speed depends on registration, never correctness.
6. **Re-hand-out.** If the entry is overwritten later with the channel's own or host token (a
   rebuilt mirror), the next trapped ring writes the channel's pool token back. If the entry still
   holds the pool token, the ring came from a writer that does not read it (nvidia-uvm) and is
   only counted.
7. **Start-time refusals.** More than 253 tokens; the pool together with the token-less
   ioeventfd; the pool without the boot model; the server's own check refuses sub-regions that
   KVM would reject (`add_ioeventfd`).
8. **Reader failure.** If `epoll_wait` or an eventfd read fails, the reader logs it and ends.
   Rings through eventfds then go unanswered; the guest would time out. That needs an OS failure
   on an eventfd and is logged in gpu.log.
9. **Guest ChID before host token (2026-09-26).** Guest and host ChIDs overlap, so a trapped
   value can be a new channel's guest token and an older mirror's host token at once
   (runs/nvenc-2026-09-25/vm4-615-cuda-vulkan/NOTES.md, "cause found").
   - `ctrl.rs` `route_token` rings the guest channel named by bits 11:0 first. The host token
     is only a fallback for a value that names no guest channel. On a collision the host
     token's owner is rung as well, since a spurious doorbell is harmless (rule 3).
   - Each collision is noted in the trace (`[collision avoided]`) and counted in the
     `doorbell routing:` exit line.
10. **No raw host token for the guest.** When a doorbell builds a channel's host mirror, the host
   RM writes the host token into the guest's notifier entry. `pin_pool_token` replaces it with
   the channel's pool token in the same doorbell, which covers an eventfd ring that does not
   block the guest. Rule 4 still applies: only an entry that holds the proven host token is
   overwritten. Without a pool nothing changes.

## (f) Expected cost and measurement plan

A submission with P21 and the pool, after the channel's first:

| Access | Today (all trapped) | P21 | P21 + token pool |
|---|---|---|---|
| GPFIFO entry, low and high dword (BAR1) | 2 traps | VRAM through the area, no exit | same |
| USERD GPPut (BAR1) | 1 trap | VRAM through the area, no exit | same |
| doorbell (BAR0 0xbb0090) | 1 trap (vfio-user write, ~25-28 us) | 1 trap | 1 KVM exit ending in the token's eventfd; no VMM, no vfio-user message |
| device wakeup | on the trap | on the trap | epoll on the eventfd; the channel from the pool, no USERD read |

- "Zero traps" means no access reaches the VMM or the vfio-user socket.
- One KVM exit per doorbell remains (probe: 1.24-1.43 us at slots 0-63 on the host, against
  4.8 us for a bare exit to userspace and ~28 us for a trapped vfio-user write, benchmark.md).
  Removing that exit is P24 (a polled doorbell page), not this item.
- The first submission of each channel still traps once; nvidia-uvm's rings keep trapping.

**Measurement plan.**
1. Pure emulation (vm1, done here, below):
   - Cloud Hypervisor logs N datamatch ioeventfds;
   - the driver loads;
   - bench-trap writes of pool tokens do not trap and count on their eventfds, and other values
     still trap;
   - a CUDA program's channels get pool tokens and ring through eventfds;
   - hot-remove is clean.
2. On the GPU with P21 (the coordinator's; command in "GPU measurement"):
   - A: P21 with the trapped doorbell, the B-on configuration of `caraxes/runs/p21-bar1-2026-09-24`;
   - C: P21 with the pool. Short-kernel ms/launch, rlprobe stage 4, bench-trap on the doorbell
     with a pool token.
   - Expected in C: the device trace shows only the first doorbell of each channel as a `W bar0
     0x00bb0090` line, and "trapped in first MiB" stays 0.
   - From P21's 0.100 ms/launch, about 25 us less per launch (one trap of the remaining one).

## Verification (pure emulation, vm1, 2026-09-24 13:05-13:09)

Run and logs: [caraxes/runs/doorbell-tokens-2026-09-24](../../caraxes/runs/doorbell-tokens-2026-09-24)
(`commands.txt`, `run.sh`). Setup: Cloud Hypervisor v53.0-6-g88e92aded with -v, the device
with `CARAXES_DOORBELL_TOKENS=253`, `CARAXES_HOST_RM=1 CARAXES_BOOT_MODEL=1`, no VRAM
reservation, no host execution, no DMA registration.

| Check | Result |
|---|---|
| handshake | the server receives `max_msg_fds: 253` from the client |
| registration | ch.log: 253 `ioeventfd at 0xe6bb0090, len 4, datamatch Some(..)` lines; `253 of 253 sub-region(s) registered as ioeventfds, 253 descriptor(s) received, 253 with datamatch in [0xca700000, 0xca7000fc]` |
| driver | RmInitAdapter succeeded, modprobe rc=0 |
| bench-trap, 20000 writes to 0xbb0090, p50 | pool slot 0: 4.1 us; slot 252: 11.2 us; 0x12345 (outside the pool, trapped): 36.1 us; trapped scratch register 0x1400: 25.5 us |
| where those writes went | 40000 pool-token writes counted by their eventfds, none trapped; all 20000 trapped doorbell writes carry 0x12345 |
| CUDA (nvprobe 2: cuInit, context, cuMemAlloc, H2D/D2H compare) | passes. 20 channels, each trapped exactly once with its guest RM token (`0x2`..`0x30015`), each got a pool token in its sysmem notifier entry; all later rings (196) came through eventfds (chid 2 and chid 10); 0 rings with an old token after a hand-out, 0 mismatches |
| exit summary | 20 handed out, 0 assigned at exit (freed with the channels), 0 pool tokens trapped |
| hot-remove | 204, no `Could not unregister`, the server exits |
| re-add (new server, 64 tokens) | 64 registered at the same address without `EEXIST` (so gpu0's 253 were deassigned); driver loads; slot 0 3.8 us, slot 63 5.6 us |

Readings:
- libcuda reads the work-submit-token entry again before its next submission. No ring came back
  with the old token, so the hand-out at the first doorbell is enough, as (a) inferred from
  d-ce-token.
- The scan cost is visible from the guest. Slot 252 costs 7 us more than slot 0, which matches
  the probe's ~25 ns per entry.
- A value outside the pool pays 36.1 us, against 25.5 us for a trapped register with no
  ioeventfd at its address. That is the 253-entry scan plus the doorbell dispatch. This is the
  cost of a first ring and of nvidia-uvm's rings, and the reason for the default of 64.

## GPU measurement (the coordinator's; nothing run here)

This is the P21 series of `caraxes/runs/p21-bar1-2026-09-24/run.sh`, on vm1 with the 1 GiB
profile at slot 4. It compares two configurations:
- A: P21 with the doorbell trapped (as B-on there);
- C: P21 with the pool.

Cloud Hypervisor must be the 0006 build, because the 0005 binary's crate announces no
`max_msg_fds`.

```sh
R=/home/silas/git/Caraxes; O=$R/caraxes/runs/doorbell-tokens-gpu-$(date +%F); mkdir -p "$O"
source "$R/caraxes/runs/doorbell-2026-09-24/env-vm1.sh"
export CARAXES_SERVER=/mnt/vmstore/caraxes/vm1/vfio-user-nvgpu-tokens   # or $R/target/release/vfio-user-nvgpu after the merge
export LEA_CH=/mnt/vmstore/caraxes/vm1/ch-0006-v.sh                     # v53.0-6-g88e92aded with -v
export CARAXES_BAR1_PASSTHROUGH=1; unset CARAXES_DOORBELL_IOEVENTFD
for cfg in A-p21-trapped C-p21-tokens; do
  D=$O/$cfg; mkdir -p "$D"
  if [[ $cfg == C-* ]]; then export CARAXES_DOORBELL_TOKENS=64; else unset CARAXES_DOORBELL_TOKENS; fi
  t0=$(date '+%Y-%m-%d %H:%M:%S')
  "$R/caraxes/tools/bench-hostwatch.sh" start "$D"
  "$R/caraxes/tools/bench-run.sh" v2-restart > "$D/up.txt" 2>&1 || { tail "$D/up.txt"; "$R/caraxes/tools/bench-hostwatch.sh" stop "$D"; continue; }
  grep -iE 'bar1 passthrough|doorbell tokens|ioeventfd' /mnt/vmstore/caraxes/$VM/gpu.log /mnt/vmstore/caraxes/$VM/ch.log | cut -c1-240 > "$D/device-start.txt"
  "$R/caraxes/tools/bench-run.sh" series --targets v2 --reps 3 --metrics "short-auto rl" --out "$D" 2>&1 | cut -c1-200 | tail -4
  "$R/tools/caraxes-vm.sh" ssh 'mkdir -p ~/bench; cat > ~/bench/bench-trap.c' < "$R/caraxes/tools/bench-trap.c"
  "$R/tools/caraxes-vm.sh" ssh 'cd ~/bench && gcc -O2 -o bench-trap bench-trap.c &&
    sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource0 20000 0xbb0090 0 0x12345 &&
    sudo ./bench-trap /sys/bus/pci/devices/0000:00:05.0/resource0 20000 0xbb0090 0 0xca70003f' | tee "$D/trap.txt"
  "$R/caraxes/tools/bench-hostwatch.sh" stop "$D"
  "$R/tools/caraxes-vm.sh" down > "$D/down.txt" 2>&1
  grep -E 'bar1 passthrough|ERROR: bar1|doorbell' /mnt/vmstore/caraxes/$VM/gpu.log | cut -c1-600 > "$D/device-summary.txt"
  grep -c ' bar1 0x000[0-9a-f]' /mnt/vmstore/caraxes/$VM/gpu-trace.log > "$D/bar1-trapped-in-first-MiB.txt"
  grep 'W bar0 0x00bb0090' /mnt/vmstore/caraxes/$VM/gpu-trace.log | grep -vc ' 0x12345 ' > "$D/doorbell-trapped-except-bench.txt"
  journalctl -k --since "$t0" --no-pager -g 'Xid' > "$D/host-xid.txt"
done
python3 "$R/caraxes/tools/bench-summary.py" "$O/A-p21-trapped/v2" "$O/C-p21-tokens/v2" > "$O/summary.md" 2>&1
```

**What to read.**
- `C-p21-tokens/device-start.txt`:
  - gpu.log `doorbell tokens: BAR0 0xbb0090, 4 bytes, 64 sub-region(s) ...`;
  - ch.log 64 `ioeventfd at ..., len 4, datamatch Some(...)` lines and `64 of 64 sub-region(s)
    registered as ioeventfds, 64 descriptor(s) received`.
- `device-summary.txt`: the `doorbell tokens:` line. Handed out should be about the number of
  CUDA channels; eventfd wakeups in the thousands; stale 20000 from the bench only; mismatch 0.
  Also `0 mismatched page(s) and 0 page-table hazard(s)` for P21.
- `doorbell-trapped-except-bench.txt`:
  - A: every doorbell of the series;
  - C: about one per channel (the first rings) plus nvidia-uvm's.
- `trap.txt`: 0x12345 traps (about 25-30 us). 0xca70003f, the last slot, is matched in KVM: a
  few microseconds, higher than slot 0 by 63 x 25 ns.
- `summary.md`: short-auto ms/launch, expected about 25 us below A's (0.100 ms in
  `p21-bar1-2026-09-24`); rlprobe stage 4; device and VMM CPU lower in C.
- `host-xid.txt` empty.

## Implementation

| File | What |
|---|---|
| `crates/vfio-user-nvgpu/src/tokens.rs` (new) | switch, token values, `TokenPool` (assign lowest free, release, lookup), counters, summary; unit tests |
| `crates/vfio-user-nvgpu/src/ctrl.rs` | `doorbell` routes pool tokens; `doorbell_pool`; `hand_out_token` (the notifier write-back, next to the mirror's host token it may overwrite) |
| `crates/vfio-user-nvgpu/src/channel.rs` | `Registry::tokens`; release on free and on ChID reuse; end-to-end dispatcher test |
| `crates/vfio-user-nvgpu/src/device.rs` | `doorbell_token_sub_regions`, `spawn_token_reader` (epoll), `Device::doorbell_tokens`; test |
| `crates/vfio-user-nvgpu/src/boot.rs` | `with_doorbell_tokens`, `doorbell_token`, reset carries the pool, exit summary |
| `crates/vfio-user-nvgpu/src/main.rs` | switch, refusals, declaration, reader; the P21 refusal limited to the token-less path |
| `vendor/vfio-user-rs/src/lib.rs`, `CHANGELOG.md` | client `max_msg_fds` 253, server cap and declaration rules; tests |
| `patches/vfio-user-rs/0002-...patch`, `patches/0006-...patch` | the crate change for rust-vmm/vfio, the Cloud Hypervisor change |
| `caraxes/tools/bench-trap.c` | optional fifth argument: a fixed write value |
| `caraxes/tools/kvm-ioeventfd-probe.c` (new) | the KVM measurement of (b) |

`mirror.rs` is unchanged: the host token it asks for is read through `Mirror::channel`, and the
write-back sits in the dispatcher, which sees both the guest's and the host's token. `shadow.rs`,
`hostmem.rs`, `fb.rs`, `ptcache.rs` and the display code are untouched.

Checks:
- `cargo test -p vfio-user-nvgpu`: 108 passed plus 2.
- `cargo clippy -p vfio-user-nvgpu --all-targets -- -D warnings -A clippy::unnecessary_cast -A
  clippy::field_reassign_with_default`: clean.
- The new code is rustfmt-clean. Files that were not rustfmt-clean before stay as they were.

## (g) Guest tokens as ioeventfds (2026-09-29, the Windows guest under Cloud Hypervisor)

- **Why the pool misses Windows.** The pool reaches a channel only through its work-submit-token
  notifier entry, which libcuda re-reads (section (a)). The Windows KMD computes its tokens in
  its own RM and caches them, and nearly all its channels have no error notifier (P36: 1 of 53;
  P37-arm2: 1355 rings "without notifier"). Under Cloud Hypervisor every Windows ring would
  still trap.
- **The spec's shape.** `VFIO_USER_DEVICE_GET_REGION_IO_FDS` is a client query; the spec has no
  server-initiated update (vfio-user.rst, the command's section). The set is therefore fixed
  when the client asks, at realize, before any channel exists: sub-regions cannot follow live
  channels.
- **The set.** A guest token is `runlist << 16 | chid` (RUNLIST_ID 22:16, VECTOR 11:0;
  kfifoGenerateWorkSubmitTokenHal_TU102). The device declares one datamatch sub-region per
  value, each with its own eventfd.
  - Automatic by default (`CARAXES_DOORBELL_GUEST_TOKENS` unset or `auto`, with the boot
    model), from the device-info table the device presents: the graphics runlist and the
    lowest async copy runlist interleaved per ChID first, then the video encoders', the other
    copy runlists and the decoders', 48 ChIDs each, as many runlists as fit the eventfds one
    reply carries beside the pool's and the re-arm and head eventfds. TU106: 0 and 1
    interleaved, then 7, 2, 3 (240 values); with the pool of 64: 0, 1, 7.
  - `off` disables it; `<hot>[,<runlist>...]:<chids>` (hot runlists joined by `+`) sets it for
    an A/B.
  - One eventfd per value: the eventfd names the written value, and the device rings it
    exactly as the trapped write of that value (ctrl.rs `doorbell`: guest ChID first, the host
    token as the fallback). No new routing, no scan.
  - Refused: a runlist without a host-driven engine in the chip's device-info table, more
    chids than the guest is told per runlist (`INTERNAL_FIFO_GET_NUM_CHANNELS`, 512), more
    than 253 eventfds together with the pool (`SCM_MAX_FD`), and the token-less wildcard
    (KVM: `EEXIST`, section (b)). Pool tokens (bits 31:23) and guest tokens never collide.
  - A value outside the set traps as before.
- **Cost.** KVM passes the entries of one address in registration order, ~25 ns each (section
  (b)): the busiest values go first. The set is fixed before any channel exists, so the order
  can only follow the runlists' expected use, not the ChIDs a driver will pick. The pool, when
  both are on, goes first (Linux's libcuda rings pool tokens).
- **The Windows ring census** (trace `doorbell` lines with the channel's engine; runlists from
  chips/tu106/device-info.toml: GR, CE0, CE1 on 0, CE2 1, CE3 2, CE4 3, NVENC 7):

  | Run | runlist 0 | 1 | 2 | 3 | 7 | highest ChID |
  |---|---|---|---|---|---|---|
  | P43 (QEMU, workload block) | 201984 | 6698 | 495 | 15 | 3655 | 37 |
  | C45 (CH, desktop, smoke, CUDA) | 5533 | 38214 | 205 | 10 | 29 | 35 |

  - Hot values: `0x1a` (GR chid 26, the CUDA probe, 201098 rings in P43), `0x1000f` (chid 15 on
    runlist 1, the KMD's paging CE, 38096 in C45), `0x70024` (NVENC chid 36).
  - The automatic set puts `0x1000f` at position 31, `0x1a` at 52, `0x70024` at 132; worst
    matched position 239 (~6 us of KVM scan, measured 7.2 us at position 253 in (b)); a
    trapped ring is an exit to the VMM (4.8 us in (b)) plus a vfio-user message and the device
    loop.
- **Status.** Built with a reactor test and parse tests; not run yet.
