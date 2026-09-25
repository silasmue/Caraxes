<!-- SPDX-License-Identifier: MIT -->
# vm4-615 night run of the BAR1-pool lock fix (304012cc), 2026-09-25

Stage `caraxes/runs/sottr-2026-09-24` (`up-615.sh`, `stream-615.sh`, `down-615.sh`), 8 GiB guest,
Sunshine NVENC unless noted. Binary `/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-poollock-304012cc`
(run 8: `...-exclrel-604a33f4`). One VM at a time; host guard (VRAM free < 600 MiB,
MemAvailable < 3 GB, any Xid) never tripped. A Windows VM (pure emulation) and builds ran beside
it for part of the night, so steal and host load are not clean baselines.

## Verdict

- **Lock hold: mostly gone.** Slow polls (> 2 ms) fell to 0.04-0.19 % of trapped BAR0 writes
  (68838c54: 3 %, 0c54a465: 0.6 %). Idle streaming at 29 min: 0.1 per minute. They remain
  while GPU clients start and stop (vkcube, allocations): 15-60 per minute in those windows,
  p50 2.8 ms, max 65 ms. The GSP queue polls are nearly gone (0x110c00: 21-44 per run).
  - What is left, by register: display kick 0x696000 (part `sync`: the replica merge,
    7 replicas with 7 homes read, about 2.5 ms) and the MMU invalidate 0xb830a0
    (part `moves`: a single page move 2.1 ms, a 128-page batch 3.7-4.1 ms).
  - The §12 expectation "homes read near 0" does not hold: `homes_read` equals the replicas
    merged in most slow `sync` polls.
- **The dma-buf pin changes little.** `gpu.log` shows "the device maps the pool through its own
  dma-buf mapping" (run 1) and "reaches the pool through the FB store's windows" with
  `CARAXES_BAR1_POOL_PIN=0` (run 2). Host BAR1 after boot: 152 MiB both (0c54a465: 157).
  PIN=0 had fewer slow polls (20 vs 25 per minute) and better benchmarks; single samples.
- **The hang at the Moonlight connect persists, and it is not the pool.**
  - The stream itself now works: video arrives 2-4 s after the connect (6/6 connects) and
    Sunshine keeps streaming (latency lines every 20 s, no "Hang detected").
  - But while the stream runs, a new process's GPU work does not complete:
    glmark2 off-screen renders 0 frames in 8 s (793 fps with Sunshine idle, run 5),
    on-screen glmark2 0 frames in 5/5 cycles, 30-50 fps 30 s after each disconnect (run 4).
  - The same with no pool at all (run 3). So neither the pool nor its lock is the cause.
- **"Failed to create Vulkan device" (the SotTR signal 6) = a live CUDA context.**
  - The game's crash dialog: `Game crashed with signal 6: Failed to create Vulkan device: -4`
    (VK_ERROR_DEVICE_LOST). Same text in the 0c54a465 crash dumps (16:37, 16:47).
  - vkcube aborts in `demo_create_device` whenever a CUDA/NVENC context is alive: Sunshine
    with NVENC (idle, streaming, runs 1-4), the CUDA NVENC probe alone while it runs
    (run 6, b2). It passes on a fresh boot, after the probe exits, after Sunshine stops,
    and while Sunshine runs with the software encoder (run 7: no CUDA context).
  - The failing call is the guest kernel's own (no GSP_RM_ALLOC reaches the device):
    `RMALLOC class=0xc5c0 parent=<GR channel> status=0x57` (NV_ERR_OBJECT_NOT_FOUND),
    after 902d, c5b5, a140, c597 on that channel succeeded; the sibling channel of the same
    TSG gets its c5c0 (`run2-pin0/vkcube-rmlog-full.txt`, `*/vkcube-rmlog.txt`).
  - Sunshine's Vulkan probe is not needed for it, NVENC is not needed beyond the CUDA context
    it creates (the probe is a CUDA context plus NVENC; the Vulkan test ran while it encoded).
  - Consequence: the game only runs without Sunshine. It then ran in its menu for 3+ minutes
    (run 6, g2) at 29-34 kicks/s; the run was cut before 10 minutes (priority change).
- **The mouse-relevant kick rate is not up while streaming.** Cursor test (uinput, about
  97 moves/s for 20 s): 28-40 kicks/s without a client, 7-17 kicks/s while streaming, the
  same with no pool (14). Idle streaming holds the display at 10-13 kicks/s. The stream caps
  the desktop, not the pool.
- **Pool against no pool, desktop:** vkcube 8.2-8.7 fps pooled against 17.7 without the pool;
  idle kicks 13 against 21. Streaming latency is better with the pool (avg 46-58 ms against
  78-132 ms). vkmark 62-67 pooled, 50 without; glmark2 53-75 against 58.
- **Steam's UI renders on llvmpipe** (`default_gpu_id: 1`, llvmpipe), still.
- **Host:** Xid 0 in every run. `VA Space alloc failed` bursts (13 lines each) in runs 1, 2 and 6
  (39, 26, 52 lines), none in runs 3-5, 7, 8. BAR1 and VRAM back to 24 MiB / about 1.3 GiB used
  after every down. Soak (30 min streaming, run 5): device RSS -0.8 MB, VRAM +1.5 MiB,
  foll_pin +461 pages, BAR1 peak 157 MiB: no leak.

## Comparison with the earlier builds

| Build | Slow polls | Hang at connect | Game |
|---|---|---|---|
| 68838c54 | 3 % (1000 per 3.4 min) | stream dies (-100), Sunshine "Hang detected" | not reached |
| 0c54a465 | 0.6 %, p50 3.5 ms | not recorded | signal 6 (Vulkan device -4) |
| 304012cc pinned (run 1) | 0.15 %, 25/min, p50 2.8 ms | stream works; new GPU clients stall while it runs | signal 6 with Sunshine (3/3), runs without Sunshine |
| 304012cc PIN=0 (run 2, 4, 5) | 0.04-0.11 %, 7-20/min | as run 1 (5/5 cycles) | signal 6 with Sunshine |
| no pool (run 3) | none | as run 1 | signal 6 with Sunshine |

## Vulkan isolation (runs 6 and 7, fresh boots)

