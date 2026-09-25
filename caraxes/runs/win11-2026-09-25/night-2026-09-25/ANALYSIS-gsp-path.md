<!-- SPDX-License-Identifier: MIT -->
# Windows 617.14 on the synthetic TU106: the falcon path after the PROM fix (run P)

- **Evidence.** Run P (`P-ifr-fullwindow-trace.log.gz`, 1604953 lines; build `vfio-user-nvgpu-v615-extcfg-ifr-d4d37c2f`, which is the hwcfg build a5db4f5d plus the IFR-aware `--vbios`; the full 1 MiB PROM window). L and M (the heads and summaries) agree with P up to the point where their heads end.
- **N does not compare.** N (the T4 costume with the data-centre driver) ran for 1203 s and never touched BAR0. The trace holds only config and ROM reads, so the driver did not bind. Q (the same costume in chain 6) did bind and shows the same falcon programming as P.
- **Times.** Seconds since the device started (trace column 1 / 1e6).
- **Main finding.** The Windows driver on this TU106 takes the CPU-RM path, not GSP-RM:
  - it runs FWSEC on the GSP falcon;
  - it runs the ACR *unload* ucode on the PMU falcon;
  - it never touches the GSP queues, Booter, SEC2 loads or RISC-V.
  - It gives up on the PMU falcon, which the device does not model.

## 1. Phases of the P trace

| t (s) | Phase |
|---|---|
| 49.07-49.21 | Bind and identification. The seven AUX writes `0xd96c + n*0x50 = 0xc80` come at 49.206. |
| 49.23-55.30 | The whole PROM window: 144471 `NV_PROM` reads. |
| 55.305-55.321 | **GSP falcon: FWSEC** (below). |
| 55.321-55.347 | `NV_PMC_ENABLE` dance: the PMU enable bit 13 is set (0xd0fdd1c1 → 0xd1fdffd1). PTIMER, PTOP table, PDISP channel reset (0x640000-0x6408xx), GR clock-gating latches, FB/LTC/MMU probes. |
| 55.346-115.321 | **PMU falcon: stall request, then a 60 s timeout** (below). |
| 115.321-115.327 | **PMU: ACR unload loaded and started, then a falcon state dump right away.** |
| 115.33-117.13 | BAR1/BAR2 block setup, then 21841 reads of `NV_VF_CPU_INTR_LEAF(0)` (0xb81000), all 0, alternating with `BOOT_0`. |
| 117.13-117.17 | **SEC2 state dump.** It is never loaded. The dump reads HWCFG1 0x84012c 535×, IMCTL/IMSTAT 0x840140/0x840144 for 256 blocks, ICD 0x840200/0x84020c, and the core registers. |
| 117.175-117.204 | **Teardown:** disable SEC2/NVDEC/NVENC in `PMC_ENABLE`; GSP reset; PMU stall request (1 read) and the ACR unload again with its dump; FB flush; interrupts off; GSP reset ×3 and FWSEC again (117.2014); `PMC_ENABLE` back to 0xd0fdd1c1; `BAR2_BLOCK` = 0. |
| 117.2047 | Config command = 0x1. Windows then re-enumerates the device (PCI-ID config reads), which is Code 43. |

### GSP falcon (0x110000): FWSEC from the VBIOS, the same as Linux

- **Prepare, 55.3059.** Reads HWCFG1 `0x11012c` ×2 (model 0), `0x110abc` (captured 0x20). Then EMEM: `EMEMC 0x110ac0 := 0x2000000` (AINCR, offset 0), and 36 reads of `EMEMD 0x110ac4` (all 0).
- **Reset, 55.3137-55.3170.** Three `ENGINE 0x1103c0` 1/0 cycles, each followed by 10 reads; `MAILBOX0 := 0`, `RM 0x110084 := 0x166000a1`, `IRQMCLR := 0xffffffff`.
- **Loader setup, 55.3170-55.3190:**
  - `FBIF_TRANSCFG(4) 0x110610 := 6` (non-coherent sysmem, physical; Linux writes 5, coherent).
  - `DMACTL := 0`.
  - `FBIF_CTL 0x110624 := 0x80`.
  - `DMEMC := 0x1000000`. Then 21 DMEMD words: the `RM_FLCN_BL_DMEM_DESC`.
  - `IMEMC 0x110180 := 0x100fe00`, tag 0xfd then 0xfe, 128 IMEMD words.
  - `MAILBOX0 := 0xfe`, `BOOTVEC := 0xfd00`.
