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
  clock correction is in progress; no FPS effect is established.

## Continue

1. Finish an admitted, bounded render timeline before another performance flag
   trial. Preserve UI preset proof, fresh archive, four successful workload
   lifetimes, physical clocks, driver health and exact cleanup.
2. Distinguish control occupancy from serial frame stalls. HEAD costs contain
   physical calls. Marker notifiers are engine-wide; they prove neither GPU
   completion nor guest ACK. Preserve losses and per-stream endpoints.
3. Correct the proven guest GPU-clock mismatch and repeat the original calibrated
   probe in a fresh stock guest. Keep logical GSP OS time separate from physical
   GPU time. Then obtain comparable native/guest samples and pilot GPU context
   visibility. Context collector drafts remain disabled and reference closed
   cases; create a fresh mapping and review before execution. No external
   benchmark code was imported.
4. Review the lost pre-build TSG timeslice request next: `ctrl.rs` only forwards
   SET0xa06c0103 after a mirror exists; the early CUDA fallback ACK loses it.
   Exact owned-object lifecycle and host scheduling validation need a separate
   patch. It is a confirmed missing state replay, not a proven FPS cause.

Read [render trials](../research/windows-render-trials-2026-10-09.md),
[latency attribution](../research/latency-attribution-2026-10-09.md), and
[GPU evidence](../research/gpu-workload-attribution-2026-10-06.md).
Private artifacts remain in `/mnt/vmstore/caraxes/3dmark-trials-2026-10-09/`.
Do not publish raw result ZIPs or process command lines. New scene recordings
are useful only after a material, qualified performance gain.
