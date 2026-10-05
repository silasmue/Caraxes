// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! KernelGraphics static info (docs/caraxes/phase5-rpcs.md rows 39-52) and the
//! split-VAS PDE copy (row 35): the `NV2080_CTRL_CMD_INTERNAL_STATIC_KGR_*`
//! controls that `kgraphicsLoadStaticInfo_KERNEL` sends on its temporary subdevice
//! (src/nvidia/src/kernel/gpu/gr/kernel_graphics.c:1160-1580), plus the teardown
//! control `INTERNAL_GR_GET_FECS_TRACE_HW_ENABLE`.
//!
//! Paths below are relative to `vendor/open-gpu-kernel-modules` (610.57.04);
//! `sdk/` is `src/common/sdk/nvidia/inc`, `kgr.c` is
//! `src/nvidia/src/kernel/gpu/gr/kernel_graphics.c`.
//!
//! Every internal params struct is an array over `NV2080_CTRL_INTERNAL_GR_MAX_ENGINES`
//! = 8 GR engines (sdk/ctrl/ctrl2080/ctrl2080internal.h:190); the guest reads index
//! `grIdx` = 0 (no MIG on TU106), so only element 0 is filled. The public host
//! controls answer for one engine; with a zeroed `grRouteInfo` (flags TYPE_NONE,
//! sdk/ctrl/ctrl0080/ctrl0080gr.h:36-39) the host routes to its GR0. A `grRouteInfo`
//! prefix or suffix in the public struct is routing input, not data, so a public
//! struct "matches" when its data fields are the internal element's fields in the
//! same order and width.
//!
//! Every public control used here is flagged NON_PRIVILEGED (0x8) in
//! src/nvidia/generated/g_subdevice_nvoc.c (GR_GET_ZCULL_INFO :9264, GR_GET_ROP_INFO
//! :9414, GR_GET_GLOBAL_SM_ORDER :9474, GR_GET_CAPS_V2 :9579, GR_GET_INFO_V2 :9594,
//! GR_GET_ENGINE_CONTEXT_PROPERTIES :9654, GR_GET_SM_ISSUE_RATE_MODIFIER :9669,
//! GR_GET_SM_ISSUE_RATE_MODIFIER_V2 :9834; GR_GET_SM_ISSUE_THROTTLE_CTRL :9849 is no
//! longer forwarded, see its handler) and
//! is served on the host from the host kernel's own copy of the same internal
//! structs (kgr.c:2979-3012, 2800-2854, 3141-3195, 3203-3263, 3281-3360, 3381-3455,
//! 3860-3892, 3960-3987, 4060-4102), which is why the mapping back is lossless.
//!
//! # Floorsweeping masks (row 41): milestone-1 shortcut and the later assembly
//!
//! For milestone 1 the reply is deliberately all zeros with NV_OK: gpcMask 0 makes
//! `_kgraphicsPostSchedulingEnableHandler` (kgr.c:467) return before the golden-image channel
//! and the Bug 4208224 WAR channel (kgr.c:487-489, 510-514), which the device cannot
//! run yet. It also leaves GR unusable for clients. The host-derived answer would be
//! assembled per element 0 of NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS
//! (ctrl2080internal.h:300-326) as:
//! - `gpcMask` = host `GR_GET_GPC_MASK` 0x2080122a `.gpcMask` (sdk/ctrl/ctrl2080/ctrl2080gr.h:1452-1455);
//!   `physGpcMask` = the same, because `GR_GET_PHYS_GPC_MASK` 0x20801232 is PRIVILEGED;
//! - for each set bit g: `tpcMask\[g\]` = `GR_GET_TPC_MASK` 0x2080122b {gpcId g} `.tpcMask`
//!   (ctrl2080gr.h:1480-1484), `tpcCount[logical index]` = `GR_GET_NUM_TPCS_FOR_GPC`
//!   0x20801234 `.numTpcs` (:1820-1823), `zcullMask\[g\]` = `GR_GET_ZCULL_MASK` 0x20801237
//!   `.zcullMask` (:1900-1903), `numPesPerGpc\[g\]` = `GPU_GET_PES_INFO` 0x20800168
//!   `.numPesInGpc` (sdk/ctrl/ctrl2080/ctrl2080gpu.h:2215-2221), whose `tpcToPesMap\[10\]`
//!   fills `tpcToPesMap`;
//! - `physGfxGpcMask`, `numGfxTpc` = `GR_GET_GFX_GPC_AND_TPC_INFO` 0x20801239 (ctrl2080gr.h:1975-1979);
//! - `mmuPerGpc` has no public source and stays 0 (the kernel only copies it, vgpu).
//!
//! Milestone 1 answered all zeros; E5c (CUDA) implements the assembly in
//! `floorsweeping`, and with a non-zero gpcMask the guest now builds its golden
//! context and runs the Bug 4208224 WAR at module load (phase5-rpcs.md rows 56-78).

use crate::chip::ChipData;
use crate::ctrl::{Answer, HostRm};
// The control numbers and parameter layouts every guest ABI shares, computed from the
// typed bindings (src/ver/common.rs); the tests below pin them.
pub use crate::ver::common::{
    CAPS_ELEM, CAPS_PARAMS, CMD_GR_GET_CAPS_V2, CMD_GR_GET_ENGINE_CONTEXT_PROPERTIES,
    CMD_GR_GET_GLOBAL_SM_ORDER, CMD_GR_GET_INFO_V2, CMD_GR_GET_ROP_INFO,
    CMD_GR_GET_SM_ISSUE_RATE_MODIFIER, CMD_GR_GET_SM_ISSUE_RATE_MODIFIER_V2,
    CMD_GR_GET_SM_ISSUE_THROTTLE_CTRL, CMD_GR_GET_ZCULL_INFO,
    CMD_INTERNAL_GMMU_COPY_RESERVED_SPLIT_GVASPACE_PDES_TO_SERVER,
    CMD_INTERNAL_GR_GET_FECS_TRACE_HW_ENABLE, CMD_INTERNAL_STATIC_KGR_GET_CAPS,
    CMD_INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO, CMD_INTERNAL_STATIC_KGR_GET_FECS_RECORD_SIZE,
    CMD_INTERNAL_STATIC_KGR_GET_FECS_TRACE_DEFINES,
    CMD_INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS, CMD_INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER,
    CMD_INTERNAL_STATIC_KGR_GET_INFO, CMD_INTERNAL_STATIC_KGR_GET_PDB_PROPERTIES,
    CMD_INTERNAL_STATIC_KGR_GET_PPC_MASKS, CMD_INTERNAL_STATIC_KGR_GET_ROP_INFO,
    CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_RATE_MODIFIER,
    CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_RATE_MODIFIER_V2,
    CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_THROTTLE_CTRL, CMD_INTERNAL_STATIC_KGR_GET_ZCULL_INFO,
    CTX_ELEM, CTX_ENGINE_COUNT, CTX_ENGINE_FECS_EVENT, CTX_PARAMS, FECS_EVENT_RECORD_SIZE,
    FECS_HW_ENABLE_BENABLE, FECS_HW_ENABLE_PARAMS, FECS_RECORD_SIZE_PARAMS, FECS_TRACE_PARAMS,
    GR_INFO_COUNT, GR_MAX_ENGINES, INFO_ELEM, INFO_PARAMS, LIST_ELEM,
    LIST_PARAMS, PDB_PARAMS, PPC_PARAMS, PUB_CAPS_V2, PUB_CTX_PROPS, PUB_CTX_PROPS_ALIGNMENT,
    PUB_CTX_PROPS_ENGINE_ID, PUB_CTX_PROPS_SIZE, PUB_INFO_V2, PUB_INFO_V2_LIST, PUB_SMIR,
    PUB_SMIR_DATA, ROP_ELEM, ROP_PARAMS, SMIR_ELEM, SMIR_PARAMS, ZCULL_ELEM, ZCULL_PARAMS,
};
use crate::hostabi::{HostLayouts, SmOrderLayout};
use crate::ver::common::{self as v, NV20_SUBDEVICE_0, NV_ERR_NOT_SUPPORTED, NV_OK};
// The floorsweeping masks and the SM order are laid out as the guest ABI in force has them
// (`ver::layouts()`: 1336 or 1356 bytes per engine, nine or ten u16 per SM).

