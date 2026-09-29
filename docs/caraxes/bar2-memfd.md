<!-- SPDX-License-Identifier: MIT -->
# BAR2 in host RAM: the memfd area behind `CARAXES_BAR2_MEMFD=1`

Written 2026-09-24 for D18's line "the guest's page tables do not have to live in VRAM, because
the GPU only reads the shadow" (README.md, D17 and D18). The target is the 96k trapped BAR2 writes
of a context creation (mapping.md §3). The code is in `crates/vfio-user-nvgpu`:

- `bar2area.rs` (new): the memfd, the remap table, the dirty scan and the sync;
- `fb.rs`: the translation hook `FakeFb::backing` and the read, write and sync paths;
- `boot.rs`: the sync points;
- `device.rs`: region 3 as an mmap-able region;
- `main.rs`: setup, fail-closed.

`shadow.rs`, `mirror.rs`, `ptcache.rs`, `msgq.rs`, `tokens.rs`, `poll.rs`, the display code and
`patches/` are unchanged. Written before any VM or GPU run; the measurement plan
for the coordinator is in §7; the measurement is §8.

Sources:

- NVIDIA open kernel modules 610.57.04, `vendor/open-gpu-kernel-modules/src/nvidia/src/kernel/gpu/bus/`
  (`kern_bus_vbar2.c`, `arch/maxwell/kern_bus_gm107.c`, `arch/volta/kern_bus_gv100.c`).
- Cloud Hypervisor with patch 0005, `/mnt/vmstore/caraxes/ch-0005/pci/src/vfio_user.rs`.
- The kernel headers of the running host kernel, `/usr/lib/modules/7.2.4-arch1-2/build/include`.
  The host has no kernel `Documentation/` tree: not under `/usr/share/doc`, not in the build tree,
  and not in the Nix kernel trees in `/nix/store`. So `Documentation/admin-guide/mm/soft-dirty.rst`
  and `userfaultfd.rst` could not be quoted. §2 cites the uapi headers and the kernel
  configuration instead, and marks the one claim that rests on those documents from memory.
- Device traces in `caraxes/runs/`.

## Design in five sentences

1. Region 3 (the 32 MiB BAR2 aperture) is a sealed memfd with sparse mmap areas over all of it
   except one trapped page. Guest BAR2 offset `o` is memfd offset `o`, so the guest RM's
   page-table, instance-block and scrub writes land in host RAM without an exit.
2. BAR2 is a virtual aperture. While an FB page is mapped through the guest's BAR2 page tables,
   its current content lives in the memfd page of the BAR2 offset that maps it. The remap table
   `bar2area::RemapTable` records that, and `FakeFb::backing(pa)` is the single translation: the FB
   store reads such a page from the memfd, and a device write goes to VRAM and to the memfd.
3. The device cannot see guest writes through the VMM's mapping. So at the guest RM's own
   synchronisation points, which stay trapped, it compares every mapped memfd page against its
   snapshot:
   - the BAR2 flush read;
   - the MMU invalidate;
   - the BAR2 bind;
   - the GSP command-queue doorbell.
4. A sync then does three things:
   - it writes back to VRAM exactly the bytes the guest changed;
   - it marks those FB pages dirty in the shadow's walk caches, as a trapped write used to (the
     incremental walk then re-reads them from the memfd);
   - it follows the BAR2 page tables with a cached walk: a page that leaves the BAR2 mapping is
     already written back, and a page that enters it is loaded from VRAM into its memfd page.
5. If the memfd cannot be created, sealed or mapped, or a write through a second mapping is not
   seen by the device's mapping, the device logs an ERROR and starts with region 3 trapped as
   before. The same happens without `--boot-model` or on a GMMU v3 chip.

## 1. How it works

**What the guest RM does with BAR2** (610.57.04, Turing, GSP client). The CPU RM keeps a cache of
BAR2 mappings of memory descriptors. `kbusMapBar2ApertureCached_VBAR2` (`kern_bus_vbar2.c:529-715`)
does the following:

- it reuses a cached mapping, or allocates BAR2 VA from an eheap (persistent mappings top-down,
  `:598-601`);
- it writes the BAR2 PTEs with `kbusUpdateRmAperture_HAL(..., UPDATE_RM_APERTURE_FLAGS_INVALIDATE)`
  (`:693-695`).

That function (`kern_bus_gm107.c:2558`) writes the PTEs through the BAR2 page tables' own BAR2
mapping. Then it runs these steps in order (`:2757-2785`):

1. `osFlushCpuWriteCombineBuffer()`;
2. `kbusFlush_HAL`, the flush read;
3. `kgmmuInvalidateTlb_HAL` on the BAR2 PDB.

An unmap writes nothing (`:2614`, "Currently don't do anything at unmap"), so a cached mapping
stays valid until its VA is reused. The flush read is `MEM_RD32(pKernelBus->pReadToFlush)`
(`kern_bus_gv100.c:392-395`). `pReadToFlush` is a persistent BAR2 mapping made once at boot
(`kern_bus_gv100.c:85-87`). The RM's contract for it is stated at `kern_bus_gm107.c:3339-3342`:
"Properly flush written PDEs, PTEs, or other instance memory data or context buffers ... Must
call kbusFlush BEFORE any calls to busInvalidate".

**What the traces show** (`caraxes/runs/mapping-2026-09-24/p3-profile/init-trace.txt`, one
`bench-cu init`):

- 96665 BAR2 accesses: 96193 writes, and 472 reads.
- All 472 reads are of BAR2 0xfea000, the flush page. The same offset is the only BAR2 read in
  `c0-doorbell-study/short-trace.txt` and in every `display-e-2026-09-24` trace.
- The writes touch 77 BAR2 pages. No BAR2 page was re-pointed at another FB page during the
  context creation.
