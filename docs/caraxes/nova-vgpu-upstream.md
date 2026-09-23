<!-- SPDX-License-Identifier: MIT -->
# The upstream NVIDIA vGPU manager (nova-core) against Caraxes

Written 2026-09-23 (research task, internet access, read-only towards the device crate).
Subject: the series "[PATCH v2 00/32] Introduce NVIDIA vGPU manager" posted to nova-gpu and
linux-kernel on 2026-09-14, read patch by patch from the lore mbox, compared with the
Caraxes device (`crates/vfio-user-nvgpu`, `D/` below) and the decisions in [README.md](README.md).
Line numbers in `D/` are those of the working tree on 2026-09-23; another agent edits that
crate concurrently, so they can drift by a few lines. Nothing was run, no VM, no GPU access.

Purpose: learn from how NVIDIA structures the host side of its own virtual GPU, and find
concrete defects or gaps in Caraxes. Not purpose: move Caraxes towards NVIDIA's architecture.

## 1. Sources

All fetched as plain HTTP GETs without identifiers; the mbox files stayed in the session
scratchpad and are not committed.

**The series (v2, 32 patches, 2026-09-14, Zhi Wang, with Alok Kumar, Eliot Courtney, John
Hubbard).** Thread mbox: `https://lore.kernel.org/all/20260914074907.1399629-1-zhiw@nvidia.com/t.mbox.gz`.
Message-ids are `<20260914074907.1399629-N@nvidia.com>` with N = patch number + 1 (the cover
letter is `-1`, patch 32 is `-33`). The thread had no replies at download time. Base commit
`9ff5fd561368ebc3cf1b82a930823ace445564cc` (drm-rust-next plus the r000 GSP, NVKV v2, memory
management, SR-IOV, bitmap/id-pool and PCI prerequisite work).

The four starting points (openwall mirror of linux-kernel):

| URL | Message |
|---|---|
| https://lists.openwall.net/linux-kernel/2026/09/14/815 | 00/32 cover letter |
| https://lists.openwall.net/linux-kernel/2026/09/14/770 | 17/32 add r000 plugin bindings |
| https://lists.openwall.net/linux-kernel/2026/09/14/774 | 30/32 scrub guest VRAM with CeUtils |
| https://lists.openwall.net/linux-kernel/2026/09/14/779 | 26/32 add GSP plugin RPC transactions |

Patch list (area `gpu: nova-core:` omitted):

| # | Title | Read for |
|---|---|---|
| 00 | Introduce NVIDIA vGPU manager | architecture, scope |
| 01 | gsp: pass boot context through setup helpers | (refactor) |
| 02 | gsp: decouple boot context from VgpuManager | (refactor) |
| 03 | vgpu: detect boot state independently | vGPU mode before GSP boot |
| 04 | gpu: add a channel ID pool for vGPU | channels |
| 05 | gsp: expose the VMMU segment size | VMMU segment |
| 06 | gsp: decode the FIFO engine table (Alok Kumar) | engine table |
| 07 | vgpu: initialize runtime parameters after GSP boot | VMMU, engines, channels |
| 08 | vgpu: reserve the 48-VM WPR2 heap | WPR2 heap |
| 09 | mm: borrow BarUser for temporary BAR1 access | BAR1 |
| 10 | mm: add VramBlock | VRAM |
| 11 | mm: add VramRegion | VRAM |
| 12 | mm: add BarMapping | BAR1 access width, ordering |
| 13 | gsp: dispatch RM RPC messages during GMC receive | shared queue |
| 14 | gsp: add synchronous GMC transactions | transaction discipline |
| 15 | gsp: add status-only GMC transactions | status check |
| 16 | gsp: wait for GMC completion events | transaction discipline |
| 17 | vgpu: add r000 plugin bindings | plugin shared ABI |
| 18 | vgpu: add VRAM slot allocator (Alok Kumar) | VRAM |
| 19 | gsp: factor out NVKV payload conversion | (refactor) |
| 20 | vgpu: query VF assignments and properties | vGPU type |
| 21 | vgpu: add instance create/destroy | VRAM, channels |
| 22 | vgpu: encode vGPU boot requests | channel map, FB segments |
| 23 | vgpu: add GSP plugin communication buffers | comm region |
| 24 | vgpu: add instance boot and shutdown | ready marker, teardown |
| 25 | vgpu: initialize GSP plugin RPC buffers | shared-buffer init |
| 26 | vgpu: add GSP plugin RPC transactions | RPC, doorbell |
| 27 | vgpu: negotiate the GSP plugin RPC version | RPC |
| 28 | vgpu: send GSP plugin configuration parameters | migration flags |
| 29 | vgpu: update the GSP plugin BME state | RPC |
| 30 | vgpu: scrub guest VRAM with CeUtils | scrubbing, fail-closed |
| 31 | add build ID headers to debugfs log buffer dumps (John Hubbard) | logging |
| 32 | vgpu: export plugin log buffers via debugfs | logging |

