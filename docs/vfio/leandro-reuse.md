<!-- SPDX-License-Identifier: MIT -->
<!-- Source survey generated 2026-09-23 by reading vendor/open-gpu-kernel-modules at tag 610.57.04
     (and the Leandro VFIO tree at e2f1aec). Every claim cites file:line; values marked "derived" were
     computed, not read. Treat as notes to be verified by experiment, not as specification. -->
# Leandro host backend: survey for reuse in Leandro VFIO

This was a read-only survey; no files were changed. Paths are relative to `/home/silas/git/Caraxes/`. `S` means `crates/vhost-user-nvrm/src/`, `A` means `crates/nvrm-abi/src/`, `W` means `crates/nvrm-wire/src/`.

**Main finding:** the reusable core is `Session::prepare`/`execute`, `host_pool`, `vram`, `mirror`, `nvrm-abi::xlate` and `nvrm-sys`. It is ioctl-shaped (NVOS64/NVOS54 envelopes plus an aux buffer) and can only be reached by passing it `nvrm-wire` message bytes. The transport layer (`nvrm.rs`) is closed and can't be reused. GSP RPCs work differently in several ways that matter here (serialized params, kernel-client privilege, no per-process identity or file descriptors), covered in §4.

---

## 1. RM request path end to end

### 1.1 Wire format (`nvrm-wire`)
- **Protocol version:** `PROTO_VERSION = 6` (W/lib.rs:15). The header says "160-byte request, 32-byte reply, then payloads" (W/lib.rs:4).
- **Message kinds** (`enum Kind`, W/lib.rs:21-38):
  - `Hello`=0, `Open`=1, `Close`=2, `Ioctl`=3, `MapPrepare`=4, `UvmPoolBack`=5.
  - Kinds handled only by the transport, never by a Session: `KIND_GET_TABLES`=6 (:60), `KIND_MAP_RELEASE`=7 (:63), `KIND_PROC_GONE`=8 (:67), `KIND_EVENT_FIRED`=9 (:101). How `EVENT_FIRED` reuses `Req` fields is documented at :69-100.
- **`Req`** (W/lib.rs:191-231):
  - Header: `seq, kind, dev_tag, ioctl_nr, target_token`.
  - Payload lengths: `inline_len, aux_len`.
  - Inline FD field: `fd_field_off, fd_field_proc, fd_field_token`.
  - Pointers: `embedded_ptr_off`, then `nested_count` plus `nested[4]` of `NestedDesc{ptr_off, aux_off, len}` (:305-315).
  - FD inside aux: `aux_fd_field_off, aux_fd_field_proc, aux_fd_field_token`.
  - Mapping and runs: `map_len, addr, gpa_run_count`.
  - `guest_proc`. The comment at :228-229 says it is "untrusted VM-local attribution".
  - Offsets are pinned by a test at :338-365.
- **`Rsp`** (:260-274): `seq, ret(-errno), token, inline_len, aux_len`. Reply bytes are `Rsp`, then the written-back inline block, then aux.
- **Device tags:** `DevTag` Ctl/Gpu/Uvm/UvmTools (:155-162).
- **Limits:** `MAX_PAYLOAD` 16384 (:176), `MAX_AUX` 1 MiB (:179).

### 1.2 Transport dispatch (`nvrm.rs`)
- `process_queue` pops a queue-0 chain, rejects sizes outside `[Req::WIRE_LEN, MAX_MSG]`, calls `handle` and writes the reply (S/nvrm.rs:1084-1156).
- `handle` (S/nvrm.rs:797-909):
  - Routes `GET_TABLES`, `MAP_RELEASE`, `PROC_GONE` and `MapPrepare` to device handlers (:806-810).
  - For everything else it resolves the inline and aux FD tokens against their named owner sessions (`resolve_fd`, :821-870).
  - It creates or looks up the session by `guest_proc` (`session_for`, :682-705, limit 1024 sessions).
  - It then calls `session.handle_msg_with(msg, aux_fd, fd_field_fd)` (:875).
  - Afterwards it collects pollables, unwatch lists and control-FD unwatch lists (:882-905).
- `Session::handle_msg_with` dispatches on `Kind` (S/session.rs:1042-1066). `on_ioctl` runs `prepare` then `execute` (:1279-1288).

