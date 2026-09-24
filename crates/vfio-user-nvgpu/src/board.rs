// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The board axis (decision D14): what a board adds to its chip. PCI ids, subsystem
//! ids, class, revision and the capability chain come from the config-space capture,
//! the ROM from the ROM capture; `boards/<board>.toml` names both captures and holds
//! what neither carries: the BAR layout, the ROM BAR size, the VBIOS version, the
//! default chip and the board name of the guest name. The RTX 2070 is built in
//! (`--board-data rtx2070`, the default); another board is a TOML file of the same
//! shape (`--board-data <path>`).

use serde::Deserialize;

/// Layout of one BAR register, in the order the card exposes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
    /// 32-bit memory BAR.
    Mem32 { size: u64, prefetch: bool },
    /// Low half of a 64-bit memory BAR.
    Mem64 { size: u64, prefetch: bool },
    /// High half of the preceding 64-bit memory BAR.
    Mem64High,
    /// I/O port BAR.
    Io { size: u64 },
    /// No BAR in this register.
    None,
}

impl Bar {
    /// The size of the region this register opens (0 for a high half or none).
    pub fn size(&self) -> u64 {
        match *self {
            Bar::Mem32 { size, .. } | Bar::Mem64 { size, .. } | Bar::Io { size } => size,
            Bar::Mem64High | Bar::None => 0,
        }
    }
}

