// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The CUDA error and teardown paths of category D (docs/caraxes/gsp-plan/d-device-model.md
//! roadmap R2, groups 6.1, 6.8 and 6.10): the guest's channel set, the RC journal and the RC
//! reflection to the guest.
//!
//! * `FIFO_DISABLE_CHANNELS` (6.1): every (hClient, hChannel) of the list must be a channel
//!   of this guest (the registry of channel.rs), or the whole request is refused with
//!   NV_ERR_INVALID_CHANNEL and nothing happens. A channel that runs on a host mirror gets the
//!   per-channel `GPFIFO_SCHEDULE` (bEnable = !bDisable) on its own host channel object, and a
//!   disable with preemption a `PREEMPT` of its own host TSG: the same controls the guest may
//!   send on its channels today (ctrl.rs `MIRROR_FORWARDED`). Never a runlist, engine or
//!   subdevice-wide action on the host. A channel of the software executor only records the
//!   state: that executor runs a channel only at the guest's doorbell.
//! * `GPU_REPORT_NON_REPLAYABLE_FAULT` (6.10): the fault packet's instance block names the
//!   channel (recorded from the channel's allocation, `instanceMem`); an unknown one is
//!   refused with NV_ERR_INVALID_ARGUMENT. A known one is RC'd: its host mirror, if any, is
//!   taken off the host runlist (as above), and RC_TRIGGERED with OS_ERROR_LOG
//!   (ROBUST_CHANNEL_FIFO_ERROR_MMU_ERR_FLT) goes to the guest through the RC event path of
//!   events.rs (boot.rs `scan_channel_errors`).
//! * `INTERNAL_RC_WATCHDOG_TIMEOUT` (6.8): the guest kernel's watchdog saw its channel make no
//!   progress (kernel_rc_watchdog_callback.c:280-296, :357-374). The device RCs the guest's
//!   GR channels whose USERD GPGet stands behind GPPut at the next event scan (the channels
//!   the engine did not finish), each with ROBUST_CHANNEL_PREEMPTIVE_REMOVAL, the mirrored
//!   ones taken off the host runlist first. No engine is reset and no host channel but the
//!   guest's own mirrors is touched; the caller ignores the status. Today the guest boots with
//!   `RmRcWatchdog=0` (gmcapi.rs:804), so the control does not arrive.
//! * `INTERNAL_LOG_OOB_XID` (6.8): the guest's own Xid, journalled with a sanitized text;
//!   never sent to the host's OOB log. `RC_SET_CLEAN_ERROR_HISTORY` clears the journal.
//!
//! **The device's one RC journal** ([`RcState::journal`]): every RC the device raises (a
//! mirror channel's error notifier, boot.rs `scan_channel_errors`; the non-replayable fault
//! and the watchdog RCs of this module) and every Xid the guest logs lands here once; the C
//! rows `RC_GET_ERROR_COUNT` and `RC_GET_ERROR_V2` (public.rs) read it, and
//! `RC_SET_CLEAN_ERROR_HISTORY` clears it. On a 615 guest boot.rs also writes one
//! operational-event record (gspmem.rs) per RC the device raises.
//!
//! **The device's one set of scheduling flags** ([`RcState::disabled`],
//! [`RcState::usermode_disabled`], [`RcState::scheduling_disabled`]): written by
//! `FIFO_DISABLE_CHANNELS` here and by the C rows `FIFO_DISABLE_USERMODE_CHANNELS` and
//! `INTERNAL_FIFO_TOGGLE_ACTIVE_CHANNEL_SCHEDULING` (public.rs), read by
//! `NV208F FIFO_GET_CHANNEL_STATE` (public.rs) and by the software executor's doorbell path
//! (ctrl.rs `doorbell_chid`): a software channel that is not enabled runs nothing; its
//! doorbell is held ([`RcState::held`]) and replayed once the channel is enabled again.
//!
//! The Xid of a watchdog RC is the device's choice: no open source names what GSP-RM
//! reports for it, and PREEMPTIVE_REMOVAL (nverror.h) is the code of a channel taken away.

use std::collections::{BTreeMap, BTreeSet};

use nvrm_sys::RmAbi;

