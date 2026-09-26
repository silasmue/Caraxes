<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# VA alloc bursts at a multi-client teardown (2026-09-26)

## Verdict

- The 31-line bursts (`virtmemAllocResources: VA Space alloc failed! 0x51`) are the shadow's own fixed-VA
  `NV50_MEMORY_VIRTUAL` allocations (`shadow.rs:1392` region object, `:1466` per-leaf fallback).
- The cause is the registry, not a shared VAS and not a stale mapping. The guest frees the client that
  **owns** the dup'ed `FERMI_VASPACE_A` first. `Registry::record_free` (`channel.rs:742-753`) drops that
  VAS and its root. The channels of the other client, which reach the VAS through a DUP alias, then
  resolve to no root (`vas_key_of`, `channel.rs:422`). As a result:
  - `exclusions_of(root)` (`mirror.rs:190`) returns nothing: set B is empty while the host TSGs still stand;
  - `held_at_free` (`mirror.rs:236`) holds nothing either: the owner client has no channels, and later
    channel frees find no root.
- The next resync of that root (here the BAR1 pool's, 13 ms later) maps the guest's leaves at 0x120000000
  and up, where the still-live host GR contexts sit. The host RM refuses every allocation.
- This is the same collision 604a33f4 fixed, reached by a path it does not cover. A single-client
  session has no DUP'ed VAS, so it stays clean.
- Harm today: none visible. Host Xid 0. The refused leaves are the guest's own set B, which is never
  meant to be mirrored, and the root is torn down about 230 ms later. The cost is 31 host log lines, about
  30 RM calls, and 22 map-failed / 779 unmapped leaves per iteration.
- Latent risk: while the owner's free leaves the root with no exclusions, a new GR channel built on that
  root mirrors set A over the host's context VAs. That is the display-e.md Xid 31 case.

## Evidence

- Device clock = host time. Device start 04:31:23 (`gpu.pid`); bursts at device 375/533/689/846/1003 s =
  host 04:37:38 / 04:40:16 / **04:42:53** / **04:45:29** / 04:48:07. Each burst has 31 host lines, 25 device lines.
- VAS ownership (`/mnt/vmstore/caraxes/vm4-615/gpu-trace.log`, 04:42:53 iteration, root 0x135677000):
  ```
  539808868 alloc 0x90f1 object 0xfade0003 (client 0xc1d000b1): vaspace recorded
  539826748 ctrl 0x90f10106: vaspace 0xfade0003: root Fb(0x135677000)
  539852604 dup 0xc1d000b4/0xbeef0300 <- 0xc1d000b1/0xfade0003 class 0x90f1; vaspace alias
  540003533 alloc 0xc46f 0xbeef0100 chid 8 ... vaspace (0xc1d000b4, 0xbeef0300)
  540233907 shadow: host-managed VAS 0x80002ad7 for guest root 0x135677000
  ```
- Teardown order and burst:
  ```
  689793974 FREE hRoot=0xc1d000b1 hObject=0xc1d000b1     <- the VAS owner client
  689797078 FREE hRoot=0xc1d000b3 hObject=0xc1d000b3
  689807289 shadow: sync ... (5063 set-A, 0 set-B excluded, ...) 22 failed ... 9 region(s) refused
  689807295 shadow: region 0x120000000 (Small) object refused 0x1a; per-leaf objects there
  689807313 shadow: map va 0x120400000 -> Fb(1e4e00000) size 0x200000 refused: per-leaf VA object 0x1a
  689807345 shadow: 779 leaf/leaves left unmapped after 4 refused per-leaf objects in their region
  689905570 free 0xcef80000: tsg 0xc1d000b4/0xcef80000 (grpID 10): host TSG 0x80002ae1 (engineType 0x1) freed
  690017004 ... (grpID 30) host TSG freed; shadow: VAS 0x80002ad7 ... handed to the worker
  ```
  - The host GR TSGs were live 98-210 ms after the refusals.
  - No "held out of set A" note appears for the b4 channel frees (689.84 s), because `root_of` no longer
    resolves.
- The same signature appears in all five bursts. Each time the VAS-owning client (0xc1d00068, ..8f, ..b1,
  ..d3, ..f5) is freed about 15 ms before the refused sync. Every teardown summary of those roots reads
  `22 map-failed ... 9 refused`.
- Committed trace: `caraxes/runs/nvenc-2026-09-25/vm4-615-concurrency/F6/p3-during-vk-trace-20s.txt.xz`
  (decompressed line 49279). FREE of clients 0xc1d000ce and ..d0, then `0 set-B excluded ... 22 failed`,
  with the same region list.
- Burst shape = `ShadowVas::apply`. Each region makes 1 refused region object plus up to
  `LEAF_OBJECT_FAILURES_PER_REGION` = 4 refused per-leaf objects (`shadow.rs:105`). Small regions carry
  pageSzLockMask 0x211000, Huge 0x200000, and 64K/4K leaves carry 0x10000/0x1000.
- The kernel's 0x51 reaches the device as 0x1a. `hostfloor::is_exhaustion` (`hostfloor.rs:58`) is false,
  so the soak line shows `no_memory=0` and the per-leaf fallback runs.
- The exclusion change was not logged. The pool's resync notes are dropped unless they contain
  REFUSED/ERROR (`bar1pool.rs:1847-1851`), so the "8 released back to set A" line never reached the trace.

## Ruled out

- One host VAS shared by several guest roots: no. `Mirror::shadows` is keyed by guest root
  (`mirror.rs:537`; execution.md "One shadow VAS per guest root"). The refused VAS 0x80002ad7 serves
  root 0x135677000 only.
- A stale shadow mapping outliving its guest owner: no. `regions reused 0, pooled 0`, `unmaps ... 0 failed`.
  The occupant is the host RM's own GR context of live host TSGs (grpID 10/26/30).
- Host VRAM exhaustion or the floor: no. The device sees 0x1a, and `floor_refused=0 no_memory=0`.
- Video layout or host-area placement: no. This root has the default layout (a GR root).
- The token collision (cuda-vulkan NOTES) is a separate bug. Its "~14 s after teardown" bursts are
  consistent with this mechanism, but that session's trace was not re-read here.

## Consequence for the channel

- The device neither retries nor falls back to another path. Refused leaves go to `ShadowVas::failed`
  and are retried only when the guest changes them. With 4 per-leaf failures the rest of the region is
  skipped (779 leaves). The host's mappings are untouched, because a refused fixed allocation cannot
  overwrite them.
- In this workload the channels are already idle and are freed within 230 ms, so no work runs on the
  missing leaves (host Xid 0).

## Fix proposal (smallest blast radius: `channel.rs` `record_free`)

- Model the guest RM's refcount. A VAS object freed with its owner client stays alive while a DUP
  alias of another client points to it.
- In `record_free`, for each dropped `vaspaces` key that is the target of a surviving alias (alias
  client != freed client):
  - move its `vaspaces`, `root_entries`, `vas_params` and `server_pdes` records to the first surviving
    alias key;
  - repoint the other aliases to that key;
  - drop the alias entry of the new owner;
  - return a note ("vaspace ... owner freed; kept under alias ...").
- About 20 lines. `vas_key_of`, `exclusions_of`, `held_at_free` and the invalidate routing then keep
  working unchanged. The 604a33f4 hold/release path then covers this teardown as well.
- Side fix (log only): forward pool-resync notes that contain "set-B" to the trace (`bar1pool.rs:1847`).
- Rejected alternative: holding the exclusions in `Mirror::free_object` for a client free. It fixes set B
  but leaves `vas_key_of` broken for later builds and frees of the surviving client.

## Test proposal

- Unit (`channel.rs` or the `mirror.rs` tests, beside `set_b_is_released_only_after_the_host_tsg_free`):
  `set_b_survives_the_vas_owner_client_free`.
  1. Client A allocs VAS (A,0x3) with root R.
  2. DUP it into client C as (C,0x300); C has a GR channel on (C,0x300) with promote entries.
  3. `record_free(A, A)`.
  4. Assert `vas_root(vas_key_of(ch)) == Fb(R)`, and that `exclusions_of(R)` equals its value before the free.
  5. Assert `held_at_free` for C's channel yields the ranges on R.
  6. Freeing C as well removes everything.
- GPU cell (vm4-615, one VM beside the desktop): the looping multi-client iteration that produced tonight's
  bursts (the concurrency F6 `p3-during-vk` shape, at least 5 iterations).
  - Pass: 0 host `virtmemAllocResources` lines; 0 `object refused` lines in the device trace; every
    `VAS of guest root ... freed` summary shows `0 map-failed`; "held out of set A" notes appear for
    the surviving client's GR channels; host Xid 0.
