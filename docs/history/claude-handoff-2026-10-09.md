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

1. Correct the callback name validator in a fresh case. CPU-test the actual
   callback publication for every literal UI checkpoint, including `dx11-page`,
   plus unsafe names, mismatched evidence, closed sessions and duplicate proof.
   Then finish an admitted, bounded render timeline. Preserve UI preset proof,
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
   C234 is a source-only baseline proposal; it must receive the callback fix
   before use. C230/C232 have no FPS baseline. The lost early request is a
   confirmed correctness bug, not a proven FPS cause.

Read [render trials](../research/windows-render-trials-2026-10-09.md),
[latency attribution](../research/latency-attribution-2026-10-09.md), and
[GPU evidence](../research/gpu-workload-attribution-2026-10-06.md).
Private artifacts remain in `/mnt/vmstore/caraxes/3dmark-trials-2026-10-09/`.
Do not publish raw result ZIPs or process command lines. New scene recordings
are useful only after a material, qualified performance gain.
