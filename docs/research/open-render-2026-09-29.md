<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Rendering through an open driver: nova, nouveau with NVK, and the device (2026-09-29)

Scope: the shortest open-source path to a render read back in a guest on the device (nova upstream, nouveau with GSP-RM plus Mesa NVK, simpler options); what nouveau's r570 path sends and what the device lacked; the guest image; the plan with sizes and the evening goal. Branch `openrender-2026-09-29` (from gfxp-2026-09-29 139662fa). Trees: Linux 7.2.7 (`/mnt/vmstore/caraxes/nova-kernel/src/linux-7.2.7`, `N/` = `drivers/gpu/drm/nouveau/`), OpenRM 570.144 (`/mnt/vmstore/caraxes/nova-kernel/src/open-gpu-kernel-modules-570.144`), the 615 tree.

## Facts

### (a) nova upstream: no rendering in any tree

- Mainline v7.3-rc5 (2026-09-27) `include/uapi/drm/nova_drm.h` has the three ioctls of 7.2.7: `GETPARAM`, `GEM_CREATE`, `GEM_INFO` [git.kernel.org torvalds/linux.git v7.3-rc5].
- nova-core v7.3-rc5 adds FSP/MCTP, vGPU and TLV firmware loading; no fifo, channel, runlist, RM alloc or interrupt code [same tree, `drivers/gpu/nova-core`].
- drm-rust-next head 10a6623a (2026-09-27) adds one read-only ioctl, `DRM_NOVA_INFO` (architecture, chip id, VRAM size, names, GID), and nova-core `mm/` (VMM, page tables, TLB, BAR1, PRAMIN over drm_buddy) [gitlab.freedesktop.org/drm/rust/kernel].
- In flight, none with submission: interrupts and GSP events v3 (2026-09-03), RM control infrastructure v2 (2026-03-18, "groundwork for eventual channel allocation"), the r000 stable-ABI firmware series v2 (2026-08-22), a vGPU manager with a VFIO variant driver (2026-09-05) [lore / ratatoskr archives].
- The nova todo list still lists the FIFO engine (channels, chid allocation), GR and CE as unclaimed [docs.kernel.org/gpu/nova/core/todo.html].
- Mesa: NVK's kernel abstraction `src/nouveau/vulkan/nvkmd/` has only the nouveau backend in main; MR !43352 "Draft: nvk: Add initial support for Nova driver" (opened 2026-07-31, last update 2026-08-24) probes a physical device only [gitlab.freedesktop.org/mesa/mesa].
- Firmware: nova since 7.3 requests unversioned `nvidia/<chip>/gsp/*.tlv`; linux-firmware 20260916 ships none, only `*-570.144.bin` and `*-535.113.01.bin` for tu102 (tu104/tu106 link to it) [linux-firmware WHENCE, tag 20260916].

### (b) nouveau speaks the firmware the device already serves

