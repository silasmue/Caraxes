<!-- SPDX-License-Identifier: MIT -->
# Locating render latency

The long Linux ALU probe reports 239.231 µs native versus 335.146 µs guest CPU
submit-through-fence medians: about 40% more time, or 71% of native throughput.
Its GPU medians are 207.968 and 209.952 µs. This supports investigating submission
and completion, but does not explain games at roughly 30% native FPS. That ratio
would mean about 233% more frame time. A matched native game reference is still
missing; online scores are targets, not an overhead measurement.

Do not subtract independent component medians or add nested profile totals.
Compare complete, aligned samples. Existing evidence and clock limitations are
in [GPU workload attribution](gpu-workload-attribution-2026-10-06.md).

## Backend observations

Set `CARAXES_PROFILE=1` and `CARAXES_PROFILE_HOST_TIMELINE=1`. Existing SIGUSR2 and
exit dumps now produce three files under `CARAXES_PROFILE_OUT`:

| File | Observations |
|---|---|
| `.TAG.tsv` | Existing aggregate costs |
| `.TAG.host.tsv` | Completed A612/A613 host ioctl intervals |
| `.TAG.markers.tsv` | Physical doorbells, drained host notifiers, requested raises, published local IRQ causes |

- Host spans and markers share the exact monotonic anchor. Unix capture brackets
  support coarse alignment; they do not calibrate guest QPC or the GPU clock.
- Markers retain known channel/generation or notifier/vector identifiers. Folded
  IRQ causes also retain epoch/ticket. Engine events cannot identify one channel.
- Doorbells are recorded after the physical write. Raise requests precede the
  call and may be rejected by generation checks. Cause publication is observed
  after successful publication and wake. None proves guest delivery or ACK.
- Host notifier events are engine-wide. Host desktop activity can notify an idle
  guest; events do not identify the completion of a particular dispatch.
- The optional rings retain 32768 host spans and 65536 markers. Actual storage
  bytes and loss counters appear in each snapshot. Overlapping dumps do not
  consume rows. Default-off mode allocates no marker ring.
- Marker writers use `try_lock`, with explicit contention/poison drops and no
  per-event allocation or file I/O. Existing aggregate/host-span observation
  still takes the profile mutex. Dumps can perturb a diagnostic run.

Analyze snapshots from the same process:

```sh
python3 tools/analyze-backend-timeline.py \
  run/host-profile.1.host.tsv run/host-profile.2.host.tsv \
  run/host-profile.1.markers.tsv run/host-profile.2.markers.tsv \
  --begin-ns BEGIN --end-ns END --output fresh-analysis.json
```

Bounds use nanoseconds relative to the common backend anchor. Without them the
report covers startup through the latest snapshot. The analyzer deduplicates
sequence IDs, rejects different anchors and conflicting rows, preserves statuses,
reports missing IDs, and computes interval unions instead of summing overlaps.
It accepts up to 4096 snapshots, 8 GiB of input and four million unique rows;
exceeding a limit fails explicitly. The previous 128-file cap could reject a
complete paired render capture at five-second cadence.
One-second buckets show control occupancy and event counts. Doorbell interarrival
gaps are grouped by channel incarnation; they are not queue latency. No nearest
event matching invents a dispatch/completion relationship.

## Hardware check

The immutable marker build uses source `b73a1da7`, backend SHA256
`a91fc1efa94e339facf0886e90cd7aa58b60b441892e84010634a5502ee1e7ac`.
It passes 1001 library tests, with two existing tests ignored, and six analyzer
checks. Windows C224/C225 boot and pass the three-frame D3D11 readback gate.

- C223 fails preflight before backend/VMM launch because the new artifact lacks
  the launcher's `SHA256SUMS`. The fresh C224 packet pins that file and records
  TAP identity after link-up, allowing early-failure cleanup after MAC assignment.
- C224 reaches its preparation deadline and exits cleanly before any benchmark.
- C225's first SIGUSR2 dump proves both streams share the exact anchor. It retains
  32330 markers without overflow, contention, poison or invalid rows, plus two
  completed host calls. Ring storage is 8 MiB markers and 1.5 MiB host spans.
  This 62.768-second startup snapshot precedes the D3D smoke and render setup.
