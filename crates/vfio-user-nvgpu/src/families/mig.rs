// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The `mig` and `mig_partitioning` families (docs/caraxes/gsp-plan/e-families.md §3.8,
//! docs/caraxes/limits-mig-ecc-vgpu.md §3): the MIG mode and the GPU and compute instances
//! as per-VM bookkeeping, from `kernel_mig_manager.c` and the headers of the emulated driver
//! version. Paths are relative to the 610.57.04 tree; KMM is
//! src/nvidia/src/kernel/gpu/mig_mgr/kernel_mig_manager.c.
//!
//! What the guest kernel asks and what the model answers:
//!
//! * **Boot** (`_kmigmgrHandlePostSchedulingEnableCallback`, KMM:943-1046; every failure is
//!   fatal to the driver load): `INTERNAL_MEMSYS_SET_PARTITIONABLE_MEM` (the range as asked,
//!   mem_mgr.c:2504-2526), `INTERNAL_GPU_GET_SMC_MODE`, then the static tables of
//!   `kmigmgrLoadStaticInfo_KERNEL` (KMM:1549-1706): the partitionable engines (NV2080 engine
//!   type bits), the skyline, the per-swizzId memory in VMMU segments
//!   (kern_mem_sys_gh100.c:595-635), the compute profiles and the GPU-instance profiles.
//!   The VMMU segment size (`GPU_GET_VMMU_SEGMENT_SIZE`, gpu.c:925-962) must not be 0
//!   (kern_mem_sys.c:765-766).
//! * **Mode** (`GPU_SET_PARTITIONING_MODE` becomes `INTERNAL_MIGMGR_SET_PARTITIONING_MODE`,
//!   then `GET_SMC_MODE`, KMM:3934-3987 and :7685-7723): on a chip with resetless MIG (GH100,
//!   `PDB_PROP_GPU_RESETLESS_MIG_SUPPORTED`, g_gpu_nvoc.c:341-351) the mode applies at once;
//!   otherwise it is pending (`*_PENDING`, ctrl2080gpu.h:168-172) until the next reset.
//! * **GPU instances**: `GPU_GET_PARTITION_CAPACITY` (routed to GSP), then
//!   `INTERNAL_MIGMGR_SET_GPU_INSTANCES` (the model picks the swizzId and the UUID,
//!   KMM:7770-7780), `INTERNAL_MIGMGR_EXPORT_GPU_INSTANCE` (the resources, KMM:4291-4330:
//!   enginesMask in RM_ENGINE_TYPE numbering, gpu_engine_type.h:34-118), and
//!   `INTERNAL_MIGMGR_GET_GPU_INSTANCES` for `GPU_GET_PARTITIONS` (KMM:7994-8150, the
//!   kernel's order: ascending swizzId).
//! * **Compute instances** on a subscription object (AMPERE_SMC_PARTITION_REF 0xC637, whose
//!   allocation is RPC'd, resource_list.h:1059-1079): `NVC637_CTRL_CMD_EXEC_PARTITIONS_CREATE`
//!   with the caller's handles, `_EXPORT` on the kernel's internal GI handles
//!   (gpu_instance_subscription.c:386-583), `_DELETE`.
//!
//! Capacity is geometry and engines: a GPU-instance profile fits at a swizzId whose memory
//! size it has (0 full, 1-2 half, 3-6 quarter, 7-14 eighth, kmigmgr_ga100.c:308-353) and
//! whose span (kmigmgr_ga100.c:392-474) holds as many of the chip's GPCs as the profile
//! needs; the instances then also need a GR engine and the profile's copy and media engines
//! from what is left. The chip file's `[mig]` holds the table. What the model cannot give is
//! isolation: every instance's work runs in the one executing context, and pure emulation
//! executes nothing.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{id, off, rd32, reply, size, wr32, wr64, wr8, Families, Mode};
use crate::chip::ChipData;
use crate::ctrl::Answer;
use crate::ver::active::{self as v, NV_ERR_INVALID_ARGUMENT, NV_ERR_NOT_SUPPORTED, NV_OK};

/// `NV_ERR_INSUFFICIENT_RESOURCES`, `NV_ERR_IN_USE`, `NV_ERR_INVALID_STATE`
/// (nvstatuscodes.h:55, :52, :93).
const NV_ERR_INSUFFICIENT_RESOURCES: u32 = 0x1A;
const NV_ERR_IN_USE: u32 = 0x17;
const NV_ERR_INVALID_STATE: u32 = 0x40;

/// `NV2080_CTRL_GPU_INFO_GPU_SMC_MODE_*` (ctrl2080gpu.h:168-172).
const SMC_ENABLED: u32 = 1;
const SMC_DISABLED: u32 = 2;
const SMC_ENABLE_PENDING: u32 = 3;
const SMC_DISABLE_PENDING: u32 = 4;
/// `NV2080_CTRL_GPU_PARTITION_FLAG_PLACE_AT_SPAN` 31:31 (ctrl2080gpu.h:2640-2642).
const PLACE_AT_SPAN: u32 = 1 << 31;
/// `NVC637_CTRL_DMA_EXEC_PARTITIONS_CREATE_REQUEST_WITH_PART_ID` bit 0 and `_AT_SPAN`
/// bit 1 (ctrlc637.h:193-198).
const CI_WITH_PART_ID: u32 = 1;
const CI_AT_SPAN: u32 = 2;
/// `KMIGMGR_MAX_GPU_SWIZZID` (g_kernel_mig_manager_nvoc.h:84) and the 8 compute instances
/// of a GPU instance (g_gpu_mgr_nvoc.h:73-74).
const SWIZZ_IDS: u32 = 15;
const MAX_CI: u32 = 8;
/// The device-profiling swizzId of AMPERE_SMC_PARTITION_REF (clc637.h:39).
const SWIZZ_DEVICE_PROFILING: u32 = 0xFFFF_FFFE;
/// AMPERE_SMC_PARTITION_REF and AMPERE_SMC_EXEC_PARTITION_REF: capDescriptor u64 @0 and
/// swizzId / execPartitionId u32 @8 (clc637.h:50-61, clc638.h:38-49).
const AMPERE_SMC_PARTITION_REF: u32 = 0xC637;
const AMPERE_SMC_EXEC_PARTITION_REF: u32 = 0xC638;
const ALLOC_ID: usize = 8;

/// D9 labels.
pub const MODE: &str = "guest (MIG mode of this VM, InfoROM analogue)";
pub const TABLE: &str = "costume (MIG profile tables of the chip file, from the kernel's rules)";
pub const INSTANCES: &str = "guest (GPU and compute instances of this VM: bookkeeping, no isolation)";
pub const REFUSED: &str = "refused (MIG path that needs the physical crossbar or a vGPU plugin)";

/// One GPU-instance profile of `[mig]`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileData {
    /// nvidia-smi's name (NVML derives the shown name itself).
    pub name: String,
    /// `NV2080_CTRL_GPU_PARTITION_FLAG_*` (ctrl2080gpu.h:2612-2665).
    pub flag: u32,
    pub gpcs: u32,
    pub ce: u32,
    pub nvdec: u32,
    pub nvenc: u32,
    pub nvjpg: u32,
    pub ofa: u32,
}

