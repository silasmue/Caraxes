<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Isolation and Xids

Scope: the isolation rules a Caraxes device enforces on the shared RTX 2070 (reservation, comptags, privilege, teardown, scrub) and the catalogue of every Xid recorded, with cause and fix.

## Facts

### Isolation rules

- D11: for real execution the device reserves a fixed-address, contiguous range of the card's VRAM sized to the profile, and refuses the reservation when the host would be left without enough free VRAM [docs/caraxes/README.md §Decisions D11].
- Slot n lies n profile sizes below the 2 MiB-aligned end of the host FB heap region, and the device asks for exactly its slot and refuses anything else the RM hands back [docs/caraxes/parallel.md §1, §3d; run: caraxes/runs/parallel-2026-09-24/e1-two-vms].
- The headroom is 1024 MiB against free VRAM = min(HEAP_FREE, nvidia-smi): a third 2 GiB VM was refused with "would leave the host 569 MiB ... shortfall 455 MiB" before its socket existed, and the two running VMs were unaffected [run: caraxes/runs/parallel-2026-09-24/e5-refusal; docs/caraxes/parallel.md §5].
- A fixed range the host RM cannot give is refused by the RM itself with `NV_ERR_NO_MEMORY`, with no floating fallback [docs/caraxes/parallel.md §8.4; run: caraxes/runs/mapping-2026-09-24/f1-concurrent].
- D16: no guest comptag line reaches the host, by three layers: no compression in every FB/memsys/caps answer, a PTE filter forcing uncompressed kinds with COMPTAGLINE 0, and an audit sweep after each gate run; the reservation is never allocated with `NVOS32_ATTR_COMPR` [docs/caraxes/README.md §Decisions D16].
- A D16 violation needs a host reboot (comptag store reset), and no host-side switch exists in the open kernel RM because compression policy lives in GSP firmware [docs/caraxes/README.md §Decisions D16].
- The D16 audit of the clean P20 runs read only kind 0x00/0x06 with comptagline 0 (vm1 505531 + 104344 PTEs, vm2 246904 + 67958) [run: caraxes/runs/parallel-2026-09-24/e3-isolation; docs/caraxes/parallel.md §3b].
- D18: the physical GPU may reach only this VM's VRAM reservation and guest-RAM pages the device registered, through shadowed and validated guest page tables; the mechanics are in [memory-model.md](memory-model.md) [docs/caraxes/README.md §Decisions D18].
- Every GR and CE mapping passes the D16/D17 filter, and the CE channels run on the filtered shadow VAS, so no host channel walks unfiltered guest tables [run: caraxes/runs/mapping-2026-09-24/d1-isolation; docs/caraxes/parallel.md §8.3].
- The cross-VM pattern test found 0 records and 0 non-zero words of the other VM's pattern in both directions [run: caraxes/runs/parallel-2026-09-24/e3-isolation; run: caraxes/runs/mapping-2026-09-24/f1-concurrent].
- The guest RM does not scrub its own frees, so stale data between processes of one VM stays a guest matter, not a cross-VM one [docs/caraxes/parallel.md §3d].
- Fail-closed teardown frees in the order mirror objects, descriptors, reservation, client, and stops at the first failure, counting any earlier failed host free as well [docs/caraxes/parallel.md §4a].
- A fail-closed teardown logs `FAIL-CLOSED teardown`, keeps the 4 guest RAM descriptors (4096 MiB pinned) and the reservation, exits with status 3, and the fd close lets the host RM free the client with 0 Xid [run: caraxes/runs/parallel-2026-09-24/e4-failclosed].
- D12: the device process runs as uid 1000 with `CAP_SYS_ADMIN` permitted but not effective (`setcap cap_sys_admin+p` on its copy, the only root action) and raises it on the calling thread only around specific ioctls: `GET_SURFACE_PHYS_ATTR` and the other D12 controls of `HostMemory` [docs/caraxes/memory-model.md §3 Launcher, Privilege].
- The host-exec tool scoped the same way raised it only around `GET_SURFACE_PHYS_ATTR` (flags 0x4), `DMA_SET_PAGE_DIRECTORY` (0x14004) and `GPU_PROMOTE_CTX` (0x10244), 5 windows per A-F run [docs/caraxes/host-exec.md §Privilege (D12)].
- The RM takes the privilege level per ioctl from `osIsAdministrator()` and refuses a PRIVILEGED control below `USER_ROOT` [615 tree: src/nvidia/arch/nvalloc/unix/src/escape.c:384; 615 tree: src/nvidia/src/kernel/rmapi/control.c:688-694].
- The TLB invalidator channel is allocated in one `CAP_SYS_ADMIN` window so its privileged `MEM_OP` is accepted [docs/caraxes/execution.md §6; run: caraxes/runs/e5b2-2026-09-23/d-ce-inval2].
- D12 is narrower than v1's `LEA_ADMIN_PRIV=1`, which keeps the capability effective for the whole process [docs/caraxes/sysmem-iova.md §2].
- Scrub on free is on by default unless `RmDisableScrubOnFree` is set, and is turned off only for Windows WDDM, simulation, MODS, a vGPU host, a non-SR-IOV virtual guest, the GSP firmware build and SLI [615 tree: src/nvidia/src/kernel/gpu/mem_mgr/mem_mgr.c:218-225; 615 tree: src/nvidia/src/kernel/gpu/mem_mgr/arch/maxwell/mem_mgr_gm107.c:1506-1515].
- The reservation is a PMA allocation with `PMA_ALLOCATE_SPECIFY_ADDRESS_RANGE` and without `SKIP_SCRUB`, so its free marks every frame `ATTRIB_SCRUBBING` and queues a CE memset [615 tree: src/nvidia/src/kernel/mem_mgr/video_mem.c:240-261; 615 tree: src/nvidia/src/kernel/gpu/mem_mgr/phys_mem_allocator/phys_mem_allocator.c:1463; 615 tree: src/nvidia/src/kernel/gpu/mem_mgr/phys_mem_allocator/phys_mem_allocator.c:1526].
- A later allocation of those frames waits for the scrubber [615 tree: src/nvidia/src/kernel/gpu/mem_mgr/phys_mem_allocator/phys_mem_allocator.c:1065].
- Measured: the 2 GiB free returns in 1 ms, the next owner's allocation of the same range takes 38, 36 and 23 ms, and a sampled scan after re-reservation reads 0 pattern records and 0 non-zero dwords [run: caraxes/runs/parallel-2026-09-24/e4-scrub].
- BAR1 (256 MiB) is one host-wide resource with no per-client quota, so each device is budgeted `CARAXES_BAR1_WINDOWS` x `CARAXES_BAR1_WINDOW_MIB` (default 2 x 32 MiB) and evicts its own windows on a refused map [docs/caraxes/parallel.md §2.3].
- The RM's non-stall completion notifier is per engine and host-wide, a timing side channel (32k events in 3 minutes on an idle guest) [docs/caraxes/README.md §Ideas P7].