use crate::channel::{GpuMem, Registry};
use crate::ctrl::Answer;
use crate::fb::Target;
use crate::ver::common as v;
use crate::ver::Abi;

/// The guest's channel allocation offsets (the same in every guest ABI; hostabi.rs).
type GuestAbi = nvrm_sys::V610;

/// D9 labels.
pub const CHANNELS: &str =
    "guest (this VM's channels; host mirrors by their own channel objects only)";
pub const RC: &str = "guest (RC of this VM's channel; reflected as RC_TRIGGERED)";
pub const RC_LOG: &str = "guest (RC error log of this VM)";

/// USERD GPGet and GPPut (Volta and later; the offsets of channel.rs:50-51).
const USERD_GP_GET: u64 = 0x88;
const USERD_GP_PUT: u64 = 0x8C;
/// Journal entries kept (the oldest go first).
pub const RC_LOG_MAX: usize = 256;
/// `NVB069_FAULT_BUF_ENTRY_INST_APERTURE` 9:8, `_INST_LO` 31:12 of word 0, `_INST_HI` word 1
/// (class/clb069.h:35-40): VID_MEM 0, SYS_MEM_COHERENT 2, SYS_MEM_NONCOHERENT 3.
const INST_APERTURE_VID: u32 = 0;

/// The host mirror of a guest channel or TSG, by the guest's own handles.
pub trait MirrorOps {
    /// Issue `cmd` on the host object that mirrors `(h_client, h_object)`: (status, note),
    /// `None` when the object has no mirror.
    fn forward(
        &mut self,
        h_client: u32,
        h_object: u32,
        cmd: u32,
        params: &[u8],
    ) -> Option<(u32, String)>;
}

/// [`MirrorOps`] over the device's mirror (`Mirror::forward_control`).
pub struct MirrorRef<'a> {
    pub mirror: &'a mut crate::mirror::Mirror,
    pub reg: &'a Registry,
}

impl MirrorOps for MirrorRef<'_> {
    fn forward(
        &mut self,
        h_client: u32,
        h_object: u32,
        cmd: u32,
        params: &[u8],
    ) -> Option<(u32, String)> {
        self.mirror
            .forward_control(self.reg, h_client, h_object, cmd, params)
    }
}

/// One RC the device raised or the guest logged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RcRecord {
    pub chid: Option<u32>,
    pub xid: u32,
    pub text: String,
}

/// An RC waiting for the event path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RcRequest {
    Channel {
        chid: u32,
        engine_type: u32,
        xid: u32,
        text: String,
    },
    /// Pick the stalled GR channels when guest memory is at hand.
    Watchdog,
}

/// What boot.rs sends: OS_ERROR_LOG (when the guest logs RCs) and RC_TRIGGERED.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RcEvent {
    pub engine_type: u32,
    pub chid: u32,
    pub xid: u32,
    pub text: String,
}

/// Per VM.
#[derive(Default, Debug)]
pub struct RcState {
    /// The device's RC journal for this guest (at most [`RC_LOG_MAX`] records).
    pub journal: Vec<RcRecord>,
    pub pending: Vec<RcRequest>,
    /// Channels the guest disabled (FIFO_DISABLE_CHANNELS), by the registry's (hClient,
    /// hChannel) of the channel (duplicates resolved).
    pub disabled: BTreeSet<(u32, u32)>,
    /// `FIFO_DISABLE_USERMODE_CHANNELS`: the guest's user channels (those of a guest TSG)
    /// are off.
    pub usermode_disabled: bool,
    /// `INTERNAL_FIFO_TOGGLE_ACTIVE_CHANNEL_SCHEDULING`: every channel is off.
    pub scheduling_disabled: bool,
    /// Software channels rung while not enabled: run when enabled again (ctrl.rs
    /// `replay_held`).
    pub held: BTreeSet<u32>,
    /// `CARAXES_RC_SELFTEST=<seconds>`: a SIMULATED device RC record of no channel every so
    /// often once the guest can take it (boot.rs `rc_selftest`), for the journal probe
    /// (caraxes/tools/rc-journal-probe.sh). A guest user process cannot log an Xid itself:
    /// `INTERNAL_LOG_OOB_XID` is an INTERNAL control (flags 0xc0,
    /// g_subdevice_nvoc.c:6616-6628 of 610.57.04) the guest kernel sends only for its own
    /// Xids (gpu.c:7655-7703).
    pub selftest: Option<SelfTest>,
    /// The instance block of each channel allocation: (aperture VID/SYS, address).
    pub instances: BTreeMap<(u32, u32), (bool, u64)>,
}

