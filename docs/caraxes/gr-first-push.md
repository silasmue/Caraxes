<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# The first GR push of cuCtxCreate on the shadow VAS: content, wait, and why it stalls

Read-only analysis (2026-09-23) of the E5b part 2 GR runs `d-gr-c2`, `d-gr-shadow`..`d-gr-shadow4`
and `d-gr-launch`..`d-gr-launch5` under `caraxes/runs/e5b2-2026-09-23/`, the software-mode capture
`kernel-bytes/kernel-bytes.txt`, and the device crate. Short names: `R/` is
`caraxes/runs/e5b2-2026-09-23/`, `D/` is `crates/vfio-user-nvgpu/src/`, `C/` is
`vendor/open-gpu-kernel-modules/src/common/sdk/nvidia/inc/class/`. Line numbers in `D/` are those
of the working tree on 2026-09-23 (the crate is being edited concurrently, `shadow.rs` is still
untracked), so they can drift.

## 0. Summary

- The GR pushes are never logged by the host path, but the software executor decodes the very same
  push when GR stays in software (`d-gr-shadow4`), so the first push is known method by method
  (section 1). It is a pure state-setup push of 169 dwords ending in a four-word report semaphore
  release of payload 1. It has no kernel launch, no QMD and no inline-to-memory upload.
- In `d-gr-launch`..`d-gr-launch4` the first push was never fetched. The host RM cleared the
  channel's USERD when the shadow channel was created, and the shadow path did not restore the
  guest's GPPut, so the host USERD read GPGet 0 / GPPut 0 after the doorbell
  (`R/d-gr-launch4/device-trace.txt:9324`). Nothing ran, so nothing raised an RC and the guest
  hung (section 2).
- In `d-gr-launch5` the restore was in place (`R/d-gr-launch5/device-trace.txt:6761`), the GR
  PBDMA fetched the entry (GPGet 1 / GPPut 1, `:6763`), and the guest went on past the eight GR init
  pushes to its next CE and GR work (`:8175`-`:8545`), the same order as in software mode. **So the
  first GR push completes on the host-built context.** The run then died on the third push of
  chid 2, an inline-to-memory write to VA 0x100_04e00000 that faulted with FAULT_PDE
  (`R/d-gr-launch5/host-xid.txt:1`), a VA that the shadow VAS never received, most likely because
  the set-A leaf walk is capped at 16384 leaves (section 5). The guest hung instead of returning
  719 because the shadow channel has no error notifier (section 2.3).

## 1. What the first GR push contains

### 1.1 Where it comes from

cuCtxCreate allocates eight GR channels (guest chids 2-9, one TSG) and gives each the same init
push. In the software-mode run `d-gr-shadow4`, guest chid 2 rings doorbell 20 and the executor
reads GPFIFO entry 0: `pushbuffer va 0x200400000 169 dwords` (`R/d-gr-shadow4/device-trace.txt:9313-9314`).
Chids 3-9 follow with 169-dword pushes at 0x200800000, 0x200c00000 and so on (`:9463-9464`,
`:9548-9549`, ..., `:9974-9975`), each releasing its own semaphore 16 bytes below the previous one
(`:9546`, `:9631`, ..., `:10057`). The host path of the launch runs does not decode GR pushes (the
trace has no pushbuffer lines, only the one-shot USERD read, `D/mirror.rs:546`), so this software
decode is the evidence for the push content; the bytes are the same because the guest writes the
same pushbuffer whichever path executes it.

The executor logs each method it does not model as `compute method <addr> = <data> skipped`, logs
`SET_OBJECT` and the report semaphore, and is silent for host NOP (0x08), host WFI (0x78), the host
semaphore address/payload registers without an execute, and the compute NO_OPERATION,
WAIT_FOR_IDLE, INVALIDATE_SHADER_CACHES, I2M spares, SEND_PCAS_A, SET_SPA_VERSION and the
report-semaphore A/B/C registers (`D/channel.rs:864-866`, `:912`, `:1034`, `:1054`, `:1113-1118`). The 82 logged lines
(`R/d-gr-shadow4/device-trace.txt:9315-9396`) account for 85 method data dwords; the other 84 of
the 169 dwords are method headers plus any of those silent methods, which the log cannot tell
apart. There is no host `SEM_EXECUTE` in the push (the executor would log it, `D/channel.rs:968-996`).

### 1.2 The decoded push