| Condition | vkcube |
|---|---|
| (a) fresh boot, Sunshine never started | PASS 8.7 fps |
| (b) after the CUDA NVENC probe exited (`nvenc-probe -n 300`: PASS, 120 fps encode) | PASS 12.6 fps |
| (b2) while the CUDA NVENC probe runs (`-n 3600 -r 60`) | FAIL, c5c0 status 0x57 |
| (b3) after that probe exited | PASS 10.2 fps |
| (e) Sunshine NVENC running, no client | FAIL (runs 1-4, 6) |
| (d) Sunshine NVENC stopped after its probes | PASS (runs 1-3 step a, run 6 i-d) |
| (c) fresh boot, Sunshine with the software encoder running (no CUDA process in `nvidia-smi`) | PASS 7.5 / 10.0 fps |
| after Sunshine (software) stopped | PASS 10.3 fps |

First condition that breaks Vulkan: a CUDA context alive on the mirror. Not Sunshine's own
Vulkan probe, not NVENC by itself.

## Run 8: the exclusion-release build (exclrel-604a33f4, old pool code)

| Check | Result |
|---|---|
| Host `virtmemAllocResources: VA Space alloc failed` in the run window | 0 lines |
| Trace: channel FREE "held out of set A ... until host TSG" / TSG FREE "back to set A after its host free" | 25 / 28 lines; each "back" is followed by a shadow sync with 0 failed (0-256 mapped; `run8-exclrel/setA-pairs.txt`) |
| Host Xid | 0 |
| vkcube x3 (desktop, no Sunshine) | PASS 4.2, 7.6, 7.1 fps |
| Stream start/stop | video after 5.6 s, Sunshine alive after the disconnect |
| Slow polls (old pool code, as expected) | 3.6 % of trapped writes, 521/min |

## Run 1: 304012cc, pool pinned (dma-buf mapping)

| Step (host time) | Result | Kicks/s | Slow polls (window; /min since last; dominant) | Sunshine latency min/max/avg | Xid | Guest us/sy/st %, irq/s | D state | Host: device threads %, vCPU sum %, load |
|---|---|---|---|---|---|---|---|---|
| a0-idle-nosunshine (20:15:43) | idle 20 s | 13.3 | 1; 0.0; moves | - | 0 | 3/3/1, 3961 | - | doorbel:32 vfio-us:23; 14; 3.33 |
| a-vkcube-nosunshine (20:16:13) | vkcube PASS 8.3 fps (14.4 s) | 15.0 | 20; 48.7; sync | - | 0 | 3/3/0, 4163 | - | doorbel:28 vfio-us:10; 33; 3.01 |
| a-bench-nosunshine (20:16:51) | vkmark 62, glmark2 53 | 32.9 | 53; 15.1; sync | - | 0 | 3/4/1, 3778 | - | doorbel:32 vfio-us:12; 26; 3.79 |
| b-vkcube-sunshine-noclient (20:21:31) | vkcube FAIL rc 134 (abort in demo_create_device) | 10.2 | 20; 59.1; sync | - | 0 | 3/3/2, 4578 | - | doorbel:31 vfio-us:12; 46; 3.72 |
| b-cursor-noclient (20:22:02) | cursor 96 moves/s | 29.5 | 0; 1.7; sync | - | 0 | 2/3/1, 4288 | nvidia-modeset/kthread_q | doorbel:26 vfio-us:20; 22; 5.81 |
| b2-vkcube-retry (20:23:06) | vkcube FAIL rc 134 (abort in demo_create_device) | 10.6 | 18; 20.3; sync | - | 0 | 3/3/3, 4425 | - | vfio-us:38 doorbel:36; 14; 7.45 |
| c0-vkcube-during-handshake (20:24:48) | vkcube FAIL rc 134 (abort in demo_create_device) | 10.5 | 12; 60.9; sync | - | 0 | 4/12/4, 6923 | - | vfio-us:57 doorbel:35; 180; 5.37 |
| c-vkcube-streaming (20:25:35) | vkcube FAIL rc 134 (abort in demo_create_device) | 11.3 | 6; 26.2; moves | 26.00ms/206.10ms/58.31ms | 0 | 5/9/4, 9962 | nvidia-modeset/kthread_q | vfio-us:54 doorbel:36; 80; 5.52 |
| c1-idle-streaming (20:26:44) | idle 20 s | 11.2 | 3; 12.6; sync | 24.30ms/153.40ms/54.54ms | 0 | 5/10/7, 9726 | nvidia-modeset/kthread_q gnome-shell | vfio-us:45 doorbel:43; 79; 6.99 |
| c2-cursor-streaming (20:27:18) | cursor 95 moves/s | 10.2 | 3; 9.6; sync | 46.20ms/230.90ms/96.45ms | 0 | 5/10/9, 9429 | - | doorbel:45 vfio-us:43; 77; 9.79 |
| c3-bench-streaming (20:27:57) | vkmark fail, glmark2 fail/0 | 6.8 | 79; 19.1; sync | 24.80ms/220.70ms/69.07ms | 0 | 5/10/6, 9755 | - | doorbel:45 vfio-us:43; 75; 8.58 |
| d-vkcube-after-5min (20:33:13) | vkcube FAIL rc 134 (abort in demo_create_device) | 9.9 | 9; 14.1; sync | 25.70ms/168.50ms/61.87ms | 0 | 5/9/4, 10251 | - | vfio-us:48 doorbel:40; 79; 5.65 |
| e-game-crash-dialog (20:37:10) | idle 20 s | 10.4 | 17; 19.8; sync | 25.10ms/164.60ms/57.87ms | 0 | 16/9/3, 10721 | - | vfio-us:46 doorbel:45; 163; 5.03 |

Totals: trapped_bar0_writes=439822 slow=667 pct=.151; n=667 p50=2840 p90=6071 p99=19146 max=64712 us; sum 2.49 s

## Run 2: 304012cc, CARAXES_BAR1_POOL_PIN=0

