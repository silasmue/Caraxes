// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! A transport-neutral DisplayPort SST sink: the monitor side of an emulated DP connector.
//!
//! - [`dpcd`]: the DPCD register file and the link-training state machine;
//! - [`aux`]: AUX requests and replies on real byte counts (1-16), reply codes kept apart
//!   from transport errors;
//! - [`ddc`]: I2C-over-AUX, the EDID EEPROM and the E-DDC segment pointer;
//! - [`edid`]: the EDID generator;
//! - [`hpd`]: hot-plug level and long/short pulses;
//! - [`sink`]: [`DpSink`], which ties them together behind one `transact` call;
//! - [`transcript`]: a text format for AUX request/reply sequences and a replayer.
//!
//! No vendor ABI lives here; the NVIDIA control mapping belongs to an adapter (README).

pub mod aux;
pub mod ddc;
pub mod dpcd;
pub mod edid;
pub mod hpd;
pub mod link;
pub mod profile;
pub mod sink;
pub mod transcript;

pub use aux::{AuxCommand, AuxError, AuxReply, AuxRequest, Fault, ReplyCode, AUX_MAX_PAYLOAD};
pub use ddc::{I2cBusState, I2cDir, DDC_EDID_ADDR, DDC_SEGMENT_ADDR};
pub use edid::{CtaExtension, DetailedTiming, EdidSpec};
pub use hpd::{HpdEvent, HpdLevel, IrqCause};
pub use link::{LinkConfig, LinkError, LinkState, TrainingPattern};
pub use profile::{LinkRate, SinkProfile};
pub use sink::{DpSink, SinkError, SinkPorts};
