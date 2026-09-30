# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-bench5/profile.1.tsv, scope device, cost guest

3740 rows, 38951.8 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 66090 | 7627.73 | 115.4 | 9294.1 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | host | 0x29:0x0 | RM_FREE | 16622 | 2498.98 | 150.3 | 34661.5 | 0x0=16622 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 14684 | 2046.19 | 139.3 | 30057.7 | - | - | - | bar2area.rs | - |
| 4 | access | bar3:R:0xfea000:- | - | 22069 | 1940.75 | 87.9 | 30045.8 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 5 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 1928.98 | 180.4 | 28837.5 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 6 | shadow | unmap | unmap | 18750 | 1893.58 | 101.0 | 11467.3 | - | - | - | shadow.rs (worker) | - |
| 7 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18750 | 1888.40 | 100.7 | 11467.0 | 0x0=18750 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 8 | shadow | map | map | 19670 | 1845.89 | 93.8 | 10397.7 | - | - | - | shadow.rs (worker) | - |
| 9 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19670 | 1833.09 | 93.2 | 9896.5 | 0x0=19670 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | shadow | object free | object free | 14719 | 1731.68 | 117.6 | 16551.0 | - | - | - | shadow.rs (worker) | - |
| 11 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 868 | 1702.24 | 1961.1 | 392846.3 | 0x0=868 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 12 | shadow | object alloc | object alloc | 14699 | 1534.53 | 104.4 | 9152.8 | - | - | - | shadow.rs (worker) | - |
| 13 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14699 | 1522.65 | 103.6 | 9151.5 | 0x0=14699 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | shadow | gather alloc | gather alloc | 460 | 1250.00 | 2717.4 | 11108.8 | - | - | - | shadow.rs (worker) | - |
| 15 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 3762 | 1199.52 | 318.9 | 13440.2 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 16 | rpc | 0xa | FREE | 1405 | 715.45 | 509.2 | 13389.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 17 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 401 | 493.50 | 1230.7 | 4681.0 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 18 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 469.43 | 43.9 | 2360.5 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 19 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 398.34 | 37.3 | 1942.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 20 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 400 | 230.10 | 575.3 | 187199.7 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 21 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 160 | 229.94 | 1437.1 | 4352.1 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 22 | access | bar0:W:0x840000:0x840184 | NV_PSEC (SEC2 falcon) | 8512 | 228.93 | 26.9 | 17.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 23 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 10240 | 173.09 | 16.9 | 2390.6 | - | - | - | bar1pool.rs | - |
| 24 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 3760 | 167.55 | 44.6 | 32.7 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 25 | access | bar0:W:0x840000:0x8401c4 | NV_PSEC (SEC2 falcon) | 6272 | 163.50 | 26.1 | 12.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 26 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 10240 | 155.54 | 15.2 | 3137.3 | - | - | - | bar1pool.rs | - |
| 27 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 3762 | 147.25 | 39.1 | 71.1 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 28 | rpc | 0x4c | GSP_RM_CONTROL | 919 | 146.19 | 159.1 | 8251.6 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 29 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 3761 | 130.72 | 34.8 | 2064.8 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 30 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 29 | 120.47 | 4154.1 | 9060.0 | 0x0=29 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 401 | 92.19 | 229.9 | 2793.5 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 401 | 66.82 | 166.6 | 3519.9 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 81 | 65.28 | 806.0 | 2760.1 | 0x0=81 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | access | bar0:R:0x31c000:- | NV_PROM_DATA (VBIOS window) | 960 | 51.64 | 53.8 | 8.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 35 | access | cfg:R:0x0:0x4 | - | 1033 | 51.11 | 49.5 | 693.2 | - | pci-config | - | config.rs | - |
| 36 | access | bar0:R:0x30c000:- | NV_PROM_DATA (VBIOS window) | 960 | 49.29 | 51.3 | 6.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 37 | shadow | gather free | gather free | 460 | 46.27 | 100.6 | 4124.6 | - | - | - | shadow.rs (worker) | - |
| 38 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 800 | 41.89 | 52.4 | 36.5 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 39 | access | bar0:R:0x30e000:- | NV_PROM_DATA (VBIOS window) | 960 | 40.35 | 42.0 | 49.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 40 | access | bar0:R:0x31d000:- | NV_PROM_DATA (VBIOS window) | 960 | 39.66 | 41.3 | 0.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 41 | access | bar0:R:0x314000:- | NV_PROM_DATA (VBIOS window) | 960 | 38.79 | 40.4 | 0.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 42 | access | bar0:R:0x30d000:- | NV_PROM_DATA (VBIOS window) | 960 | 36.89 | 38.4 | 6.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 43 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 20 | 36.58 | 1828.9 | 4018.4 | 0x0=20 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 44 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 112 | 34.38 | 307.0 | 12543.5 | 0x0=112 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 45 | host | 0x4e:0x0 | RM_MAP_MEMORY | 116 | 33.34 | 287.4 | 16599.5 | 0x0=116 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | access | bar0:R:0x30b000:- | NV_PROM_DATA (VBIOS window) | 960 | 33.28 | 34.7 | 0.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 47 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 21 | 32.42 | 1543.8 | 8257.3 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 48 | access | bar0:R:0x303000:- | NV_PROM_DATA (VBIOS window) | 960 | 31.82 | 33.1 | 1.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 49 | access | cfg:R:0x0:0x78 | - | 1998 | 31.41 | 15.7 | 682.9 | - | pci-config | - | config.rs | - |
| 50 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 128 | 31.41 | 245.4 | 8241.4 | 0x0=128 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 51 | access | cfg:R:0x0:0x60 | - | 2000 | 30.49 | 15.2 | 287.6 | - | pci-config | - | config.rs | - |
| 52 | access | bar0:W:0x70f000:- | NV_PRAMIN | 960 | 30.22 | 31.5 | 12.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 53 | access | bar0:W:0x706000:- | NV_PRAMIN | 960 | 29.76 | 31.0 | 18.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 54 | access | bar0:R:0x31b000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.99 | 30.2 | 0.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 55 | access | cfg:R:0x0:0x68 | - | 2008 | 28.95 | 14.4 | 13.1 | - | pci-config | - | config.rs | - |
| 56 | access | bar0:R:0x301000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.76 | 28.9 | 0.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 57 | access | bar0:R:0x308000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.76 | 28.9 | 4.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 58 | access | bar0:R:0x311000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.68 | 28.8 | 17.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 59 | access | bar0:R:0x304000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.58 | 28.7 | 57.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 60 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 88 | 26.99 | 306.8 | 1126.1 | 0x0=88 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 61 | access | bar0:W:0x712000:- | NV_PRAMIN | 960 | 26.90 | 28.0 | 10.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 62 | access | bar0:R:0x31e000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.62 | 27.7 | 2.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 63 | access | bar0:R:0x302000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.58 | 27.7 | 7.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 64 | access | bar0:R:0x30f000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.30 | 27.4 | 0.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 65 | access | bar0:W:0x70d000:- | NV_PRAMIN | 1000 | 25.98 | 26.0 | 76.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 66 | access | bar0:W:0x711000:- | NV_PRAMIN | 960 | 25.97 | 27.1 | 8.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 67 | access | bar0:R:0x307000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.96 | 27.0 | 0.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 68 | access | bar0:R:0x305000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.83 | 26.9 | 1.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 69 | access | bar0:W:0x702000:- | NV_PRAMIN | 960 | 25.76 | 26.8 | 16.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 70 | access | bar0:R:0x300000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.67 | 26.7 | 24.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 71 | access | bar0:R:0x309000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.55 | 26.6 | 0.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 72 | access | bar0:R:0x319000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.44 | 26.5 | 8.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 73 | rpc | 0x67 | GSP_RM_ALLOC | 1430 | 25.34 | 17.7 | 2487.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 74 | access | bar0:R:0x306000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.15 | 26.2 | 4.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 75 | access | bar0:R:0x315000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.11 | 26.2 | 0.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 76 | access | bar0:R:0x316000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.09 | 26.1 | 0.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 77 | access | bar0:W:0x707000:- | NV_PRAMIN | 960 | 24.95 | 26.0 | 17.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 78 | access | bar0:W:0x710000:- | NV_PRAMIN | 960 | 24.62 | 25.7 | 12.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 79 | access | bar0:R:0x317000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.56 | 25.6 | 0.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 80 | access | bar0:W:0x708000:- | NV_PRAMIN | 960 | 24.52 | 25.5 | 11.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 3554 | 212157 | 16389.42 |
| host | 66 | 78713 | 10848.81 |
| shadow | 6 | 68758 | 8301.94 |
| window | 22 | 156381 | 2374.82 |
| rpc | 7 | 3760 | 887.09 |
| control | 68 | 919 | 141.38 |
| alloc | 17 | 1430 | 8.33 |
