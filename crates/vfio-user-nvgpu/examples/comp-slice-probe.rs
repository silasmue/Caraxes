// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The D16b host probe (compslice.rs, docs/caraxes/README.md D16b): what the host RM grants
//! a compressed vidmem object and which comptag lines it writes into the PTEs of mappings
//! at different offsets, without a VM. It runs through the device's own unprivileged host
//! RM client (hostmem.rs `HostMemory`, no privilege window) and makes only these host calls:
//!
//! 1. `NV01_MEMORY_LOCAL_USER` objects with the reservation's attributes (contiguous, 2 MiB
//!    pages), floating: 4 MiB with `COMPR_DISABLE_PLC_ANY` (the slice's) and 2 MiB with
//!    `COMPR_REQUIRED` (for comparison); the grant read back with `GET_SURFACE_INFO`,
//!    `GET_SURFACE_COMPRESSION_COVERAGE` and `FB_GET_INFO_V2` (read-only, NON_PRIVILEGED);
//! 2. a scratch `FERMI_VASPACE_A` with two fixed `NV50_MEMORY_VIRTUAL` objects of 2 MiB;
//! 3. `RM_MAP_MEMORY_DMA` of the object with kind overrides: 64 KiB pages at offsets
//!    0x10000 (kind 0x0C, SotTR's depth kind compressed) and 0x230000 (kind 0x08), one
//!    uncompressed 64 KiB map (kind 0x03) and one 2 MiB map (kind 0x09);
//! 4. `NV0080_CTRL_CMD_DMA_GET_PTE_INFO` (read-only) of every mapped VA, with the comptag
//!    line the source predicts, 1 + SPA[35:16];
//! 5. `RM_MAP_MEMORY` BAR1 windows of the object (whole, 64 KiB at 0x10000, 4 KiB at
//!    0x1000): a pattern written and read back, then the object zeroed (the slice clear);
//! 6. `RM_UNMAP_MEMORY_DMA`, `RM_UNMAP_MEMORY` and `RM_FREE` of everything, newest first.
//!
//! Besides, the host VRAM floor reads `FB_GET_INFO_V2` HEAP_FREE before each allocation and
//! map (hostfloor.rs). Every call is printed with its status. No register access, no module,
//! no reset, no unbind.
//!
//! Run: cargo run --release -p vfio-user-nvgpu --example comp-slice-probe

use nvrm_abi::nvgpu::{nvos32_attr, nvos46_flags};
use nvrm_abi::sys;
use vfio_user_nvgpu::compslice::{self, comptag_line, PteBlock};
use vfio_user_nvgpu::hostmem::HostMemory;
use vfio_user_nvgpu::privilege::Privilege;

const MIB: u64 = 1 << 20;
/// Scratch VAs: above the server-RM range [4 GiB, 4.5 GiB) of every VA space.
const VA0: u64 = 0x2_0000_0000;
const VA1: u64 = VA0 + 2 * MIB;

fn as_bytes<T>(p: &mut T) -> &mut [u8] {
    // SAFETY: plain-data RM parameter structs viewed as bytes for the raw escapes.
    unsafe { std::slice::from_raw_parts_mut(p as *mut T as *mut u8, std::mem::size_of::<T>()) }
}

/// A contiguous vidmem object of `size` bytes in 2 MiB pages with `NVOS32_ATTR_COMPR`
/// `compr`: (handle, returned params).
fn alloc(hm: &mut HostMemory, size: u64, compr: u32, what: &'static str) -> Option<(u32, sys::NV_MEMORY_ALLOCATION_PARAMS)> {
    let mut p = sys::NV_MEMORY_ALLOCATION_PARAMS::default();
    p.owner = u32::from_le_bytes(*b"cara");
    p.type_ = sys::NVOS32_TYPE_IMAGE;
    p.attr = nvos32_attr::LOCATION.set(sys::NVOS32_ATTR_LOCATION_VIDMEM)
        | nvos32_attr::PHYSICALITY.set(sys::NVOS32_ATTR_PHYSICALITY_CONTIGUOUS)
        | nvos32_attr::PAGE_SIZE.set(sys::NVOS32_ATTR_PAGE_SIZE_HUGE)
        | (compr << compslice::ATTR_COMPR_SHIFT);
    p.attr2 = 1 << 20; // NVOS32_ATTR2_PAGE_SIZE_HUGE_2MB
    p.size = size;
    let attr_in = p.attr;
    let device = hm.device();
    let r = hm.alloc_object(device, 0x40, as_bytes(&mut p), what);
    match r {
        Ok(h) => {
            println!(
                "probe: RM_ALLOC NV01_MEMORY_LOCAL_USER {what}: {} MiB attr in {attr_in:#x} -> handle {h:#x}, status 0; offset (FB PA) {:#x}, attr {:#x} (COMPR {}), attr2 {:#x}, format (PTE kind) {:#x}, comprCovg {:#x}",
                size / MIB,
                p.offset,
                p.attr,
                (p.attr >> compslice::ATTR_COMPR_SHIFT) & 3,
                p.attr2,
                p.format,
                p.comprCovg
            );
            Some((h, p))
        }
        Err(st) => {
            println!("probe: RM_ALLOC NV01_MEMORY_LOCAL_USER {what}: attr in {attr_in:#x} -> status {st:#x}");
            None
        }
    }
}