/// A board: `boards/<board>.toml`, resolved.
#[derive(Clone, Debug)]
pub struct Board {
    pub name: String,
    pub chip: String,
    /// Config-space and ROM captures, relative to `LEA_ROOT`.
    pub config: String,
    pub rom: String,
    /// The VBIOS version as the BIT spells it (`90.06.3D.00.76`).
    pub vbios_version: String,
    /// `vbiosRevision` / `vbiosOemRevision` of the static info and BIOS_GET_INFO_V2.
    pub vbios_revision: u32,
    pub vbios_oem_revision: u32,
    /// The six BAR registers.
    pub bars: [Bar; 6],
    pub rom_bar_size: u64,
    /// The board's display outputs (`[display]`, read by display/mod.rs).
    pub display: Option<toml::Value>,
    /// Present the MSI-X capability of the capture. `false` (the default) unlinks it
    /// from the capability chain; `true` is refused until the device implements MSI-X.
    pub msix: bool,
    /// A config-space capture whose identity words (vendor, device, revision, class,
    /// subsystem; `config::ID_BYTES`) are laid over `config`, relative to `LEA_ROOT`: the
    /// costume board's 5060 Ti ids on the TU106 layout (docs/caraxes/costume-5060.md).
    pub ids_from: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BoardFile {
    name: String,
    chip: String,
    config: String,
    rom: String,
    vbios_version: String,
    bars: Vec<BarFile>,
    rom_bar_size: u64,
    #[serde(default)]
    display: Option<toml::Value>,
    #[serde(default)]
    msix: bool,
    #[serde(default)]
    ids_from: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BarFile {
    kind: String,
    size: u64,
    #[serde(default)]
    prefetch: bool,
}

const RTX2070: &str = include_str!("../boards/rtx2070.toml");
const RTX5060TI: &str = include_str!("../boards/rtx5060ti.toml");
const RTX5060TI_ON_TU106: &str = include_str!("../boards/rtx5060ti-on-tu106.toml");

/// The boards compiled into the binary, by `--board-data` name. `rtx5060ti-on-tu106` is
/// the costume (docs/caraxes/costume-5060.md): the 5060 Ti's PCI identity and name on the
/// TU106 chip, whose answers stay the truth.
pub const BOARDS: &[&str] = &["rtx2070", "rtx5060ti", "rtx5060ti-on-tu106"];

impl Board {
    /// A built-in board by name, or a board file by path.
    pub fn load(name_or_path: &str) -> Result<Self, String> {
        match name_or_path {
            "rtx2070" => Self::parse("boards/rtx2070.toml", RTX2070),
            "rtx5060ti" => Self::parse("boards/rtx5060ti.toml", RTX5060TI),
            "rtx5060ti-on-tu106" => {
                Self::parse("boards/rtx5060ti-on-tu106.toml", RTX5060TI_ON_TU106)
            }
            path => {
                let text = std::fs::read_to_string(path)
                    .map_err(|e| format!("read board data {path}: {e}"))?;
                Self::parse(path, &text)
            }
        }
    }

    pub fn parse(file: &str, text: &str) -> Result<Self, String> {
        let f: BoardFile = toml::from_str(text).map_err(|e| format!("{file}: {e}"))?;
        let mut bars = [Bar::None; 6];
        let mut i = 0usize;
        for b in &f.bars {
            let need = if b.kind == "mem64" { 2 } else { 1 };
            if i + need > 6 {
                return Err(format!("{file}: more BARs than six registers"));
            }
            if !b.size.is_power_of_two() {
                return Err(format!(
                    "{file}: BAR size {:#x} is not a power of two",
                    b.size
                ));
            }
            match b.kind.as_str() {
                "mem32" => {
                    bars[i] = Bar::Mem32 {
                        size: b.size,
                        prefetch: b.prefetch,
                    }
                }
                "mem64" => {
                    bars[i] = Bar::Mem64 {
                        size: b.size,
                        prefetch: b.prefetch,
                    };
                    bars[i + 1] = Bar::Mem64High;
                }
                "io" => bars[i] = Bar::Io { size: b.size },
                k => return Err(format!("{file}: unknown BAR kind {k:?}")),
            }
            i += need;
        }
        if !f.rom_bar_size.is_power_of_two() {
            return Err(format!(
                "{file}: ROM BAR size {:#x} is not a power of two",
                f.rom_bar_size
            ));
        }
        if f.msix {
            return Err(format!(
                "{file}: msix = true is not implemented (the device offers INTx and MSI only, device.rs irqs)"
            ));
        }
        let (vbios_revision, vbios_oem_revision) = parse_vbios_version(&f.vbios_version)
            .ok_or_else(|| {
                format!(
                    "{file}: VBIOS version {:?} is not NN.NN.NN.NN.NN",
                    f.vbios_version
                )
            })?;
        Ok(Board {
            name: f.name,
            chip: f.chip,
            config: f.config,
            rom: f.rom,
            vbios_version: f.vbios_version,
            vbios_revision,
            vbios_oem_revision,
            bars,
            rom_bar_size: f.rom_bar_size,
            display: f.display,
            msix: f.msix,
            ids_from: f.ids_from,
        })
    }

    /// Override the VBIOS version (`--vbios-version`).
    pub fn set_vbios_version(&mut self, v: &str) -> Result<(), String> {
        let (r, o) = parse_vbios_version(v)
            .ok_or_else(|| format!("VBIOS version {v:?} is not NN.NN.NN.NN.NN"))?;
        self.vbios_version = v.into();
        self.vbios_revision = r;
        self.vbios_oem_revision = o;
        Ok(())
    }

    /// Sizes of the BARs that open a region, in order (BAR0, BAR1, BAR3, BAR5 on the
    /// RTX 2070): what `BUS_GET_PCI_BAR_INFO` reports.
    pub fn region_bar_sizes(&self) -> Vec<u64> {
        self.bars.iter().map(Bar::size).filter(|&s| s > 0).collect()
    }
}

/// `90.06.3D.00.76` -> (0x90063D00, 0x76): the first four bytes are `BiosVersion`,
/// the fifth `BiosOEMVersion` of the BIT's BIOSDATA token.
pub fn parse_vbios_version(v: &str) -> Option<(u32, u32)> {
    let b: Vec<u8> = v
        .split('.')
        .map(|p| u8::from_str_radix(p, 16).ok())
        .collect::<Option<_>>()?;
    (b.len() == 5).then(|| (u32::from_be_bytes([b[0], b[1], b[2], b[3]]), b[4] as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtx2070_is_the_captured_card() {
        let b = Board::load("rtx2070").unwrap();
        assert_eq!((b.name.as_str(), b.chip.as_str()), ("RTX 2070", "tu106"));
        assert_eq!(
            (b.vbios_revision, b.vbios_oem_revision),
            (0x9006_3D00, 0x76)
        );
        assert_eq!(
            b.bars,
            [
                Bar::Mem32 {
                    size: 16 << 20,
                    prefetch: false
                },
                Bar::Mem64 {
                    size: 256 << 20,
                    prefetch: true
                },
                Bar::Mem64High,
                Bar::Mem64 {
                    size: 32 << 20,
                    prefetch: true
                },
                Bar::Mem64High,
                Bar::Io { size: 128 },
            ]
        );
        assert_eq!(b.region_bar_sizes(), [16 << 20, 256 << 20, 32 << 20, 0x80]);
        assert_eq!(b.rom_bar_size, 512 << 10);
        assert_eq!(parse_vbios_version("90.06.3d.00.7"), Some((0x9006_3D00, 7)));
        assert_eq!(parse_vbios_version("90.06"), None);
        assert!(Board::parse("x", "name = 1").is_err());
        assert!(!b.msix);
    }

    #[test]
    fn rtx5060ti_is_the_gb206_capture() {
        let b = Board::load("rtx5060ti").unwrap();
        assert_eq!((b.name.as_str(), b.chip.as_str()), ("RTX 5060 Ti", "gb206"));
        assert_eq!(
            (b.vbios_revision, b.vbios_oem_revision),
            (0x9806_1F40, 0x4A)
        );
        assert_eq!(b.region_bar_sizes(), [64 << 20, 256 << 20, 32 << 20, 0x80]);
        assert_eq!(b.rom_bar_size, 512 << 10);
        assert!(!b.msix && b.display.is_none());
        assert!(b.config.starts_with("caraxes/reference/gb206/"));
        // MSI-X on is refused until the device implements it.
        let on = RTX5060TI.replace("msix = false", "msix = true");
        let e = Board::parse("x", &on).unwrap_err();
        assert!(e.contains("msix"), "{e}");
    }

    /// The costume (docs/caraxes/costume-5060.md): the 5060 Ti's config capture, BARs
    /// and personality on the TU106, with the RTX 2070's ROM and VBIOS version.
    #[test]
    fn rtx5060ti_on_tu106_is_the_costume() {
        let b = Board::load("rtx5060ti-on-tu106").unwrap();
        let g = Board::load("rtx5060ti").unwrap();
        let t = Board::load("rtx2070").unwrap();
        assert!(BOARDS.contains(&"rtx5060ti-on-tu106"));
        assert_eq!((b.name.as_str(), b.chip.as_str()), ("RTX 5060 Ti", "tu106"));
        assert_eq!(b.name, g.name);
        // The TU106 config layout with the GB206 capture's ids (config::ID_BYTES).
        assert_eq!(b.config, t.config);
        assert_eq!(b.ids_from.as_deref(), Some(g.config.as_str()));
        assert_eq!((t.ids_from.as_deref(), g.ids_from.as_deref()), (None, None));
        assert_eq!(
            (b.bars, b.rom_bar_size, b.msix),
            (g.bars, g.rom_bar_size, g.msix)
        );
        assert_eq!(
            (b.rom.as_str(), b.vbios_version.as_str()),
            (t.rom.as_str(), t.vbios_version.as_str())
        );
        assert_eq!(
            (b.vbios_revision, b.vbios_oem_revision),
            (t.vbios_revision, t.vbios_oem_revision)
        );
        assert!(b.display.is_some());
    }
}
