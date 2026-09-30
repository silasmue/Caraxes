<!-- SPDX-License-Identifier: MIT -->
<!-- Source survey generated 2026-09-23 by reading vendor/open-gpu-kernel-modules at tag 610.57.04.
     Every claim cites file:line. Treat as notes to be verified by experiment, not as specification. -->
# What the stock nvidia.ko 610.57.04 touches directly on a TU106 GSP client, from PCI probe to first device open

All paths are relative to `/home/silas/git/Caraxes/vendor/open-gpu-kernel-modules/`. Line numbers are from this tree.

## How I worked out which code runs

- **HAL binding.** I resolved each HAL from the generated binding tables in `src/nvidia/generated/g_*_nvoc.c`, reading the `/* ChipHal: … */` and `/* RmVariantHal: … */` comments.
- **Variant.** TU106 runs as `RM_RUNTIME_VARIANT_PF_KERNEL_ONLY` whenever firmware was requested (`gpu_mgr.c:1288-1300`).
- **Always requested.** `rm_set_rm_firmware_requested` always sets `request_firmware=TRUE` and `allow_fallback=FALSE`, except for a vGPU VF (`osapi.c:5106-5120`).
- **Many kernel-variant HALs are empty stubs**, which removes whole register groups from the picture:
  - `kbusBindBar2` → `_5baef9` (asserts, returns NOT_SUPPORTED) (`g_kern_bus_nvoc.c:1672-1688`, `g_kern_bus_nvoc.h:2978`)
  - `kbusBar1InstBlkBind` → `_92bfc3` (`g_kern_bus_nvoc.c:1988`, `.h:3112`)
  - `kbusBar1InstBlkVasUpdate` → `_ac1694` (`g_kern_bus_nvoc.c:1888-1895`)
  - `kbusCommitBar2PDEs` → `_22ba1e` (`g_kern_bus_nvoc.c:1827`)
  - `kbusStateInitLockedPhysical` → `_ac1694` (`g_kern_bus_nvoc.h:692`)
  - `kgmmuFaultBufferInit`/`Load` → `_ac1694` (`g_kern_gmmu_nvoc.c:1338,1372`)
  - `kmemsysAssertSysmemFlushBufferValid` → `_ac1694` (`g_kern_mem_sys_nvoc.h:501`)

**Consequence:** the kernel RM never writes `NV_PBUS_BAR1_BLOCK`, `NV_PBUS_BAR2_BLOCK` or `BIND_STATUS`, and never programs the MMU fault buffers. GSP-RM owns all of those. What the kernel does touch directly is listed below.

**Register offset bases:**
- `GPU_VREG_*` = BAR0 + `0xB80000` + offset. `sriovState.virtualRegPhysOffset = DRF_BASE(NV_VIRTUAL_FUNCTION_FULL_PHYS_OFFSET)` for the PF (`kern_gpu_tu102.c:94-101`, `dev_vm.h:28`, `g_gpu_access_nvoc.h:256-259`).
- `GPU_BUS_CFG_RD32/WR32` on TU106 go through the **BAR0 mirror of config space at `0x88000`** (`NV_PCFG`). They do not use real config cycles (`g_gpu_nvoc.c:1261` → `gpuReadBusConfigReg_GM107` → `kern_gpu_gm107.c:172-235`, `tu102/dev_nv_xve.h:26`).
- `osPciRead*/Write*` are real config cycles.

---

## A. Kernel-open layer (probe, start device, MSI, DMA)