- **The DMEM descriptor:**
  - `reserved[0..3]` = 0x19, 0, 0xffff950f901ca000. This is stack garbage: Linux has 0xc0c07af0 there.
  - `ctxDma` = 4.
  - `codeDmaBase` = 0x1_7e7d5000.
  - Non-secure code 0/0x400, secure code 0x400/0x9400, entry 0.
  - `dataDmaBase` = 0x1_7fc45000, `dataSize` = 0x420.
- **IMEM stream.** Byte-identical (128/128 words) to linux-firmware `nvidia/tu102/acr/bl.bin` (start tag 0xfd). It is the generic HS loader, not a VBIOS part. No 16-byte run of it occurs in `prom-window-2026-09-25.bin`.
- **The ucode, from the VBIOS.** The loader DMAs code 0x400 + 0x9400 and data 0x420. That is exactly the FWSEC descriptor v2 in the second 0xE0 image:
  - `prom-window-2026-09-25.bin` 0x66528, app 0x85 (prod): IMEM 0x9800 = non-secure 0x400 + secure 0x9400 at 0x400, DMEM 0x420 at +0x9800, 0x9c20 in all;
  - image at 0x66564-0x70184.
  - App 0x45 (debug, 0x5c8cc) has the identical descriptor. The trace cannot tell prod from debug, because the bytes go by DMA.
- **Start, 55.3191.** `CPUCTL := 2`, which reads 0x10 at once. `IRQSCLR := 0`, `IRQMSET := 0`. Then the GFW_BOOT checks (0x118128, 0x118234), HWCFG1 ×2 again, and the EMEM read of 36 words again (55.3194-55.3208).
- **Windows checks no result.** It does not read `NV_PBUS_VBIOS_SCRATCH(0xE)` 0x1438 or WPR2 0x1fa824/0x1fa828, which Linux reads after FWSEC-FRTS. So the command is very likely not FRTS: it is probably FWSEC-SB, `init_cmd` 0x19, which the trace cannot show (see §3, change 3).
- **The model treats it as FRTS anyway.** It logs `fwsec: data buffer PA 0x17fc45000, frtsOffset 0x3fe00000 (fallback from the FB size), WPR2 lo 0x3fe000 hi 0x3feff0`, so WPR2 goes up.

### PMU falcon (0x10a000): stall request, then the ACR unload

