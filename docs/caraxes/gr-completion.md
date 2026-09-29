<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GR completion: how a compute push's completion reaches libcuda, and what the device owes

Written 2026-09-23 (read-only research, no run). Driver citations are relative to
`vendor/open-gpu-kernel-modules/` (tag 610.57.04): `K/` = `src/nvidia/src/kernel/`,
`SDK/` = `src/common/sdk/nvidia/inc/`, `U/` = `kernel-open/nvidia-uvm/`,
`G/` = `src/nvidia/generated/`. `D/` = `crates/vfio-user-nvgpu/src/`. `T:n` is line n of
`matrix/traces/610.57.04/cuda-core.tsv` (native run). Run directories are under
`caraxes/runs/e5b2-2026-09-23/`. Line numbers in `D/` are those of the working tree at
23:00 on 2026-09-23 (another agent is editing the crate; `mirror.rs` last changed 22:55,
`boot.rs` 23:00).

## 0. Bottom line

1. **Completion of GR work is a semaphore in memory first, an interrupt second.** In the
   default scheduling mode (`sched=auto`) libcuda spins on a semaphore payload and uses the
   non-stall OS event only as the fallback wakeup (cuda-lookahead.md §4.8). The device
   already raises the GR0 non-stall vector, continuously: the host GR0 notifier fires for
   every client's GR work on the card (the desktop), 5993 times in `d-gr-launch4` and 8316
   times in `d-gr-launch5`, each one raising guest vector 4 (`mirror.rs:1752-1759`). So the
   guest's GR0 OS event is woken many times a second whatever the mirrored channel does.
   **A lost interrupt cannot explain a permanent hang.** What hangs is a semaphore that never
   reaches the value libcuda waits for.
2. The two newest runs show two different failures upstream of completion:
   - `d-gr-launch4` (22:54): the GR push was **never fetched**. "chid 2 USERD after doorbell:
     GPGet Some(0) GPPut Some(0)" for all 8 GR channels. The host RM cleared the USERD at
     channel creation and the shadow path did not restore the guest's GPPut (the CE path did:
     "the guest's GPGet 0 / GPPut 1 restored"). Fixed after that run by
     `mirror.rs:360-377` and `:531-540`.
   - `d-gr-launch5` (22:58): all 8 first pushes were **fetched** ("GPGet Some(1) GPPut
     Some(1)"). libcuda went on (CE doorbells 2-5 on chid 10, then doorbells 2-5 on GR chid 2).
     After GR doorbell 3 the host logged **Xid 31, ENGINE GRAPHICS HUBCLIENT_FE, FAULT_PDE,
     ACCESS_TYPE_VIRT_WRITE at 0x100_04e00000** on host channel 0x3f (= guest chid 2,
     `d-gr-launch5/host-xid.txt`), and the device saw the host RC event (exceptType 0x1f,
     trace line 8546). The host channel is dead from then on, the guest is never told, and
     libcuda waits forever. This is a **write that faulted** (a set-A page the shadow VAS does
     not have), not a completion that was lost. The resync at each guest MMU invalidate that
     was added afterwards (`boot.rs:383-404`, 23:00) targets exactly this.
3. The one GR-specific piece the device does not do at all is **error delivery**: the shadow
   GR channel is created without the guest's error notifier (`mirror.rs:453-458`, no
   `hObjectError`, unlike the CE path `mirror.rs:1047-1073`), and a host RC is only counted
   (`mirror.rs:1761-1779`). A faulted GR channel therefore turns into a silent hang instead of
   a CUDA error.
4. The CE comparison in the brief needs one correction: **no CE non-stall event has ever
   fired** in any run. `rg 'notifier 0x8000019|0x800001a|0x800001b'` over all `d-*` traces
   matches only the three registration lines per run; every "fired" line is the GR0 notifier
   0x800000c. CE work completed (`d-core-fast` reached "stage 2 ok (memory)") through the
   semaphore payload alone. The non-stall path is built and exercised (by the desktop's GR0
   events) but has never been the thing that completed a CUDA wait.

## 1. The completion chain on Turing, GSP-client mode

### 1.1 In the push

