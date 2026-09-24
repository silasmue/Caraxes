// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
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
// The control numbers and parameter layouts of the emulated driver version, computed
// from the typed bindings (src/ver); the tests below pin them.
pub use crate::ver::active::{
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
    FS_ELEM, FS_PARAMS, GR_INFO_COUNT, GR_MAX_ENGINES, INFO_ELEM, INFO_PARAMS, LIST_ELEM,
    LIST_PARAMS, PDB_PARAMS, PPC_PARAMS, PUB_CAPS_V2, PUB_CTX_PROPS, PUB_CTX_PROPS_ALIGNMENT,
    PUB_CTX_PROPS_ENGINE_ID, PUB_CTX_PROPS_SIZE, PUB_INFO_V2, PUB_INFO_V2_LIST, PUB_SMIR,
    PUB_SMIR_DATA, PUB_SM_ORDER, ROP_ELEM, ROP_PARAMS, SMIR_ELEM, SMIR_PARAMS, SM_ENTRY,
    SM_ORDER_ELEM, SM_ORDER_NUM_SM, SM_ORDER_PARAMS, ZCULL_ELEM, ZCULL_PARAMS,
};
use crate::hostabi::HostLayouts;
use crate::ver::active::{
    NV20_SUBDEVICE_0, NV_ERR_NOT_SUPPORTED, NV_OK, SM_ENTRY_FIELDS, SM_ORDER_NUM_TPC,
};
// Two SM-order paths meet here: `sm_order_widened` serves a guest ABI whose SM entry is
// wider than the 610 host's (a 615 guest on a 610 host), `global_sm_order` serves a 610
// guest from whatever host layout `HostLayouts` selected at start-up (a 615 host). A 615
// guest on a 615 host still takes the widened path with the 610 host constants; that
// combination is open item 1 of docs/caraxes/gsp-transport-615.md and cutover-615.md (d).
use crate::ver::active::{HOST_SM_ENTRY, HOST_SM_ORDER_NUM_SM, SM_ENTRY_WIDENED_AT};

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