**v1 (13 patches, 2026-09-05, with the VFIO variant driver and its review).** Thread mbox:
`https://lore.kernel.org/all/20260905081116.106613-1-zhiw@nvidia.com/t.mbox.gz`, cover
`<20260905081116.106613-1-zhiw@nvidia.com>`, "[PATCH 00/13] Introduce NVIDIA vGPU manager and
VFIO variant driver". Used for the VFIO side (patch 12/13 "vfio/nvidia-vgpu: add the NVIDIA vGPU
VFIO variant driver", `<20260905081116.106613-13-zhiw@nvidia.com>`), the open/reset/close
lifecycle and the review by Alex Williamson (`<20260908210044.6514a092@shazbot.org>`,
`<20260914121217.70fa0d93@shazbot.org>`, `<20260915120148.7548a8ca@shazbot.org>`,
`<20260923140520.5cbea0c7@shazbot.org>`), Jason Gunthorpe (`<20260916141718.GU3968357@nvidia.com>`
and later), Danilo Krummrich, Dave Airlie, Gary Guo, Alistair Popple (`<aqOE4FJ6eq1qu_kQ@nvdebian.thelocal>`)
and Alexandre Courbot. The v2 cover letter's reference list swaps [2] and [3]: the lore link
labelled "prerequisite stack" is this v1 thread, the GitHub link is the WIP branch.

**Prerequisites and context** (not read in full):
- "[PATCH v2 1/1] gpu: nova-core: populate GSP_VF_INFO when vGPU is enabled" (Zhi Wang,
  2026-08-04), read through the mirror https://ratatoskr.run/nova-gpu/2026/08/17364619/t
  (message-id not shown there; not retrieved). Source of the VF-topology finding (§3.8).
- The earlier RFC referenced by the cover letter, https://lore.kernel.org/kvm/20250903221111.3866249-1-zhiw@nvidia.com/
  (not read).