- **Host semaphore release**, on any subchannel, class `TURING_CHANNEL_GPFIFO_A` 0xC46F:
  `SEM_ADDR_LO` 0x5c, `SEM_ADDR_HI` 0x60, `SEM_PAYLOAD_LO` 0x64, `SEM_PAYLOAD_HI` 0x68,
  `SEM_EXECUTE` 0x6c with `OPERATION` 2:0 = `RELEASE` 1, `RELEASE_WFI` 20:20,
  `PAYLOAD_SIZE` 24:24, `RELEASE_TIMESTAMP` 25:25 (`SDK/class/clc46f.h:201-230`). The host
  (PBDMA/ESCHED) writes the payload to the channel's VA. With `RELEASE_WFI_EN` it first waits
  for the engine to idle, which is what makes a host release after compute work a completion
  marker. UVM's Turing HAL shows the exact encoding (`U/uvm_turing_host.c:84-105`).
- **Non-stall interrupt request**: `NON_STALL_INTERRUPT` 0x20 (`SDK/class/clc46f.h:110-111`),
  emitted after the release (UVM: `U/uvm_turing_host.c:79-81`). There is no interrupt bit
  inside `SEM_EXECUTE` on C46F; the interrupt is always this separate method. Executed on a
  GR channel it becomes the GR engine's non-stall interrupt.
- **Compute-class release** (alternative): `NVC5C0_SET_REPORT_SEMAPHORE_A..D` 0x1b00-0x1b0c,
  `D.OPERATION` 1:0 = `RELEASE` 0, `D.AWAKEN_ENABLE` 20:20, `D.STRUCTURE_SIZE` 28:28
  (`SDK/class/clc5c0.h:622-638`). The release is written by the GR front end (FE); a fault on
  it is reported as `HUBCLIENT_FE ... VIRT_WRITE`, which is the `d-gr-launch5` Xid.
  `AWAKEN_ENABLE` makes the GR engine raise its non-stall interrupt when the write lands.
- The native traces carry no push contents, so which of the two libcuda uses for the
  context-creation pushes is not established (cuda-lookahead.md §4.6: "the method set is
  unknown"). Section 4 shows how to read it from the guest's own pushbuffer.

### 1.2 The GR non-stall interrupt (physical GPU, host side)

On the physical card the GR non-stall interrupt goes to the **host** kernel RM. The host's
`engineNonStallIntrNotify(GR0)` wakes every host client that registered
`NV2080_NOTIFIERS_GR0 | NV01_EVENT_NONSTALL_INTR` on its subdevice. The device is one such
client: `ensure_events` registers notifier 12 (`NOTIFIER_GRAPHICS`, `mirror.rs:59`, `:67-72`)
with `EVENT_NONSTALL` 0x08000000 (`mirror.rs:65`) through `NV_ESC_ALLOC_OS_EVENT`, an
`NV01_EVENT_OS_EVENT` on the host subdevice and `EVENT_SET_NOTIFICATION` REPEAT
(`hostmem.rs:667-712`). The notification is per engine, not per channel: the host fires it
for the desktop's work too (`intr.rs:52-58`, measured 31927 events in three minutes).

### 1.3 The guest's interrupt tree and kernel RM

- **Vector.** The guest's kernel interrupt table comes from GSP
  (`INTERNAL_INTR_GET_KERNEL_TABLE`, `K/gpu/intr/intr.c:1019-1070`), which the device answers
  from `chips/tu106/interrupts.toml`: GR0 `vector_nonstall = 4`, CE2..CE4 5..7
  (`interrupts.toml:30-33`). Vector 4 is **leaf 0, bit 4**, in subtree 0, which is the
  `ESCHED_DRIVEN_ENGINE_NOTIFICATION` category (`interrupts.toml:37-42`); subtree 0's top bit
  is bit 0 of `TOP`.
- **Enables the guest writes at init.** `intrStateLoad_TU102` clears all leaf enables, then
  sets `LEAF_EN_SET` for every table entry's stall and non-stall vector
  (`K/gpu/intr/arch/turing/intr_tu102.c:80-131`, the non-stall one at `:126-131`). The top
  enable for the non-stall subtrees is `intrEnableTopNonstall_TU102`
  (`K/gpu/intr/arch/turing/intr_nonstall_tu102.c:91-121`), mask from
  `intrGetIntrTopNonStallMask_TU102` = the NOTIFICATION category's subtrees
  (`intr_tu102.c:1443-1466`). The device models these as `leaf_en` and `top_en`
  (`intr.rs:205-220`).
- **ISR.** The Linux top half services non-stall interrupts **without the GPU lock**:
  `intrGetPendingNonStall_HAL`, then `intrServiceNonStall_HAL`
  (`src/nvidia/arch/nvalloc/unix/src/unix_intr.c:272-281`), gated on
  `intrGetIntrEn == INTERRUPT_TYPE_HARDWARE` (`:272`). `intrGetPendingNonStall_TU102` reads
  `TOP` for each non-stall subtree, then `LEAF & LEAF_EN_SET` for its leaves, and maps set bits
  back to MC engines through the table (`intr_nonstall_tu102.c:194-305`).
- **Service.** `_intrServiceNonStallLeaf_TU102` clears the leaf bit (write 1 to `LEAF`,
  `intrClearLeafVector_HAL`, `intr_nonstall_tu102.c:372`) and calls
  `intrServiceNotificationRecords(GR)` (`:373`), which calls the engine's
  `pNotificationService` (`K/gpu/intr/intr.c:1150-1171`). For GR that is
  `kgraphicsServiceNotificationInterrupt_IMPL`, registered by
  `kgraphicsRegisterIntrService_IMPL` (`K/gpu/gr/kernel_graphics.c:2650-2667`, `:2666`), whose
  body is `kgraphicsNonstallIntrCheckAndClear_HAL` then
  `engineNonStallIntrNotify(RM_ENGINE_TYPE_GR(i))` (`kernel_graphics.c:2673-2690`).
  **`kgraphicsNonstallIntrCheckAndClear` is an empty stub on this build**
  (`G/g_kernel_graphics_nvoc.h:637`, `:995-997`): the guest RM reads and writes **no
  `NV_PGRAPH_*` register** on the GR non-stall path. The only device registers it touches are
  `TOP`, `LEAF`, `LEAF_EN_SET` of the interrupt tree.
- `bFifoWaiveNotify` is false for GR (`kernel_graphics.c:2665`), so
  `intrServiceNotificationRecords` also fires the HOST engine notifier when
  `bDefaultNonstallNotify` is set (`intr.c:1180-1192`). Harmless here.
- **Notify.** `engineNonStallIntrNotify` walks `pGpu->engineNonstallIntrEventNotifications[GR0]`
  (`K/rmapi/event_notification.c:455-461`) and calls `osNotifyEvent` for each registered event
  (`event_notification.c:415`), which signals the user's fd on `/dev/nvidia0` (the "gpu" fd in
  the traces). The per-vGPU semaphore filter in that loop (`event_notification.c:312-356`)
  applies only when `hSemMemory` is set on the subdevice (vGPU host); CUDA does not set it.

