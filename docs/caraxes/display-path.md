<!-- SPDX-License-Identifier: MIT -->
# The display path: what the gates need, what a display engine costs, what to do first

Written 2026-09-23 (source survey, no VM run, no host GPU access). It answers the
operator's remark about the display gates: *"these need a display engine in the device.
Should be easy, maybe there is already something existing that we could use..."*

**Short answer.**
- A display engine in the device is **not easy**. On Turing in GSP-client mode the guest
  kernel RM hands almost the whole display path to GSP. The device would have to play
  two roles: GSP's display firmware (about 40 RPCs and controls) and the display
  hardware (BAR0 register pages, a method interpreter for four channel classes, RAMHT and
  context-DMA lookup in FB, notifiers, vblank interrupts and scanout). Estimate:
  **15-25 agent-days**, high risk (section 5, route E).
- **No NVIDIA display-engine emulation exists anywhere** (route D).
- Something existing **can** be used, but it lives in the guest, not in the device.
  NVIDIA's own driver has a virtual display, the **GRID displayless** HAL
  (`NVA083_GRID_DISPLAYLESS`). Leandro v1's `vdisplay` gate ran on exactly that. The
  stock GeForce build of the driver compiles it out. Separately, the stock driver
  already runs **headless**: the device reports no display today (fuse 0x21C04 = 1),
  and nvidia-drm falls back to a render-only device.
- **Recommendation:** run two cheap experiments (D0 and D1, section 6) before anyone
  builds a display engine.

Pointers: `vendor/open-gpu-kernel-modules` at 610.57.04 is abbreviated `OGK/`. The nouveau
tree is `linux-6.12/drivers/gpu/drm/nouveau` in the session scratchpad
(`.../scratchpad/linux/linux-6.12`). Tags: [V] means read in source, [I] means inferred,
[M] means v1 measured it.

---

## 1. What the gates check, and what each one needs from a display

`tools/caraxes-gate.sh vdisplay|display` exit 2 today ([testing.md](testing.md) §3). The
v1 originals are `../Leandro-Test/scripts/test.sh:452-731` (vdisplay) and `:734-1128`
(display).

**How v1 provided the display [V].** It was neither a real head on the physical card nor
virtio-gpu with PRIME. The guest module `virtio_nvrm` with `vdisplay=1`:
- removed the EVO display classes from `GET_CLASSLIST` and put `NVA083_GRID_DISPLAYLESS`
  in their place (`guest-module/virtio_nvrm/virtio_nvrm.c:4274-4324`);
- answered the NVA083 allocation and its six controls itself, including an EDID it
  synthesized (`nvrm_edid.c`, "LEA"/"Leandro vDisp", at the requested size;
  `virtio_nvrm.c:4041-4269`);
- served `NV9010_VBLANK_CALLBACK` with a guest `hrtimer` at the EDID's refresh rate
  (`virtio_nvrm.c:4355-4530`).

Stock nvidia-modeset and nvidia-drm (`modeset=1 vblank=1`) ran on top and believed they
had one connector. The real EVO path had been tried first: the host refused it
(`SPECIFIC_GET_ALL_HEAD_MASK` → `NV_ERR_INSUFFICIENT_PERMISSIONS`; with `CAP_SYS_ADMIN`
it failed later at `GET_PCLK_LIMIT`). The displayless route was then decided
(`docs/DISPLAY.md:57-58`, issue 25 in `docs/history/OPEN-QUESTIONS-2026-09-18.md:2566`)
[M]. The host side (`vhost-user-nvrm`) knew nothing about the display: it has no scanout
path. Every frame check reads back **inside the guest**, and the stream stage goes out
over the network (Sunshine in the guest, Moonlight on the host).

