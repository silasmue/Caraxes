# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-bench5/profile.1.tsv, scope device, cost guest

72 rows, 19150.8 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | host | 0x29:0x0 | RM_FREE | 16622 | 2498.98 | 150.3 | 34661.5 | 0x0=16622 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 2 | shadow | unmap | unmap | 18750 | 1893.58 | 101.0 | 11467.3 | - | - | - | shadow.rs (worker) | - |
| 3 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18750 | 1888.40 | 100.7 | 11467.0 | 0x0=18750 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 4 | shadow | map | map | 19670 | 1845.89 | 93.8 | 10397.7 | - | - | - | shadow.rs (worker) | - |
| 5 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19670 | 1833.09 | 93.2 | 9896.5 | 0x0=19670 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | shadow | object free | object free | 14719 | 1731.68 | 117.6 | 16551.0 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 868 | 1702.24 | 1961.1 | 392846.3 | 0x0=868 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | shadow | object alloc | object alloc | 14699 | 1534.53 | 104.4 | 9152.8 | - | - | - | shadow.rs (worker) | - |
| 9 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14699 | 1522.65 | 103.6 | 9151.5 | 0x0=14699 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | shadow | gather alloc | gather alloc | 460 | 1250.00 | 2717.4 | 11108.8 | - | - | - | shadow.rs (worker) | - |
| 11 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 401 | 493.50 | 1230.7 | 4681.0 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 12 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 160 | 229.94 | 1437.1 | 4352.1 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 29 | 120.47 | 4154.1 | 9060.0 | 0x0=29 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 401 | 92.19 | 229.9 | 2793.5 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 401 | 66.82 | 166.6 | 3519.9 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 81 | 65.28 | 806.0 | 2760.1 | 0x0=81 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 17 | shadow | gather free | gather free | 460 | 46.27 | 100.6 | 4124.6 | - | - | - | shadow.rs (worker) | - |
| 18 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 112 | 34.38 | 307.0 | 12543.5 | 0x0=112 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | host | 0x4e:0x0 | RM_MAP_MEMORY | 116 | 33.34 | 287.4 | 16599.5 | 0x0=116 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 21 | 32.42 | 1543.8 | 8257.3 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 21 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 128 | 31.41 | 245.4 | 8241.4 | 0x0=128 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 88 | 26.99 | 306.8 | 1126.1 | 0x0=88 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 23 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 80 | 19.21 | 240.1 | 2995.9 | 0x0=80 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 24 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 80 | 18.27 | 228.4 | 3676.8 | 0x0=80 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 25 | host | 0x2a:0xc36f0108 | RM_CONTROL NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN | 401 | 13.73 | 34.2 | 545.5 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 26 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 20 | 13.34 | 666.8 | 739.4 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 27 | host | 0x2a:0x801714 | RM_CONTROL NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS | 20 | 12.53 | 626.5 | 752.8 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 28 | host | 0x52:0x0 | RM_GET_EVENT_DATA | 5086 | 12.32 | 2.4 | 140.4 | 0x0=5086 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 29 | host | 0x2a:0x20802a0c | RM_CONTROL NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO | 48 | 10.85 | 226.0 | 4418.9 | 0x0=48 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 30 | host | 0x2a:0x20802a0a | RM_CONTROL NV2080_CTRL_CMD_CE_GET_ALL_CAPS | 16 | 10.59 | 661.6 | 4125.8 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | host | 0x2a:0x2080182b | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_C2C_INFO | 16 | 8.63 | 539.1 | 7272.4 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | host | 0x2a:0x2080012f | RM_CONTROL NV2080_CTRL_CMD_GPU_QUERY_ECC_STATUS | 16 | 7.06 | 441.1 | 4338.5 | 0x56=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | host | 0x2a:0x20810108 | RM_CONTROL 0x20810108 | 16 | 6.33 | 395.4 | 3053.6 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | host | 0x2a:0x20800102 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_INFO_V2 | 16 | 5.29 | 330.9 | 3529.7 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | host | 0x2a:0xa06c0103 | RM_CONTROL NVA06C_CTRL_CMD_SET_TIMESLICE | 20 | 5.19 | 259.7 | 331.5 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | host | 0x2a:0x410103 | RM_CONTROL NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR | 72 | 5.13 | 71.3 | 4045.8 | 0x0=72 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x2a:0x20808162 | RM_CONTROL 0x20808162 | 16 | 4.66 | 291.5 | 4319.6 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | host | 0xc9:0x0 | REGISTER_FD | 117 | 3.98 | 34.0 | 2296.4 | 0x0=117 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | host | 0x2a:0x20801234 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_NUM_TPCS_FOR_GPC | 3 | 3.41 | 1137.9 | 3284.2 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 40 | host | 0x2b:0x40 | RM_ALLOC NV01_MEMORY_LOCAL_USER | 1 | 3.21 | 3206.1 | 3206.1 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | host | 0x2a:0x20808159 | RM_CONTROL 0x20808159 | 16 | 2.23 | 139.1 | 640.9 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 42 | host | 0x2a:0x20801823 | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_INFO_V2 | 16 | 2.22 | 138.9 | 239.7 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 43 | host | 0x2a:0x2080122d | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ENGINE_CONTEXT_PROPERTIES | 26 | 1.36 | 52.1 | 243.9 | 0x0=19,0x56=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | host | 0x2b:0x79 | RM_ALLOC NV01_EVENT_OS_EVENT | 7 | 1.16 | 165.1 | 193.0 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 45 | host | 0x2b:0x80 | RM_ALLOC NV01_DEVICE_0 | 3 | 0.96 | 320.6 | 409.8 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | host | 0x2a:0x2080121b | RM_CONTROL NV2080_CTRL_CMD_GR_GET_GLOBAL_SM_ORDER | 1 | 0.82 | 815.8 | 815.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | host | 0x2a:0x20800301 | RM_CONTROL NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION | 7 | 0.66 | 94.3 | 170.1 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 48 | host | 0x2a:0x20801230 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_SM_ISSUE_RATE_MODIFIER | 1 | 0.45 | 447.9 | 447.9 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 49 | host | 0x2b:0x2080 | RM_ALLOC NV20_SUBDEVICE_0 | 3 | 0.44 | 146.5 | 173.1 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 50 | host | 0x2a:0x2080123c | RM_CONTROL NV2080_CTRL_CMD_GR_GET_SM_ISSUE_RATE_MODIFIER_V2 | 1 | 0.43 | 425.8 | 425.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 51 | host | 0x2a:0x2080122b | RM_CONTROL NV2080_CTRL_CMD_GR_GET_TPC_MASK | 3 | 0.39 | 130.6 | 353.9 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 52 | host | 0x2a:0x208001b0 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_CONSTRUCTED_FALCON_INFO | 2 | 0.26 | 128.6 | 182.1 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 53 | host | 0x2b:0x2081 | RM_ALLOC NV2081_BINAPI | 1 | 0.25 | 252.2 | 252.2 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 54 | host | 0x2a:0x801707 | RM_CONTROL NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES | 2 | 0.19 | 93.9 | 106.9 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 55 | host | 0x2a:0x20801237 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ZCULL_MASK | 3 | 0.18 | 59.1 | 152.3 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 56 | host | 0x2a:0x801813 | RM_CONTROL NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY | 1 | 0.14 | 137.4 | 137.4 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 57 | host | 0x2a:0x2080122a | RM_CONTROL NV2080_CTRL_CMD_GR_GET_GPC_MASK | 1 | 0.13 | 125.8 | 125.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 58 | host | 0x2a:0x20801206 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ZCULL_INFO | 1 | 0.11 | 111.4 | 111.4 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 59 | host | 0x2a:0x20803801 | RM_CONTROL NV2080_CTRL_CMD_GRMGR_GET_GR_FS_INFO | 1 | 0.10 | 100.0 | 100.0 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 60 | host | 0x2a:0x2080182a | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS | 1 | 0.08 | 80.7 | 80.7 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

| kind | rows | count | cost ms |
|---|---|---|---|
| host | 66 | 78713 | 10848.81 |
| shadow | 6 | 68758 | 8301.94 |
