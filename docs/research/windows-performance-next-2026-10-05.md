<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Next Windows performance measurements

C166 establishes the current full regular boost baseline: Fire Strike P graphics 6015/6040, GT1 20.80/20.92 FPS, GT2 35.22/35.28 FPS. Actual high-utilization graphics/SM median was 1830 MHz. Requests were continuous; clocks remained dynamic. There is no causal boost-policy comparison yet. See [the qualification](windows-3dmark-2026-10-04.md).

Prioritized research:

1. **Submission and completion tails.** Obtain actual GT boundaries and correlate publication, reactor/device-lock waits, guest acknowledgement and GPU queues. Existing oldest-doorbell to host-event timing is not an exact submission fence. Separate diagnostics from scored runs; preserve data synchronization, shadow-plan barriers and channel ownership.
2. **Exact interrupt transport.** Tree writes are 57% of C162 callbacks, but only 4.702 s of handler time in its 275.8 s bracket. Measure transport and tail costs before optimizing decoding/publication. Repeated enable stores and acknowledgements are transactions. Do not replace them with their final values or map the physical host interrupt controller.
3. **Unused trace formatting.** `Trace::event/events` still prepare output with no sink; `Device::after_write` and `ring_polled_userds` format trace-only strings. The first candidate gates these consumers with `records_events()`. Always drain notes and execute reset, rings, MSI and counter updates. Preserve tracing when access filtering is disabled, and every error/RC path. Producer success-note allocation is a separate later change.
4. **USERD discovery and CPU placement.** The poller sleeps 1 ms and scans at most 32 channels per tick. Measure polled-only submissions and discovery tails before changing cadence. Auto pinning does not reserve SMT siblings or account for later vCPU migrations. Reject higher CPU cost without a measured benefit.
5. **Exact FE qualification.** The read-only mode already exists. Measure actual callback reduction with VM policy fixed, then compare matched FPS. Leave it optional if results remain unchanged or adverse.
6. **GPU resource/state costs.** Measure aperture, kind, residency, compression and mapping churn before changing memory policy. Current evidence does not establish ZCULL as the Fire Strike cause. Retain private backing, translation/lifetime checks and image/statistics gates.

Require matched resources, clock policy, immutable binaries and fresh complete archives. Keep diagnostics separate, interleave repetitions and retain health/cleanup checks. Callback percentages and CPU savings do not predict FPS. Steel Nomad/D3D12 Xid 31 remains unfixed.

Detailed source references and acceptance tests are retained privately in `/mnt/vmstore/caraxes/windows-3dmark-2026-10-04/windows-performance-research-2026-10-05.md`. No external code or benchmarks were copied.
