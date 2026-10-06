// Copyright © 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use bitflags::bitflags;
use libc::{c_void, iovec, EINVAL};
use libc::{sysconf, _SC_PAGESIZE};
use std::ffi::CString;
use std::fs::File;
use std::io::{IoSlice, Read, Write};
use std::mem::size_of;
use std::num::Wrapping;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::{
    io::{FromRawFd, RawFd},
    net::{UnixListener, UnixStream},
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use vfio_bindings::bindings::vfio::*;
use vm_memory::{ByteValued, FileOffset};
use vmm_sys_util::sock_ctrl_msg::ScmSocket;

pub mod local_irq;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate log;

#[allow(dead_code)]
#[repr(u16)]
#[derive(Clone, Copy, Debug, Default, enumn::N)]
pub enum Command {
    #[default]
    Unknown = 0,
    Version = 1,
    DmaMap = 2,
    DmaUnmap = 3,
    DeviceGetInfo = 4,
    DeviceGetRegionInfo = 5,
    GetRegionIoFds = 6,
    GetIrqInfo = 7,
    SetIrqs = 8,
    RegionRead = 9,
    RegionWrite = 10,
    DmaRead = 11,
    DmaWrite = 12,
    DeviceReset = 13,
    UserDirtyPages = 14,
    /// Private Caraxes extension, valid only after explicit VERSION negotiation.
    LocalIrq = 0x8000,
}

#[allow(dead_code)]
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum HeaderFlags {
    #[default]
    Command = 0,
    Reply = 1,
    NoReply = 1 << 4,
    Error = 1 << 5,
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct Header {
    message_id: u16,
    command: u16,
    message_size: u32,
    flags: u32,
    error: u32,
}

impl Header {
    fn no_reply(&self) -> bool {
        self.flags & HeaderFlags::NoReply as u32 != 0
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct Version {
    header: Header,
    major: u16,
    minor: u16,
}

#[derive(Serialize, Deserialize, Debug)]
struct MigrationCapabilities {
    pgsize: u32,
}

const fn default_max_msg_fds() -> u32 {
    1
}

const fn default_max_data_xfer_size() -> u32 {
    1048576
}

#[inline(always)]
fn pagesize() -> u32 {
    // SAFETY: sysconf
    unsafe { sysconf(_SC_PAGESIZE) as u32 }
}

fn default_migration_capabilities() -> MigrationCapabilities {
    MigrationCapabilities { pgsize: pagesize() }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DmaMapFlags: u32 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const READ_WRITE = Self::READ.bits() | Self::WRITE.bits();

        // There might be unknown bits and we don't want bitflags to clear them.
        const _ = !0;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DmaUnmapFlags: u32 {
        const GET_DIRTY_PAGE_INFO = 1 << 1;
        const UNMAP_ALL = 1 << 2;

        // See above.
        const _ = !0;
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct DmaMap {
    header: Header,
    argsz: u32,
    flags: u32,
    offset: u64,
    address: u64,
    size: u64,
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct DmaUnmap {
    header: Header,
    argsz: u32,
    flags: u32,
    address: u64,
    size: u64,
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct DeviceGetInfo {
    header: Header,
    argsz: u32,
    flags: u32,
    num_regions: u32,
    num_irqs: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct DeviceGetRegionInfo {
    header: Header,
    region_info: vfio_region_info,
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct RegionAccess {
    header: Header,
    offset: u64,
    region: u32,
    count: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct GetIrqInfo {
    header: Header,
    argsz: u32,
    flags: u32,
    index: u32,
    count: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct SetIrqs {
    header: Header,
    argsz: u32,
    flags: u32,
    index: u32,
    start: u32,
    count: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct DeviceReset {
    header: Header,
}

// SAFETY: data structure only contain a series of integers
unsafe impl ByteValued for Header {}
// SAFETY: data structure only contain a series of integers
unsafe impl ByteValued for Version {}
// SAFETY: data structure only contain a series of integers
unsafe impl ByteValued for DmaMap {}
// SAFETY: data structure only contain a series of integers
unsafe impl ByteValued for DmaUnmap {}
// SAFETY: data structure only contain a series of integers
unsafe impl ByteValued for DeviceGetInfo {}
// SAFETY: data structure only contain a series of integers
unsafe impl ByteValued for DeviceGetRegionInfo {}
// SAFETY: data structure only contain a series of integers
unsafe impl ByteValued for RegionAccess {}
// SAFETY: data structure only contain a series of integers
unsafe impl ByteValued for GetIrqInfo {}
// SAFETY: data structure only contain a series of integers
unsafe impl ByteValued for SetIrqs {}
// SAFETY: data structure only contain a series of integers
unsafe impl ByteValued for DeviceReset {}

// VFIO_USER_DEVICE_GET_REGION_IO_FDS. Citations name sections of the vfio-user
// specification, vfio-user.rst (QEMU docs/interop/vfio-user.rst).

/// `VFIO_USER_DEVICE_GET_REGION_IO_FDS` request (vfio-user.rst,
/// "VFIO_USER_DEVICE_GET_REGION_IO_FDS", "Request"). The reply's fixed part has
/// the same four fields at the same offsets ("Reply").
#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
struct RegionIoFdsInfo {
    /// vfio-user.rst, "Header": message ID 0, command 2, message size 4, flags 8, error 12.
    header: Header,
    /// "Request"/"Reply": argsz, offset 0, size 4. In the request the maximum
    /// reply payload size, in the reply the size of the full payload.
    argsz: u32,
    /// "Request"/"Reply": flags, offset 4, size 4; must be zero in both.
    flags: u32,
    /// "Request"/"Reply": index, offset 8, size 4; the region queried.
    index: u32,
    /// "Request"/"Reply": count, offset 12, size 4; zero in the request, the
    /// number of sub-regions in the reply.
    count: u32,
}

// SAFETY: data structure only contain a series of integers
unsafe impl ByteValued for RegionIoFdsInfo {}

/// Size of argsz, flags, index and count; the sub-region array follows at offset
/// 16 (vfio-user.rst, "VFIO_USER_DEVICE_GET_REGION_IO_FDS", "Reply": sub-regions,
/// offset 16).
pub const REGION_IO_FDS_INFO_SIZE: u32 = 16;

/// Size of a Sub-Region IO FD info entry: the last field of both formats ends at
/// offset 40 (vfio-user.rst, "Sub-Region IO FD info format (ioeventfd)" and
/// "(ioregionfd)"; "Reply": "currently this is 40 bytes for both IO FD types").
pub const SUB_REGION_IO_FD_INFO_SIZE: u32 = 40;

/// Sub-region `type` ioeventfd (vfio-user.rst, "Sub-Region IO FD info format
/// (ioeventfd)": type is `VFIO_USER_IO_FD_TYPE_IOEVENTFD`). The specification
/// gives no number; 0 is libvfio-user's (include/vfio-user.h:174).
pub const VFIO_USER_IO_FD_TYPE_IOEVENTFD: u32 = 0;
/// Sub-region `type` ioregionfd (vfio-user.rst, "Sub-Region IO FD info format
/// (ioregionfd)"). The specification gives no number; 1 is libvfio-user's
/// (include/vfio-user.h:175).
pub const VFIO_USER_IO_FD_TYPE_IOREGIONFD: u32 = 1;

/// ioeventfd `flags` bit `KVM_IOEVENTFD_FLAG_DATAMATCH` (vfio-user.rst,
/// "Sub-Region IO FD info format (ioeventfd)", flags; value from `<linux/kvm.h>`,
/// `1 << kvm_ioeventfd_flag_nr_datamatch`).
pub const IOEVENTFD_FLAG_DATAMATCH: u32 = 1 << 0;
/// ioeventfd `flags` bit `KVM_IOEVENTFD_FLAG_PIO` (same section; `<linux/kvm.h>`,
/// `1 << kvm_ioeventfd_flag_nr_pio`).
pub const IOEVENTFD_FLAG_PIO: u32 = 1 << 1;
/// ioeventfd `flags` bit `KVM_IOEVENTFD_FLAG_VIRTIO_CCW_NOTIFY` (same section;
/// `<linux/kvm.h>`, `1 << kvm_ioeventfd_flag_nr_virtio_ccw_notify`).
pub const IOEVENTFD_FLAG_VIRTIO_CCW_NOTIFY: u32 = 1 << 3;

/// The flags vfio-user.rst lists for an ioeventfd sub-region ("flags is any of").
const IOEVENTFD_FLAGS: u32 =
    IOEVENTFD_FLAG_DATAMATCH | IOEVENTFD_FLAG_PIO | IOEVENTFD_FLAG_VIRTIO_CCW_NOTIFY;

/// Largest argsz the client asks for. It stays below 65536: a server that does
/// not read the payload of this request (vfio_user up to 0.1.6) takes the
/// payload for the next message header, and with argsz < 65536 that header has
/// command 0, which such a server refuses instead of acting on it.
const REGION_IO_FDS_MAX_ARGSZ: u32 = 0xffff;

/// argsz of the first request: room for 16 entries.
const REGION_IO_FDS_FIRST_ARGSZ: u32 = REGION_IO_FDS_INFO_SIZE + 16 * SUB_REGION_IO_FD_INFO_SIZE;

/// Linux's limit on the descriptors of one `SCM_RIGHTS` message (`SCM_MAX_FD`,
/// include/net/scm.h: 253); `sendmsg()` refuses more. No reply carries more,
/// whatever `max_msg_fds` says.
pub const SCM_MAX_FD: u32 = 253;

/// The client's `max_msg_fds` (vfio-user.rst, "VFIO_USER_VERSION", capabilities:
/// "Maximum number of file descriptors that can be received by the sender in one
/// message. Optional. If not specified then the receiver must assume a value of
/// 1"): as many as Linux delivers with one message, so that a server can hand
/// over one eventfd per ioeventfd sub-region, for example one per datamatch
/// value at the same offset.
pub const CLIENT_MAX_MSG_FDS: u32 = SCM_MAX_FD;

/// File descriptors the client accepts with one reply: its `max_msg_fds`. A
/// reply with more is cut by the kernel (`MSG_CTRUNC`); vmm-sys-util's
/// `recv_with_fds` then closes them and fails. The server side receives 16.
const MAX_RECEIVED_FDS: usize = CLIENT_MAX_MSG_FDS as usize;

/// Reply payload the client reads and drops to stay in step when a server sends
/// more than it asked for (the default max_data_xfer_size, vfio-user.rst,
/// "VFIO_USER_VERSION", capabilities).
const MAX_DRAINED_PAYLOAD: usize = 1 << 20;

/// One entry of a `VFIO_USER_DEVICE_GET_REGION_IO_FDS` reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubRegionIoFd {
    /// vfio-user.rst, "Sub-Region IO FD info format (ioeventfd)".
    IoEventFd {
        /// offset, offset 0, size 8: start of the sub-region within the region.
        offset: u64,
        /// size, offset 8, size 8: length of the sub-region; may be zero.
        size: u64,
        /// fd_index, offset 16, size 4: index into the reply's descriptors.
        fd_index: u32,
        /// flags, offset 24, size 4: `IOEVENTFD_FLAG_*`.
        flags: u32,
        /// datamatch, offset 32, size 8.
        datamatch: u64,
    },
    /// vfio-user.rst, "Sub-Region IO FD info format (ioregionfd)".
    IoRegionFd {
        /// offset, offset 0, size 8.
        offset: u64,
        /// size, offset 8, size 8.
        size: u64,
        /// fd_index, offset 16, size 4.
        fd_index: u32,
        /// flags, offset 24, size 4: `KVM_IOREGION_PIO`, `KVM_IOREGION_POSTED_WRITES`.
        flags: u32,
        /// user_data, offset 32, size 8.
        user_data: u64,
    },
}

/// The reply to `VFIO_USER_DEVICE_GET_REGION_IO_FDS`.
#[derive(Debug)]
pub struct RegionIoFds {
    /// vfio-user.rst, "Reply": index, the region queried.
    pub index: u32,
    /// "Reply": sub-regions, `count` entries.
    pub sub_regions: Vec<SubRegionIoFd>,
    /// The descriptors of the ancillary data ("Reply": "The reply message will
    /// additionally include at least one file descriptor in the ancillary
    /// data"); `fd_index` indexes this list.
    pub fds: Vec<File>,
}

/// An ioeventfd sub-region a server offers (see [`Server::add_ioeventfd`]); it
/// is sent as vfio-user.rst, "Sub-Region IO FD info format (ioeventfd)".
#[derive(Debug)]
pub struct IoEventFdSubRegion {
    /// offset (offset 0): start within the region.
    pub offset: u64,
    /// size (offset 8): length; the access size for KVM, 0 for any.
    pub size: u64,
    /// flags (offset 24): `IOEVENTFD_FLAG_*`.
    pub flags: u32,
    /// datamatch (offset 32): the value a write must carry with
    /// `IOEVENTFD_FLAG_DATAMATCH`.
    pub datamatch: u64,
    /// The eventfd the client is to signal; its position in the ancillary data
    /// is fd_index (offset 16). Sub-regions that share one eventfd share this
    /// `Arc`, and the reply then carries the descriptor once ("more than one
    /// sub-region may share the same file descriptor").
    pub fd: Arc<OwnedFd>,
}

/// A parsed `VFIO_USER_DEVICE_GET_REGION_IO_FDS` reply payload.
#[derive(Debug, PartialEq, Eq)]
pub enum RegionIoFdsReply {
    /// The full reply: its sub-regions.
    Complete { sub_regions: Vec<SubRegionIoFd> },
    /// argsz exceeded the request's: the payload is left out and `argsz` is the
    /// size needed (vfio-user.rst, "Message sizes").
    Truncated { argsz: u32 },
}

fn read_u32_ne(buf: &[u8], at: usize) -> u32 {
    u32::from_ne_bytes(buf[at..at + 4].try_into().unwrap())
}

fn read_u64_ne(buf: &[u8], at: usize) -> u64 {
    u64::from_ne_bytes(buf[at..at + 8].try_into().unwrap())
}

/// Parse the payload (what follows the 16-byte message header) of a reply to a
/// `VFIO_USER_DEVICE_GET_REGION_IO_FDS` request that had `argsz` and `index`.
/// All data is in host byte order (vfio-user.rst, "Protocol Specification").
///
/// The reply is accepted only in the exact form the specification lays out: a
/// complete payload of argsz = 16 + count * 40 bytes, flags 0, the requested
/// index and entries of type ioeventfd (0) or ioregionfd (1); a truncated one of
/// the 16-byte fixed part. Anything else is `Error::InvalidReply`, and a client
/// keeps the region trapped.
///
/// libvfio-user (nutanix/libvfio-user 803924493a7c, include/vfio-user.h:178-187)
/// sends a different ioeventfd entry: gpa_offset 0, size 8, fd_index 16, type 20,
/// flags 24, shadow_mem_fd_index 28, shadow_offset 32 and datamatch 40, 48 bytes,
/// with an extra type VFIO_USER_IO_FD_TYPE_IOEVENTFD_SHADOW = 2
/// (include/vfio-user.h:176) for its out-of-tree shadow ioeventfd
/// (docs/shadow_ioeventfd.md). Its reply array is a union of that and the
/// 40-byte ioregionfd entry, so every entry is 48 bytes apart. The specification
/// places datamatch at 32 of a 40-byte entry. Reading a libvfio-user reply by
/// the specification would take shadow_offset for datamatch, so such a reply is
/// refused here (argsz is not 16 + count * 40): on this message this client is
/// not wire-compatible with libvfio-user until the specification adopts that
/// layout. The specification's formula for the entry offset,
/// i * ((argsz - 32) / count), cannot hold either (the array starts at 16 per
/// the reply table, which gives 40 only with argsz - 16); with the exact size
/// check the entries are at 16 + i * 40.
pub fn parse_region_io_fds_reply(
    argsz: u32,
    index: u32,
    payload: &[u8],
) -> Result<RegionIoFdsReply, Error> {
    let invalid = |why| Error::InvalidReply(Command::GetRegionIoFds, why);
    if payload.len() < REGION_IO_FDS_INFO_SIZE as usize {
        return Err(invalid("shorter than argsz, flags, index and count"));
    }
    // "Reply": argsz 0, flags 4, index 8, count 12, each 4 bytes.
    let reply_argsz = read_u32_ne(payload, 0);
    let flags = read_u32_ne(payload, 4);
    let reply_index = read_u32_ne(payload, 8);
    let count = read_u32_ne(payload, 12);
    if flags != 0 {
        // "Reply": "flags must be zero".
        return Err(invalid("flags must be zero"));
    }
    if reply_index != index {
        // "Reply": "index is the index of memory region being queried".
        return Err(invalid("index differs from the request"));
    }
    if reply_argsz > argsz {
        // "Message sizes": argsz above the request's maximum means "the full
        // payload is not included", so only the fixed part may be there.
        if payload.len() != REGION_IO_FDS_INFO_SIZE as usize {
            return Err(invalid("truncated reply longer than its fixed part"));
        }
        return Ok(RegionIoFdsReply::Truncated { argsz: reply_argsz });
    }
    // "Reply": argsz "is the size of the region IO FD info structure plus the
    // total size of the sub-region array", entries of 40 bytes.
    let expected = u64::from(REGION_IO_FDS_INFO_SIZE)
        + u64::from(count) * u64::from(SUB_REGION_IO_FD_INFO_SIZE);
    if u64::from(reply_argsz) != expected {
        return Err(invalid("argsz is not 16 + count * 40"));
    }
    if payload.len() != reply_argsz as usize {
        return Err(invalid("payload size differs from argsz"));
    }
    let mut sub_regions = Vec::with_capacity(count as usize);
    for e in payload[REGION_IO_FDS_INFO_SIZE as usize..]
        .chunks_exact(SUB_REGION_IO_FD_INFO_SIZE as usize)
    {
        // "Sub-Region IO FD info format (ioeventfd)" and "(ioregionfd)": offset 0
        // (8), size 8 (8), fd_index 16 (4), type 20 (4), flags 24 (4), padding 28
        // (4), datamatch or user_data 32 (8).
        let offset = read_u64_ne(e, 0);
        let size = read_u64_ne(e, 8);
        let fd_index = read_u32_ne(e, 16);
        let type_ = read_u32_ne(e, 20);
        let flags = read_u32_ne(e, 24);
        sub_regions.push(match type_ {
            VFIO_USER_IO_FD_TYPE_IOEVENTFD => SubRegionIoFd::IoEventFd {
                offset,
                size,
                fd_index,
                flags,
                datamatch: read_u64_ne(e, 32),
            },
            VFIO_USER_IO_FD_TYPE_IOREGIONFD => SubRegionIoFd::IoRegionFd {
                offset,
                size,
                fd_index,
                flags,
                user_data: read_u64_ne(e, 32),
            },
            // "Each sub-region given in the response has one of two possible
            // structures".
            _ => {
                return Err(invalid(
                    "sub-region type is neither ioeventfd nor ioregionfd",
                ))
            }
        });
    }
    Ok(RegionIoFdsReply::Complete { sub_regions })
}

/// Build the payload of a `VFIO_USER_DEVICE_GET_REGION_IO_FDS` reply for region
/// `index`: the fixed part (argsz the full size, flags 0, index, count) and, if
/// `with_entries`, one ioeventfd entry per `(sub-region, fd_index)`.
fn encode_region_io_fds_reply(
    index: u32,
    entries: &[(&IoEventFdSubRegion, u32)],
    with_entries: bool,
) -> Vec<u8> {
    let count = entries.len() as u32;
    let argsz = REGION_IO_FDS_INFO_SIZE + count * SUB_REGION_IO_FD_INFO_SIZE;
    let mut buf = Vec::with_capacity(argsz as usize);
    // vfio-user.rst, "VFIO_USER_DEVICE_GET_REGION_IO_FDS", "Reply": argsz 0,
    // flags 4 ("must be zero"), index 8, count 12.
    buf.extend_from_slice(&argsz.to_ne_bytes());
    buf.extend_from_slice(&0u32.to_ne_bytes());
    buf.extend_from_slice(&index.to_ne_bytes());
    buf.extend_from_slice(&count.to_ne_bytes());
    if with_entries {
        for (sub_region, fd_index) in entries {
            // "Sub-Region IO FD info format (ioeventfd)": offset 0, size 8,
            // fd_index 16, type 20, flags 24, padding 28, datamatch 32.
            buf.extend_from_slice(&sub_region.offset.to_ne_bytes());
            buf.extend_from_slice(&sub_region.size.to_ne_bytes());
            buf.extend_from_slice(&fd_index.to_ne_bytes());
            buf.extend_from_slice(&VFIO_USER_IO_FD_TYPE_IOEVENTFD.to_ne_bytes());
            buf.extend_from_slice(&sub_region.flags.to_ne_bytes());
            buf.extend_from_slice(&0u32.to_ne_bytes());
            buf.extend_from_slice(&sub_region.datamatch.to_ne_bytes());
        }
    }
    buf
}

#[derive(Serialize, Deserialize, Debug)]
struct Capabilities {
    #[serde(default = "default_max_msg_fds")]
    max_msg_fds: u32,
    #[serde(default = "default_max_data_xfer_size")]
    max_data_xfer_size: u32,
    #[serde(default = "default_migration_capabilities")]
    migration: MigrationCapabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x_caraxes_local_irq: Option<LocalIrqCapability>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct LocalIrqCapability {
    versions: Vec<u16>,
    layouts: Vec<String>,
    transports: Vec<String>,
    owner: String,
    page: String,
    irq: String,
    supports_reset_epochs: bool,
}

impl LocalIrqCapability {
    fn v1() -> Self {
        Self { versions: vec![1], layouts: vec!["tu106-exact-page-v1".into()],
            transports: vec!["shm-fold-v1".into()], owner: "vmm".into(),
            page: "vmm-private-tree-page-v1".into(), irq: "single-msi".into(),
            supports_reset_epochs: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct CapabilitiesData {
    capabilities: Capabilities,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            max_msg_fds: default_max_msg_fds(),
            max_data_xfer_size: default_max_data_xfer_size(),
            migration: default_migration_capabilities(),
            x_caraxes_local_irq: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ClientOptions { pub local_irq_required: bool }

pub struct Client {
    stream: UnixStream,
    next_message_id: Wrapping<u16>,
    num_irqs: u32,
    resettable: bool,
    regions: Vec<Region>,
    local_irq_negotiated: bool,
}

#[derive(Debug)]
pub struct Region {
    pub flags: u32,
    pub index: u32,
    pub size: u64,
    pub file_offset: Option<FileOffset>,
    pub sparse_areas: Vec<vfio_region_sparse_mmap_area>,
}

#[derive(Clone, Copy, Debug)]
pub struct IrqInfo {
    pub index: u32,
    pub flags: u32,
    pub count: u32,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error connecting: {0}")]
    Connect(#[source] std::io::Error),
    #[error("Error serializing capabilities: {0}")]
    SerializeCapabilites(#[source] serde_json::Error),
    #[error("Error deserializing capabilities: {0}")]
    DeserializeCapabilites(#[source] serde_json::Error),
    #[error("Error writing to stream: {0}")]
    StreamWrite(#[source] std::io::Error),
    #[error("Error reading from stream: {0}")]
    StreamRead(#[source] std::io::Error),
    #[error("Error shutting down stream: {0}")]
    StreamShutdown(#[source] std::io::Error),
    #[error("Error writing with file descriptors: {0}")]
    SendWithFd(#[source] vmm_sys_util::errno::Error),
    #[error("Error reading with file descriptors: {0}")]
    ReceiveWithFd(#[source] vmm_sys_util::errno::Error),
    #[error("Not a PCI device")]
    NotPciDevice,
    #[error("Socket path already exists")]
    SocketPathExists,
    #[error("Error binding to socket: {0}")]
    SocketBind(#[source] std::io::Error),
    #[error("Error accepting connection: {0}")]
    SocketAccept(#[source] std::io::Error),
    #[error("Unknown command: {0}")]
    UnknownCommand(u16),
    #[error("Unsupported command: {0:?}")]
    UnsupportedCommand(Command),
    #[error("Unsupported feature")]
    UnsupportedFeature,
    #[error("Error from backend: {0:?}")]
    Backend(#[source] std::io::Error),
    /// The server answered with the Error bit set (vfio-user.rst, "Header":
    /// "Error in a reply message indicates the command being acknowledged had
    /// an error"); `errno` is the reply's Error field, which may be 0.
    #[error("Error reply to {command:?}: errno {errno}")]
    ErrorReply { command: Command, errno: u32 },
    /// A well-framed reply whose content the specification does not allow; the
    /// stream is still in step.
    #[error("Invalid reply to {0:?}: {1}")]
    InvalidReply(Command, &'static str),
    /// A message that is not the reply to the request; the stream is out of step.
    #[error("Unexpected reply to {0:?}: {1}")]
    UnexpectedReply(Command, &'static str),
    #[error("Invalid input")]
    InvalidInput,
    #[error("No_reply bit unexpectedly set for command: {0:?}")]
    UnexpectedNoReply(Command),
    #[error("Terminal local IRQ session failure: {0}")]
    LocalIrqTerminal(&'static str),
}

impl Client {
    pub fn new(path: &Path) -> Result<Client, Error> {
        Self::new_with_options(path, ClientOptions::default())
    }

    pub fn new_with_options(path: &Path, options: ClientOptions) -> Result<Client, Error> {
        let stream = UnixStream::connect(path).map_err(Error::Connect)?;

        let mut client = Client {
            next_message_id: Wrapping(0),
            stream,
            num_irqs: 0,
            resettable: false,
            regions: Vec::new(),
            local_irq_negotiated: false,
        };

        client.negotiate_version(options)?;

        client.regions = client.get_regions()?;

        Ok(client)
    }

    fn negotiate_version(&mut self, options: ClientOptions) -> Result<(), Error> {
        // "VFIO_USER_VERSION", capabilities: max_msg_fds is what this side can
        // receive in one message; the other capabilities keep their defaults.
        let caps = CapabilitiesData {
            capabilities: Capabilities {
                max_msg_fds: CLIENT_MAX_MSG_FDS,
                x_caraxes_local_irq: options.local_irq_required.then(LocalIrqCapability::v1),
                ..Default::default()
            },
        };

        let version_data = serde_json::to_string(&caps).map_err(Error::SerializeCapabilites)?;

        let version = Version {
            header: Header {
                message_id: self.next_message_id.0,
                command: Command::Version as u16,
                flags: HeaderFlags::Command as u32,
                message_size: (size_of::<Version>() + version_data.len() + 1) as u32,
                ..Default::default()
            },
            major: 0,
            minor: 1,
        };
        debug!("Command: {version:?}");

        let version_data = CString::new(version_data.as_bytes()).unwrap();
        let bufs = vec![
            IoSlice::new(version.as_slice()),
            IoSlice::new(version_data.as_bytes_with_nul()),
        ];

        if options.local_irq_required {
            // A short VERSION frame would make private extension negotiation ambiguous.
            self.stream.write_all(version.as_slice()).map_err(Error::StreamWrite)?;
            self.stream.write_all(version_data.as_bytes_with_nul()).map_err(Error::StreamWrite)?;
        } else {
            // Preserve the generic path's existing write behavior.
            let _ = self.stream.write_vectored(&bufs).map_err(Error::StreamWrite)?;
        }

        debug!(
            "Sent client version information: major = {} minor = {} capabilities = {:?}",
            version.major, version.minor, caps.capabilities
        );

        self.next_message_id += Wrapping(1);

        let mut server_version: Version = Version::default();
        self.stream
            .read_exact(server_version.as_mut_slice())
            .map_err(Error::StreamRead)?;

        if server_version.header.message_id != version.header.message_id
            || server_version.header.command != Command::Version as u16
            || server_version.header.flags != HeaderFlags::Reply as u32
            || server_version.header.error != 0
            || server_version.major != 0 || server_version.minor > 1
            || server_version.header.message_size <= size_of::<Version>() as u32
            || server_version.header.message_size > 4096
        { return Err(Error::InvalidReply(Command::Version,"version envelope")); }

        debug!("Reply: {server_version:?}");

        let mut server_version_data =
            vec![0; server_version.header.message_size as usize - size_of::<Version>()];
        self.stream
            .read_exact(server_version_data.as_mut_slice())
            .map_err(Error::StreamRead)?;

        if server_version_data.last()!=Some(&0) {
            return Err(Error::InvalidReply(Command::Version,"missing NUL"));
        }

        let server_caps: CapabilitiesData =
            serde_json::from_slice(&server_version_data[0..server_version_data.len() - 1])
                .map_err(Error::DeserializeCapabilites)?;

        self.local_irq_negotiated = options.local_irq_required
            && server_caps.capabilities.x_caraxes_local_irq.as_ref()
                == Some(&LocalIrqCapability::v1());
        if options.local_irq_required && !self.local_irq_negotiated {
            return Err(Error::UnsupportedFeature);
        }

        debug!(
            "Received server version information: major = {} minor = {} capabilities = {:?}",
            server_version.major, server_version.minor, server_caps.capabilities
        );

        Ok(())
    }

    pub fn local_irq_negotiated(&self) -> bool { self.local_irq_negotiated }

    /// Poll-only clone; callers must never read from this descriptor.
    pub fn local_irq_liveness_stream(&self) -> std::io::Result<UnixStream> {
        self.stream.try_clone()
    }

    fn local_irq_request(&mut self, op:u16, token:u64, epoch:u64, ticket:u64,
        max_payload:u32) -> Result<(Vec<u8>,Vec<File>),Error> {
        if !self.local_irq_negotiated { return Err(Error::UnsupportedFeature); }
        let result=(|| {
        let header=Header{message_id:self.next_message_id.0,command:Command::LocalIrq as u16,
            message_size:48,flags:HeaderFlags::Command as u32,..Default::default()};
        let mut request=[0u8;48];request[..16].copy_from_slice(header.as_slice());
        request[16..20].copy_from_slice(b"LIRQ");
        request[20..22].copy_from_slice(&1u16.to_le_bytes());
        request[22..24].copy_from_slice(&op.to_le_bytes());
        request[24..32].copy_from_slice(&token.to_le_bytes());
        request[32..40].copy_from_slice(&epoch.to_le_bytes());
        request[40..48].copy_from_slice(&ticket.to_le_bytes());
        self.next_message_id+=Wrapping(1);
        self.stream.write_all(&request).map_err(Error::StreamWrite)?;
        let (reply,payload,fds)=self.recv_reply(&header,max_payload)?;
        if reply.flags != HeaderFlags::Reply as u32 || reply.error != 0 {
            return Err(Error::ErrorReply{command:Command::LocalIrq,errno:reply.error});
        }
        Ok((payload,fds))
        })();
        if result.is_err() {
            self.local_irq_negotiated=false;
            let _=self.stream.shutdown(std::net::Shutdown::Both);
        }
        result
    }

    fn local_irq_invalid(&mut self,detail:&'static str)->Error {
        self.local_irq_negotiated=false;
        let _=self.stream.shutdown(std::net::Shutdown::Both);
        Error::InvalidReply(Command::LocalIrq,detail)
    }

    pub fn local_irq_prepare(&mut self) -> Result<local_irq::Prepared,Error> {
        let (payload,mut fds)=self.local_irq_request(1,0,0,0,128)?;
        if payload.len()!=128 || fds.len()!=2 {
            return Err(self.local_irq_invalid("prepare size or FD count"));
        }
        let metadata=local_irq::Metadata::decode(&payload)
            .map_err(|_|self.local_irq_invalid("prepare metadata"))?;
        let event_fd=fds.pop().unwrap();let queue_fd=fds.pop().unwrap();
        Ok(local_irq::Prepared{metadata,queue_fd,event_fd})
    }

    pub fn local_irq_commit(&mut self,token:u64,epoch:u64,ticket:u64)->Result<(),Error>{
        let (payload,fds)=self.local_irq_request(2,token,epoch,ticket,0)?;
        if !payload.is_empty() || !fds.is_empty(){return Err(self.local_irq_invalid("commit reply"));}
        Ok(())
    }

    pub fn local_irq_abort(&mut self,token:u64)->Result<(),Error>{
        let (payload,fds)=self.local_irq_request(3,token,0,0,0)?;
        if !payload.is_empty() || !fds.is_empty(){return Err(self.local_irq_invalid("abort reply"));}
        Ok(())
    }

    pub fn dma_map(
        &mut self,
        offset: u64,
        address: u64,
        size: u64,
        fd: RawFd,
    ) -> Result<(), Error> {
        let dma_map = DmaMap {
            header: Header {
                message_id: self.next_message_id.0,
                command: Command::DmaMap as u16,
                flags: HeaderFlags::Command as u32,
                message_size: size_of::<DmaMap>() as u32,
                ..Default::default()
            },
            argsz: (size_of::<DmaMap>() - size_of::<Header>()) as u32,
            flags: DmaMapFlags::READ_WRITE.bits(),
            offset,
            address,
            size,
        };
        debug!("Command: {dma_map:?}");
        self.next_message_id += Wrapping(1);
        self.stream
            .send_with_fd(dma_map.as_slice(), fd)
            .map_err(Error::SendWithFd)?;

        let mut reply = Header::default();
        self.stream
            .read_exact(reply.as_mut_slice())
            .map_err(Error::StreamRead)?;
        debug!("Reply: {reply:?}");

        Ok(())
    }

    pub fn dma_unmap(&mut self, address: u64, size: u64) -> Result<(), Error> {
        let dma_unmap = DmaUnmap {
            header: Header {
                message_id: self.next_message_id.0,
                command: Command::DmaUnmap as u16,
                flags: HeaderFlags::Command as u32,
                message_size: size_of::<DmaUnmap>() as u32,
                ..Default::default()
            },
            argsz: (size_of::<DmaUnmap>() - size_of::<Header>()) as u32,
            flags: 0,
            address,
            size,
        };
        debug!("Command: {dma_unmap:?}");
        self.next_message_id += Wrapping(1);
        self.stream
            .write_all(dma_unmap.as_slice())
            .map_err(Error::StreamWrite)?;

        let mut reply = DmaUnmap::default();
        self.stream
            .read_exact(reply.as_mut_slice())
            .map_err(Error::StreamRead)?;
        debug!("Reply: {reply:?}");

        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        let reset = DeviceReset {
            header: Header {
                message_id: self.next_message_id.0,
                command: Command::DeviceReset as u16,
                flags: HeaderFlags::Command as u32,
                message_size: size_of::<DeviceReset>() as u32,
                ..Default::default()
            },
        };
        debug!("Command: {reset:?}");
        self.next_message_id += Wrapping(1);
        self.stream
            .write_all(reset.as_slice())
            .map_err(Error::StreamWrite)?;

        let mut reply = Header::default();
        self.stream
            .read_exact(reply.as_mut_slice())
            .map_err(Error::StreamRead)?;
        debug!("Reply: {reply:?}");

        Ok(())
    }

    fn get_regions(&mut self) -> Result<Vec<Region>, Error> {
        let get_info = DeviceGetInfo {
            header: Header {
                message_id: self.next_message_id.0,
                command: Command::DeviceGetInfo as u16,
                flags: HeaderFlags::Command as u32,
                message_size: size_of::<DeviceGetInfo>() as u32,
                ..Default::default()
            },
            argsz: size_of::<DeviceGetInfo>() as u32,
            ..Default::default()
        };
        debug!("Command: {get_info:?}");
        self.next_message_id += Wrapping(1);

        self.stream
            .write_all(get_info.as_slice())
            .map_err(Error::StreamWrite)?;

        let mut reply = DeviceGetInfo::default();
        self.stream
            .read_exact(reply.as_mut_slice())
            .map_err(Error::StreamRead)?;
        debug!("Reply: {reply:?}");
        self.num_irqs = reply.num_irqs;

        if reply.flags & VFIO_DEVICE_FLAGS_PCI != VFIO_DEVICE_FLAGS_PCI {
            return Err(Error::NotPciDevice);
        }

        self.resettable = reply.flags & VFIO_DEVICE_FLAGS_RESET == VFIO_DEVICE_FLAGS_RESET;

        let num_regions = reply.num_regions;
        let mut regions = Vec::new();
        for index in 0..num_regions {
            let (region_info, fd, sparse_areas) = self.get_region_info(index)?;
            regions.push(Region {
                flags: region_info.flags,
                index: region_info.index,
                size: region_info.size,
                file_offset: fd.map(|fd| FileOffset::new(fd, region_info.offset)),
                sparse_areas,
            });
        }

        Ok(regions)
    }

    fn get_region_info(
        &mut self,
        index: u32,
    ) -> Result<
        (
            vfio_region_info,
            Option<File>,
            Vec<vfio_region_sparse_mmap_area>,
        ),
        Error,
    > {
        // Retrieve the region info without capability
        let mut get_region_info = DeviceGetRegionInfo {
            header: Header {
                message_id: self.next_message_id.0,
                command: Command::DeviceGetRegionInfo as u16,
                flags: HeaderFlags::Command as u32,
                message_size: std::mem::size_of::<DeviceGetRegionInfo>() as u32,
                ..Default::default()
            },
            region_info: vfio_region_info {
                argsz: size_of::<vfio_region_info>() as u32,
                index,
                ..Default::default()
            },
        };
        debug!("Command: {get_region_info:?}");
        self.next_message_id += Wrapping(1);

        self.stream
            .write_all(get_region_info.as_slice())
            .map_err(Error::StreamWrite)?;

        let mut reply = DeviceGetRegionInfo::default();
        let (_, fd) = self
            .stream
            .recv_with_fd(reply.as_mut_slice())
            .map_err(Error::ReceiveWithFd)?;
        debug!("Reply: {reply:?}");

        // Retrieve the region info again with capabilities if needed
        if reply.region_info.argsz > std::mem::size_of::<vfio_region_info>() as u32 {
            get_region_info.region_info.argsz = reply.region_info.argsz;
            debug!("Command: {get_region_info:?}");
            self.next_message_id += Wrapping(1);

            self.stream
                .write_all(get_region_info.as_slice())
                .map_err(Error::StreamWrite)?;

            let mut reply = DeviceGetRegionInfo::default();
            let (_, fd) = self
                .stream
                .recv_with_fd(reply.as_mut_slice())
                .map_err(Error::ReceiveWithFd)?;
            debug!("Reply: {reply:?}");

            let cap_size = reply.region_info.argsz - std::mem::size_of::<vfio_region_info>() as u32;
            assert_eq!(
                cap_size,
                reply.header.message_size - size_of::<DeviceGetRegionInfo>() as u32
            );
            let mut cap_data = vec![0; cap_size as usize];
            self.stream
                .read_exact(cap_data.as_mut_slice())
                .map_err(Error::StreamRead)?;

            let sparse_areas = Self::parse_region_caps(&cap_data, &reply.region_info)?;

            Ok((reply.region_info, fd, sparse_areas))
        } else {
            Ok((reply.region_info, fd, Vec::new()))
        }
    }

    fn parse_region_caps(
        cap_data: &[u8],
        region_info: &vfio_region_info,
    ) -> Result<Vec<vfio_region_sparse_mmap_area>, Error> {
        let mut sparse_areas: Vec<vfio_region_sparse_mmap_area> = Vec::new();

        let cap_size = cap_data.len() as u32;
        let cap_header_size = size_of::<vfio_info_cap_header>() as u32;
        let mmap_cap_size = size_of::<vfio_region_info_cap_sparse_mmap>() as u32;
        let mmap_area_size = size_of::<vfio_region_sparse_mmap_area>() as u32;

        let cap_data_ptr = cap_data.as_ptr();
        let mut region_info_offset = region_info.cap_offset;
        while region_info_offset != 0 {
            // calculate the offset from the begining of the cap_data based on the offset
            // that is relative to the begining of the VFIO region info structure
            let cap_offset = region_info_offset - size_of::<vfio_region_info>() as u32;
            if cap_offset + cap_header_size > cap_size {
                warn!(
                    "Unexpected end of cap data: 'cap_offset + cap_header_size > cap_size' \
                cap_offset = {cap_offset}, cap_header_size = {cap_header_size}, cap_size = {cap_size}"
                );
                break;
            }

            // SAFETY: `cap_data_ptr` is valid and the `cap_offset` is checked above
            let cap_ptr = unsafe { cap_data_ptr.offset(cap_offset as isize) };
            // SAFETY: `cap_ptr` is valid
            let cap_header = unsafe { &*(cap_ptr as *const vfio_info_cap_header) };
            match cap_header.id as u32 {
                VFIO_REGION_INFO_CAP_SPARSE_MMAP => {
                    if cap_offset + mmap_cap_size > cap_size {
                        warn!(
                            "Unexpected end of cap data: 'cap_offset + mmap_cap_size > cap_size' \
                        cap_offset = {cap_offset}, mmap_cap_size = {mmap_cap_size}, cap_size = {cap_size}"
                        );
                        break;
                    }
                    // SAFETY: `cap_ptr` is valid and its size is also checked above
                    let sparse_mmap = unsafe {
                        &*(cap_ptr as *mut u8 as *const vfio_region_info_cap_sparse_mmap)
                    };

                    let area_num = sparse_mmap.nr_areas;
                    if cap_offset + mmap_cap_size + area_num * mmap_area_size > cap_size {
                        warn!("Unexpected end of cap data: 'cap_offset + mmap_cap_size + area_num * mmap_area_size > cap_size' \
                        cap_offset = {cap_offset}, mmap_cap_size = {mmap_area_size}, area_num = {area_num}, mmap_area_size = {mmap_area_size}, cap_size = {cap_size}");
                        break;
                    }
                    let areas =
                        // SAFETY: `sparse_mmap` is valid and its size is also checked above
                        unsafe { sparse_mmap.areas.as_slice(sparse_mmap.nr_areas as usize) };
                    for area in areas.iter() {
                        sparse_areas.push(*area);
                    }
                }
                _ => {
                    warn!(
                        "Ignoring unsupported vfio region capability (id = '{}')",
                        cap_header.id
                    );
                }
            }
            region_info_offset = cap_header.next;
        }

        Ok(sparse_areas)
    }

    pub fn region_read(&mut self, region: u32, offset: u64, data: &mut [u8]) -> Result<(), Error> {
        let region_read = RegionAccess {
            header: Header {
                message_id: self.next_message_id.0,
                command: Command::RegionRead as u16,
                flags: HeaderFlags::Command as u32,
                message_size: size_of::<RegionAccess>() as u32,
                ..Default::default()
            },
            offset,
            count: data.len() as u32,
            region,
        };
        debug!("Command: {region_read:?}");
        self.next_message_id += Wrapping(1);
        self.stream
            .write_all(region_read.as_slice())
            .map_err(Error::StreamWrite)?;

        let mut reply = RegionAccess::default();
        self.stream
            .read_exact(reply.as_mut_slice())
            .map_err(Error::StreamRead)?;
        debug!("Reply: {reply:?}");
        self.stream.read_exact(data).map_err(Error::StreamRead)?;
        Ok(())
    }

    pub fn region_write(&mut self, region: u32, offset: u64, data: &[u8]) -> Result<(), Error> {
        let region_write = RegionAccess {
            header: Header {
                message_id: self.next_message_id.0,
                command: Command::RegionWrite as u16,
                flags: HeaderFlags::Command as u32,
                message_size: (size_of::<RegionAccess>() + data.len()) as u32,
                ..Default::default()
            },
            offset,
            count: data.len() as u32,
            region,
        };
        debug!("Command: {region_write:?}");
        self.next_message_id += Wrapping(1);

        let bufs = vec![IoSlice::new(region_write.as_slice()), IoSlice::new(data)];

        // TODO: Use write_all_vectored() when ready
        let _ = self
            .stream
            .write_vectored(&bufs)
            .map_err(Error::StreamWrite)?;

        let mut reply = RegionAccess::default();
        self.stream
            .read_exact(reply.as_mut_slice())
            .map_err(Error::StreamRead)?;
        debug!("Reply: {reply:?}");
        Ok(())
    }

    pub fn get_irq_info(&mut self, index: u32) -> Result<IrqInfo, Error> {
        let get_irq_info = GetIrqInfo {
            header: Header {
                message_id: self.next_message_id.0,
                command: Command::GetIrqInfo as u16,
                flags: HeaderFlags::Command as u32,
                message_size: size_of::<GetIrqInfo>() as u32,
                ..Default::default()
            },
            argsz: (size_of::<GetIrqInfo>() - size_of::<Header>()) as u32,
            flags: 0,
            index,
            count: 0,
        };
        debug!("Command: {get_irq_info:?}");
        self.next_message_id += Wrapping(1);

        self.stream
            .write_all(get_irq_info.as_slice())
            .map_err(Error::StreamWrite)?;

        let mut reply = GetIrqInfo::default();
        self.stream
            .read_exact(reply.as_mut_slice())
            .map_err(Error::StreamRead)?;
        debug!("Reply: {reply:?}");

        Ok(IrqInfo {
            index: reply.index,
            flags: reply.flags,
            count: reply.count,
        })
    }

    pub fn set_irqs(
        &mut self,
        index: u32,
        flags: u32,
        start: u32,
        count: u32,
        fds: &[RawFd],
    ) -> Result<(), Error> {
        let set_irqs = SetIrqs {
            header: Header {
                message_id: self.next_message_id.0,
                command: Command::SetIrqs as u16,
                flags: HeaderFlags::Command as u32,
                message_size: size_of::<SetIrqs>() as u32,
                ..Default::default()
            },
            argsz: (size_of::<SetIrqs>() - size_of::<Header>()) as u32,
            flags,
            start,
            index,
            count,
        };
        debug!("Command: {set_irqs:?}");
        self.next_message_id += Wrapping(1);

        self.stream
            .send_with_fds(&[set_irqs.as_slice()], fds)
            .map_err(Error::SendWithFd)?;

        let mut reply = Header::default();
        self.stream
            .read_exact(reply.as_mut_slice())
            .map_err(Error::StreamRead)?;
        debug!("Reply: {reply:?}");

        Ok(())
    }

    /// Ask the server which sub-regions of region `index` it wants accessed
    /// through file descriptors (vfio-user.rst, "VFIO_USER_DEVICE_GET_REGION_IO_FDS").
    ///
    /// `Error::ErrorReply`: the server refused, for example because it does not
    /// implement the request. `Error::InvalidReply`: the reply is not in the
    /// specification's form (see [`parse_region_io_fds_reply`]). After either the
    /// connection stays usable. Other errors leave it out of step.
    pub fn get_region_io_fds(&mut self, index: u32) -> Result<RegionIoFds, Error> {
        let mut argsz = REGION_IO_FDS_FIRST_ARGSZ;
        // A truncated reply names the size needed: ask once more with that.
        for _ in 0..2 {
            // "Request": argsz is "the maximum size of the reply payload", "the
            // client must set flags to zero and specify the region being queried
            // in the index", "all other fields must be zero".
            let request = RegionIoFdsInfo {
                header: Header {
                    message_id: self.next_message_id.0,
                    command: Command::GetRegionIoFds as u16,
                    flags: HeaderFlags::Command as u32,
                    message_size: size_of::<RegionIoFdsInfo>() as u32,
                    ..Default::default()
                },
                argsz,
                flags: 0,
                index,
                count: 0,
            };
            debug!("Command: {request:?}");
            self.next_message_id += Wrapping(1);
            self.stream
                .write_all(request.as_slice())
                .map_err(Error::StreamWrite)?;

            let (reply, payload, fds) = self.recv_reply(&request.header, argsz)?;
            if reply.flags & HeaderFlags::Error as u32 != 0 {
                self.resync_after_error_reply()?;
                return Err(Error::ErrorReply {
                    command: Command::GetRegionIoFds,
                    errno: reply.error,
                });
            }
            match parse_region_io_fds_reply(argsz, index, &payload)? {
                RegionIoFdsReply::Complete { sub_regions } => {
                    return Ok(RegionIoFds {
                        index,
                        sub_regions,
                        fds,
                    })
                }
                RegionIoFdsReply::Truncated { argsz: needed } => {
                    if needed > REGION_IO_FDS_MAX_ARGSZ {
                        return Err(Error::InvalidReply(
                            Command::GetRegionIoFds,
                            "argsz above 65535",
                        ));
                    }
                    argsz = needed;
                }
            }
        }
        Err(Error::InvalidReply(
            Command::GetRegionIoFds,
            "reply truncated twice",
        ))
    }

    /// Receive the reply to `request` (vfio-user.rst, "Header"): its header, the
    /// payload, which must not exceed `max_payload` bytes, and the descriptors of
    /// the ancillary data.
    fn recv_reply(
        &mut self,
        request: &Header,
        max_payload: u32,
    ) -> Result<(Header, Vec<u8>, Vec<File>), Error> {
        let command = Command::n(request.command).unwrap_or_default();
        let mut header = Header::default();
        let mut raw_fds = vec![0 as RawFd; MAX_RECEIVED_FDS];
        let mut iovecs = vec![iovec {
            iov_base: header.as_mut_slice().as_mut_ptr() as *mut c_void,
            iov_len: size_of::<Header>(),
        }];
        // SAFETY: the iovec covers `header` and `raw_fds` has MAX_RECEIVED_FDS entries
        let (bytes, fds_received) = unsafe {
            self.stream
                .recv_with_fds(&mut iovecs, &mut raw_fds)
                .map_err(Error::ReceiveWithFd)?
        };
        let fds: Vec<File> = raw_fds[..fds_received]
            .iter()
            // SAFETY: the kernel installed these descriptors for this process
            .map(|fd| unsafe { File::from_raw_fd(*fd) })
            .collect();
        if bytes == 0 {
            return Err(Error::StreamRead(std::io::ErrorKind::UnexpectedEof.into()));
        }
        if bytes < size_of::<Header>() {
            self.stream
                .read_exact(&mut header.as_mut_slice()[bytes..])
                .map_err(Error::StreamRead)?;
        }
        debug!("Reply: {header:?} with {} fd(s)", fds.len());

        // "Header": the reply echoes the message ID and the command, and its
        // Type bits are Reply (0x1).
        if header.message_id != request.message_id
            || header.command != request.command
            || header.flags & 0xf != HeaderFlags::Reply as u32
        {
            return Err(Error::UnexpectedReply(
                command,
                "not the reply to the request",
            ));
        }
        // "Message size contains the size of the entire message, including the header".
        let payload_size = (header.message_size as usize)
            .checked_sub(size_of::<Header>())
            .ok_or(Error::UnexpectedReply(command, "message size below 16"))?;
        if payload_size > MAX_DRAINED_PAYLOAD {
            return Err(Error::UnexpectedReply(command, "message size above 1 MiB"));
        }
        let mut payload = vec![0u8; payload_size];
        self.stream
            .read_exact(&mut payload)
            .map_err(Error::StreamRead)?;
        // "Message sizes": the request's argsz "defines the maximum expected reply
        // payload size".
        if payload_size > max_payload as usize {
            return Err(Error::InvalidReply(command, "payload larger than argsz"));
        }
        Ok((header, payload, fds))
    }

    /// Put the stream back in step after an error reply. A vfio_user server up
    /// to 0.1.6 answers a command it does not implement before reading the
    /// payload, takes the payload for the next message header and answers that
    /// with one more error reply, to a command that was never sent. A
    /// `VFIO_USER_DEVICE_GET_INFO` round trip finds the end of it: one
    /// header-only error reply ahead of the GET_INFO reply is dropped. A
    /// conforming server sends none ("Each command message in Commands must be
    /// replied to with a reply message").
    fn resync_after_error_reply(&mut self) -> Result<(), Error> {
        let get_info = DeviceGetInfo {
            header: Header {
                message_id: self.next_message_id.0,
                command: Command::DeviceGetInfo as u16,
                flags: HeaderFlags::Command as u32,
                message_size: size_of::<DeviceGetInfo>() as u32,
                ..Default::default()
            },
            argsz: size_of::<DeviceGetInfo>() as u32,
            ..Default::default()
        };
        debug!("Command: {get_info:?}");
        self.next_message_id += Wrapping(1);
        self.stream
            .write_all(get_info.as_slice())
            .map_err(Error::StreamWrite)?;

        for _ in 0..2 {
            let mut header = Header::default();
            self.stream
                .read_exact(header.as_mut_slice())
                .map_err(Error::StreamRead)?;
            if header.message_id == get_info.header.message_id
                && header.command == Command::DeviceGetInfo as u16
                && header.flags & 0xf == HeaderFlags::Reply as u32
            {
                let rest = (header.message_size as usize).saturating_sub(size_of::<Header>());
                if rest > size_of::<DeviceGetInfo>() - size_of::<Header>() {
                    return Err(Error::UnexpectedReply(
                        Command::DeviceGetInfo,
                        "payload larger than the request's",
                    ));
                }
                let mut payload = vec![0u8; rest];
                self.stream
                    .read_exact(&mut payload)
                    .map_err(Error::StreamRead)?;
                return Ok(());
            }
            // "If an error occurs, the reply message must only include the reply header."
            if header.flags & HeaderFlags::Error as u32 == 0
                || header.message_size as usize != size_of::<Header>()
            {
                return Err(Error::UnexpectedReply(
                    Command::DeviceGetInfo,
                    "unexpected message while resynchronising",
                ));
            }
            warn!("Dropping an error reply to a command that was not sent: {header:?}");
        }
        Err(Error::UnexpectedReply(
            Command::DeviceGetInfo,
            "more than one stray reply",
        ))
    }

    pub fn region(&self, region_index: u32) -> Option<&Region> {
        self.regions
            .iter()
            .find(|&region| region.index == region_index)
    }

    pub fn resettable(&self) -> bool {
        self.resettable
    }

    pub fn shutdown(&self) -> Result<(), Error> {
        self.stream
            .shutdown(std::net::Shutdown::Both)
            .map_err(Error::StreamShutdown)
    }
}

pub trait ServerBackend {
    /// Explicit opt-in: generic backends never advertise this private extension.
    fn local_irq_offer(&self) -> bool { false }
    fn local_irq_prepare(&mut self) -> Result<local_irq::PreparedServer, std::io::Error> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
    fn local_irq_commit(&mut self, _token:u64, _epoch:u64, _ticket:u64)
        -> Result<(), std::io::Error> { Err(std::io::ErrorKind::Unsupported.into()) }
    fn local_irq_abort(&mut self, _token:u64) -> Result<(), std::io::Error> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
    fn local_irq_disconnected(&mut self) {}
    /// Publish all causes of the just completed command before its reply.
    fn local_irq_before_reply(&mut self) -> Result<(), std::io::Error> { Ok(()) }
    fn region_read(
        &mut self,
        _region: u32,
        _offset: u64,
        _data: &mut [u8],
    ) -> Result<(), std::io::Error>;
    fn region_write(
        &mut self,
        _region: u32,
        _offset: u64,
        _data: &[u8],
    ) -> Result<(), std::io::Error>;
    fn dma_map(
        &mut self,
        _flags: DmaMapFlags,
        _offset: u64,
        _address: u64,
        _size: u64,
        _fd: Option<File>,
    ) -> Result<(), std::io::Error>;
    fn dma_unmap(
        &mut self,
        _flags: DmaUnmapFlags,
        _address: u64,
        _size: u64,
    ) -> Result<(), std::io::Error>;
    fn reset(&mut self) -> Result<(), std::io::Error>;
    fn set_irqs(
        &mut self,
        _index: u32,
        _flags: u32,
        _start: u32,
        _count: u32,
        _fds: Vec<File>,
    ) -> Result<(), std::io::Error>;
}

#[derive(Clone, Copy)]
pub struct SparseArea {
    pub area: vfio_region_sparse_mmap_area,
}

#[derive(Clone)]
pub struct ServerRegion {
    pub region_info: vfio_region_info,
    pub sparse_areas: Vec<SparseArea>,
    pub mmap_fd: Option<RawFd>,
}

/// Whether KVM would refuse `b` as a duplicate of `a` (both of one region):
/// `KVM_IOEVENTFD` returns `EEXIST` for a second ioeventfd at the same address
/// in the same I/O space when either has length 0, or both have the same length
/// and either has no datamatch or both have the same datamatch (Linux
/// virt/kvm/eventfd.c, `ioeventfd_check_collision`).
fn ioeventfds_collide(a: &IoEventFdSubRegion, b: &IoEventFdSubRegion) -> bool {
    let wildcard = |s: &IoEventFdSubRegion| s.flags & IOEVENTFD_FLAG_DATAMATCH == 0;
    a.offset == b.offset
        && a.flags & IOEVENTFD_FLAG_PIO == b.flags & IOEVENTFD_FLAG_PIO
        && (a.size == 0
            || b.size == 0
            || (a.size == b.size && (wildcard(a) || wildcard(b) || a.datamatch == b.datamatch)))
}

pub struct Server {
    listener: UnixListener,
    path: Option<PathBuf>,
    resettable: bool,
    irqs: Vec<IrqInfo>,
    regions: Vec<ServerRegion>,
    /// Sub-regions offered through VFIO_USER_DEVICE_GET_REGION_IO_FDS, by region.
    io_fds: Vec<(u32, IoEventFdSubRegion)>,
    /// The client's max_msg_fds capability; 1 until it says otherwise
    /// (vfio-user.rst, "VFIO_USER_VERSION": "If not specified then the receiver
    /// must assume a value of 1").
    client_max_msg_fds: AtomicU32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LocalIrqPhase { Off, Negotiated, Prepared, Active }
struct SessionContext {
    version_seen: bool,
    phase: LocalIrqPhase,
    token: u64,
    epoch: u64,
    producer: Option<local_irq::QueueProducer>,
}
impl Default for SessionContext {
    fn default() -> Self { Self { version_seen:false,phase:LocalIrqPhase::Off,token:0,epoch:0,producer:None } }
}
impl SessionContext {
    fn terminal(&self) -> bool { self.producer.as_ref().is_some_and(|p|p.is_terminal()) }
}

/// Optional CPU-for-latency wait before the server's unchanged blocking header receive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdlePollMode {
    Off,
    Hybrid(Duration),
    Spin,
}

fn idle_poll_header(stream: &UnixStream, mode: IdlePollMode) -> std::io::Result<()> {
    if mode == IdlePollMode::Off {
        return Ok(());
    }
    let start = Instant::now();
    loop {
        let mut byte = 0u8;
        // SAFETY: `byte` is writable for one byte. PEEK does not consume the header or
        // its ancillary FDs; the ordinary recv_with_fds below remains their sole reader.
        let n = unsafe {
            libc::recv(
                stream.as_raw_fd(),
                &mut byte as *mut u8 as *mut c_void,
                1,
                libc::MSG_PEEK | libc::MSG_DONTWAIT,
            )
        };
        if n >= 0 {
            // A byte is ready, or EOF: let the existing receive handle either case.
            return Ok(());
        }
        let e = std::io::Error::last_os_error();
        if !matches!(
            e.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
        ) {
            return Err(e);
        }
        if let IdlePollMode::Hybrid(limit) = mode {
            if start.elapsed() >= limit {
                // Fall back to the original blocking receive after the bounded spin.
                return Ok(());
            }
        }
        std::hint::spin_loop();
    }
}

fn receive_header(stream: &mut UnixStream) -> Result<Option<(Header, Vec<File>)>, Error> {
    let mut header = Header::default();
    // The maximum number of FDs that can be sent is 16 so that is also the
    // maximum that can be received.
    let mut fds = vec![0; 16];
    let mut iovecs = vec![iovec {
        iov_base: header.as_mut_slice().as_mut_ptr() as *mut c_void,
        iov_len: header.as_mut_slice().len(),
    }];
    // SAFETY: the header and descriptor arrays are writable and correctly sized.
    let (bytes, fds_received) = unsafe {
        stream
            .recv_with_fds(&mut iovecs, &mut fds)
            .map_err(Error::ReceiveWithFd)?
    };
    fds.truncate(fds_received);
    // Own every received descriptor before read_exact can fail on a short header.
    let fds: Vec<File> = fds
        .into_iter()
        // SAFETY: recv_with_fds returned ownership of these descriptors.
        .map(|fd| unsafe { File::from_raw_fd(fd) })
        .collect();
    if bytes == 0 {
        return Ok(None);
    }
    if bytes < size_of::<Header>() {
        stream
            .read_exact(&mut header.as_mut_slice()[bytes..])
            .map_err(Error::StreamRead)?;
    }
    Ok(Some((header, fds)))
}

impl Server {
    pub fn new(
        path: &Path,
        resettable: bool,
        irqs: Vec<IrqInfo>,
        regions: Vec<ServerRegion>,
    ) -> Result<Server, Error> {
        if path.exists() {
            return Err(Error::SocketPathExists);
        }

        let listener = UnixListener::bind(path).map_err(Error::SocketBind)?;

        Ok(Server {
            listener,
            path: Some(path.to_path_buf()),
            resettable,
            irqs,
            regions,
            io_fds: Vec::new(),
            client_max_msg_fds: AtomicU32::new(default_max_msg_fds()),
        })
    }

    pub fn from_owned_fd(
        fd: OwnedFd,
        resettable: bool,
        irqs: Vec<IrqInfo>,
        regions: Vec<ServerRegion>,
    ) -> Server {
        let listener = UnixListener::from(fd);

        Server {
            listener,
            path: None,
            resettable,
            irqs,
            regions,
            io_fds: Vec::new(),
            client_max_msg_fds: AtomicU32::new(default_max_msg_fds()),
        }
    }

    /// Offer an ioeventfd sub-region of region `index` through
    /// `VFIO_USER_DEVICE_GET_REGION_IO_FDS` (vfio-user.rst, "Sub-Region IO FD info
    /// format (ioeventfd)"). A client that configures it, for example with
    /// KVM_IOEVENTFD, signals `sub_region.fd` for guest writes there instead of
    /// sending `VFIO_USER_REGION_WRITE`; the written value is not passed on. A
    /// client may also ignore it, so `region_write` must still handle the range.
    ///
    /// The sub-region must lie within the region ("offset is the offset of the
    /// start of the sub-region within the region") and `flags` must be a
    /// combination of the `IOEVENTFD_FLAG_*` values ("flags is any of").
    ///
    /// Several sub-regions may start at the same offset when each carries
    /// `IOEVENTFD_FLAG_DATAMATCH` with its own datamatch: KVM then signals the
    /// eventfd of the value written, so the server learns the value without a
    /// `VFIO_USER_REGION_WRITE`. The fields follow `KVM_IOEVENTFD`, to which
    /// "Sub-Region IO FD info format (ioeventfd)" refers: with DATAMATCH the size
    /// is the access width (1, 2, 4 or 8) and the datamatch fits in it. A
    /// sub-region KVM would refuse as a duplicate of an earlier one of the same
    /// region ([`ioeventfds_collide`]) is refused here, so that every sub-region
    /// offered can be registered.
    pub fn add_ioeventfd(
        &mut self,
        index: u32,
        sub_region: IoEventFdSubRegion,
    ) -> Result<(), Error> {
        let region = self
            .regions
            .get(index as usize)
            .ok_or(Error::InvalidInput)?;
        let end = sub_region
            .offset
            .checked_add(sub_region.size.max(1))
            .ok_or(Error::InvalidInput)?;
        if end > region.region_info.size || sub_region.flags & !IOEVENTFD_FLAGS != 0 {
            return Err(Error::InvalidInput);
        }
        if sub_region.flags & IOEVENTFD_FLAG_DATAMATCH != 0 {
            let fits = match sub_region.size {
                8 => true,
                1 | 2 | 4 => sub_region.datamatch >> (8 * sub_region.size) == 0,
                _ => false,
            };
            if !fits {
                return Err(Error::InvalidInput);
            }
        }
        if self
            .io_fds
            .iter()
            .any(|(i, s)| *i == index && ioeventfds_collide(s, &sub_region))
        {
            return Err(Error::InvalidInput);
        }
        self.io_fds.push((index, sub_region));
        Ok(())
    }

    /// The reply payload and file descriptors for a `VFIO_USER_DEVICE_GET_REGION_IO_FDS`
    /// request for region `index` with `argsz`. Sub-regions whose descriptor
    /// would exceed the client's `max_msg_fds` ("Maximum number of file
    /// descriptors that can be received by the sender in one message",
    /// "VFIO_USER_VERSION") are left out. When the reply does not fit `argsz`,
    /// only the fixed part is sent, with the argsz needed ("Message sizes").
    /// Only ioeventfd entries (type 0) are ever sent.
    fn region_io_fds_reply(&self, index: u32, argsz: u32) -> (Vec<u8>, Vec<RawFd>) {
        // One message carries at most SCM_MAX_FD descriptors, whatever the
        // client announced.
        let max_fds = self
            .client_max_msg_fds
            .load(Ordering::Relaxed)
            .min(SCM_MAX_FD) as usize;
        let mut fds: Vec<RawFd> = Vec::new();
        let mut entries = Vec::new();
        for (_, sub_region) in self.io_fds.iter().filter(|(i, _)| *i == index) {
            let fd = sub_region.fd.as_raw_fd();
            let fd_index = match fds.iter().position(|&f| f == fd) {
                Some(i) => i,
                None if fds.len() < max_fds => {
                    fds.push(fd);
                    fds.len() - 1
                }
                None => {
                    warn!(
                        "Region {index}: sub-region at {:#x} left out, the client accepts {max_fds} fd(s) per message",
                        sub_region.offset
                    );
                    continue;
                }
            };
            entries.push((sub_region, fd_index as u32));
        }
        let argsz_needed =
            REGION_IO_FDS_INFO_SIZE + entries.len() as u32 * SUB_REGION_IO_FD_INFO_SIZE;
        let complete = argsz >= argsz_needed;
        let payload = encode_region_io_fds_reply(index, &entries, complete);
        (payload, if complete { fds } else { Vec::new() })
    }

    fn handle_command(
        &self,
        backend: &mut dyn ServerBackend,
        stream: &mut UnixStream,
        header: Header,
        fds: Vec<File>,
        session: &mut SessionContext,
    ) -> Result<(), Error> {
        let command = Command::n(header.command).ok_or(Error::UnknownCommand(header.command))?;
        match command {
            Command::Unknown | Command::DmaRead | Command::DmaWrite | Command::UserDirtyPages => {
                return Err(Error::UnsupportedCommand(command));
            }
            Command::Version => {
                if header.no_reply() {
                    return Err(Error::UnexpectedNoReply(Command::Version));
                }

                if header.message_size < size_of::<Version>() as u32
                    || header.message_size > 4096 || !fds.is_empty()
                    || header.flags != HeaderFlags::Command as u32 || header.error != 0
                    || session.version_seen
                { return Err(Error::InvalidInput); }
                session.version_seen=true;

                // TODO: Make version/capabilities configurable
                let mut client_version = Version {
                    header,
                    ..Default::default()
                };
                stream
                    .read_exact(&mut client_version.as_mut_slice()[size_of::<Header>()..])
                    .map_err(Error::StreamRead)?;
                if client_version.major != 0 || client_version.minor > 1 {
                    return Err(Error::InvalidInput);
                }

                let mut raw_version_data =
                    vec![0; header.message_size as usize - size_of::<Version>()];
                stream
                    .read_exact(&mut raw_version_data)
                    .map_err(Error::StreamRead)?;
                let client_version_data = CString::from_vec_with_nul(raw_version_data)
                    .map_err(|_| Error::InvalidInput)?
                    .to_string_lossy().into_owned();
                let client_capabilities: CapabilitiesData =
                    serde_json::from_str(&client_version_data)
                        .map_err(Error::DeserializeCapabilites)?;

                info!(
                    "Received client version: major = {} minor = {} capabilities = {:?}",
                    client_version.major, client_version.minor, client_capabilities.capabilities,
                );
                self.client_max_msg_fds.store(
                    client_capabilities.capabilities.max_msg_fds,
                    Ordering::Relaxed,
                );

                let mut server_capabilities = CapabilitiesData::default();
                if backend.local_irq_offer()
                    && client_capabilities.capabilities.x_caraxes_local_irq.as_ref()
                        == Some(&LocalIrqCapability::v1())
                    && client_capabilities.capabilities.max_msg_fds >= 2
                {
                    server_capabilities.capabilities.x_caraxes_local_irq =
                        Some(LocalIrqCapability::v1());
                    session.phase = LocalIrqPhase::Negotiated;
                }
                let server_version_data = serde_json::to_string(&server_capabilities)
                    .map_err(Error::SerializeCapabilites)?;
                let server_version = Version {
                    header: Header {
                        message_id: client_version.header.message_id,
                        command: Command::Version as u16,
                        flags: HeaderFlags::Reply as u32,
                        message_size: (size_of::<Version>() + server_version_data.len() + 1) as u32,
                        ..Default::default()
                    },
                    major: 0,
                    minor: 0,
                };

                let server_version_data = CString::new(server_version_data.as_bytes()).unwrap();

                let bufs = vec![
                    IoSlice::new(server_version.as_slice()),
                    IoSlice::new(server_version_data.as_bytes_with_nul()),
                ];

                if session.phase == LocalIrqPhase::Negotiated {
                    stream.write_all(server_version.as_slice()).map_err(Error::StreamWrite)?;
                    stream.write_all(server_version_data.as_bytes_with_nul()).map_err(Error::StreamWrite)?;
                } else {
                    // Preserve generic VERSION serialization and send behavior.
                    let _ = stream.write_vectored(&bufs).map_err(Error::StreamWrite)?;
                }

                info!(
                    "Sent server version: major = {} minor = {} capabilities = {:?}",
                    server_version.major, server_version.minor, server_capabilities.capabilities
                );
            }
            Command::LocalIrq => {
                if header.message_size != 48 || header.flags != HeaderFlags::Command as u32
                    || !fds.is_empty() || session.phase == LocalIrqPhase::Off
                { return Err(Error::LocalIrqTerminal("bad local IRQ request")); }
                let mut request=[0u8;32];
                stream.read_exact(&mut request).map_err(Error::StreamRead)?;
                if &request[0..4] != b"LIRQ" || u16::from_le_bytes(request[4..6].try_into().unwrap()) != 1 {
                    return Err(Error::LocalIrqTerminal("bad local IRQ magic/version"));
                }
                let op=u16::from_le_bytes(request[6..8].try_into().unwrap());
                let token=u64::from_le_bytes(request[8..16].try_into().unwrap());
                let epoch=u64::from_le_bytes(request[16..24].try_into().unwrap());
                let ticket=u64::from_le_bytes(request[24..32].try_into().unwrap());
                match op {
                    1 if session.phase == LocalIrqPhase::Negotiated && token==0 && epoch==0 && ticket==0 => {
                        let prepared=backend.local_irq_prepare().map_err(Error::Backend)?;
                        session.producer=Some(prepared.producer.clone());
                        session.phase=LocalIrqPhase::Prepared;
                        prepared.metadata.validate().map_err(|_|Error::LocalIrqTerminal("invalid prepared metadata"))?;
                        prepared.producer.attach_terminal_stream(stream).map_err(Error::StreamRead)?;
                        if prepared.producer.metadata()!=&prepared.metadata
                            || self.client_max_msg_fds.load(Ordering::Relaxed)<2
                        {return Err(Error::LocalIrqTerminal("prepared transport mismatch"));}
                        let queue_fd=prepared.producer.queue_fd().map_err(Error::StreamRead)?;
                        let event_fd=prepared.producer.event_fd().map_err(Error::StreamRead)?;
                        session.token=prepared.metadata.token;session.epoch=prepared.metadata.epoch;
                        let reply=Header{message_id:header.message_id,command:header.command,
                            message_size:144,flags:HeaderFlags::Reply as u32,..Default::default()};
                        let mut buf=Vec::with_capacity(144);buf.extend_from_slice(reply.as_slice());
                        buf.extend_from_slice(&prepared.metadata.wire());
                        let sent=stream.send_with_fds(&[buf.as_slice()],&[queue_fd.as_raw_fd(),event_fd.as_raw_fd()])
                            .map_err(Error::SendWithFd)?;
                        if sent!=buf.len(){return Err(Error::LocalIrqTerminal("short prepare reply"));}
                    }
                    2 if session.phase == LocalIrqPhase::Prepared && token==session.token
                        && epoch==session.epoch && ticket==0
                        && session.producer.as_ref().unwrap().published_tail()==0 => {
                        backend.local_irq_commit(token,epoch,ticket).map_err(Error::Backend)?;
                        backend.local_irq_before_reply().map_err(|_|Error::LocalIrqTerminal("commit fence"))?;
                        session.phase=LocalIrqPhase::Active;
                        let reply=Header{message_id:header.message_id,command:header.command,
                            message_size:16,flags:HeaderFlags::Reply as u32,..Default::default()};
                        stream.write_all(reply.as_slice()).map_err(Error::StreamWrite)?;
                    }
                    3 if session.phase == LocalIrqPhase::Prepared && token==session.token
                        && epoch==0 && ticket==0 => {
                        backend.local_irq_abort(token).map_err(Error::Backend)?;
                        session.producer=None;session.phase=LocalIrqPhase::Negotiated;
                        let reply=Header{message_id:header.message_id,command:header.command,
                            message_size:16,flags:HeaderFlags::Reply as u32,..Default::default()};
                        stream.write_all(reply.as_slice()).map_err(Error::StreamWrite)?;
                    }
                    _ => return Err(Error::LocalIrqTerminal("local IRQ state/token")),
                }
            }
            Command::DmaMap => {
                let mut cmd = DmaMap {
                    header,
                    ..Default::default()
                };
                stream
                    .read_exact(&mut cmd.as_mut_slice()[size_of::<Header>()..])
                    .map_err(Error::StreamRead)?;

                let mut fds = fds;

                // The specification demands that the caller passes 0
                // or 1 file descriptor.
                if fds.len() > 1 {
                    return Err(Error::InvalidInput);
                }

                let result = backend.dma_map(
                        DmaMapFlags::from_bits_truncate(cmd.flags),
                        cmd.offset,
                        cmd.address,
                        cmd.size,
                        fds.pop(),
                    );
                if session.phase == LocalIrqPhase::Active {
                    backend.local_irq_before_reply().map_err(|_|Error::LocalIrqTerminal("DMA map fence"))?;
                }
                result.map_err(Error::Backend)?;

                if header.no_reply() {
                    return Ok(());
                }

                let reply = Header {
                    message_id: cmd.header.message_id,
                    command: Command::DmaMap as u16,
                    flags: HeaderFlags::Reply as u32,
                    message_size: size_of::<Header>() as u32,
                    ..Default::default()
                };
                stream
                    .write_all(reply.as_slice())
                    .map_err(Error::StreamWrite)?;
            }
            Command::DmaUnmap => {
                let mut cmd = DmaUnmap {
                    header,
                    ..Default::default()
                };
                stream
                    .read_exact(&mut cmd.as_mut_slice()[size_of::<Header>()..])
                    .map_err(Error::StreamRead)?;

                let result = backend.dma_unmap(
                        DmaUnmapFlags::from_bits_truncate(cmd.flags),
                        cmd.address,
                        cmd.size,
                    );
                if session.phase == LocalIrqPhase::Active {
                    backend.local_irq_before_reply().map_err(|_|Error::LocalIrqTerminal("DMA unmap fence"))?;
                }
                result.map_err(Error::Backend)?;

                if header.no_reply() {
                    return Ok(());
                }

                let reply = DmaUnmap {
                    header: Header {
                        message_id: cmd.header.message_id,
                        command: Command::DmaUnmap as u16,
                        flags: HeaderFlags::Reply as u32,
                        message_size: size_of::<DmaUnmap>() as u32,
                        ..Default::default()
                    },
                    argsz: cmd.argsz,
                    flags: cmd.flags,
                    address: cmd.address,
                    size: cmd.size,
                };
                stream
                    .write_all(reply.as_slice())
                    .map_err(Error::StreamWrite)?;
            }
            Command::DeviceGetInfo => {
                if header.no_reply() {
                    return Err(Error::UnexpectedNoReply(Command::DeviceGetInfo));
                }

                let mut cmd = DeviceGetInfo {
                    header,
                    ..Default::default()
                };
                stream
                    .read_exact(&mut cmd.as_mut_slice()[size_of::<Header>()..])
                    .map_err(Error::StreamRead)?;

                let reply = DeviceGetInfo {
                    header: Header {
                        message_id: cmd.header.message_id,
                        command: Command::DeviceGetInfo as u16,
                        flags: HeaderFlags::Reply as u32,
                        message_size: size_of::<DeviceGetInfo>() as u32,
                        ..Default::default()
                    },
                    argsz: size_of::<DeviceGetInfo>() as u32 - size_of::<Header>() as u32,
                    // TODO: Consider non-PCI devices
                    flags: VFIO_DEVICE_FLAGS_PCI
                        | if self.resettable {
                            VFIO_DEVICE_FLAGS_RESET
                        } else {
                            0
                        },
                    num_regions: self.regions.len() as u32,
                    num_irqs: self.irqs.len() as u32,
                };
                stream
                    .write_all(reply.as_slice())
                    .map_err(Error::StreamWrite)?;
            }
            Command::DeviceGetRegionInfo => {
                if header.no_reply() {
                    return Err(Error::UnexpectedNoReply(Command::DeviceGetRegionInfo));
                }

                let mut cmd = DeviceGetRegionInfo {
                    header,
                    ..Default::default()
                };
                stream
                    .read_exact(&mut cmd.as_mut_slice()[size_of::<Header>()..])
                    .map_err(Error::StreamRead)?;

                if cmd.region_info.index as usize >= self.regions.len() {
                    return Err(Error::InvalidInput);
                }

                let server_region = &self.regions[cmd.region_info.index as usize];
                let mut region_info = server_region.region_info;
                let sparse_areas = &server_region.sparse_areas;

                let mut cap_data: Vec<u8> = Vec::new();
                if !sparse_areas.is_empty() {
                    let cap_header = vfio_info_cap_header {
                        id: VFIO_REGION_INFO_CAP_SPARSE_MMAP as u16,
                        version: 1,
                        next: 0,
                    };
                    // SAFETY: vfio_info_cap_header is repr(C) and cap_header
                    // is correctly initialized.
                    cap_data.extend_from_slice(unsafe {
                        std::slice::from_raw_parts(
                            &cap_header as *const vfio_info_cap_header as *const u8,
                            size_of::<vfio_info_cap_header>(),
                        )
                    });
                    cap_data.extend_from_slice(&(sparse_areas.len() as u32).to_le_bytes());
                    cap_data.extend_from_slice(&0u32.to_le_bytes());
                    for sparse_area in sparse_areas {
                        // SAFETY: vfio_region_sparse_mmap_area is repr(C) and
                        // sparse_area.area is correctly initialized.
                        cap_data.extend_from_slice(unsafe {
                            std::slice::from_raw_parts(
                                &sparse_area.area as *const vfio_region_sparse_mmap_area
                                    as *const u8,
                                size_of::<vfio_region_sparse_mmap_area>(),
                            )
                        });
                    }
                }

                let send_cap_data = if !cap_data.is_empty() {
                    let full_argsz = (size_of::<vfio_region_info>() + cap_data.len()) as u32;
                    region_info.flags |= VFIO_REGION_INFO_FLAG_CAPS;
                    region_info.argsz = full_argsz;
                    region_info.cap_offset = size_of::<vfio_region_info>() as u32;
                    cmd.region_info.argsz >= full_argsz
                } else {
                    false
                };

                let message_size = if send_cap_data {
                    (size_of::<DeviceGetRegionInfo>() + cap_data.len()) as u32
                } else {
                    size_of::<DeviceGetRegionInfo>() as u32
                };

                let reply = DeviceGetRegionInfo {
                    header: Header {
                        message_id: cmd.header.message_id,
                        command: Command::DeviceGetRegionInfo as u16,
                        flags: HeaderFlags::Reply as u32,
                        message_size,
                        ..Default::default()
                    },
                    region_info,
                };

                if send_cap_data {
                    let reply_bytes = reply.as_slice();
                    let mut buf = Vec::with_capacity(reply_bytes.len() + cap_data.len());
                    buf.extend_from_slice(reply_bytes);
                    buf.extend_from_slice(&cap_data);

                    if let Some(fd) = server_region.mmap_fd {
                        stream
                            .send_with_fds(&[&buf[..]], &[fd])
                            .map_err(Error::SendWithFd)?;
                    } else {
                        stream.write_all(&buf).map_err(Error::StreamWrite)?;
                    }
                } else {
                    stream
                        .write_all(reply.as_slice())
                        .map_err(Error::StreamWrite)?;
                }
            }
            Command::GetRegionIoFds => {
                if header.no_reply() {
                    return Err(Error::UnexpectedNoReply(Command::GetRegionIoFds));
                }

                let mut cmd = RegionIoFdsInfo {
                    header,
                    ..Default::default()
                };
                stream
                    .read_exact(&mut cmd.as_mut_slice()[size_of::<Header>()..])
                    .map_err(Error::StreamRead)?;

                // vfio-user.rst, "VFIO_USER_DEVICE_GET_REGION_IO_FDS", "Request": flags
                // and count "must be zero"; argsz "should be at least the size of
                // the fixed reply payload headers" ("Message sizes").
                if cmd.flags != 0
                    || cmd.count != 0
                    || cmd.index as usize >= self.regions.len()
                    || cmd.argsz < REGION_IO_FDS_INFO_SIZE
                {
                    return Err(Error::InvalidInput);
                }

                let (payload, fds) = self.region_io_fds_reply(cmd.index, cmd.argsz);
                // "Header": the reply echoes the message ID and the command, Type
                // Reply; the descriptors go as SCM_RIGHTS ancillary data ("Reply").
                let reply = Header {
                    message_id: cmd.header.message_id,
                    command: Command::GetRegionIoFds as u16,
                    flags: HeaderFlags::Reply as u32,
                    message_size: (size_of::<Header>() + payload.len()) as u32,
                    ..Default::default()
                };
                let mut buf = Vec::with_capacity(size_of::<Header>() + payload.len());
                buf.extend_from_slice(reply.as_slice());
                buf.extend_from_slice(&payload);
                if fds.is_empty() {
                    stream.write_all(&buf).map_err(Error::StreamWrite)?;
                } else {
                    stream
                        .send_with_fds(&[&buf[..]], &fds)
                        .map_err(Error::SendWithFd)?;
                }
            }
            Command::GetIrqInfo => {
                if header.no_reply() {
                    return Err(Error::UnexpectedNoReply(Command::GetIrqInfo));
                }

                let mut cmd = GetIrqInfo {
                    header,
                    ..Default::default()
                };
                stream
                    .read_exact(&mut cmd.as_mut_slice()[size_of::<Header>()..])
                    .map_err(Error::StreamRead)?;

                if cmd.index as usize >= self.irqs.len() {
                    return Err(Error::InvalidInput);
                }

                let irq = &self.irqs[cmd.index as usize];

                let reply = GetIrqInfo {
                    header: Header {
                        message_id: cmd.header.message_id,
                        command: Command::GetIrqInfo as u16,
                        flags: HeaderFlags::Reply as u32,
                        message_size: size_of::<GetIrqInfo>() as u32,
                        ..Default::default()
                    },
                    argsz: (size_of::<GetIrqInfo>() - size_of::<Header>()) as u32,
                    index: irq.index,
                    flags: irq.flags,
                    count: irq.count,
                };
                stream
                    .write_all(reply.as_slice())
                    .map_err(Error::StreamWrite)?;
            }
            Command::SetIrqs => {
                let mut cmd = SetIrqs {
                    header,
                    ..Default::default()
                };
                stream
                    .read_exact(&mut cmd.as_mut_slice()[size_of::<Header>()..])
                    .map_err(Error::StreamRead)?;

                if cmd.index as usize >= self.irqs.len() {
                    return Err(Error::InvalidInput);
                }

                if cmd.flags & VFIO_IRQ_SET_DATA_BOOL > 0 {
                    return Err(Error::UnsupportedFeature);
                }

                let result = backend.set_irqs(cmd.index, cmd.flags, cmd.start, cmd.count, fds);
                if session.phase == LocalIrqPhase::Active {
                    backend.local_irq_before_reply().map_err(|_|Error::LocalIrqTerminal("IRQ fence"))?;
                }
                result.map_err(Error::Backend)?;

                if header.no_reply() {
                    return Ok(());
                }

                let reply = Header {
                    message_id: cmd.header.message_id,
                    command: Command::SetIrqs as u16,
                    flags: HeaderFlags::Reply as u32,
                    message_size: size_of::<Header>() as u32,
                    ..Default::default()
                };
                stream
                    .write_all(reply.as_slice())
                    .map_err(Error::StreamWrite)?;
            }
            Command::RegionRead => {
                if header.no_reply() {
                    return Err(Error::UnexpectedNoReply(Command::RegionRead));
                }

                let mut cmd = RegionAccess {
                    header,
                    ..Default::default()
                };
                stream
                    .read_exact(&mut cmd.as_mut_slice()[size_of::<Header>()..])
                    .map_err(Error::StreamRead)?;

                let (region, offset, count) = (cmd.region, cmd.offset, cmd.count);

                if region as usize >= self.regions.len() {
                    return Err(Error::InvalidInput);
                }

                let mut data = vec![0u8; count as usize];
                let result = backend.region_read(region, offset, &mut data);
                if session.phase == LocalIrqPhase::Active {
                    backend.local_irq_before_reply().map_err(|_|Error::LocalIrqTerminal("region read fence"))?;
                }
                result.map_err(Error::Backend)?;

                let reply = RegionAccess {
                    header: Header {
                        message_id: cmd.header.message_id,
                        command: Command::RegionRead as u16,
                        flags: HeaderFlags::Reply as u32,
                        message_size: size_of::<RegionAccess>() as u32 + count,
                        ..Default::default()
                    },
                    region,
                    offset,
                    count,
                };
                stream
                    .write_all(reply.as_slice())
                    .map_err(Error::StreamWrite)?;
                stream.write_all(&data).map_err(Error::StreamWrite)?;
            }
            Command::RegionWrite => {
                let mut cmd = RegionAccess {
                    header,
                    ..Default::default()
                };
                stream
                    .read_exact(&mut cmd.as_mut_slice()[size_of::<Header>()..])
                    .map_err(Error::StreamRead)?;

                let (region, offset, count) = (cmd.region, cmd.offset, cmd.count);

                if region as usize >= self.regions.len() {
                    return Err(Error::InvalidInput);
                }

                let mut data = vec![0u8; count as usize];
                stream.read_exact(&mut data).map_err(Error::StreamRead)?;
                let result = backend.region_write(region, offset, &data);
                if session.phase == LocalIrqPhase::Active {
                    backend.local_irq_before_reply().map_err(|_|Error::LocalIrqTerminal("region write fence"))?;
                }
                result.map_err(Error::Backend)?;

                if header.no_reply() {
                    return Ok(());
                }

                let reply = RegionAccess {
                    header: Header {
                        message_id: cmd.header.message_id,
                        command: Command::RegionWrite as u16,
                        flags: HeaderFlags::Reply as u32,
                        message_size: size_of::<RegionAccess>() as u32,
                        ..Default::default()
                    },
                    region,
                    offset,
                    count,
                };
                stream
                    .write_all(reply.as_slice())
                    .map_err(Error::StreamWrite)?;
            }
            Command::DeviceReset => {
                let result=backend.reset();
                if session.phase == LocalIrqPhase::Active {
                    backend.local_irq_before_reply().map_err(|_|Error::LocalIrqTerminal("reset fence"))?;
                }
                result.map_err(Error::Backend)?;

                if header.no_reply() {
                    return Ok(());
                }

                let reply = Header {
                    message_id: header.message_id,
                    command: Command::DeviceReset as u16,
                    flags: HeaderFlags::Reply as u32,
                    message_size: size_of::<Header>() as u32,
                    ..Default::default()
                };
                stream
                    .write_all(reply.as_slice())
                    .map_err(Error::StreamWrite)?;
            }
        }

        Ok(())
    }

    pub fn run(&self, backend: &mut dyn ServerBackend) -> Result<(), Error> {
        self.run_inner(backend, IdlePollMode::Off)
    }

    /// Serve with an optional busy wait before each header receive. The normal receive
    /// still owns header bytes, file descriptors, replies, and message ordering.
    pub fn run_with_idle_poll(
        &self,
        backend: &mut dyn ServerBackend,
        mode: IdlePollMode,
    ) -> Result<(), Error> {
        self.run_inner(backend, mode)
    }

    fn run_inner(&self, backend: &mut dyn ServerBackend, mode: IdlePollMode) -> Result<(), Error> {
        let (mut stream, _) = self.listener.accept().map_err(Error::SocketAccept)?;
        self.client_max_msg_fds.store(default_max_msg_fds(), Ordering::Relaxed);
        let mut session=SessionContext::default();
        let result=(|| {
            loop {
                if session.terminal(){return Err(Error::LocalIrqTerminal("producer fault"));}
                idle_poll_header(&stream, mode).map_err(Error::StreamRead)?;
                let Some((header, fds)) = receive_header(&mut stream)? else {
                    info!("Connection closed");
                    break;
                };
                if let Err(e) = self.handle_command(backend, &mut stream, header, fds, &mut session) {
                    error!("Error handling command: {:?}: {e}", header.command);
                    if session.phase == LocalIrqPhase::Prepared
                        || session.phase == LocalIrqPhase::Active
                        || header.command == Command::LocalIrq as u16
                        || matches!(e,Error::LocalIrqTerminal(_))
                    {return Err(e);}
                    let reply = Header {
                        message_id: header.message_id,
                        command: header.command,
                        flags: HeaderFlags::Error as u32 | HeaderFlags::Reply as u32,
                        message_size: size_of::<Header>() as u32,
                        error: if matches!(e, Error::InvalidInput) {EINVAL as u32} else {0},
                    };
                    stream.write_all(reply.as_slice()).map_err(Error::StreamWrite)?;
                }
            }
            Ok(())
        })();
        if session.phase == LocalIrqPhase::Prepared || session.phase == LocalIrqPhase::Active {
            if let Some(p)=&session.producer {p.terminal();}
        }
        backend.local_irq_disconnected();
        result
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn device_get_info_reset_flag_controls_client_resettable() {
        for (flags, expected) in [
            (VFIO_DEVICE_FLAGS_PCI | VFIO_DEVICE_FLAGS_RESET, true),
            (VFIO_DEVICE_FLAGS_PCI, false),
        ] {
            let (stream, mut peer) = UnixStream::pair().unwrap();
            let server = std::thread::spawn(move || {
                let mut request = DeviceGetInfo::default();
                peer.read_exact(request.as_mut_slice()).unwrap();
                assert_eq!(request.header.command, Command::DeviceGetInfo as u16);
                assert_eq!(request.header.message_size as usize, size_of::<DeviceGetInfo>());
                let reply = DeviceGetInfo {
                    header: Header {
                        message_id: request.header.message_id,
                        command: Command::DeviceGetInfo as u16,
                        flags: HeaderFlags::Reply as u32,
                        message_size: size_of::<DeviceGetInfo>() as u32,
                        ..Default::default()
                    },
                    argsz: size_of::<DeviceGetInfo>() as u32,
                    flags,
                    num_regions: 0,
                    num_irqs: 2,
                };
                peer.write_all(reply.as_slice()).unwrap();
            });
            let mut client = Client {
                stream,
                next_message_id: Wrapping(0),
                num_irqs: 0,
                resettable: !expected,
                regions: Vec::new(),
                local_irq_negotiated: false,
            };
            assert!(client.get_regions().unwrap().is_empty());
            assert_eq!(client.num_irqs, 2);
            assert_eq!(client.resettable(), expected, "advertised flags {flags:#x}");
            server.join().unwrap();
        }
    }

    #[test]
    fn idle_peek_preserves_fragmented_header_and_scm_rights() {
        let (sender, mut receiver) = UnixStream::pair().unwrap();
        // SAFETY: a new descriptor, checked and owned by `original`.
        let raw = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        assert!(raw >= 0);
        // SAFETY: `raw` was returned by eventfd above.
        let mut original = unsafe { File::from_raw_fd(raw) };
        let header = Header {
            message_id: 7,
            command: Command::DeviceReset as u16,
            message_size: size_of::<Header>() as u32,
            ..Default::default()
        };
        sender
            .send_with_fds(&[&header.as_slice()[..5]], &[original.as_raw_fd()])
            .unwrap();
        let tail = header.as_slice()[5..].to_vec();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(2));
            (&sender).write_all(&tail).unwrap();
        });

        idle_poll_header(&receiver, IdlePollMode::Spin).unwrap();
        let (received, mut fds) = receive_header(&mut receiver).unwrap().unwrap();
        writer.join().unwrap();
        assert_eq!(fds.len(), 1);
        assert_eq!(received.message_id, header.message_id);
        assert_eq!(received.command, header.command);
        fds[0].write_all(&37u64.to_ne_bytes()).unwrap();
        let mut value = [0u8; 8];
        original.read_exact(&mut value).unwrap();
        assert_eq!(u64::from_ne_bytes(value), 37);
    }

    #[test]
    fn receive_header_midframe_eof_closes_passed_fd() {
        let (sender, mut receiver) = UnixStream::pair().unwrap();
        let (passed, mut peer) = UnixStream::pair().unwrap();
        let header = Header {
            message_id: 8,
            command: Command::DeviceReset as u16,
            message_size: size_of::<Header>() as u32,
            ..Default::default()
        };
        sender
            .send_with_fds(&[&header.as_slice()[..5]], &[passed.as_raw_fd()])
            .unwrap();
        drop(passed);
        drop(sender);

        let err = receive_header(&mut receiver).unwrap_err();
        assert!(matches!(
            err,
            Error::StreamRead(e) if e.kind() == std::io::ErrorKind::UnexpectedEof
        ));
        peer.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
        let mut byte = [0u8];
        assert_eq!(peer.read(&mut byte).unwrap(), 0);
    }

    #[test]
    fn idle_poll_hybrid_falls_back_and_spin_observes_eof() {
        let (sender, mut receiver) = UnixStream::pair().unwrap();
        idle_poll_header(&receiver, IdlePollMode::Hybrid(Duration::from_micros(100))).unwrap();
        drop(sender);
        idle_poll_header(&receiver, IdlePollMode::Spin).unwrap();
        assert!(receive_header(&mut receiver).unwrap().is_none());
    }

    fn build_sparse_cap_data(areas: &[vfio_region_sparse_mmap_area]) -> Vec<u8> {
        let mut cap_data: Vec<u8> = Vec::new();
        let cap_header = vfio_info_cap_header {
            id: VFIO_REGION_INFO_CAP_SPARSE_MMAP as u16,
            version: 1,
            next: 0,
        };
        // SAFETY: `cap_header` is a valid, initialized repr(C) struct.
        cap_data.extend_from_slice(unsafe {
            std::slice::from_raw_parts(
                &cap_header as *const vfio_info_cap_header as *const u8,
                size_of::<vfio_info_cap_header>(),
            )
        });
        cap_data.extend_from_slice(&(areas.len() as u32).to_le_bytes());
        cap_data.extend_from_slice(&0u32.to_le_bytes());
        for area in areas {
            // SAFETY: `area` is a valid, initialized repr(C) struct.
            cap_data.extend_from_slice(unsafe {
                std::slice::from_raw_parts(
                    area as *const vfio_region_sparse_mmap_area as *const u8,
                    size_of::<vfio_region_sparse_mmap_area>(),
                )
            });
        }
        cap_data
    }

    #[test]
    fn test_parse_sparse_mmap_caps() {
        let areas = vec![
            vfio_region_sparse_mmap_area {
                offset: 0x0,
                size: 0x1000,
            },
            vfio_region_sparse_mmap_area {
                offset: 0x2000,
                size: 0x3000,
            },
        ];

        let cap_data = build_sparse_cap_data(&areas);

        let region_info = vfio_region_info {
            argsz: (size_of::<vfio_region_info>() + cap_data.len()) as u32,
            flags: VFIO_REGION_INFO_FLAG_CAPS,
            cap_offset: size_of::<vfio_region_info>() as u32,
            ..Default::default()
        };

        let parsed = Client::parse_region_caps(&cap_data, &region_info).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].offset, 0x0);
        assert_eq!(parsed[0].size, 0x1000);
        assert_eq!(parsed[1].offset, 0x2000);
        assert_eq!(parsed[1].size, 0x3000);
    }

    #[test]
    fn test_parse_empty_sparse_mmap_caps() {
        let cap_data = build_sparse_cap_data(&[]);

        let region_info = vfio_region_info {
            argsz: (size_of::<vfio_region_info>() + cap_data.len()) as u32,
            flags: VFIO_REGION_INFO_FLAG_CAPS,
            cap_offset: size_of::<vfio_region_info>() as u32,
            ..Default::default()
        };

        let parsed = Client::parse_region_caps(&cap_data, &region_info).unwrap();
        assert!(parsed.is_empty());
    }

    /// One Sub-Region IO FD info entry, byte by byte as vfio-user.rst, "Sub-Region
    /// IO FD info format (ioeventfd)" and "(ioregionfd)" lay it out: offset 0 (8),
    /// size 8 (8), fd_index 16 (4), type 20 (4), flags 24 (4), padding 28 (4),
    /// datamatch or user_data 32 (8).
    #[allow(clippy::too_many_arguments)]
    fn spec_entry(
        offset: u64,
        size: u64,
        fd_index: u32,
        type_: u32,
        flags: u32,
        padding: u32,
        last: u64,
    ) -> Vec<u8> {
        let mut e = vec![0u8; 40];
        e[0..8].copy_from_slice(&offset.to_ne_bytes());
        e[8..16].copy_from_slice(&size.to_ne_bytes());
        e[16..20].copy_from_slice(&fd_index.to_ne_bytes());
        e[20..24].copy_from_slice(&type_.to_ne_bytes());
        e[24..28].copy_from_slice(&flags.to_ne_bytes());
        e[28..32].copy_from_slice(&padding.to_ne_bytes());
        e[32..40].copy_from_slice(&last.to_ne_bytes());
        e
    }

    /// libvfio-user's ioeventfd entry (include/vfio-user.h:178-187 at
    /// 803924493a7c): gpa_offset 0, size 8, fd_index 16, type 20, flags 24,
    /// shadow_mem_fd_index 28, shadow_offset 32, datamatch 40; 48 bytes.
    fn libvfio_user_entry(offset: u64, size: u64, flags: u32, datamatch: u64) -> Vec<u8> {
        let mut e = vec![0u8; 48];
        e[0..8].copy_from_slice(&offset.to_ne_bytes());
        e[8..16].copy_from_slice(&size.to_ne_bytes());
        e[24..28].copy_from_slice(&flags.to_ne_bytes());
        e[40..48].copy_from_slice(&datamatch.to_ne_bytes());
        e
    }

    /// A reply payload per vfio-user.rst, "VFIO_USER_DEVICE_GET_REGION_IO_FDS",
    /// "Reply": argsz 0 (4), flags 4 (4), index 8 (4), count 12 (4), sub-regions
    /// from 16.
    fn spec_reply(argsz: u32, flags: u32, index: u32, count: u32, entries: &[Vec<u8>]) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend_from_slice(&argsz.to_ne_bytes());
        p.extend_from_slice(&flags.to_ne_bytes());
        p.extend_from_slice(&index.to_ne_bytes());
        p.extend_from_slice(&count.to_ne_bytes());
        for e in entries {
            p.extend_from_slice(e);
        }
        p
    }

    #[test]
    fn test_parse_region_io_fds_reply_spec_layout() {
        let entries = [
            spec_entry(0x90, 4, 0, 0, IOEVENTFD_FLAG_DATAMATCH, 0, 0x1234_5678),
            // The padding at 28 is not read.
            spec_entry(0x1000, 8, 1, 1, 0, 0xffff_ffff, 0xdead_beef_0000_0001),
        ];
        let payload = spec_reply(16 + 2 * 40, 0, 3, 2, &entries);
        let reply = parse_region_io_fds_reply(REGION_IO_FDS_FIRST_ARGSZ, 3, &payload).unwrap();
        assert_eq!(
            reply,
            RegionIoFdsReply::Complete {
                sub_regions: vec![
                    SubRegionIoFd::IoEventFd {
                        offset: 0x90,
                        size: 4,
                        fd_index: 0,
                        flags: IOEVENTFD_FLAG_DATAMATCH,
                        datamatch: 0x1234_5678,
                    },
                    SubRegionIoFd::IoRegionFd {
                        offset: 0x1000,
                        size: 8,
                        fd_index: 1,
                        flags: 0,
                        user_data: 0xdead_beef_0000_0001,
                    },
                ],
            }
        );
    }

    #[test]
    fn test_parse_region_io_fds_reply_refuses_libvfio_user_layout() {
        // What libvfio-user sends for two ioeventfds with datamatch: 48-byte
        // entries, datamatch at 40. Read by the specification, offset 32 would
        // give shadow_offset (0) as the datamatch; the reply is refused instead.
        let entries = [
            libvfio_user_entry(0x10, 4, IOEVENTFD_FLAG_DATAMATCH, 7),
            libvfio_user_entry(0x20, 8, IOEVENTFD_FLAG_DATAMATCH, 9),
        ];
        let payload = spec_reply(16 + 2 * 48, 0, 0, 2, &entries);
        assert!(matches!(
            parse_region_io_fds_reply(REGION_IO_FDS_FIRST_ARGSZ, 0, &payload),
            Err(Error::InvalidReply(
                Command::GetRegionIoFds,
                "argsz is not 16 + count * 40"
            ))
        ));
        // One entry alone, too.
        let payload = spec_reply(16 + 48, 0, 0, 1, &entries[..1]);
        assert!(matches!(
            parse_region_io_fds_reply(REGION_IO_FDS_FIRST_ARGSZ, 0, &payload),
            Err(Error::InvalidReply(..))
        ));
        // libvfio-user's shadow ioeventfd type 2 in a 40-byte entry.
        let payload = spec_reply(56, 0, 0, 1, &[spec_entry(0, 4, 0, 2, 0, 1, 0)]);
        assert!(matches!(
            parse_region_io_fds_reply(REGION_IO_FDS_FIRST_ARGSZ, 0, &payload),
            Err(Error::InvalidReply(
                Command::GetRegionIoFds,
                "sub-region type is neither ioeventfd nor ioregionfd"
            ))
        ));
    }

    #[test]
    fn test_parse_region_io_fds_reply_empty_and_truncated() {
        let payload = spec_reply(16, 0, 2, 0, &[]);
        assert_eq!(
            parse_region_io_fds_reply(REGION_IO_FDS_FIRST_ARGSZ, 2, &payload).unwrap(),
            RegionIoFdsReply::Complete {
                sub_regions: Vec::new()
            }
        );
        // "Message sizes": argsz above the request's maximum, payload left out.
        let payload = spec_reply(16 + 20 * 40, 0, 2, 20, &[]);
        assert_eq!(
            parse_region_io_fds_reply(REGION_IO_FDS_FIRST_ARGSZ, 2, &payload).unwrap(),
            RegionIoFdsReply::Truncated { argsz: 816 }
        );
    }

    #[test]
    fn test_parse_region_io_fds_reply_rejects_malformed() {
        let e = spec_entry(0, 4, 0, 0, 0, 0, 0);
        let mut long = spec_reply(56, 0, 0, 1, std::slice::from_ref(&e));
        long.extend_from_slice(&[0; 8]);
        let mut long_truncated = spec_reply(1000, 0, 0, 24, &[]);
        long_truncated.extend_from_slice(&e);
        let cases: Vec<(Vec<u8>, &str)> = vec![
            (vec![0; 15], "short"),
            (spec_reply(56, 1, 0, 1, std::slice::from_ref(&e)), "flags"),
            (spec_reply(56, 0, 1, 1, std::slice::from_ref(&e)), "index"),
            (spec_reply(8, 0, 0, 0, &[]), "argsz below 16"),
            (
                spec_reply(56, 0, 0, 0, std::slice::from_ref(&e)),
                "count 0 with an entry",
            ),
            (
                spec_reply(56, 0, 0, 2, std::slice::from_ref(&e)),
                "count 2 with one entry",
            ),
            (
                spec_reply(96, 0, 0, 2, std::slice::from_ref(&e)),
                "payload shorter than argsz",
            ),
            (long, "payload longer than argsz"),
            (long_truncated, "truncated reply with entries"),
        ];
        for (payload, what) in cases {
            assert!(
                matches!(
                    parse_region_io_fds_reply(REGION_IO_FDS_FIRST_ARGSZ, 0, &payload),
                    Err(Error::InvalidReply(Command::GetRegionIoFds, _))
                ),
                "{what}"
            );
        }
    }

    fn eventfd() -> OwnedFd {
        // SAFETY: plain syscall; the result is checked before use
        let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC) };
        assert!(fd >= 0);
        // SAFETY: a new descriptor owned by nobody else
        unsafe { OwnedFd::from_raw_fd(fd) }
    }

    #[test]
    fn test_encode_region_io_fds_reply() {
        let a = IoEventFdSubRegion {
            offset: 0x90,
            size: 4,
            flags: IOEVENTFD_FLAG_DATAMATCH,
            datamatch: 0x2a,
            fd: Arc::new(eventfd()),
        };
        let b = IoEventFdSubRegion {
            offset: 0x94,
            size: 0,
            flags: 0,
            datamatch: 0,
            fd: Arc::new(eventfd()),
        };
        let payload = encode_region_io_fds_reply(5, &[(&a, 0), (&b, 1)], true);
        assert_eq!(
            payload,
            spec_reply(
                96,
                0,
                5,
                2,
                &[
                    spec_entry(0x90, 4, 0, 0, IOEVENTFD_FLAG_DATAMATCH, 0, 0x2a),
                    spec_entry(0x94, 0, 1, 0, 0, 0, 0),
                ]
            )
        );
        // Truncated: the fixed part with the argsz needed, and the count.
        assert_eq!(
            encode_region_io_fds_reply(5, &[(&a, 0), (&b, 1)], false),
            spec_reply(96, 0, 5, 2, &[])
        );
    }

    struct NullBackend;

    impl ServerBackend for NullBackend {
        fn region_read(&mut self, _: u32, _: u64, _: &mut [u8]) -> Result<(), std::io::Error> {
            Ok(())
        }
        fn region_write(&mut self, _: u32, _: u64, _: &[u8]) -> Result<(), std::io::Error> {
            Ok(())
        }
        fn dma_map(
            &mut self,
            _: DmaMapFlags,
            _: u64,
            _: u64,
            _: u64,
            _: Option<File>,
        ) -> Result<(), std::io::Error> {
            Ok(())
        }
        fn dma_unmap(&mut self, _: DmaUnmapFlags, _: u64, _: u64) -> Result<(), std::io::Error> {
            Ok(())
        }
        fn reset(&mut self) -> Result<(), std::io::Error> {
            Ok(())
        }
        fn set_irqs(
            &mut self,
            _: u32,
            _: u32,
            _: u32,
            _: u32,
            _: Vec<File>,
        ) -> Result<(), std::io::Error> {
            Ok(())
        }
    }

    struct LocalBackend {
        producer: Option<local_irq::QueueProducer>,
        metadata: Option<local_irq::Metadata>,
        active: bool,
        just_committed: bool,
        fail_write: bool,
        disconnects: Arc<std::sync::atomic::AtomicU32>,
    }

    impl LocalBackend {
        fn new(fail_write: bool, disconnects: Arc<std::sync::atomic::AtomicU32>) -> Self {
            Self { producer:None,metadata:None,active:false,just_committed:false,fail_write,disconnects }
        }
    }

    impl ServerBackend for LocalBackend {
        fn local_irq_offer(&self)->bool {true}
        fn local_irq_prepare(&mut self)->Result<local_irq::PreparedServer,std::io::Error>{
            let metadata=local_irq::Metadata::fresh(0,0xb81000,0xb82000,[0;8],1000,true)?;
            let producer=local_irq::QueueProducer::create(&metadata)
                .map_err(std::io::Error::other)?;
            self.producer=Some(producer.clone());self.metadata=Some(metadata.clone());
            Ok(local_irq::PreparedServer{metadata,producer})
        }
        fn local_irq_commit(&mut self,token:u64,epoch:u64,_ticket:u64)->Result<(),std::io::Error>{
            let m=self.metadata.as_ref().unwrap();
            if token!=m.token || epoch!=m.epoch {return Err(std::io::ErrorKind::InvalidInput.into());}
            self.active=true;self.just_committed=true;Ok(())
        }
        fn local_irq_abort(&mut self,token:u64)->Result<(),std::io::Error>{
            if self.metadata.as_ref().is_none_or(|m|m.token!=token){return Err(std::io::ErrorKind::InvalidInput.into());}
            self.producer=None;self.metadata=None;Ok(())
        }
        fn local_irq_disconnected(&mut self){
            self.disconnects.fetch_add(1,Ordering::Relaxed);
            self.producer=None;self.metadata=None;self.active=false;
        }
        fn local_irq_before_reply(&mut self)->Result<(),std::io::Error>{
            if self.just_committed {self.just_committed=false;return Ok(());}
            if self.active {self.producer.as_ref().unwrap().reserve()
                .and_then(|r|r.publish(local_irq::RecordKind::Hints([7;8])))
                .map_err(std::io::Error::other)?;}
            Ok(())
        }
        fn region_read(&mut self,_:u32,_:u64,data:&mut[u8])->Result<(),std::io::Error>{data.fill(0x5a);Ok(())}
        fn region_write(&mut self,_:u32,_:u64,_:&[u8])->Result<(),std::io::Error>{
            if self.fail_write {Err(std::io::ErrorKind::Other.into())}else{Ok(())}
        }
        fn dma_map(&mut self,_:DmaMapFlags,_:u64,_:u64,_:u64,_:Option<File>)->Result<(),std::io::Error>{Ok(())}
        fn dma_unmap(&mut self,_:DmaUnmapFlags,_:u64,_:u64)->Result<(),std::io::Error>{Ok(())}
        fn reset(&mut self)->Result<(),std::io::Error>{Ok(())}
        fn set_irqs(&mut self,_:u32,_:u32,_:u32,_:u32,_:Vec<File>)->Result<(),std::io::Error>{Ok(())}
    }

    fn local_test_path(tag:&str)->PathBuf {
        std::env::temp_dir().join(format!("vfio-local-irq-{tag}-{}-{}.sock",
            std::process::id(),std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
    }

    #[test]
    fn local_irq_version_prepare_two_fds_commit_and_reply_fence(){
        let path=local_test_path("complete");
        let server=Server::new(&path,false,Vec::new(),vec![region(0,4096)]).unwrap();
        let disconnects=Arc::new(AtomicU32::new(0));let seen=disconnects.clone();
        let worker=std::thread::spawn(move||server.run(&mut LocalBackend::new(false,seen)));
        let mut client=Client::new_with_options(&path,ClientOptions{local_irq_required:true}).unwrap();
        assert!(client.local_irq_negotiated());
        let prepared=client.local_irq_prepare().unwrap();
        assert_eq!(prepared.metadata.initial_ticket,0);
        let mut consumer=local_irq::QueueConsumer::map(&prepared).unwrap();
        client.local_irq_commit(prepared.metadata.token,prepared.metadata.epoch,0).unwrap();
        let mut data=[0;4];client.region_read(0,0,&mut data).unwrap();
        assert_eq!(data,[0x5a;4]);
        let ticket=consumer.published_tail().unwrap();
        assert_eq!(ticket,1,"backend fence publishes before RPC reply");
        assert_eq!(consumer.drain_through(ticket).unwrap()[0].kind,
            local_irq::RecordKind::Hints([7;8]));
        drop(client);assert!(worker.join().unwrap().is_ok());
        assert_eq!(disconnects.load(Ordering::Relaxed),1);
    }

    #[test]
    fn local_irq_default_is_generic_and_required_refuses_unsupported_peer(){
        let path=local_test_path("generic");
        let server=Server::new(&path,false,Vec::new(),vec![region(0,4096)]).unwrap();
        let worker=std::thread::spawn(move||server.run(&mut NullBackend));
        assert!(matches!(Client::new_with_options(&path,ClientOptions{local_irq_required:true}),
            Err(Error::UnsupportedFeature)));
        assert!(worker.join().unwrap().is_ok());

        let path=local_test_path("optout");
        let server=Server::new(&path,false,Vec::new(),vec![region(0,4096)]).unwrap();
        let worker=std::thread::spawn(move||server.run(&mut LocalBackend::new(false,Arc::new(AtomicU32::new(0)))));
        let client=Client::new(&path).unwrap();
        assert!(!client.local_irq_negotiated());
        drop(client);assert!(worker.join().unwrap().is_ok());
    }

    #[test]
    fn local_irq_backend_error_after_grant_closes_connection(){
        let path=local_test_path("error");
        let server=Server::new(&path,false,Vec::new(),vec![region(0,4096)]).unwrap();
        let disconnects=Arc::new(AtomicU32::new(0));let seen=disconnects.clone();
        let worker=std::thread::spawn(move||server.run(&mut LocalBackend::new(true,seen)));
        let mut client=Client::new_with_options(&path,ClientOptions{local_irq_required:true}).unwrap();
        let prepared=client.local_irq_prepare().unwrap();
        client.local_irq_commit(prepared.metadata.token,prepared.metadata.epoch,0).unwrap();
        assert!(client.region_write(0,0,&[1]).is_err());
        assert!(worker.join().unwrap().is_err());
        assert_eq!(disconnects.load(Ordering::Relaxed),1);
    }

    #[test]
    fn local_irq_prepare_rejects_missing_fd_and_private_error_reply(){
        for error in [false,true] {
            let (stream,mut peer)=UnixStream::pair().unwrap();
            let server=std::thread::spawn(move||{
                let mut request=[0u8;48];peer.read_exact(&mut request).unwrap();
                assert_eq!(&request[16..20],b"LIRQ");
                assert_eq!(u16::from_le_bytes(request[22..24].try_into().unwrap()),1);
                let message_id=u16::from_ne_bytes(request[0..2].try_into().unwrap());
                let reply=Header{message_id,command:Command::LocalIrq as u16,
                    message_size:if error {16}else{144},
                    flags:HeaderFlags::Reply as u32 | if error {HeaderFlags::Error as u32}else{0},
                    error:if error {EINVAL as u32}else{0}};
                if error {peer.write_all(reply.as_slice()).unwrap();}
                else {
                    let mut buf=Vec::from(reply.as_slice());buf.extend_from_slice(&[0;128]);
                    let fd=eventfd();
                    peer.send_with_fds(&[buf.as_slice()],&[fd.as_raw_fd()]).unwrap();
                }
            });
            let mut client=Client{stream,next_message_id:Wrapping(0),num_irqs:0,
                resettable:false,regions:Vec::new(),local_irq_negotiated:true};
            let result=client.local_irq_prepare();
            if error {assert!(matches!(result,Err(Error::ErrorReply{command:Command::LocalIrq,errno}) if errno==EINVAL as u32));}
            else {assert!(matches!(result,Err(Error::InvalidReply(Command::LocalIrq,"prepare size or FD count"))));}
            server.join().unwrap();
        }
    }

    fn region(index: u32, size: u64) -> ServerRegion {
        ServerRegion {
            region_info: vfio_region_info {
                argsz: size_of::<vfio_region_info>() as u32,
                flags: VFIO_REGION_INFO_FLAG_READ | VFIO_REGION_INFO_FLAG_WRITE,
                index,
                size,
                ..Default::default()
            },
            sparse_areas: Vec::new(),
            mmap_fd: None,
        }
    }

    #[test]
    fn test_region_io_fds_round_trip() {
        let path =
            std::env::temp_dir().join(format!("vfio-user-io-fds-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut server = Server::new(
            &path,
            false,
            Vec::new(),
            vec![region(0, 0x1000), region(1, 0)],
        )
        .unwrap();

        // Out of bounds and unknown flags are refused.
        for (offset, size, flags) in [(0xffc, 8, 0), (0x1000, 0, 0), (0, 4, 1 << 2)] {
            let r = server.add_ioeventfd(
                0,
                IoEventFdSubRegion {
                    offset,
                    size,
                    flags,
                    datamatch: 0,
                    fd: Arc::new(eventfd()),
                },
            );
            assert!(matches!(r, Err(Error::InvalidInput)));
        }

        // 20 sub-regions on one eventfd (more than the client's first argsz
        // holds), then one on a second eventfd, which the client's max_msg_fds
        // (CLIENT_MAX_MSG_FDS) admits.
        let shared = Arc::new(eventfd());
        let observer = File::from(shared.try_clone().unwrap());
        for i in 0..20u64 {
            server
                .add_ioeventfd(
                    0,
                    IoEventFdSubRegion {
                        offset: 0x80 + 4 * i,
                        size: 4,
                        flags: IOEVENTFD_FLAG_DATAMATCH,
                        datamatch: 0x100 + i,
                        fd: shared.clone(),
                    },
                )
                .unwrap();
        }
        server
            .add_ioeventfd(
                0,
                IoEventFdSubRegion {
                    offset: 0x200,
                    size: 0,
                    flags: 0,
                    datamatch: 0,
                    fd: Arc::new(eventfd()),
                },
            )
            .unwrap();

        let thread = std::thread::spawn(move || server.run(&mut NullBackend));
        let mut client = Client::new(&path).unwrap();

        let r = client.get_region_io_fds(0).unwrap();
        assert_eq!(r.index, 0);
        assert_eq!(r.sub_regions.len(), 21);
        assert_eq!(r.fds.len(), 2);
        assert_eq!(
            r.sub_regions[20],
            SubRegionIoFd::IoEventFd {
                offset: 0x200,
                size: 0,
                fd_index: 1,
                flags: 0,
                datamatch: 0,
            }
        );
        for (i, s) in r.sub_regions[..20].iter().enumerate() {
            assert_eq!(
                *s,
                SubRegionIoFd::IoEventFd {
                    offset: 0x80 + 4 * i as u64,
                    size: 4,
                    fd_index: 0,
                    flags: IOEVENTFD_FLAG_DATAMATCH,
                    datamatch: 0x100 + i as u64,
                }
            );
        }
        // The received descriptor is the server's eventfd.
        (&r.fds[0]).write_all(&3u64.to_ne_bytes()).unwrap();
        let mut value = [0u8; 8];
        (&observer).read_exact(&mut value).unwrap();
        assert_eq!(u64::from_ne_bytes(value), 3);

        // No sub-regions: count 0, no descriptors.
        let r = client.get_region_io_fds(1).unwrap();
        assert!(r.sub_regions.is_empty() && r.fds.is_empty());

        // A refused request leaves the connection usable.
        assert!(matches!(
            client.get_region_io_fds(7),
            Err(Error::ErrorReply { command: Command::GetRegionIoFds, errno }) if errno == EINVAL as u32
        ));
        assert_eq!(client.get_region_io_fds(0).unwrap().sub_regions.len(), 21);

        client.shutdown().unwrap();
        thread.join().unwrap().unwrap();
    }

    fn datamatch_sub_region(offset: u64, size: u64, datamatch: u64) -> IoEventFdSubRegion {
        IoEventFdSubRegion {
            offset,
            size,
            flags: IOEVENTFD_FLAG_DATAMATCH,
            datamatch,
            fd: Arc::new(eventfd()),
        }
    }

    /// Several sub-regions at one offset, told apart by datamatch (a token pool
    /// on one doorbell register): what KVM_IOEVENTFD would refuse as a duplicate
    /// is refused when declared, the rest is accepted.
    #[test]
    fn test_add_ioeventfd_datamatch_at_one_offset() {
        let path =
            std::env::temp_dir().join(format!("vfio-user-io-fds-dm-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut server = Server::new(&path, false, Vec::new(), vec![region(0, 0x1000)]).unwrap();
        for token in 0..4u64 {
            server
                .add_ioeventfd(0, datamatch_sub_region(0x90, 4, 0xca70_0000 + token))
                .unwrap();
        }
        // The same datamatch again, a wildcard or an any-size entry at the same
        // offset: KVM would answer EEXIST.
        let refused = [
            datamatch_sub_region(0x90, 4, 0xca70_0002),
            IoEventFdSubRegion {
                offset: 0x90,
                size: 4,
                flags: 0,
                datamatch: 0,
                fd: Arc::new(eventfd()),
            },
            IoEventFdSubRegion {
                offset: 0x90,
                size: 0,
                flags: 0,
                datamatch: 0,
                fd: Arc::new(eventfd()),
            },
            // DATAMATCH needs an access width, and the value must fit it.
            datamatch_sub_region(0x100, 0, 1),
            datamatch_sub_region(0x100, 3, 1),
            datamatch_sub_region(0x100, 2, 0x1_0000),
            datamatch_sub_region(0x100, 4, 0x1_0000_0000),
        ];
        for s in refused {
            assert!(matches!(
                server.add_ioeventfd(0, s),
                Err(Error::InvalidInput)
            ));
        }
        // Another width at the same offset, or the same value at another offset,
        // is a different ioeventfd for KVM.
        server
            .add_ioeventfd(0, datamatch_sub_region(0x90, 8, 0xca70_0000))
            .unwrap();
        server
            .add_ioeventfd(0, datamatch_sub_region(0x94, 4, 0xca70_0000))
            .unwrap();
        assert_eq!(server.io_fds.len(), 6);
        let _ = std::fs::remove_file(&path);
    }

    /// A reply never carries more descriptors than the client's max_msg_fds nor
    /// more than SCM_MAX_FD; sub-regions beyond are left out (they stay trapped).
    #[test]
    fn test_region_io_fds_reply_descriptor_limits() {
        let path =
            std::env::temp_dir().join(format!("vfio-user-io-fds-lim-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut server = Server::new(&path, false, Vec::new(), vec![region(0, 0x1000)]).unwrap();
        for token in 0..300u64 {
            server
                .add_ioeventfd(0, datamatch_sub_region(0x90, 4, token))
                .unwrap();
        }
        let argsz = REGION_IO_FDS_MAX_ARGSZ;
        for (announced, expected) in [(1u32, 1usize), (64, 64), (253, 253), (1000, 253)] {
            server
                .client_max_msg_fds
                .store(announced, Ordering::Relaxed);
            let (payload, fds) = server.region_io_fds_reply(0, argsz);
            assert_eq!(fds.len(), expected, "max_msg_fds {announced}");
            match parse_region_io_fds_reply(argsz, 0, &payload).unwrap() {
                RegionIoFdsReply::Complete { sub_regions } => {
                    assert_eq!(sub_regions.len(), expected);
                    for (i, s) in sub_regions.iter().enumerate() {
                        assert_eq!(
                            *s,
                            SubRegionIoFd::IoEventFd {
                                offset: 0x90,
                                size: 4,
                                fd_index: i as u32,
                                flags: IOEVENTFD_FLAG_DATAMATCH,
                                datamatch: i as u64,
                            }
                        );
                    }
                }
                r => panic!("{r:?}"),
            }
        }
        let _ = std::fs::remove_file(&path);
    }

    /// The whole path of a token pool: the client announces max_msg_fds
    /// CLIENT_MAX_MSG_FDS, the server offers SCM_MAX_FD datamatch sub-regions at
    /// one offset with one eventfd each (and one more, left out), and every
    /// received descriptor is the eventfd of its entry's datamatch.
    #[test]
    fn test_region_io_fds_round_trip_one_eventfd_per_datamatch() {
        let path =
            std::env::temp_dir().join(format!("vfio-user-io-fds-pool-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut server =
            Server::new(&path, false, Vec::new(), vec![region(0, 0x100_0000)]).unwrap();
        let mut observers = Vec::new();
        for token in 0..=SCM_MAX_FD as u64 {
            let s = datamatch_sub_region(0xbb_0090, 4, 0xca70_0000 + token);
            observers.push(File::from(s.fd.try_clone().unwrap()));
            server.add_ioeventfd(0, s).unwrap();
        }
        let thread = std::thread::spawn(move || server.run(&mut NullBackend));
        let mut client = Client::new(&path).unwrap();
        let r = client.get_region_io_fds(0).unwrap();
        assert_eq!(r.sub_regions.len(), SCM_MAX_FD as usize);
        assert_eq!(r.fds.len(), SCM_MAX_FD as usize);
        for (i, s) in r.sub_regions.iter().enumerate() {
            let SubRegionIoFd::IoEventFd {
                offset,
                size,
                fd_index,
                flags,
                datamatch,
            } = *s
            else {
                panic!("{s:?}");
            };
            assert_eq!(
                (offset, size, flags),
                (0xbb_0090, 4, IOEVENTFD_FLAG_DATAMATCH)
            );
            assert_eq!(datamatch, 0xca70_0000 + i as u64);
            // Signal the received descriptor; the server's eventfd of that token
            // counts it.
            (&r.fds[fd_index as usize])
                .write_all(&(i as u64 + 1).to_ne_bytes())
                .unwrap();
            let mut value = [0u8; 8];
            (&observers[i]).read_exact(&mut value).unwrap();
            assert_eq!(u64::from_ne_bytes(value), i as u64 + 1);
        }
        client.shutdown().unwrap();
        thread.join().unwrap().unwrap();
    }

    /// A hand-built reply in the specification's layout with several datamatch
    /// entries at one offset, each naming its own descriptor.
    #[test]
    fn test_parse_region_io_fds_reply_datamatch_pool() {
        let entries: Vec<Vec<u8>> = (0..3u32)
            .map(|i| {
                spec_entry(
                    0xbb_0090,
                    4,
                    i,
                    VFIO_USER_IO_FD_TYPE_IOEVENTFD,
                    IOEVENTFD_FLAG_DATAMATCH,
                    0,
                    0xca70_0000 + u64::from(i),
                )
            })
            .collect();
        let payload = spec_reply(16 + 3 * 40, 0, 0, 3, &entries);
        let reply = parse_region_io_fds_reply(REGION_IO_FDS_FIRST_ARGSZ, 0, &payload).unwrap();
        let RegionIoFdsReply::Complete { sub_regions } = reply else {
            panic!("{reply:?}");
        };
        for (i, s) in sub_regions.iter().enumerate() {
            assert_eq!(
                *s,
                SubRegionIoFd::IoEventFd {
                    offset: 0xbb_0090,
                    size: 4,
                    fd_index: i as u32,
                    flags: IOEVENTFD_FLAG_DATAMATCH,
                    datamatch: 0xca70_0000 + i as u64,
                }
            );
        }
    }

    /// A server that does not implement the request (vfio_user up to 0.1.6):
    /// it answers before reading the payload, then answers the payload as if
    /// it were a message header.
    #[test]
    fn test_region_io_fds_unsupported_by_server() {
        let (stream, mut peer) = UnixStream::pair().unwrap();
        let mut client = Client {
            stream,
            next_message_id: Wrapping(0),
            num_irqs: 0,
            resettable: false,
            regions: Vec::new(),
            local_irq_negotiated: false,
        };
        let fake = std::thread::spawn(move || {
            let error_reply = |peer: &mut UnixStream, raw: &[u8]| {
                let reply = Header {
                    message_id: u16::from_ne_bytes([raw[0], raw[1]]),
                    command: u16::from_ne_bytes([raw[2], raw[3]]),
                    message_size: size_of::<Header>() as u32,
                    flags: HeaderFlags::Reply as u32 | HeaderFlags::Error as u32,
                    error: 0,
                };
                peer.write_all(reply.as_slice()).unwrap();
            };
            let mut raw = [0u8; 16];
            peer.read_exact(&mut raw).unwrap();
            assert_eq!(
                u16::from_ne_bytes([raw[2], raw[3]]),
                Command::GetRegionIoFds as u16
            );
            error_reply(&mut peer, &raw);
            // The unread payload taken for a header: command = argsz >> 16 = 0.
            peer.read_exact(&mut raw).unwrap();
            assert_eq!(u16::from_ne_bytes([raw[2], raw[3]]), 0);
            error_reply(&mut peer, &raw);
            let mut get_info = DeviceGetInfo::default();
            peer.read_exact(get_info.as_mut_slice()).unwrap();
            assert_eq!(get_info.header.command, Command::DeviceGetInfo as u16);
            let reply = DeviceGetInfo {
                header: Header {
                    message_id: get_info.header.message_id,
                    command: Command::DeviceGetInfo as u16,
                    flags: HeaderFlags::Reply as u32,
                    message_size: size_of::<DeviceGetInfo>() as u32,
                    ..Default::default()
                },
                argsz: 16,
                flags: VFIO_DEVICE_FLAGS_PCI,
                ..Default::default()
            };
            peer.write_all(reply.as_slice()).unwrap();
            // The next command is the client's next request, in step.
            let mut next = Header::default();
            peer.read_exact(next.as_mut_slice()).unwrap();
            assert_eq!(next.command, Command::DeviceReset as u16);
            let reply = Header {
                message_id: next.message_id,
                command: next.command,
                flags: HeaderFlags::Reply as u32,
                message_size: size_of::<Header>() as u32,
                ..Default::default()
            };
            peer.write_all(reply.as_slice()).unwrap();
        });
        assert!(matches!(
            client.get_region_io_fds(0),
            Err(Error::ErrorReply {
                command: Command::GetRegionIoFds,
                errno: 0
            })
        ));
        client.reset().unwrap();
        fake.join().unwrap();
    }

    #[test]
    fn test_no_caps_returns_empty() {
        let region_info = vfio_region_info {
            argsz: size_of::<vfio_region_info>() as u32,
            flags: 0,
            cap_offset: 0,
            ..Default::default()
        };

        let parsed = Client::parse_region_caps(&[], &region_info).unwrap();
        assert!(parsed.is_empty());
    }
}