### Xid catalogue

| Xid | Trigger | Cause | Fix | Evidence |
|---|---|---|---|---|
| 32 PBENTRY (`intr0 0x00040000`) | first doorbell of each CE mirror | BAR1 is write-combined, the PBDMA fetched stale VRAM | `Window::flush` (`sfence` + read-back) before every host doorbell and TLB invalidate | run: caraxes/runs/e5b2-2026-09-23/c-pbentry; execution.md §2 |
| 32 DEVICE (`intr 0x00800000`) x6, guest Xid 154 | first push of 6 UVM CE mirrors | `SET_OBJECT(GP100_UVM_SW)` on subchannel 5, a class refused to the mirror (0x1b) | UVM-SW channels stay in software (`Mirror::eligible`) | run: caraxes/runs/e5b2-2026-09-23 (a2); exec-formats.md §1.0 |
| 32 x8 | GR mirror with a narrowed RM-internal range | host `TURING_COMPUTE_A` alloc refused 0x51, channel scheduled with its CE object only | abandoned for the shadow VAS | run: caraxes/runs/e5b2-2026-09-23 (b2b) |
| 38 (`0000c597`) x9 | first compute push on a host GR channel (c1, c2) | FECS loads the host-built context, which the guest root does not map | GR channels on a host-managed shadow VAS (D17 route 4) | run: caraxes/runs/e5b2-2026-09-23 (b2, d-gr-c2); execution.md §6 |
| 31 CE2 FAULT_PDE | fresh CE copy destination | the guest's register TLB invalidate never reached the physical TLB | `MEM_OP MMU_TLB_INVALIDATE` of the guest PDB from a privileged device channel | run: caraxes/runs/e5b2-2026-09-23 (d-ce-token) |
| 32 METHOD (`intr 0x00200000`) | the invalidator's `MEM_OP` | privileged method on a non-privileged channel | allocate that channel under `CAP_SYS_ADMIN` | run: caraxes/runs/e5b2-2026-09-23 (d-ce-inval) |
| 32 HCE (`intr1 0x8`) / PBENTRY | first CE `LAUNCH_DMA` | the per-doorbell 20 ms poll diagnostic disturbed the datapath | diagnostic opt-in (`CARAXES_MIRROR_DIAG=1`) | run: caraxes/runs/e5b2-2026-09-23 (d-ce-inval2, d-core-fast) |
| 31 GR FE FAULT_PDE | first GR push on the shadow VAS | set-A page not yet mirrored | dynamic set-A mirror with unmap-before-remap | run: caraxes/runs/e5b2-2026-09-23 (d-gr-launch5) |
| 31 FAULT_PDE x17 | a second CUDA context (gpu gate robustness) | one shadow VAS mirrored the wrong context | one `ShadowSpace` per guest root | run: caraxes/runs/e5b2-2026-09-24; execution.md §6 |
| 31 PRIV_VIOLATION (`GPCCLIENT_PROP_0`) x2 | Weston GL at 1080p | the 3D class's global context buffers collided with set A | size-0 promoted buffers excluded from set A | run: caraxes/runs/display-e-2026-09-24 (e14-e17) |
| 13 3D-Z KIND Violation | GNOME on the NVIDIA EGL | depth/stencil kinds dropped in the shadow mappings | kinds 0x01-0x05 passed as `NVOS46_FLAGS_PAGE_KIND_OVERRIDE` | run: caraxes/runs/display-e-2026-09-24 (e18, e19) |
| 31 FAULT_PTE (PROP_0) | one in six GNOME starts | the trapped GPPut store reached VRAM before the shadow worker applied the invalidate's plan | GPPut barrier (`gp_put_barrier`) waits for pending plans; 0 Xid in 5 + 5 GNOME starts after | display-e.md §9; run: caraxes/runs/display-page-2026-09-24 |
| 31 CE2 FAULT_PTE (cuda-hostreg, 719) | pinned buffer on swiotlb rounding-margin pages | the guest's alias IOVAs were missing from the device's table, the filter refused the leaves | device follows the guest's alias (`DmaTable::aliases_of`); before 2 Xid, after 0 | run: caraxes/runs/xid31-hostreg-2026-09-24; memory-model.md §2 |
| 31 GPC FAULT_PDE VIRT_WRITE | cuDNN step or `allocburn.py` writing fresh 2 MiB regions while GR is loaded | USERD in the BAR1 pool: the GPPut store does not trap and the host channel ran before the shadow had the invalidate's mappings | invalidates of untrapped-USERD roots are applied before the guest continues (`untrapped.rs`); 20 of 20 on both guests, 0 Xid | run: caraxes/runs/xid31-pool-2026-09-25; bar1-pool.md §9 |
| 32 (CE) + 13 Class Mismatch | GA106 costume, libcuda's first push | `SET_OBJECT` 0xC7B5/0xC7C0 in the method stream against TU106 engines | none: needs pushbuffer rewrite, the ladder stops | run: caraxes/runs/costume-ladder-2026-09-24/r2-ga106 |
| 31 (guest 719) | deliberate store to an unmapped address | intended fault; the device posts no RC event | none needed | run: caraxes/runs/coverage-2026-09-25/map |
| 109 | deliberate kill -9 with a kernel running | a 3.2 s TSG preempt, the host's ctx-switch timeout | none; the next process works | run: caraxes/runs/coverage-2026-09-25/map |
| guest 143 | GH100 costume, pure emulation | FSP boot poll reads Hopper's `NV_THERM_I2CS_SCRATCH` at 0x200BC, not GB20x's 0xAD00BC | `chips/gh100/boot.toml` overlay | run: caraxes/runs/costume-ladder-2026-09-24/r4-gh100 |
| guest 119 + 154 | 595 guest, pure emulation, nvidia-smi's 67396-byte control | the device used the 610 `maxRpcSize` (65520, 595 has 65488) | `ElementLayout::max_rpc_size()` | run: caraxes/runs/guest-driver-2026-09-24/595-static |

