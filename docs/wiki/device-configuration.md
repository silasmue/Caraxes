<!-- SPDX-License-Identifier: MIT -->
# Device configuration

Since 2026-09-30 one configuration of `vfio-user-nvgpu` serves every guest OS. `--host-exec`
is a mode, not an OS: it switches on everything host execution needs, and pure emulation (nova,
emu-boot) keeps its trapped paths. Five knobs tune the device; 25 open switches wait for an A/B,
24 diagnostics and 3 inputs complete the list. The table is `VARS` and `REMOVED` in
`crates/vfio-user-nvgpu/src/settings.rs`, and the test `the_table_is_the_sources_list` keeps it
equal to the variables the sources read. Evidence: `caraxes/runs/flags-2026-09-30/NOTE.md`.

## What --host-exec implies

- Flags: `--boot-model`, `--host-rm`, `--reserve-vram` and `--register-guest-ram`; naming them
  as well changes nothing (argument parsing in `crates/vfio-user-nvgpu/src/main.rs`).
- Refused: `--host-exec` with `--identity-dma-table`, and on a chip other than the host's TU106
  unless the board is a costume-ladder board with `host_chip = "tu106"`.
- On with host execution (each was a variable before):
  - every host channel on a shadow VA space (`MIRROR_CHANNELS`, `MIRROR_GR_SHADOW`, `MIRROR_BARE_CE`);
  - guest DMA addresses are GPAs, registered RAM at one linear IOVA offset (`GUEST_DMA_GPA`);
  - the interrupt page (`INTR_PAGE`) and the hybrid doorbell reactor;
  - the BAR1 pool of 32 MiB (`BAR1_PASSTHROUGH=pool`) and the host boost (`HOST_BOOST`).
- Always on: the CE shadow, the posted doorbell, the reads before the device lock, the shadow's
  coalescing, bulk free, region pool, gathers and worker, the untrapped-GPPut hold, the held
  invalidate, `MEM_OP` invalidates, the video object mirror, the BAR TLB, the forward cache,
  the linear IOVA, the compression clear, the pool pin, the deferred BAR2 memfd flush.
- Fixed values: BAR1 windows 2 x 16 MiB, host VRAM floor 1024 MiB, `LAUNCH_FAST` 2, map budget
  32768, ZBC entries 4, interrupt page sleep 1 ms and re-trigger 10 ms, reactor idle 200 us.
- Chosen by the device: the guest tokens (automatic with `--boot-model`), the reactor's core,
  the guest ABI (the guest's GSP boot decides, `--guest-abi` pins,
  [research: one-abi-binary-2026-09-28](../research/one-abi-binary-2026-09-28.md)).

## The five knobs

| Knob | Default | Values |
|---|---|---|
| VRAM: `--profile` / `--fb-mib`, else `LEA_VRAM_PROFILE_MIB` | 4096 MiB | a size |
| Display: `--display` | off | flag |
| `CARAXES_COMPRESSION` | off | `off`, `slice` (real compression per VM slice, D16b; needs `--host-exec`) |
| `CARAXES_DOORBELL_POLL` | `hybrid` with `--host-exec`, else `off` | `off`, `hybrid`, `spin`; `off` is refused while the interrupt page runs |
| `CARAXES_BAR1_POOL_MIB` | 32 with `--host-exec` | 0 (none) or an even 2 to 128; refused without host execution |

## Removed variables

- 54 removed: 42 became defaults, 11 were deleted with their code, and `CARAXES_WIN_COMPBIT`
  was retired by D16 (without compression the compbit store is refused, with `slice` it is answered).
- Deleted: the fixed-layout BAR1 passthrough (design G, four variables), the token-less doorbell
  ioeventfd, the c2 GR experiment (`MIRROR_GR`, `MIRROR_CE=0`), `BAR1_POOL_WARM`,
  `DOORBELL_POLL_ADAPT_US`, `INTR_PAGE_VBLANK_SPIN_US`, `BAR2_MEMFD_SYNC_DOORBELL`, `GSP_FW_VERSION`.
