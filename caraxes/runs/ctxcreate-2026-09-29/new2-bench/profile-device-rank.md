# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new2-bench/profile.1.tsv, scope device, cost guest

3746 rows, 26267.8 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 35199 | 2903.97 | 82.5 | 9688.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 14592 | 1845.17 | 126.5 | 20990.4 | - | - | - | bar2area.rs | - |
| 3 | window | bar2 memfd: sync scan (units: pages compared) | bar2 memfd: sync scan (units: pages compared) | 14592 | 1804.87 | 123.7 | 20989.4 | - | - | - | bar2area.rs | - |
| 4 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 1754.97 | 164.1 | 62979.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 5 | access | bar3:R:0xfea000:- | - | 22069 | 1650.45 | 74.8 | 21012.2 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 6 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 870 | 1353.73 | 1556.0 | 361600.7 | 0x0=870 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | shadow | gather alloc | gather alloc | 463 | 1333.86 | 2880.9 | 18768.4 | - | - | - | shadow.rs (worker) | - |
| 8 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 3762 | 1222.17 | 324.9 | 10568.9 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 9 | host | 0x29:0x0 | RM_FREE | 16624 | 1117.77 | 67.2 | 21354.4 | 0x0=16624 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | window | gather: window of mremaps (units: runs) | gather: window of mremaps (units: runs) | 463 | 672.25 | 1451.9 | 7373.0 | - | - | - | - | - |
| 11 | rpc | 0xa | FREE | 1405 | 668.03 | 475.5 | 9147.5 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 12 | shadow | map | map | 19634 | 603.21 | 30.7 | 15317.9 | - | - | - | shadow.rs (worker) | - |
| 13 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19634 | 588.06 | 30.0 | 6587.8 | 0x0=19634 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | shadow | unmap | unmap | 18767 | 561.86 | 29.9 | 11938.4 | - | - | - | shadow.rs (worker) | - |
| 15 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18767 | 557.90 | 29.7 | 11936.8 | 0x0=18767 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | shadow | object alloc | object alloc | 14699 | 549.59 | 37.4 | 5364.8 | - | - | - | shadow.rs (worker) | - |
| 17 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14699 | 543.98 | 37.0 | 5364.4 | 0x0=14699 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 18 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 401 | 467.80 | 1166.6 | 3777.6 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | shadow | object free | object free | 14719 | 435.34 | 29.6 | 3926.4 | - | - | - | shadow.rs (worker) | - |
| 20 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 399.54 | 37.4 | 1949.2 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 21 | window | gather: window unmap (units: pages) | gather: window unmap (units: pages) | 463 | 340.78 | 736.0 | 10667.9 | - | - | - | - | - |
| 22 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 330.82 | 30.9 | 7463.5 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 23 | window | gather: host descriptor (units: pages) | gather: host descriptor (units: pages) | 463 | 319.12 | 689.3 | 6819.6 | - | - | - | - | - |
| 24 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 160 | 273.45 | 1709.1 | 7141.5 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 25 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 400 | 223.56 | 558.9 | 89465.6 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 26 | access | bar0:W:0x840000:0x840184 | NV_PSEC (SEC2 falcon) | 8512 | 206.75 | 24.3 | 16.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 27 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 3760 | 164.35 | 43.7 | 45.7 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 28 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 10240 | 150.13 | 14.7 | 6628.7 | - | - | - | bar1pool.rs | - |
| 29 | access | bar0:W:0x840000:0x8401c4 | NV_PSEC (SEC2 falcon) | 6272 | 149.51 | 23.8 | 5.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 30 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 3762 | 145.96 | 38.8 | 69.2 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 31 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 10239 | 138.70 | 13.5 | 3032.2 | - | - | - | bar1pool.rs | - |
| 32 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 3761 | 130.13 | 34.6 | 27.4 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 33 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 401 | 116.80 | 291.3 | 9099.9 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 23 | 114.06 | 4959.3 | 11071.2 | 0x0=23 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | rpc | 0x4c | GSP_RM_CONTROL | 919 | 110.94 | 120.7 | 5951.8 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 36 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 401 | 72.13 | 179.9 | 3345.9 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 81 | 70.23 | 867.0 | 2374.6 | 0x0=81 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | access | bar0:R:0x311000:- | NV_PROM_DATA (VBIOS window) | 960 | 49.45 | 51.5 | 0.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 39 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 800 | 43.26 | 54.1 | 12.7 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 40 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 20 | 40.63 | 2031.4 | 5945.9 | 0x0=20 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 41 | access | bar0:R:0x301000:- | NV_PROM_DATA (VBIOS window) | 960 | 39.29 | 40.9 | 2.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 42 | access | bar0:W:0x712000:- | NV_PRAMIN | 960 | 39.15 | 40.8 | 20.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 43 | shadow | gather free | gather free | 463 | 37.80 | 81.6 | 2101.6 | - | - | - | shadow.rs (worker) | - |
| 44 | access | bar0:R:0x31d000:- | NV_PROM_DATA (VBIOS window) | 960 | 35.18 | 36.6 | 1.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 45 | access | bar0:R:0x312000:- | NV_PROM_DATA (VBIOS window) | 960 | 34.65 | 36.1 | 1.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 46 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 84 | 34.49 | 410.6 | 4259.9 | 0x0=84 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | access | bar0:R:0x300000:- | NV_PROM_DATA (VBIOS window) | 960 | 33.99 | 35.4 | 0.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 48 | access | bar0:R:0x30e000:- | NV_PROM_DATA (VBIOS window) | 960 | 33.39 | 34.8 | 2.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 49 | access | bar0:R:0x30c000:- | NV_PROM_DATA (VBIOS window) | 960 | 32.37 | 33.7 | 0.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 50 | access | bar0:R:0x310000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.46 | 31.7 | 6.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 51 | access | cfg:R:0x0:0x4 | - | 859 | 30.23 | 35.2 | 11.1 | - | pci-config | - | config.rs | - |
| 52 | access | bar0:R:0x30f000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.76 | 31.0 | 0.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 53 | access | bar0:R:0x308000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.74 | 31.0 | 0.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 54 | access | bar0:R:0x309000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.45 | 30.7 | 5.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 55 | access | bar0:R:0x30b000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.24 | 30.5 | 1.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 56 | access | bar0:R:0x306000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.99 | 30.2 | 0.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 57 | access | bar0:W:0x715000:- | NV_PRAMIN | 960 | 28.79 | 30.0 | 16.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 58 | access | bar0:R:0x307000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.75 | 30.0 | 0.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 59 | access | bar0:W:0x70a000:- | NV_PRAMIN | 960 | 28.61 | 29.8 | 65.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:R:0x314000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.55 | 29.7 | 0.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 61 | access | bar0:W:0x70d000:- | NV_PRAMIN | 1000 | 28.49 | 28.5 | 5.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 62 | access | bar0:W:0x709000:- | NV_PRAMIN | 960 | 28.33 | 29.5 | 5.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 63 | access | bar0:R:0x31e000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.15 | 29.3 | 13.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 64 | access | bar0:R:0x313000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.79 | 28.9 | 0.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 65 | access | bar0:W:0x707000:- | NV_PRAMIN | 960 | 27.77 | 28.9 | 7.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 66 | access | bar0:W:0x708000:- | NV_PRAMIN | 960 | 27.30 | 28.4 | 4.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 67 | access | bar0:W:0x70f000:- | NV_PRAMIN | 960 | 27.30 | 28.4 | 8.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 68 | access | bar0:W:0x70c000:- | NV_PRAMIN | 960 | 27.20 | 28.3 | 10.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 69 | access | bar0:W:0x70b000:- | NV_PRAMIN | 960 | 27.14 | 28.3 | 9.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 70 | access | bar0:R:0x305000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.09 | 28.2 | 3.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 71 | access | bar0:W:0x714000:- | NV_PRAMIN | 960 | 26.81 | 27.9 | 10.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 72 | access | bar0:R:0x30d000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.66 | 27.8 | 7.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 73 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 21 | 26.50 | 1261.9 | 1535.6 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 74 | access | bar0:R:0x315000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.35 | 27.5 | 0.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 75 | access | bar0:R:0x30a000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.17 | 27.3 | 0.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 76 | access | bar0:R:0x31f000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.13 | 27.2 | 0.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 77 | access | bar0:W:0x713000:- | NV_PRAMIN | 960 | 26.07 | 27.2 | 7.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 78 | access | bar0:W:0x710000:- | NV_PRAMIN | 960 | 25.96 | 27.0 | 60.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 79 | access | bar0:W:0x70e000:- | NV_PRAMIN | 960 | 25.95 | 27.0 | 15.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 80 | access | bar0:W:0x711000:- | NV_PRAMIN | 960 | 25.60 | 26.7 | 52.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 3554 | 179727 | 11001.41 |
| host | 66 | 73951 | 5524.82 |
| window | 28 | 151089 | 5300.30 |
| shadow | 6 | 68745 | 3521.67 |
| rpc | 7 | 3760 | 804.63 |
| control | 68 | 919 | 107.05 |
| alloc | 17 | 1430 | 7.94 |
