// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The DDC bus behind I2C-over-AUX: an E-DDC EDID EEPROM at 0x50 and the segment pointer
//! at 0x30 (7-bit addresses; 0xA0 and 0x60 in 8-bit notation).
//!
//! Transaction model (the sink's view of the I2C bus):
//! - a request whose address or direction differs from the open transaction issues a
//!   (repeated) START with that address; an absent device, or a read of the write-only
//!   segment pointer, answers the address phase with I2C NACK;
//! - a request with MOT = 0 ends with STOP; a request with MOT = 1 keeps the transaction
//!   open, so the next request to the same address and direction continues it;
//! - the segment pointer is reset to 0 by every STOP (E-DDC), so it only applies to the
//!   reads of the transaction that wrote it;
//! - the EEPROM word offset persists across STOP (a current-address read continues), and
//!   is reset to 0 only by a bus reset (hotplug, AUX reset);
//! - a write to 0x50 or 0x30 carries one pointer byte; further data bytes are refused with
//!   I2C NACK after one byte (the EEPROM is write-protected);
//! - a read returns at most `max_reply` bytes and never runs past the end of the EDID data
//!   or of the 256-byte segment: the reply is then short; a read that starts past the end
//!   is answered with I2C NACK.

/// 7-bit I2C address of the EDID EEPROM.
pub const DDC_EDID_ADDR: u8 = 0x50;
/// 7-bit I2C address of the E-DDC segment pointer.
pub const DDC_SEGMENT_ADDR: u8 = 0x30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum I2cDir {
    Read,
    Write,
}

/// Outcome of one I2C-over-AUX request at the I2C level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum I2cResult {
    /// I2C ACK; for reads, the returned bytes (possibly fewer than requested); for writes,
    /// the number of bytes accepted.
    Ack { data: Vec<u8>, written: usize },
    /// I2C NACK after `written` data bytes (0 for an address-phase NACK).
    Nack { written: usize },
}

/// Bus state visible for tests and traces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct I2cBusState {
    /// Open transaction (MOT continuation pending): address and direction.
    pub open: Option<(u8, I2cDir)>,
    pub segment: u8,
    pub offset: u8,
}

#[derive(Clone, Debug)]
pub struct Ddc {
    edid: Vec<u8>,
    open: Option<(u8, I2cDir)>,
    segment: u8,
    offset: u8,
}

impl Ddc {
    pub fn new(edid: Vec<u8>) -> Self {
        Ddc {
            edid,
            open: None,
            segment: 0,
            offset: 0,
        }
    }

    pub fn edid(&self) -> &[u8] {
        &self.edid
    }

    pub fn set_edid(&mut self, edid: Vec<u8>) {
        self.edid = edid;
        self.reset();
    }

    pub fn state(&self) -> I2cBusState {
        I2cBusState {
            open: self.open,
            segment: self.segment,
            offset: self.offset,
        }
    }

    /// Bus reset: abandon any open transaction, segment and offset back to 0. A transfer
    /// begun before the reset is not continued afterwards.
    pub fn reset(&mut self) {
        self.open = None;
        self.segment = 0;
        self.offset = 0;
    }

    /// STOP: end the open transaction (the segment pointer returns to 0).
    pub fn stop(&mut self) {
        self.open = None;
        self.segment = 0;
    }

    fn present(addr: u8, dir: I2cDir) -> bool {
        match addr {
            DDC_EDID_ADDR => true,
            DDC_SEGMENT_ADDR => dir == I2cDir::Write,
            _ => false,
        }
    }

    /// One I2C-over-AUX request. `data` is the write payload (empty for reads and
    /// address-only requests); `read_len` is the requested read size (0 for address-only).
    pub fn transfer(
        &mut self,
        addr: u8,
        dir: I2cDir,
        mot: bool,
        data: &[u8],
        read_len: usize,
        max_reply: usize,
    ) -> I2cResult {
        if self.open != Some((addr, dir)) {
            // (Repeated) START with this address.
            if !Self::present(addr, dir) {
                self.stop();
                return I2cResult::Nack { written: 0 };
            }
            self.open = Some((addr, dir));
        }
        let result = match dir {
            I2cDir::Write => self.write_data(addr, data),
            I2cDir::Read => self.read_data(read_len, max_reply),
        };
        if !mot || matches!(result, I2cResult::Nack { .. }) {
            self.stop();
        }
        result
    }

    fn write_data(&mut self, addr: u8, data: &[u8]) -> I2cResult {
        let Some(&first) = data.first() else {
            return I2cResult::Ack {
                data: Vec::new(),
                written: 0,
            };
        };
        match addr {
            DDC_SEGMENT_ADDR => self.segment = first,
            _ => self.offset = first,
        }
        if data.len() > 1 {
            I2cResult::Nack { written: 1 }
        } else {
            I2cResult::Ack {
                data: Vec::new(),
                written: 1,
            }
        }
    }

