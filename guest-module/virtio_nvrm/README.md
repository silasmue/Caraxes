<!-- SPDX-License-Identifier: GPL-2.0-only -->
# virtio_nvrm

Leandro VirtIO: the guest module.

- Takes the place of `nvidia.ko` in the guest: owns `/dev/nvidia*` and forwards RM open,
  ioctl and mmap calls over a VirtIO device to `vhost-user-nvrm` on the host. NVIDIA user
  space runs unchanged.
- Reads the ioctl tables the host sends and the generated `nvrm_wire.h`; also provides the
  virtual display and event delivery.
- Files: `virtio_nvrm.c` (nodes, transport, forwarding), `nvrm_tables.c`, `nvrm_edid.c`,
  `nvrm_vram.c`; userspace tests in `test/`.
- Build in the guest with `make -C guest-module/virtio_nvrm`; check on the host with
  `tools/ci/check-c.sh all` or `nix build .#guest-modules`.
- Docs: [reference](reference.md) (source map, parameters, invariants),
  [architecture](../../docs/virtio/architecture.md).