| Step | What it does | Needs from device | On mismatch |
|---|---|---|---|
| PCI ID match | vendor `0x10DE`, class exactly `0x030000` or `0x030200` (`class_mask ~0`) (`nv-pci-table.c` `nv_pci_table`) | class/prog-if exact | no bind |
| `rm_wait_for_bar_firewall` (`nv-pci.c:1957`) | Does nothing for devId outside `0x2900-0x29FF`/`0x3180-0x327F` (`gpu_mgr.c:4976-5002`) | – | – |
| `rm_is_supported_pci_device` (`nv-pci.c:1971`, `osapi.c:3840-3882`) | class `0x03`, subclass `0x00`/`0x02`, vendor `0x10DE`, device ≥ `0x0020`. The legacy list is empty (`osapi.c:3673-3681`) | – | probe fails |
| `pci_enable_device` (`:1988`) | – | – | probe fails |
| IRQ check (`:1995-2003`) | `pci_dev->irq==0` with no MSI-X capability is an error unless on Hyper-V (`:269-276`) | INTx line, or an MSI-X capability | probe fails ("Can't find an IRQ") |
| `nv_pci_validate_bars(only_bar0)` then `request_mem_region(BAR0)` (`:2006-2020`) | – | BAR0 start≠0, size≠0 | fail |
| BAR bookkeeping (`:2034-2047`, `:2101-2121`) | `bars[REGS]` = 1st memory BAR, `bars[FB]` = 2nd, `bars[IMEM]` = 3rd. I/O BARs are skipped | 3 memory BARs | – |
| DMA mask | set to 32-bit (`:2052`); raised later to 47-bit (see D) | – | – |
| `rm_is_supported_device` (`:2075`, `osapi.c:3693-3838`) | Maps one page of BAR0 and reads `NV_PMC_BOOT_1` `0x4`, `BOOT_0` `0x0`, `BOOT_42` `0xA00` (`:3726-3728`). Fails if BOOT_0 and BOOT_42 are both `0xFFFFFFFF` (`:3732-3743`). `halmgrGetHalForGpu` must match `BOOT_42[29:24]=0x16` (TU100 arch) and `[23:20]=6` (TU106) (`hal_mgr.c:128-150`, `g_hal_archimpl.h:69`) | BOOT_42 = TU106 | "not supported / needs GSP" → probe fails |
| `rm_init_private_state` (`:2079`, `osinit.c:1508-1614`) | Re-reads BOOT_0/1/42 (`:1537-1539`) and caches them for later GPU-lost checks | same values | – |
| `nv_pci_validate_bars(all)` (`:2085-2088`, `:1849-1899`) | Every valid BAR counts. A 64-bit BAR's zero upper half is only tolerated if the BAR is **64-bit and prefetchable** (`:1864-1884`). Needs 3 valid BARs | BAR1/BAR3: 64-bit + prefetchable (or 32-bit) | "BAR%d is invalid" → fail |
| `nv_resize_pcie_bars` (`:340-445`) | Only runs if `NVreg_EnableResizableBar` ≠ 0. **Default 0** (`nv-reg.h:1054`). Otherwise uses ReBAR on BAR index 1 and releases/reassigns BAR3 | ReBAR cap optional | fatal only if BAR1/BAR3 end up unassigned |
| ATS (`:2129-2165`) | Queries `pci_ats_supported`. Cleared if SVA cannot be enabled | ATS optional | – |
| `pci_set_master` (`:2186`) | Bus mastering on | – | – |
| `primary_vga` (`:2188`) | From `IORESOURCE_ROM_SHADOW`. Only used for x86emu and console (see B) | – | – |
| VGA arbiter (`:2191-2198`) | `vga_set_legacy_decoding(NONE)` | – | – |
| `rm_get_gpu_uuid_raw` (`:2241`) | `pciPbiReadUuid`: walks config caps looking for PBI (cap id `0x09`, length `0x14`, ctrl `0x01`), or checks the static location `0xB4`. Mailbox at +`0x04/0x08/0x0C/0x10`, polls COMMAND status (`pci_pbi.c:68-336`) | **optional** | NOT_SUPPORTED; the UUID later comes from GSP static info (`gpu_gspclient.c:169-190`) |
| Init timing | `NVreg_GpuInitOnProbe` default 0 (`nv-reg.h:1016`), so RM init happens on **first open** (`nv.c:1642-1702` → `nv_start_device`) | – | – |
| Interrupt setup (`nv.c:1428-1491`) | `EnableMSI` default 1 (`nv-reg.h:1032`). MSI-X if capability present and `rm_is_msix_allowed`. That returns TRUE for TU on bare metal and in a VM; the GFW check is only for GA100/AD10x (`gpu_mgr.c:4941-4967`). Otherwise MSI if capability present, otherwise INTx | – | none of the three → "No interrupts of any type" `-EIO` (`:1449-1456`) |
| MSI-X (`nv-msi.c:66-125`) | Vector count = `PCI_MSIX_FLAGS_QSIZE`+1, capped at 8 (`nv.h:894`). `pci_enable_msix_range(n,n)` must give all n. All vectors share one ISR (`nv-msi.c:127-150`) | – | falls back to MSI, then INTx |
| MSI (`nv-msi.c:28-64`) | Single vector | – | falls back to INTx |
| `rm_init_adapter` (`nv.c:1519`) | see B | – | `-EIO` |
| ISR (`nv.c:2885-2980`) | If `rm_isr` reports nothing handled on MSI/MSI-X, the IRQ is counted as unhandled; only a print results | – | – |
| Runtime PM | `DynamicPowerManagement` default 3 resolves to NEVER for pre-GA102 chips (`dynamic-power.c:763-806`), so no runtime D3 | – | – |