- nouveau of Linux 7.2.7 lists GSP-RM 570.144 before 535.113.01 for TU102/TU104/TU106 and loads the first whose files exist; `NvGspRm` defaults to on [N/nvkm/subdev/gsp/tu102.c:459-499, N/include/nvkm/core/firmware.h:46-80].
- Its r570 API table is r570 gsp, client, fbsr, disp, fifo, gr plus r535 rpc, ctrl, alloc, device, ce [N/nvkm/subdev/gsp/rm/r570/rm.c:52-75].
- The boot exchange equals nova's: the pre-610 queue element, `GSP_SET_SYSTEM_INFO` (928 bytes), `SET_REGISTRY` with the same three keys, the CPU sequencer, `GSP_INIT_DONE`, the 1656-byte static info, `UNLOADING_GUEST_DRIVER` [N/nvkm/subdev/gsp/rm/r570/gsp.c:85-189, r535/gsp.c:588-707; docs/caraxes/nova-guest.md §2.1].
- Beyond nova it takes the GSP interrupt, tolerates unknown events (hex dump at WARN), and allocates RM objects: root, device, subdevice, `FERMI_VASPACE_A`, bare `TURING_CHANNEL_GPFIFO_A` channels (no TSG), `TURING_A`, `TURING_COMPUTE_A`, `KEPLER_INLINE_TO_MEMORY_B`, `FERMI_TWOD_A`, `TURING_DMA_COPY_A` [N/nvkm/subdev/gsp/rm/r535/rpc.c:494-508, r570/fifo.c:19-96, r535/alloc.c:55-104].
- It is a GSP client like the 615 kernel RM: it writes its GMMU v2 tables in VRAM itself and hands the roots over with `DMA_SET_PAGE_DIRECTORY` or `COPY_SERVER_RESERVED_PDES`, promotes GR context buffers with `GPU_PROMOTE_CTX`, rings 0xBB0090 with `(runlist << 16) | chid`, uses the VF interrupt tree and one MSI [N/nvkm/subdev/gsp/rm/r535/vmm.c:27-149, r535/gr.c:59-140, N/nvkm/engine/fifo/tu102.c:34-38, N/nvkm/subdev/vfn/tu102.c:28-113].
- It needs no PRI register the device would have to forward; display is removed before any display RPC when `NV_FUSE_STATUS_OPT_DISPLAY` reads 1, as the device answers by default [N/nvkm/subdev/devinit/gm107.c:44-45; crates/vfio-user-nvgpu/src/display/mod.rs:1553-1556].
- Rendering needs host execution: pure emulation answers the RPCs but runs no GR work, and nouveau's kernel channels wait on fences at init [N/nouveau_drm.c:463-545].

### What differed, and what the branch adds

| Item | 570.144 against the device (610 layouts) | Evidence | Branch |
|---|---|---|---|
| `rpc_gsp_rm_control_v03_00` | params at 24 (one flags word); 595/610/615 at 40 | caraxes/abi/gsp-570.144/rpc layouts; 570 `g_rpc-structures.h:1388-1397` | `ver::RmControlFrame`, boot.rs (67fb1128) |
| `NV_CHANNEL_ALLOC_PARAMS` | 368 bytes; 610 inserted `hHandleVASpace` at 32, all later fields +8 | caraxes/abi/delta-615.71.09-570.144.md | generated 570<->610 tables, translated at the RPC (67fb1128) |
| `NV_VASPACE_ALLOCATION_PARAMETERS` | 48 bytes; `pasid` appended in 595 | same | same |
| `GPU_GET_FERMI_GPC_INFO`/`TPC_INFO` 0x20800137/38 | refused (0x56); nouveau fails GSP init without them | N/nvkm/subdev/gsp/rm/r570/gr.c:15-52 | answered from `GR_GET_GPC_MASK`/`TPC_MASK` as the RM does (86d0e71f) |
| `GR_GET_ZCULL_INFO` 0x20801206 | refused; fatal in nouveau's GR oneinit | r570/gr.c:190-196 | host's struct (86d0e71f) |
| 0x20800A46 | 570: `INTERNAL_KGR_INIT_BUG4208224_WAR`; 610 reused the id (CHECK_CTS_ID_VALID, refused) | 570 ctrl2080internal.h:830 | acknowledged for a 570 guest (86d0e71f) |
| NVDEC1/NVDEC2 vectors | device-info runlists 5 and 6 had no kernel-table vector; nouveau fails FIFO oneinit ("nonstall -2") | N/nvkm/engine/fifo/ga100.c:533-565, base.c:287-294 | vectors 202/10 and 203/11 (86d0e71f) |
| `rpc_os_error_log_v17_00` | runlistId 4, chid 8, errString 12 (610: 8, 12, 16) | the three snapshots | per guest ABI (86d0e71f) |
| `INTR_GET_KERNEL_TABLE` subtree map | 570: `{u8 start, u8 end}[7]` at 2052 (2068 bytes); 610: u64 masks at 2056 (2112); nouveau reads only the entries | 570 ctrl2080internal.h:1462-1480 | ranges for a 570 guest (this branch) |
| `GPFIFO_SCHEDULE` | 2 bytes in 570, 3 in 610 | caraxes/abi delta 610 -> 570 | no change: echoed at the guest's size |
| FB regions | nouveau keeps only regions with `supportCompressed` and `supportISO` | N/nvkm/subdev/gsp/rm/r535/gsp.c:191-199 | no change: run with `CARAXES_COMPRESSION=slice` (production); D16 reports none, so no VRAM heap |
| FWSEC in the VBIOS | application id 0x85 only | N/nvkm/subdev/gsp/fwsec.c:274-281 | no change: `synth-vbios-nova.bin` |
| BAR flush | `r535_bar_flush` reads FB page 0 through BAR2 after instmem writes | N/nvkm/subdev/gsp/rm/r535/bar.c:33-45, 100-114 | no change: served from the device's private store, counted as outside the reservation |

