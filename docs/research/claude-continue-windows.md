# Claude Code continuation prompt

Continue Caraxes Windows work. Work in `/mnt/vmstore/caraxes/wt-codex-stability`,
branch `codex/stability-2026-09-27`. Do not switch to or modify the unrelated dirty
worktree at `/home/silas/git/Caraxes`. Use explicit working directories.

Read these files first:

- `docs/research/claude-handoff-2026-09-27.md`: latest run, workspace and constraints.
- `docs/research/windows-deferred-context-2026-09-28.md`: Windows evidence and limitations.
- `/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/checkpoint.json`: current runtime state.
- Applicable `AGENTS.md` and the graphify skill. Query the existing graph before
  browsing code; run `graphify update .` after code changes.

Confirm Git status and actual process state before doing anything with hardware.
The checkpoint may describe a run started after this prompt was written. Preserve
uncommitted changes and inspect them before editing. Coordinate with any active
agent; never start another VM against an in-use GPU.

## Objective and authorization

- Priority: Windows desktop, then Linux/Windows desktop latency, CUDA, final
  documentation and native/v1/v2 benchmarks, then additional GPU models.
- Make Windows a working GPU, not merely a device with PnP code 0. First require
  an NVIDIA D3D11 triangle/readback pass. Then repeat startup/teardown and run
  desktop workloads. Steam/3DMark comes after hardware rendering is proven.
- Fix bugs, run bounded real-GPU tests and commit locally. Do not push.
- Keep guests stock and helper-free. Windows uses QEMU vfio-user for now;
  Cloud Hypervisor remains the long-term preference. Linux target is Wayland.
- Do not copy code or benchmarks from nvkvm/kayfabe or other comparison projects.
- Give regular short German status updates. Write code comments and documentation
  in concise English. Record failures and uncertainty as carefully as passes.

## Hardware constraints

- Host: RTX 2070 TU106, NVIDIA 615.71.09. Keep the host driver bound.
  No GPU reset, unbind, module reload or privileged-interface bypass.
- One GPU test VM at a time. Windows: 4 GiB RAM, 4 CPUs, 1 GiB guest VRAM, slot 0.
- Preserve host VRAM floor 1024 MiB and BAR1 floor 96 MiB.
- Frozen Linux desktop A is untouched. Do not use the mutable Windows diagnostic
  binary for Linux comparisons. Linux B uses 4 GiB RAM, 4 CPUs, 2 GiB VRAM, slot 1.
- Never forward guest CPU pointers, raw physical addresses or guest RM handles
  to the host. Keep exact client/TSG/channel generation ownership and shadow VA
  exclusions. Failed cleanup must retain dependent resources and accounting.
- Stop and inspect on new host Xids, host instability, refused idle, or failed
  resource cleanup. Do not reset the host GPU to get another result.

## Confirmed state

- P9 completed eight deferred INITIALIZE/PROMOTE/EVICT cycles with ordered
  completions. D3D11CreateDevice still failed with `0x8876017c`.
- Windows requests GFXP_POOL mode 2. Old pool QUERY/INIT/ADD/REMOVE replies only
  pretended to implement it. Regular builds now refuse these controls during host
  execution (`2a4a83d1`); this driver responds with Code 43 rather than falling back.
- P14 proved ordinary host GFXP mode 1 with five real buffers, 6,778,880 bytes:
  BIND1/GET1, BIND WFI/GET0, unmap and free all passed. No workload was submitted.
- `80113455`, `ec3d1fa8`, `c9531d34` implement owned buffers, accounting, per-TSG
  lifetime, private VA exclusions and guarded teardown. They also fix lost
  ownership after failed RM_FREE, the wrong shadow TSG VAS key, and premature
  parent cleanup. Tests: ABI615 535 unit + 2 integration passed, one ignored;
  ABI610 532 unit + 2 integration passed before the final VA ownership fix.
- P15 uses an isolated diagnostic translating the captured pool BIND into owned
  ordinary GFXP. Pool bookkeeping and display LUT behavior remain incomplete.
  This diagnostic is not production pool support.
- P15 stalled before deferred registration. P15b booted with NVIDIA PnP code 0,
  but bind was correctly refused because a doorbell had already been published.
  Its D3D11CreateDevice failed with `0x8876017c` after 3461 ms.
