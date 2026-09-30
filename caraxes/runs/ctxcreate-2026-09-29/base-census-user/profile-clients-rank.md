# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-census-user/profile.1.tsv, scope clients, cost guest

1504 rows, 165955.3 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 296828 | 29583.88 | 99.7 | 2524.7 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | access | bar3:R:0xfea000:- | - | 151055 | 13402.73 | 88.7 | 13176.6 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 3 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 12780.52 | 166.5 | 22601.1 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 4 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 99700 | 12161.84 | 122.0 | 13149.0 | - | - | - | bar2area.rs | - |
| 5 | host | 0x29:0x0 | RM_FREE | 122913 | 9949.48 | 80.9 | 30443.7 | 0x0=162 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | shadow | map | map | 152924 | 7500.76 | 49.0 | 15182.6 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 152924 | 7364.45 | 48.2 | 15075.4 | 0x0=40 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 20351 | 6609.36 | 324.8 | 18070.2 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 9 | shadow | gather alloc | gather alloc | 2414 | 6409.80 | 2655.3 | 19412.8 | - | - | - | shadow.rs (worker) | - |
| 10 | shadow | unmap | unmap | 147408 | 6303.30 | 42.8 | 11979.2 | - | - | - | shadow.rs (worker) | - |
| 11 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 147408 | 6255.89 | 42.4 | 11978.5 | 0x0=13 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 12 | rpc | 0xa | FREE | 7547 | 5478.41 | 725.9 | 17885.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 13 | shadow | object alloc | object alloc | 113477 | 5190.52 | 45.7 | 9911.6 | - | - | - | shadow.rs (worker) | - |
| 14 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 113477 | 5143.36 | 45.3 | 9910.0 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | shadow | object free | object free | 112838 | 4600.64 | 40.8 | 8453.0 | - | - | - | shadow.rs (worker) | - |
| 16 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 3543.51 | 46.2 | 7548.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 17 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 3176.95 | 41.4 | 6126.7 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 18 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 2128 | 2914.44 | 1369.6 | 3103.3 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 4534 | 1789.00 | 394.6 | 254757.5 | 0x0=23 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 2120 | 1348.59 | 636.1 | 171397.3 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 21 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 848 | 1055.73 | 1245.0 | 5116.3 | 0x0=8 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 20346 | 955.55 | 47.0 | 2272.8 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 23 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 54771 | 935.71 | 17.1 | 6620.2 | - | - | - | bar1pool.rs | - |
| 24 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 205 | 854.41 | 4167.9 | 8733.9 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 25 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 20351 | 794.60 | 39.0 | 7518.3 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 26 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 20351 | 761.12 | 37.4 | 1948.5 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 27 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 54272 | 720.64 | 13.3 | 1109.1 | - | - | - | bar1pool.rs | - |
| 28 | rpc | 0x4c | GSP_RM_CONTROL | 5117 | 626.60 | 122.5 | 4171.0 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 29 | access | bar0:W:0x1000:0x1478 | NV_PBUS_SW_SCRATCH(i) | 4526 | 576.90 | 127.5 | 1986.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PBUS_SW_SCRATCH(i) |
| 30 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 2128 | 534.07 | 251.0 | 3061.6 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 432 | 406.24 | 940.4 | 2722.3 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 2128 | 384.39 | 180.6 | 2196.6 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | shadow | gather free | gather free | 2414 | 300.91 | 124.7 | 3341.8 | - | - | - | shadow.rs (worker) | - |
| 34 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 601 | 252.29 | 419.8 | 3777.8 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 121 | 246.96 | 2041.0 | 4163.3 | 0x0=1 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 36 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 4240 | 234.03 | 55.2 | 1655.2 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 37 | access | cfg:R:0x0:0x4 | - | 5321 | 194.02 | 36.5 | 48.6 | - | pci-config | - | config.rs | - |
| 38 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 754 | 182.47 | 242.0 | 8902.5 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | access | cfg:R:0x0:0x78 | - | 10550 | 172.65 | 16.4 | 18.5 | - | pci-config | - | config.rs | - |
| 40 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 452 | 171.98 | 380.5 | 9279.4 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | access | cfg:R:0x0:0x60 | - | 10548 | 171.50 | 16.3 | 19.6 | - | pci-config | - | config.rs | - |
| 42 | access | cfg:R:0x0:0x68 | - | 10548 | 155.88 | 14.8 | 30.1 | - | pci-config | - | config.rs | - |
| 43 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 114 | 145.07 | 1272.6 | 1415.5 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 424 | 139.54 | 329.1 | 3927.0 | 0x0=4 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 45 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 609 | 125.25 | 205.7 | 2995.0 | 0x0=9 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | access | bar0:W:0x713000:- | NV_PRAMIN | 2880 | 123.24 | 42.8 | 1733.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 47 | rpc | 0x67 | GSP_RM_ALLOC | 7665 | 122.10 | 15.9 | 624.3 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 48 | access | bar0:W:0x712000:- | NV_PRAMIN | 2880 | 116.73 | 40.5 | 22.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 49 | control | 0xa06c0105 | NVA06C_CTRL_CMD_PREEMPT | 198 | 115.23 | 582.0 | 3789.0 | 0x0=2 | C | echo | cuda.rs | host (forwarded to the mirror object; the guest sees NV_OK) |
| 50 | access | bar0:W:0x70a000:- | NV_PRAMIN | 2880 | 110.82 | 38.5 | 21.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 51 | access | bar0:W:0x714000:- | NV_PRAMIN | 2880 | 108.45 | 37.7 | 21.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 52 | access | bar0:W:0x70d000:- | NV_PRAMIN | 3000 | 107.50 | 35.8 | 94.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 53 | access | bar0:W:0x708000:- | NV_PRAMIN | 2880 | 107.50 | 37.3 | 22.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 54 | access | bar0:W:0x710000:- | NV_PRAMIN | 2880 | 103.61 | 36.0 | 16.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 55 | host | 0x4e:0x0 | RM_MAP_MEMORY | 627 | 101.77 | 162.3 | 1101.0 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 56 | access | cfg:R:0x0:0x34 | - | 5320 | 99.04 | 18.6 | 193.2 | - | pci-config | - | config.rs | - |
| 57 | access | bar0:W:0x707000:- | NV_PRAMIN | 2880 | 95.07 | 33.0 | 28.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 58 | access | bar0:W:0x711000:- | NV_PRAMIN | 2880 | 93.87 | 32.6 | 16.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 59 | access | bar0:W:0x70b000:- | NV_PRAMIN | 2880 | 93.57 | 32.5 | 19.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 121 | 92.65 | 765.7 | 910.3 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 1328 | 875359 | 76701.93 |
| host | 59 | 562013 | 38264.08 |
| shadow | 6 | 531475 | 30305.92 |
| window | 21 | 779079 | 13818.19 |
| rpc | 5 | 20350 | 6227.33 |
| control | 68 | 5195 | 608.26 |
| alloc | 17 | 7674 | 29.59 |
