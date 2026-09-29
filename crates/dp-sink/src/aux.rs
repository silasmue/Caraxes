// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! AUX transactions as the DP specification defines them, on real byte counts.
//!
//! Two result levels are kept apart:
//! - [`AuxReply`]: the sink answered. Its [`ReplyCode`] is the AUX reply (ACK, NACK,
//!   DEFER) combined with the I2C reply for I2C-over-AUX (I2C NACK, I2C DEFER), and
//!   `count` is the number of bytes actually transferred;
//! - [`AuxError`]: no AUX transaction took place (the request cannot be encoded, or no
//!   sink answered). A transport maps these onto its own error channel.
//!
//! Nothing here knows any vendor encoding. The NVIDIA control (NV0073 `DP_AUXCH_CTRL`,
//! size minus one in the request) is an adapter concern; see the README.

use std::fmt;

/// Most data bytes one AUX request or reply carries.
pub const AUX_MAX_PAYLOAD: usize = 16;

/// The 4-bit request command of an AUX header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuxCommand {
    NativeWrite,
    NativeRead,
    I2cWrite {
        mot: bool,
    },
    I2cRead {
        mot: bool,
    },
    /// I2C write status update request (asks for the progress of a deferred write).
    I2cWriteStatus {
        mot: bool,
    },
}

impl AuxCommand {
    /// The request command nibble (bit 3 native, bit 2 MOT, bits 1:0 request type).
    pub fn code(self) -> u8 {
        let mot = |m: bool| if m { 0x4 } else { 0 };
        match self {
            AuxCommand::NativeWrite => 0x8,
            AuxCommand::NativeRead => 0x9,
            AuxCommand::I2cWrite { mot: m } => mot(m),
            AuxCommand::I2cRead { mot: m } => 0x1 | mot(m),
            AuxCommand::I2cWriteStatus { mot: m } => 0x2 | mot(m),
        }
    }

    /// Decode a request command nibble; `None` for reserved encodings.
    pub fn from_code(code: u8) -> Option<Self> {
        let mot = code & 0x4 != 0;
        match code {
            0x8 => Some(AuxCommand::NativeWrite),
            0x9 => Some(AuxCommand::NativeRead),
            0x0 | 0x4 => Some(AuxCommand::I2cWrite { mot }),
            0x1 | 0x5 => Some(AuxCommand::I2cRead { mot }),
            0x2 | 0x6 => Some(AuxCommand::I2cWriteStatus { mot }),
            _ => None,
        }
    }

    pub fn is_native(self) -> bool {
        matches!(self, AuxCommand::NativeWrite | AuxCommand::NativeRead)
    }

    pub fn is_read(self) -> bool {
        matches!(self, AuxCommand::NativeRead | AuxCommand::I2cRead { .. })
    }

    pub fn mot(self) -> bool {
        match self {
            AuxCommand::I2cWrite { mot }
            | AuxCommand::I2cRead { mot }
            | AuxCommand::I2cWriteStatus { mot } => mot,
            _ => false,
        }
    }
}

/// One AUX request. For writes `data` is the payload and `len == data.len()`; for reads
/// `len` is the requested count and `data` is empty. An I2C request with `len == 0` is an
/// address-only transaction. Constructors do not validate, so malformed requests can be
/// represented; [`crate::DpSink::transact`] rejects them with [`AuxError`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuxRequest {
    pub cmd: AuxCommand,
    /// DPCD address (20 bits) for native requests, 7-bit I2C address otherwise.
    pub addr: u32,
    pub len: usize,
    pub data: Vec<u8>,
}

impl AuxRequest {
    pub fn native_read(addr: u32, len: usize) -> Self {
        AuxRequest {
            cmd: AuxCommand::NativeRead,
            addr,
            len,
            data: Vec::new(),
        }
    }

    pub fn native_write(addr: u32, data: &[u8]) -> Self {
        AuxRequest {
            cmd: AuxCommand::NativeWrite,
            addr,
            len: data.len(),
            data: data.to_vec(),
        }
    }

    pub fn i2c_read(addr: u8, len: usize, mot: bool) -> Self {
        AuxRequest {
            cmd: AuxCommand::I2cRead { mot },
            addr: addr as u32,
            len,
            data: Vec::new(),
        }
    }

    pub fn i2c_write(addr: u8, data: &[u8], mot: bool) -> Self {
        AuxRequest {
            cmd: AuxCommand::I2cWrite { mot },
            addr: addr as u32,
            len: data.len(),
            data: data.to_vec(),
        }
    }

    pub fn i2c_write_status(addr: u8, mot: bool) -> Self {
        AuxRequest {
            cmd: AuxCommand::I2cWriteStatus { mot },
            addr: addr as u32,
            len: 0,
            data: Vec::new(),
        }
    }

    /// I2C request without data: with MOT it opens a transaction (START + address), without
    /// MOT it is START + address + STOP.
    pub fn is_address_only(&self) -> bool {
        !self.cmd.is_native() && self.len == 0
    }

    /// Structural validation (length and address encodability).
    pub fn validate(&self) -> Result<(), AuxError> {
        let bad_len = AuxError::InvalidLength { len: self.len };
        if self.len > AUX_MAX_PAYLOAD {
            return Err(bad_len);
        }
        match self.cmd {
            AuxCommand::NativeRead | AuxCommand::NativeWrite if self.len == 0 => {
                return Err(bad_len)
            }
            AuxCommand::NativeWrite | AuxCommand::I2cWrite { .. }
                if self.data.len() != self.len =>
            {
                return Err(bad_len)
            }
            AuxCommand::NativeRead | AuxCommand::I2cRead { .. } if !self.data.is_empty() => {
                return Err(bad_len)
            }
            AuxCommand::I2cWriteStatus { .. } if self.len != 0 || !self.data.is_empty() => {
                return Err(bad_len)
            }
            _ => {}
        }
        let limit = if self.cmd.is_native() { 0xF_FFFF } else { 0x7F };
        if self.addr > limit {
            return Err(AuxError::InvalidAddress(self.addr));
        }
        Ok(())
    }
}

