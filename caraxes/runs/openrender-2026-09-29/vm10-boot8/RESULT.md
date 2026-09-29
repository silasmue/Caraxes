<!-- SPDX-License-Identifier: MIT -->
# vm10-boot8: BAR2 memfd on (6246860b), boot7's fast paths

- Regressed against boot7: vulkaninfo and vk-offscreen "failed to idle channel 3" (15 s each), "job timeout, channel 5 killed"; vk-offscreen rc=3 after `vkCreateDevice` 15089 ms; weston's clients fail [serial.log, results/dmesg.txt].
- Cause: chid 3's USERD (FB 0x69808000) is behind the BAR2 memfd (nouveau clears it through its BAR2 kmap) and in the BAR1 pool (nouveau writes GPPut through BAR1, straight into VRAM). The device read USERD from the memfd, saw GPPut 0 and never ran the init push boot7 ran ("chid 3 gp 0: pushbuffer va 0x8000000000 51 dwords"); only 4 chid 3 doorbells, none of chid 5 [device.txt; gpu.log "is behind the BAR2 memfd as well"].
- The 15 s of `vkCreateDevice` are nouveau's channel-idle timeout, not BAR2 writes: 1872 syncs took 640 ms in total (49 ms max), 6680 pages loaded, 1072 live at most; 0 accesses of the trapped flush page (nouveau flushes at BAR2 0x8000) [gpu.log exit line].
- Sync points: 1190 MMU invalidates, 670 GSP queue heads, 12 clock; 34 NV_UFLUSH_FB_FLUSH writes; 17 channel doorbells, 16 of them on chid 3 (software, no walk: all live pages, 831 us max). Drift 1 of 35 sampled pages (FB 0x6980a000, a pool page: the guest's BAR1 writes).
- Fix: ec59bd82, device reads merge VRAM with the unsynced BAR2 bytes (docs/caraxes/bar2-memfd.md §3 rule 5).
