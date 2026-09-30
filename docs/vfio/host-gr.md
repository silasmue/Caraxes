<!-- SPDX-License-Identifier: MIT -->
# Host GR path: the PBDMA and GR-context walls (E5b part 2, host side)

Written 2026-09-23. This is **analysis, not measured results**: the live host-GPU
experiments this note was scoped to run (build a guest-shaped GPFIFO/pushbuffer and
find what the physical PBDMA rejects; a GR context on an externally-owned VAS; replay
the guest's kernel launch) were **not executed**. Why, and what a supervised run needs,
are in section 1. Everything below is grounded in evidence already in the tree:
`caraxes/runs/e5b2-2026-09-23/` (the vm2 device's b2/b2b runs),
`caraxes/runs/e5c-2026-09-23/c5-core-pass-jit-wall/` (the guest's real launch),
[host-exec.md](host-exec.md) (the standalone execution primitive), and
[memory-model.md](memory-model.md). Driver citations are relative to
`vendor/open-gpu-kernel-modules/` (tag 610.57.04).

## 1. Why this is analysis, not results

The tasked experiments are privileged host-GPU submissions (channels, doorbells,
`DMA_SET_PAGE_DIRECTORY`, GR context) that the task itself expects to raise Xids/RCs.
They were held for the operator's own go-ahead, for four reasons that all point the same
way:

1. **Consent.** Kickoff rule 1 and the operator's standing rule gate any touch of host
   GPU state on the operator asking/being asked. A relayed "the operator accepts
   contained RCs tonight" is not that go-ahead; only the operator's own word is.
2. **This escalates past the approved envelope.** The one sanctioned privileged model,
   host-exec (D12), stopped at **Stage F**: a bare externally-owned GR channel with a
   hand-picked promote address returned `NV_ERR_OBJECT_NOT_FOUND` and the run stopped
   there by design. Goals 2-3 push through that wall (real GR context, real kernel
   replay). That is a new step, not a repeat of an approved one.
3. **A second concurrent Xid source on the live card.** The vm2 device agent is already
   raising `Xid 38` on GR mirror channels and `Xid 32`/PBENTRY on CE mirror channels on
   this exact card (section 2). The desktop and the agent session run on this same card
   (kickoff rule 1). Adding a second stream of privileged, fault-producing submissions
   raises the chance of an **uncontained** fault that takes the desktop and the vm2 run
   down with it.
4. **Containment cannot be assumed in advance.** The accepted-tonight policy is "stop at
   the first Xid not contained to your own channels." Whether the GR-context and
   kernel-replay steps stay contained is exactly what is unknown; on a shared live
   desktop that is the operator's call to make with eyes on the card.

The value the experiments were meant to produce is delivered here as evidence-based
rules and a ready-to-run test matrix, so a supervised session executes directly.

## 2. The two walls, named from the evidence

Both walls are already reproduced by the vm2 device, contained to its process
(`pid=3610120`), teardown clean (`b2-teardown.txt`: every free NV_OK, 9 host RC events,
no host-wide fault).

**Wall A - PBDMA / PBENTRY on the CE path (SOLVED by vm2, 2026-09-23).** `ROBUST_CHANNEL_PBDMA_ERROR = 32`
(`nverror.h:50`); PBENTRY is the PBDMA subcase "the fetched GPFIFO entry / pushbuffer
method stream is invalid." Surfaced to CUDA as `719 unspecified launch failure`
(`b2b-gate-cuda-xid32/verdict.json`). The physical PBDMA could not fetch the pushbuffer
that the guest's `GPFIFO[0]` pointed at, **although the device's software walker resolved
the same tables**. Cause, confirmed by vm2: **stale write-combined BAR1 data** - the
device wrote GPFIFO entries, USERD and page tables through the BAR1 windows and rang the
host doorbell before the WC lines drained. **Fix: an `sfence` plus a window read-back
before every host doorbell and every TLB invalidate.** With it, CUDA's CE channels take
their doorbells on the physical GPU with zero Xid. This confirms hypothesis H1 (section 3)
exactly; the H1 rule below is now a measured rule, not a prediction.

**Wall B - GR exception on the GR path.** `Xid 38` on each of 8 GR mirror channels at its
first push, class `0000c597` = TURING_A (`b2-host-xid.txt`); device trace records it as
`notify 0x25 exceptType 0x26 info16 0xe9` (`b2-mirror-trace-excerpt.txt`). A GR channel
whose GR context RM never built faults the moment the compute class touches it. This is
the host-exec Stage-F refusal seen from the other side: `GPU_PROMOTE_CTX` on a bare
externally-owned GR channel is `NV_ERR_OBJECT_NOT_FOUND` (host-exec.md Stage F).

The guest's page-directory shape at both walls, copied verbatim from the device trace
(`b2-mirror-trace-excerpt.txt`), is the reference every host-side table must match:

```
vaspace root Fb(0xE9000000); va 0x100000000..0x11fffffff
levels [0xe9000000 +0x20  ap 1 shift 47   (PD3, dir root)
        0xe9001000 +0x1000 ap 1 shift 38  (PD2)
        0xe9002000 +0x1000 ap 1 shift 29  (PD1)
        0xe9003000 +0x1000 ap 1 shift 21] (PD0, dual PDE: big/small halves)
```

`ap 1` = VIDMEM; the roots are inside the reservation `[0xE9000000, 0x1E9000000)`
(memory-model.md; the vm2 profile is 4 GiB, so the reservation top is 0x1E9000000).
GPFIFO VAs sit at `0x200200000 + n*0x3000` (chid 2..9), i.e. above the mapped VAS
window - those pages resolve through sysmem PTEs (host IOVAs, D13), not vidmem.

## 3. Goal 1 - PBENTRY: ranked causes and the candidate rule set

The distinguishing fact is that the **software** walker resolves the VA but the
**hardware** PBDMA fetch does not. Only a few things differ between a software table
walk and a live PBDMA fetch. Ranked by how strongly the existing evidence supports them:

### H1 (leading) - write-combining not fenced before the doorbell
The device writes GPFIFO, USERD (GPPut), the pushbuffer and the page tables through the
**write-combined** BAR1 window, then rings the doorbell. host-exec already proved this
exact hazard on the *control* path: an un-flushed BAR1 write surfaced as
`NV_ERR_INVALID_ADDRESS (0x1e)` because "RM read the BAR1-written tables before the WC
flush" (host-exec.md Stage C, and the numbers table row "Channel alloc first try
(race)"). Its working path **flushes every table write by read-back** before the
channel/PDB calls. The doorbell path has no such RM-side retry: the PBDMA fetches
whatever is in VRAM/sysmem at that instant. If the GPFIFO entry or the pushbuffer's
first method header is still in the WC buffer (or partially landed), the PBDMA reads a
stale/zero/torn dword -> invalid entry -> PBENTRY. This matches "software resolve OK
(RM read after its own flush), hardware fetch NAK (no flush before the ring)."

Rule: **every BAR1 write to GPFIFO, USERD GPPut, pushbuffer and page tables is followed
by an `sfence` and a read-back of the last dword before the doorbell is rung.** This is
the explicit write-combining test the task calls for; H1 predicts "no fence -> PBENTRY,
fence -> accept."

### H2 - a sysmem pushbuffer/GPFIFO whose PTE carries the guest IOVA, not the host IOVA
The guest writes IOVAs into its own PTEs (D13). For any pushbuffer/GPFIFO page that lives
in **sysmem**, the PTE the hardware walks must hold the **host** IOVA of that guest page,
not the guest's. host-exec's working semaphore/CE pushbuffers were in sysmem at the host
IOVA (Stage B/D). If the mirror leaves a guest IOVA in a sysmem PTE, the software walker
(which may resolve against the device's own view) still "resolves," but the PBDMA fetches
host physical nowhere -> corrupt stream. Rule: **every sysmem address inside the tables
and inside the GPFIFO entry is translated GPA/guest-IOVA -> host IOVA before the fetch;
vidmem addresses in the reservation pass through unchanged** (host-exec section 2).

### H3 - PDE0/PTE PCF/aperture/page-size bits the hardware enforces and the walker ignores
The guest's PD0 is a **dual** PDE (shift 21, big + small halves). A pushbuffer mapped
through the big half (64 KiB) vs the small half (4 KiB) needs the matching sublevel
present and the matching page-size/aperture/volatile bits. host-exec's proven PTE
encoding is `VALID | (aperture<<1) | ((phys>>12)<<8)` with the dual-PDE small half for
PD0->PT (Stage B). A software walker that only checks VALID+aperture+address will
"resolve" a PTE whose PCF (privilege/atomic/volatile) or page-kind bits the physical
GMMU rejects. Rule: **copy the guest's actual PDE0/PTE bits (aperture, volatile, page
size, kind) byte-for-byte from what the guest wrote (readable in vm2 evidence or BAR1
read-only), do not re-synthesize them.**

### H4 (lower) - GPPut/USERD coherence
The device restores the guest's `GPGet 0 / GPPut 1` into USERD after RM cleared it at
channel creation (`b2-mirror-trace-excerpt.txt`). If that USERD write is itself in the
WC window when the doorbell rings, the PBDMA sees `GPPut == GPGet` (empty ring) - but
that yields a no-op, not PBENTRY, so H4 alone does not explain Wall A. It folds into H1
(fence USERD too).

### Candidate device rule set for Wall A (to be confirmed by the test matrix)
1. Fence (`sfence`) + read-back-flush GPFIFO, USERD GPPut, pushbuffer, and every touched
   page-table dword **before** the doorbell.
2. Translate every sysmem address (GPFIFO entry target, pushbuffer VAs' sysmem PTEs) to
   the host IOVA; pass vidmem/reservation addresses through unchanged.
3. Mirror the guest's PDE0/PTE bits verbatim (aperture, volatile, page size, kind); do
   not re-derive them.
4. Absorb the documented first-launch stall on a fresh channel (host-exec open item 3:
   one RC, or a priming push).

### Test matrix a supervised run should execute (each an evidence file under
`caraxes/runs/host-gr-2026-09-23/`)
| # | GPFIFO | pushbuffer | fence before doorbell | predicted |
|---|---|---|---|---|
| T1 | vidmem (reservation) | vidmem | yes | accept (baseline, closest to host-exec Stage D) |
| T2 | vidmem | vidmem | no (skip sfence + read-back) | PBENTRY if H1 |
| T3 | vidmem | sysmem, host IOVA PTE | yes | accept |
| T4 | vidmem | sysmem, guest IOVA PTE (unstranslated) | yes | PBENTRY if H2 |
| T5 | guest PD0 big half (64 KiB) | as guest wrote | yes | isolates H3 |
Payload for all: a single SEM release (host-exec Stage D methods), semaphore in sysmem
at a host IOVA, expect the page to read the payload and GPGet to advance. Measure latency
and read `/dev/kmsg` for Xid after each.

## 3b. New wall (redirected 2026-09-23) - CE write INTO the reservation stalls

With PBENTRY solved, the next wall (host-exec.md open item 1) is: a CE `LAUNCH_DMA` that
**writes** the reservation (sysmem -> vidmem, the H2D half of `cuMemcpy`) **stalls**,
while CE **reads** of the reservation and CE writes to sysmem both work. On vm2, cuda-core
now times out on completion with **no Xid and no RC** - a clean hang, not a fault. This
was NOT reproduced standalone here (same live-run gate, section 1, plus a second BAR1/
reservation/doorbell tool running concurrently with vm2's active tool on one card would
compete for the same windows and reservation - host-exec already hit `NO_MEMORY` on the
top range because vm2's VMs held it). The analysis and the decisive-variable prediction:

**Predicted decisive variable: the destination PTE KIND vs the reservation's compression
state.** Grounds:
- The reservation is `NV01_MEMORY_LOCAL_USER`, `CONTIGUOUS | HUGE`, `COMPR` default
  **NONE** (host-exec.md Stage A, `attr=0x11800000`; memory-model.md section 4). No
  compression tag lines are allocated for it.
- Turing v2 PTEs carry a `COMPTAGLINE` field (`dev_mmu.h:117`,
  `NV_MMU_VER2_PTE_COMPTAGLINE (20+35):36`) and a client-kind field. Turing collapsed
  kinds to `PITCH 0x00`, `GENERIC_MEMORY 0x06` (uncompressed) and
  `GENERIC_MEMORY_COMPRESSIBLE 0x08 / _DISABLE_PLC 0x09` (`dev_mmu.h:98-106`). CUDA global
  surfaces use a **compressible** kind when the guest enabled compression.
- A compressible kind tells the L2/CE write path to look up and update the page's comptag
  line. Over a `COMPR NONE` allocation that comptag line does not exist, so a CE **write**
  drives the compression path into a resource that is not there and **stalls**; a CE
  **read** returns the raw bytes without touching comptag state, so it **works**. This is
  exactly the read-OK / write-stall asymmetry host-exec observed, and the clean hang (no
  Xid/RC) fits a stalled comptag lookup rather than a fault.
- The device already answers CUDA's decompression-LCE query "no decompression LCE on
  Turing" (`b1 .../cuda-init.trace:39`, ctrl `0x20802a12`), so the compression path is in
  play from init.

Why **not** the other candidates:
- **Page size (4 KiB vs 64 KiB big).** Not decisive: CE reads through the same 4 KiB PTE
  over the 2 MiB-HUGE allocation already work, so addressing a huge allocation with small
  PTEs is fine. Page size would break reads too if it were the cause.
- **RO / volatile / atomic bits.** An RO destination would fault (MMU write fault, Xid 31),
  not hang clean; volatile only affects L2 caching, not a stall.
- **LAUNCH_DMA flags (dst layout, semaphore, flush enable).** Hold them constant across
  the test; if pitch-vs-block-linear dst layout in the method (separate from PTE kind)
  mattered it would also affect sysmem writes, which work. Secondary at most.

**Fix (either):** (a) set the destination PTE **kind to an uncompressed kind**
(`GENERIC_MEMORY 0x06`, or `PITCH 0x00`) with `COMPTAGLINE = 0`, so kind and backing agree;
or (b) allocate the reservation with compression attributes (`NVOS32_ATTR_COMPR_*` +
comptag reservation) so the guest's compressible kind is backed. (a) is the device rule
(the mirror rewrites the destination kind for reservation writes); (b) changes the
reservation and costs comptag resource. Note the guest also uses these VRAM surfaces as
compute I/O, so (a) must not break the kernel's own reads of the same surface - if the
kernel relies on compression, (b) is required.

**One-shot confirm test** (the single measurement to run under supervision): identical CE
sysmem -> vidmem `LAUNCH_DMA` into the reservation, dst-layout / semaphore / flush flags
held constant, run twice:
- (A) dst PTE kind = the **guest's kind** (compressible; read the exact value from vm2's
  PTE dump through BAR1 read-only) -> predict stall/hang;
- (B) dst PTE kind = `GENERIC_MEMORY 0x06`, `COMPTAGLINE = 0` -> predict the bytes land.
Poll the destination through BAR1 for **10 s** in case (A) before calling it a stall
(rule out late landing). Decisive variable = kind if A hangs and B lands.

**Safety carve-out for the confirmation (added 2026-09-23).** Variant A is not safe to
run on the live desktop, for the same reason the `NVOS32_ATTR_COMPR` reservation variant
is off the table: the guest's compressible-kind PTE carries the guest's `COMPTAGLINE`
(PTE bits 55:36), a line number the guest's RM assigns from its own numbering as if it
owned the whole FB's comptag store. On the physical card that line number **aliases into
the host's global comptag store**, which the running desktop's compressed surfaces also
use. A CE **write** through such a PTE read-modify-writes that comptag line. Whether the
write **stalls** (the hypothesis) or actually **lands** in the comptag store is precisely
the unknown the measurement exists to settle - so running A on the live card risks
corrupting the host desktop's compressed surfaces. That effect is **global card state,
not contained to the tool's own channels**, so it fails the "stop at the first effect not
contained to your channels" envelope. Consequences:
- Variant A must not be run on the live desktop until the operator gives it at the card,
  with the desktop's own compressed surfaces out of harm's way (a headless/idle-display
  window). This applies to any client, vm2 included.
- Variant B (uncompressed kind 0x06, `COMPTAGLINE = 0`) and a read-only check are
  non-corrupting (no comptag write), but they do not settle A-vs-B on their own.
- The clean, isolation-safe device rule does not need A confirmed to be adopted: **the
  mirror rewrites every reservation-destination PTE to an uncompressed kind
  (`GENERIC_MEMORY 0x06` / `PITCH 0x00`) with `COMPTAGLINE = 0`**, so no guest comptag
  line number ever reaches the host's global store. That is both the fix and the
  isolation boundary; A would only tell us the failure mode of violating it.

## 4. Goal 2 - GR context on an externally-owned VAS

What the evidence already fixes:
- The VAS must be `FERMI_VASPACE_A` with `IS_EXTERNALLY_OWNED`, PDB set by
  `DMA_SET_PAGE_DIRECTORY` (admin) to the guest's root FB address **before** any channel
  is scheduled on it (host-exec Stage C; channel-before-PDB = `NV_ERR_INVALID_STATE`).
- A bare GR channel + `TURING_COMPUTE_A` allocates NV_OK, but the first push faults
  (Wall B / Xid 38), and `GPU_PROMOTE_CTX` with a hand-picked address is
  `NV_ERR_OBJECT_NOT_FOUND` (host-exec Stage F). So the missing piece is the **GR context
  RM expects to exist**: the server-reserved PDEs plus the global GR context buffers
  (main ctx, patch, circular buffer, pagepool, attribute cb, priv access map).
- The mechanism for the reserved range is `NV90F1_CTRL_CMD_VASPACE_COPY_SERVER_RESERVED_PDES`
  (`rpc.c:6193`, `rpcCtrlVaspaceCopyServerReservedPdes_v1E_04`): RM returns the PDEs it
  reserved for its own global buffers, and the client writes them into its page
  directory so the channel's PDB can reach those buffers.

Open question this goal answers (needs the live run): after
`VASPACE_COPY_SERVER_RESERVED_PDES` on the externally-owned VAS and writing the returned
PDEs into the guest-shaped directory, are the host's global GR context buffers reachable
through the guest's PDB (walk the reserved range through BAR1 read-only)? If yes, the
(c1) path - device records PROMOTE_CTX entries and lets RM build the context - is viable
on a guest-shaped VAS. If the reserved range collides with the guest's own VA layout
(the guest's `vaBase/vaSize/internal limit` from its `FERMI_VASPACE_A` alloc), that
collision is the reason Wall B persists, and the device must reconcile the guest's
reserved range with RM's. Record every status code and the exact privilege used.

## 5. Goal 3 - the guest's real launch, bytes extracted

From `caraxes/runs/e5c-2026-09-23/c5-core-pass-jit-wall/device/cuda-jit.trace:16971`
(the device's software model reaches this and marks it "KERNEL LAUNCH NOT EXECUTED
(needs the physical GPU) [execution wall]"). These are the exact inputs a replay uses,
adjusting only the VAs to the host-built context's mappings:

- **QMD VA:** `0x204208000`  - **grid:** `16 x 1 x 1`
- **release0:** semaphore VA `0x203c3ff7c`, payload `0x6`, enable bit 1 (the completion
  the SMs must write; in the software model it is "NOT written" - that write is the
  success signal a real run must produce)
- **QMD raw (64 dwords, little-endian, as captured):**
```
00000000 00000000 00000000 00000000 0000047f 3c000000 00000000 00000000
0005284a 00000000 00000000 44010000 00000010 00000001 00000001 00000000
00000000 22240000 01000023 00010001 00121083 00000000 00000000 03c3ff7c
80000002 00000006 00000000 00000000 00000000 08000000 00000640 75001201
04220000 0c4c0100 04010000 04100100 00000000 00000000 00000000 00000000
00000000 00000000 00000000 00000000 00000000 00000000 04000000 80000100
05284a00 00000100 00000000 00000000 00000000 00000000 00000000 00000000
00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000000
```
  Decoded landmarks: dwords 23-25 = release0 (`03c3ff7c`, `80000002` -> addr upper 2 +
  flags, payload `00000006`) match the VA/payload above; dword 8 `0005284a` and dword 48
  `05284a00` are the program/constant-buffer address low bits (VA family `0x...5284a00`).
- **CE semaphore releases** on the passing cuda-core path (four-word release, for
  reference on the working CE path): VAs `0x203c0fexx`, value `0x1` + timestamp
  (`cuda-jit.trace:8175+`).

Replay order (analysis, per the E5c trace and host-exec Stage D method set): upload the
256-byte kernel code inline-to-memory to a code VA, upload the parameters, write the QMD
(adjust VAs to the host context), point a compute launch at the QMD, ring the doorbell,
and watch for the release0 write at `0x203c3ff7c` = `0x6` (success = the SMs ran
guest-compiled code under a host-built context, deciding c1 vs c2) or a host-journal
exception (ESR/warp/MMU fault) to map-and-retry.

## 6. What a supervised run needs

Preconditions (from the safety envelope, host-exec.md, memory-model.md):
- The operator's own go-ahead, at the card, given the vm2 agent is concurrently
  producing Xids on it; ideally the two runs are serialized, not concurrent.
- A floating contiguous reservation (host-exec fell back to floating; the top 4 GiB is
  vm2's), host VRAM under 1 GiB, RC watchdog on, free-everything on every exit path,
  stop at the first uncontained Xid and record each contained RC.
- The tool `caraxes/tools/host-gr.rs` (+ `host-gr.sh`), same rustc-only, sudo-drop-to-
  CAP_SYS_ADMIN style as host-exec.rs (D12 privilege: raise the cap only around
  `GET_SURFACE_PHYS_ATTR`, `DMA_SET_PAGE_DIRECTORY`, and - to be proven necessary and
  documented - `VASPACE_COPY_SERVER_RESERVED_PDES` / `GPU_PROMOTE_CTX`). It should reuse
  host-exec's stages A-E verbatim and add: (1) the section-3 test matrix; (2) the
  section-4 reserved-PDE + GR-context walk; (3) the section-5 replay. It was **not
  written** in this session for the same reason the experiments were not run: it is the
  execution vehicle for gated work. Its design is fully specified above so it can be
  built and run under supervision.

## Status

- Analysis complete from existing evidence; live experiments (goals 1-3) not run,
  pending operator go-ahead (section 1). Suggested README row (not applied here, to avoid
  colliding with concurrent edits):
  `host-gr.md | host GR path: PBDMA/PBENTRY (Xid 32) and GR-context (Xid 38) walls, cause analysis + test matrix + extracted launch bytes | analysis (experiments gated on operator)`.
