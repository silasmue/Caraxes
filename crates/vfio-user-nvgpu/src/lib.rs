// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Caraxes: a synthetic NVIDIA PCI device served over vfio-user for unmodified
//! NVIDIA guest drivers (docs/caraxes/architecture.md). The first device is a TU106
//! (GeForce RTX 2070) for driver 610.57.04.
//!
//! What varies is kept on three axes (decision D14, docs/caraxes/versatility.md):
//! the silicon in [`chip`] (a trait plus data files per chip, `chips/<chip>/`), the
//! board in [`board`] (`boards/<board>.toml` plus the captured config space and ROM),
//! and the driver version in [`ver`] (one module per vendored driver, behind a
//! feature). Everything else is generic: the vfio-user device, the BAR0 window, the
//! GSP message queues and RPC service, the phase-5 dispatcher and the fake FB.

pub mod bar0;
pub mod bar1pool;
pub mod bar2area;
pub mod board;
pub mod boot;
pub mod channel;
pub mod chip;
pub mod chipctl;
pub mod classes;
pub mod compslice;
pub mod config;
pub mod ctrl;
pub mod cuda;
pub mod device;
pub mod display;
pub mod dmamap;
pub mod events;
pub mod dmodel;
pub mod deferred;
pub mod falcon;
pub mod families;
pub mod fwdcache;
pub mod fb;
pub mod gmcapi;
pub mod gspboot;
pub mod gspmem;
pub mod gsprpc;
pub mod gmmu_v3;
pub mod guestmem;
pub mod hostabi;
pub mod hostfloor;
pub mod hostmem;
pub mod hostrm;
pub mod intr;
pub mod intr_page;
pub mod inv_page;
pub mod um_page;
pub mod invalidate;
pub mod invgate;
pub mod kgr;
pub mod launch;
pub mod lazy;
pub mod mirror;
pub mod msgq;
pub mod nvml;
pub mod privilege;
pub mod poll;
pub mod profile;
pub mod pdbwatch;
pub mod ptcache;
pub mod public;
pub mod rc;
pub mod recorded;
pub mod rpc;
pub mod rusd;
pub mod selftest;
pub mod shadow;
pub mod soak;
pub mod tokens;
pub mod trace;
pub mod untrapped;
pub mod userd;
pub mod uvm;
pub mod vbios;
pub mod zbc;
pub mod ver;

#[cfg(test)]
mod costume;
#[cfg(test)]
pub(crate) mod fixture;