- One FB page (0x1e8d55000) is reached through two BAR2 pages (0x2d000 and 0x2e000): two
  sub-page allocations, each mapped by its own descriptor.
- The BAR2 PT page is self-mapped at BAR2 0xfef000 (FB 0x1e8de6000). A PTE write there is
  followed by the flush read and by `NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE` with PDB
  0x1e8e00000, the BAR2 root in the carve-out (trace lines 5029-5037).

**The remap table** (`bar2area.rs` `RemapTable`, `set` at `:397`). It holds two maps:

- BAR2 page -> key, where a key is an FB page, or a guest sysmem page tagged with bit 63;
- key -> `Live { vas, snap }`, where `vas` are the BAR2 pages mapping it and `snap` is the
  device's copy of the content as of the last sync.

`set(va, new)` behaves as follows:

- It unlinks the old key. A key whose last BAR2 page goes has *left*.
- If the new key is live already, it is an *alias*: the snapshot is copied into the new BAR2 page.
- Otherwise the new key is *loaded*: its content is read from the FB store (VRAM through the BAR1
  windows) or guest RAM, and written into the memfd page and the snapshot.

**The translation hook** (`fb.rs:635` `Backing`, `:866` `FakeFb::backing`):

- `Backing::Bar2Memfd(off)` while the page is live, else `Backing::Store`.
- `FakeFb::read` goes through it (`:1052`). So the walks, PRAMIN reads, the mirror's instance and
  RAMFC reads, and the BAR TLB all see the guest's current BAR2 writes, at RAM speed.
- `FakeFb::write` writes the store as before and then the memfd copies and snapshots
  (`:1068`, `RemapTable::device_write` `bar2area.rs:521`). So a PRAMIN write or an RPC's write is
  seen by the guest through BAR2, and is not later mistaken for a guest write.
- `watched_dword` reads a memfd-backed dword directly instead of caching it.

The other agent's BAR1 pool (bar1-passthrough.md §4 H) adds its own variant to the same enum and
the same function.

**The sync** (`bar2area.rs:755` `Bar2Area::sync`, `fb.rs:932` `FakeFb::bar2_sync`):

1. *Scan* (`RemapTable::scan`, `:451`): each live page's memfd copies are compared with the
   snapshot and merged byte by byte. The first alias to change a byte wins, and a clash is
   counted. The merged bytes are copied to the other aliases and become the snapshot.
2. *Write-back*: each run of changed bytes is written to VRAM (or guest RAM), then the store's
   write-combining buffers are drained.
   - Only changed bytes are written. The RM writes 32-bit values next to each other, and the GPU
     may have written the neighbouring words of the page in VRAM; a whole-page write-back would
     overwrite them.
3. *Caches* (`FakeFb::note_pages_changed`, `fb.rs:878`). For each changed FB page:
   - the shadow's walk caches `mark_dirty` it (ptcache.rs), so the next incremental update
     re-reads that page and nothing else, exactly as after a trapped write;
   - the BAR TLB flushes if it walked through the page;
   - watched dwords on the page are dropped;
   - the P21 check re-walks if the page is one of its BAR1 table pages.
4. *BAR2 tables* (`follow_tables` `:804`, `apply_unit` `:879`). A `ptcache::WalkCache` of the
   BAR2 root does the following:
   - it is marked dirty by the scan (guest writes through the self-mapping) and patched by
     `note_write` (PRAMIN writes during the BAR2 bootstrap, and the device's own `UPDATE_BAR_PDE`
     root write);
   - it re-reads its dirty table pages and reports the changed 2 MiB units;
   - each BAR2 page of a changed unit inside the aperture, except the trapped pages, is re-pointed
     with `set`.

   The scan runs first, so a page that leaves the mapping has nothing left to write back.

**Sync points** (`boot.rs`):

| Where | Line | Why |
|---|---|---|
| a trapped BAR2 access (the flush page), before it is answered | `:358-362` | the RM's flush: "BAR2 writes visible to the GPU" |
| `NV_PFB_PRI_MMU_INVALIDATE` (any PDB), before the mirror's invalidate and resync | `:654-659` | every BAR2 and page-table update ends in one; the shadow's resync after it needs the dirty pages |
| `NV_PBUS_BAR2_BLOCK` write | `:621-626` | BAR2 (re)bound: its tables are live from here on |
| GSP command-queue doorbell | `:717-720` | an RPC may name memory the guest filled through BAR2 |
| every usermode doorbell, only with `CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1` | `:636-637`, `:746-750` | the reference for the rule of §3 |

Since 2026-09-29 (`bar2area.rs` `Point`, `boot.rs` `bar2_doorbell_sync`), the points are named after
the card's registers and counted in the exit line (`full syncs by point: ...; channel doorbells: ...`):

| Point | Register (published headers of 615) | What starts consuming |
|---|---|---|
| flush read | `pReadToFlush`, FB address 0 through BAR2 (kern_bus_gv100.c:384-399) | nothing; the flush itself (a read completes after every earlier posted write) |
| FB flush | `NV_UFLUSH_FB_FLUSH` 0x070000 (maxwell/gm107/dev_flush.h:30); semantics in kern_bus_gh100.c:2946-2950 | nothing; the sysmembar. Treated as a flush read: owed, or synced at once without the deferral |
| MMU invalidate | `NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE(_PDB)` (turing/tu102/dev_vm.h:120-131) | the MMU (page tables) |
| GSP queue head | `NV_PGSP_QUEUE_HEAD(0)` (turing/tu102/dev_gsp.h:38) | GSP-RM; on a GSP system the runlist submit is GSP-RM's own write, and the CPU's equivalents (channel/TSG alloc, `NVA06F_CTRL_CMD_BIND`, `GPFIFO_SCHEDULE`, `FIFO_DISABLE_CHANNELS`) are RPCs through this queue |
| BAR2 bind | `NV_PBUS_BAR2_BLOCK` | the BAR2 MMU |
| channel doorbell | `NV_VIRTUAL_FUNCTION_DOORBELL` (turing/tu102/dev_vm.h:229) | the channel's PBDMA and engines: rule 4 of §3 |
| function-level reset (`FakeFb::carry_bar2_area`, `fb.rs:958`) | `:456` | write back, then hand the memfd to the fresh model with nothing mapped |

