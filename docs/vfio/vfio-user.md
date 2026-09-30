<!-- SPDX-License-Identifier: MIT -->
# vfio-user: what matters for Leandro VFIO

Sources read: `vendor/qemu-vfio-user/vfio-user.rst` (the protocol spec, QEMU
`docs/interop/vfio-user.rst` at v11.1.1), `vendor/qemu-vfio-user/{proxy,device,container,pci}.c`
(QEMU's client), `vendor/qemu-vfio-user/hw-vfio-pci.c` (QEMU's generic vfio-pci
model that the vfio-user device reuses), `vendor/vfio/vfio-user/src/lib.rs`
(rust-vmm server), `vendor/libvfio-user` (C reference server).

## Protocol in one paragraph

A UNIX socket per device. The client (VMM) sends commands; the server (device)
replies. Commands used by QEMU 11.1 (`protocol.h:28-43`): VERSION, DMA_MAP,
DMA_UNMAP, DEVICE_GET_INFO, DEVICE_GET_REGION_INFO, DEVICE_GET_IRQ_INFO,
DEVICE_SET_IRQS, REGION_READ, REGION_WRITE, DEVICE_RESET, REGION_WRITE_MULTI,
DEVICE_FEATURE, plus the server-initiated DMA_READ/DMA_WRITE which QEMU answers
(`pci.c:87-212`). Regions use the VFIO PCI indices: BAR0–5 = 0–5, ROM = 6, CONFIG = 7,
VGA = 8. IRQ indices: INTX = 0, MSI = 1, MSIX = 2, ERR = 3, REQ = 4.

## What QEMU requires of the server

- `DEVICE_GET_INFO` must report at least `VFIO_PCI_CONFIG_REGION_INDEX + 1` = 8 regions
  and at least `VFIO_PCI_MSIX_IRQ_INDEX + 1` = 3 IRQ indices (`hw-vfio-pci.c:3157-3166`).
- QEMU reads the whole PCI config space from region 7 at setup and keeps emulated
  copies of BARs, MSI/MSI-X capabilities and the ROM BAR. Config writes go through
  `vfio_pci_write_config` (`hw-vfio-pci.c:1450`): BAR and ROM-BAR writes are forwarded
  to the server, so the server must implement the usual "write all-ones, read back the
  size mask" behaviour for BARs and ROM. MSI enable/disable writes become
  `DEVICE_SET_IRQS` with eventfds; the server only has to keep the capability bytes
  consistent and trigger the eventfd.
- ROM: QEMU sizes the ROM BAR through config space (`vfio_pci_size_rom`,
  `hw-vfio-pci.c:1213-1240`) and then reads the image from region 6 with REGION_READ
  (`vfio_pci_load_rom`, :1044-1130); it validates the 0x55AA signature and PCIR pointer.
  Alternatively `romfile=` bypasses the server. Note: this ROM is the *PCI* ROM the guest
  BIOS/kernel sees; the NVIDIA driver additionally reads the VBIOS through BAR0's PROM
  window (offset 0x300000), which is an ordinary REGION_READ on BAR0.
- Guest memory: QEMU sends `DMA_MAP` with a file descriptor **only if the memory
  region has one** (`container.c:90-131`, `memory_region_get_fd`). With plain anonymous
  guest RAM the map carries no fd and the server must use `DMA_READ/DMA_WRITE`
  messages, which the rust-vmm server does not implement. Therefore run QEMU with
  `-object memory-backend-memfd,id=m,size=…,share=on -machine memory-backend=m` (or
  `-numa node,memdev=m`). With no vIOMMU in the guest, DMA addresses are guest physical
  addresses, which is what the GSP init structures contain.
- Region mmap: `DEVICE_GET_REGION_INFO` may return an fd and sparse-mmap areas; QEMU
  then maps those directly into the guest (fast path, no trap). For BAR0 we want
  **no** mmap at first (every access trapped and logged). BAR1/BAR3 can later be backed
  by a memfd or by a host RM mapping.