## B. `RmInitAdapter` (`osinit.c:1971-2379`) in order

1. **`RmSetupRegisters`** (`:1995`, `:1444-1506`) maps **all of BAR0**, uncached.
2. **`RmInitDeviceDma`** (`:2002`, `:1157-1178`) creates `IO_VASPACE_A` when `iovaspace_id` is set (see D).
3. **`RmFetchGspRmImages`** (`:2022`, `:1880-1954`):
   - Firmware family is `TU10X` for impl ≤ TU106 (`nv-firmware-chip-family-select.h:40-44`).
   - `request_firmware("…gsp_tu10x")` (`nv-firmware.h:124`).
   - `ucodes_tu10x` is loaded only if regkey `GR3428ReadUcodesBin`≠0 (`:1917-1924`, `nv-firmware.h:129`).
   - If the file is missing and fallback is not allowed → NOT_SUPPORTED.
   - Parsing of the images is part of GSP boot (skipped here).
4. **`osInitNvMapping` → `gpumgrAttachGpu`** (`:775-985`, `gpu_mgr.c:1521-1638`):
   - Passes fbPhys = BAR1, devPhys/regBase/regLength = BAR0, instPhys/instLength = BAR3, and the encoded DBDF (`osinit.c:852-882`).
   - Reads `BOOT_0`, `BOOT_42` (`gpu_mgr.c:1267-1268`) and `BOOT_1` (`:1014`, VF test: `BOOT_1[17:16]` must be 0).
   - `gpuPostConstruct` (`gpu.c:360-560`): `BOOT_1` again in `gpuDetermineVirtualMode` (`gpu.c:4512`); `gpuGetIdInfo_GM107` reads **`0x88008`, `0x8802C`, `0x88000`** (`kern_gpu_gm107.c:397-425`, `gpu.c:486`).
   - Engine constructors run here (KernelBif, KernelMemorySystem; see C).
   - Then reads **`NV_PMC_INTR_EN(0)` `0x140`**. If it decodes ≠ DISABLED, the driver writes the disable (`gpu_mgr.c:1592-1603`).
5. **`osInitScalability` → `clInitPcie` → `clUpdatePcieConfig`** (`:2071`, `chipset_pcie.c:1414-1437`, `:1240-1330`):
   - Real config reads of the GPU (class `0x0B`, capability list) and of the upstream bridge.
   - `kbifInitPcieDeviceControlStatus` does read-modify-write of **`0x88080`** (relaxed ordering bit 4, no-snoop bit 11) (`kernel_bif.c:815-864`, `kernel_bif_gm107.c:249-296`).
6. **`RmDeterminePrimaryDevice` / `RmSetConsolePreservationParams`** (`:2094-2096`, `:1037-1154`): reads **`NV_FUSE_STATUS_OPT_DISPLAY` `0x21C04`**; bit0 == 0 means display present (`kern_gpu_gm107.c:61-68`, `gm107/dev_fuse.h:27-29`). The EFI/VGA console size comes from the OS; there is no device access.
7. **`kgspInitRm`** (`:2115`): GSP boot, out of scope.
8. **`RmInitX86Emu`** (`:2164`): only when primary VGA; it maps system legacy ranges, not the device (`vbioscall.c:291-292`).
9. **`RmInitNvDevice`** (`:2182`, `:1245-1319`) runs PreInit → StateInit → StateLoad; engines are covered in C.
   - `osInitMapping` (called from `kbifStateInitLocked`, `kernel_bif.c:137`) writes **`0x88030` = 0** (ROM BAR off) and **`0x8800C`** latency timer (`osinit.c:1333-1337`). These go through the BAR0 mirror, not config cycles.
   - `kbifCheckAndRearmMSI` runs after StateInit (`:1294`).
10. **`osVerifySystemEnvironment`**, the interrupt test (`:2212`); detailed under Intr in C.
11. **`krcWatchdogInit`** (`:2246`); see KernelRc in C.
12. **`RmUnixAllocRmApi`** (`:2274`) and **`RmInitGpuInfoWithRmApi`** (`:2282`): control calls. The latter **fails init if there is no UUID** (`:721-726`).
13. **`kfifoGetUserdBar1MapInfo_TU102`** (`:2301`, `kernel_fifo_tu102.c:837-867`): software state only.

