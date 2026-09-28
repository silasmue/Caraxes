// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The HOST driver's ABI, chosen at start-up (docs/caraxes/cutover-615.md).
//!
//! The device has two driver versions to agree with, and they are separate axes:
//!
//!   * the GUEST's, which fixes every layout the guest reads or writes (GSP
//!     messages, the internal controls it sends, the answers the device builds).
//!     That is [`crate::ver`], chosen at run time from what the guest shows at its
//!     GSP boot (decision D14; the guest accepts only the GSP firmware of its own
//!     version; docs/research/one-abi-binary-2026-09-28.md).
//!   * the HOST's, which fixes the layouts of the calls the device makes as an
//!     unprivileged RM client of the host kernel driver (decision D10). That is
//!     this module, chosen at run time from `/proc/driver/nvidia/version`,
//!     because the host driver is replaced underneath an unchanged guest image
//!     (2026-09-24: 610.57.04 to 615.71.09).
//!
//! What moves between the two host versions the device supports, read from the
//! generated bindings `nvrm_sys::v610` and `nvrm_sys::v615` (every struct both
//! modules define was compared by `size_of` and by field list; the tests below
//! pin each size from both modules):
//!
//! | Struct | 610.57.04 | 615.71.09 | Host call | Here |
//! |---|---:|---:|---|---|
//! | `NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS` | 20 | 28 | TSG alloc (invalidate.rs, mirror.rs) | [`HostLayouts::tsg_params`] |
//! | `NV2080_CTRL_GPU_GET_PES_INFO_PARAMS` | 56 | 76 | `GPU_GET_PES_INFO` (kgr.rs floorsweeping) | `pes_info_*` |
//! | `NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS` | 9240 | 10264 | `GR_GET_GLOBAL_SM_ORDER` (kgr.rs) | [`HostLayouts::sm_order_to_guest`] |
//! | GSS legacy pmgr 0x18 (0x2080A618) | 20000 | 20512 | forwarded for nvidia-smi (nvml.rs) | [`HostLayouts::legacy`] |
//! | GSS legacy thermal 0x36 (0x20808536) | 25552 | 24532 | forwarded for nvidia-smi (nvml.rs) | [`HostLayouts::legacy`] |
//! | GSS legacy perf 0xf1 (0x2080A0F1) | none | 9216 | forwarded for nvidia-smi (nvml.rs) | [`HostLayouts::legacy`] |
//! | `NV_CHANNEL_ALLOC_PARAMS` | 376 | 376 | channel alloc | [`ChannelAllocParams`], equality asserted at compile time |
//! | `NV2080_CTRL_FB_FLUSH_GPU_CACHE_PARAMS` `flags` bits | APERTURE 1:0 | APERTURE 2:0 | `FB_FLUSH_GPU_CACHE` (guest L2 ops, boot.rs) | [`HostLayouts::flush_gpu_cache`] |
//! | `NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO` privilege | PRIVILEGED | NON_PRIVILEGED | forwarded for libcuda 615 (cuda.rs) | [`HostLayouts::lce_shim_info_public`] |
//! | `NV2080_CTRL_PERF_BOOST_FLAGS` bits | CMD 1:0, CUDA, ASYNC, CUDA_PRIORITY | adds MULTIMEDIA 7:7 | `PERF_BOOST` (host boost, hostboost.rs) | [`HostLayouts::perf_boost_flags`] |
//!
//! The flush row is not a struct layout (the struct is 4024 bytes on both, in
//! `nvrm_sys::stable`) but a flag-word layout the bindings cannot carry: 615 widens
//! `_FLAGS_APERTURE` from 1:0 to 2:0 (adds DIRTY_MEMORY 3 and COMPTAG_MEMORY 4) and
//! moves WRITE_BACK, INVALIDATE, FLUSH_MODE and FB_FLUSH up by one bit (610
//! sdk/ctrl/ctrl2080/ctrl2080fb.h:681-696, 615 ctrl2080fb.h:682-699). A 610 word on the
//! 615 host fails: WRITE_BACK|FULL_CACHE|SYSMEM (0x15) reads as aperture 5, "Invalid
//! aperture."; INVALIDATE|FULL_CACHE|SYSMEM (0x19) reads as aperture 1, WRITE_BACK and
//! INVALIDATE, FLUSH_MODE ADDRESS_ARRAY with `addressArraySize` 0, "Invalid array size"
//! (615 src/nvidia/src/kernel/gpu/mem_sys/kern_mem_sys_ctrl.c:1393-1413, :1435-1447;
//! the same checks in 610 kern_mem_sys_ctrl.c:1483-1494, :1516-1528).
//!
//! The other rows of docs/caraxes/blackwell-gb206.md §3.4 are guest-facing only:
//! `FIFO_GET_DEVICE_INFO_TABLE` (3212/3340) is answered from the chip's table,
//! `INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER` (73760/81952) and
//! `INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS` (10688/10848) are built for the
//! guest from host public controls, `INTERNAL_GET_PCIE_P2P_CAPS` (3/4) and
//! `INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO` (1088/1072) are synthesized. None of
//! them reaches the host, so they follow the guest ABI (`src/ver`), not this one;
//! the tests record their sizes on both versions for the guest-ABI work. Every
//! other layout the device sends to the host is either in `nvrm_sys::stable`
//! (identical on every supported version by construction) or unbound and
//! unchanged between the two SDKs (`ctrl0080dma.h`, `ctrl0041.h`,
//! `ctrl2080ce.h`, `ctrla06fgpfifo.h`, `ctrlc36f.h`, `ctrl90f1.h`: no layout
//! difference; `ctrl2080event.h` changes only the RATS trace bind, unused).
//!
//! The guest's side of the SM order is the guest ABI's (`ver::layouts().sm_order`);
//! [`translate_sm_order`] moves every per-SM field by name from one [`SmOrderLayout`] to
//! the other, so the four combinations of a 610 or 615 guest on a 610 or 615 host take
//! one path (kgr.rs `global_sm_order`).
//!
//! The GSS legacy rows have no layout in either version's open headers (their numbers
//! are not defined there; matrix/catalog-610.57.04.md:384, :419), so they cannot be
//! translated field by field. The sizes are the ones each driver's own `nvidia-smi -q`
//! sends: 610.57.04 from caraxes/runs/e4-2026-09-23/nvml-it3-trace-smiq.txt:2759, :2807,
//! 615.71.09 from caraxes/runs/transport-615-2026-09-24/boot/device-events.txt:734,
//! :1150, :1166. When the guest's size is not the host's, nvml.rs answers NOT_SUPPORTED
//! instead of sending the host a buffer it would refuse (docs/caraxes/cutover-615.md (d)).
//!
//! Selection: `CARAXES_HOST_ABI=610|615` if set (an explicit override, logged
//! with the running version beside it), otherwise the running driver's version
//! string, matched EXACTLY (`nvrm_sys::DriverVersion::from_version_string`).
//! Anything else is refused before the first host call: a layout that has not
//! been measured for the running driver is not approximated.

