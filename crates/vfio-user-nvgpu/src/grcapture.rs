// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Opt-in, live C597 method capture for a mirrored graphics channel.
//!
//! An RC post-mortem cannot reconstruct old graphics state from the GPFIFO alone: Windows
//! reuses pushbuffer storage long before a later Xid.  With [`ENV`] enabled, the mirror reads
//! each newly published entry before forwarding its doorbell and retains only the C597 state
//! needed to explain a fault: exact `SET_OBJECT` bindings, the MME instruction/start RAM
//! uploads, direct constant-buffer state, and recent macro calls with their parameters.
//! Nothing is changed in guest memory and the production path pays only one disabled check.

use std::collections::{BTreeSet, VecDeque};

use crate::channel::GpuMem;
use crate::fb::Target;

pub const ENV: &str = "CARAXES_GR_CAPTURE";

const CLASS_3D: u32 = 0xC597;
const MAX_DWORDS: usize = 1 << 16;
const MAX_MME_WORDS: usize = 0x4000;
const MAX_MME_STARTS: usize = 256;
const KEEP_CALLS: usize = 16_384;
const KEEP_DIRECT: usize = 32;
const KEEP_VALID_ZERO: usize = 8;
const KEEP_PARAMS: usize = 64;
const KEEP_DMEM_SOURCES: usize = 64;

#[derive(Clone, Debug)]
struct MacroCall {
    seq: u64,
    gp: u32,
    subch: usize,
    id: u32,
    params: Vec<u32>,
    param_count: usize,
}

#[derive(Clone, Debug)]
struct OpenCall(MacroCall);

#[derive(Clone, Copy, Debug)]
struct CbBinding {
    va: u64,
    size: u32,
    gp: u32,
}

#[derive(Clone, Copy, Debug)]
struct DmemSource {
    seq: u64,
    index: u32,
    gp: u32,
    va: u64,
}

/// The live, bounded state of one mirrored C597 channel.
pub struct Capture {
    allowed_classes: BTreeSet<u32>,
    classes: [u32; 8],
    class_gp: [Option<u32>; 8],
    class_exact: [bool; 8],
    mme_pointer: usize,
    mme_words: Vec<Option<u32>>,
    start_pointer: usize,
    starts: [Option<u32>; MAX_MME_STARTS],
    mme_mem_address: u64,
    mme_data_ram_address: u32,
    dmem_sources: VecDeque<DmemSource>,
    mme_dma_reads: u64,
    cb_selector: [(u32, u32, u32); 8],
    cb: [[[Option<CbBinding>; 32]; 8]; 8],
    calls: VecDeque<MacroCall>,
    direct: VecDeque<String>,
    valid_zero: VecDeque<String>,
    entries: u64,
    pushbuffers: u64,
    methods: u64,
    unreadable_entries: u64,
    unreadable_pushbuffers: u64,
    invalid_entries: u64,
    mme_writes: u64,
    start_writes: u64,
    legacy_headers: u64,
    tertiary_headers: u64,
    reserved_headers: u64,
    unexpected_set_objects: u64,
    direct_binds: u64,
    direct_unbinds: u64,
}

impl Default for Capture {
    fn default() -> Self {
        Capture {
            allowed_classes: BTreeSet::new(),
            classes: [0; 8],
            class_gp: [None; 8],
            class_exact: [false; 8],
            mme_pointer: 0,
            mme_words: vec![None; MAX_MME_WORDS],
            start_pointer: 0,
            starts: [None; MAX_MME_STARTS],
            mme_mem_address: 0,
            mme_data_ram_address: 0,
            dmem_sources: VecDeque::with_capacity(KEEP_DMEM_SOURCES),
            mme_dma_reads: 0,
            cb_selector: [(0, 0, 0); 8],
            cb: [[[None; 32]; 8]; 8],
            calls: VecDeque::with_capacity(KEEP_CALLS),
            direct: VecDeque::with_capacity(KEEP_DIRECT),
            valid_zero: VecDeque::with_capacity(KEEP_VALID_ZERO),
            entries: 0,
            pushbuffers: 0,
            methods: 0,
            unreadable_entries: 0,
            unreadable_pushbuffers: 0,
            invalid_entries: 0,
            mme_writes: 0,
            start_writes: 0,
            legacy_headers: 0,
            tertiary_headers: 0,
            reserved_headers: 0,
            unexpected_set_objects: 0,
            direct_binds: 0,
            direct_unbinds: 0,
        }
    }
}

