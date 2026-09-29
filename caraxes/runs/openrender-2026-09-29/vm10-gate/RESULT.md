<!-- SPDX-License-Identifier: MIT -->
# Combined gate for openrender-2026-09-29 (5fcf62ae), 2026-09-29

- nova-core 5-probe (nova-gate/, pure emulation): 5 probes ("GPU name: Leandro VFIO RTX 2070-1G"), 4 unloads ("GSP successfully unloaded"), every bind rc=0, 0 device ERROR.
- vm10 R1-R8 (vm10-gate/, BAR2 memfd on, boot7's fast paths): PASS; `vkCreateDevice` 80.8 ms; vk-offscreen clear+copy 0 of 65536 wrong, triangle 20808; vkcube-wayland 39 FPS (weston FIFO); 0 device ERROR. glmark2 2733 / vkmark 3931 ran beside Windows C55's probe pack: not a performance number (the quiet-card control is vm10-boot11a/b).
- Linux B 615 kick-smoke (1 GiB, slot 4, BAR2 memfd on; /mnt/vmstore/caraxes/runs-2026-09-29/gate-openrender/): RESULT PASS; vkcube wayland/x11, glmark2 build 3451, NVENC holder PASS, guest Xid 0, stream shots change (RMSE 12653/1670), the 2 known "untrapped GPPut ran ahead" detector lines, no host Xid.
- dev fast-forwarded to a7a61aed.
