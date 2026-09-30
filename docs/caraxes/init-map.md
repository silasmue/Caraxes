<!-- SPDX-License-Identifier: MIT -->
# From PCI enumeration to the first meaningful GSP RPC

The path the stock open driver (610.57.04) takes on a Turing identity in GSP-client
mode. Paths are relative to `vendor/open-gpu-kernel-modules`. Each step names what
the synthetic device must provide ("device:") and what the guest must have ("guest:").
The register-level detail of phase 4 is in [gsp-boot-turing.md](gsp-boot-turing.md);
the queue/RPC detail of phase 5 is in [gsp-rpc-protocol.md](../history/caraxes-gsp-rpc-protocol-610.md).

## Phase 0: module load and match

1. `nv_pci_register_driver` registers `nv_pci_table` (`kernel-open/nvidia/nv-pci-table.c:18-37`):
   vendor `0x10de`, any device id, class `0x0300xx` (VGA) or `0x0302xx` (3D).
   **device:** vendor 10de, class 0x030000 (or 0x030200 for a headless identity).
2. Nothing is read from the device until probe.

## Phase 1: `nv_pci_probe` (`kernel-open/nvidia/nv-pci.c:1903-2350`)

| Step | Code | Device must |
|---|---|---|
| VF check | `pci_dev->is_virtfn` (:1924) | not be an SR-IOV VF (a plain function is fine) |
| BAR firewall wait | `rm_wait_for_bar_firewall` (:1957; `osapi.c:6184`) | Blackwell-only per its callers; confirm in [kernel-rm-hw-surface.md](kernel-rm-hw-surface.md) |
| Class/vendor/legacy | `rm_is_supported_pci_device` (:1971; `osapi.c:3840-3886`): class 03, subclass 00 or 02, vendor 10de, device ≥ 0x0020, not in the legacy list | device id `0x1f02` passes |
| Enable | `pci_enable_device` (:1988) | config space: command register writable |
| IRQ present | `pci_dev->irq == 0 && no MSI-X cap` → error unless Hyper-V (:1995-2004) | INTx pin set in config (QEMU assigns a line) or an MSI-X capability |
| BAR0 valid | `nv_pci_validate_bars(only_bar0)` (:2006; :1846-1899) | a valid memory BAR0 |
| Reserve BAR0 | `request_mem_region` (:2009) | — |
| ID registers | `rm_is_supported_device` (:2075; `osapi.c:3727-3830`) maps one page of BAR0 and reads `NV_PMC_BOOT_1` (0x4), `BOOT_0` (0x0), `BOOT_42` (0xA00). Refuses if BOOT_0 == BOOT_42 == 0xFFFFFFFF ("fallen off the bus"), if legacy, or if `halmgrGetHalForGpu` finds no HAL | **BOOT_0 = 0x166000A1, BOOT_42 with chip id 0x166 (TU106), BOOT_1 = 0** |
| Private state | `rm_init_private_state` (:2079; `osinit.c:1508-1616`) re-reads the three, registers the GPU id, picks `GpuArch` from BOOT_42 for the DMA width | same values, stable |
| Other BARs | `nv_pci_validate_bars(all)` (:2087) | BAR1 and BAR3 (64-bit prefetchable) valid; BAR5 I/O accepted |
| Resizable BAR | `nv_resize_pcie_bars` (:2090; :340-460) | no Resizable BAR capability → no-op (keep it out of the config space initially) |
| Bus master, VGA | `pci_set_master`, VGA arbitration, `primary_vga` from ROM shadow flag (:2186-2198) | — |
| GPU lost check | `nv_check_gpu_state` (:2200) | BOOT_0 unchanged |
| Init on probe | `nvl->init_on_probe = NVreg_GpuInitOnProbe` (:2210); if set, `nv_start_device` runs right here (:2285-2300) | **guest:** `modprobe nvidia NVreg_GpuInitOnProbe=1` runs the whole init at load time, no userspace needed |
| Char devices | minor assigned, `/dev/nvidia0` created by udev/nvidia-modprobe later | — |