### 1.4 The client's event

- libcuda allocates at context creation **one** subdevice event for GR:
  `NV01_EVENT_OS_EVENT` 0x79 on the subdevice with `notifyIndex = 0x3800000c` =
  `NV2080_NOTIFIERS_GRAPHICS` 12 | `NV01_EVENT_NONSTALL_INTR` 0x08000000 |
  `NV01_EVENT_WITHOUT_EVENT_DATA` 0x10000000 | `NV01_EVENT_SUBDEVICE_SPECIFIC` 0x20000000
  (`T:879`, bytes `0c 00 00 38`; `SDK/class/cl2080_notification.h:48`, `:185`;
  `SDK/nvos.h:428-435`), after `NV_ESC_ALLOC_OS_EVENT` on fd 18 (`T:874-877`). One more per
  CE TSG with CE2/CE3/CE4 (25/26/27) | NONSTALL (cuda-lookahead.md row C21), and four
  compute-object events with notifyIndex 1..4 and no NONSTALL flag (row C24).
- In GSP-client mode the kernel both registers the event locally and forwards the alloc to
  GSP: `IS_FW_CLIENT` makes `eventapiConstruct` issue `NV_RM_RPC_ALLOC_EVENT`
  (`K/rmapi/event_api.c:149-171`), then `registerEventNotification` (`:188`), which for a
  NONSTALL index inserts the event into the per-engine non-stall list of the guest's own
  `OBJGPU` (`event_notification.c:688-749`). **The wakeup is served entirely inside the
  guest**: the RPC copy that reaches the device is bookkeeping. In `d-gr-launch4` the four
  subdevice events arrive as `GSP_RM_ALLOC hClass=0x79 paramsSize=24` under 0x5c000003 and
  are echoed ("object recorded, no host object yet"), trace lines 9036, 9096, 9156, 9216.
  That is correct and needs no change.
