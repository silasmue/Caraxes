// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The table-driven public controls of the GSP plan (docs/caraxes/gsp-plan/
//! b-host-forwardable.md, c-guest-state.md): the rows of `ver::<version>::public::PUBLIC`.
//! Plain forwards stay in the `FORWARD` list (nvml.rs); this module answers the rest.
//!
//! * **Filtered forwards.** The control goes to the device's own host subdevice, and the
//!   answer is cut to what the guest may see before it leaves the device. The FB address
//!   filters keep only addresses inside the VM's VRAM reservation `[B, E)`; guest FB
//!   address X is host VRAM address X (docs/caraxes/memory-model.md §1), so a kept address
//!   is already the guest's FB offset and needs no rebasing. Without a reservation no
//!   host FB address is a guest address, and nothing address-shaped is passed on.
//!
//! * **Device answers.** Controls whose host answer is wrong for the guest are answered
//!   from the device's own model and never forwarded.
//! * **Refusals.** NOT_SUPPORTED with a label, the request echoed, nothing sent anywhere.
//! * **Guest objects** (the C plan). Controls on the guest's own channels and TSGs are
//!   recorded per object ([`GuestAttrs`]); the only host path is the object's own mirror
//!   ([`MirrorObjects`]), never the host RM client and never another handle.
//!
//! The filtered forwards call `HostRm::control` with the subdevice class, so a row can
//! never reach another host object (hostrm.rs).

use crate::ctrl::{Answer, HostRm};
use std::collections::BTreeMap;

use crate::ver::active::public::{self as p, Dev, Filter, Guest, PublicControl, Rule};
use crate::ver::active::{NV20_SUBDEVICE_0, NV_ERR_NOT_SUPPORTED, NV_OK};

/// `NV_ERR_INVALID_PARAM_STRUCT` (nvstatuscodes.h:87): a guest buffer of another size
/// than the version's struct is refused, never forwarded.
pub const NV_ERR_INVALID_PARAM_STRUCT: u32 = 0x3A;
// Further statuses the C rows answer (nvstatuscodes.h:60, 62, 80, 130).
pub const NV_ERR_INVALID_ARGUMENT: u32 = 0x1F;
pub const NV_ERR_INVALID_CHANNEL: u32 = 0x21;
pub const NV_ERR_INVALID_OBJECT_HANDLE: u32 = 0x33;
pub const NV_ERR_TIMEOUT: u32 = 0x65;

/// The guest channel `(h_client, h_channel)` in the registry.
fn channel_of(
    reg: Option<&crate::channel::Registry>,
    h_client: u32,
    h_channel: u32,
) -> Option<&crate::channel::Channel> {
    reg?.channels()
        .find(|c| c.h_client == h_client && c.h_object == h_channel)
}

/// What the rows need from the dispatcher.
pub struct Ctx<'a> {
    pub host: Option<&'a mut (dyn HostRm + 'static)>,
    /// The VM's VRAM reservation `[B, E)`, when the device holds one.
    pub reservation: Option<(u64, u64)>,
    /// The engines the device presents (the GPUMON filter zeroes the others).
    pub chip: &'static crate::chip::ChipData,
    /// The device clock, in ns (`channel::Registry::now_ns`).
    pub now_ns: u64,
    /// The guest object the control names (hClient, hObject), and its key in the
    /// attribute record (duplicates resolved, `channel::Registry::resolve`).
    pub object: (u32, u32),
    pub key: (u32, u32),
    /// The host mirrors of the guest's channels and TSGs, when the device executes on the
    /// physical GPU.
    pub mirror: Option<&'a mut dyn MirrorObjects>,
    /// What the guest set on its objects.
    pub attrs: &'a mut GuestAttrs,
    /// The device's RC journal and scheduling flags (rc.rs, shared with the D models).
    pub rc: &'a mut crate::rc::RcState,
    /// The guest's channels and TSGs (`channel::Registry`), for the rows that name a guest
    /// channel in their params.
    pub reg: Option<&'a crate::channel::Registry>,
    /// The emulated PCIe Link Control register (config.rs), for `NV208F BIF_INFO`.
    pub link_control: u16,
}

/// The host mirror of a guest channel or TSG, looked up by the guest's own handles. Every
/// method acts on the mirror's own host objects of the guest objects it is given, never on
/// another host handle; `None` means no object of the call has a mirror.
pub trait MirrorObjects {
    /// Issue `cmd` on the host object that mirrors the guest object `(h_client,
    /// h_object)`: (host status, note), or `None` when the object has no mirror.
    fn forward(&mut self, h_client: u32, h_object: u32, cmd: u32, params: &[u8])
        -> Option<(u32, String)>;
    /// The host RM's idle of the mirrored channels of `list` (mirror.rs `idle_channels`).
    fn idle(&mut self, _list: &[(u32, u32)], _flags: u32, _timeout_us: u32) -> Option<(u32, String)> {
        None
    }
    /// Disable or enable scheduling of the mirrored channels of `list` (mirror.rs
    /// `set_scheduling`).
    fn schedule(&mut self, _list: &[(u32, u32)], _disable: bool, _only: bool) -> Option<(u32, String)> {
        None
    }
    /// A subdevice getter naming a guest channel, on its mirror (mirror.rs
    /// `channel_query`).
    fn channel_query(
        &mut self,
        _h_client: u32,
        _h_channel: u32,
        _cmd: u32,
        _params: &[u8],
        _channel_off: usize,
        _client_off: Option<usize>,
    ) -> Option<(u32, Vec<u8>, String)> {
        None
    }
    /// The guest ChID of this VM's mirror resident on GR, if one is (mirror.rs
    /// `resident_guest_chid`).
    fn resident(&mut self, _params: &[u8]) -> Option<(u32, Option<u32>, String)> {
        None
    }
}

/// [`MirrorObjects`] over the device's mirror (`Mirror::forward_control`: the TSG by its
/// guest key, the channel through the registry by its guest handle, never another handle).
pub struct MirrorFwd<'a> {
    pub mirror: &'a mut crate::mirror::Mirror,
    pub reg: &'a crate::channel::Registry,
}

impl MirrorObjects for MirrorFwd<'_> {
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

    fn idle(&mut self, list: &[(u32, u32)], flags: u32, timeout_us: u32) -> Option<(u32, String)> {
        self.mirror.idle_channels(self.reg, list, flags, timeout_us)
    }

    fn schedule(&mut self, list: &[(u32, u32)], disable: bool, only: bool) -> Option<(u32, String)> {
        self.mirror.set_scheduling(self.reg, list, disable, only)
    }

    fn channel_query(
        &mut self,
        h_client: u32,
        h_channel: u32,
        cmd: u32,
        params: &[u8],
        channel_off: usize,
        client_off: Option<usize>,
    ) -> Option<(u32, Vec<u8>, String)> {
        self.mirror
            .channel_query(self.reg, h_client, h_channel, cmd, params, (channel_off, client_off))
    }

    fn resident(&mut self, params: &[u8]) -> Option<(u32, Option<u32>, String)> {
        self.mirror.resident_guest_chid(params)
    }
}

/// Per guest object: what its controls recorded (c-guest-state.md §4 `ObjAttrs`). The
/// device-wide channel state (the RC journal and the scheduling flags) is rc.rs's
/// `RcState`, one for the C rows and the D models.
#[derive(Default, Debug)]
pub struct GuestAttrs {
    objects: BTreeMap<(u32, u32), Attrs>,
}

#[derive(Default, Debug, Clone)]
pub struct Attrs {
    /// The last parameter block of each setter, by control id.
    pub recorded: BTreeMap<u32, Vec<u8>>,
    /// Resets of the channel the device answered or forwarded.
    pub resets: u32,
    /// The last MMU fault of the channel as `GET_MMU_FAULT_INFO` returns it: the host's
    /// answer on the channel's own mirror, read when the device raised an RC for it
    /// (boot.rs `scan_channel_errors`).
    pub last_fault: Option<Vec<u8>>,
}

impl GuestAttrs {
    pub fn get(&self, key: (u32, u32)) -> Option<&Attrs> {
        self.objects.get(&key)
    }

    /// The fault record of guest channel `key` for an RC the device raised (the host's
    /// `GET_MMU_FAULT_INFO` answer on the channel's own mirror, boot.rs); the RC itself
    /// goes into rc.rs's journal.
    pub fn set_fault(&mut self, key: (u32, u32), fault: Vec<u8>) {
        self.objects.entry(key).or_default().last_fault = Some(fault);
    }

    /// `FREE` of an object (and of everything a freed client owned).
    pub fn free(&mut self, h_client: u32, h_object: u32) {
        self.objects.remove(&(h_client, h_object));
        if h_client == h_object {
            self.objects.retain(|(c, _), _| *c != h_client);
        }
    }
}

