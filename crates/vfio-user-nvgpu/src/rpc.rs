// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The RPC vocabulary the device answers during init (docs/caraxes/gsp-rpc-protocol.md §3-4,
//! docs/caraxes/gsp-abi-610.md (b)-(d)) and the `GspStaticConfigInfo` reply built from
//! the profile, the chip, the board and the WPR layout the guest itself computed. The
//! function numbers and layouts are the emulated driver version's ([`crate::ver`]). Every field of the reply is
//! labelled with its provenance (decision D9 in docs/caraxes/README.md): `profile`,
//! `card` (captured from the physical card), `guest` (derived from what the guest wrote),
//! `synthesized` (a constant chosen here, to be replaced by host data in E4).

use crate::board::Board;
use crate::chip::ChipData;
use crate::config::PciIds;
use crate::fb::FbLayout;
use crate::guestmem::GuestMemory;
use crate::ver::active as v;

pub use crate::ver::active::{
    EV_GSP_INIT_DONE, FN_CONTINUATION_RECORD, FN_DUP_OBJECT, FN_FREE, FN_GET_GSP_STATIC_INFO,
    FN_GSP_RM_ALLOC, FN_GSP_RM_CONTROL, FN_GSP_SET_SYSTEM_INFO, FN_INIT_GSP_TRACE_CRASH_BUFFER,
    FN_SET_GUEST_SYSTEM_INFO, FN_SET_REGISTRY, FN_UPDATE_BAR_PDE, NV_ERR_NOT_SUPPORTED, NV_OK,
    STATIC_INFO_SIZE, WPR_META_MAGIC,
};

pub fn function_name(f: u32) -> &'static str {
    match f {
        FN_SET_GUEST_SYSTEM_INFO => "SET_GUEST_SYSTEM_INFO",
        FN_FREE => "FREE",
        FN_DUP_OBJECT => "DUP_OBJECT",
        FN_GET_GSP_STATIC_INFO => "GET_GSP_STATIC_INFO",
        FN_UPDATE_BAR_PDE => "UPDATE_BAR_PDE",
        FN_CONTINUATION_RECORD => "CONTINUATION_RECORD",
        FN_GSP_SET_SYSTEM_INFO => "GSP_SET_SYSTEM_INFO",
        FN_SET_REGISTRY => "SET_REGISTRY",
        FN_GSP_RM_CONTROL => "GSP_RM_CONTROL",
        FN_GSP_RM_ALLOC => "GSP_RM_ALLOC",
        FN_INIT_GSP_TRACE_CRASH_BUFFER => "INIT_GSP_TRACE_CRASH_BUFFER",
        EV_GSP_INIT_DONE => "GSP_INIT_DONE",
        _ => "?",
    }
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    if off + 4 > b.len() {
        return 0;
    }
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

fn u64_at(b: &[u8], off: usize) -> u64 {
    if off + 8 > b.len() {
        return 0;
    }
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}

