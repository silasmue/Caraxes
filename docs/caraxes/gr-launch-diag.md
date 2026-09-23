<!-- SPDX-License-Identifier: MIT -->
# Silence after the first GR doorbell: diagnosis from user space

Written 2026-09-23, read-only (no VM, no GPU access, no crate change). Driver citations are
relative to `vendor/open-gpu-kernel-modules/` (610.57.04): `sdk/` = `src/common/sdk/nvidia/inc/`,
`gen/` = `src/nvidia/generated/`. `D/` = `crates/vfio-user-nvgpu/src/` (lines as of this
writing; another agent is editing the crate). `R/` = `caraxes/runs/e5b2-2026-09-23/`.
Vocabulary (set A/B/C, c1/c2/c1s) is that of [exec-formats.md](exec-formats.md) section 4.5 and
[execution.md](execution.md) section 6. Claims about nouveau gv100/tu102 are from memory
(the only nouveau on disk is a 4.16 tree without gv100/tu102) and are marked **[nouveau, from
memory]**.

## 1. What the d-gr-launch runs actually show

Five c1s runs (`CARAXES_MIRROR_GR_SHADOW=1`: GR channel on a host-managed shadow VAS with the
guest's set A fixed-mapped at the same VAs, `D/mirror.rs:316-563`, `D/shadow.rs:130-222`).

- **d-gr-launch4 is the silent run, and its cause is already in the trace.** Guest chid 2 is
  built, scheduled on runlist 0 with host token 0x3f, rung, and 5 ms later its USERD reads
  `GPGet 0 GPPut 0` (`R/d-gr-launch4/device-trace.txt:9322-9324`); the same for chids 3-5. The
  host RM clears the USERD it is given at channel allocation (execution.md section 3), so the
  guest's GPPut 1 was gone and the PBDMA had nothing to fetch. No Xid, no RC
  (`R/d-gr-launch4/host-xid.txt` holds only close messages). This is branch B2 of the tree below.
- **d-gr-launch5 has the fix** (save before alloc and restore after, `D/mirror.rs:360-376`,
  `:531-541`): `restored guest GPGet 0 / GPPut 1`, then 5 ms after the doorbell `GPGet 1
  GPPut 1` (`R/d-gr-launch5/device-trace.txt:6761-6763`). The physical PBDMA fetched the GP entry
  of every GR channel 2-9. The fetch wall is gone.
- **d-gr-launch5 then ends in a contained Xid 31** on host chid 0x3f (guest chid 2):
  `ENGINE GRAPHICS HUBCLIENT_FE faulted @ 0x100_04e00000 ... FAULT_PDE ACCESS_TYPE_VIRT_WRITE`
  (`R/d-gr-launch5/host-xid.txt`), RC event exceptType 0x1f = `ROBUST_CHANNEL_FIFO_ERROR_MMU_ERR_FLT`
  (`sdk/nverror.h:49`). The trace order is the finding: the guest's **second** GR doorbell is
  forwarded at 204452665 and only **afterwards** does the shadow re-sync map 770 new set-A pages
  (`R/d-gr-launch5/device-trace.txt:8484-8485`); the fault follows at 204920063 (`:8546`). The
  already-built path rings first and re-syncs second (`D/mirror.rs:322-331`). The guest had
  issued register invalidates at 204069460/204080341 without the re-sync finding anything new, so
  the new PTEs appeared between the last invalidate and the doorbell (a guest that fills a PTE
  from invalid to valid does not have to invalidate). **Fix: re-sync before ringing, on every
  doorbell of a shadow GR channel.**
- A second latent limit: `enumerate_leaves` stops at 16384 leaves (`D/shadow.rs:146`,
  `D/fb.rs:673-705`), walking VAs in ascending order, so when the guest has more present leaves
  the **highest** VAs (0x100_xxxx_xxxx is the top of the guest's layout) are silently not mirrored.
  The launch5 re-sync reported 16139 leaves after the set-B filter, close enough to the cap that
  the cap may have been hit (not provable from the trace: the pre-filter count is not logged).
  Log `leaves.len() == max` and raise or remove the cap.

So "silence" has had two different causes in this series: never fetched (launch4, GPPut 0), and,
in launch5, fetched with completion not observable until the fault. The tree below separates them
with one observation each.

## 2. Decision tree: silence after the first GR doorbell

Each node names the cheapest observation that decides it. All reads below are of memory the
device already maps (the reservation windows and guest RAM), or RM controls callable by the
device's unprivileged client unless marked.

**A. Did the doorbell reach a runlist the channel is on?** Observation: the token used. The
device rings exactly the token `NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN` returned
(`D/mirror.rs:503-526`, `:1245-1265`); RM composes it as runlist id plus ChID and refuses it
with `NV_ERR_INVALID_STATE` when the channel has no runlist yet (`src/nvidia/src/kernel/gpu/fifo/
arch/turing/kernel_fifo_tu102.c:111-121`). A token in hand therefore excludes "wrong runlist"
and "not on a runlist" by construction. Token runlist 0 = the GR runlist (launch4/5: 0x3f).
The one remaining failure is a guest that rings a token the device does not route (execution.md
section 6, host token read back from the notifier page); the device logs every routed doorbell.

**B. Did the PBDMA fetch the GP entry?** Observation: USERD `GPGet` (+0x88) against `GPPut`
(+0x8C), read a few ms after the doorbell (`sdk/class/clc46f.h:61-62`).
- B1. `GPGet == GPPut != 0`: fetched. Go to C.
- B2. `GPPut == 0` (or not the guest's value): the host RM cleared the USERD at allocation and the
  guest's put was never restored. Restore it, flush the BAR1 window, ring again (launch4).
- B3. `GPPut` correct, `GPGet` stuck below it: not fetched. Check, in this order: the channel was
  scheduled (`GPFIFO_SCHEDULE` returned NV_OK, `D/mirror.rs:497-502`); the WC window was flushed
  before the doorbell (execution.md section 2); the GPFIFO VA resolves in the shadow VAS (a bad VA
  would raise PBENTRY or an MMU fault, Xid 32/31, so this branch is silent only if the channel
  never became resident: the TSG is not being scheduled at all, see F).

**C. Did the PBDMA consume the pushbuffer segment?** Observation: USERD `Get`/`GetHi`
(+0x44/+0x60) and `TopLevelGet` (+0x58) (`sdk/class/clc46f.h:51-57`), compared to the end of the
segment the GP entry describes (GP entry format `clc46f.h:264-282`, decoded by `D/channel.rs`).
- C1. `Get` = segment end: every method left the PBDMA. Go to D.
- C2. `Get` inside the segment and not moving: the PBDMA is blocked on a method. The usual blockers
  are a host semaphore ACQUIRE whose condition never becomes true (read the acquire address and
  payload from the push and read that memory, it is guest memory) or a WFI / engine method waiting
  for GR that cannot accept methods (go to E). A method on a subchannel without an object, or an
  illegal method, is not silent: it raises a PBDMA or GR exception (Xid 32 / Xid 13,
  `sdk/nverror.h:37`, `:50`).

**D. Did the work complete?** Observation: the completion the guest waits for, read directly:
the semaphore payload at the release VA of the push (guest memory by identity, as the CE
diagnostic already does, experiments.md d-ce-diag), and the channel error notifier (section 4).
- D1. Payload written: the work finished and the silence is **delivery**. The GR0 non-stall
  event cannot be used as the proof (it fires for the host desktop's GR work too, execution.md
  section 7). Check that the guest's release requested an interrupt (the release/AWAKEN or
  `NON_STALL_INTERRUPT` method), and that the device forwarded the MSI for vector 4 after the
  payload landed.
- D2. Payload not written, `Get` at segment end: the methods are inside GR. Go to E.

**E. Is GR running this channel's context?** Observation:
`NV2080_CTRL_CMD_GR_GET_CURRENT_RESIDENT_CHANNEL` (0x2080121c, `sdk/ctrl/ctrl2080/ctrl2080gr.h:1242`,
flags 0x48 = NON_PRIVILEGED | ROUTE_TO_PHYSICAL, `gen/g_subdevice_nvoc.c:9487`), polled a few times.
- E1. Resident is our channel for a long time with no progress: GR is executing or stuck inside our
  context (a compute kernel spinning, a semaphore acquire by the engine, an FE write waiting on a
  fault that is being replayed). Look for an RC event and the error notifier; next step is the
  push decode of C2 applied to the engine methods.
- E2. Resident is never our channel: the context was never loaded. FECS does not load a context the
  scheduler never selects: go to F. A context load that fails is not silent (FECS reports it, Xid
  38 as in d-gr-c2, or an MMU fault on set B, Xid 31).

**F. Is the TSG being scheduled?** Observation: `NVA06C_CTRL_CMD_GET_TIMESLICE` (0xa06c0104,
`sdk/ctrl/ctrla06c.h:172`, flags 0x8, `gen/g_kernel_channel_group_api_nvoc.c:360`) on the host TSG
must return a non-zero timeslice; `NV0080_CTRL_CMD_FIFO_GET_CHANNELLIST` (0x80170d,
`sdk/ctrl/ctrl0080/ctrl0080fifo.h:178`, flags 0x109 = NON_PRIVILEGED | NO_GPUS_LOCK |
API_LOCK_READONLY, `gen/g_device_nvoc.c:978`) must list the host channel. If both hold and E2 is
still true, the TSG is starved or disabled: re-issue `NVA06C_CTRL_CMD_GPFIFO_SCHEDULE` on the TSG
(0xa06c0101, `sdk/ctrl/ctrla06c.h:97`, flags 0x10008) in addition to the per-channel schedule the
device does today (`D/mirror.rs:497-502` schedules the channel only), or `NVA06C_CTRL_CMD_PREEMPT`
(0xa06c0105, flags 0x10248, `gen/g_kernel_channel_group_api_nvoc.c:375`) to force a reschedule.

**G. Nothing on any branch, then an RC arrives later.** Observation: the RC OS event
(`exceptType`, already logged, `R/d-gr-launch5/device-trace.txt:8546`) plus the error notifier
record. Decode `exceptType` with `sdk/nverror.h` (31 MMU fault, 32 PBDMA, 13 GR exception); for an
MMU fault, check whether the faulting VA is a set-A page the shadow VAS lacked at the moment of
the doorbell (launch5: yes, fixed by re-sync before ring) or a set-B VA the guest's own mappings
collide with (the collision list, `D/shadow.rs:186-197`).

## 3. Controls a device client can call, with privilege

Privilege is the `flags` value of the control's entry in the generated method table, decoded with
`src/nvidia/inc/kernel/rmapi/control.h`: `KERNEL_PRIVILEGED` = no bit, i.e. 0 in bits 2-3
(`:175`), `PRIVILEGED` 0x4 (`:199`, needs an admin client, the CAP_SYS_ADMIN window), `NON_PRIVILEGED`
0x8 (`:205`), `ROUTE_TO_PHYSICAL` 0x40 (`:230`, executed in GSP-RM on this host), `INTERNAL`
0x80 (`:236`), `API_LOCK_READONLY` 0x100 (`:244`), `GSP_PLUGIN_FOR_VGPU_GSP` 0x10000 (`:287`).

| Control | Id, header | Flags, table | Who may call | What it tells the device |
|---|---|---|---|---|
| `NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN` | 0xc36f0108, `sdk/ctrl/ctrlc36f.h:79` | 0x10008, `gen/g_kernel_channel_nvoc.c:733` | unprivileged | runlist id + ChID of the channel (A); fails if no runlist |
| `NVA06F_CTRL_CMD_GPFIFO_SCHEDULE` | 0xa06f0103, `sdk/ctrl/ctrla06f/ctrla06fgpfifo.h:66` | 0x10008, `gen/g_kernel_channel_nvoc.c:508` | unprivileged | enables the channel on its runlist (used today) |
| `NVA06F_CTRL_CMD_BIND` | 0xa06f0104, `ctrla06fgpfifo.h:96` | 0x10008, `gen/g_kernel_channel_nvoc.c:523` | unprivileged | binds a channel to an engine/runlist; a channel allocated with `engineType` already has a runlist (the token proves it, row 1) |
| `NVA06F_CTRL_CMD_STOP_CHANNEL` | 0xa06f0112, `ctrla06fgpfifo.h:237` | 0x10008, `gen/g_kernel_channel_nvoc.c:583` | unprivileged | action, not a query |
| `NV906F_CTRL_GET_CLASS_ENGINEID` | 0x906f0101, `sdk/ctrl/ctrl906f.h:94` | 0x10008, `gen/g_kernel_channel_nvoc.c:448` | unprivileged | engine id and class of an object on the channel: confirms the compute object is the GR engine's |
| `NV906F_CTRL_CMD_RESET_CHANNEL` | 0x906f0102, `ctrl906f.h:139` | 0x10008, `gen/g_kernel_channel_nvoc.c:463` | unprivileged | action (recover a hung channel) |
| `NVA06C_CTRL_CMD_GPFIFO_SCHEDULE` | 0xa06c0101, `sdk/ctrl/ctrla06c.h:97` | 0x10008, `gen/g_kernel_channel_group_api_nvoc.c:315` | unprivileged | schedules the whole TSG (F) |
| `NVA06C_CTRL_CMD_SET_TIMESLICE` | 0xa06c0103, `ctrla06c.h:146` | 0x10008, `...group_api_nvoc.c:345` | unprivileged | action |
| `NVA06C_CTRL_CMD_GET_TIMESLICE` | 0xa06c0104, `ctrla06c.h:172` | 0x8, `...group_api_nvoc.c:360` | unprivileged | the TSG's timeslice (F) |
| `NVA06C_CTRL_CMD_PREEMPT` | 0xa06c0105, `ctrla06c.h:203` | 0x10248, `...group_api_nvoc.c:375` | unprivileged | preempts the TSG; an error or timeout says the context cannot be saved (GR hung inside it) |
| `NV0080_CTRL_CMD_FIFO_GET_CHANNELLIST` | 0x80170d, `sdk/ctrl/ctrl0080/ctrl0080fifo.h:178` | 0x109, `gen/g_device_nvoc.c:978` | unprivileged | the ChIDs of channels by handle (F) |
| `NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES` | 0x801707, `ctrl0080fifo.h:119` | 0x50148, `gen/g_device_nvoc.c:963` | unprivileged | sizes/alignment of engine context buffers (sanity for set B) |
| `NV2080_CTRL_CMD_FIFO_GET_USERD_LOCATION` | 0x2080110d, `sdk/ctrl/ctrl2080/ctrl2080fifo.h:372` | 0x8, `gen/g_subdevice_nvoc.c:8962` | unprivileged | USERD aperture/attribute |
| `NV2080_CTRL_CMD_FIFO_GET_CHANNEL_MEM_INFO` | 0x2080110c, `ctrl2080fifo.h:339` | 0x10004, `gen/g_subdevice_nvoc.c:8947` | **admin** (CAP_SYS_ADMIN window) | instance block, RAMFC, method buffers of a channel: proves the channel's PDB is the shadow VAS root |
| `NV2080_CTRL_CMD_GR_GET_CURRENT_RESIDENT_CHANNEL` | 0x2080121c, `sdk/ctrl/ctrl2080/ctrl2080gr.h:1242` | 0x48, `gen/g_subdevice_nvoc.c:9487` | unprivileged | the channel whose GR context is loaded now (E) |
| `NV2080_CTRL_CMD_GR_GET_CTXSW_MODES` | 0x20801235, `ctrl2080gr.h:1846` | 0x48, `gen/g_subdevice_nvoc.c:9742` | unprivileged | the channel's ctxsw modes (preemption, PM) |
| `NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE` | 0x20801210, `ctrl2080gr.h:831` | 0x10348, `gen/g_subdevice_nvoc.c:9367` | unprivileged | action: e.g. force WFI compute preemption to rule out a CILP/CTA save hang |
| `NV2080_CTRL_CMD_GR_GET_CTX_BUFFER_INFO` | 0x20801219, `ctrl2080gr.h:1129` | 0x8000, `gen/g_subdevice_nvoc.c:9457` | **kernel only** | set-B VAs/PAs; not reachable (exec-formats.md section 4.3) |
| `NV2080_CTRL_CMD_GPU_GET_ENGINES_V2` | 0x20800170, `sdk/ctrl/ctrl2080/ctrl2080gpu.h:778` | 0x10109, `gen/g_subdevice_nvoc.c:4717` | unprivileged | engine list only, no busy/idle |
| `NV2080_CTRL_CMD_RC_GET_ERROR_COUNT` | 0x20802205, `sdk/ctrl/ctrl2080/ctrl2080rc.h:94` | 0x8, `gen/g_subdevice_nvoc.c:11242` | unprivileged | number of RC error records; a change after the doorbell means an RC the event path missed |
| `NV2080_CTRL_CMD_RC_GET_ERROR_V2` | 0x20802213, `ctrl2080rc.h:131` | 0x8, `gen/g_subdevice_nvoc.c:11407` | unprivileged | one RC error record (the RC-diag data exec-formats.md section 1.3 asks for) |
| `NV2080_CTRL_CMD_RC_GET_WATCHDOG_INFO` | 0x20802209, `ctrl2080rc.h:179` | 0x118, `gen/g_subdevice_nvoc.c:11272` | unprivileged | whether the RM watchdog is running (a disabled watchdog explains silence instead of an RC after a hang) |
| `NV2080_CTRL_CMD_RC_READ_VIRTUAL_MEM` | 0x20802204, `ctrl2080rc.h:70` | 0x0, `gen/g_subdevice_nvoc.c:11227` | **kernel only** | not reachable; the device reads its memory directly anyway |
| `NV2080_CTRL_CMD_GPU_EXEC_REG_OPS` | 0x20800122, `ctrl2080gpu.h:738` | 0x10118, `gen/g_subdevice_nvoc.c:4087` | unprivileged, but **register access** | excluded by the host rule (no register reads) |

No control in this tree reports "GR busy" or FECS status directly to a user client; the closest
proxies are the resident channel (E) and whether a TSG preempt completes.

## 4. Memory the device can read without any control

- **USERD** (its own reservation page, `D/mirror.rs:441-452`): `Put` +0x40, `Get` +0x44,
  `TopLevelGet` +0x58, `GetHi` +0x60, `GPGet` +0x88, `GPPut` +0x8C (`sdk/class/clc46f.h:48-64`).
  The device reads only GPGet/GPPut today (`D/mirror.rs:546-560`); adding Get/GetHi gives node C.
- **Channel error notifier**: `NvNotification[NV_CHANNELGPFIFO_NOTIFICATION_TYPE__SIZE_1]`,
  index 0 = `TYPE_ERROR`, index 1 = `TYPE_WORK_SUBMIT_TOKEN` (`sdk/nvos.h:2854-2857`); each
  record is `timeStamp` 8 bytes, `info32`, `info16`, `status` with bit 15 IN_PROGRESS
  (`sdk/nvgputypes.h:57-64`, `nvos.h:2858-2860`). **The shadow GR channel is allocated without an
  error notifier**: `cp.hObjectError` is never set (`D/mirror.rs:453-458`), while the CE mirror
  passes the guest's page (`D/mirror.rs:1049-1073`). So on an RC the guest never sees an error
  record and the device has none to read; and `GET_WORK_SUBMIT_TOKEN` has no notifier to write
  the host token into. Pass the guest's error-notifier page as for CE channels.
- **The push itself and its semaphores** (guest memory by identity), as in the CE diagnostic.

## 5. What a GR channel needs at first schedule that a CE channel does not

1. **An engine context.** A GR channel's context (set B: MAIN from the golden image, PATCH, PM,
   priv access map, FECS event buffer) is allocated, mapped into the channel VAS and promoted when
   the compute object is allocated (exec-formats.md section 4.2, `kernel_graphics_context.c:1615-1705`,
   `kernel_graphics_object.c:173-227`), not at schedule time. On a host-managed VAS the host RM does
   all of it; the device only has to allocate `TURING_COMPUTE_A` on the channel, which it does
   (`D/mirror.rs:477-491`; launch4/5 `host object class 0xc5c0`). A CE channel has no FECS context.
   **Device does it: yes.**
2. **Schedulability.** `kchannelIsSchedulable` refuses GR only on an externally owned VAS without a
   bound context (`src/nvidia/src/kernel/gpu/fifo/kernel_channel.c:2130-2136`); the shadow VAS is
   host-managed, and the schedule returned NV_OK. **Yes.**
3. **Bind.** `NVA06F_CTRL_CMD_BIND` is for channels without an engine; the device passes
   `engineType` at allocation and receives a token with runlist 0, which RM only issues for a
   channel with a runlist (`kernel_fifo_tu102.c:111-121`). **Not needed.**
4. **TSG scheduling and timeslice.** The device schedules the channel, not the TSG, and never sets
   a timeslice (`D/mirror.rs:497-502`). The TSG timeslice is the RM default set at TSG allocation
   (not verified in this pass; check with `GET_TIMESLICE`, node F). The guest put chids 2-9 in one
   TSG with subcontexts (`FERMI_CONTEXT_SHARE_A` 0x9067, `R/d-gr-launch4/device-trace.txt:146`); the shadow path allocates **one host TSG per guest channel** (`D/mirror.rs:417-437`
   runs for every chid, and `self.tsgs.insert` overwrites the previous handle under the same key),
   with no `hContextShare`. That gives 8 separate GR contexts instead of one shared context, and
   leaks all but the last TSG handle from the map (forwarded TSG controls and teardown reach only
   the last one, `D/mirror.rs:1351-1374`). **Partly; divergent.**
5. **Preemption mode.** Default compute preemption is chosen by the RM at object allocation;
   `GR_GET_CTXSW_MODES` reads it and `GR_SET_CTXSW_PREEMPTION_MODE` can force WFI (section 3).
   A preemption-buffer problem would fault or time out a preempt, not stay silent on first run
   (inference). **Not touched by the device; not needed for a first run.**
6. **SET_OBJECT.** Methods to a subchannel only reach an engine after `SET_OBJECT` binds the class
   to that subchannel (`clc46f.h:68-71`); the guest's first GR push does this itself (CUDA's
   channel init, the software executor already models it, `D/channel.rs:904-908`). A missing
   SET_OBJECT raises an error, it is not silent. **Guest's job; nothing for the device.**