fn get32(b: &[u8], off: usize) -> u32 {
    b.get(off..off + 4)
        .map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

fn get64(b: &[u8], off: usize) -> u64 {
    b.get(off..off + 8)
        .map_or(0, |s| u64::from_le_bytes(s.try_into().unwrap()))
}

fn put32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// The answer of the table to `cmd`, or `None` when the table has no row for it.
pub fn answer(ctx: &mut Ctx<'_>, cmd: u32, params: &[u8]) -> Option<Answer> {
    let row = p::public_control(cmd)?;
    if row.size != 0 && params.len() != row.size {
        return Some(Answer {
            status: NV_ERR_INVALID_PARAM_STRUCT,
            params: params.to_vec(),
            label: "refused (params size is not the driver's struct; not forwarded)",
            note: format!(
                "{} ({cmd:#x}): {} bytes, the struct has {}",
                row.name,
                params.len(),
                row.size
            ),
        });
    }
    Some(match row.rule {
        Rule::Filtered(f) => filtered(ctx, row, f, params),
        Rule::Device(d) => device(ctx, row, d, params),
        Rule::Guest(g) => guest(ctx, row, g, params),
        Rule::Refuse(label) => Answer {
            status: NV_ERR_NOT_SUPPORTED,
            params: params.to_vec(),
            label,
            note: format!("{} ({cmd:#x}): not supported, never forwarded", row.name),
        },
    })
}

/// A control on one of the guest's channels or TSGs. Nothing here uses the host RM
/// client; the only host path is the object's own mirror.
fn guest(ctx: &mut Ctx<'_>, row: &PublicControl, g: Guest, params: &[u8]) -> Answer {
    let (h_client, h_object) = ctx.object;
    let n = params.len();
    let ok = |b: Vec<u8>, label: &'static str, what: String| Answer {
        status: NV_OK,
        params: b,
        label,
        note: format!(
            "{} ({:#x}) on {h_client:#x}/{h_object:#x}: {what}",
            row.name, row.id
        ),
    };
    match g {
        Guest::Reset { host_cmd } => {
            let a = ctx.attrs.objects.entry(ctx.key).or_default();
            a.resets += 1;
            a.last_fault = None;
            let resets = a.resets;
            // Never forwarded: on the host, RESET_CHANNEL also resets the engine the
            // guest names (ctrl906f.h:105-124), and a host engine reset reaches every
            // tenant of that engine, the host desktop included. The reset is recorded
            // and the guest sees NV_OK; the mirror channel is not touched
            // (gsp-plan/c-guest-state.md §8; the host command would have been
            // `host_cmd`).
            let _ = (h_client, h_object);
            ok(
                params.to_vec(),
                "guest (reset recorded by the device, never forwarded: a guest-named engine reset must not reach a host engine)",
                format!("reset {resets}, host command {host_cmd:#x} withheld"),
            )
        }
        Guest::MmuFaultInfo => {
            let fault = ctx.attrs.get(ctx.key).and_then(|a| a.last_fault.clone());
            let mut b = vec![0u8; n];
            let what = match fault {
                Some(f) => {
                    let k = f.len().min(n);
                    b[..k].copy_from_slice(&f[..k]);
                    "the recorded fault".to_string()
                }
                None => "no fault recorded".to_string(),
            };
            ok(b, "guest (the device's fault state of the channel)", what)
        }
        Guest::DeferRcState => ok(
            vec![0u8; n],
            "guest (the device defers no RC: bDeferRCPending 0)",
            "no deferred RC".into(),
        ),
        Guest::RcErrorCount => {
            let count = ctx.rc.journal.len() as u32;
            let mut b = vec![0u8; n];
            put32(&mut b, 0, count);
            ok(
                b,
                "guest (the device's RC journal for this guest, rc.rs)",
                format!("error count {count}"),
            )
        }
        Guest::RcErrorV2 => {
            // kernel_rc_ctrl.c:186-283 (krcSubdeviceCtrlCmdRcGetErrorV2): record
            // `whichBuffer` (0 the oldest) is an `RmProtoBuf_RECORD` header followed by a
            // `Dcl.ErrorBlock` message that holds the element's error blocks. The device's
            // records carry no GPU dump blocks, so the message is empty: the header alone,
            // dwSize 0; outputRecordSize 0 where there is no such record.
            let which = get32(params, 0);
            let mut b = vec![0u8; n];
            b[..4].copy_from_slice(&params[..4]);
            let (label, what) = match ctx.rc.journal.get(which as usize) {
                Some(r) => {
                    let h = p::RC_ERROR_V2_RECORD;
                    b[h] = p::RC_RECORD_GROUP_RM;
                    b[h + 1] = p::RC_RECORD_TYPE_PROTOBUF;
                    b[h + 2..h + 4]
                        .copy_from_slice(&(p::RC_PROTOBUF_RECORD_SIZE as u16).to_le_bytes());
                    put32(&mut b, h + 4, 0);
                    put32(&mut b, p::RC_ERROR_V2_OUTPUT_SIZE, p::RC_PROTOBUF_RECORD_SIZE as u32);
                    (
                        "guest (a record of the device's RC journal, rc.rs: the protobuf record header, no error blocks)",
                        format!("record {which}: Xid {} chid {:?}", r.xid, r.chid),
                    )
                }
                None => (
                    "guest (the device's RC journal, rc.rs: no such record)",
                    format!("record {which}: none of {}", ctx.rc.journal.len()),
                ),
            };
            ok(b, label, what)
        }
        Guest::Record { max_level } => {
            ctx.attrs
                .objects
                .entry(ctx.key)
                .or_default()
                .recorded
                .insert(row.id, params.to_vec());
            let level = get32(params, 0);
            if max_level.is_some_and(|m| level > m) {
                return ok(
                    params.to_vec(),
                    "guest (recorded; a level above MEDIUM is not forwarded: a guest must not outrank the host desktop)",
                    format!("level {level} recorded only"),
                );
            }
            match ctx
                .mirror
                .as_deref_mut()
                .and_then(|m| m.forward(h_client, h_object, row.id, params))
            {
                Some((st, note)) => ok(
                    params.to_vec(),
                    "host (recorded; forwarded to the guest object's own mirror; the guest sees NV_OK)",
                    format!("{note}; host status {st:#x}"),
                ),
                None => ok(
                    params.to_vec(),
                    "guest (recorded; no mirror object)",
                    "recorded".into(),
                ),
            }
        }
        Guest::Get { set } => {
            let rec = ctx.attrs.get(ctx.key).and_then(|a| a.recorded.get(&set));
            let mut b = vec![0u8; n];
            if let Some(r) = rec {
                let k = r.len().min(n);
                b[..k].copy_from_slice(&r[..k]);
            }
            ok(
                b,
                "guest (the value the guest set; NONE before)",
                format!("recorded: {}", rec.is_some()),
            )
        }
        Guest::RecordOnly => {
            ctx.attrs
                .objects
                .entry(ctx.key)
                .or_default()
                .recorded
                .insert(row.id, params.to_vec());
            ok(
                params.to_vec(),
                "guest (recorded, never forwarded: decision Q-C7, no guest priority over the host desktop, no restart of a shared runlist)",
                "recorded only".into(),
            )
        }
        Guest::ChannelRecord { h_client: client_off, h_channel } => {
            let c = client_off.map_or(h_client, |o| get32(params, o));
            let h = get32(params, h_channel);
            if channel_of(ctx.reg, c, h).is_none() {
                return err(
                    row,
                    NV_ERR_INVALID_OBJECT_HANDLE,
                    params,
                    "guest (the params name no channel of this device)",
                    format!("{c:#x}/{h:#x}"),
                );
            }
            ctx.attrs
                .objects
                .entry((c, h))
                .or_default()
                .recorded
                .insert(row.id, params.to_vec());
            ok(
                params.to_vec(),
                "guest (recorded on the named guest channel, never forwarded: the mirror channel keeps the host's defaults)",
                format!("channel {c:#x}/{h:#x}: recorded"),
            )
        }
        Guest::IdleChannels => {
            let num = get32(params, 0) as usize;
            if num > p::IDLE_CHANNELS_MAX {
                return err(row, NV_ERR_INVALID_ARGUMENT, params, "guest (more channels than the struct holds)", format!("{num} channels"));
            }
            let (flags, timeout) = (
                get32(params, p::IDLE_CHANNELS_FLAGS),
                get32(params, p::IDLE_CHANNELS_TIMEOUT),
            );
            let mut mirrored = Vec::new();
            for i in 0..num {
                let h = get32(params, 4 + 4 * i);
                match channel_of(ctx.reg, h_client, h) {
                    None => {
                        return err(
                            row,
                            NV_ERR_INVALID_OBJECT_HANDLE,
                            params,
                            "guest (the list names no channel of this device)",
                            format!("{h_client:#x}/{h:#x}"),
                        )
                    }
                    Some(c) if c.mirrored => mirrored.push((h_client, h)),
                    Some(_) => {}
                }
            }
            if mirrored.is_empty() {
                return ok(
                    params.to_vec(),
                    "guest (software channels are idle: the executor runs a push to completion at its doorbell)",
                    format!("{num} channel(s) idle"),
                );
            }
            let capped = (crate::gsprpc::IDLE_WAIT_CAP.as_micros() as u32).min(timeout);
            let host = ctx
                .mirror
                .as_deref_mut()
                .and_then(|m| m.idle(&mirrored, flags, capped));
            let (status, label, what) = match host {
                Some((NV_OK, note)) => (NV_OK, "host (the host RM's idle on the guest channels' own mirrors)", note),
                Some((NV_ERR_TIMEOUT, note)) => (NV_ERR_TIMEOUT, "host (the host RM's idle on the own mirrors timed out)", note),
                Some((st, note)) => (
                    NV_ERR_TIMEOUT,
                    "host (the host RM refused the idle of the own mirrors: not shown idle)",
                    format!("host status {st:#x}: {note}"),
                ),
                None => (NV_OK, "guest (no host mirror object: nothing runs on the host)", "no host object".into()),
            };
            Answer {
                status,
                params: params.to_vec(),
                label,
                note: format!("{} ({:#x}): {num} channel(s), {} mirrored, timeout {timeout} us capped at {capped} us: {what}", row.name, row.id, mirrored.len()),
            }
        }
        Guest::DisableUsermode | Guest::ToggleScheduling => {
            let disable = params[0] != 0;
            let all = matches!(g, Guest::ToggleScheduling);
            if all {
                ctx.rc.scheduling_disabled = disable;
            } else {
                ctx.rc.usermode_disabled = disable;
            }
            let mirrored: Vec<(u32, u32)> = ctx
                .reg
                .map(|r| {
                    r.channels()
                        .filter(|c| c.mirrored && (all || crate::rc::usermode(r, c)))
                        .map(|c| (c.h_client, c.h_object))
                        .collect()
                })
                .unwrap_or_default();
            let host = if mirrored.is_empty() {
                None
            } else {
                ctx.mirror
                    .as_deref_mut()
                    .and_then(|m| m.schedule(&mirrored, disable, all))
            };
            ok(
                params.to_vec(),
                if host.is_some() {
                    "host (the device's scheduling flag over the guest's channels; their own mirrors on the host, never another host channel)"
                } else {
                    "guest (the device's scheduling flag over the guest's channels; no mirror)"
                },
                format!(
                    "{} {}; {} mirror channel(s){}",
                    if all { "active channels" } else { "user channels" },
                    if disable { "disabled" } else { "enabled" },
                    mirrored.len(),
                    host.map_or(String::new(), |(st, n)| format!("; host status {st:#x}: {n}"))
                ),
            )
        }
        Guest::CheckEngineContext => {
            let (h, engine) = (get32(params, 0), get32(params, p::CHECK_CTX_ENGINE));
            if engine != p::NV2080_ENGINE_TYPE_GRAPHICS {
                return err(row, NV_ERR_INVALID_ARGUMENT, params, "guest (only the graphics engine has a context here)", format!("engine {engine:#x}"));
            }
            let Some(c) = channel_of(ctx.reg, h_client, h) else {
                return err(row, NV_ERR_INVALID_CHANNEL, params, "guest (no channel of this device)", format!("{h_client:#x}/{h:#x}"));
            };
            let reg = ctx.reg.expect("resolved above");
            let promoted = |k: (u32, u32)| reg.promoted(k).is_some_and(|m| !m.is_empty());
            let exists = promoted((c.h_client, c.h_object)) || promoted((c.h_client, c.h_parent));
            let mut b = params.to_vec();
            b[p::CHECK_CTX_EXISTS] = exists as u8;
            ok(
                b,
                "guest (the GR context the guest promoted for the channel or its TSG, channel.rs record_promote_ctx)",
                format!("chid {}: exists {exists}", c.chid),
            )
        }
        Guest::MmuDebugMode => ok(
            vec![0u8; n],
            "guest (the TSG's MMU debug mode: the device never enables one, the SM debugger setters are refused)",
            "bMode 0".into(),
        ),
        Guest::CtxswStats | Guest::CtxswModes => {
            let h = get32(params, 0);
            let Some(c) = channel_of(ctx.reg, h_client, h) else {
                return err(row, NV_ERR_INVALID_OBJECT_HANDLE, params, "guest (no channel of this device)", format!("{h_client:#x}/{h:#x}"));
            };
            if c.mirrored {
                if let Some((st, out, note)) = ctx
                    .mirror
                    .as_deref_mut()
                    .and_then(|m| m.channel_query(h_client, h, row.id, params, 0, None))
                {
                    return Answer {
                        status: st,
                        params: out,
                        label: "host (the guest channel's own mirror, hChannel rewritten to it on the mirror's subdevice)",
                        note: format!("{} ({:#x}) chid {}: {note}", row.name, row.id, c.chid),
                    };
                }
            }
            let mut b = vec![0u8; n];
            b[..4].copy_from_slice(&h.to_le_bytes());
            if matches!(g, Guest::CtxswStats) {
                b[4..8].copy_from_slice(&params[4..8]);
                return ok(b, "guest (a software channel never context-switches: zeros)", format!("chid {}", c.chid));
            }
            let rec = ctx.attrs.get((h_client, h)).map(|a| &a.recorded);
            let zcull = rec.and_then(|r| r.get(&0x2080_1205)).map_or(0, |z| get32(z, p::ZCULL_MODE_MODE));
            let bind = rec.and_then(|r| r.get(&0x2080_1211));
            let gfxp = bind.map_or(0, |b| get32(b, p::PREEMPTION_BIND_GFXP));
            let cilp = bind.map_or(0, |b| get32(b, p::PREEMPTION_BIND_CILP));
            put32(&mut b, p::CTXSW_MODES_ZCULL, zcull);
            put32(&mut b, p::CTXSW_MODES_CILP, cilp);
            put32(&mut b, p::CTXSW_MODES_GFXP, gfxp);
            ok(
                b,
                "guest (the modes the guest set on the software channel; PM and SMPC off)",
                format!("chid {}: zcull {zcull} cilp {cilp} gfxp {gfxp}", c.chid),
            )
        }
        Guest::ResidentChannel => {
            let mut b = params.to_vec();
            let host = ctx.mirror.as_deref_mut().and_then(|m| m.resident(params));
            let (chid, what) = match host {
                Some((NV_OK, Some(g), note)) => (g, note),
                Some((_, _, note)) => (u32::MAX, note),
                None => (u32::MAX, "no mirror channel: no guest channel runs on the physical GR".into()),
            };
            put32(&mut b, 0, chid);
            ok(
                b,
                "guest (a guest ChID only when one of this VM's own mirrors is resident; never another tenant's)",
                format!("chID {chid:#x}: {what}"),
            )
        }
        Guest::ChannelState => {
            let (h, c) = (get32(params, 0), get32(params, 4));
            let Some(ch) = channel_of(ctx.reg, c, h) else {
                return err(row, NV_ERR_INVALID_CHANNEL, params, "guest (no channel of this device)", format!("{c:#x}/{h:#x}"));
            };
            let reg = ctx.reg.expect("resolved above");
            let enabled = ctx.rc.channel_enabled(reg, ch);
            let mut b = params.to_vec();
            b[8..16].fill(0);
            b[p::CHANNEL_STATE_ENABLED] = enabled as u8;
            b[p::CHANNEL_STATE_SCHEDULED] = enabled as u8;
            ok(
                b,
                "guest (the device's scheduling state of the channel; bCpuMap and bRunlistSet are the kernel's, bDeferRC 0)",
                format!("chid {} enabled {enabled}", ch.chid),
            )
        }
        Guest::ErrbarDebugMode => {
            let mut b = vec![0u8; n];
            put32(&mut b, 0, p::ERRBAR_DEBUG_DISABLED);
            ok(
                b,
                "guest (disabled: SET_MODE_ERRBAR_DEBUG is refused, so no session enables it)",
                "errbar debug disabled".into(),
            )
        }
        Guest::TsgInfo => match ctx.reg.and_then(|r| r.tsg_id(ctx.key)) {
            Some(id) => {
                let mut b = vec![0u8; n];
                put32(&mut b, 0, id);
                ok(b, "guest (the kernel's TSG id, internalFlags TSG_ID)", format!("tsgID {id}"))
            }
            None => err(
                row,
                NV_ERR_NOT_SUPPORTED,
                params,
                "guest (no kernel TSG id recorded: a 610 kernel sends none)",
                format!("{h_client:#x}/{h_object:#x}"),
            ),
        },
    }
}

