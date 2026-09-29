<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Display reserve: reproducer and bare-metal test pack

For the patch [../0001-nvidia-nvidia-modeset-hold-a-video-memory-reserve-fo.patch](../0001-nvidia-nvidia-modeset-hold-a-video-memory-reserve-fo.patch). Design and evidence: [../../../docs/research/display-reserve-2026-09-29.md](../../../docs/research/display-reserve-2026-09-29.md). PR text: [PR.md](PR.md).

## The problem in one run

- A process that sizes itself to the free video memory (a game sizing its pools to the Vulkan memory budget; here, CUDA) takes all of it.
- The next scanout allocation of the display stack has no fallback and fails. The client is typically Xwayland through NVIDIA's GBM backend, which calls `DRM_IOCTL_NVIDIA_GEM_ALLOC_NVKMS_MEMORY`.
  - The kernel logs `nv_drm_gem_alloc_nvkms_memory_ioctl … Failed to allocate NVKMS memory for GEM object`.
  - The client loses its buffer, and the display stops updating or the game exits.

## Reproducer: `scanout-oom.c`

- **Build:** `cc -O2 -Wall -o scanout-oom scanout-oom.c -ldl`. It needs the kernel UAPI headers (`drm/drm.h`). It loads CUDA at run time from the driver's `libcuda.so.1`, so no CUDA toolkit is needed.
- **Run** as root, as DRM master, so no display manager or compositor may run: `systemctl stop display-manager`, then run it over SSH or from a text console.
  - `./scanout-oom [-d /dev/dri/cardN] [-r ROUNDS] [-b BUFFERS]` (default card0, 3 rounds, 2 buffers).
- **What it does:**
  1. Lights the first connected connector with its preferred mode.
  2. Fills that GPU's free video memory from a CUDA context, in 64, 8 and 2 MiB steps.
  3. Each round: allocates the buffers as Xwayland's GBM backend does (block linear, the size of an XRGB8888 surface of the mode, 0x870000 bytes at 1920x1080), holds them for a second and frees them. It then lets CUDA take whatever became free for a second, as a game does.
- **Expected:**

| Driver | Result | Exit |
|---|---|---|
| Stock | `buffer 1 refused (Invalid argument)` in round 1, `REPRODUCED` | 1 |
| Patched, `nvidia-modeset display_reserve_mib=-1` | every buffer granted; the kernel log (with `NVreg_RmMsg=display_reserve.c`) shows `released …` at each allocation and `took back …` at each free | 0 |
| Patched, `display_reserve_mib=0` (default) | as stock | 1 |

- **Other modes:**
  - `-M` walks the modes: preferred, smallest, off. The reserve follows (`declared …` lines).
  - `-H SECONDS` lights the preferred mode and holds it, for suspend tests.

## Test pack: `testpack.sh`

- **Make the pack (at home):** `testpack.sh pack -o DIR --ogkm OPEN_GPU_KERNEL_MODULES_GIT --tag 615.71.09`.
  - The result is `DIR/display-reserve-testpack-615.71.09.tar.gz` and its `.sha256`. It holds the release's source (`git archive` of the tag), the patch, `scanout-oom.c`, this README and `SHA256SUMS`.
- **Test machine prerequisites:**
  - bare metal, a Turing or newer NVIDIA GPU, and the NVIDIA driver of exactly the pack's release installed, since its user-space libraries must match the modules;
  - kernel headers, `gcc` and `make`;
  - Secure Boot off, because the patched modules are unsigned;
  - `nvidia-drm` with `modeset=1`.
- **Run (on the test machine):**
  1. `systemctl stop display-manager` (and `systemctl stop nvidia-persistenced` if it runs).
  2. Log in over SSH, or use a text console. The console goes dark while the NVIDIA modules are reloaded, so SSH or `tmux` is the comfortable way.
  3. `tar xzf display-reserve-testpack-615.71.09.tar.gz && cd display-reserve-testpack`
  4. `sudo ./testpack.sh run ~/testpack-out`, or `--steps all` to add suspend, runtime D3 and hotplug.
  5. Reboot afterwards. The script loads the installed modules again at the end, but a reboot is the clean state.
- **Dry run:** all default steps passed in a Linux VM with a virtual Turing GPU (1 GiB). The stock driver refused all six buffers; the patched one granted all six and took each back on free (see the research note).
- **Steps** (`--steps`, comma-separated):

| Step | What | Expected |
|---|---|---|
| `check` | root, no display manager, compositor or persistenced, driver release, headers, Secure Boot, GPUs | pass |
| `build` | unpack, patch, `make modules` against the running kernel, build `scanout-oom` | pass |
| `stock` | `scanout-oom` on every NVIDIA card with the installed modules | refused (exit 1) |
| `load` | replace the modules with the patched ones: `display_reserve_mib=-1`, `NVreg_RmMsg=display_reserve.c`, the machine's `modprobe.d` options | pass |
| `repro` | `scanout-oom` on every NVIDIA card | granted (exit 0) |
| `modes` | preferred, smallest, off | `declared` lines follow |
| `control` | `display_reserve_mib=0` | refused (exit 1) |
| `suspend` (opt-in) | suspend to RAM for about 30 s with the display lit, then `scanout-oom -r 1` | resume clean, granted |
| `rtd3` (opt-in) | on a GPU with runtime D3 and no display lit: wait for D3, then light it | granted |
| `hotplug` (opt-in) | unplug and replug the display when asked | `declared 0`, then the size again |
| `unload` | a take-back, then unload the patched modules | clean: no scrubber, assertion or reserve errors (NVKMS declares 0 before RM unloads, so RM normally frees no runs itself) |

- **Result:** `~/testpack-out/display-reserve-testpack-<stamp>.tar.gz` and `.sha256`. It holds `run.log`, one log per step, `build.log` and `kernel.log` (the kernel lines of the NVIDIA modules, power management and warnings).
  - The host name and the user name are replaced.
  - GPU UUIDs and serial numbers are not collected.
  - The pack calls no network tool.
- **Risks:**
  - Suspend and runtime D3 run only when named.
  - The pack loads unsigned modules built from source. A failure can leave the machine without a display until reboot, so keep SSH or a second machine at hand.