The RM also checks BAR0 health at runtime:
- `gpuSanityCheckRegRead`: any `0xFFFFFFFF` read triggers a `BOOT_0` read; if that is also all-ones → `osHandleGpuLost` (`gpu_access.c:1291-1348`). `0xBAD00000`-masked values only produce a log (`:1352-1355`).
- `osHandleGpuLost` compares `BOOT_0` with the cached value (`osinit.c:357-383`).
- A 1 Hz callback `gpuVerifyExistence` reads `BOOT_0` (`kernel_bif.c:184-188`, `:1232-1245`, `gpu_access.c:1250-1266`).

## C. Per engine (TU106, `PF_KERNEL_ONLY`)

Children present: `kern_gpu_tu102.c` `gpuChildrenPresent_TU106`.

### OBJGPU / generic

| Registers | When | R/W | Expected | Mismatch |
|---|---|---|---|---|
| `BOOT_0` `0x0`, `BOOT_1` `0x4`, `BOOT_42` `0xA00` | probe, attach, runtime | R | constant; `BOOT_42` arch `0x16` impl 6; `BOOT_1[17:16]=0` | probe fails / "GPU lost" |
| `0x88000`/`0x88008`/`0x8802C` (XVE_ID/REV/SUBSYS) | construct | R | mirror of config dwords 0x00, 0x08, 0x2C | stored only |
| `0x21C04` fuse | pre-GSP; KernelDisplay pre-init (`kern_disp.c:444`) | R | bit0 = 0 | display engine removed |
| `0x88030` write 0, `0x8800C` RMW | StateInit; `kmcStateLoad` (`kernel_mc.c:197-262`) | W / RMW | – | – |

### KernelBif

| Registers (BAR0 `0x88000`+offset unless noted) | When | R/W | Notes / expected |
|---|---|---|---|
| `0x7C` DEVICE_CAPABILITY bit28 (FLR) | construct (`kernel_bif_tu102.c:207-226`) | R | cached only |
| `0xBD8`, `0xBE0` via mirror; `0xBF0`–`0xC00` via **real config cycles**; `0xBF4`, `0xBFC` via mirror | construct (`:234-282`) | R | SR-IOV info; only `NV_ASSERT` on error |
| `0x84` LINK_CAPABILITIES | construct (`kernel_bif.c:1256-1283`) | R | cached and passed to GSP |
| `0x80` DEVICE_CONTROL_STATUS | pre-GSP (see B5); StateLoad | RMW / R+W1C | error status bits are cleared if set (`kernel_bif.c:992-`, `kernel_bif_gm107.c:707-784`) |
| `0x424`, `0x430` AER uncorr/corr | StateLoad | R, W-clear | same |
| `0x68` bit16 (MSI enable) | first `kbifIsMSIEnabled`, then cached (`kernel_bif_gm107.c:162-181`) | R | **must mirror the real MSI Enable bit** |
| `0xC8` bit31 (MSI-X enable) | same (`kernel_bif_tu102.c:61-84`) | R | **must mirror the real MSI-X Enable bit** |
| MSI re-arm: write `0x88704` (NV_XVE_CYA_2), value ignored | every ISR and interrupt test, when MSI is enabled (`kernel_bif.c:876-904`, `kernel_bif_gm107.c:136-151`, property TRUE for TU at `g_kernel_bif_nvoc.c:268-278`) | W | – |
| MSI-X re-arm: `intrRetriggerTopLevel_TU102` writes TOP_EN_CLEAR/SET `0xB81610`/`0xB81608` | every ISR, when MSI-X is enabled (`intr_tu102.c:1094-1160`) | W | – |

### Intr (`NV_CTRL` tree at `0xB80000`)

Register map (`dev_vm.h:31-66`):

| Register | BAR0 offset |
|---|---|
| TOP(0) | `0xB81600` |
| TOP_EN_SET(0) | `0xB81608` |
| TOP_EN_CLEAR(0) | `0xB81610` |
| LEAF(i) | `0xB81000`+4i |
| LEAF_EN_SET(i) | `0xB81200`+4i |
| LEAF_EN_CLEAR(i) | `0xB81400`+4i (i < 8) |
| LEAF_TRIGGER | `0xB81640` |

Legacy PMC: `INTR_EN(0)` `0x140` (read), `EN_SET(0)` `0x160`, `EN_CLEAR(0)` `0x180` (written). The TU HAL uses the GP100 variants (`g_intr_nvoc.c:982-1000`, `intr_gp100.c:35-152`).

- **StatePreInit:** the interrupt table and subtree map come from GSP (`intr.c:1019-1078`).
- **StateLoad** (`intr_tu102.c:80-147`):
  - Writes 0xFFFFFFFF to all 8 LEAF_EN_CLEAR registers.
  - Writes LEAF_EN_SET for each table vector.
  - `intrCacheIntrFields` **reads LEAF_EN_SET** of the UVM-shared subtree leaves (`:261-268`), so **EN_SET must read back the current enable mask**.
  - Fails state load if the GSP table puts the replayable-fault and access-counter vectors in different leaves, or the UVM subtree masks are wrong (`:208-259`).