/// The host RM as the handlers see it (the device's own boxed client).
type Host<'a> = Option<&'a mut (dyn HostRm + 'static)>;

const NV_U32_MAX: u32 = u32::MAX;
/// The status `HostRmClient::control` returns when the ioctl itself fails.
const HOST_IOCTL_FAILED: u32 = 0xFFFF_FFFF;

fn put32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn get32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

/// Resize a reply built at the struct's own size to the request length (the RPC
/// carries exactly the guest's `paramsSize`).
fn fit(mut b: Vec<u8>, n: usize) -> Vec<u8> {
    b.resize(n, 0);
    b
}

fn ok(b: Vec<u8>, n: usize, label: &'static str, note: String) -> Answer {
    Answer {
        status: NV_OK,
        params: fit(b, n),
        label,
        note,
    }
}

fn not_supported(n: usize, label: &'static str, note: String) -> Answer {
    Answer {
        status: NV_ERR_NOT_SUPPORTED,
        params: vec![0u8; n],
        label,
        note,
    }
}

/// Issue a public control on the host subdevice; `None` without a host.
fn host_call(host: &mut Host<'_>, cmd: u32, buf: &mut [u8]) -> Option<u32> {
    host.as_deref_mut()
        .map(|h| h.control(NV20_SUBDEVICE_0, cmd, buf))
}

/// Forward one public control whose data fields match the internal element: call
/// `public` with a zeroed buffer of `pub_size`, copy `len` bytes from `pub_off` into
/// element 0 of an internal reply of `params_size`. `None` if there is no host or
/// the host did not answer NV_OK (the status is returned in the error).
fn forward_elem(
    host: &mut Host<'_>,
    public: u32,
    pub_size: usize,
    pub_off: usize,
    len: usize,
    params_size: usize,
) -> Result<Vec<u8>, Option<u32>> {
    let mut p = vec![0u8; pub_size];
    match host_call(host, public, &mut p) {
        None => Err(None),
        Some(NV_OK) => {
            let mut b = vec![0u8; params_size];
            b[..len].copy_from_slice(&p[pub_off..pub_off + len]);
            Ok(b)
        }
        Some(st) => Err(Some(st)),
    }
}

/// `host_call` that must answer NV_OK.
fn host_ok(host: &mut Host<'_>, cmd: u32, buf: &mut [u8]) -> Result<(), Option<u32>> {
    match host_call(host, cmd, buf) {
        Some(NV_OK) => Ok(()),
        e => Err(e),
    }
}

/// Row 41 from the host's public GR controls (module doc): element 0 of the internal
/// struct and a short summary. Any host error fails the whole assembly.
///
/// `GPU_GET_PES_INFO` is laid out as the HOST driver has it (hostabi.rs: 56 bytes
/// with `tpcToPesMap[10]` on 610, 76 with `[15]` on 615); the map copied into the
/// guest's element (ten entries for a 610 guest, fifteen for a 615 guest) is the shorter
/// of the two. TU106 has at most six TPCs per GPC, so nothing is lost on either.
fn floorsweeping(
    host: &mut Host<'_>,
    hl: &HostLayouts,
) -> Result<(Vec<u8>, String), Option<u32>> {
    use crate::ver::common as v;
    let fs = &crate::ver::layouts().fs;
    let mut p = vec![0u8; v::GR_ROUTED_SIZE];
    host_ok(host, v::CMD_GR_GET_GPC_MASK, &mut p)?;
    let gpc_mask = get32(&p, v::GR_ROUTED_ARG);
    let mut b = vec![0u8; fs.params];
    put32(&mut b, fs.gpc_mask, gpc_mask);
    put32(&mut b, fs.phys_gpc_mask, gpc_mask);
    let mut tpcs = Vec::new();
    // Non-MIG TPC/ZCULL/PES masks use physical GPC IDs; only TPC counts use
    // logical IDs (615 ctrl2080internal.h:306-325, kgr.c:3590-3674).
    // Limit shifts to the u32 mask width and writes to the guest's array bound.
    let physical_gpcs = (0..v::FS_MAX_GPC.min(u32::BITS as usize))
        .filter(|&g| gpc_mask & (1u32 << g) != 0);
    for (logical, g) in physical_gpcs.enumerate() {
        let mut p = vec![0u8; v::GR_ROUTED_SIZE];
        put32(&mut p, v::GR_ROUTED_ARG, g as u32);
        host_ok(host, v::CMD_GR_GET_TPC_MASK, &mut p)?;
        let tpc_mask = get32(&p, v::GR_ROUTED_OUT);
        put32(&mut b, fs.tpc_mask + 4 * g, tpc_mask);
        let mut p = [0u8; 8];
        put32(&mut p, 0, logical as u32);
        host_ok(host, v::CMD_GR_GET_NUM_TPCS_FOR_GPC, &mut p)?;
        put32(&mut b, fs.tpc_count + 4 * logical, get32(&p, 4));
        tpcs.push(format!("{tpc_mask:#x}/{}", get32(&p, 4)));
        // NOT_SUPPORTED means the host holds NV_U32_MAX there (kgr.c:3840-3842).
        let mut p = [0u8; 8];
        put32(&mut p, 0, g as u32);
        let zcull = match host_call(host, v::CMD_GR_GET_ZCULL_MASK, &mut p) {
            Some(NV_OK) => get32(&p, 4),
            Some(NV_ERR_NOT_SUPPORTED) => NV_U32_MAX,
            e => return Err(e),
        };
        put32(&mut b, fs.zcull_mask + 4 * g, zcull);
        let mut p = vec![0u8; hl.pes_info_size];
        put32(&mut p, 0, g as u32);
        host_ok(host, v::CMD_GPU_GET_PES_INFO, &mut p)?;
        put32(
            &mut b,
            fs.num_pes_per_gpc + 4 * g,
            get32(&p, hl.pes_info_num_pes),
        );
        if logical == 0 {
            // One shared map in the internal struct, returned for any physical
            // gpcId (615 subdevice_ctrl_gpu_kernel.c:2018-2020). Copy it even
            // when physical GPC 0 is disabled.
            let (src, dst) = (hl.pes_info_tpc_to_pes_map, fs.tpc_to_pes_map);
            let len = 4 * fs.tpc_to_pes_map_len.min(hl.pes_info_map_len);
            b[dst..dst + len].copy_from_slice(&p[src..src + len]);
        }
    }
    let mut p = vec![0u8; v::GR_ROUTED_SIZE];
    host_ok(host, v::CMD_GR_GET_GFX_GPC_AND_TPC_INFO, &mut p)?;
    put32(&mut b, fs.phys_gfx_gpc_mask, get32(&p, v::GR_ROUTED_ARG));
    put32(&mut b, fs.num_gfx_tpc, get32(&p, v::GR_ROUTED_OUT));
    Ok((
        b,
        format!(
            "gpcMask {gpc_mask:#x}, tpc mask/count per GPC [{}]",
            tpcs.join(" ")
        ),
    ))
}

/// Row 40 from the host's public `GR_GET_GLOBAL_SM_ORDER`, laid out as the HOST
/// driver has it (`hl`: 9240 bytes on 610, 10264 on 615 with `virtualDpcId` in every SM
/// entry), translated field by name into element 0 of the guest's internal struct laid
/// out as `guest` (the guest ABI's: nine u16 per SM on 610, ten on 615). One path for
/// all four combinations of guest and host (hostabi.rs `translate_sm_order`). Returns
/// the params and what the translation did, for the log.
fn global_sm_order(
    host: &mut Host<'_>,
    hl: &HostLayouts,
    guest: &SmOrderLayout,
) -> Result<(Vec<u8>, String), Option<u32>> {
    let mut p = vec![0u8; hl.sm_order.size];
    host_ok(host, CMD_GR_GET_GLOBAL_SM_ORDER, &mut p)?;
    let mut b = vec![0u8; crate::ver::layouts().sm_order_params.max(guest.size)];
    hl.sm_order_to_guest(&p, &mut b[..guest.size], guest);
    let num_sm = u16::from_le_bytes([b[guest.num_sm], b[guest.num_sm + 1]]);
    let mut how = format!(
        "host {} entry {} to guest entry {}",
        hl.driver(),
        hl.sm_order.entry_size,
        guest.entry_size
    );
    let dropped = hl.sm_order.fields_not_in(guest);
    let zero = guest.fields_not_in(&hl.sm_order);
    if !dropped.is_empty() {
        how.push_str(&format!(", dropped {}", dropped.join(" ")));
    }
    if !zero.is_empty() {
        how.push_str(&format!(", {} 0 (not in the host's)", zero.join(" ")));
    }
    Ok((b, format!("kgr global SM order: {num_sm} SMs from host ({how})")))
}

/// `NV0080_CTRL_GR_INFO_INDEX_SM_VERSION` (ctrl0080gr.h), `major << 8 | minor`.
pub const GR_INFO_INDEX_SM_VERSION: usize = 0x0C;

static SM_VERSION: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

/// The SM-version rungs of the costume ladder (docs/caraxes/costume-ladder.md):
/// `--sm-version <major>.<minor>` makes every GR info answer report that SM version
/// instead of the executing chip's. Set once, before the first control.
pub fn set_sm_version_override(v: u32) -> Result<(), String> {
    SM_VERSION
        .set(v)
        .map_err(|_| "the SM version override is set once".to_string())
}

/// `7.5` -> 0x0705, `12.0` -> 0x0C00 (`NV2080_CTRL_GR_INFO_SM_VERSION_12_00`, ctrl2080gr.h).
pub fn parse_sm_version(s: &str) -> Option<u32> {
    let (a, b) = s.split_once('.')?;
    let (a, b) = (a.parse::<u32>().ok()?, b.parse::<u32>().ok()?);
    (a < 0x100 && b < 0x100).then_some((a << 8) | b)
}

/// Apply the SM-version override to a GR info table `b` (`{index, data}` pairs) and
/// extend `note` with what it did.
fn sm_override(b: &mut [u8], note: &str) -> String {
    let at = 8 * GR_INFO_INDEX_SM_VERSION + 4;
    match SM_VERSION.get() {
        Some(&v) if b.len() >= at + 4 => {
            let was = get32(b, at);
            put32(b, at, v);
            format!(
                "{note}; SM version {was:#06x} reported as {v:#06x} (--sm-version, costume ladder)"
            )
        }
        _ => note.to_string(),
    }
}

fn why(e: Option<u32>) -> String {
    match e {
        None => "no host".into(),
        Some(HOST_IOCTL_FAILED) => "host ioctl failed".into(),
        Some(st) => format!("host status {st:#x}"),
    }
}

/// Answer a KernelGraphics static-info control, or `None` if `cmd` is not one of
/// them. `params` is the request payload; the reply has the same length.
pub fn answer(
    chip: &'static ChipData,
    cmd: u32,
    params: &[u8],
    host: Option<&mut Box<dyn HostRm>>,
) -> Option<Answer> {
    let n = params.len();
    let hl = crate::hostabi::current();
    let mut host: Host<'_> = host.map(|b| b.as_mut());
    let host = &mut host;
    let a = match cmd {
        CMD_INTERNAL_GMMU_COPY_RESERVED_SPLIT_GVASPACE_PDES_TO_SERVER => {
            // Row 35: status only (kern_gmmu.c:322-330). The device never maps into
            // the server-reserved range, so there is nothing to mirror.
            ok(
                params.to_vec(),
                n,
                "synthesized (NV_OK, echo; phase5 row 35)",
                "copy reserved split GVA space PDEs to server: ok".into(),
            )
        }
        CMD_INTERNAL_STATIC_KGR_GET_CAPS => {
            // Fatal (kgr.c:1216-1224); engineCaps[grIdx] becomes grCaps. capsTbl is
            // byte 0..23 in both structs.
            match forward_elem(host, CMD_GR_GET_CAPS_V2, PUB_CAPS_V2, 0, CAPS_ELEM, CAPS_PARAMS) {
                Ok(b) => ok(b, n, "host (GR_GET_CAPS_V2)", "kgr caps: host capsTbl into engineCaps[0]".into()),
                Err(e) => ok(
                    vec![0u8; CAPS_PARAMS],
                    n,
                    "synthesized fallback (no GR caps)",
                    format!("kgr caps: zeros ({})", why(e)),
                ),
            }
        }
        CMD_INTERNAL_STATIC_KGR_GET_INFO => {
            // Tolerated (kgr.c:1230-1250: an error leaves pGrInfo NULL, and the next
            // control overwrites status). The host's GR_GET_INFO_V2 answers
            // data = infoList[index].data (kgr.c:2850-2854), so index = i for all 63
            // entries recovers the whole table.
            let mut p = vec![0u8; PUB_INFO_V2];
            put32(&mut p, 0, GR_INFO_COUNT as u32);
            for i in 0..GR_INFO_COUNT {
                put32(&mut p, PUB_INFO_V2_LIST + 8 * i, i as u32);
            }
            match host_call(host, CMD_GR_GET_INFO_V2, &mut p) {
                Some(NV_OK) => {
                    let mut b = vec![0u8; INFO_PARAMS];
                    for i in 0..GR_INFO_COUNT {
                        put32(&mut b, 8 * i, i as u32);
                        put32(&mut b, 8 * i + 4, get32(&p, PUB_INFO_V2_LIST + 8 * i + 4));
                    }
                    let note = sm_override(&mut b, "kgr info: 63 entries from host");
                    ok(b, n, "host (GR_GET_INFO_V2, index = i for all 63)", note)
                }
                // Pure emulation: the chip's own table (the GR facts of a capture), without
                // which pGrInfo stays NULL and the first channel group fails `numMax != 0`
                // (kernel_fifo.c:2760-2775, kernel_channel_group_api.c:914).
                e => match &chip.gr.emulated_info {
                    Some(t) => {
                        let mut b = vec![0u8; INFO_PARAMS];
                        for i in 0..GR_INFO_COUNT {
                            let d = t
                                .entries
                                .iter()
                                .find(|e| e[0] as usize == i)
                                .map_or(0, |e| e[1]);
                            put32(&mut b, 8 * i, i as u32);
                            put32(&mut b, 8 * i + 4, d);
                        }
                        let note = sm_override(
                            &mut b,
                            &format!("kgr info: 63 entries, the chip's emulated table ({})", why(e)),
                        );
                        Answer {
                            status: NV_OK,
                            params: fit(b, n),
                            label: &t.label,
                            note,
                        }
                    }
                    None => not_supported(
                        n,
                        "synthesized fallback (NOT_SUPPORTED, tolerated: pGrInfo stays NULL)",
                        format!("kgr info: not supported ({})", why(e)),
                    ),
                },
            }
        }
        CMD_INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS => match floorsweeping(host, hl) {
            // E5c: CUDA's compute object needs numGpcs > 0 (kernel_graphics_object.c:192),
            // and GR_GET_GPC_MASK/TPC_MASK are served from this field (kgr.c:3465-3554).
            Ok((b, gpcs)) => ok(
                b,
                n,
                "host (GR_GET_GPC_MASK, GR_GET_TPC_MASK, GR_GET_NUM_TPCS_FOR_GPC, GR_GET_ZCULL_MASK, GPU_GET_PES_INFO, GR_GET_GFX_GPC_AND_TPC_INFO)",
                format!("kgr floorsweeping masks from host: {gpcs}"),
            ),
            Err(e) => ok(
                vec![0u8; crate::ver::layouts().fs.params],
                n,
                "synthesized fallback (gpcMask 0: no golden channel, GR unusable)",
                format!("kgr floorsweeping masks: all zero ({})", why(e)),
            ),
        },
        CMD_INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER => {
            // Fatal (kgr.c:1290-1297), so never an error. The public struct holds the
            // internal element's per-SM u16 fields, numSm and numTpc; the host fills
            // them from its own static info (kgr.c:3176-3192). Guest and host may be
            // different driver versions: translated field by name.
            match global_sm_order(host, hl, &crate::ver::layouts().sm_order) {
                Ok((b, note)) => ok(
                    b,
                    n,
                    "host (GR_GET_GLOBAL_SM_ORDER, translated field by field from the host's layout)",
                    note,
                ),
                Err(e) => ok(
                    vec![0u8; crate::ver::layouts().sm_order_params],
                    n,
                    "synthesized fallback (empty SM order)",
                    format!("kgr global SM order: zeros ({})", why(e)),
                ),
            }
        }
        CMD_INTERNAL_STATIC_KGR_GET_PPC_MASKS => not_supported(
            n,
            "synthesized (NOT_SUPPORTED, tolerated: kernel_graphics.c:1320-1327)",
            "kgr PPC masks: not supported; the public GR_GET_PPC_MASK 0x20801233 is one GPC per call \
             ({grRouteInfo, gpcId, ppcMask}, ctrl2080gr.h:1810-1814), not the internal mask[64] layout"
                .into(),
        ),
        // The public mask queries a GSP client without static GR info sends to GSP
        // (nouveau's r570 path: GPC and TPC info at GSP init, fatal on failure; Linux 7.2.7
        // nvkm/subdev/gsp/rm/r570/gr.c:15-52). The RM answers each as the GR mask control
        // of the same GPC (subdevice_ctrl_gpu_kernel.c:1829-1976 of 615), which the host
        // serves from its own static info of the same silicon.
        v::CMD_GPU_GET_FERMI_GPC_INFO => {
            let mut p = vec![0u8; v::GR_ROUTED_SIZE];
            match host_ok(host, v::CMD_GR_GET_GPC_MASK, &mut p) {
                Ok(()) => {
                    let gpc_mask = get32(&p, v::GR_ROUTED_ARG);
                    let mut b = vec![0u8; v::FERMI_GPC_INFO_SIZE];
                    put32(&mut b, 0, gpc_mask);
                    ok(b, n, "host (GR_GET_GPC_MASK)", format!("fermi gpc info: gpcMask {gpc_mask:#x}"))
                }
                Err(e) => not_supported(
                    n,
                    "synthesized fallback (NOT_SUPPORTED: no GR mask source)",
                    format!("fermi gpc info: not supported ({})", why(e)),
                ),
            }
        }
        v::CMD_GPU_GET_FERMI_TPC_INFO => {
            let gpc = params.get(..4).map_or(0, |b| get32(b, 0));
            let mut p = vec![0u8; v::GR_ROUTED_SIZE];
            put32(&mut p, v::GR_ROUTED_ARG, gpc);
            match host_ok(host, v::CMD_GR_GET_TPC_MASK, &mut p) {
                Ok(()) => {
                    let tpc_mask = get32(&p, v::GR_ROUTED_OUT);
                    let mut b = vec![0u8; v::FERMI_TPC_INFO_SIZE];
                    put32(&mut b, 0, gpc);
                    put32(&mut b, 4, tpc_mask);
                    ok(b, n, "host (GR_GET_TPC_MASK)", format!("fermi tpc info: gpc {gpc} tpcMask {tpc_mask:#x}"))
                }
                Err(e) => not_supported(
                    n,
                    "synthesized fallback (NOT_SUPPORTED: no GR mask source)",
                    format!("fermi tpc info: gpc {gpc} not supported ({})", why(e)),
                ),
            }
        }
        v::CMD_GPU_GET_FERMI_ZCULL_INFO => {
            let gpc = params.get(..4).map_or(0, |b| get32(b, 0));
            let mut p = [0u8; 8];
            put32(&mut p, 0, gpc);
            match host_ok(host, v::CMD_GR_GET_ZCULL_MASK, &mut p) {
                Ok(()) => {
                    let mut b = vec![0u8; v::FERMI_ZCULL_INFO_SIZE];
                    b[..8].copy_from_slice(&p);
                    put32(&mut b, 0, gpc);
                    ok(b, n, "host (GR_GET_ZCULL_MASK)", format!("fermi zcull info: gpc {gpc} zcullMask {:#x}", get32(&p, 4)))
                }
                Err(e) => not_supported(
                    n,
                    "synthesized fallback (NOT_SUPPORTED: no GR mask source)",
                    format!("fermi zcull info: gpc {gpc} not supported ({})", why(e)),
                ),
            }
        }
        // The public ZCULL geometry, asked by a GSP client without static GR info (nouveau's
        // r570 GR oneinit, fatal on failure: r570/gr.c:190-196): the host's answer for its
        // GR0, the same struct.
        CMD_GR_GET_ZCULL_INFO => {
            match forward_elem(host, CMD_GR_GET_ZCULL_INFO, ZCULL_ELEM, 0, ZCULL_ELEM, ZCULL_ELEM) {
                Ok(b) => ok(b, n, "host (GR_GET_ZCULL_INFO)", "zcull info from host".into()),
                Err(e) => not_supported(
                    n,
                    "synthesized fallback (NOT_SUPPORTED: no GR source)",
                    format!("zcull info: not supported ({})", why(e)),
                ),
            }
        }
        CMD_INTERNAL_STATIC_KGR_GET_ZCULL_INFO => {
            // Tolerated: on NOT_SUPPORTED without MIG the status survives only until
            // the next control assigns it (kgr.c:1350-1365).
            match forward_elem(host, CMD_GR_GET_ZCULL_INFO, ZCULL_ELEM, 0, ZCULL_ELEM, ZCULL_PARAMS) {
                Ok(b) => ok(b, n, "host (GR_GET_ZCULL_INFO)", "kgr zcull info from host".into()),
                Err(e) => not_supported(
                    n,
                    "synthesized fallback (NOT_SUPPORTED, tolerated)",
                    format!("kgr zcull info: not supported ({})", why(e)),
                ),
            }
        }
        CMD_INTERNAL_STATIC_KGR_GET_ROP_INFO => {
            // Tolerated the same way (kgr.c:1380-1395).
            match forward_elem(host, CMD_GR_GET_ROP_INFO, ROP_ELEM, 0, ROP_ELEM, ROP_PARAMS) {
                Ok(b) => ok(b, n, "host (GR_GET_ROP_INFO)", "kgr ROP info from host".into()),
                Err(e) => not_supported(
                    n,
                    "synthesized fallback (NOT_SUPPORTED, tolerated)",
                    format!("kgr ROP info: not supported ({})", why(e)),
                ),
            }
        }
        CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_RATE_MODIFIER => {
            // Tolerated (kgr.c:1410-1413). Data fields are the nine u8 after grRouteInfo.
            match forward_elem(host, CMD_GR_GET_SM_ISSUE_RATE_MODIFIER, PUB_SMIR, PUB_SMIR_DATA, SMIR_ELEM, SMIR_PARAMS) {
                Ok(b) => ok(b, n, "host (GR_GET_SM_ISSUE_RATE_MODIFIER)", "kgr SM issue rate modifier from host".into()),
                Err(e) => not_supported(
                    n,
                    "synthesized fallback (NOT_SUPPORTED, tolerated)",
                    format!("kgr SM issue rate modifier: not supported ({})", why(e)),
                ),
            }
        }
        CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_RATE_MODIFIER_V2 => {
            // Tolerated (kgr.c:1436-1439). listSize 0 asks for the whole list, which the
            // host returns index and data for (kgr.c:3340-3350).
            match forward_elem(host, CMD_GR_GET_SM_ISSUE_RATE_MODIFIER_V2, LIST_ELEM, 0, LIST_ELEM, LIST_PARAMS) {
                Ok(b) => {
                    let len = get32(&b, 0);
                    ok(b, n, "host (GR_GET_SM_ISSUE_RATE_MODIFIER_V2)", format!("kgr SM issue rate modifier v2: {len} entries from host"))
                }
                Err(e) => not_supported(
                    n,
                    "synthesized fallback (NOT_SUPPORTED, tolerated)",
                    format!("kgr SM issue rate modifier v2: not supported ({})", why(e)),
                ),
            }
        }
        CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_THROTTLE_CTRL => {
            // Tolerated (kgr.c:1462-1465: NOT_SUPPORTED becomes NV_OK). Not forwarded:
            // the host's own static info has no throttle control on TU106, so the public
            // GR_GET_SM_ISSUE_THROTTLE_CTRL makes the host driver log an assertion
            // (kernel_graphics.c:3425) before it returns NOT_SUPPORTED, once per guest load.
            not_supported(
                n,
                "synthesized (NOT_SUPPORTED; forwarding makes the host driver assert)",
                "kgr SM issue throttle ctrl: not supported, not forwarded".into(),
            )
        }
        CMD_INTERNAL_STATIC_KGR_GET_FECS_RECORD_SIZE => {
            // Fatal (kgr.c:1470-1479); no public source.
            let mut b = vec![0u8; FECS_RECORD_SIZE_PARAMS];
            put32(&mut b, 0, FECS_EVENT_RECORD_SIZE);
            ok(
                b,
                n,
                "synthesized (sizeof FECS_EVENT_RECORD = 120, fecs_event_list.c:58-69)",
                format!("kgr FECS record size: {FECS_EVENT_RECORD_SIZE}"),
            )
        }
        CMD_INTERNAL_STATIC_KGR_GET_FECS_TRACE_DEFINES => not_supported(
            n,
            "synthesized (NOT_SUPPORTED, tolerated: kernel_graphics.c:1498-1501)",
            "kgr FECS trace defines: not supported; there is no public host control for them \
             (phase5 row 50); pFecsTraceDefines stays NULL"
                .into(),
        ),
        CMD_INTERNAL_STATIC_KGR_GET_PDB_PROPERTIES => {
            // Fatal (kgr.c:1508-1520). The chip's default (TU106: TRUE,
            // g_kernel_graphics_nvoc.c:306-311).
            let p = &chip.gr.per_subctx_header;
            let mut b = vec![0u8; PDB_PARAMS];
            b[0] = p.value as u8;
            ok(
                b,
                n,
                &p.label,
                if p.value {
                    "kgr PDB properties: per-subcontext ctx header supported".into()
                } else {
                    "kgr PDB properties: no per-subcontext ctx header".into()
                },
            )
        }
        CMD_INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO => context_buffers_info(host, n),
        CMD_INTERNAL_GR_GET_FECS_TRACE_HW_ENABLE => {
            // fecsBufferDisableHw (fecs_event_list.c:1593-1640, from fecsBufferTeardown
            // :1644-1650, kgr.c:409) and fecsBufferReset (:1499-1545). bEnable FALSE:
            // the teardown path then sends no SET_FECS_TRACE_HW_ENABLE (:1627-1639).
            let mut b = fit(params.to_vec(), FECS_HW_ENABLE_PARAMS.max(n));
            b[FECS_HW_ENABLE_BENABLE] = 0;
            ok(
                b,
                n,
                "synthesized (FECS trace HW never enabled)",
                "gr get FECS trace hw enable: bEnable FALSE".into(),
            )
        }
        _ => return None,
    };
    Some(a)
}

/// Row 52, fatal (kgr.c:745-770). Host: GR_GET_ENGINE_CONTEXT_PROPERTIES per engineId
/// 0..0x19; the host serves it from the same table and answers NOT_SUPPORTED where
/// the size is NV_U32_MAX (kgr.c:4085-4091), so NOT_SUPPORTED maps back to NV_U32_MAX.
///
/// During init the only consumer is KernelGraphics PostLoad (kgr.c:446-455):
/// `kgraphicsAllocGlobalCtxBuffers_GP100` allocates the FECS_EVENT buffer with the
/// size of engine 0x17 whenever it is > 0 and ctxsw logging is supported, which it is
/// on TU106 (kgraphics_gp100.c:258-296; g_kernel_graphics_nvoc.c:273-277). NV_U32_MAX
/// there would be a 4 GiB physically contiguous allocation, so FECS_EVENT is 0
/// (no buffer) whenever it would be NV_U32_MAX. The GR global buffers are not
/// allocated at init on a GSP client (bClientRmAllocatedCtxBuffer, gpu_registry.c:152-155,
/// checked at kgr.c:793-798), and the main (0) and patch (0x10) sizes are read only
/// when a GR context is created (kgr.c:1760, kernel_graphics_context.c:1239): by the
/// golden and WAR channels, which gpcMask 0 skips (kgr.c:487-489, 510-514), or by a
/// client, which gpcMask 0 leaves without a usable GR. So the fallback leaves them
/// NV_U32_MAX.
fn context_buffers_info(host: &mut Host<'_>, n: usize) -> Answer {
    let mut b = vec![0u8; CTX_PARAMS];
    let mut from_host = 0usize;
    let mut unsupported = 0usize;
    let mut failed: Option<Option<u32>> = None;
    for id in 0..CTX_ENGINE_COUNT {
        let (size, align) = if failed.is_some() {
            (NV_U32_MAX, 0)
        } else {
            let mut p = vec![0u8; PUB_CTX_PROPS];
            put32(&mut p, PUB_CTX_PROPS_ENGINE_ID, id as u32);
            match host_call(host, CMD_GR_GET_ENGINE_CONTEXT_PROPERTIES, &mut p) {
                Some(NV_OK) => {
                    from_host += 1;
                    (
                        get32(&p, PUB_CTX_PROPS_SIZE),
                        get32(&p, PUB_CTX_PROPS_ALIGNMENT),
                    )
                }
                Some(NV_ERR_NOT_SUPPORTED) => {
                    unsupported += 1;
                    (NV_U32_MAX, 0)
                }
                e => {
                    // No host, or an unexpected error: stop asking, the rest is the fallback.
                    failed = Some(e);
                    (NV_U32_MAX, 0)
                }
            }
        };
        put32(&mut b, 8 * id, size);
        put32(&mut b, 8 * id + 4, align);
    }
    let mut fecs_forced = false;
    if get32(&b, 8 * CTX_ENGINE_FECS_EVENT) == NV_U32_MAX {
        put32(&mut b, 8 * CTX_ENGINE_FECS_EVENT, 0);
        fecs_forced = true;
    }
    let fecs = if fecs_forced {
        "; FECS_EVENT forced to 0"
    } else {
        ""
    };
    match failed {
        None => ok(
            b,
            n,
            "host (GR_GET_ENGINE_CONTEXT_PROPERTIES per engineId, NOT_SUPPORTED -> NV_U32_MAX)",
            format!("kgr context buffers info: {from_host} engines from host, {unsupported} not supported{fecs}"),
        ),
        Some(e) if from_host + unsupported > 0 => ok(
            b,
            n,
            "host (GR_GET_ENGINE_CONTEXT_PROPERTIES, partial) + synthesized NV_U32_MAX",
            format!(
                "kgr context buffers info: {} engines answered, then {}; rest NV_U32_MAX{fecs}",
                from_host + unsupported,
                why(e)
            ),
        ),
        Some(e) => ok(
            b,
            n,
            "synthesized fallback (all NV_U32_MAX, FECS_EVENT 0)",
            format!("kgr context buffers info: no sizes ({}){fecs}", why(e)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ver::{GuestAbi, TestGuest, L610, L615};
    use std::mem::size_of;

    /// The bindings of the shared layouts (the same in 610 and 615; the ones that differ are
    /// checked per version below).
    use nvrm_sys::v610 as guest_ffi;

    fn chip() -> &'static ChipData {
        crate::fixture::rtx2070().chip.data()
    }

    /// The sizes the handlers were written against (ctrl2080internal.h, ctrl2080gr.h,
    /// ctrl0080gr.h), now computed from the bindings in src/ver.
    #[test]
    fn layouts_are_the_documented_ones() {
        assert_eq!((CAPS_ELEM, CAPS_PARAMS, PUB_CAPS_V2), (23, 184, 48));
        assert_eq!((INFO_ELEM, INFO_PARAMS, PUB_INFO_V2), (504, 4032, 528));
        // The two that differ between the guest ABIs, per version.
        for (l, fs, sm) in [
            (&L610, (1336, 10688), (9220, 73760)),
            (&L615, (1356, 10848), (10244, 81952)),
        ] {
            assert_eq!((l.fs.elem, l.fs.params), fs, "{}", l.driver);
            assert_eq!((l.sm_order.size, l.sm_order_params), sm, "{}", l.driver);
            assert_eq!(l.sm_order.num_sm, 512 * l.sm_order.entry_size);
        }
        assert_eq!(
            (ZCULL_ELEM, ZCULL_PARAMS, ROP_ELEM, ROP_PARAMS),
            (40, 320, 12, 96)
        );
        assert_eq!(PPC_PARAMS, 256 * GR_MAX_ENGINES);
        assert_eq!((CTX_ELEM, CTX_PARAMS, PUB_CTX_PROPS), (208, 1664, 32));
        assert_eq!((SMIR_PARAMS, PUB_SMIR, PUB_SMIR_DATA), (72, 32, 16));
        assert_eq!((LIST_ELEM, LIST_PARAMS), (2044, 16352));
        assert_eq!(
            (FECS_RECORD_SIZE_PARAMS, FECS_TRACE_PARAMS, PDB_PARAMS),
            (32, 256, 8)
        );
        assert_eq!((FECS_HW_ENABLE_PARAMS, FECS_HW_ENABLE_BENABLE), (24, 16));
    }

    #[test]
    fn layouts_match_the_bindings() {
        use guest_ffi as v;
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_INTERNAL_STATIC_GR_GET_CAPS_PARAMS>(),
            CAPS_PARAMS
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_GR_GET_CAPS_V2_PARAMS>(),
            PUB_CAPS_V2
        );
        assert_eq!(
            size_of::<v::NV2080_CTRL_INTERNAL_STATIC_GR_GET_INFO_PARAMS>(),
            INFO_PARAMS
        );
        assert_eq!(
            size_of::<v::NV2080_CTRL_GR_GET_INFO_V2_PARAMS>(),
            PUB_INFO_V2
        );
        macro_rules! per_version {
            ($m:ident, $l:expr) => {
                assert_eq!(
                    size_of::<nvrm_sys::$m::NV2080_CTRL_INTERNAL_STATIC_GR_GET_FLOORSWEEPING_MASKS_PARAMS>(),
                    $l.fs.params
                );
                assert_eq!(
                    size_of::<nvrm_sys::$m::NV2080_CTRL_INTERNAL_STATIC_GR_FLOORSWEEPING_MASKS>(),
                    $l.fs.elem
                );
                assert_eq!(
                    size_of::<nvrm_sys::$m::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER>(),
                    $l.sm_order.size
                );
                assert_eq!(
                    size_of::<nvrm_sys::$m::NV2080_CTRL_INTERNAL_STATIC_GR_GET_GLOBAL_SM_ORDER_PARAMS>(),
                    $l.sm_order_params
                );
            };
        }
        per_version!(v610, L610);
        per_version!(v615, L615);
        assert_eq!(
            size_of::<v::NV2080_CTRL_INTERNAL_STATIC_GR_GET_PPC_MASKS_PARAMS>(),
            PPC_PARAMS
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_INTERNAL_STATIC_GR_GET_ZCULL_INFO_PARAMS>(),
            ZCULL_PARAMS
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_GR_GET_ZCULL_INFO_PARAMS>(),
            ZCULL_ELEM
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_INTERNAL_STATIC_GR_GET_ROP_INFO_PARAMS>(),
            ROP_PARAMS
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_GR_GET_ROP_INFO_PARAMS>(),
            ROP_ELEM
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_INTERNAL_STATIC_GR_GET_SM_ISSUE_RATE_MODIFIER_PARAMS>(),
            SMIR_PARAMS
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_GR_GET_SM_ISSUE_RATE_MODIFIER_PARAMS>(),
            PUB_SMIR
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_INTERNAL_STATIC_GR_GET_SM_ISSUE_RATE_MODIFIER_V2_PARAMS>(
            ),
            LIST_PARAMS
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_GR_GET_SM_ISSUE_RATE_MODIFIER_V2_PARAMS>(),
            LIST_ELEM
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_INTERNAL_STATIC_GR_GET_SM_ISSUE_THROTTLE_CTRL_PARAMS>(),
            LIST_PARAMS
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_GR_GET_SM_ISSUE_THROTTLE_CTRL_PARAMS>(),
            LIST_ELEM
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_INTERNAL_STATIC_GR_GET_CONTEXT_BUFFERS_INFO_PARAMS>(),
            CTX_PARAMS
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_GR_GET_ENGINE_CONTEXT_PROPERTIES_PARAMS>(),
            PUB_CTX_PROPS
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_INTERNAL_STATIC_GR_GET_FECS_RECORD_SIZE_PARAMS>(),
            FECS_RECORD_SIZE_PARAMS
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_INTERNAL_STATIC_GR_GET_FECS_TRACE_DEFINES_PARAMS>(),
            FECS_TRACE_PARAMS
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_INTERNAL_STATIC_GR_GET_PDB_PROPERTIES_PARAMS>(),
            PDB_PARAMS
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_INTERNAL_GR_FECS_TRACE_HW_ENABLE_PARAMS>(),
            FECS_HW_ENABLE_PARAMS
        );
        assert_eq!(FECS_EVENT_RECORD_SIZE, 120);
    }

    #[test]
    fn field_offsets_match_the_bindings() {
        use guest_ffi as v;
        use std::mem::offset_of;
        macro_rules! per_version {
            ($m:ident, $l:expr, $g:expr) => {
                assert_eq!(
                    offset_of!(nvrm_sys::$m::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER, numSm),
                    512 * $l.sm_order.entry_size
                );
                assert_eq!(
                    offset_of!(nvrm_sys::$m::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER, numTpc),
                    $l.sm_order.num_tpc
                );
                // The guest's element is the generated one of its ABI, field for field.
                assert_eq!(&$l.sm_order, $g);
            };
        }
        per_version!(v610, L610, &crate::hostabi::guest_layouts::GUEST_SM_ORDER_610);
        per_version!(v615, L615, &crate::hostabi::guest_layouts::GUEST_SM_ORDER_615);
        assert_eq!(
            offset_of!(v::NV2080_CTRL_GR_GET_INFO_V2_PARAMS, grInfoList),
            4
        );
        assert_eq!(
            offset_of!(
                nvrm_sys::NV2080_CTRL_GR_GET_SM_ISSUE_RATE_MODIFIER_PARAMS,
                imla0
            ),
            PUB_SMIR_DATA
        );
        assert_eq!(
            offset_of!(
                nvrm_sys::NV2080_CTRL_GR_GET_ENGINE_CONTEXT_PROPERTIES_PARAMS,
                engineId
            ),
            16
        );
        assert_eq!(
            offset_of!(
                nvrm_sys::NV2080_CTRL_GR_GET_ENGINE_CONTEXT_PROPERTIES_PARAMS,
                alignment
            ),
            20
        );
        assert_eq!(
            offset_of!(
                nvrm_sys::NV2080_CTRL_GR_GET_ENGINE_CONTEXT_PROPERTIES_PARAMS,
                size
            ),
            24
        );
        assert_eq!(
            offset_of!(
                nvrm_sys::NV2080_CTRL_INTERNAL_GR_FECS_TRACE_HW_ENABLE_PARAMS,
                bEnable
            ),
            FECS_HW_ENABLE_BENABLE
        );
    }

    /// A fake host that answers the public GR controls with recognisable values.
    struct FakeHost;

    impl HostRm for FakeHost {
        fn control(&mut self, class: u32, cmd: u32, p: &mut [u8]) -> u32 {
            assert_eq!(class, NV20_SUBDEVICE_0);
            match cmd {
                CMD_GR_GET_CAPS_V2 => {
                    assert_eq!(p.len(), PUB_CAPS_V2);
                    for (i, c) in p[..CAPS_ELEM].iter_mut().enumerate() {
                        *c = i as u8 + 1;
                    }
                    NV_OK
                }
                CMD_GR_GET_INFO_V2 => {
                    assert_eq!(get32(p, 0), GR_INFO_COUNT as u32);
                    for i in 0..GR_INFO_COUNT {
                        let idx = get32(p, 4 + 8 * i);
                        put32(p, 4 + 8 * i + 4, 0x100 + idx);
                    }
                    NV_OK
                }
                CMD_GR_GET_GLOBAL_SM_ORDER => {
                    let l = &crate::hostabi::current().sm_order;
                    assert_eq!(p.len(), l.size);
                    p[0] = 7; // globalSmId[0].gpcId
                    p[l.num_sm] = 36; // numSm
                    NV_OK
                }
                CMD_GR_GET_SM_ISSUE_RATE_MODIFIER => {
                    p[PUB_SMIR_DATA] = 3;
                    p[PUB_SMIR_DATA + 8] = 9;
                    NV_OK
                }
                CMD_GR_GET_ENGINE_CONTEXT_PROPERTIES => {
                    let id = get32(p, 16);
                    if id == 0x0a || id == CTX_ENGINE_FECS_EVENT as u32 {
                        return NV_ERR_NOT_SUPPORTED;
                    }
                    put32(p, 20, 0x1000);
                    put32(p, 24, 0x10_000 + id);
                    NV_OK
                }
                _ => NV_ERR_NOT_SUPPORTED,
            }
        }
    }

    #[test]
    fn unknown_controls_are_not_answered() {
        assert!(answer(chip(), 0x2080_0A36, &[0u8; 88], None).is_none());
    }

    #[test]
    fn fallbacks_without_host() {
        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_CAPS,
            &[0xAA; CAPS_PARAMS],
            None,
        )
        .unwrap();
        assert_eq!((a.status, a.params.len()), (NV_OK, CAPS_PARAMS));
        assert!(a.params.iter().all(|&x| x == 0));

        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_INFO,
            &[0u8; INFO_PARAMS],
            None,
        )
        .unwrap();
        // Pure emulation: the chip's own GR facts (chips/tu106 [gr] emulated_info).
        assert_eq!((a.status, a.params.len()), (NV_OK, INFO_PARAMS));
        assert_eq!(get32(&a.params, 8 * GR_INFO_INDEX_SM_VERSION + 4), 0x0705);
        assert_eq!(get32(&a.params, 8 * 0x14 + 4), 3);
        assert_eq!(parse_sm_version("8.6"), Some(0x0806));
        assert_eq!(parse_sm_version("12.0"), Some(0x0C00));
        assert_eq!(parse_sm_version("12"), None);

        for g in [GuestAbi::V610, GuestAbi::V615] {
            let _g = TestGuest::set(g);
            let l = crate::ver::layouts();
            let a = answer(
                chip(),
                CMD_INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS,
                &vec![0xFF; l.fs.params],
                None,
            )
            .unwrap();
            assert_eq!((a.status, a.params.len()), (NV_OK, l.fs.params));
            assert_eq!(get32(&a.params, 0), 0, "gpcMask 0 without a host");
            assert!(a.label.contains("fallback"));

            let a = answer(
                chip(),
                CMD_INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER,
                &vec![0u8; l.sm_order_params],
                None,
            )
            .unwrap();
            assert_eq!((a.status, a.params.len()), (NV_OK, l.sm_order_params));
        }

        for cmd in [
            CMD_INTERNAL_STATIC_KGR_GET_PPC_MASKS,
            CMD_INTERNAL_STATIC_KGR_GET_ZCULL_INFO,
            CMD_INTERNAL_STATIC_KGR_GET_ROP_INFO,
            CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_RATE_MODIFIER,
            CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_RATE_MODIFIER_V2,
            CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_THROTTLE_CTRL,
            CMD_INTERNAL_STATIC_KGR_GET_FECS_TRACE_DEFINES,
        ] {
            let a = answer(chip(), cmd, &[0u8; 64], None).unwrap();
            assert_eq!(
                (a.status, a.params.len()),
                (NV_ERR_NOT_SUPPORTED, 64),
                "{cmd:#x}"
            );
        }

        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_FECS_RECORD_SIZE,
            &[0u8; FECS_RECORD_SIZE_PARAMS],
            None,
        )
        .unwrap();
        assert_eq!((a.status, get32(&a.params, 0)), (NV_OK, 120));

        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_PDB_PROPERTIES,
            &[0u8; PDB_PARAMS],
            None,
        )
        .unwrap();
        assert_eq!((a.status, a.params[0]), (NV_OK, 1));
        assert_eq!(
            a.label,
            "synthesized (bPerSubCtxheaderSupported TRUE, g_kernel_graphics_nvoc.c:306-314)"
        );

        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO,
            &[0u8; CTX_PARAMS],
            None,
        )
        .unwrap();
        assert_eq!((a.status, a.params.len()), (NV_OK, CTX_PARAMS));
        for id in 0..CTX_ENGINE_COUNT {
            let want = if id == CTX_ENGINE_FECS_EVENT {
                0
            } else {
                u32::MAX
            };
            assert_eq!(get32(&a.params, 8 * id), want, "engine {id:#x}");
        }

        let mut p = vec![0x11u8; FECS_HW_ENABLE_PARAMS];
        p[FECS_HW_ENABLE_BENABLE] = 1;
        let a = answer(chip(), CMD_INTERNAL_GR_GET_FECS_TRACE_HW_ENABLE, &p, None).unwrap();
        assert_eq!(a.status, NV_OK);
        assert_eq!(a.params.len(), FECS_HW_ENABLE_PARAMS);
        assert_eq!(a.params[FECS_HW_ENABLE_BENABLE], 0);
        assert_eq!(a.params[0], 0x11, "grRouteInfo is echoed");

        let p: Vec<u8> = (0..56u8).collect();
        let a = answer(
            chip(),
            CMD_INTERNAL_GMMU_COPY_RESERVED_SPLIT_GVASPACE_PDES_TO_SERVER,
            &p,
            None,
        )
        .unwrap();
        assert_eq!((a.status, &a.params), (NV_OK, &p));
    }

    #[test]
    fn forwarding_through_a_host() {
        let mut host: Box<dyn HostRm> = Box::new(FakeHost);

        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_CAPS,
            &[0u8; CAPS_PARAMS],
            Some(&mut host),
        )
        .unwrap();
        assert_eq!(a.status, NV_OK);
        assert_eq!(&a.params[..3], &[1, 2, 3]);
        assert_eq!(a.params[CAPS_ELEM - 1], CAPS_ELEM as u8);
        assert!(a.params[CAPS_ELEM..].iter().all(|&x| x == 0));
        assert_eq!(a.label, "host (GR_GET_CAPS_V2)");

        // Never forwarded, even with a host: the host driver would assert.
        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_THROTTLE_CTRL,
            &[0u8; 64],
            Some(&mut host),
        )
        .unwrap();
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
        assert!(a.label.contains("host driver assert"));

        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_INFO,
            &[0u8; INFO_PARAMS],
            Some(&mut host),
        )
        .unwrap();
        assert_eq!(a.status, NV_OK);
        assert_eq!(
            (get32(&a.params, 8 * 5), get32(&a.params, 8 * 5 + 4)),
            (5, 0x105)
        );
        assert_eq!(get32(&a.params, 8 * 62 + 4), 0x100 + 62);
        assert!(a.params[INFO_ELEM..].iter().all(|&x| x == 0));

        let l = crate::ver::layouts();
        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER,
            &vec![0u8; l.sm_order_params],
            Some(&mut host),
        )
        .unwrap();
        assert_eq!(
            (a.status, a.params[0], a.params[l.sm_order.num_sm]),
            (NV_OK, 7, 36)
        );
        assert_eq!(a.params.len(), l.sm_order_params);
        assert!(a.note.contains("36 SMs from host (host 610.57.04"), "{}", a.note);

        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_SM_ISSUE_RATE_MODIFIER,
            &[0u8; SMIR_PARAMS],
            Some(&mut host),
        )
        .unwrap();
        assert_eq!((a.status, a.params[0], a.params[8]), (NV_OK, 3, 9));

        // The fake host has no ZCULL info: NOT_SUPPORTED passes through as tolerated.
        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_ZCULL_INFO,
            &[0u8; ZCULL_PARAMS],
            Some(&mut host),
        )
        .unwrap();
        assert_eq!(
            (a.status, a.params.len()),
            (NV_ERR_NOT_SUPPORTED, ZCULL_PARAMS)
        );

        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_CONTEXT_BUFFERS_INFO,
            &[0u8; CTX_PARAMS],
            Some(&mut host),
        )
        .unwrap();
        assert_eq!(a.status, NV_OK);
        assert_eq!(
            (get32(&a.params, 0), get32(&a.params, 4)),
            (0x10_000, 0x1000)
        );
        assert_eq!(get32(&a.params, 8 * 0x10), 0x10_010);
        assert_eq!(get32(&a.params, 8 * 0x0a), u32::MAX);
        assert_eq!(get32(&a.params, 8 * CTX_ENGINE_FECS_EVENT), 0);
        assert!(a.params[CTX_ELEM..].iter().all(|&x| x == 0));

        // The synthesized controls never ask the host.
        let mut silent: Box<dyn HostRm> = Box::new(PanicHost);
        for cmd in [
            CMD_INTERNAL_STATIC_KGR_GET_FECS_RECORD_SIZE,
            CMD_INTERNAL_STATIC_KGR_GET_PDB_PROPERTIES,
            CMD_INTERNAL_STATIC_KGR_GET_PPC_MASKS,
            CMD_INTERNAL_STATIC_KGR_GET_FECS_TRACE_DEFINES,
        ] {
            assert!(answer(chip(), cmd, &vec![0u8; l.fs.params], Some(&mut silent)).is_some());
        }
    }

    /// Row 41 assembled from a host with three GPCs of 0xf TPCs each (TU106's shape).
    #[test]
    fn floorsweeping_from_host() {
        for g in [GuestAbi::V610, GuestAbi::V615] {
            let _g = TestGuest::set(g);
            floorsweeping_from_host_case();
        }
    }

    fn floorsweeping_from_host_case() {
        use crate::ver::common as v;
        let fs = &crate::ver::layouts().fs;
        struct Gr;
        impl HostRm for Gr {
            fn control(&mut self, class: u32, cmd: u32, p: &mut [u8]) -> u32 {
                assert_eq!(class, NV20_SUBDEVICE_0);
                match cmd {
                    v::CMD_GR_GET_GPC_MASK => put32(p, 16, 0x7),
                    v::CMD_GR_GET_TPC_MASK => put32(p, 20, 0xf),
                    v::CMD_GR_GET_NUM_TPCS_FOR_GPC => put32(p, 4, 4),
                    v::CMD_GR_GET_ZCULL_MASK => return NV_ERR_NOT_SUPPORTED,
                    v::CMD_GPU_GET_PES_INFO => {
                        put32(p, 4, 2);
                        put32(p, crate::hostabi::current().pes_info_tpc_to_pes_map + 4, 1);
                    }
                    v::CMD_GR_GET_GFX_GPC_AND_TPC_INFO => {
                        put32(p, 16, 0x7);
                        put32(p, 20, 12);
                    }
                    c => panic!("unexpected {c:#x}"),
                }
                NV_OK
            }
        }
        let mut host: Box<dyn HostRm> = Box::new(Gr);
        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS,
            &vec![0u8; fs.params],
            Some(&mut host),
        )
        .unwrap();
        assert_eq!((a.status, a.params.len()), (NV_OK, fs.params));
        let p = &a.params;
        assert_eq!(
            (get32(p, fs.gpc_mask), get32(p, fs.phys_gpc_mask)),
            (7, 7)
        );
        for g in 0..3 {
            assert_eq!(get32(p, fs.tpc_mask + 4 * g), 0xf);
            assert_eq!(get32(p, fs.tpc_count + 4 * g), 4);
            assert_eq!(get32(p, fs.zcull_mask + 4 * g), u32::MAX);
            assert_eq!(get32(p, fs.num_pes_per_gpc + 4 * g), 2);
        }
        assert_eq!(get32(p, fs.tpc_mask + 12), 0);
        assert_eq!(get32(p, fs.tpc_to_pes_map + 4), 1);
        assert_eq!(
            (
                get32(p, fs.phys_gfx_gpc_mask),
                get32(p, fs.num_gfx_tpc)
            ),
            (7, 12)
        );
        assert!(p[fs.elem..].iter().all(|&x| x == 0));
        assert!(a.label.starts_with("host"));
    }

    /// Non-MIG sparse physical IDs must not become logical IDs in mask/PES
    /// queries or output arrays. TPC counts alone use dense logical IDs.
    #[test]
    fn floorsweeping_sparse_physical_and_logical_gpcs() {
        use crate::hostabi::{HOST_610, HOST_615};
        use crate::ver::common as v;

        struct Gr {
            mask: u32,
            layout: &'static HostLayouts,
            calls: Vec<(u32, u32)>,
        }

        fn physical_ids(mask: u32) -> Vec<u32> {
            (0..32).filter(|&g| mask & (1u32 << g) != 0).collect()
        }

        fn num_tpcs(physical: u32) -> u32 {
            1 + physical % 6
        }

        impl HostRm for Gr {
            fn control(&mut self, class: u32, cmd: u32, p: &mut [u8]) -> u32 {
                assert_eq!(class, NV20_SUBDEVICE_0);
                match cmd {
                    v::CMD_GR_GET_GPC_MASK => put32(p, v::GR_ROUTED_ARG, self.mask),
                    v::CMD_GR_GET_TPC_MASK | v::CMD_GR_GET_ZCULL_MASK | v::CMD_GPU_GET_PES_INFO => {
                        let g = get32(
                            p,
                            if cmd == v::CMD_GR_GET_TPC_MASK {
                                v::GR_ROUTED_ARG
                            } else {
                                0
                            },
                        );
                        assert!(
                            g < 32 && self.mask & (1u32 << g) != 0,
                            "physical query {cmd:#x} for disabled/out-of-range GPC {g}"
                        );
                        self.calls.push((cmd, g));
                        match cmd {
                            v::CMD_GR_GET_TPC_MASK => {
                                put32(p, v::GR_ROUTED_OUT, (1u32 << num_tpcs(g)) - 1)
                            }
                            v::CMD_GR_GET_ZCULL_MASK => put32(p, 4, 0x100 + g),
                            _ => {
                                assert_eq!(p.len(), self.layout.pes_info_size);
                                put32(p, self.layout.pes_info_num_pes, 1 + g % 3);
                                for i in 0..self.layout.pes_info_map_len {
                                    put32(
                                        p,
                                        self.layout.pes_info_tpc_to_pes_map + 4 * i,
                                        0x80 + i as u32,
                                    );
                                }
                            }
                        }
                    }
                    v::CMD_GR_GET_NUM_TPCS_FOR_GPC => {
                        let logical = get32(p, 0);
                        let ids = physical_ids(self.mask);
                        assert!(
                            (logical as usize) < ids.len(),
                            "invalid logical GPC {logical}"
                        );
                        self.calls.push((cmd, logical));
                        put32(p, 4, num_tpcs(ids[logical as usize]));
                    }
                    v::CMD_GR_GET_GFX_GPC_AND_TPC_INFO => {
                        put32(p, v::GR_ROUTED_ARG, self.mask);
                        put32(
                            p,
                            v::GR_ROUTED_OUT,
                            physical_ids(self.mask).into_iter().map(num_tpcs).sum(),
                        );
                    }
                    c => panic!("unexpected {c:#x}"),
                }
                NV_OK
            }
        }

        assert!(
            v::FS_MAX_GPC >= 32,
            "all u32 mask bits fit the guest arrays"
        );
        for guest in [GuestAbi::V610, GuestAbi::V615] {
            let _guest = TestGuest::set(guest);
            let fs = &crate::ver::layouts().fs;
            for layout in [&HOST_610, &HOST_615] {
                for mask in [0b101, 0x7, 1u32 << 31, 0x8000_0005, u32::MAX, 0] {
                    let ids = physical_ids(mask);
                    let mut h = Gr {
                        mask,
                        layout,
                        calls: Vec::new(),
                    };
                    let (p, _) = {
                        let mut host: Host<'_> = Some(&mut h);
                        floorsweeping(&mut host, layout).unwrap()
                    };
                    let expected_calls: Vec<_> = ids
                        .iter()
                        .enumerate()
                        .flat_map(|(logical, &g)| {
                            [
                                (v::CMD_GR_GET_TPC_MASK, g),
                                (v::CMD_GR_GET_NUM_TPCS_FOR_GPC, logical as u32),
                                (v::CMD_GR_GET_ZCULL_MASK, g),
                                (v::CMD_GPU_GET_PES_INFO, g),
                            ]
                        })
                        .collect();
                    assert_eq!(h.calls, expected_calls, "{guest:?} mask {mask:#x}");
                    assert_eq!(p.len(), fs.params);
                    assert_eq!(get32(&p, fs.gpc_mask), mask);
                    assert_eq!(get32(&p, fs.phys_gpc_mask), mask);
                    for g in 0..v::FS_MAX_GPC {
                        let physical = ids.contains(&(g as u32));
                        assert_eq!(
                            get32(&p, fs.tpc_mask + 4 * g),
                            if physical {
                                (1u32 << num_tpcs(g as u32)) - 1
                            } else {
                                0
                            }
                        );
                        assert_eq!(
                            get32(&p, fs.zcull_mask + 4 * g),
                            if physical { 0x100 + g as u32 } else { 0 }
                        );
                        assert_eq!(
                            get32(&p, fs.num_pes_per_gpc + 4 * g),
                            if physical { 1 + g as u32 % 3 } else { 0 }
                        );
                        assert_eq!(
                            get32(&p, fs.tpc_count + 4 * g),
                            ids.get(g).map_or(0, |&physical| num_tpcs(physical))
                        );
                    }
                    let copied = fs.tpc_to_pes_map_len.min(layout.pes_info_map_len);
                    for i in 0..fs.tpc_to_pes_map_len {
                        assert_eq!(
                            get32(&p, fs.tpc_to_pes_map + 4 * i),
                            if mask != 0 && i < copied {
                                0x80 + i as u32
                            } else {
                                0
                            }
                        );
                    }
                    assert_eq!(get32(&p, fs.phys_gfx_gpc_mask), mask);
                    assert_eq!(
                        get32(&p, fs.num_gfx_tpc),
                        ids.into_iter().map(num_tpcs).sum::<u32>()
                    );
                    assert!(p[fs.elem..].iter().all(|&x| x == 0));
                }
            }
        }
    }

    /// The public mask queries a GSP client without static GR info sends (nouveau's r570
    /// path): answered from the GR mask controls of the same GPC as the RM does, the
    /// request's gpcId echoed; the ZCULL geometry is the host's struct; no host, NOT_SUPPORTED.
    #[test]
    fn fermi_mask_queries_come_from_the_gr_masks() {
        use crate::ver::common as v;
        struct Gr;
        impl HostRm for Gr {
            fn control(&mut self, class: u32, cmd: u32, p: &mut [u8]) -> u32 {
                assert_eq!(class, NV20_SUBDEVICE_0);
                match cmd {
                    v::CMD_GR_GET_GPC_MASK => put32(p, v::GR_ROUTED_ARG, 0x7),
                    v::CMD_GR_GET_TPC_MASK => {
                        let g = get32(p, v::GR_ROUTED_ARG);
                        put32(p, v::GR_ROUTED_OUT, 0x3f >> g);
                    }
                    v::CMD_GR_GET_ZCULL_MASK => put32(p, 4, 0x10 + get32(p, 0)),
                    CMD_GR_GET_ZCULL_INFO => {
                        for i in 0..10 {
                            put32(p, 4 * i, 100 + i as u32);
                        }
                    }
                    c => panic!("unexpected {c:#x}"),
                }
                NV_OK
            }
        }
        let mut host: Box<dyn HostRm> = Box::new(Gr);
        let a = answer(chip(), v::CMD_GPU_GET_FERMI_GPC_INFO, &[0u8; 4], Some(&mut host)).unwrap();
        assert_eq!((a.status, get32(&a.params, 0), a.params.len()), (NV_OK, 0x7, 4));
        let mut q = [0u8; 8];
        put32(&mut q, 0, 2);
        let a = answer(chip(), v::CMD_GPU_GET_FERMI_TPC_INFO, &q, Some(&mut host)).unwrap();
        assert_eq!((a.status, get32(&a.params, 0), get32(&a.params, 4)), (NV_OK, 2, 0xf));
        let a = answer(chip(), v::CMD_GPU_GET_FERMI_ZCULL_INFO, &q, Some(&mut host)).unwrap();
        assert_eq!((a.status, get32(&a.params, 0), get32(&a.params, 4)), (NV_OK, 2, 0x12));
        let a = answer(chip(), CMD_GR_GET_ZCULL_INFO, &[0u8; ZCULL_ELEM], Some(&mut host)).unwrap();
        assert_eq!((a.status, a.params.len()), (NV_OK, 40));
        assert_eq!((get32(&a.params, 0), get32(&a.params, 36)), (100, 109));
        assert_eq!(
            (v::FERMI_GPC_INFO_SIZE, v::FERMI_TPC_INFO_SIZE, v::FERMI_ZCULL_INFO_SIZE, ZCULL_ELEM),
            (4, 8, 8, 40)
        );
        for cmd in [v::CMD_GPU_GET_FERMI_GPC_INFO, v::CMD_GPU_GET_FERMI_TPC_INFO, CMD_GR_GET_ZCULL_INFO] {
            let a = answer(chip(), cmd, &[0u8; 8], None).unwrap();
            assert_eq!(a.status, NV_ERR_NOT_SUPPORTED, "{cmd:#x} without a host");
        }
    }

    /// A host with the given layouts: PES info and SM order sized as that driver
    /// has them (a wrong size is what a 615 host would refuse or misread).
    struct SizedHost(&'static crate::hostabi::HostLayouts);

    /// A value that names its SM and field, the same in either layout: the SM in the
    /// high bits, the field's position in 615's entry (the superset) in the low four.
    fn tag(sm: usize, name: &str) -> u16 {
        let f = crate::hostabi::HOST_615.sm_order.fields;
        let i = f.iter().position(|(n, _)| *n == name).unwrap();
        ((sm as u16) << 4) | (i as u16 + 1)
    }

    impl HostRm for SizedHost {
        fn control(&mut self, class: u32, cmd: u32, p: &mut [u8]) -> u32 {
            use crate::ver::common as v;
            assert_eq!(class, NV20_SUBDEVICE_0);
            let l = self.0;
            match cmd {
                v::CMD_GR_GET_GPC_MASK => put32(p, 16, 0x1),
                v::CMD_GR_GET_TPC_MASK => put32(p, 20, 0xf),
                v::CMD_GR_GET_NUM_TPCS_FOR_GPC => put32(p, 4, 4),
                v::CMD_GR_GET_ZCULL_MASK => put32(p, 4, 0x3),
                v::CMD_GPU_GET_PES_INFO => {
                    assert_eq!(p.len(), l.pes_info_size);
                    put32(p, l.pes_info_num_pes, 2);
                    for i in 0..l.pes_info_map_len {
                        put32(p, l.pes_info_tpc_to_pes_map + 4 * i, 0x100 + i as u32);
                    }
                }
                v::CMD_GR_GET_GFX_GPC_AND_TPC_INFO => put32(p, 16, 0x1),
                CMD_GR_GET_GLOBAL_SM_ORDER => {
                    let o = &l.sm_order;
                    assert_eq!(p.len(), o.size, "the host call is sized by the host");
                    for sm in 0..o.entries {
                        for (name, off) in o.fields {
                            let at = sm * o.entry_size + off;
                            p[at..at + 2].copy_from_slice(&tag(sm, name).to_le_bytes());
                        }
                    }
                    p[o.num_sm..o.num_sm + 2].copy_from_slice(&36u16.to_le_bytes());
                    p[o.num_tpc..o.num_tpc + 2].copy_from_slice(&18u16.to_le_bytes());
                }
                c => panic!("unexpected {c:#x}"),
            }
            NV_OK
        }
    }

    /// Both host selections against both guest layouts in one build: the host calls are
    /// sized by the host (56/76 bytes PES info, 9240/10264 SM order) and the guest
    /// element is laid out as the guest ABI in force has it.
    #[test]
    fn host_selection_610_and_615() {
        use crate::hostabi::{HOST_610, HOST_615};
        for g in [GuestAbi::V610, GuestAbi::V615] {
            let _g = TestGuest::set(g);
            let gl = crate::ver::layouts();
            let gs = &gl.sm_order;
            for l in [&HOST_610, &HOST_615] {
                let mut h: Box<dyn HostRm> = Box::new(SizedHost(l));
                let mut host: Host<'_> = Some(h.as_mut());
                let (fs, _) = floorsweeping(&mut host, l).unwrap();
                assert_eq!(fs.len(), gl.fs.params);
                assert_eq!(get32(&fs, gl.fs.num_pes_per_gpc), 2);
                // The shorter of the two maps is copied (the word after the guest's map is
                // numPesPerGpc[0], checked above as 2).
                let copied = gl.fs.tpc_to_pes_map_len.min(l.pes_info_map_len);
                for i in 0..gl.fs.tpc_to_pes_map_len {
                    let want = if i < copied { 0x100 + i as u32 } else { 0 };
                    assert_eq!(get32(&fs, gl.fs.tpc_to_pes_map + 4 * i), want, "{g:?} map {i}");
                }
                let (sm, note) = global_sm_order(&mut host, l, gs).unwrap();
                assert_eq!(sm.len(), gl.sm_order_params);
                assert!(note.contains(l.driver()), "{note}");
                let rd = |o: usize| u16::from_le_bytes([sm[o], sm[o + 1]]);
                assert_eq!((rd(gs.num_sm), rd(gs.num_tpc)), (36, 18));
                for sm_id in [0usize, 1, 35, 511] {
                    for (name, off) in gs.fields {
                        let host_has = l.sm_order.fields.iter().any(|(n, _)| n == name);
                        let want = if host_has { tag(sm_id, name) } else { 0 };
                        assert_eq!(rd(sm_id * gs.entry_size + off), want, "{g:?} {} sm {sm_id} {name}", l.driver());
                    }
                }
                assert!(sm[gs.size..].iter().all(|&x| x == 0));
            }
        }
    }

    /// The four combinations through the handler's own path: a 610 or 615 guest element
    /// (from the generated modules) filled from a 610 or
    /// 615 host that answers only a call of its own size.
    #[test]
    fn global_sm_order_all_four_combinations() {
        use crate::hostabi::guest_layouts::{GUEST_SM_ORDER_610, GUEST_SM_ORDER_615};
        use crate::hostabi::{HOST_610, HOST_615};
        let cases = [
            (&HOST_610, &GUEST_SM_ORDER_610, "", ""),
            (&HOST_615, &GUEST_SM_ORDER_610, "dropped virtualDpcId", ""),
            (&HOST_610, &GUEST_SM_ORDER_615, "", "virtualDpcId 0"),
            (&HOST_615, &GUEST_SM_ORDER_615, "", ""),
        ];
        for (l, g, dropped, zero) in cases {
            let what = format!("guest entry {} on host {}", g.entry_size, l.driver());
            let mut h: Box<dyn HostRm> = Box::new(SizedHost(l));
            let mut host: Host<'_> = Some(h.as_mut());
            let (b, note) = global_sm_order(&mut host, l, g).unwrap();
            assert!(b.len() >= g.size, "{what}");
            let rd = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
            assert_eq!((rd(g.num_sm), rd(g.num_tpc)), (36, 18), "{what}");
            for sm in 0..g.entries {
                for (name, off) in g.fields {
                    let want = if *name == "virtualDpcId" && l.sm_order.entry_size == 18 {
                        0
                    } else {
                        tag(sm, name)
                    };
                    assert_eq!(rd(sm * g.entry_size + off), want, "{what}: sm {sm} {name}");
                }
            }
            assert!(b[g.size..].iter().all(|&x| x == 0), "{what}");
            for part in [dropped, zero] {
                assert!(part.is_empty() || note.contains(part), "{what}: {note}");
            }
            if dropped.is_empty() && zero.is_empty() {
                assert!(!note.contains("virtualDpcId"), "{what}: {note}");
            }
        }
    }

    struct PanicHost;

    impl HostRm for PanicHost {
        fn control(&mut self, _: u32, cmd: u32, _: &mut [u8]) -> u32 {
            panic!("unexpected host control {cmd:#x}");
        }
    }
}
