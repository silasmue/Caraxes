<!-- SPDX-License-Identifier: MIT -->
# nvrm-client

Shared: an RM client on the host.

- Owns RM clients, object trees and handle allocation above `nvrm-abi`, and frees objects in
  dependency order.
- Leandro VFIO's device reaches the host RM through it (`RmClient`, CPU mappings); Leandro
  VirtIO's backend keeps its forwarded clients itself.
- Diagnostic binaries in `src/bin/`: `classlist`, `smipids`, `fbclients`, `mmapping`,
  `e1-extmap`, `vgpuprofile`, `vsockconnect`.
- Test without a GPU: `cargo test -p nvrm-client --lib`.
- Docs: [reference](reference.md), [RM resource model](../../docs/wiki/rm-resource-model.md).