- 20:40:21 run run2-pin0 env: SOTTR_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-poollock-304012cc CARAXES_BAR1_POOL_PIN=0
- 20:44:01 up rc=0
- 20:44:01 pool mode: vfio-user-nvgpu: host BAR1: 232 MiB free (nvidia-smi -q -d MEMORY), the pool and its windows need 128 MiB, floor 96 MiB
- 20:49:41 sunshine: [2026-09-25 18:49:40.944]: Info: Found H.264 encoder: h264_nvenc [nvenc]
- 20:51:06 stream: moonlight pid 2947815, log /home/silas/git/Caraxes/caraxes/runs/sottr-2026-09-24
- 20:51:34 moonlight: 00:00:29 - SDL Info (0): Received first video packet after 3900 ms
- 21:00:57 moonlight state: 00:00:29 - FFmpeg: [h264 @ 0x7fa94a489600] Failed setup for format vulkan: hwaccel initialisation re
- 21:00:57 game launch
- 21:01:20 game crashed (new dump)
- 21:01:57 down
- 21:02:19 done: trapped_bar0_writes=398149 slow=442 pct=.111 span_s=1315.14 per_min=20.1

| Step (host time) | Result | Kicks/s | Slow polls (window; /min since last; dominant) | Sunshine latency min/max/avg | Xid | Guest us/sy/st %, irq/s | D state | Host: device threads %, vCPU sum %, load |
|---|---|---|---|---|---|---|---|---|
| a0-idle-nosunshine (20:44:03) | idle 20 s | 12.6 | 0; 0.0; moves | - | 0 | 3/3/1, 3630 | - | doorbel:27 vfio-us:19; 13; 3.03 |
| a-vkcube-nosunshine (20:44:33) | vkcube PASS 8.2 fps (14.6 s) | 13.8 | 28; 67.7; sync | - | 0 | 2/3/1, 3681 | - | vfio-us:22 doorbel:22; 11; 2.78 |
| a-cursor-nosunshine (20:44:58) | cursor 98 moves/s | 39.8 | 1; 1.9; sync | - | 0 | 2/2/0, 3314 | - | doorbel:31 vfio-us:8; 5; 2.27 |
| a-bench-nosunshine (20:45:31) | vkmark 67, glmark2 75 | 37.4 | 42; 10.8; sync | - | 0 | 3/2/1, 3637 | - | doorbel:25 vfio-us:9; 13; 4.32 |
| b-vkcube-sunshine-noclient (20:49:41) | vkcube FAIL rc 134 (abort in demo_create_device) | 12.4 | 18; 59.3; sync | - | 0 | 2/2/1, 3591 | - | doorbel:35 vfio-us:10; 5; 4.52 |
| b-cursor-noclient (20:50:28) | cursor 98 moves/s | 28.0 | 4; 74.9; sync | - | 0 | 3/3/1, 4120 | - | doorbel:27 vfio-us:23; 13; 4.07 |
| c-vkcube-after-connect (20:51:39) | vkcube FAIL rc 134 (abort in demo_create_device) | 10.5 | 9; 15.3; moves | 21.40ms/217.10ms/53.80ms | 0 | 5/10/8, 9434 | nvidia-modeset/kthread_q | doorbel:44 vfio-us:42; 94; 5.37 |
| c1-idle-streaming (20:53:18) | idle 20 s | 12.5 | 0; 17.9; sync | 22.60ms/303.70ms/45.82ms | 0 | 5/8/2, 9985 | - | vfio-us:49 doorbel:43; 70; 3.95 |
| c2-cursor-streaming (20:53:51) | cursor 98 moves/s | 17.5 | 1; 1.7; sync | 28.00ms/136.80ms/58.25ms | 0 | 4/9/2, 9915 | - | doorbel:50 vfio-us:44; 72; 3.82 |
| c3-bench-streaming (20:54:28) | vkmark fail, glmark2 fail/0 | 10.6 | 76; 15.8; sync | 24.30ms/179.10ms/53.89ms | 0 | 5/10/3, 9650 | - | vfio-us:48 doorbel:47; 85; 4.48 |
| d-vkcube-after-5min (20:59:20) | vkcube FAIL rc 134 (abort in demo_create_device) | 11.4 | 6; 9.3; moves | 24.40ms/149.10ms/54.04ms | 0 | 4/9/1, 10177 | - | vfio-us:48 doorbel:44; 66; 4.61 |
| e-game-crash (21:01:21) | idle 20 s | 11.3 | 3; 12.5; moves | 23.40ms/178.30ms/57.06ms | 0 | 18/11/4, 10427 | nvidia-modeset/kthread_q gnome-shell | vfio-us:48 doorbel:41; 167; 4.04 |

Totals: trapped_bar0_writes=398149 slow=442 pct=.111 span_s=1315.14 per_min=20.1; n=442 p50=2786 p90=7349 p99=13886 max=43817 us; sum 1.70 s

## Run 3: 304012cc, SOTTR_NO_POOL=1 (control)

- 21:02:48 run run3-nopool env: SOTTR_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-poollock-304012cc SOTTR_NO_POOL=1
- 21:06:30 up rc=0
- 21:06:30 pool mode: vfio-user-nvgpu: doorbell tokens: BAR0 0xbb0090, 4 bytes, 64 sub-region(s) with datamatch 0xca700000..=0xca70003f and one eventfd each, offered through VFIO_USE
- 21:11:47 sunshine: [2026-09-25 19:11:47.025]: Info: Found H.264 encoder: h264_nvenc [nvenc]
- 21:13:06 stream: moonlight pid 3036636, log /home/silas/git/Caraxes/caraxes/runs/sottr-2026-09-24
- 21:13:28 moonlight: 00:00:23 - SDL Info (0): Received first video packet after 2100 ms
- 21:22:52 moonlight state: 00:00:24 - FFmpeg: [h264 @ 0x7f62b4bf8a00] Failed setup for format vulkan: hwaccel initialisation re
- 21:22:52 game launch
- 21:23:15 game crashed (new dump)
- 21:23:55 moonlight pid 3036636 killed
- 21:25:15 down
- 21:25:22 done: trapped_bar0_writes=381516 slow=0 pct=0 span_s=1350.86 per_min=0

