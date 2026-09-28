// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Memory the kernel RM registers with GSP-RM (`ALLOC_MEMORY`, RPC 4) and the GSP-side
//! event buffers built on it (docs/caraxes/gsp-plan/rpc-functions.md §4.1,
//! docs/caraxes/gsp-plan/classes.md §3, last item).
//!
//! Paths are relative to the open-gpu-kernel-modules tree at tag 615.71.09.
//!
//! `memRegisterWithGsp` (src/nvidia/src/kernel/mem_mgr/mem.c:513-585) sends one
//! `rpc_alloc_memory_v13_01` (src/nvidia/generated/g_rpc-structures.h:71-83) for any
//! memory allocated with `NVOS32_ATTR2_REGISTER_MEMDESC_TO_PHYS_RM`: the client, device
//! and memory handles, the class `NV01_MEMORY_LIST_SYSTEM` or `_LIST_FBMEM`
//! (mem.c:545-551), and the page list as a `pte_desc`
//! (src/nvidia/inc/kernel/vgpu/sdk-structures.h:212-220) of PFNs, one for a contiguous
//! buffer (src/nvidia/src/kernel/vgpu/rpc.c:3516-3526, 2313-2366). GSP-RM creates a
//! Memory object under the same handles; later GSP-side objects name it by handle, and
//! the kernel frees it with `FREE` (mem.c:588-615). Here the registration is recorded,
//! every page must lie in the guest's RAM (system memory) or in the device's FB (FB
//! memory), and nothing is ever mapped on the host.
//!
//! From 615 the GSP operational-event import (src/nvidia/src/kernel/gpu/gpu_op_event_import.c,
//! started at every state load of a GSP client, src/nvidia/src/kernel/gpu/gpu.c:2796)
//! registers three such buffers (steps 3a-3c, :593-620), allocates an `NV_EVENT_BUFFER`
//! (0x90CD) on them through the physical RM API (step 5b, :413-440, :642-650), enables
//! it (`NV_EVENT_BUFFER_CTRL_CMD_ENABLE_EVENTS`, step 7, :492-509) and binds it as the
//! operational-event sink (`NV_OPERATIONAL_EVENT_BUFFER_BIND` 0x90D0, step 8,
//! :517-548); later it pushes its get pointer with `NV_EVENT_BUFFER_CTRL_CMD_UPDATE_GET`
//! (:1267-1290). The device is the producer of that buffer: it validates the objects as
//! GSP-RM would, and writes a GOE record ([`OpEvent`], [`GspMemory::produce`]) for every
//! operational event it raises itself (an RC it reports for a guest channel, boot.rs), into
//! the registered guest memory, then wakes the kernel with `POST_EVENT` on the import's
//! callback event (the bind's handle, :334-373); the kernel drains
//! `[recordGet, recordPut)` and answers with `UPDATE_GET` (:1300-1458), which clears the
//! producer's notify latch. Nothing of the host's own event log ever reaches it.

use std::collections::BTreeMap;

use crate::guestmem::GuestMemory;

// NV_STATUS values (src/common/sdk/nvidia/inc/nvstatuscodes.h).
pub const NV_OK: u32 = 0x00;
pub const NV_ERR_INSERT_DUPLICATE_NAME: u32 = 0x19;
pub const NV_ERR_INVALID_ADDRESS: u32 = 0x1E;
pub const NV_ERR_INVALID_ARGUMENT: u32 = 0x1F;
pub const NV_ERR_INVALID_CLASS: u32 = 0x22;
pub const NV_ERR_INVALID_LIMIT: u32 = 0x2E;
pub const NV_ERR_INVALID_OBJECT_HANDLE: u32 = 0x33;
pub const NV_ERR_INVALID_OBJECT_PARENT: u32 = 0x36;
pub const NV_ERR_NOT_SUPPORTED: u32 = 0x56;
pub const NV_ERR_OBJECT_NOT_FOUND: u32 = 0x57;

/// `NV_VGPU_MSG_FUNCTION_ALLOC_MEMORY` (src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:14).
pub const FN_ALLOC_MEMORY: u32 = 4;

// rpc_alloc_memory_v13_01 (g_rpc-structures.h:71-83; 56 bytes, the same in 610 and 615,
// docs/caraxes/gsp-plan/rpc-functions.md §3 row 4).
pub const AM_H_CLIENT: usize = 0;
pub const AM_H_DEVICE: usize = 4;
pub const AM_H_MEMORY: usize = 8;
pub const AM_H_CLASS: usize = 12;
pub const AM_FLAGS: usize = 16;
pub const AM_PTE_ADJUST: usize = 20;
pub const AM_FORMAT: usize = 24;
pub const AM_LENGTH: usize = 32;
pub const AM_PAGE_COUNT: usize = 40;
/// `pteDesc`: idr 1:0, reserved 15:2, length 31:16, then the 8-byte-aligned PFN array.
pub const AM_PTE_DESC: usize = 48;
pub const AM_PTES: usize = 56;
pub const AM_SIZE: usize = 56;
/// `NV_VGPU_PTEDESC_IDR_NONE`: the array holds PTEs, not PDEs (rpc.c:2355).
pub const PTEDESC_IDR_NONE: u32 = 0;
/// `RM_PAGE_SHIFT`: the PFNs are 4 KiB frame numbers (rpc.c:2360).
pub const RM_PAGE_SHIFT: u32 = 12;

/// The bytes a complete `ALLOC_MEMORY` request carries: the fixed part plus
/// `pteDesc.length` PFNs (the continuation rule of `_issuePteDescRpc`, rpc.c:2334-2347).
/// `None` while the fixed part is incomplete.
pub fn alloc_memory_size(payload: &[u8]) -> Option<usize> {
    let d = u32::from_le_bytes(payload.get(AM_PTE_DESC..AM_PTE_DESC + 4)?.try_into().ok()?);
    Some(AM_PTES + 8 * (d >> 16) as usize)
}

// Classes (src/common/sdk/nvidia/inc/class/cl0071.h ... g_allclasses.h).
pub const NV01_MEMORY_LIST_SYSTEM: u32 = 0x81;
pub const NV01_MEMORY_LIST_FBMEM: u32 = 0x82;
pub const NV_EVENT_BUFFER: u32 = 0x90CD;
pub const NV_OPERATIONAL_EVENT_BUFFER_BIND: u32 = 0x90D0;

// NV_EVENT_BUFFER_ALLOC_PARAMETERS (class/cl90cd.h:164-180), 72 bytes.
pub const EB_RECORD_SIZE: usize = 16;
pub const EB_RECORD_COUNT: usize = 20;
pub const EB_VARDATA_SIZE: usize = 32;
pub const EB_H_SUBDEVICE: usize = 52;
pub const EB_FLAGS: usize = 56;
pub const EB_H_HEADER: usize = 60;
pub const EB_H_RECORDS: usize = 64;
pub const EB_H_VARDATA: usize = 68;
pub const EB_PARAMS_SIZE: usize = 72;
/// `sizeof(NV_EVENT_BUFFER_HEADER)` (class/cl90cd.h:65-71): the producer's ring cursors.
pub const EB_HEADER_BYTES: u64 = 32;

// NV_OPERATIONAL_EVENT_BUFFER_BIND_ALLOC_PARAMETERS (class/cl90d0.h:157-168), 104 bytes.
pub const BIND_H_EVENT_BUFFER: usize = 0;
pub const BIND_RECORD_FORMAT: usize = 4;
pub const BIND_SCOPE: usize = 5;
pub const BIND_BIND_ID: usize = 8;
pub const BIND_PARAMS_SIZE: usize = 104;
/// `NV_OPERATIONAL_EVENT_BUFFER_RECORD_FORMAT_GOE` and `_CPER` (cl90d0.h:46-47).
pub const RECORD_FORMAT_GOE: u8 = 1;
pub const RECORD_FORMAT_CPER: u8 = 2;
/// `NV_OPERATIONAL_EVENT_BUFFER_SCOPE_DEVICE` (cl90d0.h:53); the MIG scopes need MIG,
/// which TU106 does not have.
pub const SCOPE_DEVICE: u8 = 0;

