// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! What one profile hook costs (src/profile.rs, docs/caraxes/coverage-map.md §1): the aggregate's
//! own work per trapped access, control and window hit, measured on this host with no device.
//! A trapped access costs the guest 23 to 35 us (optimization-backlog.md §4); the hook's share is
//! the ratio printed here.
//!
//! Run: cargo run --release -p vfio-user-nvgpu --example profile-cost

use std::time::{Duration, Instant};

use vfio_user_nvgpu::profile::{Key, Profile};

fn per_op(n: u32, f: &mut dyn FnMut(u32)) -> f64 {
    let t0 = Instant::now();
    for i in 0..n {
        f(i);
    }
    t0.elapsed().as_nanos() as f64 / n as f64
}

fn main() {
    let n = 2_000_000;
    let mut p = Profile::new();
    let t = Instant::now();
    // 64 BAR0 registers and 512 BAR2 pages, as a context creation touches them.
    let access = per_op(n, &mut |i| {
        let (region, off) = if i % 4 == 0 {
            (0, 0xb830a0 + 4 * (i as u64 % 64))
        } else {
            (3, 0x100_000 + 0x1000 * (i as u64 % 512))
        };
        p.access(region, i % 2 == 0, off, None, t, Instant::now());
    });
    let control = per_op(n, &mut |i| {
        p.record(Key::Control(0x2080_0100 + i % 200), Duration::from_nanos(900), 0, Some(0), "x");
    });
    let window = per_op(n, &mut |_| {
        p.record(Key::Window("token pool: eventfd rings (units: guest writes)"), Duration::ZERO, 1, None, "");
    });
    let t0 = Instant::now();
    p.client_open(1, Instant::now());
    let open = t0.elapsed();
    let t0 = Instant::now();
    p.client_close(1, Instant::now());
    let close = t0.elapsed();
    let rows = p.rows().len();
    let t0 = Instant::now();
    let tsv = p.tsv("cost", Instant::now());
    let dump = t0.elapsed();
    let off = per_op(n, &mut |_| {
        std::hint::black_box(vfio_user_nvgpu::profile::on());
    });
    println!("profile hook cost on this host ({n} calls each, {rows} rows):");
    println!("  trapped access    {access:.0} ns (a trap costs the guest 23-35 us: {:.2} %)", access / 250.0);
    println!("  control           {control:.0} ns");
    println!("  window hit        {window:.0} ns");
    println!("  profile off check {off:.1} ns");
    println!("  client open/close {} / {} us (the snapshot and the difference)", open.as_micros(), close.as_micros());
    println!("  TSV dump          {} us, {} bytes", dump.as_micros(), tsv.len());
}
