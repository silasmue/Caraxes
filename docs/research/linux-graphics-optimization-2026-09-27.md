<!-- SPDX-License-Identifier: MIT -->
# Linux graphics stability and hybrid-poll comparison, 2026-09-27

Desktop B was tested alone on reservation slot 1 while the Windows guest on
slot 0 was halted in the debugger. The guest had 4 GiB RAM, four vCPUs, 2 GiB
VRAM, stock NVIDIA 615.71.09, and GNOME Wayland at 1920x1080. The backend was
the established build with SHA-256
`dbf1c6e271554f15a2f6fdd623a62a0f7f430b907abba58e82168b4e34ffa348`.
Desktop A and the host driver were not touched.

## Functional result

Vulkan and OpenGL both selected `Leandro VFIO RTX 2070-2G`. `vulkaninfo`
reported the NVIDIA 615.71.09 Vulkan driver and `glxinfo` reported OpenGL 4.6.
Five consecutive native-Wayland `vkcube-wayland --c 300` processes completed
successfully in each of four configurations (20/20 total). `vkgears` ran on
Vulkan/Wayland, `glmark2-wayland` ran natively on Wayland, and `glxgears` ran
through XWayland. There were no guest Xids or backend DMA refusals.

The first default run produced 54.5 and 58.3 FPS samples from `vkgears`, 59.1
and 59.4 FPS from XWayland `glxgears`, and 1031 FPS / 0.970 ms from the
`glmark2-wayland` build scene. These short composited tests establish working
presentation; their frame rates are not a native-speed claim.

## Fixed-affinity comparison

The compared setting is the hybrid reader's post-event spin window. An idle
value of 0 blocks immediately after draining work; 200 busy-polls for up to
200 microseconds before blocking. Both fixed runs pinned the reader to host
CPU 3. Windows remained halted throughout the paired measurements.

| Metric | Hybrid idle 200 us | Hybrid idle 0 us |
|---|---:|---:|
| Backend CPU, 15 s desktop idle | 12.33% | 3.80% |
| VMM CPU, 15 s desktop idle | 5.07% | 5.47% |
| Backend CPU, matched graphics sequence | 78.36% | 54.66% |
| VMM CPU, matched graphics sequence | 69.51% | 76.20% |
| Five vkcube process times | 6.43 to 7.40 s | 6.06 to 7.23 s |
| Doorbell reader CPU over whole run | 28.910 s / 20.2% | 6.959 s / 4.8% |
| Ring p50 / p99 | 12.0 / 110.4 us | 12.0 / 76.2 us |
| Device-lock wait p50 / p99 | 0.5 / 250.0 us | 0.6 / 309.4 us |

The ring histogram begins after the eventfd wait returns. It excludes the
guest-to-eventfd path and scheduler wake time, so it cannot establish input,
frame, CUDA, or end-to-end latency. CPU percentages use one logical core as
100%. The exit histograms and reader CPU totals cover the whole VM run,
including guest and desktop startup, rather than only the matched workload.

`glmark2-wayland` was too nonstationary to use as a throughput comparison.
Three identical ten-second build scenes reported 1949/745/747 FPS at 200 us
and 1709/2192/268 FPS at 0 us. This variance prevents attribution; potential
confounders include process startup, shader-cache state, compositor state, and
changing backend state.

The 0-us exit summary also recorded 28 restored stale `AWAKEN_WIN` echoes and
9 ignored `CORE_RESET` stores without an outstanding marker. The matched
200-us run recorded no display-page inconsistency. Both runs completed all
applications and the interrupt page remained consistent, but this single-run
display difference needs reproduction before preferring 0 us.

## Decision

Keep the 200-us default. The 0-us setting materially reduced backend CPU here
without breaking the test applications, but it is only a diagnostic CPU-saving
candidate. Keep it experimental until the display inconsistencies and
end-to-end wake/frame latency are checked; do not recommend it for deployment
from this result.

Raw evidence is in
`/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-checks/D3-linux-graphics-optimization/`.
The `fixed-cpu3-*-idle.txt` and `fixed-cpu3-*-workload.txt` files contain the
matched CPU windows; `gpu-*-cpu3.log` contains the exit summaries.

## Trace follow-up

The retained final-run trace places the first recovered display stores during
the timed workload rather than GNOME startup. It first records a markerless
`CORE_RESET` at 65.342 seconds after boot and a stale `AWAKEN_WIN` echo at
66.775 seconds. The first vkcube channels were created at about 64 to 65 seconds
and freed at 70.28 to 70.40 seconds after five to six seconds of life. The trace
only logs the first occurrence of each class, so it cannot place every one of
the 9 and 28 occurrences counted at exit.

These are fail-closed state recoveries, not expected coalescing. The stale
window word is restored and the marker and acknowledgement counts still match
(3384 each), so no marker acknowledgement was lost. However, the second guest
pass has already delivered a duplicate flip event before the reactor observes
and restores the stale echo. That can complete a later DRM flip early. A
`CORE_RESET` without an outstanding marker is ignored. The one 0-us run does
not establish that the polling setting caused either race.

The three fixed-0-us glmark processes can be identified by their approximately
ten-second channel lifetimes. Their submitted doorbells track the reported
frame rates closely:

| glmark build run | FPS | Channel life | Doorbells | Doorbells/s | Doorbells/frame |
|---|---:|---:|---:|---:|---:|
| 1 | 1709 | 10.127 s | 33781 | 3336 | 1.95 |
| 2 | 2192 | 10.114 s | 43610 | 4312 | 1.97 |
| 3 | 268 | 10.237 s | 5267 | 514 | 1.97 |

The third-run collapse is therefore already visible in the guest submission
rate. The backend reported zero mirror refusals, pool exhaustion, stale tokens,
VRAM stalls, failed accesses, BAR1 hazards, and conflicting replica words.
Duplicate BAR1 mappings used the fail-closed replica path in both fixed runs
(14 at 200 us and 16 at 0 us), so they do not uniquely explain the third run.

The next diagnostic target is a per-channel doorbell inter-arrival and idle-gap
histogram correlated with display flip-marker sequence IDs. Run it around the
same repeated glmark scene. Long submission gaps that follow a flip marker or
stale echo would point to presentation pacing; gaps without display correlation
would instead focus work on guest scheduling or command submission. This is a
measurement target, not evidence for changing the polling default.

## Read-only display-page follow-up

Cloud Hypervisor commit `e7b1965c0` mapped BAR0
`0x611000..0x612000` through a read-only KVM memory slot. Reads therefore
continued through the shared mmap, while guest writes exited to vfio-user and
were applied synchronously by backend commit `419a308f`. The tested backend
binary had SHA-256
`4e1a0c3d57855ef3605c0f14175e49b9782eee2318a154fdd9a1dc937331aa8b`.

The functional run completed five native-Wayland vkcube launches, vkgears, and
two glmark build scenes. The 0-us trace contained 13,898 BAR0 writes in the FE
page and no FE reads. This is the expected protocol split: trapped writes are
visible in the vfio-user trace, while direct mmap reads are not. GNOME Wayland
ran at 1920x1080 and no guest Xid, DMA refusal, or failed VRAM access occurred.

The final comparison pinned the hybrid reader to host CPU 3 and used the same
five-vkcube plus three-glmark sequence for each setting. Windows was stopped
and the host remained otherwise quiet during the pair.

| Metric | Hybrid idle 200 us | Hybrid idle 0 us |
|---|---:|---:|
| Backend CPU, 15 s desktop idle | 11.33% | 3.13% |
| VMM CPU, 15 s desktop idle | 3.40% | 4.07% |
| Combined backend and VMM CPU, idle | 14.73% | 7.20% |
| Backend CPU, matched graphics sequence | 80.57% | 48.80% |
| VMM CPU, matched graphics sequence | 104.48% | 114.65% |
| Combined backend and VMM CPU, graphics | 185.05% | 163.45% |
| Five vkcube process times | 5.86 to 6.53 s | 5.91 to 7.04 s |
| Three glmark build results | 2534 / 2449 / 2078 FPS | 2978 / 2641 / 2693 FPS |
| Ring p50 / p99 | 10.7 / 79.4 us | 11.5 / 60.1 us |
| Device-lock wait p50 / p99 | 0.4 / 96.5 us | 0.5 / 86.4 us |

The 200-us run applied 6,945 display status acknowledgements and 13 interrupt
enable stores, with 3,501 awaken markers and 3,501 matching core resets. The
0-us run applied 6,858 acknowledgements and 13 enable stores, with 3,516 awaken
markers and 3,516 matching resets. Neither run reported a display-page or
interrupt-page inconsistency. The longest marker acknowledgement was 34.0 ms
at 200 us and 29.6 ms at 0 us; the 0-us run used two watchdog raises.

The read-only mapping and synchronous write path eliminate the stale shared
store recovery seen in the earlier run. With that correctness issue addressed,
the 0-us spin window remains a useful per-VM CPU-saving candidate: it reduced
combined backend and VMM CPU by about 51.1% at idle and 11.7% during the
graphics sequence. It also moved more work to the VMM during that sequence.
Keep the 200-us default
until an end-to-end frame or input latency measurement establishes that the
immediate-block setting does not trade latency for CPU. The ring histogram
still starts after epoll wakes and cannot answer that question.

These are native Wayland application runs under a 60 Hz composited desktop;
their frame rates and short process times are limited as performance evidence.
This round did not exercise NVENC and makes no NVENC regression claim. Treat
0 us with the read-only page as a measured CPU-saving option pending a longer
soak and end-to-end input and frame latency tests.

Final artifacts are in
`/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-checks/D4-readonly-display-page/`.
That directory includes the exact tested `caraxes-vm-readonly.sh` and
`desktop-b-readonly.sh` launcher copies, `SHA256SUMS`, the backend file
capability record, and `REPRODUCE.md`. The fixed-CPU3 default case can be
started in a detached session with:

```sh
ART=/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-checks/D4-readonly-display-page
CARAXES_DOORBELL_POLL_CPU=3 CARAXES_DOORBELL_POLL_IDLE_US=200 \
  setsid -f "$ART/desktop-b-readonly.sh" up \
  > "$ART/reproduce-launch-200us.log" 2>&1
```

Change the idle value to `0` for the immediate-block case. The preserved
wrapper selects Linux B and reservation slot 1; it must not be used for the
frozen desktop A state.