## Phase 2: first open, `nv_start_device` (`kernel-open/nvidia/nv.c:1372-1640`)

| Step | Code | Device must |
|---|---|---|
| MSI | if `NVreg_EnableMSI` (default 1): MSI-X if capability and `rm_is_msix_allowed`, else MSI if capability (:1432-1447) | an MSI capability (the real card: 64-bit, 1 vector, no masking) → QEMU turns the enable into `SET_IRQS` on index 1 |
| IRQ handler | `request_threaded_irq(nvidia_isr, nvidia_isr_kthread_bh)` (:1467-1469) | — |
| RM init | `rm_init_adapter` (:1519) → `RmInitAdapter` | phase 3–5 |
| UUID | `rm_get_gpu_uuid_raw` (:1529) | comes from `GspStaticConfigInfo.gidInfo` |

## Phase 3: `RmInitAdapter` before the GSP boot (`src/nvidia/arch/nvalloc/unix/src/osinit.c:1971-2115`)

1. `RmSetupRegisters` (:1995; :1444-1506): maps all of BAR0 (`nv_os_map_kernel_space`).
2. `RmInitDeviceDma` (:2002): DMA mask from the `GpuArch` width (47 bits for Turing per
   `/proc/driver/nvidia/gpus/.../information` on the host).
3. `RmFetchGspRmImages` (:2022; :1881-1958): `request_firmware("nvidia/610.57.04/gsp_tu10x.bin")`
   (`nv-firmware.h:111-119`); optional `ucodes_tu10x.bin`, `gsp_log_tu10x.bin`.
   **guest:** the firmware file must be present; `.fwversion` must be "610.57.04".
4. `osInitNvMapping` (:2048): `gpumgrAttachGpu` → `gpuPostConstruct`
   (`src/nvidia/src/kernel/gpu/gpu.c:362-470`): `gpumgrGetGpuHalFactor` reads BOOT_0/42/1
   (variant = PF_KERNEL_ONLY), `gpuDetermineVirtualMode` reads BOOT_1 again, child
   engines are constructed. `KernelGsp` construction (`kernel_gsp.c:4338-4400`) configures
   the falcon bases (GSP 0x110000, SEC2 0x840000), allocates the message queues and
   the boot-args pages. **device:** besides the BOOT registers, `gpuGetIdInfo_GM107`
   reads the config-space mirror at 0x88000/0x88008/0x8802C, `gpumgrAttachGpu` reads
   `NV_PMC_INTR_EN(0)` 0x140, and the KernelMemorySystem constructor read-modify-writes
   `NV_PFB_NISO_FLUSH_SYSMEM_ADDR` 0x100C10 (kernel-rm-hw-surface.md §B4, §C).
5. `osInitScalability` (:2071; PCIe setup reads real config space and read-modify-writes
   the mirrored device control register 0x88080), `gpuBootGspRmProxy` skipped for GSP clients (:2083-2085),
   `RmDeterminePrimaryDevice`, `RmSetConsolePreservationParams`, `RmInitAcpiMethods`
   (:2094-2098). **device:** primary-VGA detection depends on the ROM shadow flag from
   probe; keep the device non-primary (no ROM shadow) to avoid console preservation.

## Phase 4: `kgspInitRm` (`src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:4878-5210`)

Ordered summary; the full register table is section 2 of [gsp-boot-turing.md](gsp-boot-turing.md).

