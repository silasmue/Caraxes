// SPDX-License-Identifier: MIT
//! GB206 (Blackwell, the RTX 5060 Ti's chip): the tables in `chips/gb206/*.toml`,
//! compiled into the binary and parsed once, and the FSP firmware-boot model
//! ([`super::boot_fsp`]). The identity is the lab capture's (caraxes/reference/gb206/,
//! docs/caraxes/blackwell-gb206.md) for a 610.57.04 guest; no per-card value of the
//! capture is used.

use std::sync::OnceLock;

use super::boot_fsp::{FspBoot, FspBootData};
use super::{Chip, ChipData, ChipFiles, FirmwareBoot};

pub(crate) const FILES: ChipFiles<'static> = ChipFiles {
    chip: include_str!("../../chips/gb206/chip.toml"),
    regmap: include_str!("../../chips/gb206/regmap.toml"),
    device_info: include_str!("../../chips/gb206/device-info.toml"),
    interrupts: include_str!("../../chips/gb206/interrupts.toml"),
    controls: include_str!("../../chips/gb206/controls.toml"),
};
const BOOT: &str = include_str!("../../chips/gb206/boot.toml");

pub struct Gb206 {
    data: ChipData,
    boot: FspBootData,
}

impl Chip for Gb206 {
    fn data(&self) -> &ChipData {
        &self.data
    }

    fn firmware_boot(&'static self, fb_bytes: u64) -> Box<dyn FirmwareBoot> {
        Box::new(FspBoot::new(&self.boot, fb_bytes))
    }
}

static GB206: OnceLock<Result<Gb206, String>> = OnceLock::new();

fn load() -> Result<Gb206, String> {
    Ok(Gb206 {
        data: ChipData::load(&FILES).map_err(|e| format!("chips/gb206/{e}"))?,
        boot: FspBootData::parse(BOOT).map_err(|e| format!("chips/gb206/{e}"))?,
    })
}

/// The GB206, parsed and validated on first use.
pub fn get() -> Result<&'static dyn Chip, String> {
    match GB206.get_or_init(load) {
        Ok(c) => Ok(c),
        Err(e) => Err(e.clone()),
    }
}

/// The GB206 boot tables (for the FSP model's tests).
#[cfg(test)]
pub(crate) fn boot_data() -> &'static FspBootData {
    &GB206.get_or_init(load).as_ref().unwrap().boot
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chip() -> &'static ChipData {
        get().unwrap().data()
    }

    #[test]
    fn identity_and_windows() {
        let c = chip();
        assert_eq!((c.name.as_str(), c.short_name.as_str()), ("gb206", "GB206"));
        assert_eq!(
            (c.identity.boot_0, c.identity.boot_1, c.identity.boot_42),
            (0x1B60_00A1, 0, 0x1B6A_1000)
        );
        // The architecture and implementation fields halmgr decodes (nv_ref.h:97-150).
        assert_eq!((c.identity.boot_42 >> 20) & 0x1FF, 0x1B6);
        assert_eq!((c.bar0.pcfg, c.bar0.prom), (0x92000, 0x300000));
        assert_eq!(c.registers.bar0_window, 0x10_FD40);
        assert_eq!(c.registers.usermode_doorbell, 0xBB_0090);
        assert_eq!(c.chip_info.usermode_base, 0xBB_0000);
        assert_eq!(c.gmmu.mmu_format, "gmmu-v3-57bit");
        assert_eq!(
            c.register_name(0xAD_00BC),
            Some("NV_THERM_I2CS_SCRATCH (FSP_BOOT_COMPLETE)")
        );
        assert_eq!(c.register_name(0x8F_2AC4), Some("NV_PFSP_EMEMC/EMEMD(i)"));
    }

    /// The capture's engines resolve through the 610 names added for GB206.
    #[test]
    fn tables_resolve() {
        let c = chip();
        // GR0 1, COPY0 9, COPY4 0xD, COPY7 0x10, NVDEC0 0x13, NVENC0 0x1B | SW 0x22,
        // SEC2 0x26, NVJPEG0 0x2B, OFA0 0x33.
        assert_eq!(c.engine_caps_mask(3), vec![0x0809_2202, 0x0008_0844, 0]);
        let e: Vec<(&str, u32, u32, u32, u32)> = c
            .device_info
            .engines
            .iter()
            .map(|e| {
                (
                    e.name.as_str(),
                    e.eng_desc,
                    e.rm_engine_type,
                    e.mc_idx,
                    e.mmu_fault_id,
                )
            })
            .collect();
        assert_eq!(e[0], ("gr0", 0xd334_df00, 0x01, 84, 384));
        assert_eq!(e[3], ("ce7", 0x793c_eb07, 0x10, 22, 72));
        assert_eq!(e[7], ("nvjpg0", 0x2b3a_5400, 0x36, 51, 36));
        assert_eq!(e[8], ("ofa0", 0xdd7b_ab00, 0x3E, 81, 48));
        assert_eq!(c.memsys.ram_type, 0x15);
        assert_eq!(
            c.interrupts.subtrees,
            [(1, 0x8), (2, 0x1), (3, 0x10), (4, 0x20), (5, 0x2), (6, 0x4)]
        );
        assert_eq!(
            (c.channel.gpfifo, c.channel.copy, c.channel.compute),
            (0xCA6F, 0xCAB5, 0xCEC0)
        );
        let b = boot_data();
        assert_eq!((b.gsp.base, b.gsp_queue_head), (0x110000, 0x110C00));
        assert_eq!(
            (b.fsp.ememc, b.fsp.queue_head, b.fsp.emem_bytes),
            (0x8F2AC0, 0x8F2C00, 1024)
        );
    }

    #[test]
    fn selected_by_name() {
        assert_eq!(crate::chip::by_name("gb206").unwrap().data().name, "gb206");
        assert_eq!(crate::chip::by_name("tu106").unwrap().data().name, "tu106");
    }
}
