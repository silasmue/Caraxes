<!-- SPDX-License-Identifier: MIT -->
# CPU-RM boot chain on Turing (no GSP-RM): what the device must answer

This is a specification for the device model. It covers the falcon chain a Turing driver runs when
it does not use GSP-RM. The Windows GeForce driver 617.14 takes this path on the synthetic TU106
(`caraxes/runs/win11-2026-09-25/night-2026-09-25/ANALYSIS-gsp-path.md`, run P).

**How to read the tags**
- Sources:
  - **N**: Linux nouveau, `drivers/gpu/drm/nouveau/…`, cited as file and function.
  - **O**: `vendor/open-gpu-kernel-modules`.
  - **E**: envytools `rnndb/falcon.xml` and `docs/hw/falcon/`.
  - **W**: the Windows run P trace.
  - **FW**: the linux-firmware `nvidia/tu10x/acr/*` headers, parsed from the copies the win11 analysis used.
- **(unsourced)**: an inference with no public source. Treat it as a hypothesis to check in a trace.
- Falcon registers are written as offsets from the falcon base. The bases are:
  - GSP 0x110000;
  - SEC2 0x840000 (N `engine/sec2/tu102.c tu102_sec2_new`: the TOP entry is wrong on Turing, so the base is fixed);
  - PMU 0x10a000;
  - FECS 0x409000;
  - GPCCS 0x41a000.

## 0. Common falcon mechanics (all stages)

### 0.1 Register semantics the chain relies on

| Off | Name | Bits and behaviour | Source |
|---|---|---|---|
| 0x004 | IRQSCLR | W1C on IRQSTAT; bit 4 HALT, bit 6 SWGEN0 | O `tu102/dev_falcon_v4.h` |
| 0x008 | IRQSTAT | bit 4 HALT (set when the core halts); bit 6 SWGEN0 (an RTOS posted a message) | O same; N `engine/sec2/gp102.c gp102_sec2_intr`, `subdev/pmu/gt215.c gt215_pmu_intr` |
| 0x010/0x014/0x018 | IRQMSET/IRQMCLR/IRQMASK | mask set/clear/read | O |
| 0x01c | IRQDEST | the host takes an IRQ when `IRQSTAT & IRQDEST & ~(IRQDEST>>16)` | N `gp102_sec2_intr` |
| 0x040/0x044 | MAILBOX0/1 | RW. An HS ucode writes its status to MAILBOX0 before it halts | N `falcon/gm200.c gm200_flcn_fw_boot` |
| 0x048 | ITFEN | bit 0 = context interface enable, set only on the instance-block path (N `gm200_flcn_fw_load`). Windows writes 0x100 on the PMU (meaning unknown) | N, W |
| 0x0a4 | ENGCTL (E: `ENG_CONTROL`) | bit 1 PAUSE_TRIGGER (W), bit 2 UNPAUSE_TRIGGER (W), bit 8 PAUSE_REQ, bit 9 PAUSE_DONE (R), bit 3 set by nouveau `gm200_flcn_bind_inst` | E `rnndb/falcon.xml` |
| 0x100 | CPUCTL | bit 1 STARTCPU (W, reads 0), bit 4 HALTED (R), bit 6 ALIAS_EN (RW). When ALIAS_EN is set, the host must start the core through CPUCTL_ALIAS | O; N `falcon/v1.c nvkm_falcon_v1_start` |
| 0x104 | BOOTVEC | IMEM address the core starts at | O |
| 0x108 | HWCFG | 8:0 IMEM size, 17:9 DMEM size, both in 256-B blocks. nouveau sizes the IMEM from it and puts the BL at `code.limit - boot_size` | O; E `CAPS`; N `gm200_flcn_fw_load` |
| 0x10c | DMACTL | bit 0 REQUIRE_CTX, bit 1 DMEM scrubbing, bit 2 IMEM scrubbing. Scrubbing must read 0 within 10 ms of a reset | O; N `gm200_flcn_reset_wait_mem_scrubbing`; nova-core `falcon/hal/tu102.rs` |
| 0x130 | CPUCTL_ALIAS | bit 1 STARTCPU | O |
| 0x180/0x184/0x188 | IMEMC/IMEMD/IMEMT (port 0) | IMEMC: 15:2 address, bit 24 AINCW, bit 28 SECURE. IMEMT sets the tag for the next 256 B | O; N `gm200_flcn_pio_imem_wr` |
| 0x1c0/0x1c4 | DMEMC/DMEMD (port 0) | DMEMC: 15:2 address, bit 24 AINCW, **bit 25 AINCR (a readback)** | O; N `gm200_flcn_pio_dmem_rd_init` |
| 0x3c0 | ENGINE | bit 0 reset (1 then 0), then wait for scrubbing | N `falcon/gp102.c gp102_flcn_reset_eng` |
| 0x408 (SEC2) / 0xc08 (PMU) | debug fuse | bit 20 set = debug board, which selects the debug signatures. It must read 0 (production) | N `gm200_flcn_fw_signature`, `.debug` in `tu102_sec2_flcn` / `gp102_pmu_flcn` |
| 0xac0/0xac4 | EMEMC/EMEMD (port 0) | as DMEMC/D. DMEM-space addresses ≥ 0x01000000 map to EMEM (SEC2 and GSP) | N `falcon/gp102.c`, `falcon/base.c nvkm_falcon_pio` (`emem_addr`) |
| FBIF +0x600 (GSP/SEC2), +0xe00 (PMU) | TRANSCFG(i) | 1:0 target (0 FB, 1 coherent sys, 2 non-coherent sys), bit 2 physical. 4 = FB phys, 5 = sys coherent phys, 6 = sys non-coherent phys, 0 = virtual | O `tu102/dev_fbif_v4.h` |
| FBIF +0x624 / +0xe24 | FBIF_CTL | bit 7 ALLOW_PHYS_NO_CTX | O |

