// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Q-B1 of docs/caraxes/gsp-plan/b-host-forwardable.md §7: which of the 30 forward
//! candidates (groups F1 and F2) the host card answers, and with what. Read-only: each
//! control is a getter, issued once with a zeroed buffer of the struct's size on the
//! subdevice of the device's own unprivileged host RM client (`hostrm::HostRmClient`, the
//! object every forward uses); no setter, no other object, no privilege. Every control
//! queried is printed with its status, so the operator sees every host read.
//!
//! Output, one line per control: id, name, size, NV status, then the answer: the bytes
//! in hex where the struct is small, else the number of non-zero bytes and, for the
//! address and process rows, the fields the device's filters look at (process names and
//! PIDs are summarized, never printed).
//!
//! Run: cargo run --release -p vfio-user-nvgpu --example qb1-sweep

use vfio_user_nvgpu::ctrl::HostRm;
use vfio_user_nvgpu::hostrm::HostRmClient;
use vfio_user_nvgpu::ver::active::NV20_SUBDEVICE_0;

/// (id, name, sizeof) of the 26 F1 and 4 F2 rows (plan §2; the same in 610.57.04 and
/// 615.71.09).
const CANDIDATES: &[(u32, &str, usize)] = &[
    (0x2080_0133, "GPU_QUERY_ECC_CONFIGURATION", 8),
    (0x2080_0153, "GPU_QUERY_ILLUM_SUPPORT", 8),
    (0x2080_0154, "GPU_GET_ILLUM", 8),
    (0x2080_015F, "GPU_QUERY_SCRUBBER_STATUS", 24),
    (0x2080_0173, "GPU_QUERY_FUNCTION_STATUS", 12),
    (0x2080_01AB, "GPU_GET_RESET_STATUS", 1),
    (0x2080_01AE, "GPU_GET_DRAIN_AND_RESET_STATUS", 1),
    (0x2080_0809, "BIOS_GET_POST_TIME", 8),
    (0x2080_1037, "CLK_PMUMON_CLK_DOMAINS_GET_SAMPLES", 6024),
    (0x2080_1120, "FIFO_OBJSCHED_GET_STATE", 20),
    (0x2080_130C, "FB_GET_CALIBRATION_LOCK_FAILED", 24),
    (0x2080_1328, "FB_GET_LTC_INFO_FOR_FBP", 20),
    (0x2080_1347, "FB_GET_ROW_REMAPPER_HISTOGRAM", 20),
    (0x2080_1348, "FB_GET_DYNAMIC_OFFLINED_PAGES", 1040),
    (0x2080_1355, "FB_QUERY_DRAM_ENCRYPTION_PENDING_CONFIGURATION", 4),
    (0x2080_1358, "FB_QUERY_DRAM_ENCRYPTION_STATUS", 4),
    (0x2080_1816, "BUS_GET_PEX_LANE_COUNTERS", 18),
    (0x2080_1817, "BUS_GET_PCIE_LTR_LATENCY", 12),
    (0x2080_200B, "PERF_GET_LEVEL_INFO_V2", 780),
    (0x2080_205A, "PERF_GET_POWERSTATE", 4),
    (0x2080_206E, "PERF_RATED_TDP_GET_CONTROL", 12),
    (0x2080_2096, "PERF_GET_GPUMON_PERFMON_UTIL_SAMPLES_V2", 55888),
    (0x2080_3101, "FLCN_GET_DMEM_USAGE", 12),
    (0x2080_3400, "ECC_GET_CLIENT_EXPOSED_COUNTERS", 96),
    (0x2080_3401, "ECC_GET_VOLATILE_COUNTS", 40),
    (0x2080_3404, "ECC_GET_REPAIR_STATUS", 2),
    (0x2080_3406, "ECC_GET_UNREPAIRABLE_MEMORY_FLAG", 1),
    (0x2080_3602, "GSP_GET_RM_HEAP_STATS", 88),
    (0x2080_3604, "GSP_GET_LIBOS_HEAP_STATS", 1040),
    (0x2080_81F3, "GPU_IS_RESET_COUPLED", 1),
];

fn u32at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn u64at(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

fn describe(id: u32, b: &[u8]) -> String {
    let nz = b.iter().filter(|&&x| x != 0).count();
    match id {
        0x2080_015F => format!(
            "scrubberStatus {} remainingTimeMs {} scrubStartAddr {:#x} scrubEndAddr {:#x}",
            u32at(b, 0),
            u32at(b, 4),
            u64at(b, 8),
            u64at(b, 16)
        ),
        0x2080_1348 => {
            let n = u32at(b, 1024) as usize;
            let pages: Vec<String> = (0..n.min(64))
                .map(|i| format!("{:#x}", u64at(b, 16 * i) << 12))
                .collect();
            format!(
                "validEntries {n} baseIndex {} bMore {} pages [{}]",
                u32at(b, 1028),
                b[1032],
                pages.join(" ")
            )
        }
        0x2080_2096 => {
            let (mut procs, mut names) = (0, 0);
            for i in 0..72 {
                for off in [8, 136, 264, 392, 520, 648] {
                    let e = 16 + 776 * i + off;
                    procs += (u32at(b, e + 8) != 0 && u32at(b, e + 8) != u32::MAX) as u32;
                    names += (b[e + 16] != 0) as u32;
                }
            }
            format!(
                "type {} bufSize {} count {} tracker {}; engine samples with a procId {procs}, with a process name {names} (not printed); {nz} non-zero bytes",
                b[0],
                u32at(b, 4),
                u32at(b, 8),
                u32at(b, 12)
            )
        }
        _ if b.len() <= 128 => b.iter().map(|x| format!("{x:02x}")).collect(),
        _ => format!("{nz} non-zero bytes of {}", b.len()),
    }
}

fn main() -> anyhow::Result<()> {
    let sel = vfio_user_nvgpu::hostabi::select().map_err(anyhow::Error::msg)?;
    println!("# Q-B1 sweep: {} (chosen by {})", sel.layouts.describe(), sel.how);
    println!("# id\tname\tsize\tstatus\tanswer");
    let mut host = HostRmClient::open()?;
    for &(id, name, size) in CANDIDATES {
        let mut b = vec![0u8; size];
        let st = host.control(NV20_SUBDEVICE_0, id, &mut b);
        println!("{id:#010x}\t{name}\t{size}\t{st:#x}\t{}", describe(id, &b));
    }
    Ok(())
}
