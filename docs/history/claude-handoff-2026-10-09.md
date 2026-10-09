<!-- SPDX-License-Identifier: MIT -->
# Windows performance handoff, 9 October 2026

Continue in `/mnt/vmstore/caraxes/wt-windows-3dmark`, branch
`codex/windows-3dmark-2026-10-04`. Main `/home/silas/git/Caraxes` contains unrelated
work. Keep it untouched. User priorities: Windows rendering, Wayland desktop,
CUDA, release documentation, then more GPU models. Use host-backed GPUs and stock
guests. Floppy has priority; retain fixed RAM/disk floors and exact owned cleanup.

Disk watch: `caraxes-disk-reserve-20261009.service` is a transient user service
for twelve hours from 18:05 UTC. It samples `/` and `/mnt/vmstore` every 30 seconds
on CPU15/SCHED_IDLE/nice19. Read `disk-reserve-state.json` in the private trial
directory; a low-reserve sample also writes `disk-reserve-pressure.json`.
It observes only. Check both filesystems before new writes or VM starts; retain
at least 20 GiB on `/` and 55 GiB on vmstore. Put new large build outputs on
vmstore. Cleanup removed 3.522 GiB of old Rust incremental caches while holding
Cargo locks and checking process FD/cwd/exe/maps references. Sources, pinned
binaries, current evidence, VM images, Codex state and Floppy were preserved.
Post-cleanup free space is about 27/142 GiB. Private proof:
`disk-cleanup-oct09-1803.json`.

## Verified state

- C262 completes standard Fire Strike with the GPU clock and early TSG fixes.
  GT1 is 18.486073 FPS, GT2 29.838354; graphics score 5250. All four workload
  lifetimes exit 0; driver ProblemCode and Display 4101 remain 0. Exact owned
  cleanup passes in 671 seconds. These are instrumented local results, with no
  established speedup. A completed clock-only baseline remains pending.
  The new helper, eight visual callbacks and native UTC trace check pass. The
  gate establishes 650–700 seconds remaining; the exact returned sample was not
  saved. Guest deadline minus Run receipt is 656.083 seconds, a separate derived
  value. Callback waits total 53.544 seconds; preparation to Run request is
  319.108 seconds. Root no longer publishes live visual callbacks.
  Across the clipped 282.166-second observation, A612/A613 occupy 39.236 seconds
  in union. A613 median is 3.478 ms. This is control occupancy, not frame stall
  time. All 16690 host spans are retained; 2282 globally missing marker IDs equal
  contention losses. Marker overwrite alone is not missing union coverage.
  Preserve `C262-report.json`, SHA256
  `db4e7af7fb9e78c06e3dd760df5b13e96e98ecdd67f8c1f4380a23bf7d5cf565`.
- C264 stops before Run because its armed review takes 22.976 seconds, beyond
  the 20-second gate. All eight callback waits total 162.094 seconds, versus
  C262's 53.544. Reusing the image agent is a possible contributor, not a proven
  cause. Trace stops cleanly; driver health and exact cleanup pass in 473 seconds.
  No baseline FPS. The publisher incorrectly reports success for the late
  callback; the controller rejects it. C270 is the correction with a
  fresh reviewer, an explicit 30-second armed deadline enforced at publication
  and consumption, and writer join/catalogue/paired-before work moved before
  trace start. Native trace700/minimum650 and prep600 stay fixed. C268's offlock
  proposal stays disabled because its C264 qualification prerequisite failed.
- C270 completes the clock-only comparison: GT1 18.219490 FPS, GT2 31.251526,
  Physics 31.385401, Combined 19.529490; graphics 5294. All four workload
  lifetimes exit 0, self-test 37, driver ProblemCode/Display4101 0 and exact
  cleanup pass. Owner closes in 647.639 seconds. The corrected preparation
  qualifies: Run request in 278.768 seconds, seven callback waits 44.741 seconds,
  armed review 6.985 seconds. The native 650–700-second gate passes; its exact
  sample remains unpersisted. Same-guest deadline minus click is 657.442 seconds.
  TSG replay shows no consistent graphics gain in this single comparison.
  Backend, preparation order and fresh reviewer differ; retain the correctness
  fix without a causal FPS claim. The observation is now 324.575 seconds,
  including the pretrace gap: A612/A613 union 38.504 seconds, A613 median 3.501 ms.
  All 16410 host spans are retained; 2381 missing marker IDs are contention losses.
  Preserve final `C270-report.json`, SHA256
  `d6f0081033726f5cf629ea3d9416399f18e8bd12da8e213943f813ffef4ca992`.
  Existing high-load samples confirm P0 and median 1830 MHz; no new GPU query.