- Posted writes: QEMU sends region writes without waiting for the reply unless
  `x-no-posted-writes=on`. For strict ordering in logs, set it on during experiments.
- Version handshake: QEMU offers capabilities (`max_msg_fds`, `max_data_xfer_size`,
  `pgsizes`, `max_dma_maps`, migration, `write_multiple`); the rust server replies
  with an empty capability set, and QEMU falls back to its defaults (`proxy.c:920-930`).
- Timeouts: `x-msg-timeout` (default 5000 ms) bounds every reply; a slow emulated
  register access that blocks longer kills the device.

## The rust-vmm server (`vendor/vfio/vfio-user/src/lib.rs`)

- `Server::new(path, resettable, irqs: Vec<IrqInfo>, regions: Vec<ServerRegion>)`
  (:889-910) and `Server::run(&self, backend: &mut dyn ServerBackend)` (:1394).
- `ServerBackend` (:828-865): `region_read`, `region_write`, `dma_map(flags, offset,
  address, size, fd)`, `dma_unmap`, `reset`, `set_irqs(index, flags, start, count, fds)`.
- `ServerRegion { region_info: vfio_region_info, sparse_areas, mmap_fd }` (:872-876):
  region 7 (config space) is just another region handled by `region_read/write`.
- Not implemented (returns `UnsupportedCommand`): GET_REGION_IO_FDS, DMA_READ,
  DMA_WRITE, DIRTY_PAGES (:1005-1010). REGION_WRITE_MULTI and DEVICE_FEATURE are not in
  its `Command` enum at all, so QEMU must not use them: QEMU only sends WRITE_MULTI
  when the server advertised `write_multiple`, and DEVICE_FEATURE for migration.
- The server is single-threaded per connection (`run` loops on one stream). Long
  emulation work (polling the host RM) must not run inside a region callback; queue it.
- Cloud Hypervisor v53 depends on the same crate (`vfio_user = 0.1.3`) for its
  `--user-device` client, so the server also works there.

## Cloud Hypervisor as the client (preferred)

Cloud Hypervisor v53 ships a vfio-user client (`vendor/cloud-hypervisor/pci/src/vfio_user.rs`,
`vfio_user` crate 0.1.3 from the same rust-vmm repository) behind `--user-device socket=<path>`
(`vmm/src/vm_config.rs:830-834`; hotplug via `ch-remote add-user-device`). Its
documentation (`docs/vfio-user.md`) calls it experimental and lists two limits: no
virtio-mem and no vIOMMU with such devices. What it does:

- Reads device info, region info (sizes, flags, sparse mmap areas with fds) and IRQ
  info at creation; resets the device if the server says it is resettable
  (`vfio_user.rs:77-118`, client :511-593).
- BAR sizes come from the region info; the BAR flag bits (memory/IO, 64-bit,
  prefetchable) are read from the config-space dword through region 7
  (`vfio.rs:755-830`). The ROM BAR is built from region 6 the same way (:798-830, :933).
  Trapped BAR accesses become `REGION_READ/WRITE` (`vfio_user.rs:267-296`); regions
  with an fd are mmapped into the guest (`map_mmio_regions`, :121-211).
- Interrupts: INTx through the legacy interrupt group when IRQ index 0 exists
  (`vfio.rs:1215-1240, 1296-1310`), MSI and MSI-X through `VfioCommon` with eventfds
  (`enable_irq`, `vfio_user.rs:310-345`). Config-space capability parsing is the same
  code as for kernel VFIO (`vfio.rs:985-1200`).
- DMA: every guest memory zone is sent as `DMA_MAP` with the memory file's fd and
  offset (`VfioUserDmaMapping::map`, `vfio_user.rs:538-576`; `device_manager.rs:4460-4480`),
  so guest RAM must be file-backed: `--memory size=…,shared=on`. Without a vIOMMU the
  DMA address is the guest physical address.
- Not supported by the client: server-initiated `DMA_READ/WRITE` (not needed with
  shared memory), `REGION_WRITE_MULTI`, migration.

