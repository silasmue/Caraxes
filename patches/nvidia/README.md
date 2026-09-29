<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# NVIDIA open kernel module patches

Against [NVIDIA/open-gpu-kernel-modules](https://github.com/NVIDIA/open-gpu-kernel-modules) tag `615.71.09`. The patch is `git format-patch` output, so `git am` and `git apply` accept it.

| Patch | Change | Files |
|---|---|---|
| [0001](0001-nvidia-add-a-display-reserve-for-kernel-display-allo.patch) | Display reserve: RM holds video memory for the scanout allocations of kernel clients (NVKMS) and gives it back when one is refused for lack of memory. Registry keys `RmDisplayReserve` (off by default) and `RmDisplayReserveSizeMB`. Design and evidence: [../../docs/research/display-reserve-2026-09-29.md](../../docs/research/display-reserve-2026-09-29.md) | `src/nvidia/src/kernel/mem_mgr/display_reserve.c` (new), `src/nvidia/inc/kernel/mem_mgr/display_reserve.h` (new), `src/nvidia/interface/nvrm_registry.h`, `src/nvidia/src/kernel/gpu/mem_mgr/mem_mgr.c`, `src/nvidia/src/kernel/mem_mgr/video_mem.c`, `src/nvidia/srcs.mk` |

The patch changes only the RM core (`nv-kernel.o`), which distribution packages ship prebuilt as `nvidia/nv-kernel.o_binary`. The guest build for game tests is [../../crates/vfio-user-nvgpu/guest/display-reserve/build.sh](../../crates/vfio-user-nvgpu/guest/display-reserve/build.sh); it never replaces the installed driver.