- Xid 38 is not defined in the open headers, which jump from 36 to 39 [615 tree: src/common/sdk/nvidia/inc/nverror.h:54-55].
- The guest kernel sets "OS reboot required" when a UVM-owned channel gets an RC, which is the guest Xid 154 of a2 [615 tree: src/nvidia/src/kernel/gpu/rc/kernel_rc_notification.c:256-262; docs/caraxes/gsp-plan/events.md §R5].
- The device's own RC watchdog raises Xid 45 (`ROBUST_CHANNEL_PREEMPTIVE_REMOVAL`) on stalled guest GR channels, a device choice with no host engine reset [615 tree: src/common/sdk/nvidia/inc/nverror.h:61; docs/caraxes/gsp-plan/d-device-model.md §Round 2].
- Guest-side Xids stay in the guest: `LOG_OOB_XID` is journaled with sanitized text and nothing goes to the host's OOB log [docs/caraxes/gsp-plan/d-device-model.md §Round 2].

### Contained to the device pid

- A GSP-reported Xid is built inside GSP-RM and printed verbatim by the kernel RM with `pid=` and `name=` of the channel's owner, here `name=vfio-user-nvgpu` [615 tree: src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:849-883; docs/caraxes/exec-formats.md §1.3; docs/caraxes/display-e.md preamble].
- "Contained" means every host Xid line names the device process's pid, lands on that VM's own mirror channels, and leaves the host desktop working [docs/caraxes/execution.md §6; docs/caraxes/experiments.md §E5b part 2 (g)].
- Exception detail lines (ESR, "Class ... Mismatch") may carry the pid of whatever process was current in the interrupt path, or none: 24 of 40 lines in the GA106 run [run: caraxes/runs/costume-ladder-2026-09-24/r2-ga106/gpu/host-xid.txt].
- The P20 checker counts Xid containment per device, and a Xid on a device outside the pair under test counts as "not contained" [docs/caraxes/parallel.md §3a, §8.4].
- Every host Xid of the untrapped-GPPut reproductions was on the device's own pid (615 pid 2861668, 610 pid 2951267), and the host watch saw no other kernel line [run: caraxes/runs/xid31-pool-2026-09-25; docs/caraxes/bar1-pool.md §9].
- The host desktop is judged by a host watch sampling host `nvidia-smi` (max 0.087 s over 70 samples on two VMs) [run: caraxes/runs/mapping-2026-09-24/f1-concurrent/b-gate/hostwatch.txt].
- Host impact without any Xid is possible: 732088 refused host BAR1 mappings from a window leak, and a half-million-mapping VA teardown that timed out host `nvidia-smi` after 20 s and restarted the desktop's electron GPU process [docs/caraxes/parallel.md §2.3, §2.4].