- C225 later aborts at the 6 GiB RAM floor, before Run or trace arming. Owned VM,
  backend, TPM and TAP are gone; the physical GPU remains NVIDIA-bound. Floppy
  priority and resource limits remain unchanged. No FPS comparison follows.
- C226 passes the D3D11 gate and reaches the standard Fire Strike page, demo off,
  but exceeds its 450-second preparation bound before arming or Run. Two host
  orchestration gaps total about 169 seconds; task exit 0 did not prove that the
  first menu navigation succeeded. Preparation snapshots retain 378961 of
  672823 marker observations, so startup coverage is incomplete. Cleanup is
  clean; signed NVIDIA 617.14 remains ProblemCode0 with no Display4101 events.
- C228 stops before VM/backend launch: its frozen launcher looks for the normal
  binary basename in `SHA256SUMS`, which C227 only lists with the capability
  suffix. The binary hash is correct. Early TAP cleanup also detects udev's
  persistent MAC assignment after identity capture. Exact owned cleanup removes
  that interface under the retained guest/slot locks; no GPU workload runs.
  Fresh C229-build provides both verified checksum names for the same binary.
- C230 validates the checksum/udev corrections, boots Windows and passes three
  D3D11 readbacks. It stops before Run: launcher READY identifies PID8356, but
  the next UI helper finds no matching main window. No failure-time window census
  was collected, so the cause is unknown. The 194-second session closes cleanly;
  pre/post NVIDIA health remains ProblemCode0 with no Display4101 events.
  Nineteen preparation snapshot pairs retain all 716 selected host spans and
  248352/249130 marker IDs; 778 IDs are lost to contention. Ring overwrite does
  not add union loss. This is preparation evidence, with no render FPS.
- C232 uses the TSG candidate and the new PID-bound UI helper. Three D3D11
  readbacks and four fresh snapshot/click images pass. The separate callback
  validator rejects `dx11-page` because its name check forbids digits. No Fire
  Strike detail/settings proof, process trace, arm, Run or FPS follows. The
  332-second session closes cleanly with unchanged good driver health. Forty-three
  preparation snapshot pairs retain all 716 selected host spans and
  346805/347881 marker IDs; 1076 IDs are lost to contention. Frozen inputs remain
  unchanged. Fix and exercise the actual callback before another run.
- C244 passes all seven fresh UI gates, including the unchanged standard P
  settings and armed page. It stops before Run when the private controller
  receives a locale-comma trace remainder with a two-hour local/UTC offset.
  The 384-second session and process trace close cleanly. Fifty-one snapshot
  pairs retain all 716 selected host spans and 411515/412688 marker IDs; 1173
  IDs are lost to contention. No arm, Run or FPS result follows.
- C246 changes only that controller clock contract. The guest normalizes .NET
  date kinds to UTC and publishes one UTC timestamp pair with a typed JSON
  remainder. Host validation checks aware UTC, finite 0–700 seconds and agreement
  within 1 ms; the 650-second pre-Run minimum remains. Fifty actual-function
  mocked CPU checks reproduce both old failures and exercise malformed or
  inconsistent samples. Its hardware attempt stops before that check: the
  settings image still has a loading modal and no required values, so visual
  approval is refused; the preparation deadline follows. Cleanup is clean in
  489 seconds. No process trace, arm, Run or FPS. Native timing remains
  unqualified. Five successful reviews consume 140.945 seconds of host monotonic
  time; READY-to-controller preparation adds 27.726 seconds of host wall time.
  These are harness delays before rendering. The backend, UI helper, workload
  and host policies stay unchanged.
- C250 adds a pinned READY watcher and one five-second settings-loading recheck.
  Controller launch follows observed READY by 0.5 ms; preparation begins about
  0.42 seconds later. Four visual reviews pass, but root misses the loaded
  settings image's 40-second review deadline during context compaction. Cleanup
  is clean in 350.2 seconds. No trace, Run or FPS follows. Forty-seven preparation
  pairs retain all 716 selected host spans and 371116/372268 marker IDs; 1152
  IDs are lost to contention. Use a sole dedicated PNG reviewer for C254;
  retain every existing workload, resource and time bound.
