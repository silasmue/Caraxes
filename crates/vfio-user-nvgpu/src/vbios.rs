// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Locating the PCI image chain inside a PROM window image, as the RM does.
//!
//! A synthetic image (and `rom.bin`) starts with the 55 AA ROM signature. The real card's
//! PROM window does not: offset 0 holds an IFR header (`NVGI`), and the chain starts where
//! the IFR says (RTX 2070: 0x28600, caraxes/reference/rtx2070/prom-window-2026-09-25.md).
//! The RM follows the IFR in `s_romImgFindPciHeader_TU102` (kernel_gsp_vbios_tu102.c:202-272):
//! versions 1 and 2 keep the image offset at `FIXED_DATA_SIZE + 4`, version 3 in the ROM
//! directory (`RFRD`) 4 KiB after the flash status offset stored at `TOTAL_DATA_SIZE`.
//! [`locate`] does that and, when there is no IFR or it leads nowhere, scans 512-byte
//! boundaries for a 55 AA image whose PCIR structure checks out. The window is served
//! byte for byte either way; only what the device derives for itself uses the offset.

/// `NV_PBUS_IFR_FMT_FIXED0_SIGNATURE_VALUE` ("NVGI", turing/tu102/dev_bus.h:32).
const IFR_SIGNATURE: u32 = 0x4947_564E;
/// `NV_ROM_DIRECTORY_IDENTIFIER` ("RFRD", kernel_gsp_vbios_tu102.c:44).
const ROM_DIRECTORY: u32 = 0x4452_4652;
/// The PCI firmware spec's image alignment.
const IMAGE_ALIGN: usize = 512;
const BIT_SIGNATURE: [u8; 6] = [0xFF, 0xB8, b'B', b'I', b'T', 0];

fn rd16(img: &[u8], off: usize) -> Option<u16> {
    Some(u16::from_le_bytes(img.get(off..off + 2)?.try_into().ok()?))
}

fn rd32(img: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes(img.get(off..off + 4)?.try_into().ok()?))
}

/// Where the PCI image chain of `img` starts and what the device reads from its first image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciImage {
    /// Offset of the first 55 AA image in the window.
    pub offset: usize,
    /// How it was found.
    pub how: Found,
    /// PCIR vendor and device id.
    pub vendor: u16,
    pub device: u16,
    /// PCIR image length in bytes (512-byte units times 512).
    pub len: usize,
    /// PCIR code type (0 legacy x86).
    pub code_type: u8,
    /// Offset of the BIT header (`FF B8 'BIT' 00`) relative to the image, if present.
    pub bit: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Found {
    /// The image starts with 55 AA.
    AtZero,
    /// Through the IFR header (its version).
    Ifr(u8),
    /// By scanning 512-byte boundaries.
    Scan,
}

/// The PCI image at `off`: 55 AA, a PCIR pointer at +0x18 to "PCIR" inside `img`.
fn image_at(img: &[u8], off: usize, how: Found) -> Option<PciImage> {
    if img.get(off..off + 2)? != [0x55, 0xAA] {
        return None;
    }
    let pcir = off + rd16(img, off + 0x18)? as usize;
    if img.get(pcir..pcir + 4)? != b"PCIR" {
        return None;
    }
    let len = rd16(img, pcir + 0x10)? as usize * IMAGE_ALIGN;
    let end = (off + len.max(IMAGE_ALIGN)).min(img.len());
    let bit = img[off..end]
        .windows(BIT_SIGNATURE.len())
        .position(|w| w == BIT_SIGNATURE);
    Some(PciImage {
        offset: off,
        how,
        vendor: rd16(img, pcir + 4)?,
        device: rd16(img, pcir + 6)?,
        len,
        code_type: *img.get(pcir + 0x14)?,
        bit,
    })
}

