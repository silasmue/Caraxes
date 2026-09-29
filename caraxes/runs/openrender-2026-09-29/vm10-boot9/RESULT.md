<!-- SPDX-License-Identifier: MIT -->
# vm10-boot9: BAR2 memfd on (ec59bd82), boot7's fast paths

- R4/R5 PASS (clear+copy 0 of 65536 wrong, triangle 20808 pixels), `vkCreateDevice` 153.8 ms (boot7: 17965 ms), vkcube-wayland 39 FPS steady (weston's FIFO), weston-screenshooter rc=0, no timeout in dmesg, 0 device ERROR [serial.log, results/].
- Throughput below boot7: glmark2-wayland 2928 (3781, -23 %), vkmark 4472 (5575, -20 %) [serial.log].
- Cost in the exit line: 170455 channel doorbells, each a partial sync of about 25 pages (4.24 M compares, 6 dirty pages found, 1.65 s, 9.7 us per doorbell); 4804 full syncs in 2.16 s (3483 MMU invalidates, 1307 GSP queue heads, 14 clock) scanning up to 1374 live pages; 10534 pages entered and left [gpu.log].
- Next: 5fcf62ae, compares in place and page tables at the invalidate instead of the doorbell (docs/caraxes/bar2-memfd.md §3 rules 2 and 4).
