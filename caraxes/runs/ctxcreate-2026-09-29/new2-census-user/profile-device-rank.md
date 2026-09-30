# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new2-census-user/profile.1.tsv, scope device, cost guest

3746 rows, 22671.1 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 32673 | 2759.56 | 84.5 | 2410.9 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 1579.37 | 159.4 | 19806.2 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 13017 | 1570.98 | 120.7 | 11460.3 | - | - | - | bar2area.rs | - |
| 4 | window | bar2 memfd: sync scan (units: pages compared) | bar2 memfd: sync scan (units: pages compared) | 13017 | 1537.20 | 118.1 | 11457.5 | - | - | - | bar2area.rs | - |
| 5 | access | bar3:R:0xfea000:- | - | 19805 | 1490.68 | 75.3 | 11470.5 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 6 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 626 | 1266.26 | 2022.8 | 348212.6 | 0x0=626 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 2920 | 960.18 | 328.8 | 20343.2 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 8 | host | 0x29:0x0 | RM_FREE | 15689 | 923.12 | 58.8 | 31909.0 | 0x0=15689 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 9 | shadow | gather alloc | gather alloc | 319 | 888.62 | 2785.7 | 10830.1 | - | - | - | shadow.rs (worker) | - |
| 10 | shadow | map | map | 19197 | 558.03 | 29.1 | 3717.2 | - | - | - | shadow.rs (worker) | - |
| 11 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19197 | 552.04 | 28.8 | 3716.3 | 0x0=19197 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 12 | shadow | object alloc | object alloc | 14273 | 533.95 | 37.4 | 5189.2 | - | - | - | shadow.rs (worker) | - |
| 13 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14273 | 528.78 | 37.0 | 5188.4 | 0x0=14273 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | shadow | unmap | unmap | 18456 | 517.62 | 28.0 | 6577.2 | - | - | - | shadow.rs (worker) | - |
| 15 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18456 | 513.79 | 27.8 | 6576.6 | 0x0=18456 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | rpc | 0xa | FREE | 1068 | 469.11 | 439.2 | 2517.8 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 17 | window | gather: window of mremaps (units: runs) | gather: window of mremaps (units: runs) | 319 | 450.93 | 1413.6 | 5239.6 | - | - | - | - | - |
| 18 | shadow | object free | object free | 14288 | 425.61 | 29.8 | 3477.6 | - | - | - | shadow.rs (worker) | - |
| 19 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 402.77 | 40.7 | 2421.7 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 20 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 301 | 398.78 | 1324.8 | 4381.2 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 21 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 9907 | 379.71 | 38.3 | 2114.7 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 22 | window | gather: window unmap (units: pages) | gather: window unmap (units: pages) | 319 | 228.26 | 715.6 | 4437.7 | - | - | - | - | - |
| 23 | access | bar0:W:0x840000:0x840184 | NV_PSEC (SEC2 falcon) | 8512 | 212.42 | 25.0 | 26.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 24 | window | gather: host descriptor (units: pages) | gather: host descriptor (units: pages) | 319 | 208.30 | 653.0 | 3752.3 | - | - | - | - | - |
| 25 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 300 | 171.91 | 573.0 | 76415.0 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 26 | access | bar0:W:0x840000:0x8401c4 | NV_PSEC (SEC2 falcon) | 6272 | 157.67 | 25.1 | 26.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 27 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 120 | 151.73 | 1264.4 | 3573.0 | 0x0=120 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 28 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 2918 | 137.39 | 47.1 | 85.7 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 29 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 7680 | 111.77 | 14.6 | 2026.5 | - | - | - | bar1pool.rs | - |
| 30 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 2920 | 111.58 | 38.2 | 8919.9 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 31 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 7680 | 106.47 | 13.9 | 2704.8 | - | - | - | bar1pool.rs | - |
| 32 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 2919 | 106.18 | 36.4 | 71.8 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 33 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 24 | 74.98 | 3124.0 | 8048.9 | 0x0=24 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 301 | 70.03 | 232.7 | 5005.1 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 35 | rpc | 0x4c | GSP_RM_CONTROL | 751 | 68.06 | 90.6 | 1819.0 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 36 | access | bar0:W:0x1000:0x1478 | NV_PBUS_SW_SCRATCH(i) | 568 | 59.58 | 104.9 | 344.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PBUS_SW_SCRATCH(i) |
| 37 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 61 | 49.70 | 814.8 | 2035.8 | 0x0=61 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 301 | 44.75 | 148.7 | 949.3 | 0x0=301 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | access | cfg:R:0x0:0x4 | - | 859 | 38.57 | 44.9 | 48.1 | - | pci-config | - | config.rs | - |
| 40 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 600 | 36.61 | 61.0 | 59.9 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 41 | access | bar0:W:0x713000:- | NV_PRAMIN | 960 | 31.02 | 32.3 | 18.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 42 | access | bar0:W:0x712000:- | NV_PRAMIN | 960 | 29.80 | 31.0 | 21.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 43 | access | bar0:W:0x711000:- | NV_PRAMIN | 960 | 29.75 | 31.0 | 109.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 44 | access | bar0:W:0x710000:- | NV_PRAMIN | 960 | 29.70 | 30.9 | 21.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 45 | access | bar0:W:0x70d000:- | NV_PRAMIN | 1000 | 29.68 | 29.7 | 24.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 46 | access | bar0:W:0x70e000:- | NV_PRAMIN | 960 | 29.61 | 30.8 | 13.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 47 | access | bar0:W:0x70f000:- | NV_PRAMIN | 960 | 29.02 | 30.2 | 43.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 48 | access | bar0:W:0x714000:- | NV_PRAMIN | 960 | 28.34 | 29.5 | 15.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 49 | shadow | gather free | gather free | 319 | 28.18 | 88.3 | 4112.7 | - | - | - | shadow.rs (worker) | - |
| 50 | access | bar0:W:0x715000:- | NV_PRAMIN | 960 | 28.15 | 29.3 | 15.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 51 | access | bar0:R:0x301000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.29 | 28.4 | 16.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 52 | access | bar0:W:0x708000:- | NV_PRAMIN | 960 | 27.27 | 28.4 | 29.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 53 | access | bar0:R:0x300000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.07 | 28.2 | 7.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 54 | access | bar0:W:0x707000:- | NV_PRAMIN | 960 | 26.76 | 27.9 | 28.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 55 | access | bar0:R:0x316000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.73 | 27.8 | 30.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 56 | access | bar0:W:0x706000:- | NV_PRAMIN | 960 | 26.66 | 27.8 | 22.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 57 | access | bar0:W:0x709000:- | NV_PRAMIN | 960 | 26.53 | 27.6 | 29.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 58 | access | bar0:W:0x70a000:- | NV_PRAMIN | 960 | 26.39 | 27.5 | 22.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 59 | access | bar0:W:0x70b000:- | NV_PRAMIN | 960 | 26.30 | 27.4 | 22.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:W:0x70c000:- | NV_PRAMIN | 960 | 26.18 | 27.3 | 28.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 61 | access | bar0:R:0x315000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.16 | 27.2 | 9.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 62 | access | bar0:W:0x702000:- | NV_PRAMIN | 960 | 25.92 | 27.0 | 24.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 63 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 77 | 25.72 | 334.0 | 2364.4 | 0x0=77 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 64 | access | bar0:R:0x31f000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.52 | 26.6 | 16.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 65 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 15 | 25.33 | 1688.8 | 1814.2 | 0x0=15 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 66 | access | cfg:R:0x0:0x68 | - | 1663 | 25.23 | 15.2 | 3711.8 | - | pci-config | - | config.rs | - |
| 67 | access | cfg:R:0x0:0x60 | - | 1655 | 25.16 | 15.2 | 19.9 | - | pci-config | - | config.rs | - |
| 68 | access | bar0:R:0x306000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.70 | 25.7 | 12.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 69 | access | cfg:R:0x0:0x78 | - | 1653 | 24.56 | 14.9 | 20.4 | - | pci-config | - | config.rs | - |
| 70 | access | bar0:R:0x31a000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.48 | 25.5 | 21.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 71 | access | bar0:R:0x31b000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.46 | 25.5 | 11.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 72 | access | bar0:R:0x313000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.33 | 25.3 | 11.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 73 | access | bar0:R:0x31d000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.32 | 25.3 | 6.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 74 | access | bar0:R:0x312000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.22 | 25.2 | 6.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 75 | access | bar0:R:0x309000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.18 | 25.2 | 18.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 76 | access | bar0:R:0x31c000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.18 | 25.2 | 9.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 77 | access | bar0:R:0x30b000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.14 | 25.1 | 10.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 78 | access | bar0:R:0x305000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.05 | 25.0 | 20.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 79 | access | bar0:R:0x314000:- | NV_PROM_DATA (VBIOS window) | 960 | 23.99 | 25.0 | 6.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 80 | access | bar0:R:0x31e000:- | NV_PROM_DATA (VBIOS window) | 960 | 23.92 | 24.9 | 10.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 3555 | 169327 | 10118.52 |
| host | 66 | 70810 | 4734.08 |
| window | 27 | 130151 | 4239.49 |
| shadow | 6 | 66852 | 2952.00 |
| rpc | 7 | 2918 | 556.10 |
| control | 68 | 751 | 65.06 |
| alloc | 17 | 1093 | 5.85 |
