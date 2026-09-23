// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The sink: DPCD, DDC and HPD behind one AUX entry point.

use crate::aux::{AuxCommand, AuxError, AuxReply, AuxRequest, Fault, ReplyCode};
use crate::ddc::{Ddc, I2cBusState, I2cDir, I2cResult};
use crate::dpcd::{reg, Dpcd};
use crate::hpd::{Hpd, HpdEvent, HpdLevel, IrqCause};
use crate::link::{LinkConfig, LinkError, LinkState};
use crate::profile::{ProfileError, SinkProfile};
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

/// Why a sink could not be built or reconfigured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SinkError {
    Profile(ProfileError),
    /// EDID length is not a non-zero multiple of 128 bytes up to 128 segments.
    EdidSize(usize),
}

impl fmt::Display for SinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SinkError::Profile(e) => write!(f, "profile: {e}"),
            SinkError::EdidSize(n) => write!(f, "EDID of {n} bytes is not whole 128-byte blocks"),
        }
    }
}

impl std::error::Error for SinkError {}

fn check_edid(edid: &[u8]) -> Result<(), SinkError> {
    if edid.is_empty() || edid.len() % 128 != 0 || edid.len() > 128 * 256 {
        return Err(SinkError::EdidSize(edid.len()));
    }
    Ok(())
}

/// One DisplayPort SST sink.
#[derive(Clone, Debug)]
pub struct DpSink {
    dpcd: Dpcd,
    ddc: Ddc,
    hpd: Hpd,
    faults: VecDeque<Fault>,
}

impl DpSink {
    /// A sink with `profile` and `edid`, initially unplugged.
    pub fn new(profile: SinkProfile, edid: Vec<u8>) -> Result<Self, SinkError> {
        profile.validate().map_err(SinkError::Profile)?;
        check_edid(&edid)?;
        Ok(DpSink {
            dpcd: Dpcd::new(profile),
            ddc: Ddc::new(edid),
            hpd: Hpd::new(),
            faults: VecDeque::new(),
        })
    }

    pub fn profile(&self) -> &SinkProfile {
        self.dpcd.profile()
    }

    pub fn dpcd(&self) -> &Dpcd {
        &self.dpcd
    }

    pub fn edid(&self) -> &[u8] {
        self.ddc.edid()
    }

    pub fn i2c_state(&self) -> I2cBusState {
        self.ddc.state()
    }

    pub fn link_state(&self) -> LinkState {
        self.dpcd.state()
    }

    pub fn is_plugged(&self) -> bool {
        self.hpd.level() == HpdLevel::High
    }

    pub fn hpd(&self) -> &Hpd {
        &self.hpd
    }

    /// Pending HPD events, oldest first.
    pub fn take_events(&mut self) -> Vec<HpdEvent> {
        self.hpd.take()
    }

    /// Plug the monitor in: HPD high (long pulse), DPCD at power-on values, DDC reset.
    pub fn plug(&mut self) -> bool {
        if !self.hpd.plug() {
            return false;
        }
        self.dpcd.connect();
        self.ddc.reset();
        true
    }

    /// Unplug: HPD low (long pulse); AUX requests time out until the next plug.
    pub fn unplug(&mut self) -> bool {
        if !self.hpd.unplug() {
            return false;
        }
        self.dpcd.disconnect();
        self.ddc.reset();
        true
    }

    /// Replace the EDID. While plugged this is a replug (unplug and plug long pulses), the
    /// way a real monitor signals a changed EDID.
    pub fn set_edid(&mut self, edid: Vec<u8>) -> Result<(), SinkError> {
        check_edid(&edid)?;
        let plugged = self.is_plugged();
        if plugged {
            self.unplug();
        }
        self.ddc.set_edid(edid);
        if plugged {
            self.plug();
        }
        Ok(())
    }

    /// The sink loses the link on its own (for tests of the source's recovery). Raises a
    /// short pulse when a trained or training link was lost.
    pub fn drop_link(&mut self) -> bool {
        if self.dpcd.link_lost() {
            self.hpd.irq(IrqCause::LinkStatus);
            true
        } else {
            false
        }
    }

    /// Set `DEVICE_SERVICE_IRQ_VECTOR` bits and raise a short pulse. False when unplugged.
    pub fn service_irq(&mut self, bits: u8) -> bool {
        if !self.is_plugged() {
            return false;
        }
        self.dpcd.raise_service_irq(bits);
        self.hpd.irq(IrqCause::DeviceService(bits))
    }

    /// Queue a fault reply for a later request.
    pub fn inject(&mut self, fault: Fault) {
        self.faults.push_back(fault);
    }

    /// AUX channel reset (source side timeout recovery, or device reset): any I2C
    /// transaction in progress is abandoned and not continued; DPCD is untouched.
    pub fn aux_reset(&mut self) {
        self.ddc.reset();
    }

