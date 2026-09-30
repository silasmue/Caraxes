# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-census-user/profile.1.tsv, scope device, cost guest

3740 rows, 25075.0 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 37905 | 3753.64 | 99.0 | 2524.7 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | access | bar3:R:0xfea000:- | - | 19805 | 1754.14 | 88.6 | 13176.6 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 3 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 1682.41 | 169.8 | 22601.1 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 4 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 13166 | 1645.20 | 125.0 | 13149.0 | - | - | - | bar2area.rs | - |
| 5 | host | 0x29:0x0 | RM_FREE | 15710 | 1422.43 | 90.5 | 30443.7 | 0x0=15710 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 6 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 650 | 1372.37 | 2111.3 | 254757.5 | 0x0=650 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | shadow | map | map | 19229 | 949.94 | 49.4 | 15182.6 | - | - | - | shadow.rs (worker) | - |
| 8 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 2920 | 940.16 | 322.0 | 18070.2 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 9 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19229 | 932.85 | 48.5 | 15075.4 | 0x0=19229 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | shadow | gather alloc | gather alloc | 342 | 910.06 | 2661.0 | 19412.8 | - | - | - | shadow.rs (worker) | - |
| 11 | shadow | unmap | unmap | 18507 | 794.60 | 42.9 | 11979.2 | - | - | - | shadow.rs (worker) | - |
| 12 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18507 | 788.65 | 42.6 | 11978.5 | 0x0=18507 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | rpc | 0xa | FREE | 1068 | 781.99 | 732.2 | 17885.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 14 | shadow | object alloc | object alloc | 14270 | 659.72 | 46.2 | 9911.6 | - | - | - | shadow.rs (worker) | - |
| 15 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14270 | 653.76 | 45.8 | 9910.0 | 0x0=14270 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | shadow | object free | object free | 14285 | 586.37 | 41.0 | 8453.0 | - | - | - | shadow.rs (worker) | - |
| 17 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 454.87 | 45.9 | 7548.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 18 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 301 | 411.95 | 1368.6 | 3103.3 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 408.82 | 41.3 | 6126.7 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 20 | access | bar0:W:0x840000:0x840184 | NV_PSEC (SEC2 falcon) | 8512 | 291.33 | 34.2 | 63.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 21 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 300 | 190.74 | 635.8 | 171397.3 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 22 | access | bar0:W:0x840000:0x8401c4 | NV_PSEC (SEC2 falcon) | 6272 | 169.68 | 27.1 | 28.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 23 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 120 | 149.53 | 1246.1 | 5116.3 | 0x0=120 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 24 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 2918 | 136.89 | 46.9 | 2272.8 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 25 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 7680 | 131.53 | 17.1 | 6620.2 | - | - | - | bar1pool.rs | - |
| 26 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 2920 | 114.76 | 39.3 | 7518.3 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 27 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 27 | 114.25 | 4231.6 | 8733.9 | 0x0=27 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 28 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 2919 | 111.06 | 38.0 | 1948.5 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 29 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 7680 | 101.60 | 13.2 | 1109.1 | - | - | - | bar1pool.rs | - |
| 30 | rpc | 0x4c | GSP_RM_CONTROL | 751 | 85.42 | 113.7 | 4171.0 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 31 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 301 | 75.53 | 250.9 | 3061.6 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | access | bar0:W:0x1000:0x1478 | NV_PBUS_SW_SCRATCH(i) | 568 | 72.49 | 127.6 | 1986.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PBUS_SW_SCRATCH(i) |
| 33 | access | bar0:R:0x305000:- | NV_PROM_DATA (VBIOS window) | 960 | 63.16 | 65.8 | 28.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 34 | access | bar0:R:0x304000:- | NV_PROM_DATA (VBIOS window) | 960 | 57.67 | 60.1 | 30.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 35 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 61 | 57.31 | 939.6 | 2722.3 | 0x0=61 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 301 | 54.28 | 180.3 | 2196.6 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | access | cfg:R:0x0:0x4 | - | 1033 | 44.07 | 42.7 | 48.6 | - | pci-config | - | config.rs | - |
| 38 | shadow | gather free | gather free | 342 | 42.69 | 124.8 | 3341.8 | - | - | - | shadow.rs (worker) | - |
| 39 | access | bar0:R:0x308000:- | NV_PROM_DATA (VBIOS window) | 960 | 41.11 | 42.8 | 20.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 40 | access | bar0:W:0x713000:- | NV_PRAMIN | 960 | 41.08 | 42.8 | 1733.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 41 | access | bar0:W:0x712000:- | NV_PRAMIN | 960 | 38.91 | 40.5 | 22.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 42 | access | bar0:R:0x30b000:- | NV_PROM_DATA (VBIOS window) | 960 | 38.06 | 39.6 | 28.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 43 | access | bar0:R:0x306000:- | NV_PROM_DATA (VBIOS window) | 960 | 37.72 | 39.3 | 18.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 44 | access | bar0:W:0x70a000:- | NV_PRAMIN | 960 | 36.94 | 38.5 | 21.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 45 | access | bar0:W:0x714000:- | NV_PRAMIN | 960 | 36.15 | 37.7 | 21.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 46 | access | bar0:W:0x70d000:- | NV_PRAMIN | 1000 | 35.83 | 35.8 | 94.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 47 | access | bar0:W:0x708000:- | NV_PRAMIN | 960 | 35.83 | 37.3 | 22.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 48 | access | bar0:W:0x710000:- | NV_PRAMIN | 960 | 34.54 | 36.0 | 16.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 49 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 81 | 34.44 | 425.1 | 3777.8 | 0x0=81 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 50 | access | cfg:R:0x0:0x60 | - | 2000 | 33.46 | 16.7 | 19.6 | - | pci-config | - | config.rs | - |
| 51 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 600 | 33.17 | 55.3 | 1655.2 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 52 | access | bar0:R:0x31f000:- | NV_PROM_DATA (VBIOS window) | 960 | 32.81 | 34.2 | 37.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 53 | access | cfg:R:0x0:0x78 | - | 1998 | 32.64 | 16.3 | 18.5 | - | pci-config | - | config.rs | - |
| 54 | access | bar0:R:0x31b000:- | NV_PROM_DATA (VBIOS window) | 960 | 32.27 | 33.6 | 33.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 55 | access | bar0:R:0x303000:- | NV_PROM_DATA (VBIOS window) | 960 | 32.22 | 33.6 | 19.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 56 | access | bar0:R:0x30e000:- | NV_PROM_DATA (VBIOS window) | 960 | 31.84 | 33.2 | 11.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 57 | access | bar0:R:0x314000:- | NV_PROM_DATA (VBIOS window) | 960 | 31.77 | 33.1 | 14.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 58 | access | bar0:W:0x707000:- | NV_PRAMIN | 960 | 31.69 | 33.0 | 28.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 59 | access | bar0:R:0x313000:- | NV_PROM_DATA (VBIOS window) | 960 | 31.68 | 33.0 | 78.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 60 | access | bar0:W:0x711000:- | NV_PRAMIN | 960 | 31.29 | 32.6 | 16.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 61 | access | bar0:W:0x70b000:- | NV_PRAMIN | 960 | 31.19 | 32.5 | 19.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 62 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 15 | 30.63 | 2042.2 | 4163.3 | 0x0=15 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 63 | access | bar0:R:0x30d000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.58 | 31.9 | 75.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 64 | access | bar0:R:0x312000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.49 | 31.8 | 16.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 65 | access | bar0:R:0x30f000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.42 | 31.7 | 24.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 66 | access | cfg:R:0x0:0x68 | - | 2008 | 30.29 | 15.1 | 30.1 | - | pci-config | - | config.rs | - |
| 67 | access | bar0:R:0x31e000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.97 | 31.2 | 12.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 68 | access | bar0:W:0x70e000:- | NV_PRAMIN | 960 | 29.46 | 30.7 | 38.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 69 | access | bar0:R:0x31d000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.39 | 30.6 | 2871.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 70 | access | bar0:W:0x702000:- | NV_PRAMIN | 960 | 29.26 | 30.5 | 120.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 71 | access | bar0:W:0x70f000:- | NV_PRAMIN | 960 | 29.24 | 30.5 | 19.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 72 | access | bar0:R:0x311000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.85 | 30.1 | 23.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 73 | access | bar0:R:0x315000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.25 | 29.4 | 12.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 74 | access | bar0:R:0x300000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.17 | 29.3 | 5.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 75 | access | bar0:W:0x706000:- | NV_PRAMIN | 960 | 28.14 | 29.3 | 15.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 76 | access | bar0:R:0x31a000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.11 | 29.3 | 18.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 77 | access | bar0:W:0x715000:- | NV_PRAMIN | 960 | 27.92 | 29.1 | 53.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 78 | access | bar0:W:0x709000:- | NV_PRAMIN | 960 | 27.82 | 29.0 | 14.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 79 | access | bar0:R:0x301000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.80 | 29.0 | 12.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 80 | access | bar0:W:0x70c000:- | NV_PRAMIN | 960 | 27.57 | 28.7 | 16.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 3555 | 176098 | 12017.53 |
| host | 66 | 71341 | 6264.29 |
| shadow | 6 | 66975 | 3943.37 |
| window | 21 | 103516 | 1878.33 |
| rpc | 7 | 2918 | 885.59 |
| control | 68 | 751 | 81.70 |
| alloc | 17 | 1093 | 4.21 |
