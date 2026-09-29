# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new2-bench-b/profile.1.tsv, scope device, cost guest

72 rows, 8435.0 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 857 | 1319.36 | 1539.5 | 346725.7 | 0x0=857 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 2 | shadow | gather alloc | gather alloc | 450 | 1254.25 | 2787.2 | 10076.3 | - | - | - | shadow.rs (worker) | - |
| 3 | host | 0x29:0x0 | RM_FREE | 16613 | 1076.17 | 64.8 | 25340.4 | 0x0=16613 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 4 | shadow | map | map | 19673 | 572.68 | 29.1 | 9439.6 | - | - | - | shadow.rs (worker) | - |
| 5 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19673 | 566.26 | 28.8 | 9438.9 | 0x0=19673 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | shadow | object alloc | object alloc | 14701 | 509.01 | 34.6 | 5798.8 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14701 | 503.45 | 34.2 | 5797.7 | 0x0=14701 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | shadow | unmap | unmap | 18719 | 477.67 | 25.5 | 4335.8 | - | - | - | shadow.rs (worker) | - |
| 9 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18719 | 473.79 | 25.3 | 4335.2 | 0x0=18719 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 401 | 455.65 | 1136.3 | 3402.5 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 11 | shadow | object free | object free | 14721 | 406.24 | 27.6 | 3276.4 | - | - | - | shadow.rs (worker) | - |
| 12 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 160 | 269.71 | 1685.7 | 4382.2 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 401 | 98.99 | 246.9 | 4410.4 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 23 | 84.41 | 3669.8 | 9436.1 | 0x0=23 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 81 | 68.82 | 849.6 | 2200.6 | 0x0=81 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 401 | 60.78 | 151.6 | 2147.1 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 17 | shadow | gather free | gather free | 450 | 33.98 | 75.5 | 811.9 | - | - | - | shadow.rs (worker) | - |
| 18 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 21 | 27.15 | 1293.1 | 1687.1 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 81 | 22.58 | 278.8 | 515.9 | 0x0=81 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | host | 0x4e:0x0 | RM_MAP_MEMORY | 113 | 18.90 | 167.2 | 1264.9 | 0x0=113 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 21 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 20 | 15.88 | 794.1 | 2609.9 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 80 | 15.25 | 190.6 | 3756.0 | 0x0=80 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 23 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 109 | 15.10 | 138.5 | 2801.1 | 0x0=109 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 24 | host | 0x2a:0xc36f0108 | RM_CONTROL NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN | 401 | 14.03 | 35.0 | 1667.6 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 25 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 80 | 14.03 | 175.3 | 649.7 | 0x0=80 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 26 | host | 0x2a:0x801714 | RM_CONTROL NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS | 20 | 12.53 | 626.5 | 701.3 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 27 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 113 | 7.76 | 68.7 | 649.3 | 0x0=113 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 28 | host | 0x2a:0x20802a0a | RM_CONTROL NV2080_CTRL_CMD_CE_GET_ALL_CAPS | 16 | 6.35 | 397.0 | 473.2 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 29 | host | 0x2a:0xa06c0103 | RM_CONTROL NVA06C_CTRL_CMD_SET_TIMESLICE | 20 | 5.13 | 256.5 | 301.3 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 30 | host | 0x2a:0x20802a0c | RM_CONTROL NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO | 48 | 3.89 | 81.1 | 125.2 | 0x0=48 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | host | 0x2a:0x410103 | RM_CONTROL NV0041_CTRL_CMD_GET_SURFACE_PHYS_ATTR | 63 | 3.26 | 51.7 | 2036.3 | 0x0=63 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | host | 0x2a:0x20810108 | RM_CONTROL 0x20810108 | 16 | 2.90 | 181.4 | 417.1 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | host | 0x52:0x0 | RM_GET_EVENT_DATA | 778 | 2.61 | 3.4 | 155.7 | 0x0=778 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | host | 0x2a:0x20801823 | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_INFO_V2 | 16 | 2.57 | 160.6 | 530.6 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | host | 0x2a:0x2080012f | RM_CONTROL NV2080_CTRL_CMD_GPU_QUERY_ECC_STATUS | 16 | 2.29 | 143.0 | 332.9 | 0x56=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | host | 0x2a:0x20800102 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_INFO_V2 | 16 | 1.76 | 109.9 | 222.8 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x2a:0x20808159 | RM_CONTROL 0x20808159 | 16 | 1.65 | 103.2 | 142.9 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | host | 0x2a:0x2080182b | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_C2C_INFO | 16 | 1.42 | 88.6 | 192.6 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | host | 0x2b:0x79 | RM_ALLOC NV01_EVENT_OS_EVENT | 7 | 1.14 | 162.5 | 202.6 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 40 | host | 0xc9:0x0 | REGISTER_FD | 114 | 1.02 | 9.0 | 201.3 | 0x0=114 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | host | 0x2b:0x80 | RM_ALLOC NV01_DEVICE_0 | 3 | 0.93 | 311.4 | 396.5 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 42 | host | 0x2a:0x20800301 | RM_CONTROL NV2080_CTRL_CMD_EVENT_SET_NOTIFICATION | 7 | 0.59 | 84.0 | 104.6 | 0x0=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 43 | host | 0x2b:0x2080 | RM_ALLOC NV20_SUBDEVICE_0 | 3 | 0.45 | 148.4 | 169.3 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | host | 0x2a:0x801813 | RM_CONTROL NV0080_CTRL_CMD_DMA_SET_PAGE_DIRECTORY | 1 | 0.44 | 443.8 | 443.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 45 | host | 0x2a:0x2080122d | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ENGINE_CONTEXT_PROPERTIES | 26 | 0.34 | 13.2 | 17.1 | 0x0=19,0x56=7 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | host | 0x2a:0x20808162 | RM_CONTROL 0x20808162 | 16 | 0.28 | 17.6 | 45.0 | 0x0=16 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | host | 0x2a:0x208001b0 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_CONSTRUCTED_FALCON_INFO | 2 | 0.27 | 133.2 | 191.3 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 48 | host | 0x2b:0x2081 | RM_ALLOC NV2081_BINAPI | 1 | 0.25 | 250.0 | 250.0 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 49 | host | 0x2a:0x801707 | RM_CONTROL NV0080_CTRL_CMD_FIFO_GET_ENGINE_CONTEXT_PROPERTIES | 2 | 0.20 | 99.1 | 112.4 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 50 | host | 0x2a:0x20803801 | RM_CONTROL NV2080_CTRL_CMD_GRMGR_GET_GR_FS_INFO | 1 | 0.10 | 103.6 | 103.6 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 51 | host | 0x2b:0x41 | RM_ALLOC NV01_ROOT_CLIENT | 3 | 0.08 | 27.9 | 45.4 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 52 | host | 0x2a:0x2080182a | RM_CONTROL NV2080_CTRL_CMD_BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS | 1 | 0.08 | 80.7 | 80.7 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 53 | host | 0x2b:0x40 | RM_ALLOC NV01_MEMORY_LOCAL_USER | 1 | 0.07 | 73.6 | 73.6 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 54 | host | 0x2a:0x20801237 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_ZCULL_MASK | 3 | 0.07 | 22.3 | 41.4 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 55 | host | 0xd9:0x0 | other | 1 | 0.05 | 53.2 | 53.2 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 56 | host | 0x2a:0x20801227 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CAPS_V2 | 1 | 0.04 | 44.8 | 44.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 57 | host | 0x2a:0x20800168 | RM_CONTROL NV2080_CTRL_CMD_GPU_GET_PES_INFO | 3 | 0.04 | 13.9 | 14.3 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 58 | host | 0x2a:0x20801234 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_NUM_TPCS_FOR_GPC | 3 | 0.04 | 13.3 | 14.1 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 59 | host | 0x2a:0x2080122b | RM_CONTROL NV2080_CTRL_CMD_GR_GET_TPC_MASK | 3 | 0.04 | 12.8 | 13.1 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 60 | host | 0x2b:0xc361 | RM_ALLOC VOLTA_USERMODE_A | 2 | 0.04 | 17.8 | 20.9 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

| kind | rows | count | cost ms |
|---|---|---|---|
| host | 66 | 74313 | 5181.20 |
| shadow | 6 | 68714 | 3253.82 |
