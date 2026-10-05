<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Overnight performance investigation

No performance gain is accepted. Windows workload completion remains inconsistent.
The memory fix below addresses an independently reproduced initialization failure.

## Memory qualification

- C170 failed with Code 43 after reboot: a guest GPA alias collided numerically with
  a different registered host IOVA. No benchmark or D3D fixture passed.
- Commit `514be02c` separates these namespaces and refuses untranslated shadow
  leaves. Independent source review, 927 unit tests, two integration tests and the
  release build passed; one unit test remains ignored.
- Immutable backend `0a79272e...` booted C173, C174 and C175 with stock Windows
  617.14 and passed three fresh D3D11 readbacks in each session.
- All subsequent registrations used high IOVAs. Hardware did not recreate C170's
  low-address collision; regression tests cover that layout.

## Windows polling experiment

Same backend, VMM, stock drivers, 4 GiB FB, 8 GiB RAM, eight vCPUs, slice
compression, BAR2 memfd, direct completion, FE off and finite VM boost policy.
Backend scored diagnostics and capture were off. Separate read-only physical
clock and host-task samplers were present.

The doorbell reader used CPU 11 and host-events CPU 10. vCPUs excluded CPUs
2, 3, 10 and 11. Other host and backend tasks remained unrestricted; this does
not reserve the entire physical cores from host activity.

| Case | Outcome | Graphics | GT1 FPS | GT2 FPS | Combined FPS |
| --- | --- | ---: | ---: | ---: | ---: |
| C173 hybrid, run 1 | Locally validated | 5472 | 19.8834 | 29.6159 | 20.1561 |
| C173 hybrid, run 2 | Locally validated | 5570 | 18.7931 | 34.0560 | 10.9797 |
| C174 spin | Rejected: GT2 repeats GT1's result | Not accepted | | | |
| C175 hybrid control | Rejected: Physics repeats GT2's result | Not accepted | | | |

- C173 graphics vary by 1.8% of their mean; Combined FPS drops 45.5%. Neither
  graphics alone nor isolated sub-results establish a gain.
- C173 high-utilization physical SM median is 1830 MHz in both global windows;
  memory stays at 7000 MHz. Maximum temperatures are 80/81 C, with no sampled
  hardware thermal slowdown. Idle P0 at 1410 MHz is not a fixed clock limit.
- C173 reader execution/runnable delay is 30.248/8.382 s and 23.894/10.156 s;
  host-events is 1.328/3.767 s and 0.792/2.305 s. These global-window counters
  exclude subsecond edges, sleeping and lock waits; they are not latency tails.
- No individual GT boundaries or guest-only physical utilization are qualified.
  Alignment assumes zero host/guest clock offset; request brackets are imprecise.
- All three sessions stopped with NVIDIA bound and no new host Xid. C173 passed
  pre/post guest health and restored monitoring. C174/C175 lack post-health and
  restoration after automatic failed-collection teardown; keep that gap visible.
- Frozen originals retain an affinity helper naming error; corrected external
  helpers applied the declared masks. No runtime manifest or guard was bypassed.

## Rejected result investigation

A short QEMU maintenance boot without a GPU verified that the stored C174/C175
archives still match their collected hashes. Delayed file rewriting does not
explain the rejection. Application events contain no matching 3DMark crash or
Application 1000, and the interval has no System 4101. Queued historical WER
reports are present; their report times are not original fault times. The
searched 3DMark log directories yielded no recent text logs.

Spin is not established as the cause: the hybrid control also fails, in a CPU
workload. Do not infer that either failure is a GPU fault. Capture workload
process exit status and fresh per-workload logs before another long comparison.

## Linux timing

- The first attempt failed guest admission and masked its original exception
  during cleanup. Later packets preserve each command's stdout, stderr and return
  code, and retain the first exception separately from teardown errors.
