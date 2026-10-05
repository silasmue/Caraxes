<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Next Windows performance measurements

C166 reached Fire Strike P graphics 6015/6040 with full regular VM boost. Later unchanged-parent boots reached 5382 (C168) and 5281 (C169); repeatability remains unresolved. High-utilization SM medians stayed around 1830–1845 MHz. Continuous requests retain dynamic clocks; no causal boost-policy comparison or trace-consumer gain is established. See [the qualification](windows-3dmark-2026-10-04.md).

Prioritized research:

1. **Submission and completion tails.** Obtain actual GT boundaries and correlate publication, reactor/device-lock waits, guest acknowledgement and GPU queues. Existing oldest-doorbell to host-event timing is not an exact submission fence. Separate diagnostics from scored runs; preserve data synchronization, shadow-plan barriers and channel ownership.
2. **Exact interrupt transport.** Tree writes are 57% of C162 callbacks, but only 4.702 s of handler time in its 275.8 s bracket. Measure transport and tail costs before optimizing decoding/publication. Repeated enable stores and acknowledgements are transactions. Do not replace them with their final values or map the physical host interrupt controller.
3. **Unused trace formatting.** C167 gates consumer formatting with `records_events()` while retaining notes, reset, rings, MSI, counters and enabled tracing. Source/functional checks passed; no FPS gain is established. Producer success-note allocation remains a separate later change.
4. **USERD discovery and CPU placement.** The poller sleeps 1 ms and scans at most 32 channels per tick. Measure polled-only submissions and discovery tails before changing cadence. Auto pinning does not reserve SMT siblings or account for later vCPU migrations. Reject higher CPU cost without a measured benefit.
5. **Exact FE qualification.** The read-only mode already exists. Measure actual callback reduction with VM policy fixed, then compare matched FPS. Leave it optional if results remain unchanged or adverse.
6. **GPU resource/state costs.** Measure aperture, kind, residency, compression and mapping churn before changing memory policy. Current evidence does not establish ZCULL as the Fire Strike cause. Retain private backing, translation/lifetime checks and image/statistics gates.

Require matched resources, clock policy, immutable binaries and fresh complete archives. Keep diagnostics separate, interleave repetitions and retain health/cleanup checks. Callback percentages and CPU savings do not predict FPS. Steel Nomad/D3D12 Xid 31 remains unfixed.

Detailed source references and acceptance tests are retained privately in `/mnt/vmstore/caraxes/windows-3dmark-2026-10-04/windows-performance-research-2026-10-05.md`. No external code or benchmarks were copied.

Current hypotheses and discriminating measurements:

| Hypothesis | Evidence and next measurement |
| --- | --- |
| Submission/completion scheduling | C169's reader and host-events share CPU 10 through inherited affinity. Their global-window runnable delays are 12.128/5.266 s. Compare `CARAXES_DOORBELL_POLL=hybrid` against `spin` with the same placement. Reserve the reader's physical core from vCPUs and give completions independent allowed CPUs. Avoid pinning the entire backend to one CPU. Measure FPS and both paths' tails; spinning could delay completion on a shared CPU. |
| Additional synchronization or GPU work | CPU handler time omits transport, waiting and GPU work. Compare the same native/guest workload's GPU timestamps, submitted work and copy/fence counts. High utilization can coexist with memory stalls, inefficient occupancy or unnecessary work. It does not prove identical useful work per frame. |
| Compression or depth-state inefficiency | `CARAXES_COMPRESSION=slice` is enabled; effective per-resource compression and ZCULL/early-depth behavior are unproved. Compare kind/comptag state, memory traffic and fragment/occlusion statistics on identical workloads. Existing evidence does not establish ZCULL as the Fire Strike cause. |
| BAR1 pool or residency pressure | Current pool is 32 MiB plus two 16 MiB FB-store windows. C169 whole-session pool copies total 26,984 KiB and 137.769 ms; pool GPU checks/barriers total 36/11 ms. This weakens that measured pool path as the main explanation, but excludes neither other aperture paths nor guest paging. Measure phase-aligned churn/traffic before a larger-pool comparison. |
| Shared physical GPU/host activity | Physical utilization includes the host desktop. Obtain guest-workload attribution and matched host conditions before calling a render phase GPU-bound. Keep the current 4 GiB guest VRAM limit separate from BAR aperture size. |

`nvidia-smi` GPU utilization measures activity over a sampling interval, rather than useful frame throughput or occupancy. [NVIDIA's metric definition](https://docs.nvidia.com/deploy/nvidia-smi/index.html) and [GPU Trace guidance](https://docs.nvidia.com/nsight-graphics/UserGuide/gpu-trace-overview.html) distinguish utilization, workload duration, occupancy and unit throughput. Profiling support through the emulator remains unqualified; do not assume every performance-control path works.

Work resumed after C169. C170 exposed a GPA/IOVA publication bug; `514be02c` fixes it. C173 completed two hybrid runs with explicit reader/completion placement. C174 spin and C175 hybrid control produced incomplete workload results; no polling gain or regression is accepted. See [the overnight qualification](night-performance-2026-10-06.md).

Next: diagnose Windows per-workload completion and use the passing stock Linux Vulkan probes to test CPU placement and polling. Their GPU intervals agree closely while CPU submit/fence time remains higher in the guest. Change one declared factor per comparison, retain immutable binaries/resources/clock policy, and interleave controls. FMSIScan presence is not a rendering boundary. Larger BAR and compression experiments remain unrun.
