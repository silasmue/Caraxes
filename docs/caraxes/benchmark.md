<!-- SPDX-License-Identifier: MIT -->
# Benchmark: Leandro v1, Leandro v2 (Caraxes) and native (P13)

Written 2026-09-24. Goal P13 in [README.md](README.md) (Ideas table): measure the two paths
against the bare card. Harness: `caraxes/tools/bench-run.sh`, `bench-cu.c`, `bench-v1vm.sh`,
`bench-summary.py`. Raw outputs, summaries and the command log:
`caraxes/runs/bench-2026-09-24/` (`commands.txt` lists every command in order).

**Status.** Complete, 2026-09-24. Final series on a quiet host, n=5, native, v1 and v2 interleaved
(§4.1), v2 at main `8d44ee6`, plus always-resync rows for v2 (§4.1.1). Earlier series under
load are kept as §4.2. The short answer (§5): v1 runs GPU-bound work at native speed and pays
13-35 us per forwarded RM call, plus a flat 100 ms per blocking sync. v2 pays nothing per RM
call, about 0.25 ms per submission, and 20-55 ms of device CPU per mapping change; that makes
it 2-15x slower than native on torch and allocation-heavy work today, and native on bulk
transfers and long kernels. Found on the way: a v1 pinned-page retention bug (§6) and three
v2 limitations (§7; the flush wait is fixed, the single reservation has slots now).

## 1. What is compared

| | v1 (Leandro) | v2 (Caraxes) | native |
|---|---|---|---|
| Path | API forwarding: guest `nvrm_nodes` + `virtio_nvrm` forward every RM ioctl and mapping to `vhost-user-nvrm`, which replays it on the host RM | synthetic PCI device: the stock `nvidia.ko` in the guest drives `vfio-user-nvgpu`, which backs the FB with reserved host VRAM and mirrors the guest's channels onto host channels | the same programs on the host |
| Hypervisor | Cloud Hypervisor v53.0 + `patches/0001-0004` (`vendor/cloud-hypervisor/target/release/cloud-hypervisor`), `--generic-vhost-user device_type=60` | Cloud Hypervisor v53.0.0 (nix, `/mnt/vmstore/leandro/cloud-hypervisor/bin/cloud-hypervisor`), `--user-device` | - |
| Device process | `vhost-user-nvrm` built from `5de9b40` (`target/bench/release`, `caraxes/tools/bench-v1vm.sh`) | `vfio-user-nvgpu` from `target/release`, `tools/caraxes-vm.sh`; final series: main `8d44ee6` (sha256 `249c15ca...`, with the NV_UFLUSH fix `7faac21`); earlier series: §4.2 | - |
| Configuration | defaults (`LEA_MAX_PIN_TOTAL_MIB` 1024) | `VM=vm1 CARAXES_BOOT_MODEL=1 CARAXES_HOST_RM=1 CARAXES_HOST_EXEC=1 CARAXES_MIRROR_CHANNELS=1 CARAXES_MIRROR_GR_SHADOW=1 CARAXES_RESERVE_VRAM=1 CARAXES_DMA=register CARAXES_FB_MIB=4096`, the configuration of the first kernel launch (`caraxes/runs/e5b2-2026-09-24/d-gr-mirror1`, `d-launch4-2`) | - |
| Guest | baked Ubuntu 24.04 image `guest-baked-20260801-610.57.04-20260820-105733` (kernel 6.8.0-136), 8 vCPUs, 4 GiB shared memory, libcuda 610.57.04 in `/opt/nvrm/lib`, venv torch 2.13.0+cu130 | same image and size | Arch host, kernel 7.2.4, Ryzen 7 3800X (8 cores / 16 threads), 32 GiB, driver 610.57.04, libcuda `/usr/lib`, torch 2.13.0+cu130 in `../Leandro/vendor/hostvenv` |

GPU: the one RTX 2070 (TU106, 8 GiB) that also drives the host desktop (Wayland; Discord and
VS Code hold GPU contexts).