- C272 offlock completes: GT1 19.280205 FPS, GT2 30.086205, Physics 28.355387,
  Combined 17.541880; graphics 5405. Four workload exits, self-test, driver health
  and exact cleanup pass; owner closes in 655.276 seconds. No clear graphics
  jump. Both new workers share CPU2/SCHED_OTHER/nice10 in 122 matching samples,
  54 during render; maximum sampling gap 5.006 seconds, not continuous proof.
  Preparation to Run is 311.822 seconds; callbacks 91.272, armed 13.823 seconds.
  Median high-load graphics clock is 1845 MHz/P0 from existing samples.
  Keep original `C272-report.json` unchanged, SHA256
  `b5d1748fe0c2ff8cb56084f41b080d96c0815a7a43e750a0fd735fbb3660eea3`.
  Corrected `C272-report-v2.json`, SHA256
  `606b156336a7d27b8e33ea47cd08f313dbc60b10432e1d6f2484d6caefc92c53`,
  identifies 0x110c04 as command TAIL and adds the full queue quartet.
  Reply HEAD 0x110c80 has 180685 reads/3.655 seconds; command HEAD writes
  31985/10.976 seconds. Physical A613 median remains 3.576 ms. Worker execution,
  queue wait, ready-to-Device and serialization totals overlap these scopes.
- C278 closes cleanly but is incomplete. The first three ICF children exit 0;
  the fourth exits 1. The archive reports `NO_RESPONSE` during startup, then
  `PROCESS_EXITED`; it repeats the prior Physics result and has no Combined
  result. The independent validator rejects it. Keep FPS null and exclude it
  from comparisons. Driver ProblemCode/Display4101 remain 0; exact teardown
  leaves NVIDIA bound. Required BAR shared memory uses the same C231/C215
  binaries, offlock, CPU2 worker placement and corrected C272 harness. Private
  launcher requires BAR_SHMEM1 and required VMM mode; owned CH capture enforces
  exactly one required mode. All other guards and limits remain. Owner11 plus
  placement15/shared-BAR23, other CPU suites and actual UID1000 hashes pass.
  Manifest SHA256:
  `48a396b80d25187ccb2d65aaca2072f1365ddb3f8d386685e2e8835adabc82e5`.
  All fresh visual gates pass; the reviewer stops after armed publication.
  Native sync-slot-v1 negotiation succeeds; CH profiles report transport=shmem.
  Initial three D3D11 readbacks pass. The failed final workload does not establish
  transport corruption; its cause remains unresolved. No profiler ran.
  This transport still serves reads in the backend and retains KVM exits;
  it provides no autonomous GSP reply-HEAD snapshot.
  Closure takes 790.753 seconds. Preserve `C278-report.json`, SHA256
  `079f77e13ce8d9c699d592cdc5e30f70cdf5ecb28644ca85dcf16ab1686b2d0c`,
  together with its separate VMM-log description correction `C278-report-v2.json`,
  SHA256 `10d2139d3eca821699f7b6cb49f3c4d5a7258e86a53a0184c551f11b4394973c`.
  Both C278 and qualified C272 contain the same 15 explicit BAR1 refusal errors
  during boot; watched page-table pages remain trapped with replicas. They do
  not identify a new C278 remapping fault. C278's two VMM matches are early BAR
  relocation warnings; no narrow shmem/transport failure matches were found.
  These log observations do not establish the final workload's failure cause.
- C258 closes cleanly in 174 seconds before its first visual checkpoint. The
  screenshot helper rejects its owned-window check. The verified failure image
  shows a blank loading window with caption `...`; the old error records the
  initial census rather than the rejected predicate. Title transition is a
  plausible cause, not a proven HWND change. The helper now separates explicit
  PID-bound snapshot ownership from title readiness. Name/start time/session/
  HWND/native owner/foreground checks remain; click/wheel require exact `3DMark`
  title twice. Failure diagnostics report the rejection and a fresh census.
  Independent review, PowerShell/C# grammar, 18 structural checks and all 25
  mocked handoff checks pass. Native operation now passes C262; the failure
  branches are not fully hardware-qualified.
  The original helper/proof are archived under `ui-helper-history`; C264 will
  use the same new helper and harness with the clock-only backend. C260 must
  remain unlaunched because its candidate never qualified.