    fn read_data(&mut self, len: usize, max_reply: usize) -> I2cResult {
        if len == 0 {
            return I2cResult::Ack {
                data: Vec::new(),
                written: 0,
            };
        }
        let pos = self.segment as usize * 256 + self.offset as usize;
        let seg_left = 256 - self.offset as usize;
        let avail = self.edid.len().saturating_sub(pos);
        let n = len.min(max_reply).min(seg_left).min(avail);
        if n == 0 {
            return I2cResult::Nack { written: 0 };
        }
        let out = self.edid[pos..pos + n].to_vec();
        self.offset = self.offset.wrapping_add(n as u8);
        I2cResult::Ack {
            data: out,
            written: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edid(blocks: usize) -> Vec<u8> {
        (0..blocks * 128).map(|i| (i % 251) as u8).collect()
    }

    fn read(d: &mut Ddc, len: usize, mot: bool) -> I2cResult {
        d.transfer(DDC_EDID_ADDR, I2cDir::Read, mot, &[], len, 16)
    }

    #[test]
    fn offset_write_then_read() {
        let e = edid(1);
        let mut d = Ddc::new(e.clone());
        assert_eq!(
            d.transfer(DDC_EDID_ADDR, I2cDir::Write, true, &[0x20], 0, 16),
            I2cResult::Ack {
                data: vec![],
                written: 1
            }
        );
        assert_eq!(
            read(&mut d, 16, true),
            I2cResult::Ack {
                data: e[0x20..0x30].to_vec(),
                written: 0
            }
        );
        assert_eq!(d.state().open, Some((DDC_EDID_ADDR, I2cDir::Read)));
        assert_eq!(d.state().offset, 0x30);
    }

    #[test]
    fn short_read_at_end_and_nack_past_end() {
        let e = edid(1);
        let mut d = Ddc::new(e.clone());
        d.transfer(DDC_EDID_ADDR, I2cDir::Write, true, &[120], 0, 16);
        assert_eq!(
            read(&mut d, 16, true),
            I2cResult::Ack {
                data: e[120..128].to_vec(),
                written: 0
            }
        );
        assert_eq!(read(&mut d, 16, false), I2cResult::Nack { written: 0 });
        assert_eq!(d.state().open, None);
    }

    #[test]
    fn segment_pointer_applies_until_stop() {
        let e = edid(4);
        let mut d = Ddc::new(e.clone());
        d.transfer(DDC_SEGMENT_ADDR, I2cDir::Write, true, &[1], 0, 16);
        d.transfer(DDC_EDID_ADDR, I2cDir::Write, true, &[0x80], 0, 16);
        assert_eq!(
            read(&mut d, 16, false),
            I2cResult::Ack {
                data: e[384..400].to_vec(),
                written: 0
            }
        );
        // STOP reset the segment: the same offset now reads block 1.
        assert_eq!(d.state().segment, 0);
        d.transfer(DDC_EDID_ADDR, I2cDir::Write, true, &[0x80], 0, 16);
        assert_eq!(
            read(&mut d, 16, false),
            I2cResult::Ack {
                data: e[128..144].to_vec(),
                written: 0
            }
        );
    }

    #[test]
    fn absent_devices_and_extra_bytes_nack() {
        let mut d = Ddc::new(edid(1));
        assert_eq!(
            d.transfer(0x51, I2cDir::Read, false, &[], 1, 16),
            I2cResult::Nack { written: 0 }
        );
        assert_eq!(
            d.transfer(DDC_SEGMENT_ADDR, I2cDir::Read, false, &[], 1, 16),
            I2cResult::Nack { written: 0 }
        );
        assert_eq!(
            d.transfer(DDC_EDID_ADDR, I2cDir::Write, false, &[0, 1, 2], 0, 16),
            I2cResult::Nack { written: 1 }
        );
        // Missing segment: reads NACK.
        d.transfer(DDC_SEGMENT_ADDR, I2cDir::Write, true, &[5], 0, 16);
        assert_eq!(read(&mut d, 1, false), I2cResult::Nack { written: 0 });
    }

    #[test]
    fn max_reply_shortens_reads() {
        let e = edid(1);
        let mut d = Ddc::new(e.clone());
        assert_eq!(
            d.transfer(DDC_EDID_ADDR, I2cDir::Read, false, &[], 16, 5),
            I2cResult::Ack {
                data: e[0..5].to_vec(),
                written: 0
            }
        );
        assert_eq!(d.state().offset, 5);
    }
}