/// One line describing a request payload, for the trace.
pub fn describe(function: u32, payload: &[u8]) -> String {
    match function {
        FN_GSP_RM_ALLOC => format!(
            "hClient={:#x} hParent={:#x} hObject={:#x} hClass={:#x} paramsSize={} flags={:#x}",
            u32_at(payload, v::RM_ALLOC_H_CLIENT),
            u32_at(payload, v::RM_ALLOC_H_PARENT),
            u32_at(payload, v::RM_ALLOC_H_OBJECT),
            u32_at(payload, v::RM_ALLOC_H_CLASS),
            u32_at(payload, v::RM_ALLOC_PARAMS_SIZE),
            u32_at(payload, v::RM_ALLOC_FLAGS)
        ),
        FN_GSP_RM_CONTROL => format!(
            "hClient={:#x} hObject={:#x} cmd={:#x} paramsSize={} rpcFlags={:#x}",
            u32_at(payload, v::RM_CONTROL_H_CLIENT),
            u32_at(payload, v::RM_CONTROL_H_OBJECT),
            u32_at(payload, v::RM_CONTROL_CMD),
            u32_at(payload, v::RM_CONTROL_PARAMS_SIZE),
            u32_at(payload, v::RM_CONTROL_RPC_FLAGS)
        ),
        FN_FREE => format!(
            "hRoot={:#x} hParent={:#x} hObject={:#x}",
            u32_at(payload, v::FREE_H_ROOT),
            u32_at(payload, v::FREE_H_PARENT),
            u32_at(payload, v::FREE_H_OBJECT)
        ),
        FN_SET_GUEST_SYSTEM_INFO => format!(
            "vgx {:#x}.{:#x} driver={:?}",
            u32_at(payload, v::GUEST_INFO_VGX_MAJOR),
            u32_at(payload, v::GUEST_INFO_VGX_MINOR),
            guest_driver_version(payload)
        ),
        FN_GSP_SET_SYSTEM_INFO => format!(
            "gpuPhysAddr={:#x} gpuPhysFbAddr={:#x} gpuPhysInstAddr={:#x} bdf={:#x} pciConfigMirror={:#x}+{:#x} PCIDeviceID={:#x} PCISubDeviceID={:#x} rev={:#x} maxUserVa={:#x} hostPageSize={:#x}",
            u64_at(payload, v::SYSINFO_GPU_PHYS_ADDR),
            u64_at(payload, v::SYSINFO_GPU_PHYS_FB_ADDR),
            u64_at(payload, v::SYSINFO_GPU_PHYS_INST_ADDR),
            u64_at(payload, v::SYSINFO_BDF),
            u32_at(payload, v::SYSINFO_PCI_CONFIG_MIRROR_BASE),
            u32_at(payload, v::SYSINFO_PCI_CONFIG_MIRROR_SIZE),
            u32_at(payload, v::SYSINFO_PCI_DEVICE_ID),
            u32_at(payload, v::SYSINFO_PCI_SUB_DEVICE_ID),
            u32_at(payload, v::SYSINFO_PCI_REVISION_ID),
            u64_at(payload, v::SYSINFO_MAX_USER_VA),
            u64_at(payload, v::SYSINFO_HOST_PAGE_SIZE)
        ),
        FN_SET_REGISTRY => format!(
            "size={} entries={}",
            u32_at(payload, v::REGISTRY_SIZE),
            u32_at(payload, v::REGISTRY_NUM_ENTRIES)
        ),
        _ => {
            let head: Vec<String> = payload.iter().take(32).map(|b| format!("{b:02x}")).collect();
            format!("{} bytes: {}", payload.len(), head.join(""))
        }
    }
}

/// The driver version string of a `SET_GUEST_SYSTEM_INFO` request (the first 64 of its
/// 256 bytes, as the trace shows it).
pub fn guest_driver_version(payload: &[u8]) -> String {
    cstr(payload, v::GUEST_INFO_DRIVER_VERSION, 64)
}

/// The BAR0, BAR1, BAR3 and I/O addresses of a `GspSystemInfo` (GSP_SET_SYSTEM_INFO),
/// for `BUS_GET_PCI_BAR_INFO`.
pub fn system_info_bar_addresses(payload: &[u8]) -> Option<[u64; 4]> {
    (payload.len() >= v::SYSINFO_GPU_PHYS_IO_ADDR + 8).then(|| {
        [
            u64_at(payload, v::SYSINFO_GPU_PHYS_ADDR),
            u64_at(payload, v::SYSINFO_GPU_PHYS_FB_ADDR),
            u64_at(payload, v::SYSINFO_GPU_PHYS_INST_ADDR),
            u64_at(payload, v::SYSINFO_GPU_PHYS_IO_ADDR),
        ]
    })
}

fn cstr(b: &[u8], off: usize, max: usize) -> String {
    let end = (off + max).min(b.len());
    let s = &b[off.min(end)..end];
    let n = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf8_lossy(&s[..n]).into_owned()
}

/// What the guest computed for its framebuffer, read from `GspFwWprMeta`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WprLayout {
    pub fb_size: u64,
    pub gsp_fw_rsvd_start: u64,
    pub non_wpr_heap_offset: u64,
    pub non_wpr_heap_size: u64,
    pub gsp_fw_wpr_start: u64,
    pub frts_offset: u64,
    pub gsp_fw_wpr_end: u64,
    pub vga_workspace_offset: u64,
}