use std::mem::{offset_of, size_of};
use std::sync::OnceLock;

use nvrm_sys::{v610, v615, DriverVersion, RmAbi, V610, V615};

/// The environment variable that overrides the host ABI.
pub const ENV: &str = "CARAXES_HOST_ABI";

/// `NV_CHANNEL_ALLOC_PARAMS` for the host. The same type serves both versions:
/// `nvrm_sys::V610` and `V615` agree on its size and on every field offset
/// (asserted below at compile time, from the generated `RmAbi` constants).
pub type ChannelAllocParams = v610::NV_CHANNEL_ALLOC_PARAMS;

macro_rules! same_on_both {
    ($($c:ident),* $(,)?) => {
        const _: () = {
            $( assert!(<V610 as RmAbi>::$c == <V615 as RmAbi>::$c); )*
        };
    };
}
same_on_both!(
    ALLOC_CHANNEL_PARAMS_SIZE,
    ALLOC_CHANNEL_PARAMS_OFF_hObjectError,
    ALLOC_CHANNEL_PARAMS_OFF_hObjectBuffer,
    ALLOC_CHANNEL_PARAMS_OFF_gpFifoOffset,
    ALLOC_CHANNEL_PARAMS_OFF_gpFifoEntries,
    ALLOC_CHANNEL_PARAMS_OFF_flags,
    ALLOC_CHANNEL_PARAMS_OFF_hContextShare,
    ALLOC_CHANNEL_PARAMS_OFF_hVASpace,
    ALLOC_CHANNEL_PARAMS_OFF_hUserdMemory,
    ALLOC_CHANNEL_PARAMS_OFF_userdOffset,
    ALLOC_CHANNEL_PARAMS_OFF_engineType,
    ALLOC_CHANNEL_PARAMS_OFF_cid,
    ALLOC_CHANNEL_PARAMS_OFF_subDeviceId,
    ALLOC_CHANNEL_PARAMS_OFF_hObjectEccError,
    ALLOC_CHANNEL_PARAMS_OFF_instanceMem,
    ALLOC_CHANNEL_PARAMS_OFF_userdMem,
    ALLOC_CHANNEL_PARAMS_OFF_ramfcMem,
    ALLOC_CHANNEL_PARAMS_OFF_mthdbufMem,
    ALLOC_CHANNEL_PARAMS_OFF_hPhysChannelGroup,
    ALLOC_CHANNEL_PARAMS_OFF_internalFlags,
    ALLOC_CHANNEL_PARAMS_OFF_errorNotifierMem,
    ALLOC_CHANNEL_PARAMS_OFF_eccErrorNotifierMem,
    ALLOC_CHANNEL_PARAMS_OFF_ProcessID,
    ALLOC_CHANNEL_PARAMS_OFF_SubProcessID,
    ALLOC_CHANNEL_PARAMS_OFF_encryptIv,
    ALLOC_CHANNEL_PARAMS_OFF_decryptIv,
    ALLOC_CHANNEL_PARAMS_OFF_hmacNonce,
    ALLOC_CHANNEL_PARAMS_OFF_tpcConfigID,
    ALLOC_CHANNEL_PARAMS_OFF_hHandleVASpace,
);
const _: () =
    assert!(size_of::<ChannelAllocParams>() == <V615 as RmAbi>::ALLOC_CHANNEL_PARAMS_SIZE as usize);

/// The host layouts that differ between the supported host versions.
#[derive(Debug)]
pub struct HostLayouts {
    pub version: DriverVersion,
    /// `NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS` (610 `nvos.h:2900-2907`; 615 adds
    /// `reserved` and `internalFlags`).
    pub tsg_size: usize,
    pub tsg_h_vaspace: usize,
    pub tsg_engine_type: usize,
    /// `NV2080_CTRL_GPU_GET_PES_INFO_PARAMS` (ctrl2080gpu.h; 615 widens
    /// `tpcToPesMap` from 10 to 15 entries).
    pub pes_info_size: usize,
    pub pes_info_num_pes: usize,
    pub pes_info_tpc_to_pes_map: usize,
    pub pes_info_map_len: usize,
    /// `NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS` (ctrl2080gr.h; 615 inserts
    /// `virtualDpcId` after `virtualGpcId` in every SM entry).
    pub sm_order: SmOrderLayout,
    /// GSS legacy controls whose params size depends on the host driver, or that only
    /// this host driver has: (command, params size, name). See [`HOST_SIZED_LEGACY`].
    pub legacy: &'static [(u32, usize, &'static str)],
    /// The `NV2080_CTRL_FB_FLUSH_GPU_CACHE_FLAGS` bits of this host driver.
    pub flush: FlushFlags,
    /// `NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO` is NON_PRIVILEGED on this host, so the
    /// device's unprivileged client may forward it (610 export flags 0x145, PRIVILEGED;
    /// 615 0x149, NON_PRIVILEGED; the layout is the same, `ver::v610::LCE_SHIM_INFO_SIZE`).
    pub lce_shim_info_public: bool,
    /// The `NV2080_CTRL_PERF_BOOST_FLAGS` bits this host defines that the device may pass
    /// on from a guest request (hostboost.rs `forwarded`): the command 1:0 on both, and
    /// MULTIMEDIA 7:7, which only 615 defines (615 ctrl2080perf.h:92-94; 610 :75-90).
    /// CUDA, ASYNC and CUDA_PRIORITY exist on both and are never passed on.
    pub perf_boost_flags: u32,
}

