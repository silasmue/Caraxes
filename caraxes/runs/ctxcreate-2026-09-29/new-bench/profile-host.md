# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new-bench/profile.1.tsv, scope device, cost guest

72 rows, 11615.3 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 872 | 2446.11 | 2805.2 | 414717.8 | 0x0=872 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 2 | shadow | gather alloc | gather alloc | 464 | 1443.01 | 3109.9 | 16376.5 | - | - | - | shadow.rs (worker) | - |
| 3 | host | 0x29:0x0 | RM_FREE | 16628 | 1297.69 | 78.0 | 32676.1 | 0x0=16628 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 4 | shadow | object alloc | object alloc | 14701 | 786.05 | 53.5 | 14662.4 | - | - | - | shadow.rs (worker) | - |
| 5 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14701 | 780.74 | 53.1 | 14659.4 | 0x0=14701 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | shadow | map | map | 19615 | 752.57 | 38.4 | 10303.3 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19615 | 742.61 | 37.9 | 10302.6 | 0x0=19615 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | shadow | unmap | unmap | 18706 | 631.48 | 33.8 | 12691.2 | - | - | - | shadow.rs (worker) | - |
| 9 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18706 | 627.79 | 33.6 | 12690.3 | 0x0=18706 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | shadow | object free | object free | 14721 | 538.23 | 36.6 | 10048.4 | - | - | - | shadow.rs (worker) | - |
| 11 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 401 | 510.14 | 1272.2 | 10096.4 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 12 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 160 | 266.78 | 1667.3 | 3871.2 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 401 | 117.63 | 293.3 | 4436.6 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 24 | 109.71 | 4571.4 | 9542.7 | 0x0=24 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 401 | 93.45 | 233.0 | 4223.7 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 81 | 75.93 | 937.5 | 3316.5 | 0x0=81 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 17 | shadow | gather free | gather free | 464 | 59.61 | 128.5 | 3719.0 | - | - | - | shadow.rs (worker) | - |
| 18 | host | 0x4e:0x0 | RM_MAP_MEMORY | 113 | 41.49 | 367.1 | 10590.7 | 0x0=113 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 109 | 39.46 | 362.0 | 9893.0 | 0x0=109 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 87 | 33.89 | 389.6 | 3416.8 | 0x0=87 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 21 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 21 | 33.89 | 1613.9 | 4637.9 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 80 | 29.40 | 367.5 | 7206.4 | 0x0=80 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 23 | host | 0xc9:0x0 | REGISTER_FD | 114 | 22.49 | 197.3 | 10231.8 | 0x0=114 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 24 | host | 0x2a:0xc36f0108 | RM_CONTROL NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN | 401 | 20.61 | 51.4 | 1962.4 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 25 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 120 | 18.49 | 154.1 | 3396.9 | 0x0=120 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 26 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 80 | 15.67 | 195.8 | 589.1 | 0x0=80 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 27 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 20 | 15.06 | 752.8 | 821.8 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 28 | host | 0x2a:0x801714 | RM_CONTROL NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS | 20 | 13.07 | 653.3 | 699.8 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 29 | host | 0x2a:0x20808162 | RM_CONTROL 0x20808162 | 16 | 9.64 | 602.7 | 9406.2 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 30 | host | 0x2a:0x20802a0a | RM_CONTROL NV2080_CTRL_CMD_CE_GET_ALL_CAPS | 16 | 6.45 | 403.0 | 437.6 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | host | 0x2a:0x20802a0c | RM_CONTROL NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO | 48 | 5.80 | 120.8 | 1748.9 | 0x0=48 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | host | 0x2a:0xa06c0103 | RM_CONTROL NVA06C_CTRL_CMD_SET_TIMESLICE | 20 | 5.47 | 273.7 | 332.2 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | host | 0x2a:0x20810108 | RM_CONTROL 0x20810108 | 16 | 3.61 | 225.5 | 965.7 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | host | 0x2a:0x20801823 | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_INFO_V2 | 16 | 2.81 | 175.6 | 663.2 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | host | 0x2a:0x410103 | RM_CONTROL NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR | 72 | 2.26 | 31.4 | 191.1 | 0x0=72 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | host | 0x2a:0x2080012f | RM_CONTROL NV2080_CTRL_CMD_GPU_QUERY_ECC_STATUS | 16 | 2.18 | 136.4 | 164.5 | 0x56=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x2b:0x41 | RM_ALLOC NV01_ROOT_CLIENT | 3 | 2.10 | 700.0 | 2044.1 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | host | 0x2a:0x20800102 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_INFO_V2 | 16 | 1.78 | 111.0 | 132.6 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | host | 0x2a:0x20808159 | RM_CONTROL 0x20808159 | 16 | 1.76 | 110.1 | 172.5 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 40 | host | 0x2a:0x2080182b | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_C2C_INFO | 16 | 1.63 | 101.6 | 143.5 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | host | 0x2b:0x79 | RM_ALLOC NV01_EVENT_OS_EVENT | 7 | 1.25 | 177.9 | 215.2 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 42 | host | 0x2b:0x80 | RM_ALLOC NV01_DEVICE_0 | 3 | 1.00 | 333.4 | 380.2 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 43 | host | 0x52:0x0 | RM_GET_EVENT_DATA | 299 | 0.80 | 2.7 | 10.4 | 0x0=299 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | host | 0x2a:0x20800301 | RM_CONTROL NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION | 7 | 0.65 | 92.5 | 131.1 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 45 | host | 0x2a:0x2080122d | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ENGINE_CONTEXT_PROPERTIES | 26 | 0.63 | 24.1 | 114.6 | 0x0=19,0x56=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | host | 0x2b:0x2080 | RM_ALLOC NV20_SUBDEVICE_0 | 3 | 0.54 | 181.5 | 212.5 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | host | 0x2a:0x208001b0 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_CONSTRUCTED_FALCON_INFO | 2 | 0.28 | 138.1 | 164.8 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 48 | host | 0x2b:0x2081 | RM_ALLOC NV2081_BINAPI | 1 | 0.27 | 269.2 | 269.2 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 49 | host | 0x2a:0x801813 | RM_CONTROL NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY | 1 | 0.24 | 242.5 | 242.5 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 50 | host | 0x2a:0x801707 | RM_CONTROL NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES | 2 | 0.19 | 95.0 | 110.5 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 51 | host | 0x2a:0x20803801 | RM_CONTROL NV2080_CTRL_CMD_GRMGR_GET_GR_FS_INFO | 1 | 0.13 | 130.1 | 130.1 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 52 | host | 0x2a:0x2080182a | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS | 1 | 0.08 | 83.7 | 83.7 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 53 | host | 0x2a:0x20801230 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_SM_ISSUE_RATE_MODIFIER | 1 | 0.08 | 78.1 | 78.1 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 54 | host | 0x2b:0x40 | RM_ALLOC NV01_MEMORY_LOCAL_USER | 1 | 0.08 | 75.0 | 75.0 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 55 | host | 0x2a:0x2080122a | RM_CONTROL NV2080_CTRL_CMD_GR_GET_GPC_MASK | 1 | 0.06 | 64.9 | 64.9 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 56 | host | 0x2a:0x2080122b | RM_CONTROL NV2080_CTRL_CMD_GR_GET_TPC_MASK | 3 | 0.06 | 19.5 | 30.0 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 57 | host | 0xd9:0x0 | other | 1 | 0.05 | 55.0 | 55.0 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 58 | host | 0x2a:0x20800168 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_PES_INFO | 3 | 0.05 | 15.7 | 16.3 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 59 | host | 0x2a:0x20801234 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_NUM_TPCS_FOR_GPC | 3 | 0.05 | 15.0 | 16.2 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 60 | host | 0x2a:0x20801237 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ZCULL_MASK | 3 | 0.04 | 15.0 | 15.4 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

| kind | rows | count | cost ms |
|---|---|---|---|
| host | 66 | 73816 | 7404.34 |
| shadow | 6 | 68671 | 4210.95 |
