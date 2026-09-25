# RTX 2070: the Windows KMD's controls on the real card (2026-09-26)

- Aim: the Windows 617.14 KMD's adapter-start controls answered by the card itself, not by guesses, so that pure emulation can use the card's values (`recorded.rs`, `chips/tu106/recorded-controls.tsv`).
- Source of the control list: night run Y4-clk64-fbtopo (`caraxes/runs/win11-2026-09-25/night-2026-09-25/`), in the KMD block from `0x20809009` to `0x20802806`, plus `0x20800aaf` and `0x800292` ahead of it.
- Tool: `caraxes/tools/kmd-controls.c`. It runs unprivileged over `/dev/nvidiactl` and creates a root client (0x41), attaches the probed GPU (id 0x2d00), then allocates `NV01_DEVICE_0` and `NV20_SUBDEVICE_0`. It issues the GET controls and frees the root client at the end. Host RM 615.71.09, run with `nice -n 15`, while the desktop was on the card.
- Output: `kmd-controls-2026-09-26.tsv` uses the `recorded-controls.tsv` column order: `cmd`, `status`, `request` hex, `answer` hex (full parameter bytes), then a comment naming the control. Totals: 184 controls, 160 NV_OK.

## Limits

- The trace keeps only the decode, not the KMD's full request bytes. FB_GET_INFO_V2 and BUS_GET_INFO_V2 therefore went out once per index, for every index in the 615 headers, followed by one call with every index that answered NV_OK. `recorded.rs` matches these two controls per index, so the answers cover any list the KMD sends.
- FB and BUS values that depend on the card's size or state come from the 8 GiB host card while the desktop was running: RAM/heap sizes, heap free, largest free region, BAR1 available and PCI bus number. The device keeps scaling these to the emulated FB.
- The following were not issued:
  - BUS indices 0x20-0x23 and 0x28-0x2b (`*_CLEAR`). They clear the card's PCIe error counters (`kern_bus_ctrl.c:228-235`). The KMD does ask for them. Their counters (0x1c-0x1f, 0x24-0x27) read 0, so 0 is the likely answer.
  - GSS legacy controls 0x20809009, 0x20809064, 0x20809001 and 0x20808159: they are not in the 615 or 610 headers.
  - LPWR 0x20802806 and BINAPI 0x20810108: not in the headers either.
  - Anything that sets, initializes or allocates: GR_GFX_POOL_INITIALIZE/ADD_SLOTS, INIT_USER_SHARED_DATA, 0x730151.
- DIFR 0x20802801 went out only with `ctrlParamVal` 3 (SUPPORT_STATUS).

## Refused for a user client

- 0x1b INSUFFICIENT_PERMISSIONS (kernel-only):
  - GR_GFX_POOL_QUERY_SIZE 0x2080121f
  - LPWR_DIFR_CTRL 0x20802801
  - CE_IS_DECOMP_LCE_ENABLED 0x20802a12 (LCE 0-4)
- 0x56 NOT_SUPPORTED:
  - INTERNAL_GET_ENABLED_SEC2_CLASSES 0x20800aaf and INTERNAL_GET_PCIE_P2P_CAPS 0x20800ab8 (internal controls)
  - the seven INTERNAL_STATIC_KGR getters 0x20800a2c/a2e/a30/a34/b03/b05/a3f
  - GPU_QUERY_ECC_STATUS 0x2080012f: this is the card's own answer, same as the native traces
  - FIFO_GET_ENGINE_CONTEXT_PROPERTIES for engines 1-7

## The card's values (the answers that were suspects)

- FB_GET_INFO_V2:
  - PARTITION_COUNT (0x04) 4, PARTITION_MASK (0x14) 0xf, FBP_COUNT/MASK (0x19/0x1a) 4 / 0xf
  - LTC_MASK (0x2b) 0xff, LTC_COUNT (0x22) 8, LTS_COUNT (0x23) 0x20
  - L2CACHE_SIZE (0x1b) 0x400000
  - RAM_TYPE (0x0d) 0x11 (GDDR6), BUS_WIDTH (0x0b) 256, BANK_COUNT (0x0e) 1, MEMORYINFO_VENDOR_ID (0x1c) 1
  - COMPRESSION_SIZE (0x01) 0xf7d00000, DRAM_PAGE_STRIDE (0x02) 0x8000, RAM_CFG (0x0c) 0
  - BAR1_SIZE (0x05) 0x40000 KiB (256 MiB), BAR1_AVAIL (0x1d) 0x39f80 KiB, BAR1_MAX_CONTIGUOUS (0x1f) 0x39780 KiB
  - FB_TAX 0, VISTA_RESERVED_HEAP (0x15) 0x400, TRAINING_2T (0x21) 0, PSEUDO_CHANNEL (0x25) 0
  - 1TO1_COMPTAG (0x28) 1, SUSPEND_RESUME_RSVD (0x29) 0x20000000, ALLOW_PAGE_RETIREMENT (0x2a) 1, FORCED_BAR1_64KB (0x2f) 1
  - ECC_STATUS_SIZE (0x35) 2, PARTITION_MASK_1/LTC_MASK_1..7 (0x37/0x38, 0x3c-0x43) 0, ACCESS_COUNTER_BUFFER_COUNT (0x39) 1, NUMA_NODE_ID (0x3b) 0xffffffff, HEAP_RECLAIMABLE (0x44) 0x15000
  - The previous guesses in `recorded.rs` (4 / 0xf / 0xff) were right. LTS_COUNT is 32 (4 per LTC), not the synthesized 8 × 2.
