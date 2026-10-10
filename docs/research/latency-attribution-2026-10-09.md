<!-- SPDX-License-Identifier: MIT -->
# Locating render latency

C326/C328 completes a short native capture/export probe with clean driver health
and exact cleanup. Root verifies all 37 evidence hashes. Its SQLite export
matches the fresh physical BDF/UUID and contains 49364 values, 1763 timestamps,
28 metrics and 754 context events. No benchmark ran. C330/C332 now applies the
same reviewed ownership fix to the full Fire Strike diagnostic, but stops before
Run when its fifth PNG review exceeds 40 seconds. Exact cleanup and driver health
pass; root verifies all 54 evidence hashes. No render capture or FPS qualifies.
Four accepted reviews took 11.754/29.169/19.512/33.015 seconds. The PNG was already
local when the host checkpoint clock started; internal loader/view/publisher
boundaries are absent, so the delay cannot be assigned to one stage. C334/C336
uses GPT-Sol; its Fire Strike review takes 7.393 seconds, within the bound, but
omits explicit preset and Demo-off fields because the root brief was incomplete.
The consumer stops before Run. Root verifies all 51 evidence hashes; health and
exact cleanup pass. C338/C340 will use the corrected complete callback brief
with unchanged deadlines and GPU settings. Neither failed preparation yields
render counters or a performance comparison.

The short export lacks an embedded metric definition. Use
`tools/graphics/nsys-metrics-summary.py --metric-definition PATH
--metric-definition-sha256 SHA256` with the exact `tu10x.config` from its pinned
Nsight installation. The parser checks bounded bytes, SHA256, all metric names
and agreement with embedded definitions when present. It records the external
provenance and applied factors. Four CPU regressions and the real export pass;
raw clock values use a factor of `1e-6` before reporting MHz. No vendor definition
is copied into the repository.

Requested sampling is 200 Hz; observed median spacing is 5.745 ms, with no gaps
over three times that median and no warning/error/overflow diagnostics. The
metric tail extends 144.831 ms past analysis stop, containing 26 extra timestamps.
Preserve those samples rather than silently clipping them. Context pairing has
376 complete intervals and two unmatched capture edges; host context residence
does not prove execution or a stall cause. This idle probe does not identify the
rendering bottleneck. Private audits: `C326-root-metrics-summary.json` and
`C326-root-metric-quality-audit.json`.