fn blocks_line(b: &[PteBlock; 5]) -> String {
    b.iter()
        .filter(|b| b.page_size != 0)
        .map(|b| {
            format!(
                "[page {:#x}: valid {} aperture {} kind {:#x} comptagLine {:#x} flags {:#x}]",
                b.page_size,
                b.valid(),
                b.aperture(),
                b.kind,
                b.comptag_line,
                b.flags
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() -> anyhow::Result<()> {
    let sel = vfio_user_nvgpu::hostabi::select().map_err(anyhow::Error::msg)?;
    println!("probe: host ABI {} ({})", sel.layouts.describe(), sel.how);
    let mut hm = HostMemory::open(Privilege::none())?;
    println!(
        "probe: host RM client {:#x} with NV01_DEVICE_0 {:#x} and NV20_SUBDEVICE_0 {:#x} (RM_ALLOC x3, status 0)",
        hm.root(),
        hm.device(),
        hm.subdevice()
    );

    // 1. The slice's attributes, then COMPR_REQUIRED for comparison.
    let Some((h, p)) = alloc(&mut hm, 4 * MIB, compslice::ATTR_COMPR_DISABLE_PLC_ANY, "probe-compressed") else {
        println!("probe: the host refused the compressed object; nothing else to probe");
        return Ok(());
    };
    let pa = p.offset;
    let g = hm.query_grant(h, pa, 4 * MIB, p.attr, p.attr2, p.format);
    println!(
        "probe: GET_SURFACE_INFO status {:#x}, GET_SURFACE_COMPRESSION_COVERAGE status {:#x}, FB_GET_INFO_V2 status {:#x}",
        g.statuses[0], g.statuses[1], g.statuses[2]
    );
    println!("probe: {}", g.describe());
    match g.verdict() {
        Ok(()) => println!("probe: verdict: the grant isolates the slice (1:1, physically based, kind 0x09)"),
        Err(e) => println!("probe: verdict: REFUSED: {e}"),
    }
    if let Some((h2, p2)) = alloc(&mut hm, 2 * MIB, 1, "probe-compr-required") {
        let g2 = hm.query_grant(h2, p2.offset, 2 * MIB, p2.attr, p2.attr2, p2.format);
        println!("probe: COMPR_REQUIRED for comparison: {}", g2.describe());
        println!("probe: RM_FREE {h2:#x}: {:?}", hm.free_object(h2));
    }

    // 2. A scratch VA space with two 2 MiB virtual objects (default and huge page class,
    // as the shadow's region objects).
    let mut vp = sys::NV_VASPACE_ALLOCATION_PARAMETERS::default();
    vp.vaSize = 1 << 47;
    vp.bigPageSize = 64 * 1024;
    let device = hm.device();
    let vas = match hm.alloc_object(device, sys::FERMI_VASPACE_A, as_bytes(&mut vp), "probe-vaspace") {
        Ok(v) => v,
        Err(st) => {
            println!("probe: RM_ALLOC FERMI_VASPACE_A: status {st:#x}");
            println!("probe: RM_FREE {h:#x}: {:?}", hm.free_object(h));
            return Ok(());
        }
    };
    println!("probe: RM_ALLOC FERMI_VASPACE_A -> {vas:#x}, status 0");
    let v0 = hm.alloc_virtual_fixed(vas, VA0, 2 * MIB, None, false);
    println!("probe: RM_ALLOC NV50_MEMORY_VIRTUAL at {VA0:#x} (+2 MiB, default page size): {v0:x?}");
    let v1 = hm.alloc_virtual_fixed(vas, VA1, 2 * MIB, Some(2 * MIB), false);
    println!("probe: RM_ALLOC NV50_MEMORY_VIRTUAL at {VA1:#x} (+2 MiB, 2 MiB pages): {v1:x?}");

    // 3. The maps: (virtual object, object offset, length, VA, page size flag, kind).
    let mut maps = Vec::new();
    if let Ok(d0) = v0 {
        maps.push((d0, 0x1_0000u64, 0x1_0000u64, VA0 + 0x1_0000, nvos46_flags::PAGE_SIZE_BIG, 0x0Cu8));
        maps.push((d0, 0x23_0000, 0x1_0000, VA0 + 0x4_0000, nvos46_flags::PAGE_SIZE_BIG, 0x08));
        maps.push((d0, 0x5_0000, 0x1_0000, VA0 + 0x5_0000, nvos46_flags::PAGE_SIZE_BIG, 0x03));
    }
    if let Ok(d1) = v1 {
        maps.push((d1, 0x20_0000, 2 * MIB, VA1, nvos46_flags::PAGE_SIZE_HUGE, 0x09));
    }
    let mut live = Vec::new();
    for &(d, off, len, va, ps, kind) in &maps {
        let st = hm.map_dma_fixed(d, h, off, len, va, ps, false, Some(kind), false);
        println!(
            "probe: RM_MAP_MEMORY_DMA object +{off:#x} (+{len:#x}) at VA {va:#x}, kind override {kind:#x}: status {st:#x}"
        );
        if st == sys::NV_OK {
            live.push((d, off, len, va, kind));
        }
    }

    // 4. The PTEs the host wrote, against 1 + SPA[35:16].
    for &(_, off, len, va, kind) in &live {
        let want = if compslice::compressible(kind) {
            comptag_line(pa + off)
        } else {
            0
        };
        match hm.pte_info(vas, va) {
            Ok(b) => println!(
                "probe: DMA_GET_PTE_INFO VA {va:#x} (object +{off:#x}, PA {:#x}, {len:#x}-byte page, kind {kind:#x}): status 0 {}; predicted comptagLine {want:#x}",
                pa + off,
                blocks_line(&b)
            ),
            Err(st) => println!(
                "probe: DMA_GET_PTE_INFO VA {va:#x} (object +{off:#x}, PA {:#x}, kind {kind:#x}): status {st:#x}{}; predicted comptagLine {want:#x}",
                pa + off,
                if st == compslice::NV_ERR_TEST_ONLY_CODE_NOT_ENABLED {
                    " (NV_ERR_TEST_ONLY_CODE_NOT_ENABLED: test-only control, EnableRmTestOnlyCode not set)"
                } else {
                    ""
                }
            ),
        }
    }

    // 5. BAR1 windows of the compressed object (kind 0x09 with physically based comptags).
    match hm.map_object(h, 0, (4 * MIB) as usize) {
        Ok(w) => {
            let mut z = [0u8; 16];
            w.read(0x1_0000, &mut z);
            println!("probe: RM_MAP_MEMORY whole object (4 MiB): status 0; 16 bytes at +0x10000 before any write: {z:02x?}");
            let pat = 0xCA7A_5E5A_D16B_0001u64.to_le_bytes();
            let pat2 = 0xCA7A_5E5A_D16B_0002u64.to_le_bytes();
            w.write(0x1_0000, &pat);
            w.write(0x23_0000, &pat2);
            let (mut a, mut b) = ([0u8; 8], [0u8; 8]);
            w.read(0x1_0000, &mut a);
            w.read(0x23_0000, &mut b);
            println!(
                "probe: BAR1 write and read back at +0x10000 {} and +0x230000 {}",
                if a == pat { "ok" } else { "MISMATCH" },
                if b == pat2 { "ok" } else { "MISMATCH" }
            );
            for (off, len) in [(0x1_0000u64, 0x1_0000usize), (0x1000, 0x1000)] {
                match hm.map_object(h, off, len) {
                    Ok(w2) => {
                        let mut c = [0u8; 8];
                        w2.read(off.max(0x1_0000).min(off + len as u64 - 8), &mut c);
                        println!("probe: RM_MAP_MEMORY object +{off:#x} (+{len:#x}): status 0; read {c:02x?}");
                        drop(w2);
                        println!("probe: RM_UNMAP_MEMORY object +{off:#x}");
                    }
                    Err(st) => println!("probe: RM_MAP_MEMORY object +{off:#x} (+{len:#x}): status {st:#x}"),
                }
            }
            let t = std::time::Instant::now();
            w.fill_zero(0, (4 * MIB) as usize);
            let us = t.elapsed().as_micros().max(1);
            w.read(0x1_0000, &mut a);
            println!(
                "probe: zeroed the object through BAR1 in {us} us ({} MiB/s; 1 GiB would take about {} ms); +0x10000 now {a:02x?}",
                (4 * MIB as u128) * 1_000_000 / us / (MIB as u128),
                us * 256 / 1000
            );
            drop(w);
            println!("probe: RM_UNMAP_MEMORY whole object");
        }
        Err(st) => println!("probe: RM_MAP_MEMORY whole object: status {st:#x}"),
    }

    // 6. Everything back, newest first.
    for &(d, _, len, va, _) in live.iter().rev() {
        println!("probe: RM_UNMAP_MEMORY_DMA VA {va:#x} (+{len:#x}): status {:#x}", hm.unmap_dma(d, h, va, len, false));
    }
    for d in [v1, v0].into_iter().flatten() {
        println!("probe: RM_FREE virtual {d:#x}: {:?}", hm.free_object(d));
    }
    println!("probe: RM_FREE VA space {vas:#x}: {:?}", hm.free_object(vas));
    println!("probe: RM_FREE object {h:#x}: {:?}", hm.free_object(h));
    for l in hm.shutdown() {
        println!("probe: teardown: {l}");
    }
    Ok(())
}
