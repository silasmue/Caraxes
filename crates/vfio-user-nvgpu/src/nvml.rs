// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The controls `nvidia-smi` and `nvidia-smi -q` make the guest kernel send to GSP
//! (docs/caraxes/phase5-rpcs.md §2, docs/caraxes/nvml.md). Decision D9: telemetry is
//! forwarded to the host card where the control is public and the layout is the same
//! on both sides (temperatures, power, clocks, pstate, utilization, fan, ECC, PCIe
//! link, throttle reasons, encoder/decoder samples); what has no public source is
//! synthesized with a label (SMC mode, NVLink); the identity stays device-owned
//! (name, FB size, UUID and chip-unique ids, PCI ids are never forwarded).
//!
//! Three routes reach this module:
//! - ROUTE_TO_PHYSICAL public controls with the guest user's handles
//!   (sdk/ctrl/ctrl2080/*.h, flags in src/nvidia/generated/g_subdevice_nvoc.c);
//! - "GSS legacy" controls (bit 0x8000, not 0xC000): the guest kernel forwards any
//!   such command opaquely (interface/deprecated/rmapi_gss_legacy_control.c:38-139),
//!   and so does the host kernel for the device's unprivileged host client. Only the
//!   commands the host's own `nvidia-smi -q` issues (matrix/traces/610.57.04/nvml.tsv)
//!   are forwarded, so the device never sends the host card a command a read-only
//!   query tool would not. Three of them have a params size that depends on the host
//!   driver (hostabi.rs `HostLayouts::legacy`): forwarded only when the guest's size is
//!   the running host's, NOT_SUPPORTED otherwise;
//! - kernel-served controls that issue an RPC of their own (phase5-rpcs.md K1-K5),
//!   on the guest kernel's internal handles.
//!
//! The dispatcher records the class of every guest object (`GSP_RM_ALLOC`), so a
//! control on a guest-allocated device goes to the host device, one on a subdevice to
//! the host subdevice, and one on a BINAPI object to the host client's BINAPI object.

use crate::chip::ChipData;
use crate::ctrl::{Answer, HostRm};
use crate::hostabi::{HostLayouts, HOST_SIZED_LEGACY};
// The control numbers, lists and layouts of the emulated driver version (src/ver).
use crate::ver::active::{
    self as v, INDEX_FORWARD_TO_PHYSICAL, INFO_CHIP_UNIQUE_HI, INFO_CHIP_UNIQUE_LO,
    NV20_SUBDEVICE_0, NV_ERR_NOT_SUPPORTED, NV_OK, SMC_MODE_UNSUPPORTED,
};
pub use crate::ver::active::{
    BINAPI, CMD_BINAPI_0108, CMD_DIAG_GPU_VERIFY_INFOROM, CMD_FB_GET_INFO_V2,
    CMD_GPU_GET_CHIP_DETAILS, CMD_GPU_GET_INFO_V2, CMD_INTERNAL_GPU_GET_SMC_MODE,
    CMD_NVLINK_GET_PLATFORM_INFO, CMD_THERMAL_SYSTEM_EXECUTE_V2,
    CMD_THERMAL_SYSTEM_EXECUTE_V2_PHYSICAL, FORWARD, GF100_SUBDEVICE_INFOROM, LEGACY,
    NV2081_BINAPI, NV20_SUBDEVICE_DIAG,
};

/// What the device owns about itself (docs/caraxes/identity-profile.md, decision D6).
#[derive(Clone, Debug)]
pub struct Profile {
    /// Guest-visible framebuffer (the profile's `fb_length`).
    pub fb_bytes: u64,
    /// The GID from the static info (`gidInfo`).
    pub gid: [u8; 16],
    /// PCI device id of the emulated function (config space).
    pub pci_device_id: u16,
}

