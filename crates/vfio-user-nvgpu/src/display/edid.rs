// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The EDID of the emulated DisplayPort monitor (docs/caraxes/display-ab.md, gate D3).
//!
//! Base block (EDID 1.4, DisplayPort interface, 8 bpc, RGB 4:4:4):
//! - DTD 1 (preferred): 1920x1080 at 60 Hz, CTA-861 timing (148.5 MHz, 2200x1125);
//! - DTD 2: 1280x720 at 60 Hz (74.25 MHz, 1650x750);
//! - DTD 3: 1920x1080 at 50 Hz (148.5 MHz, 2640x1125);
//! - the monitor name; established timing 640x480 at 60 Hz (the DP fail-safe mode).
//!
//! CTA-861 extension: the three DTD timings as VICs 16, 4 and 31, and VIC 201 (7680x4320 at 120 Hz,
//! 4752 MHz). It is the deliberately impossible mode: its pixel rate exceeds the HBR3 x4
//! link budget of the profile (1080 MHz at 24 bpp) and, at 8 bpp, even what DSC could
//! carry (25.92 Gbit/s / 8 bpp = 3240 MHz), so the guest must prune it. No audio, no
//! HDMI vendor block, so nothing HDMI or audio is switched on.
//!
//! The header, chromaticity and DTD encoding come from the `dp-sink` generator
//! ([`dp_sink::EdidSpec`], [`dp_sink::DetailedTiming`]); only the descriptor slots
//! and the extension are laid out here.

use dp_sink::edid::checksum_byte;
use dp_sink::{DetailedTiming, EdidSpec};

/// The impossible mode's VIC (CTA-861-H table 3: 7680x4320p120, 16:9).
pub const IMPOSSIBLE_VIC: u8 = 201;

fn cea(pclk_khz: u32, h: (u16, u16, u16, u16), v: (u16, u16, u16, u16)) -> DetailedTiming {
    DetailedTiming {
        pixel_clock_khz: pclk_khz,
        h_active: h.0,
        h_front: h.1,
        h_sync: h.2,
        h_back: h.3,
        v_active: v.0,
        v_front: v.1,
        v_sync: v.2,
        v_back: v.3,
        h_sync_positive: true,
        v_sync_positive: true,
        h_size_mm: 527,
        v_size_mm: 296,
    }
}

/// 1920x1080p60 (CTA VIC 16 timing).
pub fn mode_1080p60() -> DetailedTiming {
    cea(148_500, (1920, 88, 44, 148), (1080, 4, 5, 36))
}

/// 1280x720p60 (CTA VIC 4 timing).
pub fn mode_720p60() -> DetailedTiming {
    cea(74_250, (1280, 110, 40, 220), (720, 5, 5, 20))
}

/// 1920x1080p50 (CTA VIC 31 timing).
pub fn mode_1080p50() -> DetailedTiming {
    cea(148_500, (1920, 528, 44, 148), (1080, 4, 5, 36))
}

fn name_descriptor(name: &str) -> [u8; 18] {
    let mut d = [0u8; 18];
    d[3] = 0xFC;
    let mut data = [0x20u8; 13];
    let b = name.as_bytes();
    let n = b.len().min(13);
    data[..n].copy_from_slice(&b[..n]);
    if n < 13 {
        data[n] = 0x0A;
    }
    d[5..].copy_from_slice(&data);
    d
}

/// The CTA-861 extension: a video data block with the impossible VIC, and a video
/// capability data block (RGB quantization selectable); no DTDs.
fn cta_block() -> [u8; 128] {
    let mut b = [0u8; 128];
    b[0] = 0x02;
    b[1] = 0x03;
    let mut i = 4;
    // Video data block: the three DTD timings as VICs (16, 4, 31) and the impossible
    // VIC last. VICs 193-253 are coded directly (never "native").
    let svds = [16u8, 4, 31, IMPOSSIBLE_VIC];
    b[i] = (0x2 << 5) | svds.len() as u8;
    b[i + 1..i + 1 + svds.len()].copy_from_slice(&svds);
    i += 1 + svds.len();
    // Video capability data block: extended tag 0x00, QS = 1, S_IT = S_CE = 10.
    b[i..i + 3].copy_from_slice(&[(0x7 << 5) | 2, 0x00, 0x4A]);
    i += 3;
    b[2] = i as u8;
    // Underscanned IT formats (matches the capability block), no audio, no YCbCr.
    b[3] = 0x80;
    b[127] = checksum_byte(&b[..127]);
    b
}

