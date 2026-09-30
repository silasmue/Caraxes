<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# BAR0 registers

BAR0 is the 16 MiB register window of a Turing GPU. A GSP-client kernel RM touches a small
part of it: the boot ids, the configuration mirror, the PROM window, the two falcons it
boots, the interrupt tree, the MMU invalidate, the flush registers, the timer, the usermode
page and the display front end. Everything else belongs to GSP-RM. In VFIO Leandro BAR0 is a
trapped region except the pages and registers the device offers as memory or as ioeventfds;
[traps-and-fast-paths](traps-and-fast-paths.md) has what each access costs and how often it
comes.

## The pages a driver touches (TU106, offsets from the published `swref` headers)

| Page or register | Offset | Who | What |
|---|---:|---|---|
| `NV_PMC_BOOT_0`, `BOOT_1`, `BOOT_42` | 0x0, 0x4, 0xA00 | HAL selection, `osHandleGpuLost` | the chip identity ([kernel-rm-hal-and-registry](kernel-rm-hal-and-registry.md)) |
| `NV_PMC_INTR_EN_SET/CLEAR(0)` | 0x160, 0x180 | every RM lock acquire and release (`intr_gp100.c`) | masked with `pmcRmOwnsIntrMask`, 0 in GSP-client mode: no effect |
| `NV_PBUS_BAR0_WINDOW`, PRAMIN | 0x1700, 0x700000 to 0x7FFFFF | `kbusSetupDefaultBar0Window`, the BAR2 bootstrap, `kbusVerifyBar2_GM107` (`kern_bus_gm107.c`) | a 1 MiB CPU window onto VRAM at a base in 64 KiB units |
| `NV_PBUS_SW_SCRATCH(30)` | 0x1478 | the census tool | markers around each measured operation |
| `NV_PTIMER_TIME_0/1` | 0x9400, 0x9410 | the Windows KMD, read hi-lo-hi | the GPU time; the base of the display notifier timestamps |
| `NV_UFLUSH_FB_FLUSH`, L2 flush | 0x70000, 0x70010 | nouveau's BAR2 flush; the guest's L2 flush | a write starts the flush, a read polls it |
| `NV_PFB_PRI_MMU_LOCAL_MEMORY_RANGE` | 0x100CE0 | the WPR layout before boot | the FB size the pre-boot code assumes |
| GSP falcon block | 0x110000 | `kernel_gsp_tu102.c`, `msgq.c` | reset, IMEM/DMEM ports, `BOOTVEC`, `CPUCTL`, mailboxes, `NV_PGSP_QUEUE_*` and `MSGQ_*` at 0x110C00 to 0x110C84 ([gsp-message-queue](gsp-message-queue.md)) |
| PROM window | 0x300000 | `kgspExtractVbiosFromRom_HAL` | the VBIOS image chain |
| display FE interrupt status | 0x611000 | nvkms through the kernel RM, the Windows KMD | 28 watched words and their acks |
| display capabilities | 0x640000 | the Windows KMD (`NVC573`) | the card's captured capability page |
| display channel USERD | 0x680000 to 0x6DFFFF | nvkms, the Windows KMD | `PUT` kicks a channel, `GET` shows its progress; window channels from 0x690000 |
| SEC2 falcon block | 0x840000 | Booter Load and Unload | the second falcon state machine |
| config mirror `NV_PCFG` | 0x88000 to 0x88FFF | `gpuGetIdInfo_GM107`, KernelBif | ids, MSI enable, the re-arm at 0x88704 ([pci-config-and-identity](pci-config-and-identity.md)) |
| VF CPU interrupt tree | page 0xb81000 of the VF window 0xb80000 | `intr_tu102.c`, `intr_nonstall_tu102.c` | leaves, enables, top, trigger ([interrupts](interrupts.md)) |
| L2 sysmem invalidate | 0xb80f00 | the guest, paired with the L2 flush | one write, then a poll read |
| MMU invalidate, fault buffers | page 0xb83000 | `kgmmuCommitTlbInvalidate_TU102`, UVM | `_PDB`, `_UPPER_PDB`, `TRIGGER`; the RM polls bit 31; fault buffer `GET`/`PUT` |
| usermode page | 0xbb0000 | libcuda, the GL and Vulkan drivers, the kernel RM | `TIME_0/1` at 0x80 and 0x84; the doorbell `NVC361_NOTIFY_CHANNEL_PENDING` at 0x90 ([channels-and-submission](channels-and-submission.md)) |

- The kernel RM never writes `NV_PBUS_BAR1_BLOCK`, `NV_PBUS_BAR2_BLOCK` or the fault buffer
  registers; those are GSP-RM's. UVM writes the replayable fault buffer's `GET`.
- The interrupt self-test at boot triggers a vector through `LEAF_TRIGGER`.
- The kernel RM's own timeouts use the OS timer in GSP-client mode, not PTIMER
  ([caraxes/kernel-rm-hw-surface.md](../caraxes/kernel-rm-hw-surface.md)).

## In the device

- `crates/vfio-user-nvgpu/src/bar0.rs` dispatches by offset into the boot model, the falcons,
  the config mirror and the interrupt tree; an offset without a model latches what was written
  and reads zero, so a new register shows up as a trace line, never as a silent gap.
  `chips/tu106/regmap.toml` and `boot.toml` hold the per-chip map.
- Memory: page 0 and page 0xb81000 with `--host-exec` (the interrupt page); 0x611000 and the
  window USERD pages with `CARAXES_DISPLAY_PAGE=1`. Opt-in and not GPU-validated: page 0xb83000
  (`CARAXES_INVALIDATE_PAGE=1`) and page 0xbb0000 (`CARAXES_USERMODE_PAGE=1`).
- ioeventfds: the doorbell at 0xbb0090 (guest tokens with `--boot-model`, the token pool with
  `CARAXES_DOORBELL_TOKENS=N`), the re-arm at 0x88704 (`CARAXES_MSI_REARM_IOEVENTFD=1`), the
  GSP queue head (`CARAXES_RPC_IOEVENTFD=1`).
- Answered before the device lock: `TRIGGER`, PTIMER and usermode `TIME` reads (`fastread.rs`)
  and the trapped doorbell, which is posted and rung by a completer thread.
- The device clock is one per process and survives an FLR: the RM sets PTIMER forward before
  every GSP boot (`kernel_gsp.c`), and display timestamps come from the same clock
  ([research: windows-findings](../research/windows-findings.md), Cloud Hypervisor step 5).
- A PRAMIN access to the device's private pages at the top of the slice is refused: the write
  is dropped, the read returns zeros (windows-findings, "Security fix").

## Depth

- [caraxes/registers-turing.md](../caraxes/registers-turing.md): the consolidated map by phase.
- [caraxes/kernel-rm-hw-surface.md](../caraxes/kernel-rm-hw-surface.md): what the kernel RM
  touches per engine.
- [research: trap-census-2026-09-30](../research/trap-census-2026-09-30.md): what still traps
  per workload, by register.