- C254 confirms the cold GUI path still exceeds the preparation allocation.
  Six successful visual reviews take 157.843 seconds; loaded standard P settings
  pass. Details is captured at owner413.903s, then the 450-second bound expires
  before its callback. Cleanup is clean in 477.6 seconds, with no trace or Run.
  Sixty-nine preparation pairs retain 471865/473367 marker IDs; 1502 are lost to
  contention. All 716 selected host spans remain. C258 will use a fresh small
  reviewer context and a bounded 600-second preparation allocation within the
  unchanged 1500-second work/1800-second session budgets. No FPS follows from
  any preparation timing change.
- C258 stops at the first snapshot's owned-window check and closes cleanly in
  174 seconds. Its fresh failure image shows a blank loading window with a
  transient caption. The old failure census was sampled during acquisition;
  it cannot prove the rejected predicate or a HWND race. The revised helper
  retains exact ownership/foreground checks, treats a bound snapshot's title
  as readiness metadata and still requires `3DMark` immediately before input.
  Grammar, 18 structural checks and the mocked handoff25 checks pass. Native
  qualification remains pending. Twenty preparation pairs retain 239542/240226
  marker IDs, with 684 contention losses and 716 host spans. No trace or Run.

Curated local evidence: `/mnt/vmstore/caraxes/3dmark-trials-2026-10-09/C225-report.json`
and `C225-runtime/results/partial-backend-timeline-report.json`, plus
`C226-report.json`, `C228-report.json`, `C230-report.json`, `C232-report.json`,
`C244-report.json`, `C246-report.json`, `C250-report.json`, `C254-report.json`
and `C258-report.json`
and `C244-report.json`, `C246-report.json` and `C246-preparation-timing.json`.
There is no
complete render timeline or demonstrated improvement from this build yet.

The replacement `tools/windows/3dmark-ui-step.ps1` refreshes process state under
one ten-second window/foreground bound. It checks the interactive session, exact
title, PID/start time and native HWND owner before input. `ExpectedPid` supports
the launcher's exact handoff. Screenshots report their hash, size and UTC;
failures include bounded process/window observations and a separate fresh image.
Thirteen CPU grammar/structure checks and independent source review pass. C232
qualifies normal snapshot/click operation on stock Windows with exact PID9220
and session1. The failure-capture path remains unqualified on Windows.

## Calibrated probe

`vk-unit-timing --calibrate` now records raw DEVICE/CLOCK_MONOTONIC pairs before
and after each sample, calibration call brackets, `max_deviation_ns`, and absolute
CPU endpoints. It prefers KHR with an EXT fallback and requires both exact time
domains. Default-off output stays compatible. CPU checks cover capability
selection and reject CLOCK_MONOTONIC_RAW as a substitute for the probe's clock.

A native RTX 2070 pilot passes complete verification of 65536 words, with no
mismatches. KHR calibration works; reported deviations are 2048–2080 ns. Both
measured GPU intervals are about 210 µs, and their endpoints lie between the raw
calibration pairs. The build passes `-Wall -Wextra -Werror` and CPU self-tests.

This is a capability and ordering check, not a new native performance baseline.
CPU15/SCHED_IDLE/nice19 preserves Floppy priority and can delay CPU observation
of completed work. The guest capability check below uses the same binary.
Pair-derived offsets do not bound drift between calls; retain that uncertainty
before assigning a delay.
Evidence: `vulkan-calibration/cpu-validation.json` and
`vulkan-calibration/native-pilot/audit.json` under the private artifact directory.

## Guest clock mismatch

The fresh headless `linux-calibration-oct09a` pilot passes all 65536 output words
on stock 615.71.09, with no helper module, four vCPUs and 4 GiB RAM. All ten ALU
samples have incompatible calibrated DEVICE and query timestamp epochs:

- Calibration returns about `1.685e10` ticks; query endpoints return about
  `1.79156e18` ticks. The reported period is 1 ns and width is 64 bits.
