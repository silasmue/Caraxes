# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-census5/profile.1.tsv, scope clients, cost guest

1506 rows, 129717.2 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 256098 | 22093.09 | 86.3 | 70503.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 11758.79 | 153.2 | 105080.1 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 3 | access | bar3:R:0xfea000:- | - | 151055 | 11446.42 | 75.8 | 77408.4 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 4 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 98075 | 10975.82 | 111.9 | 10981.4 | - | - | - | bar2area.rs | - |
| 5 | shadow | gather alloc | gather alloc | 2414 | 6382.72 | 2644.0 | 8960.2 | - | - | - | shadow.rs (worker) | - |
| 6 | host | 0x29:0x0 | RM_FREE | 121108 | 5904.16 | 48.8 | 26528.7 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 20351 | 5892.15 | 289.5 | 4790.5 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 8 | shadow | map | map | 152280 | 4583.07 | 30.1 | 3936.3 | - | - | - | shadow.rs (worker) | - |
| 9 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 152280 | 4533.33 | 29.8 | 3935.6 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | shadow | object alloc | object alloc | 113470 | 4174.21 | 36.8 | 4939.8 | - | - | - | shadow.rs (worker) | - |
| 11 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 113470 | 4109.07 | 36.2 | 4938.7 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 12 | shadow | unmap | unmap | 145627 | 3766.09 | 25.9 | 8043.4 | - | - | - | shadow.rs (worker) | - |
| 13 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 145627 | 3734.96 | 25.6 | 8042.7 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | shadow | object free | object free | 112831 | 3391.72 | 30.1 | 8498.7 | - | - | - | shadow.rs (worker) | - |
| 15 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 3134.67 | 40.8 | 2334.8 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 16 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 2821.03 | 36.7 | 8068.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 17 | rpc | 0xa | FREE | 7547 | 2628.47 | 348.3 | 3343.0 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 18 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 1624 | 2241.41 | 1380.2 | 2829.3 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 4010 | 1647.87 | 410.9 | 429580.4 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 54272 | 1574.91 | 29.0 | 23492.4 | - | - | - | bar1pool.rs | - |
| 21 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 848 | 1003.69 | 1183.6 | 2167.8 | 0x0=8 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 188 | 885.06 | 4707.8 | 42437.4 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 23 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 20346 | 878.28 | 43.2 | 71.4 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 24 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 2120 | 757.15 | 357.1 | 132175.1 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 25 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 20351 | 738.76 | 36.3 | 60.2 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 26 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 20351 | 703.48 | 34.6 | 1854.1 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 27 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 36856 | 605.57 | 16.4 | 1264.0 | - | - | - | bar1pool.rs | - |
| 28 | rpc | 0x4c | GSP_RM_CONTROL | 5117 | 602.25 | 117.7 | 4562.7 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 29 | access | bar0:W:0x1000:0x1478 | NV_PBUS_SW_SCRATCH(i) | 4526 | 345.78 | 76.4 | 2409.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PBUS_SW_SCRATCH(i) |
| 30 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 1624 | 338.14 | 208.2 | 1358.8 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | window | bar1 pool: page moved out, cold (units: bytes) | bar1 pool: page moved out, cold (units: bytes) | 20487 | 302.35 | 14.8 | 1296.0 | - | - | - | bar1pool.rs | - |
| 32 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 1624 | 279.16 | 171.9 | 3673.2 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 992 | 276.65 | 278.9 | 22548.0 | 0x0=9 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 306 | 249.37 | 814.9 | 2001.7 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 4240 | 212.57 | 50.1 | 18.8 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 36 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 121 | 209.49 | 1731.3 | 2029.2 | 0x0=1 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 37 | shadow | gather free | gather free | 2413 | 205.28 | 85.1 | 202.2 | - | - | - | shadow.rs (worker) | - |
| 38 | host | 0x4e:0x0 | RM_MAP_MEMORY | 1010 | 203.54 | 201.5 | 9585.8 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | access | cfg:R:0x0:0x4 | - | 5021 | 195.84 | 39.0 | 2102.3 | - | pci-config | - | config.rs | - |
| 40 | access | cfg:R:0x0:0x68 | - | 9952 | 152.26 | 15.3 | 1192.5 | - | pci-config | - | config.rs | - |
| 41 | access | cfg:R:0x0:0x60 | - | 9952 | 143.45 | 14.4 | 26.7 | - | pci-config | - | config.rs | - |
| 42 | access | cfg:R:0x0:0x78 | - | 9952 | 139.67 | 14.0 | 658.2 | - | pci-config | - | config.rs | - |
| 43 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 114 | 136.54 | 1197.8 | 1310.5 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 424 | 114.46 | 270.0 | 2494.6 | 0x0=4 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 45 | rpc | 0x67 | GSP_RM_ALLOC | 7665 | 109.94 | 14.3 | 534.6 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 46 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 262 | 99.21 | 378.7 | 4545.8 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | access | cfg:R:0x0:0x34 | - | 5020 | 98.87 | 19.7 | 2978.4 | - | pci-config | - | config.rs | - |
| 48 | access | bar0:W:0x702000:- | NV_PRAMIN | 2880 | 96.09 | 33.4 | 53.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 49 | access | bar0:W:0x706000:- | NV_PRAMIN | 2880 | 94.16 | 32.7 | 14.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 50 | access | bar0:W:0x707000:- | NV_PRAMIN | 2880 | 91.94 | 31.9 | 12.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 51 | access | bar0:R:0xbb0000:0xbb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 2120 | 89.08 | 42.0 | 1894.9 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 52 | access | bar0:W:0x70d000:- | NV_PRAMIN | 3000 | 87.84 | 29.3 | 62.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 53 | access | bar0:W:0x708000:- | NV_PRAMIN | 2880 | 87.40 | 30.3 | 13.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 54 | control | 0xa06c0105 | NVA06C_CTRL_CMD_PREEMPT | 198 | 86.19 | 435.3 | 4557.0 | 0x0=2 | C | echo | cuda.rs | host (forwarded to the mirror object; the guest sees NV_OK) |
| 55 | access | bar0:W:0x714000:- | NV_PRAMIN | 2880 | 85.85 | 29.8 | 30.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 56 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 121 | 85.70 | 708.3 | 812.4 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 57 | access | bar0:W:0x715000:- | NV_PRAMIN | 2880 | 85.08 | 29.5 | 13.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 58 | access | bar0:W:0x713000:- | NV_PRAMIN | 2880 | 84.95 | 29.5 | 32.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 59 | access | bar0:W:0x711000:- | NV_PRAMIN | 2880 | 84.13 | 29.2 | 10.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:W:0x70e000:- | NV_PRAMIN | 2880 | 83.99 | 29.2 | 15.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 1328 | 831965 | 63440.77 |
| host | 59 | 553720 | 26359.49 |
| shadow | 6 | 529035 | 22503.09 |
| window | 23 | 770671 | 13458.66 |
| rpc | 5 | 20350 | 3340.82 |
| control | 68 | 5195 | 589.41 |
| alloc | 17 | 7674 | 25.01 |