/// `NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE` (ctrl2080fb.h, both versions): public and
/// NON_PRIVILEGED (export flags 0x10118, g_subdevice_nvoc.c), so the device's
/// unprivileged client may issue it on its own subdevice.
pub const CMD_FB_FLUSH_GPU_CACHE: u32 = 0x2080_130E;
/// `NV2080_CTRL_FB_FLUSH_GPU_CACHE_PARAMS`: the same on every version (`nvrm_sys::stable`).
pub const FLUSH_GPU_CACHE_PARAMS_SIZE: usize =
    size_of::<nvrm_sys::stable::NV2080_CTRL_FB_FLUSH_GPU_CACHE_PARAMS>();
pub const FLUSH_GPU_CACHE_FLAGS_OFF: usize =
    offset_of!(nvrm_sys::stable::NV2080_CTRL_FB_FLUSH_GPU_CACHE_PARAMS, flags);

/// The flag values of `NV2080_CTRL_FB_FLUSH_GPU_CACHE_FLAGS` one host driver defines,
/// each already shifted to its field: `_APERTURE_SYSTEM_MEMORY`, `_WRITE_BACK_YES`,
/// `_INVALIDATE_YES`, `_FLUSH_MODE_FULL_CACHE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlushFlags {
    pub aperture_sysmem: u32,
    pub write_back: u32,
    pub invalidate: u32,
    pub full_cache: u32,
}

/// The guest's L2 operation as the host is asked to perform it (boot.rs `l2_op`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L2Flush {
    /// FLUSH_DIRTY: write back.
    WriteBack,
    /// SYSMEM_INVALIDATE: invalidate (the host RM promotes it to an evict,
    /// kmemsysCacheOp_GM200).
    Invalidate,
}

/// Every command that appears in some host's [`HostLayouts::legacy`] table. nvml.rs
/// answers these by the selected host's table: forwarded when the guest's size is the
/// host's, NOT_SUPPORTED otherwise and on a host that has no such control.
pub const HOST_SIZED_LEGACY: &[u32] = &[0x2080_A618, 0x2080_8536, 0x2080_A0F1];

/// One global-SM-order layout: the host's public `NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS`
/// or the guest's internal `NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER`. Both are
/// `globalSmId[entries]` of u16 fields, then `numSm` and `numTpc` (u16 each).
#[derive(Debug, PartialEq, Eq)]
pub struct SmOrderLayout {
    /// The whole struct (the public one ends with `grRouteInfo`).
    pub size: usize,
    pub entry_size: usize,
    pub entries: usize,
    /// The u16 fields of one SM entry: name and offset within the entry.
    pub fields: &'static [(&'static str, usize)],
    pub num_sm: usize,
    pub num_tpc: usize,
}

/// An [`SmOrderLayout`] from a generated struct and its entry type, with the entry's
/// field names listed (every field is a u16; the tests check that the list covers the
/// entry). Used here for the host's public struct and in `src/ver` for the guest's
/// internal element.
macro_rules! sm_order_layout {
    ($outer:ty, $entry:ty; $($f:ident),* $(,)?) => {
        $crate::hostabi::SmOrderLayout {
            size: ::core::mem::size_of::<$outer>(),
            entry_size: ::core::mem::size_of::<$entry>(),
            entries: ::core::mem::offset_of!($outer, numSm) / ::core::mem::size_of::<$entry>(),
            fields: &[$((stringify!($f), ::core::mem::offset_of!($entry, $f))),*],
            num_sm: ::core::mem::offset_of!($outer, numSm),
            num_tpc: ::core::mem::offset_of!($outer, numTpc),
        }
    };
}
pub(crate) use sm_order_layout;

impl SmOrderLayout {
    /// The field names of `self` that `other` lacks.
    pub fn fields_not_in(&self, other: &SmOrderLayout) -> Vec<&'static str> {
        self.fields
            .iter()
            .filter(|(n, _)| !other.fields.iter().any(|(m, _)| m == n))
            .map(|(n, _)| *n)
            .collect()
    }
}

/// Copy a global SM order from layout `from` (bytes `src`) into layout `to` (bytes
/// `dst`, at least `to.num_tpc + 2` long): every field of `to` is taken from the field
/// of the same name in `from`; a field `from` lacks stays as `dst` has it (zero), a
/// field `to` lacks is dropped. Then `numSm` and `numTpc`.
pub fn translate_sm_order(from: &SmOrderLayout, src: &[u8], to: &SmOrderLayout, dst: &mut [u8]) {
    let pairs: Vec<(usize, usize)> = to
        .fields
        .iter()
        .filter_map(|(name, t)| {
            from.fields
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, f)| (*f, *t))
        })
        .collect();
    for sm in 0..from.entries.min(to.entries) {
        let (f0, t0) = (sm * from.entry_size, sm * to.entry_size);
        for (f, t) in &pairs {
            dst[t0 + t..t0 + t + 2].copy_from_slice(&src[f0 + f..f0 + f + 2]);
        }
    }
    dst[to.num_sm..to.num_sm + 2].copy_from_slice(&src[from.num_sm..from.num_sm + 2]);
    dst[to.num_tpc..to.num_tpc + 2].copy_from_slice(&src[from.num_tpc..from.num_tpc + 2]);
}

type Sm610 = v610::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS__bindgen_ty_1;
type Sm615 = v615::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS__bindgen_ty_1;

