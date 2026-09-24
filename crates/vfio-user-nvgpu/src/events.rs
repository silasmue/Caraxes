// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Messages GSP-RM sends unasked: RPC events and the GMCAPI requests of the GSP-to-CPU
//! direction (docs/caraxes/gsp-plan/events.md). The boot model sends them on the status
//! queue ([`crate::msgq::QueueState::send`] for an event,
//! [`crate::msgq::QueueState::send_gmcapi_request`] for a GMCAPI request) and raises
//! GSP IRQSTAT.SWGEN0 as `kgspService_TU102` expects for any message the GSP posts.
//!
//! Paths are relative to the open-gpu-kernel-modules tree at tag 615.71.09; "610:"
//! marks 610.57.04. Which events the device raises, and from what:
//!
//! | Event | Raised | Trigger |
//! |---|---|---|
//! | POST_EVENT 0x1003 | yes | the display model's hotplug (boot.rs `post_display_command`) |
//! | RC_TRIGGERED 0x1004, OS_ERROR_LOG 0x1006 | yes | the host RM's error write into the error notifier of a guest channel that runs on a host mirror: the notifier is the guest's own page (the mirror's `hObjectError`, mirror.rs), so only this device's channels can write it; one RC per channel until the guest frees it ([`ChannelWatch`]) |
//! | GSP_RESUME_DONE (GMCAPI 0x10005) | yes | the device's own resume (boot.rs, B5) |
//! | MMU_FAULT_QUEUED (GMCAPI 0x40001) | no | needs fault packets in the shadow buffer UVM registered; the software executor produces no faults and the host's fault buffer is not per client (events.md R3) |
//! | SEMAPHORE_SCHEDULE_CALLBACK 0x100b | no | needs GF100_DISP_SW (0x9072) methods executed at the display's vblank; the executor does not decode them (events.md R7) |
//! | TIMED_SEMAPHORE_RELEASE 0x1018 | no | needs GF100_TIMED_SEMAPHORE_SW (0x9074) methods executed against a device timer (R8) |
//! | GPUACCT_PERFMON_UTIL_SAMPLES 0x1008 | no | needs per-context busy accounting of the guest's own channels, which the device does not keep (R11) |
//! | FECS_ERROR 0x1021 | no | needs a FECS context-switch trace model; never the host's trace (R9) |
//! | UPDATE_GSP_TRACE 0x101e | no | the device is not GSP-RM and has nothing to trace (R12) |
//! | RECOVERY_ACTION 0x1022 | no | a contained channel error needs no bus reset or reboot; never derived from the host's recovery state (R5) |
//! | PMU_HALTED 0x1029 | no | the device has no PMU; the only honest trigger, a lost host GPU, is not a decision the device makes today (R4) |
//! | GSP_POST_NOCAT_RECORD 0x1020 | no | journal only, no Linux-visible effect (R10) |
//!
//! The host-global subdevice RC subscription (mirror.rs, `NV2080_NOTIFIERS_RC_ERROR`)
//! carries other clients' RCs (kernel_rc_notification.c:483-508, gpu_rmapi.c:649-651)
//! and is never a source: at most its counter makes the boot model look at its own
//! notifier pages sooner.

/// RPC event numbers (src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:255-301).
pub const EV_POST_EVENT: u32 = 0x1003;
pub const EV_RC_TRIGGERED: u32 = 0x1004;
pub const EV_OS_ERROR_LOG: u32 = 0x1006;

/// GMCAPI GSP-to-CPU commands (src/nvidia/interface/gmcapi/gmcapi_table.h:117-124,
/// 268-275).
pub const GMCAPI_GSP_RESUME_DONE: u32 = crate::gmcapi::CMD_GSP_RESUME_DONE;
pub const GMCAPI_MMU_FAULT_QUEUED: u32 = 0x0004_0001;

// rpc_rc_triggered_v17_02: 64 bytes in 615 (g_rpc-structures.h:1510-1528), 48 in 610
// (610: g_rpc-structures.h:1481-1496). The version name did not change; four fields
// were inserted before rcJournalBufferSize (events.md, "ABI finding").
pub const RC_ENGINE_TYPE: usize = 0;
pub const RC_CHID: usize = 4;
pub const RC_GFID: usize = 8;
pub const RC_EXCEPT_LEVEL: usize = 12;
pub const RC_EXCEPT_TYPE: usize = 16;
pub const RC_SCOPE: usize = 20;
pub const RC_PARTITION_ATTRIBUTION_ID: usize = 24;
pub const RC_MMU_FAULT_ADDR_LO: usize = 28;
pub const RC_MMU_FAULT_ADDR_HI: usize = 32;
pub const RC_MMU_FAULT_TYPE: usize = 36;
pub const RC_CALLBACK_NEEDED: usize = 40;

