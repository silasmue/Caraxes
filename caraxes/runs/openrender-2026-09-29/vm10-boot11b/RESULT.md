<!-- SPDX-License-Identifier: MIT -->
# vm10-boot11b: control, BAR2 memfd on (5fcf62ae), boot7's settings, quiet card

- vk-offscreen PASS; `vkCreateDevice` 48.2 ms (boot11a: 13384.6); glmark2-wayland 4383 (build 4359, texture 4338, shading 4457), vkmark 5632 (cube 5488, clear 5776); vkcube 39 FPS (weston FIFO); 0 device ERROR [serial.log, gpu.log].
- Against boot11a: glmark2 +4.7 %, vkmark -2.7 % (cube -5.0 %, clear -0.6 %): within the run-to-run spread, so the memfd costs no throughput; boot10's loss was the Windows VM beside it.
- Exit line [gpu.log]: 231092 doorbells, 1.6 us each (4.2 pages compared, 12 dirty pages in all); 3436 invalidates synced table pages only (511 ms, 149 us each); 1307 GSP queue heads 240 ms; 41 invalidates with a flush owed, 13 clock; 4797 syncs 737 ms in total; drift 1 of 84 sampled pages (FB 0x6980a000, a BAR1 pool page).