// Event-buffer controls (src/common/sdk/nvidia/inc/ctrl/ctrl90cd.h).
pub const CMD_EVENT_BUFFER_ENABLE_EVENTS: u32 = 0x90CD_0101;
pub const CMD_EVENT_BUFFER_UPDATE_GET: u32 = 0x90CD_0102;
pub const CMD_EVENT_BUFFER_VERIF_NOTIFY: u32 = 0x90CD_0103;
pub const CMD_EVENT_BUFFER_FLUSH: u32 = 0x90CD_0104;
/// `NV_EVENT_BUFFER_FLAG_OVERFLOW_POLICY` 1:0: DISABLED 0, KEEP_OLDEST 1, KEEP_NEWEST 2
/// (ctrl90cd.h:77-81).
pub const OVERFLOW_POLICY_MAX: u32 = 2;

/// One memory registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registered {
    pub h_device: u32,
    pub class: u32,
    /// The OS02 flags the kernel converted from the allocation's attributes (mem.c:556-563).
    pub flags: u32,
    pub pte_adjust: u32,
    pub format: u32,
    pub length: u64,
    /// Physical addresses of the pages (4 KiB frames), or one for a contiguous buffer.
    pub pages: Vec<u64>,
}

impl Registered {
    /// Bytes the registration covers from its first page (the page offset included).
    pub fn span(&self) -> u64 {
        self.pte_adjust as u64 + self.length
    }
}

/// An `NV_EVENT_BUFFER` the device produces into.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventBuffer {
    pub h_subdevice: u32,
    /// The registered header, record and vardata memories.
    pub memories: [u32; 3],
    pub record_size: u32,
    pub record_count: u32,
    pub vardata_size: u32,
    /// `ENABLE_EVENTS`: on, and the overflow policy.
    pub enabled: Option<u32>,
    /// The consumer's last `UPDATE_GET`: record and vardata get offsets.
    pub get: (u32, u32),
    /// The producer's record put index and vardata put offset.
    pub put: (u32, u32),
    /// A `POST_EVENT` went to the consumer and no `UPDATE_GET` followed yet: no further
    /// wake-up until one does (gpu_op_event_import.c:977-985).
    pub notify_pending: bool,
    /// Records written, and records and vardata dropped for a full ring (KEEP_OLDEST).
    pub written: u64,
    pub record_drops: u64,
    pub vardata_drops: u64,
}

/// An `NV_OPERATIONAL_EVENT_BUFFER_BIND`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bind {
    pub h_event_buffer: u32,
    pub record_format: u8,
    pub bind_id: u8,
    /// The filters of the bind (cl90d0.h:118-126): whole-group log level and severity.
    pub min_log_level: u8,
    pub min_severity: u8,
    /// The GPU the bind is for (`resourceUuid`): the GPU UUID the guest kernel sees.
    pub uuid: [u8; 16],
}

// NV_OPERATIONAL_EVENT_BUFFER_BIND_ALLOC_PARAMETERS filters and UUID (cl90d0.h:157-168).
pub const BIND_MIN_LOG_LEVEL: usize = 6;
pub const BIND_MIN_SEVERITY: usize = 7;
pub const BIND_UUID: usize = 12;

// NV_EVENT_BUFFER_HEADER (cl90cd.h:65-71): recordGet @0 (the consumer's), recordPut @4,
// recordCount @8, recordDropcount @16, vardataDropcount @24.
pub const HDR_RECORD_PUT: u64 = 4;
pub const HDR_RECORD_COUNT: u64 = 8;
pub const HDR_RECORD_DROPS: u64 = 16;
pub const HDR_VARDATA_DROPS: u64 = 24;

/// `NV_EVENT_BUFFER_RECORD_TYPE_OPERATIONAL_EVENT_GOE` (cl90cdtypes.h:43) and
/// `NV_OPERATIONAL_EVENT_RECORD_SUBTYPE_GOE_V1` (cl90d0.h:72).
pub const RECORD_TYPE_GOE: u16 = 17;
pub const RECORD_SUBTYPE_GOE_V1: u16 = 1;
/// `NV_EVENT_VARDATA_GRANULARITY` and `_START_OFFSET_ZERO` (cl90cd.h:103-105).
pub const VARDATA_GRANULARITY: u32 = 32;
pub const VARDATA_START_ZERO: u32 = 1;
/// `sizeof(NV_GPU_OPERATIONAL_EVENT_RECORD)` (cl90d0.h:280-291): the operational record
/// (80 bytes: header, traceId @8, instanceId @16, timestamp @24, moduleSignature\[16\] @32,
/// eventCategory @48, moduleEventCode @50, severity @52, attributes @53,
/// groupAttributes @54, bindId @55, cursorValue @56, numContexts @64, totalContextSize
/// @68, groupCperSize @72, groupSize @76, groupIndex @77), then deviceUuid\[16\] @80,
/// scope @96, originator @97, moduleInstance @98, chipletId @99, migAttribution @100,
/// logLevel @101, reportingSource @102.
pub const GOE_RECORD_SIZE: usize = 104;
/// `GPU_OPERATIONAL_EVENT_CTX_TYPE_GPU_LEGACY_XID` 0x8001, version 1.0
/// (interface/events/gpu/gpu_event_ctx_defs.h:37, 62-77): {xidCode u32, message[] NUL
/// terminated, at most 235 characters}.
pub const CTX_LEGACY_XID: u16 = 0x8001;
pub const CTX_LEGACY_XID_VERSION: u16 = 0x0100;
pub const CTX_LEGACY_XID_MAX_MSG: usize = 235;
/// `NV_OPERATIONAL_EVENT_CONTEXT` {contextType u16, dataFormatVersion u16, dataSize u32}
/// (cl90d0.h:312-316).
pub const CTX_HEADER: usize = 8;

/// One operational event the device raises (interface/events/gpu/gpu_event_defs.h: the
/// category, scope, originator, reporting source and log level values), with a legacy-Xid
/// context carrying the Xid and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpEvent {
    pub signature: &'static str,
    pub category: u16,
    pub code: u16,
    pub severity: u8,
    pub attributes: u8,
    pub group_attributes: u8,
    pub scope: u8,
    pub originator: u8,
    pub reporting_source: u8,
    pub log_level: u8,
    pub timestamp_us: u64,
    pub xid: u32,
    pub message: String,
}

// Values of gpu_event_defs.h and cl90d0.h.
pub const CATEGORY_ENGINE_ERROR: u16 = 0x0009;
pub const CATEGORY_TRANSLATION_FAULT: u16 = 0x000B;
pub const CATEGORY_DEBUG: u16 = 0xFFFF;
pub const SEVERITY_INFORMATIONAL: u8 = 10;
pub const SEVERITY_RECOVERABLE: u8 = 30;
pub const ATTR_PRIMARY: u8 = 1 << 5;
pub const GROUP_ATTR_SIMULATED: u8 = 1 << 2;
pub const SCOPE_ENGINE: u8 = 4;
pub const SCOPE_DEVICE_EVENT: u8 = 1;
pub const ORIGINATOR_PF_GSP_FW: u8 = 2;
pub const SOURCE_INTERRUPT: u8 = 1;
pub const SOURCE_SW_CHECK: u8 = 0;
pub const LOG_LEVEL_DIAGNOSTIC: u8 = 20;
pub const LOG_LEVEL_ERROR: u8 = 50;
/// `ROBUST_CHANNEL_FIFO_ERROR_MMU_ERR_FLT` (nverror.h:49): Xid 31.
pub const XID_MMU_FAULT: u32 = 31;
/// `ROBUST_CHANNEL_GR_EXCEPTION` (nverror.h): Xid 13, a graphics engine exception.
pub const XID_GR_EXCEPTION: u32 = 13;
/// `ROBUST_CHANNEL_PREEMPTIVE_REMOVAL` (nverror.h): Xid 45, also the notice the kernel
/// writes into a channel's error notifier after its `STOP_CHANNEL` (kernel_channel.c:1903).
pub const XID_PREEMPTIVE_REMOVAL: u32 = 45;

