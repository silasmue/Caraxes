<!-- SPDX-License-Identifier: MIT -->
# dp-sink

Leandro VFIO: the monitor at the end of the emulated DisplayPort connector.

- A transport-neutral DisplayPort SST sink: DPCD registers, link training, the AUX engine with
  I2C-over-AUX, the EDID and hot-plug detect. It knows nothing about NVIDIA, vfio-user or GSP.
- The device's display engine drives it through its NV0073 AUX adapter
  (`crates/vfio-user-nvgpu/src/display/aux.rs`); the guest driver detects the monitor, reads
  its EDID and trains the link against it.
- Entry points: `src/sink.rs` and `src/lib.rs`; one file per part (`dpcd.rs`, `link.rs`,
  `aux.rs`, `ddc.rs`, `edid.rs`, `hpd.rs`); `examples/edid.rs` prints the EDID.
- Test: `cargo test -p dp-sink`; `cargo run -q --example edid | edid-decode --check` checks
  the EDID.
- Docs: [reference](reference.md) (contract, DPCD profile, AUX, EDID, tests),
  [display and KMS](../../docs/wiki/display-and-kms.md).