type Host<'a> = Option<&'a mut (dyn HostRm + 'static)>;

fn put32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn get32(b: &[u8], off: usize) -> u32 {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .unwrap_or(0)
}

fn answer_with(status: u32, params: Vec<u8>, label: &'static str, note: String) -> Answer {
    Answer {
        status,
        params,
        label,
        note,
    }
}

/// Issue `cmd` on the host object for `class`; `None` without a host.
fn host_call(host: &mut Host<'_>, class: u32, cmd: u32, buf: &mut [u8]) -> Option<u32> {
    host.as_deref_mut().map(|h| h.control(class, cmd, buf))
}

/// Forward unchanged, or answer NOT_SUPPORTED without a host.
fn forward(
    host: &mut Host<'_>,
    class: u32,
    cmd: u32,
    params: &[u8],
    what: &str,
    label: &'static str,
) -> Answer {
    let mut b = params.to_vec();
    match host_call(host, class, cmd, &mut b) {
        Some(st) => answer_with(st, b, label, format!("{what}: host status {st:#x}")),
        None => answer_with(
            NV_ERR_NOT_SUPPORTED,
            params.to_vec(),
            "synthesized fallback (no host: NOT_SUPPORTED)",
            format!("{what}: no host"),
        ),
    }
}

/// Opcodes of a THERMAL_SYSTEM_EXECUTE_V2-shaped request (layout in src/ver).
fn thermal_opcodes(p: &[u8]) -> String {
    let count = (get32(p, v::THERMAL_INSN_COUNT) as usize).min(v::THERMAL_MAX_INSNS);
    let ops: Vec<String> = (0..count)
        .map(|i| {
            format!(
                "{:#x}",
                get32(
                    p,
                    v::THERMAL_INSNS + v::THERMAL_INSN_STRIDE * i + v::THERMAL_INSN_OPCODE
                )
            )
        })
        .collect();
    format!(
        "api {}.{} insn size {} ops [{}]",
        get32(p, v::THERMAL_API_VERSION),
        get32(p, v::THERMAL_API_REVISION),
        get32(p, v::THERMAL_INSN_SIZE),
        ops.join(" ")
    )
}

/// Answer an nvidia-smi control, or `None` if `cmd` is not one of them.
/// `class` is the class of the guest object the control names.
pub fn answer(
    chip: &'static ChipData,
    cmd: u32,
    class: u32,
    params: &[u8],
    host: Option<&mut Box<dyn HostRm>>,
    profile: &Profile,
) -> Option<Answer> {
    let n = params.len();
    let mut host: Host<'_> = host.map(|b| b.as_mut());
    let host = &mut host;

    if let Some((_, name)) = FORWARD.iter().find(|(c, _)| *c == cmd) {
        // The host object the id addresses (NV0080 the device, NV2080 the subdevice); a
        // row of any other class is never sent (public.rs guard test).
        let Some(target) = crate::hostrm::class_of_control(cmd) else {
            return Some(answer_with(
                NV_ERR_NOT_SUPPORTED,
                params.to_vec(),
                "refused (no host object for this control's class)",
                format!("{name}: not forwarded"),
            ));
        };
        return Some(forward(
            host,
            target,
            cmd,
            params,
            name,
            "host (forwarded as-is, public control)",
        ));
    }
    if HOST_SIZED_LEGACY.contains(&cmd) {
        let mut a = host_sized_legacy(host, crate::hostabi::current(), cmd, params);
        a.note.push_str(&legacy_hashes(params, &a.params));
        return Some(a);
    }
    if let Some((_, size, name)) = LEGACY.iter().find(|(c, _, _)| *c == cmd) {
        let mut a = forward(
            host,
            NV20_SUBDEVICE_0,
            cmd,
            params,
            &format!("GSS legacy {name}"),
            "host (GSS legacy, forwarded opaquely; command and size from a native host trace)",
        );
        if n != *size {
            a.note
                .push_str(&format!(" (params {n} bytes, host trace had {size})"));
        }
        a.note.push_str(&legacy_hashes(params, &a.params));
        return Some(a);
    }
    if cmd >> 16 == GF100_SUBDEVICE_INFOROM {
        // BBX_GET_TIME_DATA / BBX_GET_LAST_FLUSH_TIME: the host card has no BBX and
        // answered 0x56 in the trace; the device has no InfoROM object on the host.
        return Some(answer_with(
            NV_ERR_NOT_SUPPORTED,
            params.to_vec(),
            "synthesized (NOT_SUPPORTED, as the host card answers; no BBX)",
            format!("inforom {cmd:#x}: not supported"),
        ));
    }

    let a = match cmd {
        CMD_GPU_GET_INFO_V2 => get_info_v2(host, params, profile),
        CMD_INTERNAL_GPU_GET_SMC_MODE if !chip.gr.mig => {
            // K2 (subdevice_ctrl_gpu_kernel.c:255-289): {smcMode}. The chip has no MIG.
            let mut b = vec![0u8; n];
            if n >= 4 {
                put32(&mut b, 0, SMC_MODE_UNSUPPORTED);
            }
            answer_with(NV_OK, b, &chip.gr.smc_label, "smc mode: unsupported".into())
        }
        CMD_THERMAL_SYSTEM_EXECUTE_V2_PHYSICAL => {
            // K5: the physical half of THERMAL_SYSTEM_EXECUTE_V2; same params struct
            // (message id 0x13 in both, ctrl2080thermal.h:135-137). Sent as the public
            // control so the host kernel validates it before its own GSP sees it.
            let ops = thermal_opcodes(params);
            let mut a = forward(
                host,
                NV20_SUBDEVICE_0,
                CMD_THERMAL_SYSTEM_EXECUTE_V2,
                params,
                "thermal system execute v2",
                "host (THERMAL_SYSTEM_EXECUTE_V2 0x20800513, same layout)",
            );
            a.note.push_str(&format!(
                "; {ops}; {} instructions succeeded",
                get32(&a.params, v::THERMAL_SUCCESSFUL_INSNS)
            ));
            a
        }
        CMD_GPU_GET_CHIP_DETAILS => {
            // ctrl2080gpu.h:4289-4294: pciDevId @0, chipSku[4] @4, chipMajor @8, chipMinor @12.
            // The SKU and revision are the host chip's; the PCI id is the device's own.
            let mut a = forward(
                host,
                NV20_SUBDEVICE_0,
                cmd,
                params,
                "chip details",
                "host (GET_CHIP_DETAILS: SKU, major, minor) + card (pciDevId from config space)",
            );
            if a.status == NV_OK && n >= v::CHIP_DETAILS_PCI_DEV_ID + 4 {
                put32(
                    &mut a.params,
                    v::CHIP_DETAILS_PCI_DEV_ID,
                    profile.pci_device_id as u32,
                );
            }
            a
        }
        CMD_NVLINK_GET_PLATFORM_INFO => {
            // ctrl2080nvlink.h:2676-2686: GUIDs and slot numbers; the host returns zeros.
            answer_with(
                NV_OK,
                vec![0u8; n],
                "synthesized (no NVLink platform: zeros, as the host card answers)",
                "nvlink platform info: zeros".into(),
            )
        }
        CMD_FB_GET_INFO_V2 => fb_get_info_v2(host, params, profile),
        _ if class == NV2081_BINAPI && BINAPI.contains(&cmd) => forward(
            host,
            NV2081_BINAPI,
            cmd,
            params,
            &format!("binapi {cmd:#x}"),
            "host (BINAPI object of the host client, forwarded opaquely)",
        ),
        CMD_DIAG_GPU_VERIFY_INFOROM if class == NV20_SUBDEVICE_DIAG => {
            // ctrl208fgpu.h:139-147: {result, checksum}. A diagnostic class the device
            // does not open on the host; GeForce boards have no InfoROM checksum to verify.
            answer_with(
                NV_ERR_NOT_SUPPORTED,
                vec![0u8; n],
                "synthesized (NOT_SUPPORTED: no InfoROM verification, diag class not forwarded)",
                "diag verify inforom: not supported".into(),
            )
        }
        _ => return None,
    };
    Some(a)
}

/// FNV-1a over `b`: the request and reply fingerprints of a GSS-legacy forward in the
/// trace, from which [`LEGACY_CACHEABLE`] was proven (byte-identical requests and answers
/// across calls, processes and load; docs/caraxes/gsp-transport-classes.md §12 question 1).
pub fn fnv64(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &x| {
        (h ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn legacy_hashes(req: &[u8], reply: &[u8]) -> String {
    format!("; req {:016x} reply {:016x}", fnv64(req), fnv64(reply))
}

/// `RMCTRL_FLAGS_NON_PRIVILEGED` and `RMCTRL_FLAGS_CACHEABLE` (control.h:205, :257 in both
/// 610.57.04 and 615.71.09).
pub const RMCTRL_FLAGS_NON_PRIVILEGED: u32 = 0x0000_0008;
pub const RMCTRL_FLAGS_CACHEABLE: u32 = 0x0000_0400;

/// The GSS-legacy commands whose answer is static for the VM's life and independent of
/// the request, so the guest kernel may answer repeats itself. Proven from the trace
/// fingerprints (`fnv64`) of the 610 guest on the 615.71.09 host
/// (caraxes/runs/rpc-path-2026-09-25/cache-before-610, legacy-forwards.txt): one request
/// and one answer, NV_OK, over 20 `nvidia-smi -q` before and after a CUDA gate, a
/// 20-sample dmon and the NVML gate, from nvidia-smi, NVML and libcuda alike. Only
/// tables: a reply of at least 64 bytes (the cuInit query 0x59 has 332). Excluded,
/// though identical there: the small replies (perf 0x14, 0x60, 0x6e, 0x80, 0x84, clk
/// 0x01, 0x09, gpu 0x62; 1 to 52 bytes), which could be a live value that only held
/// still at the states sampled. Never: the commands whose answer changed (clk 0x04,
/// 0x64, thermal 0x2f, 0x46, perf 0x26, 0x81, 0x97, 0xa8: telemetry or input-keyed) and
/// every failing one (the guest caches only NV_OK).
pub const LEGACY_CACHEABLE: &[u32] = &[
    0x2080_8159, // gpu 0x59, 332 bytes (cuInit)
    0x2080_852A, // thermal 0x2a, 1784
    0x2080_852C, // thermal 0x2c, 1440
    0x2080_852E, // thermal 0x2e, 1432 (output only)
    0x2080_8542, // thermal 0x42, 504
    0x2080_9019, // clk 0x19, 12336
    0x2080_A079, // perf 0x79, 83972 (25 ms per host call)
    0x2080_A0A4, // perf 0xa4, 67396
    0x2080_A0A7, // perf 0xa7, 17676
    0x2080_A612, // pmgr 0x12, 54352
];

/// The switch that turns the CACHEABLE reply flag off (`CARAXES_LEGACY_CACHEABLE=0`): the
/// measurement's "before", and a way back should a cached answer ever be wrong.
pub const LEGACY_CACHEABLE_ENV: &str = "CARAXES_LEGACY_CACHEABLE";

/// Whether the CACHEABLE reply flag is on (default) or switched off.
pub fn legacy_cacheable_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var(LEGACY_CACHEABLE_ENV).map_or(true, |v| v.trim() != "0"))
}

/// The `rmctrlFlags` of the reply to GSS-legacy control `cmd` answered with `status`.
///
/// The guest kernel caches a GSS-legacy answer when the reply's flags are cacheable and
/// not by input, the access right is 0 and the control succeeded
/// (`rpcRmApiControl_GSP`, src/nvidia/src/kernel/vgpu/rpc.c:10894-10905 of 615.71.09,
/// :10898-10909 of 610.57.04), keyed by GPU and command only, without the request
/// (`_rmapiControlCacheSet`, rmapi_cache.c:481-535; get :407-472, the same file in both
/// versions). A later call, whatever its input, then gets the first answer
/// (rpc.c:10762-10775 of 615), after `rmControlValidateClientPrivilegeAccess`
/// (rmapi/control.c:678-716) checks the stored flags: without NON_PRIVILEGED a user
/// process would fail the check and the cache would never serve it. So: CACHEABLE |
/// NON_PRIVILEGED (the `_LEGACY_NON_PRIVILEGED` categories, ctrl2080base.h) for the
/// proven commands on NV_OK, 0 otherwise (the value the device sent before).
pub fn legacy_reply_flags(cmd: u32, status: u32) -> u32 {
    if status == NV_OK && legacy_cacheable_enabled() && LEGACY_CACHEABLE.contains(&cmd) {
        RMCTRL_FLAGS_CACHEABLE | RMCTRL_FLAGS_NON_PRIVILEGED
    } else {
        0
    }
}

/// A GSS legacy control whose params size depends on the host driver (`hl.legacy`).
/// No open header of either version defines these layouts (matrix/catalog-610.57.04.md:384,
/// :419), so a guest buffer of another size cannot be translated field by field: it is
/// answered NOT_SUPPORTED and not sent (the host would refuse it with 0x1f, as the 610
/// host did for the 615 guest: caraxes/runs/transport-615-2026-09-24/boot/
/// device-events.txt:735, :1151). What the guest loses is that reading (nvidia-smi shows
/// N/A: power draw for pmgr 0x18); docs/caraxes/cutover-615.md (d).
fn host_sized_legacy(host: &mut Host<'_>, hl: &HostLayouts, cmd: u32, params: &[u8]) -> Answer {
    let n = params.len();
    match hl.legacy_size(cmd) {
        Some((size, name)) if size == n => forward(
            host,
            NV20_SUBDEVICE_0,
            cmd,
            params,
            &format!("GSS legacy {name} ({n} bytes, host {})", hl.driver()),
            "host (GSS legacy, forwarded opaquely; the running host driver's size)",
        ),
        Some((size, name)) => answer_with(
            NV_ERR_NOT_SUPPORTED,
            params.to_vec(),
            "synthesized (NOT_SUPPORTED: GSS legacy params differ between guest and host driver, no layout to translate)",
            format!(
                "GSS legacy {name}: guest params {n} bytes, host {} takes {size}; not sent",
                hl.driver()
            ),
        ),
        None => answer_with(
            NV_ERR_NOT_SUPPORTED,
            params.to_vec(),
            "synthesized (NOT_SUPPORTED: no such GSS legacy control measured for the host driver)",
            format!(
                "GSS legacy {cmd:#x} ({n} bytes): not in host {}'s own nvidia-smi; not sent",
                hl.driver()
            ),
        ),
    }
}

/// K1: `GPU_GET_INFO_V2` with the indices the guest kernel could not serve marked by
/// bit 31 (subdevice_ctrl_gpu_kernel.c:600-635). Layout: gpuInfoListSize @0, entries
/// {index u32, data u32} @4. Unmarked entries are the kernel's own and stay as sent.
fn get_info_v2(host: &mut Host<'_>, params: &[u8], profile: &Profile) -> Answer {
    let n = params.len();
    let mut b = params.to_vec();
    let count = (get32(params, 0) as usize).min(n.saturating_sub(v::INFO_LIST) / 8);
    let mut to_host: Vec<(usize, u32)> = Vec::new();
    let mut own = Vec::new();
    for i in 0..count {
        let o = v::INFO_LIST + 8 * i;
        let index = get32(&b, o);
        if index & INDEX_FORWARD_TO_PHYSICAL == 0 {
            continue;
        }
        let idx = index & !INDEX_FORWARD_TO_PHYSICAL;
        put32(&mut b, o, idx);
        match idx {
            INFO_CHIP_UNIQUE_LO | INFO_CHIP_UNIQUE_HI => {
                let k = if idx == INFO_CHIP_UNIQUE_LO { 0 } else { 4 };
                let v = u32::from_le_bytes(profile.gid[k..k + 4].try_into().unwrap());
                put32(&mut b, o + 4, v);
                own.push(format!("{idx:#x}"));
            }
            _ => to_host.push((o, idx)),
        }
    }
    let mut status = NV_OK;
    let mut hostnote = String::new();
    if !to_host.is_empty() {
        let mut q = vec![0u8; n];
        put32(&mut q, 0, to_host.len() as u32);
        for (k, (_, idx)) in to_host.iter().enumerate() {
            put32(&mut q, v::INFO_LIST + 8 * k, *idx);
        }
        match host_call(host, NV20_SUBDEVICE_0, CMD_GPU_GET_INFO_V2, &mut q) {
            Some(NV_OK) => {
                for (k, (o, _)) in to_host.iter().enumerate() {
                    put32(&mut b, o + 4, get32(&q, v::INFO_LIST + 8 * k + 4));
                }
            }
            Some(st) => status = st,
            None => {}
        }
        let idxs: Vec<String> = to_host.iter().map(|(_, i)| format!("{i:#x}")).collect();
        hostnote = format!("host [{}] status {status:#x}", idxs.join(" "));
    }
    let label = match (to_host.is_empty(), own.is_empty()) {
        (true, false) => "profile (chip-unique indices 0x23/0x24 from the device GID)",
        (false, true) => "host (GPU_GET_INFO_V2, marked indices)",
        (false, false) => {
            "host (GPU_GET_INFO_V2, marked indices) + profile (0x23/0x24 from the device GID)"
        }
        (true, true) => "guest (nothing marked for the physical side; echo)",
    };
    answer_with(
        status,
        b,
        label,
        format!(
            "gpu info v2: {count} entries; {hostnote}; own [{}]",
            own.join(" ")
        ),
    )
}

/// `FB_GET_INFO_V2` (0x20801303) is kernel-served in the guest (kern_mem_sys_ctrl.c:1018);
/// should it reach the device, the host's answer is rewritten to the profile size
/// (identity-profile.md, the v1 `rewrite_fb_info` rule in vhost-user-nvrm/src/vram.rs).
/// Layout (ctrl2080fb.h:492-496): fbInfoListSize @0, entries {index u32, data u32} @4, KiB.
fn fb_get_info_v2(host: &mut Host<'_>, params: &[u8], profile: &Profile) -> Answer {
    let mut a = forward(
        host,
        NV20_SUBDEVICE_0,
        CMD_FB_GET_INFO_V2,
        params,
        "fb info v2",
        "host (FB_GET_INFO_V2) rewritten to the profile size (identity-profile.md)",
    );
    let kib = (profile.fb_bytes / 1024).min(u32::MAX as u64) as u32;
    let count = (get32(params, 0) as usize).min(params.len().saturating_sub(4) / 8);
    let mut rewritten = 0;
    for i in 0..count {
        let o = 4 + 8 * i;
        // The size indices (src/ver). The guest's heap is its own, so the device
        // reports it whole.
        if v::FB_INFO_SIZE_INDICES.contains(&get32(&a.params, o)) {
            put32(&mut a.params, o + 4, kib);
            rewritten += 1;
        }
    }
    if a.status == NV_OK {
        a.note.push_str(&format!(
            "; {rewritten} size entries set to {} MiB",
            profile.fb_bytes >> 20
        ));
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reply flags the guest kernel caches by (rpc.c:10899-10905 of 615): CACHEABLE |
    /// NON_PRIVILEGED only for the proven static GSS-legacy tables and only on NV_OK; every
    /// entry is a GSS-legacy command of the forwarded list with a table-sized reply, and
    /// never a command whose answer was seen to change.
    #[test]
    fn cacheable_reply_flags_only_for_proven_static_tables() {
        assert_eq!(legacy_reply_flags(0x2080_A079, NV_OK), 0x408);
        assert_eq!(legacy_reply_flags(0x2080_A079, NV_ERR_NOT_SUPPORTED), 0);
        for telemetry in [0x2080_9004, 0x2080_9064, 0x2080_852F, 0x2080_8546, 0x2080_A081, 0x2080_A097, 0x2080_A0A8, 0x2080_A026] {
            assert_eq!(legacy_reply_flags(telemetry, NV_OK), 0, "{telemetry:#x}");
        }
        for &c in LEGACY_CACHEABLE {
            assert_ne!(c & v::CMD_GSS_LEGACY_BIT, 0, "{c:#x} is GSS legacy (IsGssLegacyCall)");
            let (_, size, _) = LEGACY.iter().find(|(l, _, _)| *l == c).expect("forwarded");
            assert!(*size >= 64, "{c:#x}: a table, not a value");
        }
        // Not by input (rpc.c:10902 refuses those), no access right, no privilege check
        // that a user process would fail (rmapi/control.c:678-716).
        let f = RMCTRL_FLAGS_CACHEABLE | RMCTRL_FLAGS_NON_PRIVILEGED;
        assert_eq!(f & 0x0002_0000, 0);
        assert_eq!(fnv64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
    use crate::ver::active::NV01_DEVICE_0;

    fn chip() -> &'static ChipData {
        crate::fixture::rtx2070().chip.data()
    }

    fn size_of<T>() -> usize {
        std::mem::size_of::<T>()
    }

    #[test]
    fn layouts_match_the_bindings() {
        assert_eq!(
            size_of::<nvrm_sys::v610::NV2080_CTRL_GPU_GET_INFO_V2_PARAMS>(),
            580
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_GPU_GET_CHIP_DETAILS_PARAMS>(),
            16
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_INTERNAL_GPU_GET_SMC_MODE_PARAMS>(),
            4
        );
        assert_eq!(
            size_of::<nvrm_sys::v610::NV2080_CTRL_NVLINK_GET_PLATFORM_INFO_PARAMS>(),
            39
        );
        assert_eq!(
            size_of::<nvrm_sys::NV2080_CTRL_FB_GET_INFO_V2_PARAMS>(),
            1028
        );
    }

    /// Records what reaches the host and answers recognisably.
    #[derive(Default)]
    struct FakeHost {
        seen: std::sync::Arc<std::sync::Mutex<Vec<(u32, u32)>>>,
    }

    impl HostRm for FakeHost {
        fn control(&mut self, class: u32, cmd: u32, p: &mut [u8]) -> u32 {
            self.seen.lock().unwrap().push((class, cmd));
            match cmd {
                CMD_GPU_GET_INFO_V2 => {
                    for i in 0..get32(p, 0) as usize {
                        assert_eq!(get32(p, 4 + 8 * i) & INDEX_FORWARD_TO_PHYSICAL, 0);
                        put32(p, 8 + 8 * i, 0x100 + get32(p, 4 + 8 * i));
                    }
                    NV_OK
                }
                CMD_GPU_GET_CHIP_DETAILS => {
                    put32(p, 0, 0xBEEF);
                    put32(p, 8, 10);
                    NV_OK
                }
                CMD_FB_GET_INFO_V2 => {
                    for i in 0..get32(p, 0) as usize {
                        put32(p, 8 + 8 * i, 8 << 20);
                    }
                    NV_OK
                }
                CMD_THERMAL_SYSTEM_EXECUTE_V2 => {
                    put32(p, 16, 2);
                    NV_OK
                }
                _ => 0x57,
            }
        }
    }

    fn profile() -> Profile {
        let mut gid = [0u8; 16];
        gid[..8].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        Profile {
            fb_bytes: 4 << 30,
            gid,
            pci_device_id: 0x1F02,
        }
    }

    #[test]
    fn cuinit_controls_are_forwarded_to_the_host_subdevice() {
        let fake = FakeHost::default();
        let seen = fake.seen.clone();
        let mut h: Box<dyn HostRm> = Box::new(fake);
        for cmd in [0x2080_182A, 0x2080_182B, 0x2080_3801] {
            let a = answer(
                chip(),
                cmd,
                NV20_SUBDEVICE_0,
                &[0u8; 16],
                Some(&mut h),
                &profile(),
            )
            .unwrap();
            // The fake host's status comes back unchanged: the row forwarded it.
            assert_eq!(a.status, 0x57, "{cmd:#x}");
        }
        assert_eq!(
            *seen.lock().unwrap(),
            [
                (NV20_SUBDEVICE_0, 0x2080_182A),
                (NV20_SUBDEVICE_0, 0x2080_182B),
                (NV20_SUBDEVICE_0, 0x2080_3801)
            ]
        );
    }

    #[test]
    fn identity_is_never_forwarded() {
        let fake = FakeHost::default();
        let seen = fake.seen.clone();
        let mut h: Box<dyn HostRm> = Box::new(fake);
        let mut p = vec![0u8; 580];
        put32(&mut p, 0, 4);
        put32(&mut p, 4, 0x8000_0011);
        put32(&mut p, 12, 0x8000_0023);
        put32(&mut p, 20, 0x8000_0024);
        put32(&mut p, 28, 0x2a); // kernel-served, left alone
        put32(&mut p, 32, 2);
        let a = answer(
            chip(),
            CMD_GPU_GET_INFO_V2,
            NV20_SUBDEVICE_0,
            &p,
            Some(&mut h),
            &profile(),
        )
        .unwrap();
        assert_eq!(a.status, NV_OK);
        assert_eq!(get32(&a.params, 4), 0x11);
        assert_eq!(get32(&a.params, 8), 0x111);
        assert_eq!(get32(&a.params, 12), 0x23);
        assert_eq!(get32(&a.params, 16), 0x0403_0201);
        assert_eq!(get32(&a.params, 24), 0x0807_0605);
        assert_eq!((get32(&a.params, 28), get32(&a.params, 32)), (0x2a, 2));
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &[(NV20_SUBDEVICE_0, CMD_GPU_GET_INFO_V2)]
        );

        let a = answer(
            chip(),
            CMD_GPU_GET_CHIP_DETAILS,
            NV20_SUBDEVICE_0,
            &[0u8; 16],
            Some(&mut h),
            &profile(),
        )
        .unwrap();
        assert_eq!((get32(&a.params, 0), get32(&a.params, 8)), (0x1F02, 10));

        let mut p = vec![0u8; 1028];
        put32(&mut p, 0, 3);
        put32(&mut p, 4, 0x8); // TOTAL_RAM_SIZE
        put32(&mut p, 12, 0x16); // HEAP_FREE
        put32(&mut p, 20, 0xB); // BUS_WIDTH: host value kept
        let a = answer(
            chip(),
            CMD_FB_GET_INFO_V2,
            NV20_SUBDEVICE_0,
            &p,
            Some(&mut h),
            &profile(),
        )
        .unwrap();
        assert_eq!(get32(&a.params, 8), 4 << 20);
        assert_eq!(get32(&a.params, 16), 4 << 20);
        assert_eq!(get32(&a.params, 24), 8 << 20);
    }

    #[test]
    fn routes_by_class_and_list() {
        let fake = FakeHost::default();
        let seen = fake.seen.clone();
        let mut h: Box<dyn HostRm> = Box::new(fake);
        let pr = profile();
        let a = answer(
            chip(),
            0x0080_0294,
            NV01_DEVICE_0,
            &[0u8; 4],
            Some(&mut h),
            &pr,
        )
        .unwrap();
        assert_eq!(a.status, 0x57);
        let a = answer(
            chip(),
            0x2080_A612,
            NV20_SUBDEVICE_0,
            &[0u8; 54352],
            Some(&mut h),
            &pr,
        )
        .unwrap();
        assert!(a.label.starts_with("host (GSS legacy"));
        let a = answer(
            chip(),
            CMD_THERMAL_SYSTEM_EXECUTE_V2_PHYSICAL,
            NV20_SUBDEVICE_0,
            &[0u8; 1432],
            Some(&mut h),
            &pr,
        )
        .unwrap();
        assert_eq!(a.status, NV_OK);
        answer(
            chip(),
            CMD_BINAPI_0108,
            NV2081_BINAPI,
            &[0u8; 992],
            Some(&mut h),
            &pr,
        )
        .unwrap();
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            &[
                (NV01_DEVICE_0, 0x0080_0294),
                (NV20_SUBDEVICE_0, 0x2080_A612),
                (NV20_SUBDEVICE_0, CMD_THERMAL_SYSTEM_EXECUTE_V2),
                (NV2081_BINAPI, CMD_BINAPI_0108),
            ]
        );
        // Not ours: unknown legacy numbers stay unanswered so the trace shows them.
        assert!(answer(
            chip(),
            0x2080_A6FF,
            NV20_SUBDEVICE_0,
            &[0u8; 4],
            Some(&mut h),
            &pr
        )
        .is_none());
        assert!(answer(
            chip(),
            0x2080_0A36,
            NV20_SUBDEVICE_0,
            &[0u8; 88],
            Some(&mut h),
            &pr
        )
        .is_none());
    }

    /// The host-sized GSS legacy controls on both hosts, with the 610 and the 615
    /// guest's sizes: forwarded exactly when the sizes agree.
    #[test]
    fn host_sized_legacy_follows_the_host() {
        use crate::hostabi::{HOST_610, HOST_615};
        // (command, 610 guest's size, 615 guest's size); 0 = that guest never sends it.
        let rows = [
            (0x2080_A618u32, 20000usize, 20512usize),
            (0x2080_8536, 25552, 24532),
            (0x2080_A0F1, 0, 9216),
        ];
        for (hl, host_is_615) in [(&HOST_610, false), (&HOST_615, true)] {
            for (cmd, g610, g615) in rows {
                for (n, guest_is_615) in [(g610, false), (g615, true)] {
                    if n == 0 {
                        continue;
                    }
                    let fake = FakeHost::default();
                    let seen = fake.seen.clone();
                    let mut h: Box<dyn HostRm> = Box::new(fake);
                    let mut host: Host<'_> = Some(h.as_mut());
                    let a = host_sized_legacy(&mut host, hl, cmd, &vec![0u8; n]);
                    let what = format!("{cmd:#x} {n} bytes on host {}", hl.driver());
                    assert_eq!(a.params.len(), n, "{what}");
                    if guest_is_615 == host_is_615 {
                        // Forwarded: the fake host's status comes back.
                        assert_eq!(a.status, 0x57, "{what}");
                        assert_eq!(*seen.lock().unwrap(), [(NV20_SUBDEVICE_0, cmd)], "{what}");
                        assert!(a.label.starts_with("host (GSS legacy"), "{what}");
                    } else {
                        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED, "{what}");
                        assert!(seen.lock().unwrap().is_empty(), "{what}: sent to the host");
                        assert!(a.label.starts_with("synthesized (NOT_SUPPORTED"), "{what}");
                    }
                }
            }
        }
        // Through `answer`, with the default host selection (610.57.04 in the tests).
        let mut h: Box<dyn HostRm> = Box::new(FakeHost::default());
        let a = answer(chip(), 0x2080_A618, NV20_SUBDEVICE_0, &[0u8; 20000], Some(&mut h), &profile())
            .unwrap();
        assert_eq!(a.status, 0x57);
        let a = answer(chip(), 0x2080_A618, NV20_SUBDEVICE_0, &[0u8; 20512], Some(&mut h), &profile())
            .unwrap();
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
        // Neither host-sized row stays in the guest-ABI LEGACY list.
        for c in HOST_SIZED_LEGACY {
            assert!(!LEGACY.iter().any(|(l, _, _)| l == c), "{c:#x}");
        }
    }

    /// FB_GET_REMAPPED_BANKS is forwarded as-is: the same control and layout on both
    /// host drivers (610 and 615 ctrl2080fb.h, 8208 bytes in both bindings).
    #[test]
    fn remapped_banks_is_forwarded() {
        assert_eq!(
            size_of::<nvrm_sys::v610::NV2080_CTRL_FB_GET_REMAPPED_BANKS_PARAMS>(),
            size_of::<nvrm_sys::v615::NV2080_CTRL_FB_GET_REMAPPED_BANKS_PARAMS>()
        );
        assert_eq!(
            size_of::<nvrm_sys::v615::NV2080_CTRL_FB_GET_REMAPPED_BANKS_PARAMS>(),
            8208
        );
        let fake = FakeHost::default();
        let seen = fake.seen.clone();
        let mut h: Box<dyn HostRm> = Box::new(fake);
        let a = answer(chip(), 0x2080_1361, NV20_SUBDEVICE_0, &[0u8; 8208], Some(&mut h), &profile())
            .unwrap();
        assert_eq!(a.status, 0x57);
        assert_eq!(*seen.lock().unwrap(), [(NV20_SUBDEVICE_0, 0x2080_1361)]);
    }

    #[test]
    fn synthesized_without_host() {
        let pr = profile();
        let a = answer(
            chip(),
            CMD_INTERNAL_GPU_GET_SMC_MODE,
            NV20_SUBDEVICE_0,
            &[9u8; 4],
            None,
            &pr,
        )
        .unwrap();
        assert_eq!(
            (a.status, get32(&a.params, 0)),
            (NV_OK, SMC_MODE_UNSUPPORTED)
        );
        assert_eq!(a.label, "synthesized (SMC/MIG unsupported on TU106)");
        let a = answer(
            chip(),
            CMD_NVLINK_GET_PLATFORM_INFO,
            NV20_SUBDEVICE_0,
            &[7u8; 39],
            None,
            &pr,
        )
        .unwrap();
        assert!(a.params.iter().all(|b| *b == 0));
        let a = answer(
            chip(),
            0x90E7_0102,
            GF100_SUBDEVICE_INFOROM,
            &[0u8; 20],
            None,
            &pr,
        )
        .unwrap();
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
        let a = answer(chip(), 0x2080_2068, NV20_SUBDEVICE_0, &[0u8; 4], None, &pr).unwrap();
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
        assert!(a.label.contains("no host"));
    }
}