### 1.3 Validation (before any resource is acquired)
All of this happens inside `Session::prepare` (S/session.rs:1294-1863), in this order:
1. Length bounds (:1298-1309) and device tag (:1313-1318).
2. **Private-client guard** `client_policy::private_client` (:1322-1333, defined at S/client_policy.rs:15-95). It checks `hClient`/`hRoot` at offset 0 for the RM escapes, `hClient`+`hClientSrc` for DUP_OBJECT, per-command `hClient` for UVM, and client fields inside alloc params for classes 0x80, 0xcb33, 0x83de, 0xc574 and 0xb2cc.
3. **Blocked controls** via `xlate::ctrl_blocked(cmd @8)` (:1338-1346).
4. **Request shape** `request_shape::validate::<A>` (:1348, defined at S/request_shape.rs:103-309):
   - Envelope size: UVM must match exactly (:119-124); frontend must be under 16 KiB and match `frontend_size` (:125-137).
   - Inline FD metadata must sit at the table offset (:139-145).
   - Refusals:
     - blocked allocation classes (:149-157)
     - blocked controls (:158-163)
     - **FINN-serialized controls and allocs** (:164-176)
     - VID_HEAP HW_ALLOC and OS descriptor (:177-187)
     - a non-null `pRightsRequested` (:188-193)
   - Embedded pointer: must match `xlate::embedded_ptr` and fit in aux (:200-217).
   - OS-descriptor GPA runs: exact byte accounting (:220-239).
   - Aux FD offset and width: 8 for allocs, 4 for controls (:241-261).
   - Nested descriptors: must match `xlate::nested_ptrs`, stay inside aux, not overlap params or each other (:263-307).
   - Zero-length pointers are cleared by `Shape::clear_unused_pointers` (:44-51).
5. Token lookup, and a check that the device tag equals the token's opened kind (S/session.rs:1357-1367).