/// The per-version layout of `rpc_rc_triggered_v17_02`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RcLayout {
    pub size: usize,
    /// `rcChannelInfoType`, `instblkAddrLo/Hi`, `instblkAperture` (615 only).
    pub channel_info_type: Option<usize>,
    pub journal_size: usize,
}

pub const RC_LAYOUT_610: RcLayout = RcLayout { size: 48, channel_info_type: None, journal_size: 44 };
pub const RC_LAYOUT_615: RcLayout = RcLayout { size: 64, channel_info_type: Some(44), journal_size: 60 };

/// The layout of a guest ABI (`GspTransport::guest_abi`).
pub fn rc_layout(guest_abi: u32) -> RcLayout {
    if guest_abi >= 615 {
        RC_LAYOUT_615
    } else {
        RC_LAYOUT_610
    }
}

/// `RC_NOTIFIER_SCOPE_CHANNEL` (src/nvidia/generated/g_kernel_rc_nvoc.h:73).
pub const RC_SCOPE_CHANNEL: u32 = 0;
/// `RC_CHANNEL_INFO_TYPE_CHANNEL_INFO` (src/nvidia/inc/kernel/gpu/rc/rc_channel_info.h:31):
/// the kernel finds the channel by chid in the chid manager of the engine type
/// (kernel_gsp.c:737-749).
pub const RC_CHANNEL_INFO_TYPE_CHANNEL_INFO: u32 = 0;

fn put32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// An RC of a PF channel (gfid 0) with no journal: what `_kgspRpcRCTriggered`
/// (kernel_gsp.c:717-840; 610: :585-716) needs to find the channel, write its error
/// notifier (615) and wake its clients.
pub fn rc_triggered(layout: RcLayout, nv2080_engine_type: u32, chid: u32, except_type: u32) -> Vec<u8> {
    let mut b = vec![0u8; layout.size];
    put32(&mut b, RC_ENGINE_TYPE, nv2080_engine_type);
    put32(&mut b, RC_CHID, chid);
    put32(&mut b, RC_EXCEPT_TYPE, except_type);
    put32(&mut b, RC_SCOPE, RC_SCOPE_CHANNEL);
    if let Some(o) = layout.channel_info_type {
        put32(&mut b, o, RC_CHANNEL_INFO_TYPE_CHANNEL_INFO);
    }
    put32(&mut b, layout.journal_size, 0);
    b
}

// rpc_os_error_log_v17_00 (g_rpc-structures.h:1533-1540; 272 bytes in both).
pub const OS_ERROR_LOG_SIZE: usize = 272;
pub const OEL_EXCEPT_TYPE: usize = 0;
pub const OEL_RUNLIST_ID: usize = 8;
pub const OEL_CHID: usize = 12;
pub const OEL_ERR_STRING: usize = 16;
pub const OEL_ERR_STRING_LEN: usize = 0x100;

/// An Xid for the guest's log (`_kgspRpcOsErrorLog`, kernel_gsp.c:849-885, prints
/// "NVRM: Xid (PCI:...): N, pid=..., name=..., <errString>" through
/// kernel_rc.c:388-400). The string carries guest facts only, never a host Xid line.
pub fn os_error_log(except_type: u32, runlist_id: u32, chid: u32, text: &str) -> Vec<u8> {
    let mut b = vec![0u8; OS_ERROR_LOG_SIZE];
    put32(&mut b, OEL_EXCEPT_TYPE, except_type);
    put32(&mut b, OEL_RUNLIST_ID, runlist_id);
    put32(&mut b, OEL_CHID, chid);
    let t = text.as_bytes();
    let n = t.len().min(OEL_ERR_STRING_LEN - 1);
    b[OEL_ERR_STRING..OEL_ERR_STRING + n].copy_from_slice(&t[..n]);
    b
}

/// `NvNotification` (src/common/sdk/nvidia/inc/nvgputypes.h:57-64): timeStamp, info32,
/// info16, status. The error notifier is index 0 (`NV_CHANNELGPFIFO_NOTIFICATION_TYPE_ERROR`,
/// nvos.h:2855).
pub const NOTIFIER_SIZE: usize = 16;
pub const NOTIFIER_INFO32: usize = 8;
pub const NOTIFIER_INFO16: usize = 12;
pub const NOTIFIER_STATUS: usize = 14;