- BUS_GET_INFO_V2 0x14-0x2b:
  - 0x14 GEN2_INFO 0x202000, 0x15 AER 0
  - 0x16 BOARD_LINK_CAPS 0x453d03, 0x17 BOARD_LINK_CTRL_STATUS 0x11010140
  - 0x18 ASLM_STATUS 9, 0x19/0x1a switch error counts 0, 0x1b CYA_ASPM 1
  - 0x1c-0x1f and 0x24-0x27 error counters 0; the `*_CLEAR` indices were not issued
  - Further: 0x2d/0x2f/0x30 GEN_INFO 0x202000, 0x2e GPU_INTERFACE_TYPE 3, 0x31 MSI_INFO 1
- GRMGR_GET_GR_FS_INFO:
  - The KMD's 0-query request returns NV_OK, unchanged.
  - GPC_COUNT 3; CHIPLET_GPC_MAP g→g; TPC_MASK 0x3f and PPC_MASK 0x7 for GPCs 0-2; GPC 3 INVALID_ARGUMENT
  - CHIPLET_SYSPIPE_MASK 1
  - CHIPLET_GRAPHICS_SYSPIPE_MASK 0 (`recorded.rs` answers 1)
  - PARTITION_SYSPIPE_ID status 0x40; ROP_MASK, PROFILER_MON/GFX_CAPABLE_GPC_MASK and PARTITION_CHIPLET_SYSPIPE_IDS NOT_SUPPORTED (`recorded.rs` answers 8/12/7/9 with NV_OK)
  - PARTITION_CHIPLET_GPC_MAP INVALID_ARGUMENT
- GPU_GET_VPR_CAPS: NV_OK, minStartAddr 0x1bf00000, maxEndAddr 0x1fbefffff. It is not refused.
- FIFO_GET_ENGINE_CONTEXT_PROPERTIES, all with 4 KiB alignment unless noted:
  - engine 0: 0xad400 (the device answers 2 MiB)
  - 8: 0x78600
  - 9: 0x8c00
  - 0xa: 0x900000
  - 0xb: 0x10f300
  - 0xc: 0xe1000 (align 0x100)
  - 0xd: 0x20000 (align 0x100)
  - 0xe: 0x3e3a80 (the device answers INVALID_ARGUMENT)
  - 0xf: 0x82000 (align 0x100)
  - 0x10: 0x5000
- Others:
  - GPU_GET_CLASSLIST_V2: 96 classes; 615 adds 0x90d0 to the 95 of the 610 recording
  - FB_GET_COMPBIT_STORE_INFO: NV_OK (the device refuses it by design, D16)
  - FB_GET_GPU_CACHE_INFO: all 0, meaning powered, WRITETHROUGH, no bypass, RCM full (the device answers write-back)
  - GPU_QUERY_FUNCTION_STATUS: 0
  - GSP_GET_FEATURES: valid, "615.71.09"
  - NVDEC caps `01 00 00 00 16 00 00 00`, NVENC caps 0
  - GPU_GET_INFO_V2 index 0x11: 0

## Follow-up: graphics context buffers 0x11-0x19

- Run: `kmd-controls engines2`. It issues only FIFO_GET_ENGINE_CONTEXT_PROPERTIES for engines 0x11-0x19. All nine answered NV_OK, and their lines were added to the TSV.
- Sizes and alignments:
  - BUNDLE_CB (0x11): 0x3000, align 0x100
  - PAGEPOOL_GLOBAL (0x12): 0x20000, align 0x100
  - ATTRIBUTE_CB (0x13): 0x3f3a00, align 0x1000
  - RTV_CB_GLOBAL (0x14): 0x80000, align 0x100
  - GFXP_POOL (0x15): 0, align 0x1000
  - GFXP_CTRL_BLK (0x16): 0x464, align 0x1000
  - FECS_EVENT (0x17): 0x10000, align 0x1000
  - PRIV_ACCESS_MAP (0x18): 0x80000, align 0x1000
  - SETUP (0x19): 0, align 0x1000
- Not done:
  - GR_GET_CTX_BUFFER_SIZE/INFO and GR_GET_INFO_V2: blocked when the session tried to read their layouts.
  - The RM_USER_SHARED_DATA (0x00de) allocation and data poll: outside the standing approval, which covers only client, device and subdevice allocation plus GET controls. Both wait for the user.

## Third follow-up: identification only, nothing replayed

- Command ids looked up in the 615 SDK headers (`vendor/nvidia-rm-headers/615.71.09`, a partial set), with the 610 tree as fallback.
- Not found in any ctrl2080*.h or ctrl0073*.h, so not sent (unknown): 0x2080a70a, 0x2080a709, 0x2080a801, 0x2080a02f, 0x20803c01, 0x20808524, 0x73117a.
- Found; all three are pure queries (params in the table below):

| id | name | header | params |
|---|---|---|---|
| 0x73013d | NV0073_CTRL_CMD_SYSTEM_QUERY_DISPLAY_IDS_WITH_MUX | ctrl0073system.h:1714 (615) | {subDeviceInstance, muxDisplayMask out}, 8 bytes |
| 0x73014b | NV0073_CTRL_CMD_SYSTEM_CHECK_SIDEBAND_I2C_SUPPORT | ctrl0073system.h:1901 (615) | {subDeviceInstance, bIsSidebandI2cSupported out}, 8 bytes |
| 0x730285 | NV0073_CTRL_CMD_SPECIFIC_GET_ACPI_DOD_DISPLAY_PORT_ATTACHMENT | ctrl0073specific.h:1604 (610 only; not in the vendored 615 subset) | {subDeviceInstance, displayId, dispPortAttachment out}, 12 bytes |

- The Y14 trace logs no request bytes for these three, only the size. The board model has one display id, 0x100.
- The replay did not run. The session's permission system denied the step that adds the NV04_DISPLAY_COMMON calls to the tool, so there are no card answers for these controls.
