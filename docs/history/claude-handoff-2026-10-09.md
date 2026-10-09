<!-- SPDX-License-Identifier: MIT -->
# Windows performance handoff, 9 October 2026

Continue in `/mnt/vmstore/caraxes/wt-windows-3dmark`, branch
`codex/windows-3dmark-2026-10-04`. Main `/home/silas/git/Caraxes` contains unrelated
work. Keep it untouched. User priorities: Windows rendering, Wayland desktop,
CUDA, release documentation, then more GPU models. Use host-backed GPUs and stock
guests. Floppy has priority; retain fixed RAM/disk floors and exact owned cleanup.

## Verified state

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
  requests and build-time replay prevent one-to-one count comparison. Render
  qualification remains pending; no FPS claim.
  TSG DUP remains unqualified: VAS-only alias tracking cannot authorize that
  handle. No stock Windows use was found; implement shared last-alias lifetime
  if traces establish this requirement.

## Continue

1. Reduce the measured preparation overhead before another render attempt.
   C246 never reaches the corrected native trace-clock check; no render timeline
   exists. Start the controller on fresh READY without intervening diagnostic
   tool rounds, keep reviews prebriefed, and handle a settings-loading image with
   a separately reviewed bounded readiness flow. A short native trace-clock
   qualification can isolate that fix from the full GUI sequence. No frozen
   runtime edits or automatic retries. Its corrected callback passes 42
   actual publication checks for all nine caller literals, unsafe names,
   mismatched evidence, closed sessions and duplicate proof. Root independently
   reran these checks. Before any future runtime launch, verify actual `silas`
   readability after root freezes the inputs. Preserve UI preset proof,
   fresh archive, four successful workload lifetimes, physical clocks, driver
   health and exact cleanup. Do not modify frozen C232 inputs or fabricate its
   rejected callback.
2. Distinguish control occupancy from serial frame stalls. HEAD costs contain
   physical calls. Marker notifiers are engine-wide; they prove neither GPU
   completion nor guest ACK. Preserve losses and per-stream endpoints.
3. Retain the corrected GPU clock domain, then obtain comparable native/guest
   samples and pilot GPU context
   visibility. Context collector drafts remain disabled and reference closed
   cases; create a fresh mapping and review before execution. No external
   benchmark code was imported.
4. Hardware-qualify the reviewed TSG replay correction and compare it with a
   clock-only backend using the same corrected UI flow, workload and host policy.
   C234 has the old callback and C238 is an unfinished proposal; both remain
   launch-disabled. C242 retains the old trace-time bug and must not launch.
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
