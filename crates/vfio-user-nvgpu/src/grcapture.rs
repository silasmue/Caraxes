// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Opt-in, live C597 method capture for a mirrored graphics channel.
//!
//! An RC post-mortem cannot reconstruct old graphics state from the GPFIFO alone: Windows
//! reuses pushbuffer storage long before a later Xid.  With [`ENV`] enabled, the mirror reads
//! each newly published entry before forwarding its doorbell and retains only the C597 state
//! needed to explain a fault: exact `SET_OBJECT` bindings, the MME instruction/start RAM
//! uploads, direct constant-buffer state, and recent macro calls with their parameters.
//! A separate bounded history retains direct depth/ZCULL methods, without interpreting
//! macro-generated methods or claiming the GPU's effective state.
//! These are submitted CPU snapshots; earlier GPU producers and conditional fetching can
//! make the bytes eventually executed differ from the captured bytes.
//! Nothing is changed in guest memory and the production path pays only one disabled check.

use std::collections::{BTreeSet, VecDeque};

use crate::channel::GpuMem;
use crate::fb::Target;

pub const ENV: &str = "CARAXES_GR_CAPTURE";

const CLASS_3D: u32 = 0xC597;
// This capture decodes C597/Turing MME64 only. Its upload pointer and macro start
// addresses name 96-bit instruction groups; retained code is indexed by 32-bit dwords.
const MME64_DWORDS_PER_GROUP: usize = 3;
const MAX_DWORDS: usize = 1 << 16;
const MAX_MME_WORDS: usize = 0x4000;
const MAX_MME_STARTS: usize = 256;
const KEEP_CALLS: usize = 16_384;
const KEEP_DIRECT: usize = 32;
const KEEP_VALID_ZERO: usize = 8;
const KEEP_PARAMS: usize = 64;
const KEEP_DMEM_SOURCES: usize = 64;
const KEEP_BOOTSTRAP_CALLS: usize = 64;
const KEEP_MME_UPLOADS: usize = 128;
const KEEP_ROOT_EVENTS: usize = 256;
const KEEP_UNEXPECTED_SET_OBJECTS: usize = 8;
const KEEP_DEPTH_EVENTS: usize = 96;

// Byte method addresses verified against NVIDIA 615.71.09 class/clc597.h. Retain
// only these scalar observations; neither register fields nor GPU state are inferred.
fn depth_method_name(method: u32) -> Option<&'static str> {
    Some(match method {
        0x02bc => "SET_ZCULL_VISIBLE_PRIM_OPTIMIZATION",
        0x02e4 => "SET_ZCULL_ROP_BYPASS",
        0x02e8 => "SET_ZCULL_SUBREGION",
        0x02f8 => "SET_ZCULL_SUBREGION_ALLOCATION",
        0x02fc => "ASSIGN_ZCULL_SUBREGIONS",
        0x07c0 => "SET_ZCULL_REGION_SIZE_A",
        0x07c4 => "SET_ZCULL_REGION_SIZE_B",
        0x07c8 => "SET_ZCULL_REGION_SIZE_C",
        0x07cc => "SET_ZCULL_REGION_PIXEL_OFFSET_C",
        0x07e0 => "SET_ZCULL_REGION_LOCATION",
        0x07e4 => "SET_ZCULL_REGION_ALIQUOTS",
        0x07e8 => "SET_ZCULL_STORAGE_A",
        0x07ec => "SET_ZCULL_STORAGE_B",
        0x07f0 => "SET_ZCULL_STORAGE_C",
        0x07f4 => "SET_ZCULL_STORAGE_D",
        0x0da8 => "SET_ZCULL_SERIALIZATION",
        0x0fe0 => "SET_ZT_A",
        0x0fe4 => "SET_ZT_B",
        0x0fe8 => "SET_ZT_FORMAT",
        0x0fec => "SET_ZT_BLOCK_SIZE",
        0x1214 => "DRAW_VERTEX_ARRAY_BEGIN_END_INSTANCE_FIRST",
        0x12c8 => "CLEAR_ZCULL_REGION",
        0x12cc => "SET_DEPTH_TEST",
        0x12e8 => "SET_DEPTH_WRITE",
        0x130c => "SET_DEPTH_FUNC",
        0x1968 => "SET_ZCULL",
        0x19d0 => "CLEAR_SURFACE",
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug)]
struct SubmittedBinding {
    seq: u64,
    gp: u32,
    data: u32,
}

#[derive(Clone, Copy, Debug)]
struct DepthEvent {
    seq: u64,
    gp: u32,
    subch: usize,
    method: u32,
    data: u32,
    exact_3d: bool,
    binding: Option<SubmittedBinding>,
}

struct UnexpectedSetObject {
    gp: u32,
    subch: usize,
    header: u32,
    dword: usize,
    data: u32,
    nearby_start: usize,
    nearby: Vec<u32>,
}

#[derive(Clone, Copy, Debug)]
struct RootEvent {
    seq: u64,
    gp: u32,
    method: u32,
    data: u32,
}

#[derive(Clone, Debug)]
struct MmeUpload {
    seq: u64,
    last_seq: u64,
    gp: u32,
    pointer: u32,
    version: Option<u32>,
    words: Vec<u32>,
    word_count: usize,
}

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

#[derive(Clone, Debug)]
struct BootstrapCall {
    call: MacroCall,
    start: Option<u32>,
    code: Vec<Option<u32>>,
}