## 6. Known silent-hang causes on a fresh Turing GR channel

| Cause | Silent? | Here |
|---|---|---|
| GPPut 0 because the host RM cleared USERD at alloc | yes | launch4; fixed in launch5 |
| Doorbell token of another channel or runlist | yes | excluded: the device rings RM's token (section 2 A) |
| Channel not on the runlist / not scheduled | yes | excluded: token issued and SCHEDULE NV_OK |
| TSG with zero timeslice or starved TSG | yes | unverified, node F decides it |
| GP_PUT written to the wrong USERD (guest's vs host's) | yes | excluded: the host channel is given the guest's USERD page itself (`D/mirror.rs:441-457`) |
| BAR1 write-combining not drained before the doorbell | fetches stale data, usually PBENTRY | flushed (`D/mirror.rs:541-543`) |
| Host semaphore ACQUIRE that never succeeds (e.g. waits on a CE semaphore) | yes | node C2 |
| Completion written but the release did not ask for an interrupt, or the MSI was coalesced | yes, work is done | node D1 |
| Engine non-stall interrupt masked or not armed | yes for event waiters | GR0 events arrive (host-wide), so armed |
| Push on a subchannel with no object, illegal method | no, Xid 32/13 | |
| Unmapped ctxsw buffer at first load | no, Xid 31/38 | |
| Unmapped set-A page at first use | no, Xid 31 | launch5, re-sync after ring |
| Channel RC with no error notifier bound | silent to the guest | shadow channel has none (section 4) |

