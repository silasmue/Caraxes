# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new-census/profile.1.tsv, scope clients, cost guest

1511 rows, 157177.1 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 251931 | 21484.34 | 85.3 | 2352.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 99074 | 12242.37 | 123.6 | 19818.2 | - | - | - | bar2area.rs | - |
| 3 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 12012.39 | 156.5 | 123830.3 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 4 | window | bar2 memfd: sync scan (units: pages compared) | bar2 memfd: sync scan (units: pages compared) | 99074 | 11812.82 | 119.2 | 19817.0 | - | - | - | bar2area.rs | - |
| 5 | access | bar3:R:0xfea000:- | - | 151055 | 11698.04 | 77.4 | 19836.6 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 6 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 20351 | 6793.58 | 333.8 | 10131.7 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 7 | shadow | gather alloc | gather alloc | 2326 | 6510.38 | 2799.0 | 8401.3 | - | - | - | shadow.rs (worker) | - |
| 8 | host | 0x29:0x0 | RM_FREE | 122817 | 6500.01 | 52.9 | 32877.5 | 0x0=159 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 9 | shadow | map | map | 152580 | 4215.99 | 27.6 | 5088.5 | - | - | - | shadow.rs (worker) | - |
| 10 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 152580 | 4164.87 | 27.3 | 5087.0 | 0x0=40 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 11 | shadow | unmap | unmap | 147041 | 4090.74 | 27.8 | 9715.3 | - | - | - | shadow.rs (worker) | - |
| 12 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 147041 | 4062.81 | 27.6 | 9714.6 | 0x0=13 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | shadow | object alloc | object alloc | 113470 | 4018.31 | 35.4 | 5046.0 | - | - | - | shadow.rs (worker) | - |
| 14 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 113470 | 3976.90 | 35.0 | 5045.1 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | rpc | 0xa | FREE | 7547 | 3526.76 | 467.3 | 10086.3 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 16 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 3303.58 | 43.0 | 14508.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 17 | window | gather: window of mremaps (units: runs) | gather: window of mremaps (units: runs) | 2326 | 3226.01 | 1386.9 | 4776.1 | - | - | - | - | - |
| 18 | shadow | object free | object free | 112831 | 3114.15 | 27.6 | 5546.8 | - | - | - | shadow.rs (worker) | - |
| 19 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 2128 | 3050.63 | 1433.6 | 5416.2 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 54272 | 2938.86 | 54.2 | 3071.2 | - | - | - | bar1pool.rs | - |
| 21 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 2919.47 | 38.0 | 7960.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 22 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 54764 | 2542.11 | 46.4 | 2224.3 | - | - | - | bar1pool.rs | - |
| 23 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 4445 | 1717.03 | 386.3 | 856114.9 | 0x0=20 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 24 | window | gather: host descriptor (units: pages) | gather: host descriptor (units: pages) | 2326 | 1646.41 | 707.8 | 4071.5 | - | - | - | - | - |
| 25 | window | gather: window unmap (units: pages) | gather: window unmap (units: pages) | 2326 | 1630.20 | 700.9 | 4344.5 | - | - | - | - | - |
| 26 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 2120 | 1219.04 | 575.0 | 87109.6 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 27 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 20346 | 1053.92 | 51.8 | 84.3 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 28 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 848 | 1031.65 | 1216.6 | 6463.5 | 0x0=8 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 29 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 186 | 918.49 | 4938.1 | 10180.4 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 30 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 20351 | 910.86 | 44.8 | 4426.1 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 31 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 20351 | 836.83 | 41.1 | 2342.3 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 32 | access | bar0:W:0x1000:0x1478 | NV_PBUS_SW_SCRATCH(i) | 4526 | 673.67 | 148.8 | 383.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PBUS_SW_SCRATCH(i) |
| 33 | rpc | 0x4c | GSP_RM_CONTROL | 5117 | 591.53 | 115.6 | 3621.8 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 34 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 2128 | 485.60 | 228.2 | 3638.4 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 432 | 412.38 | 954.6 | 3767.7 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 2128 | 372.53 | 175.1 | 2168.4 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 4240 | 239.18 | 56.4 | 1809.9 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 38 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 121 | 227.07 | 1876.6 | 3616.9 | 0x0=1 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 39 | access | cfg:R:0x0:0x4 | - | 5021 | 218.40 | 43.5 | 66.4 | - | pci-config | - | config.rs | - |
| 40 | shadow | gather free | gather free | 2326 | 214.09 | 92.0 | 1107.5 | - | - | - | shadow.rs (worker) | - |
| 41 | window | bar2 memfd: sync drift check | bar2 memfd: sync drift check | 1942 | 166.27 | 85.6 | 5169.0 | - | - | - | bar2area.rs | - |
| 42 | access | cfg:R:0x0:0x68 | - | 9952 | 165.19 | 16.6 | 1903.5 | - | pci-config | - | config.rs | - |
| 43 | access | cfg:R:0x0:0x60 | - | 9952 | 159.47 | 16.0 | 137.5 | - | pci-config | - | config.rs | - |
| 44 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 559 | 158.21 | 283.0 | 538.6 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 45 | access | cfg:R:0x0:0x78 | - | 9952 | 150.98 | 15.2 | 1964.0 | - | pci-config | - | config.rs | - |
| 46 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 114 | 144.60 | 1268.5 | 1406.8 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | rpc | 0x67 | GSP_RM_ALLOC | 7665 | 132.79 | 17.3 | 981.6 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 48 | host | 0x4e:0x0 | RM_MAP_MEMORY | 630 | 114.40 | 181.6 | 2171.0 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 49 | window | bar2 memfd: sync table walk | bar2 memfd: sync table walk | 99074 | 108.04 | 1.1 | 969.2 | - | - | - | bar2area.rs | - |
| 50 | access | bar0:R:0xbb0000:0xbb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 2120 | 96.14 | 45.4 | 1618.4 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 51 | access | bar0:W:0x70c000:- | NV_PRAMIN | 2880 | 95.98 | 33.3 | 29.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 52 | access | bar0:W:0x70b000:- | NV_PRAMIN | 2880 | 95.95 | 33.3 | 35.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 53 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 452 | 94.64 | 209.4 | 2085.2 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 54 | access | bar0:W:0x70d000:- | NV_PRAMIN | 3000 | 93.29 | 31.1 | 22.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 55 | access | cfg:R:0x0:0x34 | - | 5020 | 92.44 | 18.4 | 2154.1 | - | pci-config | - | config.rs | - |
| 56 | access | bar0:W:0x715000:- | NV_PRAMIN | 2880 | 91.54 | 31.8 | 14.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 57 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 612 | 90.30 | 147.6 | 4216.0 | 0x0=9 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 58 | access | bar0:W:0x70f000:- | NV_PRAMIN | 2880 | 90.23 | 31.3 | 33.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 59 | access | bar0:W:0x709000:- | NV_PRAMIN | 2880 | 89.21 | 31.0 | 28.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:W:0x70a000:- | NV_PRAMIN | 2880 | 88.26 | 30.6 | 12.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 1328 | 827798 | 65928.03 |
| window | 28 | 952424 | 36313.25 |
| host | 59 | 560558 | 27905.80 |
| shadow | 6 | 530574 | 22163.66 |
| rpc | 5 | 20350 | 4251.27 |
| control | 68 | 5195 | 575.06 |
| alloc | 17 | 7674 | 39.99 |
