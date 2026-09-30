<!-- SPDX-License-Identifier: GPL-2.0-only -->
# nvrm_nodes

Leandro VirtIO: a helper module beside `virtio_nvrm`.

- Supplies `/proc/driver/nvidia` in the guest from files copied from the host. It loads with
  `create_nodes=0` beside `virtio_nvrm`, which owns `/dev/nvidia*`.
- Optional placeholder nodes, and a VA2GPA ioctl for the provisioning tool's pinning
  self-test; production pinning is `virtio_nvrm`'s.
- Files: `nvrm_nodes_main.c` (the module), `nvrm-nodes-tool.c` (provisioning and self-test),
  `nvrm_nodes_uapi.h`.
- Build in the guest with `make -C guest-module/nvrm_nodes`.
- Docs: [reference](reference.md) (use, VA2GPA contract),
  [architecture](../../docs/virtio/architecture.md).
