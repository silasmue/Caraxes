<!-- SPDX-License-Identifier: MIT -->
# Windows findings

Running list for the Windows 11 guest (stock 617.14) on the synthetic TU106. One entry per
finding: evidence, fix, and the parts of the memory model it depends on, so each one can be
re-checked on the per-VM compressed VRAM slice (D16b).

Memory-model parts: **R** reservation (today one uncompressed host object), **L** shadow VAS
layout (vaBase, VAS start, internal range), **K** kinds and comptags, **Z** ZBC, **AB** set A/B
(guest leaves vs the host's own buffers), **P** BAR1 pool.
Runs: `/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/<run>/`.

## P17 (2026-09-28 08:15, P17-owned-compute-cta)

- **A lower-level COPY_SERVER_RESERVED_PDES was installed as the VAS root.**
  - Windows sends the control a second time with one PD1-level table (shift 29, range 1..0).
  - The shadow then walked a PD1 as the PD3, the GPFIFOs were never mapped, and the host took
    Xid 31 FAULT_PDE.
  - Fix 1, 4fac4c61: a call whose level 0 is not a root keeps the root. P18 confirms: 8 calls kept.
  - Depends on: L (the root the shadow walks), AB.
- **Nothing noticed when the shadow walked a root the guest no longer uses.**
  - Fix 2, 11de8415 (pdbwatch.rs): the guest's TLB invalidates name the real root; a VAS walked
    from a former root holds its host doorbells.
  - Replaying P17's trace flags all four wrong roots. P18: silent, no false positive.
  - Depends on: L.
- **MEM_OP_C TLB_INVALIDATE_PDB 0:0 was decoded inverted** (ONE is 0, clc46f.h:134).
  - Before, a single-PDB invalidate counted as ALL, and an ALL invalidate was dropped as PDB 0.
  - Fixed in 11de8415. Forwarding still invalidates every mirror VA space per MEM_OP; forwarding
    only the named root is open (needs a run).
  - Depends on: L.
- **Malformed table entries were followed** (a wrong root yields garbage 2 MiB leaves at 2^40+).
  - Fix 3, 7c1ce283 (ptcache): not followed; a root with ≥ 16 of them maps nothing. P18: none seen.
  - Depends on: K (kind check), AB.

## P18 (2026-09-28 19:34, P18-root-kept, beside Linux B)

- **Guest VAs below 1 MiB could not be mirrored.**
  - The shadow VAS had base 0 without ALLOW_ZERO_ADDRESS, so the RM starts it at 1 MiB
    (gpu_vaspace.c:1154, g_gpu_vaspace_nvoc.h:789) and refuses lower fixed VAs (0x1f).
  - Windows maps pushbuffers there (0x10000-0x13000, 0xbf000-0xcf000). First Xid: PBDMA
    FAULT_PDE at 0x13000; the FE fault at 0x10000 is the same class.
  - F1, 8f7a7f38: the default layout sets ALLOW_ZERO_ADDRESS. P19: every shadow VAS starts at 0,
    no refused leaf below 1 MiB, no low-VA fault.
  - Depends on: L.
- **The host's own buffers collide with Windows kernel data at 0x1_2000_0000 and up.**
  - Both RMs place kernel buffers at the lowest free VA ≥ 4 GiB outside the server-RM range
    [4 GiB, 4.5 GiB) (virt_mem_allocator_gm107.c:2771-2775), so the host's set B and the guest's
    kernel data both start at 0x1_2000_0000.
  - 190 guest leaves were refused there with 0x51 (host already owns the VA). FE FAULT_PTE at
    0x1_200ec000 (inside the guest's BUNDLE_CB) and 0x1_2037f000.
  - Open. VA_INTERNAL_LIMIT does not help on a GSP-client host: RM forces the internal range to
    [4 GiB, 4.5 GiB) (vaspace_api.c:374-379), which the host reserves for its own GSP
    (gpu_vaspace.c:410-417, `_gvaspaceReserveVaForClientRm`).
  - Candidate: a host-owned fence over the guest's kernel range at VAS creation, with guest
    leaves there mapped into the fence (touches the shadow's region objects).
  - Depends on: L, AB.
- **Fault attribution needed a guest-table dump.** F3, 8f7a7f38: each MMU-fault RC of a shadow
  channel logs the fault page's guest leaf, shadow leaf and set-B/private hits
  ("shadow fault diagnostic"). Depends on: L, AB.
- **The host RCs now reach the guest at once** (4d6ab0ee host notifier page): 4 in P18 instead of
  fence timeouts. Depends on: none.
- **The bar1 pool refuses remaps of guest page-table pages** (P17 and P18 gpu.log ERROR lines).
  Not yet attributed to a failure. Depends on: P.

## P19 (2026-09-28 19:55, P19-zero-base, beside Linux B)

- **F1 holds:** 6 shadow VASes start at 0, no refused leaf below 1 MiB, no low-VA fault.
- **The set-B exclusion of a size-0 global buffer swallows guest pages.**
  - The first Xid (19:56:10) was FE FAULT_PTE write at 0x1_2010c000. Then 0x1_200ec000: 3x
    FAULT_PTE and 1x FAULT_PRIV_VIOLATION. Stopped at 19:57.
  - Both VAs are exactly BUNDLE_CB + 0x3000: the promote entry 3 (size 0) is at 0x1_20109000,
    resp. 0x1_200e9000.
  - F3 on roots 0x3d485000 and 0x3d471000: the guest maps a 4 KiB sysmem page there (kind 0);
    the shadow has a gap; "set B true".
  - `promoted_ranges` sizes a size-0 entry by the host's engine context size rounded up to 64 KiB
    (mirror.rs `global_ctx_size`). The guest packs its next allocation (semaphores; P18 had CE
    semaphores at +0x5000) right after its own reservation, so the exclusion overshoots into
    guest data.
  - The PRIV_VIOLATION shows the write is non-privileged (pushbuffer-driven), and that the host
    placed a privileged buffer of its own in that excluded hole in one VAS.
  - Not attributed yet: the host's BUNDLE_CB size (engine 0x11 is in no recorded answer), and
    whether the guest leaves [base, base + 0x3000) unmapped.
  - Open. Depends on: AB, L.
- **Host-owned VAs in the guest's kernel range remain:** 64 host VA allocations refused with 0x51 in
  0x1_2000_0000..0x1_2200_0000 (P18: 192). No Xid attributed to them in P19. Depends on: L, AB.

### F2 designs (not built; main decides)

- **F2a, exclusion extent (small, mirror.rs only):** a size-0 promoted global buffer excludes only
  up to the first page the guest itself maps at or after its base. The guest never maps inside a
  VA-only reservation that the server fills. Needs the cached guest walk where the exclusions are
  recomputed (`refresh_exclusions`). Fixes the P19 faults if the guest's reservation is what it maps
  around. Depends on: AB.
- **F2b, host fence (larger, touches shadow.rs `apply`, workstream A's neighbourhood):**
  - A host-owned `NV50_MEMORY_VIRTUAL` over [0x1_2000_0000, 0x1_2000_0000 + F) is created with the
    shadow VAS.
  - The host's first fit then lands above it. Guest leaves inside it are mapped into the fence
    (hDma = fence, fixed offsets) instead of per-region objects; the fence is never freed while the
    VAS lives.
  - Removes the 0x51 collisions and the host's buffers in guest holes.
  - Guest user VAs above 0x1_2000_0000 + F can still meet the host.
  - Depends on: L, AB; the compressed slice (D16b) must keep kinds/comptags per mapped leaf inside
    the fence (K).
