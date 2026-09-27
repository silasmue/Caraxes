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
