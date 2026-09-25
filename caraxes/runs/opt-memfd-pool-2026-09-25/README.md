<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# BAR2 memfd beside the BAR1 pool, 2026-09-25 (vm3)

Backlog item 1 of [docs/caraxes/optimization-backlog.md](../../../docs/caraxes/optimization-backlog.md):
the cause, the fix and the numbers are in [docs/caraxes/bar2-memfd.md](../../../docs/caraxes/bar2-memfd.md) §8.

The device is `/mnt/vmstore/caraxes/vfio-user-nvgpu-opt1-bb52a46`, a release build of main d466e9d
plus the fix (commit bb52a46). The environment is `run.sh`'s: the census's environment
(`../trap-census-2026-09-25/run.sh`, gate610 of `../gsp-rpc-2026-09-24/run.sh`), which is vm3, the
610 guest, 1 GiB at reservation slot 5, host RM and host execution. The windows are the interrupt
page, the BAR1 pool at 32 MiB, the token pool at 64 and the hybrid reactor. `on` adds
`CARAXES_BAR2_MEMFD=1`; `off` does not.

| Directory | What | Result |
|---|---|---|
| `unit-before.txt` | the two new unit tests on the code before the fix | both fail: 0 pages moved; a read of the guest page missed the memfd's bytes |
| `on/` | census (`census-cu` init x3, ctx 3, ops 10 x2), rlprobe, PPO 30 s, short-auto, cuda gate, copy x3 | cuCtxCreate completes (0.61 s, 1228 traps); gate 10 of 12 (cuda-managed); 0 host Xid |
| `off/` | the same session's reference without the memfd: rlprobe, PPO, short-auto, copy x3 | the pool's rows |
| `on-2/`, `off-2/` | the A/B again, short-auto and copy x4 each, back to back (device `-opt1-0c2cb79` with the re-arm switch off, the same code path as bb52a46) | the copy rows |
| `on-3/`, `off-3/`, `link-*-3.csv` | copy x3 each, with `nvidia-smi --query-gpu` (P-state, PCIe link gen and width, SM clock, utilization) sampled every 500 ms | the pinned rows' 6.5 and 12.9 GB/s modes follow the card's link state (gen 1/2/3 at P8/P5/P0) in both configurations |

Tables:
- `summary.md`: `summarize.py`, the census rows of `on/` beside the census's `off` and `on-nopool`;
- `copy-summary.md`, `copy-summary-2.md`: `caraxes/tools/bench-summary.py`, pool against pool and
  memfd, for the first and the second pass.

Per run directory: the census run's files (`setup.txt`, `device-start.txt`, `device-summary.txt`,
`census*.{txt,md,json}`, `census-trace.txt.xz`, `*-rpc-cost.md`, `hostwatch.txt`, `host-xid.txt`),
`work/` (the workload runs), `copy/` (the series), and `gate-cuda*`. The rl and ppo traces are in
`/mnt/vmstore/caraxes/opt-memfd-pool-2026-09-25/<run>/`.

Host Xid: 0 lines in every part (`*/host-xid.txt`). The host watch raised no alert. The read-only
host state reads were `nvidia-smi --query-gpu`, `/proc/driver/nvidia/version` and
`journalctl -k -g Xid`. Nothing on the host was changed. During the runs, other VMs' devices
(vm2-615, vm5-615) were up and at times active (`census-others.txt`, the MEAS lines).

Reproduce, from the worktree root. `run.sh` needs `target/bench/bin` with `census-cu` and the ROM
files (see `../trap-census-2026-09-25/README.md`):

```sh
CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-opt1-bb52a46 caraxes/runs/opt-memfd-pool-2026-09-25/run.sh on on
CENSUS=0 GATE=0 COPY=3 CARAXES_SERVER=... caraxes/runs/opt-memfd-pool-2026-09-25/run.sh off off
CENSUS=0 WORK=0 GATE=0 COPY=4 SERIES="short-auto copy" CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-opt1-0c2cb79 \
    caraxes/runs/opt-memfd-pool-2026-09-25/run.sh on|off on-2|off-2
python3 caraxes/runs/opt-memfd-pool-2026-09-25/summarize.py > summary.md
```