**Exit summary** (`boot.rs:309`). One line:

`bar2 memfd: BAR2 [0, 0x2000000) in host RAM, trapped page(s) [0xfea000] with N access(es); S sync(s) (T us total, M us max), C page compare(s), D dirty page(s), B byte(s) written back, K alias conflict(s); W BAR2 table walk(s), E page(s) entered (L loaded), X left; now P page(s) mapped at V BAR2 page(s), at most Q live and A alias(es) of one page; Y sysmem write-back(s) skipped`

**Switches.**

- `CARAXES_BAR2_MEMFD=1` turns it on.
- `CARAXES_BAR2_MEMFD_TRAP` lists the trapped pages: comma-separated page-aligned offsets, or
  `none`. The default is `0xfea000`.
- `CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1` also syncs before every usermode doorbell.

## 2. Dirty tracking: which mechanism sees guest writes, and at what cost

The guest writes a BAR2 page through KVM. Cloud Hypervisor maps each sparse area with
`mmap(MAP_SHARED, fd, region offset + area offset)`, then registers that mapping as a KVM memory
slot: `create_user_memory_region(slot, BAR base + area offset, len, host address, readonly false,
log_dirty false)` (`ch-0005/pci/src/vfio_user.rs:299-388`, the call at `:372-379`). A guest store
therefore goes through KVM's stage-2 entry. That entry was made from a page-table entry of the
**VMM process**, for the shmem page of the memfd. The device holds a different mapping of the same
shmem pages in its own process. Every per-mapping tracking mechanism sees only writes through the
mapping it is applied to.

| Mechanism | Applied to | Sees guest writes? | Cost |
|---|---|---|---|
| soft-dirty bits: `clear_refs` 4, then `/proc/PID/pagemap` bit 55 or `PAGEMAP_SCAN` with `PAGE_IS_SOFT_DIRTY` (`include/uapi/linux/fs.h:462-472`; `CONFIG_MEM_SOFT_DIRTY=1`, `include/generated/autoconf.h:2786`) | the device (`/proc/self`) | **no**: the bits live in the device's own page-table entries, which the guest's stores never touch | - |
| same | the VMM (`/proc/<CH pid>`) | yes: KVM's write fault on the VMM's entry sets the bit | `clear_refs` has no range. It clears and write-protects the whole address space of the VMM, including the 4 GiB of guest RAM, and every write-protect change reaches KVM as an MMU-notifier invalidation (`MMU_NOTIFY_SOFT_DIRTY`, `include/linux/mmu_notifier.h:34,56`). Every guest RAM page would fault again after each sync, hundreds of times per context creation. The range-less `clear_refs` is from `soft-dirty.rst` as remembered, not quoted here. Rejected |
| userfaultfd write-protect: `UFFDIO_REGISTER_MODE_WP`, `UFFDIO_WRITEPROTECT` (`include/uapi/linux/userfaultfd.h:264,311-324`), on shmem since `UFFD_FEATURE_WP_HUGETLBFS_SHMEM` (`:217-218,246`), async mode `UFFD_FEATURE_WP_ASYNC` for `PAGEMAP_SCAN` (`:226-229,249`; `fs.h:488`) | the device's VMA | **no**: the write-protect markers (`CONFIG_PTE_MARKER_UFFD_WP=1`, autoconf.h:5856) are per page-table entry of the registering process | a userfaultfd can only be registered by the process that owns the memory; it cannot be put on the VMM's mapping from outside. It also needs privilege on this host: `vm.unprivileged_userfaultfd` = 0 and `/dev/userfaultfd` is root-only (0600), so only `UFFD_USER_MODE_ONLY` would remain |
| `mprotect` read-only and SIGSEGV | the device's mapping | **no**: it protects only the device's own entries | - |
| KVM dirty logging: `KVM_MEM_LOG_DIRTY_PAGES`, then `KVM_GET_DIRTY_LOG` or `KVM_CLEAR_DIRTY_LOG` per slot (`include/uapi/linux/kvm.h:56,1264,1458`) | the VMM's BAR2 slot | yes, per page and per range, exactly the guest's writes | needs a Cloud Hypervisor change: set `log_dirty` for this slot and hand the device the bitmap, a vfio-user message the specification does not have (bar1-passthrough.md §2, §4 I). Not built; `patches/` is not touched |
| **snapshot compare** (built): compare each live memfd page with the device's copy | the device's mapping of the memfd | **yes**: it reads the shared pages themselves, so a write through any mapping is seen | about 0.25 us per clean page in a warm cache. Measured with a throwaway release-build test on this host under desktop load: 77 pages 19 us, 256 pages 65 us, 1024 pages 0.62 ms per scan (the 1024-page set, 8 MiB with snapshots, no longer fits the cache). Memory: one 4 KiB snapshot per live page |

**Choice: the snapshot compare**, run only at the sync points of §1. None of the three
mechanisms named in the task sees a guest write when applied to the device's own mapping. On the
VMM's mapping, soft-dirty would, but only at whole-address-space cost. Userfaultfd and
`mprotect` cannot be applied there at all. The comparison needs no privilege and no VMM change,
and it cannot miss a write. Its cost scales with the live set, not with the number of writes. The
trace's 77 pages cost about 20 us per sync. A context creation has one flush read and one
invalidate per BAR2 mapping update: 472 flush reads in the init trace, so about 1000 syncs, or
about 20-70 ms of device time, against the 2.2-3.6 s of trapped writes it removes (mapping.md §3).

