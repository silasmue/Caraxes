<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Four-vCPU guest idle hardware results

Two short host-backed Linux runs did not reproduce the reported high guest
idle CPU. GNOME Wayland also stayed below 1% of the four-vCPU guest's capacity.
Persistent NVIDIA interrupts appeared after GPU use, including after all
guest rendering channels were freed. This is a separate optimization target;
these runs do not establish its cost in Windows or explain low game FPS.

## Results

Each window lasted about 30 seconds after at least 20 seconds of settling.
CPU is guest `/proc/stat` user + nice + system + IRQ + softirq time, divided
by four. Steal time is reported separately and is not driver CPU work.

| Run / state | Guest CPU | Kernel CPU | Steal | NVIDIA MSI/s |
| --- | ---: | ---: | ---: | ---: |
| A: initialized, headless | 0.067% | 0.033% | 0.033% | 0 |
| A: headless after GPU job | 0.067% | 0.050% | 0.341% | 386 |
| B: initialized, headless | 0.058% | 0.025% | 0.008% | 0 |
| B: GNOME Wayland, no Sunshine | 0.333% | 0.175% | 0.316% | 471 |
| B: headless after stopping GNOME | 0.075% | 0.050% | 0.217% | 182 |

- Every individual vCPU averaged below 0.7% busy in these windows.
- In Wayland, `irq/38-nvidia` and `nvidia-modeset/kthread_q` each used about
  0.16 seconds over 30 seconds: 0.53% of one CPU, or 0.13% of this guest.
  `gnome-shell` used 0.14 seconds: 0.47% of one CPU, or 0.12% of the guest.
- In run A after GPU use, `nvidia-modeset/kthread_q` accrued 0.40 seconds:
  1.33% of one CPU, or 0.33% of the guest. This exceeds the aggregate
  `/proc/stat` busy delta. Keep both raw accounting sources; they do not
  reconcile exactly. Neither shows a saturated core. Do not infer literal
  zero CPU work from a rounded utilization display.
- The one-minute load averages decayed from boot and display startup.
  Load average is not CPU utilization or a count of busy cores.

Both runs executed the existing Vulkan ALU probe once, with one warmup and
one measured sample, outside the idle windows. Each final readback verified
all 65,536 words with zero mismatches. These were correctness checks;
there was no native comparison or scored performance benchmark.

## What the interrupts show

Before the first GPU job, both guests recorded zero NVIDIA MSI interrupts.
The job armed host nonstall event subscriptions for GR0, CE2–4, NVDEC0 and
NVENC0, plus RC. Run A's final backend counters show raises and guest ACKs
on vectors 4, 5 and 6: GR0, CE2 and CE3. Its final guest shadow-channel,
TSG and VA-space counts were zero, while the host event objects remained.

The reviewed event code is unchanged between tested commit `c0e9eef5` and
diagnostic source commit `b66eaa2`:

- `mirror.rs::ensure_events` registers repeating host subdevice events at
  the first channel build. These notifications cover other host GPU clients.
- `mirror.rs::free_channel` keeps those subscriptions. Mirror reset tears
  them down. `event_loop` forwards ready engine events without checking
  which guest owns completed work; it can also forward an empty drain.
- `intr.rs::raise_in_generation` coalesces an already-pending vector.
  Guest ACKs allow the next global notification to produce another MSI.
  The log's **from idle** counter means the vector was not pending; it does
  not describe whether guest GPU work was idle.
- The vector summary covers 0–63. Display vector 156 is outside that range,
  so its absence from this summary cannot exclude display interrupts.
  The backend retained display emulation in all cases; headless describes
  the guest session. Detailed display-head counters were not collected.

These observations support persistent global engine hints as a source of
extra guest wakeups. They do not attribute every MSI or prove the cause of
the previously reported high CPU episodes. The host desktop stayed active,
and no controlled host-workload correlation was performed.

## Configuration and evidence

- RTX 2070 host GPU, driver and guest open kernel modules `615.71.09`.
  Host driver stayed bound to `nvidia` throughout.
- Cloud Hypervisor, four vCPUs, 8 GiB guest RAM, 4 GiB reserved framebuffer.
- Stock-driver guest without `caraxes_dma_range`; GPA DMA registration on
  the host. Guest boot target `multi-user.target`; run B explicitly started
  the display manager and verified an active user session of type `wayland`.
- Hybrid doorbell polling, read-only display page, direct interrupt page,
  MSI rearm ioeventfd, 32 MiB BAR1 pool, BAR2 memfd, slice compression.
- Host boost policy `vm`, default interrupt gate off, tracing/profiling off.
  No `nvidia-smi` sampling, UI automation or screenshots during idle windows.
- Sunshine was absent in the headless windows and runtime-masked before
  GNOME startup in run B. NVENC/NVDEC streaming was not tested.
- Backend C209, source `c0e9eef5a94ca890bccd29302ac3ed1d19830be5`, SHA-256
  `5db44092d983c46723d380024fca35e33640660dbeb410d20f9cdbc718a41818`.
- Paired Cloud Hypervisor SHA-256
  `7f3682ea1422c9c7335b07483960ec819c49da6405669a12cd73f8d5e582198b`.
  The new ioctl timeline/errno diagnostics in `b66eaa2` were not deployed.

Raw census endpoints, per-second CPU rows, IRQ deltas, guest state, probe
validation, pins and cleanup records are retained under:

```text
/mnt/vmstore/caraxes/windows-3dmark-2026-10-04/linux-idle-oct09a/
/mnt/vmstore/caraxes/windows-3dmark-2026-10-04/linux-idle-oct09b/
/mnt/vmstore/caraxes/host-timeline-2026-10-09/hardware-idle-results.json
```

The owner sessions lasted 125 and 173 seconds, below their 720-second limit.
Both used fresh thin overlays and cleaned up their exact VM/backend processes.
Image ancestor metadata remained unchanged. The owned tap was removed.
No new host Xid was recorded. Host available RAM stayed above 11 GiB, free
artifact storage above 145 GiB, and the higher-priority Floppy work continued.

## Next checks

1. Repeat the before/after GPU-use comparison in Windows, collecting DPC,
   interrupt and privileged CPU time alongside the active display state.
   These Linux results do not establish Windows idle behavior.
2. Record event fd readiness, drained record counts/errors, per-vector
   transitions and display ticker counters in a separate diagnostic window.
   Correlate host event rates with guest IRQs while guest submissions stop.
3. Reduce notification scope or subscription lifetime only when all relevant
   owned channels, builds and outstanding work are demonstrably quiescent.
   Audit graphics, compute, video, internal CE and invalidation paths; rearm
   before new work and preserve reset generations and page-before-MSI ordering.
   Submission age or GPGet advancement alone cannot justify dropping a hint.
4. Capture driver stacks if sustained high guest CPU recurs. The shared-data
   poll scheduler and reply semantics remain compatibility leads, not proven
   causes of a busy loop in these runs.

No interrupt-delivery policy was changed for this measurement. Short quiet
windows do not replace a long idle soak, active desktop responsiveness test,
codec test or game performance run. Diagnostic details are documented in
[Guest idle CPU and host control timing](idle-and-host-timeline-2026-10-09.md).
