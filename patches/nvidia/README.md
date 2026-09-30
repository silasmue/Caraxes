<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# NVIDIA open kernel module patches

Against [NVIDIA/open-gpu-kernel-modules](https://github.com/NVIDIA/open-gpu-kernel-modules) tag `615.71.09`. The patch is `git format-patch` output, so `git am` and `git apply` accept it.

| Patch | Change | Files |
|---|---|---|
| [0001](0001-nvidia-nvidia-modeset-hold-a-video-memory-reserve-fo.patch) | Display reserve: NVKMS declares how much video memory RM holds for scanout surfaces, from the committed modes and the bound surfaces (new kernel-privileged control `NV0073_CTRL_CMD_SYSTEM_SET_DISPLAY_RESERVE`, `nvidia-modeset` parameter `display_reserve_mib`, off by default). RM holds it as PMA page runs, gives a run to a kernel scanout allocation refused for lack of memory, and takes such an allocation back into the reserve when it is freed. Design and evidence: [../../docs/research/display-reserve-2026-09-29.md](../../docs/research/display-reserve-2026-09-29.md) | RM: `src/nvidia/src/kernel/gpu/mem_mgr/display_reserve.c` (new), its header (new), `heap_base.h`, `mem_mgr.c`, `video_mem.c`, `disp_common_kern_ctrl_minimal.c`, `srcs.mk`, generated `g_disp_objs_nvoc.c/h`, `g_mem_mgr_nvoc.h`; `ctrl0073system.h`; NVKMS: `nvkms-rm.c/h`, `nvkms-modeset.c`, `nvkms-flip.c`, `nvkms-types.h`, os-interface headers; `kernel-open/nvidia-modeset/nvidia-modeset-linux.c` |

[display-reserve/](display-reserve/README.md) holds the bare-metal reproducer (`scanout-oom.c`), the test pack (`testpack.sh`) and the pull-request text (`PR.md`); `615.71.09` is also upstream's latest release tag and `main` (checked 2026-09-29).

The patch changes the RM and NVKMS cores, which distribution packages ship prebuilt (`nv-kernel.o_binary`, `nv-modeset-kernel.o_binary`). The guest build for game tests is [../../crates/vfio-user-nvgpu/guest/display-reserve/build.sh](../../crates/vfio-user-nvgpu/guest/display-reserve/build.sh); it builds the patched tree and never replaces the installed driver.
