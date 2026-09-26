<!-- SPDX-License-Identifier: MIT -->
# vm4-615: several GPU clients at once, with and without display and stream (2026-09-26)

Question: do several GPU clients work in parallel when no display is involved, and where does it
break once the desktop, Sunshine and the stream come in?

## Verdict

- **Headless, every combination works (12 of 12 cells, all results checked).**
  - 1 and 3 concurrent CUDA processes, CUDA + Vulkan compute, 2 Vulkan compute processes, and
    CUDA + Vulkan compute + GL (EGL) offscreen.
  - The CUDA-only ladder (C1-C5) passes: vkCreateDevice before, during and after cuInit only, a
    held cuCtx, a held cuCtx with cuMemAlloc, an idle NVENC session, and an encoding NVENC
    session.
  - **There is no first failing combination without a display.** The cause (below) is timing- and
    chid-dependent, though, so a pass here is not proof that the headless path is safe.
- **Desktop without NVENC: every CUDA/Vulkan/EGL mix passes.**
  - D1-D5e: the same seven cells as headless, all PASS.
  - F1-F4 pass beside a presenting vkcube or a high-priority EGL context.
- **The failures come once an NVENC/CUDA session (Sunshine's or the probe's) lives beside a new
  client on the desktop. None needs the stream itself.**
  - Sunshine (NVENC encoder, KMS capture) running idle, without a client:
    - a new CUDA process hangs in context creation (I1, G2, G7; 60 s, `launches=0 phase=1`);
    - a new Vulkan device fails (I2, G1, G3: `vkCreateDevice` -3, c5c0 0x57).
  - While streaming the result varies: session 1 S1-S3 (CUDA, Vulkan, EGL) all PASS; session 2
    G4 Vulkan FAIL, G5 CUDA HANG. G6 (Vulkan, Sunshine idle after the stream) passed.
  - Sunshine stopped, desktop, an NVENC encoding holder:
    - F5 (NVENC alone): Vulkan probe FAIL;
    - F6, F7 (vkcube + NVENC): Vulkan FAIL, CUDA HANG.
  - The cxdbg repro (vkcube, then the CUDA NVENC probe, then a new vkcube, phase X) reproduced
    0x57.
  - Not deterministic: the headless C5 (the same NVENC encode holder) passed, F5 failed.
  - GL (EGL offscreen) never failed: I3, S3 and every H/D cell with EGL pass.