- No query interval falls inside its bracketing device calibration pairs. Native
  pairs contain every query interval. This is an epoch mismatch, not timestamp
  wrap or a small scheduling delay.
- Guest calibration reports 86–186 µs deviation, versus about 2.1 µs natively.
  Even after correcting the epoch, retain uncertainty before finer attribution.

Vulkan requires DEVICE calibration and command-buffer timestamp queries to use
comparable values. [Time-domain contract](https://docs.vulkan.org/refpages/latest/refpages/source/VkTimeDomainKHR.html).
The previous code used process-relative time in PTIMER, usermode TIME and synthetic
time-correlation replies, while mirror timestamp commands executed on the physical
GPU. The correction below fixes that domain mismatch; its FPS effect is unproven.
GPU elapsed differences and the output oracle remain valid, but a CPU/GPU boundary
split from these pairs would be wrong.

The 22-second session closes cleanly: owned backend/VMM and TAP are gone, NVIDIA
remains bound, and no new host Xid appears. Floppy keeps priority. SCHED_IDLE/nice19
makes this a clock-domain and correctness check, not a matched latency baseline.
Private audit: `guest-gpu-clock-domain-audit.json` under the artifact directory.

## GPU clock correction

Source `d42dc40b` selects one private read-only physical usermode timer lease for
host-backed GPU timestamps. PTIMER, VF/UMPage, GPU correlation fields, software
semaphores, window/core display notifiers and RUSD use that provider. Logical GSP
CPU time, display intervals and watchdog deadlines remain separate. Pure-emulation
behavior and reset continuity are covered by tests. The physical mapping is never
offered to the guest. Unstable reads are bounded and terminate the backend instead
of silently switching epochs.

- 1010 library tests and 15 client tests pass, with two existing tests ignored.
  Release build, independent source review and AST graph update pass.
- Fresh `linux-calibration-oct09b` uses the original probe binary and fixed stock
  615/no-helper, 4 vCPU/4 GiB headless policy. All ten GPU intervals now lie inside
  their raw DEVICE calibration brackets; previously none did. All 65536 words
  pass with the same checksum `365943ba07dd5422`.
- GPU median remains about 210 µs. This SCHED_IDLE/nice19 capability pilot does
  not establish a CPU latency or FPS improvement. Calibration deviation remains
  68–100 µs, too large for a fine boundary split.
- The 18-second session closes cleanly, including exact backend/VMM and TAP
  removal. The host GPU remains NVIDIA-bound. UMPage is off in this pilot; its
  1 µs minimum publication interval does not guarantee maximum clock freshness.

Immutable backend SHA256:
`9d280695f8bc78b9bbcda330668895ee12779ed364cfbe09fd0d9ab4a408744d`.
Private proof: `guest-gpu-clock-fix-audit.json` and `C227-build/cpu-validation.json`.

## Early TSG timeslice requests

Before a host mirror existed, `SET_TIMESLICE` returned success but discarded the
guest's request. Later scheduling therefore used the physical TSG's default.
The correction retains the exact eight-byte request and its allocation/revision,
then applies the latest accepted value before either first host-scheduling path.
Live SET/GET preserve the physical status and returned payload. A refused SET
does not overwrite accepted policy or permit first scheduling.

Guest FREE, client FREE and accepted handle replacement retire old policy.
Replacement also removes old children, tokens and promoted TSG buffers without
removing the newly installed allocation. Refusal preserves the old allocation.
Host context eviction retains policy for rebuilding. Display/family allocations
receive the same retirement check as registry allocations. Unknown initial GET
keeps the previous fallback: no native default is invented. Independent review
and 1016 library tests pass; two existing tests remain ignored. Release build
and AST graph update pass. The patch is committed as `c8e8ea4d`. C232 preparation
records 206 successful physical SET calls and 244 successful guest SET requests.
Build-time replay and repeated requests prevent one-to-one count comparison.
Render qualification and any FPS effect remain pending.

TSG DUP handles remain an unqualified compatibility limit: the registry tracks
VAS aliases only. Timeslice control on such a handle is refused; the old early
SET fallback falsely acknowledged it without applying policy. No stock Windows
TSG DUP use was found in the scoped evidence search. A future implementation
needs shared object lifetime through the last alias, not another silent ACK.

## Remaining candidates

These source paths are measurable candidates, not established render bottlenecks.
No source evidence shows ordinary mirrored GR work being executed twice.

C220's saved backend log confirms compression is enabled, with comptags covering
the full 4096 MiB physical reservation. An unavailable PTE audit does not disable
the mapping policy; a recorded violation does. The filter can downgrade sysmem,
small, misaligned or uncovered mappings. Downgrade notes are capped and
deduplicated, so existing logs cannot establish a complete byte/resource census.
Host-backed ZBC setters use physical host entries; the separate local reference
flag is not the table setter. No broad compression-disable or index-mapping bug
was found in this scoped review. See `compslice.rs`, `shadow.rs` and `zbc.rs`.

- A mirrored consumer can wait for a software semaphore producer held behind
  shadow invalidation. Root plans also delay physical doorbells
  (`channel.rs`, `mirror.rs`). Match one semaphore address/value to its producer,
  plan completion, forwarded doorbell and consumer progress. Current markers
  have no semaphore identity and cannot establish that dependency.
- BAR1 pool changes copy pages and can preempt affected TSGs before remapping and
  resynchronizing (`bar1pool.rs`). Measure render-only deltas of copied bytes,
  warm barriers, preempts and resync time. C220's broad 592.5-second aggregate
  bracket records 688 moved pages, 2.6875 MiB and 19.95 ms of copying. Copy time
  alone is a weak explanation; its nine warm barriers have no duration field.
  These aggregates include preparation/menu/idle, not exact GT boundaries.
- Slice compression can downgrade small/misaligned pages, sysmem, uncovered
  ranges and audit-disabled mappings (`compslice.rs`). Count mapped bytes by
  requested/effective kind, backing and downgrade reason. Valid output alone
  does not establish equivalent memory traffic. Retain physical PTE audit proof.

## Measuring the missing boundaries

1. **GPU execution and guest completion:** keep the original verified Vulkan and
   D3D probes. Compare the same work at comparable physical clocks. Run the new
   optional calibration in the guest to locate GPU endpoints within each sample.
   Vulkan requires supported device/host time domains and records `maxDeviation`;
   decline finer attribution when uncertainty approaches the delay. Guest clock
   calibration still needs a bounded guest-to-host mapping to join backend events.
   [Khronos contract](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetCalibratedTimestampsKHR.html).
2. **GPU scheduling:** pilot a short, owned Nsight context-switch capture before
   rendering attribution. Verify that Caraxes's host RM contexts are visible and
   map them to the exact backend instance. Context residency does not prove ready
   or executing work. Host API tracing does not expose guest userspace calls;
   standard vGPU host capture is documented as unsupported.
   [NVIDIA context switches](https://docs.nvidia.com/nsight-systems/UserGuide/index.html#gpu-context-switch).
3. **Graphic execution limits:** only after locating a slow GPU interval, collect
   raster/depth/color, texture and cache metrics during that same workload. The
   documented `tu10x-gfxt` set recommends at least 10 kHz; the older clean 200 Hz
   general-counter recipe is not adequate evidence for this set. Qualify overflow
   and missing data in a short pilot. Device-wide counters include host work.
   [NVIDIA sampling guidance](https://docs.nvidia.com/nsight-systems/UserGuide/index.html#launching-gpu-metrics-from-the-cli).

D3D11 disjoint queries validate GPU durations but do not calibrate CPU/GPU clocks.
Windows D3D12 exposes a separate
[GetClockCalibration API](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/nf-d3d12-id3d12commandqueue-getclockcalibration).
Adding a D3D12 diagnostic does not retroactively calibrate D3D11 or Fire Strike.

All hardware captures retain Floppy priority, exact owned process identities,
resource floors, finite deadlines, host driver binding and clean teardown. Arm
collection before rendering; launch no foreground or capture UI during scenes.
