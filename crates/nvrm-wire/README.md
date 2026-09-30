<!-- SPDX-License-Identifier: MIT -->
# nvrm-wire

Leandro VirtIO: the wire schema between guest module and host backend.

- Requests and replies between `virtio_nvrm.ko` and `vhost-user-nvrm`, and the
  descriptor-table format the host sends to the guest.
- Little-endian; `Req` is 160 bytes, `Rsp` 32 bytes, a payload at most 16384 bytes; `no_std`;
  protocol version 6.
- Files: `src/lib.rs` (messages, kinds, version), `src/tables.rs` (table records).
- The guest's C header is generated from it:
  `cargo run --bin nvrm-genhdr -- guest-module/virtio_nvrm/nvrm_wire.h`, checked with `--check`.
- Docs: [reference](reference.md) (compatibility rules),
  [architecture](../../docs/virtio/architecture.md#abi-and-protocol).
