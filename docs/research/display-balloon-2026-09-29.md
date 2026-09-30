<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Display reserve for NVKMS (2026-09-29)

Scope: why SotTR in a 4 GiB Linux 615 guest ends in `Failed to allocate NVKMS memory for GEM object`; how v1's display balloon worked; where the guest's VRAM sizes and the game's budget come from; which device answers could change them; alternatives ranked; one design with a test plan.

Abbreviations:
- `R` = `caraxes/runs/night-2026-09-28/sottr-ca2559f5/`; `M` = its `guest-monitor.txt`; `L` = the device log of that run, `/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b/gpu.log`.
- `T` = the 615 tree; `D` = `crates/vfio-user-nvgpu/src/`; `v1` = `/home/silas/git/Leandro`.
- `N` = v1 `matrix/traces/610.57.04/*.jsonl`: native host, RTX 2070, 610.57.04, 2026-08-21, decoded FB_GET_INFO index lists.

## Facts

### The failure

- **Layout of the run.** `fb_length` is 4096 MiB. Region 0 (usable) is [0xe9800000, 0x1e7600000), 4062 MiB. Region 1 (reserved) is 34 MiB: the BAR1 pool (32 MiB) plus the carve-out (2 MiB) [L lines 13, 24; D rpc.rs:408-437].
- **The guest RM's reserved heap.** It keeps about 170 MiB of region 0 [3 GiB guest: Total 3072, Reserved 173, `caraxes/runs/nvenc-2026-09-25/vm4-615-nopool/guest-nvidia-smi.txt`]. That puts HEAP_SIZE at about 3890 MiB at 4 GiB. This is an estimate: the run did not record `nvidia-smi -q`.
- **The heap was full.** `memory.used` stayed at 3880-3889 MiB from 00:01:46 to 00:02:25, around the refusal at 335 s [M]. nvidia-smi's Used excludes Reserved [N nvml: Reserved = TOTAL_RAM_SIZE − HEAP_SIZE, 8192 − 7772 = 421 natively]. So the guest PMA was full to within a few MiB (estimate).
- **The refused buffer was a SCANOUT buffer (inference).**
  - nvidia-drm retries a NO_SCANOUT buffer in sysmem and logs the error only if both attempts fail. SCANOUT has no fallback [T kernel-open/nvidia-drm/nvidia-drm-gem-nvkms-memory.c:654-673].
  - v1 traced the same refusal: Xwayland's window, 0x870000 bytes at 1920x1080 [v1 commit 71b0de6].
- **What NVKMS asks for.** SCANOUT is NV01_MEMORY_LOCAL_USER, TYPE_PRIMARY, PHYSICALITY_CONTIGUOUS, ATTR2_ISO, with forced 4 KiB alignment [T src/nvidia-modeset/kapi/src/nvkms-kapi.c:823-845; nvkms-types.h:92].
  - An explicit CONTIGUOUS request is never retried as non-contiguous [T src/nvidia/src/kernel/mem_mgr/video_mem.c:320-338].
  - Its default page size is 2 MiB whenever the size is at least 2 MiB [T src/nvidia/src/kernel/gpu/mem_mgr/mem_mgr.c:1781-1786]. So a 1080p buffer takes five adjacent free 2 MiB frames.
- **One refusal is fatal.** It wedges the compositor for good, even when memory is free again seconds later [v1 docs/history/open-questions-2026-09-18.md, "THE FREEZE LATCHES"].

### v1's display balloon (Q1)

- **Where it lived.** The guest module `virtio_nvrm.ko`, which carried every RM call to the host. That gave it a hook that v2 does not have.
- **What it held.** R bytes of VIDMEM on a client of its own ("nvrm-balloon").
  - The chunks use NVKMS's own SCANOUT parameters, so a freed chunk leaves a hole that the refused buffer fits.
  - A chunk is one scanout buffer S. The rest of R is one smaller chunk, for cursors. There are at most 64 chunks [v1 guest-module/virtio_nvrm/nvrm_vram.c:99-140, 197-214].
