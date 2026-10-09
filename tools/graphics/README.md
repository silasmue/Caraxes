<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Vulkan compute unit timing

`vk-unit-timing` is an original, headless TU106/NVIDIA Vulkan diagnostic. Build
it with `build.sh /absolute/output/vk-unit-timing`; the script embeds both
locally compiled SPIR-V shaders in the executable. `--self-test` runs only CPU
arithmetic, timestamp-wrap and calibration-capability checks. No Vulkan API is
called by that mode.

Examples for an owner-controlled native/guest harness:

```sh
vk-unit-timing --mode alu --iterations 256 --samples 7 --warmup 2 --seconds 120
vk-unit-timing --mode stream --mib 64 --samples 7 --warmup 2 --seconds 120
```

ALU transforms 65,536 device-local words with 32, 256 or 1024 runtime loop
iterations. Stream reads and writes two device-local 16 or 64 MiB buffers;
one host-coherent staging buffer is used only for upload and full verification.
Stream's reported logical byte count is one read plus one write per element,
not measured DRAM traffic. The GPU interval brackets an explicit write-order
barrier and pre-recorded compute dispatch with TOP/BOTTOM timestamp queries.
JSONL keeps warmups and measured
samples separate and reports CPU submit, fence wait, query retrieval, GPU ticks
and elapsed time. Upload, preconditioning and complete output readback happen
outside the measured command buffer. Every word is checked against a CPU
reference. Exit status 0 requires all samples and exact verification.

Add `--calibrate` to record CPU endpoints and raw CPU/GPU clock pairs before and
after each sample. It requires `VK_KHR_calibrated_timestamps` or the EXT fallback,
with both DEVICE and Linux CLOCK_MONOTONIC domains. Each pair includes API-call
brackets and `max_deviation_ns`. Calibration is off by default and fails explicitly
when unsupported. Assess uncertainty, drift, timestamp wrap and calibration age
before correlating endpoints. Guest CLOCK_MONOTONIC still needs a separate mapping
to the host backend clock; these rows do not establish a latency split.

Per-job wall/GPU time must stay under one second; fences have a ten-second
failure bound. A 120-second maximum alarm and internal deadline bound the
process, while an external harness should also apply its own timeout. One run
does not establish sustained FPS, physical DRAM bandwidth, clock equivalence,
or a depth-rendering result.

## Offline Nsight GPU-metrics summary

Export an owned Nsight Systems report to SQLite, then summarize it without
modifying the export:

```sh
nsys export --type=sqlite --output=/absolute/capture.sqlite /absolute/capture.nsys-rep
python3 tools/graphics/nsys-metrics-summary.py /absolute/capture.sqlite > /absolute/summary.json
```

`--device-index 0` is the default; `--start-ns` and `--end-ns` select inclusive
Nsight event timestamps. The analyzer requires an unambiguous device and metric
group, joins on both `typeId` and `metricId`, and checks the captured metric-set
definition in `StringIds` before reporting any values. It applies each embedded
submetric multiplier, including TU10x clock MHz and graphics PCIe GB/s, while
keeping NVIDIA's original metric names and units. Missing or ambiguous unit
definitions fail the analysis. JSON includes sample cadence, gaps, invalid-value
counts, coverage and diagnostics. These counters cover the whole physical GPU;
the summary does not attribute activity to a guest or subtract an idle baseline.

## Linux idle CPU census

`idle-census.py --seconds 60 --threads` reads guest CPU, process/thread and
interrupt counters from `/proc`. On the host, restrict process rows with
repeated `--pid` arguments using exact VM/backend PIDs from the owner harness.
Redirect JSON to a fresh evidence file. It does not access the GPU or change
services. Duration is bounded to 1–120 seconds; `100%` means one CPU.

Compare headless idle before and after a verified GPU job with GNOME Wayland
idle. Keep guest CPU separate from host reader spinning and steal time.
See [the idle diagnostic plan](../../docs/research/idle-and-host-timeline-2026-10-09.md)
for attribution limits and optional host ioctl timestamps.

CPU-only accounting checks:

```sh
python3 -m unittest discover -s tools/graphics -p test_idle_census.py -v
```