#[derive(Clone, Debug)]
struct DmaRead {
    seq: u64,
    gp: u32,
    va: u64,
    index: u32,
    words: u32,
    snapshot: Option<Result<Vec<u32>, String>>,
}

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
    submitted_bindings: [Option<SubmittedBinding>; 8],
    mme_pointer: usize,
    mme_version: Option<u32>,
    mme_words: Vec<Option<u32>>,
    mme_uploads: Vec<MmeUpload>,
    mme_upload: Option<MmeUpload>,
    start_pointer: usize,
    starts: [Option<u32>; MAX_MME_STARTS],
    mme_mem_address: u64,
    mme_data_ram_address: u32,
    dmem_sources: VecDeque<DmemSource>,
    dma_reads: VecDeque<DmaRead>,
    mme_dma_reads: u64,
    cb_selector: [(u32, u32, u32); 8],
    cb: [[[Option<CbBinding>; 32]; 8]; 8],
    calls: VecDeque<MacroCall>,
    bootstrap_calls: Vec<BootstrapCall>,
    direct: VecDeque<String>,
    root_events: VecDeque<RootEvent>,
    depth_events: VecDeque<DepthEvent>,
    depth_observations: u64,
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
    unexpected_set_object_provenance: Vec<UnexpectedSetObject>,
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
            submitted_bindings: [None; 8],
            mme_pointer: 0,
            mme_version: None,
            mme_words: vec![None; MAX_MME_WORDS],
            mme_uploads: Vec::new(),
            mme_upload: None,
            start_pointer: 0,
            starts: [None; MAX_MME_STARTS],
            mme_mem_address: 0,
            mme_data_ram_address: 0,
            dmem_sources: VecDeque::with_capacity(KEEP_DMEM_SOURCES),
            dma_reads: VecDeque::with_capacity(KEEP_DMEM_SOURCES),
            mme_dma_reads: 0,
            cb_selector: [(0, 0, 0); 8],
            cb: [[[None; 32]; 8]; 8],
            calls: VecDeque::with_capacity(KEEP_CALLS),
            bootstrap_calls: Vec::with_capacity(KEEP_BOOTSTRAP_CALLS),
            direct: VecDeque::with_capacity(KEEP_DIRECT),
            root_events: VecDeque::new(),
            depth_events: VecDeque::with_capacity(KEEP_DEPTH_EVENTS),
            depth_observations: 0,
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
            unexpected_set_object_provenance: Vec::new(),
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
            if self.bootstrap_calls.len() < KEEP_BOOTSTRAP_CALLS {
                let start = self.starts.get(call.id as usize).copied().flatten();
                let code = start
                    .and_then(|start| {
                        let start = (start as usize).saturating_mul(MME64_DWORDS_PER_GROUP);
                        let end = self
                            .starts
                            .iter()
                            .flatten()
                            .map(|x| (*x as usize).saturating_mul(MME64_DWORDS_PER_GROUP))
                            .filter(|next| *next > start)
                            .min()
                            .unwrap_or(self.mme_words.len())
                            .min(self.mme_words.len());
                        self.mme_words.get(start..end)
                    })
                    .map(|words| words.iter().copied().take_while(Option::is_some).collect())
                    .unwrap_or_default();
                self.bootstrap_calls.push(BootstrapCall {
                    call: call.clone(),
                    start,
                    code,
                });
            }
            keep(&mut self.calls, KEEP_CALLS, call);
        }
    }

    fn direct_event(&mut self, event: String) {
        keep(&mut self.direct, KEEP_DIRECT, event);
    }

    fn method(
        &mut self,
        gp: u32,
        subch: usize,
        addr: u32,
        data: u32,
        open: &mut Option<OpenCall>,
        provenance: (u32, usize, &[u32]),
    ) {
        self.methods += 1;
        if addr != 0x0118 {
            if let Some(upload) = self.mme_upload.take() {
                self.mme_uploads.push(upload);
            }
        }
        if addr == 0 {
            self.finish_call(open);
            let class = data & 0xFFFF;
            if class != 0 && !self.allowed_classes.contains(&class) {
                self.unexpected_set_objects += 1;
                if self.unexpected_set_object_provenance.len() < KEEP_UNEXPECTED_SET_OBJECTS {
                    let (header, dword, words) = provenance;
                    let nearby_start = dword.saturating_sub(3);
                    let nearby_end = (nearby_start + 8).min(words.len());
                    self.unexpected_set_object_provenance
                        .push(UnexpectedSetObject {
                            gp,
                            subch,
                            header,
                            dword,
                            data,
                            nearby_start,
                            nearby: words[nearby_start..nearby_end].to_vec(),
                        });
                }
                self.direct_event(format!(
                    "gp {gp} subch {subch} ignored SET_OBJECT {class:#x}: class is not owned by the host channel"
                ));
                return;
            }
            self.classes[subch] = class;
            self.class_gp[subch] = Some(gp);
            self.class_exact[subch] = true;
            self.submitted_bindings[subch] = Some(SubmittedBinding {
                seq: self.entries,
                gp,
                data,
            });
            return;
        }
        // Windows uses subchannel 0 for the channel's C597 object. A context captured
        // after takeover can contain SET_OBJECT(0) diagnostics even though subsequent
        // methods execute against the restored 3D context. Infer C597 only while that
        // subchannel is unbound; an exact binding to another owned class supersedes it.
        // Other subchannels still require an exact binding.
        let inferred_3d =
            subch == 0 && self.classes[subch] == 0 && self.allowed_classes.contains(&CLASS_3D);
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

        // The existing owned-class gate above applies before numerical offsets are
        // classified. Snapshot binding provenance now: a later rebind cannot relabel
        // this observation, and the subchannel-0 fallback remains explicitly inferred.
        if depth_method_name(addr).is_some() {
            self.depth_observations += 1;
            keep(
                &mut self.depth_events,
                KEEP_DEPTH_EVENTS,
                DepthEvent {
                    seq: self.entries,
                    gp,
                    subch,
                    method: addr,
                    data,
                    exact_3d: self.classes[subch] == CLASS_3D && self.class_exact[subch],
                    binding: self.submitted_bindings[subch],
                },
            );
        }

        if (0x0240..=0x025c).contains(&addr) || matches!(addr, 0x0504 | 0x0508) {
            keep(
                &mut self.root_events,
                KEEP_ROOT_EVENTS,
                RootEvent {
                    seq: self.entries,
                    gp,
                    method: addr,
                    data,
                },
            );
        }

        match addr {
            0x0114 => {
                self.mme_pointer = (data as usize).saturating_mul(MME64_DWORDS_PER_GROUP);
                if self.mme_uploads.len() < KEEP_MME_UPLOADS {
                    self.mme_upload = Some(MmeUpload {
                        seq: self.entries,
                        last_seq: self.entries,
                        gp,
                        pointer: data,
                        version: self.mme_version,
                        words: Vec::new(),
                        word_count: 0,
                    });
                }
            }
            0x0118 => {
                if let Some(upload) = self.mme_upload.as_mut() {
                    upload.last_seq = self.entries;
                    upload.word_count += 1;
                    if upload.words.len() < MAX_MME_WORDS {
                        upload.words.push(data);
                    }
                }
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
            0x0234 => self.mme_version = Some(data),
            0x0550 => {
                self.mme_mem_address =
                    (self.mme_mem_address & 0xFFFF_FFFF) | ((data as u64 & 0xFF) << 32)
            }
            0x0554 => self.mme_mem_address = (self.mme_mem_address & !0xFFFF_FFFF) | data as u64,
            0x0558 => self.mme_data_ram_address = data,
            0x055C => {
                self.mme_dma_reads += 1;
                keep(
                    &mut self.dma_reads,
                    KEEP_DMEM_SOURCES,
                    DmaRead {
                        seq: self.entries,
                        gp,
                        va: self.mme_mem_address,
                        index: self.mme_data_ram_address,
                        words: data,
                        snapshot: None,
                    },
                );
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
                        self.method(
                            gp,
                            subch,
                            addr,
                            words[i + k],
                            &mut open,
                            (hdr, i + k, words),
                        );
                    }
                    i += count;
                }
                4 => self.method(
                    gp,
                    ((hdr >> 13) & 7) as usize,
                    (hdr & 0xFFF) << 2,
                    (hdr >> 16) & 0x1FFF,
                    &mut open,
                    (hdr, i - 1, words),
                ),
                // The legacy formats remain valid on C46F. Their count is at 28:18 and
                // their byte method address is the header's 12:2 field. Treating these
                // payload dwords as fresh headers desynchronizes the entire push (C125).
                0 | 2 if sec_op == 2 || (hdr >> 16) & 3 == 0 => {
                    self.legacy_headers += 1;
                    // NVC46F_DMA_METHOD_COUNT_OLD includes bit 28. Dropping that bit
                    // exposes payload words as headers for packets of 1024+ dwords.
                    let count = ((hdr >> 18) & 0x7FF) as usize;
                    let subch = ((hdr >> 13) & 7) as usize;
                    let addr = hdr & 0x1FFC;
                    let count = count.min(words.len() - i);
                    for k in 0..count {
                        let addr = if sec_op == 0 {
                            addr + 4 * k as u32
                        } else {
                            addr
                        };
                        self.method(
                            gp,
                            subch,
                            addr,
                            words[i + k],
                            &mut open,
                            (hdr, i + k, words),
                        );
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

    /// Startup calls with the installed macro image, up to the next known start or hole.
    /// Windows replaces macro images later; the final image cannot explain initialization.
    pub fn initialization_notes(&self, chid: u32, cutoff: u64) -> Vec<String> {
        let mut out = Vec::new();
        let retained_depth = self
            .depth_events
            .iter()
            .filter(|event| event.seq <= cutoff)
            .count();
        let cutoff_label = if cutoff == u64::MAX {
            "with no finite capture cutoff".into()
        } else {
            format!("at/before capture cutoff {cutoff}")
        };
        out.push(format!(
            "gr live capture: chid {chid}: CPU-submitted depth/ZCULL direct-only history: {retained_depth} retained {cutoff_label}; capacity {KEEP_DEPTH_EVENTS}, {} evicted across whole capture; unseen and macro-generated values unknown, effective GPU state unknown",
            self.depth_observations.saturating_sub(self.depth_events.len() as u64)
        ));
        for event in self.depth_events.iter().filter(|event| event.seq <= cutoff) {
            let binding = event.binding.map_or_else(
                || "no retained SET_OBJECT".into(),
                |binding| {
                    format!(
                        "SET_OBJECT {:#010x}@gp{} capture-seq {}",
                        binding.data, binding.gp, binding.seq
                    )
                },
            );
            out.push(format!(
                "gr live capture: chid {chid}: direct depth/ZCULL gp {} capture-seq {} subch {} {}({:#06x})={:#010x}; C597 classification {}, binding {binding} [CPU-submitted observation, direct-only]",
                event.gp, event.seq, event.subch,
                depth_method_name(event.method).expect("retained allowlisted method"),
                event.method, event.data,
                if event.exact_3d { "exact" } else { "inferred" }
            ));
        }
        for event in self.root_events.iter().filter(|event| event.seq <= cutoff) {
            out.push(format!(
                "gr live capture: chid {chid}: root table gp {} capture-seq {} method {:#x}={:#x}",
                event.gp, event.seq, event.method, event.data
            ));
        }
        // Raw uploads preserve the pointer and payload without assuming pointer units.
        // A normalized image cannot recover overwritten words if that assumption is wrong.
        for upload in self
            .mme_uploads
            .iter()
            .chain(self.mme_upload.iter())
            .filter(|u| u.last_seq <= cutoff)
        {
            out.push(format!(
                "gr live capture: chid {chid}: raw MME upload gp {} capture-seq {}..{} pointer {:#x} words {}/{} SET_MME_VERSION {}",
                upload.gp, upload.seq, upload.last_seq, upload.pointer,
                upload.words.len(), upload.word_count,
                upload.version.map_or_else(|| "unknown".into(), |v| format!("{v:#x}"))
            ));
            for (chunk, words) in upload.words.chunks(16).enumerate() {
                out.push(format!(
                    "gr live capture: chid {chid}: raw MME upload gp {} pointer {:#x} data +{:#x}: {}",
                    upload.gp, upload.pointer, chunk * 16,
                    words.iter().map(|word| format!("{word:08x}")).collect::<Vec<_>>().join(" ")
                ));
            }
        }
        for bootstrap in self.bootstrap_calls.iter().filter(|b| b.call.seq <= cutoff) {
            let call = &bootstrap.call;
            out.push(format!(
                "gr live capture: chid {chid}: initial gp {} capture-seq {} macro {} start {} params {}/{}: {}",
                call.gp, call.seq, call.id,
                bootstrap.start.map_or_else(|| "unknown".into(), |start| format!("{start:#x}")),
                call.params.len(), call.param_count,
                call.params.iter().map(|word| format!("{word:#x}")).collect::<Vec<_>>().join(" ")
            ));
            for (chunk, words) in bootstrap.code.chunks(16).enumerate() {
                out.push(format!(
                    "gr live capture: chid {chid}: initial gp {} macro {} code +{:#x}: {}",
                    call.gp,
                    call.id,
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
        for read in self.dma_reads.iter().filter(|read| read.seq <= cutoff) {
            out.push(format!(
                "gr live capture: chid {chid}: direct DMA_READ gp {} capture-seq {} guest VA {:#x} -> dmem {:#x}, {} word(s)",
                read.gp, read.seq, read.va, read.index, read.words
            ));
            match &read.snapshot {
                Some(Ok(words)) => out.push(format!(
                    "gr live capture: chid {chid}: direct DMA_READ gp {} source snapshot before publication ({} of {} words): {}",
                    read.gp, words.len(), read.words,
                    words.iter().map(|word| format!("{word:08x}")).collect::<Vec<_>>().join(" ")
                )),
                Some(Err(error)) => out.push(format!(
                    "gr live capture: chid {chid}: direct DMA_READ gp {} source snapshot unreadable: {error}", read.gp
                )),
                None => {},
            }
        }
        out
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
            // This is a CPU snapshot, not the eventual DMA result: an earlier GPU
            // producer may still write the source before the macro reads it.
            for read in self
                .dma_reads
                .iter_mut()
                .rev()
                .take_while(|r| r.seq == self.entries)
            {
                let mut bytes = vec![0u8; read.words.min(64) as usize * 4];
                read.snapshot = Some(
                    gm.read_va(root, read.va, &mut bytes)
                        .map(|()| {
                            bytes
                                .chunks_exact(4)
                                .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
                                .collect()
                        })
                        .map_err(|error| format!("{error}")),
                );
            }
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
        for event in &self.unexpected_set_object_provenance {
            out.push(format!(
                "gr live capture: chid {chid}: unexpected SET_OBJECT gp {} subch {} raw header {:#010x}, payload dword {}, data {:#010x}, nearby dwords {}..{}: {} [submitted CPU snapshot]",
                event.gp, event.subch, event.header, event.dword, event.data,
                event.nearby_start, event.nearby_start + event.nearby.len(),
                event.nearby.iter().map(|word| format!("{word:08x}")).collect::<Vec<_>>().join(" ")
            ));
        }
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
                .map(|x| (x as usize).saturating_mul(MME64_DWORDS_PER_GROUP))
            else {
                out.push(format!(
                    "gr live capture: chid {chid}: macro {id} has no captured start address"
                ));
                continue;
            };
            if start >= self.mme_words.len() {
                out.push(format!(
                    "gr live capture: chid {chid}: macro {id} start dword {start} is outside the retained MME image"
                ));
                continue;
            }
            let limit = start.saturating_add(384).min(self.mme_words.len());
            let end = self
                .starts
                .iter()
                .flatten()
                .map(|x| (*x as usize).saturating_mul(MME64_DWORDS_PER_GROUP))
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
    fn direct_depth_history_preserves_exact_binding_across_owned_class_rebinds() {
        let mut capture = Capture::new(&[(CLASS_3D, 1), (0xc5c0, 2), (0x902d, 3)]);
        capture.entries = 1;
        let mut graphics = inc(3, 0, &[(2 << 16) | CLASS_3D]);
        // All allowlisted offsets are real direct C597 methods, including a valid zero.
        for (method, data) in [
            (0x02bc, 1),
            (0x02e4, 0),
            (0x02e8, 0x10),
            (0x02f8, 0xf000000),
            (0x02fc, 1),
            (0x07c0, 1024),
            (0x07c4, 1024),
            (0x07c8, 1),
            (0x07cc, 0),
            (0x07e0, 0x20001),
            (0x07e4, 0x1000),
            (0x07e8, 0x7f),
            (0x07ec, 0xffff0000),
            (0x07f0, 0x80),
            (0x07f4, 0x1234),
            (0x0da8, 3),
            (0x0fe0, 0x20),
            (0x0fe4, 0xff000000),
            (0x0fe8, 0xa),
            (0x0fec, 0x1000),
            (0x1214, 0x40030000),
            (0x12c8, 0),
            (0x12cc, 1),
            (0x12e8, 1),
            (0x130c, 2),
            (0x1968, 1),
            (0x19d0, 1),
        ] {
            graphics.extend(inc(3, method, &[data]));
        }
        // An unbound software subchannel and an unrelated C597 method are excluded.
        graphics.extend(inc(7, 0x02e4, &[0xdead]));
        graphics.extend(inc(3, 0x2388, &[0xbeef]));
        capture.push(63, &graphics);
        assert_eq!(capture.depth_events.len(), 27);

        capture.entries = 2;
        let mut other_classes = inc(3, 0, &[(1 << 16) | 0xc5c0]);
        other_classes.extend(inc(3, 0x02e4, &[0xdead_c5c0]));
        other_classes.extend(inc(3, 0, &[0x902d]));
        other_classes.extend(inc(3, 0x12cc, &[0xdead_902d]));
        other_classes.extend(inc(3, 0x07e8, &[0xdead_07e8]));
        capture.push(0, &other_classes);
        assert_eq!(capture.depth_events.len(), 27);

        capture.entries = 3;
        let mut rebound = inc(3, 0, &[(1 << 16) | CLASS_3D]);
        rebound.extend(inc(3, 0x12e8, &[0]));
        capture.push(1, &rebound);
        let first = capture.depth_events.front().unwrap();
        assert!(first.exact_3d);
        assert_eq!((first.seq, first.gp, first.subch), (1, 63, 3));
        let binding = first.binding.unwrap();
        assert_eq!((binding.seq, binding.gp, binding.data), (1, 63, 0x2c597));
        let last = capture.depth_events.back().unwrap();
        assert_eq!((last.seq, last.gp, last.data), (3, 1, 0));
        assert_eq!(last.binding.unwrap().data, 0x1c597);

        let earlier = capture.initialization_notes(54, 1).join("\n");
        assert!(earlier.contains("SET_ZCULL_ROP_BYPASS(0x02e4)=0x00000000"));
        assert!(earlier.contains("SET_ZCULL_STORAGE_B(0x07ec)=0xffff0000"));
        assert!(earlier.contains("CLEAR_ZCULL_REGION(0x12c8)=0x00000000"));
        assert!(earlier.contains("CLEAR_SURFACE(0x19d0)=0x00000001"));
        assert!(earlier.contains("DRAW_VERTEX_ARRAY_BEGIN_END_INSTANCE_FIRST(0x1214)=0x40030000"));
        assert!(earlier.contains(
            "C597 classification exact, binding SET_OBJECT 0x0002c597@gp63 capture-seq 1"
        ));
        assert!(!earlier.contains("binding SET_OBJECT 0x0001c597"));
        assert!(!earlier.contains("dead"));
        assert!(!earlier.contains("beef"));
        let latest = capture.initialization_notes(54, 3).join("\n");
        assert!(latest.contains(
            "direct depth/ZCULL gp 1 capture-seq 3 subch 3 SET_DEPTH_WRITE(0x12e8)=0x00000000"
        ));
        assert!(latest.contains("binding SET_OBJECT 0x0001c597@gp1 capture-seq 3"));
    }

    #[test]
    fn direct_depth_history_is_bounded_and_filters_capture_sequence_not_wrapped_gp() {
        let mut capture = Capture::new(&[(CLASS_3D, 1)]);
        capture.entries = 1;
        capture.push(3, &inc(0, 0, &[CLASS_3D]));
        for seq in 1..=104 {
            capture.entries = seq;
            // Repeated GP indices must not make future submissions fault-bound.
            capture.push((seq % 4) as u32, &inc(0, 0x02e4, &[seq as u32]));
        }
        assert_eq!(capture.depth_observations, 104);
        assert_eq!(capture.depth_events.len(), 96);
        assert_eq!(capture.depth_events.front().unwrap().seq, 9);
        assert_eq!(capture.depth_events.back().unwrap().seq, 104);
        let notes = capture.initialization_notes(54, 100);
        let direct: Vec<_> = notes
            .iter()
            .filter(|line| line.contains(": direct depth/ZCULL gp "))
            .collect();
        assert_eq!(direct.len(), 92);
        assert!(direct.first().unwrap().contains("capture-seq 9 "));
        assert!(direct.last().unwrap().contains("capture-seq 100 "));
        assert!(notes[0].contains("capacity 96, 8 evicted across whole capture"));
        assert!(notes[0].contains("effective GPU state unknown"));
        let evicted = capture.initialization_notes(54, 8).join("\n");
        assert!(evicted.contains("0 retained at/before capture cutoff 8"));
        assert!(!evicted.contains(": direct depth/ZCULL gp "));
    }

    #[test]
    fn direct_depth_history_keeps_inference_and_unbind_provenance_explicit() {
        let mut capture = Capture::new(&[(CLASS_3D, 1), (0xc5c0, 2)]);
        capture.entries = 1;
        capture.push(5, &inc(0, 0x1968, &[1]));
        capture.entries = 2;
        let mut compute = inc(0, 0, &[0xc5c0]);
        compute.extend(inc(0, 0x1968, &[0xdead]));
        capture.push(6, &compute);
        capture.entries = 3;
        let mut unbound = inc(0, 0, &[0]);
        unbound.extend(inc(0, 0x1968, &[0]));
        capture.push(7, &unbound);
        capture.entries = 4;
        let mut exact = inc(0, 0, &[(2 << 16) | CLASS_3D]);
        exact.extend(inc(0, 0x1968, &[1]));
        capture.push(0, &exact);
        assert_eq!(capture.depth_events.len(), 3);
        assert!(!capture.depth_events[0].exact_3d);
        assert!(capture.depth_events[0].binding.is_none());
        assert!(!capture.depth_events[1].exact_3d);
        assert_eq!(capture.depth_events[1].binding.unwrap().data, 0);
        assert!(capture.depth_events[2].exact_3d);
        let older = capture.initialization_notes(54, 3).join("\n");
        assert!(older.contains("C597 classification inferred, binding no retained SET_OBJECT"));
        assert!(older.contains(
            "C597 classification inferred, binding SET_OBJECT 0x00000000@gp7 capture-seq 3"
        ));
        assert!(!older.contains("classification exact"));
        let all = capture.initialization_notes(54, u64::MAX).join("\n");
        assert!(all.contains("no finite capture cutoff"));
        assert!(all.contains("unseen and macro-generated values unknown"));
    }

    #[test]
    fn long_legacy_payloads_do_not_become_set_object_headers() {
        for count in [1024, 1025] {
            let payload: Vec<u32> = (0..count)
                .map(|i| if i % 2 == 0 { 0x20010000 } else { 0x5540 })
                .collect();
            let mut words = old_noninc(0, 0x118, &payload);
            words.extend(inc(0, 0x11c, &[7, 0x112]));
            let mut capture = Capture::new(&[(CLASS_3D, 1)]);
            capture.push(23, &words);
            assert_eq!(capture.mme_writes, count as u64);
            assert_eq!(
                capture.mme_words[..count],
                payload.iter().copied().map(Some).collect::<Vec<_>>()
            );
            assert_eq!(capture.unexpected_set_objects, 0);
            assert!(capture.unexpected_set_object_provenance.is_empty());
            assert_eq!(capture.classes[0], CLASS_3D);
            assert_eq!(capture.starts[7], Some(0x112));
            assert_eq!(capture.start_writes, 1);
        }
    }

    #[test]
    fn first_unexpected_bind_provenance_survives_recent_event_eviction() {
        let mut words = Vec::new();
        for class in 0x5540..0x554a {
            words.extend(inc(0, 0, &[(1 << 16) | class]));
        }
        // Evict the recent direct events: provenance must still retain the first
        // unexpected packets, without retaining entire pushbuffers.
        for _ in 0..40 {
            words.extend(inc(0, 0x2410, &[0]));
        }
        let mut capture = Capture::new(&[(CLASS_3D, 1)]);
        capture.push(31, &words);
        assert_eq!(capture.unexpected_set_objects, 10);
        assert_eq!(capture.unexpected_set_object_provenance.len(), 8);
        for (index, event) in capture.unexpected_set_object_provenance.iter().enumerate() {
            assert_eq!(event.gp, 31);
            assert_eq!(event.subch, 0);
            assert_eq!(event.header, 0x20010000);
            assert_eq!(event.dword, 2 * index + 1);
            assert_eq!(event.data, 0x15540 + index as u32);
            assert!(event.nearby.len() <= 8);
            assert_eq!(event.nearby[event.dword - event.nearby_start], event.data);
        }
        let notes = capture.notes(22).join("\n");
        assert!(
            notes.contains("raw header 0x20010000, payload dword 1, data 0x00015540"),
            "{notes}"
        );
        assert!(!notes.contains("data 0x00015549"), "{notes}");
    }

    #[test]
    fn compute_falcon_methods_do_not_replace_graphics_dma_state() {
        let mut capture = Capture::new(&[(CLASS_3D, 0xff1fe026), (0xc5c0, 0xff1fe032)]);
        // Establish graphics DMA state, then use the same offsets on an owned compute
        // object. C5C0 names 0x550..0x55c SET_FALCON20..23, rather than MME DMA state.
        let mut words = inc(0, 0, &[CLASS_3D]);
        words.extend(inc(0, 0x550, &[0, 0x1000, 0x56, 2]));
        words.extend(inc(0, 0, &[(1 << 16) | 0xc5c0]));
        words.extend(inc(0, 0x550, &[5, 1, 1, 0]));
        capture.push(0, &words);
        assert_eq!(capture.classes[0], 0xc5c0);
        assert!(capture.class_exact[0]);
        assert_eq!(capture.mme_dma_reads, 1);
        assert_eq!(capture.mme_mem_address, 0x1000);
        assert_eq!(capture.mme_data_ram_address, 0x56);

        // SET_OBJECT encodes ENGINE_ID in bits 20:16, separately from CLASS_ID.
        // Rebinding graphics must resume capture with its retained graphics state.
        let mut graphics = inc(0, 0, &[(2 << 16) | CLASS_3D]);
        graphics.extend(inc(0, 0x55c, &[2]));
        capture.push(1, &graphics);
        assert_eq!(capture.classes[0], CLASS_3D);
        assert_eq!(capture.unexpected_set_objects, 0);
        assert_eq!(capture.mme_dma_reads, 2);
        assert_eq!(capture.dma_reads.back().unwrap().va, 0x1000);
        assert_eq!(capture.dma_reads.back().unwrap().index, 0x56);
    }

    #[test]
    fn owned_2d_upload_does_not_overwrite_graphics_macro_image() {
        let mut capture = Capture::new(&[(CLASS_3D, 0xff1fe026), (0x902d, 0xff1fe02c)]);
        let mut words = inc(0, 0, &[CLASS_3D]);
        words.extend(inc(0, 0x11c, &[0, 0]));
        words.extend(inc(0, 0x114, &[0]));
        words.extend(old_noninc(0, 0x118, &[0xaa, 0xbb, 0xcc]));
        // NV902D has the same upload offsets, but its code and starts are not C597's.
        words.extend(inc(0, 0, &[(1 << 16) | 0x902d]));
        words.extend(inc(0, 0x11c, &[0, 7]));
        words.extend(inc(0, 0x114, &[0]));
        words.extend(old_noninc(0, 0x118, &[0xdd, 0xee, 0xff]));
        capture.push(0, &words);
        assert_eq!(capture.classes[0], 0x902d);
        assert!(capture.class_exact[0]);
        assert_eq!(capture.start_writes, 1);
        assert_eq!(capture.mme_writes, 3);
        assert_eq!(capture.mme_uploads.len(), 1);

        let mut graphics = inc(0, 0, &[(2 << 16) | CLASS_3D]);
        graphics.extend(oneinc(0, 0x3800, &[0x56, 0x7105000]));
        capture.push(1, &graphics);
        assert_eq!(capture.unexpected_set_objects, 0);
        assert_eq!(capture.bootstrap_calls.len(), 1);
        assert_eq!(capture.bootstrap_calls[0].start, Some(0));
        assert_eq!(
            capture.bootstrap_calls[0].code,
            vec![Some(0xaa), Some(0xbb), Some(0xcc)]
        );
    }

    #[test]
    fn startup_code_survives_replacement_and_call_history_eviction() {
        let mut capture = Capture::new(&[(CLASS_3D, 1)]);
        capture.entries = 1;
        let mut first = inc(0, 0x11c, &[0, 0]);
        first.extend(inc(0, 0x114, &[0]));
        first.extend(old_noninc(0, 0x118, &[0xaa, 0xbb, 0xcc]));
        first.extend(oneinc(0, 0x3800, &[0x56, 0x1234]));
        first.extend(inc(0, 0x550, &[0, 0x1000, 0x80, 4]));
        capture.push(0, &first);
        capture.entries = 2;
        let mut replacement = inc(0, 0x114, &[0]);
        replacement.extend(old_noninc(0, 0x118, &[0xdd, 0xee, 0xff]));
        replacement.extend(oneinc(0, 0x3800, &[0x57, 0x5678]));
        replacement.extend(inc(0, 0x550, &[0, 0x2000, 0x56, 2]));
        capture.push(1, &replacement);
        for i in 0..KEEP_CALLS {
            capture.entries += 1;
            capture.push(i as u32 + 2, &oneinc(0, 0x3800, &[0]));
        }
        assert!(capture.call_notes_at(3, 0, 1).is_empty());
        let notes = capture.initialization_notes(3, 1).join("\n");
        assert!(
            notes.contains("initial gp 0 capture-seq 1 macro 0 start 0x0 params 2/2: 0x56 0x1234"),
            "{notes}"
        );
        assert!(
            notes.contains("initial gp 0 macro 0 code +0x0: 000000aa 000000bb 000000cc"),
            "{notes}"
        );
        assert!(
            notes.contains("guest VA 0x1000 -> dmem 0x80, 4 word(s)"),
            "{notes}"
        );
        assert!(!notes.contains("initial gp 1"), "{notes}");
        assert!(!notes.contains("guest VA 0x2000"), "{notes}");
        let later = capture.initialization_notes(3, 2).join("\n");
        assert!(
            later.contains("initial gp 1 macro 0 code +0x0: 000000dd 000000ee 000000ff"),
            "{later}"
        );
    }

    #[test]
    fn startup_image_keeps_initialization_beyond_eight_instructions() {
        let mut capture = Capture::new(&[(CLASS_3D, 1)]);
        capture.entries = 1;
        let image: Vec<u32> = (0..60).map(|i| 0x1000 + i).collect();
        let mut first = inc(0, 0x11c, &[0, 0]);
        first.extend(inc(0, 0x114, &[0]));
        first.extend(old_noninc(0, 0x118, &image));
        first.extend(oneinc(0, 0x3800, &[0x56, 0x1234]));
        capture.push(0, &first);
        // A later image must not replace a startup instruction past the former prefix.
        let mut replacement = inc(0, 0x114, &[0]);
        replacement.extend(old_noninc(0, 0x118, &[0xdead; 60]));
        capture.push(1, &replacement);
        let bootstrap = &capture.bootstrap_calls[0];
        assert_eq!(bootstrap.code.len(), 60);
        assert_eq!(bootstrap.code[30], Some(0x101e));
        assert_eq!(bootstrap.code[59], Some(0x103b));
    }

    #[test]
    fn raw_uploads_preserve_payloads_with_overlapping_pointer_ranges() {
        let mut capture = Capture::new(&[(CLASS_3D, 1)]);
        capture.entries = 1;
        let mut first = inc(0, 0x114, &[0]);
        first.extend(old_noninc(0, 0x118, &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]));
        capture.push(0, &first);
        capture.entries = 2;
        let mut next = inc(0, 0x114, &[1]);
        next.extend(old_noninc(0, 0x118, &[0x11, 0x22, 0x33]));
        next.extend(oneinc(0, 0x3800, &[0]));
        capture.push(1, &next);
        assert_eq!(capture.mme_words[3], Some(0x11));
        let notes = capture.initialization_notes(7, 1).join("\n");
        assert!(notes.contains("pointer 0x0 words 6/6"), "{notes}");
        assert!(
            notes.contains("000000aa 000000bb 000000cc 000000dd 000000ee 000000ff"),
            "{notes}"
        );
        assert!(!notes.contains("pointer 0x1"), "{notes}");
        let notes = capture.initialization_notes(7, 2).join("\n");
        assert!(notes.contains("pointer 0x1 words 3/3"), "{notes}");
    }

    #[test]
    fn turing_group_addresses_preserve_adjacent_uploads_and_bootstrap_bodies() {
        // C136's raw C597 uploads: 30 dwords at group 0, six at 0xa, nine at
        // 0xc, and the two-group DWRITE macro at 0x112. Treating these pointers
        // as dword addresses overlaps the first three uploads and truncates macro 27.
        let macro0 = [
            0x00d6a403, 0x18c00238, 0x318c0300, 0x0034015f, 0x18c00238, 0x71ac7b00, 0x0034015b,
            0x18c00238, 0xb1ac7b00, 0x007411e3, 0x18c00238, 0xf1a1db00, 0x00000003, 0x18c0011e,
            0x71915b00, 0x00000003, 0x18c0011e, 0xb190db00, 0x00182393, 0x18c0011e, 0xf1905b00,
            0x40600003, 0x18c00000, 0x318c0300, 0x20200003, 0x02c00000, 0x303c0301, 0x20200003,
            0x00c00000, 0x301c0300,
        ];
        let macro1 = [
            0x00d43003, 0x18c00026, 0x31c00301, 0x10740003, 0x18c00427, 0x300c0300,
        ];
        let macro2 = [
            0x00d42003, 0x1d000026, 0x31c08300, 0x1c74426b, 0x18c00143, 0x700c0301, 0x00200003,
            0x18c00000, 0x301c0300,
        ];
        let macro27 = [
            0x00000003, 0x18c00000, 0x3bcc7f01, 0x00000003, 0x18c00000, 0x318c0300,
        ];
        let mut words = inc(0, 0x11c, &[0]);
        words.extend(old_noninc(0, 0x120, &[0, 0xa, 0xc, 0xf]));
        words.extend(inc(0, 0x11c, &[27]));
        words.extend(old_noninc(0, 0x120, &[0x112, 0x114]));
        for (pointer, body) in [
            (0, macro0.as_slice()),
            (0xa, macro1.as_slice()),
            (0xc, macro2.as_slice()),
            (0x112, macro27.as_slice()),
        ] {
            words.extend(inc(0, 0x114, &[pointer]));
            words.extend(old_noninc(0, 0x118, body));
        }
        for id in [0, 1, 2, 27] {
            words.extend(oneinc(0, 0x3800 + id * 8, &[0x56, 0x14c50000]));
        }
        let mut capture = Capture::new(&[(CLASS_3D, 1)]);
        capture.push(16, &words);
        for (bootstrap, expected) in capture.bootstrap_calls.iter().zip([
            macro0.as_slice(),
            macro1.as_slice(),
            macro2.as_slice(),
            macro27.as_slice(),
        ]) {
            assert_eq!(
                bootstrap.code,
                expected.iter().copied().map(Some).collect::<Vec<_>>()
            );
        }
        assert_eq!(capture.bootstrap_calls.len(), 4);
        let notes = capture.notes(22).join("\n");
        assert!(
            notes.contains("macro 27 start dword 822, captured 6 dword(s)"),
            "{notes}"
        );
        assert!(
            notes.contains(
                "macro 27 code +0x0: 00000003 18c00000 3bcc7f01 00000003 18c00000 318c0300"
            ),
            "{notes}"
        );
        let raw = capture.initialization_notes(22, u64::MAX).join("\n");
        assert!(raw.contains("pointer 0xa words 6/6"), "{raw}");
        assert!(raw.contains("pointer 0x112 words 6/6"), "{raw}");
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
        // Raw unexpected-bind provenance can include neighboring software-subchannel
        // words. They must not enter the retained graphics macro calls themselves.
        assert!(c
            .calls
            .iter()
            .all(|call| call.subch != 7 && !call.params.contains(&0xDEAD_BEEF)));
        assert!(notes.contains("**VALID VA 0**"), "{notes}");
        assert!(notes.contains("final direct CB state none"), "{notes}");
        assert!(
            notes.contains("macro 7 code +0x0: 000000aa 000000bb 000000cc"),
            "{notes}"
        );
        assert!(notes.contains("complete MME start image: 7=0xc"), "{notes}");
        assert!(
            notes.contains("complete MME code +0x20: -------- -------- -------- -------- 000000aa 000000bb 000000cc"),
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