impl OpEvent {
    /// The event of an RC the device reports for a guest channel (events.md R1): an engine
    /// error, a translation fault for Xid 31; recoverable, primary, engine scope, raised as
    /// GSP firmware would from the fault interrupt.
    pub fn rc(xid: u32, timestamp_us: u64, message: String) -> Self {
        OpEvent {
            signature: "GPU-RC",
            category: if xid == XID_MMU_FAULT {
                CATEGORY_TRANSLATION_FAULT
            } else {
                CATEGORY_ENGINE_ERROR
            },
            code: xid as u16,
            severity: SEVERITY_RECOVERABLE,
            attributes: ATTR_PRIMARY,
            group_attributes: 0,
            scope: SCOPE_ENGINE,
            originator: ORIGINATOR_PF_GSP_FW,
            reporting_source: SOURCE_INTERRUPT,
            log_level: LOG_LEVEL_ERROR,
            timestamp_us,
            xid,
            message,
        }
    }

    /// The device's self-test event (`CARAXES_GOE_SELFTEST`): a debug-category group
    /// marked SIMULATED (`NV_OPERATIONAL_EVENT_GROUP_ATTR_SIMULATED`, cl90d0.h:109), so no
    /// consumer takes it for a real error.
    pub fn selftest(n: u64, timestamp_us: u64) -> Self {
        OpEvent {
            signature: "CARAXES-TEST",
            category: CATEGORY_DEBUG,
            code: 1,
            severity: SEVERITY_INFORMATIONAL,
            attributes: ATTR_PRIMARY,
            group_attributes: GROUP_ATTR_SIMULATED,
            scope: SCOPE_DEVICE_EVENT,
            originator: ORIGINATOR_PF_GSP_FW,
            reporting_source: SOURCE_SW_CHECK,
            log_level: LOG_LEVEL_DIAGNOSTIC,
            timestamp_us,
            xid: 0,
            message: format!("Caraxes device self-test operational event {n} (simulated)"),
        }
    }

    /// The vardata of the event: one legacy-Xid context, padded to the granularity.
    /// Returns (bytes, unpadded length = `totalContextSize`).
    pub fn vardata(&self) -> (Vec<u8>, u32) {
        let msg = self.message.as_bytes();
        let msg = &msg[..msg.len().min(CTX_LEGACY_XID_MAX_MSG)];
        let data_size = 4 + msg.len() + 1;
        let total = CTX_HEADER + data_size;
        let padded = total.div_ceil(VARDATA_GRANULARITY as usize) * VARDATA_GRANULARITY as usize;
        let mut b = vec![0u8; padded];
        b[0..2].copy_from_slice(&CTX_LEGACY_XID.to_le_bytes());
        b[2..4].copy_from_slice(&CTX_LEGACY_XID_VERSION.to_le_bytes());
        b[4..8].copy_from_slice(&(data_size as u32).to_le_bytes());
        b[8..12].copy_from_slice(&self.xid.to_le_bytes());
        b[12..12 + msg.len()].copy_from_slice(msg);
        (b, total as u32)
    }

    /// The GOE record of the event for a bind (`bind_id`, `uuid`) with the group identity
    /// `cursor` (one record per group: groupSize 1, groupIndex 0), its vardata window
    /// ending at `var_data` (the header's `varData` word).
    pub fn record(&self, record_size: usize, bind_id: u8, uuid: &[u8; 16], cursor: u64, var_data: u32, total_ctx: u32) -> Vec<u8> {
        let mut r = vec![0u8; record_size.max(GOE_RECORD_SIZE)];
        let put16 = |r: &mut [u8], o: usize, v: u16| r[o..o + 2].copy_from_slice(&v.to_le_bytes());
        let put32 = |r: &mut [u8], o: usize, v: u32| r[o..o + 4].copy_from_slice(&v.to_le_bytes());
        let put64 = |r: &mut [u8], o: usize, v: u64| r[o..o + 8].copy_from_slice(&v.to_le_bytes());
        put16(&mut r, 0, RECORD_TYPE_GOE);
        put16(&mut r, 2, RECORD_SUBTYPE_GOE_V1);
        put32(&mut r, 4, var_data);
        // traceId: originator-prefixed (the originator in the top byte), the cursor below.
        put64(&mut r, 8, (self.originator as u64) << 56 | cursor);
        put64(&mut r, 16, cursor);
        put64(&mut r, 24, self.timestamp_us);
        let sig = self.signature.as_bytes();
        r[32..32 + sig.len().min(15)].copy_from_slice(&sig[..sig.len().min(15)]);
        put16(&mut r, 48, self.category);
        put16(&mut r, 50, self.code);
        r[52] = self.severity;
        r[53] = self.attributes;
        r[54] = self.group_attributes;
        r[55] = bind_id;
        put64(&mut r, 56, cursor);
        put32(&mut r, 64, 1);
        put32(&mut r, 68, total_ctx);
        put32(&mut r, 72, 0);
        r[76] = 1;
        r[77] = 0;
        r[80..96].copy_from_slice(uuid);
        r[96] = self.scope;
        r[97] = self.originator;
        r[98] = 0;
        r[99] = 0;
        r[100] = 0xFF; // GPU_OPERATIONAL_EVENT_MIG_ATTRIBUTION_NONE
        r[101] = self.log_level;
        r[102] = self.reporting_source;
        r
    }
}

/// Write `data` at byte `off` of a registered system-memory buffer (its page list, the
/// first page's offset included); `false` when a page is not guest RAM or the range leaves
/// the registration.
pub fn write_registered(mem: &GuestMemory, r: &Registered, off: u64, data: &[u8]) -> bool {
    if r.class != NV01_MEMORY_LIST_SYSTEM || off + data.len() as u64 > r.length {
        return false;
    }
    let page = 1u64 << RM_PAGE_SHIFT;
    let mut at = r.pte_adjust as u64 + off;
    let mut rest = data;
    while !rest.is_empty() {
        let pa = if r.pages.len() == 1 {
            r.pages[0] + at
        } else {
            match r.pages.get((at / page) as usize) {
                Some(p) => p + at % page,
                None => return false,
            }
        };
        let n = if r.pages.len() == 1 {
            rest.len()
        } else {
            rest.len().min((page - at % page) as usize)
        };
        if !mem.write(pa, &rest[..n]) {
            return false;
        }
        rest = &rest[n..];
        at += n as u64;
    }
    true
}

/// What an alloc or control answered.
#[derive(Debug)]
pub struct Reply {
    pub status: u32,
    pub params: Vec<u8>,
    pub note: String,
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4)
        .map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

fn u64_at(b: &[u8], o: usize) -> u64 {
    b.get(o..o + 8)
        .map_or(0, |s| u64::from_le_bytes(s.try_into().unwrap()))
}

