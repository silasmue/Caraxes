<!-- SPDX-License-Identifier: MIT -->
# vm10-boot10: BAR2 memfd on (5fcf62ae), boot7's fast paths; Windows C54 beside it

- PASS; `vkCreateDevice` 60.2 ms; glmark2-wayland 3215 (build 3528, texture 3062, shading 3059), vkmark 4473 (cube 4468, clear 4478); vkcube 39 FPS (weston FIFO) [serial.log].
- Against boot7 (alone on the card): build equal (3528 / 3505), texture +64 us, shading +80 us, cube +40 us, clear +48 us per frame; vkmark equal to boot9 (4472) although the doorbell cost fell 5x.
- Exit line [gpu.log]: 178028 doorbells, partial syncs of 4.2 pages each (753846 compares, 14 dirty pages), 326 ms in total, 1.8 us per doorbell (boot9: 9.7 us); largest doorbell set 4 live pages, 43 live table pages left to the invalidate. 3445 invalidates synced table pages only (137557 pages, 534 ms, 155 us each: mostly the BAR2 walk's loads, 10535 pages entered and left); 41 invalidates with a flush owed (22 us); 1307 GSP queue heads 317 ms (243 us each); 13 clock. Page compares 1.29 M (boot9 4.68 M).
- Not separable from the environment: Windows C54 (idle desktop, 60 Hz flips, trace on) ran beside it. The control: vm10-boot11a (memfd off) and vm10-boot11b (memfd on), the same binary, back to back on a quiet card.