/// A refusal of a C row with its status, the request echoed.
fn err(row: &PublicControl, status: u32, params: &[u8], label: &'static str, what: String) -> Answer {
    Answer {
        status,
        params: params.to_vec(),
        label,
        note: format!("{} ({:#x}): {what}: status {status:#x}", row.name, row.id),
    }
}

fn device(ctx: &Ctx<'_>, row: &PublicControl, d: Dev, params: &[u8]) -> Answer {
    let n = params.len();
    let (status, b, label, what): (u32, Vec<u8>, &'static str, String) = match d {
        Dev::RmHeapStats => (
            NV_OK,
            vec![0u8; n],
            "synthesized (the device's own GSP heap view: no RM heap, all 0; the host GSP is never asked, decision D9/Q-B6)",
            "rm heap stats: zeros".into(),
        ),
        Dev::TimeCorrelation => {
            // Every requested sample from the one device clock: the emulated GSP's OS
            // time and the device PTIMER (usermode TIME_0/1, uvm.rs) are the same base,
            // so cpuTime = gpuTime. The host's GSP clock and PTIMER are never read.
            let mut b = params.to_vec();
            let count = (b[p::TIME_CORR_SAMPLE_COUNT] as usize).min(p::TIME_CORR_MAX_SAMPLES);
            b[p::TIME_CORR_SAMPLES..].fill(0);
            for i in 0..count {
                let o = p::TIME_CORR_SAMPLES + 16 * i;
                put64(&mut b, o, ctx.now_ns);
                put64(&mut b, o + 8, ctx.now_ns);
            }
            (
                NV_OK,
                b,
                "synthesized (the device's own clock: GSP OS time and PTIMER are one time base)",
                format!("cpuClkId {:#x}, {count} sample(s) at {} ns", params[0], ctx.now_ns),
            )
        }
        Dev::NbsiNotFound => {
            let mut b = params.to_vec();
            b[p::NBSI_V2_RET_SIZE - p::NBSI_V2_RET_BUF_LEN..p::NBSI_V2_RET_SIZE].fill(0);
            put32(&mut b, p::NBSI_V2_RET_SIZE, 0);
            put32(&mut b, p::NBSI_V2_ERROR_CODE, p::NBSI_NOT_FOUND);
            (
                NV_OK,
                b,
                "synthesized (the synthesized VBIOS has no NBSI: NOT_FOUND, retSize 0)",
                "nbsi v2: not found".into(),
            )
        }
        Dev::I2cNoPorts => (
            NV_OK,
            vec![0u8; n],
            "synthesized (no I2C ports, as I2C_GET_PORT_INFO answers)",
            "i2c port speed: no ports".into(),
        ),
        Dev::RcRecoveryEnabled => {
            let mut b = vec![0u8; n];
            put32(&mut b, 0, p::RC_RECOVERY_ENABLED);
            (
                NV_OK,
                b,
                "synthesized (the device's RC policy: recovery enabled)",
                "rc recovery: enabled".into(),
            )
        }
        Dev::NoCompTags => (
            NV_OK,
            vec![0u8; n],
            "synthesized (no compression, D16: numCompTags 0)",
            "ctags for cbc eviction: none".into(),
        ),
        Dev::NoEcc => (
            NV_ERR_NOT_SUPPORTED,
            params.to_vec(),
            "synthesized (NOT_SUPPORTED: no ECC on this board, as the host answers the ECC getters, Q-B1)",
            "sram unique uncorrected counts: no ECC".into(),
        ),
        Dev::BifInfo => {
            let index = get32(params, 0);
            let lnk = ctx.link_control as u32;
            let bit = match index {
                p::BIF_INFO_L0S => Some(lnk & 1),
                p::BIF_INFO_L1 => Some((lnk >> 1) & 1),
                _ => None,
            };
            match bit {
                Some(v) => {
                    let mut b = params.to_vec();
                    put32(&mut b, 4, v);
                    (
                        NV_OK,
                        b,
                        "synthesized (the ASPM enables of the emulated PCIe Link Control register)",
                        format!("index {index}: {v} (LnkCtl {lnk:#x})"),
                    )
                }
                None => (
                    NV_ERR_INVALID_ARGUMENT,
                    params.to_vec(),
                    "synthesized (BIF info index other than L0S_ENABLED and L1_ENABLED)",
                    format!("index {index}"),
                ),
            }
        }
    };
    Answer {
        status,
        params: b,
        label,
        note: format!("{} ({:#x}): {what}", row.name, row.id),
    }
}

fn filtered(ctx: &mut Ctx<'_>, row: &PublicControl, f: Filter, params: &[u8]) -> Answer {
    let Some(host) = ctx.host.as_deref_mut() else {
        return Answer {
            status: NV_ERR_NOT_SUPPORTED,
            params: params.to_vec(),
            label: "synthesized fallback (no host: NOT_SUPPORTED)",
            note: format!("{} ({:#x}): no host", row.name, row.id),
        };
    };
    let mut b = params.to_vec();
    let status = host.control(NV20_SUBDEVICE_0, row.id, &mut b);
    // The filter runs on every answer, whatever the status: a host that wrote the buffer
    // and failed must not leak what it wrote either.
    let (label, what) = match f {
        Filter::OfflinedPages => offlined_pages(&mut b, ctx.reservation),
        Filter::RemappedRows => remapped_rows(&mut b),
        Filter::ScrubberRange => scrubber_range(&mut b, ctx.reservation),
        Filter::DynamicOfflinedPages => dynamic_offlined_pages(&mut b, ctx.reservation),
        Filter::GpumonProcess => gpumon_process(&mut b, ctx.chip),
    };
    Answer {
        status,
        params: b,
        label,
        note: format!("{} ({:#x}): host status {status:#x}; {what}", row.name, row.id),
    }
}

/// Whether the 4 KiB page `page` lies in `[base, end)`.
fn page_inside(page: u64, (base, end): (u64, u64)) -> bool {
    page.checked_shl(p::RM_PAGE_SHIFT)
        .filter(|a| a >> p::RM_PAGE_SHIFT == page)
        .is_some_and(|a| base <= a && a < end)
}

/// `FB_GET_OFFLINED_PAGES`: keep the entries whose pages lie in the reservation, compacted
/// to the front; zero every other byte of the list. An entry is kept only if each of its
/// two addresses is either the invalid marker or inside, and at least one is inside. The
/// retirement-pending flags stay only when a kept entry is itself pending retirement.
fn offlined_pages(b: &mut [u8], res: Option<(u64, u64)>) -> (&'static str, String) {
    let n = (get32(b, p::OFFLINED_VALID_ENTRIES) as usize).min(p::OFFLINED_MAX);
    let mut kept: Vec<Vec<u8>> = Vec::new();
    let mut pending = false;
    for i in 0..n {
        let o = i * p::OFFLINED_ENTRY_SIZE;
        let e = &b[o..o + p::OFFLINED_ENTRY_SIZE];
        let addrs = [get64(e, p::OFFLINED_ECC_ON), get64(e, p::OFFLINED_ECC_OFF)];
        let inside = |a: u64| res.is_some_and(|r| page_inside(a, r));
        let keep = addrs.iter().any(|&a| inside(a))
            && addrs
                .iter()
                .all(|&a| a == p::OFFLINED_INVALID_ADDRESS || inside(a));
        if keep {
            pending |= get32(e, p::OFFLINED_STATUS) == p::OFFLINED_STATUS_PENDING_RETIREMENT;
            kept.push(e.to_vec());
        }
    }
    b[..p::OFFLINED_VALID_ENTRIES].fill(0);
    for (i, e) in kept.iter().enumerate() {
        let o = i * p::OFFLINED_ENTRY_SIZE;
        b[o..o + p::OFFLINED_ENTRY_SIZE].copy_from_slice(e);
    }
    put32(b, p::OFFLINED_VALID_ENTRIES, kept.len() as u32);
    if !pending {
        b[p::OFFLINED_B_RETIREMENT_PENDING] = 0;
        b[p::OFFLINED_RETIREMENT_PENDING] = 0;
    }
    let label = match res {
        Some(_) => "host (forwarded, filtered: offlined pages inside the VM's reservation; guest FB address = host VRAM address)",
        None => "host (forwarded, filtered: no reservation, so no host page is a guest page; empty list)",
    };
    (
        label,
        format!("{} of {n} host entries inside the reservation {res:x?}", kept.len()),
    )
}

/// `FB_GET_REMAPPED_ROWS`: a remapped row is a DRAM location (`remapRegVal`, FBPA,
/// sublocation), not an FB address, and the device has no map from one to the other, so
/// no entry can be shown to lie in the reservation: the list is emptied. The flags
/// (pending, failure) are the card's state, as the forwarded reset status is.
fn remapped_rows(b: &mut [u8]) -> (&'static str, String) {
    let n = get32(b, p::REMAPPED_ROWS_ENTRY_COUNT);
    put32(b, p::REMAPPED_ROWS_ENTRY_COUNT, 0);
    b[p::REMAPPED_ROWS_ENTRIES..].fill(0);
    (
        "host (forwarded, filtered: remapped rows cannot be placed in the reservation, empty list; card flags kept)",
        format!("{n} host entries withheld, flags {:#x}", b[p::REMAPPED_ROWS_FLAGS]),
    )
}

