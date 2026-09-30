# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new-census-user/profile.1.tsv, scope clients, cost guest

1511 rows, 207520.7 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 307126 | 33436.47 | 108.9 | 7928.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 14373.33 | 187.2 | 119341.4 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 3 | access | bar3:R:0xfea000:- | - | 151055 | 14328.82 | 94.9 | 10330.7 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 4 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 99827 | 14011.23 | 140.4 | 11475.9 | - | - | - | bar2area.rs | - |
| 5 | window | bar2 memfd: sync scan (units: pages compared) | bar2 memfd: sync scan (units: pages compared) | 99827 | 13643.23 | 136.7 | 11474.2 | - | - | - | bar2area.rs | - |
| 6 | shadow | map | map | 152351 | 9271.86 | 60.9 | 13299.1 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 152351 | 9107.32 | 59.8 | 13297.4 | 0x0=39 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | host | 0x29:0x0 | RM_FREE | 122947 | 8205.60 | 66.7 | 35304.0 | 0x0=162 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 9 | shadow | object alloc | object alloc | 113469 | 7553.91 | 66.6 | 16574.9 | - | - | - | shadow.rs (worker) | - |
| 10 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 113469 | 7506.59 | 66.2 | 16572.6 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 11 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 20351 | 7262.08 | 356.8 | 10983.0 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 12 | shadow | gather alloc | gather alloc | 2456 | 7192.46 | 2928.5 | 9932.6 | - | - | - | shadow.rs (worker) | - |
| 13 | shadow | unmap | unmap | 147203 | 6637.01 | 45.1 | 10058.2 | - | - | - | shadow.rs (worker) | - |
| 14 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 147203 | 6596.55 | 44.8 | 10057.8 | 0x0=12 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | shadow | object free | object free | 112830 | 4448.58 | 39.4 | 11381.5 | - | - | - | shadow.rs (worker) | - |
| 16 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 4084.41 | 53.2 | 17387.1 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 17 | rpc | 0xa | FREE | 7547 | 3797.34 | 503.2 | 4258.2 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 18 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 3425.21 | 44.6 | 2421.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 19 | window | gather: window of mremaps (units: runs) | gather: window of mremaps (units: runs) | 2456 | 3414.02 | 1390.1 | 4013.8 | - | - | - | - | - |
| 20 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 2128 | 3156.46 | 1483.3 | 3393.6 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 21 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 54272 | 2310.48 | 42.6 | 1944.6 | - | - | - | bar1pool.rs | - |
| 22 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 4576 | 2163.96 | 472.9 | 373205.9 | 0x0=23 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 23 | window | gather: host descriptor (units: pages) | gather: host descriptor (units: pages) | 2456 | 2016.25 | 821.0 | 9631.2 | - | - | - | - | - |
| 24 | window | gather: window unmap (units: pages) | gather: window unmap (units: pages) | 2456 | 1753.02 | 713.8 | 2233.1 | - | - | - | - | - |
| 25 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 54771 | 1641.75 | 30.0 | 2495.9 | - | - | - | bar1pool.rs | - |
| 26 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 2120 | 1333.23 | 628.9 | 227838.4 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 27 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 20346 | 1185.20 | 58.3 | 1436.1 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 28 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 212 | 1145.93 | 5405.3 | 13519.3 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 29 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 848 | 1085.96 | 1280.6 | 2861.0 | 0x0=8 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 30 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 20351 | 1037.67 | 51.0 | 6865.7 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 31 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 20351 | 894.49 | 44.0 | 2225.9 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 32 | rpc | 0x4c | GSP_RM_CONTROL | 5117 | 744.83 | 145.6 | 10928.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 33 | access | bar0:W:0x1000:0x1478 | NV_PBUS_SW_SCRATCH(i) | 4526 | 646.77 | 142.9 | 106.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PBUS_SW_SCRATCH(i) |
| 34 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 2128 | 603.48 | 283.6 | 3935.2 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 432 | 432.48 | 1001.1 | 2794.6 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 2128 | 425.39 | 199.9 | 1023.7 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | shadow | gather free | gather free | 2456 | 351.83 | 143.3 | 5185.6 | - | - | - | shadow.rs (worker) | - |
| 38 | access | cfg:R:0x0:0x4 | - | 4905 | 309.77 | 63.2 | 1564.6 | - | pci-config | - | config.rs | - |
| 39 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 4240 | 254.12 | 59.9 | 1445.0 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 40 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 121 | 223.65 | 1848.4 | 2215.4 | 0x0=1 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 41 | access | cfg:R:0x0:0x60 | - | 9722 | 217.98 | 22.4 | 2194.1 | - | pci-config | - | config.rs | - |
| 42 | access | cfg:R:0x0:0x78 | - | 9722 | 213.10 | 21.9 | 876.1 | - | pci-config | - | config.rs | - |
| 43 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 808 | 195.76 | 242.3 | 6116.3 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | access | cfg:R:0x0:0x68 | - | 9722 | 170.72 | 17.6 | 314.9 | - | pci-config | - | config.rs | - |
| 45 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 552 | 161.62 | 292.8 | 473.7 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 424 | 145.22 | 342.5 | 4036.3 | 0x0=4 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 114 | 144.29 | 1265.7 | 1492.8 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 48 | window | bar2 memfd: sync drift check | bar2 memfd: sync drift check | 2342 | 143.09 | 61.1 | 2492.9 | - | - | - | bar2area.rs | - |
| 49 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 609 | 139.28 | 228.7 | 3812.2 | 0x0=9 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 50 | rpc | 0x67 | GSP_RM_ALLOC | 7665 | 135.59 | 17.7 | 639.1 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 51 | access | bar0:W:0x70d000:- | NV_PRAMIN | 3000 | 128.09 | 42.7 | 28.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 52 | control | 0x20802a0c | NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO | 297 | 127.44 | 429.1 | 10924.1 | 0x0=3 | A | host-forwarded | cuda.rs | host (forwarded as-is; NON_PRIVILEGED on the 615 host) |
| 53 | host | 0x2a:0x20802a0c | RM_CONTROL NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO | 297 | 125.85 | 423.7 | 10916.5 | 0x0=3 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 54 | host | 0x52:0x0 | RM_GET_EVENT_DATA | 44307 | 119.98 | 2.7 | 138.8 | 0x0=9 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 55 | access | cfg:R:0x0:0x34 | - | 4904 | 119.61 | 24.4 | 119.4 | - | pci-config | - | config.rs | - |
| 56 | host | 0x4e:0x0 | RM_MAP_MEMORY | 627 | 114.49 | 182.6 | 1421.0 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 57 | access | bar0:W:0x714000:- | NV_PRAMIN | 2880 | 108.52 | 37.7 | 23.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 58 | access | bar0:W:0x707000:- | NV_PRAMIN | 2880 | 102.65 | 35.6 | 18.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 59 | window | bar2 memfd: sync table walk | bar2 memfd: sync table walk | 99827 | 100.02 | 1.0 | 1204.3 | - | - | - | bar2area.rs | - |
| 60 | access | bar0:W:0x706000:- | NV_PRAMIN | 2880 | 95.25 | 33.1 | 41.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 1328 | 881967 | 85393.38 |
| host | 59 | 601022 | 42199.83 |
| window | 28 | 1293046 | 39033.09 |
| shadow | 6 | 530765 | 35455.65 |
| rpc | 5 | 20350 | 4677.98 |
| control | 68 | 5195 | 725.83 |
| alloc | 17 | 7674 | 34.91 |
