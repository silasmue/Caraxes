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
use nvrm_abi::naming::{self, Backend, NameFormat, Personality, Transport, VirtualGpuSpec};

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
        crate::gspmem::FN_ALLOC_MEMORY => "ALLOC_MEMORY",
        // The GMCAPI commands of the GSP management category share the namespace above
        // 0xFFFF (615 on, gmcapi.rs).
        f @ crate::gmcapi::CMD_GSP_INIT..=crate::gmcapi::CMD_GSP_RESUME_DONE => {
            crate::gmcapi::command_name(f)
        }
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
        crate::gmcapi::CMD_GSP_INIT => format!(
            "{} bytes of NVKV system info ({} words)",
            payload.len(),
            payload.len() / 8
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
    /// profile's FB size and board name, with the default name (transport word,
    /// `Leandro VFIO RTX 2070-4G`). The label
    /// string is created once per call and lives for the process (one call per device).
    pub fn new(
        chip: &ChipData,
        board: &Board,
        pci: PciIds,
        board_name: &str,
        fb_bytes: u64,
    ) -> Self {
        let spec = Self::spec(board_name, fb_bytes);
        Self::from_spec(chip, board, pci, &spec, NameFormat::default(), fb_bytes)
    }

    /// The identity with the guest name generated from `spec` in `format`
    /// (`nvrm_abi::naming`). The GID follows the legacy-format name whatever the
    /// format, so neither the choice nor the 2026-09-24 default switch changes the
    /// guest's UUID.
    pub fn from_spec(
        chip: &ChipData,
        board: &Board,
        pci: PciIds,
        spec: &VirtualGpuSpec,
        format: NameFormat,
        fb_bytes: u64,
    ) -> Self {
        let gpu_name = spec.guest_name_within(format, v::SI_NAME_LEN);
        let gid_seed = spec.guest_name_within(NameFormat::Legacy, v::SI_NAME_LEN);
        StaticIdentity {
            gid: Self::synthetic_gid(&gid_seed, fb_bytes),
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

    /// The Caraxes device's spec: transport VFIO, personality `board` (the board
    /// data's `name` or `--board`), profile the guest FB size, backend the host RM.
    pub fn spec(board: &str, fb_bytes: u64) -> VirtualGpuSpec {
        VirtualGpuSpec {
            transport: Transport::Vfio,
            personality: Personality::new(board),
            profile: naming::Profile::from_fb_bytes(fb_bytes),
            backend: Backend::OpenRm,
        }
    }

    /// The default guest name (`nvrm_abi::naming`, transport format):
    /// `Leandro VFIO <board>-<size>` with whole GiB as `G`, else MiB as `M`, the same
    /// rule as Leandro v1.
    pub fn guest_name(board: &str, fb_bytes: u64) -> String {
        Self::guest_name_as(board, fb_bytes, NameFormat::default())
    }

    /// The guest name in `format`, bounded by the static info's name field.
    pub fn guest_name_as(board: &str, fb_bytes: u64, format: NameFormat) -> String {
        Self::spec(board, fb_bytes).guest_name_within(format, v::SI_NAME_LEN)
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
        (Some((b, e)), _) if layout.pool > 0 => (
            b,
            e,
            e,
            "reservation (host VRAM fixed allocation; usable = reservation minus the carve-out and the BAR1 pool below it, docs/caraxes/bar1-pool.md)",
        ),
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

    // fwWprLayoutOffset: where the WPR ended up. The kernel RM takes nonWprHeapOffset and
    // frtsOffset from here only where ACR lays the WPR out (GH100, GB20x;
    // kernel_gsp.c:4712-4728); a Turing guest does not read it (gsp-abi-610.md).
    if let Some(w) = wpr {
        put64(
            &mut b,
            v::SI_FW_WPR_NON_WPR_HEAP_OFFSET,
            w.non_wpr_heap_offset,
        );
        put64(&mut b, v::SI_FW_WPR_FRTS_OFFSET, w.frts_offset);
        label(
            &mut labels,
            "fwWprLayoutOffset",
            format!(
                "nonWprHeapOffset {:#x} frtsOffset {:#x}",
                w.non_wpr_heap_offset, w.frts_offset
            ),
            "guest (WPR meta; on GB20x as the FSP model laid it out)",
        );
    }

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

/// `[0x1, 0x2]`, on one line (the trace is one line per event).
fn hex_list<T: std::fmt::LowerHex>(v: &[T]) -> String {
    let items: Vec<String> = v.iter().map(|x| format!("{x:#x}")).collect();
    format!("[{}]", items.join(", "))
}

/// The static configuration of the 615 transport: the `GspStaticConfigInfo` image of
/// [`build_static_info`] (the same values and provenance labels as the 610 reply)
/// encoded as the NVGMC_SC keys of the `GMCAPI_CMD_GSP_INIT` response, which
/// `_kgspStaticInfoKeyHandler` decodes into the kernel's own `GspStaticConfigInfo`
/// (kernel_gsp.c:7729-7862 of 615; key table gmcapi_gsp_config.h:30-148).
///
/// Every key carries the value type and index the handler validates, or the whole
/// init fails (gmcapi.rs `sc`). `FB_REGION_COUNT` comes before the indexed region
/// keys, which the handler bounds by it (kernel_gsp.c:7762-7768, 7830-7835). The engine
/// list goes out as NVGMC engine masks (`gmcapi::engine_masks`). Fields the 610 reply
/// left zero (branding, power and misc features, SKU, PDI, ECID) are sent as zero
/// words or not at all; the kernel zeroes the struct before decoding
/// (kernel_gsp.c:7872). `VGX_VERSION` is not sent: the kernel keeps the RPC version it
/// was built with (rpc_common.c:100-104). Returns the payload and one trace line per key.
pub fn static_info_nvkv(image: &[u8], chip: &ChipData) -> (Vec<u8>, Vec<String>) {
    use crate::gmcapi::{engine_masks, sc, Encoder};
    let mut e = Encoder::new();
    let mut notes = Vec::new();
    let name = cstr(image, v::SI_GPU_NAME, v::SI_NAME_LEN);
    let short = cstr(image, v::SI_GPU_SHORT_NAME, v::SI_NAME_LEN);
    e.string8(0, sc::GPU_NAME_STRING, &name);
    e.string8(0, sc::GPU_SHORT_NAME_STRING, &short);
    notes.push(format!("GPU_NAME_STRING {name:?}, GPU_SHORT_NAME_STRING {short:?}"));
    let gid_len = (u32_at(image, v::SI_GID_LENGTH) as usize).min(16);
    let gid = &image[v::SI_GID_DATA..v::SI_GID_DATA + gid_len];
    e.array8(0, sc::GPU_GID_SHA1, gid);
    notes.push(format!("GPU_GID_SHA1 {}", hex(gid)));
    let handles = [
        u32_at(image, v::SI_H_INTERNAL_CLIENT),
        u32_at(image, v::SI_H_INTERNAL_DEVICE),
        u32_at(image, v::SI_H_INTERNAL_SUBDEVICE),
    ];
    e.seq32(0, sc::INTERNAL_CLIENT_HANDLE, &handles);
    notes.push(format!("INTERNAL_CLIENT/DEVICE/SUBDEVICE_HANDLE {}", hex_list(&handles)));
    if image[v::SI_VBIOS_VALID] != 0 {
        let vbios = [
            u32_at(image, v::SI_VBIOS_SUB_VENDOR),
            u32_at(image, v::SI_VBIOS_SUB_DEVICE),
            u32_at(image, v::SI_VBIOS_REVISION),
            u32_at(image, v::SI_VBIOS_OEM_REVISION),
        ];
        e.seq32(0, sc::VBIOS_SUB_VENDOR, &vbios);
        notes.push(format!("VBIOS_SUB_VENDOR/SUB_DEVICE/REVISION/OEM_REVISION {}", hex_list(&vbios)));
    }
    let regions = u32_at(image, v::SI_NUM_FB_REGIONS).min(sc::FB_REGION_MAX);
    e.imm32(0, sc::FB_REGION_COUNT, regions);
    for i in 0..regions as usize {
        let o = v::SI_FB_REGIONS + i * v::FB_REGION_SIZE;
        let r = [
            u64_at(image, o + v::FB_REGION_BASE),
            u64_at(image, o + v::FB_REGION_LIMIT),
            u64_at(image, o + v::FB_REGION_RESERVED),
        ];
        let perf = u32_at(image, o + v::FB_REGION_PERFORMANCE);
        let mut flags = 0;
        if image[o + v::FB_REGION_SUPPORT_COMPRESSED] != 0 {
            flags |= sc::FB_REGION_FLAGS_IS_COMPRESSION_SUPPORTED;
        }
        if image[o + v::FB_REGION_SUPPORT_ISO] != 0 {
            flags |= sc::FB_REGION_FLAGS_IS_ISO_SUPPORTED;
        }
        e.seq64(i as u16, sc::FB_REGION_BASE, &r);
        e.imm32(i as u16, sc::FB_REGION_PERFORMANCE, perf);
        e.imm32(i as u16, sc::FB_REGION_FLAGS, flags);
        notes.push(format!(
            "FB_REGION[{i}] base {:#x} limit {:#x} reserved {:#x} performance {perf} flags {flags:#x}",
            r[0], r[1], r[2]
        ));
    }
    let fb_length = u64_at(image, v::SI_FB_LENGTH);
    e.seq64(0, sc::FB_LENGTH, &[fb_length]);
    let pdes = [u64_at(image, v::SI_BAR1_PDE_BASE), u64_at(image, v::SI_BAR2_PDE_BASE)];
    e.seq64(0, sc::BAR1_PDE_BASE, &pdes);
    notes.push(format!("FB_LENGTH {fb_length:#x}, BAR1/BAR2_PDE_BASE {:#x}/{:#x}", pdes[0], pdes[1]));
    let misc = u64::from(image[v::SI_BUS_RESET_REQUIRED] != 0) << 5;
    e.seq64(0, sc::BRANDING, &[0, misc, 0]);
    notes.push(format!("BRANDING 0, MISC_FEATURES {misc:#x}, POWER_FEATURES 0"));
    let caps: Vec<u32> = (0..v::ENGINE_CAPS_WORDS * 32)
        .filter(|&b| u32_at(image, v::SI_ENGINE_CAPS + 4 * (b / 32)) & (1 << (b % 32)) != 0)
        .map(|b| b as u32)
        .collect();
    let (first, masks, unmapped) = engine_masks(&caps);
    e.array64(first, sc::ENGINE_MASK, &masks);
    notes.push(format!(
        "ENGINE_MASK from type {first}: {} ({}{})",
        hex_list(&masks),
        chip.engine_caps.summary,
        if unmapped.is_empty() {
            String::new()
        } else {
            format!("; no NVGMC id for NV2080 types {}", hex_list(&unmapped))
        }
    ));
    (e.to_bytes(), notes)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The key checks of `_kgspStaticInfoKeyHandler` (kernel_gsp.c:7729-7862 of 615),
    /// for every key the table has: the value type, index 0 for plain keys, an index
    /// below FB_REGION_COUNT (sent first) for the region keys, the array bounds of the
    /// struct fields (names 64 bytes, GID data 256, ECID two words). An error is what
    /// makes the kernel's GSP_INIT fail.
    pub(crate) fn kernel_accepts(words: &[u64]) -> Result<Vec<crate::gmcapi::Entry>, String> {
        use crate::gmcapi::{decode, sc, Value};
        let entries = decode(words)?;
        let mut regions: Option<u32> = None;
        for e in &entries {
            let (k, i) = (e.key, e.index as u32);
            let single = |ok: bool| if ok && i == 0 { Ok(()) } else { Err(()) };
            let r = match (k, &e.value) {
                (sc::FB_REGION_COUNT, Value::U32(n)) if i == 0 && *n <= sc::FB_REGION_MAX => {
                    regions = Some(*n);
                    Ok(())
                }
                (0x0011..=0x0013, Value::U32(_)) => {
                    if i < regions.unwrap_or(0) { Ok(()) } else { Err(()) }
                }
                (0x1011..=0x1013, Value::U64(_)) => {
                    if i < regions.unwrap_or(0) { Ok(()) } else { Err(()) }
                }
                (0x0001..=0x0005 | 0x0030 | 0x0031 | 0x0050..=0x0053 | 0x0200 | 0x0210 | 0x0FF0..=0x0FF2, v) => {
                    single(matches!(v, Value::U32(_)))
                }
                (0x0020 | 0x1000..=0x1002 | 0x1010 | 0x1020 | 0x1021 | 0x1050 | 0x1080 | 0x1081 | 0x1200..=0x1202, v) => {
                    single(matches!(v, Value::U64(_)))
                }
                (0x2000 | 0x2001, Value::U8s(b)) => single(!b.is_empty()),
                (0x2002, Value::U8s(b)) => single(b.len() <= 256),
                (0x2003..=0x2008, Value::U8s(_)) => single(true),
                (0x1040, Value::U64s(v)) => single(v.len() <= 2),
                (sc::ENGINE_MASK, Value::U64s(_)) => Ok(()),
                // A key of the table with the wrong type, index or count.
                (0x0010..=0x0013 | 0x1011..=0x1013 | 0x1040 | 0x1100 | 0x2000..=0x2008, _) => {
                    Err(())
                }
                _ => Ok(()), // not a key of the table: ignored (kernel_gsp.c:7850-7853)
            };
            r.map_err(|_| format!("the kernel refuses key {k:#06x} index {i} value {}", e.value))?;
        }
        Ok(entries)
    }

    #[test]
    fn static_info_as_nvkv_keys() {
        use crate::gmcapi::{sc, words, Value};
        let f = crate::fixture::rtx2070();
        let id = f.identity(4 << 30);
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
        let (img, _) = build_static_info(f.chip.data(), &id, Some(&wpr), &FbLayout::plain(4 << 30));
        let (bytes, notes) = static_info_nvkv(&img, f.chip.data());
        assert!(bytes.len() <= crate::gmcapi::GSP_INIT_MAX_RESPONSE);
        let entries = kernel_accepts(&words(&bytes)).unwrap();
        let get = |k: u16, i: u16| {
            entries
                .iter()
                .find(|e| e.key == k && e.index == i)
                .map(|e| e.value.clone())
                .unwrap_or_else(|| panic!("key {k:#x}[{i}] missing"))
        };
        assert_eq!(get(sc::GPU_NAME_STRING, 0).as_str().as_deref(), Some("Leandro VFIO RTX 2070-4G"));
        assert_eq!(get(sc::GPU_SHORT_NAME_STRING, 0).as_str().as_deref(), Some("TU106"));
        assert_eq!(get(sc::GPU_GID_SHA1, 0), Value::U8s(id.gid.to_vec()));
        assert_eq!(get(sc::FB_LENGTH, 0), Value::U64(4 << 30));
        assert_eq!(get(sc::FB_REGION_COUNT, 0), Value::U32(2));
        assert_eq!(get(sc::FB_REGION_LIMIT, 0), Value::U64(0xF75F_FFFF));
        assert_eq!(get(sc::FB_REGION_RESERVED, 1), Value::U64((4u64 << 30) - 0xF760_0000));
        // ISO only: no compression (build_static_info, fb-model.md).
        assert_eq!(get(sc::FB_REGION_FLAGS, 0), Value::U32(sc::FB_REGION_FLAGS_IS_ISO_SUPPORTED));
        assert_eq!(get(sc::BAR2_PDE_BASE, 0), Value::U64(0xF760_0000));
        assert_eq!(get(sc::INTERNAL_CLIENT_HANDLE, 0), Value::U32(0xCA7A_0001));
        assert_eq!(get(sc::INTERNAL_SUBDEVICE_HANDLE, 0), Value::U32(0xCA7A_0003));
        assert_eq!(get(sc::VBIOS_SUB_VENDOR, 0), Value::U32(0x1458));
        assert_eq!(get(sc::VBIOS_REVISION, 0), Value::U32(0x9006_3D00));
        // GR0, COPY0-8, NVDEC0-1, NVENC0, SW, SEC2, HOST as NVGMC masks from type GR.
        match get(sc::ENGINE_MASK, 1) {
            Value::U64s(m) => assert_eq!(&m[..4], &[1, 0x1FF, 0b11, 1]),
            v => panic!("{v:?}"),
        }
        assert!(notes.iter().any(|n| n.starts_with("ENGINE_MASK from type 1")), "{notes:?}");
        // What the kernel refuses, the check refuses: a region key before the count, a
        // U64 key sent as U32.
        let mut bad = crate::gmcapi::Encoder::new();
        bad.seq64(0, sc::FB_REGION_BASE, &[0, 1, 2]);
        assert!(kernel_accepts(bad.words()).is_err());
        let mut bad = crate::gmcapi::Encoder::new();
        bad.imm32(0, sc::FB_LENGTH, 1);
        assert!(kernel_accepts(bad.words()).is_err());
    }

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
        // P21 design H: a 64 MiB BAR1 pool ends the usable region 64 MiB lower and is
        // reported reserved with the carve-out; the BAR roots stay.
        let pooled = layout.with_pool(64 << 20).unwrap();
        let (b, labels) = build_static_info(f.chip.data(), &id, Some(&wpr), &pooled);
        let r = |i: usize, o: usize| u64_at(&b, v::SI_FB_REGIONS + i * v::FB_REGION_SIZE + o);
        assert_eq!((r(0, 0), r(0, 8)), (0xE900_0000, 0x1_E4DF_FFFF));
        assert_eq!((r(1, 0), r(1, 16)), (0x1_E4E0_0000, (8u64 << 30) - 0x1_E4E0_0000));
        assert_eq!(u64_at(&b, v::SI_BAR1_PDE_BASE), 0x1_E8E0_1000);
        assert!(labels
            .iter()
            .any(|l| l.field == "fbRegionInfoParams" && l.source.contains("BAR1 pool")));
    }

    #[test]
    fn static_info_layout() {
        let f = crate::fixture::rtx2070();
        let id = f.identity(4 << 30);
        assert_eq!(id.gpu_name, "Leandro VFIO RTX 2070-4G");
        assert_eq!(
            StaticIdentity::guest_name("RTX 2070", 130 << 20),
            "Leandro VFIO RTX 2070-130M"
        );
        let spec = StaticIdentity::spec("RTX 2070", 4 << 30);
        assert_eq!(spec.guest_name(NameFormat::Legacy), "Leandro RTX 2070-4G");
        let t = StaticIdentity::from_spec(
            f.chip.data(),
            &f.board,
            id.pci,
            &spec,
            NameFormat::Legacy,
            4 << 30,
        );
        assert_eq!(t.gpu_name, "Leandro RTX 2070-4G");
        assert_eq!(t.gid, id.gid, "the name format leaves the GID alone");
        // The seed is the legacy name, so the default switch kept the guest UUID
        // every recorded run shows (GPU-9bd7b93d-3d23-c074-6704-984ef33ab7a7).
        assert_eq!(
            id.gid,
            StaticIdentity::synthetic_gid("Leandro RTX 2070-4G", 4 << 30)
        );
        assert_eq!(
            id.gid,
            [
                0x9b, 0xd7, 0xb9, 0x3d, 0x3d, 0x23, 0xc0, 0x74, 0x67, 0x04, 0x98, 0x4e, 0xf3, 0x3a,
                0xb7, 0xa7
            ]
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
        assert_eq!(cstr(&b, 0x548, 64), "Leandro VFIO RTX 2070-4G");
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