So the synthetic device does not need QEMU. QEMU remains the tool for the reference
capture R3 (`vfio-pci` with `x-no-mmap=on` and `-trace vfio_region_*` on the real card),
because Cloud Hypervisor has no MMIO tracing for passthrough devices.

## Device model for experiment E0 (see experiments.md)

| Region | Size | Backing |
|---|---|---|
| 0 (BAR0, regs) | 16 MiB, 32-bit, non-prefetch | trapped; log every access; answer BOOT_0/1/42 and the PROM window from `rom.bin`; 0 elsewhere |
| 1 (BAR1, FB) | 256 MiB, 64-bit, prefetch | trapped, log, read 0 |
| 3 (BAR3, RAMIN/BAR2) | 32 MiB, 64-bit, prefetch | trapped, log, read 0 |
| 5 (I/O) | 128 B | trapped, log |
| 6 (ROM) | 512 KiB | `rom.bin` (PCI chain) |
| 7 (config) | 4 KiB | copy of `config-space.txt` with the BARs/ROM/MSI made writable per PCI rules; PCIe and PM caps as on the card; the extended space (0x100-0xFFF) reads 0 here unless `CARAXES_EXT_CONFIG=1`, and the BAR0 mirror `NV_PCFG` serves it (next section) |

IRQs: INTX count 1 (`Interrupts: pin B`, but present as pin A for simplicity), MSI
count 1 (64-bit, no masking), MSI-X count 0.

## Extended configuration space (2026-09-25)

- The reason appeared: the Windows driver (617.14) reads the extended space through
  `NV_PCFG` (BAR0 0x88000 + offset) before any GSP contact and ends in Code 43 on zeros:
  0x700 (then writes 0x20000 there), 0xBD8, 0xBE0, 0xBF4, 0xBFC. The last four are the
  hidden SR-IOV block's `NV_XVE_SRIOV_CAP_HDR3/5/10/12` (turing/tu102/dev_nv_xve.h:46-53),
  read by `kbifCacheVFInfo_TU102` (kernel_bif_tu102.c:235-282); the Linux RM reads them too.
- RTX 2070 capture, chain from 0x100: VC 0x100, LTR 0x250, L1 PM Substates 0x258, Power
  Budgeting 0x128, AER v2 0x420, VSEC 0x600, Secondary PCIe 0x900, Resizable BAR 0xBB0
  (next 0). Not linked (hidden): SR-IOV 0xBCC, then ARI 0xC14; 16 GT/s 0xC1C, Lane
  Margining 0xD00, Data Link Feature 0xE00; DVSEC 0xAC0. No Device Serial Number.
- Served now (`config.rs` `strip_ext`, `ext_masks`): all capture bytes, in the mirror always.
  - Cleared: SR-IOV, ARI and DSN when linked (GB206: 0x248, 0x250, 0x2F4, the chain skips
    them); ARI and DSN anywhere else (TU106: the hidden ARI 0xC14).
  - Kept read-only: the hidden SR-IOV block (TotalVFs 63, VF offset 4, VF BARs 64-bit, as
    the card reports; nothing enables VFs), Resizable BAR (no resizing), capability
    headers, AER header and prefix logs, `NV_XVE_CYA_2` 0x704 (MSI re-arm, unchanged).
  - RW1C: AER uncorrectable and correctable status. Everything else latches.
- Config region: already 4 KiB; it keeps reading zeros above 0xFF by default, so the
  Linux guest and Cloud Hypervisor see what they saw before. `CARAXES_EXT_CONFIG=1` shows
  the same bytes to the guest OS (Cloud Hypervisor nulls ARI/Rebar/SR-IOV ids itself,
  `pci/src/vfio.rs` `parse_extended_capabilities`; QEMU hides SR-IOV and ARI). Opt-in
  because the guest kernel then drives AER, LTR and L1SS on a model that only latches.
- Open, Windows run only: whether Code 43 goes away with the mirror alone or needs
  `CARAXES_EXT_CONFIG=1`, and which read comes next.