- **55.3466-55.3471.** The driver reads `MAILBOX0 0x10a040`, `ENGINE 0x10a3c0` (0) and GFW_BOOT. It writes `IRQMCLR 0x10a014 := 0xffff`.
- Then `NV_PFB_NISO_FLUSH_SYSMEM_ADDR 0x100c10 := 0x17fc420`, `0x10a048 (ITFEN) := 0x100` and **`0x10a0a4 := 0x2`**.
- **55.3471-115.3211: the poll that gates.** 1411471 reads of `0x10a0a4`, all 0x2 (latched), over exactly 60.0 s.
  - 0x0a4 is the falcon `ENGCTL`. nouveau sets its bit 3 (SWITCH_CONTEXT) in `gm200_flcn_bind_inst`, `nvkm/falcon/gm200.c`.
  - Bit 1 is (from memory of NVIDIA's falcon manuals; not in the vendored headers) `SET_STALLREQ`, a write-only trigger. The driver waits for the acknowledge, never for the written bit to read back.
- **115.3212-115.3240: the ACR unload load.**
  - `FBIF_TRANSCFG 0x10ae00..0x10ae10 := 4,0,4,5,6`: exactly nouveau's `gm200_pmu_flcn_bind_inst` DMA indices.
  - `DMACTL := 0`, `FBIF_CTL 0x10ae24 := 0x80`.
  - `DMEMC := 0x11000000`, then 21 descriptor words. `DMEMC` is read back after each word and returns 0x11000000 every time: no auto-increment.
  - `HWCFG 0x10a108` reads 0.
  - `IMEMC := 0x100fe00`, tags 0x100 and 0x101, 128 words; `BOOTVEC := 0x10000`; `CPUCTL := 2`.
- **What was loaded:**
  - The IMEM stream is byte-identical to linux-firmware `nvidia/tu102/acr/unload_bl.bin` (start tag 0x100).
  - The descriptor is code 0x1_7fc43000 (non-secure 0/0x100, secure 0x100/0x2700) and data 0x1_7fc45800 (0x600). This matches the load header of `nvidia/tu102/acr/ucode_unload.bin` field for field (0x100, 0x2800, 0x600, 1 app 0x100/0x2700).
  - So this is **ACR unload**, which nouveau also runs on the PMU on TU10x (`tu102_acr_unload_fwif`, `NVKM_ACR_HSF_PMU`). It is not in the VBIOS: no ucode-table descriptor has these sizes.
- **115.3241-115.3270: failure dump.** `CPUCTL` reads 0x2 (the written STARTCPU, latched) 3×. After 0.27 ms the driver dumps state:
  - HWCFG1/HWCFG;
  - 0x5c0-0x5cc and 0x450-0x47c;
  - ICD `0x10a200 := 0x1608, 0x1708, 0x1808, 0x1b08, 0x1508, 0x1408, 0xe, 0x10e … 0x50e, 0x1c08` with `0x10a20c` reads;
  - IRQ, mailbox, DMA, BOOTVEC and ENGCTL registers, CURCTX/NXTCTX, 0x810-0x83c;
  - PTOP 0x20598-0x205b4.
  - The quick bail-out and the second stall request (117.1796, one read and no poll) show that the driver's timeout budget is already spent. After the 60 s poll every later wait fails on its first read.

## 2. Linux on the same device, and the wrong or missing answers

### Linux at the same stage (E2 trace `caraxes/runs/e2-2026-09-23/device-trace.txt`; docs/caraxes/gsp-boot-turing.md §2 C1-C6)

- **Same FWSEC stage.** GSP reset, `FBIF_CTL` bit 7, `DMACTL 0`, TRANSCFG(4) = 5, DMEM descriptor, HWCFG → IMEM block 0xfe, the same BL at 0xfe00 with tags 0xfd/0xfe, `BOOTVEC 0xfd00`, `CPUCTL 2`.
- **Linux then checks the FRTS result:**
  - reads 0x1438 and WPR2 hi/lo;
  - the model answers WPR2 from `frtsRegionOffset4K` in the FWSEC data buffer in guest RAM (`chip/turing.rs fwsec_done`).
- **Then Linux:** GSP reset into RISC-V, libos args in GSP `MAILBOX0/1`, Booter Load on SEC2 (`MAILBOX0` = 0 on success, `RISCV_STATUS` 1), then the RPC queues.
- **Linux never touches** the PMU (0x10a000), `ENGCTL`, ACR, EMEM or the ICD. The PMU block is outside the model because the GSP-RM path never needed it.
- **Windows shares only FWSEC with Linux.** Everything after it is the CPU-RM ACR flow (nouveau tu102 order: ACR unload on the PMU, AHESASC on SEC2, ASB on GSP, then the LS falcons and the PMU RTOS).

### Wrong or absent answers (P trace)

**PMU block 0x10a000: no model.** Reads answer 0 or the latch.

| Register | Model answer | A real PMU after these writes |
|---|---|---|
| `0x0a4` ENGCTL | 0x2 forever (1411474 reads) | request bit clears; the STALLACK status bit rises at once on an idle, freshly enabled falcon |
| `0x100` CPUCTL | 0x2 after `:= 2` | STARTCPU reads 0; the HS ucode then halts, giving HALTED bit 4 = 0x10 |
| `0x008` IRQSTAT | 0 | HALT bit 4 after the halt |
| `0x040` MAILBOX0 | 0 | the ACR's status (0 = success in nouveau's `gm200_flcn_fw_boot`) |
| `0x108` HWCFG | 0 | the IMEM/DMEM block counts (the driver still used IMEM 0xfe00) |
| `0x12c` HWCFG1 | 0 | core rev 6 and the HS security model (GSP on the card reads 0xca101136) |
| `0x1c0` DMEMC | 0x11000000 after every DMEMD write | offset advancing by 4, as the GSP model does |
| `0x20c` ICD_RDATA | 0 | falcon-internal registers (dump only, not a gate) |
| `0x10c`, `0x3c0`, `0x3c4`, FBIF `0xe00-0xe24` | 0 / latched | fine as they are |