**[nouveau, from memory]** gv100/tu102 nouveau enables a channel by setting its runlist entry and
committing the runlist, and a TU102 channel whose runlist was not committed after the channel was
added does not run and gives no error; nouveau's gv100 fifo also names the PBDMA INTR_1 CTXNOTVALID
case (seen here in run 10) as "no valid engine context". Both are consistent with branches F and E2.

## 7. Suggested diagnosis step for the device (one function, no registers)

After a shadow GR doorbell, when a flag like `CARAXES_MIRROR_DIAG=1` is set, poll for up to 50 ms:
USERD GPGet/GPPut/Get/GetHi/TopLevelGet (B, C); `GR_GET_CURRENT_RESIDENT_CHANNEL` (E);
`RC_GET_ERROR_COUNT` (G); once, `GET_TIMESLICE` on the host TSG and `GET_CHANNELLIST` (F); and the
error notifier record once one is bound. Log one line per change. Before that, two fixes in
`gr_shadow_doorbell`: re-sync the shadow VAS **before** ringing (`D/mirror.rs:322-331`), and bind
the guest's error-notifier page (`D/mirror.rs:453-458`); then reuse one host TSG per guest TSG.

## Appendix A: control inventory for self-observation (file-search agent, 2026-09-23)

A read-only file search over `vendor/open-gpu-kernel-modules` (610.57.04), recorded as
delivered; paths relative to that tree. Privilege from `RMCTRL_FLAGS` in
`src/nvidia/inc/kernel/rmapi/control.h`: bit `0x8` NON_PRIVILEGED = any client; bit `0x4`
without `0x8` = PRIVILEGED (the CAP_SYS_ADMIN window); neither = kernel only; `0x80` = INTERNAL
(NOT_SUPPORTED to clients); `0x40` = ROUTE_TO_PHYSICAL (runs in GSP-RM on this host).

