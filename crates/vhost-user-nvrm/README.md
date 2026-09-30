<!-- SPDX-License-Identifier: MIT -->
# vhost-user-nvrm

Leandro VirtIO: the host backend.

- One process per VM. It serves `virtio_nvrm.ko` in the guest over vhost-user: virtio queues
  and a shared-memory window.
- Validates and translates each guest RM or UVM request (tables from `nvrm-abi`, messages
  from `nvrm-wire`) and replays it on the host driver; guest-process sessions own the
  mirrored FDs, mappings and resources.
- Entry points: the binary (`src/main.rs`), `src/nvrm.rs` (queues, window, sessions, events),
  `src/session.rs` (validation and execution); `fuzz/` holds the message-handler fuzz target.
- Run and test: `vhost-user-nvrm --nvrm /path/to/socket`; `cargo test -p vhost-user-nvrm --lib`
  uses fake RM calls.
- Docs: [reference](reference.md) (source map, limits),
  [architecture](../../docs/virtio/architecture.md), [security](../../docs/virtio/security.md).