/// The monitor's EDID: 256 bytes (base block and CTA-861 extension).
pub fn monitor_edid() -> Vec<u8> {
    let spec = EdidSpec {
        product_code: 0x0002,
        preferred: mode_1080p60(),
        // 640x480 at 60 Hz only.
        established: [0x20, 0x00, 0x00],
        standard: Vec::new(),
        name: "Caraxes DP".to_string(),
        ..EdidSpec::first_monitor()
    };
    let mut b = spec
        .build()
        .expect("the Caraxes monitor EDID is a valid spec");
    b.truncate(128);
    b[72..90].copy_from_slice(&mode_720p60().encode().expect("720p60 encodes"));
    b[90..108].copy_from_slice(&mode_1080p50().encode().expect("1080p50 encodes"));
    b[108..126].copy_from_slice(&name_descriptor("Caraxes DP"));
    b[126] = 1;
    b[127] = checksum_byte(&b[..127]);
    b.extend_from_slice(&cta_block());
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edid_layout() {
        let e = monitor_edid();
        assert_eq!(e.len(), 256);
        assert_eq!(e.iter().take(128).fold(0u8, |a, x| a.wrapping_add(*x)), 0);
        assert_eq!(e.iter().skip(128).fold(0u8, |a, x| a.wrapping_add(*x)), 0);
        // DisplayPort, 8 bpc.
        assert_eq!(e[20], 0xA5);
        // DTD 1: 148.5 MHz.
        assert_eq!(u16::from_le_bytes([e[54], e[55]]), 14850);
        // DTD 2: 74.25 MHz, DTD 3: 148.5 MHz.
        assert_eq!(u16::from_le_bytes([e[72], e[73]]), 7425);
        assert_eq!(u16::from_le_bytes([e[90], e[91]]), 14850);
        assert_eq!(e[108 + 3], 0xFC);
        assert_eq!(e[126], 1);
        assert_eq!(e[128 + 8], IMPOSSIBLE_VIC);
        // 1080p50 is 50 Hz, 1080p60 60 Hz.
        assert_eq!(mode_1080p50().refresh_mhz(), 50_000);
        assert_eq!(mode_1080p60().refresh_mhz(), 60_000);
        assert_eq!(mode_720p60().refresh_mhz(), 60_000);
    }

    /// The guest's `/sys/class/drm/card0-DP-1/edid` of run b3 (display-ab.md, gate D3) is
    /// byte-identical to what the device serves.
    #[test]
    fn edid_is_what_the_guest_exposed() {
        let guest =
            include_bytes!("../../../dp-sink/tests/transcripts/caraxes-dp-monitor.edid.bin");
        assert_eq!(monitor_edid(), guest.to_vec());
    }

    /// edid-decode (the second parser of gate D3), when it is installed.
    #[test]
    fn edid_decode_accepts_it() {
        let tool = std::env::var("EDID_DECODE").unwrap_or_else(|_| "edid-decode".into());
        let dir = std::env::temp_dir().join(format!("caraxes-edid-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("monitor.bin");
        std::fs::write(&path, monitor_edid()).unwrap();
        let out = match std::process::Command::new(&tool)
            .arg("--check")
            .arg(&path)
            .output()
        {
            Ok(o) => o,
            Err(_) => {
                eprintln!("edid-decode not found; skipped");
                return;
            }
        };
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            text.contains("EDID conformity: PASS"),
            "edid-decode --check:\n{text}"
        );
        assert!(text.contains("7680x4320"), "{text}");
    }
}