pub static HOST_610: HostLayouts = HostLayouts {
    version: DriverVersion::V610,
    tsg_size: <V610 as RmAbi>::TSG_PARAMS_SIZE as usize,
    tsg_h_vaspace: <V610 as RmAbi>::TSG_PARAMS_OFF_hVASpace,
    tsg_engine_type: <V610 as RmAbi>::TSG_PARAMS_OFF_engineType,
    pes_info_size: size_of::<v610::NV2080_CTRL_GPU_GET_PES_INFO_PARAMS>(),
    pes_info_num_pes: offset_of!(v610::NV2080_CTRL_GPU_GET_PES_INFO_PARAMS, numPesInGpc),
    pes_info_tpc_to_pes_map: offset_of!(v610::NV2080_CTRL_GPU_GET_PES_INFO_PARAMS, tpcToPesMap),
    pes_info_map_len: 10,
    sm_order: sm_order_layout!(v610::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS, Sm610;
        gpcId, localTpcId, localSmId, globalTpcId, virtualGpcId, migratableTpcId, ugpuId,
        physicalCpcId, virtualTpcId),
    legacy: &[
        (0x2080_A618, 20000, "pmgr 0x18"),
        (0x2080_8536, 25552, "thermal 0x36"),
    ],
    // 610 sdk/ctrl/ctrl2080/ctrl2080fb.h:681-693: APERTURE 1:0, WRITE_BACK 2:2,
    // INVALIDATE 3:3, FLUSH_MODE 4:4.
    flush: FlushFlags {
        aperture_sysmem: 1,
        write_back: 1 << 2,
        invalidate: 1 << 3,
        full_cache: 1 << 4,
    },
    // 610 src/nvidia/generated/g_subdevice_nvoc.c:11827: flags 0x145 (PRIVILEGED).
    lce_shim_info_public: false,
    perf_boost_flags: crate::hostboost::FLAGS_CMD,
};

pub static HOST_615: HostLayouts = HostLayouts {
    version: DriverVersion::V615,
    tsg_size: <V615 as RmAbi>::TSG_PARAMS_SIZE as usize,
    tsg_h_vaspace: <V615 as RmAbi>::TSG_PARAMS_OFF_hVASpace,
    tsg_engine_type: <V615 as RmAbi>::TSG_PARAMS_OFF_engineType,
    pes_info_size: size_of::<v615::NV2080_CTRL_GPU_GET_PES_INFO_PARAMS>(),
    pes_info_num_pes: offset_of!(v615::NV2080_CTRL_GPU_GET_PES_INFO_PARAMS, numPesInGpc),
    pes_info_tpc_to_pes_map: offset_of!(v615::NV2080_CTRL_GPU_GET_PES_INFO_PARAMS, tpcToPesMap),
    pes_info_map_len: 15,
    sm_order: sm_order_layout!(v615::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS, Sm615;
        gpcId, localTpcId, localSmId, globalTpcId, virtualGpcId, virtualDpcId,
        migratableTpcId, ugpuId, physicalCpcId, virtualTpcId),
    legacy: &[
        (0x2080_A618, 20512, "pmgr 0x18"),
        (0x2080_8536, 24532, "thermal 0x36"),
        (0x2080_A0F1, 9216, "perf 0xf1"),
    ],
    // 615 sdk/ctrl/ctrl2080/ctrl2080fb.h:682-695: APERTURE 2:0, WRITE_BACK 3:3,
    // INVALIDATE 4:4, FLUSH_MODE 5:5.
    flush: FlushFlags {
        aperture_sysmem: 1,
        write_back: 1 << 3,
        invalidate: 1 << 4,
        full_cache: 1 << 5,
    },
    // 615 src/nvidia/generated/g_subdevice_nvoc.c:12008: flags 0x149 (NON_PRIVILEGED).
    lce_shim_info_public: true,
    perf_boost_flags: crate::hostboost::FLAGS_CMD | crate::hostboost::FLAG_MULTIMEDIA,
};

// The map lengths are array lengths the bindings cannot give as constants; pin
// them to the struct sizes they imply.
const _: () = assert!(HOST_610_PES_END == size_of::<v610::NV2080_CTRL_GPU_GET_PES_INFO_PARAMS>());
const _: () = assert!(HOST_615_PES_END == size_of::<v615::NV2080_CTRL_GPU_GET_PES_INFO_PARAMS>());
const HOST_610_PES_END: usize =
    offset_of!(v610::NV2080_CTRL_GPU_GET_PES_INFO_PARAMS, tpcToPesMap) + 4 * 10;
const HOST_615_PES_END: usize =
    offset_of!(v615::NV2080_CTRL_GPU_GET_PES_INFO_PARAMS, tpcToPesMap) + 4 * 15;

fn put32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

impl HostLayouts {
    /// The driver version string these layouts belong to.
    pub fn driver(&self) -> &'static str {
        self.version.as_str()
    }

    /// `NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS` for the host: `hVASpace` and
    /// `engineType` set, everything else zero (615's `reserved` and
    /// `internalFlags` included, as the kernel's own callers leave them).
    pub fn tsg_params(&self, h_vaspace: u32, engine_type: u32) -> Vec<u8> {
        let mut b = vec![0u8; self.tsg_size];
        put32(&mut b, self.tsg_h_vaspace, h_vaspace);
        put32(&mut b, self.tsg_engine_type, engine_type);
        b
    }

    /// Translate a host `GR_GET_GLOBAL_SM_ORDER` reply (this host's layout) into the
    /// guest's internal element (`guest`, laid out as `guest_layout`, the guest ABI's):
    /// field by name, see [`translate_sm_order`]. 615's `virtualDpcId` is dropped for a
    /// 610 guest and stays 0 for a 615 guest on a 610 host (no DPC before Blackwell).
    pub fn sm_order_to_guest(&self, host: &[u8], guest: &mut [u8], guest_layout: &SmOrderLayout) {
        translate_sm_order(&self.sm_order, host, guest_layout, guest);
    }

    /// The params size this host driver expects for a host-sized GSS legacy control
    /// ([`HOST_SIZED_LEGACY`]), and its name; `None` if this host has no such control.
    pub fn legacy_size(&self, cmd: u32) -> Option<(usize, &'static str)> {
        self.legacy
            .iter()
            .find(|(c, _, _)| *c == cmd)
            .map(|(_, n, name)| (*n, *name))
    }

    /// The `flags` word of `FB_FLUSH_GPU_CACHE` for `op` on this host: full cache,
    /// system-memory aperture, the operation's bit; FB_FLUSH NO, as the device sent
    /// it on the 610 host. `addressArraySize`, `addressAlign` and
    /// `memBlockSizeBytes` stay 0, which FULL_CACHE ignores.
    pub fn flush_gpu_cache_flags(&self, op: L2Flush) -> u32 {
        let f = &self.flush;
        f.aperture_sysmem
            | f.full_cache
            | match op {
                L2Flush::WriteBack => f.write_back,
                L2Flush::Invalidate => f.invalidate,
            }
    }

    /// The whole `NV2080_CTRL_FB_FLUSH_GPU_CACHE_PARAMS` block for `op`: zero but for
    /// [`Self::flush_gpu_cache_flags`].
    pub fn flush_gpu_cache(&self, op: L2Flush) -> Vec<u8> {
        let mut b = vec![0u8; FLUSH_GPU_CACHE_PARAMS_SIZE];
        put32(&mut b, FLUSH_GPU_CACHE_FLAGS_OFF, self.flush_gpu_cache_flags(op));
        b
    }

    /// One line for the device log: what was selected and the sizes it implies.
    pub fn describe(&self) -> String {
        format!(
            "host ABI {}: TSG params {} bytes, GPU_GET_PES_INFO {} (tpcToPesMap[{}]), \
             GR_GET_GLOBAL_SM_ORDER {} (SM entry {}), channel params {}, GSS legacy [{}]",
            self.driver(),
            self.tsg_size,
            self.pes_info_size,
            self.pes_info_map_len,
            self.sm_order.size,
            self.sm_order.entry_size,
            size_of::<ChannelAllocParams>(),
            self.legacy
                .iter()
                .map(|(_, n, name)| format!("{name} {n}"))
                .collect::<Vec<_>>()
                .join(", "),
        )
    }
}

