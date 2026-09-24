// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The HOST driver's ABI, chosen at start-up (docs/caraxes/cutover-615.md).
//!
//! The device has two driver versions to agree with, and they are separate axes:
//!
//!   * the GUEST's, which fixes every layout the guest reads or writes (GSP
//!     messages, the internal controls it sends, the answers the device builds).
//!     That is [`crate::ver::active`], chosen at build time by the `v610` feature
//!     (decision D14; the guest accepts only the GSP firmware of its own version).
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
//! | `NV_CHANNEL_ALLOC_PARAMS` | 376 | 376 | channel alloc | [`ChannelAllocParams`], equality asserted at compile time |
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
    pub sm_order_size: usize,
    pub sm_order_entry_size: usize,
    pub sm_order_entries: usize,
    pub sm_order_num_sm: usize,
    pub sm_order_num_tpc: usize,
    /// The u16 fields of one SM entry: name and offset within the entry.
    pub sm_order_entry_fields: &'static [(&'static str, usize)],
}

macro_rules! sm_fields {
    ($t:ty; $($f:ident),* $(,)?) => {
        &[$((stringify!($f), offset_of!($t, $f))),*]
    };
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
    sm_order_size: size_of::<v610::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS>(),
    sm_order_entry_size: size_of::<Sm610>(),
    sm_order_entries: 512,
    sm_order_num_sm: offset_of!(v610::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS, numSm),
    sm_order_num_tpc: offset_of!(v610::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS, numTpc),
    sm_order_entry_fields: sm_fields!(Sm610; gpcId, localTpcId, localSmId, globalTpcId,
        virtualGpcId, migratableTpcId, ugpuId, physicalCpcId, virtualTpcId),
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
    sm_order_size: size_of::<v615::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS>(),
    sm_order_entry_size: size_of::<Sm615>(),
    sm_order_entries: 512,
    sm_order_num_sm: offset_of!(v615::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS, numSm),
    sm_order_num_tpc: offset_of!(v615::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS, numTpc),
    sm_order_entry_fields: sm_fields!(Sm615; gpcId, localTpcId, localSmId, globalTpcId,
        virtualGpcId, virtualDpcId, migratableTpcId, ugpuId, physicalCpcId, virtualTpcId),
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

    /// Translate a host `GR_GET_GLOBAL_SM_ORDER` reply into the guest's
    /// per-SM layout: every guest field is copied from the host field of the
    /// same name (a field the host lacks stays 0, one the guest lacks is
    /// dropped: 615's `virtualDpcId` for a 610 guest), then `numSm`/`numTpc`.
    /// `guest` is the guest's element (entries, then numSm and numTpc).
    pub fn sm_order_to_guest(
        &self,
        host: &[u8],
        guest: &mut [u8],
        guest_entry_size: usize,
        guest_fields: &[(&str, usize)],
        guest_num_sm: usize,
        guest_num_tpc: usize,
    ) {
        let rd16 = |b: &[u8], o: usize| [b[o], b[o + 1]];
        for sm in 0..self.sm_order_entries {
            let (h0, g0) = (sm * self.sm_order_entry_size, sm * guest_entry_size);
            for (name, g_off) in guest_fields {
                if let Some((_, h_off)) = self.sm_order_entry_fields.iter().find(|(n, _)| n == name)
                {
                    let v = rd16(host, h0 + h_off);
                    guest[g0 + g_off..g0 + g_off + 2].copy_from_slice(&v);
                }
            }
        }
        let v = rd16(host, self.sm_order_num_sm);
        guest[guest_num_sm..guest_num_sm + 2].copy_from_slice(&v);
        let v = rd16(host, self.sm_order_num_tpc);
        guest[guest_num_tpc..guest_num_tpc + 2].copy_from_slice(&v);
    }

    /// One line for the device log: what was selected and the sizes it implies.
    pub fn describe(&self) -> String {
        format!(
            "host ABI {}: TSG params {} bytes, GPU_GET_PES_INFO {} (tpcToPesMap[{}]), \
             GR_GET_GLOBAL_SM_ORDER {} (SM entry {}), channel params {}",
            self.driver(),
            self.tsg_size,
            self.pes_info_size,
            self.pes_info_map_len,
            self.sm_order_size,
            self.sm_order_entry_size,
            size_of::<ChannelAllocParams>(),
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
        assert_eq!(
            (
                l.sm_order_size,
                l.sm_order_entry_size,
                l.sm_order_num_sm,
                l.sm_order_num_tpc
            ),
            (9240, 18, 9216, 9218)
        );
        assert_eq!(l.sm_order_entry_fields.len(), 9);
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
        assert_eq!(
            (
                l.sm_order_size,
                l.sm_order_entry_size,
                l.sm_order_num_sm,
                l.sm_order_num_tpc
            ),
            (10264, 20, 10240, 10242)
        );
        assert_eq!(l.sm_order_entry_fields[5], ("virtualDpcId", 10));
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

    /// A guest element laid out as 610's internal SM order.
    fn guest_610() -> (usize, Vec<(&'static str, usize)>, usize, usize, usize) {
        (
            18,
            HOST_610.sm_order_entry_fields.to_vec(),
            9216,
            9218,
            9220,
        )
    }

    #[test]
    fn sm_order_610_host_is_a_plain_copy() {
        let (es, fields, nsm, ntpc, elem) = guest_610();
        let host: Vec<u8> = (0..HOST_610.sm_order_size)
            .map(|i| (i * 7 + 3) as u8)
            .collect();
        let mut g = vec![0u8; elem];
        HOST_610.sm_order_to_guest(&host, &mut g, es, &fields, nsm, ntpc);
        assert_eq!(&g[..], &host[..elem]);
    }

    #[test]
    fn sm_order_615_host_drops_virtual_dpc_id() {
        let (es, fields, nsm, ntpc, elem) = guest_610();
        let l = &HOST_615;
        let mut host = vec![0u8; l.sm_order_size];
        for sm in 0..512usize {
            for (i, _) in l.sm_order_entry_fields.iter().enumerate() {
                let o = sm * 20 + 2 * i;
                // field i of SM sm = sm * 16 + i, with virtualDpcId (i = 5) = 0xDDDD
                let v: u16 = if i == 5 { 0xDDDD } else { (sm * 16 + i) as u16 };
                host[o..o + 2].copy_from_slice(&v.to_le_bytes());
            }
        }
        host[10240..10242].copy_from_slice(&36u16.to_le_bytes());
        host[10242..10244].copy_from_slice(&18u16.to_le_bytes());
        let mut g = vec![0u8; elem];
        l.sm_order_to_guest(&host, &mut g, es, &fields, nsm, ntpc);
        let rd = |o: usize| u16::from_le_bytes([g[o], g[o + 1]]);
        for sm in [0usize, 1, 35, 511] {
            // guest fields 0..=4 are host 0..=4; guest 5..=8 are host 6..=9
            for gi in 0..9 {
                let hi = if gi < 5 { gi } else { gi + 1 };
                assert_eq!(
                    rd(sm * 18 + 2 * gi) as usize,
                    sm * 16 + hi,
                    "sm {sm} field {gi}"
                );
            }
        }
        assert!(!g.chunks(2).any(|c| c == [0xDD, 0xDD]));
        assert_eq!((rd(9216), rd(9218)), (36, 18));
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
