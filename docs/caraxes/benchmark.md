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
