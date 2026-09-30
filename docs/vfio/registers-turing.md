<!-- SPDX-License-Identifier: MIT -->
# BAR0 registers the emulated TU106 must answer, by phase

Offsets are BAR0 byte offsets from the swref headers under
`src/common/inc/swref/published/` (Turing `tu102/*` unless noted). "Phase" refers to
[init-map.md](init-map.md). Values are what the 610.57.04 CPU-side driver checks;
anything not listed can start as read-0 / write-ignored and be added when the access
log shows it. Bit fields use `hi:lo`.

## Identity (phase 1–3)

| Offset | Register | Expected | Notes |
|---|---|---|---|
| 0x000 | `NV_PMC_BOOT_0` | 0x166000A1 (TU106 A1) | arch 28:24 = 0x16, impl 23:20 = 6, major 7:4 = 0xA, minor 3:0 = 1 (`nv_ref.h:108-137`); must be stable (GPU-lost detection) |
| 0x004 | `NV_PMC_BOOT_1` | 0x00000000 | VGPU 17:16 = REAL (`nv_ref.h:139-149`) |
| 0xA00 | `NV_PMC_BOOT_42` | chip id 29:20 = 0x166; major rev 19:16, minor 15:12 | HAL selection (`nv_ref.h:150-175`); verify the real value in R2 |
| 0x2174C | `NV_FUSE_OPT_SECURE_GSP_DEBUG_DIS` | bit 0 = 1 (production) | selects PROD booter/FWSEC images (`kernel_gsp_tu102.c:93-104`) |
| 0x21C04 | `NV_FUSE_STATUS_OPT_DISPLAY` | DATA = disabled at first | with display enabled the CPU also reads `NV_PDISP_VGA_WORKSPACE_BASE` 0x625F04 |

## Read before the GSP boot, at attach and scalability init (phase 3)

| Offset | Register | Expected |
|---|---|---|
| 0x88000 / 0x88008 / 0x8802C | `NV_PCFG` mirror of config dwords 0x00 / 0x08 / 0x2C (vendor:device, class/rev, subsystem) | same bytes as the config space (`gpuGetIdInfo_GM107`) |
| 0x88068 bit 16, 0x880C8 bit 31 | mirror of MSI enable / MSI-X enable | must track the live config-space enable bits |
| 0x8807C, 0x88080, 0x88084 | mirror of PCIe device cap / device control-status (RMW) / link cap | as on the card |
| 0x8800C, 0x88030 | mirror of latency timer (RMW) / ROM BAR (written 0) | accept |
| 0x88704 | `NV_XVE_CYA_2` MSI re-arm | accept writes |
| 0x140 | `NV_PMC_INTR_EN(0)` | reads 0 (DISABLED); 0x160 / 0x180 EN_SET / EN_CLEAR accept writes |
| 0x100C10 | `NV_PFB_NISO_FLUSH_SYSMEM_ADDR` | read-modify-write at KernelMemorySystem construct; keep the written value |

## VBIOS through the PROM window (phase 4.1)

| Offset | Register | Expected |
|---|---|---|
| 0x300000 + i | `NV_PROM_DATA(i)` (`dev_ext_devices.h:27`) | bytes of the full VBIOS image (up to 1 MiB): PCI ROM images, NVIDIA extension image with the FWSEC ucode, IFR/"NVGI" header handling at 0x300000/4/8 (`dev_bus.h:30-37`, `kernel_gsp_vbios_tu102.c:202-273`) |

## GSP boot status (phase 4.4–4.8)

