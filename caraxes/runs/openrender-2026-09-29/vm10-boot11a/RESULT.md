<!-- SPDX-License-Identifier: MIT -->
# vm10-boot11a: control, BAR2 memfd off (5fcf62ae), boot7's settings, quiet card

- vk-offscreen PASS; `vkCreateDevice` 13384.6 ms (the GR context zeroing through trapped BAR2 writes); glmark2-wayland 4186 (build 4171, texture 4230, shading 4162), vkmark 5791 (cube 5775, clear 5808); vkcube 39 FPS (weston FIFO); 0 device ERROR [serial.log, gpu.log].
- Run back to back with vm10-boot11b; the only difference is `CARAXES_BAR2_MEMFD`.
