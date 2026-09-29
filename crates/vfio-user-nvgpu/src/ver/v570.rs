// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Driver 570.144's GSP boot vocabulary: what nova-core of Linux 7.2.7 speaks
//! (`drivers/gpu/nova-core/gsp/fw/r570_144.rs`, `FIRMWARE_VERSION = "570.144"`), as far as
//! the device answers it (docs/caraxes/nova-guest.md). Not a full version module: nova sends
//! no RM alloc or control, so only the boot exchange differs from the 610 module. The
//! device serves it when the guest's first queue element has the pre-610 frame, or under
//! `CARAXES_GUEST_ABI=570` ([`crate::ver::GuestAbi::V570`], [`crate::ver::older_guest`]):
//!
//! - the queue element: the pre-610 layout (`super::ElementLayout::PRE_610`);
//! - the GSP_RUN_CPU_SEQUENCER event before GSP_INIT_DONE ([`crate::ver::link_events`]);
//! - `GspStaticConfigInfo` in the 570.144 layout ([`crate::ver::v570::static_info`], the generated table
//!   of `caraxes/tools/gsp-remap.py` over the snapshots `caraxes/abi/gsp-610.57.04` and
//!   `caraxes/abi/gsp-570.144`).
//!
//! Every function and event number nova uses is the same in 570.144 and 610.57.04
//! (caraxes/abi/delta-615.71.09-570.144.md), as are `GspFwWprMeta`, the libos arguments,
//! the registry table and `rpc_unloading_guest_driver_v1F_07`.

#[path = "v570/static_info.rs"]
pub mod layout;

pub const DRIVER_VERSION: &str = "570.144";

/// `NV_VGPU_MSG_EVENT_GSP_RUN_CPU_SEQUENCER` (570.144
/// `src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:252`); 610 keeps the number but removed
/// the payload struct.
pub const EV_GSP_RUN_CPU_SEQUENCER: u32 = 0x1002;

/// `rpc_run_cpu_sequencer_v17_00` (570.144 `src/nvidia/generated/g_rpc-structures.h:1414-1420`):
/// `bufferSizeDWord` at 0, `cmdIndex` at 4, `regSaveArea[8]` at 8, the command buffer from
/// 40 on.
pub const RUN_CPU_SEQUENCER_SIZE: usize = 40;
pub const SEQ_BUFFER_SIZE_DWORD: usize = 0;
pub const SEQ_CMD_INDEX: usize = 4;

/// `rpc_init_done_v17_00` is one `NvU32 not_used` in 570.144
/// (`g_rpc-structures.h:1544-1547`), one byte in 610.
pub const INIT_DONE_SIZE: usize = 4;

/// An empty CPU sequence: no command, `cmdIndex` 0. nova-core's sequencer stops at once
/// (`gsp/sequencer.rs:289-293` of Linux 7.2.7: `cmds_processed >= total_cmds`). Real r570
/// firmware on Turing sends a sequence with register writes and `CORE_RESUME`; nova does
/// not check its contents, and the emulated GSP needs none of them.
pub fn empty_sequence() -> Vec<u8> {
    vec![0u8; RUN_CPU_SEQUENCER_SIZE]
}

/// `GspStaticConfigInfo` the device built in the emulated (610.57.04) layout, remapped into
/// the 570.144 layout by the generated copy table. A buffer of another size is returned
/// padded or cut to the 570 size without remapping (not expected: the device always builds
/// the 610 layout).
pub fn static_info(info610: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; layout::SIZE];
    if info610.len() != layout::FROM_SIZE {
        let n = info610.len().min(out.len());
        out[..n].copy_from_slice(&info610[..n]);
        return out;
    }
    for &(from, to, len, _) in layout::COPY {
        out[to..to + len].copy_from_slice(&info610[from..from + len]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The offsets nova-core reads (Linux 7.2.7 `gsp/fw/commands.rs`, bindings of r570.144):
    /// `gpuNameString`; drm-rust-next also `bar1PdeBase` and the FB regions (48-byte
    /// entries). Pinned to the measured 570.144 layout, and the remap carries the 610 bytes
    /// there.
    #[test]
    fn static_info_lands_where_nova_reads_it() {
        assert_eq!(layout::SIZE, 1656);
        assert_eq!(layout::at::GPU_NAME_STRING, 1260);
        assert_eq!(layout::at::BAR1_PDE_BASE, 1536);
        assert_eq!(layout::at::FB_REGION_INFO_PARAMS, 344);
        let mut info = vec![0u8; layout::FROM_SIZE];
        // 610: gpuNameString at 1352, bar1PdeBase at 1504, numFBRegions at 320, regions at
        // 328 with a 56-byte stride (caraxes/abi/gsp-610.57.04/layouts.tsv).
        info[1352..1352 + 7].copy_from_slice(b"Leandro");
        info[1504..1512].copy_from_slice(&0x1234_5000u64.to_le_bytes());
        info[320..324].copy_from_slice(&2u32.to_le_bytes());
        for (i, base) in [(0usize, 0x100_0000u64), (1, 0x3000_0000)] {
            let e = 328 + i * 56;
            info[e..e + 8].copy_from_slice(&base.to_le_bytes());
            info[e + 8..e + 16].copy_from_slice(&(base + 0xfff).to_le_bytes());
            info[e + 29] = 1; // supportISO
        }
        let out = static_info(&info);
        assert_eq!(out.len(), 1656);
        assert_eq!(&out[1260..1268], b"Leandro\0");
        assert_eq!(out[1536..1544], 0x1234_5000u64.to_le_bytes());
        assert_eq!(out[344..348], 2u32.to_le_bytes());
        for (i, base) in [(0usize, 0x100_0000u64), (1, 0x3000_0000)] {
            let e = 352 + i * 48;
            assert_eq!(out[e..e + 8], base.to_le_bytes(), "region {i} base");
            assert_eq!(out[e + 8..e + 16], (base + 0xfff).to_le_bytes(), "region {i} limit");
            assert_eq!(out[e + 29], 1, "region {i} supportISO");
        }
    }

    #[test]
    fn the_570_link_events_put_the_sequencer_first() {
        let ev = super::super::link_events(Some(570));
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0].0, EV_GSP_RUN_CPU_SEQUENCER);
        assert_eq!(ev[0].1.len(), RUN_CPU_SEQUENCER_SIZE);
        assert!(ev[0].1.iter().all(|&b| b == 0), "cmdIndex 0: an empty sequence");
        assert_eq!(ev[1].0, 0x1001);
        assert_eq!(ev[1].1.len(), INIT_DONE_SIZE);
        let emulated = super::super::link_events(None);
        assert_eq!(emulated, vec![(0x1001, vec![0u8])]);
        assert_eq!(super::super::link_events(Some(595)), emulated);
    }
}