| Offset | Register | Expected |
|---|---|---|
| 0x118128 | `NV_PGC6_AON_SECURE_SCRATCH_GROUP_05_PRIV_LEVEL_MASK` | bit 0 = 1 |
| 0x118234 | `NV_PGC6_AON_SECURE_SCRATCH_GROUP_05(0)` (GFW boot progress) | bits 7:0 = 0xFF |
| 0x9430 | `NV_PTIMER_TIME_PRIV_LEVEL_MASK` | bit 4 = 1 (failure ignored) |
| 0x9400 / 0x9410 | `NV_PTIMER_TIME_0/1` | accept writes; also the GPU time source for timeouts (see kernel-rm-hw-surface.md) |
| 0x409660 | `NV_PGRAPH_PRI_FECS_FEATURE_READOUT` | bit 16 = 0 (no ECC on a GeForce) |
| 0x100CE0 | `NV_PFB_PRI_MMU_LOCAL_MEMORY_RANGE` (`pascal/gp102/dev_fb.h:26`) | fbSize = MAG[9:4] << (SCALE[3:0] + 20); 8 GiB = MAG 8, SCALE 10 → 0x0000008A (bit 30 ECC_MODE = 0). Read several times |
| 0x1FA824 / 0x1FA828 | `NV_PFB_PRI_MMU_WPR2_ADDR_LO/HI` | HI VAL 31:4 = 0 before FRTS; after the GSP falcon "halts" from the FWSEC command: HI ≠ 0, LO 31:4 = frtsOffset >> 12 |
| 0x1438 | `NV_PBUS_VBIOS_SCRATCH(0xE)` | bits 31:16 = 0 after FRTS |

## GSP falcon block, base 0x110000 (phase 4.9–4.17)

Falcon-relative offsets (`dev_falcon_v4.h`); the same layout applies to SEC2 at 0x840000.

| Offset | Register | Behaviour to emulate |
|---|---|---|
| +0x004 | IRQSCLR | write clears bits in IRQSTAT |
| +0x008 | IRQSTAT | bit 6 SWGEN0 = "message in status queue" (set by the device when it posts, cleared by the CPU); bit 4 HALT |
| +0x018 / +0x01C | IRQMASK / IRQDEST | read as written |
| +0x040 / +0x044 | MAILBOX0 / MAILBOX1 | latch writes (GSP: libos-args PA lo/hi; SEC2: WPR-meta PA lo/hi); GSP MAILBOX0 bit 31 = "processor suspended" alternative to RISC-V active; SEC2 MAILBOX0 must read 0 after Booter "ran" |
| +0x080 | OS | accept write (appVersion, 0) |
| +0x084 | RM | accept write (= BOOT_0 after each reset) |
| +0x094 | DEBUGINFO | 0 (0xDEAD in bits 15:0 means "crash report", makes the boot fatal) |
| +0x0F4 | HWCFG2 | bit 10 RISCV = 1 (GSP only) |
| +0x100 | CPUCTL | write bit 1 STARTCPU → set bit 4 HALTED (after the "program" ran); bit 6 ALIAS_EN = 0 |
| +0x104 | BOOTVEC | latch |
| +0x108 | HWCFG | IMEM_SIZE 8:0 in 256-byte blocks (real value needed from R3; must be ≥ 0x86 so that the 512-byte bootloader at the top and 0x8500 bytes of booter fit) |
| +0x10C | DMACTL | scrub bits 2:1 read 0 after reset; write accepted |
| +0x130 | CPUCTL_ALIAS | only used if ALIAS_EN |
| +0x180 + 16i | IMEMC(i) | latch offset 7:2/blk 15:8, AINCW bit 24, SECURE bit 28 |
| +0x184 + 16i | IMEMD(i) | write data into the falcon's IMEM buffer at the current offset; auto-increment |
| +0x188 + 16i | IMEMT(i) | tag write (once per 256-byte block) |
| +0x1C0 + 8i / +0x1C4 + 8i | DMEMC(i) / DMEMD(i) | same for DMEM (the FWSEC BL descriptor at DMEM 0 contains the FRTS command; the booter DMEM contains the patched signature) |
| +0x600 + 4i | FBIF TRANSCFG(i) (`dev_fbif_v4.h`) | latch |
| +0x624 | FBIF CTL | latch (bit 7 ALLOW_PHYS_NO_CTX) |
| 0x1103C0 / 0x8403C0 | `NV_PGSP_FALCON_ENGINE` / `NV_PSEC_FALCON_ENGINE` | bit 0 RESET read-modify-write; a reset clears CPUCTL/HALTED and the scrub bits |
| 0x110C00 + 8i | `NV_PGSP_QUEUE_HEAD(i)` | doorbell: the CPU writes 0 after each command-queue submission |
| 0x110804 / 0x110808 | `NV_PGSP_MAILBOX(0/1)` | heartbeat counters (diagnostics only) |
| 0x111240 | `NV_PRISCV_RISCV_CORE_SWITCH_RISCV_STATUS` | bit 0 ACTIVE = 1 after the second reset |
| 0x1112B4 / 0x1112B8 | `NV_PRISCV_RISCV_IRQMASK / IRQDEST` | used by the interrupt service path (RISC-V mode) |