- **Reset.** The PMU is reset through `NV_PMC_ENABLE` bit 13 (nouveau `gk104_mc_reset`), not through `ENGINE`. Windows raises that bit at 55.3250. The host capture has it clear (0xd0fdd1c1).

**GSP falcon 0x110000.** `falcon::MODELLED` covers 0x000-0x13f, so these hide the capture:

| Register | Model | Card |
|---|---|---|
| `0x12c` HWCFG1 (read 12×) | 0 | 0xca101136 (rev 6, security model 3, 1 IMEM and 1 DMEM port) |
| `0x108` HWCFG | 0x20100 | 0x40420100 (same size fields) |
| `0x048` ITFEN | 0 | 0x1004 |
| `0x0f4` HWCFG2 | 0x400 | 0x7f7 (Windows does not read it) |

- **EMEM `0xac4`.** 72 reads return 0. The content on the card is unknown, and capturing it needs an `EMEMC` write (not read-only).
- **FWSEC result.** WPR2 goes up (0x3fe000/0x3feff0) on a command that is not FRTS. Windows never reads WPR2, so it does no harm yet. It will once the ACR unload runs, because unload tears down WPR regions.

**SEC2 0x840000.**
- `0x12c` HWCFG1 = 0 (535 reads) and `0x108` = 0x20100 vs the card (not captured).
- IMCTL/IMSTAT `0x140/0x144` and ICD `0x20c` read 0. These come from the dump only.

**WPR/ACR registers.** `0x1fa814/818/81c/820` (WPR allow read/write, WPR1 lo/hi) read 0 (unmodelled, not captured). 0 is plausible for "no WPR1" before AHESASC.

**Interrupts.** 21841 reads of `0xb81000` return 0 after the failed PMU start. It is not known whether a PMU halt interrupt would be expected there, because the budget was already spent.

**Display 0x640000-0x6408xx.**
- 89 offsets are written and every write is dropped ("display: read-only page").
- Only `0x64000c` is read back differently (0x100 written, 0 read, 7×). It may be self-clearing on hardware too.
- There is no poll on these registers, so they do not gate.

**Captured/latched values that should react to writes.** PMU `CPUCTL` (STARTCPU latched), `ENGCTL` (latched request), `DMEMC` (no auto-increment), `0x64000c`. Nothing else in the path polls.

## 3. Model changes, ranked