| # | What the CPU does | Device must |
|---|---|---|
| 4.1 | Extract the VBIOS through the PROM window `NV_PROM_DATA` = BAR0 + 0x300000 + i (`kernel_gsp_vbios_tu102.c:442-569`): PCI ROM signature, image chain by 512-byte blocks, NVIDIA extension images (code type 0xE0) via the NPDE, up to 1 MiB | serve a **complete** VBIOS image at 0x300000 (the sysfs `rom.bin` dump lacks the extension image, see experiment R1) |
| 4.2 | Parse the FWSEC ucode from the BIT tables (`kernel_gsp_fwsec.c:1110`); read fuse `NV_FUSE_OPT_SECURE_GSP_DEBUG_DIS` 0x2174C bit 0 (production = 1) | fuse register value; the VBIOS must contain a V2 FWSEC descriptor |
| 4.3 | Build booter images from bindata, boot ucode, radix3 of `.fwimage`, signature memdesc, libos log buffers | nothing (all in guest RAM) |
| 4.4 | `kgspWaitForGfwBootOk`: poll GSP `CPUCTL` 0x110100 bit 4 HALTED (2.05 s); read `0x118128` bit 0 = 1; read `0x118234` bits 7:0 = 0xFF | those three values |
| 4.5 | `tmrSetCurrentTime`: read `0x9430` bit 4 (ignored on failure); write `PTIMER_TIME_1/0` 0x9410/0x9400 | accept writes |
| 4.6 | Populate `GSP_ARGUMENTS_CACHED` (queue offsets, PA of the queue page table) and libos args (LOGINIT, LOGRM, RMARGS) | nothing yet |
| 4.7 | `_kgspBootGspRm`: read FECS feature readout 0x409660; `WPR2_ADDR_HI` 0x1FA828 must be 0; read `LOCAL_MEMORY_RANGE` 0x100CE0 (**this defines the FB size the guest believes in**); display fuse 0x21C04; `VGA_WORKSPACE_BASE` 0x625F04 if display present; compute the WPR meta (`GspFwWprMeta`, 256 B) | FB size register, WPR2 = 0 |
| 4.8 | `kgspPrepareForBootstrap`: GSP `HWCFG2` 0x1100F4 bit 10 (RISC-V present) = 1 | that bit |
| 4.9 | `kgspBootstrap_TU102`: reset GSP (`FALCON_ENGINE` 0x1103C0 RMW, poll `DMACTL` 0x11010C scrub bits clear, write `RM` 0x110084 = BOOT_0) | falcon reset state machine |
| 4.10 | Load the generic bootloader + FWSEC into the GSP falcon: FBIF `CTL`/`TRANSCFG(4)`, `DMEMC/DMEMD` (84-byte BL descriptor), `HWCFG` IMEM size, `IMEMC/IMEMT/IMEMD` (512 B bootloader at the top of IMEM), `BOOTVEC` = 0xFD00, `CPUCTL` = 2 (start), poll `CPUCTL` HALTED | IMEM/DMEM write acceptance, HWCFG IMEM size, "halt" shortly after start |
| 4.11 | FWSEC result: `NV_PBUS_VBIOS_SCRATCH(0xE)` 0x1438 bits 31:16 = 0; `WPR2_ADDR_HI` ≠ 0; `WPR2_ADDR_LO` 0x1FA824 bits 31:4 = frtsOffset >> 12 | after the GSP falcon "ran": scratch = 0, WPR2 registers reflecting the FRTS window the CPU asked for (the frtsOffset is in the FWSEC command the CPU wrote into DMEM; the device can compute it from the WPR meta or decode the DMEM image) |
| 4.12 | Reset GSP again ("into RISC-V"); write libos-args PA to GSP `MAILBOX0/1` 0x110040/0x110044 | latch MAILBOX0/1: **this is how the device learns where the libos args are, and through them the RMARGS page and the queue page table** |
| 4.13 | Booter-Load on SEC2: reset SEC2 (0x8403C0, 0x84010C, 0x840084), FBIF, IMEM (0x100 non-secure + 0x8400 secure), DMEM (0x6200), `BOOTVEC` 0, SEC2 `MAILBOX0/1` 0x840040/44 = **WPR meta PA**, start, poll halt, `MAILBOX0` must read 0 | SEC2 state machine; latch MAILBOX0/1 (WPR meta PA); read back 0 |
| 4.14 | `kgspSendInitRpcs`: write `GSP_SET_SYSTEM_INFO` (72) then `SET_REGISTRY` (73) into the command queue, each followed by a write of 0 to `NV_PGSP_QUEUE_HEAD(0)` 0x110C00 | doorbell register; the device can now find the queue via RMARGS → `sharedMemPhysAddr` |
| 4.15 | Write GSP `FALCON_OS` 0x110080; read `RISCV_STATUS` 0x111240 bit 0 (active) **or** GSP `MAILBOX0` bit 31 | report RISC-V active |
| 4.16 | `GspStatusQueueInit`: within 4 s the status queue at +0x41000 must carry a valid `msgqTxHeader` (version 0, size 0x40000, msgSize 0x1000, rxHdrOff 0x20, entryOff 0x1000, msgCount 63); each loop runs the health check reading `DEBUGINFO` 0x110094 (must not be 0xDEAD) | write the header into guest RAM |
| 4.17 | Poll for `GSP_INIT_DONE` (0x1001), sequence 0, `rpc_result` 0, element seqNum 0, checksum 0, MCTP/NVDM headers | write the event into the status queue |
| 4.18 | Back in `kgspInitRm`: `SET_GUEST_SYSTEM_INFO` (1), sync: reply must echo `vgxVersionMajor/Minor` 0x2E/0x0D with `rpc_result_private` = 0 | answer |
| 4.19 | `GET_GSP_STATIC_INFO` (65), sync: reply carries `GspStaticConfigInfo` | **the first meaningful RPC**: FB regions (≥ 1), FB length consistent with 4.7, GID, SKU, engine caps, internal client/device/subdevice handles, BAR1/BAR2 PDE bases, VBIOS fields |