fn rd(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4)
        .map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

fn rd64(b: &[u8], o: usize) -> u64 {
    b.get(o..o + 8)
        .map_or(0, |s| u64::from_le_bytes(s.try_into().unwrap()))
}

fn off(st: &str, path: &str) -> usize {
    Abi::active()
        .layout(st)
        .unwrap_or_else(|| panic!("{st}: no generated layout"))
        .resolve(path)
        .unwrap_or_else(|e| panic!("{e}"))
        .0
}

fn enum_(name: &str) -> u32 {
    Abi::active()
        .d_enum(name)
        .unwrap_or_else(|| panic!("{name}: no generated enum"))
}

fn ans(status: u32, params: &[u8], label: &'static str, note: String) -> Answer {
    Answer {
        status,
        params: params.to_vec(),
        label,
        note,
    }
}

/// The RC self-test schedule ([`RcState::selftest`]).
#[derive(Debug)]
pub struct SelfTest {
    pub period: std::time::Duration,
    pub last: Option<std::time::Instant>,
    pub sent: u32,
}

impl SelfTest {
    /// `CARAXES_RC_SELFTEST=<seconds>`, if set to a positive number.
    pub fn from_env() -> Option<Self> {
        let s: u64 = std::env::var("CARAXES_RC_SELFTEST").ok()?.parse().ok()?;
        (s > 0).then(|| SelfTest {
            period: std::time::Duration::from_secs(s),
            last: None,
            sent: 0,
        })
    }

    /// The number of the next record if one is due now.
    pub fn due(&mut self) -> Option<u32> {
        if self.last.is_some_and(|t| t.elapsed() < self.period) {
            return None;
        }
        self.last = Some(std::time::Instant::now());
        self.sent += 1;
        Some(self.sent)
    }
}

/// A user channel in the device's view: a channel of a guest TSG (the kernel's own
/// golden-context and WAR channels are bare, mirror.rs `eligible`).
pub fn usermode(reg: &Registry, c: &crate::channel::Channel) -> bool {
    reg.tsg((c.h_client, c.h_parent)).is_some()
}

impl RcState {
    fn push(&mut self, r: RcRecord) {
        if self.journal.len() >= RC_LOG_MAX {
            self.journal.remove(0);
        }
        self.journal.push(r);
    }

    /// An RC the device raised outside this module (boot.rs: a mirror channel's error
    /// notifier), into the one journal.
    pub fn record(&mut self, chid: Option<u32>, xid: u32, text: String) {
        self.push(RcRecord { chid, xid, text });
    }

    /// Whether any scheduling flag is set (the doorbell path skips the channel walk
    /// otherwise).
    pub fn any_disabled(&self) -> bool {
        self.scheduling_disabled || self.usermode_disabled || !self.disabled.is_empty()
    }

    /// Whether guest channel `c` is enabled in the device's view: not disabled by
    /// itself, by the usermode switch (a channel of a TSG) or by the device-wide toggle.
    pub fn channel_enabled(&self, reg: &Registry, c: &crate::channel::Channel) -> bool {
        !(self.scheduling_disabled
            || (self.usermode_disabled && usermode(reg, c))
            || self.disabled.contains(&(c.h_client, c.h_object)))
    }

    /// The chids of the guest's channels that are not enabled.
    pub fn disabled_chids(&self, reg: &Registry) -> BTreeSet<u32> {
        if !self.any_disabled() {
            return BTreeSet::new();
        }
        reg.channels()
            .filter(|c| !self.channel_enabled(reg, c))
            .map(|c| c.chid)
            .collect()
    }