**Same binaries.** `bench-cu` (this benchmark's driver-API program) and the Leandro-Test
probes `nvprobe`, `ioctlping`, `ctrlping` were built once in the v1 guest (glibc 2.39, gcc 13.3,
`cuda.h` 13.0 from the guest venv, `-L/opt/nvrm/lib -lcuda`) and copied to `target/bench/bin`;
`bench-run.sh stage` copies them into each guest. The same files run on the host (sha256 at
the top of every `series.txt`). The Python probes are Leandro-Test's `probe/python`
(`mmsweep.py`, `rlprobe.py`), read in place on the host and synced into the guests by
`tools/caraxes-vm.sh sync-tests`; torch is 2.13.0+cu130 on all three.

## 2. Method

* Each metric N=5 runs. A table cell is `median [min-max] n=runs`; each run itself reports
  a median over its internal repetitions (copies, pings, launches). Ratio = median of the
  path / median of native (for GB/s and GFLOP/s above 1 is better, for times below 1).
* Interleaved: the rep loop is outermost, and inside it every metric runs on every target
  back to back (native, v1, v2). Drift (card temperature, host load) then spreads over all
  targets instead of landing on one. `bench.sh transport` in Leandro-Test does the same for
  the same reason.
* Restart per rep, on both paths: v1 needs a fresh VM and backend for each rep (§6), and v2
  gets the same (`V2_RESTART=1`: `sync`, `caraxes-vm.sh down`, `up`, `probe`, nodes) so that
  the two paths run under the same conditions and every rep starts on a fresh device process.
  Native runs have no equivalent restart. The guest page cache is cold after a restart, which
  affects process wall times that include Python imports.
* Timing inside the target (guest `date +%s%N`, or the probe's own `CLOCK_MONOTONIC`), never
  across ssh.
* Device-process cost (metric 8): `utime+stime` from `/proc/<pid>/stat` of the device process
  (v1 `vhost-user-nvrm`, v2 `vfio-user-nvgpu`) and of the VMM (`cloud-hypervisor`, which
  includes the guest's own vCPU time) before and after each run; `VmRSS`/`VmHWM` after it.
* Conditions per run (header of each raw file): time, GPU temperature, clocks, P-state,
  utilization, memory used, persistence mode (from `nvidia-smi`, a host RM client), the
  host's device and VMM processes, the 1-min load average. A run is marked **concurrent**
  when another VM's device process used more than 20 % of a CPU during it.
* Host GPU access: only as an RM/CUDA client (the benchmark programs and `nvidia-smi`
  queries). The host modules, their parameters and registers are not touched. Reads of host
  state: `/proc/driver/nvidia/params` and `version` (the v1 guest needs the parameter file,
  as in the v1 procedure), `nvidia-smi --query-*`.

### Metrics and commands

`bench-run.sh series` runs, inside the target's bench directory (`target/bench/bin` on the
host, `~/bench` in a guest, with `LD_LIBRARY_PATH=/opt/nvrm/lib` in guests):

| # | Metric | Command |
|---|---|---|
| 1 | kernel launch, 30M-cycle kernel | `NVPROBE_ITERS=100 NVPROBE_SCHED=auto\|blocking ./nvprobe 4` (100 launches, each followed by `cuCtxSynchronize`, after one warm-up) |
| 1 | launch overhead, short kernel | the same with `NVPROBE_CYCLES=1000` |
| 2 | H2D, D2H, D2D bandwidth | `./bench-cu copy 10`: synchronous `cuMemcpyHtoD/DtoH/DtoD` of 1, 16, 256 MiB (50/20/10 timed copies after one warm-up), pinned (`cuMemHostAlloc`) and pageable (`malloc`) host memory, data checked |
| 3 | matmul throughput | `NVMM_SIZES=1024,2048,4096 NVMM_ITERS=10 python mmsweep.py` (fp32 `a @ b`, cuBLAS) |
| 4 | rlprobe | `NVRL_EPISODES=50 python rlprobe.py`: stage-4 time, `mean10` (seeded, a correctness check), transfers/s, process wall time |
| 5 | startup | `bench-run.sh startup v1\|v2`: module load (v2: `rmmod nvidia`, function-level reset, `modprobe nvidia`, then `nvidia-uvm` and the device nodes; v1: reload `nvrm_nodes` + `virtio_nvrm`) to the first `nvprobe 0` that passes; `./bench-cu init` (cuInit, cuCtxCreate, cuCtxDestroy in one process); `time ./nvprobe 0` and `time ./nvprobe 1` (bash `time`: the host has no `/usr/bin/time`) |
| 6 | RM-call cost | `./ioctlping 10000` (`NV_ESC_CHECK_VERSION_STR` on `/dev/nvidiactl`), `./ctrlping 2000 64 512 4096 16384` (`RM_CONTROL` `GET_BUILD_VERSION`, the probe reports the total parameter bytes 232/1576/12328/49192) |
| 7 | VRAM churn | `./bench-cu churn 20`: per cycle 64 x 1 MiB and 8 x 64 MiB `cuMemAlloc`, then `cuMemFree` in reverse order |
| 8 | device-process CPU and RSS | recorded around every run above |

`vrampress` and `vramcap.py` were not used for metric 7: both size themselves from the free
memory the target reports, which differs between the host (about 2 GiB free beside the
desktop and the other VMs) and a 4 GiB guest, so they are no fixed pattern.

## 3. Conditions

| Series | Time (CEST) | Targets | Host state | Label |
|---|---|---|---|---|
| **final** | 04:46-05:49 | native, v1, v2 interleaved, all metrics, restart per rep on both paths | coordinator's "quiet host": no foreign build, vm0 (display agent) up but not using the physical GPU, vm2 down; load average 1.7-4.4 (mostly our own two VMs), about 17 GiB RAM available. No other VM's device was active during any v1 or v2 run. In 7 of 65 native runs the v1 or v2 device process was still busy from its previous run (the marker counts every device process when the target is native) | **quiet** |
| final-always-resync | 05:51-06:26 | native, v2 with `CARAXES_SHADOW_ALWAYS_RESYNC=1`, metrics 1-4 | as final, v1 VM powered off | quiet |
| series3-native-quiet | 03:11 | native only | quiet host, vm0 only | quiet |
| series1b | 00:16-00:37 | native, v1 (restart per rep) | vm2 (CUDA agent) active in 27 of 65 runs; a foreign nix build (`rustc`, later qemu VMs) with load average 5-16; swap 28-32 of 32 GiB | **under load** |
| series2 | 01:40-02:09 | native, v2 (restart per rep) | vm2 down; RAM available 3-7 GiB, swap refilling; the root filesystem ran full at 01:45. Restarts before reps 3-5 failed; rep 2 ran on another device build (§4.2) | **under load** |
| series1 | 00:07-00:13 | native, v1 | as series1b, without restart; v1 fails from rep 2 on (§6) | under load, evidence only |

GPU state at run start in the final series: 49-57 °C; P0, or P5/P8 when the card had idled
before the run (it clocks up within the run); persistence mode disabled for every target.
Device builds: v2 `vfio-user-nvgpu` at main `8d44ee6` (the NV_UFLUSH fix `7faac21` and the
display merge included), sha256 `249c15ca...`, unchanged during the series (target file
04:37:29, the VM's privileged copy 04:37:45). v1 `vhost-user-nvrm` at `5de9b40`, no v1
change since. Binaries on all three: `bench-cu` `c5b48cf1...`, `nvprobe` `2735d2a9...`,
`ioctlping` `138c5073...`, `ctrlping` `68449e9e...`, `kernels.ptx` `1f921680...` (sha256
at the top of every `series.txt`).

## 4. Results

### 4.1 Final series (quiet host, n=5)

`caraxes/runs/bench-2026-09-24/final/`, 04:46-05:49. Native, v1 and v2 interleaved in every
rep; v1 and v2 each restarted before every rep (§2). The startup rows come from
`bench-run.sh startup v1|v2 --out final` just before the series, on the same builds. Tables:
`python3 caraxes/tools/bench-summary.py final/v1 final/v2 final/native`
(`summary-final.md`). A cell is `median [min-max] n=runs`. The last two columns are the path's
median divided by native's. For the (5) module-load rows and the (8) rows the native
column is n/a, since there is no guest module and no device process natively. The (8) VMM
CPU includes the guest's own vCPU time (torch, Python), so it is not overhead by itself.
All values passed their checks: nvprobe's vecAdd result, `bench-cu` data compare
(`bad_words=0`), rlprobe `mean10` 56.9 everywhere (seeded), the same mmsweep checksums.

Sources: v1=final/v1, v2=final/v2, native=final/native
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v1 0/65, v2 0/65, native 7/65
Host 1-min load average at run start, median [min-max]: v1 3.05 [1.74-4.39] n=65, v2 3.05 [1.91-4.39] n=65, native 3.05 [1.74-4.39] n=65

#### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 16.0 [15.9-16.1] n=5 | 17.6 [17.3-18.5] n=5 | 16.2 [16.2-16.5] n=5 | 0.99 | 1.08 |

#### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 100 [100-100] n=5 | 18.7 [18.5-101] n=5 | 16.4 [16.3-16.6] n=5 | 6.12 | 1.14 |

#### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 0.007 [0.007-0.008] n=5 | 0.257 [0.155-0.585] n=5 | 0.006 [0.006-0.010] n=5 | 1.17 | 42.83 |

#### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| ms/launch | ms (low is better) | 8.50 [0.036-99.6] n=5 | 0.251 [0.223-12.1] n=5 | 0.018 [0.014-0.019] n=5 | 472.22 | 13.94 |

#### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 12.2 [12.2-12.2] n=5 | 3.33 [2.15-3.58] n=5 | 12.1 [12.0-12.1] n=5 | 1.01 | 0.28 |
| d2h pinned 1 MiB | GB/s (high is better) | 12.4 [12.3-12.4] n=5 | 3.38 [1.88-3.55] n=5 | 12.3 [12.3-12.3] n=5 | 1.01 | 0.27 |
| d2d device 1 MiB | GB/s (high is better) | 145 [137-148] n=5 | 6.60 [5.85-7.40] n=5 | 135 [132-137] n=5 | 1.07 | 0.05 |
| h2d pageable 1 MiB | GB/s (high is better) | 7.35 [7.27-7.43] n=5 | 2.84 [1.86-2.97] n=5 | 7.39 [7.34-7.41] n=5 | 0.99 | 0.38 |
| d2h pageable 1 MiB | GB/s (high is better) | 6.50 [5.81-6.55] n=5 | 1.97 [1.72-2.85] n=5 | 6.44 [6.30-6.48] n=5 | 1.01 | 0.31 |
| h2d pinned 16 MiB | GB/s (high is better) | 13.0 [13.0-13.0] n=5 | 3.13 [3.08-3.17] n=5 | 13.0 [13.0-13.1] n=5 | 1.00 | 0.24 |
| d2h pinned 16 MiB | GB/s (high is better) | 13.1 [13.0-13.1] n=5 | 3.16 [3.15-6.35] n=5 | 13.1 [13.1-13.1] n=5 | 1.00 | 0.24 |
| d2d device 16 MiB | GB/s (high is better) | 177 [165-182] n=5 | 79.0 [4.42-82.3] n=5 | 183 [182-183] n=5 | 0.97 | 0.43 |
| h2d pageable 16 MiB | GB/s (high is better) | 8.62 [8.28-10.5] n=5 | 4.23 [3.37-4.41] n=5 | 10.6 [9.46-11.0] n=5 | 0.81 | 0.40 |
| d2h pageable 16 MiB | GB/s (high is better) | 8.88 [7.38-8.99] n=5 | 4.10 [3.52-4.36] n=5 | 9.13 [8.81-9.19] n=5 | 0.97 | 0.45 |
| h2d pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=5 | 13.0 [12.9-13.0] n=5 | 13.1 [13.1-13.1] n=5 | 1.00 | 0.99 |
| d2h pinned 256 MiB | GB/s (high is better) | 13.2 [13.2-13.2] n=5 | 13.1 [13.0-13.1] n=5 | 13.2 [13.2-13.2] n=5 | 1.00 | 0.99 |
| d2d device 256 MiB | GB/s (high is better) | 186 [185-188] n=5 | 177 [171-180] n=5 | 186 [183-188] n=5 | 1.00 | 0.95 |
| h2d pageable 256 MiB | GB/s (high is better) | 8.92 [8.18-9.34] n=5 | 4.12 [3.27-4.33] n=5 | 9.22 [8.84-9.45] n=5 | 0.97 | 0.45 |
| d2h pageable 256 MiB | GB/s (high is better) | 8.82 [8.42-9.07] n=5 | 4.00 [3.16-4.16] n=5 | 9.12 [8.75-9.30] n=5 | 0.97 | 0.44 |

#### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 5218 [3803-5267] n=5 | 1351 [1326-1386] n=5 | 5041 [4775-5333] n=5 | 1.04 | 0.27 |
| N=2048 | GFLOP/s (high is better) | 5598 [5316-5617] n=5 | 774 [678-840] n=5 | 5570 [5301-5911] n=5 | 1.01 | 0.14 |
| N=4096 | GFLOP/s (high is better) | 6015 [5899-6512] n=5 | 3287 [3117-4940] n=5 | 6512 [5962-6708] n=5 | 0.92 | 0.50 |

#### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| mean10 (reward, not a time) |  | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=5 | 56.9 [56.9-56.9] n=5 | 1.00 | 1.00 |
| process wall (incl. torch import) | s (low is better) | 88.1 [53.4-107] n=5 | 33.2 [32.8-33.3] n=5 | 4.43 [4.34-4.61] n=5 | 19.89 | 7.49 |
| stage4 (50 training episodes) | s (low is better) | 1.50 [1.40-1.80] n=5 | 18.6 [18.3-18.9] n=5 | 1.20 [1.10-1.30] n=5 | 1.25 | 15.50 |
| H2D+D2H transfers per second | 1/s (high is better) | 1963 [1572-1991] n=5 | 154 [152-157] n=5 | 2417 [2237-2523] n=5 | 0.81 | 0.06 |

#### (5) Module load to first successful cuInit (in the guest)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| module load | ms (low is better) | 27.0 [25.0-29.0] n=5 | 3236 [3070-3809] n=5 | n/a | - | - |
| load to first cuInit | ms (low is better) | 192 [131-240] n=5 | 3401 [3265-4056] n=5 | n/a | - | - |

#### (5) cuInit / cuCtxCreate inside one process (bench-cu init)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| cuInit | ms (low is better) | 31.1 [27.5-35.3] n=5 | 149 [140-182] n=5 | 77.4 [71.6-116] n=5 | 0.40 | 1.92 |
| cuCtxCreate | ms (low is better) | 159 [149-164] n=5 | 4727 [4555-4975] n=5 | 188 [183-197] n=5 | 0.84 | 25.14 |
| cuCtxDestroy | ms (low is better) | 87.7 [73.9-90.5] n=5 | 3390 [3356-3450] n=5 | 89.0 [83.7-93.7] n=5 | 0.99 | 38.07 |

#### (5) nvprobe 0 process, bash time

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| nvprobe 0 (cuInit) wall | ms (low is better) | 165 [151-203] n=5 | 166 [161-175] n=5 | 198 [128-209] n=5 | 0.83 | 0.84 |
| nvprobe 0 (cuInit) user+sys | ms (low is better) | 5.00 [5.00-6.00] n=5 | 166 [161-174] n=5 | 127 [112-138] n=5 | 0.04 | 1.31 |

#### (5) nvprobe 1 process, bash time

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| nvprobe 1 (cuInit+ctx) wall | ms (low is better) | 330 [310-347] n=5 | 8303 [7872-8452] n=5 | 397 [377-405] n=5 | 0.83 | 20.91 |
| nvprobe 1 (cuInit+ctx) user+sys | ms (low is better) | 38.0 [37.0-42.0] n=5 | 8300 [7867-8478] n=5 | 336 [333-362] n=5 | 0.11 | 24.70 |

#### (6) RM call: ioctlping, 10000 x NV_ESC_CHECK_VERSION_STR

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| CHECK_VERSION_STR p50 | us (low is better) | 16.0 [13.6-18.8] n=5 | 1.37 [1.33-1.98] n=5 | 2.68 [2.47-2.95] n=5 | 5.97 | 0.51 |
| CHECK_VERSION_STR p99 | us (low is better) | 37.6 [29.1-70.9] n=5 | 3.13 [3.07-6.97] n=5 | 3.57 [2.99-4.11] n=5 | 10.52 | 0.88 |
| CHECK_VERSION_STR mean | us (low is better) | 17.4 [16.1-22.4] n=5 | 1.53 [1.47-2.12] n=5 | 2.79 [2.51-3.14] n=5 | 6.23 | 0.55 |

#### (6) RM call: ctrlping, 2000 x RM_CONTROL GET_BUILD_VERSION per size

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| RM_CONTROL 232 B p50 | us (low is better) | 21.6 [18.5-21.9] n=5 | 2.39 [2.35-2.45] n=5 | 4.41 [4.32-5.52] n=5 | 4.89 | 0.54 |
| RM_CONTROL 232 B p99 | us (low is better) | 53.1 [36.9-127] n=5 | 3.49 [3.24-7.21] n=5 | 5.63 [5.44-7.02] n=5 | 9.44 | 0.62 |
| RM_CONTROL 1576 B p50 | us (low is better) | 20.2 [16.9-24.2] n=5 | 2.36 [2.36-2.47] n=5 | 4.32 [4.30-4.49] n=5 | 4.68 | 0.55 |
| RM_CONTROL 1576 B p99 | us (low is better) | 48.6 [27.2-257] n=5 | 3.73 [3.19-6.75] n=5 | 5.51 [5.42-7.04] n=5 | 8.81 | 0.68 |
| RM_CONTROL 12328 B p50 | us (low is better) | 25.9 [24.1-28.9] n=5 | 2.42 [2.35-2.49] n=5 | 4.31 [4.30-4.48] n=5 | 6.01 | 0.56 |
| RM_CONTROL 12328 B p99 | us (low is better) | 60.4 [38.2-79.4] n=5 | 3.54 [3.22-7.10] n=5 | 5.52 [5.37-5.66] n=5 | 10.94 | 0.64 |
| RM_CONTROL 49192 B p50 | us (low is better) | 38.8 [36.9-46.8] n=5 | 2.50 [2.36-2.54] n=5 | 4.30 [4.30-4.35] n=5 | 9.02 | 0.58 |
| RM_CONTROL 49192 B p99 | us (low is better) | 107 [71.8-162] n=5 | 3.66 [3.26-6.55] n=5 | 5.40 [5.30-6.58] n=5 | 19.78 | 0.68 |

#### (7) VRAM churn, 20 cycles (bench-cu churn)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| 64x1 MiB cuMemAlloc | us (low is better) | 86.6 [69.6-103] n=5 | 20569 [18550-20940] n=5 | 37.7 [32.9-62.2] n=5 | 2.30 | 545.60 |
| 64x1 MiB cuMemFree | us (low is better) | 52.8 [46.3-71.3] n=5 | 20902 [20518-21783] n=5 | 29.2 [26.7-45.8] n=5 | 1.81 | 715.84 |
| 64x1 MiB cycle | ms (low is better) | 7.90 [7.28-10.2] n=5 | 2928 [2901-2940] n=5 | 3.89 [3.79-3.95] n=5 | 2.03 | 752.76 |
| 8x64 MiB cuMemAlloc | us (low is better) | 182 [177-192] n=5 | 53381 [51364-53846] n=5 | 93.8 [87.8-98.1] n=5 | 1.94 | 569.10 |
| 8x64 MiB cuMemFree | us (low is better) | 127 [121-131] n=5 | 48999 [48309-49960] n=5 | 96.4 [93.6-96.5] n=5 | 1.32 | 508.29 |
| 8x64 MiB cycle | ms (low is better) | 9.23 [9.12-9.76] n=5 | 860 [858-866] n=5 | 12.7 [11.9-12.9] n=5 | 0.73 | 67.64 |

#### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| churn device CPU | ms (low is better) | 370 [370-390] n=5 | 77550 [76970-77720] n=5 | n/a | - | - |
| churn VMM CPU | ms (low is better) | 480 [400-650] n=5 | 9800 [8390-10080] n=5 | n/a | - | - |
| copy device CPU | ms (low is better) | 370 [280-410] n=5 | 23870 [22670-24560] n=5 | n/a | - | - |
| copy VMM CPU | ms (low is better) | 36030 [13020-69550] n=5 | 22870 [13750-43930] n=5 | n/a | - | - |
| ctrl device CPU | ms (low is better) | 130 [120-130] n=5 | 10.0 [0.000-10.0] n=5 | n/a | - | - |
| ctrl VMM CPU | ms (low is better) | 540 [360-620] n=5 | 200 [180-210] n=5 | n/a | - | - |
| init device CPU | ms (low is better) | 220 [200-240] n=5 | 5990 [5880-6120] n=5 | n/a | - | - |
| init VMM CPU | ms (low is better) | 780 [750-790] n=5 | 3110 [3040-3270] n=5 | n/a | - | - |
| ioctl device CPU | ms (low is better) | 100 [90.0-110] n=5 | 10.0 [0.000-10.0] n=5 | n/a | - | - |
| ioctl VMM CPU | ms (low is better) | 300 [250-500] n=5 | 180 [170-190] n=5 | n/a | - | - |
| launch-auto device CPU | ms (low is better) | 200 [200-210] n=5 | 5760 [5510-6010] n=5 | n/a | - | - |
| launch-auto VMM CPU | ms (low is better) | 2520 [2420-4280] n=5 | 8250 [4670-13540] n=5 | n/a | - | - |
| launch-blocking device CPU | ms (low is better) | 230 [230-240] n=5 | 5930 [5910-5990] n=5 | n/a | - | - |
| launch-blocking VMM CPU | ms (low is better) | 670 [590-9770] n=5 | 12000 [3480-13750] n=5 | n/a | - | - |
| mm device CPU | ms (low is better) | 380 [370-480] n=5 | 7280 [7010-7670] n=5 | n/a | - | - |
| mm VMM CPU | ms (low is better) | 54070 [35380-68860] n=5 | 9990 [9500-23990] n=5 | n/a | - | - |
| rl device CPU | ms (low is better) | 370 [290-450] n=5 | 15300 [14780-15480] n=5 | n/a | - | - |
| rl VMM CPU | ms (low is better) | 86390 [52250-106220] n=5 | 23500 [21790-24040] n=5 | n/a | - | - |
| short-auto device CPU | ms (low is better) | 230 [200-230] n=5 | 6330 [6050-6470] n=5 | n/a | - | - |
| short-auto VMM CPU | ms (low is better) | 590 [560-740] n=5 | 10130 [2950-10780] n=5 | n/a | - | - |
| short-blocking device CPU | ms (low is better) | 220 [210-260] n=5 | 6280 [5840-6560] n=5 | n/a | - | - |
| short-blocking VMM CPU | ms (low is better) | 570 [290-1480] n=5 | 12420 [3020-12880] n=5 | n/a | - | - |
| time0 device CPU | ms (low is better) | 120 [90.0-130] n=5 | 90.0 [90.0-110] n=5 | n/a | - | - |
| time0 VMM CPU | ms (low is better) | 140 [130-150] n=5 | 250 [230-270] n=5 | n/a | - | - |
| time1 device CPU | ms (low is better) | 220 [210-230] n=5 | 6080 [5750-6160] n=5 | n/a | - | - |
| time1 VMM CPU | ms (low is better) | 230 [200-240] n=5 | 3120 [2800-3450] n=5 | n/a | - | - |

#### (8) Device process memory (over all runs)

| value | unit | v1 | v2 | native | v1 / native | v2 / native |
|---|---|---|---|---|---|---|
| device HWM | MiB (low is better) | 425 [59.6-538] n=65 | 4123 [4108-4123] n=65 | n/a | - | - |
| device RSS after run | MiB (low is better) | 384 [14.3-493] n=65 | 4121 [4108-4121] n=65 | n/a | - | - |


#### 4.1.1 v2 with `CARAXES_SHADOW_ALWAYS_RESYNC=1` (worst case of the shadow walk)

`final-always-resync-all/` (copies of `final-always-resync/` rep 1 and `final-always-resync-b/`
reps 1-3; README there), 05:51-06:26, same build, the device started with the variable set,
the v1 VM off, v2 restarted before every rep, native interleaved. n=4 for metrics 1-3. rlprobe
ran once and did not finish: the v2 run hit the harness's 900 s timeout (native: 1.3 s). The
comparison with §4.1 isolates the walk. Under always-resync every doorbell of a shadowed
(GR/compute) channel walks the guest page tables, which adds about 44 ms per launch: 44.1
against 0.257 ms for the short kernel, 44.0 against 17.6 ms for the long one. D2D copies drop
to 0.022 GB/s at 1 MiB (47 ms each) and 2.4 GB/s at 256 MiB; libcuda evidently runs D2D on the
compute channel, since D2D slows with the walk and H2D/D2H do not. H2D/D2H do not change (2.0-3.2 GB/s small, 12.8-13.1 GB/s at 256 MiB): they
run on copy-engine channels, which execute with the guest's own page tables and have no
shadow to resync.

Sources: v2=final-always-resync-all/v2, native=final-always-resync-all/native
Runs during which another VM's device process used more than 20% of one CPU (that VM was active): v2 0/25, native 0/25
Host 1-min load average at run start, median [min-max]: v2 2.79 [1.55-3.95] n=25, native 2.77 [1.55-4.04] n=25

##### (1) Kernel launch, 30M-cycle spin kernel, sched=auto (nvprobe 4)

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 44.0 [43.4-46.8] n=4 | 16.4 [16.3-16.6] n=4 | 2.68 |

##### (1) Kernel launch, 30M-cycle spin kernel, sched=blocking

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 88.4 [82.0-102] n=4 | 16.3 [16.3-16.4] n=4 | 5.42 |

##### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=auto

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 44.1 [43.1-44.5] n=4 | 0.006 [0.006-0.006] n=4 | 7342.33 |

##### (1) Kernel launch, 1000-cycle kernel (launch overhead), sched=blocking

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| ms/launch | ms (low is better) | 122 [110-128] n=4 | 0.019 [0.015-0.024] n=4 | 6437.74 |

##### (2) Copy bandwidth, synchronous cuMemcpy (bench-cu copy)

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| h2d pinned 1 MiB | GB/s (high is better) | 2.20 [2.13-2.25] n=4 | 12.1 [12.1-12.1] n=4 | 0.18 |
| d2h pinned 1 MiB | GB/s (high is better) | 2.04 [1.86-2.26] n=4 | 12.3 [12.2-12.3] n=4 | 0.17 |
| d2d device 1 MiB | GB/s (high is better) | 0.022 [0.022-0.023] n=4 | 134 [130-141] n=4 | 0.00 |
| h2d pageable 1 MiB | GB/s (high is better) | 1.99 [1.89-2.01] n=4 | 7.38 [7.25-7.42] n=4 | 0.27 |
| d2h pageable 1 MiB | GB/s (high is better) | 1.94 [1.91-1.97] n=4 | 6.44 [6.29-6.47] n=4 | 0.30 |
| h2d pinned 16 MiB | GB/s (high is better) | 3.16 [3.13-3.18] n=4 | 13.1 [13.0-13.1] n=4 | 0.24 |
| d2h pinned 16 MiB | GB/s (high is better) | 3.23 [3.19-3.24] n=4 | 13.1 [13.1-13.1] n=4 | 0.25 |
| d2d device 16 MiB | GB/s (high is better) | 0.426 [0.303-0.521] n=4 | 182 [182-184] n=4 | 0.00 |
| h2d pageable 16 MiB | GB/s (high is better) | 4.05 [2.80-4.29] n=4 | 10.9 [9.58-11.1] n=4 | 0.37 |
| d2h pageable 16 MiB | GB/s (high is better) | 3.80 [3.69-4.08] n=4 | 8.99 [8.73-9.04] n=4 | 0.42 |
| h2d pinned 256 MiB | GB/s (high is better) | 12.8 [12.5-13.0] n=4 | 13.1 [13.1-13.1] n=4 | 0.98 |
| d2h pinned 256 MiB | GB/s (high is better) | 13.1 [13.1-13.1] n=4 | 13.2 [13.2-13.2] n=4 | 0.99 |
| d2d device 256 MiB | GB/s (high is better) | 2.41 [2.31-2.47] n=4 | 189 [186-190] n=4 | 0.01 |
| h2d pageable 256 MiB | GB/s (high is better) | 3.67 [3.62-3.96] n=4 | 9.11 [8.73-9.48] n=4 | 0.40 |
| d2h pageable 256 MiB | GB/s (high is better) | 3.49 [3.38-3.70] n=4 | 8.56 [8.39-9.05] n=4 | 0.41 |

##### (3) Matmul throughput, fp32 torch.mm (mmsweep)

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| N=1024 | GFLOP/s (high is better) | 23.1 [23.1-23.2] n=4 | 5276 [4565-5322] n=4 | 0.00 |
| N=2048 | GFLOP/s (high is better) | 156 [137-159] n=4 | 5545 [5249-5630] n=4 | 0.03 |
| N=4096 | GFLOP/s (high is better) | 2571 [2342-2969] n=4 | 6126 [5908-6413] n=4 | 0.42 |

##### (4) rlprobe, 50 REINFORCE episodes

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| mean10 (reward, not a time) |  | n/a | 56.9 [56.9-56.9] n=1 | - |
| process wall (incl. torch import) | s (low is better) | n/a | 4.50 [4.50-4.50] n=1 | - |
| stage4 (50 training episodes) | s (low is better) | n/a | 1.30 [1.30-1.30] n=1 | - |
| H2D+D2H transfers per second | 1/s (high is better) | n/a | 2288 [2288-2288] n=1 | - |

##### (8) Device process (v1 vhost-user-nvrm, v2 vfio-user-nvgpu) and VMM CPU per run

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| copy device CPU | ms (low is better) | 34000 [33870-35330] n=4 | n/a | - |
| copy VMM CPU | ms (low is better) | 47850 [46210-50120] n=4 | n/a | - |
| launch-auto device CPU | ms (low is better) | 13720 [13430-13750] n=4 | n/a | - |
| launch-auto VMM CPU | ms (low is better) | 13110 [10610-14100] n=4 | n/a | - |
| launch-blocking device CPU | ms (low is better) | 19935 [19770-22050] n=4 | n/a | - |
| launch-blocking VMM CPU | ms (low is better) | 18845 [16640-20330] n=4 | n/a | - |
| mm device CPU | ms (low is better) | 28705 [28610-28780] n=4 | n/a | - |
| mm VMM CPU | ms (low is better) | 32425 [16620-37710] n=4 | n/a | - |
| rl device CPU | ms (low is better) | 875020 [875020-875020] n=1 | n/a | - |
| rl VMM CPU | ms (low is better) | 513000 [513000-513000] n=1 | n/a | - |
| short-auto device CPU | ms (low is better) | 16100 [16080-16120] n=4 | n/a | - |
| short-auto VMM CPU | ms (low is better) | 18640 [15890-19680] n=4 | n/a | - |
| short-blocking device CPU | ms (low is better) | 24330 [22790-24960] n=4 | n/a | - |
| short-blocking VMM CPU | ms (low is better) | 20705 [17060-26730] n=4 | n/a | - |

##### (8) Device process memory (over all runs)

| value | unit | v2 | native | v2 / native |
|---|---|---|---|---|
| device HWM | MiB (low is better) | 4109 [4108-4129] n=25 | n/a | - |
| device RSS after run | MiB (low is better) | 4109 [4108-4121] n=25 | n/a | - |


### 4.2 Earlier series, under load

Under load (§3) and on earlier v2 builds. Not used for the conclusions; they show how the
numbers move with host load:
* `summary-series1b.md`: native and v1, n=5, under load. The v1 ratios match the final series
  within the spread (ioctl 6.2x, launch 0.98x, blocking 6.2x, pinned 1.00x).
* `summary-prelim-3col.md`: v1 (series1b), v2 on `223075d` (series2 rep 1, with the 60 s
  flush timeouts in every context) and on the in-progress `c09fcccc` (rep 2), each against the
  native run of its own series.
* `summary-series3-native-quiet.md`: native only, quiet host, 03:11. Under load native lost
  up to 25 % in matmul at N=1024 (5300 against 7060 GFLOP/s quiet); the final series' native
  (5041) sits between the two, since it shares the host with two busy VMs.
* `summary-series1-v1-pin-leak.md`: series1, where v1 fails from rep 2 on (§6).

## 5. Interpretation

Final series (quiet host, n=5, v2 at `8d44ee6`). The attributions below are inferred from which
metrics are slow, how much CPU the device process burns, at which size a slowdown fades, and
from the always-resync rows (§4.1.1). "Probably" marks a mechanism I did not measure directly.

### v1: the cost is the forwarded call, and GPU-bound work runs at native speed

* **Work on the GPU runs at native speed.** Long-kernel launch 0.99x, the launch overhead
  of a short kernel with spin sync 0.007 against 0.006 ms, pinned H2D/D2H 1.00-1.01x at every
  size, D2D 0.97-1.07x, matmul 0.92-1.04x. The guest's pushbuffer and doorbell reach the GPU
  through window mappings of the real doorbell and memory; the RM is not involved, so
  forwarding costs nothing here.
* **Every RM call pays one forwarding round trip.** `CHECK_VERSION_STR` p50 16 us against
  2.7 us native (6x), `RM_CONTROL` 20-39 us against 4.3 us (4.7-9x, growing with the
  parameter size), p99 up to 20x. That is virtqueue, backend and host ioctl, about 13-35 us
  per call. It shows wherever a workload makes RM calls. cuMemAlloc/cuMemFree cost 1.3-2.3x
  (a few forwarded calls each). rlprobe reaches 0.81x transfers/s and 1.25x stage-4 time.
  Pageable copies are at 0.81-0.97x, probably because libcuda's staging registrations cross.
* **Context setup is not slower.** cuInit 31 ms (0.40x), cuCtxCreate 159 ms (0.84x),
  process walls 0.83x. The guest process spends almost no CPU (`time`: 5 ms user+sys for
  cuInit against 127 ms native). The backend does the RM work (100-240 ms CPU per process),
  so the guest's time understates the cost; the device CPU rows in (8) show it.
* **Blocking synchronisation takes 100 ms flat** (`sched=blocking`: 100.0 ms per launch in
  every rep against 16.4 ms, 6.1x; on the short kernel 0.04-100 ms, bimodal). Probably the
  waiter misses the forwarded completion event and wakes on a 100 ms fallback. Spin
  synchronisation (`sched=auto`, the CUDA default) is unaffected. Worth fixing, since
  blocking sync is what frameworks use when they yield the CPU.
* **The pinned-page retention bug (§6)**, which forces a VM restart about every 1 GiB of
  pinned allocations.
* Backend footprint: 0.1-0.4 s CPU per benchmark run, 380-540 MiB RSS. Module load to the
  first cuInit takes 0.19 s.

### v2: RM calls cost nothing, and the device process is the bottleneck

* **RM calls are free.** The guest's own RM answers them: `CHECK_VERSION_STR` 1.37 us and
  `RM_CONTROL` 2.4 us, 0.5-0.6x of the host's time. Nothing crosses a boundary; why the guest is even faster
  than the host (guest kernel 6.8, host 7.2) was not investigated. The device process
  uses no CPU during the ping runs.
* **Per submission: the doorbell trap and the mirror, about 0.25 ms.** A 1000-cycle kernel
  launch plus sync takes 0.257 ms against 0.006 ms native (43x). A 1 MiB pinned copy takes
  about 310 us against 86 us. Each submission rings the doorbell, an MMIO write trapped over
  the vfio-user socket into `vfio-user-nvgpu`, which forwards it to the host mirror channel;
  completion then comes back through the guest-visible semaphore. The overhead fades with
  transfer size: pinned 1 MiB 0.28x, 16 MiB 0.24x, 256 MiB 0.99x; D2D 1 MiB 0.05x, 256 MiB
  0.95x. So once the work is submitted, the physical copy engine moves registered guest RAM
  and VRAM at full speed. The per-copy overhead of H2D/D2H (copy-engine channels, about
  0.22 ms) is similar to the GR launch overhead (0.25 ms), so the trap-and-forward path, not
  the engine, sets it. Long kernels pay 1.08x (17.6 against 16.2 ms), 1.4 ms per launch,
  more than the 0.25 ms trap; not attributed. Pageable copies stay at 0.31-0.45x at every
  size, probably because libcuda splits them into staged chunks, each a submission.
* **The shadow resync is normally skipped; without that it would dominate.** Forcing a walk on
  every doorbell (`CARAXES_SHADOW_ALWAYS_RESYNC=1`, §4.1.1) adds about 44 ms per GR/compute
  submission: short-kernel launch 44 ms instead of 0.26 ms, matmul N=1024 at 23 GFLOP/s,
  rlprobe more than 900 s instead of 18.6 s, D2D 1 MiB 47 ms per copy. Copy-engine
  transfers (H2D/D2H) are unaffected: only the GR channel runs on the host shadow VA space.
  So the build's rule (walk only when a guest write reached a page-table page) is what keeps
  the launch path at 0.26 ms. Every real mapping change still pays a walk (next point).
* **Mapping changes: 20-55 ms each, CPU-bound in the device.** cuMemAlloc/cuMemFree take
  20.6/20.9 ms for 1 MiB and 53/49 ms for 64 MiB (550-720x native). During the churn run the
  device burns 77.5 s of CPU in 84 s of wall time: one core, busy for the whole run. Each
  mapping change writes guest page tables, which triggers the walk and the mirroring into
  the host shadow VA space (2 MiB region objects, map/unmap through the host RM); the cost
  grows with the size mapped. cuCtxCreate (4.7 s, 25x) and cuCtxDestroy (3.4 s, 38x) are
  probably the same work: a context sets up and tears down many mappings (its device
  process CPU, about 6 s, matches). `nvprobe 1` takes 8.3 s against 0.4 s.
* **torch sits between the two costs.** Matmul reaches 0.27x at N=1024, 0.14x at N=2048 and
  0.50x at N=4096. rlprobe does 154 transfers/s against 2417 (0.06x); stage 4 takes 18.6 s
  against 1.2 s. One N=1024 matmul costs 1.6 ms against 0.4 ms, one N=2048 matmul 22 ms against
  3 ms. That is far more than 0.25 ms per submission, so torch's steady state probably still
  changes mappings (caching-allocator growth, cuBLAS workspaces, the per-step tensors of
  rlprobe), and each change costs a walk (about 44 ms, §4.1.1) plus a mirror update. The
  N=2048 case, slower per FLOP than both neighbours, fits a walk in some iterations.
* **Blocking synchronisation** mostly works (18.7 ms, 1.14x). It has one 101 ms outlier in
  five reps, which looks like the same missed wake-up v1 hits every time.
* **Startup.** `modprobe nvidia` plus nodes takes 3.2 s (RmInitAdapter against the device
  model and its boot model), 3.4 s to the first cuInit, against 0.19 s for v1. It is paid
  once per VM boot or driver reload.
* **Device footprint.** 6 s CPU per CUDA process that only creates a context; 24 s for the
  copy run, 77 s for churn; RSS 4.1 GiB, which is the 4 GiB guest RAM the device maps and
  registers with the host RM, not private memory.

### So

| | v1 costs | v2 costs |
|---|---|---|
| per RM call | 13-35 us (6-9x) | nothing (0.5x) |
| per submission / launch | nothing | about 0.25 ms (trap + mirror) |
| per mapping change | a few forwarded calls (about 2x) | 20-55 ms of device CPU (walk + shadow mirror) |
| bulk transfer, long kernels | native | native at 256 MiB, 1.08x on long kernels |
| blocking sync | 100 ms per wait | mostly fine |
| context create/destroy | native | 4.7 s / 3.4 s |
| startup | 0.19 s | 3.4 s |
| correctness issues seen | pinned registrations leak until guest restart (§6) | one reserving VM per GPU on `223075d` (slots since `7faac21`, untested here); NV_UFLUSH wait, fixed in `7faac21` (§7) |

On today's builds v1 is the faster path for every workload measured, with two exceptions:
raw RM-call latency and blocking synchronisation. v2's gap is not architectural in the way
v1's per-call cost is: it is almost all in the device process's handling of mapping changes, and resync avoidance already
recovered most of the launch cost. What v2 needs is mapping changes applied incrementally
(from the written PTE, not a walk) and cheaper shadow updates. After that, only the
per-submission trap remains, about 0.25 ms, which a doorbell path that bypasses the socket
(an eventfd/ioeventfd on the doorbell page, not tried) might reduce further. v1's costs are
per-call and inherent to forwarding; its two concrete bugs (blocking-sync wake-up and pinned
retention) are fixable. P13 asks which path is the product. The decision needs the v2
mapping path fixed first, and a second run of this benchmark, which the harness makes a
one-command job (`V1_RESTART=1 V2_RESTART=1 caraxes/tools/bench-run.sh series --targets
"native v1 v2" --reps 5 --out DIR`).

## 6. v1 bug found: freed pinned registrations keep their guest pages until restart

Symptom (series 1, `caraxes/runs/bench-2026-09-24/series1/`): from the second rep on, v1 runs
fail one after another. `bench-cu copy` gets `ERROR cuMemHostAlloc: 304 OS call failed or
operation not supported on this OS` (`v1/copy-2.txt`), later `cuCtxCreate: 304`
(`v1/init-4.txt`), and torch `CUDA-capable device(s) is/are busy or unavailable`
(`v1/mm-4.txt`). The guest log (`v1-guest-dmesg.txt`) shows, at the end of the first copy run
(256 MiB pinned), `virtio_nvrm: retained 65536 uncertain backing pages until guest restart
(65536 total)`, then `retained 512 ... ` (2 MiB) for most later CUDA processes, up to
154112 pages (602 MiB). The backend (`v1-backend.log`) refuses: `0x71 arena: guest-page
registration budget exceeded: 1073741824/1073741824 bytes, 1071644672 retained after source
free, requested 2097152 (LEA_MAX_PIN_TOTAL_MIB)` and `ioctl dev=1 nr=0x27: 0x71 arena failed
(errno 5)`.

What happens: pinned host memory in the guest (`cuMemHostAlloc`, and a 2 MiB registration
that most CUDA processes make on their own) becomes an OS-descriptor registration (escape 0x27, class
0x71) whose guest pages are charged to one budget, `LEA_MAX_PIN_TOTAL_MIB` (1 GiB; the guest
module's `max_pin` is also 1024 MiB). When the process frees or exits, `virtio_nvrm`
releases a pin normally only when it can prove the host freed the object; otherwise
(`nvrm_ctx_release_pins`: close error, `ctx->uncertain`, or device stopping) it
quarantines the pages "until guest restart" (`guest-module/virtio_nvrm/virtio_nvrm.c`,
`nvrm_quarantine`; counter `/sys/module/virtio_nvrm/parameters/stat_quarantined_pages`), and
the backend keeps the charge as "retained after source free"
(`crates/vhost-user-nvrm/src/host_pool.rs`). In this benchmark every pinned registration
ended in that state, so the budget is used up after about 1 GiB of pinned memory allocated
over the life of the guest, however little is live at any moment. One rep (13 CUDA
processes, 273 MiB pinned by `bench-cu copy`, torch's pinned buffers) fits; the second does
not.

Workaround used: series 1b (`V1_RESTART=1 bench-run.sh series`) powers the v1 VM off and
starts VM and backend again before every rep after the first (`bench-v1vm.sh down; up;
modules`). The final series does the same (§2), so all v1 numbers in §4 have it. The restart
leaves the guest page cache cold, which shows in the process wall time of the first Python
runs of a rep (torch import: `rl` process wall 53-107 s against 4.4 s native in the final
series); the in-process numbers (stage-4 time,
GFLOP/s) are not affected. Not tried: raising `LEA_MAX_PIN_TOTAL_MIB` and `max_pin` (only
postpones it; the quarantined guest pages stay allocated in the 4 GiB guest).

## 7. v2 limitations found

**One reserving device per host.** `CARAXES_RESERVE_VRAM=1` reserves the FB at a fixed host address, the top of the host heap
(`[0xe9000000, 0x1e9000000)` for 4096 MiB, `[0x1a9000000, 0x1e9000000)` for 1024 MiB). With
vm2's device holding its reservation, a second device on vm1 was refused even at
`CARAXES_FB_MIB=1024` although 2455 MiB were free: `the host RM did not give
[0x1a9000000, 0x1e9000000) (RM status 0x51 (NV_ERR_NO_MEMORY) on ioctl 0x2b) ... no
floating fallback (the guest's FB layout is fixed at start)` (`caraxes/runs/bench-2026-09-24/v2-up1/run.txt`).
So on `223075d` v2 ran one host-executing VM per GPU, which is why the v2 series waited
for vm2. `7faac21` adds reservation slots (`CARAXES_RESERVE_SLOT=<n>` places the FB n profile
sizes below the top, one slot per VM); this benchmark did not test two v2 VMs at once.

**Every context creation waited 2 x 30 s on the L2 flush registers (fixed).** Build `223075d`,
evidence in `caraxes/runs/bench-2026-09-24/v2-hang/`: each `cuCtxCreate` took about 68 s. During
it the guest's `NV_ESC_RM_MAP_MEMORY_DMA` (escape 0x57) sits in the kernel and the guest RM
logs `kmemsysDoCacheOp_GM107: - timeout error waiting for reg 0x70010 update` and then the
same for `reg 0x70004`, each after `_threadNodeCheckTimeout: Timeout was set to: 30000 msecs!`
(`nvprobe1-guest-dmesg.txt`, `nvprobe1-strace.txt`). 0x70010 is `NV_UFLUSH_L2_FLUSH_DIRTY`,
0x70004 `NV_UFLUSH_L2_SYSMEM_INVALIDATE`. The device model reads back the value written there,
so the pending bit the RM polls never clears. Fixed in `7faac21` (the registers are modelled
and the flushes forwarded to the host; the fix agent's hardware check is the `d-uflush` row of
[experiments.md](experiments.md)). In the final series (`8d44ee6`) cuCtxCreate takes 4.7 s and
no flush timeout occurs.

Trigger (on `223075d`): every context, from the first process on a fresh device. Seven `nvprobe 1` in a row
on a freshly started device each took 69-70 s (`sequence2-fresh.txt`). A driver reload
(function-level reset, `rmmod`/`modprobe`) does not change this (`sequence1.txt`). The same
timeouts appear in the CUDA gate's `cuda-core` and `robustness` logs
(`caraxes/runs/e5b2-2026-09-24/gate-cuda-2026-09-24/guest/*.dmesg`); the gate passes because
it does not time the context. `cuInit` alone is not affected. Consequence for §4.2: on this build the v2 process-level times (`time1`, `init` cuCtxCreate,
process wall of `rl`) carry 60 s of timeouts. The in-process measurements (launch, copies,
GFLOP/s, pings, churn), taken after the context exists, do not. Rep 2 of series 2 ran on a
build with the NV_UFLUSH work in progress (`c09fcccc`), and there cuCtxCreate took 5.4 s.

**Device nodes.** The guest image has no `nvidia-modprobe`, and `modprobe nvidia` creates no
`/dev/nvidia*`. Without `nvidia-uvm`, `cuInit` returns 999. The benchmark loads `nvidia-uvm`
and creates `nvidiactl`, `nvidia0`, `nvidia-uvm` and `nvidia-uvm-tools` itself
(`bench-run.sh v2-nodes`, the same nodes as `tools/caraxes-vm.sh uvm`). This is part of the
v2 startup time in §4.

## Doorbell over KVM ioeventfd (2026-09-24, vm1, host under load)

Patch 0005 (Cloud Hypervisor asks for the region I/O fds and registers the usermode doorbell
as a KVM ioeventfd, [../../patches/README.md](../../patches/README.md)) and the device's
token-less doorbell path (`Dispatcher::doorbell_pending`: the eventfd carries no work-submit
token, so a wakeup rings every channel whose USERD GPPut moved since its last doorbell). vm1
with a 1 GiB profile at reservation slot 4 while vm0 (display) and vm2 (the mapping series)
were active, so the absolute numbers are under load; A and B ran minutes apart under the same
load. `caraxes/runs/doorbell-2026-09-24-vm1/` (A, and B with the first scan) and
`caraxes/runs/doorbell-2026-09-24-vm1-watch/` (B with the watched GPPut copy).

| Value | A: trapped doorbell | B: ioeventfd, scan by BAR1 reads | B: ioeventfd, watched copy |
|---|---|---|---|
| doorbell write p50 (bench-trap, 20000 writes) | 29.3 us | 3.5 us | 3.9 us |
| trapped doorbell writes in the run | 196488 | 0 | 0 |
| eventfd wakeups | - | 196193 | 196244 |
| channels scanned per wakeup, pending per wakeup | - | 19.0, 0.90 | 19.0, 0.90 |
| scan per wakeup, mean | - | 31.4 us | 1.07 us |
| short kernel, ms/launch (2 reps) | 0.176, 0.159 | 0.189, 0.239 | 0.144, 0.153 |
| rlprobe stage 4 (2 reps) | 20.0 s, 20.8 s | 22.8 s, 22.8 s | 18.8 s, 22.2 s |
| host Xid | 0 | 0 | 0 |

Reading: the trap is gone and costs the device 3.9 us per doorbell; the first scan read every
channel's GPPut through the host BAR1 windows (1.6 us per uncached read, 2.5 ms on a window
miss) and gave the gain back; with the FB store's watched copy (`FakeFb::watched_dword`, kept
current by the trapped GPPut writes) the scan is a map lookup. The short kernel gains about the
one trap of four (25 us of 160); the other three (GPFIFO entry, GPPut through BAR1) and the
small copies are P21's. Once P21 passes the BAR1 windows through, the watched copy no longer
sees GPPut, and the wakeup needs the token: the datamatch token pool ([roadmap.md](roadmap.md)).

## P21 BAR1 passthrough, prototype G (2026-09-24, vm1, host under load)

The 1 MiB passthrough area of [bar1-passthrough.md](bar1-passthrough.md) (guest BAR1
`[0, 0x100000)` as one vfio-user sparse mmap area of region 1, backed by an NVIDIA dma-buf of
the reservation piece the traces showed, `CARAXES_BAR1_PASSTHROUGH=1`), merged as 572525f, on
vm1 with a 1 GiB profile at slot 4 while vm0, vm2 and vm3 were active. Both runs with the
mapping package; A with the doorbell over the ioeventfd, B with the trapped doorbell (the two
switches cannot be combined yet, see below). `caraxes/runs/p21-bar1-2026-09-24/`.

| Value | A: no area, ioeventfd doorbell | B: area, trapped doorbell |
|---|---|---|
| BAR1 read at 0x80000 (inside the area), p50 / p99 | 47.9 / 133.9 us | 1.4 / 3.2 us |
| BAR1 read at 0x400000 (outside), p50 | 44.3 us | 43.0 us |
| doorbell write, p50 | 4.1 us | 34.5 us |
| trapped BAR1 accesses in the first MiB | 549464 | 0 |
| short kernel, ms/launch (2 reps) | 0.153, 0.161 | 0.077, 0.122 |
| rlprobe stage 4 (2 reps) | 19.6 s, 18.3 s | 9.0 s, 9.6 s |
| rlprobe transfers per second | 144 | 318, 299 |
| prototype checks | - | 91 checks, 0 mismatched pages, 0 page-table hazards |
| host BAR1 use while up / after | - | 185 MiB / 152 MiB (the area released) |
| host Xid | 0 | 0 |

Reading: the dma-buf export, the Cloud Hypervisor mapping and the guest's write-combined
access all work on this host; three of the four trapped accesses of a submission are gone
and rlprobe's stage 4 halves (native 1.3 s, v1 3.4 s, so the gap is now the doorbell trap
and the device's own work per submission). The combination with the ioeventfd doorbell hangs
the guest (`B-on-with-ioeventfd-hung/`: the token-less path reads GPPut from the FB store's
watched copy, which the passthrough area bypasses; 20040 wakeups rang nothing), so the device
refuses both switches together until the datamatch token pool exists
([doorbell-token-pool.md](doorbell-token-pool.md) when it lands): with it, a submission takes
no exit at all.

## Zero-trap submission: P21 area plus the doorbell token pool (2026-09-24, 13:09-13:17, vm1)

The datamatch token pool of [doorbell-token-pool.md](doorbell-token-pool.md) (main 0d00b26,
frozen device `/mnt/vmstore/caraxes/vm1/vfio-user-nvgpu-tokens`, Cloud Hypervisor v53.0 with
patches 0001-0006, `CARAXES_BAR1_PASSTHROUGH=1`, `CARAXES_DOORBELL_TOKENS=64`) on vm1 with the
1 GiB profile at slot 4; vm2 was up with its device idle and the bench-v1 VM idle, vm0 down, so
the host was quieter than at noon but not quiet. A and C ran minutes apart.
`caraxes/runs/doorbell-tokens-gpu-2026-09-24/`.

| Value | A: P21 area, trapped doorbell | C: P21 area, token pool |
|---|---|---|
| short kernel, ms/launch (3 reps, first cold) | 0.280, 0.050, 0.058 | 0.024, 0.018, 0.053 |
| rlprobe stage 4 (3 reps) | 11.2 s, 11.8 s, 8.6 s | 3.0 s, 3.0 s, 2.9 s |
| rlprobe transfers per second | 244, 334, 257 | 952, 973, 994 |
| trapped BAR1 accesses in the first MiB | 0 | 0 |
| trapped doorbells over the run | 304732 (all) | 120 (one per channel, its first ring) |
| eventfd wakeups / guest doorbell writes | - | 264444 / 284612 |
| pool tokens handed out / peak assigned | - | 120 / 20 |
| pool tokens that trapped, own-token rings after hand-out | - | 0, 0 |
| P21 checks | 0 mismatched, 0 hazards | 146 checks, 0 mismatched, 0 hazards |
| host Xid | 0 | 0 |

Reading: with the area and the pool a CUDA submission takes no exit at all after a channel's
first ring; the short kernel is at 18 to 24 us per launch (native 6 to 9 us on the same
harness, v1 8 to 11), and rlprobe's stage 4 is at 3.0 s against native 1.3 s and v1 3.4 s, with
transfers at about 970 per second against 2266 native. The 19778 "stale" eventfd wakeups are the
bench-trap writes of the unassigned pool token, as designed. What remains between C and native
is the device's own work per submission (eventfd wakeup, the host doorbell write, mirror
bookkeeping) and the small copies outside the 1 MiB area (design H).

## Doorbell reactor: blocking, spinning and hybrid reader (2026-09-24, 13:44-14:12, vm1)

The reactor of [doorbell-poll.md](doorbell-poll.md) (device b6b52a4, frozen at
`/mnt/vmstore/caraxes/vm1/vfio-user-nvgpu-poll`), in the setup of the previous section:
P21 area, 64 pool tokens, Cloud Hypervisor 0001-0006. The table shows the second of two passes,
run in reverse order with no other device active; vm0 was up but idle.
`caraxes/runs/doorbell-poll-2026-09-24/pass2/`.

| Value | C: blocking | D0: spin, unpinned | D: spin, pinned (CPU 14) | E: hybrid 200 us, pinned (CPU 14) |
|---|---|---|---|---|
| short kernel, ms/launch (3 reps) | 0.024, 0.027, 0.026 | 0.020, 0.088, 0.014 | 0.021, 0.014, 0.015 | 0.014, 0.014, 0.014 |
| rlprobe stage 4, median | 2.7 s | 2.3 s | 2.1 s | 2.3 s |
| rlprobe transfers per second, median | 1062 | 1270 | 1375 | 1257 |
| ring to host doorbell, p50 / p99 | 4.1 / 9.8 us | 3.6 / 7.1 us | 3.6 / 4.9 us | 3.6 / 6.9 us |
| reader CPU | 1.4 % | 97.6 % | 96.0 % | 3.1 % |

Reading:
- The scheduler wake-up of the blocked reader cost about 10 to 12 us per launch.
- Pinning cuts the tail: p99 21.8 us unpinned against 5.4 us pinned in the first pass, while
  another VM's device was busy.
- Hybrid gives most of the gain at 3 % of a core.

## Closing series on driver 610.57.04 (2026-09-24, 14:15-15:03, quiet host)

The last table before the host moves to 615.71.09 (D19). native, v1 and v2 interleaved, N=5,
restart per rep on both paths, no other VM; v2 = main 81054f0 (frozen
`/mnt/vmstore/caraxes/vfio-user-nvgpu-closing-610-81054f0`) with the mapping package, the
P21 area, the doorbell token pool (64) and the spin reactor pinned to a core, on Cloud
Hypervisor v53.0 with patches 0001-0006; vm2 at slot 0 with the 2 GiB profile. The
"loaded" marks of the summary for native and v1 come from v2's spinning reactor thread (a
whole core by design), not from another VM. Full tables:
`caraxes/runs/closing-610-2026-09-24/summary.md`; 0 Xid over the series.

| Metric | native | v1 | v2 closing | v2 / native | v2 this morning |
|---|---|---|---|---|---|
| short kernel, ms/launch, auto | 0.006 | 0.009 | 0.015 | 2.5 | 0.257 (43x) |
| short kernel, blocking | 0.021 | 0.028 | 0.100 | 4.8 | 0.251 |
| launch 30M-cycle, auto | 16.3 | 16.0 | 17.3 | 1.06 | 17.6 |
| launch 30M-cycle, blocking | 16.1 | 100 | 18.0 | 1.12 | 18.7 |
| rlprobe stage 4, s | 1.20 | 1.50 | 2.10 | 1.75 | 20 (15.5x) |
| rlprobe transfers per second | 2350 | 1930 | 1340 | 0.57 | 150 |
| rlprobe process wall, s | 4.64 | 105 | 12.2 | 2.6 | 33 |
| cuMemAlloc 1 MiB, us | 35.8 | 93.4 | 830 | 23 | 18900 |
| cuMemFree 1 MiB, us | 27.1 | 52.5 | 903 | 33 | 20800 |
| cuMemAlloc 64 MiB, us | 78.3 | 184 | 6747 | 86 | 61000 |
| cuCtxCreate, ms | 195 | 165 | 3770 | 19 | 4727 |
| cuCtxDestroy, ms | 92.6 | 85.4 | 1352 | 15 | 3390 |
| cuInit, ms | 106 | 35.2 | 151 | 1.4 | 149 |
| module load to first cuInit, ms | - | 218 | 3509 | - | 3236 |
| copy h2d pinned 1 MiB, GB/s | 12.1 | 12.2 | 6.06 | 0.50 | 3.33 (0.28x) |
| copy h2d pageable 1 MiB | 7.25 | 7.42 | 4.50 | 0.62 | 2.84 |
| copy h2d pinned 16 MiB | 13.0 | 13.0 | 6.57 | 0.50 | 3.13 |
| copy d2d 16 MiB | 183 | 178 | 16.5 (16 to 167) | 0.09 | 79 |
| matmul N=1024, GFLOP/s | 4894 | 5058 | 1340 | 0.27 | 1203 |
| matmul N=2048 | 5494 | 5569 | 2248 | 0.41 | 781 (0.14x) |
| matmul N=4096 | 6052 | 6163 | 4433 | 0.73 | 5288 |

Reading, against the table of the morning (§ above, v2 at 8d44ee6): the submission path is
done (short kernel 43x to 2.5x of native, rlprobe stage 4 15.5x to 1.75x, now ahead of v1's
process wall by its pinned-page leak and behind v1's stage 4 by 0.6 s), allocations are 20x
faster but still 23x native (the RM round trip per allocation through the emulated GSP), the
context lifetime is the largest remaining gap (cuCtxCreate 19x: the 96k trapped BAR2
page-table writes, the memfd route for BAR2 is the answer), the copies below 16 MiB sit at half
of native (the 1 MiB area covers GPFIFO and USERD only, design H extends it), the d2d 16 MiB
median is an outlier band worth a look (two reps at native speed, three at 16 GB/s), and
matmul tracks the allocation and copy costs of each size.

### PPO CartPole right after the closing series (2026-09-24, 15:05-15:13)

The operator's PPO probe (`caraxes/tools/pporl.py`, metric `ppo`: 128 vectorized CartPole
environments on the CPU, the actor-critic and the PPO update on the GPU, one small pageable
host-to-device copy of the observations and one synchronous device-to-host copy of the
actions per environment step, 4 epochs over 32768 samples per rollout), 60 s per run, 2 reps,
same targets and configuration as the closing series. `caraxes/runs/closing-610-2026-09-24/ppo/`.

| Value | native | v1 | v2 closing | v1 / native | v2 / native |
|---|---|---|---|---|---|
| environment steps per second | 107663, 112259 | 56592, 95928 | 61082, 72977 | 0.69 | 0.61 |
| per environment step (128 envs), ms | 1.16 | 1.68 | 1.91 | | |
| reward100 after 60 s (learning, not a time) | 212, 400 | 255, 397 | 348, 417 | | |
| solved (mean100 at least 475) within 60 s | no | no | no | | |

Reading: PPO's step is about ten small launches plus two small copies, and the copies are
pageable, so they do not go through the 1 MiB area; v2 pays about 0.75 ms per environment
step over native, of which the two copies are the larger part (the closing series has pageable
1 MiB copies at 0.62 of native and small ones worse). v1 sits at 0.69 with a wide spread. The
full-length run (420 s, the solve time) is part of the workload gate.

## Design H, the BAR1 pool, measured (2026-09-24, 16:04-16:40, vm2)

`caraxes/runs/bar1-pool-2026-09-24/`: the 1 MiB area (mode 1) against the 64 MiB pool
(mode pool), token pool and spin reactor on, 5 reps of copy, rl and short-auto, 2 of PPO.

| Value | 1 MiB area | 64 MiB pool |
|---|---|---|
| h2d pinned 1 MiB, GB/s | 6.11 | 6.08 |
| h2d pageable 1 MiB | 4.58 | 4.58 |
| h2d pinned 16 MiB | 6.58 | 6.57 |
| d2d 1 / 16 / 256 MiB, GB/s | 69 / 16.0 / 193 | 65 / 17.3 / 193 |
| rlprobe stage 4, transfers per second | 2.30 s, 1263 | 2.30 s, 1267 |
| PPO steps per second (2 reps) | 71994, 66773 | 70433, 66032 |
| short kernel, ms/launch | 0.015 | 0.020 |
| pool mechanics | - | 2560 pages moved in, 2560 back (1536 behind the barrier), 64 barriers, 172 TSG preempts, 20 refused, 0 hazards, 10 MiB copied each way in 199 ms |
| host Xid | 0 | 0 |

Reading: the pool works mechanically but changes no number, so the CUDA copies below 16 MiB
do not go through guest BAR1 at all (they are copy-engine work on the mirror channel), and
the gap has another cause. The d2d row says which: 1 MiB takes 16 us (66 GB/s), 256 MiB 1.4 ms
(193 GB/s, native), but 16 MiB takes 1.0 ms, a fixed stall of about 0.9 ms per copy. That is
the length of a trapped guest interrupt handling (about 70 register accesses at 23-27 us,
display-e.md): a copy longer than libcuda's spin window waits for the completion interrupt,
and its delivery pays the trapped interrupt path. The same applies to the pinned 1 MiB copies
(170 us against 85 native) and to the synchronous D2H of PPO and rlprobe. The interrupt page
(intr-page.md, merged 8cfe26f, measured only on the display so far) is therefore the next
measurement for the copy rows, not the pool; H stays as an option for guest BAR1 traffic
(display scanout imports, the console mapping) and for Ampere and later, where the pool has
no host BAR1 limit.

## The interrupt page removes the copy stall (2026-09-24, 18:24-19:07, vm0)

The measurement the design H section asked for: `caraxes/runs/intr-copy-2026-09-24/run-vm0.sh`
(the vm2 script of `caraxes/runs/bar1-pool-2026-09-24/intr-copy.sh` moved to vm0 at 2 GiB in
slot 0, the display-series binary `vfio-user-nvgpu-fepage` of main with both pages, pool, token
pool, hybrid reactor), `CARAXES_INTR_PAGE` off against on, copy, rlprobe and short kernel with
5 reps each and PPO with 2; `summary.md` there. The host was busy (other VMs' device processes
were active in 3 of 15 off runs and 7 of 15 on runs; load 4.3-4.8), so the spreads are wide;
the medians tell the story. 0 host Xid in both modes.

| Row | Page off | Page on | Native (closing series) |
|---|---|---|---|
| h2d pinned 1 MiB | 3.89 GB/s | 5.36 GB/s | about 12 |
| d2h pinned 1 MiB | 3.74 | 5.41 | about 12 |
| h2d pinned 16 MiB | 6.29 | **11.9** | 12-13 |
| d2h pinned 16 MiB | 6.31 | **12.0** | 12-13 |
| d2d device 16 MiB | 14.6 | **90.1** | about 190 |
| d2d device 256 MiB | 178 | 175 | 193 |
| h2d pinned 256 MiB | 12.9 | 12.9 | 12.9 |
| rlprobe stage4 (50 episodes) | 18.7 s | 14.7 s | about 8 |
| rlprobe transfers per second | 154 | 195 | |
| PPO steps per second (2 runs) | 7628, 8340 | 10109, 9054 | |
| short kernel, ms per launch | 0.232 | 0.128 | 0.045 |

Reading: the 16 MiB copies were the stall (design H section: a fixed 0.9 ms per copy, the
trapped completion interrupt). With the page the pinned 16 MiB copies run at native bandwidth
and the d2d 16 MiB copy goes from 14.6 to 90 GB/s; the 256 MiB rows do not move because they
were bandwidth-bound already. What stays at 1 MiB (5.4 against about 12 GB/s native) is the
per-copy submission and completion cost that the page does not touch: the copies are copy-engine
work on the mirror channel with a trapped fence wait at the end (alloc-path.md, the BAR2 memfd
rows are the next candidates). rlprobe's stage4 loses 4 s of its 10.7 s gap to native, PPO gains
15-30 percent. The page is on in every later series.

Two caveats. The off mode of the first attempt collided with a second instance of the same
script on the same VM (both waiting for the v1 gate run; the folder `off/` holds only the clean
rerun of 19:00-19:07). And the binary predates the FB_FLUSH_GPU_CACHE flag fix for the 615 host
(6450203), so the host kernel logged flush refusals during both modes (`hostwatch.txt`: 204 kernel
log lines in off); both modes share the binary, so the comparison holds, and later series use a
binary from main after the fix.

## First 615 series (2026-09-24, 18:54-19:37, vm2, host 615.71.09, other VMs active)

The first numbers of a 615.71.09 guest, and the 610.57.04 guest on the same VM slot for the
comparison, both on the 615.71.09 host. `caraxes/runs/bench-615-2026-09-24/run.sh` with the
environment of the 615 guest's cuda gate runs (`caraxes/runs/cutover-615-2026-09-24/v2-615guest-run.sh`):
vm2, 1 GiB profile at reservation slot 3, token pool (64) and hybrid reactor, no P21 area
(`CARAXES_BAR1_PASSTHROUGH` unset), restart per rep; short-auto, rl, copy and init with 3 reps,
PPO (60 s) with 2. The device is built from the worktree at a0bf4ef: the v615-feature build for
the 615 guest, the default build for the 610 guest (a v615 build serves guest ABI 615 unless
told otherwise; the first 610 attempt with it, `610-v615build/`, got cuInit 999 in every run).
It needed the harness fix of that commit: bench-run.sh now takes the guest's libcuda directory
from the driver (cutover-615.md, "Closed: the bench series"). vm0 and vm1 (and later vm3) ran on
the card at the same time: 5 of 14 runs of the 615 guest and 9 of 14 of the 610 guest had
another VM's device process active, and the host load average reached 10.9 during the 610
part. 0 Xid over both parts and the failed attempt; host watch 0 alerts, nvidia-smi at most
0.15 s. Full tables: `615/summary.md`, `610/summary.md`, and `comparison.md` (both against the
closing series' native and v2 columns).

| Metric | native (closing) | v2 closing (610 host) | 610 guest | 615 guest | 615 / 610 guest |
|---|---|---|---|---|---|
| short kernel, ms/launch, auto | 0.006 | 0.015 | 0.307 [0.157-0.382] | 0.144 [0.127-0.148] | 0.47 |
| rlprobe stage 4, s | 1.20 | 2.10 | 20.4 [19.5-22.4] | 15.2 [15.0-20.2] | 0.75 |
| rlprobe transfers per second | 2350 | 1340 | 141 | 189 | 1.34 |
| rlprobe process wall, s | 4.64 | 12.2 | 41.0 | 27.5 | 0.67 |
| rlprobe mean10 (reward) | 56.9 | 56.9 | 56.9 | 56.9 | 1 |
| copy h2d pinned 1 MiB, GB/s | 12.1 | 6.06 | 3.90 | 3.85 | 0.99 |
| copy h2d pageable 1 MiB | 7.25 | 4.50 | 2.92 | 2.65 | 0.91 |
| copy h2d pinned 16 MiB | 13.0 | 6.57 | 6.28 | 6.29 | 1.00 |
| copy d2d 16 MiB | 183 | 16.5 | 14.5 | 14.1 | 0.97 |
| copy h2d pinned 256 MiB | 13.1 | 13.1 | 12.9 | 12.8 | 0.99 |
| copy h2d pageable 256 MiB | 9.27 | 8.86 | 4.43 | 3.32 | 0.75 |
| copy d2d 256 MiB | 186 | 190 | 177 | 175 | 0.99 |
| cuInit, ms | 106 | 151 | 212 | 135 | 0.64 |
| cuCtxCreate, ms | 195 | 3770 | 4144 | 4349 | 1.05 |
| cuCtxDestroy, ms | 92.6 | 1352 | 1350 | 1517 | 1.12 |
| PPO environment steps per second (2 reps) | 107663, 112259 | 61082, 72977 | 6964, 6471 | 5727, 6726 | 0.93 |
| PPO reward100 after 60 s | 212, 400 | 348, 417 | 36.8, 36.8 | 30.2, 36.8 | |

Medians with [min-max] where the spread matters; the native and closing columns are the
closing series (§ "Closing series on driver 610.57.04" and its PPO table), not rerun here.

Reading: the 615 guest costs nothing against the 610 guest on the same device configuration.
The ranges overlap in every row but the short kernel, and where the 615 guest is ahead (short
kernel, rlprobe) the 610 part had the busier host. The distance to the closing v2 column is the
configuration, not the driver: without the P21 area every submission traps its GPFIFO and
USERD writes, and the submission rows are those of the no-area run of § "P21 BAR1 passthrough"
(short kernel 0.153 and 0.161 ms, rlprobe stage 4 19.6 and 18.3 s, 144 transfers per second);
PPO, a few launches and two small copies per environment step, runs at about 0.1 of closing.
The context rows (cuCtxCreate about 20x native), the 16 MiB copies and the pinned and d2d
256 MiB copies match the closing series on both guests. Lower on both guests and not isolated
here (the area, the 1 GiB profile and the concurrent VMs all differ): the 1 MiB copies (3.9
against 6.1 GB/s pinned, d2d 9 to 10 against 62) and the pageable 256 MiB copies (3.3 to 4.4
against 8.9 GB/s). Profiles differ from the closing series in: host driver, P21 area off,
hybrid instead of spin reactor, 1 GiB at slot 3 instead of 2 GiB at slot 0, other VMs active
instead of a quiet host, n=3 instead of 5. Next: the same series with
`CARAXES_BAR1_PASSTHROUGH=1` and the spin reactor on a quiet host, the like-for-like with the
closing series.

## Night series (2026-09-24 23:16 to 2026-09-25, vm2, host 615.71.09)

The queued optimisation measurements, in order, on vm2 only: the 1 GiB profile at reservation
slot 3, Cloud Hypervisor 0001-0006, token pool 64, and the frozen binaries of main 57693d8
(`/mnt/vmstore/caraxes/vfio-user-nvgpu-main-57693d8` for the 610 guest,
`-v615-57693d8` for the 615 guest). The common environment is
`caraxes/runs/night-2026-09-24/env.sh`: hybrid reactor, interrupt page on, BAR1 pool, unless a
part says otherwise. Every part ran under the host watch, and every host Xid line was checked
against the device pids of the part (`device-pids.txt` in each run directory).

Harness changes of the night, each commented in place:

- `caraxes/tools/bench-run.sh` `v2_restart`: a reservation refused under the D11 headroom rule
  (`hostmem.rs:1328`) is retried every five minutes, up to `V2_HEADROOM_RETRIES` (12) times,
  instead of losing the rep. It never fired tonight.
- `caraxes/runs/workload-2026-09-24/run.sh`: `WORKLOAD_ENV` names the environment file in place
  of the mapping environment (vm2, slot 0, 2 GiB).
- The run scripts of the night: `caraxes/runs/{bar2-memfd,alloc-path,bench-615,workload}-2026-09-24/night/run.sh`.

### Part 1: BAR2 memfd, off against on (23:16-00:17, 610 guest, other VMs active)

`caraxes/runs/bar2-memfd-2026-09-24/night/`, `summary.md` there. The host was busy: vm1, vm3,
vm5-615 and later vm6 ran on the card, and another VM's device was active in 4 of 10 off runs
and 8 of 10 on-nopool runs; the host load average was 5 to 12.

| Mode | Trapped BAR2 writes in one context creation | Trapped BAR2 reads | Result |
|---|---|---|---|
| off (pool, memfd off) | 96269 | 478 (all the flush page 0xfea000) | series green |
| on (pool, memfd on) | 0 | 430 (flush page) | the first `bench-cu init` hung in cuCtxCreate for 16 min |
| on-syncdb (+ `CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1`) | 0 | 352 | hung (timeout 120 s) |
| on-nointr (pool, memfd on, interrupt page off) | 0 | 352 | hung (timeout 120 s) |
| on-nopool (memfd on, no pool) | 0 | 478 | series green, cuda gate 10 of 11 scored (cuda-managed, the known 801 of managed memory) |

| Row (median [min-max], n=5) | off (pool) | off-nopool | on-nopool | native (closing) | on-nopool / native |
|---|---|---|---|---|---|
| cuInit, ms | 187 [167-446] | 196 [144-240] | 71.6 [13.7-200] | 106 | 0.68 |
| cuCtxCreate, ms | 3793 [2930-11185] | 3863 [3129-4900] | **1077** [763-1726] | 195 | 5.5 |
| cuCtxDestroy, ms | 1462 [877-4920] | 1166 [946-1403] | **391** [149-726] | 92.6 | 4.2 |
| cuMemAlloc 1 MiB, us | 194 [182-347] | 581 [199-1138] (n=4) | 284 [228-391] | 35.8 | 7.9 |
| cuMemFree 1 MiB, us | 251 [247-464] | 680 [234-1680] (n=4) | 561 [349-703] | 27.1 | 21 |
| cuMemAlloc 64 MiB, us | 5174 [4544-13151] | 5590 [4605-9301] (n=4) | **892** [780-4826] | 78.3 | 11 |
| cuMemFree 64 MiB, us | 3372 [2679-9568] | 3945 [2694-6222] (n=4) | 1347 [1206-6930] | 92.6 | 15 |
| init device CPU per run, ms | 2920 | 2320 | 720 | - | |
| init VMM CPU per run, ms | 8590 | 6780 | 1190 | - | |

off-nopool is the `intr` mode of part 2 (the same configuration, recorded there with the traced
churn); its churn rep 4 lost its ssh connection mid-run and has no result.

Reading:

- The memfd does what bar2-memfd.md §6 expected: no BAR2 write traps any more, the flush page
  is the only trapped BAR2 page, and cuCtxCreate drops from 3.8 s to about 1.1 s on this busy
  host (0.53 s on the quieter host of part 2), cuCtxDestroy from 1.2-1.5 s to 0.4 s. The
  device's exit summary of the on-nopool instance (the last rep plus the cuda gate): 35237 syncs,
  7.6 s of sync time in all (32.7 ms the longest), 0 alias conflicts, 0 sysmem write-backs skipped.
- **The memfd and the BAR1 pool do not work together.** With the pool every context creation
  hangs at the first GR channel's doorbell: the guest's `bench-cu` spins in user space at 100 %
  CPU, and the device logs host notifier 0x800000c firing continuously (157978 events and MSIs
  to guest vector 4 in 16 minutes, `on/bar2-traps-whole-run.txt`, `on/trace-tail.txt.xz`;
  18067 and 17677 in the 120 s of on-syncdb and on-nointr), from the first doorbell of chid 18
  on. The pool itself moved no page (`on/device-summary.txt`: 0 pages in the pool, 2 polls).
  Syncing at every doorbell does not help, and neither does turning the interrupt page off, so
  rule 2 of bar2-memfd.md §3 is not the cause; the pool and the memfd share the translation hook
  `FakeFb::backing` (`crates/vfio-user-nvgpu/src/fb.rs:922`, the pool's remap check at `:774`
  and `:1107`), which is where to look. Not fixed here (no device code tonight); the allocation
  part and the recommendation below run without the pool.
- The two host Xid lines of part 1 (23:32:08 and 23:32:25, Xid 31, MMU fault on CE2) are
  pid 1131576, vm1's device process at that time (`off/v2/churn-5.txt` procs line), not vm2's.

### Part 2: the allocation path (00:24-01:17, 610 guest, no pool)

`caraxes/runs/alloc-path-2026-09-24/night/`, alloc-path.md §6 in the night configuration
without the pool (part 1): `off` is the 24-trap baseline (interrupt page off, memfd off, lazy
off), `intr` the interrupt page alone, `stack-off` step 1 (interrupt page and memfd), `stack-on`
step 1 plus `CARAXES_ALLOC_LAZY=1`. Per mode one traced `bench-cu-gap churn 1` (phases 300 ms
apart) through `caraxes/tools/rpc-cost.py`, then init and churn with 5 reps and a restart per
rep; the cuda gate in stack-on. Host load average 2.6 to 5.3; another VM active in 4 to 6 of
10 runs per mode. 0 host Xid in all four modes.

Series (median [min-max], n=5):

| Row | off | intr | stack-off | stack-on | native (closing) | stack-on / native | closing v2 |
|---|---|---|---|---|---|---|---|
| cuMemAlloc 1 MiB, us | 972 [835-1689] | 581 [199-1138] | 297 [181-338] | **175** [166-222] | 35.8 | 4.9 | 830 |
| cuMemFree 1 MiB, us | 1020 [899-1698] | 680 [234-1680] | 541 [292-561] | **401** [280-532] | 27.1 | 15 | 903 |
| 64 x 1 MiB cycle, ms | 227 | 102 | 48.1 | **37.1** | 3.81 | 9.7 | 168 |
| cuMemAlloc 64 MiB, us | 7337 [6452-8664] | 5590 | 650 [611-815] | **541** [529-588] | 78.3 | 6.9 | 6747 |
| cuMemFree 64 MiB, us | 4926 | 3945 | 961 | **838** [808-892] | 92.6 | 9.0 | 4422 |
| 8 x 64 MiB cycle, ms | 154 | 113 | 15.5 | **11.7** | 10.6 | 1.10 | 115 |
| cuCtxCreate, ms | 3493 [3306-4305] | 3863 | 534 [471-1183] | **533** [389-943] | 195 | 2.7 | 3770 |
| cuCtxDestroy, ms | 1272 | 1166 | 174 | **150** [119-240] | 92.6 | 1.6 | 1352 |
| cuInit, ms | 209 | 196 | 44.3 | 51.3 | 106 | 0.48 | 151 |
| churn device CPU per run, ms | 6830 | 4960 | 2410 | 1850 | - | | |
| churn VMM CPU per run, ms | 21230 | 11610 | 3290 | 2170 | - | | |

Trapped accesses per operation (rpc-cost.py section 4, one traced process per mode; the times
are inflated by tracing, the counts are not):

| Operation | off | intr | stack-off | stack-on | rpc-cost's projection for "both" from off |
|---|---|---|---|---|---|
| cuMemAlloc 1 MiB | 45.5 | 6.5 | 4.2 | 4.3 | 4.4 |
| cuMemFree 1 MiB | 50.8 | 8.4 | 7.3 | 7.2 | 7.5 |
| cuMemAlloc 64 MiB | 370.6 | 272.6 | 77.9 | 13.9 | 16.1 |
| cuMemFree 64 MiB | 232.5 | 83.8 | 18.8 | 18.9 | 19.9 |

Reading:

- The baseline's 1 MiB allocation is 45.5 traps, not 24: the VF interrupt tree accounts for
  24.4 of them (52 %) and the RM lock's interrupt-enable writes for 14.7, the MMU invalidate
  2.5, BAR2 PTE writes 2.0 and the flush read 1.0 (`off/rpc-cost.md`). The interrupt page takes
  39 of the 45.5, the memfd the BAR2 writes; what is left is the invalidate, the flush read and
  one config-mirror re-arm, as alloc-path.md §2 step 3 predicted.
- The measured counts match rpc-cost.py's projection within one trap per operation; for the
  64 MiB allocation the lazy plans remove another 64 traps (77.9 to 13.9) that the projection
  did not know about (the eager plan's work under the invalidate).
- Step 1 takes cuMemAlloc 1 MiB from 972 to 297 us, the lazy plans to 175 us (4.9x native,
  2x v1's 93 us); the 64 MiB allocation from 7.3 ms to 0.54 ms, and the 8 x 64 MiB churn cycle
  is at 1.1x native. The free path stays at 15x native for 1 MiB (seven traps: the invalidate
  twice, the flush read).
- The lazy plans cut the device's CPU per churn run by a quarter (2410 to 1850 ms) and did not
  change cuCtxCreate. Exit summary of the last stack-on instance: 8001 invalidates deferred,
  planned in 330 batches at a doorbell and 407 at the 20 ms deadline, largest batch 68.
- cuda gate in stack-on: 10 of 11 scored pass, cuda-managed fails with the known 801
  (`stack-on/gate-cuda-2026-09-25/table.txt`), as in on-nopool of part 1.

### Part 3: the like-for-like 615 series (01:21-02:59)

`caraxes/runs/bench-615-2026-09-24/night/` (`run.sh`, `615/`, `610/`, `comparison.md`): the
closing series' metrics and reps (all of bench-run.sh's metrics with 5 reps and a restart per
rep, the startup metric with 5, PPO 60 s with 2) and its spin reactor, the 615 guest and then
the 610 guest, both on vm2 at 1 GiB slot 3 with the frozen 57693d8 binaries and the interrupt
page on. Two departures from the closing configuration:

- **No P21 area.** The frozen binary refuses `CARAXES_BAR1_PASSTHROUGH=1` ("the fixed-layout
  prototype is refused: it mismatched 128 pages on 2026-09-24", starts without the area; the
  first attempt, `615-area-refused/device-start.txt`, was stopped after rep 1). Both parts run
  with the BAR1 pool (`CARAXES_BAR1_PASSTHROUGH=pool`, design H), which covers GPFIFO and USERD
  as the area did.
- **The host was quiet for the 610 guest only.** The script waited for no other device process
  before each guest; the 615 part found a quiet host at 01:21, but vm3 and vm5-615 started during
  it (another VM's device active in 25 of 65 runs, load average 3.4 to 13.9). For the 610 part
  it waited 45 minutes (vm0, vm3) and then ran with no other device process in any of its 65
  runs (load 2.6 to 7.6).

0 host Xid in both parts and in the stopped attempt; host watch 0 alerts on the quiet 610 part,
one (2.3 s) during the busy 615 part.

| Metric | native (closing) | v2 closing (610 host) | 615 guest (busy host) | 610 guest (quiet) | 615 / native | 610 / native |
|---|---|---|---|---|---|---|
| short kernel, ms/launch, auto | 0.006 | 0.015 | 0.018 | 0.016 | 3.0 | 2.7 |
| short kernel, blocking | 0.021 | 0.100 | 0.085 | 0.089 | 4.0 | 4.2 |
| launch 30M-cycle, auto | 16.3 | 17.3 | 17.3 | 17.0 | 1.06 | 1.04 |
| rlprobe stage 4, s | 1.20 | 2.10 | 2.60 | 2.00 | 2.2 | 1.67 |
| rlprobe transfers per second | 2350 | 1340 | 1106 | 1403 | 0.47 | 0.60 |
| copy h2d pinned 1 MiB, GB/s | 12.1 | 6.06 | 6.07 | 6.08 | 0.50 | 0.50 |
| copy h2d pinned 16 MiB | 13.0 | 6.57 | 6.57 | 6.57 | 0.50 | 0.50 |
| copy d2d 16 MiB | 183 | 16.5 | 16.5 (11.5 to 176) | 16.9 (11.4 to 178) | 0.09 | 0.09 |
| copy h2d pinned 256 MiB | 13.1 | 13.1 | 13.1 | 13.1 | 1.00 | 1.00 |
| matmul N=1024, GFLOP/s | 4894 | 1340 | 1310 | 4850 | 0.27 | 0.99 |
| matmul N=4096 | 6052 | 4433 | 4627 | 5419 | 0.76 | 0.90 |
| cuInit, ms | 106 | 151 | 198 | 161 | 1.9 | 1.5 |
| cuCtxCreate, ms | 195 | 3770 | 4444 | 4017 | 23 | 21 |
| cuCtxDestroy, ms | 92.6 | 1352 | 3057 | 2794 | 33 | 30 |
| module load to first cuInit, ms | - | 3509 | 3825 | 3813 | | |
| cuMemAlloc 1 MiB, us | 35.8 | 830 | 238 | 204 | 6.6 | 5.7 |
| cuMemFree 1 MiB, us | 27.1 | 903 | 463 | 273 | 17 | 10 |
| cuMemAlloc 64 MiB, us | 78.3 | 6747 | 4919 | 4841 | 63 | 62 |
| PPO environment steps per second (2 reps) | 107663, 112259 | 61082, 72977 | 64423, 51178 | 71378, 71281 | 0.54 | 0.65 |

Reading:

- The 615 guest and the 610 guest are the same device: every row overlaps, and where the 615
  guest is behind (rlprobe, PPO, cuCtxDestroy, the matmuls) it had the busier host.
- Against the closing series (610 host, P21 area, no interrupt page) the quiet 610 guest matches
  or beats it in every submission and copy row: short kernel 0.016 against 0.015 ms, rlprobe
  stage 4 2.0 against 2.1 s, PPO 71300 against 61000-73000 steps/s. The interrupt page moves
  the allocation rows (cuMemAlloc 1 MiB 830 to 204 us) and, on the quiet host, the small matmul
  (N=1024 1340 to 4850 GFLOP/s, 0.99 of native: its allocation and synchronisation traffic was
  the RM lock's interrupt-enable traps).
- Two rows did not move as the vm0 measurement promised, and one moved the wrong way:
  - d2d 16 MiB stays at 16.5 GB/s (vm0 with the page: 90); the pinned 16 MiB copies stay at
    6.57 (vm0: 11.9). vm0 ran the hybrid reactor, 2 GiB, the display configuration; here the
    spin reactor. Not isolated tonight.
  - cuCtxDestroy is 2.8-3.1 s against 1.35 s closing; part 2's hybrid runs without the pool had
    1.2 s (intr) and 0.15 s (with the memfd). Not isolated either (spin reactor, pool).
- The context rows are the BAR2 traffic of parts 1 and 2: without the memfd cuCtxCreate is
  20x native on both guests.