- The baked image blacklists NVIDIA autoload. Ordinary stock `modprobe` and
  `nvidia-modprobe -c 0` followed by `nvidia-smi` initialize the device. C passed
  the stock-module gate, then stopped because the harness parsed an SSH warning
  as a hash. D separates stdout/stderr and retains strict hash validation.
- D passed all three native and guest Vulkan probes with the same immutable
  binary, eight measured samples per case and two warmups. Stock kernel
  `6.8.0-136-generic`, NVIDIA `615.71.09`, installed/loaded source versions and
  absent `caraxes_dma_range` all passed. Every full image passed pixel checks;
  fetched guest images exactly match native image bytes and remote hashes.

Medians in microseconds:

| Size:draws | GPU render native / guest | CPU submit + fence native / guest |
| --- | ---: | ---: |
| 256:1 | 5.440 / 5.280 | 58.936 / 133.250 |
| 256:64 | 27.280 / 27.712 | 81.866 / 183.475 |
| 1024:64 | 350.288 / 350.192 | 704.796 / 800.021 |

GPU render-plus-copy medians also agree closely: 29.264/29.168,
51.296/51.328 and 672.400/672.320 us. The CPU submit-plus-fence difference
is about 74–102 us in these small workloads. Investigate submission, scheduling
and completion before attributing this gap to GPU execution. These intervals
overlap; do not add CPU fence time to GPU timestamp time.

This is a headless Vulkan diagnostic with automatic CPU placement, not a game,
Wayland, CUDA or codec qualification. Eight samples do not characterize latency
tails. Physical before/after snapshots cannot prove equal clocks inside the
microsecond GPU intervals. No performance change was made or promoted. D stopped
cleanly in 26.25 seconds; all owned processes exited, NVIDIA remained bound and
no new host Xid appeared.

## Linux polling controls

E hybrid, F spin and G hybrid control each passed 512 measured samples per case,
eight warmups, stock-module/no-helper checks, image validation and exact CPU-mask
checks before/after measured cases. All three stopped cleanly without a new Xid.
The immutable backend and VMM were unchanged. The separately pinned probe raises
only its CLI repetition ceilings and matching help text; shader and timing code
are unchanged.

Reader CPU 11, host-events CPU 10 and the eight vCPU masks were identical across
these sessions. Observed other backend workers allowed CPUs 0–15. Host tasks and
IRQs remain unreserved. Native and guest images match byte for byte in every case.

Guest CPU submit-plus-fence medians in microseconds:

| Size:draws | E hybrid | F spin | G hybrid control |
| --- | ---: | ---: | ---: |
| 256:1 | 123.220 | 168.315 | 127.370 |
| 256:64 | 142.240 | 155.310 | 146.520 |
| 1024:64 | 733.995 | 749.290 | 718.341 |

For 256:1, empirical nearest-rank p99 is 1183.940/2590.911/1174.250 us;
GPU render-plus-copy medians are 28.672/29.152/28.672 us. Full spinning did
not improve these trials. Retain hybrid; do not promote spin as an optimization.
This does not establish a universal spin regression or Windows FPS effect.
There is one spin session, two hybrid sessions and no system-wide CPU isolation.

The longer GPU intervals change as the physical GPU boosts: before/after snapshots
reach 1860–1875 MHz, while the actual microsecond clock intervals remain unknown.
Native samples also contain CPU-wait outliers. Obtain phase-aligned submission,
completion and scheduling observations before changing production behavior.

Private evidence is under
`/mnt/vmstore/caraxes/windows-3dmark-2026-10-04`: C170/C173/C174/C175 root
qualifications, strict archive checks, C173 global analysis, C176B maintenance
collection, result hash recheck and `linux-night-oct06-root-closeout.json`.
Completed Linux evidence: `linux-night-oct06d-root-qualification.json`,
`linux-night-oct06d-analysis.json`, pinned D packet, six logs and image pairs.
Polling evidence: E/F/G root qualifications, `linux-night-oct06efg-analysis.json`,
pinned packets/probe, placement records and native/guest logs/images.
Raw archives and logs contain private machine data and remain local.
