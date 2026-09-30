# flags-2026-09-30: one universal device configuration

- Branch `flags-2026-09-30` from train c3e9bd5b, worktree `/mnt/vmstore/caraxes/wt-flags`. Not merged, not pushed.
- The table in the code: `crates/vfio-user-nvgpu/src/settings.rs` (every variable read, by class; every removed one with what became of it). A test keeps it equal to the sources.
- Inventory: 109 names read at c3e9bd5b (`device-vars.txt`), plus `LEA_GPU_NAME_FORMAT` (read through nvrm_abi::naming). `EDID_DECODE` is test-only, `SUDO_UID/GID` system.
- Wrappers: `wrappers.tsv` (15 wrappers x 81 rows).

## What production ran (the evidence for the defaults)

- **Linux B 615** (kick-smoke -> night-2026-09-28/desktop-b-night.sh), **nouveau+NVK** (openrender-boot.sh; vm10-gate and boot10/11 setup.txt), **Windows** (run44.py DEV + gpu-up-host.sh exports + CH_DEV_EXTRA `CARAXES_INTR_PAGE=1 CARAXES_DOORBELL_POLL=hybrid WIN_BAR1_POOL_MIB=32`).
- All three, with host execution: MIRROR_CHANNELS=1, MIRROR_GR_SHADOW=1, MIRROR_BARE_CE=1, CE_SHADOW=1 (default), GUEST_DMA_GPA=1, INTR_PAGE=1, DOORBELL_POLL=hybrid, BAR1 pool 32 MiB, BAR1 windows 2 x 16 MiB, HOST_VRAM_FLOOR_MIB=1024.
- Linux B and nouveau only: DOORBELL_TOKENS=64, BAR2_MEMFD=1, MSI_REARM_IOEVENTFD=1, COMPRESSION=slice, HOST_BAR1_FLOOR_MIB=64. Linux B only: DISPLAY_PAGE=1 (+ CH read-only display page), ALLOC_LAZY=1.
- Windows only: SHADOW_USERD, DEFERRED_CONTEXT, FB_ZERO_BASED, SHADOW_FENCE, WIN_ACKS, WIN_COMPBIT, WIN_DISPLAY_QUERIES, RUSD, EMU_GR, PREFER_RECORDED, LPWR_2806_OK, ACK_UNANSWERED + WIN_EXCEPT; COMPRESSION=off; HOST_BAR1_FLOOR_MIB=96; the interrupt tree page mapped read-only by CH (run-ch-gpu.sh keys it on CARAXES_INTR_PAGE=1: exact mode, 76b2eb18).
- Nova gate: pure emulation (`--boot-model`), `CARAXES_GUEST_ABI=570` (the launcher passes `--guest-abi`).

### Wrapper bugs found

- alpha-2026-09-27/desktop.sh:11 (desktop-b-alpha) still lacks CARAXES_MIRROR_BARE_CE (the 2026-09-28 bug class). Now the default.
- P43-prod/env.json is written before gpu-up-host.sh exports GUEST_DMA_GPA, MIRROR_CHANNELS, MIRROR_GR_SHADOW, MIRROR_BARE_CE, GUEST_RAM_BYTES: it under-states the Windows device env.
- WIN_GPU_ROMBAR is read only by run-qemu-gpu.sh, dead on the CH path of run44.py.
- Host BAR1 floor: Windows 96, Linux 64 (OPEN).

## Rule

- DEFAULT: set to the same value by all three host-execution production wrappers and their gates, or an A/B switch whose default every gate since its merge ran with. The variable goes; a set one logs one warning line and is ignored.
- Defaults key on `--host-exec` (a mode, not an OS). Pure emulation (nova, emu-boot) keeps its trapped paths.
- DELETE: lost (measured without gain, or superseded and refused next to the production set) and set by no wrapper; code included.
- OPEN: not the same on the three guests, or no A/B verdict on the current base. Kept as they are.

## Classification of the 110 names: DEFAULT 42, DELETE 11, KNOB 4 (+ `--display`: 5), OPEN 26, DIAG 24, INPUT 3

### KNOB (5)

