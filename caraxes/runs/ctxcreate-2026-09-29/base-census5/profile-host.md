# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-census5/profile.1.tsv, scope device, cost guest

72 rows, 7905.5 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 576 | 1657.68 | 2877.9 | 429580.4 | 0x0=576 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 2 | shadow | gather alloc | gather alloc | 341 | 905.60 | 2655.7 | 8960.2 | - | - | - | shadow.rs (worker) | - |
| 3 | host | 0x29:0x0 | RM_FREE | 15473 | 839.05 | 54.2 | 26528.7 | 0x0=15473 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 4 | shadow | map | map | 19141 | 579.27 | 30.3 | 3936.3 | - | - | - | shadow.rs (worker) | - |
| 5 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19141 | 573.02 | 29.9 | 3935.6 | 0x0=19141 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | shadow | object alloc | object alloc | 14269 | 529.94 | 37.1 | 4939.8 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14269 | 521.73 | 36.6 | 4938.7 | 0x0=14269 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | shadow | unmap | unmap | 18256 | 472.42 | 25.9 | 8043.4 | - | - | - | shadow.rs (worker) | - |
| 9 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18256 | 468.51 | 25.7 | 8042.7 | 0x0=18256 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | shadow | object free | object free | 14284 | 433.44 | 30.3 | 8498.7 | - | - | - | shadow.rs (worker) | - |
| 11 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 229 | 316.44 | 1381.8 | 2829.3 | 0x0=229 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 12 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 120 | 142.05 | 1183.7 | 2167.8 | 0x0=120 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 25 | 117.11 | 4684.5 | 42437.4 | 0x0=25 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 229 | 47.68 | 208.2 | 1358.8 | 0x0=229 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 229 | 39.44 | 172.2 | 3673.2 | 0x0=229 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 133 | 35.47 | 266.7 | 22548.0 | 0x0=133 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 17 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 43 | 34.98 | 813.5 | 2001.7 | 0x0=43 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 18 | shadow | gather free | gather free | 341 | 28.98 | 85.0 | 202.2 | - | - | - | shadow.rs (worker) | - |
| 19 | host | 0x4e:0x0 | RM_MAP_MEMORY | 137 | 28.35 | 206.9 | 9585.8 | 0x0=137 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 16 | 19.30 | 1206.2 | 1310.5 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 21 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 60 | 16.28 | 271.3 | 2494.6 | 0x0=60 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 36 | 13.83 | 384.0 | 4545.8 | 0x0=36 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 23 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 60 | 10.90 | 181.6 | 699.8 | 0x0=60 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 24 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 15 | 10.63 | 708.5 | 812.4 | 0x0=15 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 25 | host | 0x2a:0x801714 | RM_CONTROL NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS | 15 | 9.31 | 620.8 | 650.8 | 0x0=15 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 26 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 101 | 7.32 | 72.4 | 812.5 | 0x0=101 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 27 | host | 0x2a:0x20802a0a | RM_CONTROL NV2080_CTRL_CMD_CE_GET_ALL_CAPS | 14 | 6.36 | 454.3 | 1068.7 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 28 | host | 0x2a:0x20810108 | RM_CONTROL 0x20810108 | 14 | 6.32 | 451.4 | 2708.2 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 29 | host | 0x2a:0xc36f0108 | RM_CONTROL NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN | 229 | 5.75 | 25.1 | 513.6 | 0x0=229 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 30 | host | 0x2a:0x20802a0c | RM_CONTROL NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO | 42 | 3.98 | 94.7 | 237.7 | 0x0=42 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | host | 0x2a:0xa06c0103 | RM_CONTROL NVA06C_CTRL_CMD_SET_TIMESLICE | 15 | 3.81 | 253.7 | 431.9 | 0x0=15 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | host | 0x2a:0x2080012f | RM_CONTROL NV2080_CTRL_CMD_GPU_QUERY_ECC_STATUS | 14 | 3.01 | 215.1 | 627.6 | 0x56=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | host | 0x2a:0x20801823 | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_INFO_V2 | 14 | 2.16 | 154.2 | 418.6 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | host | 0x2a:0x410103 | RM_CONTROL NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR | 63 | 2.12 | 33.6 | 135.0 | 0x0=63 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | host | 0x2a:0x20800102 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_INFO_V2 | 14 | 1.98 | 141.5 | 548.2 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | host | 0x2a:0x20808159 | RM_CONTROL 0x20808159 | 14 | 1.58 | 112.5 | 232.5 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x2a:0x2080182b | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_C2C_INFO | 14 | 1.24 | 88.9 | 153.1 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | host | 0x2b:0x79 | RM_ALLOC NV01_EVENT_OS_EVENT | 7 | 1.22 | 174.4 | 273.7 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | host | 0x2b:0x80 | RM_ALLOC NV01_DEVICE_0 | 3 | 1.03 | 343.5 | 405.5 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 40 | host | 0x52:0x0 | RM_GET_EVENT_DATA | 327 | 0.96 | 2.9 | 117.1 | 0x0=327 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | host | 0x2b:0x2081 | RM_ALLOC NV2081_BINAPI | 1 | 0.92 | 915.4 | 915.4 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 42 | host | 0x2a:0x208001b0 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_CONSTRUCTED_FALCON_INFO | 2 | 0.70 | 349.7 | 522.9 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 43 | host | 0xc9:0x0 | REGISTER_FD | 138 | 0.61 | 4.4 | 130.0 | 0x0=138 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | host | 0x2a:0x20800301 | RM_CONTROL NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION | 7 | 0.59 | 83.8 | 107.8 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 45 | host | 0x2b:0x2080 | RM_ALLOC NV20_SUBDEVICE_0 | 3 | 0.56 | 185.3 | 210.0 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | host | 0x2a:0x2080122d | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ENGINE_CONTEXT_PROPERTIES | 26 | 0.31 | 12.0 | 15.9 | 0x0=19,0x56=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | host | 0x2a:0x20803801 | RM_CONTROL NV2080_CTRL_CMD_GRMGR_GET_GR_FS_INFO | 1 | 0.26 | 260.4 | 260.4 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 48 | host | 0x2a:0x20808162 | RM_CONTROL 0x20808162 | 14 | 0.25 | 18.2 | 61.2 | 0x0=14 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 49 | host | 0x2a:0x801707 | RM_CONTROL NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES | 2 | 0.19 | 94.3 | 113.5 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 50 | host | 0x2a:0x2080182a | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS | 1 | 0.17 | 168.2 | 168.2 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 51 | host | 0x2a:0x801813 | RM_CONTROL NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY | 1 | 0.15 | 148.7 | 148.7 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 52 | host | 0x2b:0x41 | RM_ALLOC NV01_ROOT_CLIENT | 3 | 0.07 | 24.9 | 29.7 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 53 | host | 0x2b:0x40 | RM_ALLOC NV01_MEMORY_LOCAL_USER | 1 | 0.07 | 73.6 | 73.6 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 54 | host | 0xd9:0x0 | other | 1 | 0.07 | 65.1 | 65.1 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 55 | host | 0x2a:0x20800168 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_PES_INFO | 3 | 0.04 | 12.1 | 12.9 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 56 | host | 0x2a:0x20801234 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_NUM_TPCS_FOR_GPC | 3 | 0.04 | 11.8 | 12.3 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 57 | host | 0x2a:0x2080122b | RM_CONTROL NV2080_CTRL_CMD_GR_GET_TPC_MASK | 3 | 0.03 | 11.5 | 11.7 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 58 | host | 0x2a:0x20801237 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ZCULL_MASK | 3 | 0.03 | 11.5 | 11.6 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 59 | host | 0x2b:0xc361 | RM_ALLOC VOLTA_USERMODE_A | 2 | 0.03 | 15.0 | 15.5 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 60 | host | 0x2a:0x20801227 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CAPS_V2 | 1 | 0.03 | 25.5 | 25.5 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

| kind | rows | count | cost ms |
|---|---|---|---|
| host | 66 | 70183 | 4955.85 |
| shadow | 6 | 66632 | 2949.65 |
