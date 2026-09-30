<!-- SPDX-License-Identifier: MIT -->
# nvrm-sys

Shared: the generated NVIDIA driver bindings every host-side crate builds on.

- Committed bindgen output and layout manifests per configured driver version
  (`src/v<NNN>.rs`, `manifests/`), selected by features; a normal build needs neither
  libclang nor vendored headers.
- Used by `vhost-user-nvrm`, `vfio-user-nvgpu`, `nvrm-abi`, `nvrm-client` and `nvrm-trace`.
- Key files: `abi.toml` (the maintained version entries), `src/lib.rs` (generated dispatch),
  `src/version.rs` (running-driver detection, handwritten).
- Regenerate with `cargo xtask abi`, verify with `cargo xtask abi --check`; edit `abi.toml`
  or the generator, never the output.
- Docs: [reference](reference.md), [driver versions](../../docs/virtio/abi-versions.md),
  [guest drivers and ABIs](../../docs/wiki/guest-drivers-and-abis.md).