- **Interrupt test** (`os_sanity.c:119-294`, `intr_swintr_tu102.c:40-133`, `dev_ctrl.h:35` vector 129 → leaf 4 bit 1 → subtree 2 → TOP(0) bit 2):
  1. Save and disable the non-stall tree (TOP_EN reads/clears).
  2. Read `0x140`; write `0x180`/`0x160` with DISABLED.
  3. `_intrDisableStall` (leaf and top clears).
  4. Clear: write LEAF(4) `0xB81010` = `0x2`.
  5. Write `0x180`/`0x160` with SOFTWARE (`0x80000000` & mask).
  6. LEAF_EN_SET(4) `0xB81210` = `0x2`; TOP_EN_SET(0) `0xB81608` = `0x4`.
  7. `testIntr = TRUE`; **write 129 to `0xB81640`** (LEAF_TRIGGER); `kbifCheckAndRearmMSI`.
  8. Poll `interrupt_triggered` every 5 µs up to `timeoutData.defaultus` (= **4 s**, graphics mode, `os.c:2164-2204`).
  9. ISR (`unix_intr.c:780-828` → `g_os_nvoc.h:1186-1206`): `osSanityTestIsr` reads LEAF(4) `0xB81010` and needs bit 1 set, then clears it by writing `0x2` to `0xB81010`.
  10. Restore; mismatch → `NV_ERR_IRQ_NOT_FIRING` and `RmInitAdapter` fails (`osinit.c:2217-2227`).

  **The device must:** latch bit 129 into LEAF(4), propagate it to TOP(0) bit 2 while TOP_EN bit 2 and LEAF_EN are set, raise MSI/MSI-X/INTx, and treat LEAF writes as write-1-to-clear. `rm_isr` ignores interrupts until the GPU_STATE_LOAD init flag is set (`unix_intr.c:790-793`).
- **Normal ISR** (`unix_intr.c:173-589`): reads TOP and LEAF registers, then `0x140` (the decode must be ≤ 3, which it always is: `intr_tu102.c:1481-1505`).

### KernelBus (BAR0 window, BAR2, BAR1)

- **Construct:** BAR addresses come from attach args (`kern_bus_gm107.c:4720-4741`, `:4289-4298`). BAR2 limits are fixed for TU (`bIsEntireBar2RegionVirtuallyAddressible=FALSE`, `g_kern_bus_nvoc.c:259-268`): `rmApertureLimit` = 16 MiB−1 and `cpuVisibleLimit` = 32 MiB−1 (`:4309-4370`, `g_kern_bus_nvoc.h:185-186`).
- **PreInit:** PCI BAR sizes come from GSP via `NV2080_CTRL_CMD_BUS_GET_PCI_BAR_INFO` (`kern_bus.c:589-614`). If BAR1 ≤ 256 MB, 64 KB mappings are forced (`kern_bus.c:514-542`).
- **StateInit** (`:379-480`, `:266-376`):

| Step | Registers / aperture | R/W | Expected | Mismatch |
|---|---|---|---|---|
| `kbusSetupDefaultBar0Window` (`:198-260`, `:4744-4776`) | `NV_PBUS_BAR0_WINDOW` `0x1700` (BASE[23:0] in 64 KB units, TARGET[25:24], 0 = vidmem) | R then W | window at FB top − 1 MB, or at the reserved base | – |
| BAR2 page-table bootstrap (`:1629-1958`, `:2122-2160`, `bar2_walk.c:224-265,427-450`) | PRAMIN `0x700000`–`0x7FFFFF` (`dev_ram.h:26`, `gm107/dev_ram.h:33`) | W | writes land in FB at window base + offset | – |
| `kbusPatchBar2Pdb_GSPCLIENT` (`kern_bus.c:848-902`) | reads kernel PDE3[0] at `bar2OffsetInBar0Window` (PRAMIN) → RPC `UPDATE_BAR_PDE` | R | readback of what was written | – |
| `kbusSetupBar2CpuAperture` (`:1491-1550`) | `osMapPciMemoryKernelOld(BAR3, rmApertureLimit+1 = 16 MiB, WC)` | map | PCI BAR3 ≥ 16 MiB | "Unable to map bar2" → fail |
| `kbusCommitBar2_KERNEL` (`kern_bus.c:457-473`) | `kbusInitVirtualBar2` and a flush page, both through BAR2 | – | – | – |
| **`kbusVerifyBar2_GM107`** (`:3986-4281`; runs because COHERENT_CPU_MAPPING is false, `:359-363`) | Save `0x1700`; point it at the test page; PRAMIN read, write `0xABCDABCD`, read back ×2 (with an RPC L2 evict in between). Write 16 bytes `0xABCDABCD` through the BAR2 CPU mapping; read them back via PRAMIN; write `+0x10` via PRAMIN; read back via BAR2 | R/W | **PRAMIN and BAR2 must alias the same FB bytes**. BAR2 accesses are translated by the GMMU page tables (GSP's `bar2PdeBase` with the kernel's PDE3[0] merged in) | `NV_ERR_MEMORY_ERROR`, StateInit fails |
| Failure path | `kbusTeardownBar2CpuAperture_GM107` writes `NV_PBUS_BAR2_BLOCK` `0x1714` MODE=PHYSICAL (`:1606-1613`) | W | – | – |

