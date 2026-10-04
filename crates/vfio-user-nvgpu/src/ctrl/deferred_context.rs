// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Ordered Windows context lifecycle on host-managed shadow contexts.

use super::{Answer, Dispatcher, NV_ERR_NOT_SUPPORTED, NV_OK};
use crate::channel::{GpuMem, Registry};
use crate::deferred::{self, Request};
use crate::fb::Target;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TargetStamp {
    chid: u32,
    generation: u64,
    client: u32,
    object: u32,
    root: u64,
}

impl TargetStamp {
    fn resolve(reg: &Registry, request: Request) -> Result<Self, String> {
        let target = request.target();
        let ch = reg
            .channel(target.channel_id)
            .ok_or("target ChID is not live")?;
        if ch.h_client != target.h_channel_client
            || ch.h_parent != target.h_object
            || ch.engine_type != 1
            || ch.alloc_flags & (1 << 31) == 0
            || reg.tsg((ch.h_client, ch.h_parent)).is_none()
        {
            return Err("target is not the exact skip-context GR channel/TSG".into());
        }
        if reg
            .channels()
            .filter(|c| c.h_client == ch.h_client && c.h_parent == ch.h_parent)
            .count()
            != 1
        {
            return Err("deferred multi-channel TSG is not implemented".into());
        }
        let Some(Target::Fb(root)) = reg.vas_key_of(ch).and_then(|key| reg.vas_root(key)) else {
            return Err("target VAS has no framebuffer root".into());
        };
        Ok(Self {
            chid: ch.chid,
            generation: ch.generation,
            client: ch.h_client,
            object: ch.h_parent,
            root,
        })
    }

    fn live(self, reg: &Registry) -> bool {
        reg.channel(self.chid).is_some_and(|ch| {
            ch.generation == self.generation
                && ch.h_client == self.client
                && ch.h_parent == self.object
                && reg.tsg((self.client, self.object)).is_some()
                && reg.vas_key_of(ch).and_then(|key| reg.vas_root(key))
                    == Some(Target::Fb(self.root))
        })
    }
}

#[derive(Clone, Copy)]
struct Registered {
    owner_chid: u32,
    owner_generation: u64,
    target: TargetStamp,
    request: Request,
}

#[derive(Clone, Copy)]
struct Context {
    size: u64,
    promoted: bool,
}

pub(super) struct State {
    pub(super) enabled: bool,
    requests: BTreeMap<(u32, u32, u32), Registered>,
    management: BTreeMap<u32, u64>,
    contexts: BTreeMap<TargetStamp, Context>,
    active: BTreeMap<u32, u64>,
    takeover_refused: BTreeMap<u32, u64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            enabled: std::env::var("CARAXES_DEFERRED_CONTEXT").as_deref() == Ok("1"),
            requests: BTreeMap::new(),
            management: BTreeMap::new(),
            contexts: BTreeMap::new(),
            active: BTreeMap::new(),
            takeover_refused: BTreeMap::new(),
        }
    }
}

impl State {
    /// A skip-context GR channel submits only once its TSG's context is promoted and live.
    /// The context is the TSG's: the guest promotes it once, on the channel it names, and
    /// the TSG's other channels (Windows CUDA: eight, one per subcontext, run P26) share it.
    pub(super) fn target_submission_blocked(&self, reg: &Registry, chid: u32) -> bool {
        let Some(ch) = reg.channel(chid) else {
            return false;
        };
        self.enabled
            && ch.engine_type == 1
            && ch.alloc_flags & (1 << 31) != 0
            && !self.contexts.iter().any(|(target, context)| {
                let own = target.chid == chid && target.generation == ch.generation;
                let member = target.client == ch.h_client && target.object == ch.h_parent;
                (own || member) && context.promoted && target.live(reg)
            })
    }

    pub(super) fn is_management(&self, reg: &Registry, chid: u32) -> bool {
        self.enabled
            && self.management.get(&chid).is_some_and(|generation| {
                reg.channel(chid)
                    .is_some_and(|ch| ch.generation == *generation)
            })
    }

    /// A context stays with its TSG while any channel of it lives: the guest promotes it on
    /// one channel and evicts it through whichever channel is left (run P29b: Windows CUDA
    /// frees the promoted channel first and evicts through the last of its eight; the
    /// eviction failed "not promoted", an Xid 45 RC on the management channel and a TDR at
    /// every CUDA process exit). A context whose channel is gone moves to the lowest live
    /// skip-context GR channel of the same TSG on the same root; with none it is dropped.
    fn rehome_contexts(&mut self, reg: &Registry) {
        let stale: Vec<TargetStamp> = self
            .contexts
            .keys()
            .filter(|t| !t.live(reg))
            .copied()
            .collect();
        for old in stale {
            let context = self.contexts.remove(&old).unwrap();
            let heir = reg
                .channels()
                .filter(|ch| {
                    ch.h_client == old.client
                        && ch.h_parent == old.object
                        && ch.engine_type == 1
                        && ch.alloc_flags & (1 << 31) != 0
                })
                .map(|ch| TargetStamp {
                    chid: ch.chid,
                    generation: ch.generation,
                    client: old.client,
                    object: old.object,
                    root: old.root,
                })
                .filter(|t| t.live(reg) && !self.contexts.contains_key(t))
                .min();
            if let Some(heir) = heir {
                self.contexts.insert(heir, context);
            }
        }
    }

