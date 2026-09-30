# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/new2-bench-b/profile.1.tsv, scope device, cost guest

3746 rows, 25718.0 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 34404 | 2751.16 | 80.0 | 1113.3 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 14614 | 1968.03 | 134.7 | 35198.3 | - | - | - | bar2area.rs | - |
| 3 | window | bar2 memfd: sync scan (units: pages compared) | bar2 memfd: sync scan (units: pages compared) | 14614 | 1923.69 | 131.6 | 35196.0 | - | - | - | bar2area.rs | - |
| 4 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 1805.01 | 168.8 | 119241.1 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 5 | access | bar3:R:0xfea000:- | - | 22069 | 1655.75 | 75.0 | 35222.1 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 6 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 857 | 1319.36 | 1539.5 | 346725.7 | 0x0=857 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | shadow | gather alloc | gather alloc | 450 | 1254.25 | 2787.2 | 10076.3 | - | - | - | shadow.rs (worker) | - |
| 8 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 3762 | 1229.56 | 326.8 | 10909.0 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 9 | host | 0x29:0x0 | RM_FREE | 16613 | 1076.17 | 64.8 | 25340.4 | 0x0=16613 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 10 | rpc | 0xa | FREE | 1405 | 663.79 | 472.5 | 9838.9 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 11 | window | gather: window of mremaps (units: runs) | gather: window of mremaps (units: runs) | 450 | 638.22 | 1418.3 | 8625.4 | - | - | - | - | - |
| 12 | shadow | map | map | 19673 | 572.68 | 29.1 | 9439.6 | - | - | - | shadow.rs (worker) | - |
| 13 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19673 | 566.26 | 28.8 | 9438.9 | 0x0=19673 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | shadow | object alloc | object alloc | 14701 | 509.01 | 34.6 | 5798.8 | - | - | - | shadow.rs (worker) | - |
| 15 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14701 | 503.45 | 34.2 | 5797.7 | 0x0=14701 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | shadow | unmap | unmap | 18719 | 477.67 | 25.5 | 4335.8 | - | - | - | shadow.rs (worker) | - |
| 17 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18719 | 473.79 | 25.3 | 4335.2 | 0x0=18719 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 18 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 401 | 455.65 | 1136.3 | 3402.5 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 19 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 406.62 | 38.0 | 6523.2 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 20 | shadow | object free | object free | 14721 | 406.24 | 27.6 | 3276.4 | - | - | - | shadow.rs (worker) | - |
| 21 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 326.61 | 30.5 | 8211.2 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 22 | window | gather: window unmap (units: pages) | gather: window unmap (units: pages) | 450 | 314.27 | 698.4 | 2492.7 | - | - | - | - | - |
| 23 | window | gather: host descriptor (units: pages) | gather: host descriptor (units: pages) | 450 | 300.20 | 667.1 | 4799.1 | - | - | - | - | - |
| 24 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 160 | 269.71 | 1685.7 | 4382.2 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 25 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 10240 | 257.39 | 25.1 | 3171.1 | - | - | - | bar1pool.rs | - |
| 26 | access | bar0:W:0x840000:0x840184 | NV_PSEC (SEC2 falcon) | 8512 | 222.51 | 26.1 | 5.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 27 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 400 | 216.33 | 540.8 | 82293.5 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 28 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 3760 | 162.58 | 43.2 | 90.3 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 29 | access | bar0:W:0x840000:0x8401c4 | NV_PSEC (SEC2 falcon) | 6272 | 159.26 | 25.4 | 41.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 30 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 10239 | 158.44 | 15.5 | 2754.3 | - | - | - | bar1pool.rs | - |
| 31 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 3762 | 133.65 | 35.5 | 4591.9 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 32 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 3761 | 130.06 | 34.6 | 57.3 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 33 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 401 | 98.99 | 246.9 | 4410.4 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | rpc | 0x4c | GSP_RM_CONTROL | 919 | 93.45 | 101.7 | 3666.6 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 35 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 23 | 84.41 | 3669.8 | 9436.1 | 0x0=23 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 36 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 81 | 68.82 | 849.6 | 2200.6 | 0x0=81 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 401 | 60.78 | 151.6 | 2147.1 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | access | bar0:R:0x303000:- | NV_PROM_DATA (VBIOS window) | 960 | 44.66 | 46.5 | 0.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 39 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 800 | 41.54 | 51.9 | 10.6 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 40 | access | bar0:R:0x319000:- | NV_PROM_DATA (VBIOS window) | 960 | 39.74 | 41.4 | 1.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 41 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 20 | 36.51 | 1825.5 | 3662.1 | 0x0=20 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 42 | access | cfg:R:0x0:0x4 | - | 859 | 34.18 | 39.8 | 24.5 | - | pci-config | - | config.rs | - |
| 43 | shadow | gather free | gather free | 450 | 33.98 | 75.5 | 811.9 | - | - | - | shadow.rs (worker) | - |
| 44 | access | bar0:W:0x715000:- | NV_PRAMIN | 960 | 33.15 | 34.5 | 15.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 45 | access | bar0:R:0x314000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.61 | 31.9 | 1.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 46 | access | bar0:R:0x316000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.01 | 31.3 | 1.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 47 | access | bar0:R:0x304000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.65 | 30.9 | 0.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 48 | rpc | 0x67 | GSP_RM_ALLOC | 1430 | 28.70 | 20.1 | 2263.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 49 | access | bar0:R:0x315000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.09 | 29.3 | 0.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 50 | access | bar0:R:0x317000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.42 | 28.6 | 0.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 51 | access | bar0:R:0x31f000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.41 | 28.5 | 0.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 52 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 21 | 27.15 | 1293.1 | 1687.1 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 53 | access | bar0:R:0x318000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.07 | 28.2 | 0.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 54 | access | bar0:W:0x70f000:- | NV_PRAMIN | 960 | 27.00 | 28.1 | 5.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 55 | access | bar0:R:0x31d000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.88 | 28.0 | 1.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 56 | access | bar0:R:0x31b000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.87 | 28.0 | 1.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 57 | access | bar0:W:0x710000:- | NV_PRAMIN | 960 | 26.83 | 27.9 | 8.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 58 | access | cfg:R:0x0:0x68 | - | 1663 | 26.70 | 16.1 | 614.8 | - | pci-config | - | config.rs | - |
| 59 | access | bar0:W:0x709000:- | NV_PRAMIN | 960 | 26.63 | 27.7 | 8.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:W:0x70e000:- | NV_PRAMIN | 960 | 26.56 | 27.7 | 16.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 61 | access | bar0:W:0x711000:- | NV_PRAMIN | 960 | 26.25 | 27.3 | 7.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 62 | access | bar0:W:0x70b000:- | NV_PRAMIN | 960 | 26.00 | 27.1 | 66.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 63 | access | bar0:R:0x31e000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.71 | 26.8 | 1.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 64 | access | bar0:W:0x708000:- | NV_PRAMIN | 960 | 25.64 | 26.7 | 3.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 65 | access | bar0:R:0x31a000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.47 | 26.5 | 1.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 66 | access | bar0:W:0x714000:- | NV_PRAMIN | 960 | 25.44 | 26.5 | 8.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 67 | access | bar0:W:0x707000:- | NV_PRAMIN | 960 | 25.30 | 26.4 | 20.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 68 | access | bar0:R:0x31c000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.30 | 26.4 | 1.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 69 | access | bar0:R:0x313000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.13 | 26.2 | 0.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 70 | access | bar0:W:0x70d000:- | NV_PRAMIN | 1000 | 24.94 | 24.9 | 4.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 71 | access | bar0:W:0x70a000:- | NV_PRAMIN | 960 | 24.64 | 25.7 | 7.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 72 | access | bar0:W:0x70c000:- | NV_PRAMIN | 960 | 24.39 | 25.4 | 8.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 73 | access | bar0:W:0x702000:- | NV_PRAMIN | 960 | 24.31 | 25.3 | 9.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 74 | access | bar0:R:0x309000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.09 | 25.1 | 0.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 75 | access | bar0:R:0x302000:- | NV_PROM_DATA (VBIOS window) | 960 | 24.04 | 25.0 | 0.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 76 | access | bar0:W:0x713000:- | NV_PRAMIN | 960 | 23.98 | 25.0 | 7.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 77 | access | bar0:W:0x712000:- | NV_PRAMIN | 960 | 23.92 | 24.9 | 6.5 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 78 | access | bar0:R:0x30b000:- | NV_PROM_DATA (VBIOS window) | 960 | 23.92 | 24.9 | 1.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 79 | access | bar0:W:0x706000:- | NV_PRAMIN | 960 | 23.86 | 24.9 | 4.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 80 | access | bar0:R:0x300000:- | NV_PROM_DATA (VBIOS window) | 960 | 23.86 | 24.9 | 0.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 3554 | 178932 | 10809.03 |
| window | 28 | 153920 | 5590.84 |
| host | 66 | 74313 | 5181.20 |
| shadow | 6 | 68714 | 3253.82 |
| rpc | 7 | 3760 | 786.05 |
| control | 68 | 919 | 87.16 |
| alloc | 17 | 1430 | 9.91 |
