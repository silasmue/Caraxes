// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The RM controls `nvidia-uvm` causes while it registers a GPU (experiment E5a,
//! docs/caraxes/uvm.md; the call list is docs/caraxes/cuda-path.md §1.2). Each
//! answer carries its provenance label (decision D9). The channel, TSG and VAS
//! allocations themselves are recorded in [`crate::channel::Registry`].

use crate::channel::Registry;
use crate::chip::{ChipData, Registers};
use crate::ctrl::{Answer, HostRm};
use crate::ver::active::{self as v, NV20_SUBDEVICE_0, NV_ERR_NOT_SUPPORTED, NV_OK};
pub use crate::ver::active::{
    CMD_CE_GET_ALL_CAPS, CMD_CE_GET_ALL_PHYSICAL_CAPS, CMD_CE_GET_CAPS_V2, CMD_CE_GET_CE_PCE_MASK,
    CMD_CE_GET_PHYSICAL_CAPS, CMD_CE_IS_DECOMP_LCE_ENABLED, CMD_CHANNEL_GPFIFO_SCHEDULE,
    CMD_DMA_SET_PAGE_DIRECTORY, CMD_DMA_UNSET_PAGE_DIRECTORY,
    CMD_INTERNAL_GMMU_REGISTER_CLIENT_SHADOW_FAULT_BUFFER, CMD_INTERNAL_GMMU_REGISTER_FAULT_BUFFER,
    CMD_INTERNAL_GMMU_UNREGISTER_CLIENT_SHADOW_FAULT_BUFFER,
    CMD_INTERNAL_GMMU_UNREGISTER_FAULT_BUFFER, CMD_INTERNAL_UVM_REGISTER_ACCESS_CNTR_BUFFER,
    CMD_INTERNAL_UVM_UNREGISTER_ACCESS_CNTR_BUFFER, CMD_TSG_GPFIFO_SCHEDULE,
    CMD_TSG_INTERNAL_PROMOTE_FAULT_METHOD_BUFFERS, CMD_VASPACE_COPY_SERVER_RESERVED_PDES,
};

/// The fault-buffer and access-counter pointer registers UVM reads and writes
/// directly (the chip's `registers`; on TU106 BAR0 0xB80000 + `tu102/dev_vm.h:74-113`,
/// `:207-222`), and the usermode timer. No fault or notification is ever produced, so
/// PUT stays 0; GET keeps its PTR field and the write-1-to-clear status bits
/// (GETPTR_CORRUPTED 30, OVERFLOW 31) read as 0.
#[derive(Debug)]
pub struct VfRegs {
    r: Registers,
    /// The entry count `ACCESS_COUNTER_NOTIFY_BUFFER_SIZE` reports: the same entries
    /// as the fault buffers of the GMMU static info (ctrl.rs, row 19); 0 makes
    /// `nvUvmInterfaceInitAccessCntrInfo` fail registration (`uvm_tu102.c:264-274`).
    access_counter_entries: u32,
    fault_get: [u32; 2],
    access_counter_get: u32,
}

impl VfRegs {
    pub fn new(chip: &ChipData) -> Self {
        VfRegs {
            r: chip.registers,
            access_counter_entries: chip.gmmu.fault_buffer_entries,
            fault_get: [0; 2],
            access_counter_get: 0,
        }
    }

    pub fn handles(&self, off: u64) -> bool {
        let r = &self.r;
        r.fault_buffer_get.contains(&off)
            || r.fault_buffer_put.contains(&off)
            || [
                r.access_counter_notify_buffer_size,
                r.access_counter_get,
                r.access_counter_put,
                r.access_counter_info,
                r.usermode_time_0,
                r.usermode_time_1,
            ]
            .contains(&off)
    }

    /// `now_ns` is the device clock (the one the channel emulation stamps
    /// semaphores with): TIME_0 NSEC 31:5, TIME_1 NSEC 28:0. Synthesized.
    pub fn read32(&self, off: u64, now_ns: u64) -> u32 {
        let r = &self.r;
        match off {
            o if o == r.access_counter_notify_buffer_size => self.access_counter_entries,
            o if o == r.access_counter_get => self.access_counter_get,
            o if o == r.usermode_time_0 => now_ns as u32 & !0x1F,
            o if o == r.usermode_time_1 => (now_ns >> 32) as u32 & 0x1FFF_FFFF,
            o if o == r.fault_buffer_get[0] => self.fault_get[0],
            o if o == r.fault_buffer_get[1] => self.fault_get[1],
            _ => 0,
        }
    }

