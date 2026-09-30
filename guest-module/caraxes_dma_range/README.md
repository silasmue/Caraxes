<!-- SPDX-License-Identifier: GPL-2.0-only -->
# caraxes_dma_range

Leandro VFIO: a former guest module, no longer loaded (decision D13, replaced by D18).

- Installed a DMA range map on the GPU's PCI device in the guest before `nvidia.ko` bound,
  so every DMA address the guest computed for it became a host IOVA.
- Today the device registers guest RAM and the guest writes guest physical addresses; the
  module stays as a small example of kernel programming.
- Files: `caraxes_dma_range.c`, `caraxes_dma_range_pci.h`, `caraxes_dma_range_test.c`.
- Build in the guest with `make -C guest-module/caraxes_dma_range`.
- Docs: [reference](reference.md), [walkthrough](WALKTHROUGH.md),
  [sysmem and DMA](../../docs/wiki/sysmem-and-dma.md).
