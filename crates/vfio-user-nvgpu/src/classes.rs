// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Which `GSP_RM_ALLOC` classes the device refuses (docs/caraxes/gsp-plan/classes.md
//! §5, §8): fail closed, with the status GSP-RM's resource server gives, instead of
//! acknowledging every class.
//!
//! Paths are relative to the open-gpu-kernel-modules tree at tag 615.71.09.
//!
//! GSP-RM runs the same resource server as the kernel RM. A class outside the GPU's
//! class database is refused with `NV_ERR_NOT_SUPPORTED` for a GPU-parented class
//! (`resservResourceFactory`, src/nvidia/src/kernel/rmapi/alloc_free.c:1393-1398) and
//! with `NV_ERR_INVALID_CLASS` under a channel (the class and engine lookup of
//! src/nvidia/src/kernel/gpu/fifo/channel_descendant.c:185-233). The database is the
//! chip's (`[classes]` of chip.toml, from g_gpu_class_list.c). A stock kernel applies
//! the same check first, so a correct guest never sends such a class; the refusal keeps
//! a modified or confused guest from creating phantom objects.
//!
//! Some classes of the database are refused as well, because a GSP client never sends
//! them or because they reach host-global state (classes.md §3 and §8):
//!
//! - `NV01_ROOT_NON_PRIV` 0x0001 and `NV01_ROOT_CLIENT` 0x0041: the kernel sends the
//!   root of a client only as `NV01_ROOT` (src/nvidia/inc/kernel/vgpu/rpc.h:84-87,
//!   src/nvidia/src/kernel/vgpu/rpc.c:10985-10988);
//! - `NV50_P2P` 0x503B, `NVENC_SW_SESSION` 0xA0BC, `NVFBC_SW_SESSION` 0xA0BD: their
//!   constructor RPC is taken only in a vGPU guest (p2p_api.c:650, nvencsession.c:67,
//!   nvfbc_session.c:84);
//! - `GF100_PROFILER` 0x90CC, `MAXWELL_PROFILER_CONTEXT` 0xB1CC, `MAXWELL_PROFILER_DEVICE`
//!   0xB2CC, `G84_PERFBUFFER` 0x844C: the HWPM and PMA units of the physical card are
//!   host-global; refused until the device models a profiler.

use crate::chip::ChipData;
use crate::gspmem::{NV_ERR_INVALID_CLASS, NV_ERR_NOT_SUPPORTED};

/// Classes of the database the device refuses, with the status and the reason.
pub const REFUSED: &[(u32, u32, &str)] = &[
    (0x0001, NV_ERR_INVALID_CLASS, "NV01_ROOT_NON_PRIV never reaches GSP (rpc.h:84-87)"),
    (0x0041, NV_ERR_INVALID_CLASS, "NV01_ROOT_CLIENT never reaches GSP (rpc.h:84-87)"),
    (0x503B, NV_ERR_NOT_SUPPORTED, "NV50_P2P is sent by a vGPU guest only (p2p_api.c:650)"),
    (0xA0BC, NV_ERR_NOT_SUPPORTED, "NVENC_SW_SESSION is sent by a vGPU guest only (nvencsession.c:67)"),
    (0xA0BD, NV_ERR_NOT_SUPPORTED, "NVFBC_SW_SESSION is sent by a vGPU guest only (nvfbc_session.c:84)"),
    (0x90CC, NV_ERR_NOT_SUPPORTED, "GF100_PROFILER: host-global HWPM, no profiler model"),
    (0xB1CC, NV_ERR_NOT_SUPPORTED, "MAXWELL_PROFILER_CONTEXT: host-global HWPM/PMA, no profiler model"),
    (0xB2CC, NV_ERR_NOT_SUPPORTED, "MAXWELL_PROFILER_DEVICE: host-global HWPM/PMA, no profiler model"),
    (0x844C, NV_ERR_NOT_SUPPORTED, "G84_PERFBUFFER: host-global perf buffer, no profiler model"),
];

/// The refusal of `class` on `chip`, if any: (status, reason). `under_channel` says
/// whether the parent is a channel the device knows (a channel descendant).
pub fn refusal(chip: &ChipData, class: u32, under_channel: bool) -> Option<(u32, String)> {
    if let Some(&(_, st, why)) = REFUSED.iter().find(|(c, _, _)| *c == class) {
        return Some((st, format!("class {class:#x} refused {st:#x}: {why}")));
    }
    let db = chip.classes.as_ref()?;
    if db.ids.contains(&class) {
        return None;
    }
    let (st, what) = if under_channel {
        (NV_ERR_INVALID_CLASS, "channel descendant, channel_descendant.c:185-233")
    } else {
        (NV_ERR_NOT_SUPPORTED, "resservResourceFactory, alloc_free.c:1393-1398")
    };
    Some((
        st,
        format!(
            "class {class:#x} refused {st:#x}: not in the {} class database ({what})",
            chip.short_name
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tu106() -> &'static ChipData {
        crate::fixture::rtx2070().chip.data()
    }

    /// Every class a stock TU106 Linux stack allocates (classes.md §6, "TU106 Linux
    /// client allocates") plus the two the 615 event import allocates stays accepted.
    #[test]
    fn the_stock_stack_keeps_its_classes() {
        const STOCK: &[u32] = &[
            0x0000, 0x0080, 0x2080, 0x2081, 0x208F, 0x83DE, 0xC46F, 0xC574, 0x9067, 0xA06C,
            0x0070, 0x90F1, 0x90E6, 0x90E7, 0x402C, 0x9096, 0x0004, 0x90EC, 0xC570, 0xC372,
            0x0073, 0xC57A, 0xC57B, 0xC57D, 0xC57E, 0x9072, 0x9074, 0x5080, 0xC076, 0xC5B5,
            0xC4B0, 0xC4B7, 0xC597, 0xC5C0, 0x902D, 0xA140, 0x0005, 0x0078, 0x0079, 0x007E,
            0x2082, 0x90CD, 0x90D0,
        ];
        for &c in STOCK {
            assert_eq!(refusal(tu106(), c, false), None, "{c:#x}");
            assert_eq!(refusal(tu106(), c, true), None, "{c:#x} under a channel");
        }
        assert_eq!(tu106().classes.as_ref().unwrap().ids.len(), 101);
    }

    #[test]
    fn foreign_and_host_global_classes_are_refused() {
        // AMPERE_COMPUTE_A under a channel, AMPERE_SMC_PARTITION_REF under the subdevice.
        assert_eq!(refusal(tu106(), 0xC6C0, true).unwrap().0, NV_ERR_INVALID_CLASS);
        assert_eq!(refusal(tu106(), 0xC637, false).unwrap().0, NV_ERR_NOT_SUPPORTED);
        // A Blackwell channel, a Hopper copy engine.
        assert_eq!(refusal(tu106(), 0xCA6F, false).unwrap().0, NV_ERR_NOT_SUPPORTED);
        assert_eq!(refusal(tu106(), 0xC8B5, true).unwrap().0, NV_ERR_INVALID_CLASS);
        assert_eq!(refusal(tu106(), 0xB2CC, false).unwrap().0, NV_ERR_NOT_SUPPORTED);
        assert_eq!(refusal(tu106(), 0x0041, false).unwrap().0, NV_ERR_INVALID_CLASS);
    }
}
