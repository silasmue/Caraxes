# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new-census-user/profile.1.tsv, scope device, cost guest

3747 rows, 30840.3 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 39203 | 4240.91 | 108.2 | 7928.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 1890.53 | 190.8 | 119341.4 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 13185 | 1890.17 | 143.4 | 11475.9 | - | - | - | bar2area.rs | - |
| 4 | access | bar3:R:0xfea000:- | - | 19805 | 1876.08 | 94.7 | 10330.7 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 5 | window | bar2 memfd: sync scan (units: pages compared) | bar2 memfd: sync scan (units: pages compared) | 13185 | 1839.46 | 139.5 | 11474.2 | - | - | - | bar2area.rs | - |
| 6 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 656 | 1631.32 | 2486.8 | 373205.9 | 0x0=656 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | host | 0x29:0x0 | RM_FREE | 15715 | 1181.38 | 75.2 | 35304.0 | 0x0=15715 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | shadow | map | map | 19147 | 1178.84 | 61.6 | 13299.1 | - | - | - | shadow.rs (worker) | - |
| 9 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19147 | 1157.45 | 60.5 | 13297.4 | 0x0=19147 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 2920 | 1033.06 | 353.8 | 10983.0 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 11 | shadow | gather alloc | gather alloc | 348 | 1020.92 | 2933.7 | 9932.6 | - | - | - | shadow.rs (worker) | - |
| 12 | shadow | object alloc | object alloc | 14269 | 971.42 | 68.1 | 16574.9 | - | - | - | shadow.rs (worker) | - |
| 13 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14269 | 965.39 | 67.7 | 16572.6 | 0x0=14269 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | shadow | unmap | unmap | 18478 | 834.06 | 45.1 | 10058.2 | - | - | - | shadow.rs (worker) | - |
| 15 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18478 | 828.97 | 44.9 | 10057.8 | 0x0=18478 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | shadow | object free | object free | 14284 | 566.15 | 39.6 | 11381.5 | - | - | - | shadow.rs (worker) | - |
| 17 | rpc | 0xa | FREE | 1068 | 541.74 | 507.2 | 4258.2 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 18 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 525.51 | 53.0 | 17387.1 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 19 | window | gather: window of mremaps (units: runs) | gather: window of mremaps (units: runs) | 348 | 484.51 | 1392.3 | 4013.8 | - | - | - | - | - |
| 20 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 301 | 446.59 | 1483.7 | 3393.6 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 21 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 441.14 | 44.5 | 2421.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 22 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 7680 | 328.99 | 42.8 | 1944.6 | - | - | - | bar1pool.rs | - |
| 23 | window | gather: host descriptor (units: pages) | gather: host descriptor (units: pages) | 348 | 286.38 | 822.9 | 9631.2 | - | - | - | - | - |
| 24 | window | gather: window unmap (units: pages) | gather: window unmap (units: pages) | 348 | 248.73 | 714.7 | 2233.1 | - | - | - | - | - |
| 25 | access | bar0:W:0x840000:0x840184 | NV_PSEC (SEC2 falcon) | 8512 | 242.53 | 28.5 | 50.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 26 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 7680 | 232.14 | 30.2 | 2495.9 | - | - | - | bar1pool.rs | - |
| 27 | access | bar0:W:0x840000:0x8401c4 | NV_PSEC (SEC2 falcon) | 6272 | 204.92 | 32.7 | 47.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 28 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 300 | 188.74 | 629.1 | 227838.4 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 29 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 2918 | 169.44 | 58.1 | 1436.1 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 30 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 120 | 153.71 | 1280.9 | 2861.0 | 0x0=120 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 28 | 151.66 | 5416.6 | 13519.3 | 0x0=28 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 2920 | 148.18 | 50.7 | 6865.7 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 33 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 2919 | 127.85 | 43.8 | 2225.9 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 34 | rpc | 0x4c | GSP_RM_CONTROL | 751 | 102.55 | 136.5 | 10928.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 35 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 301 | 85.27 | 283.3 | 3935.2 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | access | bar0:W:0x1000:0x1478 | NV_PBUS_SW_SCRATCH(i) | 568 | 81.18 | 142.9 | 106.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PBUS_SW_SCRATCH(i) |
| 37 | access | bar0:R:0x31c000:- | NV_PROM_DATA (VBIOS window) | 960 | 62.69 | 65.3 | 29.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 38 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 61 | 61.05 | 1000.8 | 2794.6 | 0x0=61 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 301 | 60.15 | 199.8 | 1023.7 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 40 | access | bar0:R:0x30a000:- | NV_PROM_DATA (VBIOS window) | 960 | 57.48 | 59.9 | 37.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 41 | access | cfg:R:0x0:0x4 | - | 975 | 57.12 | 58.6 | 1564.6 | - | pci-config | - | config.rs | - |
| 42 | shadow | gather free | gather free | 348 | 49.93 | 143.5 | 5185.6 | - | - | - | shadow.rs (worker) | - |
| 43 | access | bar0:W:0x70d000:- | NV_PRAMIN | 1000 | 42.70 | 42.7 | 28.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 44 | access | cfg:R:0x0:0x60 | - | 1885 | 39.38 | 20.9 | 2194.1 | - | pci-config | - | config.rs | - |
| 45 | access | bar0:R:0x307000:- | NV_PROM_DATA (VBIOS window) | 960 | 37.84 | 39.4 | 30.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 46 | access | cfg:R:0x0:0x78 | - | 1883 | 37.76 | 20.1 | 876.1 | - | pci-config | - | config.rs | - |
| 47 | access | bar0:W:0x714000:- | NV_PRAMIN | 960 | 36.17 | 37.7 | 23.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 48 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 600 | 36.08 | 60.1 | 1445.0 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 49 | access | bar0:R:0x30b000:- | NV_PROM_DATA (VBIOS window) | 960 | 35.81 | 37.3 | 34.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 50 | access | bar0:R:0x318000:- | NV_PROM_DATA (VBIOS window) | 960 | 34.26 | 35.7 | 14.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 51 | access | bar0:W:0x707000:- | NV_PRAMIN | 960 | 34.22 | 35.6 | 18.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 52 | access | bar0:R:0x309000:- | NV_PROM_DATA (VBIOS window) | 960 | 33.54 | 34.9 | 18.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 53 | access | cfg:R:0x0:0x68 | - | 1893 | 32.14 | 17.0 | 314.9 | - | pci-config | - | config.rs | - |
| 54 | access | bar0:W:0x706000:- | NV_PRAMIN | 960 | 31.75 | 33.1 | 41.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 55 | access | bar0:W:0x711000:- | NV_PRAMIN | 960 | 31.73 | 33.1 | 159.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 56 | access | bar0:R:0x319000:- | NV_PROM_DATA (VBIOS window) | 960 | 31.32 | 32.6 | 33.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 57 | access | bar0:R:0x31b000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.91 | 32.2 | 37.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 58 | access | bar0:R:0x305000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.70 | 32.0 | 11.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 59 | access | bar0:W:0x708000:- | NV_PRAMIN | 960 | 30.36 | 31.6 | 26.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:R:0x316000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.33 | 31.6 | 12.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 61 | access | bar0:R:0x300000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.99 | 31.2 | 11.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 62 | access | bar0:R:0x31e000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.62 | 30.9 | 22.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 63 | access | bar0:W:0x712000:- | NV_PRAMIN | 960 | 29.56 | 30.8 | 49.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 64 | access | bar0:R:0x302000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.55 | 30.8 | 7.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 65 | access | bar0:R:0x303000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.40 | 30.6 | 13.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 66 | access | bar0:W:0x70f000:- | NV_PRAMIN | 960 | 29.40 | 30.6 | 23.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 67 | access | bar0:R:0x31d000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.14 | 30.3 | 22.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 68 | access | bar0:R:0x30e000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.12 | 30.3 | 12.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 69 | access | bar0:R:0x301000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.09 | 30.3 | 21.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 70 | access | bar0:R:0x304000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.97 | 30.2 | 13.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 71 | access | bar0:R:0x311000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.95 | 30.2 | 11.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 72 | access | bar0:R:0x310000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.93 | 30.1 | 11.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 73 | access | bar0:R:0x31f000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.88 | 30.1 | 10.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 74 | access | bar0:R:0x30f000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.74 | 29.9 | 16.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 75 | access | bar0:R:0x317000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.73 | 29.9 | 18.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 76 | access | bar0:R:0x31a000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.53 | 29.7 | 12.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 77 | access | bar0:R:0x30c000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.46 | 29.6 | 9.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 78 | access | bar0:R:0x315000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.19 | 29.4 | 11.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 79 | access | bar0:W:0x702000:- | NV_PRAMIN | 960 | 28.15 | 29.3 | 16.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 80 | access | bar0:W:0x713000:- | NV_PRAMIN | 960 | 28.12 | 29.3 | 53.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 3555 | 176883 | 13134.40 |
| host | 66 | 76528 | 6971.16 |
| window | 28 | 171606 | 5344.91 |
| shadow | 6 | 66874 | 4621.31 |
| rpc | 7 | 2918 | 664.58 |
| control | 68 | 751 | 98.98 |
| alloc | 17 | 1093 | 4.98 |