fn keep<T>(q: &mut VecDeque<T>, capacity: usize, value: T) {
    if q.len() == capacity {
        q.pop_front();
    }
    q.push_back(value);
}

impl Capture {
    /// Monotonic count of GPFIFO entries captured for fault-bound cutoff calculations.
    pub fn entries(&self) -> u64 {
        self.entries
    }

    /// Start a capture with the classes the host channel actually owns. A Windows graphics
    /// context commonly established C597 on subchannel 0 before mirror takeover; seed that
    /// binding as inferred until a captured SET_OBJECT supersedes it.
    pub fn new(objects: &[(u32, u32)]) -> Self {
        let mut capture = Self::default();
        capture.allowed_classes = objects.iter().map(|(class, _)| *class).collect();
        if capture.allowed_classes.contains(&CLASS_3D) {
            capture.classes[0] = CLASS_3D;
        }
        capture
    }

    fn finish_call(&mut self, call: &mut Option<OpenCall>) {
        if let Some(OpenCall(call)) = call.take() {
            keep(&mut self.calls, KEEP_CALLS, call);
        }
    }

    fn direct_event(&mut self, event: String) {
        keep(&mut self.direct, KEEP_DIRECT, event);
    }

    fn method(&mut self, gp: u32, subch: usize, addr: u32, data: u32, open: &mut Option<OpenCall>) {
        self.methods += 1;
        if addr == 0 {
            self.finish_call(open);
            let class = data & 0xFFFF;
            if class != 0 && !self.allowed_classes.contains(&class) {
                self.unexpected_set_objects += 1;
                self.direct_event(format!(
                    "gp {gp} subch {subch} ignored SET_OBJECT {class:#x}: class is not owned by the host channel"
                ));
                return;
            }
            self.classes[subch] = class;
            self.class_gp[subch] = Some(gp);
            self.class_exact[subch] = true;
            return;
        }
        // Windows uses subchannel 0 for the channel's C597 object. A context captured
        // after takeover can contain SET_OBJECT(0) diagnostics even though subsequent
        // methods execute against the restored 3D context; keep classifying that known
        // hardware subchannel as C597. Other subchannels still require an exact binding.
        let inferred_3d = subch == 0 && self.allowed_classes.contains(&CLASS_3D);
        if self.classes[subch] != CLASS_3D && !inferred_3d {
            self.finish_call(open);
            return;
        }

        let mme_call = (0x3800..=0x3FFC).contains(&addr) && (addr - 0x3800) % 8 == 0;
        let mme_data = (0x3804..=0x3FFC).contains(&addr) && (addr - 0x3804) % 8 == 0;
        if mme_call {
            self.finish_call(open);
            *open = Some(OpenCall(MacroCall {
                seq: self.entries,
                gp,
                subch,
                id: (addr - 0x3800) / 8,
                params: vec![data],
                param_count: 1,
            }));
            return;
        }
        if mme_data {
            let id = (addr - 0x3804) / 8;
            if let Some(OpenCall(call)) =
                open.as_mut().filter(|c| c.0.subch == subch && c.0.id == id)
            {
                call.param_count += 1;
                if call.params.len() < KEEP_PARAMS {
                    call.params.push(data);
                }
                return;
            }
        }
        self.finish_call(open);

        match addr {
            0x0114 => self.mme_pointer = data as usize,
            0x0118 => {
                if let Some(word) = self.mme_words.get_mut(self.mme_pointer) {
                    *word = Some(data);
                }
                self.mme_pointer = self.mme_pointer.saturating_add(1);
                self.mme_writes += 1;
            }
            0x011C => self.start_pointer = data as usize,
            0x0120 => {
                if let Some(start) = self.starts.get_mut(self.start_pointer) {
                    *start = Some(data);
                }
                self.start_pointer = self.start_pointer.saturating_add(1);
                self.start_writes += 1;
            }
            0x0550 => {
                self.mme_mem_address =
                    (self.mme_mem_address & 0xFFFF_FFFF) | ((data as u64 & 0xFF) << 32)
            }
            0x0554 => self.mme_mem_address = (self.mme_mem_address & !0xFFFF_FFFF) | data as u64,
            0x0558 => self.mme_data_ram_address = data,
            0x055C => {
                self.mme_dma_reads += 1;
                for index in [0x56u32, 0x57] {
                    if index >= self.mme_data_ram_address
                        && index - self.mme_data_ram_address < data
                    {
                        let source = DmemSource {
                            seq: self.entries,
                            index,
                            gp,
                            va: self.mme_mem_address
                                + 4 * (index - self.mme_data_ram_address) as u64,
                        };
                        keep(&mut self.dmem_sources, KEEP_DMEM_SOURCES, source);
                    }
                }
            }
            0x2380 => self.cb_selector[subch].0 = data & 0x1_FFFF,
            0x2384 => self.cb_selector[subch].1 = data & 0xFF,
            0x2388 => self.cb_selector[subch].2 = data,
            0x10F4 if data & (1 << 8) != 0 => {
                self.cb[subch] = [[None; 32]; 8];
                self.direct_event(format!("gp {gp} subch {subch} UNBIND_ALL constant buffers"));
            }
            a if (0x2410..=0x24F0).contains(&a) && (a - 0x2410) % 0x20 == 0 => {
                let group = ((a - 0x2410) / 0x20) as usize;
                let slot = ((data >> 4) & 0x1F) as usize;
                if data & 1 != 0 {
                    let (size, upper, lower) = self.cb_selector[subch];
                    let va = ((upper as u64) << 32) | lower as u64;
                    self.cb[subch][group][slot] = Some(CbBinding { va, size, gp });
                    self.direct_binds += 1;
                    let event = format!(
                        "gp {gp} subch {subch} CB group {group} slot {slot} BIND va {va:#x} size {size:#x}"
                    );
                    self.direct_event(event.clone());
                    if va == 0 {
                        keep(
                            &mut self.valid_zero,
                            KEEP_VALID_ZERO,
                            format!("{event} **VALID VA 0**"),
                        );
                    }
                } else {
                    self.cb[subch][group][slot] = None;
                    self.direct_unbinds += 1;
                    self.direct_event(format!(
                        "gp {gp} subch {subch} CB group {group} slot {slot} UNBIND"
                    ));
                }
            }
            _ => {}
        }
    }