## Interrupt tree window (phase 5)

`NV_VIRTUAL_FUNCTION_FULL_PHYS_OFFSET` = 0xB80000 is the PF's copy of the VF register
block (`dev_vm.h:26-28`). The kernel RM reads/writes `NV_VIRTUAL_FUNCTION_PRIV_CPU_INTR_TOP(i)`
at 0xB80000 + 0x1600 + 4i and `..._LEAF(i)` / `LEAF_EN_SET` / `LEAF_EN_CLEAR` in the
0xB81000 range, the doorbell at 0xB82200, the timer at 0xB82300, the MMU fault
buffer registers at 0xB83000–0xB830B0, and BAR1/BAR2 block registers at 0xB80F40/0xB80F48
(`dev_vm.h`). Phase 5 also uses `NV_PBUS_BAR0_WINDOW` 0x1700 with the PRAMIN window
0x700000–0x7FFFFF, the TLB-invalidate registers 0xB830A0/A4/B0 (TRIGGER bit 31 must
self-clear), the interrupt tree 0xB81000–0xB81640 (LEAF write-1-to-clear, LEAF_EN_SET
readable, LEAF_TRIGGER 0xB81640 written with vector 129 by the interrupt self-test),
the doorbell 0xBB0090, and the timer 0xBB0080/84 (TIME_0 low 5 bits zero). The
ordered list is in [kernel-rm-hw-surface.md](kernel-rm-hw-surface.md).

## Observed in E0 (2026-09-23)

Every BAR0 offset the stock driver touched before failing at the VBIOS
(`caraxes/runs/e0-2026-09-23/device-trace.txt`), in order of first access:

| Offset | Name | Access | Note |
|---|---|---|---|
| 0x000004 | `NV_PMC_BOOT_1` | R | first register read; three rounds together with BOOT_0/BOOT_42 |
| 0x000000 | `NV_PMC_BOOT_0` | R | 0x166000A1 accepted as TU106 A1 |
| 0x000A00 | `NV_PMC_BOOT_42` | R | 0x166A1000 accepted |
| 0x08807C | `NV_PCFG` + PCIe device capabilities | R | config mirror |
| 0x088BD8, 0x088BE0, 0x088BF4, 0x088BFC | `NV_PCFG` extended space | R | zeros accepted (Resizable BAR neighbourhood; `resizable BAR disabled by regkey`) |
| 0x088084 | `NV_PCFG` + PCIe link capabilities | R | |
| 0x100C10 | `NV_PFB_NISO_FLUSH_SYSMEM_ADDR` | R, W | write 0x228370 (sysmem flush page, PA >> 8) |
| 0x001478 | `NV_PBUS_SW_SCRATCH(30)` | R | reset-required / drain-recommended flags (`kern_gpu_gp100.c:351`, `kern_gpu_ga100.c:357`); 0 |
| 0x000140 | `NV_PMC_INTR_EN(0)` | R | |
| 0x088080 | `NV_PCFG` + PCIe device control | R, W, R, W | 0x10291F then 0x10211F (Enable No Snoop cleared) |
| 0x021C04 | `NV_FUSE_STATUS_OPT_DISPLAY` | R | 0 |
| 0x300000 … | `NV_PROM_DATA` | R (32-bit) | image chain walk; see experiments.md E0 result |

