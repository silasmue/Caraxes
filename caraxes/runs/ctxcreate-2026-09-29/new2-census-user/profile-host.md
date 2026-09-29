# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new2-census-user/profile.1.tsv, scope device, cost guest

72 rows, 7686.1 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 626 | 1266.26 | 2022.8 | 348212.6 | 0x0=626 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 2 | host | 0x29:0x0 | RM_FREE | 15689 | 923.12 | 58.8 | 31909.0 | 0x0=15689 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 3 | shadow | gather alloc | gather alloc | 319 | 888.62 | 2785.7 | 10830.1 | - | - | - | shadow.rs (worker) | - |
| 4 | shadow | map | map | 19197 | 558.03 | 29.1 | 3717.2 | - | - | - | shadow.rs (worker) | - |
| 5 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19197 | 552.04 | 28.8 | 3716.3 | 0x0=19197 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | shadow | object alloc | object alloc | 14273 | 533.95 | 37.4 | 5189.2 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14273 | 528.78 | 37.0 | 5188.4 | 0x0=14273 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | shadow | unmap | unmap | 18456 | 517.62 | 28.0 | 6577.2 | - | - | - | shadow.rs (worker) | - |
| 9 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18456 | 513.79 | 27.8 | 6576.6 | 0x0=18456 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | shadow | object free | object free | 14288 | 425.61 | 29.8 | 3477.6 | - | - | - | shadow.rs (worker) | - |
| 11 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 301 | 398.78 | 1324.8 | 4381.2 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 12 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 120 | 151.73 | 1264.4 | 3573.0 | 0x0=120 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 24 | 74.98 | 3124.0 | 8048.9 | 0x0=24 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 301 | 70.03 | 232.7 | 5005.1 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 61 | 49.70 | 814.8 | 2035.8 | 0x0=61 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 301 | 44.75 | 148.7 | 949.3 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 17 | shadow | gather free | gather free | 319 | 28.18 | 88.3 | 4112.7 | - | - | - | shadow.rs (worker) | - |
| 18 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 77 | 25.72 | 334.0 | 2364.4 | 0x0=77 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 60 | 20.50 | 341.6 | 3618.3 | 0x0=60 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 16 | 19.83 | 1239.6 | 1333.2 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 21 | host | 0x4e:0x0 | RM_MAP_MEMORY | 88 | 12.75 | 144.9 | 581.8 | 0x0=88 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 15 | 10.31 | 687.1 | 806.1 | 0x0=15 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 23 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 60 | 10.16 | 169.4 | 253.8 | 0x0=60 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 24 | host | 0x2a:0x801714 | RM_CONTROL NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS | 15 | 9.25 | 616.3 | 638.1 | 0x0=15 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 25 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 84 | 8.11 | 96.5 | 418.7 | 0x0=84 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 26 | host | 0x2a:0xc36f0108 | RM_CONTROL NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN | 301 | 6.62 | 22.0 | 349.3 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 27 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 100 | 6.26 | 62.6 | 629.4 | 0x0=100 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 28 | host | 0x2a:0x20802a0a | RM_CONTROL NV2080_CTRL_CMD_CE_GET_ALL_CAPS | 14 | 5.49 | 392.0 | 413.0 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 29 | host | 0x2a:0xa06c0103 | RM_CONTROL NVA06C_CTRL_CMD_SET_TIMESLICE | 15 | 3.90 | 260.3 | 279.1 | 0x0=15 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 30 | host | 0x2a:0x20802a0c | RM_CONTROL NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO | 42 | 3.46 | 82.3 | 151.0 | 0x0=42 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | host | 0x2a:0x20810108 | RM_CONTROL 0x20810108 | 14 | 2.33 | 166.6 | 204.3 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | host | 0x2a:0x2080012f | RM_CONTROL NV2080_CTRL_CMD_GPU_QUERY_ECC_STATUS | 14 | 1.84 | 131.3 | 161.6 | 0x56=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | host | 0x2a:0x20801823 | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_INFO_V2 | 14 | 1.83 | 130.8 | 151.3 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | host | 0x2a:0x20800102 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_INFO_V2 | 14 | 1.43 | 101.9 | 153.3 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | host | 0x2a:0x410103 | RM_CONTROL NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR | 63 | 1.38 | 21.9 | 170.2 | 0x0=63 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | host | 0x2a:0x20808159 | RM_CONTROL 0x20808159 | 14 | 1.37 | 97.7 | 142.5 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x2a:0x2080182b | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_C2C_INFO | 14 | 1.11 | 79.2 | 90.2 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | host | 0x2b:0x79 | RM_ALLOC NV01_EVENT_OS_EVENT | 7 | 1.07 | 152.2 | 195.1 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | host | 0x2b:0x80 | RM_ALLOC NV01_DEVICE_0 | 3 | 0.93 | 310.9 | 378.7 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 40 | host | 0x52:0x0 | RM_GET_EVENT_DATA | 230 | 0.71 | 3.1 | 64.1 | 0x0=230 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | host | 0x2b:0x2080 | RM_ALLOC NV20_SUBDEVICE_0 | 3 | 0.57 | 191.1 | 205.4 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 42 | host | 0x2a:0x20800301 | RM_CONTROL NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION | 7 | 0.54 | 76.9 | 105.6 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 43 | host | 0xc9:0x0 | REGISTER_FD | 89 | 0.41 | 4.6 | 154.7 | 0x0=89 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | host | 0x2a:0x2080122d | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ENGINE_CONTEXT_PROPERTIES | 26 | 0.31 | 11.7 | 17.4 | 0x0=19,0x56=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 45 | host | 0x2a:0x801707 | RM_CONTROL NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES | 2 | 0.30 | 150.2 | 162.0 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | host | 0x2a:0x208001b0 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_CONSTRUCTED_FALCON_INFO | 2 | 0.24 | 122.1 | 161.1 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | host | 0x2a:0x20808162 | RM_CONTROL 0x20808162 | 14 | 0.24 | 17.2 | 41.1 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 48 | host | 0x2a:0x801813 | RM_CONTROL NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY | 1 | 0.18 | 176.9 | 176.9 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 49 | host | 0x2b:0x2081 | RM_ALLOC NV2081_BINAPI | 1 | 0.17 | 173.1 | 173.1 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 50 | host | 0x2a:0x20803801 | RM_CONTROL NV2080_CTRL_CMD_GRMGR_GET_GR_FS_INFO | 1 | 0.10 | 96.1 | 96.1 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 51 | host | 0xd9:0x0 | other | 1 | 0.10 | 95.1 | 95.1 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 52 | host | 0x2b:0x41 | RM_ALLOC NV01_ROOT_CLIENT | 3 | 0.08 | 26.8 | 34.3 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 53 | host | 0x2a:0x2080182a | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS | 1 | 0.08 | 79.5 | 79.5 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 54 | host | 0x2b:0x40 | RM_ALLOC NV01_MEMORY_LOCAL_USER | 1 | 0.07 | 66.0 | 66.0 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 55 | host | 0x2a:0x20801227 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CAPS_V2 | 1 | 0.04 | 41.5 | 41.5 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 56 | host | 0x2a:0x20800168 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_PES_INFO | 3 | 0.04 | 12.0 | 12.9 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 57 | host | 0x2a:0x20801234 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_NUM_TPCS_FOR_GPC | 3 | 0.04 | 11.8 | 12.4 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 58 | host | 0x2a:0x2080122b | RM_CONTROL NV2080_CTRL_CMD_GR_GET_TPC_MASK | 3 | 0.03 | 11.3 | 11.6 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 59 | host | 0x2a:0x20801237 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ZCULL_MASK | 3 | 0.03 | 11.3 | 11.5 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 60 | host | 0x2b:0xc361 | RM_ALLOC VOLTA_USERMODE_A | 2 | 0.03 | 15.8 | 16.2 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

| kind | rows | count | cost ms |
|---|---|---|---|
| host | 66 | 70810 | 4734.08 |
| shadow | 6 | 66852 | 2952.00 |
