// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! TU106 (Turing, the RTX 2070's chip): the tables in `chips/tu106/*.toml`, compiled
//! into the binary and parsed once, and the Turing firmware-boot model.

use std::sync::OnceLock;

use super::turing::{TuringBoot, TuringBootData};
use super::{Chip, ChipData, ChipFiles, FirmwareBoot};

pub(crate) const FILES: ChipFiles<'static> = ChipFiles {
    chip: include_str!("../../chips/tu106/chip.toml"),
    regmap: include_str!("../../chips/tu106/regmap.toml"),
    device_info: include_str!("../../chips/tu106/device-info.toml"),
    interrupts: include_str!("../../chips/tu106/interrupts.toml"),
    controls: include_str!("../../chips/tu106/controls.toml"),
};
const BOOT: &str = include_str!("../../chips/tu106/boot.toml");

pub struct Tu106 {
    data: ChipData,
    boot: TuringBootData,
}

impl Chip for Tu106 {
    fn data(&self) -> &ChipData {
        &self.data
    }

    fn firmware_boot(&'static self, fb_bytes: u64) -> Box<dyn FirmwareBoot> {
        Box::new(TuringBoot::new(&self.boot, fb_bytes))
    }
}

static TU106: OnceLock<Result<Tu106, String>> = OnceLock::new();

fn load() -> Result<Tu106, String> {
    Ok(Tu106 {
        data: ChipData::load(&FILES).map_err(|e| format!("chips/tu106/{e}"))?,
        boot: TuringBootData::parse(BOOT).map_err(|e| format!("chips/tu106/{e}"))?,
    })
}

/// The TU106, parsed and validated on first use.
pub fn get() -> Result<&'static dyn Chip, String> {
    match TU106.get_or_init(load) {
        Ok(c) => Ok(c),
        Err(e) => Err(e.clone()),
    }
}

/// The TU106 boot tables (for the Turing model's tests).
#[cfg(test)]
pub(crate) fn boot_data() -> &'static TuringBootData {
    &TU106.get_or_init(load).as_ref().unwrap().boot
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chip::DeviceInfoEntry;

    fn chip() -> &'static ChipData {
        get().unwrap().data()
    }

    #[test]
    fn identity_and_windows() {
        let c = chip();
        assert_eq!(c.name, "tu106");
        assert_eq!(c.short_name, "TU106");
        assert_eq!(
            (c.identity.boot_0, c.identity.boot_1, c.identity.boot_42),
            (0x1660_00A1, 0, 0x166A_1000)
        );
        assert_eq!((c.bar0.pcfg, c.bar0.prom), (0x88000, 0x300000));
        assert_eq!(c.register_name(0x1478), Some("NV_PBUS_SW_SCRATCH(i)"));
        assert_eq!(c.register_name(0xA00), Some("NV_PMC_BOOT_42"));
        assert_eq!(c.register_name(0x110C00), Some("NV_PGSP_QUEUE_HEAD/TAIL"));
        assert_eq!(c.register_name(0x88004), Some("NV_PCFG (config mirror)"));
        assert_eq!(
            c.register_name(0x3FFFFC),
            Some("NV_PROM_DATA (VBIOS window)")
        );
        assert_eq!(c.register_name(0x12345), None);
        assert_eq!(c.regmap.len(), 52);
    }

    /// The tables resolve to exactly the numbers the E4/E5a device answered with.
    #[test]
    fn tables_resolve_to_the_milestone_values() {
        let c = chip();
        assert_eq!(c.engine_caps_mask(3), vec![0x081B_FE02, 0x0000_00C4, 0]);
        let e = |name: &str,
                 desc: u32,
                 rm: u32,
                 runlist,
                 fault,
                 mc,
                 inst,
                 host,
                 dev,
                 pbdma: &[[u32; 2]]| {
            DeviceInfoEntry {
                name: name.into(),
                eng_desc: desc,
                rm_engine_type: rm,
                runlist,
                mmu_fault_id: fault,
                mc_idx: mc,
                instance: inst,
                host_driven: host,
                dev_type: dev,
                pbdma: pbdma.to_vec(),
            }
        };
        let want = [
            e(
                "gr0",
                0xd334_df00,
                0x01,
                0,
                64,
                84,
                0,
                1,
                0,
                &[[0, 32], [1, 33]],
            ),
            e("ce0", 0x793c_eb00, 0x09, 0, 15, 15, 0, 1, 19, &[[0, 32]]),
            e("ce1", 0x793c_eb01, 0x0A, 0, 16, 16, 1, 1, 19, &[[1, 33]]),
            e("ce2", 0x793c_eb02, 0x0B, 1, 17, 17, 2, 1, 19, &[[2, 34]]),
            e("ce3", 0x793c_eb03, 0x0C, 2, 18, 18, 3, 1, 19, &[[3, 35]]),
            e("ce4", 0x793c_eb04, 0x0D, 3, 19, 19, 4, 1, 19, &[[4, 36]]),
            e("nvdec0", 0x8f99_e100, 0x1D, 4, 10, 65, 0, 1, 0, &[[5, 37]]),
            e("nvdec1", 0x8f99_e101, 0x1E, 5, 25, 66, 1, 1, 0, &[[6, 38]]),
            e("nvdec2", 0x8f99_e102, 0x1F, 6, 26, 67, 2, 1, 0, &[[7, 39]]),
            e("nvenc0", 0xe97b_6c00, 0x25, 7, 11, 38, 0, 1, 0, &[[8, 40]]),
            e("sec2", 0x28c4_0800, 0x31, 8, 14, 47, 0, 1, 0, &[[9, 41]]),
            e("sw", 0x95a6_f500, 0x2D, 0, 0xFFFF_FFFF, 0, 0, 0, 0, &[]),
        ];
        assert_eq!(c.device_info.engines, want);
        let intr: Vec<(u16, u32, u32)> = c
            .interrupts
            .entries
            .iter()
            .map(|e| (e.mc_idx, e.vector_stall, e.vector_nonstall))
            .collect();
        assert_eq!(
            intr,
            [
                (50, 192, 1),
                (4, 193, 2),
                (1, 194, 3),
                (59, 64, 0),
                (60, 65, 0),
                (61, 195, 0),
                // E5b part 2: the mirrored engines' non-stall vectors in subtree 0.
                (84, 196, 4),
                (17, 197, 5),
                (18, 198, 6),
                (19, 199, 7),
                // The display vector in leaf 4 (docs/caraxes/display-ab.md).
                (2, 156, 0)
            ]
        );
        assert_eq!(
            c.interrupts.subtrees,
            [(1, 0x8), (2, 0x1), (5, 0x2), (6, 0x4)]
        );
        assert_eq!(c.memsys.ram_type, 0x11);
        assert_eq!(
            (
                c.channel.gpfifo,
                c.channel.copy,
                c.channel.uvm_sw,
                c.channel.compute
            ),
            (0xC46F, 0xC5B5, 0xC076, 0xC5C0)
        );
        let b = boot_data();
        assert_eq!(
            (b.gsp.base, b.sec2.base, b.gsp_queue_head),
            (0x110000, 0x840000, 0x110C00)
        );
        assert_eq!(b.consts.len(), 7);
    }

    #[test]
    fn unknown_names_refuse_the_chip() {
        let bad = FILES.chip.replace("\"GDDR6\"", "\"HBM9\"");
        let files = ChipFiles {
            chip: &bad,
            ..FILES
        };
        let e = ChipData::load(&files).unwrap_err();
        assert!(e.contains("HBM9"), "{e}");
        assert!(crate::chip::by_name("gb202").is_err());
    }
}