    fn push(&mut self, gp: u32, words: &[u32]) {
        let mut open = None;
        let mut i = 0usize;
        while i < words.len() {
            let hdr = words[i];
            i += 1;
            let sec_op = hdr >> 29;
            match sec_op {
                1 | 3 | 5 => {
                    let count = ((hdr >> 16) & 0x1FFF) as usize;
                    let subch = ((hdr >> 13) & 7) as usize;
                    let addr = (hdr & 0xFFF) << 2;
                    let count = count.min(words.len() - i);
                    for k in 0..count {
                        let addr = match sec_op {
                            1 => addr + 4 * k as u32,
                            3 => addr,
                            _ => addr + if k == 0 { 0 } else { 4 },
                        };
                        self.method(gp, subch, addr, words[i + k], &mut open);
                    }
                    i += count;
                }
                4 => self.method(
                    gp,
                    ((hdr >> 13) & 7) as usize,
                    (hdr & 0xFFF) << 2,
                    (hdr >> 16) & 0x1FFF,
                    &mut open,
                ),
                // The legacy formats remain valid on C46F. Their accepted count is at 27:18 and
                // their byte method address is the header's 12:2 field. Treating these
                // payload dwords as fresh headers desynchronizes the entire push (C125).
                0 | 2 if sec_op == 2 || (hdr >> 16) & 3 == 0 => {
                    self.legacy_headers += 1;
                    // Match Mesa's C46F parser: the legacy count accepted by the
                    // hardware path is the ten-bit field at 27:18. Bit 28 is not part
                    // of the payload count for these GRP forms.
                    let count = ((hdr >> 18) & 0x3FF) as usize;
                    let subch = ((hdr >> 13) & 7) as usize;
                    let addr = hdr & 0x1FFC;
                    let count = count.min(words.len() - i);
                    for k in 0..count {
                        let addr = if sec_op == 0 {
                            addr + 4 * k as u32
                        } else {
                            addr
                        };
                        self.method(gp, subch, addr, words[i + k], &mut open);
                    }
                    i += count;
                }
                // GRP0 SET/STORE_SUBDEVICE_MASK carry their data in the header. USE has
                // one following word in the reference decoder; consume it without
                // interpreting it as a fresh push header.
                0 => {
                    self.tertiary_headers += 1;
                    self.finish_call(&mut open);
                    if (hdr >> 16) & 3 == 3 && i < words.len() {
                        i += 1;
                    }
                }
                7 => break,
                _ => {
                    self.reserved_headers += 1;
                    self.finish_call(&mut open);
                    break;
                }
            }
        }
        self.finish_call(&mut open);
    }