- A removed variable that is still set is ignored and named once at start:
  `vfio-user-nvgpu: config: WARNING <VAR>=<value> is no longer read (removed 2026-09-30: <what now>); ignored`
  (`settings::removed_warnings`). The Windows kit (`caraxes/runs/win11-2026-09-25/run44.py`,
  `DEV`) still sets some of them.

## The config block

- After the warnings and before the server starts, `config_block` in `main.rs` logs lines with
  the prefix `vfio-user-nvgpu: config:`:
  - `mode ...; chip ..., board ...; guest ABI ...`: host execution with its slot, or emulation with the host parts in use;
  - `knobs:` the VRAM size, the display, and the three variables, each marked `(set)` or `(default)`;
  - `paths:` shadow VA spaces, GPA DMA and host boost, the BAR1 windows, the interrupt page, doorbell
    and guest tokens, the MSI re-arm and GSP queue-head ioeventfds, the display, kick, usermode and
    invalidate pages, the BAR2 memfd, a zero-based FB;
  - `open switches:` and `diagnostics:` the set members, or `none`;
  - `inputs:`, and `not read by the device (launcher settings or misspelled):`, only when non-empty.

## Open switches per guest (production wrappers, 2026-09-30)

| Switch (default off) | Linux 615 | nouveau + NVK | Windows | A/B owed |
|---|---|---|---|---|
| `DOORBELL_TOKENS=64` | set | set | not set | pool tokens beside the guest tokens |
| `BAR2_MEMFD=1` | set | set | not set | Windows |
| `MSI_REARM_IOEVENTFD=1` | set | set | not set | Windows |
| `DISPLAY_PAGE=1` | set | not set | not set | Windows |
| `ALLOC_LAZY=1` | set | not set | not set | nouveau, Windows |
| `HOST_BAR1_FLOOR_MIB` (96) | 64 | 64 | 96 | host Vulkan with 64 MiB free |
| `SHADOW_USERD`, `DEFERRED_CONTEXT`, `FB_ZERO_BASED`, `SHADOW_FENCE` | not set | not set | set | Linux 615, nouveau |
| `WIN_ACKS`, `WIN_DISPLAY_QUERIES`, `RUSD`, `LPWR_2806_OK` | not set | not set | set | Linux (the card answers every OS alike) |
| knob `COMPRESSION` | `slice` | `slice` | `off` | Windows (D16) |

- `EMU_GR` and `PREFER_RECORDED` act only in pure emulation (Windows bring-up); `EMU_GR` breaks
  the Linux emu-boot.
- Built, off, set by no wrapper: `USERMODE_PAGE`, `INVALIDATE_PAGE`, `BAR2_FLUSH_PAGE=memory`,
  `INTR_DIRECT`, `INTR_GATE_MS`, `RPC_IOEVENTFD`, `LEGACY_CACHEABLE`, `EXT_CONFIG`.
- Interrupt tree mode is a launcher choice: Windows maps the tree page read-only in Cloud
  Hypervisor (exact mode), Linux keeps it read-write (tagged mode).
- The nova gate runs pure emulation (`--boot-model`) with the guest ABI pinned to 570.

## Diagnostics and inputs

- Diagnostics (24): the profile and its output prefix, the access trace filter, launch timing,
  the soak interval, mirror, shadow-channel and semaphore diagnostics, the deferred API capture,
  the shadow reference modes (full resync, re-read), kernel payload dumps, recorded host answers,
  the Windows bisect aids (`ACK_UNANSWERED`, `WIN_EXCEPT`, `WIN_ACK_EXTRA`, `WIN_DISP_CAPS`),
  an unplugged display, the isolation scan, fault and self-test injection, the host layout override.
- `ACK_UNANSWERED` with `WIN_EXCEPT` is still part of Windows production.
- Inputs (3): `LEA_ROOT`, `CARAXES_GUEST_RAM_BYTES` (the guest RAM size for the linear IOVA),
  `LEA_GPU_NAME_FORMAT` (`transport` or `legacy`, shared with v1; [identity-and-costume](identity-and-costume.md)).

## Depth

- `caraxes/runs/flags-2026-09-30/NOTE.md`: the 110 names, the rule, the evidence per default.
- [verification-methods](verification-methods.md): the gates that set the defaults.
