// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The driver-version axis (decision D14): everything the device speaks that the
//! vendored driver release fixes and that no chip changes. That is the GSP RPC
//! vocabulary and its message layouts (`rpc_*_v03_00`, `rpc_*_v17_00`,
//! `GspStaticConfigInfo`, `GspSystemInfo`, `GspFwWprMeta`, the libos arguments),
//! the RM class and control numbers, and the control parameter layouts the
//! handlers in `ctrl.rs`, `kgr.rs`, `nvml.rs`, `uvm.rs` and `channel.rs` read and
//! write (docs/caraxes/other-architectures.md §6: one ABI per driver release, shared
//! by every chip).
//!
//! One module per vendored driver version, each behind a feature of the same name,
//! mirroring `crates/nvrm-sys`. The modules export the same names; generic code
//! reaches them through [`active`], the version this build emulates. A build
//! emulates exactly one driver version, because the guest accepts only the GSP
//! firmware whose `.fwversion` equals its own `NV_VERSION_STRING`
//! (docs/caraxes/gsp-abi-610.md (e), "Version selection"); the device notes a guest
//! that announces another version in `SET_GUEST_SYSTEM_INFO`.
//!
//! Where `nvrm-sys` has a typed layout for a parameter struct, the offsets and sizes
//! here are computed from it (`offset_of!`, `size_of`) and the tests pin them to the
//! numbers the handlers were written against. The GSP message structs are not in
//! `nvrm-sys` (gsp-abi-610.md (e) proposes a generated `nvgsp-sys` for them); their
//! hand-written offsets are checked against the compiler-measured tables of
//! gsp-abi-610.md (a) instead. How to add a version: docs/caraxes/versatility.md.

#[cfg(feature = "v610")]
pub mod v610;

/// The driver version this build emulates.
#[cfg(feature = "v610")]
pub use v610 as active;

#[cfg(not(feature = "v610"))]
compile_error!("vfio-user-nvgpu needs a driver-version feature (v610)");

/// The compiler-measured layout tables of docs/caraxes/gsp-abi-610.md (a), parsed
/// for the tests: `(struct, field) -> offset`, and `struct -> sizeof`.
#[cfg(test)]
pub(crate) mod measured {
    use std::collections::BTreeMap;

    pub struct Tables {
        pub sizes: BTreeMap<String, usize>,
        pub fields: BTreeMap<(String, String), usize>,
    }

    /// Parse the `#### \`name\`: sizeof N` headings and the `| offset | hex | size |
    /// field |` rows below them.
    pub fn parse(md: &str) -> Tables {
        let mut sizes = BTreeMap::new();
        let mut fields = BTreeMap::new();
        let mut current: Option<String> = None;
        for line in md.lines() {
            if let Some(rest) = line.strip_prefix("#### `") {
                let Some((name, tail)) = rest.split_once('`') else {
                    current = None;
                    continue;
                };
                current = Some(name.to_string());
                if let Some(n) = tail
                    .strip_prefix(": sizeof ")
                    .and_then(|t| t.split_whitespace().next())
                    .and_then(|t| t.parse().ok())
                {
                    sizes.insert(name.to_string(), n);
                }
                continue;
            }
            if line.starts_with("## ") || line.starts_with("### ") {
                current = None;
                continue;
            }
            let Some(s) = &current else { continue };
            let cols: Vec<&str> = line.split('|').map(str::trim).collect();
            if cols.len() >= 5 {
                if let Ok(off) = cols[1].parse::<usize>() {
                    let field = cols[4].trim_matches('`');
                    let field = field.split('`').next().unwrap_or(field);
                    fields.insert((s.clone(), field.to_string()), off);
                }
            }
        }
        Tables { sizes, fields }
    }
}