`MemfdArea::self_test` (`bar2area.rs:282`) is the fail-closed check of this choice. Before the
area is offered, a write through a second shared mapping must be visible through the device's
mapping.

KVM's dirty log is the mechanism to move to if the live set grows or the sync points turn out too
frequent. It needs one Cloud Hypervisor change for the BAR2 slot, and it would replace the
compare: the scan would then read only the logged pages.

## 3. What stays trapped, and the rules the design relies on

**Trapped:**

- the flush page (default BAR2 0xfea000, `CARAXES_BAR2_MEMFD_TRAP`). It is the device's signal
  that "the RM wants its BAR2 writes visible". The device answers it from the FB store as before
  (`boot.rs:352-363`), after a sync;
- all of BAR0, including PRAMIN, which has side effects and a device-computed target, and the MMU
  invalidate, BAR2 block and doorbells;
- all of BAR1, unless P21 is on.

Nothing else in BAR2 is answered synchronously by the device: BAR2 is plain memory behind the
guest's tables. So no other page needs to trap.

**Rules** (each is the RM's own ordering, as on hardware):

1. A BAR2 PTE change is followed by a flush read and an MMU invalidate before the new mapping is
   used (`kern_bus_gm107.c:2757-2785`). The remap therefore happens before the guest touches the
   new page. A guest write to a BAR2 page before its mapping is known to the device would be
   overwritten when the page is loaded. On hardware such a write would use a stale TLB entry.
2. A page-table change is followed by an MMU invalidate before any engine uses it. The shadow is
   resynced at that invalidate (mapping.md §4), right after the sync, so it sees the change.
   So the invalidate is the sync point of the page tables: with no flush owed it compares and
   writes back exactly the live table pages the device follows (the walks of the guest roots,
   the BAR2 and BAR1 tables, the pool's area walk; `FakeFb::bar2_sync_tables`), and a BAR2 page
   that the walk re-points is written back before it leaves the mapping. With a flush owed it
   settles it with a full sync. A doorbell syncs only what its channel could observe (rule 4),
   page tables excluded; `CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1`, a full sync at every doorbell,
   is the reference setting that tests this rule. The channel's VAS leaves are taken as of the
   last walk update (the shadow's resync at the invalidate): a mapping the guest made without
   an invalidate is not guaranteed to reach the engine on hardware either.
3. BAR2 data the GPU consumes (instance memory, scrubbed buffers) is flushed by the RM before it
   is handed to the GPU (`kern_bus_gm107.c:3339-3342`), and reaches VRAM at that flush.
   Since 2026-09-26 the flush read only marks the sync owed, and the hand-over settles it: the
   MMU invalidate, the GSP command-queue doorbell, a channel doorbell, or the 1 ms flush clock
   ([display-pacing.md](display-pacing.md) §3; `CARAXES_BAR2_MEMFD_DEFER_FLUSH=0` syncs at every
   flush read again).
4. **The doorbell rule** (2026-09-29, for guests whose flush the device does not see: nouveau
   reads FB address 0 through its own BAR2 mapping at 0x8000, not at the trapped 0xfea000). A
   channel doorbell with no flush owed syncs the live pages the rung channels could observe, and
   only those (`Bar2Area::sync_pages`):
   - through the VAS: every leaf of the guest's tables under the channel's root, less the shadow's
     set-B ranges (the promoted context buffers the host builds itself). The GPFIFO is among them
     wherever it lives (`NV_RAMFC_GP_BASE` is a VA, gm107/dev_ram.h:72-73), in VRAM as the Windows
     KMD puts it or in sysmem;
   - by physical address: USERD (`NV_RAMFC_USERD`, tu102/dev_ram.h:29-30; the host channel of a
     mirror uses the guest's USERD page itself), the instance block with RAMFC and the method
     buffer (`instanceMem`, `ramfcMem`, `mthdbufMem`; `NV_RAMIN_RAMFC`, `NV_RAMIN_ENG_METHOD_BUFFER_ADDR`,
     tu102/dev_ram.h:27, 41-42). Runlist entries name the instance block and USERD by physical
     address (`NV_RAMRL_ENTRY_CHAN_USERD_TARGET`, :46-50); the runlist memory is GSP-RM's. The page
     tables (`NV_RAMIN_PAGE_DIR_BASE`, gm107/dev_ram.h:58-66) are read by physical address as well,
     but they are the invalidate's (rule 2), not the doorbell's (vm10-boot9: the doorbell set with
     the tables cost 9.7 us per doorbell, 25 pages compared, 6 dirty pages in 170455 doorbells).
   - A channel whose memory the device cannot name (no VAS root, no cached walk of it) could observe
     anything: every live page is synced.
   The sets are cached per channel (channel, root, walk and table generations, set-B stamp) and
   intersected with the live pages when those change. The cost is one `memcmp` of the BAR2 page
   with its snapshot per observable live page per doorbell (0.14 us warm, 0.28 us for a set that
   no longer fits the cache, measured on this host); KVM dirty logging (§2) would make it one per
   dirty page. Write-protect tracking on the device's side cannot replace it: userfaultfd-wp,
   soft-dirty and `mprotect` all act on the device's own mapping, which the guest's stores (KVM
   stage 2 through the VMM's mapping) never touch (§2).
5. **Reads** (2026-09-29, vm10-boot8). FB is one memory: a write through any aperture or engine is
   what the next read through any other returns. A page behind BAR2 can have other writers:
   nouveau clears a channel's USERD through BAR2 and writes GPPut through BAR1 (the BAR1 pool,
   straight into VRAM), and the host PBDMA writes GPGet there. vm10-boot8 read USERD from the
   memfd, saw GPPut 0, ran nothing, and nouveau's channel idle timed out after 15 s. The device now
   reads VRAM (the store where the pool puts the page) with the bytes the guest wrote through BAR2
   since the last sync over it (`Bar2Area::read_merged`: the memfd bytes that differ from the
   snapshot). Page-table reads (`FakeFb::read_table`: the walks, the BAR TLB, the pool's area
   walk) keep the memfd at RAM speed, which is exact for tables: no other writer (the pool never
   holds a table page, no host engine writes the guest's tables). The sync still writes back only
   the guest's BAR2 bytes, so GPPut and GPGet in VRAM survive it.
   Left open: the guest's own BAR2 reads of such a page see the memfd, not the other writers (the
   drift counter samples it); and a guest BAR2 write that is not yet synced lands after a later
   BAR1 write to the same bytes. No trace shows either for the page-table and instance memory the
   guests read back through BAR2.

**Hazards, counted or stated:**

- *Guest reads, through BAR2, of what the GPU wrote to VRAM later.* The memfd holds the content
  as of the page's load, plus guest and device writes. A GPU write after that is not visible
  through BAR2 until the page leaves and enters the mapping again. No trace has a BAR2 read other
  than the flush page. If a workload shows one, the answer is to trap that page:
  `CARAXES_BAR2_MEMFD_TRAP=0xfea000,<page>`.
- *Aliases.* Two BAR2 pages that map one FB page are merged at each sync. Between syncs, a read
  through one alias does not see a write through the other. The init trace has one such page,
  and it is only written. The exit summary counts aliases and byte clashes.
- *Sysmem through BAR2.* Supported: loaded from guest RAM, written back to guest RAM. Device paths
  that read that guest page directly see the guest's BAR2 writes only after the next sync. The
  traces show none.
- *Writes to unmapped BAR2 offsets.* They land in the memfd and are ignored. Before, they trapped
  and counted as `bar_faults`.
- *GMMU v3 (Hopper, Blackwell).* Refused: the BAR2 walk uses the v2 cache (`FakeFb::set_bar2_area`,
  `fb.rs:915`).
- *Isolation* is unchanged. The memfd is host RAM that only this VM maps. The device writes back
  only into FB pages the guest's own BAR2 tables name, through `FakeFb`'s bounds as before. The GPU
  still reads only the shadow tables, validated by `resolve_leaf` (shadow.rs).

## 4. Fail closed

`main.rs:425-429` and `:527-550` handle the failure cases:

- A malformed `CARAXES_BAR2_MEMFD_TRAP` ends start-up.
- The following cases start the device with region 3 trapped and one
  `ERROR: bar2 memfd: ...; starting without the area, BAR2 stays trapped` line:
  - no `--boot-model`;
  - `memfd_create`, the size, the seals (`F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_SEAL`, so the
    client can neither shrink the file, which would turn device accesses into SIGBUS, nor change
    the seals) or the mapping failed;
  - the self-test failed;
  - a GMMU v3 chip.
- `device.rs:265` `regions_with_bar2` offers region 3 as mmap-able only when every area lies
  inside the BAR.
- Cloud Hypervisor itself refuses to create the device if an area cannot be mapped ("Could not
  mmap sparse area", `vfio_user.rs:355-360`).

## 5. Tests and build state

- `bar2area::tests::sparse_areas_leave_the_trapped_pages_out`: the area layout, the trap-list
  parser, refused offsets.
- `bar2area::tests::the_remap_table_moves_pages_and_shares_aliases`: the remap table. Covers load
  on entry, an alias taking the snapshot without a load, a page staying live through its alias,
  leaving, sysmem keys, and device writes reaching every alias without looking dirty.
- `bar2area::tests::the_scan_sees_writes_through_another_mapping`: the dirty-page scan with a
  second shared mapping standing in for the VMM. Covers byte-exact write-back runs, alias
  propagation, merging different dwords of one qword, a same-byte clash, and a clean rescan.
- `fb::tests::bar2_memfd_follows_the_bar2_tables_and_feeds_the_walk_cache`: end to end on the fake
  FB.
  - BAR2 tables with a self-mapped PT, and a channel's tables mapped through BAR2.
  - A guest PTE write through the memfd is invisible until a sync. After the sync the walk cache
    is dirty, and its incremental update reads exactly one page, from the memfd, and finds the new
    leaf.
  - A guest re-point of a BAR2 page through the self-mapping moves the FB page back to the store,
    with the write written back, and loads the new page.
  - A device write reaches the guest's view without counting as dirty.
  - A function-level reset carries the area over with nothing mapped.
- `device::tests::bar2_memfd_makes_region_3_mmapable_around_the_trapped_page`: region 3 with
  sparse areas, alone and together with the P21 area, and refused when an area exceeds the BAR.

Run with `CARGO_TARGET_DIR=target/bar2`:

- `cargo build --release -p vfio-user-nvgpu`: clean.
- `cargo test -p vfio-user-nvgpu`: 166 unit tests plus 2, all pass.
- `cargo clippy --locked -p vfio-user-nvgpu --all-targets [--all-features] -- -D warnings -A clippy::unnecessary_cast -A clippy::field_reassign_with_default`: clean.
- `RUSTDOCFLAGS="-D warnings" cargo doc -p vfio-user-nvgpu --no-deps`: clean.

## 6. Expected effect

Per context creation (mapping.md §3: 96193 trapped BAR2 writes, 23-37 us each):

- BAR2 writes that trap: about 0. What remains trapped per BAR2 mapping update is the flush read
  (472 in the init trace) and the four BAR0 accesses of the invalidate. These trapped before as
  well.
- Added device work:
  - about 1000 syncs at 20-70 us;
  - one VRAM page read for each page entering the mapping. The 107 fresh table pages are read
    once each, at 16-byte streaming loads (mapping.md §1).
- cuCtxCreate is expected to drop by the 2.2-3.6 s the BAR2 write traps cost, toward the
  sub-second range. cuCtxDestroy is expected to drop by its 20k PTE-clearing traps, about
  0.5-0.7 s.
- Not expected to change: the doorbell path (4 traps per submission, P21 and patch 0005 territory)
  and the GR interrupt noise (mapping.md §3).

## 7. Measurement plan (coordinator, vm2, nothing run here)

vm2 alone at slot 0 with the mapping environment. Run the same series with the area off, then on.
The patch 0005 client is used as the operator asked. Stock v53 maps sparse areas the same way
(bar1-passthrough.md §2).

```sh
R=/home/silas/git/Caraxes          # the merged tree
(cd "$R" && CARGO_TARGET_DIR=target/bar2 cargo build --release -p vfio-user-nvgpu)
M=$R/caraxes/runs/bar2-memfd-$(date +%F); mkdir -p "$M"
source "$R/caraxes/runs/mapping-2026-09-24/env.sh"
export CARAXES_SERVER=$R/target/bar2/release/vfio-user-nvgpu
export LEA_CH=/mnt/vmstore/caraxes/ch-0005/target/release/cloud-hypervisor
W=/mnt/vmstore/caraxes/vm2
for mode in off on; do
  if [[ $mode == on ]]; then export CARAXES_BAR2_MEMFD=1; else unset CARAXES_BAR2_MEMFD; fi
  T0=$(date '+%F %T')
  "$R/caraxes/tools/bench-run.sh" v2-restart > "$M/$mode-up.txt" 2>&1
  grep -E 'bar2 memfd' "$W/gpu.log" | tee "$M/$mode-device-start.txt"
  # One context creation with its device trace: the trapped BAR2 accesses.
  L0=$(wc -l < "$W/gpu-trace.log")
  "$R/tools/caraxes-vm.sh" ssh 'cd ~/bench && LD_LIBRARY_PATH=/opt/nvrm/lib ./bench-cu init' | tee "$M/$mode-init-once.txt"
  tail -n +"$((L0 + 1))" "$W/gpu-trace.log" > "$M/$mode-init-trace.txt"
  awk '$3=="bar3"{print $2, $4}' "$M/$mode-init-trace.txt" | sort | uniq -c | sort -rn | head > "$M/$mode-bar2-traps.txt"
  awk '$3=="bar3" && $2=="W"' "$M/$mode-init-trace.txt" | wc -l | tee "$M/$mode-bar2-trapped-writes.txt"
  # cuCtxCreate/cuCtxDestroy (init), churn, with device CPU per run in the MEAS lines.
  "$R/caraxes/tools/bench-run.sh" series --targets v2 --reps 5 --metrics "init churn" --out "$M/$mode" 2>&1 | tail -3
  "$R/tools/caraxes-gate.sh" cuda > "$M/$mode-cuda-gate.txt" 2>&1; tail -5 "$M/$mode-cuda-gate.txt"
  "$R/tools/caraxes-vm.sh" down
  grep -E 'bar2 memfd|bar tlb|vram store|ERROR' "$W/gpu.log" > "$M/$mode-device-summary.txt"
  journalctl -k --since "$T0" | grep -c 'NVRM: Xid' | tee "$M/$mode-host-xid.txt"
done
python3 "$R/caraxes/tools/bench-summary.py" "$M/off/v2" "$M/on/v2" > "$M/summary.md"
```

**What to read:**

- `on-device-start.txt` names the sparse areas. `on-up.txt` shows no "Could not mmap sparse
  area".
- `on-bar2-trapped-writes.txt` should be 0 against about 96k for `off`.
  `on-bar2-traps.txt` should show only `R 0x00fea000`, about 470 per context creation. If other
  offsets appear, the flush page moved: set `CARAXES_BAR2_MEMFD_TRAP` to the offset `off` shows
  as the only BAR2 read, and repeat.
- `summary.md`: cuCtxCreate and cuCtxDestroy (the `init` metric), the churn metric, and the device
  CPU per run, off against on.
- `on-device-summary.txt`, the `bar2 memfd` line:
  - syncs and their total time, which is the device CPU the design adds;
  - pages entered and loaded;
  - alias conflicts, which must be 0;
  - sysmem write-backs skipped, which must be 0.

  The `bar tlb` line should show far fewer flushes by a table write.
- The cuda gate green in both modes, and the host Xid count 0 in both.
- If `on` is red where `off` is green, repeat `on` with `CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1`. If
  that is green, a guest path writes tables through BAR2 without an invalidate before its
  doorbell (rule 2 of §3). Both results are worth keeping.

## 8. With the BAR1 pool (2026-09-25)

The trap census found that the memfd and the BAR1 pool (`CARAXES_BAR1_PASSTHROUGH=pool`,
bar1-pool.md) did not work together: the first `cuCtxCreate` hung, also with
`CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1`, and the shadow's first walk of the context's root found 15617
leaves where the pool alone finds 16128 (optimization-backlog.md §3.1). The census's hypothesis
was the order of the two translations in `FakeFb::read`. That order was wrong, but it was not
what hung.

**The cause.** The pool follows the guest's BAR1 page tables over its area. It learns about a
change from `FakeFb::note_write`, which every trapped write passes (`fb.rs` `note_write`, the
`p.note_table_write` call). The guest RM writes those BAR1 PTEs through BAR2. With the memfd, those
writes no longer trap: only a sync sees them, and it reported them through
`FakeFb::note_pages_changed`. That function told the shadow's walk caches, the BAR TLB, the watched
dwords and the P21 check, but not the pool. So after its first walk the pool never moved a page.
The device summaries of the census's hung runs show this: `on/` and `on-syncdb/` have 0 pages moved
in, `off/` has 3072 (`caraxes/runs/trap-census-2026-09-25/*/device-summary.txt`). The guest's CPU
writes through the area then reached only the pool pages, while the GPU and the device read the
guest pages, so the context's first work never ran. The 511 missing leaves are the same fact seen
from the shadow: `FakeFb::remap_leaves` splits a 2 MiB leaf that holds a pooled page into 512 4 KiB
leaves (`bar1pool.rs` `split_leaf`). With no page pooled, the leaf stayed whole: one leaf where
there should be 512.

**The second fault, the hypothesis.** A page can be behind BAR2 and in the pool at the same time.
The memfd's remap table is keyed by the guest FB address, because that is what the guest's BAR2
tables name. `FakeFb::read` first translated the address to the pool page and then asked the memfd
for the pool address. That missed the guest's unsynced bytes. The sync wrote back to the guest
address, which the GPU no longer reads, and a device write updated the memfd under the pool
address, which the guest's BAR2 view does not use. In the measured runs no page was in both
places (the pool's new counter below is 0), so this fault did not cause the hang. It is fixed
anyway.

**The fix** (commit bb52a46):
- `FakeFb::note_pages_changed` (`fb.rs:934`, the block at `fb.rs:943`) marks the pool's units
  dirty for every page a sync found changed, as `note_write` does for a trapped write. The pool
  then walks at the next trapped BAR0 write, as before.
- One translation order. The memfd is asked first, with the guest address. Every store access is
  translated through the pool's remap, and the translation is applied when the access happens:
  - `FakeFb::read` (`fb.rs:1121`), and `Bar2Area::read_through` (`bar2area.rs:576`, `:732`) with a
    `Translate` argument (`bar2area.rs:567`);
  - `FakeFb::write` (`fb.rs:1145`): the store gets the remapped pieces, and the memfd's
    `device_write` gets the guest address;
  - the sync's write-back (`bar2area.rs:787`) and the memfd's loads of pages that enter BAR2
    (`bar2area.rs:948`). `FakeFb::bar2_sync` (`fb.rs:997`) and `carry_bar2_area` (`fb.rs:1029`)
    pass the pool's table in.

  The pool's copy-in and copy-back (`copy_page_raw`) stay raw store copies. That is consistent:
  bytes the guest wrote through BAR2 before a move are written back by the next sync to wherever
  the page lives by then.
- One aliasing case remains. If a pooled page is also behind BAR2, a guest CPU write through the
  BAR1 area reaches the pool page, and the guest's own BAR2 view of that page sees it only after
  a device write of the same bytes. The pool counts such pages in its exit summary ("page(s) also
  behind the BAR2 memfd", `bar1pool.rs:844`) and logs each one. The count was 0 in every run here.

**Tests** (both fail without the fix, `caraxes/runs/opt-memfd-pool-2026-09-25/unit-before.txt`):
- `bar1pool::tests::with_the_bar2_memfd_the_pool_sees_bar1_ptes_written_through_bar2`
  (`bar1pool.rs:1397`). A guest BAR1 PTE is written through the memfd, then synced and polled.
  The page must move into the pool, and the shadow must see a 2 MiB leaf over it as 512 leaves,
  not 1. Without the fix, 0 pages move.
- `bar1pool::tests::a_page_in_the_pool_and_behind_bar2_reads_and_writes_through_one_order`
  (`bar1pool.rs:1433`). A page is both pooled and behind BAR2. The test checks:
  - device reads see the guest's unsynced bytes;
  - the sync writes them to the pool page;
  - device writes reach the guest's BAR2 view;
  - after the move back and the BAR2 unmap, every byte is at the guest page.

  Without the fix, the first read returns the pool page's stale 0.

**Measured on vm3** (`caraxes/runs/opt-memfd-pool-2026-09-25/`, README there): the census's `on`
configuration (interrupt page, BAR1 pool 32 MiB, token pool 64, hybrid reactor, memfd) with the
fixed device, main plus bb52a46. Census rows are guest medians against the census's own runs
(`summary.md` there):

| Operation | `off` (pool, no memfd) | `on-nopool` (memfd, no pool) | `on` fixed (memfd and pool) | native |
|---|---|---|---|---|
| cuInit | 177 ms, 3290 traps | 20.4 ms, 174 | 28.2 ms (14.9-48.7 over 3 processes), 174 | 94 ms |
| cuCtxCreate | 4.09 s, 75070 traps | 0.87 s, 1884 | **0.61 s, 1228** | 0.15 s |
| cuCtxDestroy | 1.24 s, 21507 | 0.18 s, 924 | 0.20 s, 926 | 0.09 s |
| cuMemAllocHost 1 MiB | 24.8 ms, 807 | 1.65 ms, 8.2 | 1.72 ms, 8.2 | 0.76 ms |
| cuMemHostRegister 1 MiB | 38.9 ms, 1049 | 1.73 ms, 15.2 | 2.29 ms, 15.4 | 0.28 ms |
| cuMemAlloc 64 MiB | 4.4 ms, 188 | 0.86 ms, 8.0 | 0.97 ms, 8.2 | 0.13 ms |
| launch + sync, 1000 back to back | 19.6 us, 1.0 | 130 us, 3.6 | 20.4 us, 0.0 | 6.3 us |

- cuCtxCreate traps 1228 times, which is `on-nopool`'s 1884 minus its 642 BAR1 writes. It is
  faster than `on-nopool`: the submissions of the context's setup are eventfd rings, not trapped
  BAR1 writes.
- cuMemHostRegister is 0.5 ms slower than `on-nopool` with the same traps, and the other rows are
  within the spread of single runs.
- The steady-state rows keep the pool's 0 traps.
- The whole process of each workload (one run each, same session, `on` against `off` in `off/`):
  - rlprobe: process wall 15.5 s against 21.3 s, stage 4 2.6 s against 3.3 s;
  - PPO: 46872 against 50367 steps per second. Another VM's device was busy during both;
  - BAR2 writes per process: 0 against 100700.
- cuda gate: 10 of 12 (the gate script's count: the declared-unsupported cuda-gdb row is among the 12; documents that count only the 11 scored rows write the same result as 10 of 11, testing.md section 3, cuda), cuda-managed failing as before (801), cuda-gdb declared unsupported.
- Pool mechanics: 15872 pages moved in and back, 358 barriers, 0 refused at the end, 0 pages also
  behind the memfd.
- memfd: 40456 syncs, 8.8 s in total over the whole session, 25 ms at most; 0 alias conflicts.
- Host Xid: 0 in every part.

**Copies** (`bench-run.sh` copy; pool alone against pool with memfd, three passes back to back:
`off`/`on` 3 reps each in `copy-summary.md`, `off-2`/`on-2` 4 reps in `copy-summary-2.md`,
`off-3`/`on-3` 3 reps with the card's P-state and PCIe link sampled):
- The 256 MiB rows do not move: 13.1 GB/s pinned both ways, and d2d 187-192 GB/s against 177-190.
- d2d 1 MiB is 57-65 GB/s against 30-57, and d2d 16 MiB 13.9-16.9 against 5.7-16.2.
- The pinned 1 and 16 MiB rows are bimodal in both configurations: about 6.0 and 6.5 GB/s, or
  about 11.4 and 12.9 GB/s. The mode follows the physical card's power state, not the
  configuration:
  - `link-on-3.csv` and `link-off-3.csv` sample `nvidia-smi --query-gpu` every 500 ms. The link
    runs at PCIe gen 1 at P8, gen 2 at P5 and gen 3 at P0, and the card sat at P8 most of the time.
  - `on-3` had one of its three reps in the fast mode. `off-3` had none.
  - In `off-2` the fast reps coincide with another VM's work holding the card at P0 (the MEAS
    lines and `# gpu` headers).
  - The medians of `copy-summary-2.md`, pool alone 12.8 against 6.56 with the memfd, therefore
    compare power states. They are not a regression.
- Pageable 16 and 256 MiB went from 6.2-8.1 to 3.2-3.7 GB/s in the first pass. The second pass
  did not repeat it (7.2-8.9 against 6.6-9.6), with the host load at 8.4 in the first pass and 2
  in the second.
- Device CPU per copy run: 2.6-3.5 s against 10.9-11.4 s. VMM CPU: 3.4-6.3 s against 30.5-32.5 s.
  This is the context's BAR2 traffic that no longer traps.

Conclusion: the memfd with the pool is the recommended CUDA configuration from here on:
- the interrupt page, the BAR1 pool, the token pool with the hybrid reactor, and the BAR2 memfd;
- with `CARAXES_MSI_REARM_IOEVENTFD=1` (intr-page.md §4) on top.

`caraxes/tools/bench-run.sh` says so in its header.
## 8. Measured (night series, 2026-09-24/25, vm2)

`caraxes/runs/bar2-memfd-2026-09-24/night/` (`run.sh`, `summary.md`; tables and reading in
benchmark.md "Night series", Part 1). vm2, 610 guest, 1 GiB profile at slot 3, main 57693d8
frozen, token pool 64, hybrid reactor, interrupt page on, host 615.71.09 with other VMs active.

| Mode | Trapped BAR2 writes, one context creation | cuCtxCreate, ms (median, n=5) | cuCtxDestroy, ms |
|---|---|---|---|
| off, BAR1 pool | 96269 | 3793 | 1462 |
| off, no pool (`alloc-path-2026-09-24/night/intr`) | - | 3863 | 1166 |
| on, no pool | **0** (flush page 0xfea000 read 478 times, the only trapped BAR2 page) | **1077** (533 in the quieter stack runs of alloc-path.md §7) | **391** (150) |
| on, BAR1 pool | 0 | hangs | - |

- §7's expectations hold: no BAR2 write traps, only the flush page, 0 alias conflicts, 0 sysmem
  write-backs skipped, cuda gate 10 of 11 scored (the known cuda-managed 801) and 0 host Xid on
  vm2's device. cuCtxCreate lands at 0.5-1.1 s, the sub-second range §6 expected; the device's
  sync work of one instance (last rep plus the cuda gate) was 35237 syncs and 7.6 s.
- **With `CARAXES_BAR1_PASSTHROUGH=pool` the first context creation hangs** at the first GR
  channel's doorbell, with host notifier 0x800000c firing continuously; the same with
  `CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1` and with the interrupt page off, so it is not rule 2 of
  §3. Until that is found (the shared translation hook, `fb.rs:922`), the memfd runs without the
  pool; the pool changed no number anyway (benchmark.md "Design H, the BAR1 pool, measured").

## README file-table row

| [bar2-memfd.md](bar2-memfd.md) | D18's BAR2 line behind `CARAXES_BAR2_MEMFD=1`: region 3 as a sealed memfd with sparse areas around the trapped flush page; FB pages mapped through BAR2 live in the memfd (`FakeFb::backing`), synced at the RM's flush reads, MMU invalidates, BAR2 binds and GSP doorbells by a snapshot compare that writes back only the changed bytes and marks the shadow's walk-cache pages dirty; why soft-dirty, userfaultfd write-protect and mprotect on the device's mapping cannot see guest writes (KVM dirty logging would, with a VMM change); what stays trapped, the hazards, fail-closed setup, tests, measurement plan (subagent report) |
