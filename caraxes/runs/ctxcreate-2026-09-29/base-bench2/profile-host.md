# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-bench2/profile.1.tsv, scope device, cost guest

72 rows, 10685.8 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 876 | 2583.95 | 2949.7 | 263227.6 | 0x0=876 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 2 | host | 0x29:0x0 | RM_FREE | 16630 | 1453.69 | 87.4 | 34300.7 | 0x0=16630 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 3 | shadow | gather alloc | gather alloc | 460 | 1281.12 | 2785.0 | 12889.2 | - | - | - | shadow.rs (worker) | - |
| 4 | shadow | map | map | 19654 | 608.65 | 31.0 | 8850.3 | - | - | - | shadow.rs (worker) | - |
| 5 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19654 | 598.84 | 30.5 | 8849.3 | 0x0=19654 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | shadow | object alloc | object alloc | 14699 | 564.74 | 38.4 | 6916.5 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14699 | 558.70 | 38.0 | 6915.8 | 0x0=14699 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | shadow | unmap | unmap | 18767 | 555.50 | 29.6 | 4691.1 | - | - | - | shadow.rs (worker) | - |
| 9 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18767 | 550.83 | 29.4 | 4690.7 | 0x0=18767 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 401 | 523.36 | 1305.1 | 4104.3 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 11 | shadow | object free | object free | 14719 | 480.98 | 32.7 | 9153.4 | - | - | - | shadow.rs (worker) | - |
| 12 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 160 | 230.29 | 1439.3 | 4634.3 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 25 | 151.74 | 6069.5 | 42136.4 | 0x0=25 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 401 | 88.85 | 221.6 | 1142.7 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 81 | 74.95 | 925.3 | 1957.1 | 0x0=81 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 401 | 68.13 | 169.9 | 3118.7 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 17 | shadow | gather free | gather free | 460 | 53.80 | 117.0 | 2981.3 | - | - | - | shadow.rs (worker) | - |
| 18 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 85 | 30.27 | 356.1 | 3288.5 | 0x0=85 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 112 | 27.24 | 243.2 | 14276.8 | 0x0=112 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 21 | 26.70 | 1271.6 | 1374.0 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 21 | host | 0x2a:0x410103 | RM_CONTROL NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR | 144 | 23.75 | 164.9 | 7742.3 | 0x0=144 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | host | 0x2a:0xc36f0108 | RM_CONTROL NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN | 401 | 19.38 | 48.3 | 3423.5 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 23 | host | 0x4e:0x0 | RM_MAP_MEMORY | 116 | 17.63 | 152.0 | 431.0 | 0x0=116 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 24 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 80 | 15.00 | 187.4 | 489.3 | 0x0=80 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 25 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 20 | 13.87 | 693.3 | 735.7 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 26 | host | 0x2a:0x801714 | RM_CONTROL NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS | 20 | 13.17 | 658.6 | 759.1 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 27 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 80 | 12.85 | 160.6 | 1119.9 | 0x0=80 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 28 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 117 | 8.25 | 70.5 | 431.3 | 0x0=117 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 29 | host | 0x2a:0x20802a0a | RM_CONTROL NV2080_CTRL_CMD_CE_GET_ALL_CAPS | 16 | 7.42 | 463.9 | 1102.0 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 30 | host | 0x2a:0xa06c0103 | RM_CONTROL NVA06C_CTRL_CMD_SET_TIMESLICE | 20 | 5.15 | 257.5 | 304.0 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | host | 0x2a:0x20802a0c | RM_CONTROL NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO | 48 | 4.61 | 96.1 | 262.4 | 0x0=48 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | host | 0xc9:0x0 | REGISTER_FD | 117 | 4.34 | 37.1 | 3464.5 | 0x0=117 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | host | 0x2b:0x80 | RM_ALLOC NV01_DEVICE_0 | 3 | 3.51 | 1170.4 | 1762.6 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | host | 0x52:0x0 | RM_GET_EVENT_DATA | 873 | 3.25 | 3.7 | 403.6 | 0x0=873 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | host | 0x2a:0x20810108 | RM_CONTROL 0x20810108 | 16 | 3.08 | 192.5 | 450.7 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | host | 0x2a:0x20801823 | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_INFO_V2 | 16 | 2.52 | 157.6 | 411.4 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x2a:0x2080012f | RM_CONTROL NV2080_CTRL_CMD_GPU_QUERY_ECC_STATUS | 16 | 2.46 | 153.6 | 391.9 | 0x56=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | host | 0x2a:0x20800102 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_INFO_V2 | 16 | 2.21 | 138.0 | 325.3 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | host | 0x2a:0x20808159 | RM_CONTROL 0x20808159 | 16 | 1.96 | 122.4 | 236.1 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 40 | host | 0x2b:0x2081 | RM_ALLOC NV2081_BINAPI | 1 | 1.87 | 1865.7 | 1865.7 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | host | 0x2a:0x2080182b | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_C2C_INFO | 16 | 1.51 | 94.5 | 213.1 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 42 | host | 0x2b:0x2080 | RM_ALLOC NV20_SUBDEVICE_0 | 3 | 1.20 | 401.3 | 491.5 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 43 | host | 0x2b:0x79 | RM_ALLOC NV01_EVENT_OS_EVENT | 7 | 1.19 | 170.1 | 233.1 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | host | 0x2a:0x20800301 | RM_CONTROL NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION | 7 | 0.61 | 87.5 | 107.5 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 45 | host | 0x2a:0x208001b0 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_CONSTRUCTED_FALCON_INFO | 2 | 0.53 | 265.3 | 368.2 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | host | 0x2a:0x2080122d | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ENGINE_CONTEXT_PROPERTIES | 26 | 0.38 | 14.8 | 24.5 | 0x0=19,0x56=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | host | 0x2a:0x20808162 | RM_CONTROL 0x20808162 | 16 | 0.28 | 17.3 | 34.2 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 48 | host | 0x2a:0x20803801 | RM_CONTROL NV2080_CTRL_CMD_GRMGR_GET_GR_FS_INFO | 1 | 0.26 | 260.9 | 260.9 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 49 | host | 0x2a:0x801707 | RM_CONTROL NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES | 2 | 0.21 | 105.2 | 118.5 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 50 | host | 0x2a:0x2080182a | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS | 1 | 0.19 | 189.0 | 189.0 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 51 | host | 0x2a:0x801813 | RM_CONTROL NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY | 1 | 0.16 | 159.6 | 159.6 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 52 | host | 0x2b:0x41 | RM_ALLOC NV01_ROOT_CLIENT | 3 | 0.08 | 26.3 | 35.7 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 53 | host | 0x2b:0x40 | RM_ALLOC NV01_MEMORY_LOCAL_USER | 1 | 0.07 | 71.8 | 71.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 54 | host | 0xd9:0x0 | other | 1 | 0.05 | 52.8 | 52.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 55 | host | 0x2a:0x20801227 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CAPS_V2 | 1 | 0.05 | 48.8 | 48.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 56 | host | 0x2a:0x20800168 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_PES_INFO | 3 | 0.05 | 15.2 | 15.7 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 57 | host | 0x2a:0x20801237 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ZCULL_MASK | 3 | 0.04 | 14.4 | 15.1 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 58 | host | 0x2a:0x20801234 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_NUM_TPCS_FOR_GPC | 3 | 0.04 | 14.3 | 15.3 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 59 | host | 0x2a:0x2080122b | RM_CONTROL NV2080_CTRL_CMD_GR_GET_TPC_MASK | 3 | 0.04 | 13.3 | 13.8 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 60 | host | 0x2b:0xc361 | RM_ALLOC VOLTA_USERMODE_A | 2 | 0.03 | 16.4 | 17.1 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

| kind | rows | count | cost ms |
|---|---|---|---|
| host | 66 | 74571 | 7141.04 |
| shadow | 6 | 68759 | 3544.79 |