## Consequence for Caraxes

- Every new run collects the host Xid lines since its start and compares them with the device pid, and stops at the first Xid outside it.
- A Xid on the device pid is a device bug with a VM-local blast radius; the fix goes into the mapping or ordering path, never into a host-side reset.
- Host resources without quota (BAR1, RM mappings, GPU lock time) need per-device budgets, because they break the desktop without raising a Xid.
- The reservation relies on the host's scrub on free, so a host with `RmDisableScrubOnFree` set is outside the supported configuration.

## Open

- The device does not yet refuse to reserve when `RmDisableScrubOnFree` is set, because an unprivileged client cannot read the key [docs/caraxes/parallel.md §4b].
- There is no QoS: budgets exist for BAR1 only, not for time, channels or RM work [docs/caraxes/parallel.md §6].
- Malicious guests are untested; every isolation run used the stock driver [docs/caraxes/parallel.md §6].
- Idle interrupt forwarding runs at 331 MSIs per second from other VMs' work, and progress-gated forwarding is still in the backlog [docs/caraxes/coverage-map.md §4 row 9].
- The 8 DMA addresses vm1's device refused in the mapping-tree concurrency check have no known source [docs/caraxes/parallel.md §8.4].
- The third-VM refusal was not repeated on the mapping tree [docs/caraxes/parallel.md §8.4].
- The VRAM store should close its BAR1 windows before the reservation is freed (two `RM_UNMAP_MEMORY` returned 0x33 after it) [docs/caraxes/parallel.md §8.4].
- Holding only invalidate plans that map something new would recover the free cost of the untrapped-USERD rule (cuMemFree 64 MiB 791 to 2402 us) and is not built [docs/caraxes/bar1-pool.md §9].
- What a stalled watchdog channel sends once `RmRcWatchdog` is on, including Xid 10 for a stalled vblank, is unmeasured (Q7) [docs/caraxes/gsp-plan/d-device-model.md §8 Q7].