/// Reply code; the numeric values are the DP reply field (AUX bits 1:0, I2C bits 3:2) and
/// coincide with NV0073 `DP_AUXCH_REPLYTYPE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyCode {
    Ack,
    Nack,
    Defer,
    /// AUX ACK with I2C NACK.
    I2cNack,
    /// AUX ACK with I2C DEFER.
    I2cDefer,
}

impl ReplyCode {
    pub fn code(self) -> u8 {
        match self {
            ReplyCode::Ack => 0x0,
            ReplyCode::Nack => 0x1,
            ReplyCode::Defer => 0x2,
            ReplyCode::I2cNack => 0x4,
            ReplyCode::I2cDefer => 0x8,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ReplyCode::Ack => "ack",
            ReplyCode::Nack => "nack",
            ReplyCode::Defer => "defer",
            ReplyCode::I2cNack => "i2c-nack",
            ReplyCode::I2cDefer => "i2c-defer",
        }
    }
}

/// The sink's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuxReply {
    pub code: ReplyCode,
    /// Bytes actually transferred: returned bytes for a read (equal to `data.len()`, may be
    /// fewer than requested), accepted bytes for a write (on NACK, the M of the spec).
    pub count: usize,
    /// Read data (empty for writes and for anything but ACK).
    pub data: Vec<u8>,
}

impl AuxReply {
    pub fn read_ack(data: Vec<u8>) -> Self {
        AuxReply {
            code: ReplyCode::Ack,
            count: data.len(),
            data,
        }
    }

    pub fn write_ack(count: usize) -> Self {
        AuxReply {
            code: ReplyCode::Ack,
            count,
            data: Vec::new(),
        }
    }

    pub fn status(code: ReplyCode, count: usize) -> Self {
        AuxReply {
            code,
            count,
            data: Vec::new(),
        }
    }
}

/// No AUX transaction happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuxError {
    /// Length outside 1..=16 for native, above 16 for I2C, or payload size not matching.
    InvalidLength { len: usize },
    /// Address does not fit the 20-bit DPCD or 7-bit I2C field.
    InvalidAddress(u32),
    /// No sink answered (HPD low, or an injected timeout).
    Timeout,
    /// The port a transport addressed does not exist ([`crate::SinkPorts`]).
    NoSuchPort(u32),
}

impl AuxError {
    pub fn name(self) -> &'static str {
        match self {
            AuxError::InvalidLength { .. } => "invalid-length",
            AuxError::InvalidAddress(_) => "invalid-address",
            AuxError::Timeout => "timeout",
            AuxError::NoSuchPort(_) => "no-such-port",
        }
    }
}

impl fmt::Display for AuxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuxError::InvalidLength { len } => write!(f, "invalid AUX length {len}"),
            AuxError::InvalidAddress(a) => write!(f, "invalid AUX address {a:#x}"),
            AuxError::Timeout => write!(f, "AUX timeout (no sink)"),
            AuxError::NoSuchPort(p) => write!(f, "no such port {p}"),
        }
    }
}

impl std::error::Error for AuxError {}

/// A reply the sink gives instead of its normal one, for testing a source's retry paths.
/// Queued faults are consumed one per valid request, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// AUX DEFER (any request).
    AuxDefer,
    /// AUX NACK (any request).
    AuxNack,
    /// AUX ACK + I2C DEFER; on a native request it becomes AUX DEFER.
    I2cDefer,
    /// AUX ACK + I2C NACK; on a native request it becomes AUX NACK.
    I2cNack,
    /// No reply at all.
    Timeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_codes_round_trip() {
        for code in 0..16u8 {
            if let Some(c) = AuxCommand::from_code(code) {
                assert_eq!(c.code(), code);
            }
        }
        assert_eq!(AuxCommand::from_code(0x3), None);
        assert_eq!(AuxCommand::from_code(0xA), None);
        assert_eq!(AuxCommand::I2cRead { mot: true }.code(), 0x5);
    }

    #[test]
    fn validation_rules() {
        assert!(AuxRequest::native_read(0, 1).validate().is_ok());
        assert!(AuxRequest::native_read(0, 16).validate().is_ok());
        assert_eq!(
            AuxRequest::native_read(0, 0).validate(),
            Err(AuxError::InvalidLength { len: 0 })
        );
        assert_eq!(
            AuxRequest::native_read(0, 17).validate(),
            Err(AuxError::InvalidLength { len: 17 })
        );
        assert_eq!(
            AuxRequest::native_write(0x100, &[0; 17]).validate(),
            Err(AuxError::InvalidLength { len: 17 })
        );
        let mut w = AuxRequest::native_write(0x100, &[1, 2]);
        w.len = 3;
        assert_eq!(w.validate(), Err(AuxError::InvalidLength { len: 3 }));
        assert!(AuxRequest::i2c_read(0x50, 0, true).validate().is_ok());
        assert!(AuxRequest::i2c_write(0x50, &[], false).validate().is_ok());
        assert_eq!(
            AuxRequest::native_read(0x10_0000, 1).validate(),
            Err(AuxError::InvalidAddress(0x10_0000))
        );
        assert_eq!(
            AuxRequest::i2c_read(0x80, 1, false).validate(),
            Err(AuxError::InvalidAddress(0x80))
        );
    }
}
