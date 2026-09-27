# Caraxes working handoff for Claude Code

Updated: 2026-09-27, 23:15 CEST. Work continues; this is a checkpoint.

## Workspace and constraints

- Branch: codex/stability-2026-09-27.
- Worktree: /mnt/vmstore/caraxes/wt-codex-stability.
- Preserve the unrelated dirty worktree at /home/silas/git/Caraxes.
- User priority: Windows desktop, desktop performance, CUDA, final documentation
  and native/v1/v2 benchmarks, more GPU models.
- GPU testing and local commits are authorized. Do not push.
- Host-backed execution only. Keep the host NVIDIA driver bound; no host GPU
  reset, unbind, or module reload. Run one test GPU VM at a time.
- Host: RTX 2070 TU106, NVIDIA 615.71.09. Keep existing VRAM/BAR1 floors.
- Frozen desktop A is untouched. Linux B: 4 GiB RAM, 4 CPUs, 2 GiB VRAM, slot 1.
  Windows target: 4 GiB RAM, 4 CPUs, 1 GiB VRAM, slot 0. Debugger VM has no GPU.
- Artifact paths below are relative to /mnt/vmstore/caraxes/alpha-2026-09-27/.

## Windows: confirmed progress and blocker

Stock Windows driver 617.14 still does not provide a working NVIDIA desktop.
P1 reported NVIDIA PnP code 0 once with host execution and display enabled.
Rendering and repeatable startup remain unvalidated; see P1 below.

1. Zero-based guest framebuffer translation (ad397d73, opt-in
   CARAXES_FB_ZERO_BASED=1) fixes the stock driver's large fixed allocation at
   guest FB 0. KD7 observed a successful 905 MiB allocation without changing
   the guest request or driver instructions.
2. Later startup fails with 0xC000009A in display resource initialization.
   The first zero-sized resource is TMO: the model clears presence on every
   window. An isolated capability diagnostic makes its 36 KiB allocation
   succeed. Adding measured window ILUT CAPB then makes its 132 KiB resource
   succeed.
3. KD12 confirms the next failure: head-0 output-LUT constructor at
   nvlddmkm+0x15f2e32. Its allocation succeeded, but its descriptor has SFCLOAD
   clear and a nonzero pointer matrix. Static inspection shows that combination
   is rejected. Final status remains 0xC000009A.
4. The OLUT geometry diagnostic is built. KD13/KD14 produced no startup verdict
   because the serial debugger connection was unusable for the intended probe.

**Do not merge the diagnostic capability profile as supported functionality.**
The model stores LUT methods but does not yet load/apply the color pipeline.
The checked-in TU106 profile remains unchanged.

| Backend | SHA-256 | Status |
|---|---|---|
| Zero-FB baseline | 14e4519adc85b917cc6c674b6a2367f0fe3bb4224b07fc7c21b7318fea092d5b | Large stock allocation succeeds |
| TMO + ILUT | 55331d64b4207776e534255ff7f95cf0654da6e4e54d7d045f24e0d10f13ff35 | KD12 confirms later OLUT failure |
| TMO + ILUT + OLUT | 77f0c97e961c63303439c9afb145c31639181f5318ab370cbb5e24e2253781ac | Built, startup unvalidated |

- Diagnostic binaries, patches, TOML inputs and build logs:
  windows-kd/tmo-caps-experiment/.
- Evidence: windows-kd/debugger/KD12-complete.log and KD12-olut-failure.log.
- Detailed addresses: [Windows KD report](../../caraxes/runs/windows-kd-2026-09-27/README.md).
- windows-kd/debuggee/vfio-user-nvgpu-cap was restored to the zero-FB baseline
  after KD14. Windows target and debugger VMs are stopped.

## Windows transport and BAR1 pool

- WIN_BAR1_POOL_MIB=64 is a launcher opt-in (bfdf5458; preflight fix 490d4c99).
  QEMU mapped the 64 MiB dma-buf successfully.
- KD10/KD12 moved 603 cold pages in and back without pool conflicts or refused
  moves. No host rendering channels existed yet. Rendering coherency and a
  Windows performance gain are unvalidated.
- Serial KD repeatedly stalls/reconnects unreliably. A restarted debugger VM,
  local symbol-cache startup path and three direct failure probes worked in
  KD12; the cause of subsequent stalls remains unresolved.
- Avoid large conditional breakpoint sets. Prefer direct failure entries and
  +0x192c5af for final startup status. Re-read each boot's module base.
- Use windows-kd/kd-command.py and safe kd-log.ps1 snapshots. Queue consumption
  does not prove execution. Do not open the live log with a reader that blocks
  its writer. No parallel QGA calls against one VM.
