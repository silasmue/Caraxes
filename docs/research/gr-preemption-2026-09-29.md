<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GR preemption and the SotTR Xid 109 (2026-09-29)

Scope: the host Xid 109 CTX SWITCH TIMEOUT of the SotTR 720p run on Linux B (`caraxes/runs/night-2026-09-28/sottr720-8e621e0a/`); the preemption modes the host context of a Linux graphics channel has on dev; the candidate causes; the fix (compute parity, owned GFXP in the VM's slice) and its tests. `L` = `/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b/gpu.log` (this run, trace off); `W` = `/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/`; `R` = the run folder.

## Facts

### The event

- Host: `Xid 109, pid=2002548 vfio-user-nvgpu, channel 0x00000097, CTX SWITCH TIMEOUT, Info 0x4c078` at 04:32:22 [`sudo -n dmesg`, read-only].
- Info 0x4c078 decodes as `NV_PFIFO_INTR_CTXSW_TIMEOUT_INFO`: PREV_TSGID 0x78 (120), CTXSW_STATE 3 (SWITCH), NEXT_TSGID 4, STATUS 0 (AWAITING_ACK) [NVIDIA open-gpu-doc `manuals/turing/tu104/dev_fifo.ref.txt`: fields 13:0, 15:14, 29:16, 31:30].
  - GR was asked to switch from the game's TSG to host TSG 4, and FECS had not acknowledged when the timeout fired.
  - TSG 4 is a low, early id on the GR runlist: a long-lived host client, probably the host desktop. Not identified.
- The timeout is the RM default, 0x3fffff ticks of 1.024 us = 4.29 s [615 tree: src/nvidia/interface/nvrm_registry.h:1580-1604]; the host sets no override [/etc/modprobe.d, /proc/cmdline]. The stall began at about 04:32:18.0.
- The only non-idle guest GPU sample (88 %, guest 02:32:19) lies inside that 4.3 s window. The samples before were 4 and 6 % (loading), the next 1 % after the RC [R/timeline.txt:18-21].
  - So "kept rendering after the NVKMS refusal" (guest 216.9 s = 04:31:20) is not shown; the stream stayed frozen from then on [R/timeline.txt RMSE].
- The RC hit the game's GR TSG only: guest chids 65 and 66, engine 1 [R/guest-monitor.txt:172-173]. One host RC event, exceptType 0x6d = 109 [L:166, L:388]. Chid 65 had 154 GPFIFO entries not fetched (GPGet 18234, GPPut 18388) [L:167].
- The device issued no preempt then: the pool's 8 TSG preempts were at start-up, every barrier after the RC was refused [L:101-113, L:167 ff., L:392].
- No Xid 31 and no IOMMU fault on the host. The shadow VAS has no ENABLE_FAULTING, so a GPU fault RCs (Xid 31) instead of stalling GR [shadow.rs:1086-1101].
- Earlier Xid 109s were CUDA contexts: kill -9 with a kernel running (one host preempt of 3.2 s), and vm2 in series F (Weston stalled 4.3 s) [docs/caraxes/coverage-map.md:43-44, 55-56; docs/caraxes/intr-page.md:895-896].

### What a native Linux client asks for

- `GR_SET_CTXSW_PREEMPTION_MODE` 0x20801210 {flags, hChannel, gfxpPreemptMode, cilpPreemptMode}: flags bit 0 CILP_SET, bit 1 GFXP_SET; graphics WFI 0, GFXP 1, GFXP_POOL 2; compute WFI 0, CTA 1, CILP 2 [615 tree: src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080gr.h:829-857].
  - `GR_CTXSW_PREEMPTION_BIND` 0x20801211 adds `vMemPtrs[9]` [ctrl2080gr.h:917-929].
  - Both are NON_PRIVILEGED and ROUTE_TO_PHYSICAL (flags 0x10348, 0x10248) [615 tree: src/nvidia/generated/g_subdevice_nvoc.c:9516-9539; src/nvidia/inc/kernel/rmapi/control.h:205, 230].
- The bare-metal traces (610.57.04, RTX 2070) hold 28 SETs from Vulkan, GL, EGL and GLES. All are flags 1, cilp 1 (compute CTA); none sets GFXP [matrix/traces/610.57.04/{vk,gl,egl,gles}*.jsonl].
  - CUDA, OpenCL, NVENC, NVDEC: flags 1, cilp 2 (CILP).
  - None of the 22 traces has a BIND 0x20801211.
  - Graphics clients set no TSG timeslice or interleave level; only the CUDA-class clients set 2048 us (0xa06c0103 in 10 traces, 0xa06c0107/0xa06f0109 in none).
- The 615 guest's driver sends 0x20801210 too (56 in the 2026-09-27 trace, payloads not logged) and no 0x20801211 [/mnt/vmstore/caraxes/alpha-2026-09-27/desktop-b/gpu-trace.log].
- NVIDIA's vGPU guest RM on Unix allocates five GFXP buffers for every 3D object on Pascal and later, maps them in the channel's VAS and BINDs GFXP; it BINDs WFI before freeing them [615 tree: src/nvidia/src/kernel/vgpu/rpc.c:3603-3812, 3951-3956, 3051-3110].

### What the device does on dev, end to end

1. The guest's SET 0x20801210 gets NV_OK and is dropped ("synthesized (NV_OK echo, nothing acted on)") [cuda.rs:48-51, 76-83].
2. A guest BIND 0x20801211 would be recorded on the guest channel only ("never forwarded: the mirror channel keeps the host's defaults") [public.rs:463-486; ver/v610/public.rs:256].
3. At the first doorbell the mirror builds the host TSG, a channel with flags bit 7 only (no SKIP_CTXBUFFER_ALLOC) and the guest's engine objects with the guest's parameters, on its unprivileged host client [mirror.rs:159, 2383-2388, 2511, 2549-2551]. The host kernel RM allocates the context; GSP-RM chooses its modes.
4. Nothing changes the modes afterwards. `bind_owned_gfxp`, `bind_owned_compute` and `confirm_gfx_wfi` have no caller on dev [mirror.rs:2845, 2986, 3124; `git grep`: only mirror.rs:2975]. Only the Windows diagnostic patch calls them [W/P36-build/diagnostic.patch:54-86].
5. GET_CTXSW_MODES of a mirrored channel is forwarded to its host channel [public.rs:606-624]; the Linux driver does not ask. `deferred_context_modes` runs only with `CARAXES_SHADOW_CHANNEL_DIAG=1` and the deferred context, never on Linux [ctrl.rs:1618-1654; mirror.rs:2815-2840].
6. TSG SET_TIMESLICE and PREEMPT go to the host TSG once it exists [ctrl.rs:232-238, 1826-1837]; FIFO_CONFIG_CTXSW_TIMEOUT is refused [docs/caraxes/gsp-abi-615-tables.md:432].

### The host context's modes

- A host GR TSG built this way (P13b: 902D, C597, C5C0, before any submission) read graphics WFI: GET 0x20801235 gave (NV_OK, gfxp 0) [W/P13b-empty-host-mode/gpu.log:106; windows-deferred-context-2026-09-28.md, P13]. Its compute mode was not read.
- The default TSG timeslice is 128 << 3 = 1024 us, for native and mirror alike [615 tree: src/nvidia/src/kernel/gpu/fifo/arch/volta/kernel_fifo_gv100.c:46-51; kernel_channel_group.c:186].
- The 100 Windows refusals "graphics WFI not confirmed: gfxp 0x1, cilp 0x1" (P23-P38) are all WFI BINDs from clients that had bound owned GFXP before: teardown unbinds, not a GFXP default [W/P23-P38 gpu.log: 18 runs, 0 exceptions]. P22's "9 other D3D contexts" are probably the same unbinds.
- **SotTR's host context on dev: graphics WFI** (the measured default, and no code path changes it), **compute at the host default instead of the CTA the driver asked for.**

## Verdicts

| # | Candidate | Evidence | Verdict | Confidence |
|---|---|---|---|---|
| 1 | Host GR context without GFXP, WFI | Host default WFI (P13b); dev never binds. But native Linux graphics never asks for GFXP either (28 of 28 SETs compute only) | Graphics WFI on the host: yes. "Unlike native": probably not. The proven gap is compute: CTA (CUDA: CILP) asked, not applied. WFI granularity is the mechanism of every Xid 109; whether a draw or a dispatch held GR is not in this run's evidence | host WFI 0.85; native also WFI 0.6; compute gap 0.95 |
| 2 | Timeslice or timeout configuration | Timeslice default 1024 us (graphics sets none), timeout default 4.29 s, no host override, the guest's timeout control refused | Same as native; excluded. What differs is load: host Moonlight (Vulkan) and the host desktop plus the guest's compositor contexts are separate host TSGs on the GR runlist, so the game's TSG is switched out more often | 0.85 |
| 3 | Semaphore or fence wait tied to the NVKMS buffer | Present and queue waits are host (PBDMA) acquires, which leave GR idle and switchable; the refusal came 62 s earlier; the sample in between was 6 %; the 88 % sample is the stall itself | Unlikely as the direct cause. Indirect link possible: the full VRAM behind the refusal may have pushed resources to sysmem and slowed work | 0.65 |
| 4 | A device preempt (pool barrier, forwarded TSG_PREEMPT) | No barrier from 04:29 to the RC; the Info names a switch to TSG 4 | Excluded | 0.8 |
| 5 | GR stalled on a fault | Non-faulting shadow VAS; no Xid 31, no IOMMU fault | Excluded | 0.85 |
| 6 | Runaway work from stale shadow mappings | 95 doorbells found the host channel ahead with plans pending; only the first 16 are logged, the last at 04:29:20 [L:388; untrapped.rs:46] | Open | - |

## Consequence for Caraxes: the fix

Step 0, read-only diagnostic (about 60 lines, no behaviour change):
- Per host GR TSG, once after the build (before its first GPFIFO_SCHEDULE) and once at teardown: GET_CTXSW_MODES on its first host channel; log `tsg K: host modes gfxp G cilp C zcull Z`.
- The cuda.rs acknowledgement decodes the guest's SET (flags, hChannel, gfxp, cilp) instead of "ok".
- Optional (about 80 lines): at a host RC of a GR channel, log the class and method of the push at host GPGet-1 (draw or dispatch, indirect or not) and the last ran-ahead time of its root.

Step 1, compute parity (about 150-200 lines):
- Record the guest's SET per guest TSG (CUDA names the TSG [docs/caraxes/cuda-lookahead.md:250]).
- Apply it at the host TSG's build, before the first schedule: the pointer-free BIND {CILP_SET, cilp}, checked by GET (`bind_owned_compute` without the deferred-context precondition; multi-channel TSGs as in e705186b). A SET to a built TSG idles it first, as `bind_owned_compute` does.
- Return the host status; no CILP-to-CTA downgrade (P22 decision). GFXP bits of a SET go to step 2, never pointer-free (P14: SET1 is accepted without buffers; where GSP-RM keeps that state is unknown).

Step 2, owned GFXP in the VM's slice, both guests (about 500-700 lines):
- Carve-out: K slots at the top of the usable FB beside the BAR1 pool, reported reserved to the guest like the pool.
- One slot = the five buffers (ids 0x0b-0x0f: 0x10f300, 0xe1000, 0x20000, 0x3e3a80, 0x82000 [astra-review-2026-09-26.md:97]) at 64 KiB: 0x6a0000 = 6.63 MiB (6.47 MiB at 4 KiB).
- Slice cost per VM: 8 slots 53 MiB (1.3 % of 4 GiB), 16 slots 106 MiB (2.6 %). Linux B peaked at 30 host TSGs of all engines [L soak lines]; step 0 gives the GR count.
- Mapping: the slot's pages through the reservation's own handle at private VAs of the shadow VAS, with the private-VA exclusions of ec3d1fa8. No new host VRAM object; today each buffer is its own host vidmem object outside the slice [hostmem.rs:1989-2031].
- Bind: Linux, every host TSG with a C597 object at build, before the first schedule (as NVIDIA's vGPU does for Unix guests). Windows, the KMD's GFXP_POOL BIND translated to the same slot (P17's translation made production).
  - BIND {GFXP_SET, gfxp 1, vMemPtrs[0..5]} on the host TSG; GET must show 1. Multi-channel TSGs allowed (the game's TSG had two GR channels).
- Teardown as `free_owned_gfxp`: BIND WFI, GET 0, unmap, free the slot. No slot left: the TSG stays WFI with a WARN.

Isolation:
- Memory: the buffers are part of the VM's reservation and go with it; a guest cannot grow host VRAM by creating contexts (today 6.47 MiB of host VRAM per set, capped at 8 sets [mirror.rs:591-592]).
- Reach: the carve-out is outside the guest's FB range, among the device's private pages, which guest BAR and PRAMIN accesses cannot reach since 8e621e0a. The VAs are private and excluded from set A. The buffers hold only this VM's context state: a guest kernel that finds them corrupts its own context (its own RC).
- Scheduling: GFXP and CTA/CILP bound how long a guest context holds the shared GR engine at a switch. With WFI any guest can hold it up to 4.29 s per switch (host desktop frozen, Xid 109). Work that cannot drain even then (an endless shader) still ends in Xid 109 and an RC of that TSG only, as here.

## Test plan

- Unit: SET decode and record; BIND payloads with host handles and private VAs only (as mirror.rs:7313-7340); slot bounds and reuse; bind before the first GPFIFO_SCHEDULE; no CILP-to-CTA downgrade; teardown order; slots exhausted gives WFI and a WARN.
- Gate 0 (step 0): Linux B vkcube and SotTR 720p; Windows D3D11, Vulkan, CUDA. Expected on dev: gfxp 0 on every GR TSG; the 615 SET payloads.
- Synthetic long draw in the guest: a GLES2 full-screen triangle whose fragment shader loops a uniform count, about 6 s per draw, while the host renders (host vkcube).
  - dev: host Xid 109 (the reproducer; it provokes a host Xid and a 4 s host desktop freeze, so it needs the user's go).
  - Step 2: no Xid; host vkcube frame time p99 unchanged.
- Synthetic long dispatch: many short CTAs summing to about 6 s (step 1, CTA); a CUDA spin kernel ended by kill -9 (the coverage-map case; step 1, CILP).
- Native reference on the host (needs the user's go): the same long draw natively shows whether native Linux graphics is WFI.
- SotTR 720p benchmark x3 on Linux B after step 1, then after step 2: result file written, no host or guest Xid; FPS and frame times against native; GFXP's cost from the step 2 A/B (one diagnostic build, no new env flag).
- Soak 30 min desktop plus game: no slot left at exit, host VRAM unchanged by GFXP.

## Implemented: steps 0 and 1 (branch `preempt-2026-09-29`)

Code: `preempt.rs` (decode, record, the rule, `apply_compute`), `mirror.rs` (build, late SET, teardown), `cuda.rs` (the SET answer), `channel.rs` (the record), `ctrl.rs` and `boot.rs` (RC post-mortem, exit summary).

Step 0, read only:
- Every host GR TSG, after its first GR object and before that channel's `GPFIFO_SCHEDULE`: GET_CTXSW_MODES on the building channel. One gpu.log line: `preemption: tsg K (guest C/T, host TSG H, chid N) at build, before the first schedule: <guest request>; host gfxp G (..) cilp C (..) zcull Z; <outcome>`.
- At teardown: one GET on the TSG's last host channel, before its free and before an owned GFXP set unbinds: `preemption: tsg K ... at teardown: host ...; earlier: <outcome>`.
- The guest's SET: decoded in the cuda.rs note (flags, hChannel, graphics, compute), NV_OK echo as before. The first SET of each payload shape goes to gpu.log (`preemption: guest SET of a new shape`); all SETs are counted by shape in the exit summary (`preemption, guest SETs:`).
- Any RC of a mirrored GR channel: `gr rc push: chid N (exceptType X)`: USERD GPGet/GPPut; the entry at GPGet-1 (VA, dwords, its last 6 methods as `subch[class]:method=data`); `compute dispatch` (SEND_PCAS_A 0x2b4, SEND_SIGNALING_PCAS_B 0x2bc) or `MME macro call` (0x3800 and up, the driver's draw path). Guest memory only.

Step 1, compute parity:
- Record: under the guest object the host TSG stands for (`tsg_key_of`): the TSG (CUDA, GL and Vulkan name it), a member channel's TSG, or a bare channel (Vulkan's 0xc46f SETs). Per flag, the last mode wins. Dropped with the guest object.
- Apply at the build: GET, at most one pointer-free BIND {CILP_SET, mode} on the host TSG handle, GET. Bound only if the GET shows the mode and an unchanged graphics mode. Later members of a multi-channel TSG join the bound context.
- A SET after the build: FIFO_IDLE_CHANNELS over the TSG's host channels first, only when a BIND follows; not idle, nothing bound.
- The rule: modes only rise (WFI < CTA < CILP).
  - A refused CILP is not retried as CTA; a CTA request leaves a CILP context in CILP; WFI or unknown requests are not bound.
  - A refusal keeps the host default, leaves one `WARN preemption:` line and is counted; the guest still gets NV_OK.
- Graphics: logged in the build line and the SET shape line, never bound (step 2).
- Windows: the KMD sends no SET (P30, P32, P36: 0 SETs; 326, 252 and 64 BINDs).
  - Without diagnostic.patch: the step 0 lines; compute stays at the host default.
  - With the patch: its BIND routing is unchanged. Entry points kept; the patch applies and compiles; the build script's seven markers are unique.
  - `bind_owned_compute` now shares `apply_compute`, so it no longer lowers CILP to CTA either.
- Exit summary: `preemption, host GR TSGs: ...` (read, bound CTA/CILP, already, no request, REFUSED, not idle, graphics asked).

The COMPUTE_PREEMPT buffer (0x900000) for CILP:
- No open code path allocates or binds it:
  - the kernel RM maps GR_CTX_BUFFER_PREEMPT to GRAPHICS_PREEMPT 0x0b; nothing maps to 0x0a [615 tree: src/nvidia/src/kernel/gpu/gr/kernel_graphics_context.c:256-297];
  - its preemption buffers are only unmapped and freed there, never allocated [kernel_graphics_context.c:2341-2358, 2852-2869];
  - the BIND has no slot for it [ctrl2080gr.h:861-870];
  - NVIDIA's vGPU guest allocates only 0x0b-0x0f, for GFXP [src/nvidia/src/kernel/vgpu/rpc.c:3662-3666].
- GSP-RM's own heap is fixed at boot and sized at 48 KiB per channel [src/nvidia/inc/kernel/gpu/gsp/gsp_fw_heap.h:41, 71]; 9 MiB per CILP context would exhaust it within a few contexts.
- 0x900000 = 36 SMs x 256 KiB, the TU106 register file. Inference, not in the open code: CILP saves into the context's local memory, which the guest UMD allocates in the VM's slice (C28, 58 MiB, "probably local memory" [docs/caraxes/cuda-lookahead.md:252]).
- Verdict: CILP most likely adds no host vidmem outside the reservation (0.7). Not measured. The host GR context itself (main, patch, PM) is host vidmem outside the slice today, CILP or not.
- Measure in the CUDA series: soak `vram_used_mib` with a CUDA context alive, this build against dev.

## Open

- The host default compute mode of a mirror GR context (step 0 reads it).
- Whether native Linux 615 Vulkan runs GFXP (the 615 SET payload; a native GET).
- Where GSP-RM keeps CILP state after a pointer-free CILP BIND: most likely nowhere on the host (see above); the CUDA series' soak lines confirm or refute it.
- Which work held GR in this run: needs step 0's RC post-mortem or a traced repeat.
- The owner of host TSG 4.
- The carve-out takes guest VRAM, which SotTR already exhausts ([display-balloon-2026-09-29.md](display-balloon-2026-09-29.md)).