- **StatePostLoad:** `kbusInitBar1_GM107` builds the BAR1 VA space and patches its root to GSP's `bar1PdeBase` (`:879-1180`, `kern_bus.c:766-827`). Static BAR1 is off unless BAR1 ≥ FB + extra (`kern_bus_tu102.c:367-500`).
- **Flushes:** `kbusFlushSingle_GV100` reads a BAR2-mapped FB page (`kern_bus_gv100.c:340-420`); sysmembar is an RPC (`kern_bus.c:431-445`).

### KernelGmmu

- **TLB invalidate** on every BAR1/BAR2 PTE change: `kgmmuInvalidateTlb_GM107` (`kern_gmmu_gm107.c:106-285`), `_TU102` (`kern_gmmu_tu102.c:50-150`).
  - Poll `MMU_INVALIDATE` **`0xB830B0`** until TRIGGER (bit 31) = 0.
  - Write `INVALIDATE_PDB`/`UPPER_PDB` `0xB830A0`/`0xB830A4`.
  - Write `0xB830B0`, then poll again.
  - **TRIGGER must self-clear**, otherwise the 4 s OS-timer timeout fails the operation.
- Fault buffers and access counters: not touched at init (stubs above; replayable buffer registration is an RPC at `kern_gmmu.c:1232-1290`).

### KernelMemorySystem

- **At construct**, before GSP boot (`kern_mem_sys.c:81-103`, `kern_mem_sys_gm107.c:213-352`):
  - Allocates a 4 KB sysmem page with the 32-bit DMA flag (must end below 2^40, otherwise retries with a temporary 40-bit mask).
  - **Read-modify-write of `NV_PFB_NISO_FLUSH_SYSMEM_ADDR` `0x100C10` = dmaAddr>>8** (`gm107/dev_fb.h:26-28`).
- **StatePreLoad:** writes `0x100C10` again (`kern_mem_sys.c:261-273`); the validity assert is a stub.
- L2 flush/evict: RPC (`kern_mem_sys.c:1122-1136`).

### KernelFifo

- **StatePostLoad** (`kernel_fifo_gm107.c:162-231`):
  - Preallocates USERD in FB and maps it into BAR1 (`kernel_fifo_tu102.c:489-680`).
  - Then **`kfifoTriggerPostSchedulingEnableCallback`** → `memmgrInitInternalChannels` → global CeUtils (`mem_mgr.c:463-509`, `:4114-4172`).
- **CeUtils test** `memmgrTestCeUtils` (`mem_mgr.c:390-460`):
  - CE memset of 4 bytes in FB, BAR2 write `0xAABBCCDD`, **CE copy FB → sysmem**, CPU readback must match.
  - The doorbell is at `NV_VIRTUAL_FUNCTION_DOORBELL` `0xBB0090` (`dev_vm.h:229`, `kernel_fifo_tu102.c:59`, `channel_utils.c:580`).
  - **Failure propagates and fails StateLoad.**
  - Skipped by regkey `RmDisableGlobalCeUtils=1` (`mem_mgr.c:330-334`, `nvrm_registry.h:975`).

### OBJTMR

- RMTIMEOUT uses the **OS timer** for GSP clients (`gpu_timeout.c:45-77`), not PTIMER.
- `tmrGetCurrentTime` reads **`0xBB0084`/`0xBB0080`** (VF TIME_1/TIME_0, `dev_vm.h:224-227`, `timer_tu102.c:142,159`):
  - TIME_0 bits[4:0] must be 0, otherwise 3 retries and then "Consistently Bad TimeLo" plus a timer state reset.
  - TIME_1 must be stable across the two reads (`timer_gm107.c:226-276`).
  - Used for notifier timestamps (e.g. `kernel_channel.c:1852`). Advancing is not required for init polling.