- Unchanged and served: RC_TRIGGERED (48 bytes in 570 and 610, same layout hash), the MC engine indices (GR 84, NVDEC 65-67, NVENC 38 in both), the doorbell register 0xBB0090, the MSI re-arm at `NV_PCFG` + `NV_XVE_CYA_2` [caraxes/abi/gsp-570.144/events.tsv; N/nvkm/subdev/gsp/rm/r570/nvrm/engine.h; chips/tu106/chip.toml:66].
- 595 lacks `hHandleVASpace` too (368 bytes): the pinned 595 experiment's channels were read with 610 offsets [gsp-abi-sizes of ogkm-595.91.07].

### Guest image and tools

- The 615 image has Mesa 25.2.8-0ubuntu0.24.04.2 with the NVK ICD `nouveau_icd.json` and lavapipe; noble-updates and noble-security carry that version, NVK is conformant on Turing since Mesa 24.1 and needs Linux 6.6 or later [caraxes/runs/nvenc-2026-09-25/vm4-615-invhold/A-invhold/session/checks-driver.txt; launchpad mesa publishing history; docs.mesa3d.org/drivers/nvk.html].
- Kernel: the nova tree of caraxes/runs/nova-2026-09-25 plus `DRM_NOUVEAU=m` (trace level compiled in, no SVM); only modules were added, 403 s at `-j2` [caraxes/runs/openrender-2026-09-29/build-kernel.sh, kernel.config].
- `vk-offscreen`: step A a `vkCmdClearColorImage` plus a copy to a host buffer, step B a render pass with one constant-colour triangle; every pixel compared; PASS on lavapipe in an Ubuntu 24.04 container (20808 triangle pixels of 20972 expected at ±1) [caraxes/runs/openrender-2026-09-29/vk-offscreen/].

### The vm10 runs (host execution, slot 5, 1 GiB, compression slice)

| Run | Outcome | Cause | Fix |
|---|---|---|---|
| vm10-boot1 | nouveau never loads | the 615 image's NVIDIA packaging has `alias nouveau off` in modprobe.d | the check loads it with `modprobe -C /dev/null` (70411c7e) |
| vm10-boot2 | probe fails, `fwsec-sb(v2): -2` | `nvidia/tu106/acr/bl.bin` missing: nouveau loads a v2 FWSEC descriptor through the generic falcon bootloader (N/nvkm/subdev/gsp/fwsec.c `nvkm_gsp_fwsec_v2`) | vm10 gets it (8abe58c9) |
| vm10-boot3 | FWSEC-FRTS and Booter-Load run, then `GSP_INIT_DONE` times out (-110) | nouveau queues `GSP_SET_SYSTEM_INFO`/`SET_REGISTRY` and rings before the mailboxes carry the libos arguments, then only polls (r535/gsp.c:1782-1795); the device linked message queue v0 only at a doorbell | the queues link at Booter-Load when commands are queued, as GSP-RM does at its init (6715c4e3) |
| vm10-boot4 | R1, R2: probe, kernel channels ("MM: using COPY for buffer copies"), `card0`, `renderD128`; NVK absent | nouveau allocates a user channel's objects with hObject 0; the device refused the second copy object as a live handle (0x1f) | RM's handle assignment for hObject 0 (f7e9f4c7) |
| vm10-boot5 | R1-R5 and R7 pass: NVK lists "NVIDIA GeForce RTX 2070 (NVK TU106)"; vk-offscreen's clear and copy 0 of 65536 pixels wrong, its triangle 20808 pixels (`vk-offscreen.png`); GL 4.6 core as zink on NVK through EGL surfaceless | - | - |