| # | Method (subch) | Data | Meaning | What it needs to complete |
|---|---|---|---|---|
| 1 | host `SET_OBJECT` 0x000 (1) | class 0xc5c0 | bind `TURING_COMPUTE_A` to subchannel 1 (`C/clc56f.h:68`, trace `:9315`) | a compute object of that class on the channel, i.e. the class in the channel's GR context; FECS must have loaded the channel's context (the MAIN image) before FE accepts class methods |
| 2 | `SET_SHADER_SHARED_MEMORY_WINDOW_A/B` 0x2a0/0x2a4 | 0x7500 / 0xf5000000 | shared-memory window base 0x7500_f500_0000 (`C/clc5c0.h:340-344`, trace `:9316-9317`) | nothing in memory: it is an aperture in the VA space, not backed; it must only not overlap a mapping the kernels use |
| 3 | `SET_CWD_REF_COUNTER` 0x248, 64 times | 0x4803f .. 0x48000 | compute work distributor reference counter select 0x3f..0x00, value 0x480 (`C/clc5c0.h:268-270`, trace `:9318-9381`) | GR register state only |
| 4 | `SET_RESERVED_SW_METHOD07` 0x23c | 1 | driver-defined software state (`C/clc5c0.h:259`, trace `:9382`) | handled inside GR/firmware; the open kernel RM has no handler for it (no SW-method code under `src/nvidia/src/kernel/gpu/gr/`), so on this GSP system any trap would be serviced by GSP-RM |
| 5 | `SET_RESERVED_SW_METHOD10` 0x254 | 1 | as above (`C/clc5c0.h:278`, trace `:9383`) | as above |
| 6 | `SET_VALID_SPAN_OVERFLOW_AREA_A/B/C` 0x200-0x208 | 2 / 0 / 0x90000 | the valid-span overflow buffer at VA 0x2_0000_0000, 0x90000 bytes (`C/clc5c0.h:203-210`, trace `:9384-9386`) | a mapping of 0x2_0000_0000 at the time the SMs spill into it (not when the method executes); it is a guest 2 MiB vidmem page that the shadow VAS maps (`R/d-gr-shadow2/server-stderr.txt:12`) |
| 7 | `SET_SHADER_LOCAL_MEMORY_NON_THROTTLED_C` 0x2ec | 0x12 | MAX_SM_COUNT 18 for the non-throttled local-memory sizing (`C/clc5c0.h:383-384`, trace `:9387`) | register state; the local-memory base (`SET_SHADER_LOCAL_MEMORY_A/B`, 0x790/0x794, `C/clc5c0.h:495-499`) is not set in this push, nor anywhere in `d-gr-shadow4` (no 0x790 line in that trace) |
| 8 | `SET_RESERVED_SW_METHOD09` 0x250 | 0 | driver software state (`C/clc5c0.h:275`, trace `:9388`) | as row 4 |
| 9 | host `SET_OBJECT` 0x000 (4) | class 0xc5b5 | bind `TURING_DMA_COPY_A` to subchannel 4 of the GR channel (trace `:9389`) | a copy object on the channel; the host allocated both classes on the shadow channel (`R/d-gr-launch5/device-trace.txt:6758-6759`) |
| 10 | `SET_TEX_HEADER_POOL_A/B/C` 0x1574-0x157c (1) | 0x100 / 0 / 0xfffff | texture header pool at VA 0x100_0000_0000, max index 0xfffff (`C/clc5c0.h:581-587`, trace `:9390-9392`) | nothing at method time; the pool must be mapped when a kernel first fetches a texture header |
| 11 | `SET_TEX_SAMPLER_POOL_A/B/C` 0x155c-0x1564 (1) | 0x100 / 0x2000000 / 0 | sampler pool at VA 0x100_0200_0000 (`C/clc5c0.h:572-578`, trace `:9393-9395`) | as row 10 |
| 12 | `SET_REPORT_SEMAPHORE_A/B/C/D` 0x1b00-0x1b0c (1) | VA 0x2_03c0_fff0, payload 1, four-word, release | the end-of-push tracking semaphore (`C/clc5c0.h:622-639`, trace `:9396`); the executor wrote it with a timestamp and no "awaken requested" note, so the push does not ask for a non-stall interrupt (`D/channel.rs:1163-1206`) | FE must drain the preceding state methods, then write 16 bytes through the channel's VAS at 0x2_03c0_fff0: the page must be mapped in the VAS the GR channel runs on |