| Gate stage (v1) | What it needs | Headless stock (D0) | GRID displayless (D1) | Device display engine (E) |
|---|---|---|---|---|
| vdisplay: preconditions, tables (virtio_nvrm parameters, descriptor checksum) | v1's module | n/a, rewrite | n/a, rewrite | n/a, rewrite |
| vdisplay: device (1 connector, connected, offers WxH) | nvidia-drm connector | **no** | yes, but modes capped by max resolution | yes |
| vdisplay: edid (`edid-verify`, `edid-decode --check`) | an EDID on the connector | no | NVIDIA's fixed "NVIDIA VGX" EDID, not settable | yes, synthesized (reuse `nvrm_edid.c`) |
| vdisplay: pixel (dumb buffer, SETCRTC, GETFB2 readback, committed hash) | KMS with one CRTC | no | yes [I] | yes |
| display: card, connector, edid (byte comparison with `edidcheck`) | as above | no | partly (EDID differs from v1's) | yes |
| display: modeset (30 flips, all completed) | flip completion | no | open: vblank callback missing (§4) | yes |
| display: capture (Xorg, paintprobe, XGetImage/XShm/x11grab) | an X screen on the NVIDIA GPU | open: headless X (§6 D0) | yes [I] | yes |
| display: swapchain, present (120 frames at ~60 FPS), rt, events | Vulkan WSI on that X screen | open (D0) | yes [I], pacing open | yes |
| display: stream (Sunshine NVENC → Moonlight, non-black pixels) | an X screen and NVENC | open (D0); NVENC is compute-path work | same | same |
| display: kernel (dmesg clean) | — | yes | yes | yes |

None of the gates needs the **device** to scan out. A device-side scanout matters only for
things outside the gates: a human watching the VM without Moonlight, a boot console, or a
Windows guest later.

## 2. What the guest stack demands of the device for a real display engine

Turing is display IP **v04.00** (`INTERNAL_DISPLAY_GET_IP_VERSION` = `0x04000000`).
Its classes are **C57x, not C37x** (C37x is Volta). The TU106 class list
(`OGK/src/nvidia/generated/g_gpu_class_list.c:338-359`) has:
- `NV04_DISPLAY_COMMON`, `NVC570_DISPLAY`, `NVC57D_CORE_CHANNEL_DMA`,
  `NVC57E_WINDOW_CHANNEL_DMA`, `NVC57B_WINDOW_IMM_CHANNEL_DMA`,
  `NVC57A_CURSOR_IMM_CHANNEL_PIO`, `NVC573_DISP_CAPABILITIES`, `NVC372_DISPLAY_SW`,
  `NVC371_DISP_SF_USER`;
- `NVA083_GRID_DISPLAYLESS`, which is deleted later unless the display is absent and
  the build is GRID (§4).

NVKMS drives these classes through `nvEvoC5` (`OGK/src/nvidia-modeset/src/nvkms-hal.c:157`,
`nvkms-evo3.c`, C37x method names, 7986 lines). The headers are `clc57d.h` (1207 defines)
and `clc57e.h` (617) under `OGK/src/common/sdk/nvidia/inc/class/` [V].

### 2.1 How the kernel decides that a display exists [V]

- **Fuse.** `kdispStatePreInitLocked` reads BAR0 `NV_FUSE_STATUS_OPT_DISPLAY` 0x21C04.
  Bit 0 = 1 means no display. `gpuStatePreInit` then deletes `ENG_KERNEL_DISPLAY` and
  all its classes before any display RPC (`kern_disp.c:435-466`,
  `kern_gpu_gm107.c:62-68`, `gpu.c:2231-2271`).
  - **The device answers 1 today** (`crates/vfio-user-nvgpu/chips/tu106/boot.toml:48-50`).
  - The same fuse changes the FB layout: the VGA workspace and the 64 KiB display instance
    memory move the reserved block from 0x20000 to 0x30000
    ([fb-model.md](fb-model.md) §3.2, `kernel_gsp_tu102.c:787`).
- **IP version.** After the fuse, the RPC `INTERNAL_DISPLAY_GET_IP_VERSION` 0x20800a4b.
  `NOT_SUPPORTED` also removes the display. Any other error aborts GPU init
  (`gpu.c:2272-2275`).
- **No other switch.** `GspStaticConfigInfo` has no display field, and no NVreg/Rm
  registry key disables the display.

### 2.2 RPCs and controls the device must answer (GSP's display firmware role) [V]

- **PreInit, Init and Load** (inventory in [phase5-rpcs.md](phase5-rpcs.md) §1.7):
  - `GET_IP_VERSION`;
  - `NV04_DISPLAY_COMMON` alloc;
  - `INTERNAL_DISPLAY_GET_STATIC_INFO` 0x20800a01 (`numHeads`, `windowPresentMask`,
    `feHwSysCap`, `numDispChannels` ≥ 81, `i2cPort` = 16; `kern_disp.c:570-628`);
  - `INTERNAL_INIT_BRIGHTC_STATE_LOAD` 0x20800ac6;
  - `INTERNAL_SET_STATIC_EDID_DATA` 0x20800adf;
  - `INTERNAL_DISPLAY_WRITE_INST_MEM` 0x20800a49 (base and size of the 64 KiB instance
    memory in FB);
  - `INTERNAL_DISPLAY_SET_IMP_INIT_INFO` 0x20800a54;
  - `NV0073_CTRL_CMD_SYSTEM_MAP_SHARED_DATA` 0x730151.
- **Unix console:** `INTERNAL_DISPLAY_PRE/POST_UNIX_CONSOLE`
  (`arch/nvalloc/unix/src/unix_console.c:105-209`).
- **RG line:** `INTERNAL_DISPLAY_SETUP_RG_LINE_INTR` (`rg_line_callback.c:74`).
- **Per channel:** `INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER` 0x20800a58, carrying hclass,
  instance, pushbuffer physical address, aperture, limit and valid
  (`disp_channel.c:793-871`), followed by the `GSP_RM_ALLOC` of the channel class.
  C570, C57D, C57E, C57B, C57A and C372 are all `RS_FLAGS_ALLOC_RPC_TO_ALL`
  (`resource_list.h:1104-1314`).
- **NVKMS device bring-up** (`nvRmAllocDisplays`, `nvkms-rm.c:1787-2026`;
  `nvAllocCoreChannelEvo`, `nvkms-evo.c:5177-5330`), all routed to GSP
  (`g_disp_objs_nvoc.c`, flag 0x40):
  - NV0073 `SYSTEM_GET_CAPS_V2`, `GET_NUM_HEADS`, `SPECIFIC_GET_ALL_HEAD_MASK`,
    `GET_VALID_HEAD_WINDOW_ASSIGNMENT`, `SYSTEM_GET_SUPPORTED`, `GET_CONNECT_STATE`,
    `GET_CONNECTOR_TABLE`, `SPECIFIC_GET_CONNECTOR_DATA`, `DFP_GET_INFO`,
    `SPECIFIC_OR_GET_INFO` (PRIVILEGED), `SPECIFIC_GET_EDID_V2`, `GET_BOOT_DISPLAYS`,
    `GET_ACTIVE`, `GET_PCLK_LIMIT`, `DFP_ASSIGN_SOR`;
  - NV5070 `SYSTEM_GET_CAPS_V2`;
  - NVC370 `GET_CHANNEL_INFO` and `IDLE_CHANNEL`;
  - NVC372 `IS_MODE_POSSIBLE` (IMP, which must say "possible").
- **A DP connector adds much more.** It brings the `DP_*` link-training controls
  (`DP_CTRL`, `DP_SET_LANE_DATA`, `DP_CONFIG_STREAM`). A virtual **DVI-D or HDMI TMDS
  connector avoids link training**, so the first connector should be one of those, not
  DP.
- **Hotplug** arrives as the `POST_EVENT` RPC with `NV2080_NOTIFIERS_HOTPLUG` on the
  handles NVKMS registered (`kernel_gsp.c:505-570`).

### 2.3 BAR0 the guest touches directly (the display hardware role) [V]

| Range | Use | Source |
|---|---|---|
| 0x021C04 | display fuse | `kern_gpu_gm107.c:62` |
| 0x625F04 | `NV_PDISP_VGA_WORKSPACE_BASE` | `kern_disp_0400.c:53` |
| 0x640000 (`NV_PDISP_FE_SW`, `sizeof(NvC373DispCapabilities_Map)`) | caps page mapped for NVC573. NVKMS parses SYS_CAP/CAPB, head, SOR and window caps from it; `numWindows` must be > 0 | `kern_disp_0300.c:142-160`, `nvkms-evo3.c:5095-5320`, `nvkms-evo.c:5275` |
| 0x680000 (64 KiB), 0x690000 + 0x1000·i, 0x690000 + (32+i)·0x1000, 0x6D8000 + 0x1000·i | USERD of core, window, window-imm and cursor: PUT at +0, GET at +4. **The device must move GET to PUT** | `kern_disp_0300.c:206-251`, `nvkms-dma.c:48-69`, `clc57d.h:76-78`; nouveau `r535.c:53-66` |
| 0x6104E0-0x610784 | `FE_CHNCTL_*` / `FE_CHNSTATUS_*` (idle, connect, abort) | `kern_disp_channel_0300.c` |
| 0x611800 + 4h, 0x611C00 + 4h, 0x611EC0, 0x611C30, 0x611858/5C | head-timing event, status and dispatch, write-1-to-clear (vblank, RG line, awaken) | `kern_disp_0300.c:434-631`; nouveau `r535.c:1383-1408` |
| 0x616330 + 0x800h, 0x61611C + 0x800h | RG scanline/frame counter, LOADV counter | `kernel_head_0400.c:33-78` |
| 0x6F0000 | `NV_PDISP_SF_USER_0` (NVC371), when allocated | `kern_disp_0300.c` |
| DISP leaf in the 0xB8xxxx tree | the kernel RM owns the display interrupt on dGPU Turing. The vector comes from the `INTR_GET_KERNEL_TABLE` answer (the device's `interrupts.toml` has no DISP entry yet) | `intr_tu102.c:277-291,1191-1212`, `kern_disp.c:1895-1945` |

### 2.4 Guest memory the device must read and write [V]

- **Pushbuffers** in guest sysmem, one per channel, allocated by NVKMS
  (`RmAllocEvoChannel`, `nvkms-rm.c:2772-2893`). They hold standard NV method headers.
  The address arrives in the `CHANNEL_PUSHBUFFER` RPC.
- **Display instance memory: RAMHT at 0x0-0x1FFF, context-DMA objects at
  0x2000-0xFFFF.** The **guest kernel** writes it into FB with no RPC when NVKMS binds
  a ctxdma (`NV0002_CTRL_CMD_BIND_CONTEXTDMA` → `instmemBindContextDma`,
  `disp_inst_mem.c:602-855`; object layout `disp_inst_mem_0300.c:144-275`; hash
  `:94-124`). Every handle in a method (`SET_CONTEXT_DMA_NOTIFIER`, the window's ISO
  surface, semaphores) has to be resolved by walking these structures in the device's
  FB model.
- **Notifiers.** These are 16-byte records. The CPU writes NOT_BEGUN, the device must
  write **FINISHED** (bits 31:30 = 2) after an `UPDATE`, and NVKMS spin-polls for it
  (`nvkms-evo3.c:2638-2731,6260-6288`, `clc57d.h:35-48`). Window semaphores
  acquire/release the same way.
- **Scanout surfaces** in FB, often **block-linear** (Turing GOBs). A sink must de-tile
  them. From E5b on, the FB is a real VRAM reservation: every frame then has to be read
  from host VRAM through a host RM mapping. That is slow for CPU reads, and it is an
  open question whether it is allowed under the host-GPU rules. With the host-RAM fake
  FB the read is free.

### 2.5 What GSP hides, which the device must then invent

- The **supervisor** interrupt sequence is handled entirely inside GSP. The kernel has no
  supervisor handler, and the events `RG_LINE_INTR` 0x1007 and `DISPLAY_MODESET` 0x1011
  are handled only by the Tegra DCE client (`dce_client_rpc.c:359-376`) [V].
- For Caraxes this cuts both ways:
  - the device need not reproduce the three-stage supervisor handshake;
  - but no guest-visible source says what state the device must present after a
    modeset (armed state, CHNSTATUS values, IMP answers). Only NVKMS's reads, found by
    trial and error, will tell.
- nouveau's GSP client, `nvkm/engine/disp/r535.c` (1725 lines), is the best second
  reference for this boundary. It covers channel USERD offsets, the pushbuffer control,
  RAMHT insertion by the client, head-timing interrupt servicing and the `oneinit` RPC
  order. It confirms the list above from an independent implementation [V].

## 3. What the device reports today

- Fuse 0x21C04 = 1 (`boot.toml:48-50`).
- `NV_PDISP_VGA_WORKSPACE_BASE` is named in `regmap.toml:38`, and the FB layout is
  computed "with no display" (`chip/turing.rs:152`).
- `ctrl.rs`, `rpc.rs` and `nvml.rs` have no display handlers. An unknown control is
  answered `NV_ERR_NOT_SUPPORTED` with the label "unanswered" (`ctrl.rs:470-477`), and
  no display control reaches the device because KernelDisplay is removed first.
- The guest has no nvidia-modeset, no nvidia-drm and no `/dev/dri` node
  (`caraxes/runs/gate-display-2026-09-23/verdict.json`).

## 4. The GRID displayless HAL: the existing piece, and its catches

This is what "maybe there is already something existing" finds. It is compiled into
every open-kernel-module build [V]:

- **RM side (kernel-local, no RPC).**
  - `OBJGRIDDISPLAYLESS` is a child engine on TU106 (`kern_gpu_tu102.c:141`).
  - Class `NVA083_GRID_DISPLAYLESS` is non-privileged and **not** RPC-to-all
    (`resource_list.h:303-311`).
  - Its six controls are local: `GET_NUM_HEADS`, `GET_MAX_RESOLUTION`,
    `GET_DEFAULT_EDID`, `IS_ACTIVE`, `IS_CONNECTED`, `GET_MAX_PIXELS`
    (`src/kernel/griddisplayless/*.c`, flag 0x8 only).
  - Defaults when not `IS_VIRTUAL`: 1 head, 2560x1600, 4096000 pixels. The EDID is a
    fixed table, "NVIDIA VGX", DVI-D or VGA (`objgriddisplayless.c:33-120,330-370`).
  - GSP may override heads, resolution and pixels with the event
    `NV_VGPU_MSG_EVENT_UPDATE_GRID_DISPLAYLESS_PARAMS`, which also raises a hotplug to
    NVKMS (`kernel_gsp.c:1571,1753-1771`). **So the device can steer the head count
    and maximum resolution with one event.**
- **NVKMS side.**
  - `nvRmAllocDisplays` takes `NV04_DISPLAY_COMMON`, else NVA083 (sets
    `displaylessHw`), else NO_HARDWARE (`nvkms-rm.c:1819-1860`).
  - The HAL `nvDisplayless` (`nvkms-displayless.c`, 885 lines) queues flips and
    completes them in software: a timer worker polls acquire semaphores and writes
    release semaphores. There is no scanout, and no method stream reaches any device.
- **Catch 1: the stock build deletes the class.**
  - `gpuStatePreInit` keeps NVA083 only if KernelDisplay is absent **and**
    `osIsGridSupported()` (`gpu.c:1961-1985,2279-2287`).
  - That is `os_is_grid_supported() || os_get_grid_csp_support() ||
    hypervisorIsVgxHyper()` (`arch/nvalloc/unix/src/os-hypervisor.c:918-921`). All three
    are **compile-time** (`NV_GRID_BUILD`, `NV_GRID_BUILD_CSP`, `NV_VGX_HYPER`;
    `kernel-open/nvidia/os-interface.c:1419-1461`, set from the `GRID_BUILD`
    environment in `kernel-open/conftest.sh:725-740`).
  - The device cannot turn it on. A guest `nvidia.ko` built with `GRID_BUILD=1` can.
  - That goes against the brief's "stock (non-GRID) driver" and needs the operator's
    word as a temporary aid (like D13).
  - Other effects of `NV_GRID_BUILD` [V]:
    - `NV0080_CTRL_CMD_GPU_GET_VIRTUALIZATION_MODE` reports `isGridBuild = 1` to
      userspace (`device_ctrl.c:210-212`);
    - `GspSystemInfo.isGridBuild` reaches the device (`kernel_gsp.c:4603`);
    - `rm_is_vgpu_supported_device` changes only for SR-IOV/PV guests
      (`os-hypervisor.c:971-984`), which this device is not.
  - [I] What the closed userspace does with `isGridBuild = 1` (licensing behaviour) is
    unknown and must be measured.
- **Catch 2: no vblank callback.**
  - `NV9010_VBLANK_CALLBACK` fails with NOT_SUPPORTED when KernelDisplay is absent
    (`vblank_callback.c:62-70`). NVKMS only logs "Failed to enable VBlank callback"
    (`nvkms-rm.c:5141-5172`).
  - v1 served this class itself with an hrtimer because its transport could intercept
    the allocation. In Caraxes the allocation stays inside the guest's `nvidia.ko`, and
    only a guest patch could serve it.
  - Whether flips, DRM vblank events and 60 FPS pacing work without it is **open**
    (D1 measures it).
- **Catch 3: the EDID is fixed.** It cannot be set to v1's "LEA" EDID or to an
  arbitrary WxH. Only the maximum resolution is steerable (event above). The vdisplay
  gate's EDID and exact-size criteria have to become "NVIDIA's displayless EDID,
  conformant, offers the configured mode".
- **Version note:** v1 recorded that NVA083 is absent before R595 (docs/OPEN-QUESTIONS.md
  issue 74). That is irrelevant for 610.57.04.

## 5. Candidate routes

| Route | What it is | Gates covered | Effort (agent-days) | Main risks |
|---|---|---|---|---|
| **A. NVKMS as the specification** | Use `nvkms-evo3.c`/`clc57*.h` to learn the method streams for route E | nothing by itself; it is the reference for E | included in E | NVKMS shows what it sends, not what hardware or GSP answers. The supervisor and IMP state are inside GSP |
| **B. nouveau dispnv50 + `r535.c`** | A second, much smaller reference: `corec37d.c` (179 lines), `wndwc37e.c`, `headc37d.c`, and the GSP-client boundary in `r535.c` | nothing by itself | included in E | nouveau issues a smaller method set than NVKMS. The stock guest runs NVKMS, so NVKMS's stream is the one that must be served |
| **C1. Headless stock (D0)** | Keep fuse = 1. Guest: nvidia-modeset + nvidia-drm `modeset=1` (KAPI falls back to "displayless mode", `nvkms-kapi.c:494-503`; nvidia-drm registers with zero connectors plus renderD [I]); Xorg on the NVIDIA GPU with no display device (`UseDisplayDevice "None"`, virtual screen), the standard headless/VirtualGL setup on display-less GPUs [I] | display: capture, swapchain, present, rt, events, stream, kernel. **Not** vdisplay, and not the card/connector/edid/modeset stages | 0.5-1 for the experiment. Gate split into "desktop" and "connector" parts: 1 | Headless X on a GPU that NVKMS reports as NO_HARDWARE is untested here [I]. The stream stage needs NVENC and all present stages need real rendering, so they wait on the compute path (E5b+) whatever the display route |
| **C2. GRID displayless (D1)** | C1 plus a guest `nvidia.ko` built with `GRID_BUILD=1`. Optionally the device sends `UPDATE_GRID_DISPLAYLESS_PARAMS` to set heads and maximum resolution | vdisplay device/pixel, display card/connector/modeset(?) plus everything in C1; EDID stages with adapted criteria | 1-2 (build switch, the one event in `rpc.rs`, gate criteria) | Breaks "stock non-GRID driver" (operator decision). Vblank callback missing (catch 2). Unknown userspace GRID behaviour. Fixed EDID |
| **C3. virtio-gpu display + PRIME render offload** | Fuse = 1 (C1), plus a second display device: guest X on virtio-gpu, NVIDIA renders through PRIME | capture and stream on a Mesa X server; **fails** every connector stage | 3-6: Cloud Hypervisor v53 has no virtio-gpu display front end. `vhost-device-gpu` exports its display only over QEMU's vhost-user-gpu socket (`vendor/vhost-device/vhost-device-gpu/README.md`). v1's interim crosvm carrier was removed on 2026-08-18 | v1 measured NVIDIA EGL on an X server owned by virtio-gpu: `eglplat xcb` resolves to Mesa [M] (`docs/history/OPEN-QUESTIONS-2026-09-18.md:4940-5040`). It adds a device the gates do not want. **Not recommended** |
| **D. Existing emulation** | None exists. QEMU has no NVIDIA display model (only vfio-pci quirks). xemu/XQEMU (NV2A PCRTC) and 86Box/PCem (Riva/NV3-4 VGA) predate EVO/NVDisplay entirely. envytools documents registers but runs against real hardware. A web search on 2026-09-23 found nothing for EVO/NVDisplay | — | 0 | — |
| **E. A display engine in the device** | One head, one virtual TMDS (DVI-D/HDMI) connector with an EDID synthesized by reusing `guest-module/virtio_nvrm/nvrm_edid.c` logic in Rust, and all of section 2 | everything in both gates, with the EDID and size under the device's control; the only route that also gives a boot console and a later Windows guest a real head | **15-25** (breakdown below) | See the list below |

### Breakdown of route E (agent-days)

| Part | Days |
|---|---|
| Fuse to 0 and the FB layout with the display (instance memory, VGA workspace; boot and `fb.rs` changes, re-verify `kbusVerifyBar2`) | 1-2 |
| GSP display firmware role: ~12 internal RPCs, the NV04_DISPLAY_COMMON/C570/C372 allocations, ~20 NV0073/NV5070/NVC370/NVC372 controls, a consistent head/window/SOR/connector model, IMP answers | 3-5 |
| Hardware role, registers: caps page 0x640000, USERD PUT/GET, CHNCTL/CHNSTATUS, head-timing interrupt registers, DISP leaf and vblank timer, scanline and LOADV counters | 2-3 |
| Method interpreter: core C57D, window C57E, window-imm C57B, cursor C57A; RAMHT and ctxdma lookup in FB; interlocked `UPDATE`; notifier FINISHED; window semaphores | 5-8 |
| Scanout sink: resolve the window surface, de-tile block-linear, formats, deliver to a sink (a memfd and PNG per frame first, VNC or vhost-user-gpu later) | 2-4 |
| Bring-up against NVKMS, nvidia-drm, X, the gate, the trace decoder for display methods | 2-3 |

**Risks for route E:**
1. The post-modeset state GSP would normally present is unspecified (section 2.5). NVKMS
   may poll channel status or IMP results the device gets wrong, and the only feedback
   is a timeout.
2. Block-linear scanout and the reservation. From E5b on, frames live in real VRAM, and
   CPU reads through a host mapping are slow. A copy-engine blit on the host card
   would be a host GPU action and needs the operator's word.
3. `SPECIFIC_OR_GET_INFO` is PRIVILEGED in the guest (`g_disp_objs_nvoc.c:4298`). This
   is the same class of refusal that stopped v1 on the host. Here the device answers it
   itself, so it should not recur [I].
4. Every new driver version moves display structures. D14 keeps this manageable (per
   version ABI), but it adds a whole subsystem to the version surface.
5. Scope: [kickoff.md](kickoff.md) §6 keeps nvidia-modeset/nvidia-drm after the compute
   path. Route E competes with E5b for the same device code (FB model, interrupts).

## 6. Recommendation and the smallest first experiment

1. **Do not start route E now.** Run D0 and D1, which need no device display code.
   Their results decide whether E is needed at all, and for which stages.
2. **D0: headless stock** (0.5-1 agent-day, no device change, no host GPU action).
   - With the current device (fuse = 1), load `nvidia-modeset` and `nvidia-drm
     modeset=1` in vm0.
   - **Expectation:**
     - nvidia-drm registers a DRM device with zero connectors and a render node;
     - dmesg shows KAPI's "falling back to displayless mode";
     - the device trace shows **no** display RPC;
     - Xorg starts on the NVIDIA GPU with `UseDisplayDevice "None"` and a virtual
       1920x1080 screen, and `paintprobe` plus XGetImage read back the colour.
   - **Falsified if:**
     - nvidia-drm fails to load (then record the RPC or control it stopped at), or
     - the NVIDIA X driver refuses a GPU without display hardware.
   - The Vulkan and stream stages are recorded, not scored, until the compute path works.
3. **D1: GRID displayless** (1-2 agent-days, **only with the operator's word** on a
   `GRID_BUILD=1` guest `nvidia.ko` as a temporary aid, same status as D13).
   - **Expectation:**
     - `GET_CLASSLIST` contains 0xA083;
     - nvidia-drm has one connected connector carrying NVIDIA's VGX EDID, with modes
       up to 2560x1600;
     - v1's `vdisp-frame` pixel check passes;
     - `drm-modeset --flips 30` either completes all flips or shows the missing
       vblank callback as the cause.
   - The optional second step sends `UPDATE_GRID_DISPLAYLESS_PARAMS` from `rpc.rs`
     (1 head, 1920x1080) and expects NVKMS to take it as a hotplug.
4. **Only if** the operator needs a connector on the stock non-GRID driver, a
   device-side scanout for a human viewer or a console, or a Windows guest with a real
   head, build route E.
   - Order: the RPC and control model first, with the fuse at 0, until
     `nvAllocCoreChannelEvo` completes. That alone is falsifiable, because NVKMS logs
     every probe failure by name.
   - Then GET/PUT and notifiers until a modeset completes, then scanout to a PNG sink.
   - Use NVKMS (A) and nouveau `r535.c` plus dispnv50 (B) as the two references.

**Operator decisions this needs:**
- whether D1's `GRID_BUILD=1` guest module is acceptable as a temporary aid;
- whether the gates may be split, with stages scored per route, instead of v1's
  all-or-nothing.

Sources for route D: web search 2026-09-23 (NVIDIA open-gpu-doc
[Display-Class-Methods](https://download.nvidia.com/open-gpu-doc/Display-Class-Methods/2/README.txt)
describes the EVO classes; the [NVIDIA QEMU fork](https://github.com/NVIDIA/QEMU) is about
passthrough, not display emulation).