- The compute-object events (C24) would be signalled by a `POST_EVENT` 0x1003 message from GSP
  (`K/gpu/gsp/kernel_gsp.c:500`, dispatched at `:1443`); the device never sends one. Whether
  libcuda needs them is open (cuda-lookahead.md §7).

### 1.5 How libcuda waits

- `sched=auto` (nvprobe's mode): spin, then yield, polling the semaphore in memory; the
  event fds are a fallback (cuda-lookahead.md §4.8, `P:lvl4auto`).
- `CU_CTX_SCHED_BLOCKING_SYNC`: sleeps in `poll` on the 8 event fds; natively
  `cuda-launch.tsv` has ~1100 `poll` rows per fd, almost all timeouts.
- **Stuck-wait signature (measured, new).** While waiting on a value that does not come,
  libcuda issues `NV2080_CTRL_CMD_MC_SERVICE_INTERRUPTS` 0x20801702 once per second: 35 of
  them in `d-gr-launch4` from 180.77 s (1.0 s after the last GR doorbell), 36 in
  `d-gr-launch5` from 206.15 s (1.2 s after the RC), 52 in the stuck CE run `d-ce-inval`, and
  **none** in any completing run (`d-core-fast`, `d-gr-shadow`) and none in the native traces.
  In the guest this control services the **stall** list only
  (`K/gpu/intr/intr.c:189-283`, `intrServiceStallList_HAL` at `:280`) after forwarding it to
  GSP (`:203-228`); the device answers it with a synthesized NV_OK. It never runs the
  non-stall path, so it cannot rescue a lost non-stall interrupt either. The count and start
  time of this control are the cheapest "libcuda is stuck in a wait" probe the device has.

## 2. Guest business and device business

### 2.1 Served by the guest itself (the device must not interfere)

- The event objects, their registration and the fd wakeup (§1.3, §1.4).
- Leaf/top enables, leaf clearing, ISR, notification records, `engineNonStallIntrNotify`.
- The semaphore wait in libcuda.

### 2.2 What the device must produce

| # | What | Register/bit | Today |
|---|---|---|---|
| a | The semaphore payload at the guest's semaphore PA, visible to the guest CPU | memory, not a register: guest sysmem (the GPU writes guest RAM through the registered descriptor) or reservation VRAM (read by the guest through BAR1, served by the VRAM store) | done by the physical GPU when the shadow VAS maps that VA to the same backing (`shadow.rs:148-214`); fails when the VA is missing (`d-gr-launch5` Xid 31) |
| b | The GR0 non-stall vector | `LEAF[0]` bit 4 (`intr_tree` 0xB81000 + 0x000, `chip.toml:49`, `intr.rs:15`); `TOP` bit 0 follows from `leaf & leaf_en` (`intr.rs:138-150`) | done: `IntrShared::raise(4)` sets the pending bit and writes the MSI eventfd (`intr.rs:59-81`), folded into the leaf at the guest's next access (`intr.rs:126-132`) |
| c | An MSI | the eventfd | done (`intr.rs:70-80`), coalesced while the bit is still pending (`intr.rs:63-68`) |
| d | Write-1-to-clear of the leaf and a correct `TOP` | `LEAF` W1C, `LEAF_EN_SET/CLEAR`, `TOP_EN_SET/CLEAR` | done (`intr.rs:195-237`) |
| e | No `NV_PGRAPH_*` status or clear register | none: the check-and-clear is a stub (§1.3) | nothing needed |
| f | Host registration: `NV2080_NOTIFIERS_GR0 \| NONSTALL` OS event on the host subdevice, REPEAT | host RM | done (`mirror.rs:654-721`, `hostmem.rs:667-712`); the host channel needs no per-channel notifier for completion |
| g | The host RC reflected to the guest (error notifier page, or an RC event) so libcuda fails instead of hanging | the channel's error notifier, or GSP event `RC_TRIGGERED` (`K/gpu/gsp/kernel_gsp.c:585`, `:1447-1448`) | **not done** for GR: shadow channel has no `hObjectError` (`mirror.rs:453-458`); RC only counted (`mirror.rs:1761-1779`) |

`NVA06F_CTRL_*`/`NVC36F_CTRL_*` controls are not needed for completion. The only per-channel
notifier write that matters is the work-submit token (`kchannelNotifyWorkSubmitToken`,
`K/gpu/fifo/kernel_channel.c:3286`, `:4015-4030`), which goes into the channel's error
notifier memory. For CE channels that memory is the guest's page, so libcuda re-reads the
**host** token and rings it (`mirror.rs:565-578`, `ctrl.rs:180-185`). For the shadow GR channel
the host has no guest page to write into, so the guest keeps ringing its own token and
`ctrl.rs:184` maps it back by ChID; that works, and is the reason the GR token path differs.

### 2.3 CE versus GR: the differences, as a checklist

| Item | CE mirror channel | GR shadow channel |
|---|---|---|
| VA space | host `FERMI_VASPACE_A`, externally owned, root = guest root (`mirror.rs:723-760`) | host-managed shadow VAS with set A fixed-mapped per leaf (`shadow.rs:77-222`) |
| Pages the push can reach | every page the guest root maps, by construction | only the leaves the last sync mapped; refused leaves are dropped silently (`shadow.rs:148-156`, not counted in `maps_failed`) |
| Sync of new guest mappings | none needed | at each guest MMU invalidate (`boot.rs:383-404`, added after `d-gr-launch5`) and at each doorbell, but **after** the ring (`mirror.rs:323-330`) |
| USERD GPGet/GPPut restore after the host clears it | yes (`mirror.rs:1181-1195`) | yes since 22:55 (`mirror.rs:360-377`, `:531-540`); missing in `d-gr-launch4` |
| Error notifier | the guest's page (`mirror.rs:1047-1073`) | none (`mirror.rs:453-458`) |
| Work-submit token in the guest notifier | host token, re-read by libcuda | not written; guest token used |
| Host non-stall event | CE2/CE3/CE4 notifiers 25-27 (`mirror.rs:60-62`) | GR0 notifier 12 (`mirror.rs:59`) |
| Guest vector | 5/6/7 (`interrupts.toml:31-33`) | 4 (`interrupts.toml:30`) |
| Events ever received | **never** (all runs) | continuously, from the host desktop |
| Push diagnostic | `mirror_push_diag` behind `CARAXES_MIRROR_DIAG=1` (`ctrl.rs:254-255`, `:276`) decodes CE semaphores | none on the shadow path (`ctrl.rs:188-203` returns before it); one-shot USERD read only (`mirror.rs:546-561`) |

## 3. What on the guest's GR non-stall path could itself block the notification

- **Top/leaf enables.** If the guest had not enabled vector 4 (`LEAF_EN_SET[0]` bit 4) or top
  subtree 0, `top()` would stay 0 for it and the ISR would find nothing. The guest enables
  every table vector at state load (§1.3). Indirect evidence that the guest services vector
  4: most raises in the trace write an MSI ("MSI" 5623 vs "no MSI" 370 in `d-gr-launch4`),
  and a raise only writes a new MSI after the guest's previous tree access took the pending
  bit (`intr.rs:63-68`, `:126-132`). Note that a coalesced raise is logged as "no MSI
  (disabled or no fd)" (`mirror.rs:1758`), which is misleading; it is usually "still pending".
- **`intrGetIntrEn`.** The ISR skips non-stall servicing unless the interrupt enable state is
  `INTERRUPT_TYPE_HARDWARE` (`unix_intr.c:272`). This is the same gate the CE and the MSI
  self-test pass, so it is not GR-specific.
- **NV_PGRAPH_INTR_EN or a GR non-stall enable.** Not consulted: the GR check-and-clear is
  the empty stub and the kernel RM owns no PGRAPH registers in GSP-client mode. Nothing the
  guest wrote at init gates GR non-stall delivery beyond the tree enables.
- **FECS / ctxsw.** `intrCheckFecsEventbufferPending` runs in the same ISR
  (`unix_intr.c:276`) but returns at once without a ctxsw-log consumer
  (`K/gpu/intr/intr.c:1196-1216`); CUDA without a profiler has none. The FECS log service is
  a stall service on `MC_ENGINE_IDX_GRn_FECS_LOG` (`kernel_graphics.c:2657-2660`), which is
  not in the device's table, so it never fires. The GR0 **stall** vector 196 in
  `interrupts.toml:30` must never be raised: in GSP-client mode GR has no stall service in
  the kernel RM, and nothing raises it today.
- **A value the guest expects in a notifier or USERD after the first GR push.** The guest
  kernel expects nothing. libcuda reads: GPGet in USERD (`SDK/class/clc46f.h:61-62`, offsets
  0x88/0x8C, `mirror.rs:97-99`) to reclaim GPFIFO slots; the semaphore payload; the error
  notifier page for an RC status; and, if the host wrote it, the work-submit token in that
  same notifier page. For a GR shadow channel only GPGet and the semaphore come from the
  physical GPU; the error notifier stays zero even after an RC, which is item g.

## 4. Diagnosis recipe for a stuck GR wait

Work top down; stop at the first check that fails.

1. **Is libcuda stuck in a wait?** Count `cmd=0x20801702` in `device-trace.txt` and note when
   the first one comes. Present and starting about 1 s after the last GR doorbell: yes (§1.5).
2. **Was the channel built and rung?** The `shadow: GR chid N ...` block must end with
   "scheduled; host token ..." and "doorbell: guest chid N -> host token ... (k on this
   channel)" (`mirror.rs:525-545`). The host token's ChID and runlist are in the line.