- **What it hid from processes, in two ways.**
  - The host ledger charged the chunks, so every process hit the per-VM cap R earlier.
  - FB_GET_INFO V1/V2 and NVOS32 INFO reported free = limit − used, with the chunks counted as used [v1 crates/vhost-user-nvrm/src/vram.rs:1152-1215].
- **Who got it back: only NVKMS.** The trigger is an NVKMS kernel op for LOCAL_USER VIDMEM with ISO, without NO_SCANOUT or VIRTUAL, that comes back NV_ERR_NO_MEMORY.
  - The module then freed chunks and retried the same parameters in the same call.
  - During the retry, a write lock held back every process VIDMEM allocation [nvrm_vram.c:165-195; virtio_nvrm.c:2811-2835, 6290-6350].
- **Refill.** At once when the served buffer was freed, otherwise with a 1-30 s backoff [virtio_nvrm.c:6199-6285, 6355-6375].
- **Automatic size.** `display_reserve_mib=-1` with the display on gives 5·S plus four 256x256x4 cursors, rounded up to MiB [nvrm_vram.c:57-97].
  - S = align(4W, 64) × align(H, 128), rounded to 64 KiB, as libnvidia-allocator sizes it.
  - The factor 5 comes from the measured peak above idle, 4.67·S.
- **Result.** Without the balloon, 7 of 9 SotTR runs froze at 2816 MiB. With it, two 3 GiB guests had 0 NVKMS failures [v1 commit 71b0de6; v1 docs/virtio/measurements/sottr-4q/README.md].
- **Tried and dropped in v1.** "Telling userspace a smaller card did not help" [v1 commit 71b0de6]. The game fills the room it can get until it is refused.

### Sizes and budget in a 615 guest (Q2)

- **Every size index is computed in the guest kernel.** Only the leftover non-size indices go to GSP.
  - `_kmemsysGetFbInfos` answers them from memmgr and PMA and sends the rest by RPC [T src/nvidia/src/kernel/gpu/mem_sys/kern_mem_sys_ctrl.c:342-541, 866-905].
  - FB_GET_INFO V1 is converted to V2 inside the guest [T src/nvidia/interface/deprecated/rmapi_deprecated_control.c:87].
  - The device sees `0 size entries` [caraxes/runs/display-615-2026-09-24/s4-series/gnome-start-1/device-events.txt:2905-2906; D nvml.rs:463-497].
- **Allocations never reach the device.** vkAllocateMemory (NV_ESC_RM_ALLOC or VidHeapControl ALLOC_SIZE) and NVKMS both end in `vidmemConstruct` and the guest PMA [video_mem.c:682-712; rmapi_deprecated_vidheapctrl.c:140]. A GSP client sends no RPC for them [video_mem.c:980-1000].

| Answer | Computed in the guest from | Device input | Read natively by [N] |
|---|---|---|---|
| RAM_SIZE (0x7), TOTAL_RAM_SIZE (0x8) | `fbTotalMemSizeMb` = static `fb_length` [T mem_mgr_gsp_client.c:118; mem_mgr.c:4178-4214] | `fb_length` [D rpc.rs:479] | RAM_SIZE: Vulkan/GL/EGL driver, GL total; TOTAL: CUDA, NVML (nvidia-smi Total) |
| USABLE_RAM_SIZE (0x20) | sum of non-reserved regions [mem_mgr_gsp_client.c:96-106] | region table [D rpc.rs:408-437, 651-679] | Vulkan/GL/EGL driver at init |
| HEAP_SIZE (0x9) | PMA total = region 0 minus the guest RM's reserved heap [kern_mem_sys_ctrl.c:423-448] | region 0 | CUDA; NVML (Reserved = Total − HEAP_SIZE) |
| HEAP_FREE (0x16) | PMA free frames [mem_mgr.c:4216-4292; phys_mem_allocator.c:1737-1766] | none | vulkaninfo, GL free, CUDA free, NVML free |
| FB_TAX_SIZE_KB (0x10) | memmgr HAL | none | CUDA (0) |
| NV0080 FB_GET_CAPS | capability bits, no sizes [mem_mgr_ctrl.c:217-270] | none | — |