- kdnet.exe and matching VerifiedNICList.xml exist in the debugger VM's
  Windows Kits x64 directory. A private second NIC/KDNET link is a candidate;
  validate target NIC support and PCI topology first. Unverified address 6
  can collide with the NVIDIA endpoint.
- Local launcher/bridge artifacts may differ from checked-in examples.
  Inspect windows-kd/ and checkpoint.json before resuming.

## Linux: measured improvements and remaining issue

The synchronous display-page acknowledgement fix plus read-only KVM mapping
traps writes while retaining direct reads (Cloud Hypervisor patch 0007).
Final backend: 419a308f. With fixed host CPU 3, stock 615, GNOME Wayland
1920x1080 and no guest helper, the final D4 pair measured:

| Combined backend + VMM CPU | Hybrid idle 200 us | 0 us |
|---|---:|---:|
| Idle | 14.73% | 7.20% |
| Graphics sequence | 185.05% | 163.45% |

100% means one logical CPU. Both runs had matching markers/acknowledgements
and no display/interrupt inconsistency. Keep the 200-us production default
pending soak and input/frame latency validation.

- Reproducer: desktop-checks/D4-readonly-display-page/.
- The shared build path was later rebuilt for Windows. Use the frozen binary
  beside the D4 scripts and SHA256SUMS-pinned, not that mutable build path.
- Validated Linux backend SHA-256:
  4e1a0c3d57855ef3605c0f14175e49b9782eee2318a154fdd9a1dc937331aa8b.
- ba9ceab5 adds launcher CARAXES_TRACE=0; exit summaries remain.
  D5 uses that same frozen backend. **No extra performance win established.**
- Completed combined graphics CPU: traced A1/A2 152.54%/165.49%, untraced
  B1/B2 146.84%/164.68%. OpenGL FPS has large outliers in both trace modes.
  No trace-off gain demonstrated; do not change the default.
- Completed D5 sequences ran five bounded Wayland vkcubes and three bounded
  glmark2 Wayland scenes. Runner:
  desktop-checks/D5-trace-overhead/measure-d5.sh.
- Earlier pre-fix D4 runs also slowed down, but used different builds. Do not
  mix them with the final 419a308f comparison. Next localization needs
  per-process monotonic timestamps and per-channel submission gaps.
- This round did not validate NVENC/NVDEC across guests, SOTTR, or a
  native/v1/v2 performance comparison.

## New shutdown fix

0cda1479 releases framebuffer mappings before host RM teardown. KD12 had two
late RM_UNMAP_MEMORY calls after the RM root client was freed (status 0x33).
The fix flushes/drains LRU and pinned mappings after mirror reset, credits
LRU capacity, and preserves cumulative counters.

- After this fix: ABI615 suite 499 unit tests and 2 integration tests passed,
  1 test ignored. Log: windows-kd/teardown-fix-tests.log.
- Fix validation: framebuffer tests 16 passed; revised ordering test passed.
  It checks flush/drop counts, drop-before-credit ordering and idempotence.
- Hardware validation of the new ordering is pending.
- Tracked post-teardown objects and pinned chunks were zero. This alone does
  not establish absence of every possible leak.

## Next actions and runtime checkpoint

1. D5 is archived and Linux B stopped. Continue Windows P1 below.
2. Validate the shutdown fix on a host-backed run; preserve baseline binaries.
3. Restore reliable Windows debugging or obtain a bounded non-debug boot
   result. Test OLUT diagnostic startup, then capture the next real failure.
4. Implement behavior behind required display capabilities; require repeated
   successful driver starts and actual GPU rendering.
5. Windows desktop/3D stability first, then desktop latency and simultaneous
   Sunshine/NVENC/NVDEC. Keep the guest stock and helper-free.

Linux B, the KD VMs, and P1 are now stopped. P1 used a new child overlay at
windows-plain/ (parent windows/win11.qcow2 unchanged), OLUT diagnostic 77f0c97e,
and the same host-backed limits. It reported NVIDIA 617.14 PnP code 0 in one query,
but the bounded D3D11 readback probe timed out after 30 seconds. Two host channel
allocations failed with NV_ERR_INVALID_ADDRESS (GPFIFO VA 0x12026e000 and
0x20104000); software channels 14/15 wait for semaphore VA 0x20002560 value 3,
observed 0. Source diagnostics are being prepared for the next P2 run.

No new host Xid was observed through the last check. Historical Xids at
uptime 3058.691336 and 3959.278221 predate these trials.

