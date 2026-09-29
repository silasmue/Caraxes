// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! EDID 1.4 generator for a DisplayPort monitor, with an optional CTA-861 extension.
//!
//! The first monitor ([`EdidSpec::first_monitor`]): digital input over DisplayPort at
//! 8 bpc, RGB 4:4:4, one preferred detailed timing 1920x1080 at 60 Hz with CVT reduced
//! blanking (138.5 MHz), the 640x480 fail-safe mode DisplayPort requires plus a few
//! established and standard timings, a monitor name and a range-limits descriptor, no
//! extension block. The CTA-861 extension (video data block only; no audio, no HDR, no
//! HDMI blocks) is off by default.

use std::fmt;

/// Why an EDID could not be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EdidError {
    /// A value does not fit the field that carries it.
    Field(&'static str, u64),
    /// Manufacturer id must be three letters A-Z.
    Manufacturer([u8; 3]),
    /// Monitor name longer than 13 bytes or not printable ASCII.
    Name(String),
    /// More than 8 standard timings.
    TooManyStandardTimings(usize),
    /// CVT needs a horizontal size that is a multiple of 8 and a sane refresh.
    Cvt(&'static str),
}

impl fmt::Display for EdidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EdidError::Field(name, v) => write!(f, "{name} = {v} does not fit its EDID field"),
            EdidError::Manufacturer(m) => write!(f, "manufacturer id {m:?} is not three letters"),
            EdidError::Name(n) => write!(f, "monitor name {n:?} is not 1-13 printable ASCII"),
            EdidError::TooManyStandardTimings(n) => write!(f, "{n} standard timings (max 8)"),
            EdidError::Cvt(why) => write!(f, "CVT: {why}"),
        }
    }
}

impl std::error::Error for EdidError {}

/// One detailed timing (all sizes in pixels or lines).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DetailedTiming {
    /// Pixel clock in kHz; the EDID stores units of 10 kHz, so it must be a multiple of 10.
    pub pixel_clock_khz: u32,
    pub h_active: u16,
    pub h_front: u16,
    pub h_sync: u16,
    pub h_back: u16,
    pub v_active: u16,
    pub v_front: u16,
    pub v_sync: u16,
    pub v_back: u16,
    pub h_sync_positive: bool,
    pub v_sync_positive: bool,
    /// Image size in millimetres.
    pub h_size_mm: u16,
    pub v_size_mm: u16,
}

impl DetailedTiming {
    pub fn h_blank(&self) -> u16 {
        self.h_front + self.h_sync + self.h_back
    }

    pub fn v_blank(&self) -> u16 {
        self.v_front + self.v_sync + self.v_back
    }

    pub fn h_total(&self) -> u32 {
        self.h_active as u32 + self.h_blank() as u32
    }

    pub fn v_total(&self) -> u32 {
        self.v_active as u32 + self.v_blank() as u32
    }

    /// Refresh rate in millihertz (rounded down).
    pub fn refresh_mhz(&self) -> u64 {
        self.pixel_clock_khz as u64 * 1_000_000 / (self.h_total() as u64 * self.v_total() as u64)
    }

    /// Line rate in Hz (rounded down).
    pub fn h_freq_hz(&self) -> u64 {
        self.pixel_clock_khz as u64 * 1000 / self.h_total() as u64
    }