| Knob | Default | Values |
|---|---|---|
| VRAM: `--profile`/`--fb-mib` (`LEA_VRAM_PROFILE_MIB`) | 4096 MiB | size |
| Display: `--display` | off | flag |
| `CARAXES_COMPRESSION` | off | off, slice (Linux gates slice; Windows D16 A/B is the Windows agent's) |
| `CARAXES_DOORBELL_POLL` | hybrid with --host-exec, else off | off, hybrid, spin (off refused with the interrupt page) |
| `CARAXES_BAR1_POOL_MIB` | 32 with --host-exec | 0 (none), even 2-128; refused without host execution |

### DEFAULT (42): variable removed, behaviour kept

| Var | Now | Evidence |
|---|---|---|
| MIRROR_CHANNELS, MIRROR_GR_SHADOW, MIRROR_BARE_CE | on with --host-exec | all 3 wrappers + gates |
| CE_SHADOW | always | default everywhere; =0 refused next to pool/compression/zero FB |
| GUEST_DMA_GPA | on with --host-exec | all 3 |
| INTR_PAGE | on with --host-exec | all 3 |
| BAR1_PASSTHROUGH (=pool) | pool 32 MiB with --host-exec | all 3; `=1` deleted (below) |
| BAR1_WINDOWS, BAR1_WINDOW_MIB | 2 x 16 MiB | all 3 |
| HOST_VRAM_FLOOR_MIB | 1024 | all 3 |
| TRACE_FLUSH | flush at E lines and every 100 ms | Windows, Linux B |
| LAUNCH_FAST | 2 | default since ca2559f5 |
| DISPLAY_KICK_PAGE | on with the display page | default since 4dde63e1 |
| HOST_BOOST | on with --host-exec | default since 78d4b0fd |
| POSTED_DOORBELL, FAST_READS | on | 097570e1, "to be removed once proven"; own commit 1843beb1, proven by the train gate |
| SHADOW_COALESCE, _BULK_FREE, _REGION_POOL, _GATHER, _WORKER | on | A/B switches (memory-model.md) |
| SHADOW_DEFER_TLB | off | "Measured, it saved nothing" (memory-model.md §7.5); the dormant mechanism stays (follow-up) |
| SHADOW_UNTRAPPED_HOLD, SHADOW_INVALIDATE_ASYNC | on | "A/B only" |
| MEMOP_INVALIDATE, MIRROR_VIDEO_OBJECT, BAR_TLB, FORWARD_CACHE, LINEAR_IOVA, COMPRESSION_CLEAR, BAR1_POOL_PIN, BAR2_MEMFD_DEFER_FLUSH | on | A/B switches of defaults |
| DOORBELL_GUEST_TOKENS | automatic with --boot-model | Windows relies on auto since cded5158 |
| DOORBELL_POLL_CPU, _IDLE_US; INTR_PAGE_SLEEP_MS, _RETRIGGER_MS; BAR2_MEMFD_TRAP | auto, 200 us; 1 ms, 10 ms; 0xfea000 | sub-settings no wrapper sets |
| MAP_BUDGET, ZBC_MAX, FALCON_STALLACK | 32768, 4, boot.toml | constants no wrapper sets |
| GUEST_ABI (variable) | the guest chooses | `--guest-abi` stays; caraxes-vm.sh passes it and unsets the variable |

### DELETE (11): code included

| Var | Evidence |
|---|---|
| BAR1_PASSTHROUGH=1, _UNSAFE_LAYOUT, BAR1_PT_MIB, BAR1_PT_FB_OFFSET (design G) | refused since 2026-09-24 (128 pages mismatched); the pool replaces it |
| DOORBELL_IOEVENTFD | wildcard ioeventfd, refused with pool, zero FB, translated FB, compression; KVM EEXIST next to datamatch |
| MIRROR_GR (c2) | the GR-context experiment the GR shadow replaced (VAS replay, host SET_PAGE_DIRECTORY, GPU_PROMOTE_CTX) |
| MIRROR_CE (=0) | kept CE in software for that experiment |
| BAR1_POOL_WARM | A/B of 2026-09-24 |
| DOORBELL_POLL_ADAPT_US | "did not pay off. Keep it off." (launch-path.md §5) |
| INTR_PAGE_VBLANK_SPIN_US | "no measured gain" (intr-page.md) |
| BAR2_MEMFD_SYNC_DOORBELL | the reference full sync, superseded by 5fcf62ae |
| GSP_FW_VERSION | no wrapper; the served ABI's version is given |

### OPEN (26): kept, each needs an A/B

| Var | Why / the A/B |
|---|---|
| DOORBELL_TOKENS=64 | Linux B + nouveau; Windows: pool tokens beside the guest tokens |
| BAR2_MEMFD=1 | Linux B + nouveau (vkCreateDevice 13 s -> 48 ms); Windows A/B running (the Windows agent) |
| MSI_REARM_IOEVENTFD=1 | Linux B + nouveau; Windows A/B |
| DISPLAY_PAGE=1 | Linux B only; Windows A/B |
| ALLOC_LAZY=1, ALLOC_LAZY_MS | Linux B only; =0 did not change the plan cost (night-2026-09-28); A/B nouveau, Windows |
| HOST_BAR1_FLOOR_MIB | Windows 96, Linux 64; A/B host Vulkan with 64 MiB free |
| SHADOW_USERD, DEFERRED_CONTEXT, FB_ZERO_BASED, SHADOW_FENCE | Windows only; A/B on Linux B and nouveau (the zero-based FB is the card's own layout) |
| WIN_ACKS, WIN_DISPLAY_QUERIES, RUSD, LPWR_2806_OK | Windows-only answers; A/B on Linux (the card answers every OS alike) |
| EMU_GR, PREFER_RECORDED | Windows only, pure emulation only (inert with host execution); EMU_GR breaks Linux emu-boot by design |
| USERMODE_PAGE | -40 % on 2026-09-28, reactor fixed in 0460db79, not re-measured |
| INVALIDATE_PAGE | unsafe on 2026-09-28, fixed by 9b52ab1c, not GPU-validated |
| BAR2_FLUSH_PAGE=memory | "Needs a GPU run before the knob is trusted" |
| INTR_DIRECT | no A/B verdict |
| INTR_GATE_MS | no measurable change under a loaded host |
| RPC_IOEVENTFD, LEGACY_CACHEABLE | opt-in since the 2026-09-28 merge, no verdict |
| EXT_CONFIG | the card shows the extended space to the OS; no A/B |
| Compression default (knob) | slice on Linux, off on Windows until D16 |
| Launcher: interrupt tree read-only in CH | Windows exact mode vs Linux tagged mode; A/B on Linux B |

### DIAG (24)

PROFILE, PROFILE_OUT, TRACE_ACCESSES, LAUNCH_TIMING, SOAK_SECS, MIRROR_DIAG, SHADOW_CHANNEL_DIAG, SEMAPHORE_DIAG, SEMAPHORE_DIAG_CHID, DUMP_KERNEL, RECORD_HOST, ACK_UNANSWERED (Windows production depends on it: OPEN note), WIN_EXCEPT, WIN_ACK_EXTRA, WIN_DISP_CAPS, DISPLAY_UNPLUGGED, P20_SCAN, INJECT_TEARDOWN_FAILURE, GOE_SELFTEST, RC_SELFTEST, DEFERRED_API_CAPTURE, SHADOW_ALWAYS_RESYNC, SHADOW_REREAD, HOST_ABI (host layout override for bring-up).

### INPUT (3)

LEA_ROOT, CARAXES_GUEST_RAM_BYTES (the launcher's guest RAM size, linear IOVA), LEA_GPU_NAME_FORMAT (nvrm_abi, shared with v1).

## Status

- Commits on c3e9bd5b (one change each): 15158300 args (--host-exec implies the four host flags), 2097f125 mirror shadow-only + c2 deleted, dfce17f4 GPA DMA, 3827ae4d intr page + hybrid poll, 1d5fe476 pool 32 MiB + windows, b184e771 design G deleted, 4c5a954a token-less ioeventfd deleted, 5e258339 A/B switches of defaults, 1843beb1 posted doorbell / fast reads switches (drop if the gate needs them), a873400a settings table + config block + warnings, 5c3020f1 launcher and gate scripts, 91c79663 mode line.
- Head **91c79663**. Binary `/mnt/vmstore/caraxes/vfio-user-nvgpu-flags1-91c79663` and `-cap` (cap_sys_admin+p), sha256 7574d6d077f382c525efadb70f198609a3dd724a49adbc1efae2cda840e59496. 40 files, +1023 -2566 lines.
- Tests: `cargo test -p vfio-user-nvgpu` 845 passed, 1 ignored (train 848: 10 tests of deleted code gone, 7 new: settings 6, the pool knob; the poll, interrupt-page, window-budget, host-floor and guest-token tests rewritten for the defaults). clippy -D warnings clean (workspace, all features). tools/check.sh (`check.log`): the Rust steps pass; cargo doc fails on 4 links from train commits (shadow.rs 532/1043/1046, ver/mod.rs:36), licence on 4 files from other branches, class-sizes and kapi-abi need the vendor tree (absent in the worktree): all pre-existing. Two timing flakes seen once each in ~15 full runs (release display::tests::channel_pages_read_back_assembly_and_armed_state; debug bar1pool a_page_the_gpu_maps_moves_behind_held_rings...), both pass alone and in 6 reruns; code untouched by this branch.
- Smoke: the release binary in pure emulation only (no host RM, no GPU), to see the warnings and the config block. No VM run (priority file names windows).
- 53 variables removed (42 promoted to defaults, 11 deleted with their code; `CARAXES_BAR1_PASSTHROUGH` counted as a default), 57 read now (4 knobs, 26 open, 24 diagnostics, 3 inputs).
- After the rebase onto final-2026-09-30 (Windows agent): `CARAXES_WIN_COMPBIT` moved from OPEN to REMOVED. D16 (fc4c297e, on train+D16) retired it: without compression the compbit store is refused, with `CARAXES_COMPRESSION=slice` compression.rs answers it. The device now warns through settings.rs like every removed variable, and D16's own warning line in main.rs is gone. Now 54 removed and 56 read (25 open).
- At start the device prints `config: WARNING <VAR>=<v> is no longer read (removed 2026-09-30: <what now>); ignored` per removed variable, then before the server starts one block of `config:` lines: mode, knobs (set/default), paths in force, open switches set, diagnostics set, inputs, CARAXES_ variables not read.
- Behaviour against the train under each gate's wrapper: the same set (every default equals what the wrapper set); differences only in the log and in the trace flush (buffered for nova/openrender, as production).
- Left: the dormant deferred-TLB mechanism in shadow.rs (variable gone, stays off); the dmem wrappers in caraxes/runs/dmem-2026-09-29 (they run the launcher copy caraxes-vm-night.sh outside the repo, which still reads GUEST_DMA_GPA, INTR_PAGE, the mirror variables for its own checks); docs that still name removed variables (the docs polish pass).
- Host GPU state: none touched. The only device start was pure emulation (no host RM) to check the config block.