- **Every failure has the same two signatures. They match the doorbell token collision of
  `vm4-615-cuda-vulkan/NOTES.md`.**
  - New Vulkan client: after c597 its last ioctl is `0xd01` (CLIENT_GET_ADDR_SPACE_TYPE). Then
    comes an **11.0-11.1 s gap with no ioctl** (user mode, waiting for its first submission),
    then c5c0 → 0x57 and teardown (VK_ERROR_INITIALIZATION_FAILED, -4 once). 7 of 7 Vulkan
    failures.
  - New CUDA client: its channel objects are allocated (c5b5, 0079, 003e, 50a0). Then it polls
    `0x20801702` once per second, 57-58 times, until the 60 s alarm. 5 of 5 CUDA hangs.
    - This is the reverse direction of the same stall (cuda-vulkan NOTES: "the reverse
      direction stalls the CUDA probe for 60 s").
  - **Device side in every failure window: 0 RPC answers later than 100 ms, 0 RPCs
    unanswered.** The device answered everything. The client waits for work that never runs.
  - **Slow ioctls** (> 100 ms) during the stall:
    - Vulkan: `RM_ALLOC 902d` (105-150 ms) at setup, and `RM_FREE` (nr 0x29, 100-290 ms) at the
      teardown;
    - CUDA: `MAP_MEMORY_DMA` (nr 0x57, 370-640 ms) once at context creation.
    - None lies inside the 11 s gap or the 60 s poll.
  - **Chid values are not in these logs.** Each failure trace window runs from 10 s before to
    10 s after the reported failure, which here is the teardown or the alarm. It starts after the
    lost first ring (about 0.4 s after the channel's creation), so neither the colliding raw
    token nor its owner is captured. The full traces were overwritten by the next VM on
    vm4-615.
    - `collide.py` (in tools) finds no collision line in any window.
    - The only chid-resolved capture of this run is phase X, analysed in the cuda-vulkan
      NOTES: chid 54, guest token 0x36 = host token of guest chid 12.
    - I1/I2 (and all later failures) are attributed to the collision by that signature, not by
      a captured token.
- **Why Sunshine makes it likely:** Sunshine keeps a CUDA/NVENC context and an EGL context
  alive (99 MiB in `nvidia-smi`, "EGL: context priority set to HIGH"). That means more mirrored
  channels, and host chids that climb into the guest's range. This fits the collision
  mechanism. The high EGL priority alone does not cause it (F3, F4 PASS).
- **Host:** Xid 0 in all three sessions. Host watch SIGTERM 0.
  - `VA Space alloc failed` bursts (13 or 31 lines) came at client teardowns: 03:50 and 03:52
    (phase X), and 04:24-04:30 (session 3 holder kills). This is the consequence noted in the
    cuda-vulkan NOTES.
  - MemAvailable dropped to 3.8 GB during session 2 (other agents' builds and the Windows VM)
    but stayed above the 3 GB stop line.

## Matrix (session 1, one boot, 03:23-03:59)

Binary `/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-invhold-3bad9810` (main + gate). The flippace
build was not used: its A/B has the work regression while streaming (vm4-615-flippace/NOTES.md).
Stage `up-615.sh` with `SOTTR_NAT=0 SOTTR_SUN_ENCODER=nvenc CARAXES_MEM=8G`. Guest programs were
built in the guest with the host's Vulkan/EGL/GLES headers copied in (no apt). The SPIR-V was
compiled on the host with glslangValidator. Each process ran under the timed `rmioctl-log.so`.

Kicks/s = `W bar0 0x00696000` per second in the cell. ERR = device-log ERROR lines in the cell
(all 0). Xid = host Xid lines since up (all 0).

| Cell | State | Workload | Result | Throughput | Kicks/s |
|---|---|---|---|---|---|
| C1 | headless | Vulkan probe before, during, after `cuInit` only | 3/3 PASS | vk 157-171 disp/s | 0 |
| C2 | headless | same around `cuCtxCreate` held | 3/3 PASS | 165-185 | 0 |
| C3 | headless | same around ctx + cuMemAlloc 1 MiB | 3/3 PASS | 173-240 | 0 |
| C4 | headless | same around an NVENC session opened, idle | 3/3 PASS | 120-146 | 0 |
| C5 | headless | same around NVENC encoding 1080p60 (holder alive, NVENC class c4b7 allocated) | 3/3 PASS | 208-225 | 0 |
| H1 | headless | 1 CUDA | PASS | 1209 launches/s, 1783 GFLOP/s | 0 |
| H2 | headless | 3 CUDA concurrently | 3/3 PASS | 548-558 each (1656 total) | 0 |
| H3 | headless | CUDA + Vulkan compute (from +15 s) | 2/2 PASS | cu 647/s, vk 243 disp/s | 0 |
| H4a | headless | Vulkan compute alone | PASS | 288 disp/s | 0 |
| H4b | headless | 2 Vulkan compute | 2/2 PASS | 245 + 244 | 0 |
| H5 | headless | CUDA + Vulkan + EGL offscreen (device platform, pbuffer) | 3/3 PASS | cu 434/s, vk 204, GL 202 fps | 0 |
| H5e | headless | EGL offscreen alone | PASS | 577 fps | 0 |
| D1 | desktop, no Sunshine | 1 CUDA | PASS | 849/s | 7.1 |
| D2 | desktop | 3 CUDA | 3/3 PASS | 374-379 each | 11.2 |
| D3 | desktop | CUDA + Vulkan | 2/2 PASS | cu 424, vk 183 | 27.4 |
| D4a | desktop | Vulkan alone | PASS | 140 | 12.9 |
| D4b | desktop | 2 Vulkan | 2/2 PASS | 228 + 225 | 3.1 |
| D5 | desktop | CUDA + Vulkan + EGL | 3/3 PASS | cu 441, vk 174, GL 227 fps | 0.0 |
| D5e | desktop | EGL alone | PASS | 537 fps | 0.0 |
| X | desktop | cxdbg repro2: vkcube, NVENC probe, new vkcube | **FAIL**: new vkcube c5c0 0x57 after 15.1 s | - | - |
| I1 | Sunshine idle (NVENC), no client | CUDA | **HANG** in context creation (60 s) | 0 | 0 |
| I2 | Sunshine idle | Vulkan | **FAIL** vkCreateDevice -3 at 12.9 s (c5c0 0x57) | - | 0 |
| I3 | Sunshine idle | EGL | PASS | 525 fps | 0 |
| S1 | streaming (Moonlight 1080p60) | CUDA | PASS | 316/s | 0 |
| S2 | streaming | Vulkan | PASS | 178 | 0 |
| S3 | streaming | EGL | PASS | 259 fps | 0 |

Kicks near 0 from D5 on and while streaming: the desktop was static (no flips owed). Sunshine's
KMS capture does not need flips.

## Follow-up sessions (same binary)

Session 2 (04:01-04:19): its F cells are invalid (`invalid-s2-F*`: `hold.sh` was not executable,
so no holder ran; the probes passed alone). The G cells used a fresh Sunshine, and each ran
60 s after the previous step.

| Cell | State | Workload | Result |
|---|---|---|---|
| G1 | Sunshine idle | Vulkan | FAIL -3 at 14.0 s (0x57) |
| G2 | Sunshine idle | CUDA | HANG (60 s) |
| G3 | Sunshine idle, 2 min later | Vulkan | FAIL -3 at 13.7 s |
| G4 | streaming | Vulkan | FAIL -3 at 14.5 s |
| G5 | streaming | CUDA | HANG |
| G6 | stream ended, Sunshine idle | Vulkan | PASS 181 disp/s |
| G7 | same | CUDA | HANG |

Session 3 (04:20-04:31): desktop with Sunshine stopped. Each ladder runs a 5 s probe before, 12 s
into, and after a holder (`hold.sh`).

| Cell | Holder | Probe | before / during / after |
|---|---|---|---|
| F1 | vkcube (presenting) | Vulkan | PASS / PASS / PASS |
| F2 | vkcube | CUDA | PASS / PASS / PASS |
| F3 | EGL context priority HIGH, rendering | Vulkan | PASS / PASS / PASS |
| F4 | EGL HIGH | CUDA | PASS / PASS / PASS |
| F5 | NVENC encoding 1080p60 | Vulkan | PASS / **FAIL -3 at 15.4 s** / PASS |
| F6 | vkcube, then NVENC encoding | Vulkan | PASS / **FAIL -4 at 13.6 s** / PASS |
| F7 | vkcube, then NVENC encoding | CUDA | PASS / **HANG 60 s** / PASS |

## Stall data per failure

| Cell | Client | Longest ioctl gap | Then | Slow ioctls (> 100 ms) | Late or missing RPC answers |
|---|---|---|---|---|---|
| I2, G1, G3, G4, F5, F6, X | Vulkan | 11.0-11.1 s after `RMCTRL 0xd01` | c5c0 0x57, FREE of the channel | 902d alloc 105-150 ms; RM_FREE 100-290 ms | 0 / 0 |
| I1, G2, G5, G7, F7 | CUDA | none > 1.3 s: `0x20801702` polled 1/s, 57-58 times | alarm at 60 s | MAP_MEMORY_DMA 370-640 ms (context creation) | 0 / 0 |

`*/p*-stall.txt` has the per-cell detail (late RPCs, doorbells per second, slow ioctls). The traces
are `*/p*-trace-20s.txt.xz`. The ioctl logs with wall times and durations are `*/p*.rm`.

## Files

- `tools/`: guest programs, host scripts and analysers.
  - `cuwork.c`: CUDA through the driver API, loaded at run time, with a PTX FMA kernel.
  - `cuhold.c`, `nvenc-hold.c`: `nvenc-probe.c` + `NVENC_HOLD_S` for an idle session.
  - `vkcomp.c` + `comp.comp`/`comp_spv.h`: headless Vulkan compute.
  - `eglwork.c`: EGL device platform, pbuffer, GLES2, `EGL_PRIO=high|low`.
  - `rmioctl-log.c`: the repo logger + a wall time and duration per escape, and a SLOW line for
    any ioctl over 100 ms.
  - `grun.sh`, `ladder.sh`, `ladder2.sh`, `hold.sh` (guest).
  - `setup.sh`, `matrix.sh`, `cell.sh`, `all.sh`, `followup.sh`, `session3.sh`, `guard.sh`,
    `conc-wait2.sh` (host).
  - `stall.py`, `collide.py` (analysis).
- `table.txt`, `run.log`: every cell and step. The phase X console is `X-repro2.txt`. Its
  guest output and full trace are in the scratchpad (`conc/X-out/`, analysed in
  vm4-615-cuda-vulkan).
- `gpu.log.xz`, `gpu2.log.xz`, `gpu3.log.xz`: device logs. `host-kernel-run*.txt`: host journals.
- `session1.txt`, `session2.txt`, `session3.txt` name the stage session folders.