- C254 closes cleanly in 477.6 seconds without trace or Run. Standard P settings
  pass, but six visual callback waits total 157.843 seconds. Details is captured
  413.903 seconds after owner start; the 450-second preparation bound expires
  before its callback. This is harness overhead, with no new FPS evidence.
  C258 will use a fresh GPT-Sol reviewer with a small context and a reviewed
  600-second preparation allocation. Work1500/session1800, render600, cleanup
  reserve150, RAM/disk limits, visual40/armed20 and trace remainder650 stay fixed.
  No frozen C254 inputs are edited. C260 will be the matched clock-only case.
- C250 closes cleanly in 350.2 seconds before Run. Root's settings review misses
  its 40-second deadline during context compaction; the image shows the expected
  loaded standard P settings. No trace, arm or FPS result. The pinned READY
  watcher starts the controller 0.5 ms after observing READY, removing C246's
  27.7-second executor gap. Owner11/controller13+callback3/UI25/publication43/
  trace50 CPU checks pass; the native trace-clock check remains unqualified.
  C254 will retain the same sources, backend and limits with new case identifiers
  and a dedicated sole PNG reviewer. Root must not publish concurrent callbacks.
- Source `f7c431d6` completes instrumented stock Windows Fire Strike in C220–C222.
  Driver 617.14, GSP ABI 615, physical RTX 2070, 8 vCPUs/8 GiB RAM/4 GiB reservation.
- C220 baseline graphics 5696, GT1 19.563662 FPS, GT2 33.734764 FPS.
  Server `hybrid:100` and BAR1 pool64 do not show graphics gains. Defaults stay
  server polling0/pool32. Single runs with varying host activity are diagnostic.
- Physical graphics clocks during C220 heavy load reach 1830–1860 MHz, memory
  7000 MHz. Guest telemetry's 1410 MHz is not proof of an unboosted physical card.
- `b73a1da7` adds optional backend markers sharing the host-control clock,
  bounded rings, overlapping-snapshot analysis, and fresh Fire Strike validation.
  1001 library tests pass; two existing ignored. Six analyzer checks pass.
- That build boots Windows and passes three D3D11 readbacks. C225 proves marker
  dumps work, but aborts on RAM before Run. No new render FPS or critical path is
  established. C223–C226 have no active owned VM/backend/TPM/TAP. C226 reaches
  standard Fire Strike, but exceeds preparation time before Run. Its incomplete
  startup marker coverage and UI orchestration gaps are recorded in C226-report.
- `vk-unit-timing --calibrate` adds optional raw CPU/GPU clock pairs and CPU
  endpoints. Native KHR calibration and all 65536 output words pass; deviation
  is about 2.1 µs. CPU-only capability/wrap checks and warning-clean build pass.
  This low-priority native pilot is not a performance baseline. The guest pilot
  below uses the same binary; guest-to-host clock mapping remains pending.
- Fresh `linux-calibration-oct09a` closes cleanly in 22 seconds, stock615/no-helper,
  4 vCPU/4 GiB RAM. All output words pass. All ten samples prove incompatible
  DEVICE calibration (`~1.685e10` ticks) and physical GPU query (`~1.79156e18`)
  epochs at 1 ns/tick, 64 bits. Guest boundary attribution is invalid. The GPU
  clock correction below now passes the repeated hardware check; no FPS effect
  is established.
- `d42dc40b` separates logical GSP CPU/interval time from a private RO physical
  GPU clock. All identified GPU timestamp paths share it; pure emulation and
  reset continuity are preserved. 1010 library tests, 15 client tests, release
  build, independent review and AST update pass. `f2b22d28` extends the offline
  analyzer to complete paired captures with explicit input/resource limits;
  eight analyzer tests pass.
- Fresh `linux-calibration-oct09b` passes all ten raw timestamp brackets and
  65536 output words using the unchanged probe. It closes cleanly in 18 seconds.
  Calibration deviation remains 68–100 µs; no fine latency split or FPS gain.
  Candidate artifact: `C227-build`, backend SHA256 `9d280695f8bc78b9bbcda330668895ee12779ed364cfbe09fd0d9ab4a408744d`.
- C228 closes before VM/backend launch. The frozen launcher expects a checksum
  basename missing from C227; binary/source hashes are correct. udev changes the
  TAP MAC after capture, so exact manual cleanup runs under the original locks.
  No FPS. Fresh C229-build has both checksum aliases for the unchanged clock
  binary; the next owner must settle udev before sealing TAP identity.