### 0.2 HS boot protocol (every HS stage below)

What the host does, in order (N `falcon/fw.c nvkm_falcon_fw_boot`, `falcon/gm200.c gm200_flcn_fw_load` and `gm200_flcn_fw_boot`):
- reset (ENGINE, or `NV_PMC_ENABLE` for the PMU);
- physical DMA: `FBIF_CTL |= 0x80` and `DMACTL = 0`;
- or nouveau's virtual DMA: ITFEN bit 0, instance block at +0x054/+0x480, ENGCTL bit 3, then a wait for bind status 5 and then 0;
- the BL is PIO-loaded to IMEM `limit - 0x200`, tag = `start_tag`;
- the BL descriptor (`flcn_bl_dmem_desc_v2`, 84 B = 21 words) goes to DMEM 0;
- `MAILBOX0 = 0xcafebeef` (nouveau only; Windows writes nothing);
- `BOOTVEC = start_tag << 8`, `CPUCTL = 2`;
- a poll of `CPUCTL & 0x10`, for up to 2 s;
- MAILBOX0 and MAILBOX1 are read, and success means **MAILBOX0 == mbox0_ok**;
- optionally `IRQSCLR = intr_clear`.

For Turing, `mbox0_ok` = 0 and `intr_clear` = 0 for all three ACR ucodes (N `subdev/acr/tu102.c`: the `tu102_acr_{unload,ahesasc,asb}_fwif` rows). Older ACR builds differ: gp102/gp108 unload expects `mbox0_ok` 0x1d. So "success" depends on the build and should be configurable per ucode.

What the device must do for every HS start:
- CPUCTL reads 0x10: STARTCPU cleared, HALTED set;
- IRQSTAT bit 4 set;
- MAILBOX0 = the success code;
- MAILBOX1 left as it was (nouveau only prints it);
- the stage's side effects applied before the halt becomes visible.