| Step (host time) | Result | Kicks/s | Slow polls (window; /min since last; dominant) | Sunshine latency min/max/avg | Xid | Guest us/sy/st %, irq/s | D state | Host: device threads %, vCPU sum %, load |
|---|---|---|---|---|---|---|---|---|
| a0-idle-nosunshine (21:06:32) | idle 20 s | 20.7 | 0; 0.0; - | - | 0 | 4/1/1, 3645 | - | doorbel:28 vfio-us:10; 11; 2.06 |
| a-vkcube-nosunshine (21:07:03) | vkcube PASS 17.7 fps (6.8 s) | 24.9 | 0; 0.0; - | - | 0 | 4/2/0, 3783 | - | doorbel:44 vfio-us:11; 24; 2.24 |
| a-cursor-nosunshine (21:07:20) | cursor 98 moves/s | 33.3 | 0; 0.0; - | - | 0 | 3/1/0, 3258 | - | doorbel:34 vfio-us:8; 19; 2.96 |
| a-bench-nosunshine (21:07:54) | vkmark 50, glmark2 58 | 36.5 | 0; 0.0; - | - | 0 | 3/1/1, 3353 | - | doorbel:37 vfio-us:9; 14; 4.56 |
| b-vkcube-sunshine-noclient (21:11:47) | vkcube FAIL rc 134 (abort in demo_create_device) | 13.3 | 0; 0.0; - | - | 0 | 3/1/1, 3874 | - | doorbel:39 vfio-us:10; 5; 4.50 |
| b-cursor-noclient (21:12:27) | cursor 98 moves/s | 36.0 | 0; 0.0; - | - | 0 | 4/1/1, 4140 | - | doorbel:36 vfio-us:10; 0; 3.70 |
| c-vkcube-after-connect (21:13:33) | vkcube FAIL rc 134 (abort in demo_create_device) | 12.2 | 0; 0.0; - | 35.90ms/242.40ms/76.66ms | 0 | 10/5/2, 9767 | nvidia-modeset/kthread_q | doorbel:59 vfio-us:31; 71; 3.41 |
| c1-idle-streaming (21:15:11) | idle 20 s | 11.1 | 0; 0.0; - | 33.30ms/313.40ms/78.11ms | 0 | 9/5/5, 9692 | - | doorbel:56 vfio-us:35; 67; 4.76 |
| c2-cursor-streaming (21:15:47) | cursor 96 moves/s | 14.2 | 0; 0.0; - | 52.90ms/252.20ms/131.78ms | 0 | 10/6/4, 9599 | - | doorbel:59 vfio-us:31; 75; 5.37 |
| c3-bench-streaming (21:16:25) | vkmark fail, glmark2 fail/0 | 11.2 | 0; 0.0; - | 41.30ms/230.90ms/100.86ms | 0 | 11/7/3, 10117 | - | doorbel:60 vfio-us:31; 68; 6.41 |
| d-vkcube-after-5min (21:21:14) | vkcube FAIL rc 134 (abort in demo_create_device) | 12.3 | 0; 0.0; - | 33.40ms/235.20ms/80.10ms | 0 | 10/6/2, 10033 | - | doorbel:60 vfio-us:30; 60; 4.75 |
| e-game-crash (21:23:16) | idle 20 s | 12.1 | 0; 0.0; - | 36.80ms/184.00ms/87.06ms | 0 | 24/6/3, 10464 | - | doorbel:60 vfio-us:31; 125; 4.06 |
| f-vkcube-after-sunshine-stop (21:24:07) | vkcube PASS 8.5 fps (14.1 s) | 18.9 | 0; 0.0; - | 37.40ms/246.80ms/80.51ms | 0 | 11/6/4, 4989 | nvidia-modeset/kthread_q | doorbel:57 vfio-us:37; 56; 5.27 |
| f2-vkcube-again (21:24:41) | vkcube PASS 7.6 fps (15.7 s) | 16.1 | 0; 0.0; - | 37.40ms/246.80ms/80.51ms | 0 | 11/6/3, 5313 | - | doorbel:58 vfio-us:35; 71; 4.68 |

Totals: trapped_bar0_writes=381516 slow=0 pct=0 span_s=1350.86 per_min=0; dominant part: {}

## Run 4: five Moonlight connect/disconnect cycles, PIN=0

- 21:26:16 run run4-cycles-pin0 env: SOTTR_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-poollock-304012cc CARAXES_BAR1_POOL_PIN=0
- 21:30:00 up rc=0
- 21:30:33 cycle 1 connect: 00:00:30 - SDL Info (0): Received first video packet after 3100 ms
- 21:33:17 cycle 1 glmark2: [build] duration=8: FPS: 0 FrameTime: inf ms                                   glmark2 Score: 0  glrc=124 
- 21:33:18 cycle 1: moonlight pid 3122221 killed
- 21:34:27 cycle 2 connect: 00:00:27 - SDL Info (0): Received first video packet after 3300 ms
- 21:37:18 cycle 2 glmark2: [build] duration=8: FPS: 0 FrameTime: inf ms                                   glmark2 Score: 0  glrc=124 
- 21:37:19 cycle 2: moonlight pid 3135310 killed
- 21:38:28 cycle 3 connect: 00:00:27 - SDL Info (0): Received first video packet after 3500 ms
- 21:41:20 cycle 3 glmark2: [build] duration=8: FPS: 0 FrameTime: inf ms                                   glmark2 Score: 0  glrc=124 
- 21:41:21 cycle 3: moonlight pid 3156122 killed
- 21:42:30 cycle 4 connect: 00:00:27 - SDL Info (0): Received first video packet after 2700 ms
- 21:45:26 cycle 4 glmark2: [build] duration=8: FPS: 0 FrameTime: inf ms                                   glmark2 Score: 0  glrc=124 
- 21:45:27 cycle 4: moonlight pid 3197425 killed
- 21:46:36 cycle 5 connect: 00:00:26 - SDL Info (0): Received first video packet after 3000 ms
- 21:49:27 cycle 5 glmark2: [build] duration=8: FPS: 0 FrameTime: inf ms                                   glmark2 Score: 0  glrc=124 
- 21:49:28 cycle 5: moonlight pid 3239850 killed
- 21:50:50 down
- 21:50:57 done: trapped_bar0_writes=451561 slow=370 pct=.081 span_s=1477.41 per_min=15.0