- **Native values** with the host desktop up: RAM 8192, USABLE 7952, HEAP_SIZE 7772 and HEAP_FREE 6402-6414 MiB [N vk-enum, cuda-core, nvml].
- **Vulkan heap size and budget (inference).** The heap size probably comes from RAM_SIZE or USABLE_RAM_SIZE. The budget probably comes from HEAP_FREE, which is the only extra index vulkaninfo asks for [N vk-enum]. The formula itself is closed source.
- **SotTR's native budget tracks free VRAM, not card size (indicative).** Its native "Video Memory Budget: 5888 MB" (2026-08-20, host SaveData) is 92 % of the HEAP_FREE that vulkaninfo saw a day later, in a different desktop state.
- **Only `fb_length` is independent of the PMA.** A budget cut without a smaller PMA could only work through it. HEAP_SIZE, HEAP_FREE and USABLE_RAM_SIZE follow the region table, and so does the room NVKMS draws from.

### What the 615 guest itself offers (Q3 a, d)

- **PMA ignores ISO.**
  - `bSupportISO` is copied into PMA's region descriptor, but no PMA code reads it [mem_mgr.c:3558; grep of phys_mem_allocator/].
  - Only the legacy heap places ISO surfaces by region [heap.c:1087-1098].
- **Reserved regions are no way in for NVKMS.**
  - Reserved regions never enter the PMA [mem_mgr.c:3522-3533].
  - The reserved heap takes only kernel allocations flagged ALLOCATE_FROM_RESERVED_HEAP (RC watchdog, GR, CE utils). NVKMS does not set this flag [video_mem.c:632-638].
- **There is no display reserve natively.** `rsvdISOSize` is 0 [mem_mgr_gm107.c:1859-1874], and NVKMS and nvidia-drm hold no VRAM for later use. Natively the game just leaves headroom: a heap of 7772 MiB, of which "7.4 GiB used" leaves about 350 MiB.
- **The PMA cannot shrink or grow at runtime in a usable way.** The only runtime shrink is dynamic page offlining. It is one-way and runs only on an ECC DBE event [kernel_gsp.c:592-619; heap.c:3889-3935].
- **615 has per-cgroup VRAM accounting (memacct).**
  - Every LOCAL_USER allocation is charged to its client's cgroup before the PMA is asked [video_mem.c:626-630].
  - On kernels ≥ 6.14 it uses the dmem controller, with region `nvidia/<domain>:<bus>:<dev>.0/vidmem`. Otherwise it uses a misc-cgroup fallback [os-interface.c:2967-2978; os.c:2118-2216].
  - In the fallback, a root process sets the limits with `NV0000_CTRL_OS_UNIX_CMD_MEMACCT_SET_LIMITS` (0x3d0d, PRIVILEGED). A new limit counts only later allocations [os.c:2963-2993; memacct.c:358-417; g_client_resource_nvoc.c:2341-2355].
  - Above the limit, an allocation fails with RESOURCE_ACCOUNTING_HARD_LIMIT_EXCEEDED [memacct.c:126-160].
- **NVKMS allocations are not charged (inference).** They run on the kapi client that nvidia-drm creates at probe [nvidia-drm-drv.c:727-756, 2109]. A kernel client keeps no pid info and charges the cgroup of its creator's pid, which is gone after modprobe [client.c:120-134; os-interface.c:2980-2998].

## Alternatives (Q3), ranked

