// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The display engine in the GSP role: phases A and B of docs/caraxes/display-connector.md
//! with the DisplayPort decision D15 (docs/caraxes/display-ab.md, display-adapter.md).
//!
//! What is here:
//! - the display's presence (the fuse `NV_FUSE_STATUS_OPT_DISPLAY`, the VGA workspace
//!   register) and the kernel's init RPCs (IP version, static info, instance memory);
//! - the NV0073/NV5070/NVC370/NVC372 controls NVKMS and its DP library send for one
//!   DisplayPort SST connector per head, answered from the chip data
//!   (`chips/<chip>/display.toml`), the board's connectors (`boards/<board>.toml`
//!   `[display]`) and one [`dp_sink::DpSink`] per connector ([`aux`] adapts the NV0073
//!   AUX encoding onto it);
//! - the BAR0 pages of the channels (USERD PUT/GET, the core channel's ARMED state
//!   mirror, cursor `Free`) and the NVC573 capabilities page.
//!
//! Nothing here is forwarded to the host: every answer is `synthesized` (chip data),
//! `board` (the connector table) or `guest` (derived from what the guest wrote), D9.
//! Phase C (docs/caraxes/display-cd.md): the channels' methods go to [`engine`], which
//! applies core `UPDATE`s (heads, window owners, notifier), queues window presentations
//! and latches them on a per-head clock that also raises vblank.

pub mod aux;
pub mod edid;
pub mod engine;
pub mod chan_page;
pub mod fe_page;

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::Deserialize;

use crate::ctrl::Answer;
use crate::fb::FakeFb;
use crate::guestmem::GuestMemory;
use crate::ver::common::display as d;
use crate::ver::common::{NV_ERR_NOT_SUPPORTED, NV_OK};
use dp_sink::{DpSink, LinkConfig, LinkRate, SinkProfile};

/// `NV2080_NOTIFIERS_HOTPLUG` (cl2080_notification.h).
pub const NV2080_NOTIFIERS_HOTPLUG: u32 = 1;

/// `NV_ERR_INVALID_ARGUMENT` (nvstatuscodes.h).
pub const NV_ERR_INVALID_ARGUMENT: u32 = 0x1F;

/// BAR0 registers and pages (disp/v03_00/dev_disp.h, v04_00/dev_disp.h, gm107/dev_fuse.h).
pub const NV_FUSE_STATUS_OPT_DISPLAY: u64 = 0x0002_1C04;
pub const NV_PDISP_VGA_WORKSPACE_BASE: u64 = 0x0062_5F04;
pub const NV_PDISP_FE_SW: u64 = 0x0064_0000;
pub const CAPS_PAGE_SIZE: u64 = 0x1000;
pub const CORE_USERD: u64 = 0x0068_0000;
pub const CORE_ARMED: u64 = 0x0068_8000;
pub const WIN_USERD: u64 = 0x0069_0000;
pub const CURSOR_USERD: u64 = 0x006D_8000;
pub const USERD_END: u64 = 0x006E_0000;
/// `NVC57A_FREE` (+0x08 of the cursor PIO page): space for methods; must be non-zero.
const CURSOR_FREE: u64 = 0x08;
/// A value for `Free`: the PIO FIFO is empty (clc57a.h: `Free` counts dwords).
const CURSOR_FREE_ENTRIES: u32 = 0x40;

/// Who configured the display (main.rs `--display`); read by every new [`Display`], so
/// a device reset keeps the choice.
#[derive(Clone, Debug, Default)]
pub struct Setup {
    /// The display engine exists: the fuse reads 0 and the controls are answered.
    pub enabled: bool,
    /// The chip whose `display.toml` applies.
    pub chip: String,
    /// The board's `[display]` table.
    pub board: Option<toml::Value>,
}

static SETUP: Mutex<Option<Setup>> = Mutex::new(None);

/// Configure the display of every device model created from now on.
pub fn configure(setup: Setup) {
    *SETUP.lock().unwrap() = Some(setup);
}

static INTR: Mutex<Option<std::sync::Arc<crate::intr::IntrShared>>> = Mutex::new(None);

/// The interrupt path the display raises its vector through (vblank, AWAKEN), from the
/// server thread and from the ticker thread (engine.rs). The boot model's interrupt
/// tree folds it in ([`intr_shared`]).
pub fn set_intr_shared(shared: std::sync::Arc<crate::intr::IntrShared>) {
    *INTR.lock().unwrap() = Some(shared);
}

static FE_PAGE: Mutex<Option<std::sync::Arc<fe_page::FePage>>> = Mutex::new(None);

/// The display page (`CARAXES_DISPLAY_PAGE=1`, fe_page.rs): every display engine created
/// from now on (a device reset makes a new one) keeps its interrupt status there.
pub fn set_fe_page(page: std::sync::Arc<fe_page::FePage>) {
    *FE_PAGE.lock().unwrap() = Some(page);
}

/// The display page, when on.
pub fn fe_page() -> Option<std::sync::Arc<fe_page::FePage>> {
    FE_PAGE.lock().unwrap().clone()
}

static CHAN_PAGE: Mutex<Option<std::sync::Arc<chan_page::ChanPage>>> = Mutex::new(None);

/// The window channels' USERD pages as memory (`CARAXES_DISPLAY_KICK_PAGE=1`,
/// chan_page.rs): every display from now on shows its channels' `PUT` and `GET` there.
pub fn set_chan_page(page: std::sync::Arc<chan_page::ChanPage>) {
    *CHAN_PAGE.lock().unwrap() = Some(page);
}

/// The kick page, when on.
pub fn chan_page() -> Option<std::sync::Arc<chan_page::ChanPage>> {
    CHAN_PAGE.lock().unwrap().clone()
}

/// The file the display polls for host-side connector commands (engine.rs).
pub fn set_control_file(path: std::path::PathBuf) {
    let vector = SETUP
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|s| chip_display(&s.chip).ok().flatten())
        .map_or(0, |c| c.gsp_vector);
    engine::ticker().set_control(path, vector);
}

/// The display's interrupt path when the display is on (for the interrupt tree).
pub fn intr_shared() -> Option<std::sync::Arc<crate::intr::IntrShared>> {
    let on = SETUP.lock().unwrap().as_ref().is_some_and(|s| s.enabled);
    if on {
        INTR.lock().unwrap().clone()
    } else {
        None
    }
}

/// `chips/<chip>/display.toml`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChipDisplay {
    pub label: String,
    pub ip_version: u32,
    pub heads: u32,
    pub windows: u32,
    pub sors: u32,
    pub num_disp_channels: u32,
    pub i2c_port: u32,
    pub vga_workspace_base: u32,
    pub intr_vector: u32,
    /// The GSP stall vector (`chips/<chip>/interrupts.toml`): raised after the device
    /// posts a hotplug event on the status queue.
    pub gsp_vector: u32,
    /// Per-head pixel clock cap (HEAD_CLK_CAP, display-scaling.md section 2.2), kHz.
    pub head_clk_cap_khz: u32,
    pub link: LinkProfile,
    pub caps: Vec<CapReg>,
}

/// The one DisplayPort link profile every link answer derives from.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkProfile {
    pub rate: String,
    pub lanes: u8,
    pub dsc: bool,
    pub fec: bool,
}

impl LinkProfile {
    pub fn rate(&self) -> LinkRate {
        match self.rate.as_str() {
            "RBR" => LinkRate::Rbr,
            "HBR" => LinkRate::Hbr,
            "HBR2" => LinkRate::Hbr2,
            _ => LinkRate::Hbr3,
        }
    }

    /// `NV0073_CTRL_CMD_DP_GET_CAPS_MAX_LINK_RATE_*` and `DFP_FLAGS_DP_LINK_BW_*`
    /// (both 1 = 1.62 .. 4 = 8.10 Gbit/s).
    pub fn nv_rate_code(&self) -> u32 {
        match self.rate() {
            LinkRate::Rbr => 1,
            LinkRate::Hbr => 2,
            LinkRate::Hbr2 => 3,
            LinkRate::Hbr3 => 4,
        }
    }

    /// The sink profile of a monitor that matches this link (dp-sink).
    pub fn sink_profile(&self) -> SinkProfile {
        let mut p = SinkProfile::dp14_hbr3_x4_dsc();
        p.max_rate = self.rate();
        p.max_lanes = self.lanes;
        if !self.dsc {
            p.dsc = None;
        }
        p.fec = self.fec;
        p
    }

