# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new-census/profile.1.tsv, scope device, cost guest

3747 rows, 24993.4 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 32274 | 2738.26 | 84.8 | 2352.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 638 | 2313.47 | 3626.1 | 856114.9 | 0x0=638 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 13089 | 1667.86 | 127.4 | 19818.2 | - | - | - | bar2area.rs | - |
| 4 | window | bar2 memfd: sync scan (units: pages compared) | bar2 memfd: sync scan (units: pages compared) | 13089 | 1608.57 | 122.9 | 19817.0 | - | - | - | bar2area.rs | - |
| 5 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 1591.13 | 160.6 | 123830.3 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 6 | access | bar3:R:0xfea000:- | - | 19805 | 1541.16 | 77.8 | 19836.6 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 7 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 2920 | 966.60 | 331.0 | 10131.7 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 8 | host | 0x29:0x0 | RM_FREE | 15697 | 957.56 | 61.0 | 32877.5 | 0x0=15697 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 9 | shadow | gather alloc | gather alloc | 330 | 925.62 | 2804.9 | 8401.3 | - | - | - | shadow.rs (worker) | - |
| 10 | shadow | map | map | 19181 | 533.72 | 27.8 | 5088.5 | - | - | - | shadow.rs (worker) | - |
| 11 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19181 | 527.28 | 27.5 | 5087.0 | 0x0=19181 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 12 | shadow | unmap | unmap | 18454 | 514.04 | 27.9 | 9715.3 | - | - | - | shadow.rs (worker) | - |
| 13 | shadow | object alloc | object alloc | 14269 | 511.49 | 35.8 | 5046.0 | - | - | - | shadow.rs (worker) | - |
| 14 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18454 | 510.52 | 27.7 | 9714.6 | 0x0=18454 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 15 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14269 | 506.22 | 35.5 | 5045.1 | 0x0=14269 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | rpc | 0xa | FREE | 1068 | 503.79 | 471.7 | 10086.3 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 17 | window | gather: window of mremaps (units: runs) | gather: window of mremaps (units: runs) | 330 | 458.70 | 1390.0 | 4776.1 | - | - | - | - | - |
| 18 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 301 | 431.81 | 1434.6 | 5416.2 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 426.24 | 43.0 | 14508.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 20 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 7680 | 418.29 | 54.5 | 3071.2 | - | - | - | bar1pool.rs | - |
| 21 | shadow | object free | object free | 14284 | 397.96 | 27.9 | 5546.8 | - | - | - | shadow.rs (worker) | - |
| 22 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 377.58 | 38.1 | 7960.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 23 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 7679 | 356.35 | 46.4 | 2224.3 | - | - | - | bar1pool.rs | - |
| 24 | window | gather: host descriptor (units: pages) | gather: host descriptor (units: pages) | 330 | 233.94 | 708.9 | 4071.5 | - | - | - | - | - |
| 25 | window | gather: window unmap (units: pages) | gather: window unmap (units: pages) | 330 | 231.88 | 702.7 | 4344.5 | - | - | - | - | - |
| 26 | access | bar0:W:0x840000:0x840184 | NV_PSEC (SEC2 falcon) | 8512 | 225.61 | 26.5 | 33.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 27 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 300 | 172.38 | 574.6 | 87109.6 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 28 | access | bar0:W:0x840000:0x8401c4 | NV_PSEC (SEC2 falcon) | 6272 | 170.38 | 27.2 | 34.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 29 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 2918 | 150.18 | 51.5 | 84.3 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 30 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 120 | 145.99 | 1216.6 | 6463.5 | 0x0=120 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 2920 | 130.36 | 44.6 | 4426.1 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 32 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 25 | 123.24 | 4929.7 | 10180.4 | 0x0=25 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 2919 | 119.75 | 41.0 | 2342.3 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 34 | access | bar0:W:0x1000:0x1478 | NV_PBUS_SW_SCRATCH(i) | 568 | 84.53 | 148.8 | 383.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PBUS_SW_SCRATCH(i) |
| 35 | rpc | 0x4c | GSP_RM_CONTROL | 751 | 80.27 | 106.9 | 3621.8 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 36 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 301 | 68.75 | 228.4 | 3638.4 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 61 | 58.42 | 957.7 | 3767.7 | 0x0=61 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 301 | 52.64 | 174.9 | 2168.4 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | access | bar0:R:0x31f000:- | NV_PROM_DATA (VBIOS window) | 960 | 50.21 | 52.3 | 54.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 40 | access | cfg:R:0x0:0x4 | - | 975 | 43.83 | 45.0 | 66.4 | - | pci-config | - | config.rs | - |
| 41 | access | bar0:R:0x31e000:- | NV_PROM_DATA (VBIOS window) | 960 | 38.69 | 40.3 | 307.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 42 | access | cfg:R:0x0:0x68 | - | 1893 | 35.61 | 18.8 | 1903.5 | - | pci-config | - | config.rs | - |
| 43 | access | bar0:R:0x310000:- | NV_PROM_DATA (VBIOS window) | 960 | 35.52 | 37.0 | 20.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 44 | access | bar0:R:0x30c000:- | NV_PROM_DATA (VBIOS window) | 960 | 34.10 | 35.5 | 26.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 45 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 600 | 33.94 | 56.6 | 1809.9 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 46 | access | bar0:R:0x307000:- | NV_PROM_DATA (VBIOS window) | 960 | 33.69 | 35.1 | 22.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 47 | access | bar0:R:0x311000:- | NV_PROM_DATA (VBIOS window) | 960 | 33.66 | 35.1 | 25.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 48 | access | bar0:W:0x70c000:- | NV_PRAMIN | 960 | 31.99 | 33.3 | 29.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 49 | access | bar0:W:0x70b000:- | NV_PRAMIN | 960 | 31.98 | 33.3 | 35.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 50 | access | bar0:R:0x305000:- | NV_PROM_DATA (VBIOS window) | 960 | 31.37 | 32.7 | 12.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 51 | access | bar0:R:0x300000:- | NV_PROM_DATA (VBIOS window) | 960 | 31.13 | 32.4 | 13.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 52 | access | bar0:W:0x70d000:- | NV_PRAMIN | 1000 | 31.10 | 31.1 | 22.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 53 | access | cfg:R:0x0:0x78 | - | 1883 | 30.58 | 16.2 | 1964.0 | - | pci-config | - | config.rs | - |
| 54 | access | bar0:W:0x715000:- | NV_PRAMIN | 960 | 30.51 | 31.8 | 14.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 55 | access | cfg:R:0x0:0x60 | - | 1885 | 30.44 | 16.1 | 137.5 | - | pci-config | - | config.rs | - |
| 56 | shadow | gather free | gather free | 330 | 30.32 | 91.9 | 1107.5 | - | - | - | shadow.rs (worker) | - |
| 57 | access | bar0:R:0x303000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.30 | 31.6 | 14.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 58 | access | bar0:R:0x31d000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.16 | 31.4 | 14.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 59 | access | bar0:W:0x70f000:- | NV_PRAMIN | 960 | 30.08 | 31.3 | 33.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:W:0x709000:- | NV_PRAMIN | 960 | 29.74 | 31.0 | 28.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 61 | access | bar0:R:0x304000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.70 | 30.9 | 13.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 62 | access | bar0:W:0x70a000:- | NV_PRAMIN | 960 | 29.42 | 30.6 | 12.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 63 | access | bar0:W:0x713000:- | NV_PRAMIN | 960 | 29.40 | 30.6 | 15.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 64 | access | bar0:W:0x70e000:- | NV_PRAMIN | 960 | 29.30 | 30.5 | 32.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 65 | access | bar0:W:0x711000:- | NV_PRAMIN | 960 | 29.20 | 30.4 | 34.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 66 | access | bar0:W:0x710000:- | NV_PRAMIN | 960 | 28.98 | 30.2 | 25.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 67 | access | bar0:R:0x30f000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.97 | 30.2 | 18.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 68 | access | bar0:W:0x712000:- | NV_PRAMIN | 960 | 28.90 | 30.1 | 12.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 69 | access | bar0:W:0x714000:- | NV_PRAMIN | 960 | 28.69 | 29.9 | 15.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 70 | access | bar0:R:0x306000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.59 | 29.8 | 8.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 71 | access | bar0:W:0x707000:- | NV_PRAMIN | 960 | 28.38 | 29.6 | 26.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 72 | access | bar0:W:0x708000:- | NV_PRAMIN | 960 | 28.29 | 29.5 | 12.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 73 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 15 | 28.17 | 1878.0 | 3616.9 | 0x0=15 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 74 | access | bar0:R:0x319000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.70 | 28.9 | 379.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 75 | access | bar0:R:0x30e000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.57 | 28.7 | 14.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 76 | access | bar0:R:0x308000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.28 | 28.4 | 27.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 77 | access | bar0:R:0x30d000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.08 | 28.2 | 13.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 78 | access | bar0:R:0x316000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.81 | 27.9 | 15.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 79 | access | bar0:R:0x301000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.40 | 27.5 | 31.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 80 | access | bar0:R:0x31a000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.06 | 27.1 | 14.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 3555 | 169954 | 10507.59 |
| host | 66 | 71127 | 5871.40 |
| window | 28 | 126446 | 5014.53 |
| shadow | 6 | 66848 | 2913.15 |
| rpc | 7 | 2918 | 603.97 |
| control | 68 | 751 | 77.09 |
| alloc | 17 | 1093 | 5.72 |