How to tell the ucodes apart: by falcon, BOOTVEC and the BL descriptor sizes (from FW; Windows' unload matched them field for field in run P).

| Ucode | Falcon | BL start tag / BOOTVEC | non-sec code | sec code (off/size) | data size |
|---|---|---|---|---|---|
| FWSEC (VBIOS, v2 desc) | GSP | 0xfd / 0xfd00 | 0x400 | 0x400/0x9400 | 0x420 |
| ACR unload | PMU | 0x100 / 0x10000 | 0x100 | 0x100/0x2700 | 0x600 |
| AHESASC (ACR load) | SEC2 | 0xfd / 0xfd00 | 0x100 | 0x100/0x3c00 | 0x1000 |
| ASB | GSP | 0xfd / 0xfd00 | 0x100 | 0x100/0x2900 | 0x800 |

## 1. The stages

### S0. FWSEC on GSP (VBIOS ucode; SB and/or FRTS)

**Host.** As in §0.2 (see `gsp-boot-turing.md` C2). The DMEMMAPPER `init_cmd` in the data image selects the command: 0x15 = FRTS, 0x19 = SB (O `kernel_gsp_frts_tu102.c`, `FALCON_APPLICATION_INTERFACE_DMEM_MAPPER_V3_CMD_SB`).

**Result the host checks (O `kgspExecuteFwsec_TU102`):**

| Command | Register | Expected |
|---|---|---|
| FRTS 0x15 | `NV_PBUS_VBIOS_SCRATCH(0xE)` 0x1438 | bits 31:16 = 0 |
| | WPR2_HI 0x1fa828 | VAL 31:4 ≠ 0 |
| | WPR2_LO 0x1fa824 | VAL 31:4 = frtsOffset >> 12 |
| SB 0x19 | `AON_SECURE_SCRATCH_GROUP_05_PRIV_LEVEL_MASK` 0x118128 | bit 0 = 1 |
| | `…GROUP_05(0)` GFW_BOOT 0x118234 | bits 7:0 = 0xff |
| | `NV_PBUS_VBIOS_SCRATCH(0x15)` 0x1454 | bits 15:0 = 0 |

- **SB leaves WPR2 alone.** Only FRTS raises it. The branch model already decodes the command (`turing.rs fwsec_command`).
- **What Windows read (W).** 0x118128 = 1 and 0x118234 = 0xff at 55.3057, 55.3192 and 55.3276. It never read 0x1438 or 0x1454, and 0x1454 must read 0 whenever it is read.
- **Order in the CPU-RM chain:**
  - SB at bind (W 55.3059);
  - FWSEC again at teardown (W 117.2014).
  - FRTS is not seen on the CPU-RM path. The FRTS "VBIOS tables" shared sub-WPR (`LSF_SHARED_DATA_SUB_WPR_USE_CASE_ID_FRTS_VBIOS_TABLES`, O `rmlsfm.h`) suggests that CPU-RM may place it inside WPR1 later (unsourced).

### S1. PMU pause request (Windows only)

**Host (W).** At 55.3466:
- reads PMU `MAILBOX0` and `ENGINE`;
- `IRQMCLR = 0xffff`;
- `0x100c10 = 0x17fc420` (the FB NISO flush address);
- `ITFEN = 0x100`;
- **`ENGCTL = 0x2`**, then polls ENGCTL.

nouveau has no such step.

**Device.** ENGCTL reads PAUSE_DONE (bit 9, 0x200) right after a write with bit 1 set, and bits 3:0 read 0. A write with bit 2 (UNPAUSE_TRIGGER) clears bit 9. The branch default is 0x200 (`CARAXES_FALCON_STALLACK`).
- **Unknown:** whether Windows waits on bit 9 or bit 8 (PAUSE_REQ). The X0 run (tonight) tests 0x200; 0x300 is the fallback.

### S2. ACR unload on PMU

- **Host (W 115.3212, N `tu102_acr_unload_fwif` → `NVKM_ACR_HSF_PMU`):**
  - PMU reset through `NV_PMC_ENABLE` bit 13 (nouveau `gm200_pmu_flcn` uses `reset_pmc`);
  - `TRANSCFG(0..4) = 4,0,4,5,6`;
  - `DMACTL = 0`, `FBIF_CTL = 0x80`;
  - DMEM descriptor with `ctx_dma` = 4 (→ TRANSCFG(4) = 6, sysmem non-coherent physical), code 0x1_7fc43000, data 0x1_7fc45800;
  - `IMEMC = 0x100fe00`, tags 0x100/0x101;
  - `BOOTVEC = 0x10000`, `CPUCTL = 2`.
- **What the ucode does:** it tears down the WPR regions an earlier ACR load set up (WPR1, sub-WPRs). nouveau runs it on `fini`, and before a reload when no RTOS owns bootstrapping (N `subdev/acr/base.c nvkm_acr_unload`, `nvkm_acr_reload`). Windows runs it first, to clear any leftover state.
- **Device after the halt:**
  - CPUCTL 0x10;
  - IRQSTAT |= 0x10;
  - MAILBOX0 = 0;
  - WPR1 back to the reset values (§2).
  - WPR2: CPU-RM does not use it here. The branch model clears it. That is harmless for SB boots, but it would break a FRTS-then-unload order, so keep it switchable (unsourced).
- **Next trace:** whether Windows reads DMEM back after the halt. That would show as a `DMEMC` write with bit 25 (0x02xxxxxx), then DMEMD reads.

### S3. AHESASC (ACR load) on SEC2

**Host (N `tu102_acr_init` → `nvkm_acr_hsfw_boot("AHESASC")`, `tu102_acr_ahesasc_fwif` → `NVKM_ACR_HSF_SEC2`).**
- Before the boot, `gp102_acr_load_setup` patches the **`flcn_acr_desc_v1` at the start of the data image**. The BL DMAs the data image to DMEM 0, so these are both data-image and DMEM offsets:
  - `wpr_region_id` (0x210) = 1;
  - `regions.no_regions` (0x21c) = 2;
  - `region_props[0]` (0x220) = `{start_addr = wpr_start>>8, end_addr = wpr_end>>8, region_id 1, read_mask 0xf, write_mask 0xc, client_mask 0x2, shadow_mem_start_addr = shadow>>8}`.
- The WPR blob is built in the **shadow** copy (the lower half of a 2× allocation), and the WPR is the upper half (N `gp102_acr_wpr_alloc`). On a dGPU both are in FB (VRAM, `NVKM_MEM_TARGET_INST`).

**What the ucode does (from what nouveau checks next; the ucode itself is closed):**
- sets WPR1 = [wpr_start, wpr_end), which read back through the MMU registers;
- copies the shadow blob into WPR;
- validates each LSB signature and updates each WPR header's `status` (1 COPY → 4 VALIDATION_DONE, or 6 BOOTSTRAP_READY);
- boots the non-lazy LS falcons whose bootstrap owner it is. On Turing nouveau marks every falcon lazy and names GSPLITE as owner (N `tu102_acr_wpr_build`), so AHESASC boots none.

**Device after the halt:**
- CPUCTL 0x10, IRQSTAT |= 0x10, MAILBOX0 = 0;
- WPR1 from the descriptor (§2);
- the WPR copy in FB = the shadow with `status` = 4 per header (unsourced; nouveau never reads it back).

**Host check (N `gm200_acr_wpr_check` via `tu102_acr.wpr_check`):**
- `0x100cd4 = 2`, then read → `start = (v & 0xffffff00) << 8`;
- `0x100cd4 = 3`, then read → `limit = ((v & 0xffffff00) << 8) + 0x20000`;
- both must equal `wpr_start` / `wpr_end`, or "WPR not configured as expected" (N `subdev/acr/base.c nvkm_acr_load`).
- Windows may read 0x1fa81c/0x1fa820 instead: it read both, and 0x1fa814/0x1fa818, at 55.3456 and 117.1783 (W).

### S4. ASB on GSP

- **Host.** `nvkm_acr_hsfw_boot("ASB")` on the GSP falcon, the same protocol as §0.2, with no descriptor patching (`gp108_acr_hsfw_0` has no `.setup`).
- **What the ucode does.** It is the bootstrap owner (`bootstrap_owner` = 1 GSPLITE in every WPR header, N `tu102_acr_wpr_build`). It loads the **SEC2 LS RTOS** from WPR into SEC2:
  - BL code at `bl_imem_off`;
  - BL data (`flcn_bl_dmem_desc_v2` from `bl_data_off`, `ctx_dma` = SEC2 UCODE index, `argc` 1, `argv` = 0x01000000, i.e. EMEM) into DMEM 0 (N `engine/sec2/gp102.c gp102_sec2_acr_bld_write_1`);
  - BOOTVEC;
  - LS mode (unsourced).
- **Device after the halt:**
  - GSP CPUCTL 0x10, MAILBOX0 = 0;
  - SEC2 CPUCTL = 0x50 (HALTED, ALIAS_EN). ALIAS_EN is sourced by the start path; HALTED before the start is unsourced.
- **Windows.** Unknown whether it uses ASB, or boots SEC2 LS / PMU LS itself from AHESASC. `rmlsfm.h` states that "the PMU supports the ACR task on GM20X_thru_VOLTA profiles only", so on Turing SEC2 is the RTOS that bootstraps (O `rmlsfm.h`, the `LSF_FALCON_ID_END_PMU` and `…_ACR_ON_SEC2` comments).

### S5. SEC2 RTOS start, INIT message, LS bootstrap of FECS/GPCCS

**Host start (N `engine/sec2/base.c nvkm_sec2_init`):**
- `IRQMCLR = 0xffffffff`;
- `nvkm_falcon_start`: because CPUCTL bit 6 is set, it writes **`CPUCTL_ALIAS` 0x840130 = 2**.

**Device.** CPUCTL reads 0x40 (running, ALIAS_EN). The RTOS posts its INIT message:
- writes the message to EMEM;
- sets the **msgq tail 0x840c84** = its DMEM-space address (≥ 0x01000000), and the **msgq head 0x840c80** = tail + 32;
- sets IRQSTAT bit 6, with IRQMASK/IRQDEST routing bit 6 to the host.
- Queue registers on Turing: cmdq head/tail 0x840c00/0x840c04, stride 8; msgq head/tail 0x840c80/0x840c84 (N `tu102_sec2_flcn`).

**Host reads (N `falcon/msgq.c nvkm_falcon_msgq_recv_initmsg`, `gp102_sec2_initmsg`):**
- reads the tail; pops 32 B from DMEM[tail] through the EMEM port (`EMEMC = BIT(25) | (addr - 0x01000000)`);
- checks `hdr.size == 32`, `unit_id == 0x01` (`NV_SEC2_UNIT_V2_INIT`), `msg_type == 0`;
- writes msgq tail = tail + 32.

INIT message (`nv_sec2_init_msg`, N `include/nvfw/sec2.h`; 32 B):

| Off | Size | Field | Value to give |
|---|---|---|---|
| 0x00 | 1 | unit_id | 0x01 |
| 0x01 | 1 | size | 0x20 |
| 0x02 | 1 | ctrl_flags | 0 |
| 0x03 | 1 | seq_id | 0 |
| 0x04 | 1 | msg_type | 0 (INIT) |
| 0x05 | 1 | num_queues | 2 |
| 0x06 | 2 | os_debug_entry_point | any |
| 0x08 | 8 | queue_info[0] `{u32 offset, u16 size, u8 index, u8 id}` | cmdq: 0x01000000 + X, size, index 0, id 0 |
| 0x10 | 8 | queue_info[1] | msgq: 0x01000000 + Y, size, index 0, id 1 |
| 0x18 | 4 | sw_managed_area_offset | any |
| 0x1c | 2 | sw_managed_area_size | any |

**Commands (N `falcon/cmdq.c`, `gp102_sec2_acr_bootstrap_falcon`):**
- The host writes the command at the cmdq head position through EMEM, then writes **cmdq head 0x840c00** = the new position.
- A `unit_id 0x00` (REWIND) entry means: wrap to the queue offset.
- Command, 16 B:

  | Off | Field | Value |
  |---|---|---|
  | 0x00 | unit_id | 0x07 (ACR, v2) |
  | 0x01 | size | 16 |
  | 0x02 | ctrl_flags | (see below) |
  | 0x03 | seq_id | host-chosen |
  | 0x04 | cmd_type | 0 (BOOTSTRAP_FALCON) |
  | 0x08 | flags | 0 = RESET_YES |
  | 0x0c | falcon_id | 2 FECS, 3 GPCCS, 7 SEC2 |

  - ctrl_flags: bit 0 STATUS, bit 1 INTR (N); RM names bit 2 EVENT and bit 5 UNIT_ACK (O `flcnifcmn.h`).

**Device answer:**
- Advance the cmdq tail 0x840c04 to the head.
- Put a message in the msgq, 16 B:

  | Off | Field | Value |
  |---|---|---|
  | 0x00 | unit_id | 0x07 |
  | 0x01 | size | 16 |
  | 0x03 | seq_id | the command's |
  | 0x04 | msg_type | 0 |
  | 0x08 | error_code | 0 |
  | 0x0c | falcon_id | the command's |

- Advance the msgq head and raise IRQSTAT bit 6.
- Load the named falcon in LS mode:
  - it is reset;
  - its CPUCTL reads 0x50;
  - its DMEM 0 holds `flcn_bl_dmem_desc_v1` (N `engine/gr/gp108.c`, `gm200_gr_acr_bld_write`).
- SEC2 unload at fini (N `nvkm_sec2_fini`): a command with `unit_id` 0x05, header only. The device answers with a message, then halts (CPUCTL bit 4) within 2 s.

### S6. PMU RTOS (Windows; nouveau has no Turing PMU firmware)

- **nouveau.** It loads no PMU LS image on Turing (N `subdev/pmu/gp102.c`: `gp102_pmu_fwif` = nofw only).
- **Windows.** It very likely puts PMU (falcon id 0) into WPR1 and starts it after the ACR boots it (unsourced).
- **Queue registers (N `gp102_pmu_flcn` / `gm200_pmu_flcn`):**
  - **cmdq(i) head 0x10a4a0 + 4i, tail 0x10a4b0 + 4i**;
  - **msgq head 0x10a4c8, tail 0x10a4cc**;
  - the queues live in PMU DMEM (no EMEM).
- **Message IRQ.** IRQSTAT bit 6 (N `gt215_pmu_intr`).
- **INIT message layout: unsourced for the dGPU PMU.** The only public one is the Tegra PMU's `nv_pmu_init_msg` (N `include/nvfw/pmu.h`, 42 B):
  - hdr: `unit_id` 0x07 (INIT), `msg_type` 0;
  - `os_debug` u16 at 0x06;
  - `queue_info[5]` `{u16 size, u16 offset, u8 index, u8 pad}` at 0x08: [0] HPQ, [1] LPQ, [4] msgq;
  - `sw_managed_area` at 0x26.

  Use it only as a starting guess. Take the real size from the first read Windows makes (`hdr.size`, and how many DMEMD words follow).
- **ACR commands on the PMU** (unit 0x0a; only up to Volta, per `rmlsfm.h`) are not expected on Turing.

### S7. FECS/GPCCS start and context-switch init (N `engine/gr/gf100.c gf100_gr_init_ctxctl_ext`)

**Host after the bootstrap:**
- `0x409800 = 0`, `0x41a10c = 0` (GPCCS DMACTL), `0x40910c = 0` (FECS DMACTL);
- `CPUCTL_ALIAS` 0x41a130 = 2, then 0x409130 = 2;
- poll **`0x409800 & 1`**, for up to 2 s.

**Device.** CPUCTL of both reads 0x40. `0x409800` (FECS CTXSW_MAILBOX(0)) reads 1 once FECS is up.

**Methods.** The host writes `0x409800 = 0` (or masks it), `0x409500 = data`, `0x409504 = method`. The device answers in 0x409800:

| Method | Meaning | 0x409800 answer |
|---|---|---|
| 0x10 | main context image size | size (≠ 0) |
| 0x16 | ZCULL image size | size (≠ 0) |
| 0x25 | PM image size | size (≠ 0) |
| 0x21 | watchdog timeout | none (no wait) |
| 0x03 | bind pointer (data = inst) | bit 4 = ok, bit 5 = error |
| 0x09 | WFI golden save | bit 0 = ok, bit 1 = error |
| 0x30 | reglist size | size |
| 0x31 / 0x32 | reglist bind / VA (`0x409810` = arg) | == 1 |
| 0x38 / 0x39 | start / stop ctxsw | answered in **0x409804** instead (the host sets 0x409804 = 0xffffffff first): 1 = ok, 2 = error |

- The sizes should be the TU106 values the GSP static info reports on the host (the same numbers the GSP-RM path hands the guest).
- FECS reset: `0x409614 = 0x70`, then `|= 0x700` (N `gf100_gr_fecs_reset`).

## 2. Registers to answer in a new way (per event)

| Register | Bits | Reset / power-on | After event → value |
|---|---|---|---|
| any falcon CPUCTL +0x100 | 1 STARTCPU (W), 4 HALTED, 5 SLEEPING (E), 6 ALIAS_EN | 0x10 on a card after reset. STARTCPU never reads back. Host capture (card under GSP-RM, `caraxes/reference/rtx2070/`): GSP 0x10, PMU 0x40 (LS RTOS running), SEC2 0x20 (idle RTOS, sleeping) | HS start → 0x10 · ACR/ASB loads an LS falcon → 0x50 · host `CPUCTL_ALIAS = 2` → 0x40 (running) · RTOS unload → 0x50. An idle RTOS may read 0x20 or 0x60 |
| IRQSTAT +0x008 | 4 HALT, 6 SWGEN0 | 0 | halt → \|= 0x10 · RTOS message → \|= 0x40 · IRQSCLR write → &= ~v |
| MAILBOX0 +0x040 | 31:0 | 0 | HS halt → per-ucode ok code (TU: 0 for unload/AHESASC/ASB; FWSEC: unchanged) |
| ENGCTL +0x0a4 | 1/2 triggers, 8 REQ, 9 DONE | 0 | W bit 1 → 0x200 · W bit 2 → 0 · triggers read 0 |
| DMACTL +0x10c | 0 REQ_CTX, 1-2 scrubbing | 0 | reset → 0 (scrub done at once) · write → bit 0 only |
| DMEMC/IMEMC/EMEMC | 15:2 addr, 24 AINCW, 25 AINCR | 0 | each D access with AINC → addr += 4 (DMEMC readback shows it) |
| PMU HWCFG +0x108 / HWCFG1 +0x12c | sizes | host capture 0x400e0100 / 0xea104136 | fixed |
| SEC2/PMU debug +0x408/+0xc08 | bit 20 debug | 0 (production) | fixed |
| `NV_PFB_PRI_MMU_WPR1_ADDR_LO` 0x1fa81c | VAL 31:4 = addr >> 12 | **0xfffffff0** (VAL_INIT 0x0fffffff, O `dev_fb.h`) | AHESASC → `wpr_start >> 8` · unload → 0xfffffff0 |
| `…WPR1_ADDR_HI` 0x1fa820 | VAL 31:4 | 0 (VAL_INIT 0) | AHESASC → `(wpr_end - 0x1000) >> 8` (inclusive last 4K page, as the model's WPR2; unsourced for WPR1) · unload → 0 |
| `…WPR2_ADDR_LO/HI` 0x1fa824/0x1fa828 | VAL 31:4 | 0xfffffff0 / 0 by VAL_INIT; the model returns 0 / 0, and RM tests only HI == 0 | FWSEC-FRTS → frts window · SB → unchanged |
| 0x1fa814 / 0x1fa818 | "WPR allow read/write" (name from the analysis, not in O) | 0 | AHESASC → region 1 read/write masks, format unknown (unsourced); keep 0 until a trace polls it |
| 0x100cd4 (indexed) | W: index; R: 31:8 = addr >> 16 | 0 | idx 2 → `(wpr_start >> 8) & ~0xff`; idx 3 → `((wpr_end - 0x20000) >> 8) & ~0xff` (N `gm200_acr_wpr_check`; name not in O) |
| SEC2 0x840c00/04, 0x840c80/84 | queue head/tail | 0 | SEC2 start → msgq tail = Y, head = Y + 0x20; cmd head write → tail = head, message queued |
| PMU 0x10a4a0.., 0x10a4c8/cc | queue head/tail | 0 | PMU RTOS start → the INIT message (layout TBD) |
| FECS 0x409800 | status/value | 0 | FECS start → 1; method → see S7 |
| FECS 0x409804 | ctxsw status | 0 | methods 0x38/0x39 → 1 |
| `NV_PBUS_VBIOS_SCRATCH(0x15)` 0x1454 | 15:0 SB error | 0 | FWSEC-SB → 0 |

## 3. Memory structures

### 3.1 BL descriptor at DMEM 0 (`flcn_bl_dmem_desc_v2`, packed, 84 B; N `include/nvfw/flcn.h`)

| Word | Offset | Field |
|---|---|---|
| 0-3 | 0x00 | reserved |
| 4-7 | 0x10 | signature |
| 8 | 0x20 | `ctx_dma` |
| 9-10 | 0x24 | `code_dma_base` (u64) |
| 11 | 0x2c | non_sec off |
| 12 | 0x30 | non_sec size |
| 13 | 0x34 | sec off |
| 14 | 0x38 | sec size |
| 15 | 0x3c | entry |
| 16-17 | 0x40 | `data_dma_base` (u64) |
| 18 | 0x48 | `data_size` |
| 19 | 0x4c | argc |
| 20 | 0x50 | argv |

- Windows' unload descriptor matches it (W): ctx 4, code 0x1_7fc43000, 0/0x100, 0x100/0x2700, data 0x1_7fc45800, size 0x600.
- Its argc/argv are stack garbage (0xffff920d / 0x2085d70a), so do not validate them.

### 3.2 ACR descriptor at DMEM 0 of AHESASC (= start of its data image; `flcn_acr_desc_v1`; N `include/nvfw/acr.h`; O `RM_FLCN_ACR_DESC` in `rmlsfm.h`, after the 0x200 `ACR_RESERVED_DMEM`)

| Off | Field | nouveau value |
|---|---|---|
| 0x000 | `reserved_dmem[0x200]` | — |
| 0x200 | `signatures[4]` | — |
| 0x210 | `wpr_region_id` | 1 (`LSF_WPR_EXPECTED_REGION_ID`) |
| 0x214 | `wpr_offset` | 0 |
| 0x218 | `mmu_memory_range` | — |
| 0x21c | `no_regions` | 2 |
| 0x220 | `region_props[0]`: start (addr >> 8) / end (addr >> 8) / region_id / read_mask / write_mask / client_mask / shadow (addr >> 8) | wpr / 1 / 0xf / 0xc / 0x2 / shadow |
| 0x23c | `region_props[1]` | 0 |
| 0x258 | `ucode_blob_size` | 0 |
| 0x260 | `ucode_blob_base` (u64) | 0 |
| 0x268 | `vpr_desc {enabled, start, end, hdcp}` | 0 |

- The model reads `region_props[0]` from guest memory at `data_dma_base + 0x220` when AHESASC halts.
- If Windows fills `ucode_blob_base/size` instead of a shadow address, the blob is at that address (check in the trace).

### 3.3 WPR blob (FB; layout by N `gp102_acr_wpr_layout`, `tu102_acr_wpr_build`)

**WPR header area:**
- `wpr_header_v1[]` at 0x0, 24 B each (0x18);
- terminator `falcon_id = 0xffffffff`;
- area sized for 11 entries and aligned to 0x200. This matches O `LSF_FALCON_ID_END_ACR_ON_SEC2` = 21 → 21 × 24 → 0x200.

| Off in entry | Field | nouveau (TU) |
|---|---|---|
| 0x00 | `falcon_id` (0 PMU, 1 GSPLITE, 2 FECS, 3 GPCCS, 4 NVDEC, 5 NVENC0, 6 NVENC1, 7 SEC2; O `LSF_FALCON_ID_*`) | |
| 0x04 | `lsb_offset` | |
| 0x08 | `bootstrap_owner` | 1 (GSPLITE) |
| 0x0c | `lazy_bootstrap` | 1 |
| 0x10 | `bin_version` | sig version |
| 0x14 | `status` | 1 COPY before ACR; success = 4 VALIDATION_DONE / 6 BOOTSTRAP_READY; failures 2/3/7 (O `LSF_IMAGE_STATUS_*`) |

- **Shared sub-WPR headers at 0x200** (`LSF_SHARED_SUB_WPR_HEADER {useCaseId, startAddr, size4K}`, O). nouveau writes only the terminator 0xffffffff.
- **LSB header per falcon at `lsb_offset`** (256-aligned; `lsb_header_v1`, 0xF0 B):
  - `lsf_signature_v1` 0x00-0xBF (`falcon_id` at 0x48, `version` at 0x50);
  - `ucode_off` 0xC0, `ucode_size` 0xC4, `data_size` 0xC8;
  - `bl_code_size` 0xCC, `bl_imem_off` 0xD0, `bl_data_off` 0xD4, `bl_data_size` 0xD8;
  - `app_code_off` 0xDC, `app_code_size` 0xE0, `app_data_off` 0xE4, `app_data_size` 0xE8;
  - `flags` 0xEC: bit 0 LOAD_CODE_AT_0, bit 2 DMACTL_REQ_CTX, bit 3 FORCE_PRIV_LOAD.
  - The RM `LSF_LSB_HEADER` (O) adds `monitorCodeOffset`, `monitorDataOffset` and `manifestOffset` at 0xF0-0xFB. Take the size Windows uses from the `lsb_offset` spacing.
- **Then the ucode image** (4K-aligned) and the **BL data** (256-aligned):
  - `flcn_bl_dmem_desc_v2` for SEC2;
  - `flcn_bl_dmem_desc_v1` for FECS/GPCCS;
  - `loader_config` (52 B) for a PMU.
  - The ACR copies this BL data into the LS falcon's DMEM 0.

## 4. Windows against nouveau, and what to look for in the next traces

**Differences seen already (W):**
- **FWSEC.** Windows runs SB (0x19) at bind, where nouveau/GSP-RM run FRTS. Windows reads only the GFW_BOOT/PLM registers.
- **PMU pause.** An ENGCTL PAUSE_TRIGGER before the unload; nouveau has none.
- **Physical DMA.** No instance block. TRANSCFG is written 4,0,4,5,6 (nouveau's values, but from `gm200_pmu_flcn_bind_inst`), then FBIF_CTL 0x80, and `ctx_dma` 4 → PHYS_SYS_NCOH.
- **Unload first.** Windows runs the unload before any load. nouveau runs it only at fini or reload.
- **Mailbox.** No MAILBOX0 pre-write, so the success code cannot be inferred from a changed value.
- **Unexplained reads before the unload load:** 0x1221fc, PMU 0x3c4, 0x268, and 0x10a7b0 (in the dump).
- **Registry-test boot (README, 22:36).** About 25000 SEC2 port writes after the PMU poll, roughly 100 KB.
  - That is too much for AHESASC through the BL (about 150 writes), or for a full PIO load (about 4900 words).
  - It may be a direct (non-BL) load, such as Booter, which would mean the GSP-RM path.
  - The discriminator is the SEC2 BOOTVEC and the first IMEMT tag: 0xfd = BL/ACR; 0 with no BL = DIRECT/Booter. Then check whether the GSP RISC-V registers or the RPC queues follow.

**Checklist for tonight's traces:**
1. ENGCTL: the number of reads after `W 0x10a0a4 0x2` (a few = acknowledged), and whether a bit-2 write follows (unpause).
2. The E line `fwsec: command 0x19 (SB)` or `0x15`, and whether 0x1454 or 0x1438 is read.
3. PMU unload:
   - the reads after `CPUCTL = 2` (MAILBOX0/1, IRQSTAT, DMEMC with bit 25 = a readback);
   - then 0x1fa81c/820/824/828 or 0x100cd4.
4. SEC2 load:
   - BOOTVEC, tags, and the 21 descriptor words, giving the code/data sizes against the §0.2 table;
   - `data_dma_base`, so the model can read `flcn_acr_desc_v1` there (region_props, shadow or blob base);
   - the host reads after the halt.
5. GSP second HS load (ASB) and its sizes. Or, instead: SEC2/PMU `CPUCTL_ALIAS` 0x130 writes, which mean an LS falcon start.
6. The WPR blob in FB: the `falcon_id` list and `bootstrap_owner` (0 PMU, 1 GSPLITE or 7 SEC2), which tell who boots whom.
7. The RTOS INIT: which queue registers Windows reads first (SEC2 0x840c80/84 or PMU 0x10a4c8/cc), which port and address it reads (EMEM 0xac0 or DMEM 0x1c0), and the byte count (= `hdr.size`).
8. FECS: `0x409800` polls, the 0x409504 method numbers, and 0x409130/0x41a130 writes.

## 5. Not sourced (open)

- **ENGCTL.** Bit semantics come only from envytools (older falcons). Which bit Windows polls is untested.
- **What the HS ucodes do inside.** AHESASC, ASB and unload are closed. The effects above are inferred from what nouveau checks afterwards: MAILBOX0, `0x100cd4`, the SEC2 RTOS start. The WPR-header status update is not checked by any public driver.
- **ACR error codes** in MAILBOX0/1 (non-zero values), and 0x1fa814/0x1fa818 (names and format).
- **The inclusive or exclusive HI convention for WPR1.** It is assumed equal to WPR2.
- **The low byte of 0x100cd4 reads**, and its register name.
- **The dGPU PMU RTOS INIT message** and its unit ids, and whether Windows starts the PMU through ACR or itself.
- **LS-mode registers the ACR sets** on a bootstrapped falcon (SCTL 0x240, PLMs) and whether Windows reads them. The HALTED bit before an LS start is also unsourced.
- **The Turing host interrupt tree** for SEC2/PMU messages (the leaf and bit under 0xb81000). Windows polled `0xb81000` 21841× after the failed PMU start; the bit it expects is not known.
- **FECS image sizes for TU106.** Take them from the host's GSP static info, not from a public header.