/// `[mig]` of chip.toml.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigData {
    /// `PDB_PROP_GPU_RESETLESS_MIG_SUPPORTED`: the mode applies without a reset.
    pub resetless: bool,
    /// `GPU_GET_VMMU_SEGMENT_SIZE`, MiB.
    pub vmmu_segment_mib: u64,
    /// GPCs of the chip MIG can hand out (GPC `i` sits in span slot `i`).
    pub gpcs: u32,
    /// `MAX_PARTITIONABLE_GPCS`: the span length the kernel slices (kmigmgr_ga100.c:411).
    pub span_len: u32,
    pub tpcs_per_gpc: u32,
    pub sms_per_gpc: u32,
    pub veids_per_gpc: u32,
    /// swizzIds whose memory range is empty ("rejected by HW", kern_mem_sys_gh100.c:577-583).
    pub rejected_swizz: Vec<u32>,
    /// The partitionable engines, by RM/NV2080 engine name (`GR0`, `COPY0`, `NVDEC0`, ...).
    pub engines: Vec<String>,
    pub profile: Vec<ProfileData>,
    pub label: String,
}

/// A compute instance.
#[derive(Clone, Debug)]
pub struct Ci {
    pub id: u32,
    /// GI-local GPC indices.
    pub gpcs: Vec<u32>,
    pub compute_size: u32,
    pub span_start: u32,
    pub flags: u32,
}

/// A GPU instance.
#[derive(Clone, Debug)]
pub struct Gi {
    pub swizz: u32,
    pub flag: u32,
    pub profile: usize,
    /// Global GPC indices.
    pub gpcs: Vec<u32>,
    /// RM_ENGINE_TYPE values of its engines (GR first).
    pub engines: Vec<u32>,
    pub uuid: [u8; 16],
    pub cis: BTreeMap<u32, Ci>,
}

/// What a subscription object stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sub {
    Gi(u32),
    Ci(u32, u32),
}

/// The GPU and compute instances of one VM.
#[derive(Clone, Debug, Default)]
pub struct MigState {
    pub gis: BTreeMap<u32, Gi>,
    /// (hClient, hObject) of a 0xC637/0xC638 object -> what it subscribes to.
    pub subs: BTreeMap<(u32, u32), Sub>,
    /// The partitionable range the kernel set: [start, end].
    pub range: Option<(u64, u64)>,
}

impl MigState {
    pub fn free(&mut self, h_client: u32, h_object: u32) {
        if h_client == h_object {
            self.subs.retain(|(c, _), _| *c != h_client);
        } else {
            self.subs.remove(&(h_client, h_object));
        }
    }
}

fn is(cmd: u32, name: &str) -> bool {
    id(name) == Some(cmd)
}

/// `COMPUTE_SIZE` 4:2 and `MEMORY_SIZE` 1:0 of a partition flag.
fn compute_size(flag: u32) -> u32 {
    (flag >> 2) & 7
}
fn memory_size(flag: u32) -> u32 {
    flag & 3
}

/// The swizzIds of a memory size (kmigmgrMemSizeFlagToSwizzIdRange_GA100).
fn swizz_range(mem: u32) -> std::ops::RangeInclusive<u32> {
    match mem {
        0 => 0..=0,
        1 => 1..=2,
        2 => 3..=6,
        _ => 7..=14,
    }
}

/// The span of `swizz` in slots of `len` (kmigmgrSwizzIdToSpan_GA100): [lo, hi].
fn span(swizz: u32, len: u32) -> (u32, u32) {
    match swizz {
        0 => (0, len - 1),
        1 => (0, len / 2 - 1),
        2 => (len / 2, len - 1),
        3..=6 => {
            let q = len / 4;
            let k = swizz - 3;
            (k * q, (k + 1) * q - 1)
        }
        s => (s - 7, s - 7),
    }
}

/// `NV2080_CTRL_GPU_PARTITION_FLAG_COMPUTE_SIZE_*` -> CTS id range (KMM:8857-8886).
fn cts_mask(cs: u32, gpcs: u32) -> u64 {
    let (lo, n) = match cs {
        0 => (0, 1),
        1 => (1, 2),
        2 => (3, 2),
        3 => (5, 4),
        4 => (9, 4),
        _ => (13, 8),
    };
    // Only as many eighth-size ids as the chip has GPCs.
    let n = if cs == 5 { n.min(gpcs) } else { n };
    ((1u64 << n) - 1) << lo
}

fn engine(table: &[(&str, u32)], name: &str) -> u32 {
    v::lookup(table, name).unwrap_or_else(|| panic!("[mig] engines: {name} is no engine name"))
}

fn kind(name: &str) -> &'static str {
    ["GR", "COPY", "NVDEC", "NVENC", "NVJPEG", "OFA"]
        .into_iter()
        .find(|k| name.starts_with(k))
        .unwrap_or("")
}

impl MigData {
    fn smc_mode(&self, m: Mode) -> u32 {
        match (m.current, m.pending) {
            (true, true) => SMC_ENABLED,
            (false, false) => SMC_DISABLED,
            (false, true) => SMC_ENABLE_PENDING,
            (true, false) => SMC_DISABLE_PENDING,
        }
    }

    /// GPCs of the chip inside `swizz`'s span.
    fn span_gpcs(&self, swizz: u32) -> Vec<u32> {
        let (lo, hi) = span(swizz, self.span_len);
        (lo..=hi).filter(|&g| g < self.gpcs).collect()
    }

    /// swizzIds where profile `p` fits the chip at all (geometry and memory).
    fn placements(&self, p: &ProfileData) -> Vec<u32> {
        swizz_range(memory_size(p.flag))
            .filter(|s| !self.rejected_swizz.contains(s))
            .filter(|&s| self.span_gpcs(s).len() as u32 >= p.gpcs)
            .collect()
    }

    fn profile_by_flag(&self, flag: u32) -> Option<usize> {
        self.profile.iter().position(|p| p.flag == flag & !PLACE_AT_SPAN)
    }

    /// Names of the engines no GI holds, by kind.
    fn free_engines(&self, st: &MigState) -> Vec<&str> {
        let used: Vec<u32> = st.gis.values().flat_map(|g| g.engines.clone()).collect();
        self.engines
            .iter()
            .map(String::as_str)
            .filter(|n| !used.contains(&engine(v::RM_ENGINE_TYPES, n)))
            .collect()
    }

    /// The engines a GI of profile `p` takes from `free`, or `None` when they are not there.
    fn take_engines(&self, p: &ProfileData, free: &[&str]) -> Option<Vec<u32>> {
        let mut out = Vec::new();
        for (k, n) in [
            ("GR", 1),
            ("COPY", p.ce),
            ("NVDEC", p.nvdec),
            ("NVENC", p.nvenc),
            ("NVJPEG", p.nvjpg),
            ("OFA", p.ofa),
        ] {
            let have: Vec<&&str> = free.iter().filter(|e| kind(e) == k).collect();
            if (have.len() as u32) < n {
                return None;
            }
            out.extend(have[..n as usize].iter().map(|e| engine(v::RM_ENGINE_TYPES, e)));
        }
        Some(out)
    }

    /// Placements of profile `p` still free in `st`: the span overlaps no GI's.
    fn available(&self, p: &ProfileData, st: &MigState) -> Vec<u32> {
        if self.take_engines(p, &self.free_engines(st)).is_none() {
            return Vec::new();
        }
        self.placements(p)
            .into_iter()
            .filter(|&s| {
                let (lo, hi) = span(s, self.span_len);
                st.gis.values().all(|g| {
                    let (a, b) = span(g.swizz, self.span_len);
                    hi < a || b < lo
                })
            })
            .collect()
    }

