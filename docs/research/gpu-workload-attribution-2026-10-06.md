<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GPU workload attribution, 6 October 2026

**High GPU activity does not mean high instruction issue.** One clean physical
counter window during Windows Fire Strike shows approximately 91% GR activity,
45% SM activity and 5.2% SM instruction issue. This shows high engine activity
with low shader issue; it does not identify a cause or a loss relative to native.
No FPS improvement or complete Windows qualification is claimed.

## Physical counters during rendering

C214 retry collected the physical RTX 2070 with host Nsight Systems 2026.5.1,
`tu10x`, nominal 200 Hz, for ten seconds. API tracing and CPU profiling were off.
The counter interval is 17:57:31.216629–17:57:41.218923 UTC: 2,233 samples,
median spacing 4.491484 ms, with no overflow, invalid samples or error diagnostics.
The stock Windows workload process was active. Exact GT boundaries and guest-only
context attribution are unavailable; the `gt1` filename is a planned label.

| Physical metric | Mean | Median |
| --- | ---: | ---: |
| GR active | 90.71% | 99% |
| SMs active | 44.75% | 46% |
| SM instruction issue | 5.21% | 3% |
| DRAM read / write activity | 4.54% / 3.50% | 3% / 2% |
| GPC clock, correctly scaled | 1809 MHz | 1806 MHz |

In the same samples with GR activity at least 90%, mean SM activity is 48.71%
and issue is 5.69%. Approximately 80.43% of those samples have issue below 10%.
This avoids comparing unrelated peaks from different intervals. Concurrent NVML
snapshots show graphics clocks of 1815–1830 MHz, memory at 7000 MHz and GPU
utilization of 77–94%. Idle 1410 MHz is not a fixed execution clock limit.