    /// A channel allocation of the guest: remember its instance block (`instanceMem` of
    /// `NV_CHANNEL_ALLOC_PARAMS`), which a fault packet names.
    pub fn record_channel(&mut self, h_client: u32, h_object: u32, params: &[u8]) {
        let o = <GuestAbi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_instanceMem;
        let base = rd64(params, o + v::MEM_DESC_BASE);
        let vid = match rd(params, o + v::MEM_DESC_ADDRESS_SPACE) {
            v::ADDR_FBMEM => true,
            v::ADDR_SYSMEM => false,
            _ => return,
        };
        self.instances.insert((h_client, h_object), (vid, base));
    }

    /// A handle freed or reused: forget what was recorded for it.
    pub fn forget(&mut self, h_client: u32, h_object: u32) {
        self.instances.remove(&(h_client, h_object));
        self.disabled.remove(&(h_client, h_object));
        if h_client == h_object {
            self.disabled.retain(|(c, _)| *c != h_client);
        }
    }

    /// `NV2080_CTRL_CMD_FIFO_DISABLE_CHANNELS` (ctrl2080fifo.h:198-251).
    pub fn disable_channels(
        &mut self,
        reg: &Registry,
        mut mirror: Option<&mut (dyn MirrorOps + '_)>,
        params: &[u8],
    ) -> Answer {
        const P: &str = "NV2080_CTRL_FIFO_DISABLE_CHANNELS_PARAMS";
        let disable = params.get(off(P, "bDisable")).copied().unwrap_or(0) != 0;
        let only_sched = params
            .get(off(P, "bOnlyDisableScheduling"))
            .copied()
            .unwrap_or(0)
            != 0;
        let n = rd(params, off(P, "numChannels"));
        let max = Abi::active()
            .layout(P)
            .unwrap()
            .fields
            .iter()
            .find(|f| f.path == "hClientList[]")
            .unwrap()
            .dims[0]
            .1;
        if n as usize > max {
            return ans(
                crate::dmodel::status("NV_ERR_INVALID_ARGUMENT"),
                params,
                CHANNELS,
                format!("disable channels: {n} entries, at most {max}"),
            );
        }
        let mut chans = Vec::new();
        for i in 0..n as usize {
            let key = (
                rd(params, off(P, &format!("hClientList[{i}]"))),
                rd(params, off(P, &format!("hChannelList[{i}]"))),
            );
            let rkey = reg.resolve(key);
            match reg.channels().find(|c| (c.h_client, c.h_object) == rkey) {
                Some(c) => chans.push((rkey, c.chid, c.mirrored, c.h_parent)),
                None => return ans(
                    crate::dmodel::status("NV_ERR_INVALID_CHANNEL"),
                    params,
                    CHANNELS,
                    format!(
                        "disable channels: {:#x}/{:#x} is no channel of this guest; nothing done",
                        key.0, key.1
                    ),
                ),
            }
        }
        let mut notes = Vec::new();
        for (rkey, chid, mirrored, parent) in chans {
            if disable {
                self.disabled.insert(rkey);
            } else {
                self.disabled.remove(&rkey);
            }
            let m = mirror.as_deref_mut().filter(|_| mirrored);
            let Some(m) = m else {
                notes.push(format!(
                    "chid {chid}: {} (software executor: its doorbells are held while disabled)",
                    if disable { "disabled" } else { "enabled" }
                ));
                continue;
            };
            let sched = [(!disable) as u8, 0, 0];
            let s = m.forward(rkey.0, rkey.1, v::CMD_CHANNEL_GPFIFO_SCHEDULE, &sched);
            notes.push(format!(
                "chid {chid}: host mirror GPFIFO_SCHEDULE bEnable {}: {}",
                !disable as u8,
                s.map_or("no host channel yet".into(), |s| s.1)
            ));
            if disable && !only_sched && reg.tsg((rkey.0, parent)).is_some() {
                // NVA06C_CTRL_PREEMPT_PARAMS: bWait 1, bManualTimeout 0, timeoutUs 0.
                let p = [1u8, 0, 0, 0, 0, 0, 0, 0];
                if let Some(s) = m.forward(rkey.0, parent, v::CMD_TSG_PREEMPT, &p) {
                    notes.push(format!("chid {chid}: host mirror TSG preempt: {}", s.1));
                }
            }
        }
        ans(
            0,
            params,
            CHANNELS,
            format!(
                "disable channels: bDisable {} bOnlyDisableScheduling {}; {}",
                disable as u8,
                only_sched as u8,
                notes.join("; ")
            ),
        )
    }

    /// `NV2080_CTRL_CMD_GPU_REPORT_NON_REPLAYABLE_FAULT` (ctrl2080gpu.h:2874-2893).
    pub fn report_nr_fault(
        &mut self,
        reg: &Registry,
        mirror: Option<&mut (dyn MirrorOps + '_)>,
        params: &[u8],
    ) -> Answer {
        const P: &str = "NV2080_CTRL_GPU_REPORT_NON_REPLAYABLE_FAULT_PARAMS";
        let base = off(P, "faultPacket.data[0]");
        let w = |i: usize| rd(params, base + 4 * i);
        let vid = (w(0) >> 8) & 3 == INST_APERTURE_VID;
        let inst = ((w(1) as u64) << 32) | (w(0) & 0xFFFF_F000) as u64;
        let addr = ((w(3) as u64) << 32) | w(2) as u64;
        let key = self
            .instances
            .iter()
            .find(|(_, &(v2, a))| v2 == vid && a == inst)
            .map(|(k, _)| *k);
        let ch = key.and_then(|k| reg.channels().find(|c| (c.h_client, c.h_object) == k));
        let Some(ch) = ch else {
            return ans(
                crate::dmodel::status("NV_ERR_INVALID_ARGUMENT"),
                params,
                RC,
                format!("non-replayable fault: instance block {inst:#x} ({}) is no channel of this guest", if vid { "vidmem" } else { "sysmem" }),
            );
        };
        let (chid, eng, hc, ho, mirrored) = (
            ch.chid,
            ch.engine_type,
            ch.h_client,
            ch.h_object,
            ch.mirrored,
        );
        let xid = enum_("ROBUST_CHANNEL_FIFO_ERROR_MMU_ERR_FLT");
        let text =
            format!("Caraxes: channel {chid} non-replayable fault at {addr:#x} (reported by UVM)");
        let mut note = format!("non-replayable fault on chid {chid} ({hc:#x}/{ho:#x}) at {addr:#x}: RC queued (Xid {xid})");
        if let Some(m) = mirror.filter(|_| mirrored) {
            if let Some(s) = m.forward(hc, ho, v::CMD_CHANNEL_GPFIFO_SCHEDULE, &[0, 0, 0]) {
                note.push_str(&format!("; host mirror taken off the runlist: {}", s.1));
            }
        }
        self.pending.push(RcRequest::Channel {
            chid,
            engine_type: eng,
            xid,
            text: text.clone(),
        });
        self.push(RcRecord {
            chid: Some(chid),
            xid,
            text,
        });
        ans(0, params, RC, note)
    }

    /// The host refused a guest channel's mirror for memory (NO_MEMORY, or the host VRAM
    /// floor, hostfloor.rs): the channel is RC'd with ROBUST_CHANNEL_PREEMPTIVE_REMOVAL at
    /// the next event scan, so the guest's context fails with an error instead of waiting
    /// on a channel that never runs (docs/caraxes/host-resources.md).
    pub fn host_refused(&mut self, chid: u32, engine_type: u32) {
        let xid = enum_("ROBUST_CHANNEL_PREEMPTIVE_REMOVAL");
        let text = format!(
            "Caraxes: channel {chid} refused by the host (out of host VRAM or below the host VRAM floor)"
        );
        self.pending.push(RcRequest::Channel {
            chid,
            engine_type,
            xid,
            text: text.clone(),
        });
        self.push(RcRecord {
            chid: Some(chid),
            xid,
            text,
        });
    }

    /// `NV2080_CTRL_CMD_INTERNAL_RC_WATCHDOG_TIMEOUT` (ctrl2080internal.h:1660-1665).
    pub fn watchdog_timeout(&mut self, params: &[u8]) -> Answer {
        self.pending.push(RcRequest::Watchdog);
        ans(0, params, RC, "RC watchdog timeout: the stalled GR channels of this guest are RC'd at the next event scan".into())
    }

    /// `NV2080_CTRL_CMD_INTERNAL_LOG_OOB_XID` (ctrl2080internal.h:4722-4751): the guest's
    /// own Xid, journalled; answered as the plan's ACK.
    pub fn log_oob_xid(&mut self, params: &[u8]) -> Answer {
        const P: &str = "NV2080_CTRL_INTERNAL_LOG_OOB_XID_PARAMS";
        let xid = rd(params, off(P, "xid"));
        let cap = Abi::active()
            .layout(P)
            .unwrap()
            .fields
            .iter()
            .find(|f| f.path == "message[]")
            .unwrap()
            .dims[0]
            .1;
        let len = (rd(params, off(P, "len")) as usize).min(cap);
        let m0 = off(P, "message[0]");
        let raw = params.get(m0..m0 + len).unwrap_or(&[]);
        let text: String = raw
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| {
                if (0x20..0x7F).contains(&c) {
                    c as char
                } else {
                    '.'
                }
            })
            .collect();
        self.push(RcRecord {
            chid: None,
            xid,
            text: text.clone(),
        });
        ans(
            0,
            params,
            crate::dmodel::ACKNOWLEDGED,
            format!("guest xid {xid}: {text} (journalled; never sent to the host's OOB log)"),
        )
    }

    /// `NV2080_CTRL_CMD_RC_SET_CLEAN_ERROR_HISTORY` (ctrl2080rc.h): the journal is cleared.
    pub fn clean_history(&mut self, params: &[u8]) -> Answer {
        let n = self.journal.len();
        self.journal.clear();
        ans(
            0,
            params,
            RC_LOG,
            format!("RC error history: {n} journal entries cleared"),
        )
    }
}

/// The RCs the models queued, resolved against guest memory: the watchdog's stalled GR
/// channels are chosen here (their USERD is readable only with `gm`). Called by boot.rs on
/// the RC event path.
pub fn take(ctrl: &mut crate::ctrl::Dispatcher, gm: &GpuMem) -> Vec<RcEvent> {
    let pending = std::mem::take(&mut ctrl.dstate.rc.pending);
    let mut out = Vec::new();
    for r in pending {
        match r {
            RcRequest::Channel {
                chid,
                engine_type,
                xid,
                text,
            } => out.push(RcEvent {
                engine_type,
                chid,
                xid,
                text,
            }),
            RcRequest::Watchdog => {
                let xid = enum_("ROBUST_CHANNEL_PREEMPTIVE_REMOVAL");
                let stalled: Vec<_> = ctrl
                    .channels
                    .channels()
                    .filter(|c| c.engine_type == nvrm_sys::NV2080_ENGINE_TYPE_GR0)
                    .filter_map(|c| {
                        let u = c.userd?;
                        let at = |o: u64| match u {
                            Target::Fb(pa) => Some(Target::Fb(pa + o)),
                            Target::Sys(pa) => Some(Target::Sys(pa + o)),
                            _ => None,
                        };
                        let get = gm.read_u32(at(USERD_GP_GET)?)?;
                        let put = gm.read_u32(at(USERD_GP_PUT)?)?;
                        (get != put).then_some((
                            c.chid,
                            c.engine_type,
                            c.h_client,
                            c.h_object,
                            c.mirrored,
                            get,
                            put,
                        ))
                    })
                    .collect();
                for (chid, eng, hc, ho, mirrored, get, put) in stalled {
                    let mut text = format!("Caraxes: channel {chid} stalled at GPGet {get} of GPPut {put} (RC watchdog)");
                    if mirrored {
                        if let Some(m) = ctrl.mirror.as_mut() {
                            if let Some((st, _)) = m.forward_control(
                                &ctrl.channels,
                                hc,
                                ho,
                                v::CMD_CHANNEL_GPFIFO_SCHEDULE,
                                &[0, 0, 0],
                            ) {
                                text.push_str(&format!(
                                    "; host mirror off the runlist, status {st:#x}"
                                ));
                            }
                        }
                    }
                    ctrl.dstate.rc.push(RcRecord {
                        chid: Some(chid),
                        xid,
                        text: text.clone(),
                    });
                    out.push(RcEvent {
                        engine_type: eng,
                        chid,
                        xid,
                        text,
                    });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ver::common as v;

    /// Records what reached the host mirrors.
    #[derive(Default)]
    struct Fake(Vec<(u32, u32, u32, Vec<u8>)>);
    impl MirrorOps for Fake {
        fn forward(&mut self, hc: u32, ho: u32, cmd: u32, p: &[u8]) -> Option<(u32, String)> {
            self.0.push((hc, ho, cmd, p.to_vec()));
            Some((0, "fake host".into()))
        }
    }

    fn registry_with_channel(mirrored: bool) -> Registry {
        let chip = crate::chip::by_name("tu106").unwrap().data();
        let mut reg = Registry::new(chip.channel);
        // A TSG under the device, then a channel in it (the shapes of channel.rs tests).
        let tsg = vec![0u8; crate::ver::layouts().tsg_size];
        reg.record_alloc(0xC1, 0xD0, 0x7000, v::KEPLER_CHANNEL_GROUP_A, &tsg);
        let mut p = vec![0u8; 1024];
        let o = <GuestAbi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_instanceMem;
        p[o..o + 8].copy_from_slice(&0x1234_5000u64.to_le_bytes());
        p[o + v::MEM_DESC_ADDRESS_SPACE..o + v::MEM_DESC_ADDRESS_SPACE + 4]
            .copy_from_slice(&v::ADDR_FBMEM.to_le_bytes());
        reg.record_alloc(0xC1, 0x7000, 0x7001, chip.channel.gpfifo, &p);
        if mirrored {
            let chid = reg.channels().next().unwrap().chid;
            reg.channel_mut(chid).unwrap().mirrored = true;
        }
        reg
    }

    fn disable_params(disable: bool, list: &[(u32, u32)]) -> Vec<u8> {
        const P: &str = "NV2080_CTRL_FIFO_DISABLE_CHANNELS_PARAMS";
        let mut b = vec![0u8; Abi::active().layout(P).unwrap().size];
        b[off(P, "bDisable")] = disable as u8;
        b[off(P, "numChannels")..][..4].copy_from_slice(&(list.len() as u32).to_le_bytes());
        for (i, (c, h)) in list.iter().enumerate() {
            b[off(P, &format!("hClientList[{i}]"))..][..4].copy_from_slice(&c.to_le_bytes());
            b[off(P, &format!("hChannelList[{i}]"))..][..4].copy_from_slice(&h.to_le_bytes());
        }
        b
    }

    /// A handle that is no channel of this guest refuses the whole request, before any
    /// host control; the size is the generated 536 bytes.
    #[test]
    fn disable_channels_refuses_a_foreign_handle() {
        let reg = registry_with_channel(true);
        let mut s = RcState::default();
        let mut f = Fake::default();
        assert_eq!(
            Abi::active()
                .layout("NV2080_CTRL_FIFO_DISABLE_CHANNELS_PARAMS")
                .unwrap()
                .size,
            536
        );
        let p = disable_params(true, &[(0xC1, 0x7001), (0xC1, 0x9999)]);
        let a = s.disable_channels(&reg, Some(&mut f), &p);
        assert_eq!(a.status, crate::dmodel::status("NV_ERR_INVALID_CHANNEL"));
        assert!(f.0.is_empty() && s.disabled.is_empty());
    }

    /// A mirrored channel gets GPFIFO_SCHEDULE on its own host channel and a TSG preempt;
    /// enabling undoes the schedule; a software channel reaches no host object.
    #[test]
    fn disable_channels_acts_on_the_guests_own_mirrors_only() {
        let reg = registry_with_channel(true);
        let mut s = RcState::default();
        let mut f = Fake::default();
        let a = s.disable_channels(&reg, Some(&mut f), &disable_params(true, &[(0xC1, 0x7001)]));
        assert_eq!((a.status, a.label), (0, CHANNELS));
        assert_eq!(f.0.len(), 2, "{:?}", f.0);
        assert_eq!(
            (f.0[0].1, f.0[0].2, f.0[0].3[0]),
            (0x7001, v::CMD_CHANNEL_GPFIFO_SCHEDULE, 0)
        );
        assert_eq!((f.0[1].1, f.0[1].2), (0x7000, v::CMD_TSG_PREEMPT));
        assert!(s.disabled.contains(&(0xC1, 0x7001)));
        s.disable_channels(
            &reg,
            Some(&mut f),
            &disable_params(false, &[(0xC1, 0x7001)]),
        );
        assert_eq!((f.0[2].2, f.0[2].3[0]), (v::CMD_CHANNEL_GPFIFO_SCHEDULE, 1));
        assert!(s.disabled.is_empty());
        let reg = registry_with_channel(false);
        let mut f = Fake::default();
        let a = s.disable_channels(&reg, Some(&mut f), &disable_params(true, &[(0xC1, 0x7001)]));
        assert_eq!(a.status, 0);
        assert!(f.0.is_empty(), "software channel: no host control");
    }

    fn fault_packet(inst: u64, aperture: u32) -> Vec<u8> {
        let mut b = vec![0u8; 32];
        let w0 = (inst as u32 & 0xFFFF_F000) | (aperture << 8);
        b[0..4].copy_from_slice(&w0.to_le_bytes());
        b[4..8].copy_from_slice(&((inst >> 32) as u32).to_le_bytes());
        b[8..12].copy_from_slice(&0xDEAD_0000u32.to_le_bytes());
        b
    }

    /// The fault packet's instance block finds the channel; an unknown one is refused; the
    /// RC goes out as RC_TRIGGERED with Xid 31.
    #[test]
    fn nr_fault_rcs_the_guest_channel() {
        let reg = registry_with_channel(true);
        let mut s = RcState::default();
        let mut p = vec![0u8; 1024];
        let o = <GuestAbi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_instanceMem;
        p[o..o + 8].copy_from_slice(&0x1234_5000u64.to_le_bytes());
        p[o + v::MEM_DESC_ADDRESS_SPACE..o + v::MEM_DESC_ADDRESS_SPACE + 4]
            .copy_from_slice(&v::ADDR_FBMEM.to_le_bytes());
        s.record_channel(0xC1, 0x7001, &p);
        let mut f = Fake::default();
        let a = s.report_nr_fault(&reg, Some(&mut f), &fault_packet(0x9999_0000, 0));
        assert_eq!(a.status, crate::dmodel::status("NV_ERR_INVALID_ARGUMENT"));
        assert!(s.pending.is_empty() && f.0.is_empty());
        let a = s.report_nr_fault(&reg, Some(&mut f), &fault_packet(0x1234_5000, 0));
        assert_eq!((a.status, a.label), (0, RC));
        assert_eq!(f.0.len(), 1, "the mirror is taken off the runlist");
        let chid = reg.channels().next().unwrap().chid;
        assert!(
            matches!(&s.pending[..], [RcRequest::Channel { chid: c, xid: 31, .. }] if *c == chid)
        );
        // A sysmem instance block at the same address is another block.
        s.pending.clear();
        let a = s.report_nr_fault(&reg, None, &fault_packet(0x1234_5000, 2));
        assert_ne!(a.status, 0);
    }

    /// LOG_OOB_XID journals a sanitized text and acknowledges; the clean-history control
    /// empties the journal.
    #[test]
    fn oob_xid_journal() {
        const P: &str = "NV2080_CTRL_INTERNAL_LOG_OOB_XID_PARAMS";
        let mut s = RcState::default();
        let mut p = vec![0u8; Abi::active().layout(P).unwrap().size];
        assert_eq!(p.len(), 92);
        p[off(P, "xid")..][..4].copy_from_slice(&10u32.to_le_bytes());
        let m = b"vblank\x1b[31m timeout";
        p[off(P, "message[0]")..][..m.len()].copy_from_slice(m);
        p[off(P, "len")..][..4].copy_from_slice(&(m.len() as u32).to_le_bytes());
        let a = s.log_oob_xid(&p);
        assert_eq!((a.status, a.label), (0, crate::dmodel::ACKNOWLEDGED));
        assert_eq!(s.journal[0].text, "vblank.[31m timeout");
        assert_eq!(a.params, p);
        let a = s.clean_history(&[]);
        assert_eq!(a.status, 0);
        assert!(s.journal.is_empty());
    }
}