    /// The memory of every swizzId in VMMU segments (absolute), from the partitionable
    /// range the way kmemsysPopulateMIGGPUInstanceMemConfig_KERNEL cuts it
    /// (kern_mem_sys.c:769-784).
    fn page_ranges(&self, range: Option<(u64, u64)>, fb: u64) -> Vec<(u64, u64)> {
        let seg = self.vmmu_segment_mib << 20;
        let (start, end) = range.unwrap_or((0, fb - 1));
        let a = if start != 0 {
            if start % seg == 0 {
                start + seg
            } else {
                start.div_ceil(seg) * seg
            }
        } else {
            0
        };
        let e = if (end + 1) % seg == 0 { end - seg } else { end };
        let total = (e.saturating_sub(a) + 1) / seg;
        let first = a / seg;
        let eighth = total / 8;
        (0..SWIZZ_IDS)
            .map(|s| {
                if self.rejected_swizz.contains(&s) || eighth == 0 {
                    return (1, 0);
                }
                // Slot 0 at the top of the range: the kernel's own allocations sit at the
                // bottom of PMA, and a GI whose range overlaps them cannot reserve its
                // context-buffer pool (ctx_buf_pool.c:315; caraxes/runs/limits-2026-09-25/
                // mig/gh100-2, with slot 0 at the bottom). Which end a real GSP-RM uses is
                // its AMAP's; the kernel only takes the ranges as given.
                let (lo, hi) = span(s, 8);
                let top = first + 8 * eighth;
                (top - (hi as u64 + 1) * eighth, top - lo as u64 * eighth - 1)
            })
            .collect()
    }
}

fn uuid(swizz: u32, ci: Option<u32>) -> [u8; 16] {
    // A per-VM instance UUID: "MIG" instances are this device's, never a host card's.
    let mut u = *b"CARAXES-MIG-GI\0\0";
    u[14] = swizz as u8;
    u[15] = ci.map_or(0xFF, |c| c as u8);
    u
}

