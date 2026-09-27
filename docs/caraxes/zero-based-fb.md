<!-- SPDX-License-Identifier: MIT -->
# Experimental zero-based framebuffer

`CARAXES_FB_ZERO_BASED=1` presents the owned framebuffer at guest PA zero while
retaining the physical host reservation. It targets Windows drivers that build
WDDM segments from a zero HEAP_START despite a high emulated usable region.
The established identity-addressed mode remains the default.

- Requires `--boot-model --reserve-vram --host-exec`, GR shadow execution and CE
  shadow execution. Raw GR replay and token-less doorbell ioeventfd are refused.
- Guest FB address `p` maps to offset `p` in the owned reservation descriptor.
  Shadow mappings and CPU VRAM windows use the same checked guest range.
- BAR roots, software physical copies and BAR1 pool bookkeeping use guest
  coordinates. BAR1 exports use translated host addresses; pinned window offsets
  remain relative to the physical reservation. Fixed BAR1 passthrough is refused.
- The card-sized guest address space remains unchanged. Fake WPR bytes above the
  owned guest range stay in host RAM and cannot be mapped by shadow engines.
- Raw page-directory adoption, physical invalidation and raw channel creation
  are refused. Filtered host memory-health reports are translated after filtering
  to the host reservation.

This is a compatibility experiment, not a supported Windows desktop profile.
Boot, rendering, video and multi-guest validation must be recorded separately.

Validation: final ABI 615 library tests passed (498 passed, one ignored);
ABI 610 library tests also passed. `cargo check` and the release build passed. Clippy passed with the existing
`unnecessary_cast` and `field_reassign_with_default` warnings allowed; strict
Clippy still reports those pre-existing patterns.

## Windows BAR1 pool trial

The Windows host launcher accepts `WIN_BAR1_POOL_MIB=64` in `exec` mode to
opt into the existing BAR1 mmap pool. It remains off by default. QEMU uses
a fixed sparse mmap of the owned pool; Caraxes updates page contents and
shadow mappings as guest BAR1 mappings change. Fixed passthrough is still
refused in zero-based mode.

The KD7 trace contains long byte-at-a-time VRAM clears through trapped BAR1
accesses. This makes the pool a performance candidate, not a demonstrated
Windows optimization. Validate initialization, alias coherence and mapped
ranges before comparing CPU usage. Use `CARAXES_TRACE_ACCESSES=0` for timed
runs; keep control/event logs. Reserve enough host BAR1 for the pool, VRAM
windows and the configured free-space floor.
