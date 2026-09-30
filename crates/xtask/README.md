<!-- SPDX-License-Identifier: MIT -->
# xtask

Shared: repository automation.

- `cargo xtask abi` regenerates the versioned NVIDIA bindings, manifests and version metadata
  of `nvrm-sys` from the vendored headers; `cargo xtask abi --check` verifies them and runs in
  `tools/check.sh`.
- Entry point: `src/main.rs`; the generator in `src/abi/`.
- Needs libclang and the headers that `tools/build.sh vendor-abi <version>` fetches.
- Docs: [driver versions](../../docs/virtio/abi-versions.md).