/// The MIG answers, when the chip models the family.
pub fn answer(
    f: &mut Families,
    chip: &ChipData,
    object: (u32, u32),
    cmd: u32,
    params: &[u8],
) -> Option<Answer> {
    let m = chip.mig.as_ref()?;
    if !super::modelled(chip, "mig") {
        return None;
    }
    let n = params.len();
    let mode = f.persist.mig.unwrap_or_default();
    let blank = |st: &str| vec![0u8; n.max(size(st))];
    let done = |mut b: Vec<u8>, label: &'static str, note: String| {
        b.truncate(n);
        Some(reply(NV_OK, b, label, note))
    };
    if is(cmd, "NV2080_CTRL_CMD_GPU_GET_VMMU_SEGMENT_SIZE") {
        let st = "NV2080_CTRL_GPU_GET_VMMU_SEGMENT_SIZE_PARAMS";
        let mut b = blank(st);
        wr64(&mut b, off(st, "vmmuSegmentSize"), m.vmmu_segment_mib << 20);
        return done(b, TABLE, format!("vmmu segment size: {} MiB", m.vmmu_segment_mib));
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_GPU_GET_SMC_MODE") {
        let mut b = blank("NV2080_CTRL_INTERNAL_GPU_GET_SMC_MODE_PARAMS");
        let smc = m.smc_mode(mode);
        wr32(&mut b, 0, smc);
        return done(b, MODE, format!("smc mode {smc} (current {}, pending {})", mode.current, mode.pending));
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_MIGMGR_SET_PARTITIONING_MODE") {
        let on = rd32(params, off("NV2080_CTRL_GPU_SET_PARTITIONING_MODE_PARAMS", "partitioningMode")) & 3 != 0;
        if !on && !f.mig.gis.is_empty() {
            return Some(reply(
                NV_ERR_IN_USE,
                params.to_vec(),
                MODE,
                format!("set partitioning mode off: {} GPU instance(s) exist", f.mig.gis.len()),
            ));
        }
        let rec = f.persist.mig.get_or_insert_with(Mode::default);
        rec.pending = on;
        if m.resetless {
            rec.current = on;
        }
        let rec = *rec;
        f.save();
        return Some(reply(
            NV_OK,
            params.to_vec(),
            MODE,
            format!(
                "set partitioning mode {}: current {}, pending {} ({})",
                on,
                rec.current,
                rec.pending,
                if m.resetless { "resetless" } else { "applies at the next GPU reset" }
            ),
        ));
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_MEMSYS_SET_PARTITIONABLE_MEM") {
        let st = "NV2080_CTRL_INTERNAL_MEMSYS_SET_PARTITIONABLE_MEM_PARAMS";
        let rd64 = |p: &str| {
            let o = off(st, p);
            params.get(o..o + 8).map_or(0, |s| u64::from_le_bytes(s.try_into().unwrap()))
        };
        let (sz, bottom) = (rd64("partitionableMemSize"), rd64("bottomRsvdSize"));
        let mut b = params.to_vec();
        b.resize(n.max(size(st)), 0);
        let (lo, hi) = (bottom, bottom + sz.max(1) - 1);
        wr64(&mut b, off(st, "partitionableStartAddr"), lo);
        wr64(&mut b, off(st, "partitionableEndAddr"), hi);
        f.mig.range = Some((lo, hi));
        return done(b, TABLE, format!("partitionable memory [{lo:#x}, {hi:#x}] as asked"));
    }
    for name in [
        "NV2080_CTRL_CMD_INTERNAL_KMEMSYS_GET_MIG_MEMORY_CONFIG",
        "NV2080_CTRL_CMD_INTERNAL_MEMSYS_GET_MIG_MEMORY_PARTITION_TABLE",
    ] {
        if is(cmd, name) {
            return done(vec![0u8; n], TABLE, format!("{name}: zeros (the GA100 AMAP configuration; no AMAP in a model)"));
        }
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_STATIC_KMIGMGR_GET_PARTITIONABLE_ENGINES") {
        let st = "NV2080_CTRL_INTERNAL_STATIC_MIGMGR_GET_PARTITIONABLE_ENGINES_PARAMS";
        let mut b = blank(st);
        let mut mask = [0u64; 4];
        for e in &m.engines {
            let t = engine(v::NV2080_ENGINE_TYPES, e);
            mask[(t / 64) as usize] |= 1 << (t % 64);
        }
        for (i, w) in mask.iter().enumerate() {
            wr64(&mut b, off(st, &format!("engineMask[{i}]")), *w);
        }
        return done(b, TABLE, format!("partitionable engines {} ({})", m.engines.join(" "), m.label));
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO") {
        let st = "NV2080_CTRL_INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO_PARAMS";
        let mut b = blank(st);
        let mut sizes: Vec<(u32, u32)> = m.profile.iter().map(|p| (compute_size(p.flag), p.gpcs)).collect();
        sizes.sort();
        sizes.dedup_by_key(|x| x.0);
        for (i, (cs, gpcs)) in sizes.iter().enumerate() {
            let e = |f: &str| off(st, &format!("skylineTable[{i}].{f}"));
            for j in 0..*gpcs {
                wr8(&mut b, off(st, &format!("skylineTable[{i}].skylineVgpcSize[{j}]")), m.tpcs_per_gpc as u8);
            }
            let max = m.profile.iter().find(|p| compute_size(p.flag) == *cs).map_or(0, |p| m.placements(p).len() as u32);
            wr32(&mut b, e("maxInstances"), max);
            wr32(&mut b, e("computeSizeFlag"), *cs);
            wr32(&mut b, e("numNonSingletonVgpcs"), *gpcs);
        }
        wr32(&mut b, off(st, "validEntries"), sizes.len() as u32);
        return done(b, TABLE, format!("skyline: {} entries", sizes.len()));
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_STATIC_KMIGMGR_GET_SWIZZ_ID_FB_MEM_PAGE_RANGES") {
        let st = "NV2080_CTRL_INTERNAL_STATIC_MIGMGR_GET_SWIZZ_ID_FB_MEM_PAGE_RANGES_PARAMS";
        let mut b = blank(st);
        let ranges = m.page_ranges(f.mig.range, f.fb_bytes);
        for (s, (lo, hi)) in ranges.iter().enumerate() {
            wr64(&mut b, off(st, &format!("fbMemPageRanges[{s}].lo")), *lo);
            wr64(&mut b, off(st, &format!("fbMemPageRanges[{s}].hi")), *hi);
        }
        return done(b, TABLE, format!("swizzId memory in {} MiB segments: 0 {:?}, 7 {:?}, 14 {:?}", m.vmmu_segment_mib, ranges[0], ranges[7], ranges[14]));
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_STATIC_KMIGMGR_GET_COMPUTE_PROFILES") {
        let st = "NV2080_CTRL_INTERNAL_STATIC_MIGMGR_GET_COMPUTE_PROFILES_PARAMS";
        let mut b = blank(st);
        let mut sizes: Vec<(u32, u32)> = m.profile.iter().map(|p| (compute_size(p.flag), p.gpcs)).collect();
        sizes.sort_by_key(|x| std::cmp::Reverse(x.0));
        sizes.dedup_by_key(|x| x.0);
        for (i, (cs, gpcs)) in sizes.iter().enumerate() {
            let e = |f: &str| off(st, &format!("profiles[{i}].{f}"));
            wr8(&mut b, e("computeSize"), *cs as u8);
            wr32(&mut b, e("gpcCount"), *gpcs);
            wr32(&mut b, e("veidCount"), gpcs * m.veids_per_gpc);
            wr32(&mut b, e("smCount"), gpcs * m.sms_per_gpc);
            wr32(&mut b, e("physicalSlots"), *gpcs);
        }
        wr32(&mut b, off(st, "profileCount"), sizes.len() as u32);
        return done(b, TABLE, format!("compute profiles: {} sizes", sizes.len()));
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_STATIC_KMIGMGR_GET_PROFILES") {
        let st = "NV2080_CTRL_INTERNAL_STATIC_MIGMGR_GET_PROFILES_PARAMS";
        let mut b = blank(st);
        for (i, p) in m.profile.iter().enumerate() {
            let e = |f: &str| off(st, &format!("table[{i}].{f}"));
            let cs = compute_size(p.flag);
            wr32(&mut b, e("partitionFlag"), p.flag);
            wr32(&mut b, e("grCount"), 1);
            wr32(&mut b, e("gpcCount"), p.gpcs);
            wr32(&mut b, e("virtualGpcCount"), p.gpcs);
            wr32(&mut b, e("veidCount"), p.gpcs * m.veids_per_gpc);
            wr32(&mut b, e("smCount"), p.gpcs * m.sms_per_gpc);
            wr32(&mut b, e("ceCount"), p.ce);
            wr32(&mut b, e("nvEncCount"), p.nvenc);
            wr32(&mut b, e("nvDecCount"), p.nvdec);
            wr32(&mut b, e("nvJpgCount"), p.nvjpg);
            wr32(&mut b, e("nvOfaCount"), p.ofa);
            let o = e("validCTSIdMask");
            b[o..o + 8].copy_from_slice(&cts_mask(cs, m.gpcs).to_le_bytes());
        }
        wr32(&mut b, off(st, "count"), m.profile.len() as u32);
        let names: Vec<&str> = m.profile.iter().map(|p| p.name.as_str()).collect();
        return done(b, TABLE, format!("gpu instance profiles: {} ({})", names.join(" "), m.label));
    }
    if is(cmd, "NV2080_CTRL_CMD_GPU_GET_PARTITION_CAPACITY") {
        let st = "NV2080_CTRL_GPU_GET_PARTITION_CAPACITY_PARAMS";
        let flag = rd32(params, off(st, "partitionFlag"));
        let Some(pi) = m.profile_by_flag(flag) else {
            return Some(reply(NV_ERR_INVALID_ARGUMENT, params.to_vec(), TABLE, format!("partition capacity: no profile has flag {flag:#x}")));
        };
        let p = &m.profile[pi];
        let mut b = params.to_vec();
        b.resize(n.max(size(st)), 0);
        let total = m.placements(p);
        let avail = m.available(p, &f.mig);
        let put = |b: &mut Vec<u8>, list: &[u32], arr: &str, count: &str| {
            for (i, s) in list.iter().enumerate().take(8) {
                let (lo, hi) = span(*s, m.span_len);
                wr64(b, off(st, &format!("{arr}[{i}].lo")), lo as u64);
                wr64(b, off(st, &format!("{arr}[{i}].hi")), hi as u64);
            }
            wr32(b, off(st, count), list.len() as u32);
        };
        put(&mut b, &total, "totalSpans", "totalSpansCount");
        wr32(&mut b, off(st, "totalPartitionCount"), total.len() as u32);
        put(&mut b, &avail, "availableSpans", "availableSpansCount");
        wr32(&mut b, off(st, "partitionCount"), avail.len() as u32);
        return done(b, INSTANCES, format!("partition capacity {}: {} of {} (swizzIds {avail:?} of {total:?})", p.name, avail.len(), total.len()));
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_MIGMGR_SET_GPU_INSTANCES") {
        return Some(set_gpu_instances(f, m, params));
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_MIGMGR_EXPORT_GPU_INSTANCE") {
        let st = "NV2080_CTRL_INTERNAL_KMIGMGR_IMPORT_EXPORT_GPU_INSTANCE_PARAMS";
        let swizz = rd32(params, off(st, "swizzId"));
        let Some(g) = f.mig.gis.get(&swizz) else {
            return Some(reply(NV_ERR_INVALID_ARGUMENT, params.to_vec(), INSTANCES, format!("export gpu instance: no GI at swizzId {swizz}")));
        };
        let mut b = params.to_vec();
        b.resize(n.max(size(st)), 0);
        let o = off(st, "uuid[0]");
        b[o..o + 16].copy_from_slice(&g.uuid);
        let mut mask = [0u64; 4];
        for &e in &g.engines {
            mask[(e / 64) as usize] |= 1 << (e % 64);
        }
        for (i, w) in mask.iter().enumerate() {
            wr64(&mut b, off(st, &format!("info.enginesMask[{i}]")), *w);
        }
        wr32(&mut b, off(st, "info.partitionFlags"), g.flag);
        wr32(&mut b, off(st, "info.gpcMask"), g.gpcs.iter().fold(0, |a, x| a | 1 << x));
        wr32(&mut b, off(st, "info.virtualGpcCount"), g.gpcs.len() as u32);
        wr32(&mut b, off(st, "info.veidOffset"), g.gpcs.first().copied().unwrap_or(0) * m.veids_per_gpc);
        wr32(&mut b, off(st, "info.veidCount"), g.gpcs.len() as u32 * m.veids_per_gpc);
        return done(b, INSTANCES, format!("export gpu instance {swizz}: GPCs {:?}, engines {:?}", g.gpcs, g.engines));
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_MIGMGR_GET_GPU_INSTANCES") {
        let st = "NV2080_CTRL_GPU_GET_PARTITIONS_PARAMS";
        let all = params.get(off(st, "bGetAllPartitionInfo")).is_some_and(|&x| x != 0);
        let mine: Vec<u32> = if all {
            f.mig.gis.keys().copied().collect()
        } else {
            f.mig
                .subs
                .iter()
                .filter(|((c, _), _)| *c == object.0)
                .filter_map(|(_, s)| match s {
                    Sub::Gi(w) | Sub::Ci(w, _) => f.mig.gis.contains_key(w).then_some(*w),
                })
                .take(1)
                .collect()
        };
        let mut b = params.to_vec();
        b.resize(n.max(size(st)), 0);
        for (i, s) in mine.iter().enumerate() {
            let g = &f.mig.gis[s];
            let e = |x: &str| off(st, &format!("queryPartitionInfo[{i}].{x}"));
            wr32(&mut b, e("swizzId"), *s);
            wr8(&mut b, e("bValid"), 1);
            wr8(&mut b, e("bPartitionError"), 0);
            let (lo, hi) = span(*s, m.span_len);
            wr64(&mut b, e("span.lo"), lo as u64);
            wr64(&mut b, e("span.hi"), hi as u64);
            wr32(&mut b, e("gpcsPerGr[0]"), g.gpcs.len() as u32);
            wr32(&mut b, e("virtualGpcsPerGr[0]"), g.gpcs.len() as u32);
            wr32(&mut b, e("veidsPerGr[0]"), g.gpcs.len() as u32 * m.veids_per_gpc);
        }
        wr32(&mut b, off(st, "validPartitionCount"), mine.len() as u32);
        return done(b, INSTANCES, format!("get gpu instances (all {all}): swizzIds {mine:?}"));
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_FIFO_PROMOTE_RUNLIST_BUFFERS") {
        return done(params.to_vec(), INSTANCES, "promote runlist buffers: recorded (the runlists are the channel emulation's)".into());
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_GPU_CHECK_CTS_ID_VALID") {
        let mut b = params.to_vec();
        b.resize(n.max(12), 0);
        let cts = rd32(params, 0);
        let ok = m.profile.iter().any(|p| cts_mask(compute_size(p.flag), m.gpcs) >> cts & 1 == 1);
        wr8(&mut b, 8, ok as u8);
        return done(b, INSTANCES, format!("check cts id {cts}: valid {ok}"));
    }
    if is(cmd, "NV2080_CTRL_CMD_INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_MIG_GPU_INSTANCE") {
        // kceGetMappingsForMIGGpuInstance_GH100 (kernel_ce_gh100.c:1340-1358 of 615.71.09)
        // asks once for the even and once for the odd LCEs of a GI, and the KMM print path
        // (kernel_mig_manager.c:3344) with lceMask 0 for the mapped count. The model has
        // one PCE per LCE (PCE i serves LCE i): pceAvailableMask = lceMask, one PCE per
        // LCE, as many LCEs to map as asked; the kernel then maps 1-1
        // (kernel_ce_gh100.c:1398-1411). No crossbar is programmed.
        let st = "NV2080_CTRL_CMD_INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_MIG_GPU_INSTANCE_PARAMS";
        let mut b = params.to_vec();
        b.resize(n.max(size(st)), 0);
        let lce_mask = rd32(params, off(st, "lceMask"));
        let num = rd32(params, off(st, "numLces"));
        wr32(&mut b, off(st, "pceAvailableMask"), lce_mask);
        wr32(&mut b, off(st, "numLcesToMap"), num);
        wr32(&mut b, off(st, "lceAvailableMask"), lce_mask);
        wr32(&mut b, off(st, "numMinPcesPerLce"), 1);
        return done(b, INSTANCES, format!("pce config for GI LCEs {lce_mask:#x}: PCE i serves LCE i, {num} to map"));
    }
    if is(cmd, "NV2080_CTRL_CE_UPDATE_PCE_LCE_MIG_MAPPINGS") {
        // kceApplyMIGMappings (kernel_ce.c:992-1037 of 615.71.09): a failure fails the GI
        // (kmigmgr_gh100.c:163-168, NV_ERR_INVALID_STATE). Recorded, nothing programmed:
        // the copy engines the guest sees are the channel emulation's.
        let st = "NV2080_CTRL_CE_UPDATE_PCE_LCE_MIG_MAPPINGS_PARAMS";
        let mask = rd32(params, off(st, "lceAvailableMask"));
        return done(params.to_vec(), INSTANCES, format!("pce/lce mig mappings for LCEs {mask:#x}: recorded, nothing programmed"));
    }
    for name in [
        "NV2080_CTRL_CMD_INTERNAL_KMIGMGR_PROMOTE_GPU_INSTANCE_MEM_RANGE",
        "NV2080_CTRL_CMD_INTERNAL_MIGMGR_IMPORT_GPU_INSTANCE",
        "NVC637_CTRL_CMD_EXEC_PARTITIONS_IMPORT",
    ] {
        if is(cmd, name) {
            return Some(reply(NV_ERR_NOT_SUPPORTED, params.to_vec(), REFUSED, format!("{name}: refused")));
        }
    }
    if is(cmd, "NVC637_CTRL_CMD_EXEC_PARTITIONS_CREATE") {
        return Some(ci_create(f, m, object, params));
    }
    if is(cmd, "NVC637_CTRL_CMD_EXEC_PARTITIONS_DELETE") {
        let st = "NVC637_CTRL_EXEC_PARTITIONS_DELETE_PARAMS";
        let Some(Sub::Gi(swizz)) = f.mig.subs.get(&object).copied() else {
            return Some(reply(NV_ERR_INVALID_ARGUMENT, params.to_vec(), INSTANCES, "exec partitions delete: not a GI subscription".into()));
        };
        let count = rd32(params, off(st, "execPartCount")).min(MAX_CI);
        let g = f.mig.gis.get_mut(&swizz)?;
        let ids: Vec<u32> = (0..count).map(|i| rd32(params, off(st, &format!("execPartId[{i}]")))).collect();
        for i in &ids {
            g.cis.remove(i);
        }
        return done(params.to_vec(), INSTANCES, format!("exec partitions delete in GI {swizz}: {ids:?}"));
    }
    if is(cmd, "NVC637_CTRL_CMD_EXEC_PARTITIONS_EXPORT") {
        let st = "NVC637_CTRL_EXEC_PARTITIONS_IMPORT_EXPORT_PARAMS";
        let Some(Sub::Gi(swizz)) = f.mig.subs.get(&object).copied() else {
            return Some(reply(NV_ERR_INVALID_ARGUMENT, params.to_vec(), INSTANCES, "exec partitions export: not a GI subscription".into()));
        };
        let ci = rd32(params, off(st, "id"));
        let Some((g, c)) = f.mig.gis.get(&swizz).and_then(|g| g.cis.get(&ci).map(|c| (g, c))) else {
            return Some(reply(NV_ERR_INVALID_ARGUMENT, params.to_vec(), INSTANCES, format!("exec partitions export: no CI {ci} in GI {swizz}")));
        };
        let mut b = params.to_vec();
        b.resize(n.max(size(st)), 0);
        // CI engines in GI-local RM_ENGINE_TYPE numbering (KMM:5401, 5416-5418): the GI's
        // GR is local GR0, its copy engines local COPY0.. in order.
        let mut mask = [0u64; 4];
        let gr0 = engine(v::RM_ENGINE_TYPES, "GR0");
        let ce0 = engine(v::RM_ENGINE_TYPES, "COPY0");
        mask[0] |= 1 << gr0;
        let ces = g.engines.iter().filter(|&&e| (ce0..ce0 + 20).contains(&e)).count() as u32;
        for k in 0..ces {
            mask[0] |= 1 << (ce0 + k);
        }
        for (i, w) in mask.iter().enumerate() {
            wr64(&mut b, off(st, &format!("info.enginesMask[{i}]")), *w);
        }
        let o = off(st, "info.uuid[0]");
        b[o..o + 16].copy_from_slice(&uuid(swizz, Some(ci)));
        wr32(&mut b, off(st, "info.gpcMask"), c.gpcs.iter().fold(0, |a, x| a | 1 << x));
        wr32(&mut b, off(st, "info.veidOffset"), c.gpcs.first().copied().unwrap_or(0) * m.veids_per_gpc);
        wr32(&mut b, off(st, "info.veidCount"), c.gpcs.len() as u32 * m.veids_per_gpc);
        wr32(&mut b, off(st, "info.smCount"), c.gpcs.len() as u32 * m.sms_per_gpc);
        wr32(&mut b, off(st, "info.spanStart"), c.span_start);
        wr32(&mut b, off(st, "info.computeSize"), c.compute_size);
        wr32(&mut b, off(st, "info.execPartFlags"), c.flags);
        return done(b, INSTANCES, format!("exec partitions export GI {swizz} CI {ci}: local GPCs {:?}", c.gpcs));
    }
    None
}