/// The registry, keyed by (hClient, handle): handles are per client in RM.
#[derive(Debug, Default)]
pub struct GspMemory {
    pub memories: BTreeMap<(u32, u32), Registered>,
    pub buffers: BTreeMap<(u32, u32), EventBuffer>,
    pub binds: BTreeMap<(u32, u32), Bind>,
    next_bind_id: u8,
    /// `CARAXES_GOE_SELFTEST=<seconds>`: a SIMULATED debug event every so often once a
    /// GOE bind exists, for the guest probe (caraxes/tools/goe-probe.sh). Off by default.
    pub selftest: Option<SelfTest>,
}

/// The self-test event's schedule.
#[derive(Debug, Clone)]
pub struct SelfTest {
    pub period: std::time::Duration,
    pub last: Option<std::time::Instant>,
    pub sent: u64,
}

impl GspMemory {
    pub fn new() -> Self {
        let selftest = std::env::var("CARAXES_GOE_SELFTEST")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|s| *s > 0)
            .map(|s| SelfTest {
                period: std::time::Duration::from_secs(s),
                last: None,
                sent: 0,
            });
        GspMemory {
            selftest,
            ..Self::default()
        }
    }

    /// The number of the next self-test event when one is due (a GOE bind exists and the
    /// period passed since the last), counting it as sent.
    pub fn selftest_due(&mut self) -> Option<u64> {
        let bound = self.has_goe_bind();
        let t = self.selftest.as_mut()?;
        if !bound || t.last.is_some_and(|l| l.elapsed() < t.period) {
            return None;
        }
        t.last = Some(std::time::Instant::now());
        t.sent += 1;
        Some(t.sent)
    }

    /// `ALLOC_MEMORY`: record the registration after checking what GSP-RM's
    /// memory-list constructor would (a known class, a page list that covers the length)
    /// and what the device must (every page in the guest's RAM for system memory, in the
    /// FB of `fb_bytes` for FB memory; never a host page). Returns the status and a note.
    pub fn alloc_memory(&mut self, payload: &[u8], mem: &GuestMemory, fb_bytes: u64) -> (u32, String) {
        if payload.len() < AM_PTES {
            return (
                NV_ERR_INVALID_ARGUMENT,
                format!("alloc memory: {} bytes, short of the fixed {AM_PTES}", payload.len()),
            );
        }
        let (h_client, h_device, h_memory, class) = (
            u32_at(payload, AM_H_CLIENT),
            u32_at(payload, AM_H_DEVICE),
            u32_at(payload, AM_H_MEMORY),
            u32_at(payload, AM_H_CLASS),
        );
        let desc = u32_at(payload, AM_PTE_DESC);
        let (idr, count) = (desc & 3, (desc >> 16) as usize);
        let r = Registered {
            h_device,
            class,
            flags: u32_at(payload, AM_FLAGS),
            pte_adjust: u32_at(payload, AM_PTE_ADJUST),
            format: u32_at(payload, AM_FORMAT),
            length: u64_at(payload, AM_LENGTH),
            pages: (0..count)
                .map(|i| u64_at(payload, AM_PTES + 8 * i) << RM_PAGE_SHIFT)
                .collect(),
        };
        let what = format!(
            "alloc memory {h_client:#x}/{h_memory:#x} under {h_device:#x} class {class:#x} flags {:#x} length {:#x} adjust {:#x} format {} pages {count} (pageCount {})",
            r.flags,
            r.length,
            r.pte_adjust,
            r.format,
            u32_at(payload, AM_PAGE_COUNT)
        );
        let refuse = |st: u32, why: String| (st, format!("{what}: refused {st:#x}: {why}"));
        if class != NV01_MEMORY_LIST_SYSTEM && class != NV01_MEMORY_LIST_FBMEM {
            // memRegisterWithGsp sends only these two (mem.c:545-551).
            return refuse(NV_ERR_INVALID_CLASS, "not a memory-list class".into());
        }
        if idr != PTEDESC_IDR_NONE || count == 0 || payload.len() < AM_PTES + 8 * count {
            return refuse(
                NV_ERR_INVALID_ARGUMENT,
                format!("pte_desc idr {idr} length {count}, {} bytes", payload.len()),
            );
        }
        if r.length == 0 {
            return refuse(NV_ERR_INVALID_LIMIT, "zero length".into());
        }
        // One PTE: a contiguous buffer (rpc.c:3517-3519); several: one per 4 KiB page.
        let page = 1u64 << RM_PAGE_SHIFT;
        let ranges: Vec<(u64, u64)> = if count == 1 {
            vec![(r.pages[0], r.span())]
        } else {
            if (count as u64) * page < r.span() {
                return refuse(
                    NV_ERR_INVALID_ARGUMENT,
                    format!("{count} pages cannot hold {:#x} bytes", r.span()),
                );
            }
            r.pages.iter().map(|&p| (p, page)).collect()
        };
        for &(pa, len) in &ranges {
            let ok = if class == NV01_MEMORY_LIST_SYSTEM {
                mem.covers(pa, len)
            } else {
                pa.checked_add(len).is_some_and(|end| end <= fb_bytes)
            };
            if !ok {
                return refuse(
                    NV_ERR_INVALID_ADDRESS,
                    format!(
                        "page {pa:#x}+{len:#x} outside the guest's {}",
                        if class == NV01_MEMORY_LIST_SYSTEM { "RAM" } else { "FB" }
                    ),
                );
            }
        }
        if self.memories.contains_key(&(h_client, h_memory)) {
            return refuse(NV_ERR_INSERT_DUPLICATE_NAME, "handle already registered".into());
        }
        let first = r.pages[0];
        self.memories.insert((h_client, h_memory), r);
        (
            NV_OK,
            format!("{what}: registered, first page {first:#x} [guest, recorded; no host mapping]"),
        )
    }

    /// A `GSP_RM_ALLOC` of the two event-buffer classes; `None` for any other class.
    pub fn alloc(
        &mut self,
        h_client: u32,
        h_parent: u32,
        h_object: u32,
        h_class: u32,
        params: &[u8],
    ) -> Option<Reply> {
        let reply = |status: u32, params: Vec<u8>, note: String| Some(Reply { status, params, note });
        match h_class {
            NV_EVENT_BUFFER => {
                let what = format!("event buffer {h_client:#x}/{h_object:#x} under {h_parent:#x}");
                if params.len() < EB_PARAMS_SIZE {
                    return reply(NV_ERR_INVALID_ARGUMENT, params.to_vec(), format!("{what}: {} parameter bytes", params.len()));
                }
                let b = EventBuffer {
                    h_subdevice: u32_at(params, EB_H_SUBDEVICE),
                    memories: [
                        u32_at(params, EB_H_HEADER),
                        u32_at(params, EB_H_RECORDS),
                        u32_at(params, EB_H_VARDATA),
                    ],
                    record_size: u32_at(params, EB_RECORD_SIZE),
                    record_count: u32_at(params, EB_RECORD_COUNT),
                    vardata_size: u32_at(params, EB_VARDATA_SIZE),
                    enabled: None,
                    get: (0, 0),
                    put: (0, 0),
                    notify_pending: false,
                    written: 0,
                    record_drops: 0,
                    vardata_drops: 0,
                };
                // The external backing: every memory registered by this client and large
                // enough for its part (the producer writes the header cursors, the
                // records and the vardata into them).
                let need = [
                    EB_HEADER_BYTES,
                    b.record_size as u64 * b.record_count as u64,
                    b.vardata_size as u64,
                ];
                for (h, n) in b.memories.iter().zip(need) {
                    match self.memories.get(&(h_client, *h)) {
                        Some(m) if m.length >= n => {}
                        Some(m) => {
                            return reply(
                                NV_ERR_INVALID_ARGUMENT,
                                params.to_vec(),
                                format!("{what}: memory {h:#x} holds {:#x} of {n:#x} bytes", m.length),
                            )
                        }
                        None => {
                            return reply(
                                NV_ERR_OBJECT_NOT_FOUND,
                                params.to_vec(),
                                format!("{what}: memory {h:#x} is not registered with GSP"),
                            )
                        }
                    }
                }
                if b.record_size == 0 || b.record_count == 0 {
                    return reply(NV_ERR_INVALID_ARGUMENT, params.to_vec(), format!("{what}: empty record ring"));
                }
                if self.buffers.contains_key(&(h_client, h_object)) {
                    return reply(NV_ERR_INSERT_DUPLICATE_NAME, params.to_vec(), format!("{what}: handle in use"));
                }
                let note = format!(
                    "{what}: {} records of {} bytes, vardata {:#x}, memories {:#x}/{:#x}/{:#x}; the device is the producer of the operational events it raises [guest, recorded]",
                    b.record_count, b.record_size, b.vardata_size, b.memories[0], b.memories[1], b.memories[2]
                );
                self.buffers.insert((h_client, h_object), b);
                reply(NV_OK, params.to_vec(), note)
            }
            NV_OPERATIONAL_EVENT_BUFFER_BIND => {
                let what = format!("operational event bind {h_client:#x}/{h_object:#x}");
                if params.len() < BIND_PARAMS_SIZE {
                    return reply(NV_ERR_INVALID_ARGUMENT, params.to_vec(), format!("{what}: {} parameter bytes", params.len()));
                }
                let h_eb = u32_at(params, BIND_H_EVENT_BUFFER);
                let (format, scope) = (params[BIND_RECORD_FORMAT], params[BIND_SCOPE]);
                if !self.buffers.contains_key(&(h_client, h_eb)) {
                    return reply(NV_ERR_OBJECT_NOT_FOUND, params.to_vec(), format!("{what}: no event buffer {h_eb:#x}"));
                }
                if format != RECORD_FORMAT_GOE && format != RECORD_FORMAT_CPER {
                    return reply(NV_ERR_INVALID_ARGUMENT, params.to_vec(), format!("{what}: record format {format}"));
                }
                if scope != SCOPE_DEVICE {
                    return reply(NV_ERR_NOT_SUPPORTED, params.to_vec(), format!("{what}: scope {scope} (MIG, absent on this chip)"));
                }
                if self.binds.contains_key(&(h_client, h_object)) {
                    return reply(NV_ERR_INSERT_DUPLICATE_NAME, params.to_vec(), format!("{what}: handle in use"));
                }
                // bindId [OUT]: an 8-bit identifier the producer assigns (cl90d0.h:150-151).
                self.next_bind_id = self.next_bind_id.wrapping_add(1).max(1);
                let id = self.next_bind_id;
                let mut out = params.to_vec();
                out[BIND_BIND_ID] = id;
                let mut uuid = [0u8; 16];
                uuid.copy_from_slice(&params[BIND_UUID..BIND_UUID + 16]);
                self.binds.insert(
                    (h_client, h_object),
                    Bind {
                        h_event_buffer: h_eb,
                        record_format: format,
                        bind_id: id,
                        min_log_level: params[BIND_MIN_LOG_LEVEL],
                        min_severity: params[BIND_MIN_SEVERITY],
                        uuid,
                    },
                );
                reply(
                    NV_OK,
                    out,
                    format!("{what}: event buffer {h_eb:#x} format {format} scope device, bindId {id}; the device's own operational events go there [guest, recorded]"),
                )
            }
            _ => None,
        }
    }

    /// The event-buffer controls on a recorded buffer; `None` for anything else.
    pub fn control(&mut self, h_client: u32, h_object: u32, cmd: u32, params: &[u8]) -> Option<Reply> {
        if cmd >> 16 != NV_EVENT_BUFFER {
            return None;
        }
        let Some(b) = self.buffers.get_mut(&(h_client, h_object)) else {
            return Some(Reply {
                status: NV_ERR_INVALID_OBJECT_HANDLE,
                params: params.to_vec(),
                note: format!("event buffer control {cmd:#x}: no event buffer {h_client:#x}/{h_object:#x}"),
            });
        };
        let (status, note) = match cmd {
            // NV_EVENT_BUFFER_CTRL_CMD_ENABLE_EVENTS_PARAMS {NvBool enable; NvU32 flags}
            // (ctrl90cd.h:94-97): enable at 0, flags at 4.
            CMD_EVENT_BUFFER_ENABLE_EVENTS if params.len() >= 8 => {
                let (on, flags) = (params[0] != 0, u32_at(params, 4));
                if flags & 3 > OVERFLOW_POLICY_MAX {
                    (NV_ERR_INVALID_ARGUMENT, format!("enable events: overflow policy {}", flags & 3))
                } else {
                    b.enabled = on.then_some(flags & 3);
                    (NV_OK, format!("enable events {on} policy {}", flags & 3))
                }
            }
            // NV_EVENT_BUFFER_CTRL_CMD_UPDATE_GET_PARAMS {recordBufferGet, varDataBufferGet}
            // (ctrl90cd.h:124-127): NV_ERR_INVALID_ARGUMENT past the sizes.
            CMD_EVENT_BUFFER_UPDATE_GET if params.len() >= 8 => {
                let (r, vd) = (u32_at(params, 0), u32_at(params, 4));
                if r >= b.record_count || vd > b.vardata_size {
                    (NV_ERR_INVALID_ARGUMENT, format!("update get {r}/{vd} past {}/{}", b.record_count, b.vardata_size))
                } else {
                    b.get = (r, vd);
                    // The drain's UPDATE_GET clears the producer's notify latch.
                    b.notify_pending = false;
                    (NV_OK, format!("update get {r}/{vd} (put {}/{}, {} record(s) written)", b.put.0, b.put.1, b.written))
                }
            }
            // The device writes a record completely before it moves recordPut: nothing is
            // ever held back for a flush.
            CMD_EVENT_BUFFER_FLUSH => (NV_OK, "flush: nothing held back".into()),
            // The verification notification is a test hook (ctrl90cd.h:131-134).
            CMD_EVENT_BUFFER_VERIF_NOTIFY => (NV_ERR_NOT_SUPPORTED, "verif notify: test hook, not supported".into()),
            _ => (NV_ERR_INVALID_ARGUMENT, format!("{} parameter bytes", params.len())),
        };
        Some(Reply {
            status,
            params: params.to_vec(),
            note: format!("event buffer {h_client:#x}/{h_object:#x} control {cmd:#x}: {note} [guest]"),
        })
    }

    /// Produce `ev` into every GOE bind whose filters take it: the vardata context, the
    /// record at `recordPut`, then `recordPut` and the counters in the header (the drain
    /// reads `recordPut` last, gpu_op_event_import.c:1345-1348), all in the registered
    /// guest memory. Returns the `POST_EVENT` targets `(hClient, hEvent)` (the bind's
    /// handle is the callback event's, :334-373) of the buffers that got a record and have
    /// no wake-up pending, and a note per bind.
    pub fn produce(&mut self, mem: &GuestMemory, ev: &OpEvent) -> (Vec<(u32, u32)>, Vec<String>) {
        let mut wake = Vec::new();
        let mut notes = Vec::new();
        let binds: Vec<((u32, u32), Bind)> = self.binds.iter().map(|(k, b)| (*k, b.clone())).collect();
        for ((c, h_bind), bind) in binds {
            let what = format!("goe record {} code {} for bind {c:#x}/{h_bind:#x}", ev.signature, ev.code);
            if bind.record_format != RECORD_FORMAT_GOE {
                notes.push(format!("{what}: CPER bind, not produced"));
                continue;
            }
            if ev.log_level < bind.min_log_level || ev.severity < bind.min_severity {
                notes.push(format!("{what}: filtered (log level {} severity {})", bind.min_log_level, bind.min_severity));
                continue;
            }
            let Some(b) = self.buffers.get(&(c, bind.h_event_buffer)).cloned() else {
                continue;
            };
            if b.enabled.is_none() {
                notes.push(format!("{what}: event buffer disabled"));
                continue;
            }
            let (Some(hdr), Some(rec), Some(var)) = (
                self.memories.get(&(c, b.memories[0])).cloned(),
                self.memories.get(&(c, b.memories[1])).cloned(),
                self.memories.get(&(c, b.memories[2])).cloned(),
            ) else {
                notes.push(format!("{what}: backing memory freed"));
                continue;
            };
            let mut nb = b.clone();
            // Record ring: one slot always free (GET == PUT + 1 is full, cl90cd.h:44-47).
            let next = (b.put.0 + 1) % b.record_count;
            let (vbytes, total) = ev.vardata();
            let vlen = vbytes.len() as u32;
            // Vardata ring: after the put offset, or from 0 when the rest of the ring is too
            // short (the record says so with START_OFFSET_ZERO); never over unread data.
            let (vstart, zero) = if b.put.1 + vlen <= b.vardata_size {
                (b.put.1, false)
            } else {
                (0, true)
            };
            let vend = vstart + vlen;
            let used_by_consumer = |s: u32, e: u32| {
                // Unread vardata lies in [get, put) (or wraps); a write into it is a drop.
                let (g, p) = (b.get.1, b.put.1);
                if g == p {
                    false
                } else if g < p {
                    s < p && e > g
                } else {
                    s < p || e > g
                }
            };
            if next == b.get.0 {
                nb.record_drops += 1;
                let _ = write_registered(mem, &hdr, HDR_RECORD_DROPS, &nb.record_drops.to_le_bytes());
                notes.push(format!("{what}: record ring full, dropped (KEEP_OLDEST)"));
                self.buffers.insert((c, bind.h_event_buffer), nb);
                continue;
            }
            if vend > b.vardata_size || (zero && used_by_consumer(0, vend)) || (!zero && used_by_consumer(vstart, vend)) {
                nb.vardata_drops += 1;
                let _ = write_registered(mem, &hdr, HDR_VARDATA_DROPS, &nb.vardata_drops.to_le_bytes());
                notes.push(format!("{what}: vardata ring full, dropped"));
                self.buffers.insert((c, bind.h_event_buffer), nb);
                continue;
            }
            let var_data = vend | if zero { VARDATA_START_ZERO } else { 0 };
            let cursor = b.written + 1;
            let record = ev.record(b.record_size as usize, bind.bind_id, &bind.uuid, cursor, var_data, total);
            let slot = b.put.0 as u64 * b.record_size as u64;
            let ok = write_registered(mem, &var, vstart as u64, &vbytes)
                && write_registered(mem, &rec, slot, &record[..b.record_size as usize])
                && write_registered(mem, &hdr, HDR_RECORD_PUT, &next.to_le_bytes())
                && write_registered(mem, &hdr, HDR_RECORD_COUNT, &(b.written + 1).to_le_bytes());
            if !ok {
                notes.push(format!("{what}: a registered page is not guest RAM; not produced"));
                continue;
            }
            nb.put = (next, vend);
            nb.written += 1;
            let wake_now = !nb.notify_pending;
            nb.notify_pending = true;
            notes.push(format!(
                "{what}: slot {} cursor {cursor}, vardata [{vstart:#x}, {vend:#x}){}, recordPut {next}{} [guest memory]",
                b.put.0,
                if zero { " from 0" } else { "" },
                if wake_now { ", POST_EVENT" } else { ", wake-up pending" }
            ));
            self.buffers.insert((c, bind.h_event_buffer), nb);
            if wake_now {
                wake.push((c, h_bind));
            }
        }
        (wake, notes)
    }

    /// Whether a GOE bind exists (the 615 event import completed).
    pub fn has_goe_bind(&self) -> bool {
        self.binds.values().any(|b| b.record_format == RECORD_FORMAT_GOE)
    }

    /// `FREE`: drop what the handle named; freeing a client drops all of its entries.
    /// A freed event buffer takes its binds along (resserv frees dependants).
    pub fn free(&mut self, h_client: u32, h_object: u32) -> Option<String> {
        let mut dropped = Vec::new();
        if h_client == h_object {
            let before = self.memories.len() + self.buffers.len() + self.binds.len();
            self.memories.retain(|(c, _), _| *c != h_client);
            self.buffers.retain(|(c, _), _| *c != h_client);
            self.binds.retain(|(c, _), _| *c != h_client);
            let n = before - (self.memories.len() + self.buffers.len() + self.binds.len());
            if n > 0 {
                dropped.push(format!("{n} GSP memory object(s) of the client"));
            }
        } else {
            if let Some(m) = self.memories.remove(&(h_client, h_object)) {
                dropped.push(format!("registered memory ({:#x} bytes)", m.length));
            }
            if self.buffers.remove(&(h_client, h_object)).is_some() {
                dropped.push("event buffer".into());
                let before = self.binds.len();
                self.binds
                    .retain(|(c, _), b| !(*c == h_client && b.h_event_buffer == h_object));
                if self.binds.len() < before {
                    dropped.push(format!("{} bind(s)", before - self.binds.len()));
                }
            }
            if self.binds.remove(&(h_client, h_object)).is_some() {
                dropped.push("operational event bind".into());
            }
        }
        (!dropped.is_empty()).then(|| dropped.join(", "))
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn guest(tag: &str) -> (GuestMemory, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("nvgpu-gspmem-{tag}-{}", std::process::id()));
        let f = std::fs::File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        f.set_len(0x10000).unwrap();
        let mut mem = GuestMemory::new();
        mem.map(0x1_0000_0000, 0x10000, 0, &f).unwrap();
        (mem, path)
    }

    /// The request the first 615 boot sent for the import's header buffer
    /// (caraxes/runs/transport-615-2026-09-24/boot/device-events.txt:517: client
    /// 0xc1e00007, device 0xcaf00000, memory 0xcaf00002, class 0x81), with one PTE.
    fn request(h_memory: u32, class: u32, length: u64, pfns: &[u64]) -> Vec<u8> {
        let mut p = vec![0u8; AM_PTES + 8 * pfns.len()];
        p[AM_H_CLIENT..][..4].copy_from_slice(&0xc1e0_0007u32.to_le_bytes());
        p[AM_H_DEVICE..][..4].copy_from_slice(&0xcaf0_0000u32.to_le_bytes());
        p[AM_H_MEMORY..][..4].copy_from_slice(&h_memory.to_le_bytes());
        p[AM_H_CLASS..][..4].copy_from_slice(&class.to_le_bytes());
        p[AM_FLAGS..][..4].copy_from_slice(&0x4c00_0000u32.to_le_bytes());
        p[AM_FORMAT..][..4].copy_from_slice(&6u32.to_le_bytes());
        p[AM_LENGTH..][..8].copy_from_slice(&length.to_le_bytes());
        p[AM_PAGE_COUNT..][..4].copy_from_slice(&(pfns.len() as u32).to_le_bytes());
        p[AM_PTE_DESC..][..4].copy_from_slice(&((pfns.len() as u32) << 16).to_le_bytes());
        for (i, f) in pfns.iter().enumerate() {
            p[AM_PTES + 8 * i..][..8].copy_from_slice(&f.to_le_bytes());
        }
        p
    }

    #[test]
    fn registrations_are_checked_and_recorded() {
        let (mem, path) = guest("reg");
        let mut g = GspMemory::new();
        let r = request(0xcaf0_0002, NV01_MEMORY_LIST_SYSTEM, 32, &[0x10_0000]);
        assert_eq!(alloc_memory_size(&r), Some(AM_PTES + 8));
        let (st, note) = g.alloc_memory(&r, &mem, 1 << 30);
        assert_eq!(st, NV_OK, "{note}");
        assert_eq!(g.memories[&(0xc1e0_0007, 0xcaf0_0002)].pages, vec![0x1_0000_0000]);
        // Again under the same handle: the resource server's duplicate error.
        assert_eq!(g.alloc_memory(&r, &mem, 1 << 30).0, NV_ERR_INSERT_DUPLICATE_NAME);
        // A page outside the guest's RAM (a host address, say) is never accepted.
        let bad = request(0xcaf0_0003, NV01_MEMORY_LIST_SYSTEM, 32, &[0x20_0000]);
        assert_eq!(g.alloc_memory(&bad, &mem, 1 << 30).0, NV_ERR_INVALID_ADDRESS);
        // Discontiguous pages must cover the length, each in guest RAM.
        let two = request(0xcaf0_0004, NV01_MEMORY_LIST_SYSTEM, 0x2000, &[0x10_0002, 0x10_0005]);
        assert_eq!(g.alloc_memory(&two, &mem, 1 << 30).0, NV_OK);
        let short = request(0xcaf0_0005, NV01_MEMORY_LIST_SYSTEM, 0x3000, &[0x10_0002, 0x10_0005]);
        assert_eq!(g.alloc_memory(&short, &mem, 1 << 30).0, NV_ERR_INVALID_ARGUMENT);
        // FB memory is checked against the device's FB, not guest RAM.
        let fb = request(0xcaf0_0006, NV01_MEMORY_LIST_FBMEM, 0x1000, &[0x3_ffff]);
        assert_eq!(g.alloc_memory(&fb, &mem, 1 << 30).0, NV_OK);
        let fb_out = request(0xcaf0_0007, NV01_MEMORY_LIST_FBMEM, 0x2000, &[0x3_ffff]);
        assert_eq!(g.alloc_memory(&fb_out, &mem, 1 << 30).0, NV_ERR_INVALID_ADDRESS);
        let other = request(0xcaf0_0008, 0x3e, 0x1000, &[0x10_0000]);
        assert_eq!(g.alloc_memory(&other, &mem, 1 << 30).0, NV_ERR_INVALID_CLASS);
        // FREE forgets it; a client free forgets the rest.
        assert!(g.free(0xc1e0_0007, 0xcaf0_0002).is_some());
        assert!(g.free(0xc1e0_0007, 0xc1e0_0007).is_some());
        assert!(g.memories.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    /// Steps 3a-3c, 5b, 7 and 8 of the operational-event import
    /// (gpu_op_event_import.c:593-685) and its teardown.
    #[test]
    fn the_operational_event_import_completes() {
        let (mem, path) = guest("goe");
        let mut g = GspMemory::new();
        let c = 0xc1e0_0007;
        // Header 32 bytes, records 64 x 64, vardata 0x1000 (the sizes are the import's
        // GOEIMPORT_*; any that fit do).
        for (h, len, pfn) in [(0x10u32, 32u64, 0x10_0000u64), (0x11, 0x1000, 0x10_0001), (0x12, 0x1000, 0x10_0002)] {
            assert_eq!(g.alloc_memory(&request(h, NV01_MEMORY_LIST_SYSTEM, len, &[pfn]), &mem, 1 << 30).0, NV_OK);
        }
        let mut p = vec![0u8; EB_PARAMS_SIZE];
        let put = |p: &mut Vec<u8>, o: usize, v: u32| p[o..o + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut p, EB_RECORD_SIZE, 64);
        put(&mut p, EB_RECORD_COUNT, 64);
        put(&mut p, EB_VARDATA_SIZE, 0x1000);
        put(&mut p, EB_H_SUBDEVICE, 0xcaf0_0001);
        put(&mut p, EB_H_HEADER, 0x10);
        put(&mut p, EB_H_RECORDS, 0x11);
        put(&mut p, EB_H_VARDATA, 0x12);
        let a = g.alloc(c, c, 0x20, NV_EVENT_BUFFER, &p).unwrap();
        assert_eq!(a.status, NV_OK, "{}", a.note);
        // Too many records for the registered memory.
        put(&mut p, EB_RECORD_COUNT, 65);
        assert_eq!(g.alloc(c, c, 0x21, NV_EVENT_BUFFER, &p).unwrap().status, NV_ERR_INVALID_ARGUMENT);
        put(&mut p, EB_RECORD_COUNT, 64);
        put(&mut p, EB_H_VARDATA, 0x99);
        assert_eq!(g.alloc(c, c, 0x21, NV_EVENT_BUFFER, &p).unwrap().status, NV_ERR_OBJECT_NOT_FOUND);
        // Step 7: ENABLE_EVENTS with KEEP_OLDEST.
        let mut en = vec![0u8; 8];
        en[0] = 1;
        en[4] = 1;
        assert_eq!(g.control(c, 0x20, CMD_EVENT_BUFFER_ENABLE_EVENTS, &en).unwrap().status, NV_OK);
        assert_eq!(g.buffers[&(c, 0x20)].enabled, Some(1));
        // Step 8: the bind, scope device, GOE records; bindId comes back.
        let mut b = vec![0u8; BIND_PARAMS_SIZE];
        b[..4].copy_from_slice(&0x20u32.to_le_bytes());
        b[BIND_RECORD_FORMAT] = RECORD_FORMAT_GOE;
        let r = g.alloc(c, c, 0x30, NV_OPERATIONAL_EVENT_BUFFER_BIND, &b).unwrap();
        assert_eq!(r.status, NV_OK, "{}", r.note);
        assert_ne!(r.params[BIND_BIND_ID], 0);
        b[BIND_SCOPE] = 1;
        assert_eq!(g.alloc(c, c, 0x31, NV_OPERATIONAL_EVENT_BUFFER_BIND, &b).unwrap().status, NV_ERR_NOT_SUPPORTED);
        // The drain's UPDATE_GET, bounded by the ring.
        let get = [3u8, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(g.control(c, 0x20, CMD_EVENT_BUFFER_UPDATE_GET, &get).unwrap().status, NV_OK);
        let past = [64u8, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(g.control(c, 0x20, CMD_EVENT_BUFFER_UPDATE_GET, &past).unwrap().status, NV_ERR_INVALID_ARGUMENT);
        assert!(g.control(c, 0x99, CMD_EVENT_BUFFER_FLUSH, &[]).unwrap().status == NV_ERR_INVALID_OBJECT_HANDLE);
        assert!(g.control(c, 0x20, 0x2080_0101, &[]).is_none());
        assert!(g.alloc(c, c, 0x40, 0xc46f, &[]).is_none());
        // Teardown: the event buffer takes its bind along.
        assert_eq!(g.free(c, 0x20).unwrap(), "event buffer, 1 bind(s)");
        assert!(g.binds.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    /// The import of the 615 kernel as the first boot sent it (header 32 bytes, `count`
    /// records of 104 bytes, vardata 0x1000), enabled and bound; returns the registry.
    fn imported(mem: &GuestMemory, count: u32) -> GspMemory {
        let mut g = GspMemory::new();
        let c = 0xc1e0_0007;
        for (h, len, pfn) in [(0x10u32, 32u64, 0x10_0000u64), (0x11, 104 * count as u64, 0x10_0001), (0x12, 0x1000, 0x10_0003)] {
            assert_eq!(g.alloc_memory(&request(h, NV01_MEMORY_LIST_SYSTEM, len, &[pfn]), mem, 1 << 30).0, NV_OK);
        }
        let mut p = vec![0u8; EB_PARAMS_SIZE];
        let put = |p: &mut Vec<u8>, o: usize, v: u32| p[o..o + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut p, EB_RECORD_SIZE, 104);
        put(&mut p, EB_RECORD_COUNT, count);
        put(&mut p, EB_VARDATA_SIZE, 0x1000);
        put(&mut p, EB_H_HEADER, 0x10);
        put(&mut p, EB_H_RECORDS, 0x11);
        put(&mut p, EB_H_VARDATA, 0x12);
        assert_eq!(g.alloc(c, c, 0x20, NV_EVENT_BUFFER, &p).unwrap().status, NV_OK);
        assert_eq!(g.control(c, 0x20, CMD_EVENT_BUFFER_ENABLE_EVENTS, &[1, 0, 0, 0, 1, 0, 0, 0]).unwrap().status, NV_OK);
        let mut b = vec![0u8; BIND_PARAMS_SIZE];
        b[..4].copy_from_slice(&0x20u32.to_le_bytes());
        b[BIND_RECORD_FORMAT] = RECORD_FORMAT_GOE;
        b[BIND_UUID..BIND_UUID + 16].copy_from_slice(&[0xAB; 16]);
        assert_eq!(g.alloc(c, c, 0x30, NV_OPERATIONAL_EVENT_BUFFER_BIND, &b).unwrap().status, NV_OK);
        g
    }

    fn rd32(mem: &GuestMemory, pa: u64) -> u32 {
        mem.read_u32(pa).unwrap()
    }

    /// Item 5 of the GSP plan round 2: an RC the device raises becomes a GOE record in the
    /// import's registered guest memory, laid out as the kernel's drain reads it
    /// (gpu_op_event_import.c:1345-1440: slot `recordGet % count`, vardata window framed
    /// to the granularity, one context), with one wake-up until the drain's UPDATE_GET.
    #[test]
    fn rc_events_become_goe_records() {
        let (mem, path) = guest("produce");
        let mut g = imported(&mem, 4);
        let (hdr, rec, var) = (0x1_0000_0000u64, 0x1_0000_1000u64, 0x1_0000_3000u64);
        let ev = OpEvent::rc(31, 1234, "Caraxes: channel 2 error (host mirror)".into());
        let (wake, notes) = g.produce(&mem, &ev);
        assert_eq!(wake, [(0xc1e0_0007, 0x30)], "{notes:?}");
        // Header: recordPut 1, recordCount 1; recordGet is the kernel's and untouched.
        assert_eq!((rd32(&mem, hdr), rd32(&mem, hdr + 4)), (0, 1));
        assert_eq!(mem.read_u64(hdr + 8), Some(1));
        // The record as the drain reads it.
        assert_eq!(rd32(&mem, rec) & 0xFFFF, RECORD_TYPE_GOE as u32);
        assert_eq!(rd32(&mem, rec) >> 16, RECORD_SUBTYPE_GOE_V1 as u32);
        let var_data = rd32(&mem, rec + 4);
        let total = rd32(&mem, rec + 68);
        let start = 0; // the kernel's vardataGet, 0 at first
        assert_eq!(var_data & !(VARDATA_GRANULARITY - 1), start + total.div_ceil(32) * 32, "framing");
        assert_eq!(var_data & VARDATA_START_ZERO, 0);
        assert_eq!(rd32(&mem, rec + 48) & 0xFFFF, CATEGORY_TRANSLATION_FAULT as u32);
        assert_eq!(rd32(&mem, rec + 48) >> 16, 31, "moduleEventCode: the Xid");
        let mut sig = [0u8; 16];
        mem.read(rec + 32, &mut sig);
        assert_eq!(&sig[..6], b"GPU-RC");
        let mut group = [0u8; 4];
        mem.read(rec + 76, &mut group);
        assert_eq!(&group[..2], &[1, 0], "groupSize 1, groupIndex 0");
        let mut uuid = [0u8; 16];
        mem.read(rec + 80, &mut uuid);
        assert_eq!(uuid, [0xAB; 16]);
        // Vardata: one legacy-Xid context with the Xid and the text.
        assert_eq!(rd32(&mem, var) & 0xFFFF, CTX_LEGACY_XID as u32);
        assert_eq!(rd32(&mem, var + 8), 31);
        let mut msg = [0u8; 9];
        mem.read(var + 12, &mut msg);
        assert_eq!(&msg, b"Caraxes: ");
        assert_eq!(rd32(&mem, var + 4) + CTX_HEADER as u32, total, "dataSize + header");
        // A second event before the drain: written, no second wake-up.
        let (wake, _) = g.produce(&mem, &ev);
        assert!(wake.is_empty());
        assert_eq!(rd32(&mem, hdr + 4), 2);
        let var_data2 = rd32(&mem, rec + 104 + 4);
        assert_eq!(var_data2, 2 * (var_data & !31), "the second window follows the first");
        // The drain's UPDATE_GET clears the latch.
        let mut get = [0u8; 8];
        get[..4].copy_from_slice(&2u32.to_le_bytes());
        get[4..].copy_from_slice(&var_data2.to_le_bytes());
        assert_eq!(g.control(0xc1e0_0007, 0x20, CMD_EVENT_BUFFER_UPDATE_GET, &get).unwrap().status, NV_OK);
        let (wake, _) = g.produce(&mem, &ev);
        assert_eq!(wake.len(), 1);
        // Ring of 4 with get 2: puts 3, 0 and 1 fit (one slot stays free), the next would
        // reach get.
        let _ = g.produce(&mem, &ev);
        let _ = g.produce(&mem, &ev);
        let (wake, notes) = g.produce(&mem, &ev);
        assert!(wake.is_empty());
        assert!(notes[0].contains("record ring full"), "{notes:?}");
        assert_eq!(mem.read_u64(hdr + 16), Some(1), "recordDropcount");
        // A disabled buffer gets nothing; a CPER bind gets nothing.
        g.control(0xc1e0_0007, 0x20, CMD_EVENT_BUFFER_ENABLE_EVENTS, &[0; 8]).unwrap();
        let (wake, notes) = g.produce(&mem, &ev);
        assert!(wake.is_empty() && notes[0].contains("disabled"), "{notes:?}");
        let _ = std::fs::remove_file(&path);
    }

    /// The self-test event is SIMULATED, debug category, and only due with a GOE bind.
    #[test]
    fn selftest_events_are_marked_simulated() {
        let (mem, path) = guest("selftest");
        let ev = OpEvent::selftest(3, 99);
        assert_eq!(ev.group_attributes & GROUP_ATTR_SIMULATED, GROUP_ATTR_SIMULATED);
        assert_eq!(ev.category, CATEGORY_DEBUG);
        let mut g = GspMemory::new();
        g.selftest = Some(SelfTest { period: std::time::Duration::from_secs(3600), last: None, sent: 0 });
        assert_eq!(g.selftest_due(), None, "no bind yet");
        let mut g2 = imported(&mem, 8);
        g2.selftest = g.selftest.take();
        assert_eq!(g2.selftest_due(), Some(1));
        assert_eq!(g2.selftest_due(), None, "not before the period");
        let (wake, _) = g2.produce(&mem, &ev);
        assert_eq!(wake.len(), 1);
        let rec = 0x1_0000_1000u64;
        let mut attrs = [0u8; 4];
        mem.read(rec + 52, &mut attrs);
        assert_eq!(attrs[2], GROUP_ATTR_SIMULATED);
        let _ = std::fs::remove_file(&path);
    }
}
