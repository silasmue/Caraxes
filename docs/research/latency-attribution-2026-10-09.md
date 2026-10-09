<!-- SPDX-License-Identifier: MIT -->
# Locating render latency

The long Linux ALU probe reports 239.231 µs native versus 335.146 µs guest CPU
submit-through-fence medians: about 40% more time, or 71% of native throughput.
Its GPU medians are 207.968 and 209.952 µs. This supports investigating submission
and completion, but does not explain games at roughly 30% native FPS. That ratio
would mean about 233% more frame time. A matched native game reference is still
missing; online scores are targets, not an overhead measurement.

Do not subtract independent component medians or add nested profile totals.
Compare complete, aligned samples. Existing evidence and clock limitations are
in [GPU workload attribution](gpu-workload-attribution-2026-10-06.md).

## Backend observations

Set `CARAXES_PROFILE=1` and `CARAXES_PROFILE_HOST_TIMELINE=1`. Existing SIGUSR2 and
exit dumps now produce three files under `CARAXES_PROFILE_OUT`:

| File | Observations |
|---|---|
| `.TAG.tsv` | Existing aggregate costs |
| `.TAG.host.tsv` | Completed A612/A613 host ioctl intervals |
| `.TAG.markers.tsv` | Physical doorbells, drained host notifiers, requested raises, published local IRQ causes |

- Host spans and markers share the exact monotonic anchor. Unix capture brackets
  support coarse alignment; they do not calibrate guest QPC or the GPU clock.
- Markers retain known channel/generation or notifier/vector identifiers. Folded
  IRQ causes also retain epoch/ticket. Engine events cannot identify one channel.
- Doorbells are recorded after the physical write. Raise requests precede the
  call and may be rejected by generation checks. Cause publication is observed
  after successful publication and wake. None proves guest delivery or ACK.
- Host notifier events are engine-wide. Host desktop activity can notify an idle
  guest; events do not identify the completion of a particular dispatch.
- The optional rings retain 32768 host spans and 65536 markers. Actual storage
  bytes and loss counters appear in each snapshot. Overlapping dumps do not
  consume rows. Default-off mode allocates no marker ring.
- Marker writers use `try_lock`, with explicit contention/poison drops and no
  per-event allocation or file I/O. Existing aggregate/host-span observation
  still takes the profile mutex. Dumps can perturb a diagnostic run.

Analyze snapshots from the same process:

```sh
python3 tools/analyze-backend-timeline.py \
  run/host-profile.1.host.tsv run/host-profile.2.host.tsv \
  run/host-profile.1.markers.tsv run/host-profile.2.markers.tsv \
  --begin-ns BEGIN --end-ns END --output fresh-analysis.json
```

Bounds use nanoseconds relative to the common backend anchor. Without them the
report covers startup through the latest snapshot. The analyzer deduplicates
sequence IDs, rejects different anchors and conflicting rows, preserves statuses,
reports missing IDs, and computes interval unions instead of summing overlaps.
One-second buckets show control occupancy and event counts. Doorbell interarrival
gaps are grouped by channel incarnation; they are not queue latency. No nearest
event matching invents a dispatch/completion relationship.

## Hardware check

The immutable marker build uses source `b73a1da7`, backend SHA256
`a91fc1efa94e339facf0886e90cd7aa58b60b441892e84010634a5502ee1e7ac`.
It passes 1001 library tests, with two existing tests ignored, and six analyzer
checks. Windows C224/C225 boot and pass the three-frame D3D11 readback gate.

- C223 fails preflight before backend/VMM launch because the new artifact lacks
  the launcher's `SHA256SUMS`. The fresh C224 packet pins that file and records
  TAP identity after link-up, allowing early-failure cleanup after MAC assignment.