/// What the device knows about one mirrored channel's error notifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelWatch {
    /// The notifier as first seen: a write that changes it with a nonzero status is an
    /// error the host RM reported for this channel.
    pub baseline: [u8; NOTIFIER_SIZE],
    /// An RC went to the guest; nothing more until the guest frees the channel (after
    /// RC_TRIGGERED the 615 kernel writes the notifier itself, kernel_gsp.c:826).
    pub raised: bool,
}

/// The error the host wrote into a watched notifier, as (exceptType, info16), once.
pub fn notifier_error(w: &mut ChannelWatch, now: &[u8; NOTIFIER_SIZE]) -> Option<(u32, u16)> {
    let status = u16::from_le_bytes([now[NOTIFIER_STATUS], now[NOTIFIER_STATUS + 1]]);
    if w.raised || *now == w.baseline || status == 0 {
        return None;
    }
    w.raised = true;
    let info32 = u32::from_le_bytes(now[NOTIFIER_INFO32..NOTIFIER_INFO32 + 4].try_into().unwrap());
    let info16 = u16::from_le_bytes([now[NOTIFIER_INFO16], now[NOTIFIER_INFO16 + 1]]);
    Some((info32, info16))
}

/// `GmcApiGspResumeDoneParams` {u64 flags} (gmcapi_gsp.h:58-69): flags 0 unless the
/// resume was from D3hot (`GMCAPI_GSP_RESUME_DONE_FLAGS_D3HOT`, bit 0).
pub fn resume_done(d3hot: bool) -> Vec<u8> {
    (d3hot as u64).to_le_bytes().to_vec()
}

/// One unsolicited message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unsolicited {
    /// An RPC event: function number and payload.
    Event(u32, Vec<u8>),
    /// A GMCAPI request of the GSP-to-CPU direction: command id and parameters.
    Gmcapi(u32, Vec<u8>),
}

impl Unsolicited {
    pub fn name(&self) -> String {
        match self {
            Unsolicited::Event(EV_POST_EVENT, _) => "event POST_EVENT".into(),
            Unsolicited::Event(EV_RC_TRIGGERED, _) => "event RC_TRIGGERED".into(),
            Unsolicited::Event(EV_OS_ERROR_LOG, _) => "event OS_ERROR_LOG".into(),
            Unsolicited::Event(f, _) => format!("event {f:#x}"),
            Unsolicited::Gmcapi(GMCAPI_MMU_FAULT_QUEUED, _) => "gmcapi MMU_FAULT_QUEUED".into(),
            Unsolicited::Gmcapi(id, _) => format!("gmcapi {}", crate::gmcapi::command_name(*id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rc_triggered_per_layout() {
        let a = rc_triggered(rc_layout(610), 0x1, 7, 31);
        let b = rc_triggered(rc_layout(615), 0x1, 7, 31);
        assert_eq!((a.len(), b.len()), (48, 64));
        assert_eq!(&a[..44], &b[..44], "the shared prefix");
        assert_eq!(u32::from_le_bytes(b[RC_CHID..RC_CHID + 4].try_into().unwrap()), 7);
        assert_eq!(u32::from_le_bytes(b[RC_EXCEPT_TYPE..RC_EXCEPT_TYPE + 4].try_into().unwrap()), 31);
        let e = os_error_log(31, 0, 7, "MMU fault");
        assert_eq!(e.len(), OS_ERROR_LOG_SIZE);
        assert_eq!(&e[OEL_ERR_STRING..OEL_ERR_STRING + 9], b"MMU fault");
        assert_eq!(e[OS_ERROR_LOG_SIZE - 1], 0, "terminated");
        assert_eq!(resume_done(false), vec![0; 8]);
    }

    #[test]
    fn a_notifier_error_is_reported_once() {
        let mut w = ChannelWatch { baseline: [0; 16], raised: false };
        assert_eq!(notifier_error(&mut w, &[0; 16]), None);
        let mut n = [0u8; 16];
        n[NOTIFIER_INFO32] = 31;
        n[NOTIFIER_INFO16] = 1;
        n[NOTIFIER_STATUS] = 0xFF;
        n[NOTIFIER_STATUS + 1] = 0xFF;
        assert_eq!(notifier_error(&mut w, &n), Some((31, 1)));
        assert_eq!(notifier_error(&mut w, &n), None, "one RC per channel lifetime");
        // A notifier that already held an error when first seen is not reported.
        let mut w = ChannelWatch { baseline: n, raised: false };
        assert_eq!(notifier_error(&mut w, &n), None);
    }
}
