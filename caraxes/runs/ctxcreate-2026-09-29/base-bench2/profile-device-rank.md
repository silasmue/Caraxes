# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-bench2/profile.1.tsv, scope device, cost guest

3740 rows, 26767.1 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 34857 | 3232.15 | 92.7 | 2965.3 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 876 | 2583.95 | 2949.7 | 263227.6 | 0x0=876 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 3 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 14841 | 2170.83 | 146.3 | 12674.1 | - | - | - | bar2area.rs | - |
| 4 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 2003.30 | 187.4 | 119607.8 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 5 | access | bar3:R:0xfea000:- | - | 22069 | 1981.60 | 89.8 | 12672.6 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 6 | host | 0x29:0x0 | RM_FREE | 16630 | 1453.69 | 87.4 | 34300.7 | 0x0=16630 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 7 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 3762 | 1326.69 | 352.7 | 8731.3 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 8 | shadow | gather alloc | gather alloc | 460 | 1281.12 | 2785.0 | 12889.2 | - | - | - | shadow.rs (worker) | - |
| 9 | rpc | 0xa | FREE | 1405 | 713.78 | 508.0 | 6075.6 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 10 | shadow | map | map | 19654 | 608.65 | 31.0 | 8850.3 | - | - | - | shadow.rs (worker) | - |
| 11 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 19654 | 598.84 | 30.5 | 8849.3 | 0x0=19654 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 12 | shadow | object alloc | object alloc | 14699 | 564.74 | 38.4 | 6916.5 | - | - | - | shadow.rs (worker) | - |
| 13 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 14699 | 558.70 | 38.0 | 6915.8 | 0x0=14699 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 14 | shadow | unmap | unmap | 18767 | 555.50 | 29.6 | 4691.1 | - | - | - | shadow.rs (worker) | - |
| 15 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 18767 | 550.83 | 29.4 | 4690.7 | 0x0=18767 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 16 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 401 | 523.36 | 1305.1 | 4104.3 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 17 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 495.82 | 46.4 | 7890.8 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 18 | shadow | object free | object free | 14719 | 480.98 | 32.7 | 9153.4 | - | - | - | shadow.rs (worker) | - |
| 19 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 10692 | 441.19 | 41.3 | 6975.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 20 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 10239 | 358.40 | 35.0 | 1449.4 | - | - | - | bar1pool.rs | - |
| 21 | access | bar0:W:0x840000:0x840184 | NV_PSEC (SEC2 falcon) | 8512 | 246.04 | 28.9 | 16.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 22 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 160 | 230.29 | 1439.3 | 4634.3 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 23 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 400 | 229.24 | 573.1 | 135993.2 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 24 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 3760 | 195.89 | 52.1 | 82.5 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 25 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 10240 | 192.75 | 18.8 | 15543.9 | - | - | - | bar1pool.rs | - |
| 26 | access | bar0:W:0x840000:0x8401c4 | NV_PSEC (SEC2 falcon) | 6272 | 163.86 | 26.1 | 11.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PSEC (SEC2 falcon) |
| 27 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 3762 | 153.61 | 40.8 | 6798.1 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 28 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 25 | 151.74 | 6069.5 | 42136.4 | 0x0=25 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 29 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 3761 | 150.20 | 39.9 | 58.5 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 30 | rpc | 0x4c | GSP_RM_CONTROL | 919 | 103.05 | 112.1 | 3308.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 31 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 401 | 88.85 | 221.6 | 1142.7 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 81 | 74.95 | 925.3 | 1957.1 | 0x0=81 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 33 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 401 | 68.13 | 169.9 | 3118.7 | 0x0=401 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 34 | shadow | gather free | gather free | 460 | 53.80 | 117.0 | 2981.3 | - | - | - | shadow.rs (worker) | - |
| 35 | access | bar0:W:0x70e000:- | NV_PRAMIN | 960 | 50.83 | 52.9 | 30.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 36 | access | bar0:W:0x706000:- | NV_PRAMIN | 960 | 47.23 | 49.2 | 98.2 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 37 | access | cfg:R:0x0:0x4 | - | 975 | 45.14 | 46.3 | 1276.4 | - | pci-config | - | config.rs | - |
| 38 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 800 | 44.30 | 55.4 | 2.1 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 39 | access | bar0:W:0x715000:- | NV_PRAMIN | 960 | 36.58 | 38.1 | 11.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 40 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 20 | 36.08 | 1804.1 | 1954.3 | 0x0=20 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 41 | access | bar0:W:0x702000:- | NV_PRAMIN | 960 | 32.33 | 33.7 | 28.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 42 | access | bar0:W:0x70b000:- | NV_PRAMIN | 960 | 32.31 | 33.7 | 8.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 43 | access | bar0:R:0x303000:- | NV_PROM_DATA (VBIOS window) | 960 | 32.20 | 33.5 | 0.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 44 | access | bar0:W:0x714000:- | NV_PRAMIN | 960 | 32.10 | 33.4 | 14.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 45 | access | bar0:W:0x70d000:- | NV_PRAMIN | 1000 | 31.87 | 31.9 | 39.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 46 | access | bar0:R:0x31f000:- | NV_PROM_DATA (VBIOS window) | 960 | 31.22 | 32.5 | 0.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 47 | access | bar0:R:0x31b000:- | NV_PROM_DATA (VBIOS window) | 960 | 31.22 | 32.5 | 1.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 48 | access | bar0:R:0x31d000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.84 | 32.1 | 0.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 49 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 85 | 30.27 | 356.1 | 3288.5 | 0x0=85 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 50 | access | bar0:R:0x31c000:- | NV_PROM_DATA (VBIOS window) | 960 | 30.05 | 31.3 | 0.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 51 | access | bar0:W:0x70f000:- | NV_PRAMIN | 960 | 29.98 | 31.2 | 69.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 52 | access | bar0:R:0x31a000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.83 | 31.1 | 0.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 53 | access | bar0:W:0x712000:- | NV_PRAMIN | 960 | 29.63 | 30.9 | 6.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 54 | access | cfg:R:0x0:0x60 | - | 1885 | 29.61 | 15.7 | 69.0 | - | pci-config | - | config.rs | - |
| 55 | access | bar0:R:0x302000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.45 | 30.7 | 0.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 56 | access | bar0:W:0x708000:- | NV_PRAMIN | 960 | 29.37 | 30.6 | 22.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 57 | access | bar0:R:0x31e000:- | NV_PROM_DATA (VBIOS window) | 960 | 29.26 | 30.5 | 1.4 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 58 | access | cfg:R:0x0:0x68 | - | 1893 | 29.18 | 15.4 | 641.3 | - | pci-config | - | config.rs | - |
| 59 | access | bar0:W:0x70c000:- | NV_PRAMIN | 960 | 28.59 | 29.8 | 11.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | access | bar0:R:0x305000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.59 | 29.8 | 6.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 61 | access | bar0:W:0x707000:- | NV_PRAMIN | 960 | 28.54 | 29.7 | 9.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 62 | access | bar0:R:0x316000:- | NV_PROM_DATA (VBIOS window) | 960 | 28.16 | 29.3 | 0.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 63 | access | bar0:R:0x30d000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.94 | 29.1 | 0.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 64 | access | bar0:R:0x30f000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.93 | 29.1 | 0.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 65 | access | bar0:W:0x713000:- | NV_PRAMIN | 960 | 27.89 | 29.1 | 10.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 66 | access | bar0:R:0x30a000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.83 | 29.0 | 2.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 67 | access | bar0:R:0x30b000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.46 | 28.6 | 1.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 68 | access | bar0:R:0x30c000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.45 | 28.6 | 0.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 69 | rpc | 0x67 | GSP_RM_ALLOC | 1430 | 27.34 | 19.1 | 758.0 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 70 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 112 | 27.24 | 243.2 | 14276.8 | 0x0=112 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 71 | access | bar0:R:0x309000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.16 | 28.3 | 0.9 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 72 | access | bar0:R:0x317000:- | NV_PROM_DATA (VBIOS window) | 960 | 27.06 | 28.2 | 0.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 73 | access | bar0:W:0x711000:- | NV_PRAMIN | 960 | 26.91 | 28.0 | 61.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 74 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 21 | 26.70 | 1271.6 | 1374.0 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 75 | access | bar0:W:0x70a000:- | NV_PRAMIN | 960 | 26.37 | 27.5 | 9.1 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 76 | access | bar0:W:0x710000:- | NV_PRAMIN | 960 | 26.23 | 27.3 | 6.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 77 | access | bar0:R:0x310000:- | NV_PROM_DATA (VBIOS window) | 960 | 26.18 | 27.3 | 0.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 78 | access | bar0:R:0x301000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.69 | 26.8 | 0.6 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |
| 79 | access | cfg:R:0x0:0x78 | - | 1883 | 25.52 | 13.6 | 2.1 | - | pci-config | - | config.rs | - |
| 80 | access | bar0:R:0x304000:- | NV_PROM_DATA (VBIOS window) | 960 | 25.46 | 26.5 | 0.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PROM_DATA (VBIOS window) |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 3554 | 180411 | 12410.41 |
| host | 66 | 74571 | 7141.04 |
| shadow | 6 | 68759 | 3544.79 |
| window | 22 | 120942 | 2721.99 |
| rpc | 7 | 3760 | 844.31 |
| control | 68 | 919 | 97.81 |
| alloc | 17 | 1430 | 6.74 |
