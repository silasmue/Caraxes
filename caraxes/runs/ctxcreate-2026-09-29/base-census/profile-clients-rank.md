# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-census/profile.1.tsv, scope clients, cost guest

1506 rows, 133545.4 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 248763 | 21699.05 | 87.2 | 8195.7 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 12499.66 | 162.8 | 106525.8 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 3 | access | bar3:R:0xfea000:- | - | 151055 | 12360.78 | 81.8 | 80385.9 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 4 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 98876 | 11977.52 | 121.1 | 18125.0 | - | - | - | bar2area.rs | - |
| 5 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 20351 | 6357.52 | 312.4 | 7023.1 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 6 | shadow | gather alloc | gather alloc | 2359 | 6272.74 | 2659.1 | 8595.0 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x29:0x0 | RM_FREE | 121228 | 5968.69 | 49.2 | 38169.0 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | shadow | map | map | 152045 | 4409.54 | 29.0 | 9104.3 | - | - | - | shadow.rs (worker) | - |
| 9 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 152045 | 4356.38 | 28.7 | 9102.6 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | shadow | object alloc | object alloc | 113485 | 4039.46 | 35.6 | 4511.7 | - | - | - | shadow.rs (worker) | - |
| 11 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 113485 | 3997.60 | 35.2 | 4511.0 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 12 | shadow | unmap | unmap | 145585 | 3863.00 | 26.5 | 5393.1 | - | - | - | shadow.rs (worker) | - |
| 13 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 145585 | 3816.25 | 26.2 | 5391.3 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 3468.00 | 45.2 | 3842.8 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 15 | shadow | object free | object free | 112846 | 3252.76 | 28.8 | 5147.7 | - | - | - | shadow.rs (worker) | - |
| 16 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 76778 | 2975.62 | 38.8 | 9384.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 17 | rpc | 0xa | FREE | 7547 | 2817.46 | 373.3 | 3889.2 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 18 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 1624 | 2409.56 | 1483.7 | 4815.7 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 3975 | 1749.22 | 440.1 | 288673.5 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 54272 | 1700.58 | 31.3 | 21766.8 | - | - | - | bar1pool.rs | - |
| 21 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 195 | 1078.50 | 5530.7 | 43208.6 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 848 | 960.72 | 1132.9 | 2268.5 | 0x0=8 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 23 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 20346 | 887.96 | 43.6 | 60.2 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 24 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 2120 | 809.96 | 382.1 | 139491.7 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 25 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 20351 | 768.46 | 37.8 | 133.1 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 26 | rpc | 0x4c | GSP_RM_CONTROL | 5117 | 732.49 | 143.1 | 6672.1 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 27 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 20351 | 721.50 | 35.5 | 138.2 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 28 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 36863 | 581.32 | 15.8 | 2034.1 | - | - | - | bar1pool.rs | - |
| 29 | access | bar0:W:0x1000:0x1478 | NV_PBUS_SW_SCRATCH(i) | 4526 | 470.56 | 104.0 | 2345.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PBUS_SW_SCRATCH(i) |
| 30 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 1624 | 356.71 | 219.7 | 993.2 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | window | bar1 pool: page moved out, cold (units: bytes) | bar1 pool: page moved out, cold (units: bytes) | 20480 | 269.23 | 13.1 | 1181.5 | - | - | - | bar1pool.rs | - |
| 32 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 993 | 266.24 | 268.1 | 20802.6 | 0x0=9 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 1624 | 257.21 | 158.4 | 1102.1 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 306 | 252.91 | 826.5 | 1838.9 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 4240 | 219.12 | 51.7 | 32.8 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 36 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 121 | 211.61 | 1748.8 | 1828.8 | 0x0=1 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 37 | access | cfg:R:0x0:0x4 | - | 4797 | 208.58 | 43.5 | 676.8 | - | pci-config | - | config.rs | - |
| 38 | shadow | gather free | gather free | 2359 | 192.19 | 81.5 | 177.3 | - | - | - | shadow.rs (worker) | - |
| 39 | access | cfg:R:0x0:0x78 | - | 9508 | 149.48 | 15.7 | 512.0 | - | pci-config | - | config.rs | - |
| 40 | access | cfg:R:0x0:0x60 | - | 9508 | 147.88 | 15.6 | 42.5 | - | pci-config | - | config.rs | - |
| 41 | access | cfg:R:0x0:0x68 | - | 9508 | 145.76 | 15.3 | 60.9 | - | pci-config | - | config.rs | - |
| 42 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 114 | 141.97 | 1245.3 | 1385.8 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 43 | host | 0x4e:0x0 | RM_MAP_MEMORY | 1011 | 139.04 | 137.5 | 616.7 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 44 | rpc | 0x67 | GSP_RM_ALLOC | 7665 | 121.63 | 15.9 | 709.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 45 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 757 | 105.84 | 139.8 | 4972.7 | 0x0=6 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | access | bar0:W:0x70e000:- | NV_PRAMIN | 2880 | 96.93 | 33.7 | 156.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 47 | access | bar0:W:0x70d000:- | NV_PRAMIN | 3000 | 94.39 | 31.5 | 15.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 48 | access | cfg:R:0x0:0x34 | - | 4796 | 94.34 | 19.7 | 347.3 | - | pci-config | - | config.rs | - |
| 49 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 424 | 94.22 | 222.2 | 2433.0 | 0x0=4 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 50 | access | bar0:W:0x714000:- | NV_PRAMIN | 2880 | 89.89 | 31.2 | 14.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 51 | access | bar0:W:0x70f000:- | NV_PRAMIN | 2880 | 89.75 | 31.2 | 26.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 52 | access | bar0:W:0x713000:- | NV_PRAMIN | 2880 | 89.27 | 31.0 | 22.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 53 | access | bar0:W:0x710000:- | NV_PRAMIN | 2880 | 87.84 | 30.5 | 18.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 54 | access | bar0:W:0x715000:- | NV_PRAMIN | 2880 | 86.65 | 30.1 | 14.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 55 | access | bar0:W:0x711000:- | NV_PRAMIN | 2880 | 86.50 | 30.0 | 16.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 56 | access | bar0:W:0x712000:- | NV_PRAMIN | 2880 | 86.37 | 30.0 | 15.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 57 | access | bar0:W:0x70b000:- | NV_PRAMIN | 2880 | 86.23 | 29.9 | 16.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 58 | access | bar0:W:0x70a000:- | NV_PRAMIN | 2880 | 85.47 | 29.7 | 13.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 59 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 121 | 85.08 | 703.2 | 737.3 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 60 | access | bar0:W:0x70c000:- | NV_PRAMIN | 2880 | 84.77 | 29.4 | 23.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 1328 | 822698 | 65905.20 |
| host | 59 | 557718 | 26690.14 |
| shadow | 6 | 528679 | 22029.69 |
| window | 23 | 765337 | 14528.65 |
| rpc | 5 | 20350 | 3671.78 |
| control | 68 | 5195 | 689.45 |
| alloc | 17 | 7674 | 30.51 |