- The r000 firmware ABI rework ("[PATCH 00/27] gpu: nova-core: boot on the r000 ...",
  https://ratatoskr.run/nova-gpu/2026/08/17431894/t), the NVKV codec
  (https://ratatoskr.run/rust-for-linux/2026/08/17470089/t) and the vGPU-boot series ("gpu:
  nova-core: boot GSP with vGPU ...", https://ratatoskr.run/nova-gpu/2026/07/17302683/t): found
  by web search, only their summaries were read.
- lore's search endpoint answered 403 to plain requests, so the keyword searches of the brief
  ("NVIDIA vGPU", "nova vGPU manager", "nova-core VFIO", "GSP vGPU plugin", "nvidia-vgpu",
  "vfio-pci variant", "CeUtils", "VMMU segment", "FIFO engine table") were run through a web
  search engine instead; they surfaced only the threads above. **Gap:** threads on kvm@ or
  dri-devel@ that do not mention nova-core in the subject may have been missed.

## 2. The two architectures

**Upstream.** The physical GPU runs in SR-IOV mode. nova-core, the PF driver, detects vGPU mode
before GSP boot (FSP knob plus the SR-IOV VF count), tells GSP-RM the VF topology
(`GSP_VF_INFO`, `RMSetSriovMode`), sizes the WPR2 heap for the VF count (patch 08), and after
GSP_INIT keeps the VMMU segment size and the host-driven FIFO engine list (05-07). Each VF that
QEMU opens through the `nvidia-vgpu` VFIO variant driver becomes one instance: nova-core asks
GSP-RM for the VF's assigned vGPU type (20), reserves a contiguous channel-ID range and one
slot of a contiguous VRAM pool holding the guest FB plus a per-instance plugin management heap
(18, 21), boots a per-VF **GSP vGPU plugin task** inside the GSP firmware with a GMCAPI
`BOOTLOAD` (22, 24), then talks to that plugin over a BAR1-mapped communication region in the
management heap with a sequence-numbered request/response protocol and a VF doorbell (23, 25-29).
Guest VRAM is scrubbed by a GSP-owned per-instance CeUtils channel at allocation and shutdown
(30). The guest runs NVIDIA's vGPU guest driver on the VF; the VF's BARs, GMMU and interrupts are
hardware-partitioned, and the plugin inside GSP is the guest's RM server.

**Caraxes.** No SR-IOV and no plugin. The card is a Turing GeForce PF owned by the stock host
driver. The device process emulates a whole PCI GPU over vfio-user and itself plays GSP-RM
towards a guest that runs the stock driver in its **bare-metal** GSP-client configuration
(D1). RPCs arrive in the guest-RAM message queues and are answered locally or forwarded to the
host RM, where the device is an unprivileged client with a scoped CAP_SYS_ADMIN window (D10,
D12). Guest VRAM is a fixed contiguous reservation at the top of the host FB heap (D11), guest
RAM is registered as OS descriptors (D13), guest channels are mirrored as host channels whose
doorbells the device rings after a write-combining flush (execution.md §3-4), and the route to
isolation is shadow page tables in the device (D17), with no compression ever (D16).

**Caraxes must not copy this blindly.** NVIDIA's design assumes real SR-IOV VFs: hardware
per-VF BAR windows, a VF GMMU with VMMU segments, per-GFID interrupt and doorbell routing in
GSP, a vGPU-mode guest driver and a firmware plugin that Caraxes does not have and cannot
load on a GeForce PF. Caraxes is a synthetic device on a PF (decision D17: the guest stays bare
metal; that is the point). Where upstream's mechanism exists only because of VFs (GFID
doorbells, VF BAR topology, VMMU alignment, the plugin comm region), the right Caraxes answer is
usually "not applicable". What transfers is discipline: transaction ordering, fail-closed
teardown, scrubbing, and bookkeeping of engines and channels. The SR-IOV route is P18 (Ada and
later, a VF consumed through VFIO underneath the synthetic device), and it is there that this
series becomes the host stack Caraxes would sit on.

## 3. Findings per topic

Verdicts: **same**, **different-and-fine**, **different-and-actionable**, **unknown**.

### 3.1 GSP plugin shared ABI

Upstream: patch 17 adds the nova-core subset of `dev_vgpu_gsp_shared.h` to
`drivers/gpu/nova-core/gsp/fw/r000_00/bindings.rs`, generated from the r000 firmware interface
so the manager cannot drift from the firmware layout. Constants: `GSP_PLUGIN_BOOTLOADED` =
0x4E654A6F, `VGPU_CPU_GSP_CTRL_BUFF_VERSION` = 2, region sizes (control, response, message and
error 4 KiB each, migration 2 MiB, init/vGPU/kernel task logs 128/256/64 KiB, guest RPC trace
64 KiB, total 0x284000). Structs `VGPU_CPU_GSP_CTRL_BUFF_REGION`,
`VGPU_CPU_GSP_RESPONSE_BUFF_REGION`, `VGPU_CPU_GSP_GUEST_INFO`,
`VGPU_CPU_GSP_DISPLAYLESS_SURFACE`, 17 `MESSAGE_NV_VGPU_CPU_RPC_MSG_*` ids (version negotiation,
config/init, reset, six migration steps, console VNC state, VF BAR0 register access, BME state,
migration pointer reset, release client database, is-alive, static info). The v2 cover letter
stresses that the VFIO side no longer carries copies of firmware headers.

Caraxes: no plugin, so none of this ABI is consumed. The discipline is the same as D14 and
`crates/nvgsp-sys`: the 610 message-queue and RPC layouts come from bindgen over the vendored
headers (gsp-abi-610.md, the `ver::active` constants imported at `D/msgq.rs:17-20` and
`D/rpc.rs:20-25`), not hand-typed.

Verdict: **same** in discipline (generated ABI, no copies), **different-and-fine** in content.
One side note: the response region publishes up to four displayless surfaces with a
`sequence_update_start`/`sequence_update_end` pair, a seqlock the host reads to scan out the
guest console. That is the pattern the present backend of the display work (P12, P16 D) can use
for its own surface descriptor.

### 3.2 GSP plugin RPC transactions

Upstream, patch 26, `vgpu/gsp_plugin_rpc.rs` `PluginRpc::rpc_call` and
`vgpu/gsp_plugin_comm.rs` `CommBufferRegion::submit`/`response`:
1. `next_sequence()`: `message_sequence + 1`, wrapping, skipping 0 (0 is the "initialised"
   value and never a request).
2. `submit`: reject payloads larger than the message region (`E2BIG`); copy the payload into the
   message region as 32-bit BAR1 writes; then write `message_type`; then write
   `message_seq_num` last. The sequence write is the publication.
3. `NV_VIRTUAL_FUNCTION_PRIV_DOORBELL::ring_gsp_plugin` (`regs.rs`): write
   `gfid * 32 + 17` to PF BAR0 `0x00b8_2200`, then **read the register back**.
4. `wait_response`: poll `message_seq_num_processed` in the response region every 1 ms; when it
   equals the expected sequence, read `result_code`; non-zero is `EIO`; 120 s monotonic timeout
   gives `ETIMEDOUT` (logging the last processed sequence).
5. Sequence ownership: only the host writes the request sequence; only the plugin writes the
   processed sequence; one outstanding request per instance (the caller holds `&mut PluginRpc`).
There is no explicit CPU barrier between steps 2 and 3: every BAR1 access goes through kernel
MMIO accessors (`try_write32`, `BarMapping`, patch 12), which are uncached and ordered on x86 and
carry the write barrier of `writel` on weakly ordered architectures. The typed callers follow in
27 (`VersionNegotiation`, empty payload), 28 (`SetupConfigParamsAndInit`, an NVKV stream
prefixed by its u64 word count) and 29 (`UpdateBmeState`, "the guest can then use the vGPU").

Caraxes counterpart: the GSP message queues in guest RAM, `D/msgq.rs` `QueueState::receive`
(313-364) and `send` (379-421), driven by the guest's queue-head doorbell in `D/boot.rs`
(443-457, `service_queues` 468-812). The roles are reversed: Caraxes is the server (the plugin's
role), the guest kernel RM is the client, and the channel is a ring of 4 KiB slots rather than a
single mailbox. Section 5 checks it step by step.

Verdict: **different-and-fine** in shape; two **actionable** details in section 5 (acquire
and release fences on the consumer side, and what a malformed element does to the ring).

### 3.3 Shared-buffer initialisation

Upstream: the control region's `message_seq_num` field doubles as the plugin's boot-ready
marker. Patch 24 `CommBufferRegion::clear_plugin_ready` writes 0 to it **and reads it back**
("Complete the posted clear before firmware can publish its new marker") before `BOOTLOAD`, so a
marker left by a previous instance in a reused slot cannot be mistaken for the new one;
`wait_plugin_ready` polls for `GSP_PLUGIN_BOOTLOADED` (1 ms, 10 s). Only then (patch 25
`CommBufferRegion::initialize`, called by `PluginRpc::init_rpc` in 27) does the host write the
region offsets (response, message, migration, error, guest RPC trace), zero the migration and
index fields, set `attached_vgpu_count` = 1, zero `message_type`, overwrite the marker with
sequence 0, zero `message_seq_num_processed` and `result_code`, and write `version` = 2 **last**.
Order matters for two reasons stated in the commit messages: the marker shares the sequence
field, and a stale processed-sequence equal to the first request's sequence would complete a
request that was never processed.

Caraxes: the analogous handshake is the queue link. `D/boot.rs` `link_queues` (815-883) reads
RMARGS and the queue page table from the guest, validates the guest's command-queue header,
writes our status-queue header and zeroes our rx header (`D/msgq.rs` `QueueState::link`
273-306), then sends `GSP_INIT_DONE`. The guest zeroes and builds its own queues before the
first doorbell (gsp-rpc-protocol.md §1-2), so nothing stale can be inherited across a guest
boot. The same "clear, confirm, then publish" rule applies in Caraxes to the one place where
the host RM overwrites guest-visible state behind the device: the host RM clears USERD at mirror
channel creation, and the device restores GPGet/GPPut through the WC window and flushes before
the doorbell (`D/mirror.rs` 540-559, execution.md §3).

Verdict: **same** in principle. A guest reset without device restart also starts clean:
`D/boot.rs` `reset` (239-257) replaces the whole boot model, so `queues`, `pages` and `pending`
(a half-received continuation) are dropped and the mirror frees its host objects; only the
reservation and the host client are carried over, unscrubbed (§3.5).

### 3.4 VRAM and channel allocation

Upstream:
- Channels (04, 21, 30): a GPU-wide `ChannelIdPool` of 2048 IDs (`gpu/channel.rs`
  `TOTAL_CHANNELS`, with a TODO to query it). Each instance reserves a **contiguous** range of
  `total_channels / max_instance` IDs (`reserve_ids(..., Alignment::SZ_1)`); the **last** ID of
  the range is the CeUtils channel, and the plugin is told `len - 1` channels
  (`configure_plugin`, patch 30). The boot request carries a channel map: one
  `ChannelMapEntry { engine_type, index, chid_offset }` per host-driven engine, all with the
  range start as offset (22, 24 `channel_mapping`), plus `num_plugin_channels` = 3.
- VRAM (10, 11, 18, 21): `GpuMm::alloc_vram_range` takes an **exact** range relative to the
  buddy allocator base. `VgpuVramSlotAllocator::new` allocates the whole pool at offset
  `0..pool_size` of usable VRAM for one vGPU type: all guest FB slots first (so every slot stays
  aligned to `fb_align`), then all management-heap slots, paired by bitmap index. A pool for a
  different type can only replace an empty pool. The boot request passes one guest FB segment
  (physical address and length, up to 8 supported) and the heap.

Caraxes: the reservation is one `NV01_MEMORY_LOCAL_USER`, FIXED_ADDRESS, CONTIGUOUS, 2 MiB-page
allocation at the **top** of the host FB heap, `[E - S, E)` with `E` the heap end aligned down to
2 MiB (`D/hostmem.rs` `reserve` 765-870, memory-model.md §1 and §4, D11), refused when the host
would keep less than 1 GiB free. The last 2 MiB is a device carve-out for the BAR roots and the
device-owned invalidator channel (`D/invalidate.rs`, execution.md §6). Channel IDs are not
partitioned at all: the guest sees a synthesized 512-channel FIFO
(`chips/tu106/chip.toml:102`, answered at `D/ctrl.rs:928`), allocates its own ChIDs, and each
mirrored channel gets whatever host ChID the host RM picks; the device keeps a guest-token to
host-token map (`D/mirror.rs` `doorbell` 1259, `chid_of_host_token` 587).

Verdict: **different-and-fine.** Upstream partitions a GPU it owns; Caraxes borrows from a host
RM it does not own, so a fixed contiguous reservation plus a token map is the right shape. Worth
taking over: upstream reserves a dedicated maintenance channel per instance inside the
instance's own resources. Caraxes already has one (the invalidator in the carve-out), and it is
the natural place for scrubbing (§3.5).

### 3.5 CeUtils VRAM scrubbing and fail-closed behaviour

Upstream, patch 30, `vgpu/scrubber.rs` and `vgpu/instance.rs`:
- `alloc_ceutils` (GMCAPI `VGPU_MGR_ALLOC_GSP_CEUTILS` with `fixed_chid` = the reserved last
  ChID, `force_ceid` = any) returns a semaphore address that must be non-zero, page aligned and
  in `NV_ADDR_FBMEM`; the page must carry the magic `0xce5e5ea0` (from OpenRM `channel_utils.h`).
- `scrub_guest_fb` submits `VGPU_MGR_SCRUB_GUEST_FB` in 4 GiB chunks; each returns a `work_id`;
  `wait_scrub_complete` maps the semaphore page through a temporary BAR1 mapping and polls the
  payload at offset 4 until `value.wrapping_sub(work_id) < 0x8000_0000` (a wrap-safe "reached"
  test), 1 ms polls, 5 s timeout per chunk.
- When: immediately after allocation (the instance is registered first with
  `initialized = false`, and cannot be activated until the scrub succeeded) and at destruction
  after the plugin shutdown and before `FREE_GSP_CEUTILS`, `CLEANUP` and the release of the slot.
  (v1 also scrubbed on VFIO reset; v2 dropped reset with the VFIO split.)
- Fail-closed: `CeUtilsAllocError::NotOwned` (the firmware explicitly refused) lets the
  instance be removed; `MayOwn` (transport or validation error: the firmware may own the ChID)
  returns the error **with the instance left registered**, so its channels and VRAM stay out of
  the allocators. A scrub failure in `destroy_instance` returns early through `?` before the
  instance is removed: its VRAM is quarantined rather than handed to the next VM. The cover
  letter states the rule: "If ownership or completion is uncertain, keep the affected VRAM and
  channels out of the allocators."

Caraxes: the reservation is **not scrubbed** by the device, neither when it is allocated nor
before it is freed (`D/hostmem.rs` `reserve` 765-870 and `shutdown` 1065-1122). Teardown frees
mirror channels, then descriptors, then the reservation, then the client, and logs but
**continues on errors** (`free_live` 583-592 and `shutdown`). What does happen today: the host
RM enables scrub-on-free by default "for GK110+" unless `RmDisableScrubOnFree` is set
(`vendor/open-gpu-kernel-modules/src/nvidia/src/kernel/gpu/mem_mgr/mem_mgr.c:200-208`,
`PMA_INIT_SCRUB_ON_FREE` at :3321-3324). Whether that path covers a user
`NV01_MEMORY_LOCAL_USER` FIXED_ADDRESS allocation on a GSP host (where the scrubber is a
CeUtils channel of the host kernel RM) is not verified.

Verdict: **different-and-actionable.** Three leak directions exist: host data left in VRAM that
the new reservation covers reaches the guest (allocation side), guest data reaches the host
desktop's next allocation (free side), and one VM's data reaches the next VM on the same
reservation. Upstream closes all three explicitly. Cost of the equivalent (estimates, not
measured):
- **Verify first** (cheapest): whether host scrub-on-free already zeroes a freed reservation;
  if it does, the free side and the VM-to-VM side are covered by the host RM, and only the
  allocation side remains (which is also covered if every earlier owner of those pages freed them
  through a scrubbing RM).
- **CE memset on the device-owned channel** (the invalidator of `D/invalidate.rs` is already a
  privileged host channel with GPFIFO, pushbuffer and semaphore in the carve-out, on COPY2): a
  `LAUNCH_DMA` memset over the reservation in chunks, semaphore release per chunk, wrap-safe
  wait. For a 4 GiB profile on TU106 (448 GB/s peak), a few tens of milliseconds of copy-engine
  time, shared with the desktop; code: one push builder and a loop. It inherits the open CE
  first-launch stall (execution.md §6.1), so it needs the warm-up push host-exec used.
- **CPU writes through BAR1 windows** (no channel): 4 GiB through 64 MiB WC windows at PCIe
  write speed, on the order of one to a few seconds per scrub, no new GPU state. Acceptable at
  device start and stop as a fallback.
- **Fail-closed**: if the scrub or any mirror free fails or is uncertain, do not free the
  descriptors and the reservation explicitly; keep them until process exit, and let the
  `/dev/nvidiactl` close tear down the whole client, where the RM orders the frees itself. Today
  a failed mirror-channel free is followed by unpinning guest RAM (`shutdown` 1080-1087), which
  is the inverse of upstream's rule and can leave a host channel pointing at unpinned IOVAs
  (memory-model.md §4 already names DMA-FQ IOVA reuse as the hazard).

### 3.6 VMMU segment size returned at GSP init

Upstream: patch 05 decodes key `0x1050` of the r000 `GSP_INIT` reply (zero when omitted), 07
stores it in `VgpuManager`, 18/21 use it as `fb_align`: each guest FB slot size and the pool
base must be multiples of it ("VMMU-segment alignment"), minimum 4 KiB.

Caraxes: no VMMU. On a PF without SR-IOV, the GPU sees the reservation through the host's own
GMMU and the guest's page tables; the alignment Caraxes needs is the page and PDE granularity,
and the reservation is 2 MiB aligned in base and size (memory-model.md §1, `E` aligned down to
2 MiB, `S` rounded up).

Verdict: **different-and-fine** for D17. For P18 the value matters: a VF's VRAM is whatever
slot the vGPU manager carved at VMMU-segment granularity, so the synthetic device on top of a VF
cannot choose its reservation; it inherits the slot. Record the segment size the host reports
when P18 is prototyped.

### 3.7 FIFO engine table handling

Upstream: patch 06 (Alok Kumar) decodes keys `0x0500` (count), `0x0501` (GMC engine ids,
indexed, up to 64) and `0x0502` (flags) of `GSP_INIT`, and keeps only engines with
`NVGMC_SC_ENGINE_FLAGS_IS_HOST_DRIVEN`, **in firmware FIFO order and with duplicate ids
preserved** (`FifoEngineList`, "Host-driven GMC engine IDs in hardware FIFO order, including
any repeated IDs"). Patch 24 `channel_mapping` turns each id into `(engine_type = id & 0xffff,
index = id >> 16, chid_offset)` in the same order. The commit messages do not say why duplicates
occur; the likely reading is one entry per FIFO/runlist slot of an engine that appears more than
once, and the firmware indexing the channel map positionally, so dedup or sorting would shift
every later entry.

Caraxes: the guest-visible table (`FIFO_GET_DEVICE_INFO_TABLE`, `D/ctrl.rs:756-788`) is the
chip's `chips/tu106/device-info.toml`, emitted in file order (`D/chip/mod.rs:460-489`, no dedup,
no sort). Engine identities, fault ids and PBDMA counts are the host card's; runlists and PBDMA
ids are synthesized (fifo-device-info.md); `gr0`, `ce0` and `ce1` share runlist 0, which the
table keeps as three entries. Mirrored channels take the host's engine type from the guest's
own `engineType` (execution.md §3), not from the guest-visible runlist.

Verdict: **same** on order and duplicates (both keep the source order verbatim).
**Different-and-fine** on meaning: Caraxes' table is a guest-facing description, not a
channel-to-engine binding the hardware uses. Keep the rule when D14 adds chips: never sort or
dedup a device-info or engine table; positions are meaning.

### 3.8 VF topology for GSP interrupt and doorbell routing

Upstream: the prerequisite "populate GSP_VF_INFO when vGPU is enabled" reads the SR-IOV
extended capability and fills `GSP_VF_INFO { totalVFs, firstVFOffset, FirstVFBar0/1/2Address,
b64bitBar0/1/2 }` in the GSP system info, because "GSP firmware requires VF BAR layout
information to correctly route interrupts and doorbell events to individual virtual
functions". The series then rings the per-VF plugin doorbell through the PF's view of the
virtual-function register window (`0x00b8_0000` base, doorbell `0x00b8_2200`, value
`gfid * 32 + 17`, patch 26), with GFID = VF index + 1 and GFID 0 the PF (patch 21 `Gfid`; v1
cover letter). Guest interrupts are hardware-routed per VF by GSP.

Caraxes: nothing is routed by hardware. The guest's GSP-client driver reads its interrupt tree
through the same `NV_VIRTUAL_FUNCTION` register window (`0xB80000 + ...`, gsp-rpc-protocol.md
"Receive (interrupt-driven)"), which the device emulates (`D/intr.rs`); the guest's own
`GspSystemInfo.gspVFInfo` is only parsed for BAR addresses (`D/rpc.rs` 123-134). Host
non-stall events from the mirror's OS events are turned into guest vectors plus an MSI eventfd
write, coalesced per pending vector (`D/intr.rs` `IntrShared::raise` 59-83, execution.md §4);
the host notifier is engine-wide, so the desktop's own completions also wake the guest
(execution.md §7: about 209k GR0 events in three minutes).

Verdict: **different-and-actionable**, already known: without hardware routing, the device
must do the filtering upstream gets from GSP. The designed progress gate (execution.md §7:
forward a vector only when a mirrored channel on that engine advanced its GPGet) is the
Caraxes-shaped answer. Nothing in the upstream VF topology applies to D17.

### 3.9 Public GSP-vGPU plugin communication-region structures

Upstream: patch 23 maps the first `VGPU_CPU_GSP_COMMUNICATION_BUFF_TOTAL_SIZE` bytes of the
instance's management heap through BAR1 and slices it, in the documented order: control
`0x000000`, response `0x001000`, message `0x002000`, migration `0x003000` (2 MiB), error
`0x203000`, init task log `0x204000`, vGPU task log `0x224000`, kernel task log `0x264000`,
guest RPC trace `0x274000`, end `0x284000`. It checks that the sizes add up and that the control
and response regions have exactly the bindgen struct sizes. The management heap is in VRAM
(`gsp_heap_size` of the vGPU type, 18/21), next to the guest FB slots.

Caraxes: the plugin's state is the device process itself (host RAM); the guest-visible
equivalents are the message queues in guest RAM (`D/msgq.rs`) and the device trace. No VRAM is
spent on a management heap beyond the 2 MiB carve-out.

Verdict: **different-and-fine.**

### 3.10 Migration and shared buffers

Upstream: the ABI reserves a 2 MiB migration region with CPU and GSP access offsets
(`migration_buf_cpu_access_offset`, `migration_buf_gsp_access_offset`,
`is_migration_in_progress`, `migration_state_save_complete`, `is_migration_allowed`) and the
migration message ids 4-10 and 14 (patch 17); patch 25 registers the offset and zeroes the
fields; patch 28 declares `MigrationFeature::PRESERVE_CTX_BUF` (0x4000) and
`FeatureFlags.vmm_migration = true` in the config. **No migration is implemented in the
series**; in the v1 review Alex Williamson points out that the variant driver must bind to what
it matches and cites hisi_acc, where "only the VFs are supported for the migration feature of the
variant driver". Migration is the VFIO side's future work.

Caraxes: no migration. State to move would be the device process state (RM object registry,
queue positions, mirror maps), the reservation contents, guest RAM (the VMM's), and the host
channels (which cannot move; they would be re-created from the registry and replayed).

Verdict: **unknown / not applicable yet.** One design seam to keep: upstream separates the
"stop work, save, restore deferred state, resume" phases into distinct messages; Caraxes'
registry already has what a replay needs, and quiescing mirrored channels (preempt, wait idle,
save GPGet/GPPut) is the step every other phase depends on.

### 3.11 Guest-RPC tracing and logging

Upstream: the control region holds `guest_rpc_trace_buff_offset` and a CPU get index, the
response region a GSP put index (17, 25): a 64 KiB ring in which the plugin records the guest's
RPCs. The series registers the ring but **does not consume it**. What it exposes (patch 32) are
the three plugin task logs through debugfs, read over BAR1, each prefixed (patch 31) with the
`LIBOS_LOG_NVLOG_BUFFER_V2` header (chip, 32-byte build id from the `BLID` tag of `gsp.tlv`) so
`nvlog_decoder` can decode a dump without out-of-band information.

Caraxes: every received and sent element is in the device trace with element sequence, function,
RPC sequence, length, checksum state and a decoded summary (`D/boot.rs` 545-554 and 776-800);
P10 plans one tracer with timestamps across the guest-ioctl, RPC and host-mirror layers.

Verdict: **different-and-fine.** Idea worth copying into P10: make each dump self-describing
(driver version, chip, device build) in a fixed header, as patch 31 does, so traces from
different runs and versions (Q29) can be decoded without guessing.

### 3.12 GMC transactions on the shared GSP-to-CPU queue (context for §5)

Upstream, patches 13-16, `gsp/cmdq.rs`: RM RPC elements and GMCAPI elements share the
GSP-to-CPU queue and are told apart by the NVDM type in the element header. Common framing is
validated before the format is chosen; a corrupt element **poisons** the queue (`EIO`);
interleaved RM RPC elements are consumed and dispatched while a GMC reply is awaited. A
synchronous transaction holds the queue lock from send to matching receive ("so another receiver
cannot consume the reply"), matches response flag, command id and sequence, uses one deadline
for everything observed while waiting, and consumes stale non-matching elements. An
asynchronous command waits under the same lock for its completion event (16).

Caraxes: the device answers every request synchronously inside the doorbell handler on the
server thread, so there is no second receiver and no interleaving; the mirror's event thread
never touches the queues (`D/intr.rs` 24-31, only interrupt bits). Our elements carry
`NVDM_HEADER` = type `RM_RPC` (`D/msgq.rs:395-396`, gsp-abi-610.md: 0x2510de7e).

Verdict: **same** in effect for the 610 guest. For D14 and the Nova work: nova-core is moving
to r000 (GMCAPI plus NVKV, NVDM-typed elements in one queue); a future guest on r000 would need
the device to dispatch by NVDM type as upstream does. nova.md assumed r570.144 framing
(nova.md §N2, row 3); that assumption should be re-checked against the r000 series before the
N gates.

## 4. Actionable differences, ranked

1. **VRAM isolation: scrub and fail-closed teardown** (§3.5). First verify whether the host
   RM's scrub-on-free covers the reservation (one supervised experiment on the device's own
   allocation, no guest). If not, scrub at reservation creation and before the free, with the
   invalidator channel (CE memset, tens of ms of CE time at 4 GiB) or BAR1 writes (seconds).
   Independently of scrubbing: when any mirror free fails, skip the explicit descriptor and
   reservation frees and leave them to the fd close. D16's comptag rule is untouched by this
   series (it says nothing about compression); keep it as is.
2. **Interrupts: progress-gated forwarding** (§3.8). Upstream gets per-VF routing from GSP;
   Caraxes must filter host-wide notifier events itself. Already designed (execution.md §7).
3. **Shared-memory ordering in the GSP queues** (§5): an acquire fence after reading the
   guest's write pointer and a release fence before publishing our read pointer (free on x86,
   required for a non-x86 host and against compiler reordering); handling of a malformed or
   out-of-range element so it cannot wedge the ring silently.
4. **Wrap-safe completion tests** (§3.5, §5): the invalidator waits for `sem == seq`
   (`D/invalidate.rs:343`). Correct while pushes are strictly serialized on one channel; the
   upstream form `value.wrapping_sub(target) < 0x8000_0000` also survives a missed value (for
   example a timed-out push that completes later, followed by the next one). Use it for any new
   semaphore wait (scrubber, warm-up push).
5. **Doorbells** (§5): same principle as upstream (payload visible before the doorbell).
   Upstream reads the doorbell back; Caraxes does `sfence` plus a read through every BAR1
   window before the host doorbell (`D/hostmem.rs` `Window::flush` 97-110, `D/ctrl.rs:239` and
   `:249`, `D/mirror.rs:326` and `:555`), which is what exec-formats.md §3 requires for WC
   mappings. No change needed; a read-back of the host doorbell page would only add a
   round trip.
6. **Channels** (§3.4): nothing to change; if a scrubber or warm-up path is added, reuse the
   device-owned channel in the carve-out rather than taking one from the guest's view.
7. **Engine topology** (§3.7): keep tables verbatim (no sort, no dedup) as D14 adds chips.
8. **Migration** (§3.10): nothing now; keep the registry sufficient for a replay.

## 5. Cross-check of the GSP message-queue implementation

The queue discipline lives in `D/msgq.rs` (ring access) and `D/boot.rs` (`service_queues`,
`link_queues`); `D/rpc.rs` is the RPC vocabulary and the static-info builder and has no queue
logic. Guest RAM is ordinary cached memory shared with the VMM; the guest side is
`message_queue_cpu.c` of 610.57.04 as surveyed in gsp-rpc-protocol.md.

| Upstream discipline | Caraxes (device as GSP) | Assessment |
|---|---|---|
| Write the complete payload before publishing (plugin RPC: payload, type, then sequence) | Status queue: the whole slot range is built in a zeroed buffer with MCTP/NVDM, element sequence, RPC header, payload and checksum, written, then `fence(SeqCst)`, then `writePtr` (`D/msgq.rs:394-417`) | same |
| No stale contents in published slots | Slots are written whole from a zeroed buffer, so padding never leaks old bytes (`:394`); received slots are read whole, and `slot_payload` deliberately keeps bytes beyond `length` for `GSP_RM_ALLOC` (`:248-251`, `:352`) | same |
| Consumer reads the publication, then the payload, with ordering between them | Command queue: reads `writePtr`, then the slots, no fence between (`:314-347`). x86 does not reorder loads with loads, but both reads are plain `copy_nonoverlapping` (`D/guestmem.rs:149-165`), which the compiler may reorder | **actionable, low**: `fence(Acquire)` after the `writePtr` read; zero cost on x86 |
| Consumer releases the slot only after copying it | `readPtr` is advanced and published after the copy (`:353-354`), no fence | **actionable, low**: `fence(Release)` before the store, same reasoning |
| Sequence ownership | Our element sequence `tx_seq` is ours alone (`:397`, `:418-419`); the RPC `sequence` is echoed from the request (`RpcHeader::reply`, `:232-239`), continuation records count on from it (`D/boot.rs:786-797`); the guest matches function and sequence and drains everything else as events (`_kgspRpcDrainOneEvent`). The guest's element sequence `seqNum` is read (`:350`) but not compared with `rx_seq` (`:355`) | same for replies; not validating the guest's `seqNum` is harmless while there is one producer |
| Single transaction owner | One server thread handles doorbell, receive and reply; the mirror event thread never writes the queues | same (upstream needs a lock because two receivers exist) |
| Doorbell after payload visibility | Guest to device: the guest stores the element, `portAtomicMemoryFenceStore`, `writePtr`, then writes 0 to `0x110C00`; the device learns of the doorbell through a vfio-user message on a socket (system calls on both sides are full barriers), and drains all pending elements per doorbell (`D/boot.rs:443-457`, `:483-491`). Device to guest: no doorbell; the guest polls for synchronous replies (`_kgspRpcRecvPoll`), and init is polled | same |
| Barriers and read-backs on device memory | Not applicable to the queues (guest RAM, not MMIO). The equivalent for BAR1 VRAM is `Window::flush` before every host doorbell and invalidate | same in principle |
| Check the firmware status | The guest checks `rpc_result`; the device sets it per request (`NV_ERR_NOT_SUPPORTED` for unanswered functions, `D/boot.rs:761-767`) | same |
| Corrupt framing poisons the queue (`EIO`) | Bad checksum: the element is consumed and **answered anyway**, only marked `BAD` in the trace (`D/boot.rs:546-552`, `D/msgq.rs:348-349`) | **actionable, low**: answer with an error status or drop it loudly; answering a torn request can apply half-written parameters |
| Timeouts and forward progress | Three silent wedge conditions: (a) a guest `writePtr >= msgCount` returns "empty" forever (`D/msgq.rs:317`); (b) an element with a bad length is an error, the read pointer does not move, and every later doorbell re-reads the same slot (`:330-335`, `D/boot.rs:487-489`); (c) a full status queue drops the reply with a trace line (`D/boot.rs:782`, `:798-801`) and the guest waits for that sequence until its own RPC timeout | **actionable, low**: (a) and (b) should log once loudly and either skip the element (answering an error for its sequence when the header is readable) or declare the queue dead; (c) should be impossible while the guest drains, but it deserves a counter |

**Could any of this explain a stalled RPC in Caraxes?** Only through the three wedge conditions
of the last row, and each leaves a distinctive line in the trace (`receive: ...`, `send: status
queue full`) or, for (a), a guest RPC timeout with no `rx` line at all. The memory-ordering gaps
cannot bite on this x86 host in practice (TSO, and the doorbell arrives through system calls).
**They do not explain the current stalls**, which are in the GR channel path: the GR doorbell is a
BAR0 usermode doorbell write (`0xBB0090`) handled by `D/ctrl.rs` and `D/mirror.rs`, not an RPC,
and the recorded failures are PBDMA/GR faults and GPGet not advancing on host channels
(execution.md §6). One indirect interaction to keep in mind: the guest's `MMU_INVALIDATE`
register write now runs the shadow re-sync synchronously on the server thread
(`D/boot.rs:383-404`); while it runs, no doorbell and no RPC is serviced, so a long re-sync
stretches every RPC's latency. That is a latency risk, not a wedge.

## 6. Unresolved questions

1. Does the host RM's scrub-on-free (`mem_mgr.c:200-208`) apply to a user
   `NV01_MEMORY_LOCAL_USER` FIXED_ADDRESS CONTIGUOUS allocation on a GSP-client host, and does it
   run on free or lazily on the next allocation? Needs a supervised measurement on the device's
   own reservation.
2. What does the CE first-launch stall (execution.md §6.1) do to a device-owned scrub channel;
   is the host-exec warm-up enough?
3. Why does GSP report duplicate engine ids in the FIFO engine table (§3.7)? The series
   preserves them without saying which engines repeat.
4. What value does GSP report for the VMMU segment size on Ada, and how does it constrain the
   P18 middleman's view of VF VRAM?
5. For P18: the upstream stack needs a vGPU type assigned to the VF by the firmware
   (`QUERY_ASSIGNED_VF_VGPU_TYPE`) and boots a plugin that expects a vGPU-mode guest (guest RPC
   trace, guest info, VNC console surfaces). Can a VF under this manager be driven by anything
   other than NVIDIA's vGPU guest driver, that is, can the Caraxes device speak the guest side of
   the plugin protocol while presenting a bare-metal device upward? This is P18's open check
   seen from the host side.
6. The r000 queue element format (MCTP/NVDM, GMC header) versus the 610 and r570 formats: what
   would the device's `msgq.rs` need for a guest on r000 (D14, nova.md)?
7. Threads outside nova-gpu (kvm@, dri-devel@) that did not surface through web search, since
   lore's search refused plain requests; and the message-id of the GSP_VF_INFO patch.
