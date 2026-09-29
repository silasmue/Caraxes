# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new2-bench-b/profile.1.tsv, scope clients, cost guest

1510 rows, 170913.7 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 266936 | 21484.34 | 80.5 | 1113.3 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 109824 | 14367.28 | 130.8 | 35198.3 | - | - | - | bar2area.rs | - |
| 3 | window | bar2 memfd: sync scan (units: pages compared) | bar2 memfd: sync scan (units: pages compared) | 109824 | 14045.76 | 127.9 | 35196.0 | - | - | - | bar2area.rs | - |
| 4 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 13528.63 | 164.3 | 119241.1 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 5 | access | bar3:R:0xfea000:- | - | 167033 | 12485.19 | 74.7 | 35222.1 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 6 | shadow | gather alloc | gather alloc | 3169 | 8818.56 | 2782.8 | 10076.3 | - | - | - | shadow.rs (worker) | - |
| 7 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 26333 | 8682.57 | 329.7 | 10909.0 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 8 | host | 0x29:0x0 | RM_FREE | 129240 | 7544.70 | 58.4 | 25340.4 | 0x0=159 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 9 | rpc | 0xa | FREE | 9934 | 4647.67 | 467.9 | 9838.9 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 10 | shadow | map | map | 156219 | 4513.92 | 28.9 | 9439.6 | - | - | - | shadow.rs (worker) | - |
| 11 | window | gather: window of mremaps (units: runs) | gather: window of mremaps (units: runs) | 3169 | 4486.77 | 1415.8 | 8625.4 | - | - | - | - | - |
| 12 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 156219 | 4463.02 | 28.6 | 9438.9 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | shadow | object alloc | object alloc | 116686 | 3979.81 | 34.1 | 5798.8 | - | - | - | shadow.rs (worker) | - |
| 14 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 116686 | 3936.27 | 33.7 | 5797.7 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | shadow | unmap | unmap | 148940 | 3784.31 | 25.4 | 4335.8 | - | - | - | shadow.rs (worker) | - |
| 16 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 148940 | 3753.50 | 25.2 | 4335.2 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 17 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 2828 | 3217.74 | 1137.8 | 3402.5 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 18 | shadow | object free | object free | 115892 | 3167.89 | 27.3 | 3276.4 | - | - | - | shadow.rs (worker) | - |
| 19 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 3134.66 | 38.1 | 6523.2 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 20 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 2512.52 | 30.5 | 8211.2 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 21 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 5988 | 2276.11 | 380.1 | 346725.7 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | window | gather: window unmap (units: pages) | gather: window unmap (units: pages) | 3169 | 2209.04 | 697.1 | 2492.7 | - | - | - | - | - |
| 23 | window | gather: host descriptor (units: pages) | gather: host descriptor (units: pages) | 3169 | 2111.83 | 666.4 | 4799.1 | - | - | - | - | - |
| 24 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 1128 | 1897.58 | 1682.3 | 4382.2 | 0x0=8 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 25 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 72192 | 1813.65 | 25.1 | 3171.1 | - | - | - | bar1pool.rs | - |
| 26 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 2820 | 1525.42 | 540.9 | 82293.5 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 27 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 26328 | 1141.09 | 43.3 | 90.3 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 28 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 72684 | 1126.46 | 15.5 | 2754.3 | - | - | - | bar1pool.rs | - |
| 29 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 26333 | 937.61 | 35.6 | 4591.9 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 30 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 26333 | 913.30 | 34.7 | 57.3 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 31 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 2828 | 698.36 | 246.9 | 4410.4 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | rpc | 0x4c | GSP_RM_CONTROL | 6313 | 690.32 | 109.3 | 3666.6 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 33 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 179 | 650.07 | 3631.7 | 9436.1 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 572 | 485.65 | 849.0 | 2200.6 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 2828 | 428.42 | 151.5 | 2147.1 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 161 | 293.83 | 1825.0 | 3662.1 | 0x0=1 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 37 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 5640 | 293.55 | 52.0 | 10.6 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 38 | shadow | gather free | gather free | 3168 | 239.39 | 75.6 | 811.9 | - | - | - | shadow.rs (worker) | - |
| 39 | rpc | 0x67 | GSP_RM_ALLOC | 10064 | 194.93 | 19.4 | 2263.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 40 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 149 | 191.81 | 1287.3 | 1687.1 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 597 | 167.62 | 280.8 | 515.9 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 42 | access | cfg:R:0x0:0x4 | - | 4093 | 147.99 | 36.2 | 24.5 | - | pci-config | - | config.rs | - |
| 43 | host | 0x4e:0x0 | RM_MAP_MEMORY | 792 | 130.95 | 165.3 | 1264.9 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 161 | 127.71 | 793.2 | 2609.9 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 45 | window | bar2 memfd: sync drift check | bar2 memfd: sync drift check | 2050 | 121.86 | 59.4 | 1783.5 | - | - | - | bar2area.rs | - |
| 46 | access | cfg:R:0x0:0x68 | - | 8112 | 116.25 | 14.3 | 614.8 | - | pci-config | - | config.rs | - |
| 47 | access | cfg:R:0x0:0x60 | - | 8112 | 110.77 | 13.7 | 0.7 | - | pci-config | - | config.rs | - |
| 48 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 774 | 107.60 | 139.0 | 2801.1 | 0x0=9 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 49 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 564 | 107.41 | 190.4 | 3756.0 | 0x0=4 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 50 | access | cfg:R:0x0:0x78 | - | 8112 | 106.57 | 13.1 | 2012.6 | - | pci-config | - | config.rs | - |
| 51 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 602 | 105.85 | 175.8 | 649.7 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 52 | host | 0x2a:0x801714 | RM_CONTROL NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS | 161 | 100.87 | 626.5 | 701.3 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 53 | access | bar0:W:0x715000:- | NV_PRAMIN | 2880 | 99.45 | 34.5 | 15.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 54 | host | 0x2a:0xc36f0108 | RM_CONTROL NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN | 2828 | 98.89 | 35.0 | 1667.6 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 55 | window | bar2 memfd: sync table walk | bar2 memfd: sync table walk | 109824 | 94.51 | 0.9 | 1039.3 | - | - | - | bar2area.rs | - |
| 56 | access | bar0:R:0xbb0000:0xbb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 2820 | 91.72 | 32.5 | 12.0 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 57 | access | cfg:R:0x0:0x34 | - | 4092 | 82.62 | 20.2 | 1394.2 | - | pci-config | - | config.rs | - |
| 58 | access | bar0:W:0x70f000:- | NV_PRAMIN | 2880 | 81.01 | 28.1 | 5.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 59 | access | bar0:W:0x710000:- | NV_PRAMIN | 2880 | 80.48 | 27.9 | 8.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:W:0x709000:- | NV_PRAMIN | 2880 | 79.89 | 27.7 | 8.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 1327 | 889856 | 68974.93 |
| window | 28 | 1149491 | 40377.25 |
| host | 59 | 583431 | 30803.93 |
| shadow | 6 | 544074 | 24503.89 |
| rpc | 5 | 26332 | 5533.11 |
| control | 68 | 6391 | 652.57 |
| alloc | 17 | 10066 | 68.01 |
