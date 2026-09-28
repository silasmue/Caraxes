// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The NV0073 adapter of `DP_AUXCH_CTRL` (0x731341) onto the transport-neutral sink
//! (dp-sink README section 6, display-adapter.md sections 1.2 and 7 item 1).
//!
//! The NVIDIA encoding stays here; the sink works on real byte counts:
//! - request `size` is the count **minus one**, except that an address-only I2C
//!   transaction (`bAddrOnly`) has no data and `size` 0; `bAddrOnly` is tested first;
//! - reply `size` is the count **actually transferred**, `replyType` the DP reply code
//!   (ACK 0, NACK 1, DEFER 2, I2C NACK 4, I2C DEFER 8), and a sink-level NACK or DEFER is
//!   `NV_OK` plus `replyType`: only a malformed request is a control error;
//! - an unplugged sink (no transaction) is `NV_OK` with `replyType` TIMEOUT (3), which
//!   the DP library treats as a NACK.

use super::{get32, put32, Display, NV_ERR_INVALID_ARGUMENT};
use crate::ctrl::Answer;
use crate::ver::common::display as d;
use crate::ver::common::NV_OK;
use dp_sink::transcript::{format_outcome, format_request};
use dp_sink::{AuxCommand, AuxError, AuxRequest};

/// `NV0073_CTRL_DP_AUXCH_REPLYTYPE_TIMEOUT`.
const REPLYTYPE_TIMEOUT: u32 = 3;

const LABEL: &str = "synthesized (dp-sink model behind the connector, NV0073 AUX adapter)";

/// Decode the control's parameters into a sink request.
pub fn decode(params: &[u8]) -> Result<AuxRequest, String> {
    let cmd = get32(params, d::AUX_CMD) & 0xF;
    let cmd =
        AuxCommand::from_code(cmd as u8).ok_or_else(|| format!("reserved AUX command {cmd:#x}"))?;
    let addr_only = params.get(d::AUX_ADDR_ONLY).copied().unwrap_or(0) != 0;
    let size = get32(params, d::AUX_SIZE_FIELD) as usize;
    let len = if addr_only { 0 } else { size + 1 };
    let data = match cmd {
        AuxCommand::NativeWrite | AuxCommand::I2cWrite { .. } => params
            .get(d::AUX_DATA..d::AUX_DATA + len.min(d::AUX_DATA_LEN))
            .map(<[u8]>::to_vec)
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    // A write status request carries no payload; its length is what the size says.
    let len = match cmd {
        AuxCommand::I2cWriteStatus { .. } => 0,
        _ => len,
    };
    Ok(AuxRequest {
        cmd,
        addr: get32(params, d::AUX_ADDR) & 0xF_FFFF,
        len,
        data,
    })
}

/// Serve one `DP_AUXCH_CTRL`; returns the answer and the transcript lines (request and
/// outcome in dp-sink's format) for the replay test.
pub fn transact(disp: &mut Display, params: &[u8]) -> (Answer, Option<String>) {
    let did = get32(params, d::DISPLAY_ID);
    let mut b = params.to_vec();
    let req = match decode(params) {
        Ok(r) => r,
        Err(e) => {
            put32(&mut b, d::AUX_REPLY_TYPE, d::AUX_REPLYTYPE_INVALID_ARGUMENT);
            return (
                Answer {
                    status: NV_ERR_INVALID_ARGUMENT,
                    params: b,
                    label: LABEL,
                    note: format!("aux {did:#x}: {e}"),
                },
                None,
            );
        }
    };
    let result = match disp.connectors.iter_mut().find(|c| c.display_id == did) {
        Some(c) => c.sink.transact(&req),
        None => Err(AuxError::NoSuchPort(did)),
    };
    let line = format!("> {}\n< {}", format_request(&req), format_outcome(&result));
    let (status, note) = match &result {
        Ok(rep) => {
            put32(&mut b, d::AUX_SIZE_FIELD, rep.count as u32);
            put32(&mut b, d::AUX_REPLY_TYPE, rep.code.code() as u32);
            if req.cmd.is_read() {
                let at = d::AUX_DATA;
                b[at..at + d::AUX_DATA_LEN].fill(0);
                let n = rep.data.len().min(d::AUX_DATA_LEN);
                b[at..at + n].copy_from_slice(&rep.data[..n]);
            }
            (NV_OK, "")
        }
        Err(AuxError::Timeout) => {
            put32(&mut b, d::AUX_SIZE_FIELD, 0);
            put32(&mut b, d::AUX_REPLY_TYPE, REPLYTYPE_TIMEOUT);
            (NV_OK, " (no sink: replyType TIMEOUT)")
        }
        Err(_) => {
            put32(&mut b, d::AUX_SIZE_FIELD, 0);
            put32(&mut b, d::AUX_REPLY_TYPE, d::AUX_REPLYTYPE_INVALID_ARGUMENT);
            (NV_ERR_INVALID_ARGUMENT, " (not a transaction)")
        }
    };
    put32(&mut b, d::AUX_RETRY_TIME_MS, 0);
    (
        Answer {
            status,
            params: b,
            label: LABEL,
            note: format!(
                "aux {did:#x}: {} -> {}{note}",
                format_request(&req),
                format_outcome(&result)
            ),
        },
        Some(line),
    )
}