- P16 binds owned GFXP and forwards target doorbells, but D3D still fails. The
  next BIND requests compute-only CTA mode (flags1, cilp1, all pointers zero).
  P17 is being prepared for that request. Check the handoff/checkpoint for its
  outcome; do not assume it passed.
- `178bfd2f` repairs shared pending interrupt replay on PCI MSI enable/eventfd
  installation. Full tests passed: ABI610 535 unit +2 integration, ABI615 538 unit +2
  integration, one ignored. Select ABI615 with `--features v615`; an environment
  variable alone does not select the build feature. This does not yet fix pending
  bits already drained into IntrTree or its local MSI flag being consumed while
  delivery is unavailable. Startup-stall causation remains unproven.

## Next work

1. Read the latest P16 result and artifact patch. Preserve the empty-first-use
   bind requirement. Check that held work resumes only after successful BIND1
   and GET1, with the same live target generation and root.
2. Require an actual NVIDIA hardware D3D11 rendering result. The checked-in
   `tools/windows/d3d11-render.ps1` tests triangle/clear/triangle readback.
   PnP code 0, a QEMU VGA screenshot, or wrapper exit 0 is not a rendering pass.
3. If rendering fails, compare the earliest divergent request/completion with
   prior runs. Fix one observed contract at a time; do not acknowledge unsupported
   operations or widen capabilities simply to pass initialization.
4. Implement a justified guest pool lifecycle before enabling the translation in
   regular builds. Captured INIT.maxSlots is zero despite QUERY capacity four;
   public headers say it is an input. Slot-to-context ownership and control-block
   semantics are still unknown. Current geometry comes from our fabricated
   response, not a native TU106 measurement. Keep this distinction explicit.
5. Finish the pending-interrupt audit with ACK-aware tests. The event log text
   `no MSI (disabled or no fd)` also covers ordinary coalescing and cannot prove
   a masking problem. Interrupt-page mode has its own reactor and watchdog.
6. After repeatable rendering, test desktop, teardown, memory recovery and
   Windows workloads; then return to Linux Wayland graphics and simultaneous
   multi-guest NVENC/NVDEC. None of these larger milestones is established yet.

## Run and artifact locations

Set a task-specific shell variable if useful:

```sh
WIN_TEST=/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain
```

- Harness: `$WIN_TEST/run.py`, `build-p16.py`, `d3d-probe.py`, `pool-capture.py`.
- Each run has `env.json`, logs and results. Each diagnostic build has its base
  commit, exact patch, build log and SHA-256. Read these before reusing a binary.
- `build-p16.py` uses a fixed output directory; use a new name for a new build.
  Builders temporarily patch sources and restore them in `finally`; do not
  concurrently edit the affected files. Verify restoration afterward.
- Mutable executable: `$WIN_TEST/vfio-user-nvgpu-cap`. It needs the existing
  `cap_sys_admin+p` setup. This is an authorized RM allocation path, not kernel
  caller identity. Kernel-only pool controls are not available through it.
- Shared Cargo target: `/mnt/vmstore/caraxes/codex-review-target`.
  Its release executable may still be a diagnostic build. Restore/build the
  regular source executable when ending work and record which binary is installed.
- Start only after reviewing the chosen binary and confirming no GPU VM runs:

```sh
CARAXES_DEFERRED_CONTEXT=1 CARAXES_SHADOW_USERD=1 CARAXES_FB_ZERO_BASED=1 \
CARAXES_TRACE_ACCESSES=0 WIN_BAR1_POOL_MIB=64 \
CARAXES_SHADOW_CHANNEL_DIAG=1 CARAXES_SEMAPHORE_DIAG=1 \
python3 "$WIN_TEST/run.py" start UNIQUE_RUN_NAME exec 1
```

- Use `run.py ping`, `ps`, `put`, `get` for QGA. Never issue parallel QGA calls.
- `d3d-probe.py start` and `status` run the bounded guest rendering probe. Inspect
  the output for the explicit NVIDIA triangle/readback PASS and all frame checks.
- `run.py stop` shuts down the owned VM/backend and archives active logs. It can
  take about a minute; poll without blocking communication. Record host kernel
  logs and check for new Xids, pinned memory and host object cleanup.
- Local primary NVIDIA source: `/mnt/vmstore/caraxes/ogkm-615.71.09`.
  The GFXP allocation/BIND/WFI ordering is documented in its public `rpc.c`.

Keep the handoff and checkpoint current after every completed experiment. Commit
coherent changes regularly, include what was tested, and leave an honest next step.
