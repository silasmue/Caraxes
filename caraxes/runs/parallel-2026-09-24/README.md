<!-- SPDX-License-Identifier: MIT -->
# P20 evidence, 2026-09-24

Write-up: [docs/caraxes/parallel.md](../../../docs/caraxes/parallel.md). Tools: `caraxes/tools/p20*.{sh,c,py}`.
Guest clocks are UTC, host clocks are CEST (UTC+2). Device traces under `*/device/` are git-ignored.

| Directory | What |
|---|---|
| `e1-two-vms/` | vm1 slot 0 and vm2 slot 1 up, reservations, host nvidia-smi before and after, nvml gate per VM |
| `e2-concurrent/` | first concurrent run (build before the BAR1 fixes): nvprobe alone and concurrent, the concurrent cuda gate, the deadlock in the suites stage (`hang-*`), device logs and shadow lines (`dev-*`), `check.txt` |
| `e2b-aborted-bar1-leak/` | first fix build: the scan windows leaked BAR1 (no `RM_UNMAP_MEMORY`), vm1's store stalled; stopped |
| `e2c-bar1-unmap/` | standalone device: 256 BAR1 windows mapped and released with the unmap fix, 0 refused, 0 host asserts |
| `e2d-concurrent-fixed/` | final build: nvprobe alone and concurrent, the concurrent cuda gate (scored tests pass on both), then the host RM stall in `suite-test_async_streams` (`hang-*`); the gate verdicts read "exited before the verdict" because the runs were stopped there |
| `e3-isolation/` | the pattern tests in both directions with the positive control, device logs of a clean teardown (`dev-*`), `check.txt` (PASS), host Xid |
| `e4-scrub/` | scan before the free (pattern present), VM C's device and guest scans (zero), the 2 GiB scrub wait (`timing-*.log`) |
| `e4-failclosed/` | vm2 with an injected free failure: FAIL-CLOSED, exit 3 |
| `e5-refusal/` | vm3 on slot 2 refused by the D11 headroom rule; vm1 and vm2 unaffected |