- C230 proves those startup corrections and passes three D3D11 readbacks.
  Launcher READY reports PID8356, then the legacy screenshot helper cannot find
  its main window. No failure-time census establishes why. No Run/FPS. All owned
  resources close cleanly in 194 seconds; pre/post driver health remains good.
  Preparation retains 19 paired snapshots, all 716 selected host spans and
  248352/249130 marker IDs; 778 lost to contention. See C230-report.
- `tools/windows/3dmark-ui-step.ps1` replaces the immediate legacy selection
  with a ten-second same-session window/foreground bound, optional launcher PID
  pin and actual screenshot hash/size/time proof. Failure captures bounded window
  diagnostics and a separate fresh image. Thirteen CPU grammar/structure checks
  pass. C232 proves normal PID-bound snapshot/click operation on stock Windows;
  failure capture remains unqualified.
- C232 stops before Run because the separate visual-review callback rejects
  `dx11-page`: its validator forbids digits. D3D11 readbacks and four fresh UI
  images pass. Exact callback failure and image are preserved; no Fire Strike
  detail/settings, trace, arm or FPS. All owned resources close cleanly in
  332 seconds; pre/post driver health remains good and all frozen pins match.
  Forty-three preparation snapshot pairs retain all 716 selected host spans and
  346805/347881 marker IDs; 1076 IDs are lost to contention. See C232-report.
- C236 stops in preflight before VM/backend launch: root froze the runtime as
  0500/0400, but `run44.py` preflight runs as `silas`. Cleanup completes in
  0.261 seconds; frozen inputs remain unchanged. C240 derives the same reviewed
  scripts with case identifiers changed, root-owned 0555/0444 inputs, and actual
  UID1000 readability/hash checks of all 42 external and 24 runtime inputs.
  Owner11/controller6+callback3/UI25/publication42 checks pass.
- C240 passes five fresh UI captures through the settings tab. The executor
  spends over 40 seconds reviewing settings, so the controller correctly aborts
  before trace/arm/Run. All visible settings match the archived C220 standard P
  configuration; the custom-run button belongs to the selected settings tab.
  The controller returns to Details before Run. Cleanup completes in 371 seconds
  with good driver health. Forty-eight paired captures retain all 716 selected
  host spans and 386020/387046 marker IDs; 1026 IDs are lost to contention.
  C244 changes case identifiers only and uses a separate actual screenshot
  reviewer. All seven visual gates pass within their deadlines. It then stops
  before Run: the trace remainder is `7863,5350053`, about 7200 seconds above
  the actual remainder and invalid for Python's decimal parser. Cleanup completes
  in 384 seconds; the process trace stops cleanly. No arm, Run or FPS. Fifty-one
  paired captures retain all 716 selected host spans and 411515/412688 marker
  IDs; 1173 IDs are lost to contention. See C244-report.
- C246 fixes only the private controller's trace-time contract: normalize known
  .NET date kinds to UTC, capture one UTC now, publish invariant UTC strings and
  a typed JSON number. The host checks schema, aware UTC, a finite 0–700 second
  interval and agreement with the timestamp pair within 1 ms. The existing
  minimum of 650 seconds remains. Root reruns owner11/controller6/UI25/
  publication42/trace50 CPU checks and verifies actual UID1000 reads/hashes of
  all 25 frozen inputs and 42 external pins. C231 backend and every workload,
  resource, scheduling and time limit stay unchanged. Its fresh Windows
  diagnostic closes cleanly in 489 seconds. The settings screenshot still shows
  a loading modal; required values are absent, so the reviewer refuses approval.
  The controller reaches the preparation deadline. No trace, arm, Run or FPS;
  native timing remains unqualified. Five successful image-review waits total
  140.945 seconds. READY-to-controller preparation adds 27.726 seconds of host
  wall time; that executor gap can be removed. The settings checkpoint arrives
  418.252 seconds after owner start, leaving too little preparation margin.
  Preserve C246-report and C246-preparation-timing; guest UTC stays separate.
- `c8e8ea4d` retains early TSG SET0xa06c0103 requests and adds
  GET0xa06c0104. Both first-scheduling paths replay the latest accepted revision.
  Physical refusal/output and exact allocation lifetime are preserved. The
  independent review caught and verified a fix for display/family allocation
  bypasses and old children/promoted buffers. 1016 library tests pass, two existing
  tests ignored. Release and AST update pass. C232 preparation records 206
  successful physical SET calls and 244 successful guest SET requests. Repeated
  requests and build-time replay prevent one-to-one count comparison. C262 now
  qualifies rendering; a matched comparison is required for an FPS claim.
  TSG DUP remains unqualified: VAS-only alias tracking cannot authorize that
  handle. No stock Windows use was found; implement shared last-alias lifetime
  if traces establish this requirement.