    /// CVT reduced blanking (version 1) timing for `h` x `v` at `refresh_hz`, progressive,
    /// no margins, computed from the VESA CVT 1.2 formulas: fixed 160-pixel horizontal
    /// blank (48 front porch, 32 sync), 3-line vertical front porch, at least 460 us of
    /// vertical blank, aspect-dependent vertical sync, 0.25 MHz clock step, +H/-V sync.
    pub fn cvt_rb(h: u16, v: u16, refresh_hz: u32) -> Result<Self, EdidError> {
        if h == 0 || h % 8 != 0 {
            return Err(EdidError::Cvt(
                "horizontal size must be a positive multiple of 8",
            ));
        }
        if v == 0 || !(24..=240).contains(&refresh_hz) {
            return Err(EdidError::Cvt("vertical size or refresh out of range"));
        }
        const H_BLANK: u32 = 160;
        const H_SYNC: u32 = 32;
        const H_FRONT: u32 = 48;
        const V_FRONT: u32 = 3;
        const MIN_V_BACK: u32 = 6;
        const MIN_V_BLANK_US: u64 = 460;
        const CLOCK_STEP_HZ: u64 = 250_000;
        let (h32, v32) = (h as u32, v as u32);
        let v_sync = match () {
            _ if h32 * 3 == v32 * 4 => 4,
            _ if h32 * 9 == v32 * 16 => 5,
            _ if h32 * 10 == v32 * 16 => 6,
            _ if h32 * 4 == v32 * 5 || h32 * 9 == v32 * 15 => 7,
            _ => 10,
        };
        // h_period_est = (1e6 / refresh - 460) / v  [us];
        // vbi_lines = floor(460 / h_period_est) + 1, in exact integer arithmetic.
        let r = refresh_hz as u64;
        let denom = 1_000_000 - MIN_V_BLANK_US * r;
        let vbi = (MIN_V_BLANK_US * v as u64 * r / denom) as u32 + 1;
        let vbi = vbi.max(V_FRONT + v_sync + MIN_V_BACK);
        let v_total = v32 + vbi;
        let h_total = h32 + H_BLANK;
        let clock_hz = r * v_total as u64 * h_total as u64 / CLOCK_STEP_HZ * CLOCK_STEP_HZ;
        let t = DetailedTiming {
            pixel_clock_khz: (clock_hz / 1000) as u32,
            h_active: h,
            h_front: H_FRONT as u16,
            h_sync: H_SYNC as u16,
            h_back: (H_BLANK - H_FRONT - H_SYNC) as u16,
            v_active: v,
            v_front: V_FRONT as u16,
            v_sync: v_sync as u16,
            v_back: (vbi - V_FRONT - v_sync) as u16,
            h_sync_positive: true,
            v_sync_positive: false,
            h_size_mm: 0,
            v_size_mm: 0,
        };
        Ok(t)
    }

    /// Same timing with an image size.
    pub fn with_size_mm(mut self, h: u16, v: u16) -> Self {
        self.h_size_mm = h;
        self.v_size_mm = v;
        self
    }

    /// The 18-byte detailed timing descriptor.
    pub fn encode(&self) -> Result<[u8; 18], EdidError> {
        fn fit(name: &'static str, v: u32, bits: u32) -> Result<u32, EdidError> {
            if v >> bits != 0 {
                Err(EdidError::Field(name, v as u64))
            } else {
                Ok(v)
            }
        }
        if self.pixel_clock_khz % 10 != 0 || self.pixel_clock_khz == 0 {
            return Err(EdidError::Field(
                "pixel clock kHz",
                self.pixel_clock_khz as u64,
            ));
        }
        let clk = fit("pixel clock", self.pixel_clock_khz / 10, 16)?;
        let ha = fit("h active", self.h_active as u32, 12)?;
        let hb = fit("h blank", self.h_blank() as u32, 12)?;
        let va = fit("v active", self.v_active as u32, 12)?;
        let vb = fit("v blank", self.v_blank() as u32, 12)?;
        let hfp = fit("h front porch", self.h_front as u32, 10)?;
        let hs = fit("h sync", self.h_sync as u32, 10)?;
        let vfp = fit("v front porch", self.v_front as u32, 6)?;
        let vs = fit("v sync", self.v_sync as u32, 6)?;
        let hmm = fit("h size mm", self.h_size_mm as u32, 12)?;
        let vmm = fit("v size mm", self.v_size_mm as u32, 12)?;
        let lo = |x: u32| (x & 0xFF) as u8;
        let mut d = [0u8; 18];
        d[0] = lo(clk);
        d[1] = (clk >> 8) as u8;
        d[2] = lo(ha);
        d[3] = lo(hb);
        d[4] = ((ha >> 8) << 4 | (hb >> 8)) as u8;
        d[5] = lo(va);
        d[6] = lo(vb);
        d[7] = ((va >> 8) << 4 | (vb >> 8)) as u8;
        d[8] = lo(hfp);
        d[9] = lo(hs);
        d[10] = ((vfp & 0xF) << 4 | (vs & 0xF)) as u8;
        d[11] = ((hfp >> 8) << 6 | (hs >> 8) << 4 | (vfp >> 4) << 2 | (vs >> 4)) as u8;
        d[12] = lo(hmm);
        d[13] = lo(vmm);
        d[14] = ((hmm >> 8) << 4 | (vmm >> 8)) as u8;
        // Digital separate sync (bits 4:3 = 11), polarities in bits 2 (V) and 1 (H).
        d[17] = 0x18
            | if self.v_sync_positive { 0x04 } else { 0 }
            | if self.h_sync_positive { 0x02 } else { 0 };
        Ok(d)
    }
}