### Channel (a06f, inherited 906f, c36f, b06f), export table `src/nvidia/generated/g_kernel_channel_nvoc.c`

| Control | Id | Flags | Privilege | Reveals |
|---|---|---|---|---|
| GET_CLASS_ENGINEID | 0x906f0101 | 0x10008 | any | engine id and class tag of one of the client's objects |
| RESET_CHANNEL | 0x906f0102 | 0x10008 | any | resets the channel; returns `bIsRcPending` |
| GET_DEFER_RC_STATE | 0x906f0105 | 0x48 | any, physical | whether a deferred RC is pending |
| **GET_MMU_FAULT_INFO** | 0x906f0106 | 0x10048 | any, physical | the last MMU fault of the channel: address, fault type, fault string, faulting shader VA (`ctrl906f.h:214-220`) |
| GPFIFO_SCHEDULE | 0xa06f0103 | 0x10008 | any | control, not observation |
| BIND | 0xa06f0104 | 0x10008 | any | control |
| SET_ERROR_NOTIFIER | 0xa06f0108 | 0x8 | any | enables error-notifier writes |
| RESTART_RUNLIST | 0xa06f0111 | 0x48 | any, physical | control |
| STOP_CHANNEL | 0xa06f0112 | 0x10008 | any | control |
| GET_CONTEXT_ID | 0xa06f0113 | 0x109 | any, read-only | context id |
| GPFIFO_GET_WORK_SUBMIT_TOKEN | 0xc36f0108 | 0x10008 | any | the doorbell token |
| GPFIFO_UPDATE_FAULT_METHOD_BUFFER | 0xc36f0109 | 0x10244 | admin, physical | control |
| **GetChannelHwState** | 0xb06f010f | 0x10048 | any, physical | channel enable, next, pending, on-PBDMA bits |
| b06f GetEngineCtxSize/Data/State | 0xb06f01xx | 0x10048 | any, physical | hardware context state reads |
| b06f Set/Migrate/Restore | | 0x14044 / 0x14240 | admin | |