- Callbacks use the OS timer on `PF_KERNEL_ONLY` (`g_objtmr_nvoc.c:298-313`).

### KernelRc (watchdog)

`krcWatchdogInit` (`kernel_rc_watchdog.c:467-1267`) runs by default. It is disabled only by `RmRcWatchdog=0` (`kernel_rc.c:197-211`). It:
- allocates client/device/subdevice, a sysmem pushbuffer and notifiers, a USERD in vidmem (CPU-mapped through BAR1), a GR channel, and a `FERMI_TWOD_A` object (all RPCs);
- writes the GPFIFO entry and `GPPut=1` through the BAR1 USERD mapping;
- rings **`0xBB0090`** with the work-submit token (`:1467-1511`).

Errors other than NOT_SUPPORTED fail `RmInitAdapter` (`osinit.c:2246-2266`). The watchdog is then disabled (`:2250`).

### KernelDisplay, CE, GR, PMU, SEC2, Nvlink, MC, Perf, HWPM

- **Display:**
  - Construct/init/load are RPCs, apart from the `0x21C04` fuse read.
  - Instance-memory and shared-memory FB allocations (`kern_disp.c:435-465,553-645,772-788`).
  - `PDISP` registers are only in ISR/vblank paths.
- **KernelMc** touches only `0x8800C` (see OBJGPU).
- **GR** FECS mailbox registers only for the ctxsw trace feature (`kgraphics_gp100.c:48-104`).
- **SEC2** reset only during GSP boot.
- **CE, PMU, Nvlink, Perf, HWPM:** I found no direct register access in any of these engines' init paths.
- **UVM access counters** (`uvm_tu102.c`) only when the UVM module sets them up.

### KernelGsp after boot

Every RPC writes **`NV_PGSP_QUEUE_HEAD(0)` `0x110C00`** (`kernel_gsp_tu102.c:383`, `dev_gsp.h:38`). Replies are drained from the sysmem shared queue by polling (`kernel_gsp.c:2960-3060`).

## D. DMA

- **DMA mask:** 32-bit at probe; then `gpuarchGetSystemPhysAddrWidth_TU102` = **47** (`hwproject.h:27`) because `DmaAddrWidth` is 0 for TU (`g_gpu_arch_nvoc.c:195-204`). Applied by `dma_set_mask` and `dma_set_coherent_mask` (`osinit.c:1590-1595`, `nv.c:3235-3256`).
- **IOMMU:** `nv_requires_dma_remap` = !swiotlb (`nv.c:5243-5250`, `nv-linux.h:967-`). If true, `iovaspace_id = gpuId` and an `IO_VASPACE_A` is created (`osinit.c:1575`, `:1157-1178`).
- **Sysmem buffers whose addresses reach hardware or GSP:**
  - NISO flush page → register `0x100C10`
  - GSP WPR meta, libos args, GSP args, sysmem heap, message-queue shared memory, logs (`kernel_gsp_tu102.c:125-214`, `message_queue_cpu.c:254`) → via GSP boot/mailboxes
  - watchdog pushbuffer/notifier and CeUtils buffers → through GPU page tables
  - replayable fault client buffer → RPC (only when UVM requests it)
- **FB accesses by the CPU:** only via PRAMIN (bootstrap/verify) and BAR2/BAR1 GMMU mappings.

## E. PCI config and BAR expectations

- **Interrupts:** MSI-X preferred, then MSI, then INTx. MSI-only with `irq==0` fails probe (`nv-pci.c:1995`).
- **Capability offsets:** the RM reads MSI/MSI-X enable through the BAR0 mirror at **fixed offsets `0x68`/`0xC8`**. The mirror at `0x88000+` must track the live config space, including Linux's MSI/MSI-X writes.
- **PM capability:** no D-state transitions (runtime PM is NEVER for TU).
- **Link width/speed:** cached from `0x88084`; link control is read only in control calls.
- **Expansion ROM:** the PCI ROM BAR is never read. It is disabled via mirror `0x88030` (`osinit.c:1333`). The VBIOS is read through BAR0 PROM `0x300000` during GSP boot (`kernel_gsp_vbios_tu102.c:70,85`).
- **BAR sizes the code depends on:**