/// Standard timing aspect ratio (EDID 1.3 and later encoding).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aspect {
    R16x10,
    R4x3,
    R5x4,
    R16x9,
}

/// One standard timing (2-byte code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StandardTiming {
    pub h_active: u16,
    pub aspect: Aspect,
    pub refresh_hz: u8,
}

impl StandardTiming {
    pub const fn new(h_active: u16, aspect: Aspect, refresh_hz: u8) -> Self {
        StandardTiming {
            h_active,
            aspect,
            refresh_hz,
        }
    }

    pub fn encode(&self) -> Result<[u8; 2], EdidError> {
        if self.h_active % 8 != 0 || !(256..=2288).contains(&self.h_active) {
            return Err(EdidError::Field("standard timing h", self.h_active as u64));
        }
        if !(60..=123).contains(&self.refresh_hz) {
            return Err(EdidError::Field(
                "standard timing refresh",
                self.refresh_hz as u64,
            ));
        }
        let ar = match self.aspect {
            Aspect::R16x10 => 0,
            Aspect::R4x3 => 1,
            Aspect::R5x4 => 2,
            Aspect::R16x9 => 3,
        };
        Ok([
            (self.h_active / 8 - 31) as u8,
            ar << 6 | (self.refresh_hz - 60),
        ])
    }
}

/// Display range limits descriptor (tag 0xFD), "range limits only" form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RangeLimits {
    pub v_min_hz: u8,
    pub v_max_hz: u8,
    pub h_min_khz: u8,
    pub h_max_khz: u8,
    /// Maximum pixel clock in MHz, a multiple of 10.
    pub max_pixel_clock_mhz: u16,
}

/// CTA-861 extension block: a video data block and a video capability data block (RGB
/// quantization selectable, IT and CE formats always underscanned). No audio, no HDR, no
/// vendor-specific blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CtaExtension {
    /// Short video descriptors (VICs 1-127).
    pub vics: Vec<u8>,
    /// VICs (1-64) flagged native.
    pub native: Vec<u8>,
}

impl CtaExtension {
    /// 1080p60 (VIC 16), 720p60 (VIC 4), 640x480p60 (VIC 1). None is flagged native: the
    /// native mode is the base block's CVT-RB detailed timing, which no VIC describes.
    pub fn basic() -> Self {
        CtaExtension {
            vics: vec![16, 4, 1],
            native: vec![],
        }
    }

    fn build(&self) -> Result<[u8; 128], EdidError> {
        if self.vics.is_empty() || self.vics.len() > 31 {
            return Err(EdidError::Field("CTA VIC count", self.vics.len() as u64));
        }
        let mut b = [0u8; 128];
        b[0] = 0x02;
        b[1] = 0x03;
        let mut i = 4;
        b[i] = 0x2 << 5 | self.vics.len() as u8;
        i += 1;
        for &vic in &self.vics {
            if vic == 0 || vic > 127 {
                return Err(EdidError::Field("VIC", vic as u64));
            }
            let native = self.native.contains(&vic) && vic <= 64;
            b[i] = vic | if native { 0x80 } else { 0 };
            i += 1;
        }
        // Video capability data block: extended tag 0x00; QS = 1, S_IT = S_CE = 10.
        b[i..i + 3].copy_from_slice(&[0x7 << 5 | 2, 0x00, 0x4A]);
        i += 3;
        // No detailed timings in the extension: the offset points past the data blocks.
        b[2] = i as u8;
        // Underscans IT formats by default; no basic audio, no YCbCr; zero native DTDs.
        b[3] = 0x80;
        b[127] = checksum_byte(&b[..127]);
        Ok(b)
    }
}