### Channel group (a06c), export table `g_kernel_channel_group_api_nvoc.c`

| Control | Id | Flags | Privilege | Reveals |
|---|---|---|---|---|
| GPFIFO_SCHEDULE | 0xa06c0101 | 0x10008 | any | control |
| SET_TIMESLICE / GET_TIMESLICE | 0xa06c0103 / 0104 | 0x10008 / 0x8 | any | the group's timeslice |
| PREEMPT | 0xa06c0105 | 0x10248 | any, physical | forces a group preempt; a completing preempt is the closest "GR not hung" proxy |
| GET_INFO | 0xa06c0106 | 0x8 | any | the group's runlist and tsg id |
| GET_INTERLEAVE_LEVEL | 0xa06c0108 | | any | |
| INTERNAL_* / PROMOTE_FAULT_METHOD_BUFFERS | | 0x100c8 / 0x14240 | kernel / admin | |

### Subdevice (2080), export table `g_subdevice_nvoc.c`

| Control | Id | Flags | Privilege | Reveals |
|---|---|---|---|---|
| FIFO_GET_INFO | 0x20801109 | 0x30008 | any | channel count, runlist and engine facts |
| FIFO_GET_CHANNEL_MEM_INFO | 0x2080110c | 0x10004 | admin | USERD, instance block and RAMFC apertures and addresses |
| FIFO_GET_USERD_LOCATION | 0x2080110d | 0x8 | any | USERD aperture |
| FIFO_GET_DEVICE_INFO_TABLE | 0x20801112 | 0x5c040 | kernel only | |
| GPU_GET_ENGINES_V2 / ENGINE_CLASSLIST / ENGINE_PARTNERLIST | 0x20800170 / 0124 / 0147 | | any | engines, classes, the GR partner copy engine |
| GPU_EXEC_REG_OPS | 0x20800122 | 0x10118 | any at the ACL, per-register allow-list | register access: excluded by the no-register rule |
| PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2 | 0x20802096 | 0x50008 | any | GR, FB, video and PCIe utilization samples: the practical "is GR busy" signal |
| PERF_GPU_IS_IDLE | 0x20802089 | | not exported in this build | |
| GR_GET_CURRENT_RESIDENT_CHANNEL | 0x2080121c | 0x48 | any, physical | which channel GR has resident |
| GR_GET_CTXSW_MODES | 0x20801235 | 0x48 | any, physical | zcull, pm, smpc, CILP and GFXP modes of a channel |
| GR_GET_CTXSW_STATS | 0x20801215 | 0x48 | any, physical | context-switch counters of a channel |
| GR_GET_CTX_BUFFER_SIZE | 0x20801218 | 0x18 | any | total context buffer size |
| GR_GET_CTX_BUFFER_INFO | 0x20801219 | 0x8000 | kernel only | |
| RC_GET_ERROR_COUNT / RC_GET_ERROR_V2 | 0x20802205 / 0x20802213 | 0x8 | any | queued RC records (exceptType, engine) for the client's channels |
| RC_GET_WATCHDOG_INFO | 0x20802209 | 0x118 | any | watchdog state |
| RC_READ_VIRTUAL_MEM | 0x20802204 | 0x0 | kernel only | |

