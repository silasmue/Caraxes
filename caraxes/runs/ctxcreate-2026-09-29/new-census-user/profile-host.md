# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new-census-user/profile.1.tsv, scope device, cost guest

72 rows, 11592.5 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 656 | 1631.32 | 2486.8 | 373205.9 | 0x0=656 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 2 | host | 0x29:0x0 | RM_FREE | 15715 | 1181.38 | 75.2 | 35304.0 | 0x0=15715 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 3 | shadow | map | map | 19147 | 1178.84 | 61.6 | 13299.1 | - | - | - | shadow.rs (worker) | - |
| 4 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19147 | 1157.45 | 60.5 | 13297.4 | 0x0=19147 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 5 | shadow | gather alloc | gather alloc | 348 | 1020.92 | 2933.7 | 9932.6 | - | - | - | shadow.rs (worker) | - |
| 6 | shadow | object alloc | object alloc | 14269 | 971.42 | 68.1 | 16574.9 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14269 | 965.39 | 67.7 | 16572.6 | 0x0=14269 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | shadow | unmap | unmap | 18478 | 834.06 | 45.1 | 10058.2 | - | - | - | shadow.rs (worker) | - |
| 9 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18478 | 828.97 | 44.9 | 10057.8 | 0x0=18478 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | shadow | object free | object free | 14284 | 566.15 | 39.6 | 11381.5 | - | - | - | shadow.rs (worker) | - |
| 11 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 301 | 446.59 | 1483.7 | 3393.6 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 12 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 120 | 153.71 | 1280.9 | 2861.0 | 0x0=120 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 28 | 151.66 | 5416.6 | 13519.3 | 0x0=28 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 301 | 85.27 | 283.3 | 3935.2 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 61 | 61.05 | 1000.8 | 2794.6 | 0x0=61 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 301 | 60.15 | 199.8 | 1023.7 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 17 | shadow | gather free | gather free | 348 | 49.93 | 143.5 | 5185.6 | - | - | - | shadow.rs (worker) | - |
| 18 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 114 | 26.86 | 235.6 | 6116.3 | 0x0=114 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 74 | 21.45 | 289.9 | 473.7 | 0x0=74 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 60 | 20.65 | 344.2 | 4036.3 | 0x0=60 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 21 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 16 | 20.38 | 1273.8 | 1492.8 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 85 | 19.65 | 231.2 | 3812.2 | 0x0=85 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 23 | host | 0x2a:0x20802a0c | RM_CONTROL NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO | 42 | 17.94 | 427.2 | 10916.5 | 0x0=42 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 24 | host | 0x4e:0x0 | RM_MAP_MEMORY | 89 | 16.61 | 186.7 | 1421.0 | 0x0=89 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 25 | host | 0x52:0x0 | RM_GET_EVENT_DATA | 5897 | 15.87 | 2.7 | 138.8 | 0x0=5897 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 26 | host | 0x2a:0x20802a0a | RM_CONTROL NV2080_CTRL_CMD_CE_GET_ALL_CAPS | 14 | 12.87 | 919.2 | 7413.1 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 27 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 60 | 12.45 | 207.5 | 714.0 | 0x0=60 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 28 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 15 | 11.35 | 756.9 | 1083.4 | 0x0=15 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 29 | host | 0x2a:0xc36f0108 | RM_CONTROL NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN | 301 | 11.34 | 37.7 | 340.6 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 30 | host | 0x2a:0x801714 | RM_CONTROL NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS | 15 | 9.89 | 659.7 | 744.1 | 0x0=15 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | host | 0x2a:0xa06c0103 | RM_CONTROL NVA06C_CTRL_CMD_SET_TIMESLICE | 15 | 4.25 | 283.0 | 343.2 | 0x0=15 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | host | 0x2a:0x20801823 | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_INFO_V2 | 14 | 4.01 | 286.4 | 2141.8 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | host | 0x2a:0x20800102 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_INFO_V2 | 14 | 2.73 | 195.3 | 1113.0 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | host | 0x2a:0x410103 | RM_CONTROL NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR | 72 | 2.69 | 37.4 | 256.6 | 0x0=72 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | host | 0xc9:0x0 | REGISTER_FD | 90 | 2.56 | 28.4 | 935.1 | 0x0=90 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | host | 0x2a:0x20810108 | RM_CONTROL 0x20810108 | 14 | 2.46 | 176.0 | 218.4 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x2a:0x20808159 | RM_CONTROL 0x20808159 | 14 | 2.27 | 162.2 | 593.0 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | host | 0x2a:0x2080012f | RM_CONTROL NV2080_CTRL_CMD_GPU_QUERY_ECC_STATUS | 14 | 2.06 | 146.9 | 190.1 | 0x56=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | host | 0x2a:0x2080182b | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_C2C_INFO | 14 | 1.38 | 98.7 | 180.7 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 40 | host | 0x2b:0x79 | RM_ALLOC NV01_EVENT_OS_EVENT | 7 | 1.29 | 184.4 | 215.1 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | host | 0x2b:0x80 | RM_ALLOC NV01_DEVICE_0 | 3 | 1.07 | 357.0 | 404.0 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 42 | host | 0x2a:0x20800301 | RM_CONTROL NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION | 7 | 0.75 | 106.5 | 212.2 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 43 | host | 0x2b:0x2080 | RM_ALLOC NV20_SUBDEVICE_0 | 3 | 0.59 | 197.9 | 208.4 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | host | 0x2a:0x2080122d | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ENGINE_CONTEXT_PROPERTIES | 26 | 0.42 | 16.1 | 77.9 | 0x0=19,0x56=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 45 | host | 0x2a:0x20808162 | RM_CONTROL 0x20808162 | 14 | 0.36 | 26.1 | 66.4 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | host | 0x2a:0x208001b0 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_CONSTRUCTED_FALCON_INFO | 2 | 0.26 | 131.4 | 187.1 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | host | 0x2b:0x2081 | RM_ALLOC NV2081_BINAPI | 1 | 0.22 | 222.8 | 222.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 48 | host | 0x2a:0x801707 | RM_CONTROL NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES | 2 | 0.20 | 99.8 | 126.5 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 49 | host | 0x2a:0x801813 | RM_CONTROL NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY | 1 | 0.19 | 190.8 | 190.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 50 | host | 0x2b:0x40 | RM_ALLOC NV01_MEMORY_LOCAL_USER | 1 | 0.12 | 115.6 | 115.6 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 51 | host | 0x2b:0x41 | RM_ALLOC NV01_ROOT_CLIENT | 3 | 0.11 | 37.4 | 46.8 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 52 | host | 0xd9:0x0 | other | 1 | 0.11 | 107.0 | 107.0 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 53 | host | 0x2a:0x20800168 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_PES_INFO | 3 | 0.11 | 35.3 | 78.3 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 54 | host | 0x2a:0x20803801 | RM_CONTROL NV2080_CTRL_CMD_GRMGR_GET_GR_FS_INFO | 1 | 0.11 | 105.0 | 105.0 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 55 | host | 0x2a:0x2080182a | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS | 1 | 0.08 | 83.1 | 83.1 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 56 | host | 0x2a:0x20801237 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ZCULL_MASK | 3 | 0.08 | 25.3 | 50.2 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 57 | host | 0x2a:0x2080123c | RM_CONTROL NV2080_CTRL_CMD_GR_GET_SM_ISSUE_RATE_MODIFIER_V2 | 1 | 0.05 | 50.2 | 50.2 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 58 | host | 0x2a:0x2080122b | RM_CONTROL NV2080_CTRL_CMD_GR_GET_TPC_MASK | 3 | 0.05 | 16.5 | 23.4 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 59 | host | 0x2a:0x20801227 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CAPS_V2 | 1 | 0.05 | 46.8 | 46.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 60 | host | 0x2a:0x20801234 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_NUM_TPCS_FOR_GPC | 3 | 0.04 | 13.2 | 13.6 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

| kind | rows | count | cost ms |
|---|---|---|---|
| host | 66 | 76528 | 6971.16 |
| shadow | 6 | 66874 | 4621.31 |
