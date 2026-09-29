# profile-rank: /mnt/vmstore/caraxes/wt-ctx/caraxes/runs/ctxcreate-2026-09-29/base-bench5/profile.1.tsv, scope clients, cost guest

1504 rows, 274720.6 ms in all.

| # | kind | key | name | count | cost ms | mean us | max us | status | category | provenance | module | label |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | access | bar0:R:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 520187 | 60449.22 | 116.2 | 9294.1 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 2 | host | 0x29:0x0 | RM_FREE | 129273 | 18400.63 | 142.3 | 34661.5 | 0x0=160 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 3 | shadow | unmap | unmap | 149169 | 15107.04 | 101.3 | 11467.3 | - | - | - | shadow.rs (worker) | - |
| 4 | host | 0x58:0x0 | RM_UNMAP_MEMORY_DMA | 149169 | 15065.85 | 101.0 | 11467.0 | 0x0=12 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 5 | window | bar2 memfd: syncs (units: dirty pages) | bar2 memfd: syncs (units: dirty pages) | 110378 | 15035.14 | 136.2 | 30057.7 | - | - | - | bar2area.rs | - |
| 6 | access | bar3:R:0xfea000:- | - | 167033 | 14739.22 | 88.2 | 30045.8 | - | bar2-flush-read | - | fb.rs, bar2area.rs (the memfd sync point) | - |
| 7 | shadow | map | map | 156195 | 14689.43 | 94.0 | 10397.7 | - | - | - | shadow.rs (worker) | - |
| 8 | host | 0x57:0x0 | RM_MAP_MEMORY_DMA | 156195 | 14587.52 | 93.4 | 9896.5 | 0x0=39 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 9 | access | bar0:W:0xb83000:0xb830b0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 14546.86 | 176.7 | 28837.5 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 10 | shadow | object free | object free | 115855 | 13568.93 | 117.1 | 16551.0 | - | - | - | shadow.rs (worker) | - |
| 11 | shadow | object alloc | object alloc | 116669 | 12120.66 | 103.9 | 9152.8 | - | - | - | shadow.rs (worker) | - |
| 12 | host | 0x2b:0x50a0 | RM_ALLOC NV50_MEMORY_VIRTUAL | 116669 | 12027.76 | 103.1 | 9151.5 | 0x0=38 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 13 | shadow | gather alloc | gather alloc | 3239 | 8789.54 | 2713.7 | 11108.8 | - | - | - | shadow.rs (worker) | - |
| 14 | access | bar0:W:0x110000:0x110c00 | NV_PGSP_QUEUE_HEAD/TAIL | 26333 | 8463.99 | 321.4 | 13440.2 | - | gsp-queue-doorbell | - | boot.rs (service_queues), msgq.rs | NV_PGSP_QUEUE_HEAD/TAIL |
| 15 | rpc | 0xa | FREE | 9934 | 5007.20 | 504.0 | 13389.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 16 | access | bar0:W:0xb83000:0xb830a0 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 3631.00 | 44.1 | 2360.5 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 17 | host | 0x2b:0xc46f | RM_ALLOC TURING_CHANNEL_GPFIFO_A | 2828 | 3478.73 | 1230.1 | 4681.0 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 18 | access | bar0:W:0xb83000:0xb830a4 | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE | 82343 | 3081.87 | 37.4 | 1942.0 | - | mmu-invalidate | - | invalidate.rs, shadow.rs (plans) | NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE |
| 19 | host | 0x27:0x0 | RM_ALLOC_MEMORY | 6059 | 2731.66 | 450.8 | 392846.3 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 20 | host | 0x2b:0xc5c0 | RM_ALLOC TURING_COMPUTE_A | 1128 | 1623.10 | 1438.9 | 4352.1 | 0x0=8 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 21 | access | bar0:W:0xbb0000:0xbb0090 | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) | 2820 | 1621.98 | 575.2 | 187199.7 | - | usermode-doorbell | - | channel.rs, tokens.rs, mirror.rs | NV_VIRTUAL_FUNCTION_DOORBELL (usermode) |
| 22 | window | bar1 pool: page moved out, warm (units: bytes) | bar1 pool: page moved out, warm (units: bytes) | 72691 | 1225.18 | 16.9 | 2390.6 | - | - | - | bar1pool.rs | - |
| 23 | access | bar0:R:0x110000:0x110c80 | NV_PGSP (GSP falcon) | 26328 | 1179.44 | 44.8 | 32.7 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 24 | window | bar1 pool: page moved in, cold (units: bytes) | bar1 pool: page moved in, cold (units: bytes) | 72192 | 1100.21 | 15.2 | 3137.3 | - | - | - | bar1pool.rs | - |
| 25 | rpc | 0x4c | GSP_RM_CONTROL | 6313 | 1045.97 | 165.7 | 8251.6 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 26 | access | bar0:W:0x110000:0x110c84 | NV_PGSP (GSP falcon) | 26333 | 1033.60 | 39.3 | 71.1 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 27 | host | 0x2a:0x2080200a | RM_CONTROL NV2080_CTRL_CMD_PERF_BOOST | 225 | 933.18 | 4147.5 | 9060.0 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 28 | access | bar0:R:0x110000:0x110094 | NV_PGSP (GSP falcon) | 26333 | 917.83 | 34.9 | 2064.8 | - | gsp-falcon | - | falcon.rs, gspboot.rs | NV_PGSP (GSP falcon) |
| 29 | host | 0x2b:0xc5b5 | RM_ALLOC TURING_DMA_COPY_A | 2828 | 650.06 | 229.9 | 2793.5 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 30 | host | 0x2a:0xa06f0103 | RM_CONTROL NVA06F_CTRL_CMD_GPFIFO_SCHEDULE | 2828 | 470.69 | 166.4 | 3519.9 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 31 | host | 0x2b:0xa06c | RM_ALLOC KEPLER_CHANNEL_GROUP_A | 572 | 460.22 | 804.6 | 2760.1 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 32 | shadow | gather free | gather free | 3237 | 326.03 | 100.7 | 4124.6 | - | - | - | shadow.rs (worker) | - |
| 33 | access | bar0:R:0xbb0000:0xbb0084 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 5640 | 294.77 | 52.3 | 36.5 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 34 | control | 0x20801210 | NV2080_CTRL_CMD_GR_SET_CTXSW_PREEMPTION_MODE | 161 | 294.42 | 1828.7 | 4018.4 | 0x0=1 | D | echo | cuda.rs | guest (NV_OK echo; recorded per guest TSG, the compute mode bound on its host TSG: preempt.rs) |
| 35 | access | cfg:R:0x0:0x4 | - | 5543 | 281.79 | 50.8 | 693.2 | - | pci-config | - | config.rs | - |
| 36 | host | 0x4f:0x0 | RM_UNMAP_MEMORY | 797 | 242.24 | 303.9 | 12543.5 | 0x0=9 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 37 | host | 0x4e:0x0 | RM_MAP_MEMORY | 815 | 233.19 | 286.1 | 16599.5 | 0x0=11 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 38 | host | 0x2b:0x90f1 | RM_ALLOC FERMI_VASPACE_A | 149 | 228.35 | 1532.5 | 8257.3 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 39 | host | 0x2a:0x20801303 | RM_CONTROL NV2080_CTRL_CMD_FB_GET_INFO_V2 | 917 | 228.32 | 249.0 | 8241.4 | 0x0=5 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 40 | host | 0x2a:0xa06c0105 | RM_CONTROL NVA06C_CTRL_CMD_PREEMPT | 650 | 200.22 | 308.0 | 1126.1 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 41 | access | cfg:R:0x0:0x78 | - | 10987 | 188.73 | 17.2 | 682.9 | - | pci-config | - | config.rs | - |
| 42 | rpc | 0x67 | GSP_RM_ALLOC | 10064 | 174.31 | 17.3 | 2487.4 | - | - | answered | boot.rs (service_queues), gsprpc.rs | - |
| 43 | access | cfg:R:0x0:0x60 | - | 10987 | 173.85 | 15.8 | 287.6 | - | pci-config | - | config.rs | - |
| 44 | access | cfg:R:0x0:0x68 | - | 10987 | 166.90 | 15.2 | 13.1 | - | pci-config | - | config.rs | - |
| 45 | host | 0x2a:0x20801235 | RM_CONTROL NV2080_CTRL_CMD_GR_GET_CTXSW_MODES | 602 | 144.60 | 240.2 | 2995.9 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 46 | host | 0x2a:0x2080130e | RM_CONTROL NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE | 564 | 128.39 | 227.6 | 3676.8 | 0x0=4 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 47 | access | cfg:R:0x0:0x34 | - | 5542 | 125.71 | 22.7 | 1.1 | - | pci-config | - | config.rs | - |
| 48 | control | 0x20801303 | NV2080_CTRL_CMD_FB_GET_INFO_V2 | 226 | 113.77 | 503.4 | 8247.4 | 0x0=2 | A | host-forwarded | nvml.rs | host (FB_GET_INFO_V2) rewritten to the profile size (identity-profile.md) |
| 49 | host | 0x2a:0x20801211 | RM_CONTROL NV2080_CTRL_CMD_GR_CTXSW_PREEMPTION_BIND | 161 | 107.37 | 666.9 | 739.4 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 50 | access | bar0:R:0xbb0000:0xbb0080 | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) | 2820 | 101.28 | 35.9 | 2.6 | - | usermode page, other | - | channel.rs (TIME_0/1, usermode page) | NV_VIRTUAL_FUNCTION_TIME_0/1 (usermode) |
| 51 | host | 0x2a:0x801714 | RM_CONTROL NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS | 161 | 100.96 | 627.1 | 752.8 | 0x0=1 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 52 | host | 0x2a:0xc36f0108 | RM_CONTROL NVC36F_CTRL_CMD_GPFIFO_GET_WORK_SUBMIT_TOKEN | 2828 | 96.54 | 34.1 | 545.5 | 0x0=21 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 53 | host | 0x52:0x0 | RM_GET_EVENT_DATA | 39755 | 96.20 | 2.4 | 140.4 | 0x0=2 | - | - | hostrm.rs, hostmem.rs, mirror.rs, shadow.rs | - |
| 54 | access | bar0:W:0x70f000:- | NV_PRAMIN | 2880 | 90.65 | 31.5 | 12.0 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 55 | access | bar0:W:0x706000:- | NV_PRAMIN | 2880 | 89.28 | 31.0 | 18.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 56 | control | 0xa06c0105 | NVA06C_CTRL_CMD_PREEMPT | 268 | 83.96 | 313.3 | 1137.4 | 0x0=2 | C | echo | cuda.rs | host (forwarded to the mirror object; the guest sees NV_OK) |
| 57 | access | bar0:W:0x712000:- | NV_PRAMIN | 2880 | 80.69 | 28.0 | 10.3 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 58 | access | bar0:W:0x70d000:- | NV_PRAMIN | 3000 | 77.94 | 26.0 | 76.7 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 59 | access | bar0:W:0x711000:- | NV_PRAMIN | 2880 | 77.92 | 27.1 | 8.8 | - | bar0-other | - | bar0.rs, boot.rs | NV_PRAMIN |
| 60 | control | 0x20802a0c | NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO | 339 | 77.60 | 228.9 | 4426.1 | 0x0=3 | A | host-forwarded | cuda.rs | host (forwarded as-is; NON_PRIVILEGED on the 615 host) |

| kind | rows | count | cost ms |
|---|---|---|---|
| access | 1327 | 1155932 | 112682.10 |
| host | 59 | 617892 | 72770.86 |
| shadow | 6 | 544364 | 64601.64 |
| window | 22 | 1180828 | 17360.54 |
| rpc | 5 | 26332 | 6227.64 |
| control | 68 | 6391 | 1019.31 |
| alloc | 17 | 10066 | 58.49 |