3. **Was the push fetched?** Read "shadow: chid N USERD after doorbell: GPGet G GPPut P"
   (`mirror.rs:556-560`, 5 ms after the ring). Also check for "restored guest GPGet / GPPut"
   (`mirror.rs:536-538`).
   - `P == 0`: the host USERD has no work: **push not fetched** (the `d-gr-launch4` state).
   - `P > 0, G < P`: fetch pending or stalled: scheduling, token, or a PBDMA fault (look in
     `host-xid.txt` for Xid 31/32 on the host ChID).
   - `G == P`: **fetched**. Go on.
4. **Did anything fault?** `host-xid.txt` and "HOST RC EVENT" in the trace. A fault client of
   `HUBCLIENT_FE` with `VIRT_WRITE` is a GR front-end write (compute semaphore release or
   inline-to-memory), `HUBCLIENT_HOST` or `ESC` would be the host semaphore. The fault address
   is a guest VA. For it:
   - walk the guest root at that VA (`FakeFb::walk_in`, as `ctrl.rs` does in its push
     diagnostic) and print the PTE: aperture, kind, address;
   - check whether `resolve_leaf` would refuse it (vidmem outside the reservation, sysmem not
     in a registered descriptor, or compressible kind 0x08-0x0E, `shadow.rs:50-75`); those
     leaves are dropped without a count (`shadow.rs:148-156`);
   - check whether it lies in an excluded set-B range (`mirror.rs:343-356`);
   - check whether it was mapped by the guest after the last sync (compare the time of the
     guest's MMU invalidate for root 0xe9000000 with the last "shadow: sync" line; before
     `boot.rs:383-404` only doorbells resynced, and only after ringing, `mirror.rs:323-330`).
   `d-gr-launch5` stops here: FE write to 0x100_04e00000, FAULT_PDE, on guest chid 2's third
   push, 0.47 s after a resync that added 770 maps.