    pub fn write32(&mut self, off: u64, v: u32) -> Option<String> {
        let r = self.r;
        match off {
            o if o == r.access_counter_get => self.access_counter_get = v & 0xF_FFFF,
            o if o == r.fault_buffer_get[0] => self.fault_get[0] = v & 0xF_FFFF,
            o if o == r.fault_buffer_get[1] => self.fault_get[1] = v & 0xF_FFFF,
            o => return Some(format!("write {v:#x} to read-only {o:#x} dropped")),
        }
        None
    }
}

/// The UVM buffers the guest registered, as it described them (label `guest`).
#[derive(Default, Debug)]
pub struct UvmBuffers {
    /// Replayable fault buffer: size and the first PTE of its page array.
    pub fault_buffer: Option<(u32, u64)>,
    /// Client shadow fault buffer (non-replayable): queue PA and size.
    pub shadow_fault_buffer: Option<(u64, u32)>,
    /// Access counter buffer: size and the first PTE.
    pub access_counter_buffer: Option<(u32, u64)>,
}

fn rd32(b: &[u8], off: usize) -> u32 {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .unwrap_or(0)
}

fn rd64(b: &[u8], off: usize) -> u64 {
    b.get(off..off + 8)
        .map(|s| u64::from_le_bytes(s.try_into().unwrap()))
        .unwrap_or(0)
}

fn forward(
    host: Option<&mut Box<dyn HostRm>>,
    host_cmd: u32,
    params: &[u8],
    what: &str,
    label: &'static str,
) -> Answer {
    match host {
        Some(h) => {
            let mut b = params.to_vec();
            let status = h.control(NV20_SUBDEVICE_0, host_cmd, &mut b);
            Answer {
                status,
                params: b,
                label,
                note: format!("{what}: host {host_cmd:#x} status {status:#x}"),
            }
        }
        None => Answer {
            status: NV_ERR_NOT_SUPPORTED,
            params: params.to_vec(),
            label: "synthesized fallback (no host RM)",
            note: format!("{what}: no host, not supported"),
        },
    }
}