impl WprLayout {
    /// Offsets of the emulated driver's `GspFwWprMeta` (gsp-abi-610.md (a)).
    pub fn read(mem: &GuestMemory, pa: u64) -> Result<Self, String> {
        let mut b = [0u8; v::WPR_META_SIZE];
        if !mem.read(pa, &mut b) {
            return Err(format!("WPR meta at {pa:#x} not mapped"));
        }
        let magic = u64_at(&b, v::WPR_META_MAGIC_OFF);
        if magic != WPR_META_MAGIC {
            return Err(format!("WPR meta at {pa:#x}: magic {magic:#x}"));
        }
        Ok(WprLayout {
            fb_size: u64_at(&b, v::WPR_META_FB_SIZE),
            gsp_fw_rsvd_start: u64_at(&b, v::WPR_META_GSP_FW_RSVD_START),
            non_wpr_heap_offset: u64_at(&b, v::WPR_META_NON_WPR_HEAP_OFFSET),
            non_wpr_heap_size: u64_at(&b, v::WPR_META_NON_WPR_HEAP_SIZE),
            gsp_fw_wpr_start: u64_at(&b, v::WPR_META_GSP_FW_WPR_START),
            frts_offset: u64_at(&b, v::WPR_META_FRTS_OFFSET),
            gsp_fw_wpr_end: u64_at(&b, v::WPR_META_GSP_FW_WPR_END),
            vga_workspace_offset: u64_at(&b, v::WPR_META_VGA_WORKSPACE_OFFSET),
        })
    }
}

/// The identity the device presents (docs/caraxes/identity-profile.md): the profile
/// (name, FB size, GID), the chip (short name), and the board (PCI ids from the
/// config-space capture, VBIOS version, BAR sizes).
#[derive(Clone, Debug)]
pub struct StaticIdentity {
    pub gpu_name: String,
    pub short_name: String,
    pub fb_bytes: u64,
    pub gid: [u8; 16],
    /// Vendor, device, revision and subsystem ids of the emulated function.
    pub pci: PciIds,
    pub vbios_revision: u32,
    pub vbios_oem_revision: u32,
    /// The provenance label of the VBIOS answers, naming the version.
    pub vbios_label: &'static str,
    /// Sizes of the BARs that open a region (BAR0, BAR1, BAR3, BAR5).
    pub bar_sizes: Vec<u64>,
}

impl StaticIdentity {
    /// The identity from the chip, the board, the captured config space and the
    /// profile's FB size and board name. The label string is created once per call
    /// and lives for the process (one call per device).
    pub fn new(
        chip: &ChipData,
        board: &Board,
        pci: PciIds,
        board_name: &str,
        fb_bytes: u64,
    ) -> Self {
        let gpu_name = Self::guest_name(board_name, fb_bytes);
        StaticIdentity {
            gid: Self::synthetic_gid(&gpu_name, fb_bytes),
            gpu_name,
            short_name: chip.short_name.clone(),
            fb_bytes,
            pci,
            vbios_revision: board.vbios_revision,
            vbios_oem_revision: board.vbios_oem_revision,
            vbios_label: Box::leak(
                format!("card (VBIOS {} from the captured BIT)", board.vbios_version)
                    .into_boxed_str(),
            ),
            bar_sizes: board.region_bar_sizes(),
        }
    }

    /// The Leandro naming rule (`guest_card_name` in crates/vhost-user-nvrm/src/vram.rs):
    /// `Leandro <board>-<size>` with whole GiB as `G`, else MiB as `M`.
    pub fn guest_name(board: &str, fb_bytes: u64) -> String {
        let mib = fb_bytes >> 20;
        if mib % 1024 == 0 {
            format!("Leandro {board}-{}G", mib / 1024)
        } else {
            format!("Leandro {board}-{mib}M")
        }
    }

    /// A stable, non-zero GID derived from the name and size (`synthesized`; E3 replaces
    /// it with the per-VM UUID from Leandro's `grid::vm_uuid`).
    pub fn synthetic_gid(name: &str, fb_bytes: u64) -> [u8; 16] {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in name.bytes().chain(fb_bytes.to_le_bytes()) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        let mut g = [0u8; 16];
        g[..8].copy_from_slice(&h.to_le_bytes());
        g[8..].copy_from_slice(&h.rotate_left(29).to_le_bytes());
        g[0] |= 1;
        g
    }
}

/// A labelled field of the static-info reply.
#[derive(Clone, Debug)]
pub struct Labelled {
    pub field: &'static str,
    pub value: String,
    pub source: &'static str,
}

