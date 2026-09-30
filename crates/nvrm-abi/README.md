<!-- SPDX-License-Identifier: MIT -->
# nvrm-abi

Shared: the RM ioctl ABI and the naming model.

- Describes RM and UVM ioctl payloads: sizes, embedded pointers, FD fields and encoding.
  `src/xlate.rs` holds the per-call translation rules behind Leandro VirtIO's forwarding and
  the tables its guest module reads.
- Curated ABI types (`src/nvgpu.rs`), status names and the GPU naming model
  (`src/naming.rs`) that both versions use for the name a guest sees.
- `nvrm-genhdr` (`src/bin/`) generates the guest module's C header;
  `cargo run --bin nvrm-genhdr -- --check` finds a stale one.
- Docs: [reference](reference.md) (source map, editing rules),
  [RM controls](../../docs/wiki/rm-controls.md),
  [PCI config and identity](../../docs/wiki/pci-config-and-identity.md).