5. **Fetched, no fault: did the semaphore land?** Decode the push the guest rang: the GPFIFO
   entry at `(GPPut - 1) mod entries` of the channel's `gpfifo_va` (8-byte entries), its
   pushbuffer VA and length, through the guest root. The existing CE decoder in
   `ctrl.rs:276-400` does exactly this (entry, pushbuffer, PTEs); extend its method match to
   the host methods 0x5c/0x60/0x64/0x68/0x6c (release when `SEM_EXECUTE & 7 == 1`, note
   `RELEASE_WFI` bit 20) and to the compute subchannel's 0x1b00-0x1b0c (release when
   `D & 3 == 0`, note `AWAKEN_ENABLE` bit 20). Resolve the semaphore VA through the guest
   root to a PA and read it some milliseconds after the ring.
   - value == payload: **executed and the semaphore landed**. If libcuda still waits, either
     it waits on a different semaphore (a later push) or the guest reads a stale copy: for a
     vidmem semaphore, check that the guest's BAR1 read of that PA goes to VRAM and not to a
     cached window of the store (`fb.rs:518-520` flushes writes; reads are the store's).
   - value unchanged, no Xid: executed up to a point, or the release went elsewhere (VA mapped
     to a different backing in the shadow VAS than in the guest root: compare the shadow's
     `mapped` entry for that VA, `shadow.rs:194-213`, with the guest walk).
6. **Semaphore landed and the wait is blocking-mode: was the interrupt lost?** Only relevant
   under `CU_CTX_SCHED_BLOCKING_SYNC` or if the push carries `NON_STALL_INTERRUPT`/
   `AWAKEN_ENABLE`. With the host desktop firing GR0 continuously this is practically
   impossible to lose; to test it in isolation, trigger vector 4 by `LEAF_TRIGGER`
   (`intr.rs:221-232`, logs "leaf 0 bit 4 (enabled ..., top ... & en ...)") and check that
   `enabled` is true and top bit 0 is in `top_en`.

## 5. What the device must do for GR completion

1. **Restore the guest's GPGet/GPPut in the USERD after the host RM clears it**, before the
   first doorbell. Done: `mirror.rs:360-377`, `:531-540` (shadow), `:1181-1195` (CE). Proven
   by `d-gr-launch5` ("GPGet Some(1) GPPut Some(1)").
2. **Forward the guest's doorbell to the host token.** Done: `mirror.rs:1245-1265`, via
   `ctrl.rs:188-203`.
3. **Every set-A page a GR push can write is mapped in the shadow VAS, at the same VA and the
   same backing, before the GPU reaches it.** Partly done: initial sync at the first doorbell
   (`mirror.rs:398-408`), resync at each guest MMU invalidate (`boot.rs:383-404`, not yet run
   when this was written). Not done: the doorbell-time resync runs after the ring
   (`mirror.rs:323-330`; it should run before `self.doorbell(chid)`), and refused leaves are
   silently dropped (`shadow.rs:148-156`); log them with VA and reason. The
   `d-gr-launch5` fault VA 0x100_04e00000 must be explained by one of these before the next run.
4. **Semaphore visible to the guest CPU.** Done by construction for sysmem semaphores (the GPU
   writes guest RAM) and for reservation VRAM read through BAR1, provided item 3 maps the
   semaphore VA to the same PA the guest's root does. Not verified for GR: no run has read a
   GR semaphore back (§4 step 5).
5. **Register the host's GR0 non-stall event (`NV2080_NOTIFIERS_GR0 | NV01_EVENT_NONSTALL_INTR`,
   REPEAT) on the host subdevice.** Done: `mirror.rs:654-721`, `hostmem.rs:667-712`; fires
   (`d-gr-launch5`: 8316 events).