## Observed in E1 (2026-09-23)

New offsets after the PROM walk succeeded (`caraxes/runs/e1-2026-09-23/device-trace.txt`):

| Offset | Name | Access | Note |
|---|---|---|---|
| 0x300000–0x3201FC | `NV_PROM_DATA` | R (32-bit), 32930 reads | the whole `biosSize` is copied, dword by dword, in address order |
| 0x02174C | `NV_FUSE_OPT_SECURE_GSP_DEBUG_DIS` | R, three times | value 0 answered; answer 1 (production) from E2 on (Q25) |
| 0x118128 | `NV_PGC6_AON_SECURE_SCRATCH_GROUP_05_0` (GFW boot progress) | R, once | read before the CPUCTL poll; "progress 0x0" in the error message |
| 0x110100 | GSP `NV_PFALCON_FALCON_CPUCTL` | R, 72754 polls in 3.0 s | must show HALTED (bit 4) for the GFW wait to pass |

## Observed in E2 (2026-09-23)

New offsets with the boot model attached (`caraxes/runs/e2-2026-09-23/device-trace-flr-probe.txt`),
after the PROM walk, in order of first access:

| Offset | Name | Access | Device answer |
|---|---|---|---|
| 0x110100 | GSP `CPUCTL` | R (poll) | HALTED before the first reset; after `STARTCPU` the model halts at once |
| 0x118128 / 0x118234 | PLM / GFW_BOOT progress | R | 1 / 0xFF |
| 0x009430 | `NV_PTIMER_TIME_PRIV_LEVEL_MASK` | R | 0xFF (level 0 may write; 0 makes `timer_gv100.c:80` assert) |
| 0x009410, 0x009400 | `PTIMER_TIME_1/0` | W | latched |
| 0x409660 | FECS mailbox | R | 0 |
| 0x1FA828, 0x1FA824 | WPR2 HI/LO | R | 0 before FWSEC; lo = frtsOffset >> 12 << 4, hi = end of FRTS after |
| 0x100CE0 | `LOCAL_MEMORY_RANGE` | R | 0x4A for 4 GiB |
| 0x021C04 | display fuse | R | 1 (no display) |
| 0x1100F4, 0x1103C0, 0x11010C, 0x110084, 0x110624, 0x110610, 0x1101C0/C4, 0x110108, 0x110180/84/88, 0x110104, 0x110100, 0x110040/44, 0x110080, 0x110094, 0x110C00, 0x111240 | GSP falcon block | R/W | `falcon.rs`; `HWCFG` reports 0x100 IMEM and 0x100 DMEM blocks (our choice, Q3) |
| 0x001438 | `NV_PBUS_VBIOS_SCRATCH(0xE)` | R | 0 (FRTS error code) |
| 0x8403C0, 0x84010C, 0x840084, 0x840040/44, 0x840624, 0x840180/84/88, 0x8401C0/C4, 0x840104, 0x840100 | SEC2 falcon block | R/W | `falcon.rs`; MAILBOX0 = 0 after the run |
| 0x900488 + i·0x4000 (+4) | `NV_PFB_FBPA(i)_ECC_DED_COUNT` | R | 0; read in the failure path |
| 0x1404F8 + i·0x2000 (+0x200) | `NV_PLTCG_LTC(i)_LTS(j)_L2_CACHE_ECC_UNCORRECTED_ERR_COUNT` | R | 0; read in the failure path |
| PCIe device control bit 15 (config 0x80, mirror 0x88080) | Initiate FLR | W | resets the device model |

