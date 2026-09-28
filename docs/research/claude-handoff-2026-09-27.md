<!-- SPDX-License-Identifier: MIT -->
# Caraxes working handoff for Claude Code

Updated: 2026-09-28, 16:40 CEST (Claude Code). P17 ran at 08:15 and was stopped after
four host Xid 31; its result is below. No VM is running; the host was rebooted at 15:15.

Ready-to-use continuation prompt: [Claude Code prompt](claude-continue-windows.md).

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

## Latest Windows result: P17

- Build 7b435a18 plus P17-build/diagnostic.patch (binary SHA-256 5600ea77...);
  artifacts in windows-plain/P17-owned-compute-cta/. Started 08:14:59, stopped 08:17.
- **D3D11CreateDevice succeeded for the first time** (0x00000000 after 17.5 s),
  render resources were created at 18.2 s and frame 0 was submitted. The readback of
  frame 0 never completed; the probe ended with D3D_PROBE_TIMEOUT. Not a rendering pass.
- Owned GFXP buffers were mapped at private VAs and bound as in P16. Two later
  GFXP_POOL binds were refused by the diagnostic ("accepts only captured graphics pool
  binding shape", status 0x56).
- **Four host Xid 31** on the device's client: MMU fault ENGINE HOST0 HUBCLIENT_HOST,
  FAULT_PDE, VIRT_READ at 0x144e6000, 0x14d54000, 0x14201000 and 0x151ef000. Each is
  preceded in the host log by `dmaAllocMapping_GM107: can't update VA space for mapping
  @vaddr=0x5xxx_xxxx_xxxx` (e.g. 0x534d25a00000). host-kernel.log also has 150 VA
  allocation failures: 0x51 in 0x120200000-0x120630000 (the guest kernel-RM buffers,
  set B) and 0x1f at VA 0x0 and 0x13000.
- Device side: the shadow refuses Windows guest VAs in the 0x5262_xxxx_xxxx range with
  0x1f (for example `map va 0x526209400000 -> Fb(67a000) size 0x200000 refused 0x1f`),
  about 9,000 refused leaves per full walk, and the VAS of guest root 0xd3e9000 ended
  with 1007 map-failed. The faulting VAs are low push-buffer addresses; whether their
  leaves were refused, deferred or never walked is not yet attributed.
- Teardown: `exit 3: teardown failed closed (owned GFXP resources remain attached to a
  host context)`. The host client's resources went with the process; the host was
  rebooted at 15:15. Do not rerun P17 unchanged.

## Previous Windows result: P16

- Deferred context ordering and lifetime fixes remain in place. P9 completed
  eight INITIALIZE/PROMOTE/EVICT cycles, 24 APIs and 16 ordered semaphore
  releases. Its NVIDIA PnP code 0 did not establish rendering: D3D11CreateDevice
  failed with 0x8876017c.
- P10 captured Windows requesting GFXP_POOL mode 2 on a TSG, including guest
  pool GPU addresses and a guest CPU control-block pointer. P11 measured the
  actual owned host context in WFI mode 0.
- Found and fixed a false acknowledgement: bootstrap pool sizing/lifecycle
  responses were still active during host execution. They never constructed a
  usable pool. Commit 2a4a83d1 refuses those controls before recorded overrides.
- P12 proves this driver does not fall back after that refusal: GSP unloads and
  NVIDIA reports PnP code 43. This is the current baseline limitation.
- Pool controls require a kernel RM caller. CAP_SYS_ADMIN does not grant that
  access. Public headers expose some control-block offsets, but do not establish
  freelist initialization or justify the old estimated pool sizing.
- P13b: public SET_CTXSW_PREEMPTION_MODE (mode 2) on an empty, idled host TSG
  returns INVALID_ARGUMENT. GET remains WFI; SET0/GET restores successfully.
  P13's first boot did not reach the probe; that separate startup stall remains
  unexplained. Neither run submitted work on the tested target.
- 80113455 adds floor-accounted owned GFXP buffers and aligned host VA allocation.
  It also fixes a teardown bug: failed RM_FREE previously removed the object and
  children from the ledger, making Mirror's retained ownership impossible to retry.
- P14: direct SET1 succeeds, SET2 fails. Five owned host buffers (6,778,880 bytes)
  bind to the host TSG in mode 1; GET confirms it. BIND WFI/GET0 and complete
  buffer cleanup pass. No target workload was submitted.
- ec3d1fa8/c9531d34 implement per-TSG owned GFXP lifetime, private shadow VA
  exclusions, retryable cleanup, parent retention, and final-exit safeguards.
  The regular guest pool refusal remains. P15/P16 isolated compatibility
  tests retain incomplete bootstrap pool replies. Host mode 1 is not
  proof of guest pool mode 2 support or rendering.
- P15 stalls before deferred registration, like P13. P15b boots with PnP code 0
  but refuses owned GFXP because the target received a doorbell before BIND.
  D3D11CreateDevice returns 0x8876017c after 3461 ms. No rendering pass.
  P16 will hold the experimental target until its owned buffers are bound.
- P16 binds real owned GFXP successfully and then forwards target doorbells.
  NVIDIA PnP code 0, but D3D11CreateDevice still fails 0x8876017c after 4025 ms.
  The next BIND requests compute CTA (flags1, cilp1, all pointers zero); the
  diagnostic rejects it. P17 will translate this exact request and verify both modes.
- 178bfd2f repairs pending shared MSI replay on availability transitions. Tests:
  ABI610 535 unit +2 integration; ABI615 538 unit +2 integration, one ignored.
  IntrTree-local pending delivery still needs an ACK-aware audit; startup-stall
  causation remains unproven.
- No new host Xid through P15b. All completed runs stopped. LUT capability
  changes remain diagnostic only; the actual display color pipeline is unfinished.
- Tests: ABI615 535 unit + two integration passed, one ignored at c9531d34;
  ABI610 532 unit + two integration passed at ec3d1fa8.
- Detailed evidence: [Deferred context report](windows-deferred-context-2026-09-28.md).

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
| TMO + ILUT + OLUT | 77f0c97e961c63303439c9afb145c31639181f5318ab370cbb5e24e2253781ac | P1 PnP code 0 once; D3D11 timeout |

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
- P2 hardware shutdown completed with zero held windows, pinned chunks and
  tracked host objects; no late RM_UNMAP_MEMORY status 0x33 appeared.
- Tracked post-teardown objects and pinned chunks were zero. This alone does
  not establish absence of every possible leak.

## P2: USERD address-width failure confirmed

- P2 is stopped; all owned test VMs are stopped.
- Artifact directory: windows-plain/P2-channel-proof/.
- Backend: 7455aa13 plus the isolated LUT geometry profile; SHA-256
  f267cd5a22005925c8c1f2125d7003fd148de6d8edf726a870cf792f0ce2c1b5.
  Build inputs are in windows-plain/P2-build/. Checked-in profile unchanged.
- ABI615 diagnostic suite: 501 unit tests and 2 integration tests passed,
  1 ignored. See windows-kd/channel-diagnostic-tests.log.
- Both rejected GPFIFO spans are fully covered by matching guest/host leaves,
  without set-B overlaps. Shadow mapping failures do not explain the rejection.
- host-kernel.log identifies the actual rejection in
  kchannelCreateUserdMemDesc_GV100: USERD address high word 0x7fff exceeds its
  field width. Public GV100/Turing USERD fields allow a 40-bit address with
  512-byte alignment. The host uses valid 47-bit IOMMU addresses for guest RAM.
- This is an address-width mismatch, not evidence that a CPU virtual pointer
  was accidentally used as a physical address. Reusing the existing high-IOVA
  descriptor or mapping its CPU alias below 4 GiB does not solve it.
- Software CE chid 15 GP191 releases VA 0x20002540=2, then waits for
  VA 0x20002560=3, observed 0. The later lifecycle repeats on chid 14.
  Pending GR channels have no successful host channel. They are candidate
  producers, but their pending pushes have not established the exact producer.
- P2 created zero host channels and forwarded zero doorbells. It is not a
  rendering success. QGA was intermittent; the P2 startup query timed out.
- P2 teardown validated the mapping-order fix. No new host Xid observed in P2 (P3 below has a new Xid).
- D3D11 probe now emits flushed stage markers to distinguish DXGI, device
  creation and readback hangs. Uploaded to P2 but not executed in that run;
  runtime validation remains pending.

## P3: USERD fixed, deferred API blocks execution

- P3-private-userd is stopped; all owned test VMs are stopped.
- Commit 19f70c3e, isolated LUT geometry profile, CARAXES_SHADOW_USERD=1.
  Binary SHA-256: 0dd448a631a3fdb8faf47a159aa79c3906e3b76e829512cf23335dbc36cc3a9c.
- Both previously refused GR channels were created and scheduled with private
  host USERD. Whole-run totals: 4 channels created/freed, 5 doorbells forwarded.
- A new host Xid 32 occurred on host channel 0x2f, guest chid 4. Stop initiated
  after observing it; no host reset/unbind. Journal is in P3-private-userd/.
- Interrupt 0x00800000 is TU104 PBDMA DEVICE (bit 23), not METHOD (bit 21).
  Method 0x80050200 means subchannel 5, method 0x200: NV5080_DEFERRED_API,
  handle 0x40000003. Two preceding registrations, control 0x50800101, were
  refused as legacy_uncalled. No matching deferred operation was registered
  on the host object. This is the next compatibility target.
- The following method 0x1b00 only gives REPORT_SEMAPHORE_A. Without B/C/D,
  its exact connection to the CE wait at 0x20002560 remains unproven.
- The guest supplied no error notifier for that channel. The later host
  OBJECT_NOT_FOUND is a fault-reporting failure, not the cause of Xid 32.
- Teardown reports zero held windows/pinned chunks/tracked objects; no late
  unmap status 0x33. Windows rendering and stable startup remain unvalidated.
- Do not rerun P3 unchanged. Bounded deferred-control capture is implemented;
  capture mode holds channels containing a 0x5080 object before host submission.

## P4: deferred context operations captured

- P4-deferred-capture is stopped. All owned test VMs are stopped.
- Base f56b8bb2 plus isolated LUT geometry profile; source profile restored.
  Binary SHA-256: 69a4992196e545d5b1535791058be263564d75ceb4b7c686ff88c348aedf4469.
- CARAXES_DEFERRED_API_CAPTURE=1 held affected GR channels before submission:
  zero host channels created, zero forwarded doorbells. No new host Xid in P4.
  Exit reports zero pinned chunks and tracked host objects.
- Eight complete 584-byte registration payloads captured. First sequence:

| API handle | Nested command | Meaning |
|---|---|---|
| 0x40000002 | 0x2080012d | GPU_INITIALIZE_CTX |
| 0x40000003 | 0x2080012b | GPU_PROMOTE_CTX |
| 0x40000004 | 0x2080012c | GPU_EVICT_CTX |

- First sequence: GR0, hClient 0xc1d0002a, hChanClient 0xc1d0002c,
  hObject 0xff0e0000, ChID 17. This targets another client's TSG; the deferred
  object belongs to client 0xc1d00018. Do not substitute the submitting channel.
- Initialize: FB offset 0xe2000, size 0xad400, physAttr 0, hVirtMemory 0,
  hDmaHandle 0, index 17. Promote: VA 0x17000, same size, entryCount 0.
  Flags, hClientVA and hDeviceVA are zero. Later requests reuse object values
  under different clients: resolve by client and object, not object alone.
- Reference layout: OGKM 615.71.09 ctrl5080.h and ctrl2080/ctrl2080gpu.h.
  Raw trace and decoded JSON: windows-plain/P4-deferred-capture/.
- This is capture evidence, not context execution or a rendering pass.

## P5: ordered context implementation and first hardware test

- Experimental implementation committed as 73483c8d, CARAXES_DEFERRED_CONTEXT=1.
- Exact engine identities, strict management yield/resume, target-generation
  validation, private empty USERD preparation and single-TSG eviction are present.
- ABI615: 522 unit tests and two integration tests passed, one ignored.
- P5 accepted both registrations, then stopped on historical subchannel 3 method
  0x888 during takeover-prefix validation. No deferred execution or rendering pass.
- No new host Xid. VM stopped; diagnostic LUT profile remains unmerged.
- Next: capture the complete initial prefix and validate it before retiring the
  host channel. FIFO GPGet equality alone does not prove GPU work completion.
- Details: [Deferred context report](windows-deferred-context-2026-09-28.md).

## Next actions

1. Attribute the P17 faults before any GPU run: why the host refuses the 0x5262_xxxx_xxxx
   guest VAs (compare the mirror's host VA-space range with the VA space the Windows KMD
   allocated), and map each fault VA through the guest page tables of the faulting
   channel (refused, deferred or never walked). Then rerun the triangle/readback gate.
   Check checkpoint.json and the latest report before launching another VM.
2. Resolve the next observed failure, preserving ownership, completion ordering
   and cleanup. PnP code 0 and wrapper exit code 0 do not establish rendering.
3. Validate guest pool lifecycle and implement the display LUT behavior before
   enabling the diagnostic capabilities in a supported profile. Current pool
   sizing is synthetic; closed Windows slot/control-block semantics are unknown.
4. Audit IntrTree-local MSI delivery across disabled/no-eventfd transitions with
   ACK-aware tests. The shared pending fix does not cover every interrupt source.
5. Once rendering passes, repeat startup/teardown, desktop and Windows workloads.
   Steam/3DMark is conditional on a hardware rendering pass. Then resume Linux
   Wayland latency and concurrent Sunshine/NVENC/NVDEC validation.

Historical host Xids at uptime 3058.691336 and 3959.278221 predate these trials.