6. **Raise guest vector 4 (leaf 0 bit 4) and the MSI when that event fires.** Done:
   `mirror.rs:1752-1759`, `intr.rs:59-81`; table entry `interrupts.toml:30`.
7. **Emulate the tree faithfully for the guest's ISR**: `TOP` derived from `LEAF & LEAF_EN`,
   W1C `LEAF`, `LEAF_EN_SET/CLEAR`, `TOP_EN_SET/CLEAR`. Done: `intr.rs:138-237`.
8. **No NV_PGRAPH status/clear register emulation for the non-stall path.** Nothing to do:
   `kgraphicsNonstallIntrCheckAndClear` is a stub (`G/g_kernel_graphics_nvoc.h:995-997`).
9. **Leave the guest's event objects to the guest**: echo the `GSP_RM_ALLOC 0x79` of the
   GR0/CE non-stall events. Done (trace lines 9036-9216 of `d-gr-launch4`, synthesized echo).
10. **Never raise the GR0 stall vector 196.** Done (nothing raises it).
11. **Reflect a host RC of the GR shadow channel to the guest** (give the shadow channel the
    guest's error notifier page as `hObjectError`, as the CE path does, and/or send the guest
    an `RC_TRIGGERED` event), so a fault becomes a CUDA error and not a hang. **Not done**:
    `mirror.rs:453-458` sets no `hObjectError`; the RC event is only counted
    (`mirror.rs:1761-1779`). Consequence to keep in mind: once the host token is written into
    the guest's notifier page, libcuda will ring the host token, which `ctrl.rs:180-185`
    already handles.
12. **A GR push diagnostic on the shadow path** (entry, pushbuffer, host and compute semaphore
    methods, semaphore read-back, §4 step 5). **Not done**: `ctrl.rs:188-203` returns before
    `mirror_push_diag`, which decodes CE methods only.
13. **Treat the 1 Hz `MC_SERVICE_INTERRUPTS` as the stuck-wait probe** in the run summary
    (count, first time). Not done; today it is only visible by grepping the trace.
14. **Compute-object events (C24) via `POST_EVENT`.** Not done and probably not needed for
    `sched=auto`; open (cuda-lookahead.md §7). Revisit only if a run reaches a correct
    semaphore and still waits.