/// Everything that goes into the EDID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdidSpec {
    /// Three letters, the PNP manufacturer id.
    pub manufacturer: [u8; 3],
    pub product_code: u16,
    pub serial: u32,
    pub week: u8,
    pub year: u16,
    /// Bits per colour: 6, 8, 10, 12, 14 or 16.
    pub bpc: u8,
    pub screen_cm: (u8, u8),
    pub preferred: DetailedTiming,
    /// Established timings I and II and the manufacturer byte (0x23-0x25).
    pub established: [u8; 3],
    pub standard: Vec<StandardTiming>,
    pub name: String,
    pub range: RangeLimits,
    pub cta: Option<CtaExtension>,
}

/// sRGB primaries and D65 white as 10-bit CIE xy fractions (x * 1024, rounded).
const SRGB: [(u32, u32); 4] = [
    (6400, 3300), // red
    (3000, 6000), // green
    (1500, 600),  // blue
    (3127, 3290), // white
];

impl EdidSpec {
    /// The first Caraxes monitor (briefing section 3.2).
    pub fn first_monitor() -> Self {
        EdidSpec {
            manufacturer: *b"CRX",
            product_code: 0x0001,
            serial: 0,
            week: 39,
            year: 2026,
            bpc: 8,
            screen_cm: (53, 30),
            preferred: DetailedTiming::cvt_rb(1920, 1080, 60)
                .expect("1920x1080 is a CVT size")
                .with_size_mm(527, 296),
            // 640x480@60 and 800x600@60; 1024x768@60.
            established: [0x21, 0x08, 0x00],
            standard: vec![
                StandardTiming::new(1280, Aspect::R16x9, 60),
                StandardTiming::new(1280, Aspect::R5x4, 60),
                StandardTiming::new(1440, Aspect::R16x10, 60),
                StandardTiming::new(1600, Aspect::R16x9, 60),
                StandardTiming::new(1680, Aspect::R16x10, 60),
            ],
            name: "Caraxes DP".to_string(),
            range: RangeLimits {
                v_min_hz: 56,
                v_max_hz: 61,
                h_min_khz: 30,
                h_max_khz: 70,
                max_pixel_clock_mhz: 150,
            },
            cta: None,
        }
    }