The first push is therefore state only: `SET_OBJECT`, windows, pools, counters, three reserved
software methods, and one semaphore release. The launch material in `kernel-bytes.txt` (the SASS
and constant-buffer uploads, `R/kernel-bytes/kernel-bytes.txt:79-84`, and the inline QMD with
`release0 va 0x203c3ff7c payload 0x6`, `:87`) belongs to a much later push of the vecAdd launch, not
to cuCtxCreate's first push. What the push needs from the context is only that the context is loaded
and holds the compute class; what it needs from memory is the GPFIFO entry, the 169-dword pushbuffer
at 0x2_0040_0000 and the semaphore page.

### 1.3 What follows on chid 2

After the eight init pushes and a burst of CE pushes on chid 10, chid 2 continues with small pushes
(`R/d-gr-shadow4/device-trace.txt:10231-10259`): gp 1, 3, 4 and 6 are
`INVALIDATE_SHADER_CACHES_NO_WFI` (0x1698, `C/clc5c0.h:590`) plus a semaphore release of payload 2,
4, 5 and 7; gp 2 is an inline-to-memory upload of 128 bytes to VA 0x100_04e0_0000 plus payload 3
(`:10238-10239`); gp 5 is a 256-byte upload to 0x100_04e0_0900 (`:10253`). The same uploads appear in
the list of `kernel-bytes.txt` (`R/kernel-bytes/kernel-bytes.txt:13-14`). These matter because gp 2 is
the push that faulted in `d-gr-launch5` (section 5).

## 2. What the guest waits on

### 2.1 The semaphore and USERD

The CUDA driver's completion signal for each GR push is the report semaphore at the end of it: chid 2
releases 0x2_03c0_fff0 with payloads 1, 2, 3, ... (`R/d-gr-shadow4/device-trace.txt:9396`,
`:10234-10259`), chid 3 releases 0x2_03c0_ffe0 (`:9546`), down to chid 9 at 0x2_03c0_ff80
(`:10057`). The CE channels' init semaphores lie in the same 4 KiB page: chid 18 releases
0x2_03c0_fef0, chid 21 releases 0x2_03c0_fec0 (`R/d-ce-diag/device-trace.txt:9236`, `:9274`). That
page is a valid guest sysmem mapping (aperture SYS_COH) in the guest's own page tables, and the
physical CE wrote payload 1 plus a timestamp into it (`R/d-ce-diag/device-trace.txt:9238`). So the GR
semaphore address is in set A and mapped in the guest root. The release does not request an
awaken, so completion is observed by polling that memory, not by an interrupt (inference from the
executor's decode, `D/channel.rs:1163-1206`; that CUDA polls the payload rather than GPGet is an
inference from the payload sequence, not observed).

The USERD fields are the other half: the guest writes GPPut (USERD 0x8C) and rings the doorbell with
its work-submit token; GPGet (USERD 0x88) is advanced by the PBDMA when it fetches an entry
(`D/mirror.rs:359-376`, the offsets the shadow path saves). On the shadow channel the doorbell is
rewritten to the host token (`R/d-gr-launch5/device-trace.txt:6760-6762`).

That the guest really blocks on the GR init pushes is visible from the order of doorbells. In software
mode it rings chids 2-9, then chid 10 (CE) about 366 ms later, then chid 2 again
(`R/d-gr-shadow4/device-trace.txt:9313`, `:10124`, `:10231`). In `d-gr-launch4`, where no GR push was
fetched, the last doorbell of the whole run is chid 9's first (`R/d-gr-launch4/device-trace.txt:9717`):
the guest never reached its next CE work.

### 2.2 Was the entry fetched?

| Run | GPGet/GPPut after the first doorbell | Outcome |
|---|---|---|
| `d-gr-launch`..`d-gr-launch3` | not logged; no "restored" line on any GR chid | 8 GR doorbells, then silence, guest timeout (`R/d-gr-launch/guest.txt:2`, `R/d-gr-launch2/guest.txt:2-3`) |
| `d-gr-launch4` | GPGet 0 / GPPut 0 on all eight GR chids (`R/d-gr-launch4/device-trace.txt:9324` and following) | never fetched, no RC, guest timeout rc 124 (`R/d-gr-launch4/guest.txt:2`) |
| `d-gr-launch5` | guest GPGet 0 / GPPut 1 restored (`:6761`), then GPGet 1 / GPPut 1 (`:6763`), same for chids 3-9 (`:6943`, `:7004`, ...) | fetched and completed; the guest proceeds to chid 10 and chid 2 gp 1-2 (`:8175-8545`) |