fn set_gpu_instances(f: &mut Families, m: &MigData, params: &[u8]) -> Answer {
    let st = "NV2080_CTRL_GPU_SET_PARTITIONS_PARAMS";
    let n = params.len();
    let mut b = params.to_vec();
    b.resize(n.max(size(st)), 0);
    let count = rd32(params, off(st, "partitionCount")).min(8);
    let mut notes = Vec::new();
    for i in 0..count {
        let e = |x: &str| off(st, &format!("partitionInfo[{i}].{x}"));
        let flag = rd32(&b, e("partitionFlag"));
        let valid = b[e("bValid")] != 0;
        if !valid {
            let swizz = rd32(&b, e("swizzId"));
            match f.mig.gis.get(&swizz) {
                Some(g) if !g.cis.is_empty() => {
                    return reply(NV_ERR_IN_USE, params.to_vec(), INSTANCES, format!("destroy GI {swizz}: {} compute instance(s) left", g.cis.len()))
                }
                Some(_) => {
                    f.mig.gis.remove(&swizz);
                    notes.push(format!("GI {swizz} destroyed"));
                }
                None => return reply(NV_ERR_INVALID_ARGUMENT, params.to_vec(), INSTANCES, format!("destroy GI {swizz}: none")),
            }
            continue;
        }
        if !f.persist.mig.is_some_and(|x| x.current) {
            return reply(NV_ERR_INVALID_STATE, params.to_vec(), INSTANCES, "create GI: MIG mode is off".into());
        }
        let Some(pi) = m.profile_by_flag(flag) else {
            return reply(NV_ERR_INVALID_ARGUMENT, params.to_vec(), INSTANCES, format!("create GI: no profile has flag {flag:#x}"));
        };
        let p = &m.profile[pi];
        let avail = m.available(p, &f.mig);
        let pick = if flag & PLACE_AT_SPAN != 0 {
            let lo = u64::from_le_bytes(b[e("placement.lo")..e("placement.lo") + 8].try_into().unwrap());
            avail.iter().copied().find(|&s| span(s, m.span_len).0 as u64 == lo)
        } else {
            avail.first().copied()
        };
        let Some(swizz) = pick else {
            let why = if m.take_engines(p, &m.free_engines(&f.mig)).is_none() {
                format!("the engines a {} needs are taken (the rung has {})", p.name, m.engines.join(" "))
            } else {
                "no free placement".to_string()
            };
            return reply(NV_ERR_INSUFFICIENT_RESOURCES, params.to_vec(), INSTANCES, format!("create GI {}: {why}", p.name));
        };
        let engines = m.take_engines(p, &m.free_engines(&f.mig)).expect("checked by available");
        let gpcs: Vec<u32> = m.span_gpcs(swizz).into_iter().take(p.gpcs as usize).collect();
        let u = uuid(swizz, None);
        wr32(&mut b, e("swizzId"), swizz);
        let o = e("uuid[0]");
        b[o..o + 16].copy_from_slice(&u);
        notes.push(format!("GI {} at swizzId {swizz}, GPCs {gpcs:?}", p.name));
        f.mig.gis.insert(
            swizz,
            Gi {
                swizz,
                flag: flag & !PLACE_AT_SPAN,
                profile: pi,
                gpcs,
                engines,
                uuid: u,
                cis: BTreeMap::new(),
            },
        );
    }
    b.truncate(n);
    reply(NV_OK, b, INSTANCES, format!("set gpu instances: {}", notes.join("; ")))
}