    /// Build the EDID: 128 bytes, or 256 with the CTA extension.
    pub fn build(&self) -> Result<Vec<u8>, EdidError> {
        let mut b = [0u8; 128];
        b[0..8].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
        let mut id = 0u16;
        for &c in &self.manufacturer {
            if !c.is_ascii_uppercase() {
                return Err(EdidError::Manufacturer(self.manufacturer));
            }
            id = id << 5 | (c - b'A' + 1) as u16;
        }
        b[8..10].copy_from_slice(&id.to_be_bytes());
        b[10..12].copy_from_slice(&self.product_code.to_le_bytes());
        b[12..16].copy_from_slice(&self.serial.to_le_bytes());
        if self.week > 54 {
            return Err(EdidError::Field("week", self.week as u64));
        }
        b[16] = self.week;
        if !(1990..=2245).contains(&self.year) {
            return Err(EdidError::Field("year", self.year as u64));
        }
        b[17] = (self.year - 1990) as u8;
        b[18] = 1;
        b[19] = 4;
        let depth = match self.bpc {
            6 => 1,
            8 => 2,
            10 => 3,
            12 => 4,
            14 => 5,
            16 => 6,
            other => return Err(EdidError::Field("bpc", other as u64)),
        };
        // Digital input, colour depth, interface 0101 = DisplayPort.
        b[20] = 0x80 | depth << 4 | 0x05;
        b[21] = self.screen_cm.0;
        b[22] = self.screen_cm.1;
        // Gamma 2.2: (2.2 * 100) - 100.
        b[23] = 120;
        // No DPM states; RGB 4:4:4 only; sRGB default colour space; preferred timing mode
        // includes native pixel format and refresh; not continuous frequency.
        b[24] = 0x06;
        let c: Vec<u32> = SRGB
            .iter()
            .flat_map(|&(x, y)| [(x * 1024 + 5000) / 10000, (y * 1024 + 5000) / 10000])
            .collect();
        let low2 = |v: u32| (v & 0x3) as u8;
        b[25] = low2(c[0]) << 6 | low2(c[1]) << 4 | low2(c[2]) << 2 | low2(c[3]);
        b[26] = low2(c[4]) << 6 | low2(c[5]) << 4 | low2(c[6]) << 2 | low2(c[7]);
        for (i, v) in c.iter().enumerate() {
            b[27 + i] = (v >> 2) as u8;
        }
        b[35..38].copy_from_slice(&self.established);
        if self.standard.len() > 8 {
            return Err(EdidError::TooManyStandardTimings(self.standard.len()));
        }
        for i in 0..8 {
            let code = match self.standard.get(i) {
                Some(s) => s.encode()?,
                None => [0x01, 0x01],
            };
            b[38 + 2 * i..40 + 2 * i].copy_from_slice(&code);
        }
        b[54..72].copy_from_slice(&self.preferred.encode()?);
        b[72..90].copy_from_slice(&name_descriptor(&self.name)?);
        b[90..108].copy_from_slice(&range_descriptor(&self.range)?);
        // Dummy descriptor (tag 0x10).
        b[108..126].copy_from_slice(&display_descriptor(0x10, [0; 13]));
        b[126] = u8::from(self.cta.is_some());
        b[127] = checksum_byte(&b[..127]);
        let mut out = b.to_vec();
        if let Some(cta) = &self.cta {
            out.extend_from_slice(&cta.build()?);
        }
        Ok(out)
    }
}

fn display_descriptor(tag: u8, data: [u8; 13]) -> [u8; 18] {
    let mut d = [0u8; 18];
    d[3] = tag;
    d[5..].copy_from_slice(&data);
    d
}

fn name_descriptor(name: &str) -> Result<[u8; 18], EdidError> {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > 13 || !bytes.iter().all(|c| (0x20..0x7F).contains(c)) {
        return Err(EdidError::Name(name.to_string()));
    }
    let mut data = [0x20u8; 13];
    data[..bytes.len()].copy_from_slice(bytes);
    if bytes.len() < 13 {
        data[bytes.len()] = 0x0A;
    }
    Ok(display_descriptor(0xFC, data))
}

fn range_descriptor(r: &RangeLimits) -> Result<[u8; 18], EdidError> {
    if r.v_min_hz == 0 || r.v_min_hz > r.v_max_hz || r.h_min_khz == 0 || r.h_min_khz > r.h_max_khz {
        return Err(EdidError::Field("range limits", 0));
    }
    if r.max_pixel_clock_mhz % 10 != 0 || !(10..=2550).contains(&r.max_pixel_clock_mhz) {
        return Err(EdidError::Field(
            "max pixel clock MHz",
            r.max_pixel_clock_mhz as u64,
        ));
    }
    let mut d = display_descriptor(0xFD, [0; 13]);
    d[4] = 0; // no rate offsets
    d[5] = r.v_min_hz;
    d[6] = r.v_max_hz;
    d[7] = r.h_min_khz;
    d[8] = r.h_max_khz;
    d[9] = (r.max_pixel_clock_mhz / 10) as u8;
    d[10] = 0x01; // range limits only, no timing formula
    d[11] = 0x0A;
    for x in &mut d[12..18] {
        *x = 0x20;
    }
    Ok(d)
}