GPPut 0 is decisive on its own: with GPGet equal to GPPut the PBDMA has nothing to fetch, whatever the
state of the context. The cause is the same one found for the CE mirror in a1 (the host RM clears the
USERD it is given, `docs/caraxes/experiments.md:968`); the CE path restored the guest's values from a2
on (`R/d-gr-launch4/device-trace.txt:9232`), the shadow path only from `d-gr-launch5`
(`D/mirror.rs:359-365`, `:531-541`, whose comment records the GPGet 0 / GPPut 0 hang). The earlier
caveat that an immediate read-back can show GPGet 0 even though the PBDMA fetched later
(`docs/caraxes/experiments.md:981`) does not apply here, because GPPut itself read 0.

### 2.3 Hang or 719

CUDA's wait has no timeout of its own within the 60-75 s the probe allows: the GR host channels of the
hanging runs lived 37-52 s until the probe was killed (`R/d-gr-launch/device-trace.txt:29923`,
`R/d-gr-launch4/device-trace.txt:24018`). cuCtxCreate returns 719 only when the host RM reports an RC
through the channel's error notifier: in `d-gr-c2` the CE-style mirror passes the guest's error
notifier page (`R/d-gr-c2/device-trace.txt:8800`, "error notifier the guest's page"), the GR channel
RC'd (Xid 38, `R/d-gr-c2/host-xid.txt:1`) and the guest printed `cuCtxCreate: 719`
(`R/d-gr-c2/guest.txt:2`). The shadow channel is allocated without `hObjectError` (`D/mirror.rs:453-458`,
compare the CE path at `:1047-1073`), so in `d-gr-launch5` the Xid 31 RC (`R/d-gr-launch5/host-xid.txt:1`,
exceptType 0x1f = `ROBUST_CHANNEL_FIFO_ERROR_MMU_ERR_FLT`, `vendor/open-gpu-kernel-modules/src/common/sdk/nvidia/inc/nverror.h:49`) never reached the guest,
which kept ringing chid 2 (`R/d-gr-launch5/device-trace.txt:8580`, `:8618`) and timed out
(`R/d-gr-launch5/guest.txt:2`).

## 3. Does the first push assume the guest's own context initialization?

Under the shadow design (variant c1s) the host RM builds the GR context from its own golden image
when the device allocates `TURING_COMPUTE_A` on the shadow channel, and the guest's promoted buffers
are excluded from the mirror (`D/mirror.rs:343-355`, `docs/caraxes/exec-formats.md` section 4.5). The
guest promoted four entries: MAIN at VA 0x1_2001_0000 (0xee000 bytes), PATCH at 0x1_200f_e000 (0x5000),
FECS_EVENT at 0x1_2000_0000 and PRIV_ACCESS_MAP at 0x1_2011_0000 (buffer ids 0, 2, 9, 10;
`R/d-gr-launch5/device-trace.txt:1735`, ids from `vendor/open-gpu-kernel-modules/src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080gpu.h:903-913`). Item by item:

- **Class in the context.** `SET_OBJECT(0xc5c0)` and `SET_OBJECT(0xc5b5)` need those objects on the
  channel. The host allocated both with the guest's parameters (`R/d-gr-launch5/device-trace.txt:6758-6759`).
  Satisfied.
- **Golden image.** The guest RM's golden context was produced at module load on the emulated channel
  and is never loaded by the hardware here; FECS loads the host's MAIN image. Nothing in the first push
  reads or patches the context image, and `d-gr-launch5` shows the push completing on the host image
  (section 2.2). Not a factor for the first push.
- **Local and shared memory.** The push sets only the shared window and the non-throttled SM count
  (rows 2 and 7); the local-memory base and size are set later by the driver, not by the RM's context.
  Not a factor for the first push. The value 18 in `SET_SHADER_LOCAL_MEMORY_NON_THROTTLED_C` must agree
  with the physical floorsweeping at the first kernel that uses local memory; the guest RM was given the
  host's floorsweeping masks (`docs/caraxes/experiments.md:883`).