fn ci_create(f: &mut Families, m: &MigData, object: (u32, u32), params: &[u8]) -> Answer {
    let st = "NVC637_CTRL_EXEC_PARTITIONS_CREATE_PARAMS";
    let n = params.len();
    let Some(Sub::Gi(swizz)) = f.mig.subs.get(&object).copied() else {
        return reply(NV_ERR_INVALID_ARGUMENT, params.to_vec(), INSTANCES, "exec partitions create: not a GI subscription".into());
    };
    let Some(g) = f.mig.gis.get_mut(&swizz) else {
        return reply(NV_ERR_INVALID_STATE, params.to_vec(), INSTANCES, format!("exec partitions create: GI {swizz} gone"));
    };
    let mut b = params.to_vec();
    b.resize(n.max(size(st)), 0);
    if b[off(st, "bQuery")] != 0 {
        b.truncate(n);
        return reply(NV_OK, b, INSTANCES, "exec partitions create: query".into());
    }
    let flags = rd32(&b, off(st, "flags"));
    let count = rd32(&b, off(st, "execPartCount")).min(MAX_CI);
    let mut notes = Vec::new();
    for i in 0..count {
        let e = |x: &str| off(st, &format!("execPartInfo[{i}].{x}"));
        let cs = rd32(&b, e("computeSize"));
        let want = m
            .profile
            .iter()
            .find(|p| compute_size(p.flag) == cs)
            .map_or(rd32(&b, e("gpcCount")), |p| p.gpcs);
        let used: Vec<u32> = g.cis.values().flat_map(|c| c.gpcs.clone()).collect();
        let local: Vec<u32> = (0..g.gpcs.len() as u32).collect();
        let start = if flags & CI_AT_SPAN != 0 { rd32(&b, e("spanStart")) } else { 0 };
        let free: Vec<u32> = local
            .iter()
            .copied()
            .filter(|x| *x >= start && !used.contains(x))
            .take(want as usize)
            .collect();
        if (free.len() as u32) < want || want == 0 {
            return reply(NV_ERR_INSUFFICIENT_RESOURCES, params.to_vec(), INSTANCES, format!("exec partitions create in GI {swizz}: {want} GPCs asked, {} free", local.len() - used.len()));
        }
        let id = if flags & CI_WITH_PART_ID != 0 {
            rd32(&b, off(st, &format!("execPartId[{i}]")))
        } else {
            (0..MAX_CI).find(|x| !g.cis.contains_key(x)).unwrap_or(MAX_CI)
        };
        if id >= MAX_CI || g.cis.contains_key(&id) {
            return reply(NV_ERR_INSUFFICIENT_RESOURCES, params.to_vec(), INSTANCES, format!("exec partitions create in GI {swizz}: id {id} taken"));
        }
        let span_start = free[0];
        wr32(&mut b, off(st, &format!("execPartId[{i}]")), id);
        wr32(&mut b, e("gpcCount"), want);
        wr32(&mut b, e("veidCount"), want * m.veids_per_gpc);
        wr32(&mut b, e("smCount"), want * m.sms_per_gpc);
        wr32(&mut b, e("spanStart"), span_start);
        wr32(&mut b, e("veidStartOffset"), span_start * m.veids_per_gpc);
        notes.push(format!("CI {id} ({want} GPCs, local {free:?})"));
        g.cis.insert(
            id,
            Ci {
                id,
                gpcs: free,
                compute_size: cs,
                span_start,
                flags: rd32(&b, e("execPartFlags")),
            },
        );
    }
    b.truncate(n);
    reply(NV_OK, b, INSTANCES, format!("exec partitions create in GI {swizz}: {}", notes.join("; ")))
}