fn put64(b: &mut [u8], off: usize, v: u64) {
    b[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

/// `GPU_QUERY_SCRUBBER_STATUS`: the scrub range `[start, end)` clamped to the
/// reservation; 0 and 0 when they do not meet or there is no reservation. The status
/// and the remaining time are the card's.
fn scrubber_range(b: &mut [u8], res: Option<(u64, u64)>) -> (&'static str, String) {
    let (s, e) = (get64(b, p::SCRUBBER_START), get64(b, p::SCRUBBER_END));
    let (ns, ne) = match res {
        Some((base, end)) if s.max(base) < e.min(end) => (s.max(base), e.min(end)),
        _ => (0, 0),
    };
    put64(b, p::SCRUBBER_START, ns);
    put64(b, p::SCRUBBER_END, ne);
    (
        "host (forwarded, filtered: scrub range clamped to the VM's reservation)",
        format!("host range [{s:#x}, {e:#x}) -> [{ns:#x}, {ne:#x})"),
    )
}

/// `FB_GET_DYNAMIC_OFFLINED_PAGES`: keep the entries whose page lies in the reservation,
/// compacted, zero the rest and recount `validEntries`; `baseIndex` and `bMore` pass.
fn dynamic_offlined_pages(b: &mut [u8], res: Option<(u64, u64)>) -> (&'static str, String) {
    let n = (get32(b, p::DYN_OFFLINED_VALID_ENTRIES) as usize).min(p::DYN_OFFLINED_MAX);
    let es = p::DYN_OFFLINED_ENTRY_SIZE;
    let kept: Vec<Vec<u8>> = (0..n)
        .map(|i| b[i * es..(i + 1) * es].to_vec())
        .filter(|e| res.is_some_and(|r| page_inside(get64(e, 0), r)))
        .collect();
    b[..p::DYN_OFFLINED_VALID_ENTRIES].fill(0);
    for (i, e) in kept.iter().enumerate() {
        b[i * es..(i + 1) * es].copy_from_slice(e);
    }
    put32(b, p::DYN_OFFLINED_VALID_ENTRIES, kept.len() as u32);
    (
        "host (forwarded, filtered: dynamically offlined pages inside the VM's reservation)",
        format!("{} of {n} host entries inside the reservation {res:x?}", kept.len()),
    )
}

/// `PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2`: in every sample slot (the ring holds 72;
/// `count` says how many are valid and `tracker` where the oldest is, so all slots are
/// filtered) the host process fields of the six engine samples are zeroed, and so is the
/// utilization of an engine the device does not present (nvjpg and nvofa on TU106). The
/// utilization of the others is the card's, the host desktop's load included (P7,
/// accepted for telemetry under D9).
fn gpumon_process(b: &mut [u8], chip: &crate::chip::ChipData) -> (&'static str, String) {
    let presents = |engine: &str| {
        engine == "fb"
            || chip
                .device_info
                .engines
                .iter()
                .any(|e| e.name.trim_end_matches(|c: char| c.is_ascii_digit()) == engine)
    };
    let absent: Vec<&str> = p::GPUMON_ENGINES
        .iter()
        .map(|(e, _)| *e)
        .filter(|e| !presents(e))
        .collect();
    for i in 0..p::GPUMON_SAMPLE_COUNT {
        let s = p::GPUMON_SAMPLES + i * p::GPUMON_SAMPLE_SIZE;
        for (engine, off) in p::GPUMON_ENGINES {
            let e = s + off;
            b[e + p::GPUMON_ENGINE_PROC.start..e + p::GPUMON_ENGINE_PROC.end].fill(0);
            if absent.contains(engine) {
                put32(b, e + p::GPUMON_ENGINE_UTIL, 0);
            }
        }
    }
    (
        "host (forwarded, filtered: host process fields zeroed, engines the device lacks zeroed)",
        format!(
            "{} samples; process fields zeroed; absent engines {absent:?}",
            get32(b, p::GPUMON_COUNT)
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ver::active::NV_OK;

    /// A host that answers every control with a fixed parameter block.
    struct Canned(Vec<u8>, std::sync::Arc<std::sync::Mutex<Vec<(u32, u32)>>>);
    impl HostRm for Canned {
        fn control(&mut self, class: u32, cmd: u32, b: &mut [u8]) -> u32 {
            self.1.lock().unwrap().push((class, cmd));
            b.copy_from_slice(&self.0[..b.len()]);
            NV_OK
        }
    }

    const RES: (u64, u64) = (0xE900_0000, 0x1_E900_0000);

    fn tu106() -> &'static crate::chip::ChipData {
        crate::chip::by_name("tu106").unwrap().data()
    }

    fn offlined_answer(entries: &[(u64, u64, u32)], pending: bool) -> Vec<u8> {
        let mut b = vec![0u8; p::OFFLINED_PAGES_SIZE];
        for (i, &(on, off, status)) in entries.iter().enumerate() {
            let o = i * p::OFFLINED_ENTRY_SIZE;
            b[o..o + 8].copy_from_slice(&on.to_le_bytes());
            b[o + 8..o + 16].copy_from_slice(&off.to_le_bytes());
            b[o + 16] = 0x77; // rbcAddress, kept with its entry
            put32(&mut b, o + p::OFFLINED_STATUS, status);
        }
        put32(&mut b, p::OFFLINED_VALID_ENTRIES, entries.len() as u32);
        b[p::OFFLINED_B_RETIREMENT_PENDING] = pending as u8;
        b[p::OFFLINED_RETIREMENT_PENDING] = if pending { 3 } else { 0 };
        b
    }

    fn run(host: Vec<u8>, cmd: u32, res: Option<(u64, u64)>) -> (Answer, Vec<(u32, u32)>) {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = Canned(host, seen.clone());
        let mut ctx = Ctx {
            host: Some(&mut h),
            reservation: res,
            chip: tu106(),
            now_ns: 0,
            object: (0, 0),
            key: (0, 0),
            mirror: None,
            attrs: &mut GuestAttrs::default(),
            rc: &mut crate::rc::RcState::default(),
            reg: None,
            link_control: 0,
        };
        let size = p::public_control(cmd).unwrap().size;
        let a = answer(&mut ctx, cmd, &vec![0u8; size]).unwrap();
        let s = seen.lock().unwrap().clone();
        (a, s)
    }

    /// Every address the guest gets back from FB_GET_OFFLINED_PAGES lies in the
    /// reservation; host pages below and above it never reach the guest, nor does a
    /// pending flag that belongs to a withheld page.
    #[test]
    fn offlined_pages_outside_the_reservation_never_reach_the_guest() {
        let pg = |a: u64| a >> 12;
        let inv = p::OFFLINED_INVALID_ADDRESS;
        let host = offlined_answer(
            &[
                (inv, pg(0x0360_0000), 1),                 // below B: pending, withheld
                (inv, pg(RES.0), 0),                       // first page inside
                (pg(RES.1 - 0x1000), pg(RES.1 - 0x1000), 0), // last page inside
                (inv, pg(RES.1), 0),                       // E itself: outside
                (pg(RES.0 + 0x2000), pg(0x1F76_0000), 0), // one address outside: withheld
                (inv, inv, 0),                             // no address: withheld
                (inv, u64::MAX >> 4, 0),                   // overflows when shifted
            ],
            true,
        );
        let (a, seen) = run(host, 0x2080_1322, Some(RES));
        assert_eq!(seen, [(NV20_SUBDEVICE_0, 0x2080_1322)]);
        assert_eq!(a.status, NV_OK);
        assert_eq!(get32(&a.params, p::OFFLINED_VALID_ENTRIES), 2);
        let entry = |i: usize| {
            let o = i * p::OFFLINED_ENTRY_SIZE;
            (get64(&a.params, o), get64(&a.params, o + 8), a.params[o + 16])
        };
        assert_eq!(entry(0), (inv, pg(RES.0), 0x77));
        assert_eq!(entry(1), (pg(RES.1 - 0x1000), pg(RES.1 - 0x1000), 0x77));
        assert!(a.params[2 * p::OFFLINED_ENTRY_SIZE..p::OFFLINED_VALID_ENTRIES]
            .iter()
            .all(|&x| x == 0));
        assert_eq!(a.params[p::OFFLINED_B_RETIREMENT_PENDING], 0);
        assert_eq!(a.params[p::OFFLINED_RETIREMENT_PENDING], 0);
        // Every address the guest sees, over all 64 slots: invalid or inside.
        for i in 0..p::OFFLINED_MAX {
            for off in [p::OFFLINED_ECC_ON, p::OFFLINED_ECC_OFF] {
                let x = get64(&a.params, i * p::OFFLINED_ENTRY_SIZE + off);
                assert!(
                    x == 0 || x == inv || page_inside(x, RES),
                    "slot {i}: {x:#x}"
                );
            }
        }
        // A kept page pending retirement keeps the flags.
        let host = offlined_answer(&[(inv, pg(RES.0 + 0x5000), 1)], true);
        let (a, _) = run(host, 0x2080_1322, Some(RES));
        assert_eq!(get32(&a.params, p::OFFLINED_VALID_ENTRIES), 1);
        assert_eq!(a.params[p::OFFLINED_B_RETIREMENT_PENDING], 1);
    }

    /// Through the dispatcher: the rows are not plain forwards any more, and the answer
    /// the guest kernel gets holds only pages inside the reservation.
    #[test]
    fn the_dispatcher_filters() {
        use crate::ctrl::{Dispatcher, INTERNAL_CLIENT, INTERNAL_SUBDEVICE};
        for id in [0x2080_1322u32, 0x2080_1344] {
            assert!(!crate::ver::active::FORWARD.iter().any(|(c, _)| *c == id));
        }
        let inv = p::OFFLINED_INVALID_ADDRESS;
        let host = offlined_answer(&[(inv, 0x3600, 0), (inv, (RES.0 >> 12) + 1, 0)], false);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let f = crate::fixture::rtx2070();
        let mut d = Dispatcher::new(
            f.chip.data(),
            &f.identity(4 << 30),
            Some(Box::new(Canned(host, seen))),
        )
        .with_reservation(Some(RES));
        let a = d.control(
            INTERNAL_CLIENT,
            INTERNAL_SUBDEVICE,
            0x2080_1322,
            &[0u8; p::OFFLINED_PAGES_SIZE],
        );
        assert_eq!(get32(&a.params, p::OFFLINED_VALID_ENTRIES), 1);
        assert_eq!(get64(&a.params, p::OFFLINED_ECC_OFF), (RES.0 >> 12) + 1);
    }

    /// Without a reservation no host page is a guest page: the list is empty.
    #[test]
    fn offlined_pages_without_reservation_are_empty() {
        let host = offlined_answer(&[(u64::MAX, 0x1000, 0), (0x2000, 0x2000, 0)], false);
        let (a, _) = run(host, 0x2080_1322, None);
        assert_eq!(get32(&a.params, p::OFFLINED_VALID_ENTRIES), 0);
        assert!(a.params.iter().all(|&x| x == 0));
        assert!(a.label.contains("no reservation"), "{}", a.label);
    }

    /// Remapped rows never reach the guest; the card flags do.
    #[test]
    fn remapped_rows_are_withheld() {
        let mut host = vec![0xABu8; p::REMAPPED_ROWS_SIZE];
        put32(&mut host, p::REMAPPED_ROWS_ENTRY_COUNT, 3);
        host[p::REMAPPED_ROWS_FLAGS] = 1;
        let (a, _) = run(host, 0x2080_1344, Some(RES));
        assert_eq!(get32(&a.params, p::REMAPPED_ROWS_ENTRY_COUNT), 0);
        assert_eq!(a.params[p::REMAPPED_ROWS_FLAGS], 1);
        assert!(a.params[p::REMAPPED_ROWS_ENTRIES..].iter().all(|&x| x == 0));
    }

    /// A buffer of another size is refused before any host call.
    #[test]
    fn wrong_size_is_not_forwarded() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = Canned(vec![0; 8192], seen.clone());
        let mut ctx = Ctx {
            host: Some(&mut h),
            reservation: Some(RES),
            chip: tu106(),
            now_ns: 0,
            object: (0, 0),
            key: (0, 0),
            mirror: None,
            attrs: &mut GuestAttrs::default(),
            rc: &mut crate::rc::RcState::default(),
            reg: None,
            link_control: 0,
        };
        let a = answer(&mut ctx, 0x2080_1322, &[0u8; 2048]).unwrap();
        assert_eq!(a.status, NV_ERR_INVALID_PARAM_STRUCT);
        assert!(seen.lock().unwrap().is_empty());
    }

    /// The scrub range the guest sees lies in the reservation, or is empty.
    #[test]
    fn scrubber_range_is_clamped() {
        let mk = |s: u64, e: u64| {
            let mut b = vec![0u8; p::SCRUBBER_SIZE];
            put32(&mut b, 0, 1);
            put64(&mut b, p::SCRUBBER_START, s);
            put64(&mut b, p::SCRUBBER_END, e);
            b
        };
        let range = |a: &Answer| {
            (
                get64(&a.params, p::SCRUBBER_START),
                get64(&a.params, p::SCRUBBER_END),
            )
        };
        let (a, _) = run(mk(0, 0x2_0000_0000), 0x2080_015F, Some(RES));
        assert_eq!(range(&a), RES);
        assert_eq!(get32(&a.params, 0), 1, "status passes");
        let (a, _) = run(mk(0x1000, 0x2000), 0x2080_015F, Some(RES));
        assert_eq!(range(&a), (0, 0));
        let (a, _) = run(mk(RES.0 + 0x1000, 0x2_0000_0000), 0x2080_015F, Some(RES));
        assert_eq!(range(&a), (RES.0 + 0x1000, RES.1));
        let (a, _) = run(mk(0, 0x2_0000_0000), 0x2080_015F, None);
        assert_eq!(range(&a), (0, 0));
    }

    /// Dynamic offlined pages: three host pages (below, inside, above), one reaches the guest.
    #[test]
    fn dynamic_offlined_pages_keep_the_reservation() {
        let mut b = vec![0u8; p::DYN_OFFLINED_SIZE];
        for (i, a) in [0x1000u64, RES.0 + 0x3000, RES.1 + 0x1000]
            .into_iter()
            .enumerate()
        {
            put64(&mut b, i * 16, a >> 12);
            b[i * 16 + 8] = 2;
        }
        put32(&mut b, p::DYN_OFFLINED_VALID_ENTRIES, 3);
        b[1032] = 1; // bMore
        let (a, seen) = run(b, 0x2080_1348, Some(RES));
        assert_eq!(seen, [(NV20_SUBDEVICE_0, 0x2080_1348)]);
        assert_eq!(get32(&a.params, p::DYN_OFFLINED_VALID_ENTRIES), 1);
        assert_eq!(get64(&a.params, 0), (RES.0 + 0x3000) >> 12);
        assert_eq!(a.params[8], 2);
        assert!(a.params[16..p::DYN_OFFLINED_VALID_ENTRIES]
            .iter()
            .all(|&x| x == 0));
        assert_eq!(a.params[1032], 1);
    }

    /// A host PID and process name in a GPUMON sample come back as 0; nvjpg and nvofa,
    /// which TU106 lacks, carry no utilization; the others keep the card's.
    #[test]
    fn gpumon_host_processes_are_zeroed() {
        let mut b = vec![0u8; p::GPUMON_SIZE];
        put32(&mut b, p::GPUMON_COUNT, 72);
        for i in 0..p::GPUMON_SAMPLE_COUNT {
            let s = p::GPUMON_SAMPLES + i * p::GPUMON_SAMPLE_SIZE;
            for (_, off) in p::GPUMON_ENGINES {
                let e = s + off;
                put32(&mut b, e, 4200); // util
                put32(&mut b, e + 8, 1234); // procId: a host PID
                put32(&mut b, e + 12, 99);
                b[e + 16..e + 24].copy_from_slice(b"Xwayland");
                put64(&mut b, e + 120, 0xFFFF_8880_1234_5678);
            }
        }
        let (a, _) = run(b, 0x2080_2096, Some(RES));
        for i in 0..p::GPUMON_SAMPLE_COUNT {
            let s = p::GPUMON_SAMPLES + i * p::GPUMON_SAMPLE_SIZE;
            for (engine, off) in p::GPUMON_ENGINES {
                let e = s + off;
                assert!(a.params[e + 8..e + 128].iter().all(|&x| x == 0), "{engine}");
                let want = if ["nvjpg", "nvofa"].contains(engine) {
                    0
                } else {
                    4200
                };
                assert_eq!(get32(&a.params, e), want, "{engine}");
            }
        }
        assert_eq!(get32(&a.params, p::GPUMON_COUNT), 72);
    }

    /// The RM heap stats are the device's: zeros, and the host is not asked.
    #[test]
    fn rm_heap_stats_are_the_devices() {
        let mut b = vec![0xEEu8; p::RM_HEAP_STATS_SIZE];
        put32(&mut b, 0, 3); // gfid of a VF
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = Canned(vec![0x11; 8192], seen.clone());
        let mut ctx = Ctx {
            host: Some(&mut h),
            reservation: Some(RES),
            chip: tu106(),
            now_ns: 0,
            object: (0, 0),
            key: (0, 0),
            mirror: None,
            attrs: &mut GuestAttrs::default(),
            rc: &mut crate::rc::RcState::default(),
            reg: None,
            link_control: 0,
        };
        let a = answer(&mut ctx, 0x2080_3602, &b).unwrap();
        assert!(seen.lock().unwrap().is_empty());
        assert_eq!(a.status, NV_OK);
        assert!(a.params.iter().all(|&x| x == 0));
    }

    /// Guard (hostrm.rs:84 used to send any other class to the host root client): every
    /// FORWARD row and every filtered row of the table reaches the host only on the
    /// subdevice, device or BINAPI object, the one its id addresses, whatever object the
    /// guest named; and the two lists never overlap.
    #[test]
    fn every_forward_targets_subdevice_device_or_binapi() {
        use crate::ctrl::{Dispatcher, INTERNAL_CLIENT, INTERNAL_SUBDEVICE};
        use crate::hostrm::{class_of_control, host_object};
        use crate::ver::active::FORWARD;
        type Seen = std::sync::Arc<std::sync::Mutex<Vec<(u32, u32)>>>;
        struct Rec(Seen);
        impl HostRm for Rec {
            fn control(&mut self, class: u32, cmd: u32, _: &mut [u8]) -> u32 {
                self.0.lock().unwrap().push((class, cmd));
                NV_OK
            }
        }
        let forwarded: Vec<(u32, &str)> = FORWARD
            .iter()
            .copied()
            .chain(
                p::PUBLIC
                    .iter()
                    .filter(|c| matches!(c.rule, Rule::Filtered(_)))
                    .map(|c| (c.id, c.name)),
            )
            .collect();
        for &(id, name) in &forwarded {
            let class = class_of_control(id).unwrap_or_else(|| panic!("{name}: no host object"));
            assert!(host_object(class).is_some(), "{name}");
            assert!(
                !(FORWARD.iter().any(|(c, _)| *c == id) && p::public_control(id).is_some()),
                "{name} is in FORWARD and in the table"
            );
        }
        let seen: Seen = Default::default();
        let f = crate::fixture::rtx2070();
        let mut d = Dispatcher::new(
            f.chip.data(),
            &f.identity(4 << 30),
            Some(Box::new(Rec(seen.clone()))),
        )
        .with_reservation(Some(RES));
        // The guest names objects of other classes too: a diag object and a ZBC object.
        d.alloc(0xA, 0xA, 0, crate::ver::active::NV01_ROOT, &[]);
        d.alloc(0xA, 0x20, 0xA, 0x208F, &[]);
        d.alloc(0xA, 0x21, 0xA, 0x9096, &[]);
        for &(id, name) in &forwarded {
            let size = p::public_control(id).map_or(64, |c| c.size);
            for (c, o) in [(INTERNAL_CLIENT, INTERNAL_SUBDEVICE), (0xA, 0x20), (0xA, 0x21)] {
                seen.lock().unwrap().clear();
                d.control(c, o, id, &vec![0u8; size]);
                assert_eq!(
                    seen.lock().unwrap().as_slice(),
                    &[(class_of_control(id).unwrap(), id)],
                    "{name} on {o:#x}"
                );
            }
        }
    }

    /// The 26 F1 rows of b-host-forwardable.md are plain forwards, among them the two
    /// illumination controls seen in the run logs.
    #[test]
    fn f1_rows_are_forwarded() {
        use crate::ver::active::FORWARD;
        let f1 = [
            0x2080_0133u32, 0x2080_0153, 0x2080_0154, 0x2080_0173, 0x2080_01AB, 0x2080_01AE,
            0x2080_0809, 0x2080_1037, 0x2080_1120, 0x2080_130C, 0x2080_1328, 0x2080_1347,
            0x2080_1355, 0x2080_1358, 0x2080_1816, 0x2080_1817, 0x2080_200B, 0x2080_205A,
            0x2080_206E, 0x2080_3101, 0x2080_3400, 0x2080_3401, 0x2080_3404, 0x2080_3406,
            0x2080_3604, 0x2080_81F3,
        ];
        assert_eq!(f1.len(), 26);
        for id in f1 {
            assert!(FORWARD.iter().any(|(c, _)| *c == id), "{id:#x}");
        }
    }

    fn ask(cmd: u32, params: &[u8], now_ns: u64) -> (Answer, Vec<(u32, u32)>) {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = Canned(vec![0x5A; 65536], seen.clone());
        let mut ctx = Ctx {
            host: Some(&mut h),
            reservation: Some(RES),
            chip: tu106(),
            now_ns,
            object: (0, 0),
            key: (0, 0),
            mirror: None,
            attrs: &mut GuestAttrs::default(),
            rc: &mut crate::rc::RcState::default(),
            reg: None,
            link_control: 0,
        };
        let a = answer(&mut ctx, cmd, params).unwrap();
        let s = seen.lock().unwrap().clone();
        (a, s)
    }

    /// The S rows: answered by the device, never sent to the host.
    #[test]
    fn device_rows_never_reach_the_host() {
        // Time correlation: every requested sample is the device clock.
        let mut t = vec![0u8; p::TIME_CORR_SIZE];
        t[0] = 0x14; // _GSP_OS on _PROCESSOR_GSP
        t[1] = 3;
        let (a, seen) = ask(0x2080_0406, &t, 123_456);
        assert!(seen.is_empty());
        assert_eq!(a.status, NV_OK);
        for i in 0..3 {
            let o = p::TIME_CORR_SAMPLES + 16 * i;
            assert_eq!((get64(&a.params, o), get64(&a.params, o + 8)), (123_456, 123_456));
        }
        assert_eq!(get64(&a.params, p::TIME_CORR_SAMPLES + 48), 0);
        // NBSI: not found, nothing returned.
        let (a, seen) = ask(0x2080_080E, &vec![0x77u8; p::NBSI_V2_SIZE], 0);
        assert!(seen.is_empty());
        assert_eq!(get32(&a.params, p::NBSI_V2_ERROR_CODE), p::NBSI_NOT_FOUND);
        assert_eq!(get32(&a.params, p::NBSI_V2_RET_SIZE), 0);
        let buf = p::NBSI_V2_RET_SIZE - p::NBSI_V2_RET_BUF_LEN;
        assert!(a.params[buf..p::NBSI_V2_RET_SIZE].iter().all(|&x| x == 0));
        assert!(a.params[..buf].iter().all(|&x| x == 0x77), "the request is kept");
        // RC recovery, comptags, I2C port speed, SRAM ECC.
        let (a, _) = ask(0x2080_220E, &[0u8; 4], 0);
        assert_eq!(get32(&a.params, 0), p::RC_RECOVERY_ENABLED);
        let (a, _) = ask(0x2080_1338, &vec![0xFFu8; p::CTAGS_SIZE], 0);
        assert!(a.params.iter().all(|&x| x == 0));
        let (a, _) = ask(0x402C_0103, &[0xFFu8; 64], 0);
        assert!(a.params.iter().all(|&x| x == 0));
        let (a, seen) = ask(0x2080_3402, &[0u8; 16], 0);
        assert!(seen.is_empty());
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
    }

    /// The 13 R rows and the 4 GPUACCT rows: NOT_SUPPORTED with a label, request echoed,
    /// no host call.
    #[test]
    fn refusals_are_labelled_and_local() {
        let refused: Vec<&PublicControl> = p::PUBLIC
            .iter()
            .filter(|c| matches!(c.rule, Rule::Refuse(_)))
            .collect();
        assert_eq!(refused.len(), 13 + 4);
        for c in refused {
            let (a, seen) = ask(c.id, &[0xA5; 24], 0);
            assert!(seen.is_empty(), "{}", c.name);
            assert_eq!(a.status, NV_ERR_NOT_SUPPORTED, "{}", c.name);
            assert!(a.label.starts_with("refused ("), "{}", c.name);
            assert_eq!(a.params, [0xA5; 24], "{}", c.name);
        }
    }

    /// GPU_QUERY_ILLUM_SUPPORT and GPU_GET_ILLUM, the rows seen in the run logs: plain
    /// forwards whose host status and bytes reach the guest unchanged.
    #[test]
    fn illum_rows_are_forwarded_as_is() {
        use crate::ctrl::{Dispatcher, INTERNAL_CLIENT, INTERNAL_SUBDEVICE};
        struct Illum;
        impl HostRm for Illum {
            fn control(&mut self, class: u32, cmd: u32, b: &mut [u8]) -> u32 {
                assert_eq!(class, NV20_SUBDEVICE_0);
                match cmd {
                    0x2080_0153 => NV_OK, // the host card: attribute 0, not supported (Q-B1)
                    0x2080_0154 => NV_ERR_NOT_SUPPORTED,
                    _ => {
                        b.fill(0xEE);
                        0xFFFF
                    }
                }
            }
        }
        let f = crate::fixture::rtx2070();
        let mut d = Dispatcher::new(f.chip.data(), &f.identity(4 << 30), Some(Box::new(Illum)));
        let a = d.control(INTERNAL_CLIENT, INTERNAL_SUBDEVICE, 0x2080_0153, &[0u8; 8]);
        assert_eq!((a.status, &a.params[..]), (NV_OK, &[0u8; 8][..]));
        assert_eq!(a.label, "host (forwarded as-is, public control)");
        let a = d.control(INTERNAL_CLIENT, INTERNAL_SUBDEVICE, 0x2080_0154, &[0u8; 8]);
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
        assert_eq!(a.label, "host (forwarded as-is, public control)");
    }

    type Calls = std::sync::Arc<std::sync::Mutex<Vec<(u32, u32, u32)>>>;

    /// A mirror that holds host twins of the guest objects in `mirrored` only.
    struct FakeMirror {
        mirrored: Vec<(u32, u32)>,
        calls: Calls,
    }
    impl MirrorObjects for FakeMirror {
        fn forward(&mut self, c: u32, o: u32, cmd: u32, _: &[u8]) -> Option<(u32, String)> {
            if !self.mirrored.contains(&(c, o)) {
                return None;
            }
            self.calls.lock().unwrap().push((c, o, cmd));
            Some((NV_OK, format!("host object of {o:#x}")))
        }
    }

    /// One control of the C table on guest object `obj`, with a host RM that must never be
    /// called and a mirror of `mirrored`.
    fn guest_call(
        attrs: &mut GuestAttrs,
        mirrored: &[(u32, u32)],
        obj: (u32, u32),
        cmd: u32,
        params: &[u8],
    ) -> (Answer, Vec<(u32, u32, u32)>) {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = Canned(vec![0; 8192], seen.clone());
        let calls: Calls = Default::default();
        let mut m = FakeMirror {
            mirrored: mirrored.to_vec(),
            calls: calls.clone(),
        };
        let mut ctx = Ctx {
            host: Some(&mut h),
            reservation: Some(RES),
            chip: tu106(),
            now_ns: 0,
            object: obj,
            key: obj,
            mirror: Some(&mut m),
            attrs,
            rc: &mut crate::rc::RcState::default(),
            reg: None,
            link_control: 0x3,
        };
        let a = answer(&mut ctx, cmd, params).unwrap();
        assert!(
            seen.lock().unwrap().is_empty(),
            "{cmd:#x} reached the host RM client"
        );
        let c = calls.lock().unwrap().clone();
        (a, c)
    }

    const CH: (u32, u32) = (0xC1, 0x5C00_0010);
    const OTHER: (u32, u32) = (0xC1, 0x5C00_0020);

    /// The resets are recorded by the device and never forwarded, with or without a mirror
    /// channel: on the host a channel reset also resets the engine the guest names
    /// (c-guest-state.md §8).
    #[test]
    fn resets_never_reach_the_host() {
        let mut attrs = GuestAttrs::default();
        for (cmd, host_cmd, size) in [
            (0x906F_0102u32, 0x906F_0102u32, 16usize),
            (0x506F_0105, 0x506F_0105, 8),
            (0x506F_0106, 0x506F_0105, 8),
        ] {
            let (a, calls) = guest_call(&mut attrs, &[CH, OTHER], CH, cmd, &vec![1u8; size]);
            assert_eq!(a.status, NV_OK);
            assert!(calls.is_empty(), "{cmd:#x} reached the mirror as {host_cmd:#x}");
            assert!(a.label.starts_with("guest (reset recorded"), "{}", a.label);
            assert!(a.note.contains(&format!("{host_cmd:#x}")), "{}", a.note);
            // No mirror: the same answer.
            let (a, calls) = guest_call(&mut attrs, &[OTHER], CH, cmd, &vec![1u8; size]);
            assert_eq!(a.status, NV_OK);
            assert!(calls.is_empty());
            assert!(a.label.starts_with("guest (reset recorded"), "{}", a.label);
        }
        assert_eq!(attrs.get(CH).unwrap().resets, 6);
    }

    /// The fault state and the RC journal are the device's: zeros, no host call.
    #[test]
    fn fault_and_rc_state_come_from_the_device() {
        let mut attrs = GuestAttrs::default();
        for (cmd, size) in [
            (0x906F_0106u32, p::MMU_FAULT_INFO_SIZE),
            (0x906F_0105, 1),
            (0x2080_2205, 4),
        ] {
            let (a, calls) = guest_call(&mut attrs, &[CH], CH, cmd, &vec![0xEEu8; size]);
            assert!(calls.is_empty(), "{cmd:#x}");
            assert_eq!(a.status, NV_OK);
            assert!(a.params.iter().all(|&x| x == 0), "{cmd:#x}");
        }
        let mut q = vec![0xEEu8; p::RC_ERROR_V2_SIZE];
        put32(&mut q, 0, 2);
        let (a, _) = guest_call(&mut attrs, &[CH], CH, 0x2080_2213, &q);
        assert_eq!(get32(&a.params, 0), 2, "whichBuffer kept");
        assert_eq!(get32(&a.params, p::RC_ERROR_V2_OUTPUT_SIZE), 0);
        assert!(a.params[8..].iter().all(|&x| x == 0));
    }

    /// Group M records and forwards to the object's own mirror; a level above MEDIUM stays
    /// recorded; the getter reads the record back.
    #[test]
    fn group_m_records_and_forwards() {
        let mut attrs = GuestAttrs::default();
        let tsg = (0xC1, 0x5C00_0001);
        let (a, calls) = guest_call(&mut attrs, &[tsg], tsg, 0xA06C_0107, &1u32.to_le_bytes());
        assert_eq!(a.status, NV_OK);
        assert_eq!(calls, [(tsg.0, tsg.1, 0xA06C_0107)]);
        let (a, calls) = guest_call(&mut attrs, &[tsg], tsg, 0xA06C_0107, &2u32.to_le_bytes());
        assert_eq!(a.status, NV_OK);
        assert!(calls.is_empty(), "HIGH is not forwarded");
        let (_, calls) = guest_call(&mut attrs, &[CH], CH, 0xA06F_0109, &2u32.to_le_bytes());
        assert!(calls.is_empty(), "HIGH is not forwarded");
        // TPC partition mode: set (forwarded), then read back without a host call.
        let mut set = [0u8; 8];
        set[0] = 1; // STATIC
        set[4] = 1;
        let (_, calls) = guest_call(&mut attrs, &[tsg], tsg, 0x0090_0101, &set);
        assert_eq!(calls, [(tsg.0, tsg.1, 0x0090_0101)]);
        let (a, calls) = guest_call(&mut attrs, &[tsg], tsg, 0x0090_0103, &[0u8; 8]);
        assert!(calls.is_empty());
        assert_eq!(a.params, set);
        // Unset on another TSG: NONE.
        let (a, _) = guest_call(&mut attrs, &[], (0xC1, 0x5C00_0002), 0x0090_0103, &[9u8; 8]);
        assert_eq!(a.params, [0u8; 8]);
        // Without a mirror: recorded only.
        let (a, calls) = guest_call(&mut attrs, &[], CH, 0xA06C_0109, &[3u8; 8]);
        assert!(calls.is_empty());
        assert_eq!(a.label, "guest (recorded; no mirror object)");
        // FREE forgets the object's record.
        attrs.free(tsg.0, tsg.1);
        assert!(attrs.get(tsg).is_none());
        attrs.free(0xC1, 0xC1);
        assert!(attrs.get(CH).is_none());
    }

    /// Q-C7: MAKE_REALTIME and RESTART_RUNLIST are recorded and never leave the device, not
    /// even on a mirrored object.
    #[test]
    fn realtime_and_runlist_restart_are_never_forwarded() {
        let mut attrs = GuestAttrs::default();
        for (cmd, size) in [(0xA06C_0110u32, 1usize), (0xA06F_0111, 2)] {
            let (a, calls) = guest_call(&mut attrs, &[CH], CH, cmd, &vec![1u8; size]);
            assert_eq!(a.status, NV_OK);
            assert!(calls.is_empty(), "{cmd:#x}");
            assert!(a.label.contains("Q-C7"), "{}", a.label);
            assert!(attrs.get(CH).unwrap().recorded.contains_key(&cmd));
        }
    }

    /// Groups V, K and X are refused without any host or mirror call.
    #[test]
    fn c_refusals() {
        let mut attrs = GuestAttrs::default();
        let refused: Vec<&PublicControl> = p::PUBLIC_C
            .iter()
            .filter(|c| matches!(c.rule, Rule::Refuse(_)))
            .collect();
        // V, K, X; then the round-2 refusals: SETUP_BIND, the two DP readbacks, the two TPC
        // partition setters and the two debugger getters without a session model (the MMU
        // debug mode getter is dmodel.rs's).
        assert_eq!(refused.len(), 8 + 5 + 2 + 7);
        for c in refused {
            let (a, calls) = guest_call(&mut attrs, &[CH], CH, c.id, &[0x3C; 16]);
            assert!(calls.is_empty(), "{}", c.name);
            assert_eq!(a.status, NV_ERR_NOT_SUPPORTED, "{}", c.name);
            assert!(a.label.starts_with("refused ("), "{}", c.name);
        }
    }

    /// NV_EVENT_BUFFER UPDATE_GET is left to the event-buffer model (ALLOC_MEMORY).
    #[test]
    fn event_buffer_update_get_is_not_in_the_table() {
        assert!(p::public_control(0x90CD_0102).is_none());
    }

    /// Through the dispatcher (no mirror, no host): every C row reaches this table ahead of
    /// the other handlers, and a record dies with its object.
    #[test]
    fn the_dispatcher_takes_the_c_rows() {
        use crate::ctrl::Dispatcher;
        let f = crate::fixture::rtx2070();
        let mut d = Dispatcher::new(f.chip.data(), &f.identity(4 << 30), None);
        for c in p::PUBLIC_C {
            let size = if c.size == 0 { 16 } else { c.size };
            let a = d.control(CH.0, CH.1, c.id, &vec![0u8; size]);
            assert_ne!(a.label, "unanswered", "{}", c.name);
            assert!(
                a.label.starts_with("guest (")
                    || a.label.starts_with("refused (")
                    || a.label.starts_with("synthesized ("),
                "{}: {}",
                c.name,
                a.label
            );
        }
        assert!(d.guest_attrs.get(CH).is_some());
        d.free(CH.0, CH.1);
        assert!(d.guest_attrs.get(CH).is_none());
    }

    // ---------------------------------------------------------------------------------
    // Round 2 (the 17 C rows of the D plan, the open B rows, the TSG id).

    use crate::ver::active as va;
    type Abi = va::RmAbi;

    /// The device state the C rows reach: the per-object records and rc.rs's journal and
    /// scheduling flags.
    #[derive(Default)]
    struct St {
        attrs: GuestAttrs,
        rc: crate::rc::RcState,
    }

    /// Guest client 0xC1: TSG 0x30 (kernel id 7 on 615) with channel 0x40 (chid 2, host
    /// mirror) and channel 0x41 (chid 3, software); a bare kernel channel 0x50 (chid 4).
    fn world() -> crate::channel::Registry {
        let chip = crate::fixture::rtx2070().chip.data();
        let mut reg = crate::channel::Registry::new(chip.channel);
        let mut tsg = vec![0u8; <Abi as nvrm_sys::RmAbi>::TSG_PARAMS_SIZE as usize];
        let off = <Abi as nvrm_sys::RmAbi>::TSG_PARAMS_OFF_internalFlags;
        if tsg.len() >= off + 4 {
            tsg[off..off + 4].copy_from_slice(&(7u32 | 1 << 16).to_le_bytes());
        }
        reg.record_alloc(0xC1, 0xC1, 0x30, va::KEPLER_CHANNEL_GROUP_A, &tsg);
        let ch = |chid: u32| {
            let mut b = vec![0u8; <Abi as nvrm_sys::RmAbi>::ALLOC_CHANNEL_PARAMS_SIZE as usize];
            let o = <Abi as nvrm_sys::RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_flags;
            b[o..o + 4].copy_from_slice(&(((chid / 8) << 12) | ((chid % 8) << 8)).to_le_bytes());
            b
        };
        reg.record_alloc(0xC1, 0x30, 0x40, chip.channel.gpfifo, &ch(2));
        reg.record_alloc(0xC1, 0x30, 0x41, chip.channel.gpfifo, &ch(3));
        reg.record_alloc(0xC1, 0xC1, 0x50, chip.channel.gpfifo, &ch(4));
        reg.channel_mut(2).unwrap().mirrored = true;
        reg
    }

    /// A mirror that holds a host twin of chid 2 (guest 0xC1/0x40) only and records every
    /// call it gets.
    #[derive(Default)]
    struct Mirror2 {
        calls: Vec<String>,
        idle_status: u32,
    }
    impl MirrorObjects for Mirror2 {
        fn forward(&mut self, _: u32, o: u32, cmd: u32, _: &[u8]) -> Option<(u32, String)> {
            (o == 0x40).then(|| {
                self.calls.push(format!("forward {cmd:#x}"));
                (NV_OK, String::new())
            })
        }
        fn idle(&mut self, list: &[(u32, u32)], flags: u32, t: u32) -> Option<(u32, String)> {
            self.calls.push(format!("idle {list:x?} {flags:#x} {t}"));
            Some((self.idle_status, "fake host idle".into()))
        }
        fn schedule(&mut self, list: &[(u32, u32)], d: bool, only: bool) -> Option<(u32, String)> {
            self.calls.push(format!("schedule {list:x?} {d} {only}"));
            Some((NV_OK, "fake host disable".into()))
        }
        fn channel_query(
            &mut self,
            c: u32,
            h: u32,
            cmd: u32,
            params: &[u8],
            off: usize,
            client_off: Option<usize>,
        ) -> Option<(u32, Vec<u8>, String)> {
            if (c, h) != (0xC1, 0x40) {
                return None;
            }
            self.calls.push(format!("query {cmd:#x} {off} {client_off:?}"));
            let mut b = params.to_vec();
            b[8..12].copy_from_slice(&77u32.to_le_bytes());
            Some((NV_OK, b, "fake host counters".into()))
        }
        fn resident(&mut self, _: &[u8]) -> Option<(u32, Option<u32>, String)> {
            self.calls.push("resident".into());
            Some((NV_OK, Some(2), "fake: host chid of chid 2".into()))
        }
    }

    /// One control on object `obj` with the registry of [`world`], a host RM that fails the
    /// test when called, and `m` as the mirror.
    fn round2(
        st: &mut St,
        reg: &crate::channel::Registry,
        m: &mut Mirror2,
        obj: (u32, u32),
        cmd: u32,
        params: &[u8],
    ) -> Answer {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = Canned(vec![0; 65536], seen.clone());
        let mut ctx = Ctx {
            host: Some(&mut h),
            reservation: Some(RES),
            chip: tu106(),
            now_ns: 0,
            object: obj,
            key: obj,
            mirror: Some(m),
            attrs: &mut st.attrs,
            rc: &mut st.rc,
            reg: Some(reg),
            link_control: 0x2,
        };
        let a = answer(&mut ctx, cmd, params).unwrap();
        assert!(seen.lock().unwrap().is_empty(), "{cmd:#x} reached the host RM client");
        a
    }

    const SUB: (u32, u32) = (0xC1, 0x5C00_0100);

    /// The 17 category C rows of d-device-model.md §10: twelve are rows of the C table and
    /// have left the D rule table (the generator drops a control another handler answers);
    /// five are answered by the D models (rc.rs, dmodel.rs), have no C row and keep their D
    /// rule with reclass C (the reconciliation of 2026-09-25).
    #[test]
    fn the_17_d_plan_c_rows_are_c_rows() {
        let c_table = [
            0x0080_1714u32, 0x2080_1117, 0x208F_0401, 0x2080_1205, 0x2080_1211, 0x2080_123A,
            0x208F_0704, 0x0073_137E, 0x0073_137F, 0x0080_1108, 0x2080_0AC3, 0x2080_122C,
        ];
        let d_models = [0x2080_110Bu32, 0x2080_0177, 0x2080_0A6A, 0x2080_2207, 0x2080_0198];
        for id in c_table {
            assert!(p::PUBLIC_C.iter().any(|c| c.id == id), "{id:#x}");
            assert!(crate::dmodel::rule(id).is_none(), "{id:#x} still in D");
        }
        for id in d_models {
            assert!(p::public_control(id).is_none(), "{id:#x} has a C row");
            let r = crate::dmodel::rule(id).unwrap_or_else(|| panic!("{id:#x} not in D"));
            assert_eq!(r.reclass, 'C', "{id:#x}");
            assert!(crate::dmodel::MODELLED_HERE.contains(&r.name), "{id:#x}");
        }
        let c_rows = crate::ver::Abi::active()
            .d_rules
            .iter()
            .filter(|r| r.reclass == 'C')
            .count();
        assert_eq!(c_rows, d_models.len());
    }

    /// FIFO_IDLE_CHANNELS: software channels are idle; mirrored ones go to the host idle
    /// with the capped timeout, whose NV_OK and NV_ERR_TIMEOUT are the guest's, another
    /// status is not idle; an unknown channel is 0x33 before any host call.
    #[test]
    fn idle_channels_control() {
        let reg = world();
        let mut attrs = St::default();
        let req = |hs: &[u32], timeout: u32| {
            let mut b = vec![0u8; p::IDLE_CHANNELS_SIZE];
            put32(&mut b, 0, hs.len() as u32);
            for (i, h) in hs.iter().enumerate() {
                put32(&mut b, 4 + 4 * i, *h);
            }
            put32(&mut b, p::IDLE_CHANNELS_FLAGS, 0x5);
            put32(&mut b, p::IDLE_CHANNELS_TIMEOUT, timeout);
            b
        };
        let mut m = Mirror2::default();
        let a = round2(&mut attrs, &reg, &mut m, SUB, 0x0080_1714, &req(&[0x41], 1_000_000));
        assert_eq!(a.status, NV_OK);
        assert!(m.calls.is_empty(), "software only: no host call");
        let a = round2(&mut attrs, &reg, &mut m, SUB, 0x0080_1714, &req(&[0x41, 0x40], 1_000_000));
        assert_eq!(a.status, NV_OK, "{}", a.note);
        assert_eq!(m.calls, ["idle [(c1, 40)] 0x5 50000"]);
        let mut m = Mirror2 { idle_status: NV_ERR_TIMEOUT, ..Default::default() };
        let a = round2(&mut attrs, &reg, &mut m, SUB, 0x0080_1714, &req(&[0x40], 300));
        assert_eq!(a.status, NV_ERR_TIMEOUT);
        assert_eq!(m.calls, ["idle [(c1, 40)] 0x5 300"]);
        let mut m = Mirror2 { idle_status: 0x56, ..Default::default() };
        let a = round2(&mut attrs, &reg, &mut m, SUB, 0x0080_1714, &req(&[0x40], 300));
        assert_eq!(a.status, NV_ERR_TIMEOUT, "a refused host idle is not idle");
        let mut m = Mirror2::default();
        let a = round2(&mut attrs, &reg, &mut m, SUB, 0x0080_1714, &req(&[0x40, 0x99], 300));
        assert_eq!(a.status, NV_ERR_INVALID_OBJECT_HANDLE);
        assert!(m.calls.is_empty());
    }

    /// DISABLE_USERMODE_CHANNELS and the scheduling toggle write the device's one set of
    /// scheduling flags (rc.rs), which FIFO_DISABLE_CHANNELS (the D model) writes too; the
    /// host only for the channels' own mirrors.
    #[test]
    fn scheduling_rows_act_on_the_guest_channel_set() {
        let reg = world();
        let mut attrs = St::default();
        let mut m = Mirror2::default();
        let state = |attrs: &mut St, _: &mut Mirror2, h: u32| {
            let c = reg.channels().find(|c| c.h_object == h).unwrap();
            let e = attrs.rc.channel_enabled(&reg, c) as u8;
            (e, e)
        };
        assert_eq!(state(&mut attrs, &mut m, 0x41), (1, 1));
        // FIFO_DISABLE_CHANNELS has no C row: the D model answers it (rc.rs).
        assert!(p::public_control(0x2080_110B).is_none());
        attrs.rc.disabled.insert((0xC1, 0x41));
        assert_eq!(state(&mut attrs, &mut m, 0x41), (0, 0));
        assert_eq!(state(&mut attrs, &mut m, 0x50), (1, 1));
        attrs.rc.disabled.clear();
        // Usermode: the TSG's channels, not the kernel's bare channel.
        let a = round2(&mut attrs, &reg, &mut m, SUB, 0x2080_1117, &[1]);
        assert_eq!(a.status, NV_OK);
        assert_eq!(m.calls, ["schedule [(c1, 40)] true false"]);
        assert!(attrs.rc.usermode_disabled);
        assert_eq!(state(&mut attrs, &mut m, 0x40), (0, 0));
        assert_eq!(state(&mut attrs, &mut m, 0x50), (1, 1), "the kernel's own channel stays");
        round2(&mut attrs, &reg, &mut m, SUB, 0x2080_1117, &[0]);
        // The device-wide toggle: every channel.
        m.calls.clear();
        round2(&mut attrs, &reg, &mut m, SUB, 0x2080_0AC3, &[1]);
        assert_eq!(m.calls, ["schedule [(c1, 40)] true true"]);
        assert!(attrs.rc.scheduling_disabled);
        assert_eq!(state(&mut attrs, &mut m, 0x50), (0, 0));
        round2(&mut attrs, &reg, &mut m, SUB, 0x2080_0AC3, &[0]);
        assert_eq!(state(&mut attrs, &mut m, 0x40), (1, 1));
    }

    /// CHECK_ENGINE_CONTEXT, ZCULL_MODE, PREEMPTION_BIND, GET_CTXSW_MODES and _STATS:
    /// software channels answer from the device's records, the mirrored one through its
    /// own mirror; the bind setters are recorded, never forwarded.
    #[test]
    fn context_rows() {
        let reg = world();
        let mut attrs = St::default();
        let mut m = Mirror2::default();
        let mut c = vec![0u8; 12];
        put32(&mut c, 0, 0x41);
        put32(&mut c, 4, p::NV2080_ENGINE_TYPE_GRAPHICS);
        let a = round2(&mut attrs, &reg, &mut m, SUB, 0x208F_0401, &c);
        assert_eq!((a.status, a.params[p::CHECK_CTX_EXISTS]), (NV_OK, 0));
        put32(&mut c, 4, 0x13);
        assert_eq!(round2(&mut attrs, &reg, &mut m, SUB, 0x208F_0401, &c).status, NV_ERR_INVALID_ARGUMENT);
        put32(&mut c, 0, 0x99);
        put32(&mut c, 4, p::NV2080_ENGINE_TYPE_GRAPHICS);
        assert_eq!(round2(&mut attrs, &reg, &mut m, SUB, 0x208F_0401, &c).status, NV_ERR_INVALID_CHANNEL);
        // Zcull mode and preemption bind on chid 3, read back by GET_CTXSW_MODES.
        let mut z = vec![0u8; 16];
        put32(&mut z, 0, 0x41);
        put32(&mut z, p::ZCULL_MODE_MODE, 2);
        assert_eq!(round2(&mut attrs, &reg, &mut m, SUB, 0x2080_1205, &z).status, NV_OK);
        let mut pb = vec![0u8; p::PREEMPTION_BIND_SIZE];
        put32(&mut pb, 4, 0xC1);
        put32(&mut pb, 8, 0x41);
        put32(&mut pb, p::PREEMPTION_BIND_GFXP, 1);
        put32(&mut pb, p::PREEMPTION_BIND_CILP, 2);
        assert_eq!(round2(&mut attrs, &reg, &mut m, SUB, 0x2080_1211, &pb).status, NV_OK);
        // The bind on the mirrored channel is recorded too, and not forwarded.
        put32(&mut pb, 8, 0x40);
        assert_eq!(round2(&mut attrs, &reg, &mut m, SUB, 0x2080_1211, &pb).status, NV_OK);
        assert!(m.calls.is_empty(), "{:?}", m.calls);
        let rec = &attrs.attrs.get((0xC1, 0x41)).unwrap().recorded;
        assert_eq!(get32(&rec[&0x2080_1205], p::ZCULL_MODE_MODE), 2);
        assert_eq!(get32(&rec[&0x2080_1211], p::PREEMPTION_BIND_CILP), 2);
        // The Setup bind and the TPC partition setters are refused locally.
        for id in [0x2080_123Au32, 0x0080_1108, 0x2080_122C, 0x0073_137E, 0x0073_137F] {
            let a = round2(&mut attrs, &reg, &mut m, SUB, id, &[0x11; 16]);
            assert_eq!(a.status, NV_ERR_NOT_SUPPORTED, "{id:#x}");
            assert!(a.label.starts_with("refused ("), "{id:#x}");
        }
        assert!(m.calls.is_empty());
    }

    /// BIF_INFO reads the emulated LnkCtl (VALIDATE_MEM_MAP_REQUEST is the D model's BAR0
    /// policy, dmodel.rs).
    #[test]
    fn device_state_rows() {
        let reg = world();
        let mut attrs = St::default();
        let mut m = Mirror2::default();
        assert!(p::public_control(0x2080_0198).is_none());
        // link_control 0x2: L1 enabled, L0s not.
        let a = round2(&mut attrs, &reg, &mut m, SUB, 0x208F_0704, &[0u8; 8]);
        assert_eq!((a.status, get32(&a.params, 4)), (NV_OK, 0));
        let mut b = [0u8; 8];
        put32(&mut b, 0, p::BIF_INFO_L1);
        let a = round2(&mut attrs, &reg, &mut m, SUB, 0x208F_0704, &b);
        assert_eq!((a.status, get32(&a.params, 4)), (NV_OK, 1));
        put32(&mut b, 0, 7);
        assert_eq!(round2(&mut attrs, &reg, &mut m, SUB, 0x208F_0704, &b).status, NV_ERR_INVALID_ARGUMENT);
    }

    /// The RC journal is rc.rs's: what the device raised and the guest logged is counted by
    /// RC_GET_ERROR_COUNT and returned by RC_GET_ERROR_V2 (the record header, no error
    /// blocks); the fault record feeds GET_MMU_FAULT_INFO.
    #[test]
    fn rc_journal() {
        let reg = world();
        let mut attrs = St::default();
        let mut m = Mirror2::default();
        let count = |attrs: &mut St, m: &mut Mirror2| {
            get32(&round2(attrs, &reg, m, SUB, 0x2080_2205, &[0u8; 4]).params, 0)
        };
        let v2 = |attrs: &mut St, m: &mut Mirror2, which: u32| {
            let mut q = vec![0xEEu8; p::RC_ERROR_V2_SIZE];
            put32(&mut q, 0, which);
            round2(attrs, &reg, m, SUB, 0x2080_2213, &q)
        };
        assert_eq!(count(&mut attrs, &mut m), 0);
        let mut fault = vec![0u8; p::MMU_FAULT_INFO_SIZE];
        fault[0..8].copy_from_slice(&0x7f00_1000u64.to_le_bytes());
        attrs.attrs.set_fault((0xC1, 0x40), fault.clone());
        attrs.rc.record(Some(2), 31, "mirror fault".into());
        attrs.rc.record(None, 13, "guest xid".into());
        assert_eq!(count(&mut attrs, &mut m), 2);
        let a = v2(&mut attrs, &mut m, 1);
        assert_eq!(a.status, NV_OK);
        assert_eq!(get32(&a.params, 0), 1, "whichBuffer kept");
        assert_eq!(get32(&a.params, p::RC_ERROR_V2_OUTPUT_SIZE), p::RC_PROTOBUF_RECORD_SIZE as u32);
        let h = p::RC_ERROR_V2_RECORD;
        assert_eq!(
            (a.params[h], a.params[h + 1], a.params[h + 2], get32(&a.params, h + 4)),
            (p::RC_RECORD_GROUP_RM, p::RC_RECORD_TYPE_PROTOBUF, 8, 0)
        );
        assert!(a.params[h + 8..].iter().all(|&x| x == 0));
        let a = v2(&mut attrs, &mut m, 2);
        assert_eq!(get32(&a.params, p::RC_ERROR_V2_OUTPUT_SIZE), 0, "no third record");
        let a = round2(&mut attrs, &reg, &mut m, (0xC1, 0x40), 0x906F_0106, &[0u8; p::MMU_FAULT_INFO_SIZE]);
        assert_eq!(a.params, fault, "the recorded fault of the channel");
        // The clean-history control is the D model's (rc.rs), not a C row.
        assert!(p::public_control(0x2080_2207).is_none());
        attrs.rc.clean_history(&[]);
        assert_eq!(count(&mut attrs, &mut m), 0);
        for i in 0..crate::rc::RC_LOG_MAX + 3 {
            attrs.rc.record(Some(3), i as u32, String::new());
        }
        assert_eq!(count(&mut attrs, &mut m), crate::rc::RC_LOG_MAX as u32);
        assert_eq!(attrs.rc.journal[0].xid, 3, "the oldest go first");
    }

    /// The open B rows: MMU debug mode 0, resident channel only for an own mirror, the
    /// debugger getters (ERRBAR disabled; per-SM status needs the R5 session model, GCC is
    /// not on this chip; the MMU debug mode is dmodel.rs's), and the TSG's kernel id.
    #[test]
    fn b_rows_from_device_state() {
        let reg = world();
        let mut attrs = St::default();
        let mut m = Mirror2::default();
        // GET_CTXSW_MODES of a software channel: what ZCULL_MODE and PREEMPTION_BIND
        // recorded; PM and SMPC off.
        let mut z = vec![0u8; 16];
        put32(&mut z, 0, 0x41);
        put32(&mut z, p::ZCULL_MODE_MODE, 2);
        round2(&mut attrs, &reg, &mut m, SUB, 0x2080_1205, &z);
        let mut pb = vec![0u8; p::PREEMPTION_BIND_SIZE];
        put32(&mut pb, 4, 0xC1);
        put32(&mut pb, 8, 0x41);
        put32(&mut pb, p::PREEMPTION_BIND_GFXP, 1);
        put32(&mut pb, p::PREEMPTION_BIND_CILP, 2);
        round2(&mut attrs, &reg, &mut m, SUB, 0x2080_1211, &pb);
        let mut q = vec![0xEEu8; 24];
        put32(&mut q, 0, 0x41);
        let a = round2(&mut attrs, &reg, &mut m, SUB, 0x2080_1235, &q);
        assert_eq!(a.status, NV_OK);
        assert_eq!(
            (get32(&a.params, p::CTXSW_MODES_ZCULL), get32(&a.params, p::CTXSW_MODES_CILP), get32(&a.params, p::CTXSW_MODES_GFXP)),
            (2, 2, 1)
        );
        assert_eq!((get32(&a.params, 8), get32(&a.params, 12)), (0, 0), "PM and SMPC off");
        // GET_CTXSW_STATS: zeros for software, the mirror's for the mirrored channel.
        let mut st = vec![0xEEu8; 32];
        put32(&mut st, 0, 0x41);
        let a = round2(&mut attrs, &reg, &mut m, SUB, 0x2080_1215, &st);
        assert_eq!(get32(&a.params, 0), 0x41);
        assert!(a.params[8..].iter().all(|&x| x == 0));
        put32(&mut st, 0, 0x40);
        let a = round2(&mut attrs, &reg, &mut m, SUB, 0x2080_1215, &st);
        assert_eq!((a.status, get32(&a.params, 8)), (NV_OK, 77));
        assert_eq!(get32(&a.params, 0), 0x40, "the guest's handle comes back");
        assert_eq!(m.calls, ["query 0x20801215 0 None"]);
        m.calls.clear();
        // FIFO_GET_CHANNEL_STATE: the device's scheduling flags.
        let state = |attrs: &mut St, m: &mut Mirror2, h: u32| {
            let mut b = vec![0u8; 16];
            put32(&mut b, 0, h);
            put32(&mut b, 4, 0xC1);
            let a = round2(attrs, &reg, m, (0xC1, 0x208F_0001), 0x208F_0403, &b);
            (a.status, a.params[p::CHANNEL_STATE_ENABLED], a.params[p::CHANNEL_STATE_SCHEDULED])
        };
        assert_eq!(state(&mut attrs, &mut m, 0x41), (NV_OK, 1, 1));
        round2(&mut attrs, &reg, &mut m, SUB, 0x2080_1117, &[1]);
        assert_eq!(state(&mut attrs, &mut m, 0x41), (NV_OK, 0, 0));
        assert_eq!(state(&mut attrs, &mut m, 0x50), (NV_OK, 1, 1), "the kernel's own channel");
        assert_eq!(state(&mut attrs, &mut m, 0x99).0, NV_ERR_INVALID_CHANNEL);
        m.calls.clear();
        let a = round2(&mut attrs, &reg, &mut m, (0xC1, 0x30), 0x0090_0105, &[1]);
        assert_eq!((a.status, a.params[0]), (NV_OK, 0));
        let a = round2(&mut attrs, &reg, &mut m, SUB, 0x2080_121C, &[0xAA; 24]);
        assert_eq!((a.status, get32(&a.params, 0)), (NV_OK, 2));
        assert_eq!(a.params[8..], [0xAA; 16], "grRouteInfo kept");
        assert_eq!(m.calls, ["resident"]);
        // No mirror: no guest channel is resident on the physical GR.
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = Canned(vec![0; 64], seen.clone());
        let mut ctx = Ctx {
            host: Some(&mut h),
            reservation: None,
            chip: tu106(),
            now_ns: 0,
            object: SUB,
            key: SUB,
            mirror: None,
            attrs: &mut attrs.attrs,
            rc: &mut attrs.rc,
            reg: Some(&reg),
            link_control: 0,
        };
        let a = answer(&mut ctx, 0x2080_121C, &[0u8; 24]).unwrap();
        assert_eq!(get32(&a.params, 0), u32::MAX);
        assert!(seen.lock().unwrap().is_empty());
        let a = round2(&mut attrs, &reg, &mut m, (0xC1, 0x83DE), 0x83DE_0320, &[0xFF; 4]);
        assert_eq!((a.status, get32(&a.params, 0)), (NV_OK, p::ERRBAR_DEBUG_DISABLED));
        // DEBUG_GET_MODE_MMU_DEBUG is the debugger-session record of dmodel.rs.
        assert!(p::public_control(0x83DE_0308).is_none());
        for id in [0x83DE_0325u32, 0x83DE_032B] {
            let a = round2(&mut attrs, &reg, &mut m, (0xC1, 0x83DE), id, &[0x5A; 12]);
            assert_eq!(a.status, NV_ERR_NOT_SUPPORTED, "{id:#x}");
            assert_eq!(a.params, [0x5A; 12], "{id:#x}: echoed");
        }
        // GET_INFO: the kernel's TSG id (615) or no id (610).
        let a = round2(&mut attrs, &reg, &mut m, (0xC1, 0x30), 0xA06C_0106, &[0; 4]);
        if reg.tsg_id((0xC1, 0x30)).is_some() {
            assert_eq!((a.status, get32(&a.params, 0)), (NV_OK, 7));
        } else {
            assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
        }
    }
}