- **PATCH buffer, PRIV_ACCESS_MAP, FECS_EVENT.** FECS uses the host's own buffers, mapped by the host
  RM in the shadow VAS at VAs it chose. The guest's VAs for them are excluded from set A, so the two
  do not overlap as long as the host's choices do not land on a set-A VA; the sync that runs before the
  host allocation reports 0 run collisions in the launch runs (`R/d-gr-launch5/device-trace.txt:6756`).
  The exclusion covers the guest's ids 9 and 10 with 4 KiB only (their recorded size is 0,
  `D/mirror.rs:350`); that is harmless for the first push.
- **FECS-trapped methods.** The push carries three `SET_RESERVED_SW_METHOD` writes (rows 4, 5, 8) and
  no method that needs a guest-mapped ctxsw buffer (no `SET_SHADER_LOCAL_MEMORY`, no preemption or
  debugger method). If a reserved SW method trapped, the servicing is in GSP-RM on the host, against
  the host context, which is consistent.

Conclusion: the first push assumes nothing that only the guest's own context initialization would
have produced. The differences between guest and host context matter later, if at all (preemption
mode and buffers, debugger state, anything the guest reads back from its own MAIN/PATCH), and none of
them is exercised before the first kernel launch.

## 4. The comparison case: the CE channels

The CE first push is 8 dwords: `20018000 0000c5b5 20038090 00000002 03c0fef0 00000001 200180c0 00000014`
(`R/d-ce-diag/device-trace.txt:9236`), i.e. `SET_OBJECT(TURING_DMA_COPY_A)`, `SET_SEMAPHORE_A/B/PAYLOAD`
(0x240-0x248, VA 0x2_03c0_fef0, payload 1, `C/clc5b5.h:39-43`) and `LAUNCH_DMA` 0x14 (0x300: no data
transfer, flush enabled, four-word semaphore release, `C/clc5b5.h:68-82`). The physical CE wrote the
payload and a timestamp into guest sysmem (`:9238`). Why that completes and what is structurally
different for GR:

1. **Page tables.** The CE channels run on the externally owned mirror VAS rooted at the guest's own
   page directory (`R/d-gr-launch5/device-trace.txt:6663`), so every guest mapping is visible to them
   after a TLB invalidate. The GR channel cannot schedule there (b1, `docs/caraxes/experiments.md:971`)
   and a replayed VAS puts the host context out of reach (b2/c2, `:972`, `:991`), so GR runs on a
   **copy** of set A. A copy can be incomplete or stale; the CE view cannot. This is the difference that
   bit `d-gr-launch5` (section 5).
2. **Engine context.** CE has no context image for FECS to load; its first schedule needs only the
   channel's RAMIN. GR needs FECS to load the MAIN image from the RAMIN's engine context pointer, with
   PATCH and PRIV_ACCESS_MAP reachable (`docs/caraxes/exec-formats.md` section 4.5 table). On the shadow
   VAS the host RM owns and maps all of these, which is why the first GR push now completes.
3. **USERD restore and error notifier.** The CE path restored the guest's GPGet/GPPut
   (`R/d-gr-launch4/device-trace.txt:9232`) and mirrors the error notifier (`D/mirror.rs:1047-1073`);
   the shadow path lacked the restore until `d-gr-launch5` and still lacks the notifier (section 2).
4. **Non-stall interrupts.** Neither init push asks for an awaken; the guest polls memory. The GR0
   non-stall notifier the device forwards to guest vector 4 is subdevice-wide: in `d-gr-launch4` it
   fires at `:9235`, before any GR host channel exists (`:9316`), so it reports the host desktop's GR
   work, not the guest's. It cannot be used as a completion signal for the guest's GR channel, and it
   was not needed for the CE completions either.
5. **Runlist.** The GR channels sit on runlist 0 with the host desktop's graphics work
   (`R/d-gr-launch5/device-trace.txt:6760`); the CE channels on runlists 8-10 (`:6668`, `:6701`, `:6729`).
   Time-slicing with the compositor delays but does not block a push; no evidence points at it.

## 5. Why the third push of chid 2 faulted in `d-gr-launch5`

The fault is `ENGINE GRAPHICS HUBCLIENT_FE faulted @ 0x100_04e00000 ... FAULT_PDE ACCESS_TYPE_VIRT_WRITE`
(`R/d-gr-launch5/host-xid.txt:1`), right after the third chid 2 doorbell (`device-trace.txt:8545-8546`).
FE performs inline-to-memory writes, and gp 2 is exactly the 128-byte upload to 0x100_04e0_0000
(section 1.3). FAULT_PDE means no directory entry existed for that VA in the shadow VAS, i.e. the leaf
was never mirrored.