    /// Run one AUX request against the sink.
    pub fn transact(&mut self, req: &AuxRequest) -> Result<AuxReply, AuxError> {
        req.validate()?;
        if !self.is_plugged() {
            return Err(AuxError::Timeout);
        }
        if let Some(f) = self.faults.pop_front() {
            let native = req.cmd.is_native();
            return match f {
                Fault::Timeout => Err(AuxError::Timeout),
                Fault::AuxDefer => Ok(AuxReply::status(ReplyCode::Defer, 0)),
                Fault::AuxNack => Ok(AuxReply::status(ReplyCode::Nack, 0)),
                Fault::I2cDefer if native => Ok(AuxReply::status(ReplyCode::Defer, 0)),
                Fault::I2cNack if native => Ok(AuxReply::status(ReplyCode::Nack, 0)),
                Fault::I2cDefer => Ok(AuxReply::status(ReplyCode::I2cDefer, 0)),
                Fault::I2cNack => Ok(AuxReply::status(ReplyCode::I2cNack, 0)),
            };
        }
        Ok(match req.cmd {
            AuxCommand::NativeRead | AuxCommand::NativeWrite => self.native(req),
            AuxCommand::I2cWriteStatus { mot } => {
                // Writes complete immediately in this model, so a status request finds
                // nothing outstanding; it ends the transaction unless MOT is set.
                if !mot {
                    self.ddc.stop();
                }
                AuxReply::write_ack(0)
            }
            AuxCommand::I2cRead { mot } | AuxCommand::I2cWrite { mot } => {
                let dir = if req.cmd.is_read() {
                    I2cDir::Read
                } else {
                    I2cDir::Write
                };
                let max = self.profile().max_i2c_reply as usize;
                match self
                    .ddc
                    .transfer(req.addr as u8, dir, mot, &req.data, req.len, max)
                {
                    I2cResult::Ack { data, written } => {
                        if dir == I2cDir::Read {
                            AuxReply::read_ack(data)
                        } else {
                            AuxReply::write_ack(written)
                        }
                    }
                    I2cResult::Nack { written } => AuxReply::status(ReplyCode::I2cNack, written),
                }
            }
        })
    }

    fn native(&mut self, req: &AuxRequest) -> AuxReply {
        let last = req.addr as u64 + req.len as u64 - 1;
        if last > reg::ADDR_MAX as u64 {
            // The request runs past the end of the DPCD address space.
            return AuxReply::status(ReplyCode::Nack, 0);
        }
        if req.cmd == AuxCommand::NativeRead {
            let mut buf = vec![0; req.len];
            self.dpcd.read(req.addr, &mut buf);
            AuxReply::read_ack(buf)
        } else {
            self.dpcd.write(req.addr, &req.data);
            AuxReply::write_ack(req.len)
        }
    }

    /// Train the link to `cfg` through the sink's own DPCD, the way a source would
    /// (TPS1, then the best equalization pattern, then pattern 0). For transports whose
    /// training happens behind a control (NVIDIA `DP_CTRL`) rather than AUX writes; the
    /// DPCD ends up in the same state either way.
    pub fn train(&mut self, cfg: LinkConfig) -> Result<(), LinkError> {
        if !self.is_plugged() {
            return Err(LinkError::Disconnected);
        }
        let refused = |s: LinkState| match s {
            LinkState::Refused(e) => Err(e),
            _ => Ok(()),
        };
        self.dpcd
            .write(reg::LINK_BW_SET, &[cfg.bw_byte(), cfg.lane_byte()]);
        self.dpcd
            .write(reg::TRAINING_PATTERN_SET, &[0x21, 0, 0, 0, 0]);
        refused(self.dpcd.state())?;
        let eq = match self.profile().best_eq_pattern() {
            4 => 0x27,
            3 => 0x23,
            _ => 0x22,
        };
        self.dpcd.write(reg::TRAINING_PATTERN_SET, &[eq]);
        refused(self.dpcd.state())?;
        self.dpcd.write(reg::TRAINING_PATTERN_SET, &[0x00]);
        match self.dpcd.state() {
            LinkState::Trained => Ok(()),
            LinkState::Refused(e) => Err(e),
            _ => Err(LinkError::ClockRecoveryNotDone),
        }
    }
}

/// Several sinks addressed by a transport-chosen port id (a head, a connector, or an
/// NVIDIA display id bit in the adapter).
#[derive(Clone, Debug, Default)]
pub struct SinkPorts {
    ports: BTreeMap<u32, DpSink>,
}

impl SinkPorts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, port: u32, sink: DpSink) -> Option<DpSink> {
        self.ports.insert(port, sink)
    }

    pub fn get(&self, port: u32) -> Option<&DpSink> {
        self.ports.get(&port)
    }

    pub fn get_mut(&mut self, port: u32) -> Result<&mut DpSink, AuxError> {
        self.ports.get_mut(&port).ok_or(AuxError::NoSuchPort(port))
    }

    pub fn ports(&self) -> impl Iterator<Item = u32> + '_ {
        self.ports.keys().copied()
    }

    pub fn transact(&mut self, port: u32, req: &AuxRequest) -> Result<AuxReply, AuxError> {
        self.get_mut(port)?.transact(req)
    }
}