/// The image offset the IFR header names, as `s_romImgFindPciHeader_TU102` computes it,
/// with the IFR version; `None` without an IFR or when it cannot be followed.
fn ifr_offset(img: &[u8]) -> Option<(usize, u8)> {
    if rd32(img, 0)? != IFR_SIGNATURE {
        return None;
    }
    let fixed1 = rd32(img, 4)?;
    let version = ((fixed1 >> 8) & 0xFF) as u8;
    let off = match version {
        1 | 2 => rd32(img, ((fixed1 >> 16) & 0x7FFF) as usize + 4)?,
        3 => {
            let total = (rd32(img, 8)? & 0xF_FFFF) as usize;
            let dir = rd32(img, total)? as usize + 4096;
            if rd32(img, dir)? != ROM_DIRECTORY {
                return None;
            }
            rd32(img, dir + 8)?
        }
        _ => return None,
    };
    (off % 4 == 0).then_some((off as usize, version))
}

/// The first PCI image of `img`: at 0, where the IFR header points, or the first 512-byte
/// boundary holding a 55 AA image with a valid PCIR. `None`: no image anywhere.
pub fn locate(img: &[u8]) -> Option<PciImage> {
    if let Some(i) = image_at(img, 0, Found::AtZero) {
        return Some(i);
    }
    if let Some(i) = ifr_offset(img).and_then(|(o, v)| image_at(img, o, Found::Ifr(v))) {
        return Some(i);
    }
    (IMAGE_ALIGN..img.len())
        .step_by(IMAGE_ALIGN)
        .find_map(|o| image_at(img, o, Found::Scan))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_window_is_found_through_its_ifr() {
        let Some(window) = crate::fixture::rtx2070_prom_window() else {
            return;
        };
        let i = locate(window).unwrap();
        assert_eq!(i.offset, 0x28600);
        assert_eq!(i.how, Found::Ifr(3));
        assert_eq!(
            (i.vendor, i.device, i.len, i.code_type),
            (0x10DE, 0x1F02, 0xEC00, 0)
        );
        // BIT at ROM offset 0x1B0 (prom-window-2026-09-25.md, "Tables").
        assert_eq!(i.bit, Some(0x1B0));
        // Without the IFR the scan finds the same image.
        let mut no_ifr = window.to_vec();
        no_ifr[..4].fill(0);
        let s = locate(&no_ifr).unwrap();
        assert_eq!((s.offset, s.how), (0x28600, Found::Scan));
        assert_eq!(PciImage { how: i.how, ..s }, i);
    }

    /// The derived info is the same as from `rom.bin` (the window's 0x28600-0x48200, the
    /// PCI-visible part): the window's slice from the found offset is that image.
    #[test]
    fn the_found_image_is_the_pci_rom() {
        let Some(window) = crate::fixture::rtx2070_prom_window() else {
            return;
        };
        let i = locate(window).unwrap();
        let rom = &window[i.offset..0x48200];
        let r = locate(rom).unwrap();
        assert_eq!(r.how, Found::AtZero);
        assert_eq!(
            PciImage {
                how: i.how,
                offset: i.offset,
                ..r
            },
            i
        );
        // rom.bin is not versioned (.gitignore); compare when the checkout has it.
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../caraxes/reference/rtx2070/rom.bin"
        );
        if let Ok(bin) = std::fs::read(p) {
            assert_eq!(bin, rom);
            assert_eq!(
                PciImage {
                    how: i.how,
                    offset: i.offset,
                    ..locate(&bin).unwrap()
                },
                i
            );
        }
    }

    #[test]
    fn images_without_a_valid_pcir_are_refused() {
        assert_eq!(locate(&[0x55, 0xAA, 0, 0]), None);
        assert_eq!(locate(&[0u8; 4096]), None);
        let mut bad = vec![0u8; 1024];
        bad[..2].copy_from_slice(&[0x55, 0xAA]);
        bad[0x18] = 0x40;
        bad[0x40..0x44].copy_from_slice(b"PCIX");
        assert_eq!(locate(&bad), None);
        bad[0x40..0x44].copy_from_slice(b"PCIR");
        assert_eq!(locate(&bad).unwrap().how, Found::AtZero);
    }
}
