<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Coverage follow-up, 2026-09-25 (vm2)

The runs behind the status lines of [coverage-map.md](../../../docs/vfio/coverage-map.md) for:
- the 21 sweep getters now answered (chips/tu106/controls.toml, `FORWARD`);
- the forward cache (`src/fwdcache.rs`).

**Environment.** `run.sh`, the coverage map's environment:
- vm2, 610 guest, 1 GiB at reservation slot 3;
- the night environment plus BAR1 pool 32 MiB, `CARAXES_BAR2_MEMFD=1`,
  `CARAXES_MSI_REARM_IOEVENTFD=1`, `CARAXES_ALLOC_LAZY=1`;
- the profile on;
- this branch's plain release build, `target/cov2/release`, commit 61e7190 (`*/setup.txt`).

vm0, vm3 and vm5 were up throughout (`*/other-vms-at-start.txt`).

| Directory | What | Result |
|---|---|---|
| `refused-bar1-1/` | the first `off` start | Cloud Hypervisor could not mmap the BAR1 pool area (EIO). The host logged `can't alloc VA space for mapping` (host BAR1 full with the other VMs), so the guest never booted. `run.sh` now retries this case after five minutes, like a refused reservation |
| `off/` | `CARAXES_FORWARD_CACHE=0`: 10 x `nvidia-smi -q`, 20 x `nvidia-smi --query-gpu`, dmon/pmon/query loop, the control sweep | all ok; 0 host Xid |
| `on/` | the cache on: the same, then the nvml and cuda gates (the cuda gate's run directory in `on/gate-cuda-run/`, its device traces in `/mnt/vmstore/caraxes/coverage-followup-2026-09-25/gate-cuda-device/`) | all ok; cuda gate 10 of 11 (cuda-managed 801, the known exception); nvml gate fails on power N/A only, as in the map (known for the 610 guest); 0 host Xid |
| `off-2/` | `off` again, right after `on`, with the host quiet | all ok; 0 host Xid |

Every part left vm2 down (`vm2 left: 0` in `*/console.txt`). No host Xid in any part
(`*/xid-summary.txt`).

## The sweep

The sweep ran with the coverage map's table (`../coverage-2026-09-25/ctlsweep-table.h`):
- 119 controls sent, 53 skipped without their object (DISPLAY_COMMON; vm2 has no display);
- no control in the sweep's window has the label `unanswered`, in `off/` or `on/`
  (`*/sweep-controls.tsv`, profile-rank.py on the sweep's window);
- all 21 getters the map's sweep left unanswered have their new labels (`off/sweep-controls.tsv`);
- NV_OK answers: 31 in the map's sweep, 39 now (`*/work/ctlsweep-table-out.txt`).

Four of the seven new forwards get an error from the host card for the sweep's zeroed request,
and the device passes it on:
- `FLCN_GET_ENGINE_ARCH`, `FIFO_GET_LATENCY_BUFFER_SIZE` and `GPU_GET_IP_VERSION` answer
  `INVALID_ARGUMENT` (0x1f), because 0 is no engine or unit;
- `GR_GET_GPC_TILE_MAP` answers `INVALID_STATE` (0x40).

## The forward cache

`cache-compare.md` is from `compare.py`. For each nvidia-smi window it gives:
- the guest's controls and their device time;
- the host RM controls;
- for the 22 cached getters, the guest controls against the host calls.

| Window | Cached getters' guest controls | Host calls, off | Host calls, on | Their device ms, off (loaded) | off-2 (quiet) | on |
|---|---:|---:|---:|---:|---:|---:|
| 10 x nvidia-smi -q | 93 | 93 | 12 | 154.7 | 27.9 | 15.1 |
| 20 x nvidia-smi --query-gpu | 160 | 160 | 0 | 176.7 | 77.4 | 0.6 |
| dmon 20 s, pmon 5 s, query loop 6 s | 10 | 10 | 0 | 20.2 | 1.2 | 0.03 |

Cache rows over the device lifetime of `on/`, in `on/profile/dev.exit.tsv.xz`, window rows:
- 311 hits;
- 21 fills;
- 5 not kept.

The on run's 12 host calls in the `-q` window are that window's fills: the first process asks
each getter once.

The whole windows did not get faster because of the cache.

**Wall time per process, median.**

| | off | on | off-2 |
|---|---:|---:|---:|
| `nvidia-smi -q` | 1.46 s | 0.29 s | 0.20 s |
| `--query-gpu` | 0.41 s | 0.13 s | 0.07 s |

These times follow the host's load:
- the host watch's own `nvidia-smi` averaged 131 ms in `off`, 72 ms in `on` and 36 ms in
  `off-2`;
- the device time is dominated by the uncached GSS-legacy perf and clock commands. In `off`,
  perf 0xa8 took up to 1.36 s per call and perf 0x79 624 ms on the first call. In `on`,
  perf 0x79 took at most 23 ms per call.

The cache takes out what its getters cost, measured on a quiet host (`off-2` against `on`):
- 2.8 ms of about 83 ms of device time per `nvidia-smi -q`;
- 3.9 ms of about 13 ms per `--query-gpu` process;
- with the host loaded, 14 ms and 9 ms per process.

The GSS-legacy commands stay live on purpose:
- their layouts are not in the open headers;
- perf 0x79 moves between runs (fwdcache.rs module comment).

Reproduce, from the worktree root:

```sh
caraxes/runs/coverage-followup-2026-09-25/run.sh off      # then: run.sh on; run.sh off 2
cd caraxes/runs/coverage-followup-2026-09-25 && python3 compare.py off on && python3 compare.py off-2 on
```