### 1.4 Buffer preparation and translation (also in `prepare`)
- Inline block is copied to `self.scratch`, aux to `self.aux` (:1369-1375).
- **Step (1), inline FD:** token replaced by the host FD number (:1378-1400).
- **`NV_ESC_ALLOC_OS_EVENT`:** the target FD is queued as `Pollable::Client` (:1409-1415).
- **Step (2b'), OS descriptor 0x71 with GPA runs** (:1425-1567):
  1. Decode the runs.
  2. Build an arena through `pool.arena_for_osdesc`.
  3. Write the host VA into `NVOS02.pMemory`, or into the kernel-form `descriptor`.
  4. For the kernel form (NVOS64 class 0x71 from NVKMS), rewrite it into NVOS02+fd (56 bytes) on a registered GPU FD (`osdesc_gpu_fd`, :972-999) with WRITE_BACK/NO_MAP flags (`nvos02_flags_for_osdesc`, :2639-2667). Bytes 16..40 of the original NVOS64 are saved for the reply.
- **Step (2), embedded pointer:** `scratch[embedded_ptr_off] = aux.as_ptr()` (:1571-1575). That is, aux becomes the host-owned params buffer.
- **Step (1b), FD inside aux:** width taken from the host table (:1580-1628).
- **Step (2b), nested pointers:** `aux[ptr_off] = aux.as_ptr()+aux_off` (:1633-1648).
- **Step (3), request encoding:** UVM uses the raw number; frontend uses `iowr_raw(nr, inline_len)`. Anything above 16 KiB is refused ("XFER repack not built yet") (:1655-1664).
- **Emulated or substituted calls:**
  - UVM_ALLOC_SEMAPHORE_POOL is faked (:1670).
  - "Managed compat" UVM calls are faked (:1677-1679).
  - `NV01_EVENT_KERNEL_CALLBACK(_EX)` becomes a host `NV01_EVENT_OS_EVENT` on a per-client private event ctl (:1685-1724, `alloc_os_event_id` :594-619).
  - Semaphore-surface waiter callback pointers become pooled OS-event IDs (:1734-1782).
  - VRAM is reserved before RM_ALLOC and VID_HEAP_CONTROL (:1791-1831).
- The resulting `Plan` is at :483-530 and is filled at :1842-1862.

### 1.5 Execution (the real ioctl)
- `execute` spans S/session.rs:1894-2546.
- **The one forwarded-ioctl call site:** `self.sys.ioctl(plan.target_fd, plan.request, self.scratch.as_mut_ptr(), n)` (S/session.rs:2017-2021).
- `RealSyscalls::ioctl` is simply `libc::ioctl` (S/syscalls.rs:36-38). The `NvSyscalls` trait (:12-28) is injectable through `Session::set_syscalls` (S/session.rs:1013).
- UUID swap: guest to host before the call, host to guest after (:2012, :2031, `uuid_swap` :1868-1889).
- Other RM calls issued directly by the session: `alloc_os_event_on` (:631-661), `free_os_event` (:826-856), `drain_os_events` GET_EVENT_DATA (:885-965), REGISTER_FD (:972-999), SET_SUB_PROCESS_ID and DUP grant via `share` (A/share.rs:94-133).

### 1.6 Reply translation and bookkeeping (`execute`, after the call)
- **OS descriptor:** arena is kept keyed by `(hRoot, hObjectNew)` (:2035-2042).
- **Events:** registered on success, ID freed on failure (:2046-2061). Waiters are armed or recycled (:2065-2107).
- **Kernel-form 0x71:** the saved NVOS64 bytes 16..40 are restored (:2113-2120).
- **Control-result rewrites** (:2125-2202):
  - FB_GET_INFO V2 and V1, GPU name, GET_PIDS / GET_PID_INFO. The two PID controls get a VM-only process list; an unexpected buffer size becomes EINVAL.
  - Virtualization mode and encoder capacity.
  - VID_HEAP INFO (:2206-2219).
- **VRAM settle** (:2225-2275).
- **Frees:** UVM_FREE releases the pool (:2278-2296). VID_HEAP FREE (:2299-2314). Root-client token record (:2324-2334). RM_FREE drops the arena, VRAM charge, events or the whole client (:2338-2355).
- **After root-client alloc:** SET_SUB_PROCESS_ID (:2360-2387). DUP grant for UVM-duplicated classes (:2391-2412).
- **Reply:** `Rsp{ret, inline_len, aux_len}` plus the mutated scratch and aux (:2527-2545, `reply` :2550-2558). RM status stays inside the payload; only the syscall errno goes in `ret`.

### 1.7 How each operation is handled
| Operation | Handling |
|---|---|
| RM_ALLOC | NVOS64 (48 B) or NVOS21 (32 B) (A/xlate.rs:215-217). Params length comes from `alloc_param_size::<A>(hClass)` (A/xlate.rs:841+). Unknown class returns Err, which becomes ENOTSUP (A/xlate.rs:737-753). |
| RM_CONTROL | NVOS54; `paramsSize`@24 gives the length (A/xlate.rs:722-733). Nested pointers per `nested_ptrs` (A/xlate.rs:1240-1375). |
| RM_FREE | NVOS00, flat; post-call bookkeeping at S/session.rs:2338-2355. |
| RM_MAP_MEMORY | `Nvos33WithFd`; fd@48 translated (A/xlate.rs:672). The following `mmap` on the guest file goes through MapPrepare (§3.1). |
| RM_ALLOC_MEMORY | `Nvos02WithFd`, fd@48; must be an OS descriptor with GPA runs (S/request_shape.rs:220-239). |
| RM_DUP_OBJECT | NVOS55, flat; `hClient`/`hClientSrc` checked against private clients (S/client_policy.rs:36-39). |
| ALLOC_OS_EVENT | fd@8 translated (A/xlate.rs:670); the FD is watched as a pollable. |
| UVM | Sizes from `uvm_param_size_for::<A>` (A/xlate.rs:194-200). rmCtrlFd offsets per command (A/xlate.rs:647-662). All treated as flat (A/xlate.rs:713-718). |

### 1.8 Descriptor tables (`nvrm-abi`)
- **Source of truth:** `xlate.rs`.
  - `frontend_size` (:225-258), `fd_field_offset` (:647-677), `embedded_ptr` (:707-771).
  - `alloc_class_blocked` (:780-785, classes c637/c638/c639/c640/b0cd/b0ce/cdcd), `alloc_class_verified` (:791-824).
  - `alloc_fd_field` / `alloc_fd_guard` (:1095-1120).
  - `LenSource`/`NestedPtr` (:1128-1142), `nested_cmds` (:1151-1156), `nested_ptrs` (:1240-1375), `LenSource::resolve` (:1382-1396).
  - `blocked_ctrls` (:1166-1203, 34 controls), `ctrl_fd_offset` (:1213-1228).
- **Serialization:** `table::build::<A>()` (A/table.rs:91-103) serializes these rules into the `nvrm-wire::tables` records:
  - `TableHdr` (W/tables.rs:57-94)
  - `IoctlDesc{dev, nr, size, fd_off, emb_ptr_off, emb_len_kind, emb_len_off, cmd_off, rights_off, rights_if_size, handle_off, flags}` (:101-125)
  - `ClassDesc{hclass, param_size, fd_off, flags, fd_if_off, fd_if_val}` (:130-142)
  - `CtrlDesc{cmd, first, count, flags(CF_BLOCK/CF_FD), fd_off}` (:147-157)
  - `NestedDescRow{ptr_off, len_kind, len_off, elem}` (:179-185)
- **Special frontend rows** (RM_CONTROL, RM_ALLOC, FREE, ALLOC_MEMORY, XFER, GET_EVENT_DATA) come from `frontend_special` (A/table.rs:475-531). `verify_against_xlate` runs at build time (A/table.rs:535).
- **Who uses the tables:** only the guest C interpreter. The host re-validates everything from `xlate` directly.

### 1.9 Handle and client ownership
- **Mirror:** maps `token -> (OwnedFd, Dev)`. Tokens start at 1 and are never reused (S/mirror.rs:18-57).
  - RM binds clients to the open file description, so the mirrored FD must be the one the client rode on (S/mirror.rs:6-7).
  - Tokens are created only by `on_open`, which opens `/dev/nvidiactl`, `/dev/nvidiaN` (N < 32) or `/dev/nvidia-uvm` (S/session.rs:1082-1148). `insert_token` (:1019) exists for tests.
- **Sessions:** one per `guest_proc` (S/nvrm.rs:143-147).
- **RM client roots:** the guest allocates them. Handles are passed through verbatim; the backend does no handle translation.
  - The backend records `client_token[hClient] = token` so that closing the FD tears the client down (S/session.rs:190-192, 778-788).
  - VRAM charges are keyed by `(root, handle)` (S/vram.rs:837-840).
  - OS-descriptor arenas are keyed by `(root, hmemory)` (S/host_pool.rs:691).

---

## 2. The virtualization boundary
- **Per VM (one backend process):**
  - guest memory, window map, sessions, epoll/event routing, `WaiterPoller` (S/nvrm.rs:143-188)
  - `vram::Ledger` (S/vram.rs:619-629) and `PinBudget` (S/host_pool.rs:30-37, which includes `private_clients`)
  - the cached `grid::Card` (S/grid.rs:97-102, 150)
- **Per guest process:** the `Session` fields (S/session.rs:122-198):
  - mirror, `pending_maps`
  - `PoolState` (pools and osdesc arenas, S/host_pool.rs:687-692)
  - `vram::Books` (S/vram.rs:843-855)
  - per-client `event_ctls`, events, waiter pool and armed waiters, `client_token`
  - `osdesc_gpu` (one cached GPU FD, OPEN-QUESTIONS #79)
- **Per RM client:** root plus object handles; the same number can exist under different roots (docs/virtio/architecture.md:67). One private event ctl per client (S/session.rs:152-154).
- **What is trusted:** nothing from the guest. `guest_proc`, FD owners, handles and lengths are all untrusted (docs/virtio/security.md:9-10; W/lib.rs:228-229). The policy boundary is the VM (S/nvrm.rs:7-8). Known open gaps:
  - VM-wide ownership of source handles across DUP and UVM imports (docs/virtio/security.md:43; OPEN-QUESTIONS #76)
  - unaudited fields (#81)
  - native aliases of backing pages (#80)
- **Private pool clients:** host-owned RM root clients created by `Backing::new` (S/host_pool.rs:405-453).
  - Setup: ROOT_CLIENT on a private ctl, a REGISTER_FD'd GPU0 FD, DEVICE_0, SUBDEVICE_0, then GID for the UUID.
  - They back UVM semaphore pools and are registered in `PinBudget.private_clients` (`private_client`, :120-126; `driver`, :710-721).
  - Guest requests that name them are refused with EPERM (S/client_policy.rs; S/session.rs:1322-1333; `is_private_client` S/host_pool.rs:725-727).

---

## 3. Shared-memory mechanisms

### 3.1 The SHMEM window
- **Negotiation:**
  - Backend advertises `REPLY_ACK|BACKEND_REQ|SHMEM` (S/nvrm.rs:1189-1193).
  - `get_shmem_config` returns regions `[0, 8 GiB]`; region 1 is the host-visible window (:1197-1199; `HOST_VISIBLE_SIZE` :44, `SHM_ID_HOST_VISIBLE`=1 :40).
  - Patch 0001 has cloud-hypervisor query GET_SHMEM_CONFIG, allocate a power-of-two MMIO BAR, back it with PROT_NONE anonymous memory and register a KVM userspace mapping (patches/0001-generic-vhost-user-shmem.patch, device_manager hunk ~395-491).
  - The VMM-side handler bounds-checks the request (`window_target` patch:112-126), then `shmem_map` does `mmap(MAP_SHARED|MAP_FIXED, fd, fd_offset)` (patch:137-165). `shmem_unmap` puts PROT_NONE back (patch:169-190).
- **Flow for a guest `mmap`:** `nvrm_node_mmap` requires offset 0 (guest-module/virtio_nvrm/virtio_nvrm.c:3380-3401). `nvrm_mmap_window` picks the window offset, sends MAP_PREPARE and `io_remap_pfn_range`s `win_base+off` (virtio_nvrm.c:3133-3215).
- **Host side:**
  1. `Session::on_map_prepare` validates the token, `0 < map_len ≤ 256 MiB` (`MAX_MAP_LEN` :203) and tag match, and records a `PendingMap` under a blob ID (S/session.rs:1153-1195).
  2. `NvrmDevice::on_map_prepare` (S/nvrm.rs:940-1064):
     - fetches a duplicated FD through `take_pending_map` (S/session.rs:1248-1258)
     - checks 4 KiB alignment of offset and length and the window bound (:976-983), then overlap (:986-989, `window_overlaps` :100-109)
     - **probe-mmaps the RM FD** to validate the mapping context before involving the VMM (:994-1012)
     - sends `SHMEM_MAP{shmid 1, fd_offset 0, WRITABLE}` (:1014-1039)
     - records a `WindowMap` that retains the FD (:1040-1047)
     - replies with cacheability: GPU node = uncached, ctl node = cached (:53-58, :1056-1063)
  3. The RM mapping context comes from the prior forwarded RM_MAP_MEMORY on the same file description, and mmap needs offset 0 (S/session.rs:1150-1152).
- **Release:** `MAP_RELEASE` goes to `release_window`, which removes the slot only after `shmem_unmap` succeeds (S/nvrm.rs:708-727, 1067-1078). `PROC_GONE` releases a process's windows (:742-790).

### 3.2 Guest-page backing (`host_pool.rs`)
- **GPA runs:** the guest pins pages (`nvrm_pin_range`, virtio_nvrm.c:1217+; `pin_user_pages_fast` :1251) and sends 16-byte `GpaRun{gpa, len}` records (S/host_pool.rs:223-247).
- **`Arena::build`** (S/host_pool.rs:268-373):
  1. Runs must be page-aligned; their sum must equal `total` exactly; `total` must not exceed `LEA_MAX_PIN_MIB` (default 256 MiB, :200-219).
  2. Reserve one PROT_NONE VA range.
  3. For each run, find its `vm_memory` region and **file offset** (fails unless guest RAM is file-backed with `shared=on`, :322-324), check bounds, then `mmap(MAP_SHARED|MAP_FIXED)` it from the guest RAM file.
  4. Drop munmaps the range (:388-392).
- **Forwarded 0x71:** `arena_for_osdesc` (:738-747) charges the `PinBudget` (`reserve` :82-102, page-rounded :188-197) and returns the host VA. RM pins it when the forwarded NVOS02 runs. The arena is kept per `(root, handle)` (:749-772). After a source free it is *retained* (still charged), because DUP/export aliases are not tracked (:104-113, 756-760).
- **UVM pools:** `back_pool_for::<A>` (:806-870):
  1. Refuse overlap for the same token (:820-824).
  2. Build the arena and dup the UVM FD.
  3. `Backing::os_descriptor` (NVOS02 `NV01_MEMORY_SYSTEM_OS_DESCRIPTOR` on the registered GPU FD, flags `OSDESC_FLAGS` :170, :486-504).
  4. DUP grant (:506-514).
  5. `UVM_CREATE_EXTERNAL_RANGE` (:567-580).
  6. `UVM_MAP_EXTERNAL_ALLOCATION` with the private client's `rmCtrlFd`/`hClient`/`hMemory` (:582-602).
  - Cleanup order is UVM_FREE, then RM_FREE, then arena (`PoolMap::cleanup` :657-673). Failures are quarantined and retried (:115-139, 872-876, 912-926).
- **PinBudget:** VM-wide limit `LEA_MAX_PIN_TOTAL_MIB`, default 1 GiB (:55-63). It counts pending, active, retained and quarantined registrations, not unique pages (docs/virtio/architecture.md:83-93).

### 3.3 Events (`waiters.rs` plus the `nvrm.rs` epoll)
- **Two event paths:**
  - Guest OS-event FDs are watched edge-triggered through a nested epoll (S/nvrm.rs:257-314).
  - Per-client private event ctls are level-triggered and drained with GET_EVENT_DATA (`drain_os_events`, S/session.rs:885-965).
- **Delivery:** `on_poll` turns each firing into a `KIND_EVENT_FIRED` `Req` (class, token, hClient, hEvent, notifyIndex, guest callback pointer in `addr`). `push_event` writes it into a guest-posted queue-1 buffer; missing buffers are counted as drops (S/nvrm.rs:474-621, 412-470).
- **Semaphore-surface waiters** are dataless one-shot events. A dedicated `poll(2)` thread handles them (S/waiters.rs:1-13, `run` :211-340):
  - It uses acknowledged `Watch`, `CancelSlots` and `CancelProcess` commands (:152-172).
  - Each arm carries a `RegistrationId{event_id, generation}` (:48-58).
  - It signals completion through an eventfd that sits in the epoll set (S/nvrm.rs:232-241, 569-606).
  - The session retires completed arms via `semsurf_wake` (S/session.rs:689-698).

### 3.4 VRAM ledger (`vram.rs`)
- **What is charged:** only class 0x40, non-virtual, VIDMEM, size > 0 (`request_bytes` S/vram.rs:367-390), plus VID_HEAP ALLOC_SIZE (:399-478). The reservation is kept only if RM returns VIDMEM (`Books::settle` :963-977).
- **Policies:** Off, Accounting, Reserved or Grid, chosen from environment variables (:57-71, 328). Grid profiles come from `nvrm_abi::vgpu` (A/vgpu.rs).
- **Rewritten controls and why:**
  - FB_GET_INFO(V2) and VID_HEAP INFO: report capped capacity (:1152-1220).
  - GPU name: mark the device as mediated (:1222-1280).
  - GET_PIDS / GET_PID_INFO: return this VM's processes instead of host PIDs (:1047-1107).
- **Over the cap:** the answer is native NV_ERR_NO_MEMORY with ioctl success (S/session.rs:1899-1924).

### 3.5 `grid.rs`
- **Cached card properties:** host UUID and total VRAM, queried once at startup through a Backing client (`host_pool::card`, S/host_pool.rs:529-537; `set_card` S/grid.rs:155-166).
- **Derived per VM:** a stable per-VM UUID from the socket path (S/grid.rs:82-116, 184-196).
- **Rewrites:**
  - UUID swap (binary and ASCII) in UVM params and in three UUID-bearing controls (:117-148).
  - Encoder capacity (:31-38).
  - Opt-in VGX virtualization mode (:18-27, 44-79).

---

## 4. Reuse map for Leandro VFIO

### 4.1 Library surface
`vhost-user-nvrm` is a library plus a binary (crates/vhost-user-nvrm/Cargo.toml `[lib] vhost_user_nvrm`). Public modules: `grid, guest_words, host_pool, mirror, nvrm, session, syscalls, vram, waiters`. `client_policy` and `request_shape` are **private** (S/lib.rs:6-16). The fuzz target already drives `Session::detached_proc` plus `handle_msg` from outside the crate (crates/vhost-user-nvrm/fuzz/fuzz_targets/handle_msg.rs:47).

### 4.2 Reusable directly
- **ABI layer:**
  - `nvrm-sys`: `detect()` (crates/nvrm-sys/src/version.rs:48-58), `dispatch`/`AbiVisitor` (crates/nvrm-sys/src/lib.rs:331-355), and `RmAbi` (:153) as the per-version layout bundle.
  - `nvrm-abi`: `NvDevice`, `iowr_raw`, `check_status` (A/lib.rs:60, 125-251), and every `xlate` rule (sizes, embedded, nested, blocked, FD offsets).
  - `share` (A/share.rs:94-133), `nvgpu` wrapper structs `Nvos02WithFd`/`Nvos33WithFd` (A/nvgpu.rs:189, 210), `mediate` constants.
- **`Session` as the RM executor:** `Session::detached_proc(sub_id, Arc<Ledger>, Arc<PinBudget>)` (S/session.rs:534), `set_guest_mem` (:1025), `set_syscalls` (:1013), `handle_msg_with` (:1042).
  - Only the **wire-byte API** is public; `prepare`, `execute` and `Plan` are private.
  - Leandro VFIO can reuse it by building `nvrm_wire::Req` messages:
    - Open ctl/gpu, which gives tokens.
    - Ioctl with `ioctl_nr=NV_ESC_RM_ALLOC` and NVOS64 inline with `embedded_ptr_off=16`, params in aux.
    - RM_CONTROL with NVOS54 inline, `paramsSize` equal to the aux length, `embedded_ptr_off=16`.
    - RM_FREE with NVOS00.
  - Otherwise it would need a refactor that exposes a typed prepare/execute API.
- **Guest-page backing:**
  - `host_pool::Arena::build(mem, &[GpaRun], total)` (S/host_pool.rs:268), `GpaRun` (:223), `PinBudget::new`/`from_env` (:40, 55), and `PoolState::arena_for_osdesc` / `keep_osdesc_arena` / `drop_osdesc_*` (:738-772).
  - This is a natural fit for GSP-style page-list memory descriptors: GPA list, then host VA, then NVOS02 0x71.
  - `Backing` and the UVM `back_pool_for` are private or UVM-specific.
  - The `Mem` type is `vm_memory::GuestMemoryAtomic<GuestMemoryMmap<()>>` (S/host_pool.rs:21), so Leandro VFIO must build it from the vfio-user DMA regions, and those regions must be file-backed with an offset (:322).
- **Other session-independent pieces:** `vram::{Ledger, Books, request_bytes, rewrite_*}`, `grid::{Card, rewrite_*}`, `mirror::Mirror`, `guest_words`, `waiters::WaiterPoller`.

### 4.3 Tied to the vhost-user transport (needs a new front end)
- All of `nvrm.rs`:
  - `NvrmDevice::new` is private; only `serve(socket)` is public (S/nvrm.rs:191, 1261).
  - It includes the `VhostUserBackendMut` implementation (:1169-1257), virtqueue I/O, event push to queue 1, window/SHMEM_MAP, and PROC_GONE.
  - `serve_with` also fixes the startup order: `grid::set_card(socket, card())` must run before `Ledger::new()`, which reads `grid::card()` (S/nvrm.rs:1278-1279; S/vram.rs:633-636).
- MapPrepare/window handling: replace it with a vfio-user BAR region (sparse mmap of the RM-mapped FD). The probe-mmap pattern (S/nvrm.rs:994-1012) is worth copying.

### 4.4 Assumptions that break for GSP RPCs
1. **Envelope shape.** Leandro validates NVOS64/NVOS21/NVOS54 ioctl envelopes with a pointer to params. GSP RPCs carry params inline:
   - `rpc_gsp_rm_alloc_v03_00{hClient, hParent, hObject, hClass, status, paramsSize, flags, params[]}` (vendor/open-gpu-kernel-modules/src/nvidia/generated/g_rpc-structures.h:1408-1419)
   - `rpc_gsp_rm_control_v03_00{hClient, hObject, cmd, status, paramsSize, rmapiRpcFlags, rmctrlFlags, rmctrlAccessRight, params[]}` (:1423-1435)
   - A shim is needed. `nvrm-sys` binds no RPC structs: crates/nvrm-sys/wrapper.h and the abi.toml footprint (:113+) contain no rpc/GSP headers.
2. **Serialized params.** CPU-RM FINN-serializes embedded-pointer controls and allocs for the RPC (vendor .../src/nvidia/src/kernel/vgpu/rpc.c:10735-10806, 11019-11044). Leandro refuses serialized payloads (S/request_shape.rs:164-176). The nested-pointer tables (A/xlate.rs:1240-1375) only describe unserialized NVOS54 layouts.
3. **Privilege.** The backend is an unprivileged userspace RM client (CAP_SYS_ADMIN dropped, crates/vhost-user-nvrm/src/main.rs:50+). The guest kernel RM will issue kernel-only and internal controls (`NV2080_CTRL_CMD_INTERNAL_*`), plus non-alloc/control RPCs (system info, registry, static info, and so on). These have no ioctl equivalent in Leandro.
4. **Identity.** `guest_proc`/Session and `ProcInfo` assume guest processes (W/lib.rs:105-112, 228-229). With GSP there is one kernel-RM caller, so Leandro VFIO needs one session per VM or per guest `hClient`. SET_SUB_PROCESS_ID labeling (S/session.rs:2360-2387) and the PID roster (S/vram.rs:715, 790) lose their meaning.
5. **Handles.** Leandro forwards guest handles verbatim and lets host RM choose root handles (`hObjectNew=0` pattern, crates/nvrm-client/src/lib.rs:55-60). Guest CPU-RM chooses its own `hClient` values, so Leandro VFIO likely needs a guest-to-host handle map. Leandro has none.
6. **FD tokens.** GSP RPCs have no FDs. `Mirror` is still needed internally, because RM binds clients to the open file description: one ctl FD per client, plus REGISTER_FD'd GPU FDs for mappings and OS descriptors. The GPU index is hard-coded to 0 in `osdesc_gpu_fd` (S/session.rs:975) and `Backing::new` (S/host_pool.rs:415).
7. **Events.** Leandro substitutes guest kernel callbacks with OS events and delivers `KIND_EVENT_FIRED` on virtio queue 1. Leandro VFIO needs a GSP event/message-queue equivalent. The draining and substitution logic (S/session.rs:594-661, 885-965) is reusable conceptually but reached only through the wire API.
8. **Mediation hooks.** The control rewrites (FB info, name, PIDs, UUID) key on guest-issued NVOS54 controls (S/session.rs:2125-2202). A GSP guest gets most of its static card data through static-info RPCs instead, so these hooks may never fire (see §6).

### 4.5 `nvrm-client`
Direct-client diagnostics: `RmClient`, `HandleAllocator` with `0x8000_0000` daemon-handle bit, `ObjectTree` free order, `map_cpu` via NVOS33 + mmap (crates/nvrm-client/src/lib.rs:28-177; handle.rs:8-40; object.rs; mem.rs:222-286). These are useful building blocks for host-owned clients, but `RmClient::new` panics unless the running driver equals `DRIVER_VERSION` (lib.rs:38-41).

### 4.6 How the bindings are generated
- `cargo xtask abi` reads `abi.toml` (versions at :49-95; footprint regex allowlists at :113+ with `.mediated` :263, `.renamed` :317, `.mediated_constants` :342).
- It runs bindgen per version over `wrapper.h` with `INCLUDE_DIRS` (crates/xtask/src/abi/mod.rs:22-28, `run_bindgen` :302-325).
- It parses layout assertions into `manifests/<v>.json` (crates/xtask/src/abi/manifest.rs:4-13) and classifies pairs as identical, append-only or breaking (crates/xtask/src/abi/classify.rs:4-5).
- It emits `stable.rs`, `v<NNN>.rs`, `lib.rs` with `RmAbi`, and `versions.toml` (crates/xtask/src/abi/emit.rs:4-11, 441; `refuse_on_breaking` :839).
- **Adding RPC structs** means a footprint pattern plus including `g_rpc-structures.h`-style headers in `wrapper.h`, and `INCLUDE_DIRS` would need the generated directory.

---

## 5. Constraints from docs/virtio/future.md and docs/virtio/open-questions.md
- "QEMU integration, snapshots, suspend and migration remain unimplemented" (docs/virtio/future.md:35; also docs/virtio/virtio-uapi.md:51). A remote or other transport needs a replacement for local shared mappings (docs/virtio/future.md:42-43).
- Multi-GPU needs explicit GPU identity in tables, sessions and policy (docs/virtio/future.md:34). Today GPU 0 is hard-coded (see §4.4 item 6).
- Mapping acknowledgement: correctness depends on REPLY_ACK, and the backend does not enforce it (docs/virtio/architecture.md:77; docs/virtio/future.md:14; docs/virtio/security.md:51).
- Teardown and ownership:
  - live unbind is unsupported (#82, docs/virtio/open-questions.md:113-118)
  - SHMEM/NVKMS teardown barriers are missing (docs/virtio/future.md:12)
  - a source FREE is not a release fence for backing pages (#80, :99-104; docs/virtio/architecture.md:91-93)
  - the pin budget can be exhausted by retained registrations (#83, :120-125)
  - source-handle VM ownership is missing (#76, :72-77)
- Reloading the guest module needs a fresh backend; reconnecting to retained state is unsupported (docs/virtio/security.md:47).
- The cached OS-descriptor GPU FD is associated with the first ctl FD only (#79, docs/virtio/open-questions.md:93-97).
- VRAM accounting is not a physical reservation (#68).
- A branch is not a layout: R610 carries two layouts and only exact supported versions are accepted (#72; crates/nvrm-sys/abi.toml:36-40; crates/nvrm-sys/src/lib.rs:105-116).
- Hostile guests are not supported (docs/virtio/security.md:6-7).

## 6. Open questions
1. Can a host userspace client allocate `NV01_ROOT_CLIENT` with a caller-chosen `hObjectNew`, so guest CPU-RM client handles could pass through untranslated? Leandro always lets RM choose (crates/nvrm-client/src/lib.rs:55-60; S/session.rs:2331-2333).
2. Does the host ioctl path accept `NVOS54_FLAGS_FINN_SERIALIZED` from a userspace client, which would let serialized RPC params be forwarded as-is? Or must Leandro VFIO deserialize to NVOS54 plus nested buffers? Leandro refuses both forms (S/request_shape.rs:164-176); the vendor RPC path is at rpc.c:10735-10806.
3. Which GSP RPCs have no RM ioctl equivalent, and which controls in them require kernel or internal privilege (compare A/xlate.rs:1166-1203, which already blocks GSP_CRYPTO_CONTROL and RPC_GSP_TEST)?
4. Can the typed `prepare`/`execute` be exposed (S/session.rs:1294, 1894) instead of re-encoding `nvrm-wire` bytes, and should `request_shape`/`client_policy` become `pub` (S/lib.rs:6, 12)?
5. vfio-user DMA regions must be convertible to a `GuestMemoryMmap` with file offsets for `Arena::build` (S/host_pool.rs:318-324). Is guest RAM file-backed in the planned QEMU setup?
6. Do the VRAM and UUID mediations still apply under a GSP guest, which gets card data from static-info RPCs rather than FB_GET_INFO or GID controls (S/session.rs:2125-2202; S/grid.rs:144-148)?
7. What do events and semaphore waiters look like on the GSP side, and what replaces queue-1 `KIND_EVENT_FIRED` (S/nvrm.rs:474-621; W/lib.rs:69-101)?
8. What is the per-client ctl-FD and OFD model when all guest clients come from one kernel caller? This is the #79 caching issue (S/session.rs:967-999) and the event-ctl-per-client growth in #31.