/// The layouts for a supported driver version, `None` for any other.
pub fn for_version(v: DriverVersion) -> Option<&'static HostLayouts> {
    match v {
        DriverVersion::V610 => Some(&HOST_610),
        DriverVersion::V615 => Some(&HOST_615),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

/// What start-up chose, and why.
#[derive(Debug)]
pub struct Selection {
    pub layouts: &'static HostLayouts,
    /// How it was chosen, for the log.
    pub how: String,
}

fn refusal(what: &str) -> String {
    format!(
        "{what}; this device carries host layouts for 610.57.04 and 615.71.09 only. \
         A version with no measured layout is refused rather than approximated \
         (crates/nvrm-sys/src/version.rs `detect`). Set {ENV}=610 or {ENV}=615 only \
         if you know the running driver has that layout."
    )
}

/// Choose the host layouts from the override (`CARAXES_HOST_ABI`) and the
/// running driver's version. Pure, for the tests; [`select`] feeds it.
pub fn resolve(
    override_: Option<&str>,
    running: std::io::Result<String>,
) -> Result<Selection, String> {
    match override_ {
        Some(o) => {
            let layouts = match o.trim() {
                "610" => &HOST_610,
                "615" => &HOST_615,
                other => {
                    return Err(format!(
                        "{ENV}={other:?} is not a host ABI: expected 610 or 615"
                    ))
                }
            };
            let how = match &running {
                Ok(r) if r == layouts.driver() => {
                    format!("{ENV}={o} (running driver {r})")
                }
                Ok(r) => format!(
                    "{ENV}={o} OVERRIDES the running driver {r}: host calls use {} layouts",
                    layouts.driver()
                ),
                Err(e) => format!("{ENV}={o} (running driver not readable: {e})"),
            };
            Ok(Selection { layouts, how })
        }
        None => {
            let r = running.map_err(|e| {
                refusal(&format!(
                    "host driver version not readable from /proc/driver/nvidia/version ({e})"
                ))
            })?;
            let layouts = DriverVersion::from_version_string(&r)
                .and_then(for_version)
                .ok_or_else(|| refusal(&format!("host driver {r} is not supported")))?;
            Ok(Selection {
                layouts,
                how: format!("running driver {r} (/proc/driver/nvidia/version)"),
            })
        }
    }
}

static SELECTED: OnceLock<&'static HostLayouts> = OnceLock::new();

/// Read the override and the running version, choose, install. Called once at
/// start-up, before the first host RM client is opened; an error ends start-up.
pub fn select() -> Result<Selection, String> {
    let o = std::env::var(ENV).ok();
    let sel = resolve(o.as_deref(), nvrm_sys::running_driver_version())?;
    install(sel.layouts)?;
    Ok(sel)
}

/// Install a selection (once; a second, different one is an error).
pub fn install(l: &'static HostLayouts) -> Result<(), String> {
    let got = *SELECTED.get_or_init(|| l);
    if std::ptr::eq(got, l) {
        Ok(())
    } else {
        Err(format!(
            "host ABI already selected as {}, not {}",
            got.driver(),
            l.driver()
        ))
    }
}

/// The selected layouts, or an error if start-up has not selected any. The host
/// RM clients (hostrm.rs, hostmem.rs) open only after this succeeds, which is
/// what replaces `nvrm_client::RmClient::new`'s build-version assertion.
pub fn selected() -> Result<&'static HostLayouts, String> {
    SELECTED.get().copied().ok_or_else(|| {
        format!("no host ABI selected ({ENV} or the running driver); refusing to open the host RM")
    })
}