## Continue

1. Investigate C278's final-workload startup failure from the preserved archive,
   process trace and backend/VMM logs. C272 remains the last fully qualified
   baseline. Any new run needs a fresh frozen runtime, actual UID1000 input proof
   and one PNG reviewer. Preserve the strict 40-second normal and 30-second armed
   deadlines, typed guest trace margin and exact teardown; do not retry C278.
   An actual settings-loading modal permits one five-second wait and a fresh
   settings-only recheck. Loaded values, Details return, the native UTC trace
   remainder of at least 650 seconds, fresh archive and all four successful
   workload lifetimes remain required. No frozen edits or automatic retries.
   Verify actual `silas` reads and hashes after freezing each fresh runtime.
   The preparation budget is 600 seconds within the unchanged owner
   work/session budgets. Record physical clocks, driver health and exact cleanup.
   C250/C254 contain only preparation evidence; their missing marker IDs are
   contention losses. C254 retains 471865/473367 marker IDs and 716 host spans.
2. Distinguish control occupancy from serial frame stalls. HEAD costs contain
   physical calls. Marker notifiers are engine-wide; they prove neither GPU
   completion nor guest ACK. Preserve losses and per-stream endpoints.
3. Retain the corrected GPU clock domain, then obtain comparable native/guest
   samples and pilot GPU context
   visibility. C276 is a source-only ten-second `tu10x`/200 Hz and context-switch
   proposal with 61 CPU checks. It fixes C266's mapping-age check: validate
   freshness after evidence reads and again immediately before tool launch,
   after guest observation and guards. C266 remains disabled and unchanged.
   C276 remains disabled until a separate frozen
   owner and owner-attested Nsight index/PCI/UUID mapping are supplied. Use
   Nsight metrics help's PCI BDF to join the owner's UUID inventory, never its
   NVML index. `C282-mapping-draft` adds a bounded pure parser; ten CPU methods
   independently pass, including a full64 join and malformed/ambiguous/unsupported
   input refusal. Native output remains unqualified. Source review SHA256:
   `96d7ed2ec759de8f4c013bcec4762516a435b8458d6dececd7b9964d4e994455`.
   `C292-counter-draft` replaces direct tool spawn with a gate-closed helper:
   capture its pidfd before GO, recheck mapping freshness before exec, and clean
   only the dedicated tool subtree. SIGTERM records cancellation for cleanup.
   The pinned SMI path permits only the read-only BDF/UUID query. Independent
   61 collector and 28 gate CPU checks pass. Source review SHA256:
   `eea92680cb309136d9585c6c173e2eab2c05f1f557952f2d425ef333dcfffdf1`.
   C292 is still disabled, with no native tool qualification. `C284-draft`
   integrates it through a separate coordinator so the render observer remains
   outside tool cleanup. Cancellation during initialization and parent cleanup
   supervision need correction before freeze. Native mapping and post-render
   export remain unqualified. General
   counters and context residency do not establish a stall cause or guest
   ownership. Export only after rendering ends. No external
   benchmark code was imported.
4. Retain the TSG replay correction for correctness. C270 now provides a
   qualified clock-only comparison with the explicit preparation differences
   above, with no consistent graphics gain. C272 offlock also has no clear
   graphics jump. C278 is incomplete, so it provides no accepted transport FPS
   comparison. Preserve its queue-register/worker observations separately from
   performance claims. Qualify the startup path before a matched repeat; prepare
   the separate counter pilot and private USERD progress measurements meanwhile.
   C252/C256/C260 remain source-only predecessor proposals. C234,
   C238 and C242 remain launch-disabled.
   C248 is a source-only clock baseline with that correction; independent review
   confirms all 18 execution/test inputs differ from C246 only by case names.
   It remains launch-disabled. C230/C232/C240/C244/C246 have no FPS baseline.
   The lost early request is a
   confirmed correctness bug, not a proven FPS cause.

Read [render trials](../research/windows-render-trials-2026-10-09.md),
[latency attribution](../research/latency-attribution-2026-10-09.md), and
[GPU evidence](../research/gpu-workload-attribution-2026-10-06.md).
Private artifacts remain in `/mnt/vmstore/caraxes/3dmark-trials-2026-10-09/`.
Do not publish raw result ZIPs or process command lines. New scene recordings
are useful only after a material, qualified performance gain.