| # | Change (file, function) | Answer | Success in the next trace |
|---|---|---|---|
| 1 | **Model the PMU falcon**: `chips/tu106/boot.toml` gets `pmu = { base = 0x10a000, … }`; `chip/turing.rs` `TuringBootData`/`TuringBoot` gain `pmu: Falcon`, routed in `handles`/`read32`/`write32` like SEC2; PMU FBIF at +0xe00 | `DMEMC`/`IMEMC` auto-increment, `DMACTL` 0, `HWCFG` 0x20100 (or the captured value, see 5), `CPUCTL` start → halted 0x10 plus `IRQSTAT` bit 4, `MAILBOX0` 0 after the halt. Reset on `NV_PMC_ENABLE` bit 13 rising (bar0 PMC write hook) | PMU `0x10a1c0` reads advance 0x11000004…; `CPUCTL` reads 0x10 after `:= 2`; no ICD/0x450 dump after the start |
| 2 | **`ENGCTL` 0x0a4 in `falcon.rs` `Falcon::write32/read32`** (all three falcons) | bits 0-3 write-only (read 0); a write with bit 1 (SET_STALLREQ) sets STALLACK: 0x200 on the first try, per the recalled layout (STALLREQ_MODE 8, STALLACK_T 9, STALLACK_N 10). If the poll does not end, try 0x0 and then 0x600 in two short runs | `W 0x0010a0a4 0x2` followed by at most a few reads, and the ACR unload load within ms (not at +60 s). This gates now |
| 3 | **FWSEC command decode in `chip/turing.rs fwsec_done`** | read the DMEM image in guest RAM (data PA + the descriptor's `interface_offset` 0xe0 → appif table → DMEMMAPPER v3 `init_cmd`, nova-core `firmware/fwsec.rs:73-138`). Only 0x15 (FRTS) raises WPR2; 0x19 (SB) leaves it; log the command | E line `fwsec: command 0x19 (SB)` and no WPR2 note for Windows; Linux still gets `0x15` and its WPR2 (selftest and vm4-615 unchanged) |
| 4 | **ACR unload effects** (the PMU halt handler in `chip/turing.rs`, keyed on `BOOTVEC 0x10000` / the PMU's first HS start) | WPR1 `0x1fa81c/820`, WPR2 `0x1fa824/828` read 0 afterwards; `MAILBOX0` 0 | the driver goes past 115.33 without the SEC2 dump and next loads **SEC2** (AHESASC: an IMEM/DMEM load at 0x840180/0x8401c0 and `CPUCTL 0x840100 := 2`) |
| 5 | **Real falcon ID registers** (`falcon.rs` `MODELLED`; values from `boot.toml` or the capture) | GSP `HWCFG1` 0xca101136, `ITFEN` 0x1004; PMU/SEC2 `HWCFG`/`HWCFG1`/`ENGCTL` from a read-only capture of 0x10a000-0x10a1ff and 0x840000-0x8401ff on the host card (hostreg-read; no EMEM, because that needs a port write) | GSP `0x11012c` reads 0xca101136 and Linux boots unchanged (test `unmodelled_falcon_registers_read_the_capture`) |
| 6 | Display `0x64000c` self-clear and dropped writes (display model) | read 0 after the trigger is fine; keep | no change expected, low priority |

- **What gates now:** change 2 (with 1, which gives it a block to live in).
  - The 60 s `ENGCTL` poll spends the driver's timeout budget.
  - Every later wait (PMU halt, the second stall request, SEC2) then fails on its first read, and the driver tears down.
- **After 1+2+4, the next gates are the CPU-RM ACR chain the model has no notion of:**
  - AHESASC on SEC2 (WPR1 set by ACR, LS falcon boot of PMU/FECS/GPCCS);
  - ASB on GSP;
  - then the **PMU RTOS** command/message queues (nouveau: cmdq 0x10a4a0/0x10a4b0, msgq 0x10a4c8/0x10a4cc) and FECS.
  - That is a CPU-RM device model, a much larger scope than the GSP-RM model that serves Linux.
- **Worth one test before investing:** whether 617.14 honours the RM regkey `EnableGpuFirmware` (GSP-RM mode) in the Windows registry. If so, Windows would take the GSP path this device already emulates.
