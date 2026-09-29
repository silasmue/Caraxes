// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The host's view of the scanout: a window's latched surface as a PNG, on request
//! (`echo dump > <state dir>/display-ctl`, or `dump <path>`). The guest is not involved:
//! the device reads the surface the window scans out from, undoes the block-linear
//! layout and writes the file. For checking a guest driver's presentation without a
//! capture tool in the guest.
//!
//! Block-linear surfaces (NVC57E `SET_STORAGE`: block height 3:0 in GOBs, clc57e.h:114-123;
//! `SET_PLANAR_STORAGE` pitch in blocks of one GOB width): a GOB is 64 bytes by 8 rows, a
//! block one GOB wide and 2^h GOBs high, blocks run along the row first. Inside a GOB the
//! bytes of (x, y) sit at ((x & 32) << 3) | ((y & 6) << 5) | ((x & 16) << 1) | ((y & 1) << 4)
//! | (x & 15), x in bytes: 16-byte by 2-row sectors, the GOB layout of the desktop GPUs
//! since Fermi as nouveau and Mesa implement it (unverified against a Turing capture).

/// Byte offset of byte `x` of row `y` in a block-linear surface `pitch_gobs` GOBs wide
/// with blocks 2^`bh_log2` GOBs high.
pub fn block_linear_offset(x: u64, y: u64, pitch_gobs: u64, bh_log2: u32) -> u64 {
    let gob_rows = 8u64 << bh_log2;
    let block_row = y / gob_rows;
    let gob_in_block = (y % gob_rows) / 8;
    let (gx, yy) = (x % 64, y % 8);
    let in_gob = ((gx & 32) << 3) | ((yy & 6) << 5) | ((gx & 16) << 1) | ((yy & 1) << 4) | (gx & 15);
    ((block_row * pitch_gobs + x / 64) << bh_log2 | gob_in_block) * 512 + in_gob
}

/// RGB of one 32-bit pixel of an NVC57E `SET_PARAMS` format (clc57e.h:127-137); `None` for
/// a format this dump does not decode.
pub fn rgb(format: u32, px: u32) -> Option<[u8; 3]> {
    let b = px.to_le_bytes();
    match format {
        // A8R8G8B8, X8R8G8B8: bytes B, G, R, A.
        0xCF | 0xE6 => Some([b[2], b[1], b[0]]),
        // A8B8G8R8, X8B8G8R8: bytes R, G, B, A.
        0xD5 | 0xF9 => Some([b[0], b[1], b[2]]),
        // A2R10G10B10: B 9:0, G 19:10, R 29:20.
        0xDF => Some([(px >> 22) as u8, (px >> 12) as u8, (px >> 2) as u8]),
        // A2B10G10R10: R 9:0, G 19:10, B 29:20.
        0xD1 => Some([(px >> 2) as u8, (px >> 12) as u8, (px >> 22) as u8]),
        _ => None,
    }
}

/// A PNG of `width` x `height` RGB pixels (8 bits each), deflate with stored blocks only
/// (RFC 1950/1951; PNG specification ISO/IEC 15948, IHDR colour type 2).
pub fn png(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    // Filter byte 0 before each row.
    let row = width as usize * 3;
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for y in 0..height as usize {
        raw.push(0);
        raw.extend_from_slice(&rgb[y * row..(y + 1) * row]);
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, b) in blocks.iter().enumerate() {
        z.push((i + 1 == blocks.len()) as u8);
        z.extend_from_slice(&(b.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut a, mut s) = (1u32, 0u32);
    for &x in &raw {
        a = (a + u32::from(x)) % 65521;
        s = (s + a) % 65521;
    }
    z.extend_from_slice(&((s << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gob_and_block_layout() {
        // The first GOB: 16-byte runs, rows interleaved per the sector layout.
        assert_eq!(block_linear_offset(0, 0, 1, 0), 0);
        assert_eq!(block_linear_offset(15, 0, 1, 0), 15);
        assert_eq!(block_linear_offset(0, 1, 1, 0), 16);
        assert_eq!(block_linear_offset(16, 0, 1, 0), 32);
        assert_eq!(block_linear_offset(0, 2, 1, 0), 64);
        assert_eq!(block_linear_offset(32, 0, 1, 0), 256);
        assert_eq!(block_linear_offset(63, 7, 1, 0), 511);
        // Blocks of 16 GOBs: row 8 is the next GOB of the same block; x 64 the next block.
        assert_eq!(block_linear_offset(0, 8, 120, 4), 512);
        assert_eq!(block_linear_offset(64, 0, 120, 4), 16 * 512);
        // Row 128 starts the next row of blocks: 120 blocks of 16 GOBs further.
        assert_eq!(block_linear_offset(0, 128, 120, 4), 120 * 16 * 512);
        // Every byte of a surface two blocks (of two GOBs) wide and two blocks high lands
        // once: 128 bytes by 32 rows.
        let mut seen = vec![false; 2 * 2 * 2 * 512];
        for y in 0..32 {
            for x in 0..128 {
                let o = block_linear_offset(x, y, 2, 1) as usize;
                assert!(!seen[o]);
                seen[o] = true;
            }
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn formats_and_png() {
        assert_eq!(rgb(0xCF, 0xFF11_2233), Some([0x11, 0x22, 0x33]));
        assert_eq!(rgb(0xD5, 0xFF11_2233), Some([0x33, 0x22, 0x11]));
        assert_eq!(rgb(0xDF, 0x3FF << 20), Some([0xFF, 0, 0]));
        assert_eq!(rgb(0x1E, 0), None);
        let p = png(2, 1, &[1, 2, 3, 4, 5, 6]);
        assert_eq!(&p[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&p[12..16], b"IHDR");
        assert_eq!(&p[p.len() - 8..p.len() - 4], b"IEND");
        // The IEND CRC is the PNG constant.
        assert_eq!(&p[p.len() - 4..], &[0xAE, 0x42, 0x60, 0x82]);
    }
}