/// The global SM order for a guest whose SM entry is wider than the host's (a 615 guest
/// on the 610 host: `virtualDpcId` inserted after `virtualGpcId`, ten u16 instead of
/// nine; docs/caraxes/gsp-transport-615.md §7). Each host entry is copied field by field
/// around the new u16, which stays 0 (no DPC before Blackwell); numSm and numTpc follow
/// the entries in both. Never an error, as the unwidened answer.
fn sm_order_widened(host: &mut Host<'_>, n: usize) -> Answer {
    let mut p = vec![0u8; PUB_SM_ORDER];
    match host_call(host, CMD_GR_GET_GLOBAL_SM_ORDER, &mut p) {
        Some(NV_OK) => {
            let mut b = vec![0u8; SM_ORDER_PARAMS];
            let wide = SM_ENTRY - HOST_SM_ENTRY;
            for i in 0..HOST_SM_ORDER_NUM_SM / HOST_SM_ENTRY {
                let (h, g) = (i * HOST_SM_ENTRY, i * SM_ENTRY);
                b[g..g + SM_ENTRY_WIDENED_AT].copy_from_slice(&p[h..h + SM_ENTRY_WIDENED_AT]);
                b[g + SM_ENTRY_WIDENED_AT + wide..g + SM_ENTRY]
                    .copy_from_slice(&p[h + SM_ENTRY_WIDENED_AT..h + HOST_SM_ENTRY]);
            }
            b[SM_ORDER_NUM_SM..SM_ORDER_NUM_SM + 4]
                .copy_from_slice(&p[HOST_SM_ORDER_NUM_SM..HOST_SM_ORDER_NUM_SM + 4]);
            let num_sm = u16::from_le_bytes([b[SM_ORDER_NUM_SM], b[SM_ORDER_NUM_SM + 1]]);
            ok(
                b,
                n,
                "host (GR_GET_GLOBAL_SM_ORDER, entries widened; virtualDpcId 0 synthesized)",
                format!("kgr global SM order: {num_sm} SMs from host, entries widened {HOST_SM_ENTRY} to {SM_ENTRY} bytes"),
            )
        }
        e => ok(
            vec![0u8; SM_ORDER_PARAMS],
            n,
            "synthesized fallback (empty SM order)",
            format!("kgr global SM order: zeros ({})", why(e)),
        ),
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
/// guest's element is the shorter of the two.
fn floorsweeping(
    host: &mut Host<'_>,
    hl: &HostLayouts,
) -> Result<(Vec<u8>, String), Option<u32>> {
    use crate::ver::active as v;
    let mut p = vec![0u8; v::GR_ROUTED_SIZE];
    host_ok(host, v::CMD_GR_GET_GPC_MASK, &mut p)?;
    let gpc_mask = get32(&p, v::GR_ROUTED_ARG);
    let mut b = vec![0u8; FS_PARAMS];
    put32(&mut b, v::FS_GPC_MASK, gpc_mask);
    put32(&mut b, v::FS_PHYS_GPC_MASK, gpc_mask);
    let mut tpcs = Vec::new();
    // The host indexes every per-GPC query by 0..popcount(gpcMask) (kgr.c:3544-3585).
    for g in 0..(gpc_mask.count_ones() as usize).min(v::FS_MAX_GPC) {
        let mut p = vec![0u8; v::GR_ROUTED_SIZE];
        put32(&mut p, v::GR_ROUTED_ARG, g as u32);
        host_ok(host, v::CMD_GR_GET_TPC_MASK, &mut p)?;
        let tpc_mask = get32(&p, v::GR_ROUTED_OUT);
        put32(&mut b, v::FS_TPC_MASK + 4 * g, tpc_mask);
        let mut p = [0u8; 8];
        put32(&mut p, 0, g as u32);
        host_ok(host, v::CMD_GR_GET_NUM_TPCS_FOR_GPC, &mut p)?;
        put32(&mut b, v::FS_TPC_COUNT + 4 * g, get32(&p, 4));
        tpcs.push(format!("{tpc_mask:#x}/{}", get32(&p, 4)));
        // NOT_SUPPORTED means the host holds NV_U32_MAX there (kgr.c:3840-3842).
        let mut p = [0u8; 8];
        put32(&mut p, 0, g as u32);
        let zcull = match host_call(host, v::CMD_GR_GET_ZCULL_MASK, &mut p) {
            Some(NV_OK) => get32(&p, 4),
            Some(NV_ERR_NOT_SUPPORTED) => NV_U32_MAX,
            e => return Err(e),
        };
        put32(&mut b, v::FS_ZCULL_MASK + 4 * g, zcull);
        let mut p = vec![0u8; hl.pes_info_size];
        put32(&mut p, 0, g as u32);
        host_ok(host, v::CMD_GPU_GET_PES_INFO, &mut p)?;
        put32(
            &mut b,
            v::FS_NUM_PES_PER_GPC + 4 * g,
            get32(&p, hl.pes_info_num_pes),
        );
        if g == 0 {
            // One map in the internal struct; the host fills it from GPC 0's
            // (subdevice_ctrl_gpu_kernel.c:2021-2028 serves it for any gpcId).
            let (src, dst) = (hl.pes_info_tpc_to_pes_map, v::FS_TPC_TO_PES_MAP);
            let len = 4 * v::PES_INFO_MAP_LEN.min(hl.pes_info_map_len);
            b[dst..dst + len].copy_from_slice(&p[src..src + len]);
        }
    }
    let mut p = vec![0u8; v::GR_ROUTED_SIZE];
    host_ok(host, v::CMD_GR_GET_GFX_GPC_AND_TPC_INFO, &mut p)?;
    put32(&mut b, v::FS_PHYS_GFX_GPC_MASK, get32(&p, v::GR_ROUTED_ARG));
    put32(&mut b, v::FS_NUM_GFX_TPC, get32(&p, v::GR_ROUTED_OUT));
    Ok((
        b,
        format!(
            "gpcMask {gpc_mask:#x}, tpc mask/count per GPC [{}]",
            tpcs.join(" ")
        ),
    ))
}

/// Row 40 from the host's public `GR_GET_GLOBAL_SM_ORDER`, laid out as the HOST
/// driver has it (hostabi.rs: 9240 bytes on 610, 10264 on 615 with `virtualDpcId`
/// in every SM entry), translated field by field into element 0 of the guest's
/// internal struct.
fn global_sm_order(host: &mut Host<'_>, hl: &HostLayouts) -> Result<Vec<u8>, Option<u32>> {
    let mut p = vec![0u8; hl.sm_order_size];
    host_ok(host, CMD_GR_GET_GLOBAL_SM_ORDER, &mut p)?;
    let mut b = vec![0u8; SM_ORDER_PARAMS];
    hl.sm_order_to_guest(
        &p,
        &mut b[..SM_ORDER_ELEM],
        SM_ENTRY,
        SM_ENTRY_FIELDS,
        SM_ORDER_NUM_SM,
        SM_ORDER_NUM_TPC,
    );
    Ok(b)
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
                    ok(b, n, "host (GR_GET_INFO_V2, index = i for all 63)", "kgr info: 63 entries from host".into())
                }
                e => not_supported(
                    n,
                    "synthesized fallback (NOT_SUPPORTED, tolerated: pGrInfo stays NULL)",
                    format!("kgr info: not supported ({})", why(e)),
                ),
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
                vec![0u8; FS_PARAMS],
                n,
                "synthesized fallback (gpcMask 0: no golden channel, GR unusable)",
                format!("kgr floorsweeping masks: all zero ({})", why(e)),
            ),
        },
        CMD_INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER if SM_ENTRY != HOST_SM_ENTRY => {
            sm_order_widened(host, n)
        }
        CMD_INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER => {
            // Fatal (kgr.c:1290-1297), so never an error. The public struct's first
            // 9220 bytes are the internal element field by field (same nine u16 per
            // SM, numSm, numTpc); the host fills them from its own static info
            // (kgr.c:3176-3192).
            match global_sm_order(host, hl) {
                Ok(b) => {
                    let num_sm = u16::from_le_bytes([b[SM_ORDER_NUM_SM], b[SM_ORDER_NUM_SM + 1]]);
                    ok(b, n, "host (GR_GET_GLOBAL_SM_ORDER)", format!("kgr global SM order: {num_sm} SMs from host"))
                }
                Err(e) => ok(
                    vec![0u8; SM_ORDER_PARAMS],
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
    use std::mem::size_of;

    fn chip() -> &'static ChipData {
        crate::fixture::rtx2070().chip.data()
    }

    /// The sizes the handlers were written against (ctrl2080internal.h, ctrl2080gr.h,
    /// ctrl0080gr.h), now computed from the bindings in src/ver.
    #[test]
    fn layouts_are_the_documented_ones() {
        assert_eq!((CAPS_ELEM, CAPS_PARAMS, PUB_CAPS_V2), (23, 184, 48));
        assert_eq!((INFO_ELEM, INFO_PARAMS, PUB_INFO_V2), (504, 4032, 528));
        assert_eq!((FS_ELEM, FS_PARAMS), (1336, 10688));
        assert_eq!(
            (SM_ORDER_ELEM, SM_ORDER_PARAMS, PUB_SM_ORDER),
            (9220, 73760, 9240)
        );
        assert_eq!(SM_ORDER_NUM_SM, 512 * SM_ENTRY);
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
        use nvrm_sys::v610 as v;
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
        assert_eq!(
            size_of::<v::NV2080_CTRL_INTERNAL_STATIC_GR_GET_FLOORSWEEPING_MASKS_PARAMS>(),
            FS_PARAMS
        );
        assert_eq!(
            size_of::<v::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER>(),
            SM_ORDER_ELEM
        );
        assert_eq!(
            size_of::<v::NV2080_CTRL_INTERNAL_STATIC_GR_GET_GLOBAL_SM_ORDER_PARAMS>(),
            SM_ORDER_PARAMS
        );
        assert_eq!(
            size_of::<v::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS>(),
            PUB_SM_ORDER
        );
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
        use nvrm_sys::v610 as v;
        use std::mem::offset_of;
        assert_eq!(
            offset_of!(v::NV2080_CTRL_INTERNAL_STATIC_GR_GLOBAL_SM_ORDER, numSm),
            512 * SM_ENTRY
        );
        assert_eq!(
            offset_of!(v::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS, numSm),
            512 * SM_ENTRY
        );
        assert_eq!(
            offset_of!(v::NV2080_CTRL_GR_GET_GLOBAL_SM_ORDER_PARAMS, grRouteInfo),
            9224
        );
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
                    p[0] = 7; // globalSmId[0].gpcId
                    p[512 * SM_ENTRY] = 36; // numSm
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
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);

        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_FLOORSWEEPING_MASKS,
            &[0xFF; FS_PARAMS],
            None,
        )
        .unwrap();
        assert_eq!((a.status, a.params.len()), (NV_OK, FS_PARAMS));
        assert_eq!(get32(&a.params, 0), 0, "gpcMask 0 without a host");
        assert!(a.label.contains("fallback"));

        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER,
            &[0u8; SM_ORDER_PARAMS],
            None,
        )
        .unwrap();
        assert_eq!((a.status, a.params.len()), (NV_OK, SM_ORDER_PARAMS));

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

        let a = answer(
            chip(),
            CMD_INTERNAL_STATIC_KGR_GET_GLOBAL_SM_ORDER,
            &[0u8; SM_ORDER_PARAMS],
            Some(&mut host),
        )
        .unwrap();
        assert_eq!(
            (a.status, a.params[0], a.params[512 * SM_ENTRY]),
            (NV_OK, 7, 36)
        );

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
            assert!(answer(chip(), cmd, &[0u8; FS_PARAMS], Some(&mut silent)).is_some());
        }
    }

    /// Row 41 assembled from a host with three GPCs of 0xf TPCs each (TU106's shape).
    #[test]
    fn floorsweeping_from_host() {
        use crate::ver::active as v;
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
                        put32(p, v::PES_INFO_TPC_TO_PES_MAP + 4, 1);
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
            &[0u8; FS_PARAMS],
            Some(&mut host),
        )
        .unwrap();
        assert_eq!((a.status, a.params.len()), (NV_OK, FS_PARAMS));
        let p = &a.params;
        assert_eq!(
            (get32(p, v::FS_GPC_MASK), get32(p, v::FS_PHYS_GPC_MASK)),
            (7, 7)
        );
        for g in 0..3 {
            assert_eq!(get32(p, v::FS_TPC_MASK + 4 * g), 0xf);
            assert_eq!(get32(p, v::FS_TPC_COUNT + 4 * g), 4);
            assert_eq!(get32(p, v::FS_ZCULL_MASK + 4 * g), u32::MAX);
            assert_eq!(get32(p, v::FS_NUM_PES_PER_GPC + 4 * g), 2);
        }
        assert_eq!(get32(p, v::FS_TPC_MASK + 12), 0);
        assert_eq!(get32(p, v::FS_TPC_TO_PES_MAP + 4), 1);
        assert_eq!(
            (
                get32(p, v::FS_PHYS_GFX_GPC_MASK),
                get32(p, v::FS_NUM_GFX_TPC)
            ),
            (7, 12)
        );
        assert!(p[FS_ELEM..].iter().all(|&x| x == 0));
        assert!(a.label.starts_with("host"));
    }

    /// A host with the given layouts: PES info and SM order sized as that driver
    /// has them (a wrong size is what a 615 host would refuse or misread).
    struct SizedHost(&'static crate::hostabi::HostLayouts);

    /// A value that names its SM and field, the same on either host layout.
    fn tag(sm: usize, name: &str) -> u16 {
        ((sm as u16) << 8) | name.bytes().map(u16::from).sum::<u16>() & 0xff
    }

    impl HostRm for SizedHost {
        fn control(&mut self, class: u32, cmd: u32, p: &mut [u8]) -> u32 {
            use crate::ver::active as v;
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
                    assert_eq!(p.len(), l.sm_order_size);
                    for sm in 0..2usize {
                        for (name, off) in l.sm_order_entry_fields {
                            let v = if *name == "virtualDpcId" { 0xDDDD } else { tag(sm, name) };
                            let o = sm * l.sm_order_entry_size + off;
                            p[o..o + 2].copy_from_slice(&v.to_le_bytes());
                        }
                    }
                    p[l.sm_order_num_sm..l.sm_order_num_sm + 2].copy_from_slice(&2u16.to_le_bytes());
                    p[l.sm_order_num_tpc..l.sm_order_num_tpc + 2].copy_from_slice(&1u16.to_le_bytes());
                }
                c => panic!("unexpected {c:#x}"),
            }
            NV_OK
        }
    }

    /// Both host selections against this build's guest layout: the host calls are
    /// sized by the host (56/76 bytes PES info, 9240/10264 SM order) and the guest
    /// element is the same either way.
    #[test]
    fn host_selection_610_and_615() {
        use crate::hostabi::{HOST_610, HOST_615};
        use crate::ver::active as v;
        let mut out = Vec::new();
        for l in [&HOST_610, &HOST_615] {
            let mut h: Box<dyn HostRm> = Box::new(SizedHost(l));
            let mut host: Host<'_> = Some(h.as_mut());
            let (fs, _) = floorsweeping(&mut host, l).unwrap();
            assert_eq!(fs.len(), FS_PARAMS);
            assert_eq!(get32(&fs, v::FS_NUM_PES_PER_GPC), 2);
            // The guest's map has 10 entries; 615's five extra are not copied (the
            // word after the map is numPesPerGpc[0], checked above as 2).
            for i in 0..v::PES_INFO_MAP_LEN {
                assert_eq!(get32(&fs, v::FS_TPC_TO_PES_MAP + 4 * i), 0x100 + i as u32);
            }
            assert_eq!(v::FS_TPC_TO_PES_MAP + 4 * v::PES_INFO_MAP_LEN, v::FS_NUM_PES_PER_GPC);
            let sm = global_sm_order(&mut host, l).unwrap();
            assert_eq!(sm.len(), SM_ORDER_PARAMS);
            let rd = |o: usize| u16::from_le_bytes([sm[o], sm[o + 1]]);
            assert_eq!((rd(SM_ORDER_NUM_SM), rd(SM_ORDER_NUM_TPC)), (2, 1));
            for (name, off) in SM_ENTRY_FIELDS {
                assert_eq!(rd(SM_ENTRY + off), tag(1, name), "{name}");
            }
            assert!(!sm.chunks(2).any(|c| c == [0xDD, 0xDD]));
            out.push((fs, sm));
        }
        assert_eq!(out[0].0, out[1].0);
        assert_eq!(out[0].1, out[1].1);
    }

    struct PanicHost;

    impl HostRm for PanicHost {
        fn control(&mut self, _: u32, cmd: u32, _: &mut [u8]) -> u32 {
            panic!("unexpected host control {cmd:#x}");
        }
    }
}