| BAR | Requirement | Source |
|---|---|---|
| BAR0 | Fully mapped; accesses reach `0xBB0090` and `0xB90000`+ (MSI-X table offset `0x10000` in the VF window, `dev_vm.h:222`), so ≥ ~12 MiB (16 MiB is fine) | – |
| BAR1 | Any size; ≤ 256 MB forces 64 KB mappings. Must hold USERD for all channels plus watchdog and CeUtils mappings | – |
| BAR3 | ≥ 16 MiB (mapped); BAR2 VA spans 32 MiB | – |
| BAR5 | Ignored (I/O) | – |
| ReBAR | Off by default | – |

## Minimum emulation checklist, in the order the driver first needs it

1. **Config space:** class `0x0300xx`/`0x0302xx` (prog-if 0), vendor `0x10DE`. Three memory BARs; 64-bit ones prefetchable. An INTx line or an MSI-X capability; MSI/MSI-X capabilities recommended. Optional PBI at `0xB4`.
2. **BAR0 reads:** `0x0` = BOOT_0, `0x4` = 0, `0xA00` = TU106 BOOT_42. Never return all-ones for these.
3. **BAR0 mirror `0x88000`–`0x88FFF`:** dwords `0x00`, `0x08`, `0x2C`, `0x68` (live MSI enable), `0x7C`, `0x80` (RW), `0x84`, `0xC8` (live MSI-X enable), `0x424`, `0x430`, `0xBD8`–`0xC00`; writes to `0x0C`, `0x30`, `0x80`, `0x704` accepted. Extended config `0xBF0`–`0xC00` must also be readable through real config cycles.
4. **`0x140`** reads 0; `0x160`/`0x180` accepted. **`0x21C04`** bit0 = 0.
5. **`0x100C10`** read/write, keeps the written value.
6. **GSP boot and RPC channel:** `0x110C00` doorbell plus everything GSP boot needs (out of scope). GSP static info must supply a consistent FB size and reserved layout, `bar1PdeBase`/`bar2PdeBase`, the interrupt table and subtree map, BAR sizes and the GID.
7. **`NV_PBUS_BAR0_WINDOW` `0x1700`** read/write, plus the **PRAMIN** 1 MiB window to FB (vidmem target).
8. **BAR2 (PCI BAR3)** GMMU-translated to FB, coherent with PRAMIN, rooted at GSP's BAR2 PDB with the kernel's PDE3[0] merged in.
9. **`0xB830A0`/`A4`/`B0`** TLB invalidate, with TRIGGER self-clearing.
10. **Interrupt tree** `0xB81000`–`0xB81640`: LEAF (W1C), LEAF_EN_SET/CLEAR (EN_SET reads back), TOP, TOP_EN_SET/CLEAR, LEAF_TRIGGER. Must raise MSI/MSI-X/INTx within 4 s. Accept `0x88704` (MSI re-arm).
11. **BAR1** GMMU-translated (USERD, watchdog).
12. **Doorbell `0xBB0090`** plus real CE execution for the CeUtils memset/copy, or set `RmDisableGlobalCeUtils=1`. The GR channel alloc/object for the watchdog, or set `RmRcWatchdog=0`.
13. **`0xBB0080`/`0xBB0084`** timer with low 5 bits zero and a stable high word.
14. **`BOOT_0`** keeps matching (1 Hz existence check).

## Open questions

1. Which MSI-X vector hardware/GSP uses on TU106. The kernel requests all vectors (≤ 8) and handles them identically (`nv-msi.c:127-150`), and the VF table has 6 entries (`dev_vm.h:223`).
2. The mirror offsets (`0x68`, `0xC8`, `0x78`/`0x80`/`0x84` PCIe) are hardcoded in `gm107/dev_nv_xve.h:40-169` and `tu102/dev_nv_xve.h:35-54`. Whether the emulated config space must place its capabilities at the same offsets depends on how the mirror is implemented; the source does not say.
3. No kernel code checks that the GSP-reported BAR sizes equal the PCI resource sizes; BAR3 is mapped with `rmApertureLimit+1` without a check (`kern_bus_gm107.c:1528-1532`).
4. When the watchdog is re-enabled after `krcWatchdogDisable`, and whether it then waits on the notifier: `krcWatchdogTimerProc` (`kernel_rc_watchdog_callback.c`) was not traced.
5. The exact GMMU v2 PDE/PTE layout the emulator must walk for BAR1/BAR2 comes from `kgmmuFmtGet(...VERSION_DEFAULT)` (`kern_gmmu.c:708`, `kern_gmmu_fmt_*`); not detailed here.
6. `hypervisorDetection` inside a VM and the `IS_PASSTHRU` effects (`gpu.c:5030-5075` reads real config `0x14`/`0x18`) were not fully traced.