The shadow sync walks the guest root in ascending VA order and stops at 16384 leaves
(`D/shadow.rs:146`, `D/fb.rs:673-706`). The filter then drops exactly the guest's promoted set-B pages:
0xee000/4 KiB = 238 MAIN pages, 5 PATCH pages, one leaf each at the FECS_EVENT and PRIV_ACCESS_MAP VAs,
245 in all. The sync at the second chid 2 doorbell reports `16139 set-A leaves`
(`R/d-gr-launch5/device-trace.txt:8485`), and 16139 + 245 = 16384, the cap. The same arithmetic holds
for the older cap: `7947 set-A leaves` in `d-gr-launch` and `d-gr-shadow4`
(`R/d-gr-launch/device-trace.txt:9317`, `R/d-gr-shadow4/device-trace.txt:9400`) is 8192 - 245. So the walk
was truncated, and the leaves it never reached are the highest VAs: the 0x100_xxxx_xxxx region
(PD2 index 4) comes after every leaf at 0x2_xxxx_xxxx (PD2 index 0). The texture pools and the I2M
targets of the launch live there (rows 10-11, section 1.3). The guest mapped the target before that
sync (the last invalidates before it are at `:8480`, `:8483`, none between the second and third
doorbell), so the truncation, not timing, is the likely reason it was missing. A second, smaller gap:
on an already-built channel the device rings the doorbell first and resyncs afterwards
(`D/mirror.rs:322-329`), so a page mapped since the last walk is mirrored only after the PBDMA may
already need it; the working tree now also resyncs at each guest TLB invalidate (`D/boot.rs:394-404`).

## 6. Ranked reasons a GR push on the shadow channel does not complete

1. **The host RM's USERD clear leaves GPPut 0 (confirmed, fixed in `d-gr-launch5`).** This is the
   stall at the first push in `d-gr-launch`..`d-gr-launch4`. Cheapest check: the existing one-shot
   `USERD after doorbell` line; GPPut must equal the guest's GPPut (it read 0 in
   `R/d-gr-launch4/device-trace.txt:9324`, 1 in `R/d-gr-launch5/device-trace.txt:6763`).
2. **Set A only partly mirrored because the leaf walk hits its cap.** Explains the Xid 31 FAULT_PDE at
   0x100_04e00000 on the third push in `d-gr-launch5`, and will hit every high-VA pool and upload.
   Cheapest check: in the existing traces, `sync N set-A leaves` + 245 equal to the cap (true at
   `R/d-gr-launch5/device-trace.txt:8485`); on the next run, log whether `enumerate_leaves` returned
   exactly `max` and the last VA it reached, or translate 0x100_04e00000 through the guest root
   (`FakeFb::walk_in`) at the third doorbell.
3. **An RC on the shadow channel is invisible to the guest (no error notifier).** Turns every GR fault
   into a hang instead of 719 and hides which push failed. Cheapest check: after the next
   `HOST RC EVENT` on a shadow channel, read the guest's error-notifier page (its sysmem IOVA is in the
   channel alloc line, e.g. `R/d-gr-launch5/device-trace.txt:5900` for chid 6): all zero confirms.
4. **Doorbell before resync.** A page the guest maps between two doorbells without an intervening
   invalidate, or mapped after the last resync, is absent when the PBDMA fetches. Cheapest check:
   compare the timestamp of the first fault with the last `shadow: sync` line; a fault VA that a later
   sync maps confirms it.
5. **Host-built context disagreeing with the guest's assumptions at the first kernel launch** (local
   memory sizing with MAX_SM_COUNT 18 against the physical SMs, preemption mode, anything the guest
   reads from its own MAIN/PATCH). Not a first-push cause: `d-gr-launch5` completed the init pushes on
   the host image. Cheapest check: a GR exception (Xid 13) rather than an MMU fault at the first
   `SEND_PCAS`/inline-QMD push once 2-4 are fixed.
6. **A set-B VA collision between the host RM's own context buffers and a mirrored set-A run.** Would
   make FECS or the pipeline fault on a context buffer. Cheapest check: the `run/collision-failed`
   counter of the sync line (0 in `R/d-gr-launch5/device-trace.txt:6756`, `:8485`) and any `VA collision`
   line (none in that run).
7. **Runlist 0 shared with the host desktop.** Only delays. Cheapest check: GPGet advancing on a
   second read of the USERD some hundred milliseconds after the doorbell.