    /// Recent page-aligned, non-zero macro parameters that may be GPU virtual addresses.
    /// This deliberately returns only a few candidates: the RC path can snapshot them from
    /// the guest VA space, while ordinary scalar macro parameters are excluded by alignment.
    pub fn recent_page_aligned_params(&self) -> Vec<u64> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for call in self.calls.iter().rev() {
            for &param in call.params.iter().rev() {
                let va = param as u64;
                if va != 0 && va & 0xfff == 0 && seen.insert(va) {
                    out.push(va);
                    if out.len() == 4 {
                        return out;
                    }
                }
            }
        }
        out
    }

    /// Guest VAs last loaded into the two MME data-RAM words from which the Windows macro
    /// image emits SET_CONSTANT_BUFFER_SELECTOR_B/C.
    pub fn constant_buffer_dmem_sources(&self, cutoff: u64) -> Vec<(u32, u32, u64)> {
        [0x56u32, 0x57]
            .into_iter()
            .filter_map(|index| {
                self.dmem_sources
                    .iter()
                    .rev()
                    .find(|source| source.index == index && source.seq <= cutoff)
                    .map(|source| (index, source.gp, source.va))
            })
            .collect()
    }

    /// Exact retained macro calls in the GPFIFO entry the PBDMA fetched last. Calls after
    /// GPGet may already be submitted in large batches and are not fault-proximate.
    pub fn call_notes_at(&self, chid: u32, gp: u32, cutoff: u64) -> Vec<String> {
        let Some(seq) = self
            .calls
            .iter()
            .rev()
            .find(|call| call.gp == gp && call.seq <= cutoff)
            .map(|call| call.seq)
        else {
            return Vec::new();
        };
        self.calls
            .iter()
            .filter(|call| call.gp == gp && call.seq == seq)
            .map(|call| format!(
                "gr live capture: chid {chid}: fault-bound gp {gp} capture-seq {seq} subch {} macro {} params {}/{}: {}{}",
                call.subch,
                call.id,
                call.params.len(),
                call.param_count,
                call.params.iter().map(|word| format!("{word:#x}")).collect::<Vec<_>>().join(" "),
                if call.params.len() == call.param_count { "" } else { " ..." }
            ))
            .collect()
    }

    /// The last macro calls actually submitted at or before the host's fault cutoff. This
    /// covers RCs where GPGet-1 is a trailing semaphore rather than the draw that faulted.
    pub fn consumed_call_notes(&self, chid: u32, cutoff: u64) -> Vec<String> {
        let calls = self
            .calls
            .iter()
            .filter(|call| call.seq <= cutoff)
            .rev()
            .take(32)
            .collect::<Vec<_>>();
        calls
            .into_iter()
            .rev()
            .map(|call| format!(
                "gr live capture: chid {chid}: consumed gp {} capture-seq {} subch {} macro {} params {}/{}: {}{}",
                call.gp,
                call.seq,
                call.subch,
                call.id,
                call.params.len(),
                call.param_count,
                call.params.iter().map(|word| format!("{word:#x}")).collect::<Vec<_>>().join(" "),
                if call.params.len() == call.param_count { "" } else { " ..." }
            ))
            .collect()
    }

    /// Capture the newly published half-open GPFIFO range `[start, put)` before its host
    /// doorbell is forwarded. `start` and `put` are ring indices and may wrap.
    pub fn ring(
        &mut self,
        gm: &GpuMem,
        root: Target,
        gpfifo_va: u64,
        entries: u32,
        start: u32,
        put: u32,
    ) -> Option<String> {
        if entries == 0 || start >= entries || put >= entries {
            return Some(format!(
                "graphics capture refused malformed ring: start {start}, put {put}, entries {entries}"
            ));
        }
        let count = (put + entries - start) % entries;
        for step in 0..count {
            let gp = (start + step) % entries;
            self.entries += 1;
            let mut e = [0u8; 8];
            if gm.read_va(root, gpfifo_va + 8 * gp as u64, &mut e).is_err() {
                self.unreadable_entries += 1;
                continue;
            }
            let e0 = u32::from_le_bytes(e[0..4].try_into().unwrap());
            let e1 = u32::from_le_bytes(e[4..8].try_into().unwrap());
            let len = ((e1 >> 10) & 0x1F_FFFF) as usize;
            let va = ((e1 as u64 & 0xFF) << 32) | (e0 as u64 & !3);
            if len == 0 || len > MAX_DWORDS {
                self.invalid_entries += 1;
                continue;
            }
            let mut raw = vec![0u8; len * 4];
            if gm.read_va(root, va, &mut raw).is_err() {
                self.unreadable_pushbuffers += 1;
                continue;
            }
            let words: Vec<u32> = raw
                .chunks_exact(4)
                .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
                .collect();
            self.pushbuffers += 1;
            self.push(gp, &words);
        }
        None
    }

    /// Bounded RC output. MME dumps are raw dwords suitable for Mesa's `nv_mme_dump`
    /// (`TURING`); only macro ids in the most recent calls are included.
    pub fn notes(&self, chid: u32) -> Vec<String> {
        let mut out = Vec::new();
        let classes: Vec<String> = self
            .classes
            .iter()
            .zip(self.class_gp.iter().zip(self.class_exact))
            .enumerate()
            .filter(|(_, (class, _))| **class != 0)
            .map(|(subch, (class, (gp, exact)))| {
                format!(
                    "{subch}={}{}{class:#x}@gp{}",
                    if exact { "" } else { "~" },
                    if exact { "" } else { "inferred:" },
                    gp.map_or_else(|| "?".into(), |gp| gp.to_string())
                )
            })
            .collect();
        let mme_words = self.mme_words.iter().filter(|word| word.is_some()).count();
        let starts = self.starts.iter().filter(|start| start.is_some()).count();
        out.push(format!(
            "gr live capture: chid {chid}: {} entries, {} pushbuffers, {} methods; unreadable entries {}, pushbuffers {}, invalid {}; legacy headers {}, tertiary {}, reserved {}; SET_OBJECT state {} ({} unexpected ignored); MME words {mme_words} ({} writes), starts {starts} ({} writes), DMA reads {}; direct CB binds {}, unbinds {} [guest memory, opt-in]",
            self.entries,
            self.pushbuffers,
            self.methods,
            self.unreadable_entries,
            self.unreadable_pushbuffers,
            self.invalid_entries,
            self.legacy_headers,
            self.tertiary_headers,
            self.reserved_headers,
            if classes.is_empty() { "none".into() } else { classes.join(" ") },
            self.unexpected_set_objects,
            self.mme_writes,
            self.start_writes,
            self.mme_dma_reads,
            self.direct_binds,
            self.direct_unbinds,
        ));
        // CARAXES_GR_CAPTURE is already an explicit, fault-analysis-only mode. Preserve the
        // complete final MME image here: Windows may install several generations of macros,
        // so bounding a called macro by the next start address can splice a newer start table
        // onto older instruction RAM and hide the code that actually emitted faulting state.
        let start_image = self
            .starts
            .iter()
            .enumerate()
            .filter_map(|(id, start)| start.map(|start| format!("{id}={start:#x}")))
            .collect::<Vec<_>>();
        out.push(format!(
            "gr live capture: chid {chid}: complete MME start image: {}",
            if start_image.is_empty() {
                "none".into()
            } else {
                start_image.join(" ")
            }
        ));
        if let Some(last) = self.mme_words.iter().rposition(Option::is_some) {
            for (chunk, words) in self.mme_words[..=last].chunks(16).enumerate() {
                out.push(format!(
                    "gr live capture: chid {chid}: complete MME code +{:#x}: {}",
                    chunk * 16,
                    words
                        .iter()
                        .map(|word| word
                            .map_or_else(|| "--------".into(), |word| format!("{word:08x}")))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
        }
        if self.valid_zero.is_empty() {
            out.push(format!(
                "gr live capture: chid {chid}: no direct valid constant-buffer bind to VA 0 observed"
            ));
        } else {
            out.extend(
                self.valid_zero
                    .iter()
                    .map(|event| format!("gr live capture: chid {chid}: CRITICAL {event}")),
            );
        }

        let mut active = Vec::new();
        for (subch, groups) in self.cb.iter().enumerate() {
            for (group, slots) in groups.iter().enumerate() {
                for (slot, binding) in slots.iter().enumerate() {
                    if let Some(binding) = binding {
                        active.push(format!(
                            "{subch}:{group}/{slot}={:#x}+{:#x}@gp{}",
                            binding.va, binding.size, binding.gp
                        ));
                    }
                }
            }
        }
        out.push(format!(
            "gr live capture: chid {chid}: final direct CB state {}",
            if active.is_empty() {
                "none".into()
            } else {
                active.join(" ")
            }
        ));
        out.extend(
            self.direct
                .iter()
                .map(|event| format!("gr live capture: chid {chid}: recent direct state: {event}")),
        );

        for call in self.calls.iter().rev().take(32).rev() {
            let params = call
                .params
                .iter()
                .map(|word| format!("{word:#x}"))
                .collect::<Vec<_>>()
                .join(" ");
            out.push(format!(
                "gr live capture: chid {chid}: gp {} subch {} macro {} params {}/{}: {}{}",
                call.gp,
                call.subch,
                call.id,
                call.params.len(),
                call.param_count,
                params,
                if call.params.len() == call.param_count {
                    ""
                } else {
                    " ..."
                }
            ));
        }

        let mut ids = BTreeSet::new();
        for call in self.calls.iter().rev() {
            ids.insert(call.id as usize);
            if ids.len() == 4 {
                break;
            }
        }
        for id in ids {
            let Some(start) = self
                .starts
                .get(id)
                .and_then(|start| *start)
                .map(|x| x as usize)
            else {
                out.push(format!(
                    "gr live capture: chid {chid}: macro {id} has no captured start address"
                ));
                continue;
            };
            let limit = start.saturating_add(384).min(self.mme_words.len());
            let end = self
                .starts
                .iter()
                .flatten()
                .map(|x| *x as usize)
                .filter(|next| *next > start)
                .min()
                .unwrap_or(self.mme_words.len())
                .min(limit);
            let words: Vec<u32> = self.mme_words[start..end]
                .iter()
                .take_while(|word| word.is_some())
                .flatten()
                .copied()
                .collect();
            out.push(format!(
                "gr live capture: chid {chid}: macro {id} start dword {start}, captured {} dword(s){}",
                words.len(),
                if end == limit { " (bounded)" } else { "" }
            ));
            for (chunk, words) in words.chunks(16).enumerate() {
                out.push(format!(
                    "gr live capture: chid {chid}: macro {id} code +{:#x}: {}",
                    chunk * 16,
                    words
                        .iter()
                        .map(|word| format!("{word:08x}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inc(subch: u32, addr: u32, data: &[u32]) -> Vec<u32> {
        let mut out = vec![(1 << 29) | ((data.len() as u32) << 16) | (subch << 13) | (addr >> 2)];
        out.extend_from_slice(data);
        out
    }

    fn oneinc(subch: u32, addr: u32, data: &[u32]) -> Vec<u32> {
        let mut out = vec![(5 << 29) | ((data.len() as u32) << 16) | (subch << 13) | (addr >> 2)];
        out.extend_from_slice(data);
        out
    }

    fn old_inc(subch: u32, addr: u32, data: &[u32]) -> Vec<u32> {
        let mut out = vec![((data.len() as u32) << 18) | (subch << 13) | addr];
        out.extend_from_slice(data);
        out
    }

    fn old_noninc(subch: u32, addr: u32, data: &[u32]) -> Vec<u32> {
        let mut out = vec![(2 << 29) | ((data.len() as u32) << 18) | (subch << 13) | addr];
        out.extend_from_slice(data);
        out
    }

    #[test]
    fn live_capture_keeps_exact_mme_upload_calls_and_cb_state() {
        let mut words = old_inc(0, 0, &[CLASS_3D]);
        words.extend(old_inc(0, 0x11C, &[7, 12]));
        words.extend(inc(0, 0x114, &[12]));
        words.extend(old_noninc(0, 0x118, &[0xAA, 0xBB, 0xCC]));
        words.extend(inc(0, 0x550, &[0, 0x1234_5000, 0x50, 16]));
        words.extend(inc(0, 0x2380, &[0x100, 0, 0]));
        words.extend(inc(0, 0x2410, &[1]));
        words.extend(oneinc(0, 0x3800 + 7 * 8, &[0x11, 0x22, 0x33]));
        // The same numerical methods on a software subchannel are never C597 state.
        words.extend(inc(7, 0x3800, &[0xDEAD_BEEF]));
        // A value that is not one of the host channel's engine classes cannot be a
        // successful SET_OBJECT and must not destroy the known C597 binding.
        words.extend(inc(0, 0, &[0x5540]));
        words.extend(inc(0, 0, &[0]));
        words.extend(oneinc(0, 0x3800 + 8 * 8, &[0x44, 0x55]));
        words.extend(oneinc(0, 0x3800 + 48 * 8, &[0, 0, 0x0412_A000]));
        words.extend(inc(0, 0x10F4, &[1 << 8]));

        let mut c = Capture::new(&[(CLASS_3D, 1)]);
        c.push(9, &words);
        let notes = c.notes(3).join("\n");
        assert!(notes.contains("SET_OBJECT state none"), "{notes}");
        assert!(notes.contains("1 unexpected ignored"), "{notes}");
        assert!(notes.contains("macro 8 params 2/2: 0x44 0x55"), "{notes}");
        assert!(notes.contains("legacy headers 3"), "{notes}");
        assert!(
            notes.contains("MME words 3") && notes.contains("starts 1"),
            "{notes}"
        );
        assert!(
            notes.contains("macro 7 params 3/3: 0x11 0x22 0x33"),
            "{notes}"
        );
        assert!(!notes.contains("deadbeef"), "{notes}");
        assert!(notes.contains("**VALID VA 0**"), "{notes}");
        assert!(notes.contains("final direct CB state none"), "{notes}");
        assert!(
            notes.contains("macro 7 code +0x0: 000000aa 000000bb 000000cc"),
            "{notes}"
        );
        assert!(notes.contains("complete MME start image: 7=0xc"), "{notes}");
        assert!(
            notes.contains("complete MME code +0x0: -------- -------- -------- -------- -------- -------- -------- -------- -------- -------- -------- -------- 000000aa 000000bb 000000cc"),
            "{notes}"
        );
        assert_eq!(c.recent_page_aligned_params(), vec![0x0412_A000]);
        assert_eq!(
            c.constant_buffer_dmem_sources(u64::MAX),
            vec![(0x56, 9, 0x1234_5018), (0x57, 9, 0x1234_501c)]
        );
        assert!(
            c.call_notes_at(3, 9, u64::MAX)
                .iter()
                .any(|line| line.contains("fault-bound gp 9 capture-seq 0 subch 0 macro 48")),
            "{notes}"
        );

        // A later submitted reuse of the same ring index must not replace state selected by
        // the consumed-entry cutoff of an RC.
        c.entries = 1;
        let mut later = inc(0, 0x550, &[0, 0x5678_9000, 0x50, 16]);
        later.extend(oneinc(0, 0x3800 + 49 * 8, &[0xDEAD]));
        c.push(9, &later);
        assert_eq!(
            c.constant_buffer_dmem_sources(0),
            vec![(0x56, 9, 0x1234_5018), (0x57, 9, 0x1234_501c)]
        );
        assert!(c
            .call_notes_at(3, 9, 0)
            .iter()
            .any(|line| line.contains("macro 48")));
        assert_eq!(
            c.call_notes_at(3, 9, 1)
                .iter()
                .filter(|line| line.contains("macro 49"))
                .count(),
            1
        );
        assert!(c
            .consumed_call_notes(3, 0)
            .iter()
            .all(|line| line.contains("capture-seq 0")));
    }
}