| # | Option | Device-only | Effort | Isolation | Compatibility | Verdict |
|---|---|---|---|---|---|---|
| 1 | (e) memacct cap on the user's application cgroup at HEAP_FREE − R; NVKMS, gnome-shell and Xwayland stay outside | no: one guest unit, stock driver, no patch, no module option | small (tool, unit, tests) | unchanged (inside the guest) | stock 615 API; at the cap Vulkan fails as on a full card, which v1's games survived; contiguity of R not guaranteed | **recommended** |
| 2 | (h) bigger profile | yes | none | host VRAM per VM | none | only delays: SotTR sizes its pool to the budget (2026-09-28, 3 and 4 GiB) |
| 3 | (b) cut `fb_length` below the regions | yes, `D rpc.rs:479` | tiny | — | nvidia-smi Total and GL total drop; Reserved = Total − HEAP_SIZE goes negative beyond about 205 MiB at 4 GiB; CUDA unchanged | reject: v1 found that a smaller card did not help |
| 4 | (c) more FB in region 0 than advertised | yes, `D rpc.rs:408-437`, `fb.rs:188-191` | small | + host VRAM | nvidia-smi Reserved turns negative | reject: the extra joins HEAP_FREE, so it becomes the game's budget |
| 5 | (a) ISO-only or reserved region for the display | yes | — | — | — | impossible (PMA ignores ISO; reserved regions and the reserved heap are closed to NVKMS) |
| 6 | (f) port the v1 balloon into a guest kernel module | no | medium | — | needs a hook in the NVKMS→RM path | out: a guest patch |
| 7 | (g) disable direct scanout in Mutter, or put the display on another device | no | medium to large | — | one extra copy per frame; Mutter's own buffers remain | reject |

- **Windows guests.** VidMm owns placement and evicts application allocations to make room for display surfaces [T mem_mgr.c:2120-2127]. Nothing to do there (not measured in v2).
- **Effects of a size cut (answers to Q2):**
  - `fb_length` changes RAM_SIZE and TOTAL_RAM_SIZE: nvidia-smi Total and Reserved, the GL total and probably the Vulkan heap size.
  - Region 0 changes USABLE_RAM_SIZE, HEAP_SIZE and HEAP_FREE: nvidia-smi Reserved and Free, CUDA `cuMemGetInfo` free and total, and the Vulkan budget. It also changes the room NVKMS has.

## Sizing (Q3)

| Mode | S (MiB) | 3·S + cursors | v1 auto: 5·S + 1 MiB | proposed auto: 5 × S rounded up to 2 MiB, + 2 MiB |
|---|---|---|---|---|
| 1920x1080 | 8.44 (0x870000) | 27 | 44 | 52 |
| 2560x1440 | 15.00 | 46 | 76 | 82 |
| 3840x2160 | 31.88 | 97 | 161 | 162 |

- **Measured peaks above idle** (v1, GNOME 46, one head):
  - 1080p: 34.0 MiB (X11 fullscreen) and 25.4 MiB (Wayland);
  - 1440p: 38.6 and 39.7 MiB;
  - 4K: 148.9 and 88.1 MiB;
  - plus at most 1.3 MiB for cursors and 1.4 MiB for Moonlight [v1 nvrm_vram.c:67-77].
- **Triple buffering is too small.** At 1080p, X11 fullscreen peaks at 34.0 MiB, above 27.
- **Why round to 2 MiB.** The PMA hands out 2 MiB pages for these buffers, but memacct charges the requested bytes. Rounding each S up to 2 MiB keeps five buffers inside R.

## Recommendation (Q4)

- **Design: the application cap.** The stock 615 memacct limits the user's application cgroup (`user@<uid>.service/app.slice`: Steam, games, CUDA, Sunshine) to the HEAP_FREE at session start minus R.
  - gnome-shell and Xwayland (session.slice) and NVKMS's kapi client stay outside the cap, so R stays free for display buffers.
  - The semantics are v1's: a process never takes R, and NVKMS gets it without a hook.