| vm10-boot6 | R8: weston 13 headless with the GL renderer (zink on NVK) from the local repository; vkcube-wayland 600 frames in 43.4 s (start-up; about 40 FPS by the doorbell rate once running); glmark2-wayland score 592, vkmark (wayland) 574 | trapped BAR1 accesses (1.82 M writes, 0.79 M reads), BAR2 writes (5.04 M), doorbells and MSI re-arms: the fast paths were off | - |
| vm10-boot7 | the same with the 615 guest's fast paths as arguments (BAR1 pool, doorbell tokens, interrupt page, MSI re-arm ioeventfd): glmark2-wayland score 3781 (build 3505, texture 3797, shading 4046 FPS), vkmark 5575 (cube 5423, clear 5727 FPS); `vkCreateDevice` still 18 s | BAR2 writes, 5.04 M, still trapped (no BAR2 memfd) | open: the BAR2 memfd's sync points |

- In vm10-boot5 NVK's GR channel (chid 5, bare) ran on a host GR channel in a host TSG of its own on the shadow VAS of its root, with the host objects TURING_DMA_COPY_A (twice), TURING_A and TURING_COMPUTE_A; the kernel's fence channel (chid 3, no compute class) stayed on the software executor ("bare channel (no TSG)") [vm10-boot5 device.txt].
- Times in vm10-boot5: `vkCreateDevice` 18.1 s (nouveau zeroes the channel's GR context buffers through trapped BAR2 writes, see below), step A 4.12 ms, step B 1.79 ms, pipeline creation 4 ms; no device ERROR line, no host Xid [vm10-boot5 serial.log, gpu.log].
- R6 did not run: the image's vkmark (2017.08+git20220909) has no headless plugin and its vkcube (vulkan-tools 1.3.275) no headless WSI; the image has neither weston nor mutter [vm10-boot5 serial.log "packages"].

- Accel init took about 7 s; about 5 s of it are trapped 8-byte BAR2 writes (26 us each) with which nouveau zeroes the GR context buffers of its golden and scrubber channels: 952 KiB, 2 x 512 KiB and smaller pieces, 288k BAR2 writes in the boot [vm10-boot4 trace].
- The BAR2 memfd would remove those exits, but its sync points are the 615 kernel's (the flush page at BAR2 0xfea000); nouveau flushes by reading FB page 0 through its own BAR2 mapping, 202 reads in vm10-boot4, served from the device's private store and counted as outside the reservation [vm10-boot4 gpu.log; N/nvkm/subdev/gsp/rm/r535/bar.c:33-45, 100-114].

### (c) Simpler options

- A raw nouveau channel from userspace (`CHANNEL_ALLOC`, `EXEC`, a 3D-class clear) exercises the same device path as NVK's first submit and adds nothing NVK's clear step does not show; it stays a debugging fallback.
- OpenGL through nouveau is Zink on NVK for Turing in Mesa 25.x, so it is not a shorter path.
- The 615 open kernel module with a hand-written RM client that clears through the 3D class renders today, but the userspace would be ours and the kernel is the production path already; it does not answer "an open driver renders".

## Plan

| Step | Work | Size | State |
|---|---|---|---|
| D1-D3 | 570 control frame, alloc translation, snapshots | ~400 lines incl. generated | done (67fb1128) |
| G3, G5, G7 | Fermi mask queries, ZCULL, WAR id, NVDEC1/2 vectors, OS error log | ~250 lines | done (86d0e71f) |
| K | kernel with nouveau, image vm10, check service, vk-offscreen | scripts | done (this commit) |
| R1-R3 | nouveau probe with accel, render node, NVK device | run | pass (vm10-boot5) |
| B | bare (TSG-less) GR channels: those with `TURING_COMPUTE_A` (NVK's) already take the GR shadow path with a host TSG of their own (`mirror.rs` `is_gr_shadow`, `tsg_key_of`); fixes only if R4/R5 show a gap | 0 to 350 lines | to verify in the run |
| R4-R5 | clear+copy and triangle read back through NVK | run | pass (vm10-boot5); B needed no change |
| R6, R7 | vkmark headless where the image's build has the plugin (vkcube 1.3.275 has no headless WSI), GL through EGL surfaceless | check script | in the image from the next prepare |
| R8 | vkcube on a compositor (the image has neither weston nor mutter) | image work | later |

- Evening goal: R1-R3 (nouveau binds with accel, `renderD128`, `vulkaninfo` lists NVK on the device); R4/R5 if the bare GR channels mirror without change.
- The kernel's own GR and CE channels (fences, TTM moves) run on the device's software executor; NVK's GR channels carry `TURING_COMPUTE_A` and take the GR shadow path [crates/vfio-user-nvgpu/src/mirror.rs `is_gr_shadow` 1762-1776, `shadow_channel` 2297-2382, `tsg_key_of` 928-941, `eligible` 3379-3438; ctrl.rs 775-781].

## Consequence for Caraxes

- An open driver stack renders on the device: nouveau (GSP-RM 570.144, the firmware nova uses) with Mesa NVK draws a triangle on the physical GPU through a host mirror channel, and Mesa's GL runs on it as zink (vm10-boot5). nova can follow when it gains channels, with the same device answers.
- A 570 guest is no longer "controls as 610": the control frame and the two alloc structs are translated, and every other 570 layout the device answers is checked against the snapshots, not assumed.
- No nouveau or Mesa patch is needed so far: every gap was a device answer.

## Open

- R1-R5 on the device: the run commands are in caraxes/runs/openrender-2026-09-29/README.md.
- Bare channels on the GR shadow mirror and GR objects allocated with no parameters.
- The BAR2 flush read of FB page 0 makes the outside-reservation counter non-zero for every nouveau guest (202 reads in vm10-boot4); the audit needs to name it.
- nouveau's GR context zeroing through trapped BAR2 writes (about 18 s per Vulkan or GL device creation, 5.04 M BAR2 writes in vm10-boot6/7). The BAR2 memfd's flush-read trap sits where OpenRM maps FB address 0 (BAR2 0xfea000); both drivers flush by reading FB address 0 through BAR2 (615 kern_bus_gv100.c:384-399, N/nvkm/subdev/gsp/rm/r535/bar.c:33-45, 100-114; nouveau's mapping at BAR2 0x8000, 4157 reads in vm10-boot7), or by NV_UFLUSH_FB_FLUSH 0x070000 (published/maxwell/gm107/dev_flush.h:30; 615 kern_bus_gh100.c:2946-2950; N/nvkm/subdev/bar/g84.c:29-40). Done in 6246860b (docs/caraxes/bar2-memfd.md §1, §3 rule 4): `NV_UFLUSH_FB_FLUSH` is a flush like the read; the full sync points are named after the card's registers and counted (flush read, FB flush, MMU invalidate `tu102/dev_vm.h:120-131`, GSP queue head `tu102/dev_gsp.h:38`, which carries the CPU's runlist submits as RPCs, BAR2 bind); a channel doorbell (`tu102/dev_vm.h:229`) without an owed flush syncs the live pages the rung channels could observe: VAS leaves less set B, page tables, USERD, instance block, RAMFC and method buffer (`gm107/dev_ram.h:58-73`, `tu102/dev_ram.h:27-62`). vm10-boot8 measures it with the BAR2 memfd on.
- Later, on the Cloud Hypervisor patch list: KVM dirty logging for the BAR2 slot (`KVM_MEM_LOG_DIRTY_PAGES`, the bitmap handed to the device), so a sync compares only the pages the guest wrote; upstreamable, after a look at existing CH and QEMU work on dirty logging for vfio-user regions.
- Of the 57 changes `gsp-abi.py diff gsp-610.57.04 gsp-570.144` lists as affecting the device, nouveau reaches the control frame, the two alloc structs, the OS error log and the kernel table (all translated) and `GPFIFO_SCHEDULE` (echoed); the rest (display static info, KGR static sizes, `GPU_GET_INFO_V2`, `FB_GET_INFO_V2`, user shared data, PCE maps) only an OpenRM 570 kernel would send.
- 570 control layouts beyond the ones nouveau sends (generated 570 tables `surface570`, `layouts570`, `dmodel570`; `d-controls.tsv` needs a `size_570` column).
- The 595 pin reads channel allocations with 610 offsets (368 against 376 bytes).
