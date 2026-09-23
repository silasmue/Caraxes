// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! AUX transcripts: a line format for request/reply sequences, a replayer that checks
//! every reply exactly, and a recorder.
//!
//! ```text
//! # comment
//! ! plug                      action: plug | unplug | aux-reset | drop-link
//! ! fault i2c-defer           queue a fault: aux-defer | aux-nack | i2c-defer | i2c-nack | timeout
//! ! service-irq 02            set DEVICE_SERVICE_IRQ_VECTOR bits (short pulse)
//! ? events plug irq-link      expect exactly these pending HPD events (and consume them)
//! > nr 000 16                 native read: address (hex), length (decimal)
//! > nw 100 0a 84              native write: address, data bytes (hex)
//! > ir 50 16 / irm 50 16      I2C read without / with MOT: 7-bit address, length
//! > iw 50 00 / iwm 50 00      I2C write without / with MOT: address, data
//! > is 50 / ism 50            I2C write status update request
//! < ack 16 11 0a 84 ...       reply: code, byte count, read data
//! < i2c-nack 1                reply codes: ack nack defer i2c-nack i2c-defer
//! < err timeout               transport error: invalid-length invalid-address timeout
//! ```
//!
//! Every `>` line must be followed by exactly one `<` line.

use crate::aux::{AuxCommand, AuxError, AuxReply, AuxRequest, Fault};
use crate::hpd::{HpdEvent, IrqCause};
use crate::sink::DpSink;
use std::fmt::Write as _;

fn hex_bytes(out: &mut String, bytes: &[u8]) {
    for b in bytes {
        let _ = write!(out, " {b:02x}");
    }
}

/// Canonical text of a request (without the leading `>`).
pub fn format_request(req: &AuxRequest) -> String {
    let mut s = String::new();
    match req.cmd {
        AuxCommand::NativeRead => {
            let _ = write!(s, "nr {:03x} {}", req.addr, req.len);
        }
        AuxCommand::NativeWrite => {
            let _ = write!(s, "nw {:03x}", req.addr);
            hex_bytes(&mut s, &req.data);
        }
        AuxCommand::I2cRead { mot } => {
            let _ = write!(
                s,
                "ir{} {:02x} {}",
                if mot { "m" } else { "" },
                req.addr,
                req.len
            );
        }
        AuxCommand::I2cWrite { mot } => {
            let _ = write!(s, "iw{} {:02x}", if mot { "m" } else { "" }, req.addr);
            hex_bytes(&mut s, &req.data);
        }
        AuxCommand::I2cWriteStatus { mot } => {
            let _ = write!(s, "is{} {:02x}", if mot { "m" } else { "" }, req.addr);
        }
    }
    s
}

/// Canonical text of an outcome (without the leading `<`).
pub fn format_outcome(r: &Result<AuxReply, AuxError>) -> String {
    match r {
        Ok(rep) => {
            let mut s = format!("{} {}", rep.code.name(), rep.count);
            hex_bytes(&mut s, &rep.data);
            s
        }
        Err(e) => format!("err {}", e.name()),
    }
}

fn format_event(e: &HpdEvent) -> String {
    match e {
        HpdEvent::Plug => "plug".into(),
        HpdEvent::Unplug => "unplug".into(),
        HpdEvent::Irq(IrqCause::LinkStatus) => "irq-link".into(),
        HpdEvent::Irq(IrqCause::DeviceService(b)) => format!("irq-service:{b:02x}"),
    }
}

fn parse_hex(tok: &str) -> Result<u32, String> {
    u32::from_str_radix(tok, 16).map_err(|_| format!("bad hex {tok:?}"))
}

fn parse_byte(tok: &str) -> Result<u8, String> {
    u8::from_str_radix(tok, 16).map_err(|_| format!("bad byte {tok:?}"))
}

/// Parse a request line body (without `>`).
pub fn parse_request(line: &str) -> Result<AuxRequest, String> {
    let mut t = line.split_whitespace();
    let op = t.next().ok_or("empty request")?;
    let addr = parse_hex(t.next().ok_or("missing address")?)?;
    let rest: Vec<&str> = t.collect();
    let len = || -> Result<usize, String> {
        match rest.as_slice() {
            [n] => n.parse().map_err(|_| format!("bad length {n:?}")),
            _ => Err("expected one length".into()),
        }
    };
    let data = || -> Result<Vec<u8>, String> { rest.iter().map(|b| parse_byte(b)).collect() };
    let i2c_addr = || -> Result<u8, String> {
        u8::try_from(addr).map_err(|_| format!("I2C address {addr:#x} too large"))
    };
    let req = match op {
        "nr" => AuxRequest::native_read(addr, len()?),
        "nw" => AuxRequest::native_write(addr, &data()?),
        "ir" | "irm" => AuxRequest::i2c_read(i2c_addr()?, len()?, op == "irm"),
        "iw" | "iwm" => AuxRequest::i2c_write(i2c_addr()?, &data()?, op == "iwm"),
        "is" | "ism" => {
            if !rest.is_empty() {
                return Err("write status takes no data".into());
            }
            AuxRequest::i2c_write_status(i2c_addr()?, op == "ism")
        }
        other => return Err(format!("unknown request {other:?}")),
    };
    Ok(req)
}

