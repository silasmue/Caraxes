<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Verification methods

How both architectures are tested and measured: the gates that decide pass or fail, the
census and profile that attribute cost, the coverage map, the ABI drift check, the emu-boot
check and the lab kit. A number counts only with its run recorded and 0 host Xid on the
device's pid.

## Gates

- `tools/caraxes-gate.sh` runs five gates against a VFIO Leandro VM, one per invocation:
  `nvml` (the guest's `nvidia-smi -q` against two host readings over 11 checks), `cuda` (12
  scored tests from cuda-init to rlprobe, convburn and robustness, one ssh call each so
  every test gets its own device-trace window), `gpu` (v1's stages a guest can carry: smi,
  memory, kernel, torch bit-identical to a native reference, robustness, own, encode),
  `vdisplay` and `display` (flips and compositor stages within 60 +- 0.5 Hz).
- The verdict rules are v1's: one JSON line on stdout, exit 0 pass, 1 fail, 2 skip, skip
  only for a missing precondition, binary verdicts, a dying gate still reports a fail
  ([TESTING.md](../TESTING.md)). A green gate means what a green v1 gate meant.
- A failed `cuda` test is re-run once under the v1 tracer (`LD_PRELOAD=libnvrm_trace.so`)
  and its row carries the failing CUDA call, the last non-`NV_OK` ioctl and every device
  answer in the window that was not `NV_OK`.
- Each run writes `caraxes/runs/gate-<name>-<date>/` with `verdict.json`, the transcript,
  the guest logs and the trace windows. The v1 path on the 615 image is gated by
  `caraxes/tools/v1-gates.sh` with the same masks.

## Benchmarks

- `caraxes/tools/bench-run.sh series` interleaves the targets inside every rep (native, v1,
  v2), restarts the device per rep, times inside the target and records GPU state, running
  VMs, device and VMM CPU; a run is marked concurrent when another VM's device used more than
  20 % of a CPU. `bench-summary.py` prints `median [min-max] n` and the ratio to native;
  `bench-hostwatch.sh` stops the device when the host's `nvidia-smi` answers slower than 5 s.

## Census and profile

- Census: `census-cu.c` runs one driver-API operation at a time and brackets each repetition
  with two writes to `NV_PBUS_SW_SCRATCH(30)`; `census-table.py` splits the device trace at
  the markers and gives per operation the traps by class, page and register, the RPCs, the
  shadow plans, the doorbells and MSIs; a background of 100 ms idle windows is subtracted.
- Profile: `CARAXES_PROFILE=1` aggregates per row a count, device wall time, the largest
  time, a status histogram and the answer's label, for trapped accesses, RPCs, controls,
  allocations, events, host RM calls, shadow call kinds and window hits, per device and per
  guest client; `SIGUSR2` dumps a TSV; a workload's profile is the dump after it minus the
  dump before. A hook costs 84 ns per trapped access. `profile-rank.py` ranks by count times
  mean latency and joins the ABI snapshot.
- Coverage map: 29 workload windows on a device built with `-C instrument-coverage`
  reached 208 of 829 routed controls and 931 of 1464 device functions; `ctlsweep.c` issues
  every user-callable getter once through the stock ioctl path (119 of 172 generated).

## Checks without a GPU

- `tools/check.sh`: formatting, clippy, unit tests, the licence headers, the ABI generation
  of `nvrm-sys`, the `gsp-abi` drift step ([gsp-abi-615](gsp-abi-615.md)), and with
  `CARAXES_CHECK_EMU_BOOT=1` the emu-boot step: `tools/emu-boot-check.sh [--driver 610|615]
  [--chip tu106|gb206]` boots the synthetic GPU in pure emulation (no host RM, no
  reservation, no GPU) and exits 0 on `RmInitAdapter succeeded` and a working `nvidia-smi`.
- Unit tests in the device crate cover the queue, the boot model, the static configuration,
  the chip files (`chipctl::tests`), the pool and the memfd.

## The lab kit

- `caraxes/tools/lab-kit.sh pack --ogkm <tree>` builds a self-contained pack; `run` on the
  lab machine captures a card in nine steps (preflight, capture, RM tools, a sweep of 164
  getters, traces under the tracer, display, extras, scrub, pack); `verify` checks the
  tarball; `lab-ingest.py` files it at home ([contributing-a-capture](contributing-a-capture.md)).

## How a new workload is measured

1. Add it as a `bench-run.sh` metric printing one `# NAME key=value` line.
2. Start the VM in the configuration under test with `CARAXES_PROFILE=1` and the host watch.
3. `kill -USR2` before and after; run native and v2 interleaved, 3 reps, `V2_RESTART=1`.
4. Rank the profile delta; read the ratio, device CPU against wall time, and step p90 against
   median for stalls.
5. Compare the touched surface with the coverage map; extend the sweep for untouched getters.
6. Keep the dumps, the markers and the host Xid lines of the device's pid; count numbers only
   with 0 host Xid there.

## Depth

- [research/methods.md](../research/methods.md): every tool with its lines.
- [caraxes/testing.md](../caraxes/testing.md), [caraxes/benchmark.md](../caraxes/benchmark.md) §2,
  [caraxes/coverage-map.md](../caraxes/coverage-map.md), [caraxes/optimization-backlog.md](../caraxes/optimization-backlog.md) §1.
