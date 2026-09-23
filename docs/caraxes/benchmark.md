<!-- SPDX-License-Identifier: MIT -->
# Benchmark: Leandro v1, Leandro v2 (Caraxes) and native (P13)

Written 2026-09-24. Goal P13 in [README.md](README.md) (Ideas table): measure the two paths
against the bare card. Harness: `caraxes/tools/bench-run.sh`, `bench-cu.c`, `bench-v1vm.sh`,
`bench-summary.py`. Raw outputs, summaries and the command log:
`caraxes/runs/bench-2026-09-24/` (`commands.txt` lists every command in order).

**Status.** STATUS_LINE

## 1. What is compared

| | v1 (Leandro) | v2 (Caraxes) | native |
|---|---|---|---|
| Path | API forwarding: guest `nvrm_nodes` + `virtio_nvrm` forward every RM ioctl and mapping to `vhost-user-nvrm`, which replays it on the host RM | synthetic PCI device: the stock `nvidia.ko` in the guest drives `vfio-user-nvgpu`, which backs the FB with reserved host VRAM and mirrors the guest's channels onto host channels | the same programs on the host |
| Hypervisor | Cloud Hypervisor v53.0 + `patches/0001-0004` (`vendor/cloud-hypervisor/target/release/cloud-hypervisor`), `--generic-vhost-user device_type=60` | Cloud Hypervisor v53.0.0 (nix, `/mnt/vmstore/leandro/cloud-hypervisor/bin/cloud-hypervisor`), `--user-device` | - |
| Device process | `vhost-user-nvrm` built from `5de9b40` (`target/bench/release`, `caraxes/tools/bench-v1vm.sh`) | `vfio-user-nvgpu` from `target/release` (sha256 in the run's `run.txt`), `tools/caraxes-vm.sh` | - |
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
| 5 | startup | `bench-run.sh startup v1\|v2`: module load (v2: `rmmod nvidia`, function-level reset, `modprobe nvidia`; v1: reload `nvrm_nodes` + `virtio_nvrm`) to the first `nvprobe 0` that passes; `./bench-cu init` (cuInit, cuCtxCreate, cuCtxDestroy in one process); `time ./nvprobe 0` and `time ./nvprobe 1` (bash `time`: the host has no `/usr/bin/time`) |
| 6 | RM-call cost | `./ioctlping 10000` (`NV_ESC_CHECK_VERSION_STR` on `/dev/nvidiactl`), `./ctrlping 2000 64 512 4096 16384` (`RM_CONTROL` `GET_BUILD_VERSION`, the probe reports the total parameter bytes 232/1576/12328/49192) |
| 7 | VRAM churn | `./bench-cu churn 20`: per cycle 64 x 1 MiB and 8 x 64 MiB `cuMemAlloc`, then `cuMemFree` in reverse order |
| 8 | device-process CPU and RSS | recorded around every run above |

`vrampress` and `vramcap.py` were not used for metric 7: both size themselves from the free
memory the target reports, which differs between the host (about 2 GiB free beside the
desktop and the other VMs) and a 4 GiB guest, so they are no fixed pattern.

## 3. Conditions

CONDITIONS

## 4. Results

RESULTS

## 5. Interpretation

INTERPRETATION

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
modules`). All v1 numbers in §4 come from series 1b. The restart leaves the guest page cache
cold, which shows in the process wall time of the first Python run of a rep (torch import:
`rl` process wall 17-86 s against 5-10 s native); the in-process numbers (stage-4 time,
GFLOP/s) are not affected. Not tried: raising `LEA_MAX_PIN_TOTAL_MIB` and `max_pin` (only
postpones it; the quarantined guest pages stay allocated in the 4 GiB guest).

## 7. v2 limitation found: one reserving device per host

`CARAXES_RESERVE_VRAM=1` reserves the FB at a fixed host address, the top of the host heap
(`[0xe9000000, 0x1e9000000)` for 4096 MiB, `[0x1a9000000, 0x1e9000000)` for 1024 MiB). With
vm2's device holding its reservation, a second device on vm1 was refused even at
`CARAXES_FB_MIB=1024` although 2455 MiB were free: `the host RM did not give
[0x1a9000000, 0x1e9000000) (RM status 0x51 (NV_ERR_NO_MEMORY) on ioctl 0x2b) ... no
floating fallback (the guest's FB layout is fixed at start)` (`caraxes/runs/bench-2026-09-24/v2-up1/run.txt`).
So v2 currently runs one host-executing VM per GPU, which is why the v2 series waited for vm2.