fn parse_fault(name: &str) -> Result<Fault, String> {
    Ok(match name {
        "aux-defer" => Fault::AuxDefer,
        "aux-nack" => Fault::AuxNack,
        "i2c-defer" => Fault::I2cDefer,
        "i2c-nack" => Fault::I2cNack,
        "timeout" => Fault::Timeout,
        other => return Err(format!("unknown fault {other:?}")),
    })
}

fn normalize(s: &str) -> String {
    s.split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

/// A replay failure: the line (1-based) and what went wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ReplayError {}

/// Replay `script` against `sink`. Returns the number of request/reply pairs checked.
pub fn replay(sink: &mut DpSink, script: &str) -> Result<usize, ReplayError> {
    let mut pending: Option<(usize, String, String)> = None;
    let mut checked = 0;
    for (i, raw) in script.lines().enumerate() {
        let n = i + 1;
        let err = |message: String| ReplayError { line: n, message };
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (tag, body) = line.split_at(1);
        let body = body.trim();
        if tag != "<" {
            if let Some((l, req, _)) = &pending {
                return Err(ReplayError {
                    line: *l,
                    message: format!("request {req:?} has no expected reply"),
                });
            }
        }
        match tag {
            ">" => {
                let req = parse_request(body).map_err(err)?;
                let got = format_outcome(&sink.transact(&req));
                pending = Some((n, format_request(&req), got));
            }
            "<" => {
                let (_, req, got) = pending
                    .take()
                    .ok_or_else(|| err("reply without request".into()))?;
                let want = normalize(body);
                if want != got {
                    return Err(err(format!(
                        "request {req:?}: expected {want:?}, sink replied {got:?}"
                    )));
                }
                checked += 1;
            }
            "!" => {
                let mut t = body.split_whitespace();
                match (t.next(), t.next()) {
                    (Some("plug"), None) => {
                        sink.plug();
                    }
                    (Some("unplug"), None) => {
                        sink.unplug();
                    }
                    (Some("aux-reset"), None) => sink.aux_reset(),
                    (Some("drop-link"), None) => {
                        sink.drop_link();
                    }
                    (Some("fault"), Some(f)) => sink.inject(parse_fault(f).map_err(err)?),
                    (Some("service-irq"), Some(b)) => {
                        sink.service_irq(parse_byte(b).map_err(err)?);
                    }
                    _ => return Err(err(format!("unknown action {body:?}"))),
                }
            }
            "?" => {
                let mut t = body.split_whitespace();
                if t.next() != Some("events") {
                    return Err(err(format!("unknown expectation {body:?}")));
                }
                let want: Vec<String> = t.map(str::to_ascii_lowercase).collect();
                let got: Vec<String> = sink.take_events().iter().map(format_event).collect();
                if want != got {
                    return Err(err(format!("expected events {want:?}, got {got:?}")));
                }
            }
            _ => return Err(err(format!("unknown line {line:?}"))),
        }
    }
    if let Some((l, req, _)) = pending {
        return Err(ReplayError {
            line: l,
            message: format!("request {req:?} has no expected reply"),
        });
    }
    Ok(checked)
}

/// Run `requests` against `sink` and write them with the sink's replies as a transcript.
pub fn record(sink: &mut DpSink, requests: &[AuxRequest]) -> String {
    let mut s = String::new();
    for req in requests {
        let out = sink.transact(req);
        let _ = writeln!(s, "> {}", format_request(req));
        let _ = writeln!(s, "< {}", format_outcome(&out));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip() {
        for line in [
            "nr 000 16",
            "nw 100 0a 84",
            "ir 50 1",
            "irm 50 16",
            "iw 30 01",
            "iwm 50",
            "is 50",
            "ism 50",
        ] {
            let req = parse_request(line).unwrap();
            assert_eq!(format_request(&req), line);
        }
        assert!(parse_request("xx 0 1").is_err());
        assert!(parse_request("ir 150 1").is_err());
        assert!(parse_request("nr 000").is_err());
    }
}
