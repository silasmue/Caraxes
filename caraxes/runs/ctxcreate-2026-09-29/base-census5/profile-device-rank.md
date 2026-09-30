# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-census5/profile.1.tsv, scope device, cost guest

3742 rows, 20208.7 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 32824 | 2814.52 | 85.7 | 70503.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 576 | 1657.68 | 2877.9 | 429580.4 | 0x0=576 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 3 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 1555.45 | 157.0 | 105080.1 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 4 | access | bar3:R:0xfea000:- | - | 19805 | 1501.61 | 75.8 | 77408.4 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 5 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 12961 | 1494.21 | 115.3 | 10981.4 | - | - | - | bar2area.rs | - |
| 6 | shadow | gather alloc | gather alloc | 341 | 905.60 | 2655.7 | 8960.2 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x29:0x0 | RM_FREE | 15473 | 839.05 | 54.2 | 26528.7 | 0x0=15473 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 2920 | 838.06 | 287.0 | 4790.5 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 9 | shadow | map | map | 19141 | 579.27 | 30.3 | 3936.3 | - | - | - | shadow.rs (worker) | - |
| 10 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19141 | 573.02 | 29.9 | 3935.6 | 0x0=19141 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 11 | shadow | object alloc | object alloc | 14269 | 529.94 | 37.1 | 4939.8 | - | - | - | shadow.rs (worker) | - |
| 12 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14269 | 521.73 | 36.6 | 4938.7 | 0x0=14269 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | shadow | unmap | unmap | 18256 | 472.42 | 25.9 | 8043.4 | - | - | - | shadow.rs (worker) | - |
| 14 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18256 | 468.51 | 25.7 | 8042.7 | 0x0=18256 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | shadow | object free | object free | 14284 | 433.44 | 30.3 | 8498.7 | - | - | - | shadow.rs (worker) | - |
| 16 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 404.31 | 40.8 | 2334.8 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 17 | rpc | 0xa | FREE | 1068 | 374.99 | 351.1 | 3343.0 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 18 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 363.20 | 36.7 | 8068.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 19 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 229 | 316.44 | 1381.8 | 2829.3 | 0x0=229 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 7680 | 219.80 | 28.6 | 23492.4 | - | - | - | bar1pool.rs | - |
| 21 | access | bar0:W:0x840000:0x840184 | NV_PSEC (SEC2 falcon) | 8512 | 204.70 | 24.0 | 20.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 22 | access | bar0:W:0x840000:0x8401c4 | NV_PSEC (SEC2 falcon) | 6272 | 150.04 | 23.9 | 11.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 23 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 120 | 142.05 | 1183.7 | 2167.8 | 0x0=120 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 24 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 2918 | 125.44 | 43.0 | 71.4 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 25 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 25 | 117.11 | 4684.5 | 42437.4 | 0x0=25 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 26 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 300 | 106.38 | 354.6 | 132175.1 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 27 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 2920 | 105.81 | 36.2 | 60.2 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 28 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 2919 | 100.57 | 34.5 | 1854.1 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 29 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 5119 | 83.93 | 16.4 | 1264.0 | - | - | - | bar1pool.rs | - |
| 30 | rpc | 0x4c | GSP_RM_CONTROL | 751 | 82.90 | 110.4 | 4562.7 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 31 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 229 | 47.68 | 208.2 | 1358.8 | 0x0=229 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | access | bar0:W:0x1000:0x1478 | NV_PBUS_SW_SCRATCH(i) | 568 | 43.54 | 76.7 | 2409.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PBUS_SW_SCRATCH(i) |
| 33 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 229 | 39.44 | 172.2 | 3673.2 | 0x0=229 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | access | cfg:R:0x0:0x4 | - | 975 | 39.30 | 40.3 | 2102.3 | - | pci-config | - | config.rs | - |
| 35 | window | bar1 pool: page moved out, cold (units: bytes) | bar1 pool: page moved out, cold (units: bytes) | 2561 | 37.80 | 14.8 | 1296.0 | - | - | - | bar1pool.rs | - |
| 36 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 133 | 35.47 | 266.7 | 22548.0 | 0x0=133 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 43 | 34.98 | 813.5 | 2001.7 | 0x0=43 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | access | bar0:W:0x702000:- | NV_PRAMIN | 960 | 32.03 | 33.4 | 53.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 39 | access | bar0:W:0x706000:- | NV_PRAMIN | 960 | 31.39 | 32.7 | 14.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 40 | access | bar0:W:0x707000:- | NV_PRAMIN | 960 | 30.65 | 31.9 | 12.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 41 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 600 | 30.12 | 50.2 | 18.8 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 42 | access | bar0:W:0x70d000:- | NV_PRAMIN | 1000 | 29.28 | 29.3 | 62.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 43 | access | bar0:W:0x708000:- | NV_PRAMIN | 960 | 29.13 | 30.3 | 13.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 44 | shadow | gather free | gather free | 341 | 28.98 | 85.0 | 202.2 | - | - | - | shadow.rs (worker) | - |
| 45 | access | bar0:W:0x714000:- | NV_PRAMIN | 960 | 28.62 | 29.8 | 30.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 46 | access | cfg:R:0x0:0x68 | - | 1893 | 28.37 | 15.0 | 1192.5 | - | pci-config | - | config.rs | - |
| 47 | access | bar0:W:0x715000:- | NV_PRAMIN | 960 | 28.36 | 29.5 | 13.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 48 | host | 0x4e:0x0 | RM_MAP_MEMORY | 137 | 28.35 | 206.9 | 9585.8 | 0x0=137 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 49 | access | bar0:W:0x713000:- | NV_PRAMIN | 960 | 28.32 | 29.5 | 32.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 50 | access | bar0:W:0x711000:- | NV_PRAMIN | 960 | 28.04 | 29.2 | 10.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 51 | access | bar0:W:0x70e000:- | NV_PRAMIN | 960 | 28.00 | 29.2 | 15.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 52 | access | bar0:W:0x709000:- | NV_PRAMIN | 960 | 27.96 | 29.1 | 29.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 53 | access | bar0:W:0x710000:- | NV_PRAMIN | 960 | 27.92 | 29.1 | 13.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 54 | access | bar0:W:0x70f000:- | NV_PRAMIN | 960 | 27.86 | 29.0 | 20.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 55 | access | bar0:W:0x712000:- | NV_PRAMIN | 960 | 27.79 | 28.9 | 14.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 56 | access | bar0:W:0x70c000:- | NV_PRAMIN | 960 | 27.74 | 28.9 | 12.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 57 | access | bar0:W:0x70a000:- | NV_PRAMIN | 960 | 27.69 | 28.8 | 19.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 58 | access | cfg:R:0x0:0x60 | - | 1885 | 27.61 | 14.6 | 26.7 | - | pci-config | - | config.rs | - |
| 59 | access | bar0:W:0x70b000:- | NV_PRAMIN | 960 | 27.54 | 28.7 | 34.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | cfg:R:0x0:0x78 | - | 1883 | 26.52 | 14.1 | 658.2 | - | pci-config | - | config.rs | - |
| 61 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 15 | 25.97 | 1731.2 | 2029.2 | 0x0=15 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 62 | access | bar0:R:0x30e000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.44 | 26.5 | 12.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 63 | access | bar0:R:0x30b000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.15 | 26.2 | 12.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 64 | access | bar0:R:0x304000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.11 | 26.2 | 12.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 65 | access | bar0:R:0x308000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.73 | 25.8 | 10.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 66 | access | bar0:R:0x30f000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.65 | 25.7 | 13.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 67 | access | bar0:R:0x301000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.62 | 25.6 | 20.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 68 | access | bar0:R:0x303000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.62 | 25.6 | 14.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 69 | access | bar0:R:0x300000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.44 | 25.5 | 78.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 70 | access | bar0:R:0x305000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.39 | 25.4 | 13.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 71 | access | bar0:R:0x306000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.29 | 25.3 | 11.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 72 | access | bar0:R:0x30a000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.16 | 25.2 | 10.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 73 | access | bar0:R:0x318000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.12 | 25.1 | 7.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 74 | access | bar0:R:0x31b000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.11 | 25.1 | 10.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 75 | access | bar0:R:0x31f000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.11 | 25.1 | 18.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 76 | access | bar0:R:0x30c000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.05 | 25.1 | 9.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 77 | access | bar0:R:0x302000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.00 | 25.0 | 19.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 78 | access | bar0:R:0x317000:- | NV_PROM_DATA (VBIOS window) | 960 | 23.95 | 24.9 | 9.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 79 | access | bar0:R:0x31d000:- | NV_PROM_DATA (VBIOS window) | 960 | 23.92 | 24.9 | 10.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 80 | access | bar0:R:0x31e000:- | NV_PROM_DATA (VBIOS window) | 960 | 23.92 | 24.9 | 6.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 3555 | 170504 | 9910.09 |
| host | 66 | 70183 | 4955.85 |
| shadow | 6 | 66632 | 2949.65 |
| window | 23 | 102210 | 1835.74 |
| rpc | 7 | 2918 | 474.33 |
| control | 68 | 751 | 79.45 |
| alloc | 17 | 1093 | 3.58 |