## Phase 5: after `kgspInitRm` (`osinit.c:2115-2378`)

`RmInitNvHal` → `RmInitNvDevice` → `gpuStateInit`/`gpuStateLoad` across all kernel
engines. From here the guest issues `GSP_RM_ALLOC` / `GSP_RM_CONTROL` / `FREE` /
`DUP_OBJECT`, starting with `_gpuAllocateInternalObjects` (`gpu.c:975`) using the
internal handles from static info, and `NV2080_CTRL_CMD_INTERNAL_INTR_GET_KERNEL_TABLE`
(`intr.c:1041`) to learn interrupt vectors. This is where the Leandro backend enters,
and where the kernel RM also starts to program hardware directly
([kernel-rm-hw-surface.md](kernel-rm-hw-surface.md)). Milestone 1 ("recognize and
initialize") ends at `RmInitAdapter succeeded!` (`osinit.c:2354`) with
`osVerifySystemEnvironment` (:2212) having seen a test interrupt.

## Where the first experiments will stop (predictions)

- E0 (identity only, all other reads 0): probe succeeds; `nv_start_device` on first open
  runs `RmInitAdapter`; the log shows the 0x88000 mirror reads, 0x140, 0x100C10 and
  0x21C04 before any PROM access; `kgspInitRm` fails at 4.1 with an invalid ROM signature
  (`kernel_gsp_vbios_tu102.c:492-500`, PROM reads return 0) → "RmInitAdapter failed!
  (0x…:0x…:line)" in the guest log. Before that, if the firmware file is missing,
  it fails at 3.3 instead.
- E1 (+ PROM window with the full VBIOS): proceeds to 4.4 and times out after 2.05 s
  on GSP `CPUCTL` HALTED, then `NV_ERR_NOT_READY` from the GFW check.
- E2 (+ falcon state machines and scratch values): reaches 4.16 and times out after
  4 s waiting for the status-queue header. From here on the device has to write into
  guest RAM, so DMA_MAP with fds must be working.