    /// Uncompressed pixel-clock budget at 24 bpp, in kHz (8b/10b payload / 24).
    pub fn pclk_budget_khz(&self) -> u32 {
        (self.rate().mbps() as u64 * self.lanes as u64 * 8 / 10 * 1000 / 24) as u32
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapReg {
    pub offset: u32,
    pub name: String,
    pub value: u32,
}

/// `boards/<board>.toml` `[display]`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardDisplay {
    pub label: String,
    pub connector: Vec<ConnectorFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorFile {
    /// One bit: the NV0073 displayId.
    pub display_id: u32,
    pub sor: u32,
    /// The connector's physical index (`GET_CONNECTOR_DATA`, the DCB index).
    pub index: u32,
    /// Only "DP" (D15).
    pub kind: String,
}

const TU106_DISPLAY: &str = include_str!("../../chips/tu106/display.toml");

/// The chip's display data, if the chip has any.
pub fn chip_display(chip: &str) -> Result<Option<ChipDisplay>, String> {
    let text = match chip {
        "tu106" => TU106_DISPLAY,
        _ => return Ok(None),
    };
    toml::from_str(text)
        .map(Some)
        .map_err(|e| format!("chips/{chip}/display.toml: {e}"))
}

/// One emulated connector and the per-display state the adapter keeps
/// (display-adapter.md section 2.3).
pub struct Connector {
    pub display_id: u32,
    pub sor: u32,
    pub index: u32,
    pub sink: DpSink,
    /// `SET_EDID_V2` with a non-zero size; cleared by size 0.
    pub override_edid: Option<Vec<u8>>,
    /// What the last `GET_EDID_V2` returned (`COPY_CACHE_YES`).
    pub last_answer: Option<Vec<u8>>,
    /// Hotplug masks not yet read through `INTERNAL_GET_HOTPLUG_UNPLUG_STATE`.
    pub pending_plug: bool,
    pub pending_unplug: bool,
    /// `DP_SET_WAR_FLAGS` `SW_AUTO_READ_ENABLE` (615 on): the DP library asks for software
    /// DPCD auto-read on a DP-tunneling port. The AUX model reads in software anyway, so
    /// the flag is only recorded.
    pub sw_auto_read_war: bool,
}

/// The kicks of each window channel whose methods the trace lists (twice as many of the core
/// channel's): the setup and the first presentations, which differ between guest drivers
/// (NVKMS, the Windows KMD).
const WINDOW_TRACE_KICKS: u32 = 8;

/// A display channel: its pushbuffer (from `INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER`) and
/// its USERD page.
#[derive(Clone, Copy, Debug, Default)]
pub struct Channel {
    pub hclass: u32,
    pub instance: u32,
    pub pb: u64,
    pub pb_limit: u64,
    pub pb_aspace: u32,
    pub valid: bool,
    pub put: u32,
    pub get: u32,
    pub kicks: u32,
    pub methods: u32,
}

/// The display state of one device.
pub struct Display {
    pub enabled: bool,
    pub chip: Option<ChipDisplay>,
    pub board_label: String,
    pub connectors: Vec<Connector>,
    /// `INTERNAL_DISPLAY_WRITE_INST_MEM`: base, size, address space.
    pub inst_mem: Option<(u64, u64, u32)>,
    /// Channels by USERD page offset in BAR0.
    pub channels: BTreeMap<u64, Channel>,
    /// Core channel method state: assembly (written) and armed (after `UPDATE`).
    pub core_assembly: BTreeMap<u32, u32>,
    pub core_armed: BTreeMap<u32, u32>,
    /// Latched writes to display pages without a model.
    latched: BTreeMap<u64, u32>,
    /// The AUX transcript in dp-sink's format (`aux::record_line`).
    pub aux_transcript: Vec<String>,
    /// Link configuration the last DP_CTRL trained (rate code, lanes).
    pub link: Option<(u8, u8)>,
    /// FEC on the trained link (`DP_CONFIGURE_FEC` after training; cleared when the link
    /// is trained again or powered down, ctrl0073dp.h:356-360).
    pub fec: bool,
    pub notes: Vec<String>,
    /// NV2080_NOTIFIERS_HOTPLUG events the guest allocated (hClient, hEvent), for
    /// POST_EVENT (docs/caraxes/display-cd.md, phase D).
    pub hotplug_events: Vec<(u32, u32)>,
    /// Phase C: methods, heads, the clock and the interrupt registers (engine.rs).
    pub engine: engine::Engine,
}

fn put32(b: &mut [u8], off: usize, v: u32) {
    if off + 4 <= b.len() {
        b[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
}

fn get32(b: &[u8], off: usize) -> u32 {
    b.get(off..off + 4)
        .map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

/// (offset, size) of `path` in the version's generated layout of `params`
/// (src/ver/v<major>/ctrl_layouts.rs; the tests check every one the display uses).
fn field(params: &str, path: &str) -> (usize, usize) {
    crate::ver::Abi::active()
        .layout(params)
        .ok_or_else(|| format!("no generated layout of {params}"))
        .and_then(|l| l.resolve(path))
        .unwrap_or_else(|e| panic!("{e}"))
}

fn get_field(b: &[u8], (off, size): (usize, usize)) -> u64 {
    let mut w = [0u8; 8];
    if let Some(s) = b.get(off..off + size) {
        w[..size].copy_from_slice(s);
    }
    u64::from_le_bytes(w)
}

fn put_field(b: &mut [u8], (off, size): (usize, usize), v: u64) {
    if off + size <= b.len() {
        b[off..off + size].copy_from_slice(&v.to_le_bytes()[..size]);
    }
}

fn get64(b: &[u8], off: usize) -> u64 {
    b.get(off..off + 8)
        .map_or(0, |s| u64::from_le_bytes(s.try_into().unwrap()))
}

fn ok(params: Vec<u8>, label: &'static str, note: impl Into<String>) -> Answer {
    Answer {
        status: NV_OK,
        params,
        label,
        note: note.into(),
    }
}

fn fail(status: u32, params: Vec<u8>, label: &'static str, note: impl Into<String>) -> Answer {
    Answer {
        status,
        params,
        label,
        note: note.into(),
    }
}

const SYNTH: &str = "synthesized (chips/tu106/display.toml)";
const BOARD: &str = "synthesized (board connector table, boards/rtx2070.toml [display])";
const ACK: &str = "synthesized (acknowledged; no display state behind it yet)";
const GUEST: &str = "guest (recorded from the request)";
const SINK: &str = "synthesized (dp-sink model behind the connector)";
const LINK: &str = "synthesized (the display.toml [link] profile)";
const EDID_STATE: &str = "guest-derived (per-display EDID state: sink EDID and override)";
const HDCP: &str = "synthesized (connector without HDCP: not capable, not authenticated)";
const ENGINE: &str = "synthesized (display engine: no merge pair, no scan or flip lock)";

impl Default for Display {
    fn default() -> Self {
        Self::new()
    }
}

impl Display {
    /// A display as [`configure`] set it up (disabled when nothing was configured).
    pub fn new() -> Self {
        let setup = SETUP.lock().unwrap().clone().unwrap_or_default();
        Self::from_setup(&setup).unwrap_or_else(|e| {
            log::error!("display: {e}; display disabled");
            Self::disabled()
        })
    }

    pub fn disabled() -> Self {
        Display {
            enabled: false,
            chip: None,
            board_label: String::new(),
            connectors: Vec::new(),
            inst_mem: None,
            channels: BTreeMap::new(),
            core_assembly: BTreeMap::new(),
            core_armed: BTreeMap::new(),
            latched: BTreeMap::new(),
            aux_transcript: Vec::new(),
            link: None,
            fec: false,
            notes: Vec::new(),
            hotplug_events: Vec::new(),
            engine: engine::Engine::new(0, 0),
        }
    }

    pub fn from_setup(setup: &Setup) -> Result<Self, String> {
        let mut disp = Self::disabled();
        if !setup.enabled {
            return Ok(disp);
        }
        let chip = chip_display(&setup.chip)?
            .ok_or_else(|| format!("chip {} has no display data", setup.chip))?;
        let board: BoardDisplay = match &setup.board {
            Some(v) => v
                .clone()
                .try_into()
                .map_err(|e| format!("board [display]: {e}"))?,
            None => return Err("the board has no [display] table".into()),
        };
        let profile = chip.link.sink_profile();
        profile
            .validate()
            .map_err(|e| format!("link profile: {e:?}"))?;
        for c in &board.connector {
            if c.kind != "DP" || c.display_id.count_ones() != 1 || c.sor >= chip.sors {
                return Err(format!("unsupported connector {c:?}"));
            }
            let mut sink = DpSink::new(profile.clone(), edid::monitor_edid())
                .map_err(|e| format!("sink: {e:?}"))?;
            // CARAXES_DISPLAY_UNPLUGGED=1 (diagnostic): the connectors start with no monitor.
            if !std::env::var("CARAXES_DISPLAY_UNPLUGGED").is_ok_and(|v| v == "1") {
                sink.plug();
            }
            sink.take_events();
            disp.connectors.push(Connector {
                display_id: c.display_id,
                sor: c.sor,
                index: c.index,
                sink,
                override_edid: None,
                last_answer: None,
                pending_plug: false,
                pending_unplug: false,
                sw_auto_read_war: false,
            });
        }
        disp.enabled = true;
        disp.engine = engine::Engine::new(chip.heads as usize, chip.windows as usize);
        if let Some(p) = fe_page() {
            disp.engine.attach_page(p);
        }
        if let Some(i) = intr_shared() {
            engine::ticker().attach(i, chip.intr_vector);
        }
        disp.board_label = board.label;
        disp.chip = Some(chip);
        Ok(disp)
    }

    fn chip(&self) -> &ChipDisplay {
        self.chip.as_ref().expect("enabled display has chip data")
    }

    fn supported_mask(&self) -> u32 {
        self.connectors.iter().fold(0, |m, c| m | c.display_id)
    }

    fn connected_mask(&self) -> u32 {
        self.connectors
            .iter()
            .filter(|c| c.sink.is_plugged())
            .fold(0, |m, c| m | c.display_id)
    }

    fn connector(&mut self, display_id: u32) -> Option<&mut Connector> {
        self.connectors
            .iter_mut()
            .find(|c| c.display_id == display_id)
    }

    // -----------------------------------------------------------------------------
    // Controls.

    /// Answer a display control, or `None` when `cmd` is not the display's (or the
    /// display is switched off: then the controls stay unanswered, as before phase A).
    pub fn control(&mut self, cmd: u32, params: &[u8]) -> Option<Answer> {
        fn win_display_queries() -> bool {
            std::env::var("CARAXES_WIN_DISPLAY_QUERIES").is_ok_and(|v| v == "1")
        }
        const SYSTEM_GET_HOTPLUG_CONFIG: u32 = 0x0073_0109;
        const WIN_MUX: u32 = 0x0073_013D;
        const WIN_SIDEBAND_I2C: u32 = 0x0073_014B;
        const WIN_ACPI_DOD_ATTACHMENT: u32 = 0x0073_0285;
        if !self.enabled {
            return None;
        }
        let n = params.len();
        let mut b = params.to_vec();
        let did = get32(params, d::DISPLAY_ID);
        let a = match cmd {
            // ---- kernel init (phase A)
            d::CMD_INTERNAL_DISPLAY_GET_IP_VERSION => {
                put32(&mut b, 0, self.chip().ip_version);
                ok(
                    b,
                    SYNTH,
                    format!("display ip version {:#x}", self.chip().ip_version),
                )
            }
            d::CMD_INTERNAL_DISPLAY_GET_STATIC_INFO => {
                let c = self.chip().clone();
                let mut b = vec![0u8; n];
                put32(&mut b, d::STATIC_INFO_FE_HW_SYS_CAP, (1 << c.heads) - 1);
                put32(
                    &mut b,
                    d::STATIC_INFO_WINDOW_PRESENT_MASK,
                    (1 << c.windows) - 1,
                );
                put32(&mut b, d::STATIC_INFO_NUM_HEADS, c.heads);
                put32(&mut b, d::STATIC_INFO_I2C_PORT, c.i2c_port);
                put32(
                    &mut b,
                    d::STATIC_INFO_NUM_DISP_CHANNELS,
                    c.num_disp_channels,
                );
                ok(
                    b,
                    SYNTH,
                    format!(
                        "display static info: {} heads, windows {:#x}, i2cPort {}, {} channels, no internal panel",
                        c.heads,
                        (1u32 << c.windows) - 1,
                        c.i2c_port,
                        c.num_disp_channels
                    ),
                )
            }
            d::CMD_INTERNAL_INIT_BRIGHTC_STATE_LOAD => ok(
                b,
                ACK,
                "brightc state load: no backlight (no internal panel)",
            ),
            d::CMD_INTERNAL_SET_STATIC_EDID_DATA => ok(
                b,
                ACK,
                format!("static (ACPI) EDID data: tableLen {}", get32(params, 0)),
            ),
            d::CMD_INTERNAL_DISPLAY_SET_IMP_INIT_INFO => ok(b, ACK, "imp init info"),
            d::CMD_INTERNAL_DISPLAY_WRITE_INST_MEM => {
                let (pa, size, aspace) = (
                    get64(params, d::WRITE_INST_MEM_PHYS_ADDR),
                    get64(params, d::WRITE_INST_MEM_SIZE_FIELD),
                    get32(params, d::WRITE_INST_MEM_ADDR_SPACE),
                );
                self.inst_mem = Some((pa, size, aspace));
                self.engine.inst = Some(pa);
                ok(
                    b,
                    GUEST,
                    format!(
                        "display instance memory: pa {pa:#x} size {size:#x} addrSpace {aspace}"
                    ),
                )
            }
            d::CMD_SYSTEM_MAP_SHARED_DATA => ok(
                b,
                GUEST,
                format!(
                    "map shared data: base {:#x} size {:#x} bMap {}",
                    get64(params, 0),
                    get64(params, 8),
                    params.get(36).copied().unwrap_or(0)
                ),
            ),
            d::CMD_INTERNAL_DISPLAY_PRE_UNIX_CONSOLE
            | d::CMD_INTERNAL_DISPLAY_POST_UNIX_CONSOLE
            | d::CMD_INTERNAL_DISPLAY_SETUP_RG_LINE_INTR
            | d::CMD_INTERNAL_DISPLAY_ACPI_SUBSYSTEM_ACTIVATED
            | d::CMD_INTERNAL_DISPLAY_PRE_MODESET
            | d::CMD_INTERNAL_DISPLAY_POST_MODESET => {
                ok(b, ACK, format!("internal display control {cmd:#x}"))
            }
            // `{displayMask, numHeads}` (ctrl2080internal.h:1544-1548). `numHeads` is the head
            // count, not the lit heads: the kernel's DispSw (GF100_DISP_SW, disp_sw.c:86)
            // refuses a logical head at or above it with INSUFFICIENT_RESOURCES, and the
            // NVIDIA GL/EGL driver allocates one per head (display-e.md). The mask names
            // the connected displays.
            d::CMD_INTERNAL_DISPLAY_GET_ACTIVE_DISPLAY_DEVICES => {
                let mut b = vec![0u8; n];
                let (mask, heads) = (self.connected_mask(), self.chip().heads);
                put32(&mut b, 0, mask);
                put32(&mut b, 4, heads);
                ok(b, SYNTH, format!("active display devices: mask {mask:#x}, {heads} heads"))
            }
            d::CMD_INTERNAL_DISPLAY_CHANNEL_PUSHBUFFER => {
                let ch = Channel {
                    hclass: get32(params, d::PUSHBUFFER_HCLASS),
                    instance: get32(params, d::PUSHBUFFER_CHANNEL_INSTANCE),
                    pb: get64(params, d::PUSHBUFFER_PHYSICAL_ADDR),
                    pb_limit: get64(params, d::PUSHBUFFER_LIMIT),
                    pb_aspace: get32(params, d::PUSHBUFFER_ADDRESS_SPACE),
                    valid: params.get(d::PUSHBUFFER_VALID).copied().unwrap_or(0) != 0,
                    ..Channel::default()
                };
                let note = format!(
                    "channel pushbuffer: class {:#x} instance {} pa {:#x} limit {:#x} addrSpace {} valid {} pbSize {:#x}",
                    ch.hclass,
                    ch.instance,
                    ch.pb,
                    ch.pb_limit,
                    ch.pb_aspace,
                    ch.valid,
                    get32(params, d::PUSHBUFFER_CHANNEL_PB_SIZE)
                );
                if let Some(base) = userd_base(ch.hclass, ch.instance) {
                    if let Some(p) = chan_page() {
                        p.set_channel(base, ch.put, ch.get);
                    }
                    self.channels.insert(base, ch);
                }
                ok(b, GUEST, note)
            }

            // ---- NV0073 system
            d::CMD_SYSTEM_GET_CAPS_V2 => {
                let mut b = vec![0u8; n];
                let (byte, bit) = d::SYSTEM_CAPS_CROSS_BAR_SUPPORTED;
                if byte < n {
                    b[byte] |= bit;
                }
                ok(b, SYNTH, "system caps: CROSS_BAR_SUPPORTED")
            }
            d::CMD_SYSTEM_GET_NUM_HEADS => {
                put32(&mut b, d::GET_NUM_HEADS_NUM_HEADS, self.chip().heads);
                ok(b, SYNTH, format!("num heads {}", self.chip().heads))
            }
            d::CMD_SYSTEM_GET_SUPPORTED => {
                let m = self.supported_mask();
                put32(&mut b, d::GET_SUPPORTED_DISPLAY_MASK, m);
                put32(&mut b, d::GET_SUPPORTED_DISPLAY_MASK_DDC, 0);
                ok(b, BOARD, format!("supported displays {m:#x}"))
            }
            d::CMD_SYSTEM_GET_CONNECT_STATE => {
                let asked = get32(params, d::CONNECT_STATE_DISPLAY_MASK);
                let asked = if asked == 0 {
                    self.supported_mask()
                } else {
                    asked
                };
                let m = asked & self.connected_mask();
                put32(&mut b, d::CONNECT_STATE_DISPLAY_MASK, m);
                put32(&mut b, d::CONNECT_STATE_RETRY_TIME_MS, 0);
                ok(
                    b,
                    SINK,
                    format!("connect state: asked {asked:#x} connected {m:#x}"),
                )
            }
            // NV0073_CTRL_CMD_SYSTEM_GET_HOTPLUG_CONFIG (0x730109, ctrl0073system.h:
            // {subDeviceInstance, flags, hotplugEventMask, hotplugPollable,
            // hotplugInterruptible, hotplugAlwaysAttached}): the Windows KMD asks it at adapter
            // start (WINDOWS-LOOP.md Y13). Every connector is a DisplayPort sink: hot-plug
            // interruptible and pollable, no events requested yet, none always attached.
            SYSTEM_GET_HOTPLUG_CONFIG => {
                let m = self.supported_mask();
                b.resize(n.max(24), 0);
                put32(&mut b, 8, 0);
                put32(&mut b, 12, m);
                put32(&mut b, 16, m);
                put32(&mut b, 20, 0);
                b.truncate(n);
                ok(b, BOARD, format!("hotplug config: pollable and interruptible {m:#x}"))
            }
            d::CMD_SYSTEM_GET_HEAD_ROUTING_MAP => {
                // Echo the request when every display is ours and there are heads for
                // them; head i drives the i-th display (4 bits per display).
                let asked = get32(params, d::HEAD_ROUTING_DISPLAY_MASK);
                let fits =
                    asked & !self.supported_mask() == 0 && asked.count_ones() <= self.chip().heads;
                let mut map = 0u32;
                if fits {
                    for (i, bit) in (0..32).filter(|b| asked & (1 << b) != 0).enumerate() {
                        let _ = bit;
                        map |= (i as u32) << (4 * i);
                    }
                }
                put32(
                    &mut b,
                    d::HEAD_ROUTING_DISPLAY_MASK,
                    if fits { asked } else { 0 },
                );
                put32(&mut b, d::HEAD_ROUTING_MAP, map);
                ok(
                    b,
                    BOARD,
                    format!("head routing map: {asked:#x} -> map {map:#x} (fits {fits})"),
                )
            }
            // SYSTEM_GET_ACTIVE (ctrl0073system.h:611-678): the displayId driven by `head`,
            // "setup at system boot by low-level software or later by an NV client", 0 for
            // none; a head outside the chip's heads is an invalid argument. No head is lit
            // by firmware here, so the input flag CLIENT (only heads an NV client set up)
            // gives the same answer, and no head is stale: from 615 on the output flag
            // GOP_STALE_HEAD_NEEDS_CLEANUP stays NO and `staleSorIndex` 0 (ver/v615/display.rs).
            // The driving SOR is the one whose ARMED owner mask names the head (engine.rs
            // `head_sor`), and the display is the connector on that SOR.
            d::CMD_SYSTEM_GET_ACTIVE => {
                let head = get32(params, d::GET_ACTIVE_HEAD);
                if head as usize >= self.engine.heads.len() {
                    return Some(fail(
                        NV_ERR_INVALID_ARGUMENT,
                        b,
                        SYNTH,
                        format!(
                            "head {head} active: not a head of this chip ({} heads; the header's INVALID_ARGUMENT)",
                            self.engine.heads.len()
                        ),
                    ));
                }
                let sor = self.engine.head_sor(head as usize);
                let did = sor
                    .and_then(|s| self.connectors.iter().find(|c| c.sor == s))
                    .map_or(0, |c| c.display_id);
                put32(&mut b, d::GET_ACTIVE_DISPLAY_ID, did);
                let l = &crate::ver::layouts().display;
                let flags = get32(params, d::GET_ACTIVE_FLAGS) & !l.get_active_flags_gop_stale_head;
                put32(&mut b, d::GET_ACTIVE_FLAGS, flags);
                if let Some(o) = l.get_active_stale_sor_index {
                    put32(&mut b, o, 0);
                }
                let what = match (sor, did) {
                    (Some(s), 0) => format!("none (SOR{s} drives it, no connector on that SOR)"),
                    (Some(s), d) => format!("{d:#x} (SOR{s}, set up by a client)"),
                    (None, _) => "none (idle; no firmware-lit head)".into(),
                };
                ok(b, SYNTH, format!("head {head} active: {what}"))
            }
            d::CMD_SYSTEM_GET_INTERNAL_DISPLAYS => {
                ok(vec![0u8; n], BOARD, "internal displays: none")
            }
            d::CMD_SYSTEM_GET_BOOT_DISPLAYS => ok(vec![0u8; n], BOARD, "boot displays: none"),
            d::CMD_SYSTEM_GET_SCANLINE => {
                let head = get32(params, 4) as usize;
                let line = self.engine.scanline(head, engine::now_ns());
                put32(&mut b, d::GET_SCANLINE_CURRENT, line);
                ok(b, SYNTH, format!("scanline of head {head}: {line} (device clock)"))
            }
            d::CMD_SYSTEM_INTERNAL_ALLOCATE_DISPLAY_BANDWIDTH => {
                ok(b, ACK, "allocate display bandwidth")
            }
            d::CMD_INTERNAL_GET_HOTPLUG_UNPLUG_STATE => {
                let (mut plug, mut unplug) = (0u32, 0u32);
                for c in &mut self.connectors {
                    if std::mem::take(&mut c.pending_plug) {
                        plug |= c.display_id;
                    }
                    if std::mem::take(&mut c.pending_unplug) {
                        unplug |= c.display_id;
                    }
                }
                put32(&mut b, d::HOTPLUG_UNPLUG_PLUG_MASK, plug);
                put32(&mut b, d::HOTPLUG_UNPLUG_UNPLUG_MASK, unplug);
                ok(
                    b,
                    SINK,
                    format!("hotplug/unplug state: plug {plug:#x} unplug {unplug:#x} (cleared)"),
                )
            }

            // ---- NV0073 specific
            d::CMD_SPECIFIC_GET_I2C_PORTID => {
                put32(&mut b, d::I2C_PORTID_COMM_PORT_ID, 0);
                put32(&mut b, d::I2C_PORTID_DDC_PORT_ID, 0);
                ok(
                    b,
                    BOARD,
                    format!("i2c port id of {did:#x}: ddcPortId 0 (I2C_PORT_NONE)"),
                )
            }
            d::CMD_SPECIFIC_GET_TYPE => {
                if self.supported_mask() & did == 0 {
                    return Some(self.bad_display(cmd, b, did));
                }
                put32(&mut b, d::GET_TYPE_DISPLAY_TYPE, d::DISPLAY_TYPE_DFP);
                ok(b, BOARD, format!("display type of {did:#x}: DFP"))
            }
            d::CMD_SPECIFIC_OR_GET_INFO => {
                let Some(c) = self.connector(did) else {
                    return Some(self.bad_display(cmd, b, did));
                };
                let (sor, index) = (c.sor, c.index);
                put32(&mut b, d::OR_INFO_INDEX, sor);
                put32(&mut b, d::OR_INFO_TYPE, d::OR_TYPE_SOR);
                put32(&mut b, d::OR_INFO_PROTOCOL, d::OR_PROTOCOL_SOR_DP_A);
                put32(&mut b, d::OR_INFO_LOCATION, d::OR_LOCATION_CHIP);
                put32(&mut b, d::OR_INFO_DCB_INDEX, index);
                if b.len() > d::OR_INFO_IS_DISP_DYNAMIC {
                    b[d::OR_INFO_IS_LIT_BY_VBIOS] = 0;
                    b[d::OR_INFO_IS_DISP_DYNAMIC] = 0;
                }
                ok(
                    b,
                    BOARD,
                    format!("or info of {did:#x}: SOR{sor} DP_A, chip, static"),
                )
            }
            d::CMD_SPECIFIC_GET_CONNECTOR_DATA => {
                let Some(c) = self.connector(did) else {
                    return Some(self.bad_display(cmd, b, did));
                };
                let index = c.index;
                let mut b = params.to_vec();
                put32(&mut b, 8, 0);
                put32(&mut b, d::CONNECTOR_DATA_DDC_PARTNERS, 0);
                put32(&mut b, d::CONNECTOR_DATA_COUNT, 1);
                let e = d::CONNECTOR_DATA_DATA;
                put32(&mut b, e, index);
                put32(&mut b, e + 4, d::CONNECTOR_TYPE_DP_EXT);
                put32(&mut b, e + 8, 0);
                put32(&mut b, d::CONNECTOR_DATA_PLATFORM, 0);
                ok(
                    b,
                    BOARD,
                    format!("connector data of {did:#x}: 1 connector, DP_EXT, index {index}"),
                )
            }
            d::CMD_SPECIFIC_GET_HDCP_STATE => {
                put32(&mut b, d::HDCP_STATE_FLAGS, 0);
                ok(b, SYNTH, "hdcp state: all false")
            }
            d::CMD_SPECIFIC_GET_ALL_HEAD_MASK => {
                let m = (1u32 << self.chip().heads) - 1;
                put32(&mut b, d::ALL_HEAD_MASK_HEAD_MASK, m);
                ok(b, SYNTH, format!("all head mask {m:#x}"))
            }
            d::CMD_SPECIFIC_GET_VALID_HEAD_WINDOW_ASSIGNMENT => {
                let w = self.chip().windows as usize;
                for i in 0..d::WINDOW_HEAD_MASK_COUNT {
                    if d::WINDOW_HEAD_MASK + i < b.len() {
                        b[d::WINDOW_HEAD_MASK + i] = if i < w { 1 << (i / 2) } else { 0 };
                    }
                }
                ok(
                    b,
                    SYNTH,
                    format!("window head assignment: windows 2h, 2h+1 on head h ({w} windows)"),
                )
            }
            d::CMD_SPECIFIC_GET_PCLK_LIMIT => {
                let k = self.chip().link.pclk_budget_khz();
                put32(&mut b, d::PCLK_LIMIT_PCLK, k);
                put32(&mut b, d::PCLK_LIMIT_OR_PCLK, k);
                put32(&mut b, d::PCLK_LIMIT_VB_PCLK, k);
                ok(
                    b,
                    LINK,
                    format!("pclk limit of {did:#x}: {k} kHz (link budget at 24 bpp)"),
                )
            }
            d::CMD_SPECIFIC_SET_MONITOR_POWER
            | d::CMD_SPECIFIC_DISPLAY_CHANGE
            | d::CMD_DFP_SET_ELD_AUDIO_CAPS
            | d::CMD_DFP_UPDATE_DYNAMIC_DFP_CACHE
            | d::CMD_DFP_SET_AUDIO_ENABLE
            | d::CMD_DP_MAIN_LINK_CTRL
            | d::CMD_DP_SET_AUDIO_MUTESTREAM
            | d::CMD_DP_CONFIG_STREAM
            | d::CMD_DP_SET_MANUAL_DISPLAYPORT
            | d::CMD_DP_NOTIFY_LT
            | d::CMD_DP_SET_MSA_PROPERTIES_V2
            | d::CMD_DP_SET_STEREO_MSA_PROPERTIES
            | d::CMD_SPECIFIC_SET_OD_PACKET => ok(b, ACK, format!("control {cmd:#x} on {did:#x}")),
            d::CMD_SYSTEM_VRR_DISPLAY_INFO => ok(
                b,
                BOARD,
                format!("vrr display info of {did:#x}: not a VRR display (recorded)"),
            ),
            d::CMD_SPECIFIC_IS_DIRECTMODE_DISPLAY => {
                if b.len() > d::DIRECTMODE_IS_DIRECTMODE {
                    b[d::DIRECTMODE_IS_DIRECTMODE] = 0;
                }
                ok(b, SYNTH, "direct mode (VR headset): no")
            }
            d::CMD_SPECIFIC_GET_EDID_V2 => self.get_edid(cmd, b, did),
            d::CMD_SPECIFIC_SET_EDID_V2 => self.set_edid(cmd, b, did),
            d::CMD_SPECIFIC_APPLY_EDID_OVERRIDE_V2 => ok(
                b,
                EDID_STATE,
                "apply edid override: input returned unchanged (no regkey overrides)",
            ),

            // ---- NV0073 DFP and DP
            d::CMD_DFP_GET_INFO => {
                if self.supported_mask() & did == 0 {
                    return Some(self.bad_display(cmd, b, did));
                }
                let flags = d::DFP_FLAGS_SIGNAL_DISPLAYPORT
                    | (d::DFP_FLAGS_LANE_QUAD << d::DFP_FLAGS_LANE_SHIFT)
                    | (self.chip().link.nv_rate_code() << d::DFP_FLAGS_DP_LINK_BW_SHIFT)
                    | (d::DFP_FLAGS_LINK_SINGLE << d::DFP_FLAGS_LINK_SHIFT);
                put32(&mut b, d::DFP_INFO_FLAGS, flags);
                put32(&mut b, d::DFP_INFO_FLAGS + 4, 0);
                ok(
                    b,
                    LINK,
                    format!(
                        "dfp info of {did:#x}: flags {flags:#x} (DP, 4 lanes, link bw code {})",
                        self.chip().link.nv_rate_code()
                    ),
                )
            }
            d::CMD_DFP_ASSIGN_SOR => {
                let Some(c) = self.connector(did) else {
                    return Some(self.bad_display(cmd, b, did));
                };
                let sor = c.sor as usize;
                let sors = self.chip().sors as usize;
                for s in 0..sors.min(4) {
                    let m = if s == sor { did } else { 0 };
                    put32(&mut b, d::ASSIGN_SOR_LIST + 4 * s, m);
                    let t = d::ASSIGN_SOR_LIST_WITH_TAG + d::ASSIGN_SOR_TAG_SIZE * s;
                    put32(&mut b, t, m);
                    put32(&mut b, t + 4, if s == sor { d::SOR_TYPE_SINGLE } else { 0 });
                }
                ok(
                    b,
                    BOARD,
                    format!("assign sor: {did:#x} -> SOR{sor} (single)"),
                )
            }
            d::CMD_DP_GET_CAPS => {
                let l = self.chip().link.clone();
                let mut b = params.to_vec();
                put32(&mut b, d::DP_CAPS_MAX_LINK_RATE, l.nv_rate_code());
                put32(&mut b, d::DP_CAPS_DP_VERSIONS, d::DP_VERSIONS_DP1_2_DP1_4);
                put32(&mut b, 16, 0);
                for o in 20..36 {
                    if o < b.len() {
                        b[o] = 0;
                    }
                }
                if b.len() >= d::DP_CAPS_SIZE {
                    b[d::DP_CAPS_FEC] = l.fec as u8;
                    b[d::DP_CAPS_DOWNSPREAD] = 1;
                    b[d::DP_CAPS_DSC_SUPPORTED] = l.dsc as u8;
                }
                put32(
                    &mut b,
                    d::DP_CAPS_DSC_COLOR_FORMAT_MASK,
                    if l.dsc { 1 } else { 0 },
                );
                put32(
                    &mut b,
                    d::DP_CAPS_DSC_LINE_BUFFER_KB,
                    if l.dsc { 32 } else { 0 },
                );
                put32(
                    &mut b,
                    d::DP_CAPS_DSC_RATE_BUFFER_KB,
                    if l.dsc { 6 } else { 0 },
                );
                put32(
                    &mut b,
                    d::DP_CAPS_DSC_BPP_PRECISION,
                    if l.dsc { 1 } else { 0 },
                );
                put32(
                    &mut b,
                    d::DP_CAPS_DSC_MAX_HZT_SLICES,
                    if l.dsc { 4 } else { 0 },
                );
                put32(
                    &mut b,
                    d::DP_CAPS_DSC_LINE_BUFFER_BIT_DEPTH,
                    if l.dsc { 8 } else { 0 },
                );
                ok(b, LINK, format!("dp caps of SOR{}: max link rate code {} ({}), DP1.2+1.4, no MST, FEC {}, DSC {}", get32(params, 4), l.nv_rate_code(), l.rate, l.fec, l.dsc))
            }
            d::CMD_DP_AUXCH_CTRL => {
                let (a, line) = aux::transact(self, params);
                if let Some(l) = line {
                    self.aux_transcript.push(l);
                }
                a
            }
            d::CMD_DP_CTRL => self.dp_ctrl(cmd, b, did),
            d::CMD_DP_GET_LINK_CONFIG => {
                let (bw, lanes) = self.link.unwrap_or((0, 0));
                put32(&mut b, d::LINK_CONFIG_LANE_COUNT, lanes as u32);
                put32(&mut b, d::LINK_CONFIG_LINK_BW, bw as u32);
                put32(&mut b, d::LINK_CONFIG_DP2_LINK_BW, 0);
                put32(&mut b, d::LINK_CONFIG_FEC_ENABLED, self.fec as u32);
                ok(
                    b,
                    SINK,
                    format!(
                        "link config of {did:#x}: {lanes} lanes, link bw {bw:#x} (0 = untrained), FEC {}",
                        self.fec
                    ),
                )
            }

            // ---- NV5070 / NVC370 / NVC372 on the NVC570 and NVC372 objects
            d::CMD_5070_SYSTEM_GET_CAPS_V2 => {
                let mut b = params.to_vec();
                // CARAXES_WIN_DISP_CAPS=<hex byte> (diagnostic): capsTbl[0] (ctrl5070system.h:40-46,
                // e.g. 0x20 DEEP_COLOR_SUPPORT, 0x40 BUG_644815_DNISO_VIDMEM_ONLY).
                let caps = std::env::var("CARAXES_WIN_DISP_CAPS")
                    .ok()
                    .and_then(|v| u8::from_str_radix(v.trim_start_matches("0x"), 16).ok())
                    .unwrap_or(0);
                if b.len() > d::CAPS_5070_TBL {
                    b[d::CAPS_5070_TBL] = caps;
                }
                ok(b, SYNTH, format!("5070 system caps: {caps:#x}"))
            }
            d::CMD_5070_SET_SOR_FLUSH_MODE | d::CMD_5070_SET_RMFREE_FLAGS => {
                ok(b, ACK, format!("5070 control {cmd:#x}"))
            }
            d::CMD_C370_GET_CHANNEL_INFO => {
                let class = get32(params, 4);
                let inst = get32(params, 8);
                // A window UPDATE is pending until the clock latches it (NVKMS's
                // IsChannelMethodPending, nvkms-evo3.c:6474-6490).
                let latching = class == d::NVC57E_WINDOW_CHANNEL_DMA
                    && self.engine.window_busy(inst as usize);
                let idle = !latching
                    && userd_base(class, inst)
                        .and_then(|base| self.channels.get(&base))
                        .is_none_or(|c| c.get == c.put);
                put32(
                    &mut b,
                    d::CHANNEL_INFO_STATE,
                    if idle { d::CHANNEL_STATE_IDLE } else { 0x40 },
                );
                ok(
                    b,
                    GUEST,
                    format!(
                        "channel info: class {class:#x} instance {inst}: {}",
                        if idle {
                            "IDLE (GET == PUT)"
                        } else if latching {
                            "BUSY (UPDATE waiting for the next tick)"
                        } else {
                            "BUSY"
                        }
                    ),
                )
            }
            d::CMD_C370_IDLE_CHANNEL => ok(b, GUEST, "idle channel: idle (GET == PUT)"),
            d::CMD_C370_GET_LOCKPINS_CAPS => {
                put32(&mut b, d::LOCKPINS_STEREO_PIN, d::LOCKPIN_NONE);
                ok(b, SYNTH, "lockpins caps: no stereo pin")
            }
            d::CMD_C370_GET_ACCL | d::CMD_C370_SET_ACCL => {
                ok(b, ACK, format!("accelerators control {cmd:#x}"))
            }
            d::CMD_C372_IS_MODE_POSSIBLE => self.is_mode_possible(b),
            d::CMD_C372_GET_ACTIVE_VIEWPORT_POINT_IN => ok(b, ACK, "active viewport point in: 0,0"),
            // Features this connector does not have: refused like a GSP without them.
            d::CMD_SPECIFIC_GET_BACKLIGHT_BRIGHTNESS => fail(
                NV_ERR_NOT_SUPPORTED,
                b,
                BOARD,
                format!("backlight brightness of {did:#x}: not supported (external DP monitor, no panel)"),
            ),
            // CARAXES_WIN_DISPLAY_QUERIES=1: three plain queries of the Windows KMD's display
            // start answered NV_OK (WINDOWS-LOOP.md Y21): QUERY_DISPLAY_IDS_WITH_MUX
            // {subDeviceInstance, muxDisplayMask out} mask 0 (no mux), CHECK_SIDEBAND_I2C_SUPPORT
            // {subDeviceInstance, bIsSidebandI2cSupported out} false, and
            // SPECIFIC_GET_ACPI_DOD_DISPLAY_PORT_ATTACHMENT {subDeviceInstance, displayId,
            // dispPortAttachment out} UNKNOWN (a desktop board has no ACPI _DOD).
            c if win_display_queries()
                && matches!(c, WIN_MUX | WIN_SIDEBAND_I2C | WIN_ACPI_DOD_ATTACHMENT) =>
            {
                b.resize(n.max(12), 0);
                let what = match c {
                    WIN_MUX => {
                        put32(&mut b, 4, 0);
                        "display ids with mux: none".to_string()
                    }
                    WIN_SIDEBAND_I2C => {
                        b[4] = 0;
                        "sideband I2C: not supported (false)".to_string()
                    }
                    _ => {
                        put32(&mut b, 8, 0xFFFF_FFFF);
                        format!("ACPI DOD port attachment of {did:#x}: UNKNOWN (no ACPI _DOD)")
                    }
                };
                b.truncate(n);
                ok(b, BOARD, what)
            }
            d::CMD_SYSTEM_QUERY_DISPLAY_IDS_WITH_MUX => fail(
                NV_ERR_NOT_SUPPORTED,
                b,
                BOARD,
                "display ids with mux: not supported (no dynamic mux)",
            ),
            d::CMD_LPWR_DIFR_CTRL => fail(
                NV_ERR_NOT_SUPPORTED,
                b,
                SYNTH,
                "DIFR (display idle frame refresh): not supported",
            ),
            // The category D rows of the display (docs/caraxes/gsp-plan/d-device-model.md 6.5
            // rule 1), the NV2080 and NV9072 ones included: the board has no HDMI, FRL, MST,
            // HDCP, panel, mux, pinsets or DP 2.x, so the rule table's REFUSE and ACK rows are
            // this board's answers. The rows the plan moves to A or E are the chip tables'
            // (chipctl.rs), the MODEL rows without a model stay unanswered.
            // DP_SET_WAR_FLAGS (615 on): the rule table's ACK, the WAR recorded per connector.
            c if crate::ver::layouts().display.cmd_dp_set_war_flags == Some(c) => self.set_war_flags(c, params),
            // The D rows with a model here (roadmap R1): HDCP per sub-command, FEC, RG status.
            c if crate::dmodel::rule(c).is_some_and(|r| crate::dmodel::MODELLED.contains(&r.name)) => {
                self.d_model(c, params)
            }
            c if crate::dmodel::rule(c).is_some_and(|r| matches!(r.reclass, 'A' | 'E')) => {
                return None
            }
            c if crate::dmodel::rule(c).is_some_and(|r| r.group == "disp") => {
                crate::dmodel::answer(c, params).unwrap_or_else(|| {
                    fail(
                        NV_ERR_NOT_SUPPORTED,
                        b,
                        "unanswered",
                        format!("display control {cmd:#x} ({n} bytes): D row without a model yet"),
                    )
                })
            }
            c if is_display_cmd(c) => fail(
                NV_ERR_NOT_SUPPORTED,
                b,
                "unanswered",
                format!(
                    "display control {cmd:#x} ({n} bytes): not modelled; request {}",
                    params.iter().take(48).map(|x| format!("{x:02x}")).collect::<String>()
                ),
            ),
            _ => return None,
        };
        Some(a)
    }

    /// The category D display controls with a model (docs/caraxes/gsp-plan/d-device-model.md
    /// 6.5 rule 3, roadmap R1). Fields are resolved through the version's generated layout
    /// table (`ctrl_layouts.rs`), enum values through the generated D table.
    fn d_model(&mut self, cmd: u32, params: &[u8]) -> Answer {
        let name = crate::dmodel::rule(cmd).map_or("", |r| r.name);
        match name {
            "NV0073_CTRL_CMD_SPECIFIC_HDCP_CTRL" => self.hdcp_ctrl(cmd, params),
            "NV0073_CTRL_CMD_DP_CONFIGURE_FEC" => self.configure_fec(cmd, params),
            "NV5070_CTRL_CMD_GET_RG_STATUS" => self.rg_status(params),
            _ => fail(
                NV_ERR_NOT_SUPPORTED,
                params.to_vec(),
                "unanswered",
                format!("display control {cmd:#x}: no model"),
            ),
        }
    }

    /// `SPECIFIC_HDCP_CTRL` per `cmd` (ctrl0073specific.h:911-1315): the connector has no
    /// HDCP, so it is never authenticated. Disabling or aborting authentication (the DP
    /// library's unplug path, dp_connectorimpl.cpp:8491) succeeds with `err` 0; asking for
    /// a cipher (renegotiate, validate, link status, stream type) succeeds with
    /// `err` UNSUCCESSFUL; the flag, head-config and topology queries return no data (all
    /// outputs 0, `flags` 0: nothing valid). An unknown `cmd` is an invalid argument.
    fn hdcp_ctrl(&mut self, cmd: u32, params: &[u8]) -> Answer {
        const P: &str = "NV0073_CTRL_SPECIFIC_HDCP_CTRL_PARAMS";
        const PREFIX: &str = "NV0073_CTRL_SPECIFIC_HDCP_CTRL_CMD_";
        let abi = crate::ver::Abi::active();
        let mut b = params.to_vec();
        let did = get_field(&b, field(P, "displayId")) as u32;
        if self.connector(did).is_none() {
            return self.bad_display(cmd, b, did);
        }
        let sub = get_field(&b, field(P, "cmd")) as u32;
        let sub_name = abi
            .d_enums
            .iter()
            .find(|(n, v)| n.starts_with(PREFIX) && *v == sub)
            .map_or("?", |(n, _)| &n[PREFIX.len()..]);
        // `err` is a bit field; ERR_UNSUCCESSFUL is bit 0 (ctrl0073specific.h:1290-1292).
        let unsuccessful = abi
            .d_enum("NV0073_CTRL_SPECIFIC_HDCP_CTRL_ERR_UNSUCCESSFUL_YES")
            .unwrap_or(1) as u64;
        let err = field(P, "err");
        let (what, e) = match sub_name {
            "NULL"
            | "DISABLE_AUTHENTICATION"
            | "ABORT_AUTHENTICATION"
            | "FORWARD_KSVLIST_READY" => ("nothing to do, not authenticated", 0),
            "RENEGOTIATE"
            | "VALIDATE_LINK"
            | "READ_LINK_STATUS"
            | "READ_LINK_STATUS_NO_DISPLAY"
            | "SET_TYPE" => ("no HDCP cipher: err UNSUCCESSFUL", unsuccessful),
            "GET_ALL_FLAGS" | "GET_SPECIFIED_FLAGS" | "QUERY_HEAD_CONFIG" | "READ_TOPOLOGY" => {
                let keep = ["subDeviceInstance", "displayId", "cmd"].map(|f| field(P, f));
                let mut z = vec![0u8; b.len()];
                for (off, size) in keep {
                    if off + size <= b.len() {
                        z[off..off + size].copy_from_slice(&b[off..off + size]);
                    }
                }
                b = z;
                ("no HDCP receiver data: every output 0, no flag valid", 0)
            }
            _ => {
                return fail(
                    NV_ERR_INVALID_ARGUMENT,
                    b,
                    HDCP,
                    format!("hdcp ctrl of {did:#x}: unknown cmd {sub:#x}"),
                )
            }
        };
        put_field(&mut b, err, e);
        ok(
            b,
            HDCP,
            format!("hdcp ctrl of {did:#x}: cmd {sub_name} ({sub}): {what}"),
        )
    }

    /// `DP_SET_WAR_FLAGS` (615 ctrl0073dp.h:3734-3773): one DP software workaround per call
    /// on the root port. The only WAR id, `SW_AUTO_READ_ENABLE`, disables the hardware's
    /// automatic DPCD read on a DP-tunneling port in favour of a software read (bug 6102109);
    /// the DP library clears it after every OUI handshake and at every unplug
    /// (dp_connectorimpl.cpp:8048, :8475) and sets it only for one Realtek sink behind a
    /// USB4 tunnel (dp_wardatabase.cpp:130-140). The AUX model has no hardware auto-read, so
    /// the flag is recorded per connector and changes nothing. The status and label are the
    /// D rule table's ACK row (ver/v615/dmodel.rs; dmodel.rs checks every display row against
    /// it); the note names the WAR, and a request the header calls NV_ERR_INVALID_ARGUMENT
    /// (not a connector, unknown WAR id) is named there too but still acknowledged, as the
    /// row says, until the row becomes a MODEL row of the control tables.
    fn set_war_flags(&mut self, cmd: u32, params: &[u8]) -> Answer {
        let mut a = crate::dmodel::answer(cmd, params).unwrap_or_else(|| {
            ok(
                params.to_vec(),
                ACK,
                format!("display control {cmd:#x}: DP_SET_WAR_FLAGS"),
            )
        });
        let did = get32(params, d::WAR_FLAGS_DISPLAY_ID);
        let war = get32(params, d::WAR_FLAGS_WAR_ID);
        let on = params.get(d::WAR_FLAGS_ENABLE).copied().unwrap_or(0) != 0;
        let what = match self.connector(did) {
            None => {
                format!("displayId {did:#x} is not a connector (the header's INVALID_ARGUMENT)")
            }
            Some(_) if war != d::DP_WAR_SW_AUTO_READ_ENABLE => {
                format!("unknown warId {war} (the header's INVALID_ARGUMENT)")
            }
            Some(c) => {
                let was = std::mem::replace(&mut c.sw_auto_read_war, on);
                format!(
                    "SW_AUTO_READ_ENABLE {} on {did:#x} (was {}; AUX reads are software, nothing to switch)",
                    on as u8, was as u8
                )
            }
        };
        a.note = format!("dp war flags: {what}; {}", a.note);
        a
    }

    /// `DP_CONFIGURE_FEC` (ctrl0073dp.h:2271-2303): FEC on the trained link when the link
    /// profile has it (`display.toml` [link] fec), reported back through
    /// `DP_GET_LINK_CONFIG.bFECEnabled`.
    fn configure_fec(&mut self, cmd: u32, params: &[u8]) -> Answer {
        const P: &str = "NV0073_CTRL_CMD_DP_CONFIGURE_FEC_PARAMS";
        let b = params.to_vec();
        let did = get_field(&b, field(P, "displayId")) as u32;
        if self.connector(did).is_none() {
            return self.bad_display(cmd, b, did);
        }
        let on = get_field(&b, field(P, "bEnableFec")) != 0;
        if on && !self.chip().link.fec {
            return fail(
                NV_ERR_NOT_SUPPORTED,
                b,
                LINK,
                format!("configure fec of {did:#x}: the link profile has no FEC"),
            );
        }
        if on && self.link.is_none() {
            return fail(
                NV_ERR_INVALID_ARGUMENT,
                b,
                SINK,
                format!("configure fec of {did:#x}: the link is not trained"),
            );
        }
        self.fec = on;
        ok(
            b,
            SINK,
            format!(
                "configure fec of {did:#x}: FEC {}",
                if on { "on" } else { "off" }
            ),
        )
    }

    /// `NV5070_CTRL_CMD_GET_RG_STATUS` (ctrl5070rg.h:37-73): the engine has no merge pairs
    /// and no scan or flip lock between heads, and its raster generators never stall.
    fn rg_status(&mut self, params: &[u8]) -> Answer {
        const P: &str = "NV5070_CTRL_CMD_GET_RG_STATUS_PARAMS";
        let abi = crate::ver::Abi::active();
        let e = |n: &str| {
            abi.d_enum(&format!("NV5070_CTRL_CMD_GET_RG_STATUS_{n}"))
                .unwrap_or(0) as u64
        };
        let mut b = params.to_vec();
        let head = get_field(&b, field(P, "head")) as u32;
        if head >= self.chip().heads {
            return fail(
                NV_ERR_INVALID_ARGUMENT,
                b,
                SYNTH,
                format!("rg status: head {head} does not exist"),
            );
        }
        put_field(&mut b, field(P, "scanLocked"), e("SCANLOCKED_NO"));
        put_field(&mut b, field(P, "flipLocked"), e("FLIPLOCKED_NO"));
        put_field(&mut b, field(P, "rgStalled"), e("STALLED_NO"));
        ok(
            b,
            ENGINE,
            format!("rg status of head {head}: not scan locked, not flip locked, not stalled"),
        )
    }

    /// `NVC372_CTRL_CMD_IS_MODE_POSSIBLE` (IMP): possible when the heads exist and every
    /// head's pixel clock fits the link budget and the head clock cap
    /// (display-adapter.md section 4; display-scaling.md section 2.2).
    fn is_mode_possible(&self, mut b: Vec<u8>) -> Answer {
        let n = b.get(d::IMP_NUM_HEADS).copied().unwrap_or(0) as usize;
        let cap = self
            .chip()
            .link
            .pclk_budget_khz()
            .min(self.chip().head_clk_cap_khz);
        let mut worst = (0u32, 0u32);
        let mut heads = Vec::new();
        for i in 0..n.min(8) {
            let o = d::IMP_HEAD + i * d::IMP_HEAD_SIZE;
            let head = b.get(o + d::IMP_HEAD_INDEX).copied().unwrap_or(0) as u32;
            let k = get32(&b, o + d::IMP_HEAD_MAX_PCLK_KHZ);
            heads.push(format!("head {head} {k} kHz"));
            if k > worst.1 {
                worst = (head, k);
            }
        }
        let possible = n <= self.chip().heads as usize && worst.1 <= cap;
        if b.len() > d::IMP_IS_POSSIBLE {
            b[d::IMP_IS_POSSIBLE] = possible as u8;
        }
        ok(
            b,
            SYNTH,
            format!(
                "is mode possible: {} ({n} heads: {}; limit {cap} kHz = min(link budget, head clock cap))",
                if possible { "yes" } else { "no" },
                if heads.is_empty() { "none".into() } else { heads.join(", ") }
            ),
        )
    }

    fn bad_display(&self, cmd: u32, b: Vec<u8>, did: u32) -> Answer {
        fail(
            NV_ERR_INVALID_ARGUMENT,
            b,
            BOARD,
            format!("control {cmd:#x}: displayId {did:#x} is not a connector"),
        )
    }

    /// `SPECIFIC_GET_EDID_V2` (display-adapter.md section 2.2): the override if set
    /// (unless RAW), else the sink EDID; `COPY_CACHE_YES` repeats the last answer; a
    /// disconnected display is an invalid argument.
    fn get_edid(&mut self, cmd: u32, mut b: Vec<u8>, did: u32) -> Answer {
        let flags = get32(&b, d::GET_EDID_FLAGS);
        let Some(c) = self.connector(did) else {
            return self.bad_display(cmd, b, did);
        };
        if !c.sink.is_plugged() {
            return fail(
                NV_ERR_INVALID_ARGUMENT,
                b,
                EDID_STATE,
                format!("get edid of {did:#x}: disconnected"),
            );
        }
        let (bytes, from) =
            if flags & d::GET_EDID_FLAGS_COPY_CACHE_YES != 0 && c.last_answer.is_some() {
                (c.last_answer.clone().unwrap(), "last answer")
            } else if flags & d::GET_EDID_FLAGS_READ_MODE_RAW == 0 && c.override_edid.is_some() {
                (c.override_edid.clone().unwrap(), "override")
            } else {
                (c.sink.edid().to_vec(), "sink")
            };
        c.last_answer = Some(bytes.clone());
        let len = bytes.len().min(d::EDID_MAX_BYTES);
        put32(&mut b, d::GET_EDID_BUFFER_SIZE, len as u32);
        let at = d::GET_EDID_BUFFER;
        if at + d::EDID_MAX_BYTES <= b.len() {
            b[at..at + d::EDID_MAX_BYTES].fill(0);
            b[at..at + len].copy_from_slice(&bytes[..len]);
        }
        ok(
            b,
            EDID_STATE,
            format!("get edid of {did:#x} flags {flags:#x}: {len} bytes from the {from}"),
        )
    }

    /// `SPECIFIC_SET_EDID_V2`: `bufferSize` 0 clears the override, whatever the buffer
    /// holds (display-adapter.md section 2.2, the device rule).
    fn set_edid(&mut self, cmd: u32, b: Vec<u8>, did: u32) -> Answer {
        let size = get32(&b, d::SET_EDID_BUFFER_SIZE) as usize;
        let at = d::SET_EDID_BUFFER;
        let bytes = b
            .get(at..at + size.min(d::EDID_MAX_BYTES))
            .map(<[u8]>::to_vec);
        let Some(c) = self.connector(did) else {
            return self.bad_display(cmd, b, did);
        };
        let note = if size == 0 {
            c.override_edid = None;
            format!("set edid of {did:#x}: size 0, override cleared")
        } else {
            let e = bytes.unwrap_or_default();
            let same = e.as_slice() == c.sink.edid();
            c.override_edid = Some(e);
            format!("set edid of {did:#x}: {size} bytes stored as override (identical to the sink EDID: {same})")
        };
        ok(b, EDID_STATE, note)
    }

    /// `DP_CTRL`: link training behind a control. A non-zero lane count trains the sink
    /// (dp-sink `train`, which leaves the DPCD as a real training would); zero powers
    /// the link down.
    fn dp_ctrl(&mut self, cmd: u32, mut b: Vec<u8>, did: u32) -> Answer {
        let (c_cmd, data) = (get32(&b, d::DP_CTRL_CMD), get32(&b, d::DP_CTRL_DATA));
        let lanes = (data & d::DP_DATA_LANE_COUNT_MASK) as u8;
        let bw = ((data >> d::DP_DATA_LINK_BW_SHIFT) & 0xFF) as u8;
        let profile_rate = self.chip().link.rate();
        let Some(c) = self.connector(did) else {
            return self.bad_display(cmd, b, did);
        };
        if lanes == 0 {
            c.sink.drop_link();
            c.sink.take_events();
            self.link = None;
            self.fec = false;
            return ok(
                b,
                SINK,
                format!("dp ctrl of {did:#x}: cmd {c_cmd:#x} data {data:#x}: link powered down"),
            );
        }
        // GSP's training starts with the sink in D0 (the DP library may have left it in
        // D3 after detection, DPCD 0x600 = 2); the device does the same before training.
        let mut woke = String::new();
        let mut power = [0u8];
        c.sink.dpcd().read(0x600, &mut power);
        if power[0] != 1 {
            let r = c
                .sink
                .transact(&dp_sink::AuxRequest::native_write(0x600, &[1]));
            woke = format!(" (sink set to D0 first: {r:?})");
        }
        let cfg = LinkRate::from_code(bw).map(|rate| LinkConfig {
            rate,
            lanes,
            enhanced_framing: c_cmd & (1 << 7) != 0,
        });
        match cfg.map(|cfg| (cfg, c.sink.train(cfg))) {
            Some((cfg, Ok(()))) if cfg.rate <= profile_rate => {
                self.link = Some((bw, lanes));
                self.fec = false;
                put32(&mut b, d::DP_CTRL_ERR, 0);
                ok(b, SINK, format!("dp ctrl of {did:#x}: cmd {c_cmd:#x}: trained {lanes} lanes at link bw {bw:#x}{woke}"))
            }
            other => {
                put32(&mut b, d::DP_CTRL_ERR, 1);
                fail(
                    NV_ERR_INVALID_ARGUMENT,
                    b,
                    SINK,
                    format!("dp ctrl of {did:#x}: cmd {c_cmd:#x} data {data:#x}: training refused ({other:?})"),
                )
            }
        }
    }

    // -----------------------------------------------------------------------------
    // Allocations.

    /// A display class allocation: recorded with its label; `None` for other classes.
    pub fn alloc(&mut self, h_class: u32, params: &[u8]) -> Option<Answer> {
        if !self.enabled {
            return None;
        }
        let name = match h_class {
            d::NV04_DISPLAY_COMMON => "NV04_DISPLAY_COMMON",
            d::NVC570_DISPLAY => "NVC570_DISPLAY",
            d::NVC372_DISPLAY_SW => "NVC372_DISPLAY_SW",
            d::NVC573_DISP_CAPABILITIES => "NVC573_DISP_CAPABILITIES",
            d::NVC371_DISP_SF_USER => "NVC371_DISP_SF_USER",
            d::NVC57D_CORE_CHANNEL_DMA => "NVC57D_CORE_CHANNEL_DMA",
            d::NVC57E_WINDOW_CHANNEL_DMA => "NVC57E_WINDOW_CHANNEL_DMA",
            d::NVC57B_WINDOW_IMM_CHANNEL_DMA => "NVC57B_WINDOW_IMM_CHANNEL_DMA",
            d::NVC57A_CURSOR_IMM_CHANNEL_PIO => "NVC57A_CURSOR_IMM_CHANNEL_PIO",
            _ => return None,
        };
        let mut note = format!("alloc {name} {h_class:#x}");
        if let Some(base) = userd_base(h_class, get32(params, d::CHANNEL_ALLOC_INSTANCE)) {
            let inst = get32(params, d::CHANNEL_ALLOC_INSTANCE);
            let ch = self.channels.entry(base).or_insert(Channel {
                hclass: h_class,
                instance: inst,
                ..Channel::default()
            });
            // nvos.h: DMA allocations supply the initial PUT/GET offset. Replaying
            // from zero can consume stale methods before the requested start. PIO
            // has no offset field (byte 12 is part of its pControl pointer).
            let initial_offset = if h_class == d::NVC57A_CURSOR_IMM_CHANNEL_PIO {
                0
            } else {
                get32(params, d::CHANNELDMA_ALLOC_OFFSET)
            };
            ch.put = initial_offset;
            ch.get = initial_offset;
            if let Some(p) = chan_page() {
                p.set_channel(base, initial_offset, initial_offset);
            }
            note.push_str(&format!(
                " instance {inst}: USERD at BAR0 {base:#x}, pushbuffer {:#x} (valid {}), initial PUT/GET {initial_offset:#x}",
                ch.pb, ch.valid
            ));
        }
        Some(ok(
            params.to_vec(),
            "synthesized (display object recorded; channel state in display/mod.rs)",
            note,
        ))
    }

    // -----------------------------------------------------------------------------
    // BAR0.

    /// The value of the display register at the dword-aligned `off`, or `None` when it
    /// is not the display's.
    pub fn read32(&mut self, off: u64) -> Option<u32> {
        if off == NV_FUSE_STATUS_OPT_DISPLAY {
            // DATA 0 = display present (gm107/dev_fuse.h:29).
            return Some(if self.enabled { 0 } else { 1 });
        }
        if !self.enabled {
            return None;
        }
        if off == NV_PDISP_VGA_WORKSPACE_BASE {
            return Some(self.chip().vga_workspace_base);
        }
        if let Some(v) = self.engine.read32(off, engine::now_ns()) {
            return Some(v);
        }
        if (NV_PDISP_FE_SW..NV_PDISP_FE_SW + CAPS_PAGE_SIZE).contains(&off) {
            let rel = (off - NV_PDISP_FE_SW) as u32;
            return Some(
                self.chip()
                    .caps
                    .iter()
                    .find(|c| c.offset == rel)
                    .map_or(0, |c| c.value),
            );
        }
        if (CORE_ARMED..CORE_ARMED + 0x8000).contains(&off) {
            let m = (off - CORE_ARMED) as u32;
            return Some(self.core_armed.get(&m).copied().unwrap_or(0));
        }
        if (CORE_USERD..USERD_END).contains(&off) {
            let (base, rel) = page_of(off);
            if let Some(ch) = self.channels.get(&base) {
                match rel {
                    0 => return Some(ch.put),
                    4 => return Some(ch.get),
                    _ => {}
                }
            }
            if base >= CURSOR_USERD && rel == CURSOR_FREE {
                return Some(CURSOR_FREE_ENTRIES);
            }
            return Some(self.latched.get(&off).copied().unwrap_or(0));
        }
        None
    }

    /// A guest read through BAR0: the clock advances first (engine.rs), so the
    /// interrupt handler sees the state of now, with the notifiers already written.
    pub fn read32_io(&mut self, off: u64, fb: &mut FakeFb, mem: &GuestMemory) -> Option<u32> {
        if self.enabled && engine::Engine::handles(off) {
            self.engine.catch_up(engine::now_ns(), fb, mem);
        }
        self.read32(off)
    }

    /// The display clock's tick with the display page (engine.rs `Ticker::set_driver`):
    /// the guest's reads of the status no longer trap, so the clock advances the engine
    /// (latches, notifiers, `LAST_DATA`, AWAKEN, the raise) and returns the trace's notes.
    pub fn tick(&mut self, fb: &mut FakeFb, mem: &GuestMemory) -> Vec<String> {
        if !self.enabled {
            return Vec::new();
        }
        let t0 = engine::now_ns();
        self.engine.catch_up(t0, fb, mem);
        engine::ticker().note_catch_up(engine::now_ns().saturating_sub(t0) / 1000);
        let mut n = self.take_engine_notes();
        if let Some(p) = fe_page() {
            n.extend(p.take_notes());
        }
        n
    }

    /// Notes of the engine not yet in the trace.
    pub fn take_engine_notes(&mut self) -> Vec<String> {
        let mut n = std::mem::take(&mut self.notes);
        n.append(&mut self.engine.notes);
        n
    }

    /// A kick the kick page's reactor found (chan_page.rs): `PUT` of the channel at USERD
    /// `base` is `put` now. The same kick as the trapped `PUT` write; returns the
    /// channel's `GET` after it and the notes, `None` when the page has no channel.
    pub fn page_kick(
        &mut self,
        base: u64,
        put: u32,
        mem: &GuestMemory,
        fb: &mut FakeFb,
    ) -> Option<(u32, Vec<String>)> {
        self.channels.get(&base)?;
        let notes = self.write32(base, put, mem, fb).unwrap_or_default();
        Some((self.channels.get(&base).map_or(put, |c| c.get), notes))
    }

    /// A write to a display register; `None` when it is not the display's.
    pub fn write32(
        &mut self,
        off: u64,
        v: u32,
        mem: &GuestMemory,
        fb: &mut FakeFb,
    ) -> Option<Vec<String>> {
        if !self.enabled {
            return None;
        }
        if engine::Engine::handles(off) {
            self.engine.catch_up(engine::now_ns(), fb, mem);
            let n = self.engine.write32(off, v).unwrap_or_default();
            let mut notes = self.take_engine_notes();
            if !n.is_empty() {
                notes.push(n);
            }
            return Some(notes);
        }
        if off == NV_FUSE_STATUS_OPT_DISPLAY || off == NV_PDISP_VGA_WORKSPACE_BASE {
            return Some(vec!["display: read-only register, write dropped".into()]);
        }
        if (NV_PDISP_FE_SW..NV_PDISP_FE_SW + CAPS_PAGE_SIZE).contains(&off)
            || (CORE_ARMED..CORE_ARMED + 0x8000).contains(&off)
        {
            return Some(vec!["display: read-only page, write dropped".into()]);
        }
        if !(CORE_USERD..USERD_END).contains(&off) {
            return None;
        }
        let (base, rel) = page_of(off);
        if rel == 0 && base < CURSOR_USERD {
            if let Some(mut ch) = self.channels.get(&base).copied() {
                ch.put = v;
                ch.kicks += 1;
                self.engine.catch_up(engine::now_ns(), fb, mem);
                let note = self.kick(base, &mut ch, mem, fb);
                self.channels.insert(base, ch);
                let mut notes = vec![note];
                notes.extend(self.take_engine_notes());
                return Some(notes);
            }
        }
        self.latched.insert(off, v);
        Some(Vec::new())
    }

    /// Walk the pushbuffer from GET to PUT (display-adapter.md section 4.3), keep the
    /// core channel's assembly state, make it ARMED at `UPDATE`, and move GET to PUT.
    fn kick(&mut self, base: u64, ch: &mut Channel, mem: &GuestMemory, fb: &mut FakeFb) -> String {
        // Window channels 0..31 (NVC57E) take their methods into the engine.
        let window = (WIN_USERD..WIN_USERD + 32 * 0x1000)
            .contains(&base)
            .then(|| ((base - WIN_USERD) / 0x1000) as usize);
        let (start, end) = (ch.get as u64, ch.put as u64);
        let mut methods = 0u32;
        let mut updates = 0u32;
        // Methods written in this kick, for the trace: which state a core UPDATE commits, and
        // a window channel's first kicks and first presentations (`WINDOW_TRACE_KICKS`).
        let mut seen: Vec<(u32, u32)> = Vec::new();
        let early = ch.kicks <= WINDOW_TRACE_KICKS * 2;
        let listed = match (base == CORE_USERD, window.is_some()) {
            (true, _) if early => 1024,
            (true, _) => 64,
            (false, true) if ch.kicks <= WINDOW_TRACE_KICKS => 1024,
            _ => 0,
        };
        let mut unreadable = false;
        if ch.valid && ch.pb_aspace == 1 && end <= ch.pb_limit + 1 {
            let mut pos = start;
            let mut budget = 4096u32;
            let read = |p: u64| mem.read_u32(ch.pb + p);
            while pos != end && budget > 0 {
                budget -= 1;
                let Some(hdr) = read(pos) else {
                    unreadable = true;
                    break;
                };
                pos += 4;
                let op = hdr >> 29;
                let count = (hdr >> 18) & 0x3FF;
                let addr = hdr & 0x3FFC;
                match op {
                    1 => {
                        pos = (hdr & 0xFFC) as u64;
                        continue;
                    }
                    0 | 2 => {
                        for k in 0..count {
                            let Some(data) = read(pos) else {
                                unreadable = true;
                                break;
                            };
                            pos += 4;
                            let m = if op == 0 { addr + 4 * k } else { addr };
                            methods += 1;
                            if seen.len() < listed {
                                seen.push((m, data));
                            }
                            if base == CORE_USERD {
                                if m == 0x200 {
                                    updates += 1;
                                    let a = self.core_assembly.clone();
                                    self.core_armed.extend(a);
                                    let armed = self.core_armed.clone();
                                    self.engine.core_update(&armed, engine::now_ns(), fb, mem);
                                } else {
                                    self.core_assembly.insert(m, data);
                                }
                            } else if let Some(w) = window {
                                if m == 0x200 {
                                    updates += 1;
                                    self.engine.window_update(w, engine::now_ns(), fb, mem);
                                } else {
                                    self.engine.window_method(w, m, data);
                                }
                            } else if m == 0x200 {
                                updates += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        ch.methods += methods;
        ch.get = ch.put;
        // HEAD_SET_RASTER_SIZE/SYNC_END/BLANK_END/BLANK_START(h) = 0x2064-0x2070 + 0x400h
        // (clc57d.h): an UPDATE carrying them is a modeset (display-adapter.md §5).
        let raster = seen
            .iter()
            .filter(|_| base == CORE_USERD)
            .filter(|(m, _)| {
                *m >= 0x2000 && ((m - 0x2000) % 0x400) >= 0x64 && ((m - 0x2000) % 0x400) <= 0x70
            })
            .count();
        let list = if (base == CORE_USERD && (updates > 0 || early)) || (window.is_some() && !seen.is_empty()) {
            let l: Vec<String> = seen.iter().map(|(m, v)| format!("{m:#x}={v:#x}")).collect();
            format!("; {} methods {}", if window.is_some() { "window" } else { "core" }, l.join(" "))
        } else {
            String::new()
        };
        format!(
            "display channel {:#x}/{} kick {}: PUT {:#x}, {methods} method(s), {updates} UPDATE(s), {raster} raster method(s){}{list}; GET := PUT [guest]",
            ch.hclass,
            ch.instance,
            ch.kicks,
            ch.put,
            if unreadable { ", pushbuffer unreadable" } else { "" }
        )
    }

    /// An event object the guest allocated (`GSP_RM_ALLOC` of NV01_EVENT_KERNEL_CALLBACK_EX
    /// or NV01_EVENT_OS_EVENT, NV0005_ALLOC_PARAMETERS: notifyIndex at 12): the hotplug
    /// ones are kept for POST_EVENT.
    pub fn record_event(&mut self, h_client: u32, h_object: u32, h_class: u32, params: &[u8]) {
        if !self.enabled || !matches!(h_class, 0x79 | 0x7E) {
            return;
        }
        if get32(params, 12) & 0xFFFF == NV2080_NOTIFIERS_HOTPLUG
            && !self.hotplug_events.contains(&(h_client, h_object))
        {
            self.hotplug_events.push((h_client, h_object));
            self.notes.push(format!(
                "display: hotplug event {h_client:#x}/{h_object:#x} recorded for POST_EVENT [guest]"
            ));
        }
    }

    /// Apply a host-side connector command (engine.rs `Ticker::poll_control`) and return
    /// the POST_EVENT payloads (`rpc_post_event_v17_00`, 32 bytes) for every hotplug event
    /// the guest registered. Commands: `edid-alt` / `edid-main` (unplug, change the
    /// monitor's EDID, replug), `unplug`, `plug`.
    pub fn connector_command(&mut self, cmd: &str) -> Vec<Vec<u8>> {
        if !self.enabled || self.connectors.is_empty() {
            return Vec::new();
        }
        let cmd = cmd.trim();
        let mut mask = 0u32;
        let mut what = Vec::new();
        for c in &mut self.connectors {
            let before = c.sink.is_plugged();
            match cmd {
                "edid-alt" | "edid-main" => {
                    c.sink.unplug();
                    let e = if cmd == "edid-alt" {
                        edid::monitor_edid_alt()
                    } else {
                        edid::monitor_edid()
                    };
                    let r = c.sink.set_edid(e);
                    c.sink.plug();
                    c.override_edid = None;
                    c.last_answer = None;
                    c.pending_plug = true;
                    what.push(format!(
                        "{:#x}: EDID {cmd} ({r:?}), replugged",
                        c.display_id
                    ));
                }
                "unplug" => {
                    c.sink.unplug();
                    c.pending_unplug = before;
                    what.push(format!("{:#x}: unplugged", c.display_id));
                }
                "plug" => {
                    c.sink.plug();
                    c.pending_plug = !before;
                    what.push(format!("{:#x}: plugged", c.display_id));
                }
                _ => {
                    what.push(format!("unknown command {cmd:?}"));
                    continue;
                }
            }
            c.sink.take_events();
            mask |= c.display_id;
        }
        let mut out = Vec::new();
        if mask != 0 {
            for &(h_client, h_event) in &self.hotplug_events {
                let mut p = vec![0u8; 32];
                put32(&mut p, 0, h_client);
                put32(&mut p, 4, h_event);
                put32(&mut p, 8, NV2080_NOTIFIERS_HOTPLUG);
                put32(&mut p, 12, mask);
                out.push(p);
            }
        }
        self.notes.push(format!(
            "display: connector command {cmd:?}: {}; {} POST_EVENT(s) NV2080_NOTIFIERS_HOTPLUG [host command]",
            what.join(", "),
            out.len()
        ));
        out
    }

    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }
}

/// The page and offset within it of a USERD access.
fn page_of(off: u64) -> (u64, u64) {
    if off < WIN_USERD {
        (CORE_USERD, off - CORE_USERD)
    } else {
        (off & !0xFFF, off & 0xFFF)
    }
}

/// The USERD page of a channel (disp/v03_00/dev_disp.h `NV_UDISP_FE_CHN_ASSY_BASEADR*`).
pub fn userd_base(hclass: u32, instance: u32) -> Option<u64> {
    let i = instance as u64;
    match hclass {
        d::NVC57D_CORE_CHANNEL_DMA => Some(CORE_USERD),
        d::NVC57E_WINDOW_CHANNEL_DMA if i < 32 => Some(WIN_USERD + i * 0x1000),
        d::NVC57B_WINDOW_IMM_CHANNEL_DMA if i < 32 => Some(WIN_USERD + (32 + i) * 0x1000),
        d::NVC57A_CURSOR_IMM_CHANNEL_PIO if i < 8 => Some(CURSOR_USERD + i * 0x1000),
        _ => None,
    }
}

/// The command belongs to the display's control families (NV0073, NV5070, NVC370,
/// NVC372, and the internal NV2080 display range).
fn is_display_cmd(cmd: u32) -> bool {
    matches!(cmd >> 16, 0x0073 | 0x5070 | 0xC370 | 0xC372)
}

#[cfg(test)]
mod tests;