/// The byte that makes `bytes` plus it sum to 0 modulo 256.
pub fn checksum_byte(bytes: &[u8]) -> u8 {
    0u8.wrapping_sub(bytes.iter().fold(0u8, |a, &x| a.wrapping_add(x)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cvt_rb_matches_published_values() {
        // Published CVT-RB values (VESA DMT and CVT tables), not computed here.
        let t = DetailedTiming::cvt_rb(1920, 1080, 60).unwrap();
        assert_eq!(t.pixel_clock_khz, 138_500);
        assert_eq!((t.h_front, t.h_sync, t.h_back), (48, 32, 80));
        assert_eq!((t.v_front, t.v_sync, t.v_back), (3, 5, 23));
        assert_eq!((t.h_total(), t.v_total()), (2080, 1111));
        assert_eq!(t.refresh_mhz(), 59_933);
        // 1920x1200 RB: 154.0 MHz, 2080x1235 (DMT 0x44).
        let t = DetailedTiming::cvt_rb(1920, 1200, 60).unwrap();
        assert_eq!(
            (t.pixel_clock_khz, t.v_total(), t.v_sync),
            (154_000, 1235, 6)
        );
        // 2560x1440 RB: 241.5 MHz, 2720x1481.
        let t = DetailedTiming::cvt_rb(2560, 1440, 60).unwrap();
        assert_eq!((t.pixel_clock_khz, t.v_total()), (241_500, 1481));
        // DMT reduced-blanking entries: 1440x900 (0x2E), 1680x1050 (0x39), 2560x1600 (0x4C).
        let t = DetailedTiming::cvt_rb(1440, 900, 60).unwrap();
        assert_eq!(
            (t.pixel_clock_khz, t.h_total(), t.v_total()),
            (88_750, 1600, 926)
        );
        let t = DetailedTiming::cvt_rb(1680, 1050, 60).unwrap();
        assert_eq!(
            (t.pixel_clock_khz, t.h_total(), t.v_total()),
            (119_000, 1840, 1080)
        );
        let t = DetailedTiming::cvt_rb(2560, 1600, 60).unwrap();
        assert_eq!(
            (t.pixel_clock_khz, t.h_total(), t.v_total()),
            (268_500, 2720, 1646)
        );
        assert!(DetailedTiming::cvt_rb(1366, 768, 60).is_err());
    }

    #[test]
    fn first_monitor_layout() {
        let e = EdidSpec::first_monitor().build().unwrap();
        assert_eq!(e.len(), 128);
        assert_eq!(&e[0..8], &[0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0]);
        assert_eq!(e[18..20], [1, 4]);
        assert_eq!(e[20], 0xA5, "digital, 8 bpc, DisplayPort");
        assert_eq!(e[126], 0, "no extension");
        assert_eq!(e.iter().fold(0u8, |a, &x| a.wrapping_add(x)), 0);
    }

    #[test]
    fn cta_extension_is_optional() {
        let mut s = EdidSpec::first_monitor();
        s.cta = Some(CtaExtension::basic());
        let e = s.build().unwrap();
        assert_eq!(e.len(), 256);
        assert_eq!(e[126], 1);
        assert_eq!(
            &e[128..139],
            &[0x02, 0x03, 0x0B, 0x80, 0x43, 0x10, 0x04, 0x01, 0xE2, 0x00, 0x4A]
        );
        assert_eq!(e[128..].iter().fold(0u8, |a, &x| a.wrapping_add(x)), 0);
    }

    #[test]
    fn bad_inputs_are_rejected() {
        let mut s = EdidSpec::first_monitor();
        s.name = "a name that is too long".into();
        assert!(matches!(s.build(), Err(EdidError::Name(_))));
        let mut s = EdidSpec::first_monitor();
        s.manufacturer = *b"cr1";
        assert!(matches!(s.build(), Err(EdidError::Manufacturer(_))));
        let mut s = EdidSpec::first_monitor();
        s.preferred.pixel_clock_khz = 138_505;
        assert!(matches!(s.build(), Err(EdidError::Field(..))));
        let mut s = EdidSpec::first_monitor();
        s.preferred.v_front = 64;
        assert!(matches!(
            s.build(),
            Err(EdidError::Field("v front porch", 64))
        ));
    }
}
