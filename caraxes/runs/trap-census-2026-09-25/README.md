<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Trap census, 2026-09-25 (vm3)

The runs behind [docs/caraxes/optimization-backlog.md](../../../docs/caraxes/optimization-backlog.md).
The device is `/mnt/vmstore/caraxes/vfio-user-nvgpu-main-1ea2529`, the newest main binary at the
start. The environment is `run.sh`'s: vm3, 610 guest, 1 GiB at slot 5, host RM and execution.

| Directory | What | Result |
|---|---|---|
| `build/` | vm3 up once; `census-cu` and `census.cubin` built in the guest (gcc 13.3, glibc 2.39, cuda.h 13.0 of the guest venv), copied to `target/bench/bin` | sha256 in `build.txt` |
| `build-failed-rom/` | the first attempt: the worktree lacked the git-ignored `caraxes/reference/rtx2070/rom.bin` | nothing started |
| `bare/`, `bare-2/` | no window | census, rl, PPO 30 s, short-auto |
| `off/`, `off-2/` | interrupt page, BAR1 pool 32 MiB, token pool 64, hybrid reactor | same; `off/`'s GR operations ran while another VM held the GR engine (10-30 ms each) |
| `on/` | `off` plus `CARAXES_BAR2_MEMFD=1` | cuInit x3 passes, then the first cuCtxCreate hung; stopped by hand after 8 min (`teardown.sh`), guest log in `guest-dmesg-hang.txt` |
| `on-syncdb/` | `on` plus `CARAXES_BAR2_MEMFD_SYNC_DOORBELL=1` | the same hang (census-cu's `timeout 240`), then stopped by hand |
| `on-nopool/` | interrupt page, token pool, hybrid reactor, BAR2 memfd, no pool | census, rl, PPO, short-auto |
| `native/`, `native-2/` | `census-cu` on the host | times only |
| `aborted-off-2/` | a start of `off 2` stopped during bring-up because `run.sh` was edited while it ran; the measured `off-2/` ran later | nothing measured |

Per run directory:
- `setup.txt`: the environment;
- `device-start.txt` and `device-summary.txt`: the device's start and exit lines;
- `census.txt`: census-cu's `OP` lines;
- `census-trace.txt.xz`: the device trace of all census processes;
- `census-table.md` and `census.json`: `caraxes/tools/census-table.py` on that trace;
- `census-others.txt`: the other devices' CPU during the census, from `off-2` on;
- `work/`: the bench-run.sh runs;
- `short-auto-trace.txt.xz` and `*-rpc-cost.md`: `caraxes/tools/rpc-cost.py` on each workload
  trace;
- `hostwatch.txt` and `host-xid.txt`.

The rl and PPO traces (47 MB) are in `/mnt/vmstore/caraxes/trap-census-2026-09-25/<run>/`.

Tables:
- `summary.md`: `summarize.py`, the passes pooled, background subtracted;
- `workload-rates.md`: `workload-rates.py`.

Host Xid: 0 lines in every part. Host watch: no alert.

Reproduce, from the worktree root:

```sh
caraxes/runs/trap-census-2026-09-25/run.sh build          # once
caraxes/runs/trap-census-2026-09-25/run.sh bare|off|on|on-nopool [label]
caraxes/runs/trap-census-2026-09-25/run.sh native [label]
python3 caraxes/runs/trap-census-2026-09-25/summarize.py > summary.md
python3 caraxes/runs/trap-census-2026-09-25/workload-rates.py > workload-rates.md
```

`run.sh` needs `caraxes/reference/rtx2070/rom.bin` and `synth-vbios.bin` (git-ignored, copied from
the main checkout) and `target/bench/bin` (bench binaries of the main checkout plus `census-cu`).
Do not edit `run.sh` while it runs: bash reads the script as it goes.