| Step (host time) | Result | Kicks/s | Slow polls (window; /min since last; dominant) | Sunshine latency min/max/avg | Xid | Guest us/sy/st %, irq/s | D state | Host: device threads %, vCPU sum %, load |
|---|---|---|---|---|---|---|---|---|
| c1-vkcube-after-connect (21:30:38) | vkcube FAIL rc 134 (abort in demo_create_device) | 10.6 | 28; 0.0; moves | 29.10ms/211.20ms/67.12ms | 0 | 5/9/3, 10071 | - | doorbel:41 vfio-us:38; 83; 3.79 |
| c2-vkcube-after-connect (21:34:32) | vkcube FAIL rc 134 (abort in demo_create_device) | 10.3 | 8; 13.5; moves | 22.60ms/223.10ms/59.55ms | 0 | 5/10/4, 9980 | nvidia-modeset/kthread_q | vfio-us:46 doorbel:42; 69; 6.54 |
| c3-vkcube-after-connect (21:38:33) | vkcube FAIL rc 134 (abort in demo_create_device) | 9.6 | 6; 9.3; moves | 22.20ms/205.00ms/64.48ms | 0 | 5/10/6, 9852 | - | vfio-us:44 doorbel:43; 86; 4.50 |
| c4-vkcube-after-connect (21:42:35) | vkcube FAIL rc 134 (abort in demo_create_device) | 11.0 | 16; 13.9; moves | 24.90ms/183.90ms/55.29ms | 0 | 5/10/3, 10374 | - | vfio-us:45 doorbel:42; 91; 5.44 |
| c5-vkcube-after-connect (21:46:41) | vkcube FAIL rc 134 (abort in demo_create_device) | 9.9 | 7; 17.8; sync | 29.10ms/209.50ms/69.31ms | 0 | 5/10/3, 9775 | - | vfio-us:46 doorbel:46; 82; 5.34 |
| f-vkcube-after-sunshine-stop (21:50:20) | vkcube PASS 9.0 fps (13.3 s) | 15.3 | 16; 17.8; moves | 22.00ms/139.90ms/54.58ms | 0 | 3/2/0, 3647 | - | doorbel:34 vfio-us:13; 10; 4.21 |

Totals: trapped_bar0_writes=451561 slow=370 pct=.081 span_s=1477.41 per_min=15.0; n=370 p50=2935 p90=6383 p99=11194 max=21381 us; sum 1.39 s

## Run 5: 30 min streaming soak, PIN=0

- 21:51:37 run run5-soak-pin0 env: SOTTR_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-poollock-304012cc CARAXES_BAR1_POOL_PIN=0
- 21:55:19 up rc=0
- 21:56:05 moonlight: 00:00:32 - SDL Info (0): Received first video packet after 4100 ms
- 21:58:10 gl while streaming: == offscreen, streaming [build] duration=8: FPS: 0 FrameTime: inf ms                                   glmark2 Score: 0  rc=124 == onscreen, streaming 
- 22:28:50 down
- 22:29:12 done: trapped_bar0_writes=759492 slow=280 pct=.036 span_s=2251.37 per_min=7.4

| Step (host time) | Result | Kicks/s | Slow polls (window; /min since last; dominant) | Sunshine latency min/max/avg | Xid | Guest us/sy/st %, irq/s | D state | Host: device threads %, vCPU sum %, load |
|---|---|---|---|---|---|---|---|---|
| s01-streaming-idle-1min (21:58:10) | idle 20 s | 8.5 | 6; 0.0; moves | 38.50ms/267.10ms/106.05ms | 0 | 4/13/4, 9664 | nvidia-modeset/kthread_q FeralLinuxMessa | doorbel:42 vfio-us:40; 153; 7.90 |
| s02-cursor-streaming (21:58:42) | cursor 95 moves/s | 7.3 | 10; 19.2; sync | 32.20ms/357.80ms/94.13ms | 0 | 18/10/8, 10066 | nvidia-modeset/kthread_q | vfio-us:45 doorbel:41; 163; 7.13 |
| s15-streaming-idle-15min (22:13:17) | idle 20 s | 13.0 | 0; 6.2; moves | 20.10ms/80.90ms/36.24ms | 0 | 4/7/2, 9853 | nvidia-modeset/kthread_q | doorbel:49 vfio-us:41; 63; 3.55 |
| s29-streaming-idle-29min (22:27:24) | idle 20 s | 12.7 | 0; 0.1; sync | 20.20ms/62.00ms/35.69ms | 0 | 4/8/2, 10036 | nvidia-modeset/kthread_q | doorbel:46 vfio-us:44; 56; 2.83 |
| s29-cursor-streaming (22:28:06) | cursor 97 moves/s | 13.2 | 2; 2.7; sync | 23.80ms/147.70ms/53.81ms | 0 | 4/8/2, 9993 | - | vfio-us:49 doorbel:45; 61; 2.99 |

Totals: trapped_bar0_writes=759492 slow=280 pct=.036 span_s=2251.37 per_min=7.4; n=280 p50=2828 p90=6607 p99=19219 max=25933 us; sum 1.08 s

## Run 6: fresh boot without Sunshine, isolation, SotTR without Sunshine (pinned)

- 22:29:29 run run6-iso-pinned env: SOTTR_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-poollock-304012cc
- 22:32:55 up rc=0
- 22:32:55 iso: no sunshine 
- 22:33:31 iso b1 probe: RESULT PASS rc=0 
- 22:34:03 iso b2: probe alive pid 5676
- 22:39:24 run run6-iso-pinned env: 
- 22:39:24 resume after the script error (VM kept running)
- 22:39:35 iso e: sunshine (nvenc): [2026-09-25 20:39:35.333]: Info: Found H.264 encoder: h264_nvenc [nvenc]
- 22:41:37 g2: game launched without Sunshine
- 22:43:19 g2: running (pid 9045)
- 22:44:54 run cut short by the coordinator's priority change (Windows first); the game ran from 22:43 without Sunshine