/// The allocation of a subscription object (0xC637 under a subdevice, 0xC638 under a
/// 0xC637), recorded as what it subscribes to.
pub fn alloc(
    f: &mut Families,
    object: (u32, u32),
    h_parent: u32,
    class: u32,
    params: &[u8],
) -> Option<Answer> {
    if class != AMPERE_SMC_PARTITION_REF && class != AMPERE_SMC_EXEC_PARTITION_REF {
        return None;
    }
    let chip = f.chip;
    if !super::modelled(chip, "mig") {
        return None;
    }
    let idv = rd32(params, ALLOC_ID);
    let fail = |why: String| Some(reply(NV_ERR_INVALID_ARGUMENT, params.to_vec(), INSTANCES, why));
    let sub = if class == AMPERE_SMC_PARTITION_REF {
        if idv != SWIZZ_DEVICE_PROFILING && !f.mig.gis.contains_key(&idv) {
            return fail(format!("alloc 0xc637 swizzId {idv}: no such GPU instance"));
        }
        Sub::Gi(idv)
    } else {
        let Some(Sub::Gi(swizz)) = f.mig.subs.get(&(object.0, h_parent)).copied() else {
            return fail(format!("alloc 0xc638 under {h_parent:#x}: parent is no GI subscription"));
        };
        if !f.mig.gis.get(&swizz).is_some_and(|g| g.cis.contains_key(&idv)) {
            return fail(format!("alloc 0xc638 CI {idv} in GI {swizz}: no such compute instance"));
        }
        Sub::Ci(swizz, idv)
    };
    f.mig.subs.insert(object, sub);
    Some(reply(
        NV_OK,
        params.to_vec(),
        INSTANCES,
        format!("alloc {class:#x} object {:#x}: subscription {sub:?}", object.1),
    ))
}

