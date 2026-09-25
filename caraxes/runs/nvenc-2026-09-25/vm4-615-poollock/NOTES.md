<!-- SPDX-License-Identifier: MIT -->
# vm4-615 night run of the BAR1-pool lock fix (304012cc), 2026-09-25/26

Work in progress (the night run commits after every configuration). Method, per step: `vkcube-wayland --c 120`
(60 s timeout) or a plain 20 s window, then the device trace of that window (display kicks
`W bar0 0x00696000`, slow polls `held the device`), Sunshine's last `Frame processing latency`
line, host Xid since boot, guest `vmstat 1 5` (us/sy/st, interrupts), D-state processes, host
`top -H` of the device and of Cloud Hypervisor's vCPU threads. Scripts: `tools/` in this folder.

## Run 1: 304012cc, pool pinned (dma-buf mapping on)

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