### Device (0080) and client (0000)

| Control | Id | Flags | Privilege | Reveals |
|---|---|---|---|---|
| FIFO_GET_ENGINE_CONTEXT_PROPERTIES | 0x801707 | 0x50148 | any | per-engine context buffer sizes and alignments |
| FIFO_GET_CHANNELLIST | 0x80170d | 0x109 | any | channel handles to hardware channel ids |
| NVD_GET_RCERR_RPT | 0x607 | 0x8 | any | RC error report records, filtered by client |

### Debugger (GT200_DEBUGGER, 0x83de)

An unprivileged client can allocate a debugger session bound to its own compute object: the
constructor checks only `RS_ACCESS_DEBUG` on the target object, which the owner holds
(`kernel_sm_debugger_session.c:255-302`). Then `DEBUG_READ_ALL_SM_ERROR_STATES` 0x83de030c
(0x50048, physical) and `DEBUG_GET_MODE_MMU_DEBUG` 0x83de0308 (0x10248) read SM error and MMU
debug state without registers. This is the only path to SM error state from user space.

### Notifier and RC codes

The channel's error notifier is entry 0 (`NV_CHANNELGPFIFO_NOTIFICATION_TYPE_ERROR`,
`nvos.h:2854`) of `NvNotification` (`nvgputypes.h:57-64`: timestamp, info32, info16, status);
at RC the RM writes info32 = exceptType, info16 = engine, status = notifier status
(`kernel_rc_notification.c:42-91`). exceptType 0x1f = MMU fault, 0x20 = PBDMA error
(`nverror.h:49-50`); 0x26 has no `ROBUST_CHANNEL_*` name in this tree (values 37 and 38 are gaps).
