# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-census/profile.1.tsv, scope device, cost guest

3742 rows, 21950.0 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 31879 | 2763.33 | 86.7 | 8195.7 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 578 | 2412.51 | 4173.9 | 288673.5 | 0x0=578 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 3 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 1650.56 | 166.6 | 106525.8 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 4 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 13064 | 1624.32 | 124.3 | 18125.0 | - | - | - | bar2area.rs | - |
| 5 | access | bar3:R:0xfea000:- | - | 19805 | 1621.77 | 81.9 | 80385.9 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 6 | host | 0x29:0x0 | RM_FREE | 15477 | 1104.91 | 71.4 | 38169.0 | 0x0=15477 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 2920 | 904.01 | 309.6 | 7023.1 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 8 | shadow | gather alloc | gather alloc | 334 | 890.49 | 2666.1 | 8595.0 | - | - | - | shadow.rs (worker) | - |
| 9 | shadow | map | map | 19106 | 560.14 | 29.3 | 9104.3 | - | - | - | shadow.rs (worker) | - |
| 10 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19106 | 553.32 | 29.0 | 9102.6 | 0x0=19106 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 11 | shadow | object alloc | object alloc | 14271 | 514.56 | 36.1 | 4511.7 | - | - | - | shadow.rs (worker) | - |
| 12 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14271 | 509.25 | 35.7 | 4511.0 | 0x0=14271 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | shadow | unmap | unmap | 18250 | 484.57 | 26.6 | 5393.1 | - | - | - | shadow.rs (worker) | - |
| 14 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18250 | 478.71 | 26.2 | 5391.3 | 0x0=18250 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 447.34 | 45.2 | 3842.8 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 16 | shadow | object free | object free | 14286 | 414.07 | 29.0 | 5147.7 | - | - | - | shadow.rs (worker) | - |
| 17 | rpc | 0xa | FREE | 1068 | 402.07 | 376.5 | 3889.2 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 18 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 383.75 | 38.7 | 9384.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 19 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 229 | 340.12 | 1485.2 | 4815.7 | 0x0=229 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | access | bar0:W:0x840000:0x840184 | NV_PSEC (SEC2 falcon) | 8512 | 247.31 | 29.1 | 38.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 21 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 7680 | 237.21 | 30.9 | 21766.8 | - | - | - | bar1pool.rs | - |
| 22 | access | bar0:W:0x840000:0x8401c4 | NV_PSEC (SEC2 falcon) | 6272 | 160.41 | 25.6 | 35.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 23 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 26 | 141.91 | 5458.2 | 43208.6 | 0x0=26 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 24 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 120 | 135.99 | 1133.3 | 2268.5 | 0x0=120 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 25 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 2918 | 126.69 | 43.4 | 60.2 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 26 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 300 | 114.05 | 380.2 | 139491.7 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 27 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 2920 | 110.59 | 37.9 | 133.1 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 28 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 2919 | 103.00 | 35.3 | 138.2 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 29 | rpc | 0x4c | GSP_RM_CONTROL | 751 | 101.27 | 134.8 | 6672.1 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 30 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 5120 | 80.43 | 15.7 | 2034.1 | - | - | - | bar1pool.rs | - |
| 31 | access | bar0:W:0x1000:0x1478 | NV_PBUS_SW_SCRATCH(i) | 568 | 59.17 | 104.2 | 2345.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PBUS_SW_SCRATCH(i) |
| 32 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 229 | 50.39 | 220.0 | 993.2 | 0x0=229 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | access | bar0:R:0x31e000:- | NV_PROM_DATA (VBIOS window) | 960 | 46.14 | 48.1 | 33.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 34 | access | cfg:R:0x0:0x4 | - | 975 | 45.36 | 46.5 | 676.8 | - | pci-config | - | config.rs | - |
| 35 | access | bar0:R:0x317000:- | NV_PROM_DATA (VBIOS window) | 960 | 43.75 | 45.6 | 30.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 36 | access | bar0:R:0x316000:- | NV_PROM_DATA (VBIOS window) | 960 | 43.67 | 45.5 | 23.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 37 | access | bar0:R:0x306000:- | NV_PROM_DATA (VBIOS window) | 960 | 38.17 | 39.8 | 21.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 38 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 229 | 36.30 | 158.5 | 1102.1 | 0x0=229 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | access | bar0:R:0x305000:- | NV_PROM_DATA (VBIOS window) | 960 | 36.17 | 37.7 | 23.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 40 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 43 | 35.66 | 829.2 | 1838.9 | 0x0=43 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 133 | 34.17 | 256.9 | 20802.6 | 0x0=133 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 42 | window | bar1 pool: page moved out, cold (units: bytes) | bar1 pool: page moved out, cold (units: bytes) | 2560 | 33.65 | 13.1 | 1181.5 | - | - | - | bar1pool.rs | - |
| 43 | access | bar0:W:0x70e000:- | NV_PRAMIN | 960 | 32.31 | 33.7 | 156.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 44 | access | bar0:W:0x70d000:- | NV_PRAMIN | 1000 | 31.46 | 31.5 | 15.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 45 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 600 | 31.02 | 51.7 | 32.8 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 46 | access | bar0:R:0x303000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.42 | 31.7 | 12.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 47 | access | bar0:W:0x714000:- | NV_PRAMIN | 960 | 29.96 | 31.2 | 14.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 48 | access | bar0:W:0x70f000:- | NV_PRAMIN | 960 | 29.92 | 31.2 | 26.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 49 | access | bar0:W:0x713000:- | NV_PRAMIN | 960 | 29.76 | 31.0 | 22.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 50 | access | bar0:R:0x304000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.49 | 30.7 | 31.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 51 | access | bar0:W:0x710000:- | NV_PRAMIN | 960 | 29.28 | 30.5 | 18.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 52 | access | bar0:W:0x715000:- | NV_PRAMIN | 960 | 28.88 | 30.1 | 14.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 53 | access | bar0:W:0x711000:- | NV_PRAMIN | 960 | 28.83 | 30.0 | 16.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 54 | access | bar0:R:0x302000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.82 | 30.0 | 20.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 55 | access | bar0:W:0x712000:- | NV_PRAMIN | 960 | 28.79 | 30.0 | 15.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 56 | access | bar0:W:0x70b000:- | NV_PRAMIN | 960 | 28.74 | 29.9 | 16.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 57 | access | cfg:R:0x0:0x78 | - | 1883 | 28.73 | 15.3 | 512.0 | - | pci-config | - | config.rs | - |
| 58 | access | cfg:R:0x0:0x60 | - | 1885 | 28.61 | 15.2 | 42.5 | - | pci-config | - | config.rs | - |
| 59 | access | bar0:W:0x70a000:- | NV_PRAMIN | 960 | 28.49 | 29.7 | 13.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:W:0x70c000:- | NV_PRAMIN | 960 | 28.26 | 29.4 | 23.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 61 | access | cfg:R:0x0:0x68 | - | 1893 | 28.21 | 14.9 | 60.9 | - | pci-config | - | config.rs | - |
| 62 | access | bar0:W:0x709000:- | NV_PRAMIN | 960 | 28.08 | 29.2 | 11.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 63 | access | bar0:R:0x30c000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.88 | 29.0 | 10.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 64 | access | bar0:R:0x319000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.74 | 28.9 | 13.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 65 | access | bar0:W:0x708000:- | NV_PRAMIN | 960 | 27.72 | 28.9 | 11.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 66 | access | bar0:W:0x707000:- | NV_PRAMIN | 960 | 27.68 | 28.8 | 23.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 67 | access | bar0:R:0x31d000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.45 | 28.6 | 20.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 68 | access | bar0:W:0x706000:- | NV_PRAMIN | 960 | 27.37 | 28.5 | 24.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 69 | shadow | gather free | gather free | 334 | 27.16 | 81.3 | 177.3 | - | - | - | shadow.rs (worker) | - |
| 70 | access | bar0:R:0x318000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.75 | 27.9 | 15.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 71 | access | bar0:W:0x702000:- | NV_PRAMIN | 960 | 26.58 | 27.7 | 13.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 72 | access | bar0:R:0x30f000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.39 | 27.5 | 10.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 73 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 15 | 26.23 | 1749.0 | 1828.8 | 0x0=15 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 74 | access | bar0:R:0x314000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.22 | 27.3 | 13.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 75 | access | bar0:R:0x30e000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.20 | 27.3 | 19.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 76 | access | bar0:R:0x309000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.15 | 27.2 | 15.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 77 | access | bar0:R:0x308000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.74 | 26.8 | 19.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 78 | access | bar0:R:0x30b000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.68 | 26.7 | 12.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 79 | access | bar0:R:0x30d000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.53 | 26.6 | 9.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 80 | access | bar0:R:0x30a000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.46 | 26.5 | 12.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 3555 | 169559 | 10442.86 |
| host | 66 | 70786 | 6021.90 |
| shadow | 6 | 66581 | 2890.99 |
| window | 23 | 101588 | 1975.62 |
| rpc | 7 | 2918 | 521.81 |
| control | 68 | 751 | 92.08 |
| alloc | 17 | 1093 | 4.77 |