/// `DUP_OBJECT` of a subscription: the duplicate subscribes to the same instance.
pub fn dup(f: &mut Families, dst: (u32, u32), src: (u32, u32)) {
    if let Some(s) = f.mig.subs.get(&src).copied() {
        f.mig.subs.insert(dst, s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrl::{Dispatcher, INTERNAL_CLIENT, INTERNAL_SUBDEVICE};

    fn dispatcher() -> Dispatcher {
        let f = crate::fixture::rtx2070();
        Dispatcher::new(crate::chip::by_name("gh100").unwrap().data(), &f.identity(4 << 30), None)
    }

    fn ctl(d: &mut Dispatcher, obj: (u32, u32), name: &str, p: &[u8]) -> Answer {
        d.control(obj.0, obj.1, id(name).unwrap(), p)
    }

    const SUB: (u32, u32) = (INTERNAL_CLIENT, INTERNAL_SUBDEVICE);

    fn capacity(d: &mut Dispatcher, flag: u32) -> (u32, u32) {
        let st = "NV2080_CTRL_GPU_GET_PARTITION_CAPACITY_PARAMS";
        let mut p = vec![0u8; size(st)];
        wr32(&mut p, 0, flag);
        let a = ctl(d, SUB, "NV2080_CTRL_CMD_GPU_GET_PARTITION_CAPACITY", &p);
        assert_eq!(a.status, NV_OK, "{}", a.note);
        (rd32(&a.params, off(st, "partitionCount")), rd32(&a.params, off(st, "totalPartitionCount")))
    }

    fn set_gi(d: &mut Dispatcher, flag: u32, valid: bool, swizz: u32) -> Answer {
        let st = "NV2080_CTRL_GPU_SET_PARTITIONS_PARAMS";
        let mut p = vec![0u8; size(st)];
        wr32(&mut p, off(st, "partitionCount"), 1);
        wr32(&mut p, off(st, "partitionInfo[0].partitionFlag"), flag);
        wr32(&mut p, off(st, "partitionInfo[0].swizzId"), swizz);
        p[off(st, "partitionInfo[0].bValid")] = valid as u8;
        ctl(d, SUB, "NV2080_CTRL_CMD_INTERNAL_MIGMGR_SET_GPU_INSTANCES", &p)
    }

    fn mig_on(d: &mut Dispatcher, on: u32) -> Answer {
        ctl(d, SUB, "NV2080_CTRL_CMD_INTERNAL_MIGMGR_SET_PARTITIONING_MODE", &on.to_le_bytes())
    }

    fn smc(d: &mut Dispatcher) -> u32 {
        rd32(&ctl(d, SUB, "NV2080_CTRL_CMD_INTERNAL_GPU_GET_SMC_MODE", &[0u8; 4]).params, 0)
    }

    /// The GH100 table: 7g once, 4g once, 3g twice, 2g three times, 1g seven times (the
    /// swizzId geometry with seven GPCs); the rung's one GR engine lets one exist at a time.
    #[test]
    fn gh100_profile_geometry_and_capacity() {
        let mut d = dispatcher();
        for (flag, total) in [(0x00, 1), (0x05, 1), (0x09, 2), (0x0E, 3), (0x17, 7)] {
            assert_eq!(capacity(&mut d, flag), (total, total), "flag {flag:#x}");
        }
        // The static tables load (the fatal ones of kmigmgrLoadStaticInfo_KERNEL).
        for name in [
            "NV2080_CTRL_CMD_INTERNAL_STATIC_KMIGMGR_GET_PARTITIONABLE_ENGINES",
            "NV2080_CTRL_CMD_INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO",
            "NV2080_CTRL_CMD_INTERNAL_STATIC_KMIGMGR_GET_SWIZZ_ID_FB_MEM_PAGE_RANGES",
            "NV2080_CTRL_CMD_INTERNAL_STATIC_KMIGMGR_GET_COMPUTE_PROFILES",
            "NV2080_CTRL_CMD_INTERNAL_STATIC_KMIGMGR_GET_PROFILES",
            "NV2080_CTRL_CMD_GPU_GET_VMMU_SEGMENT_SIZE",
        ] {
            let a = ctl(&mut d, SUB, name, &vec![0u8; 8192]);
            assert_eq!(a.status, NV_OK, "{name}");
        }
        // swizzId 14 is rejected (an empty range), 7 is an eighth of the FB's segments.
        let st = "NV2080_CTRL_INTERNAL_STATIC_MIGMGR_GET_SWIZZ_ID_FB_MEM_PAGE_RANGES_PARAMS";
        let a = ctl(&mut d, SUB, "NV2080_CTRL_CMD_INTERNAL_STATIC_KMIGMGR_GET_SWIZZ_ID_FB_MEM_PAGE_RANGES", &vec![0u8; size(st)]);
        let r = |s: usize, f: &str| u64::from_le_bytes(a.params[off(st, &format!("fbMemPageRanges[{s}].{f}"))..][..8].try_into().unwrap());
        assert!(r(14, "lo") > r(14, "hi"));
        assert_eq!(r(0, "hi") - r(0, "lo") + 1, 8 * (r(7, "hi") - r(7, "lo") + 1));
    }

    /// `-mig 1` applies at once on GH100 (resetless); a GI and a CI come and go; the second
    /// GI finds no GR engine; MIG cannot be turned off while a GI exists.
    #[test]
    fn instances_lifecycle() {
        let mut d = dispatcher();
        assert_eq!(smc(&mut d), SMC_DISABLED);
        assert_eq!(set_gi(&mut d, 0x17, true, 0).status, NV_ERR_INVALID_STATE);
        assert_eq!(mig_on(&mut d, 2).status, NV_OK);
        assert_eq!(smc(&mut d), SMC_ENABLED);
        let a = set_gi(&mut d, 0x17, true, 0);
        assert_eq!(a.status, NV_OK, "{}", a.note);
        let swizz = rd32(&a.params, off("NV2080_CTRL_GPU_SET_PARTITIONS_PARAMS", "partitionInfo[0].swizzId"));
        assert_eq!(swizz, 7);
        // The GI's copy-engine mapping (kmigmgrApplyMIGGpuInstanceCeMapping_GH100): the
        // PCE config answers one PCE per LCE, the mapping is accepted.
        let pc = "NV2080_CTRL_CMD_INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_MIG_GPU_INSTANCE_PARAMS";
        let mut p = vec![0u8; size(pc)];
        wr32(&mut p, off(pc, "lceMask"), 0x14);
        wr32(&mut p, off(pc, "numLces"), 2);
        let a = ctl(&mut d, SUB, "NV2080_CTRL_CMD_INTERNAL_CE_GET_PCE_CONFIG_FOR_LCE_MIG_GPU_INSTANCE", &p);
        assert_eq!(a.status, NV_OK);
        assert_eq!(rd32(&a.params, off(pc, "pceAvailableMask")), 0x14);
        assert_eq!(rd32(&a.params, off(pc, "numLcesToMap")), 2);
        assert_eq!(rd32(&a.params, off(pc, "numMinPcesPerLce")), 1);
        let mm = "NV2080_CTRL_CE_UPDATE_PCE_LCE_MIG_MAPPINGS_PARAMS";
        let a = ctl(&mut d, SUB, "NV2080_CTRL_CE_UPDATE_PCE_LCE_MIG_MAPPINGS", &vec![0u8; size(mm)]);
        assert_eq!(a.status, NV_OK);
        assert_eq!(capacity(&mut d, 0x17), (0, 7));
        assert_eq!(set_gi(&mut d, 0x17, true, 0).status, NV_ERR_INSUFFICIENT_RESOURCES);
        assert_eq!(mig_on(&mut d, 0).status, NV_ERR_IN_USE);
        // Subscribe (0xC637) and create a compute instance of the eighth size.
        let gi_obj = (0xC1D0_0001, 0x5000_0001);
        let mut ap = vec![0u8; 16];
        wr32(&mut ap, ALLOC_ID, swizz);
        let a = d.alloc(gi_obj.0, gi_obj.1, INTERNAL_SUBDEVICE, AMPERE_SMC_PARTITION_REF, &ap);
        assert_eq!(a.status, NV_OK, "{}", a.note);
        let st = "NVC637_CTRL_EXEC_PARTITIONS_CREATE_PARAMS";
        let mut p = vec![0u8; size(st)];
        wr32(&mut p, off(st, "execPartCount"), 1);
        wr32(&mut p, off(st, "execPartInfo[0].computeSize"), 5);
        let a = ctl(&mut d, gi_obj, "NVC637_CTRL_CMD_EXEC_PARTITIONS_CREATE", &p);
        assert_eq!(a.status, NV_OK, "{}", a.note);
        assert_eq!(rd32(&a.params, off(st, "execPartId[0]")), 0);
        let ex = "NVC637_CTRL_EXEC_PARTITIONS_IMPORT_EXPORT_PARAMS";
        let a = ctl(&mut d, gi_obj, "NVC637_CTRL_CMD_EXEC_PARTITIONS_EXPORT", &vec![0u8; size(ex)]);
        assert_eq!(a.status, NV_OK, "{}", a.note);
        assert_eq!(rd32(&a.params, off(ex, "info.gpcMask")), 1);
        // The GI cannot go while its CI exists; delete the CI, then the GI.
        assert_eq!(set_gi(&mut d, 0, false, swizz).status, NV_ERR_IN_USE);
        let dl = "NVC637_CTRL_EXEC_PARTITIONS_DELETE_PARAMS";
        let mut p = vec![0u8; size(dl)];
        wr32(&mut p, 0, 1);
        assert_eq!(ctl(&mut d, gi_obj, "NVC637_CTRL_CMD_EXEC_PARTITIONS_DELETE", &p).status, NV_OK);
        assert_eq!(set_gi(&mut d, 0, false, swizz).status, NV_OK);
        assert_eq!(mig_on(&mut d, 0).status, NV_OK);
        assert_eq!(smc(&mut d), SMC_DISABLED);
    }

    /// A chip without resetless MIG (GA100) keeps the new mode pending until a reset.
    #[test]
    fn pending_until_reset_without_resetless() {
        let mut d = dispatcher();
        d.families.persist.mig = Some(Mode::default());
        let mut m = d.families.chip.mig.clone().unwrap();
        m.resetless = false;
        assert_eq!(m.smc_mode(Mode { current: false, pending: true }), SMC_ENABLE_PENDING);
        assert_eq!(m.smc_mode(Mode { current: true, pending: false }), SMC_DISABLE_PENDING);
    }

    /// TU106 has no MIG table: the controls keep their old answers.
    #[test]
    fn tu106_is_untouched() {
        let f = crate::fixture::rtx2070();
        let mut d = Dispatcher::new(crate::chip::by_name("tu106").unwrap().data(), &f.identity(4 << 30), None);
        let a = ctl(&mut d, SUB, "NV2080_CTRL_CMD_INTERNAL_MIGMGR_SET_GPU_INSTANCES", &[0u8; 64]);
        assert_eq!(a.label, crate::chipctl::REFUSED);
    }
}