SM activity includes resident warps; issue measures scheduler cycles that issue
instructions. These are different measures. Low issue cannot be converted into
an exact percentage of waiting, CUDA-core utilization or lost FPS. DRAM activity
does not measure memory capacity, effective compression or individual access
latency. Definitions and the device-wide scope are documented in the
[NVIDIA GPU metrics guide](https://docs.nvidia.com/nsight-systems/UserGuide/index.html#gpu-metrics).

The host desktop remained active. Its idle reference averaged GR 10.97%, SM
activity 1.49% and issue 0.45%; it is not a guest-cost subtraction. A preceding
1000 Hz render capture overflowed and is excluded from complete statistics.
The graphics-throughput capture was idle-like, with GR about 11%
and an unavailable RTCore metric. Its ROP, texture and cache counters cannot
explain the high-load interval.

## Windows workload probes

The original D3D11 probe completed 70 full-suite samples and 14 small-chunk
samples, including warmups. Independent checks passed the hardware adapter,
raw timestamp arithmetic, disjoint flags, hardware counts and recorded output
oracles. Raster checks are exhaustive; memory checks cover 16 distributed
4 KiB blocks per sample. Readback is outside workload timing.

| Workload | GPU median | CPU encode-through-completion median |
| --- | ---: | ---: |
| Fill, 1920 × 1080, four draws | 135.360 µs | 654.8 µs |
| Integer ALU, 64 rounds | 67.584 µs | 311.9 µs |
| Visible depth workload | 67.744 µs | 465.0 µs |
| Occluded depth workload | 6.048 µs | 473.0 µs |
| Memory, 128 MiB, 16 MiB chunks | 871.264 µs | 1566.7 µs |
| Same memory work, 1 MiB chunks | 1251.232 µs | 2453.7 µs |

The depth workload records 1,048,576 pixel-shader invocations when visible and
zero when occluded: early depth rejection works in this case. It does not
qualify every game depth format, compression mode or driver path.

Reducing memory chunks from 16 to 1 MiB changes two/eight dispatches into
32/128 for the 32/128 MiB cases, with equal logical work and invocation counts.
GPU intervals grow 42.8%/43.6%. The interval includes constant-buffer updates,
driver synchronization and possible CPU starvation between commands; it does
not isolate dispatch execution or establish an emulator optimization. There is
no native Windows control or matched clock measurement for these short probes.
The smaller working set's CPU total does not increase consistently. Component
medians are independent and must not be added.

## Linux compute controls

The same original Vulkan binary ran natively and in a fresh stock-driver guest
with no helper module. Each case has eight warmups and sixteen measured samples.
Every output word and all five native/guest checksums agree. Timing excludes
upload, preconditioning and complete verification.

| Case | GPU native / guest, µs | CPU submit + fence native / guest, µs |
| --- | ---: | ---: |
| Stream, 16 MiB per buffer | 101.584 / 121.792 | 128.031 / 246.296 |
| Stream, 64 MiB per buffer | 389.040 / 466.912 | 419.653 / 600.725 |
| ALU, 32 rounds | 8.880 / 10.480 | 40.305 / 110.215 |
| ALU, 256 rounds | 46.080 / 56.160 | 75.576 / 180.310 |
| ALU, 1024 rounds | 207.968 / 209.952 | 239.231 / 335.146 |

Native short/stream cases commonly sampled 1710 MHz, versus guest 1410 MHz.
That clock ratio is about 1.21, close to their GPU-time ratios. Long ALU sampled
similar clocks and differs by about 1% in GPU time, while CPU submit-plus-fence
differs by about 96 µs. These clock samples do not resolve individual
microsecond intervals. This supports investigating submission/completion costs;
it does not establish equal game performance or memory bandwidth.

## Lifecycle and evidence limits

- C213 and C214 used the immutable C209 backend `5db44092…`, source `c0e9eef5`,
  and paired VMM `7f3682ea…`, source `5defb5c5`. Worker off, hybrid polling,
  local IRQ on, trapped RPC HEAD writes, slice compression and finite VM boost.
  Each guest had 8 GiB RAM, 4 GiB framebuffer and eight vCPUs.
- Both Windows sessions later hit the host RAM floor while an unrelated Nix
  job grew. C214's clean counter capture completed before the guard failure.
  The full session is unqualified; post-health is unavailable. Host CPU/resource
  interference remains a possible contributor to the measured behavior.
- C214's first Fire Strike attempt lost focus to `WindowsTerminal.exe` before
  counter collection. Its archive reports `CANCEL`, `window_focus_lost` and
  no workload FPS. A scheduled screenshot helper overlapped the focus loss.
  The retry ran without any scheduled UI/capture task during rendering, but
  resource teardown prevented a complete result.
- Both exact Windows owner/backend/VMM process sets exited. Available closeout
  checks pass, trace checks are explicitly skipped, NVIDIA remains bound and no
  new host Xid appears. These are not substitutes for guest post-health.
- The Linux case completed and stopped cleanly, with stock-module/no-helper
  checks and no new host Xid.

Private evidence lives under `/mnt/vmstore/caraxes/windows-3dmark-2026-10-04`:
`C214-unit-audit.json`, `C214R2-counter-audit.json`, the
`C214-host-counter-r2-gt1-general-200hz` packet, `linux-unit-oct06a-analysis.json`,
and both `C21{3,4}-resource-abort-root-closeout.json` records. Raw result archives
contain machine data and remain local. The tools are in `tools/graphics/` and
`tools/windows/d3d11-benchmark.ps1`; they contain no imported benchmark code.

## Next backend investigation

1. Align GPU context switches, guest workload boundaries, backend command
   publication and completion. Capture without launching a new interactive
   task during the scene; keep host resource contention out of comparisons.
2. Split host A613 ioctl execution from RM locking, firmware work and guest
   reply polling. The previous [worker comparison](host-control-worker-2026-10-06.md)
   reduced HEAD-write time without improving GT FPS; worker mode remains off.
3. Correlate low instruction issue with dependencies, tiny batches and scheduler
   gaps. Also test memory latency/address translation and cache locality. Low
   DRAM traffic argues against saturated external bandwidth, not against every
   memory problem. Fixed-function saturation remains unmeasured under load.
4. Measure the authoritative reply HEAD before proposing shared reads. Preserve
   publication/reset ordering and guest isolation; unknown host controls still
   require real status and output. Do not cache or synthesize A613 replies.
5. Repeat one bounded, quiet-host render comparison only after a measured
   change. Neither a larger BAR nor full CPU spinning is justified by this
   packet. New recordings are unnecessary until a real performance improvement.
