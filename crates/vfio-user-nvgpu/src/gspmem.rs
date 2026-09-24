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
//! GSP-RM would and writes no records (it has no operational events of its own).

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

/// An `NV_EVENT_BUFFER` the device produces into (it writes no records).
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
}

/// An `NV_OPERATIONAL_EVENT_BUFFER_BIND`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bind {
    pub h_event_buffer: u32,
    pub record_format: u8,
    pub bind_id: u8,
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
}

impl GspMemory {
    pub fn new() -> Self {
        Self::default()
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
                    "{what}: {} records of {} bytes, vardata {:#x}, memories {:#x}/{:#x}/{:#x}; the device is the producer and writes no records [guest, recorded]",
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
                self.binds.insert(
                    (h_client, h_object),
                    Bind { h_event_buffer: h_eb, record_format: format, bind_id: id },
                );
                reply(
                    NV_OK,
                    out,
                    format!("{what}: event buffer {h_eb:#x} format {format} scope device, bindId {id}; no operational events are produced [guest, recorded]"),
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
                    (NV_OK, format!("update get {r}/{vd}"))
                }
            }
            // Nothing is ever pending in a buffer the device does not write.
            CMD_EVENT_BUFFER_FLUSH => (NV_OK, "flush: nothing pending".into()),
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
}
