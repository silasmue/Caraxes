<!-- SPDX-License-Identifier: MIT -->
# Stability checks, 2026-09-27

Evidence root: `/mnt/vmstore/caraxes/alpha-2026-09-27/RESULTS.md` and the
adjacent raw run directories. One host-backed GPU VM at a time. RTX 2070,
host NVIDIA 615.71.09; no host driver unbind, reset or module reload.
No new host Xids observed through Windows B11, Linux L6 and the V1 import trials.

## Confirmed improvements

- V1 host-memory import: 32 sequential 2 MiB import/free operations and a
  separate 4096-byte GPU copy with all 1024 words verified. Repeated with
  NVIDIA-only adapter selection. Final guest counters: opened = closed = 246,
  active contexts = 0, pinned KiB = 0, pool pages = 0, quarantined pages = 0.
  Backend source `0e23ed72246b4e84546d8287a1075db5ed27e09f`; probes
  `1cf19cd030d83d76767df8a48f9ce0a621411a29`. Alias/share/UVM paths deliberately
  remain restricted; this is not complete external-memory extension coverage.
- V2 context-property cache and redundant-refresh removal: the identical own
  nvprobe workload returned from approximately 7 ms/launch to 19–32 us.
  Frozen historical reference: 12/22/13 us. Three repetitions, active host
  desktop, no claim of statistical equivalence or recovered native latency.
- Bounded software execution of internal physical CE operations and legacy
  semaphore releases: Windows passes its first memset/copy checks. Unsupported
  operations fail without publishing successful completion. Host physical
  privilege is not inherited from guest channel flags.
- Physical context promotion validates ownership and arithmetic before host
  submission. Software tests cover invalid ranges and untranslated system memory.
- Histogram quantiles above 200 us use actual tail buckets instead of returning
  the global maximum for every overflow percentile.
- Linux CUDA works with GPA aliases and no `caraxes_dma_range`, including guest
  NVIDIA/UVM unload/reload. After the FB-region correction, CUDA and the own
  NVIDIA-only Vulkan host-import/GPU-copy probe also pass without the helper.
  The Vulkan test uses the staged 615 ICD explicitly; the default test image
  loader path otherwise falls back to llvmpipe. This is a transfer test; rendered
  frames, desktop, streaming and longer soak still need testing.
- RPC/GMC compatibility: reject invalid RPC signatures/versions before dispatch,
  preserve all 64 GMC sequence bits in normal and error replies, and keep GMC
  responses separate from RPC continuations. See
  [the Nova reference analysis](nova-transport-compatibility-2026-09-27.md).

## Final transport verification

Commit `a0f1366d`; v615 release SHA-256
`dbf1c6e271554f15a2f6fdd623a62a0f7f430b907abba58e82168b4e34ffa348`.
The binary was built before final comment-only clarification; the archived patch
and build logs identify its source. Default/610: 486 library tests pass. v615:
489 pass, one expected ignore. Strict all-targets v615 Clippy passes with the
repository's existing `unnecessary_cast` and `field_reassign_with_default`
allowances. Release build, diff whitespace checks and graph update pass.

L6 boots the stock Linux 615 driver with GPA aliases and no guest DMA helper.
The first test command stops because this image does not automatically create
`/dev/nvidia*`; driver initialization itself succeeds. After creating the device
nodes using the majors registered in `/proc/devices`, CUDA returns correct data
and the NVIDIA-only Vulkan copy verifies all 4096 bytes. The one CUDA smoke run
reports 61 us/launch; this uncontrolled observation does not establish a latency
improvement or explain the difference from L5's 25 us. No new host Xids.

B11 uses the same binary with host-backed Windows execution. Code 43 and fresh
DxgKrnl 549/457 `0xC000009A` recur; no invalid RPC schema rejection is logged.
The transport fixes therefore do not resolve the observed Windows start failure.
Raw evidence: `L6-rpc-final/`, `windows/B11-rpc-final/`, `v2-rpc-*.log`.

## Windows remains blocked at driver initialization

B5 (virtual display) and B6 (headless) both end at Code 43. B7's temporary
four-KGR-getter refusal and B8's below-4-GiB VRAM placement do not fix it.
The temporary KGR diagnostic was removed. B9 confirms DxgKrnl-Admin 549/457:
`0xC000009A`, reason 1; PnP ProblemStatus = 0. This generic status does not
identify which driver allocation/check failed.

B10 bounds the reserved FB region to the actual owned carveout/pool rather
than including the unassigned tail of the physical card. Tests cover both
wire formats and different slots. Windows still ends at Code 43. This is a
metadata correction, not a demonstrated Windows startup fix.

The last class-0x70 allocation before teardown supplies a 40-bit limit
`0xffffffffff`, which is echoed unchanged. Its immediate free alone does not
prove that this allocation failed. Avoid inventing reply semantics from that
sequence alone. Earlier ETW/DbgPrint work yielded generic statuses; further
Windows work needs a more specific failing check or stack.

No Windows NVIDIA hardware rendering, desktop, 3DMark or game success is
claimed. The own D3D11 probe rejects software adapters and verifies rendered
pixels; it cannot pass while the NVIDIA adapter is absent from DXGI.

## Personality

The same Windows boot reports `Leandro VFIO RTX 2070-1G` as VideoProcessor
and `NVIDIA GeForce RTX 2070` as PnP Name. RM identity and the installed
Windows device description are separate. No alternate-chip workload support
is established by changing the name or PCI IDs.

## Additional open compatibility issue

`nvml.rs::fb_get_info_v2` and the recorded fallback currently substitute full
profile capacity for heap/free/usable-size queries. The VM's usable heap
excludes the carveout and optional pool; current free space additionally needs
allocation accounting. Separate capacity from usable/free/mappable quantities.
These indices were absent from the observed Windows startup query, so this is
not evidence for its Code 43 cause.

No external nvkvm/Kayfabe code or benchmarks were imported. GPU probes are
original project tests. Full alpha, desktop and cross-card claims remain gated
by [alpha-gates-2026-09-27.md](alpha-gates-2026-09-27.md).