- C224 reaches its preparation deadline and exits cleanly before any benchmark.
- C225's first SIGUSR2 dump proves both streams share the exact anchor. It retains
  32330 markers without overflow, contention, poison or invalid rows, plus two
  completed host calls. Ring storage is 8 MiB markers and 1.5 MiB host spans.
  This 62.768-second startup snapshot precedes the D3D smoke and render setup.
- C225 later aborts at the 6 GiB RAM floor, before Run or trace arming. Owned VM,
  backend, TPM and TAP are gone; the physical GPU remains NVIDIA-bound. Floppy
  priority and resource limits remain unchanged. No FPS comparison follows.

Curated local evidence: `/mnt/vmstore/caraxes/3dmark-trials-2026-10-09/C225-report.json`
and `C225-runtime/results/partial-backend-timeline-report.json`. There is no
complete render timeline or demonstrated improvement from this build yet.

## Calibrated probe

`vk-unit-timing --calibrate` now records raw DEVICE/CLOCK_MONOTONIC pairs before
and after each sample, calibration call brackets, `max_deviation_ns`, and absolute
CPU endpoints. It prefers KHR with an EXT fallback and requires both exact time
domains. Default-off output stays compatible. CPU checks cover capability
selection and reject CLOCK_MONOTONIC_RAW as a substitute for the probe's clock.

A native RTX 2070 pilot passes complete verification of 65536 words, with no
mismatches. KHR calibration works; reported deviations are 2048–2080 ns. Both
measured GPU intervals are about 210 µs, and their endpoints lie between the raw
calibration pairs. The build passes `-Wall -Wextra -Werror` and CPU self-tests.

This is a capability and ordering check, not a new native performance baseline.
CPU15/SCHED_IDLE/nice19 preserves Floppy priority and can delay CPU observation
of completed work. No matching guest run exists yet. Pair-derived offsets do not
bound drift between calls; retain that uncertainty before assigning a delay.
Evidence: `vulkan-calibration/cpu-validation.json` and
`vulkan-calibration/native-pilot/audit.json` under the private artifact directory.

## Measuring the missing boundaries

1. **GPU execution and guest completion:** keep the original verified Vulkan and
   D3D probes. Compare the same work at comparable physical clocks. Run the new
   optional calibration in the guest to locate GPU endpoints within each sample.
   Vulkan requires supported device/host time domains and records `maxDeviation`;
   decline finer attribution when uncertainty approaches the delay. Guest clock
   calibration still needs a bounded guest-to-host mapping to join backend events.
   [Khronos contract](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetCalibratedTimestampsKHR.html).
2. **GPU scheduling:** pilot a short, owned Nsight context-switch capture before
   rendering attribution. Verify that Caraxes's host RM contexts are visible and
   map them to the exact backend instance. Context residency does not prove ready
   or executing work. Host API tracing does not expose guest userspace calls;
   standard vGPU host capture is documented as unsupported.
   [NVIDIA context switches](https://docs.nvidia.com/nsight-systems/UserGuide/index.html#gpu-context-switch).
3. **Graphic execution limits:** only after locating a slow GPU interval, collect
   raster/depth/color, texture and cache metrics during that same workload. The
   documented `tu10x-gfxt` set recommends at least 10 kHz; the older clean 200 Hz
   general-counter recipe is not adequate evidence for this set. Qualify overflow
   and missing data in a short pilot. Device-wide counters include host work.
   [NVIDIA sampling guidance](https://docs.nvidia.com/nsight-systems/UserGuide/index.html#launching-gpu-metrics-from-the-cli).

D3D11 disjoint queries validate GPU durations but do not calibrate CPU/GPU clocks.
Windows D3D12 exposes a separate
[GetClockCalibration API](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/nf-d3d12-id3d12commandqueue-getclockcalibration).
Adding a D3D12 diagnostic does not retroactively calibrate D3D11 or Fire Strike.

All hardware captures retain Floppy priority, exact owned process identities,
resource floors, finite deadlines, host driver binding and clean teardown. Arm
collection before rendering; launch no foreground or capture UI during scenes.