/// The layouts host calls use. Before any selection (unit tests, and a device
/// without a host client, which makes no host calls) this is 610.57.04, the
/// layouts the device used before the selection existed.
pub fn current() -> &'static HostLayouts {
    SELECTED.get().copied().unwrap_or(&HOST_610)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io_err() -> std::io::Result<String> {
        Err(std::io::Error::other("no such file"))
    }

    #[test]
    fn host_610_sizes_are_the_generated_ones() {
        let l = &HOST_610;
        assert_eq!(l.driver(), "610.57.04");
        assert_eq!(
            size_of::<v610::NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS>(),
            20
        );
        assert_eq!(
            (l.tsg_size, l.tsg_h_vaspace, l.tsg_engine_type),
            (20, 8, 12)
        );
        assert_eq!(size_of::<v610::NV2080_CTRL_GPU_GET_PES_INFO_PARAMS>(), 56);
        assert_eq!(
            (
                l.pes_info_size,
                l.pes_info_num_pes,
                l.pes_info_tpc_to_pes_map,
                l.pes_info_map_len
            ),
            (56, 4, 16, 10)
        );
        assert_eq!(
            size_of::<v610::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS>(),
            9240
        );
        let o = &l.sm_order;
        assert_eq!(
            (o.size, o.entry_size, o.entries, o.num_sm, o.num_tpc),
            (9240, 18, 512, 9216, 9218)
        );
        assert_eq!(o.fields.len() * 2, o.entry_size, "every u16 of the entry is named");
        assert_eq!(
            offset_of!(v610::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS, globalSmId),
            0
        );
        assert_eq!(l.legacy_size(0x2080_A618), Some((20000, "pmgr 0x18")));
        assert_eq!(l.legacy_size(0x2080_8536), Some((25552, "thermal 0x36")));
        assert_eq!(l.legacy_size(0x2080_A0F1), None);
        assert_eq!(size_of::<ChannelAllocParams>(), 376);
    }

    #[test]
    fn host_615_sizes_are_the_generated_ones() {
        let l = &HOST_615;
        assert_eq!(l.driver(), "615.71.09");
        assert_eq!(
            size_of::<v615::NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS>(),
            28
        );
        assert_eq!(
            offset_of!(v615::NV_CHANNEL_GROUP_ALLOCATION_PARAMETERS, internalFlags),
            24
        );
        assert_eq!(
            (l.tsg_size, l.tsg_h_vaspace, l.tsg_engine_type),
            (28, 8, 12)
        );
        assert_eq!(size_of::<v615::NV2080_CTRL_GPU_GET_PES_INFO_PARAMS>(), 76);
        assert_eq!(
            (
                l.pes_info_size,
                l.pes_info_num_pes,
                l.pes_info_tpc_to_pes_map,
                l.pes_info_map_len
            ),
            (76, 4, 16, 15)
        );
        assert_eq!(
            size_of::<v615::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS>(),
            10264
        );
        let o = &l.sm_order;
        assert_eq!(
            (o.size, o.entry_size, o.entries, o.num_sm, o.num_tpc),
            (10264, 20, 512, 10240, 10242)
        );
        assert_eq!(o.fields.len() * 2, o.entry_size, "every u16 of the entry is named");
        assert_eq!(o.fields[5], ("virtualDpcId", 10));
        assert_eq!(
            offset_of!(v615::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS, globalSmId),
            0
        );
        assert_eq!(l.legacy_size(0x2080_A618), Some((20512, "pmgr 0x18")));
        assert_eq!(l.legacy_size(0x2080_8536), Some((24532, "thermal 0x36")));
        assert_eq!(l.legacy_size(0x2080_A0F1), Some((9216, "perf 0xf1")));
        assert_eq!(size_of::<v615::NV_CHANNEL_ALLOC_PARAMS>(), 376);
    }

    /// The §3.4 rows that do not reach the host, recorded on both versions: they
    /// follow the guest ABI (src/ver), and a v615 guest module will need these.
    #[test]
    fn guest_facing_rows_differ_as_documented() {
        let pairs: [(&str, usize, usize); 7] = [
            (
                "FIFO_GET_DEVICE_INFO_TABLE",
                size_of::<v610::NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS>(),
                size_of::<v615::NV2080_CTRL_FIFO_GET_DEVICE_INFO_TABLE_PARAMS>(),
            ),
            (
                "FIFO_DEVICE_ENTRY",
                size_of::<v610::NV2080_CTRL_FIFO_DEVICE_ENTRY>(),
                size_of::<v615::NV2080_CTRL_FIFO_DEVICE_ENTRY>(),
            ),
            (
                "INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER",
                size_of::<v610::NV2080_CTRL_INTERNAL_STATIC_GR_GET_GLOBAL_SM_ORDER_PARAMS>(),
                size_of::<v615::NV2080_CTRL_INTERNAL_STATIC_GR_GET_GLOBAL_SM_ORDER_PARAMS>(),
            ),
            (
                "INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS",
                size_of::<v610::NV2080_CTRL_INTERNAL_STATIC_GR_GET_FLOORSWEEPING_MASKS_PARAMS>(),
                size_of::<v615::NV2080_CTRL_INTERNAL_STATIC_GR_GET_FLOORSWEEPING_MASKS_PARAMS>(),
            ),
            (
                "INTERNAL_GET_PCIE_P2P_CAPS",
                size_of::<v610::NV2080_CTRL_INTERNAL_GET_PCIE_P2P_CAPS_PARAMS>(),
                size_of::<v615::NV2080_CTRL_INTERNAL_GET_PCIE_P2P_CAPS_PARAMS>(),
            ),
            (
                "INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO",
                size_of::<v610::NV2080_CTRL_INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO_PARAMS>(),
                size_of::<v615::NV2080_CTRL_INTERNAL_NVLINK_GET_NVLINK_DEVICE_INFO_PARAMS>(),
            ),
            (
                "INTERNAL_STATIC_GR_GLOBAL_SM_ORDER (element)",
                size_of::<v610::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER>(),
                size_of::<v615::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER>(),
            ),
        ];
        let want = [
            (3212, 3340),
            (100, 104),
            (73760, 81952),
            (10688, 10848),
            (3, 4),
            (1088, 1072),
            (9220, 10244),
        ];
        for ((name, a, b), (wa, wb)) in pairs.iter().zip(want) {
            assert_eq!((*a, *b), (wa, wb), "{name}");
        }
    }

    /// The layouts the device forwards to the host as the guest sent them
    /// (nvml.rs, ctrl.rs `forward_or`) must be the same on both host versions,
    /// or forwarding a 610 guest's bytes to a 615 host would be wrong.
    #[test]
    fn forwarded_as_is_layouts_are_equal_on_both() {
        assert_eq!(
            size_of::<v610::NV2080_CTRL_GPU_GET_INFO_V2_PARAMS>(),
            size_of::<v615::NV2080_CTRL_GPU_GET_INFO_V2_PARAMS>()
        );
        assert_eq!(
            size_of::<v610::NV2080_CTRL_NVLINK_GET_PLATFORM_INFO_PARAMS>(),
            size_of::<v615::NV2080_CTRL_NVLINK_GET_PLATFORM_INFO_PARAMS>()
        );
        assert_eq!(
            size_of::<v610::NV2080_CTRL_GR_GET_INFO_V2_PARAMS>(),
            size_of::<v615::NV2080_CTRL_GR_GET_INFO_V2_PARAMS>()
        );
        assert_eq!(
            size_of::<v610::NV2080_CTRL_CMD_FB_GET_FB_REGION_INFO_PARAMS>(),
            size_of::<v615::NV2080_CTRL_CMD_FB_GET_FB_REGION_INFO_PARAMS>()
        );
        // The 26 F1 rows of gsp-plan/b-host-forwardable.md: 19 params structs are in
        // `nvrm_sys::stable` (generated once because they are identical in every version),
        // the rest are not bound (CLK_PMUMON samples, PERF level info V2, power state, rated TDP,
        // FLCN DMEM usage, LIBOS heap stats: sizes measured with cc against both trees,
        // equal, plan §2: 6); the one bound per version is compared here.
        assert_eq!(
            size_of::<v610::NV2080_CTRL_CMD_GPU_IS_RESET_COUPLED_PARAMS>(),
            size_of::<v615::NV2080_CTRL_CMD_GPU_IS_RESET_COUPLED_PARAMS>()
        );
    }

    #[test]
    fn tsg_params_follow_the_selection() {
        let a = HOST_610.tsg_params(0x20, 0x0B);
        assert_eq!(a.len(), 20);
        assert_eq!(&a[8..12], &0x20u32.to_le_bytes());
        assert_eq!(&a[12..16], &0x0Bu32.to_le_bytes());
        let b = HOST_615.tsg_params(0x20, 0x0B);
        assert_eq!(b.len(), 28);
        assert_eq!(&b[..20], &a[..]);
        assert_eq!(&b[20..], &[0u8; 8]);
    }

    /// Every host-sized legacy command is in [`HOST_SIZED_LEGACY`], and every entry of
    /// that list is in some host's table.
    #[test]
    fn host_sized_legacy_is_the_union_of_the_tables() {
        for l in [&HOST_610, &HOST_615] {
            for (c, _, _) in l.legacy {
                assert!(HOST_SIZED_LEGACY.contains(c), "{c:#x}");
            }
        }
        for c in HOST_SIZED_LEGACY {
            assert!(HOST_610.legacy_size(*c).is_some() || HOST_615.legacy_size(*c).is_some());
        }
    }

    /// Host bytes in which every field of every SM names itself: SM `sm`, field `name`
    /// holds [`tag`]; numSm 36, numTpc 18.
    fn tagged(l: &SmOrderLayout) -> Vec<u8> {
        let mut b = vec![0u8; l.size];
        for sm in 0..l.entries {
            for (name, off) in l.fields {
                let o = sm * l.entry_size + off;
                b[o..o + 2].copy_from_slice(&tag(sm, name).to_le_bytes());
            }
        }
        b[l.num_sm..l.num_sm + 2].copy_from_slice(&36u16.to_le_bytes());
        b[l.num_tpc..l.num_tpc + 2].copy_from_slice(&18u16.to_le_bytes());
        b
    }

    /// A value that names its SM and field, the same in either layout: the SM in the
    /// high bits, the field's position in 615's entry (the superset) in the low four.
    pub(crate) fn tag(sm: usize, name: &str) -> u16 {
        let i = HOST_615.sm_order.fields.iter().position(|(n, _)| *n == name).unwrap();
        ((sm as u16) << 4) | (i as u16 + 1)
    }

    /// The four combinations, built from the generated modules' sizes and field
    /// offsets: every field the guest has holds the host's value of the same name,
    /// `virtualDpcId` is 0 where only the guest has it, and nothing else is written.
    #[test]
    fn sm_order_all_four_combinations() {
        use guest_layouts::{GUEST_SM_ORDER_610 as G610, GUEST_SM_ORDER_615 as G615};
        assert_eq!((G610.size, G610.entry_size, G610.num_sm, G610.num_tpc), (9220, 18, 9216, 9218));
        assert_eq!((G615.size, G615.entry_size, G615.num_sm, G615.num_tpc), (10244, 20, 10240, 10242));
        for g in [&G610, &G615] {
            assert_eq!(g.entries, 512);
            assert_eq!(g.fields.len() * 2, g.entry_size, "every u16 of the entry is named");
        }
        for (h, g) in [(&HOST_610, &G610), (&HOST_615, &G610), (&HOST_610, &G615), (&HOST_615, &G615)] {
            let what = format!("host {} entry {}, guest entry {}", h.driver(), h.sm_order.entry_size, g.entry_size);
            let host = tagged(&h.sm_order);
            let mut out = vec![0u8; g.size];
            h.sm_order_to_guest(&host, &mut out, g);
            let rd = |o: usize| u16::from_le_bytes([out[o], out[o + 1]]);
            for sm in [0usize, 1, 35, 511] {
                for (name, off) in g.fields {
                    let want = if h.sm_order.fields.iter().any(|(n, _)| n == name) {
                        tag(sm, name)
                    } else {
                        0
                    };
                    assert_eq!(rd(sm * g.entry_size + off), want, "{what}: sm {sm} {name}");
                }
            }
            assert_eq!((rd(g.num_sm), rd(g.num_tpc)), (36, 18), "{what}");
            // Same layout on both sides: the translation is a plain copy.
            if h.sm_order.entry_size == g.entry_size {
                assert_eq!(&out[..], &host[..g.size], "{what}");
            }
        }
        assert_eq!(HOST_615.sm_order.fields_not_in(&G610), ["virtualDpcId"]);
        assert_eq!(G615.fields_not_in(&HOST_610.sm_order), ["virtualDpcId"]);
        assert!(HOST_610.sm_order.fields_not_in(&G615).is_empty());
    }

    /// The start-up line names the legacy sizes (docs/caraxes/cutover-615.md §0).
    /// Decode a flush `flags` word as `kmemsysFlushGpuCache_IMPL` does on 615 (615
    /// kern_mem_sys_ctrl.c:1375-1466): (aperture, write back, invalidate, full cache).
    fn decode_615(f: u32) -> (u32, bool, bool, bool) {
        (f & 7, f >> 3 & 1 == 1, f >> 4 & 1 == 1, f >> 5 & 1 == 1)
    }

    /// The same on 610 (610 kern_mem_sys_ctrl.c:1464-1547).
    fn decode_610(f: u32) -> (u32, bool, bool, bool) {
        (f & 3, f >> 2 & 1 == 1, f >> 3 & 1 == 1, f >> 4 & 1 == 1)
    }

    /// FB_FLUSH_GPU_CACHE per host: the parameter block is the same 4024 bytes on both
    /// (bindings v610 and v615), the flags word is the host's own. The 610 words sent to
    /// a 615 host are the two refusals of caraxes/runs/cutover-615-2026-09-24.
    #[test]
    fn flush_gpu_cache_per_host() {
        use v610::NV2080_CTRL_FB_FLUSH_GPU_CACHE_PARAMS as P610;
        use v615::NV2080_CTRL_FB_FLUSH_GPU_CACHE_PARAMS as P615;
        for (size, flags, count) in [
            (size_of::<P610>(), offset_of!(P610, flags), offset_of!(P610, addressArraySize)),
            (size_of::<P615>(), offset_of!(P615, flags), offset_of!(P615, addressArraySize)),
        ] {
            assert_eq!((size, flags, count), (4024, 4016, 4000));
        }
        assert_eq!(FLUSH_GPU_CACHE_PARAMS_SIZE, 4024);
        assert_eq!(FLUSH_GPU_CACHE_FLAGS_OFF, 4016);

        let words = |l: &HostLayouts| {
            (
                l.flush_gpu_cache_flags(L2Flush::WriteBack),
                l.flush_gpu_cache_flags(L2Flush::Invalidate),
            )
        };
        assert_eq!(words(&HOST_610), (0x15, 0x19));
        assert_eq!(words(&HOST_615), (0x29, 0x31));

        // Each host reads its own word as sysmem, full cache, the one operation.
        let (wb, inv) = words(&HOST_610);
        assert_eq!(decode_610(wb), (1, true, false, true));
        assert_eq!(decode_610(inv), (1, false, true, true));
        let (wb, inv) = words(&HOST_615);
        assert_eq!(decode_615(wb), (1, true, false, true));
        assert_eq!(decode_615(inv), (1, false, true, true));
        // The 610 words on 615: aperture 5 ("Invalid aperture."), and an evict in
        // ADDRESS_ARRAY mode with no addresses ("Invalid array size (0x0)").
        let (wb, inv) = words(&HOST_610);
        assert_eq!(decode_615(wb).0, 5);
        assert_eq!(decode_615(inv), (1, true, true, false));

        // The block: zero except the flags word (addressArraySize 0, FULL_CACHE).
        for l in [&HOST_610, &HOST_615] {
            for op in [L2Flush::WriteBack, L2Flush::Invalidate] {
                let b = l.flush_gpu_cache(op);
                assert_eq!(b.len(), 4024);
                assert_eq!(
                    u32::from_le_bytes(b[4016..4020].try_into().unwrap()),
                    l.flush_gpu_cache_flags(op)
                );
                assert!(b[..4016].iter().chain(&b[4020..]).all(|&x| x == 0));
            }
        }
    }

    #[test]
    fn describe_names_the_sizes() {
        assert_eq!(
            HOST_615.describe(),
            "host ABI 615.71.09: TSG params 28 bytes, GPU_GET_PES_INFO 76 (tpcToPesMap[15]), \
             GR_GET_GLOBAL_SM_ORDER 10264 (SM entry 20), channel params 376, GSS legacy \
             [pmgr 0x18 20512, thermal 0x36 24532, perf 0xf1 9216]"
        );
        assert!(HOST_610.describe().ends_with("GSS legacy [pmgr 0x18 20000, thermal 0x36 25552]"));
    }

    #[test]
    fn resolve_by_running_version() {
        let s = resolve(None, Ok("610.57.04".into())).unwrap();
        assert_eq!(s.layouts.version, DriverVersion::V610);
        let s = resolve(None, Ok("615.71.09".into())).unwrap();
        assert_eq!(s.layouts.version, DriverVersion::V615);
        assert!(s.how.contains("615.71.09"));
    }

    #[test]
    fn resolve_refuses_other_versions() {
        for v in [
            "615.71.10",
            "610.57.05",
            "595.99.02",
            "580.178.04",
            "620.1.2",
            "",
        ] {
            let e = resolve(None, Ok(v.into())).unwrap_err();
            assert!(e.contains("not supported") && e.contains(ENV), "{v}: {e}");
        }
        let e = resolve(None, io_err()).unwrap_err();
        assert!(e.contains("not readable"), "{e}");
    }

    #[test]
    fn resolve_override() {
        let s = resolve(Some("615"), Ok("610.57.04".into())).unwrap();
        assert_eq!(s.layouts.version, DriverVersion::V615);
        assert!(s.how.contains("OVERRIDES"), "{}", s.how);
        let s = resolve(Some("610"), Ok("610.57.04".into())).unwrap();
        assert_eq!(s.layouts.version, DriverVersion::V610);
        assert!(!s.how.contains("OVERRIDES"));
        let s = resolve(Some("615"), io_err()).unwrap();
        assert_eq!(s.layouts.version, DriverVersion::V615);
        for bad in ["595", "615.71.09", "v615", ""] {
            assert!(resolve(Some(bad), Ok("615.71.09".into())).is_err(), "{bad}");
        }
    }

    #[test]
    fn current_defaults_to_610() {
        // No test installs a selection, so the default is what every other test sees.
        assert_eq!(current().version, DriverVersion::V610);
    }
}

/// The guest's internal SM-order element of both guest ABIs, from the generated modules,
/// for the tests of all four guest and host combinations (`ver::L610` and `ver::L615`
/// carry the same, which the tests compare with the ones here).
#[cfg(test)]
pub(crate) mod guest_layouts {
    use super::SmOrderLayout;
    use nvrm_sys::{v610, v615};

    pub static GUEST_SM_ORDER_610: SmOrderLayout = sm_order_layout!(
        v610::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER,
        v610::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER__bindgen_ty_1;
        gpcId, localTpcId, localSmId, globalTpcId, virtualGpcId, migratableTpcId, ugpuId,
        physicalCpcId, virtualTpcId
    );
    pub static GUEST_SM_ORDER_615: SmOrderLayout = sm_order_layout!(
        v615::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER,
        v615::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER__bindgen_ty_1;
        gpcId, localTpcId, localSmId, globalTpcId, virtualGpcId, virtualDpcId,
        migratableTpcId, ugpuId, physicalCpcId, virtualTpcId
    );
}