/// Build the `GspStaticConfigInfo` of the emulated driver (offsets from
/// docs/caraxes/gsp-abi-610.md); `chip` supplies the engine list.
pub fn build_static_info(
    chip: &'static ChipData,
    id: &StaticIdentity,
    wpr: Option<&WprLayout>,
    layout: &FbLayout,
) -> (Vec<u8>, Vec<Labelled>) {
    let mut b = vec![0u8; STATIC_INFO_SIZE];
    let mut labels = Vec::new();
    let put32 =
        |b: &mut [u8], off: usize, v: u32| b[off..off + 4].copy_from_slice(&v.to_le_bytes());
    fn put64(b: &mut [u8], off: usize, v: u64) {
        b[off..off + 8].copy_from_slice(&v.to_le_bytes());
    }
    fn label(labels: &mut Vec<Labelled>, field: &'static str, value: String, source: &'static str) {
        labels.push(Labelled {
            field,
            value,
            source,
        });
    }

    // gidInfo: index, flags, length, data[256]; the CPU reads data[0..16].
    put32(&mut b, v::SI_GID_LENGTH, 16);
    b[v::SI_GID_DATA..v::SI_GID_DATA + 16].copy_from_slice(&id.gid);
    label(
        &mut labels,
        "gidInfo.data[0..16]",
        hex(&id.gid),
        "synthesized (E4: per-VM UUID)",
    );

    // fbRegionInfoParams: numFBRegions, then the regions. With a host VRAM reservation
    // the usable region is the reservation minus the device's carve-out at its top, and
    // everything above it is reserved; nothing below the reservation is described (as
    // on the host, whose one region starts at 54 MiB). docs/caraxes/memory-model.md §1.
    let space = layout.addr_space;
    let (usable_start, usable_end, rsvd_start, source) = match (layout.usable(), wpr) {
        (Some((b, e)), _) => (
            b,
            e,
            e,
            "reservation (host VRAM fixed allocation; usable = reservation minus the carve-out)",
        ),
        (None, Some(w)) if w.gsp_fw_rsvd_start > 0 && w.gsp_fw_rsvd_start < space => (
            0,
            w.gsp_fw_rsvd_start,
            w.gsp_fw_rsvd_start,
            "guest (WPR meta gspFwRsvdStart)",
        ),
        _ => (
            0,
            space - (160 << 20),
            space - (160 << 20),
            "synthesized fallback (fb - 160 MiB)",
        ),
    };
    put32(&mut b, v::SI_NUM_FB_REGIONS, 2);
    let region = |b: &mut [u8], i: usize, base: u64, limit: u64, reserved: u64| {
        let o = v::SI_FB_REGIONS + i * v::FB_REGION_SIZE;
        put64(b, o + v::FB_REGION_BASE, base);
        put64(b, o + v::FB_REGION_LIMIT, limit);
        put64(b, o + v::FB_REGION_RESERVED, reserved);
        let perf = o + v::FB_REGION_PERFORMANCE;
        b[perf..perf + 4].copy_from_slice(&1u32.to_le_bytes());
        // The FB region does NOT support compression. The guest's memory manager, told a
        // region is compressible, gives its vidmem surfaces a compressible KIND
        // (GENERIC_MEMORY_COMPRESSIBLE) and a COMPTAGLINE in their PTEs. The reservation
        // is allocated COMPR-NONE, so a copy-engine write through such a PTE drives the L2
        // comptag path into a compression resource that does not exist and hangs (no Xid;
        // host-gr.md, execution.md §6). Worse for isolation: the COMPTAGLINE (PTE bits
        // 55:36) indexes the host's GLOBAL comptag store that the desktop's own compressed
        // surfaces use, so it must never carry a guest value. Reporting no compression
        // makes the guest pick uncompressible kinds (GENERIC_MEMORY 0x06 / PITCH 0x00),
        // COMPTAGLINE 0. Synthesized; fb-model.md §510, memory-model.md.
        b[o + v::FB_REGION_SUPPORT_COMPRESSED] = 0;
        b[o + v::FB_REGION_SUPPORT_ISO] = 1;
    };
    region(&mut b, 0, usable_start, usable_end - 1, 0);
    region(&mut b, 1, rsvd_start, space - 1, space - rsvd_start);
    label(
        &mut labels,
        "fbRegionInfoParams",
        format!(
            "2 regions: usable [{usable_start:#x}, {usable_end:#x}), reserved [{rsvd_start:#x}, {space:#x})"
        ),
        source,
    );

    // engineCaps: NV2080_ENGINE_TYPE bits of the chip's engines (chip data).
    let caps = chip.engine_caps_mask(v::ENGINE_CAPS_WORDS);
    for (i, c) in caps.iter().enumerate() {
        put32(&mut b, v::SI_ENGINE_CAPS + 4 * i, *c);
    }
    let words: Vec<String> = caps.iter().map(|c| format!("{c:#x}")).collect();
    label(
        &mut labels,
        "engineCaps",
        format!("{} ({})", words.join(" "), chip.engine_caps.summary),
        &chip.engine_caps.source,
    );

    put64(&mut b, v::SI_FB_LENGTH, id.fb_bytes);
    label(
        &mut labels,
        "fb_length",
        format!("{:#x}", id.fb_bytes),
        "profile",
    );
    let name = id.gpu_name.as_bytes();
    let n = name.len().min(v::SI_NAME_LEN - 1);
    b[v::SI_GPU_NAME..v::SI_GPU_NAME + n].copy_from_slice(&name[..n]);
    label(
        &mut labels,
        "gpuNameString",
        id.gpu_name.clone(),
        "profile (guest_card_name)",
    );
    let short = id.short_name.as_bytes();
    let n = short.len().min(v::SI_NAME_LEN - 1);
    b[v::SI_GPU_SHORT_NAME..v::SI_GPU_SHORT_NAME + n].copy_from_slice(&short[..n]);
    label(
        &mut labels,
        "gpuShortNameString",
        id.short_name.clone(),
        "card",
    );

    let (bar1, bar2, pde_source) = match (layout.bar_roots(), wpr) {
        (Some((bar2, bar1)), _) => (
            bar1,
            bar2,
            "reservation carve-out (top 2 MiB; docs/caraxes/memory-model.md)",
        ),
        (None, Some(w)) if w.non_wpr_heap_offset > 0 => (
            w.non_wpr_heap_offset + 0x1000,
            w.non_wpr_heap_offset,
            "guest (WPR meta nonWprHeapOffset; docs/caraxes/fb-model.md)",
        ),
        _ => (rsvd_start + 0x1000, rsvd_start, "synthesized fallback"),
    };
    put64(&mut b, v::SI_BAR1_PDE_BASE, bar1);
    put64(&mut b, v::SI_BAR2_PDE_BASE, bar2);
    label(
        &mut labels,
        "bar1PdeBase/bar2PdeBase",
        format!("{bar1:#x}/{bar2:#x}"),
        pde_source,
    );

    b[v::SI_VBIOS_VALID] = 1;
    put32(&mut b, v::SI_VBIOS_SUB_VENDOR, id.pci.sub_vendor as u32);
    put32(&mut b, v::SI_VBIOS_SUB_DEVICE, id.pci.sub_device as u32);
    put32(&mut b, v::SI_VBIOS_REVISION, id.vbios_revision);
    put32(&mut b, v::SI_VBIOS_OEM_REVISION, id.vbios_oem_revision);
    label(
        &mut labels,
        "vbios*",
        format!(
            "valid, subsystem {:#06x}:{:#06x}, revision {:#010x}, oem {:#x}",
            id.pci.sub_vendor, id.pci.sub_device, id.vbios_revision, id.vbios_oem_revision
        ),
        "card (config space, VBIOS BIT)",
    );

    put32(
        &mut b,
        v::SI_H_INTERNAL_CLIENT,
        crate::ctrl::INTERNAL_CLIENT,
    );
    put32(
        &mut b,
        v::SI_H_INTERNAL_DEVICE,
        crate::ctrl::INTERNAL_DEVICE,
    );
    put32(
        &mut b,
        v::SI_H_INTERNAL_SUBDEVICE,
        crate::ctrl::INTERNAL_SUBDEVICE,
    );
    label(
        &mut labels,
        "hInternalClient/Device/Subdevice",
        "0xca7a0001/2/3".into(),
        "synthesized (device-owned handles)",
    );
    label(
        &mut labels,
        "branding, power, platform flags, SKU, PDI, ECID, bBusResetRequired",
        "0".into(),
        "synthesized (desktop GeForce; E4: host controls)",
    );
    (b, labels)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_info_with_a_reservation() {
        // The 4 GiB profile on the RTX 2070 host: reservation [0xe9000000, 0x1e9000000),
        // FB address space 8 GiB, the guest's WPR from 0x1f7600000 (memory-model.md §1).
        let f = crate::fixture::rtx2070();
        let id = f.identity(4 << 30);
        let layout = FbLayout::reserved(4 << 30, 8 << 30, 0xE900_0000, 0x1_E900_0000).unwrap();
        let wpr = WprLayout {
            fb_size: 8 << 30,
            gsp_fw_rsvd_start: 0x1_F760_0000,
            non_wpr_heap_offset: 0x1_F760_0000,
            non_wpr_heap_size: 0x10_0000,
            gsp_fw_wpr_start: 0x1_F770_0000,
            frts_offset: 0x1_FFE0_0000,
            gsp_fw_wpr_end: 0x1_FFF0_0000,
            vga_workspace_offset: 0x1_FFF0_0000,
        };
        let (b, labels) = build_static_info(f.chip.data(), &id, Some(&wpr), &layout);
        let r = |i: usize, o: usize| u64_at(&b, v::SI_FB_REGIONS + i * v::FB_REGION_SIZE + o);
        assert_eq!(u32_at(&b, v::SI_NUM_FB_REGIONS), 2);
        assert_eq!(
            (r(0, 0), r(0, 8), r(0, 16)),
            (0xE900_0000, 0x1_E8DF_FFFF, 0)
        );
        assert_eq!(
            (r(1, 0), r(1, 8), r(1, 16)),
            (
                0x1_E8E0_0000,
                (8u64 << 30) - 1,
                (8u64 << 30) - 0x1_E8E0_0000
            )
        );
        // nvidia-smi's total stays the profile.
        assert_eq!(u64_at(&b, v::SI_FB_LENGTH), 4 << 30);
        assert_eq!(u64_at(&b, v::SI_BAR2_PDE_BASE), 0x1_E8E0_0000);
        assert_eq!(u64_at(&b, v::SI_BAR1_PDE_BASE), 0x1_E8E0_1000);
        assert!(labels
            .iter()
            .any(|l| l.field == "fbRegionInfoParams" && l.source.starts_with("reservation")));
    }

    #[test]
    fn static_info_layout() {
        let f = crate::fixture::rtx2070();
        let id = f.identity(4 << 30);
        assert_eq!(id.gpu_name, "Leandro RTX 2070-4G");
        assert_eq!(
            StaticIdentity::guest_name("RTX 2070", 130 << 20),
            "Leandro RTX 2070-130M"
        );
        let wpr = WprLayout {
            fb_size: 4 << 30,
            gsp_fw_rsvd_start: 0xF760_0000,
            non_wpr_heap_offset: 0xF760_0000,
            non_wpr_heap_size: 0x10_0000,
            gsp_fw_wpr_start: 0xF770_0000,
            frts_offset: 0xFFE0_0000,
            gsp_fw_wpr_end: 0xFFF0_0000,
            vga_workspace_offset: 0xFFF0_0000,
        };
        let (b, labels) =
            build_static_info(f.chip.data(), &id, Some(&wpr), &FbLayout::plain(4 << 30));
        assert_eq!(b.len(), STATIC_INFO_SIZE);
        assert_ne!(&b[0x0C..0x1C], &[0u8; 16]);
        assert_eq!(u32_at(&b, 0x140), 2);
        assert_eq!(u64_at(&b, 0x148 + 8), 0xF75F_FFFF);
        assert_eq!(u64_at(&b, 0x148 + 56), 0xF760_0000);
        assert_eq!(u64_at(&b, 0x148 + 56 + 16), (4u64 << 30) - 0xF760_0000);
        assert_eq!(u64_at(&b, 0x540), 4 << 30);
        assert_eq!(cstr(&b, 0x548, 64), "Leandro RTX 2070-4G");
        assert_eq!(cstr(&b, 0x588, 64), "TU106");
        assert_eq!(u64_at(&b, 0x5E8), 0xF760_0000);
        assert_eq!(u32_at(&b, 0x52C) & 2, 2);
        assert_eq!(u32_at(&b, 0x530) & (1 << 6), 1 << 6);
        assert_eq!(u32_at(&b, 0x608), 0xCA7A_0001);
        assert_eq!(b[0x638], 0);
        assert_eq!(u32_at(&b, 0x5F4), 0x1458);
        assert_eq!(u32_at(&b, 0x5FC), 0x9006_3D00);
        assert!(labels.iter().any(|l| l.field == "engineCaps"
            && l.value == "0x81bfe02 0xc4 0x0 (GR0, CE0-8, NVDEC0/1, NVENC0, SW, SEC2, HOST)"
            && l.source == "synthesized (TU106 child list; E4: host GET_ENGINES_V2)"));
        assert!(labels
            .iter()
            .any(|l| l.field == "fb_length" && l.source == "profile"));
        assert_eq!(
            describe(
                FN_GSP_RM_CONTROL,
                &[1, 0, 0, 0, 2, 0, 0, 0, 0x1c, 0x0a, 0x80, 0x20]
            ),
            "hClient=0x1 hObject=0x2 cmd=0x20800a1c paramsSize=0 rpcFlags=0x0"
        );
    }
}