| Step (host time) | Result | Kicks/s | Slow polls (window; /min since last; dominant) | Sunshine latency min/max/avg | Xid | Guest us/sy/st %, irq/s | D state | Host: device threads %, vCPU sum %, load |
|---|---|---|---|---|---|---|---|---|
| i-a-vkcube-fresh-nosunshine (22:32:55) | vkcube PASS 8.7 fps (13.7 s) | 18.5 | 8; 0.0; moves | 23.00ms/104.60ms/40.77ms | 0 | 11/8/2, 6930 | - | doorbel:60 vfio-us:32; 54; 3.39 |
| i-b1-vkcube-after-cuda-probe (22:33:31) | vkcube PASS 12.6 fps (9.5 s) | 20.0 | 6; 24.8; moves | 23.00ms/104.60ms/40.77ms | 0 | 3/3/0, 3890 | nvidia-modeset/kthread_q | doorbel:23 vfio-us:19; 5; 2.98 |
| i-b2-vkcube-while-cuda-probe-runs (22:34:03) | vkcube FAIL rc 134 (abort in demo_create_device) | 10.8 | 13; 23.4; moves | 23.00ms/104.60ms/40.77ms | 0 | 7/4/2, 5243 | - | doorbel:40 vfio-us:12; 46; 3.60 |
| i-b3-vkcube-after-probe-exit (22:35:46) | vkcube PASS 10.3 fps (11.7 s) | 17.0 | 14; 57.4; sync | 23.00ms/104.60ms/40.77ms | 0 | 2/2/0, 3581 | - | doorbel:30 vfio-us:9; 35; 3.27 |
| i-e-vkcube-sunshine-running (22:39:35) | vkcube FAIL rc 134 (abort in demo_create_device) | 12.0 | 19; 40.2; sync | - | 0 | 2/2/1, 4149 | - | doorbel:26 vfio-us:11; 5; 2.56 |
| i-d-vkcube-after-sunshine-stop (22:40:30) | vkcube PASS 9.7 fps (12.4 s) | 16.1 | 14; 55.2; resync | - | 0 | 2/3/1, 3708 | - | doorbel:35 vfio-us:29; 12; 2.06 |
| g2a-idle-nosunshine (22:41:01) | idle 20 s | 12.9 | 1; 17.0; sync | - | 0 | 2/3/1, 3717 | - | doorbel:30 vfio-us:17; 17; 2.16 |
| g2-game-2min (22:43:19) | idle 20 s | 29.4 | 0; 17.5; sync | - | 0 | 2/3/0, 5642 | - | doorbel:34 vfio-us:32; 34; 3.36 |
| g2-cursor-game (22:43:51) | cursor 97 moves/s | 34.3 | 6; 13.8; sync | - | 0 | 9/7/3, 7559 | nvidia-modeset/kthread_q | vfio-us:43 doorbel:41; 79; 3.88 |

Totals: trapped_bar0_writes=233299 slow=452 pct=.193 span_s=930.093 per_min=29.1; n=452 p50=2495 p90=7171 p99=18217 max=30511 us; sum 1.69 s

## Run 7: Sunshine with the software encoder (pinned)

- 22:45:21 run env: SOTTR_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-poollock-304012cc SOTTR_SUN_ENCODER=software
- 22:48:55 up rc=0
- 22:48:55 sunshine: [2026-09-25 20:48:54.364]: Info: Found H.264 encoder: libx264 [software]

| Step (host time) | Result | Kicks/s | Slow polls (window; /min since last; dominant) | Sunshine latency min/max/avg | Xid | Guest us/sy/st %, irq/s | D state | Host: device threads %, vCPU sum %, load |
|---|---|---|---|---|---|---|---|---|
| i-c-vkcube-sunshine-software (22:48:56) | vkcube PASS 7.5 fps (16.0 s) | 15.5 | 26; 0.0; moves | - | 0 | 8/9/3, 6398 | nvidia-modeset/kthread_q | doorbel:55 vfio-us:39; 67; 3.33 |
| i-c2-vkcube-sunshine-software-again (22:49:23) | vkcube PASS 10.0 fps (12.0 s) | 16.8 | 15; 39.4; sync | - | 0 | 3/3/1, 4162 | - | doorbel:38 vfio-us:10; 18; 3.07 |
| i-c3-vkcube-after-sunshine-stop (22:49:53) | vkcube PASS 10.4 fps (11.6 s) | 16.8 | 16; 34.8; sync | - | 0 | 3/3/1, 4107 | - | doorbel:25 vfio-us:17; 5; 2.97 |

Totals: trapped_bar0_writes=84624 slow=111 pct=.131 span_s=297.538 per_min=22.3; n=111 p50=2597 p90=6170 p99=14325 max=22414 us; sum 0.39 s

## Run 8: exclrel-604a33f4

- 22:50:45 run env: SOTTR_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-v615-exclrel-604a33f4
- 22:54:33 up rc=0
- 22:58:08 moonlight: 00:00:34 - SDL Info (0): Received first video packet after 5600 ms
- 22:59:00 moonlight pid 3652280 killed
- held_out=25 back=25 resyncs_after_back=0 resyncs_failed_nonzero=0

| Step (host time) | Result | Kicks/s | Slow polls (window; /min since last; dominant) | Sunshine latency min/max/avg | Xid | Guest us/sy/st %, irq/s | D state | Host: device threads %, vCPU sum %, load |
|---|---|---|---|---|---|---|---|---|
| v1-vkcube-desktop (22:54:40) | vkcube PASS 4.2 fps (28.3 s) | 8.2 | 877; 0.0; - | - | 0 | 3/3/1, 4114 | - | vfio-us:33 doorbel:32; 17; 4.26 |
| v2-vkcube-desktop (22:55:40) | vkcube PASS 7.6 fps (15.7 s) | 13.4 | 374; 510.5; - | - | 0 | 3/3/1, 3760 | - | doorbel:25 vfio-us:21; 5; 4.05 |
| v3-vkcube-desktop (22:56:26) | vkcube PASS 7.1 fps (16.8 s) | 12.4 | 539; 834.8; - | - | 0 | 3/4/1, 3790 | nvidia-modeset/kthread_q | doorbel:31 vfio-us:22; 20; 2.99 |
| s1-streaming-idle (22:58:28) | idle 20 s | 3.6 | 428; 787.1; - | 41.90ms/312.40ms/112.63ms | 0 | 5/9/3, 9754 | nvidia-modeset/kthread_q gnome-shell | vfio-us:59 doorbel:34; 83; 7.90 |

Totals: trapped_bar0_writes=129160 slow=4601 pct=3.562 span_s=530.16 per_min=520.7; dominant part: {}