/// Answer a UVM-related control, or `None` when `cmd` is not one of them.
#[allow(clippy::too_many_arguments)]
pub fn answer(
    chip: &'static ChipData,
    cmd: u32,
    h_client: u32,
    h_object: u32,
    params: &[u8],
    channels: &mut Registry,
    buffers: &mut UvmBuffers,
    host: Option<&mut Box<dyn HostRm>>,
) -> Option<Answer> {
    let ok = |label: &'static str, note: String| Answer {
        status: NV_OK,
        params: params.to_vec(),
        label,
        note,
    };
    Some(match cmd {
        // cuda-path.md §1.2 row 6: the PTE array of a guest-sysmem buffer
        // (ctrl2080internal.h:1813-1830: hClient @0, hObject @4, faultBufferSize @8,
        // faultBufferPteArray @16). Real GSP programs the hardware fault buffer
        // from it; nothing here ever writes a fault entry.
        CMD_INTERNAL_GMMU_REGISTER_FAULT_BUFFER => {
            let (size, pte0) = (
                rd32(params, v::FAULT_BUFFER_SIZE),
                rd64(params, v::FAULT_BUFFER_PTE_ARRAY),
            );
            buffers.fault_buffer = Some((size, pte0));
            ok(
                "guest (buffer recorded; no fault is ever reported)",
                format!("register replayable fault buffer: size {size:#x} pte[0] {pte0:#x}"),
            )
        }
        CMD_INTERNAL_GMMU_UNREGISTER_FAULT_BUFFER => {
            buffers.fault_buffer = None;
            ok("guest", "unregister replayable fault buffer".into())
        }
        // Row 6, second half (ctrl2080internal.h:1863-1880): queue PA @0, size @8.
        CMD_INTERNAL_GMMU_REGISTER_CLIENT_SHADOW_FAULT_BUFFER => {
            let (pa, size) = (
                rd64(params, v::SHADOW_FAULT_BUFFER_QUEUE_PA),
                rd32(params, v::SHADOW_FAULT_BUFFER_SIZE),
            );
            buffers.shadow_fault_buffer = Some((pa, size));
            ok(
                "guest (shadow buffer recorded; MMU_FAULT_QUEUED is never sent)",
                format!("register client shadow fault buffer: queue pa {pa:#x} size {size:#x}"),
            )
        }
        CMD_INTERNAL_GMMU_UNREGISTER_CLIENT_SHADOW_FAULT_BUFFER => {
            buffers.shadow_fault_buffer = None;
            ok("guest", "unregister client shadow fault buffer".into())
        }
        // Row 9 (ctrl2080internal.h:160-176): accessCounterIndex @0, bufferSize @4,
        // bufferPteArray @8. Counters stay disabled on x86 by default
        // (uvm_gpu_access_counters.c:69).
        CMD_INTERNAL_UVM_REGISTER_ACCESS_CNTR_BUFFER => {
            let (size, pte0) = (
                rd32(params, v::ACCESS_CNTR_BUFFER_SIZE),
                rd64(params, v::ACCESS_CNTR_PTE_ARRAY),
            );
            buffers.access_counter_buffer = Some((size, pte0));
            ok(
                "guest (buffer recorded; no notification is ever written)",
                format!(
                    "register access counter buffer {}: size {size:#x} pte[0] {pte0:#x}",
                    rd32(params, v::ACCESS_CNTR_INDEX)
                ),
            )
        }
        CMD_INTERNAL_UVM_UNREGISTER_ACCESS_CNTR_BUFFER => {
            buffers.access_counter_buffer = None;
            ok("guest", "unregister access counter buffer".into())
        }
        // Row 11: CE_GET_PHYSICAL_CAPS has the CE_GET_CAPS_V2 layout
        // {ceEngineType u32, capsTbl[2]} (kernel_ce.c:556-575, ctrl2080ce.h:82-85);
        // the host answers its public twin for the same engine type.
        CMD_CE_GET_PHYSICAL_CAPS => {
            let mut a = forward(
                host,
                CMD_CE_GET_CAPS_V2,
                params,
                &format!(
                    "ce physical caps for engine type {:#x}",
                    rd32(params, v::CE_CAPS_ENGINE_TYPE)
                ),
                "host (CE_GET_CAPS_V2 0x20802a03 of the same engine type, same layout)",
            );
            if a.params.len() >= v::CE_CAPS_TBL + 2 {
                a.note.push_str(&format!(
                    ", caps {:#04x} {:#04x}",
                    a.params[v::CE_CAPS_TBL],
                    a.params[v::CE_CAPS_TBL + 1]
                ));
            }
            a
        }
        // cuInit (2026-09-23): the INTERNAL all-CE twin, answered by the host's public
        // CE_GET_ALL_CAPS (same params type, ctrl2080ce.h:348; host-facts.md H6).
        CMD_CE_GET_ALL_PHYSICAL_CAPS => forward(
            host,
            CMD_CE_GET_ALL_CAPS,
            params,
            "ce all physical caps",
            "host (CE_GET_ALL_CAPS 0x20802a0a, same layout)",
        ),
        // Row 11, routed half (ctrl2080ce.h:171-180; flags 0x30349, public).
        CMD_CE_GET_CE_PCE_MASK => forward(
            host,
            CMD_CE_GET_CE_PCE_MASK,
            params,
            &format!(
                "ce pce mask for engine type {:#x}",
                rd32(params, v::CE_CAPS_ENGINE_TYPE)
            ),
            "host (forwarded as-is, public control)",
        ),
        // kceIsDecompLce (kernel_ce.c:1172-1197) asks once per LCE; privileged
        // (flags 0x154), so not forwardable. {lceIndex u32, bDecompEnabled u8 @4}
        // (ctrl2080ce.h:513-520). Whether the chip has one is chip data.
        CMD_CE_IS_DECOMP_LCE_ENABLED => {
            let mut b = params.to_vec();
            if b.len() > v::DECOMP_ENABLED {
                b[v::DECOMP_ENABLED] = chip.ce.decomp_lce as u8;
            }
            Answer {
                status: NV_OK,
                params: b,
                label: &chip.ce.decomp_label,
                note: format!(
                    "ce lce {} decomp enabled: {}",
                    rd32(params, v::DECOMP_LCE_INDEX),
                    chip.ce.decomp_lce
                ),
            }
        }
        CMD_VASPACE_COPY_SERVER_RESERVED_PDES => ok(
            "guest (VAS root recorded; the server-reserved range stays empty)",
            channels.record_server_reserved_pdes(h_client, h_object, params),
        ),
        CMD_DMA_SET_PAGE_DIRECTORY => ok(
            "guest (VAS root recorded for the channel emulation)",
            channels.record_set_page_directory(h_client, params),
        ),
        CMD_DMA_UNSET_PAGE_DIRECTORY => ok(
            "guest",
            channels.record_unset_page_directory(h_client, params),
        ),
        // cuda-path.md §1.2 row 12: scheduling has no effect here, because the
        // device runs a channel's work on its doorbell.
        CMD_CHANNEL_GPFIFO_SCHEDULE | CMD_TSG_GPFIFO_SCHEDULE => ok(
            "synthesized (the emulated channel runs on its doorbell)",
            format!(
                "gpfifo schedule {h_object:#x}: bEnable {}",
                params.first().copied().unwrap_or(0)
            ),
        ),
        // kernel_channel_group_api.c:426-505: guest-sysmem method buffers.
        CMD_TSG_INTERNAL_PROMOTE_FAULT_METHOD_BUFFERS => ok(
            "guest (recorded only; no CE fault is ever replayed)",
            format!("promote fault method buffers on tsg {h_object:#x}"),
        ),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_registers() {
        let chip = crate::fixture::rtx2070().chip.data();
        let (time_0, time_1) = (
            chip.registers.usermode_time_0,
            chip.registers.usermode_time_1,
        );
        let mut r = VfRegs::new(chip);
        assert!(r.handles(0x00B8_3028) && r.handles(time_1));
        assert!(!r.handles(0x00B8_30B0));
        assert_eq!(r.read32(0x00B8_3110, 0), 256);
        // UVM clears the status bits with the pointer (uvm_turing_fault_buffer.c:85-100).
        assert_eq!(r.write32(0x00B8_3028, 0xC000_0000), None);
        assert_eq!(r.read32(0x00B8_3028, 0), 0);
        r.write32(0x00B8_3028, 5);
        assert_eq!(r.read32(0x00B8_3028, 0), 5);
        assert!(r.write32(0x00B8_302C, 7).is_some());
        assert_eq!(r.read32(0x00B8_302C, 0), 0);
        let t = 0x1_2345_6789u64;
        assert_eq!(r.read32(time_0, t), 0x2345_6780);
        assert_eq!(r.read32(time_1, t), 1);
    }

    #[test]
    fn controls_record_and_answer() {
        let chip = crate::fixture::rtx2070().chip.data();
        let mut reg = Registry::new(chip.channel);
        let mut buf = UvmBuffers::default();
        let mut p = vec![0u8; 32];
        p[0..8].copy_from_slice(&0x20_0000u64.to_le_bytes());
        p[12..16].copy_from_slice(&8u32.to_le_bytes());
        p[16..20].copy_from_slice(&0x55u32.to_le_bytes());
        let a = answer(
            chip,
            CMD_DMA_SET_PAGE_DIRECTORY,
            1,
            2,
            &p,
            &mut reg,
            &mut buf,
            None,
        )
        .unwrap();
        assert_eq!(a.status, NV_OK);
        assert_eq!(
            reg.vas_root((1, 0x55)),
            Some(crate::fb::Target::Fb(0x20_0000))
        );
        let mut f = vec![0u8; 2064];
        f[8..12].copy_from_slice(&0x2000u32.to_le_bytes());
        answer(
            chip,
            CMD_INTERNAL_GMMU_REGISTER_FAULT_BUFFER,
            1,
            2,
            &f,
            &mut reg,
            &mut buf,
            None,
        );
        assert_eq!(buf.fault_buffer, Some((0x2000, 0)));
        let a = answer(
            chip,
            CMD_CE_IS_DECOMP_LCE_ENABLED,
            1,
            2,
            &[2, 0, 0, 0, 1, 0, 0, 0],
            &mut reg,
            &mut buf,
            None,
        )
        .unwrap();
        assert_eq!(a.params[4], 0);
        assert_eq!(a.label, "synthesized (no decompression LCE on Turing)");
        let a = answer(
            chip,
            CMD_CE_GET_PHYSICAL_CAPS,
            1,
            2,
            &[9, 0, 0, 0, 0, 0, 0, 0],
            &mut reg,
            &mut buf,
            None,
        )
        .unwrap();
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
        assert!(answer(chip, 0x2080_0BAD, 1, 2, &[], &mut reg, &mut buf, None).is_none());
    }
}
