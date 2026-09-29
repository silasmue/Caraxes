// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Write the first monitor's EDID to stdout, binary (`--cta` adds the CTA-861 extension).
//! Usage: cargo run --example edid [-- --cta] | edid-decode --check

use dp_sink::{CtaExtension, EdidSpec};
use std::io::Write;

fn main() {
    let mut spec = EdidSpec::first_monitor();
    if std::env::args().any(|a| a == "--cta") {
        spec.cta = Some(CtaExtension::basic());
    }
    let edid = spec.build().expect("first monitor EDID builds");
    std::io::stdout()
        .write_all(&edid)
        .expect("write EDID to stdout");
}