Columns: kicks/s = `W bar0 0x00696000` per second in the step's window; slow polls: in the window;
per minute since the previous step; the part with the largest share. Guest us/sy/st: `vmstat 1 5`
means. D state: guest processes in D. Host: `top -H` of the device threads (vfio-user, doorbell
reactor) and the sum of the 8 vCPU threads, host load. Steal above 5 % marks a loaded host.
In run 1, c0 ran during the Moonlight handshake (before the first video packet). A latency value
in a step without a running Sunshine (run 6, i-a to i-b3) is the last line of an old log, not a measurement.
A failed vkcube is re-run under `rmioctl-log.so` after the window (`vkcube-rmlog.txt`), so the
next step's "per minute" count includes that re-run. Run 5 has one spontaneous SotTR start at
guest 359 s (source unknown, possibly Moonlight's absolute mouse into the Steam window); it
crashed with the same signal 6.

## Trapped accesses per phase (the zero-trap candidates)

Top 15 trapped offsets per second from the step's trace window (E lines excluded). `bar3` is
the BAR2 flush read. Device-side events are ioeventfd or notifier wakeups, not traps.
The reactor's own summary lines are printed only at exit (`*/session/device-summary.txt`).

#### desktop idle, no Sunshine (pool, PIN=0) (`run2-pin0/a0-idle-nosunshine`)

- window 20.7 s; trapped per second: R bar0 730, R bar3 311, W bar0 142
- device-side events per second (not traps): doorbell ioeventfd 88, msi re-arm ioeventfd 337, host notifier events 183, mmu invalidates handled 43, shadow syncs 17, GSP RM controls 0, display kicks 13

| BAR | Offset | Name | Reads/s | Writes/s |
|---|---|---|---|---|
| bar0 | 0x00bb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 427 | 0 |
| bar3 | 0x00fea000 | fb 0x1e55cf000 | 311 | 0 |
| bar0 | 0x00bb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 214 | 0 |
| bar0 | 0x00b830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 85 | 43 |
| bar0 | 0x00b830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 43 |
| bar0 | 0x00b830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 43 |
| bar0 | 0x00696000 |  | 0 | 13 |
| bar0 | 0x00696004 |  | 3 | 0 |
| bar0 | 0x00110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 0 | 1 |
| bar0 | 0x00110c80 | NV_PGSP (GSP falcon) | 1 | 0 |
| bar0 | 0x00110c84 | NV_PGSP (GSP falcon) | 0 | 1 |
| bar0 | 0x00110094 | NV_PGSP (GSP falcon) | 1 | 0 |
| bar0 | 0x00001100 |  | 0 | 0 |
| bar0 | 0x00070010 | NV_UFLUSH_L2_FLUSH_DIRTY | 0 | 0 |
| bar0 | 0x00b80f00 | NV_VIRTUAL_FUNCTION_PRIV_L2_SYSMEM_INVALIDATE | 0 | 0 |

#### desktop idle, no Sunshine (no pool) (`run3-nopool/a0-idle-nosunshine`)

- window 20.5 s; trapped per second: R bar0 924, R bar3 485, W bar0 199, W bar1 243
- device-side events per second (not traps): doorbell ioeventfd 242, msi re-arm ioeventfd 396, host notifier events 201, mmu invalidates handled 60, shadow syncs 9, GSP RM controls 0, display kicks 21

| BAR | Offset | Name | Reads/s | Writes/s |
|---|---|---|---|---|
| bar0 | 0x00bb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 534 | 0 |
| bar3 | 0x00fea000 | fb 0x1e95cf000 | 485 | 0 |
| bar0 | 0x00bb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 267 | 0 |
| bar0 | 0x00b830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 119 | 60 |
| bar1 | 0x003dfe8c | fb 0x134a6fe8c | 0 | 109 |
| bar1 | 0x0029fe8c | fb 0x132fefe8c | 0 | 79 |
| bar0 | 0x00b830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 60 |
| bar0 | 0x00b830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 60 |
| bar1 | 0x001efe8c | fb 0x12998fe8c | 0 | 56 |
| bar0 | 0x00696000 |  | 0 | 21 |
| bar0 | 0x00696004 |  | 4 | 0 |
| bar0 | 0x00110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 0 | 0 |
| bar0 | 0x00110c80 | NV_PGSP (GSP falcon) | 0 | 0 |
| bar0 | 0x00110c84 | NV_PGSP (GSP falcon) | 0 | 0 |
| bar0 | 0x00110094 | NV_PGSP (GSP falcon) | 0 | 0 |

#### cursor test, no Sunshine (pool, PIN=0) (`run2-pin0/a-cursor-nosunshine`)

- window 22.8 s; trapped per second: R bar0 1241, R bar3 891, W bar0 350
- device-side events per second (not traps): doorbell ioeventfd 328, msi re-arm ioeventfd 509, host notifier events 268, mmu invalidates handled 103, shadow syncs 27, GSP RM controls 0, display kicks 40

| BAR | Offset | Name | Reads/s | Writes/s |
|---|---|---|---|---|
| bar3 | 0x00fea000 | fb 0x1e55cf000 | 891 | 0 |
| bar0 | 0x00bb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 684 | 0 |
| bar0 | 0x00bb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 342 | 0 |
| bar0 | 0x00b830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 207 | 103 |
| bar0 | 0x00b830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 103 |
| bar0 | 0x00b830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 103 |
| bar0 | 0x00696000 |  | 0 | 40 |
| bar0 | 0x00696004 |  | 8 | 0 |
| bar0 | 0x00110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 0 | 0 |
| bar0 | 0x00110c80 | NV_PGSP (GSP falcon) | 0 | 0 |
| bar0 | 0x00110c84 | NV_PGSP (GSP falcon) | 0 | 0 |
| bar0 | 0x00110094 | NV_PGSP (GSP falcon) | 0 | 0 |

#### streaming idle (pool, PIN=0) (`run2-pin0/c1-idle-streaming`)

- window 20.8 s; trapped per second: R bar0 925, R bar3 423, W bar0 377
- device-side events per second (not traps): doorbell ioeventfd 240, msi re-arm ioeventfd 454, host notifier events 439, mmu invalidates handled 101, shadow syncs 76, GSP RM controls 0, display kicks 13

| BAR | Offset | Name | Reads/s | Writes/s |
|---|---|---|---|---|
| bar0 | 0x00bb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 481 | 0 |
| bar3 | 0x00fea000 | fb 0x1e55cf000 | 423 | 0 |
| bar0 | 0x00b830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 202 | 101 |
| bar0 | 0x00bb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 240 | 0 |
| bar0 | 0x00b830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 101 |
| bar0 | 0x00b830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 101 |
| bar0 | 0x00bb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 0 | 62 |
| bar0 | 0x00696000 |  | 0 | 13 |
| bar0 | 0x00696004 |  | 3 | 0 |

#### streaming idle (no pool) (`run3-nopool/c1-idle-streaming`)

- window 20.8 s; trapped per second: R bar0 877, R bar3 361, W bar0 292, W bar1 1048
- device-side events per second (not traps): doorbell ioeventfd 447, msi re-arm ioeventfd 398, host notifier events 298, mmu invalidates handled 83, shadow syncs 29, GSP RM controls 0, display kicks 11

| BAR | Offset | Name | Reads/s | Writes/s |
|---|---|---|---|---|
| bar0 | 0x00bb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 472 | 0 |
| bar3 | 0x00fea000 | fb 0x1e95cf000 | 361 | 0 |
| bar0 | 0x00b830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 166 | 83 |
| bar0 | 0x00bb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 236 | 0 |
| bar1 | 0x0058208c | fb 0x13ac0208c | 0 | 189 |
| bar0 | 0x00b830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 83 |
| bar0 | 0x00b830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 83 |
| bar1 | 0x0058508c | fb 0x13ac0508c | 0 | 63 |
| bar1 | 0x0049f28c | fb 0x12fb1f28c (GPPut of a mirrored channel held | 0 | 63 |
| bar1 | 0x003dfe8c | fb 0x134a6fe8c | 0 | 42 |
| bar1 | 0x001efe8c | fb 0x12998fe8c | 0 | 35 |
| bar1 | 0x0058b08c | fb 0x13ac0b08c (GPPut of a mirrored channel held | 0 | 32 |
| bar1 | 0x007a008c | fb 0x1359c008c | 0 | 32 |
| bar0 | 0x00bb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 0 | 32 |
| bar1 | 0x0029fe8c | fb 0x132fefe8c | 0 | 26 |

#### cursor test while streaming (pool, PIN=0) (`run2-pin0/c2-cursor-streaming`)

- window 22.9 s; trapped per second: R bar0 952, R bar3 520, W bar0 370
- device-side events per second (not traps): doorbell ioeventfd 220, msi re-arm ioeventfd 425, host notifier events 357, mmu invalidates handled 104, shadow syncs 70, GSP RM controls 0, display kicks 17

| BAR | Offset | Name | Reads/s | Writes/s |
|---|---|---|---|---|
| bar3 | 0x00fea000 | fb 0x1e55cf000 | 520 | 0 |
| bar0 | 0x00bb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 493 | 0 |
| bar0 | 0x00b830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 208 | 104 |
| bar0 | 0x00bb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 247 | 0 |
| bar0 | 0x00b830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 104 |
| bar0 | 0x00b830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 104 |
| bar0 | 0x00bb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 0 | 41 |
| bar0 | 0x00696000 |  | 0 | 17 |
| bar0 | 0x00696004 |  | 3 | 0 |
| bar0 | 0x00001100 |  | 0 | 0 |

#### SotTR menu, no Sunshine (pool, pinned) (`run6-iso-pinned/g2-game-2min`)

- window 20.7 s; trapped per second: R bar0 823, R bar3 610, W bar0 190
- device-side events per second (not traps): doorbell ioeventfd 141, msi re-arm ioeventfd 414, host notifier events 199, mmu invalidates handled 54, shadow syncs 1, GSP RM controls 0, display kicks 29

| BAR | Offset | Name | Reads/s | Writes/s |
|---|---|---|---|---|
| bar3 | 0x00fea000 | fb 0x1e55cf000 | 610 | 0 |
| bar0 | 0x00bb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 474 | 0 |
| bar0 | 0x00bb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 237 | 0 |
| bar0 | 0x00b830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 107 | 54 |
| bar0 | 0x00b830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 54 |
| bar0 | 0x00b830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 54 |
| bar0 | 0x00696000 |  | 0 | 29 |
| bar0 | 0x00696004 |  | 6 | 0 |

#### SotTR menu + cursor test, no Sunshine (pool, pinned) (`run6-iso-pinned/g2-cursor-game`)

- window 22.8 s; trapped per second: R bar0 882, R bar3 710, W bar0 220
- device-side events per second (not traps): doorbell ioeventfd 174, msi re-arm ioeventfd 443, host notifier events 210, mmu invalidates handled 62, shadow syncs 0, GSP RM controls 0, display kicks 34

| BAR | Offset | Name | Reads/s | Writes/s |
|---|---|---|---|---|
| bar3 | 0x00fea000 | fb 0x1e55cf000 | 710 | 0 |
| bar0 | 0x00bb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 501 | 0 |
| bar0 | 0x00bb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 251 | 0 |
| bar0 | 0x00b830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 124 | 62 |
| bar0 | 0x00b830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 62 |
| bar0 | 0x00b830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 0 | 62 |
| bar0 | 0x00696000 |  | 0 | 34 |
| bar0 | 0x00696004 |  | 7 | 0 |

- Every phase is led by three trap classes: the usermode TIME reads 0xbb0080/84 (640-760/s),
  the BAR2 flush read (310-710/s), and the MMU invalidate triple 0xb830a0/a4/b0 (43-104
  invalidates/s, each 3 writes and 2 reads). The display kick itself is 13-40/s.
- Without the pool, guest BAR1 writes trap as well (243-1048/s).

## Files

- `runN-*/table.txt` (raw rows), `runN-*/<step>/`: `trace-window.txt.xz` (non-PROM lines, at
  most 45 s), `slowpolls.txt`, `guest.txt` (top, vmstat, D state, Sunshine latency),
  `host.txt`, `irq-rate.txt`, `vkcube*.txt`, `bench.txt`.
- `runN-*/session/`: the stage's session folder (Sunshine, Moonlight, guest journal and dmesg,
  device summary, host Xid). `gpu.log`, `host-kernel-run.txt`, `slowpoll-total.txt`.
- `run1-pinned/e-game-crash/`: the device trace 60 s before the game crash and the journal.
- `run5-soak-pin0/soak-2026-09-25-poollock-run5-soak-pin0.csv`, `soak-summary.txt`.
- `tools/`: the scripts of this run (they were run from the session scratchpad).