    pub(super) fn prune(&mut self, reg: &Registry) {
        self.requests.retain(|(client, object, _), r| {
            r.target.live(reg)
                && reg.engine_owner(*client, *object) == Some((r.owner_chid, r.owner_generation))
        });
        self.rehome_contexts(reg);
        self.management.retain(|chid, generation| {
            reg.channel(*chid)
                .is_some_and(|ch| ch.generation == *generation)
        });
        self.active.retain(|chid, generation| {
            reg.channel(*chid)
                .is_some_and(|ch| ch.generation == *generation)
        });
        self.takeover_refused.retain(|chid, generation| {
            reg.channel(*chid)
                .is_some_and(|ch| ch.generation == *generation)
        });
    }
}

impl Dispatcher {
    pub(super) fn register_deferred(
        &mut self,
        client: u32,
        object: u32,
        cmd: u32,
        params: &[u8],
    ) -> Answer {
        self.deferred_context.prune(&self.channels);
        let owner = self
            .channels
            .engine_object(client, object)
            .map(|(chid, generation, _, _)| (chid, generation));
        let was_management =
            owner.is_some_and(|(c, g)| self.deferred_context.management.get(&c) == Some(&g));
        // A ring migration (NV2080_CTRL_CMD_FIFO_UPDATE_CHANNEL_INFO as the nested command):
        // not a context request; the host runs it in the owner's stream or the device's
        // executor at the method, so the owner is never taken over for it.
        let migration = deferred::is_registration(cmd)
            && deferred::nested_command(params) == Some(deferred::CMD_UPDATE_CHANNEL_INFO);
        let result = (|| -> Result<String, String> {
            let (chid, generation, class, _) = self
                .channels
                .engine_object(client, object)
                .ok_or("deferred object has no live channel owner")?;
            if class != 0x5080 {
                return Err("control target is not a deferred object".into());
            }
            if migration {
                return self.register_channel_migration(object, chid, params);
            }
            // Even a refused registration must never leave its software-class method
            // eligible for raw host submission with a missing host API handle.
            if cmd != deferred::CMD_REMOVE_API {
                self.deferred_context.management.insert(chid, generation);
            }
            if cmd == deferred::CMD_REMOVE_API {
                let bytes: [u8; 4] = params
                    .try_into()
                    .map_err(|_| "remove API requires four bytes")?;
                let api = u32::from_le_bytes(bytes);
                if self.channels.remove_ring_switch(chid, api) {
                    return Ok(format!("deferred API {api:#x} (ring migration) removed"));
                }
                if let Some(n) = self
                    .mirror
                    .as_mut()
                    .and_then(|m| m.cancel_migration(chid, object, api))
                {
                    return n;
                }
                return self
                    .deferred_context
                    .requests
                    .remove(&(client, object, api))
                    .map(|_| format!("deferred API {api:#x} removed"))
                    .ok_or_else(|| "deferred API is unknown or already consumed".into());
            }
            if !deferred::is_registration(cmd) {
                // The request (NV5080_CTRL_DEFERRED_API[_V2]_PARAMS, one layout): what it asks for.
                let rd = |o: usize| {
                    params
                        .get(o..o + 4)
                        .map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()))
                };
                let bundle: Vec<String> =
                    (0..8).map(|i| format!("{:08x}", rd(24 + 4 * i))).collect();
                return Err(format!(
                    "unknown deferred API control (control {cmd:#x}: hApiHandle {:#x} cmd {:#x} flags {:#x} hClientVA {:#x} hDeviceVA {:#x}, bundle {})",
                    rd(0),
                    rd(4),
                    rd(8),
                    rd(12),
                    rd(16),
                    bundle.join(" ")
                ));
            }
            let request = deferred::parse(params).map_err(|e| e.to_string())?;
            let target = TargetStamp::resolve(&self.channels, request)?;
            let owner = self
                .channels
                .channel(chid)
                .ok_or("deferred owner is gone")?;
            if owner.engine_type != 1
                || owner.alloc_flags & 0x20 == 0
                || owner.alloc_flags & (1 << 31) != 0
                || chid == target.chid
            {
                return Err(
                    "deferred caller is not a distinct privileged management channel".into(),
                );
            }
            if let Request::Initialize {
                phys_address,
                size,
                index,
                ..
            } = request
            {
                if index != target.chid || size == 0 || size % 256 != 0 || phys_address % 4096 != 0
                {
                    return Err("unsupported legacy context index, size or alignment".into());
                }
                if self
                    .fb_layout
                    .and_then(|layout| layout.guest_range_to_host(phys_address, size))
                    .is_none()
                {
                    return Err("deferred context range is outside this VM's framebuffer".into());
                }
            }
            if let Request::Promote {
                virt_address, size, ..
            } = request
            {
                if size == 0
                    || size % 256 != 0
                    || virt_address % 4096 != 0
                    || virt_address
                        .checked_add(size)
                        .is_none_or(|end| end > (1 << 49))
                {
                    return Err("unsupported legacy context virtual range".into());
                }
            }
            let key = (client, object, request.api_handle());
            if request.api_handle() == 0 || self.deferred_context.requests.contains_key(&key) {
                return Err("deferred API handle is zero or already registered".into());
            }
            if self.deferred_context.requests.len() >= 4096 {
                return Err("deferred API table is full".into());
            }
            self.deferred_context.requests.insert(
                key,
                Registered {
                    owner_chid: chid,
                    owner_generation: generation,
                    target,
                    request,
                },
            );
            self.deferred_context.management.insert(chid, generation);
            Ok(format!("deferred API {:#x} registered on {client:#x}/{object:#x} chid {chid}, nested {:#x} -> target chid {} generation {} (pending execution)", request.api_handle(), request.command(), target.chid, target.generation))
        })();
        match result {
            Ok(note) => Answer {
                status: NV_OK,
                params: params.to_vec(),
                label: "guest (deferred registration)",
                note,
            },
            Err(mut note) => {
                // A refused registration on a channel the host runs: its method goes to the
                // host, which completes it with the error as GSP-RM would for an unknown
                // handle, instead of a takeover that cannot happen past the channel's start
                // (preview_deferred_takeover). A software owner keeps the marking: the
                // device completes that method with the error itself.
                if let Some((c, _)) = owner {
                    let hosted = self.mirror.as_ref().is_some_and(|m| m.has_host_channel(c));
                    if hosted
                        && !was_management
                        && self.deferred_context.management.remove(&c).is_some()
                    {
                        note.push_str(
                            "; the owner is not taken over (its method completes on the host)",
                        );
                    }
                }
                Answer {
                    status: NV_ERR_NOT_SUPPORTED,
                    params: params.to_vec(),
                    label: "refused (deferred context)",
                    note,
                }
            }
        }
    }

    /// A deferred NV2080_CTRL_CMD_FIFO_UPDATE_CHANNEL_INFO (ctrl2080fifo.h:693-725) on the
    /// 0x5080 object `object` of channel `owner`, registered as GSP-RM registers any bundle
    /// (deferred_api.c `defapiCtrlCmdDeferredApiInternal_IMPL`), run where GSP-RM runs it: at
    /// the owner's software method (`_class5080DeferredApiV2`). A channel on the host: the
    /// host RM registers and runs it in the host channel's stream (mirror.rs
    /// `begin_migration`). A channel the device executes: its executor switches the ring
    /// after the method's entry (channel.rs `RingSwitch`).
    fn register_channel_migration(
        &mut self,
        object: u32,
        owner: u32,
        params: &[u8],
    ) -> Result<String, String> {
        let (api, flags, info) =
            deferred::parse_channel_info_request(params).map_err(|e| e.to_string())?;
        let (userd, initial_get) = self
            .migration_userd
            .take()
            .ok_or("the new USERD memory was not resolved")??;
        let target = self
            .channels
            .channels()
            .find(|c| c.h_client == info.h_client && c.h_object == info.h_channel)
            .map(|c| (c.chid, c.generation, c.mirrored))
            .ok_or("hChannel is no channel of hClient")?;
        let owner_mirrored = self.channels.channel(owner).is_some_and(|c| c.mirrored);
        if target.2 || owner_mirrored {
            // kernel_fifo_ctrl.c: only a client-allocated USERD; the host channel's private
            // USERD mirrors a guest system-memory USERD.
            let Target::Sys(new_guest) = userd else {
                return Err("a host channel's new USERD must be guest system memory".into());
            };
            if !(target.2 && owner_mirrored) {
                return Err(
                    "a ring migration between a host channel and a software one is not modelled"
                        .into(),
                );
            }
            let mirror = self.mirror.as_mut().ok_or("no host mirror")?;
            return mirror.begin_migration(
                &self.channels,
                owner,
                object,
                target.0,
                api,
                flags,
                &info,
                new_guest,
                initial_get,
            );
        }
        if flags != 0 {
            return Err(format!(
                "deferred flags {flags:#x} for a software channel's ring migration"
            ));
        }
        self.channels.add_ring_switch(
            owner,
            crate::channel::RingSwitch {
                api,
                target: target.0,
                target_generation: target.1,
                userd,
                gpfifo_va: info.gp_fifo_offset,
                entries: info.gp_fifo_entries,
            },
        )?;
        Ok(format!(
            "deferred API {api:#x} (UpdateChannelInfo) registered on chid {owner}: chid {} moves to {} entries at GPFIFO va {:#x}, USERD {userd:?} (guest GPGet {initial_get}) at the method [guest, software channel]",
            target.0, info.gp_fifo_entries, info.gp_fifo_offset
        ))
    }

    /// A direct NV2080_CTRL_CMD_FIFO_UPDATE_CHANNEL_INFO: the channel moves now (RM requires
    /// it idle: the guest disabled it).
    pub(super) fn update_channel_info(&mut self, params: &[u8]) -> Answer {
        let result = (|| -> Result<String, String> {
            let info = deferred::ChannelInfo::parse(params).map_err(|e| e.to_string())?;
            let (userd, initial_get) = self
                .migration_userd
                .take()
                .ok_or("the new USERD memory was not resolved")??;
            let (chid, generation, mirrored) = self
                .channels
                .channels()
                .find(|c| c.h_client == info.h_client && c.h_object == info.h_channel)
                .map(|c| (c.chid, c.generation, c.mirrored))
                .ok_or("hChannel is no channel of hClient")?;
            if mirrored {
                let Target::Sys(new_guest) = userd else {
                    return Err("a host channel's new USERD must be guest system memory".into());
                };
                let mirror = self.mirror.as_mut().ok_or("no host mirror")?;
                let note =
                    mirror.migrate_now(&self.channels, chid, &info, new_guest, initial_get)?;
                let applied = self.apply_migrations();
                return Ok(std::iter::once(note)
                    .chain(applied)
                    .collect::<Vec<_>>()
                    .join("; "));
            }
            self.channels.switch_ring(
                chid,
                Some(generation),
                userd,
                info.gp_fifo_offset,
                info.gp_fifo_entries,
                initial_get,
            )
        })();
        match result {
            Ok(note) => Answer {
                status: NV_OK,
                params: params.to_vec(),
                label: "guest (channel migration)",
                note,
            },
            Err(note) => Answer {
                status: 0x40, // NV_ERR_INVALID_STATE (nvstatuscodes.h), as kernel_fifo_ctrl.c lists it
                params: params.to_vec(),
                label: "refused (channel migration)",
                note,
            },
        }
    }

    /// The registry follows the ring migrations the host completed (mirror.rs
    /// `take_migrations`).
    pub(super) fn apply_migrations(&mut self) -> Vec<String> {
        let done = self
            .mirror
            .as_mut()
            .map(|m| m.take_migrations())
            .unwrap_or_default();
        done.into_iter()
            .map(|(chid, generation, m)| {
                match self.channels.switch_ring(
                    chid,
                    Some(generation),
                    Target::Sys(m.new_guest),
                    m.new_gpfifo_va,
                    m.new_entries,
                    0,
                ) {
                    Ok(n) => format!("registry: {n}"),
                    Err(e) => format!("registry: {e}"),
                }
            })
            .collect()
    }

    pub(super) fn run_deferred_management(
        &mut self,
        gm: &mut GpuMem,
        chid: u32,
        off: &BTreeSet<u32>,
    ) -> Vec<String> {
        let mut notes = Vec::new();
        let generation = match self.channels.channel(chid) {
            Some(ch) => ch.generation,
            None => return vec!["deferred management channel is gone".into()],
        };
        if self.deferred_context.active.get(&chid) != Some(&generation) {
            let Some(mirror) = self.mirror.as_mut() else {
                return vec!["deferred context requires a host mirror".into()];
            };
            if self.deferred_context.takeover_refused.get(&chid) == Some(&generation) {
                return notes;
            }
            let get = match mirror.management_cursor(&self.channels, chid) {
                Ok(get) => get,
                Err(e) => return vec![format!("deferred management chid {chid}: waiting: {e}")],
            };
            let plan = match self.channels.preview_deferred_takeover(gm, chid, get) {
                Ok(plan) => plan,
                Err(e) => {
                    self.deferred_context
                        .takeover_refused
                        .insert(chid, generation);
                    return vec![e];
                }
            };
            match mirror.retire_for_management(&self.channels, gm, chid, get) {
                Ok(n) => notes.extend(n),
                Err(e) => return vec![format!("deferred management chid {chid}: waiting: {e}")],
            }
            if let Err(e) = self.channels.apply_deferred_takeover(plan) {
                return vec![e];
            }
            if let Err(e) = self.channels.enable_deferred_management(chid) {
                return vec![e];
            }
            self.deferred_context.active.insert(chid, generation);
        }
        // Each pass stops at the pending software method before any following release.
        for _ in 0..64 {
            notes.extend(self.channels.run_deferred_one(gm, chid));
            let pending = self.channels.pending_deferred();
            if pending.is_empty() {
                break;
            }
            let mut completed = false;
            for (owner_chid, object, api) in pending {
                if owner_chid != chid || off.contains(&owner_chid) {
                    continue;
                }
                let Some(owner) = self.channels.channel(owner_chid) else {
                    continue;
                };
                let key = (owner.h_client, object, api);
                let entry = self.deferred_context.requests.remove(&key);
                let result = match entry {
                    Some(entry)
                        if entry.owner_chid == owner_chid
                            && entry.owner_generation == owner.generation
                            && entry.target.live(&self.channels) =>
                    {
                        match self.execute_deferred(gm, entry, &mut notes) {
                            // Not ready yet (a context's build on the worker): the method
                            // stays pending and its channel held; the slice clock retries.
                            Ok(false) => {
                                self.deferred_context.requests.insert(key, entry);
                                self.dstate.rc.held.insert(owner_chid);
                                crate::channel::request_ce_slice();
                                continue;
                            }
                            r => r.map(|_| ()),
                        }
                    }
                    _ => Err("unregistered or stale deferred API".into()),
                };
                notes.push(format!(
                    "deferred method chid {owner_chid} API {api:#x}: {}",
                    result
                        .as_ref()
                        .map(|_| "completed".to_string())
                        .unwrap_or_else(|e| e.clone())
                ));
                if let Err(e) = self.channels.complete_deferred(owner_chid, api, result) {
                    notes.push(e);
                }
                completed = true;
            }
            if !completed {
                break;
            }
        }
        notes
    }

    /// Run a deferred request; `Ok(false)` when it is not ready yet (ask again).
    fn execute_deferred(
        &mut self,
        gm: &mut GpuMem,
        entry: Registered,
        notes: &mut Vec<String>,
    ) -> Result<bool, String> {
        let state = &mut self.deferred_context;
        let mirror = self.mirror.as_mut().ok_or("host mirror is unavailable")?;
        match entry.request {
            Request::Initialize { size, .. } => {
                if state.contexts.contains_key(&entry.target) {
                    return Err("context is already initialized".into());
                }
                // The target's build goes to the worker (execution.md §9.12); the method
                // completes once the context is ready, not under the lock while it builds.
                let (ready, n) =
                    mirror.start_deferred_context(&self.channels, gm, entry.target.chid)?;
                notes.extend(n);
                if !ready {
                    return Ok(false);
                }
                state.contexts.insert(
                    entry.target,
                    Context {
                        size,
                        promoted: false,
                    },
                );
            }
            Request::Promote { size, .. } => {
                let context = state
                    .contexts
                    .get_mut(&entry.target)
                    .ok_or("context has not been initialized")?;
                if context.size != size || context.promoted {
                    return Err("context promotion size or lifecycle mismatch".into());
                }
                mirror.promote_deferred_context(&self.channels, entry.target.chid)?;
                context.promoted = true;
            }
            Request::Evict { .. } => {
                if !state
                    .contexts
                    .get(&entry.target)
                    .is_some_and(|ctx| ctx.promoted)
                {
                    return Err("context has not been promoted".into());
                }
                // Close the dispatcher gate before teardown. Any failure keeps the
                // target held, including a partial host FREE failure.
                state.contexts.get_mut(&entry.target).unwrap().promoted = false;
                let (evict_notes, final_get) =
                    mirror.evict_deferred_context(&self.channels, gm, entry.target.chid)?;
                notes.extend(evict_notes);
                self.channels.retire_deferred_target(
                    entry.target.chid,
                    entry.target.generation,
                    final_get,
                )?;
                state.contexts.remove(&entry.target);
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deferred::{ContextTarget, Request, CMD_DEFERRED_API};
    use crate::fb::FbLayout;
    use crate::ver::common::{self as v, FERMI_VASPACE_A, KEPLER_CHANNEL_GROUP_A};
    use nvrm_sys::RmAbi;

    /// The guest's channel allocation layout (the same in every guest ABI).
    type Abi = nvrm_sys::V610;

    fn put32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn channel_params(chid: u32, flags: u32, vas: u32) -> Vec<u8> {
        let mut params = vec![0; <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_SIZE as usize];
        put32(
            &mut params,
            <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_flags,
            flags | ((chid / 8) << 12) | ((chid % 8) << 8),
        );
        put32(
            &mut params,
            <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_gpFifoEntries,
            32,
        );
        put32(
            &mut params,
            <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_hVASpace,
            vas,
        );
        put32(
            &mut params,
            <Abi as RmAbi>::ALLOC_CHANNEL_PARAMS_OFF_engineType,
            1,
        );
        params
    }

    fn dispatcher() -> Dispatcher {
        let fixture = crate::fixture::rtx2070();
        let mut dispatcher = Dispatcher::new(fixture.chip.data(), &fixture.identity(4 << 30), None)
            .with_layout(FbLayout::plain(4 << 30));
        dispatcher.deferred_context.enabled = true;
        dispatcher
    }

    fn add_pair(
        dispatcher: &mut Dispatcher,
        client: u32,
        management_chid: u32,
        target_chid: u32,
    ) -> (u32, u32, u32) {
        let classes = crate::fixture::rtx2070().chip.data().channel;
        let (vas, tsg, management_channel, target_channel, deferred_object) =
            (0x20, 0x30, 0x40, 0x41, 0x77);
        dispatcher
            .channels
            .record_alloc(client, 0x10, vas, FERMI_VASPACE_A, &[]);
        let mut pd = vec![0; 24];
        put64(
            &mut pd,
            v::SET_PD_PHYS_ADDRESS,
            0x10_0000 + u64::from(client) * 0x1000,
        );
        put32(&mut pd, v::SET_PD_NUM_ENTRIES, 1);
        put32(&mut pd, v::SET_PD_H_VASPACE, vas);
        dispatcher.channels.record_set_page_directory(client, &pd);
        let mut tsg_params = vec![0; crate::ver::layouts().tsg_size];
        put32(&mut tsg_params, v::TSG_HVASPACE, vas);
        put32(&mut tsg_params, v::TSG_ENGINE_TYPE, 1);
        dispatcher
            .channels
            .record_alloc(client, 0x10, tsg, KEPLER_CHANNEL_GROUP_A, &tsg_params);
        dispatcher.channels.record_alloc(
            client,
            0x10,
            management_channel,
            classes.gpfifo,
            &channel_params(management_chid, 1 << 5, vas),
        );
        dispatcher.channels.record_alloc(
            client,
            tsg,
            target_channel,
            classes.gpfifo,
            &channel_params(target_chid, 1 << 31, 0),
        );
        dispatcher
            .channels
            .record_alloc(client, management_channel, deferred_object, 0x5080, &[]);
        (deferred_object, target_channel, tsg)
    }

    fn evict(api: u32, client: u32, target_chid: u32, tsg: u32) -> Request {
        Request::Evict {
            api_handle: api,
            target: ContextTarget {
                engine_type: 1,
                h_client: client,
                channel_id: target_chid,
                h_channel_client: client,
                h_object: tsg,
            },
        }
    }

    #[test]
    fn target_submission_requires_live_promotion_even_after_eviction_or_reuse() {
        let mut dispatcher = dispatcher();
        let (_, target_object, tsg) = add_pair(&mut dispatcher, 1, 3, 4);
        let target = TargetStamp::resolve(&dispatcher.channels, evict(1, 1, 4, tsg)).unwrap();
        let blocked = |d: &Dispatcher| d.deferred_context.target_submission_blocked(&d.channels, 4);
        assert!(blocked(&dispatcher)); // Before any API registration or INIT.
        assert!(!dispatcher
            .deferred_context
            .target_submission_blocked(&dispatcher.channels, 3));
        dispatcher.deferred_context.contexts.insert(
            target,
            Context {
                size: 0x2000,
                promoted: false,
            },
        );
        assert!(blocked(&dispatcher));
        dispatcher
            .deferred_context
            .contexts
            .get_mut(&target)
            .unwrap()
            .promoted = true;
        assert!(!blocked(&dispatcher));
        dispatcher
            .deferred_context
            .contexts
            .get_mut(&target)
            .unwrap()
            .promoted = false;
        assert!(blocked(&dispatcher)); // Failed eviction must keep the gate closed.
        dispatcher.deferred_context.contexts.remove(&target);
        assert!(blocked(&dispatcher)); // Successful eviction cannot rebuild on a doorbell.
        dispatcher.deferred_context.contexts.insert(
            target,
            Context {
                size: 0x2000,
                promoted: true,
            },
        );
        dispatcher.channels.record_free(1, target_object);
        dispatcher.deferred_context.prune(&dispatcher.channels); // as Dispatcher::free
        let classes = crate::fixture::rtx2070().chip.data().channel;
        dispatcher.channels.record_alloc(
            1,
            tsg,
            target_object,
            classes.gpfifo,
            &channel_params(4, 1 << 31, 0),
        );
        assert!(blocked(&dispatcher)); // A reused ChID cannot inherit promotion.
        dispatcher.deferred_context.enabled = false;
        assert!(!blocked(&dispatcher));
    }

    #[test]
    fn tsg_members_submit_on_the_tsg_context_promoted_on_another_channel() {
        let mut dispatcher = dispatcher();
        let (_, _, tsg) = add_pair(&mut dispatcher, 1, 3, 4);
        let target = TargetStamp::resolve(&dispatcher.channels, evict(1, 1, 4, tsg)).unwrap();
        // Run P26: CUDA's TSG gets seven more skip-context channels after the promotion.
        let classes = crate::fixture::rtx2070().chip.data().channel;
        for (chid, object) in [(5, 0x42), (6, 0x43)] {
            dispatcher.channels.record_alloc(
                1,
                tsg,
                object,
                classes.gpfifo,
                &channel_params(chid, 1 << 31, 0),
            );
        }
        // A skip-context channel of another client's TSG with the same handles.
        let (_, _, other_tsg) = add_pair(&mut dispatcher, 2, 7, 8);
        dispatcher.channels.record_alloc(
            2,
            other_tsg,
            0x42,
            classes.gpfifo,
            &channel_params(9, 1 << 31, 0),
        );
        let blocked = |d: &Dispatcher, chid| {
            d.deferred_context
                .target_submission_blocked(&d.channels, chid)
        };
        assert!([4, 5, 6, 9].iter().all(|c| blocked(&dispatcher, *c)));
        dispatcher.deferred_context.contexts.insert(
            target,
            Context {
                size: 0x2000,
                promoted: false,
            },
        );
        assert!([4, 5, 6].iter().all(|c| blocked(&dispatcher, *c)));
        dispatcher
            .deferred_context
            .contexts
            .get_mut(&target)
            .unwrap()
            .promoted = true;
        assert!([4, 5, 6].iter().all(|c| !blocked(&dispatcher, *c)));
        assert!(blocked(&dispatcher, 9)); // Another client's TSG shares nothing.
        assert!(blocked(&dispatcher, 8));
        // The promoted channel freed (Dispatcher::free prunes): the context moves to a live
        // member, and the TSG's channels keep submitting.
        dispatcher.channels.record_free(1, 0x41);
        dispatcher.deferred_context.prune(&dispatcher.channels);
        assert!([5, 6].iter().all(|c| !blocked(&dispatcher, *c)));
        assert!(blocked(&dispatcher, 9));
    }

    /// Run P29b: Windows CUDA promotes on its first channel, frees it first and evicts the
    /// TSG's context through the last channel left. The context moves with the TSG.
    #[test]
    fn the_tsg_context_moves_to_a_live_member_and_is_evicted_there() {
        let mut dispatcher = dispatcher();
        let (_, first, tsg) = add_pair(&mut dispatcher, 1, 3, 4);
        let promoted = TargetStamp::resolve(&dispatcher.channels, evict(1, 1, 4, tsg)).unwrap();
        let classes = crate::fixture::rtx2070().chip.data().channel;
        for (chid, object) in [(5, 0x42), (6, 0x43)] {
            dispatcher.channels.record_alloc(
                1,
                tsg,
                object,
                classes.gpfifo,
                &channel_params(chid, 1 << 31, 0),
            );
        }
        dispatcher.deferred_context.contexts.insert(
            promoted,
            Context {
                size: 0x2000,
                promoted: true,
            },
        );
        let blocked = |d: &Dispatcher, chid| {
            d.deferred_context
                .target_submission_blocked(&d.channels, chid)
        };
        // The promoted channel is freed (Dispatcher::free prunes): the context moves to the
        // lowest live member, and the members still submit.
        dispatcher.channels.record_free(1, first);
        dispatcher.deferred_context.prune(&dispatcher.channels);
        let homes: Vec<u32> = dispatcher
            .deferred_context
            .contexts
            .keys()
            .map(|t| t.chid)
            .collect();
        assert_eq!(homes, vec![5]);
        assert!(!blocked(&dispatcher, 5) && !blocked(&dispatcher, 6));
        // Channel 5 goes too: the eviction the guest names on channel 6 finds the context.
        dispatcher.channels.record_free(1, 0x42);
        dispatcher.deferred_context.prune(&dispatcher.channels);
        let last = TargetStamp::resolve(&dispatcher.channels, evict(2, 1, 6, tsg)).unwrap();
        assert!(dispatcher
            .deferred_context
            .contexts
            .get(&last)
            .is_some_and(|c| c.promoted && c.size == 0x2000));
        // The TSG's last channel gone: no heir, the context is dropped.
        dispatcher.channels.record_free(1, 0x43);
        dispatcher.deferred_context.prune(&dispatcher.channels);
        assert!(dispatcher.deferred_context.contexts.is_empty());
    }

    fn migration(client: u32, channel: u32) -> deferred::ChannelInfo {
        deferred::ChannelInfo {
            h_client: client,
            h_channel: channel,
            h_userd_memory: 0x99,
            gp_fifo_entries: 64,
            gp_fifo_offset: 0x1000_0000,
            userd_offset: 0,
        }
    }

    /// KNOWN BUG #1 (Windows step 23): a V2 registration forwarded as
    /// DEFERRED_API_INTERNAL with an UpdateChannelInfo bundle is registered as GSP-RM
    /// registers it, for a software channel as a ring switch at its method, and the owner
    /// is never marked for a management takeover; REMOVE_API drops it.
    #[test]
    fn a_deferred_channel_migration_registers_without_a_takeover() {
        let mut d = dispatcher();
        let (object, _, _) = add_pair(&mut d, 1, 4, 5);
        d.migration_userd = Some(Ok((Target::Sys(0x1000), 0)));
        let params = deferred::encode_channel_info_request(0x4000_007e, 0, &migration(1, 0x40));
        let a = d.control(1, object, deferred::CMD_DEFERRED_API_INTERNAL, &params);
        assert_eq!(a.status, NV_OK, "{}", a.note);
        assert!(a.note.contains("registered on chid 4"), "{}", a.note);
        assert!(
            !d.deferred_context.management.contains_key(&4),
            "no takeover for a migration"
        );
        let a = d.control(
            1,
            object,
            deferred::CMD_REMOVE_API,
            &0x4000_007eu32.to_le_bytes(),
        );
        assert_eq!(a.status, NV_OK, "{}", a.note);
        assert!(a.note.contains("ring migration"), "{}", a.note);
    }

    /// A migration whose new USERD the kernel never registered is refused, and the
    /// refusal leaves no management marking behind: nothing waits for a takeover.
    #[test]
    fn a_refused_channel_migration_leaves_no_takeover_behind() {
        let mut d = dispatcher();
        let (object, _, _) = add_pair(&mut d, 1, 4, 5);
        d.migration_userd = Some(Err("hUserdMemory 0x99 is not registered with GSP".into()));
        let params = deferred::encode_channel_info_request(0x4000_007e, 0, &migration(1, 0x40));
        let a = d.control(1, object, deferred::CMD_DEFERRED_API_V2, &params);
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
        assert!(a.note.contains("not registered"), "{}", a.note);
        assert!(!d.deferred_context.management.contains_key(&4));
        // An unknown channel is refused the same way.
        d.migration_userd = Some(Ok((Target::Sys(0x1000), 0)));
        let params = deferred::encode_channel_info_request(0x4000_007f, 0, &migration(1, 0x4f));
        let a = d.control(1, object, deferred::CMD_DEFERRED_API_V2, &params);
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
        assert!(!d.deferred_context.management.contains_key(&4));
    }

    /// A direct UpdateChannelInfo on an idle software channel moves it at once.
    #[test]
    fn a_direct_channel_migration_moves_a_software_channel_now() {
        let mut d = dispatcher();
        add_pair(&mut d, 1, 4, 5);
        d.migration_userd = Some(Ok((Target::Sys(0x2000), 3)));
        let a = d.control(
            1,
            0x10,
            deferred::CMD_UPDATE_CHANNEL_INFO,
            &migration(1, 0x40).encode(),
        );
        assert_eq!(a.status, NV_OK, "{}", a.note);
        let ch = d.channels.channel(4).unwrap();
        assert_eq!(
            (ch.userd, ch.gpfifo_va, ch.gpfifo_entries),
            (Some(Target::Sys(0x2000)), 0x1000_0000, 64)
        );
        assert_eq!(d.channels.channel_gp_get(4), Some(3));
        // Without the resolved USERD it is refused, as RM refuses a bad state.
        let a = d.control(
            1,
            0x10,
            deferred::CMD_UPDATE_CHANNEL_INFO,
            &migration(1, 0x40).encode(),
        );
        assert_eq!(a.status, 0x40);
    }

    #[test]
    fn registration_is_exact_unique_and_client_scoped() {
        let mut dispatcher = dispatcher();
        let (object1, _, tsg1) = add_pair(&mut dispatcher, 1, 3, 4);
        let (object2, _, tsg2) = add_pair(&mut dispatcher, 2, 5, 6);
        let p1 = evict(0x101, 1, 4, tsg1).encode();
        let p2 = evict(0x101, 2, 6, tsg2).encode();
        assert_eq!(
            dispatcher
                .register_deferred(1, object1, CMD_DEFERRED_API, &p1)
                .status,
            NV_OK
        );
        assert_eq!(
            dispatcher
                .register_deferred(2, object2, CMD_DEFERRED_API, &p2)
                .status,
            NV_OK
        );
        assert_eq!(dispatcher.deferred_context.requests.len(), 2);
        assert_eq!(
            dispatcher
                .register_deferred(1, object1, CMD_DEFERRED_API, &p1)
                .status,
            NV_ERR_NOT_SUPPORTED
        );
        assert!(dispatcher
            .deferred_context
            .is_management(&dispatcher.channels, 3));
        assert!(dispatcher
            .deferred_context
            .is_management(&dispatcher.channels, 5));
    }

    #[test]
    fn target_generation_reuse_and_free_prune_registration_and_readiness() {
        let mut dispatcher = dispatcher();
        let (object, target_object, tsg) = add_pair(&mut dispatcher, 1, 3, 4);
        let request = evict(0x101, 1, 4, tsg);
        let params = request.encode();
        assert_eq!(
            dispatcher
                .register_deferred(1, object, CMD_DEFERRED_API, &params)
                .status,
            NV_OK
        );
        let registered = *dispatcher
            .deferred_context
            .requests
            .get(&(1, object, 0x101))
            .unwrap();
        dispatcher.deferred_context.contexts.insert(
            registered.target,
            Context {
                size: 0x2000,
                promoted: true,
            },
        );
        assert!(registered.target.live(&dispatcher.channels));

        dispatcher.channels.record_free(1, target_object);
        dispatcher.deferred_context.prune(&dispatcher.channels); // as Dispatcher::free
        let classes = crate::fixture::rtx2070().chip.data().channel;
        dispatcher.channels.record_alloc(
            1,
            tsg,
            target_object,
            classes.gpfifo,
            &channel_params(4, 1 << 31, 0),
        );
        assert!(!registered.target.live(&dispatcher.channels));
        dispatcher.deferred_context.prune(&dispatcher.channels);
        assert!(dispatcher.deferred_context.requests.is_empty());
        assert!(dispatcher.deferred_context.contexts.is_empty());

        dispatcher.channels.record_free(1, 0x40);
        dispatcher.deferred_context.prune(&dispatcher.channels);
        assert!(!dispatcher
            .deferred_context
            .is_management(&dispatcher.channels, 3));
    }

    #[test]
    fn initialization_rejects_out_of_bounds_and_unsupported_flags() {
        let mut dispatcher = dispatcher();
        let (object, _, tsg) = add_pair(&mut dispatcher, 1, 3, 4);
        let target = evict(1, 1, 4, tsg).target();
        let outside = Request::Initialize {
            api_handle: 0x201,
            target,
            h_virt_memory: 0,
            phys_address: (4 << 30) - 0x1000,
            phys_attr: 0,
            h_dma_handle: 0,
            index: 0,
            size: 0x2000,
        }
        .encode();
        assert_eq!(
            dispatcher
                .register_deferred(1, object, CMD_DEFERRED_API, &outside)
                .status,
            NV_ERR_NOT_SUPPORTED
        );

        let mut unsupported = outside;
        put32(&mut unsupported, 24 + 32, 2);
        assert_eq!(
            dispatcher
                .register_deferred(1, object, CMD_DEFERRED_API, &unsupported)
                .status,
            NV_ERR_NOT_SUPPORTED
        );
        assert!(dispatcher.deferred_context.requests.is_empty());
    }
}