- **Device: no answer changes.**
  - Keep `fb_length` = profile [D rpc.rs:479], the two regions [D rpc.rs:408-437, 651-679] and usable = reservation − carve-out − pool [D fb.rs:188-191].
  - Keep FB_GET_INFO_V2 as it is: size indices never reach it [D nvml.rs:463-497].
  - R comes out of the profile, as in v1.
- **Guest: one unit** (for example `caraxes-vram-cap`, delivered with the launcher's cloud-init seed like v1's `leandro-display.conf`).
  - It runs as root when `app.slice` of the desktop user appears, before Steam and Sunshine.
  - It reads HEAP_FREE (FB_GET_INFO_V2) and the connector's current mode (DRM), then computes R from the table.
  - It sets the hard limit to HEAP_FREE − R:
    - on 6.8 (Ubuntu 24.04 GA), with SET_LIMITS (cgroupFd = app.slice, the guest's gpuId);
    - on kernels ≥ 6.14, by writing the dmem region and the byte count to app.slice's `dmem.max`.
- **Knob: one, automatic, v1-compatible.** `display_reserve_mib`: −1 = auto (default), 0 = off, N = fixed MiB.

### Test plan

1. **Checks before the game.**
   - The guest's memacct implementation, with `NV0000_CTRL_OS_UNIX_CMD_MEMACCT_GET_IMPL` (0x3d0f): FALLBACK is expected on 6.8.
   - The cgroups of Xwayland, gnome-shell, Sunshine and Steam, from `/proc/<pid>/cgroup`.
   - `nvidia-smi -q -d MEMORY` (Total, Reserved, Free).
   - `vulkaninfo` heaps and budget, inside and outside app.slice.
2. **Recording per run.**
   - `nvidia-smi -q -d MEMORY` every 5 s.
   - The count of NVKMS failures and Xids in dmesg.
   - The game's budget line and its result file.
   - The host stream's frame difference (as in R).
3. **SotTR benchmark**, 1920x1080 exclusive fullscreen, texture quality 2.
   - Five runs at 4 GiB and five at 3 GiB, with the unit on.
   - Pass: 0 NVKMS failures, 0 Xids, the benchmark result file written, the picture changing on the host until the benchmark ends, Sunshine streaming throughout.
   - Five runs is the minimum, because v1 failed 7 of 9.
4. **Negative control.** One run with `display_reserve_mib=0` must reproduce the refusal. It did twice on 2026-09-28 and once in R.
5. **Side checks.**
   - Smoke tests: vkcube, glmark2/vkmark, CUDA.
   - CUDA inside app.slice: `cuMemGetInfo` free must be no more than the cap; allocating past it fails cleanly.
   - nvidia-smi Total and Reserved unchanged.
6. **Performance.** FPS and frame time against native (91.7 FPS) and v1, as in every milestone.

## Decisions for the user

1. Allow one guest-side unit? It uses the stock driver and the stock NVIDIA API, with no patch and no module option. If not, a Linux guest keeps this failure mode, and the only device-side lever is a larger profile.
2. R out of the profile (as in v1), or on top of it (the device grows the host reservation by R, which costs +R of host VRAM per VM)?
3. `display_reserve_mib` (auto by default) as one of the 4-5 knobs?

## Open

- **The Vulkan budget formula:** which indices it uses, and whether the 615 driver reads the memacct limit (`MEMACCT_GET_LIMITS`, which is unprivileged). Settled by step 1 of the test plan.
- **Contiguity of R under churn.** The cap guarantees bytes, not five adjacent free 2 MiB frames. If failures persist, raise the margin.
- **Pid reuse.** If an app.slice process reuses the kapi client's creator pid, NVKMS lands under the cap.
- **UVM.** Managed allocations take PMA pages uncharged. They can be evicted for pinned allocations, so the expectation is that they do not block NVKMS (not measured).