C314's fixed-file diagnostic retains failed replies before validation, but its
numeric GZip parameter is ambiguous on Windows. The wrapper fails before AST
validation or any installed-file read. C316 uses explicit
`CompressionMode::Decompress`, matching the
[Microsoft constructor overload](https://learn.microsoft.com/en-us/dotnet/api/system.io.compression.gzipstream.-ctor?view=netframework-4.8.1).
The 7102-character command retains the exact source hash and all bounded query
and cleanup checks. C316's native probe passes in 7.094 seconds. The installed
Fire Strike x64 file reports `NotSigned`, version `1.1.0.0`, size 5853696 bytes,
SHA256 `e68d2fad78ab1d73d0f053c922f8be03e8e5b791f576f4779f092328c0f73ef5`.
Its complete raw reply is saved; driver health and exact cleanup pass. The
collector's `Valid`-only predicate therefore rejects this installed payload.
C310's lost live reply remains unknown. C318/C320 passes the live file tuple and
fresh physical GPU mapping; complete 424-byte stdout and empty stderr are saved.
Nsight then fails its process ownership check and reports an agent connection
ending. The offending PID/census was not retained; the exact cause and error
order remain unknown. No capture, export, completed benchmark or FPS qualifies.
Driver health and exact cleanup pass in 560.897 seconds. Root verifies all 42
hashes in `C318-report.json`, SHA256
`e61ecc5681d221ed122673fab598af5095d8da827f93b638a7c2fc4d607906f1`.
The old collector discovers children before a blocking guard, then scans Nsight
processes: a new owned descendant can appear during the guard. The successor
checks positive ancestry and exact retained process identity at the scan,
keeps prelaunch exclusion strict, and saves bounded rejection evidence. C326
passes its short native lifecycle. This repairs measurement admission; it does
not establish C318's exact rejection cause or a rendering speedup.

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

The VRAM reservation is one contiguous allocation. Compression adds attributes
to that allocation; it does not allocate another 4 GiB slice. Comptag occupancy
has not been measured. The FB store uses two 16 MiB data windows, plus four 2 MiB
BAR2 windows; the separate BAR1 pool holds 32 MiB. Those windows can remap on
misses, but the preserved C272 log has no runtime churn or final store totals.
Startup BAR1 admission alone cannot establish pressure during rendering.

C272's fifteen `remap refused` rows identify protected guest page-table pages,
not exhausted BAR1 capacity. Their CPU view uses mapped replicas, which are
merged into the tracked home pages at synchronization points. A write need
not trap; merge and shadow-resync costs remain unmeasured. Read-only source/log
reviews: `C318-bar1-vram-offline-review.json` and
`C318-bar1-table-replicas-offline-review.json` in the private trial directory.
Check runtime churn, replica syncs, shadow resyncs and their costs before changing
window sizes or table handling. Keep ownership and address validation intact.

C318's before-trace and last pre-abort snapshots span 126.710 seconds, including
preparation and only part of the workload. Their cumulative differences show
1702 A613 host calls occupying 7.114 seconds, 48194 BAR2 memfd syncs occupying
2.220 seconds with 851 dirty-page units, and 6369 RM_FREE calls occupying
2.221 seconds. These costs overlap other profile categories and do not establish
frame stalls. BAR2 memfd sync is separate from BAR1 replica synchronization.
`C318-partial-profile-delta.json`, SHA256
`f6008969898ec1da2ff63bc74dd2920a850f182464a6381bdca2a59775575063`,
preserves input hashes, monotone counter checks and the full delta. Its join
uses kind/key/name/label; status includes cumulative counts and is not a key.
The same bracket records 3929 worker queue waits averaging 29.075 µs and
3399 host-ready-to-Device intervals averaging 99.000 µs. Finish/serialization
averages 32.484 µs across 3399 calls; physical A613 averages 4179.726 µs.
The different counts include bracket boundaries and different call categories.
These means do not identify a frame dependency or a GPU waiting percentage.

Zero-dirty BAR2 syncs still compare live mapped pages against their snapshots.
Clean cached page tables already skip `follow_tables` traversal; the periodic
drift read remains separate. Skipping the method's setup could save a small
cost, but skipping page comparisons requires reliable VMM write tracking.
No such optimization is applied without render-specific cost evidence.

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
Standard Fire Strike now qualifies in C262. GT1 is 18.486073 FPS and GT2 is
29.838354; all four workload lifetimes exit 0 with clean driver health and exact
owned teardown. C264's clock-only comparison aborts on a late armed review before
Run. C270 now completes with GT1 18.219490 FPS and GT2 31.251526. This single
comparison shows no consistent graphics gain; backend, preparation order and
fresh reviewer differ. Retain TSG replay for correctness without a causal FPS
claim. C272 tests existing host-control offlock with both new workers on CPU2.

## C262 render observation

- The clipped backend interval is 282.166 seconds, including launch and result
  handling. It is not an exact moving-scene interval or guest-to-host clock map.
- A612 has 7984 calls, median 0.377 ms; A613 has 7990, median 3.478 ms and
  p95 7.561 ms. All statuses are zero. Their union occupies 39.236 seconds,
  about 13.9% of the interval; occupancy is not critical-path frame delay.
- All 16690 host spans are present. The marker union retains 702240 of 704522
  sequence IDs. The 2282 missing IDs equal global contention losses; their
  locations are unknown. No invalid or poisoned records. Snapshot overwrite
  reaches 636704, but overlapping captures recover those overwritten rows.
- The native trace-time gate passes with 650–700 seconds remaining. Its exact
  returned value was not stored. The original guest UTC deadline minus actual
  Run receipt is 656.083 seconds; this is a separate derived margin.
- The new helper succeeds natively and eight visual callback waits total
  53.544 seconds. Preparation to Run request takes 319.108 seconds. Stock driver,
  settings and owner limits remain fixed; no UI captures occur during rendering.

Private evidence: `C262-report.json`, `controller-report.json`,
`render-qualification.json` and `backend-timeline-report.json` under the trial
directory. This instrumented result does not establish a speedup.

TSG DUP handles remain an unqualified compatibility limit: the registry tracks
VAS aliases only. Timeslice control on such a handle is refused; the old early
SET fallback falsely acknowledged it without applying policy. No stock Windows
TSG DUP use was found in the scoped evidence search. A future implementation
needs shared object lifetime through the last alias, not another silent ACK.

## C270 comparison

All four workload lifetimes succeed and exact teardown passes with healthy
stock driver. The corrected preparation takes 278.768 seconds to Run request;
seven callback waits total 44.741 seconds, including 6.985 for the armed page.
The native trace-time gate passes; its exact sample was not persisted. Derived
same-guest deadline minus click is 657.442 seconds, not a host clock mapping.

The clipped observation is 324.575 seconds because its before pair now precedes
trace start and armed review. A612/A613 union is 38.504 seconds; A613 median is
3.501 ms. That larger bracket prevents a direct occupancy-percentage comparison
with C262. All 16410 host spans are retained; 2381 global marker IDs are missing
through contention. Neither observation proves critical-path frame delay.

C272 releases Device during the same physical calls while preserving ordered
replies and allocation retirement. Both new threads are pinned to the existing
backend CPU2 with actual SCHED_OTHER/nice10 checks. Record its existing queue
wait, physical execution, ready-to-Device and serialization stages. The C270
comparison also changes the TSG backend; C262 retains that backend but has the
older preparation order. Disclose these factors before attributing a difference.

## Remaining candidates

These source paths are measurable candidates, not established render bottlenecks.
No source evidence shows ordinary mirrored GR work being executed twice.

The counter/context pilot's source and runtime bindings are reviewed. C292's private tool GO follows
pidfd admission, mapping freshness is checked again before exec, and SIGTERM
allows cleanup of its dedicated tool subtree. Independent CPU checks pass
61 collector and 28 gate cases. C282's BDF/UUID parser also passes; native help
grammar remains unqualified. C284 places the collector in a separate coordinator
to protect the render observer. Initialization cancellation and bounded parent
cleanup supervision are corrected, with explicit descendant-exit proof even
after nonzero coordinator exit; integer trigger timing also avoids float
rounding. Independent 26 counter cases and inherited controller checks pass.
Actual UID1000 pins/access pass. C284 closes before Run after the armed review
misses its 30-second deadline. No native mapping, capture or export starts.
The six accepted prior reviews consume 141.598 seconds; preparation takes
412.570 seconds. These are harness timings, not frame latency. Exact cleanup
passes in 494.139 seconds. No GPU counters or FPS are collected by this pilot.

C296 completes the separate backend-profile-off comparison: GT1 18.775480,
GT2 32.267513, Physics 34.306366 and Combined 20.259274 FPS. All four workloads,
trace, fresh archive, driver health and exact cleanup pass. Against C272, graphics
changes are mixed (-2.6% GT1, +7.3% GT2), with no broad gain or isolated causal
profiling effect. Snapshot formatting holds shared profiler and marker locks;
file writes occur afterward. C296 disables profile and host timeline together
and removes every snapshot request, including post-health SIGUSR2, which can
terminate an unprofiled backend. The workload, trace, clock, worker, resource
and cleanup policies remain unchanged.
The explicit validator mode `--no-host-profile` passes five saved-packet CPU
fixture tests; the default also requalifies the saved C272 result. Observed
effective cadence also differs: median clock intervals are 1.307
versus 1.320 seconds; guest queries 11.775 versus 12.534. These are whole-observer
host intervals without scene or guest/GPU clock mapping. Guest tracing and CH
write profiling remain. No backend timeline or new GPU counters exist for C296.
See [render trials](windows-render-trials-2026-10-09.md) for qualification and hashes.

C302's counter pilot reaches Run but stops before capture: native Nsight help
uses leading tabs, which C282's parser rejects. Help and BDF/UUID inventory
commands both exit 0. Exact cleanup, driver health and seven fresh visual reviews
pass. The incomplete workload yields no FPS or GPU counters. Root verifies all
29 evidence hashes in `C302-report.json`. The canonical pure parser in
`tools/graphics/nsys-device-map.py` accepts one leading help tab or four spaces,
while preserving bounded input, control rejection and the complete BDF/UUID join.
Twelve CPU tests and unchanged captured-output regression pass. Historical
output cannot attest a future device mapping.

C310/C312 subsequently qualifies fresh Nsight-help/inventory mapping for the
target PCI BDF/UUID and namespace index 0. Capture admission then rejects
`3DMarkICFWorkload.exe`'s signature before Popen. The query's precise status,
path and signer are not retained: `guest_observation` writes the reply only
after validation and discards stderr. No unsigned-payload or certificate-chain
conclusion is supported. All eight fresh visual reviews, driver health and
exact cleanup pass; no capture/export/archive/FPS qualifies. Root verifies the
41 evidence hashes in `C310-report.json`. Preserve failed bounded query replies
before validation; use a short fixed-path installed-file signature/hash query
before another full benchmark. Keep host process isolation and freshness checks.

Compression activation is present: C272/C296/C302/C310 all pin
`CARAXES_COMPRESSION=slice`. C272's preserved log reports 4 GiB covered and
compressible maps enabled, but the host PTE audit is unavailable (test-only
control refused with 0x7e); all 81 VAS summaries retain zero samples. There are
no explicit downgrade/error rows. This does not prove every effective PTE kind
or exclude fallback. `compslice.rs::kind_use` downgrades compressed kinds on
small, unaligned, sysmem or uncovered leaves. Missing activation is therefore
not an explanation for these cases; effective surface compression still needs
measurement. Private evidence: `C310-compression-offline-review.json`, SHA256
`6b2ec16939e597a0841cbf9f50e9d225c044df54780eaf6c5b02730c29bcac00`.

A613 is the opaque GSS legacy PMGR 0x13 control, with a 79936-byte buffer on
the current 610 ABI (`ver/v610.rs`, `nvml.rs`). Eligible requests forward their
exact bytes/status; no field interpretation or version translation is defined.
It is excluded from the experimental cache candidates. The measured host span
starts immediately before `libc::ioctl` and ends after return/errno capture
(`nvrm-abi/src/lib.rs`, `profile.rs`). Its 3.576 ms C272 median therefore includes
host syscall residence, with kernel waits and scheduling; emulator admission,
Device lock wait, payload preparation and reply publication are outside it.
Worker submit/start/ready times currently feed aggregates, not per-call records.
Future correlation needs a shared ticket ID in a bounded trace; the existing
host timeline alone provides cadence and duration, without payload fingerprints
or guest reply times. These observations do not justify caching opaque replies
or establish critical-path GPU stalls.

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
- The GSP reply HEAD is BAR0 0x110c80 on TU106; 0x110c04 is command TAIL.
  C272 records 180685 reply-HEAD reads/3.655 seconds in its broad observation.
  C278 negotiates synchronous host BAR shared memory, but its final workload
  fails at startup, so there is no accepted transport FPS comparison. An autonomous
  host-only snapshot could avoid the client/backend round trip for exactly this
  four-byte read through the VMM's existing local route. No such snapshot exists.
  Publish only after committed v2 status slots; invalidate on queue retirement,
  reset, unmap and disconnect. Keep other falcon reads on the current path:
  the full 0x110000 guest page also contains data ports with read side effects.
  Neither transport nor a host-only snapshot removes KVM exits or driver polling.
- Private USERD progress reads currently use VRAM through BAR1. `userd.rs`
  reads two words for an idle channel and six after movement; the P37 source
  comment records 9.6 µs for six uncached reads, not a new render measurement.
  Polling takes the slot lock; some device-side paths also hold Device. Measure
  generation-bound forward-to-fetch and progress-publication delays before
  changing this path. GET/GP_GET movement does not prove workload completion.
  The existing non-shadow channel path accepts a sysmem USERD OS descriptor,
  and `hostmem.rs` already has a private OS-descriptor page allocator. A private
  pinned sysmem USERD is therefore a plausible experiment, with no demonstrated
  native acceptance or speedup yet. Cover initial channel creation, migration,
  coherency, host allocation failure and retirement before enabling it. Preserve
  the guest/private-page split and keep the current allocator as the default.
- Slice compression can downgrade small/misaligned pages, sysmem, uncovered
  ranges and audit-disabled mappings (`compslice.rs`). Count mapped bytes by
  requested/effective kind, backing and downgrade reason. Valid output alone
  does not establish equivalent memory traffic. Retain physical PTE audit proof.

## USERD cancellation correction

C300 fixes pending-migration cancellation in `mirror.rs` and its dispatcher:

- API IDs are local to an owner/object. Cancellation previously searched all
  channels by API alone, which could select another registration's ring.
- The old path took the pending page and attempted its retirement even when
  the host object was missing or REMOVE_API failed. Those failures now retain
  the page and poller-visible slot.
- Exact host cancellation now precedes local software-ring removal. A local
  equal-API switch can no longer bypass an existing host registration, and a
  host error does not consume that local switch. This overlap was identified
  in source; it was not observed in a native run.

The pending ring records owner/object/API and physical host generation/object.
The actual selector checks the scoped identity. Removal validates its live host
identity, calls the host outside the slot lock, then rechecks the pending stamp
before taking storage. The regression exercises equal APIs on different owners,
missing/stale host identity, host refusal, retry, and mutation during removal.
Independent review, 49 Mirror, six USERD and ten deferred-context tests pass;
one existing Mirror benchmark remains ignored. No hardware test or performance
gain follows yet.

Successful REMOVE_API after the host already switched to the pending page still
uses the prior retirement assumption. Open RM 615.71.09 source shows REMOVE
deleting a registration without channel detach. Its USERD subdescriptor can
keep the physical allocation referenced after handle free; Caraxes still drops
the CPU BAR1 window and removes the pending poller. Physical allocation lifetime
alone therefore does not establish safe continued emulation.

The private C306 retention proposal remains unapplied. Keeping `next` until
observed new GPGet or channel teardown bounds extra backing to one page per
channel, but blocks later migrations/API reuse and can depend on an unmapped
guest page. Examined local controls expose channel idle and general USERD
location, without a per-channel current binding query. Idle does not detach a
bound page. A later native diagnostic needs binding or controlled rebind evidence;
no physical use-after-free or FPS cause has been demonstrated. Versioned source
paths/hashes are in `C306-userd-retirement-draft/SOURCE-EVIDENCE.md`.
CPU correction proof: `C300-userd-cpu-review-v2.json` in the trial directory.

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
