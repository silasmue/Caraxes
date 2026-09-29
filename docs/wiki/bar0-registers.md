<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# BAR0 registers

BAR0 is the 16 MiB register window of a Turing GPU. A GSP-client kernel RM touches a small
part of it: the boot ids, the configuration mirror, the PROM window, the two falcons it
boots, the interrupt tree, the MMU invalidate, the flush registers, the timer, the usermode
page and the display front end. Everything else belongs to GSP-RM. In VFIO Leandro BAR0 is a
trapped region with every access logged, except the pages the device offers as memory.

## The pages a driver really touches (TU106, addresses from the published `swref` headers)

| Page or register | Offset | Who | What |
|---|---:|---|---|
| `NV_PMC_BOOT_0`, `BOOT_1`, `BOOT_42` | 0x0, 0x4, 0xA00 | HAL selection, `osHandleGpuLost` | the chip identity ([kernel-rm-hal-and-registry](kernel-rm-hal-and-registry.md)) |
| `NV_PMC_INTR_EN_SET/CLEAR(0)` | 0x160, 0x180 | every RM lock acquire and release (`intr_gp100.c`) | masked with `pmcRmOwnsIntrMask`, 0 in GSP-client mode: no effect, 183 writes per Mutter frame |
| `NV_PBUS_BAR0_WINDOW`, PRAMIN | 0x1700, 0x700000 to 0x7FFFFF | `kbusSetupDefaultBar0Window`, the BAR2 bootstrap and `kbusVerifyBar2_GM107` (`kern_bus_gm107.c`) | a 1 MiB CPU window onto VRAM at a base in 64 KiB units |
| `NV_PBUS_SW_SCRATCH(30)` | 0x1478 | the census tool | markers around each measured operation |
| `NV_PFB_PRI_MMU_LOCAL_MEMORY_RANGE` | 0x100CE0 | the WPR layout before boot | the FB size the pre-boot code assumes |
| GSP falcon block | 0x110000 | `kernel_gsp_tu102.c`, `msgq.c` | reset, IMEM/DMEM ports, `BOOTVEC`, `CPUCTL`, mailboxes, `NV_PGSP_QUEUE_*` and `MSGQ_*` at 0x110C00 to 0x110C84, `DEBUGINFO` at 0x110094 |
| PROM window | 0x300000 | `kgspExtractVbiosFromRom_HAL` | the VBIOS image chain |
| display FE interrupt status, display channels | 0x611000, 0x696000 | nvkms through the kernel RM | 28 watched words and their acks; the display channel kick |
| SEC2 falcon block | 0x840000 | Booter Load and Unload | the second falcon state machine |
| config mirror `NV_PCFG` | 0x88000 to 0x88FFF | `gpuGetIdInfo_GM107`, KernelBif | ids, MSI enable, the re-arm at 0x88704 ([pci-config-and-identity](pci-config-and-identity.md)) |
| VF CPU interrupt tree | 0xb80000, page 0xb81000 | `intr_tu102.c`, `intr_nonstall_tu102.c` | leaves, leaf enables, top, top enables, trigger ([interrupts](interrupts.md)) |
| MMU invalidate, fault buffer registers | page 0xb83000 | `kgmmuInvalidateTlb` path, the RM after each mapping change | `_PDB`, `_UPPER_PDB`, trigger; the RM polls bit 31 |
| usermode page | 0xbb0000 | libcuda and the kernel RM | `TIME_0/1` reads (11.8 per Mutter frame); the doorbell `NVC361_NOTIFY_CHANNEL_PENDING` at 0xbb0090 ([channels-and-submission](channels-and-submission.md)) |

- The kernel RM never writes `NV_PBUS_BAR1_BLOCK`, `NV_PBUS_BAR2_BLOCK` or the fault
  buffers; those are GSP-RM's.
- The interrupt self-test at boot triggers a vector through `LEAF_TRIGGER`; the RM reads
  `NV_PTIMER` for its timeouts.

## In the device

- `bar0.rs` dispatches by offset into the boot model, the falcons, the config mirror and the
  interrupt tree; an offset without a model latches what was written and reads zero, so a new
  register shows up as a trace line, never as a silent gap. `chips/tu106/regmap.toml` and
  `boot.toml` hold the per-chip map.
- Pages offered as memory instead of traps: page 0 and 0xb81000 (the interrupt page),
  0x611000 (the display page); the doorbell at 0xbb0090 and the re-arm at 0x88704 as
  ioeventfds; page 0x110000 as a read-only area is the polled-queue design.
- A trapped access costs 23 to 27 us on a quiet host, 28 to 35 us busy, of which 1 to 2 us is
  the device ([performance-shape](performance-shape.md)).

## Depth

- [caraxes/registers-turing.md](../caraxes/registers-turing.md): the consolidated map by phase.
- [caraxes/kernel-rm-hw-surface.md](../caraxes/kernel-rm-hw-surface.md): what the kernel RM
  touches per engine.
- [research/windows-and-traps.md](../research/windows-and-traps.md): what still traps per
  operation and per frame.
