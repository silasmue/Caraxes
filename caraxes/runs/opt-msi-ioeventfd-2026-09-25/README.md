<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# MSI re-arm through an ioeventfd, 2026-09-25 (vm3)

Backlog item 4 of [docs/caraxes/optimization-backlog.md](../../../docs/caraxes/optimization-backlog.md).
The mechanism and the numbers are in [docs/caraxes/intr-page.md](../../../docs/caraxes/intr-page.md) §4.

The device is `/mnt/vmstore/caraxes/vfio-user-nvgpu-opt1-0c2cb79`, main d466e9d plus bb52a46 (the
memfd with the pool) and 0c2cb79 (`CARAXES_MSI_REARM_IOEVENTFD`). The runs use the script
`../opt-memfd-pool-2026-09-25/run.sh on` with `OUT` pointing here. The environment is the
memfd-with-pool one: vm3, the 610 guest, 1 GiB at slot 5, host RM and host execution, the
interrupt page, the BAR1 pool at 32 MiB, the token pool at 64, the hybrid reactor and the BAR2
memfd. `CARAXES_MSI_REARM_IOEVENTFD=1` is added on top. The "before" is
`../opt-memfd-pool-2026-09-25/on/`: the same windows, with the re-arm trapped.

| Directory | What | Result |
|---|---|---|
| `on-rearm/` | census, rlprobe, PPO 30 s, short-auto, cuda gate | 0 writes of 0x88704 in every trace; 26017 re-arms through the eventfd; gate 10 of 12 (cuda-managed); 0 host Xid |
| `on-rearm-2/` | short-auto and copy x4 | short-auto 0.013-0.027 ms per launch (the single 0.312 of `on-rearm/` was not repeated) |

Tables:
- `workload-rates.md`: `workload-rates.py ../opt-memfd-pool-2026-09-25/on on-rearm`. These are
  the census's whole-process counts with two more rows: all trapped accesses, and the re-arms that
  came through the eventfd.
- `summary.md`: `summarize.py`, the census rows of `on-rearm` beside `on`.

The rl and ppo traces are in `/mnt/vmstore/caraxes/opt-msi-ioeventfd-2026-09-25/<run>/`, and
`workload-rates.py` finds them there. Host Xid: 0 lines in every part. The host watch raised no
alert. vm0 (Mutter) was not borrowed.

Reproduce, from the worktree root:

```sh
OUT=caraxes/runs/opt-msi-ioeventfd-2026-09-25 CARAXES_MSI_REARM_IOEVENTFD=1 COPY=0 \
  CARAXES_SERVER=/mnt/vmstore/caraxes/vfio-user-nvgpu-opt1-0c2cb79 caraxes/runs/opt-memfd-pool-2026-09-25/run.sh on on-rearm
python3 caraxes/runs/opt-msi-ioeventfd-2026-09-25/workload-rates.py ../opt-memfd-pool-2026-09-25/on on-rearm > workload-rates.md
```
