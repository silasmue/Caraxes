<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# GSP plan B and C: run record, 2026-09-24

The plan and its status are in docs/caraxes/gsp-plan/b-host-forwardable.md §8 and
c-guest-state.md §8.

## Device binaries

| Phase | Binary | sha256 |
|---|---|---|
| before | the worktree base 0899c72, `/mnt/vmstore/caraxes/vfio-user-nvgpu-gspbc-before-0899c72` | 122e23ba93ef46dab26a87d33cbcd399ead59367ad616564a981681bcf461913 |
| after | commit b49cc5c, `/mnt/vmstore/caraxes/vfio-user-nvgpu-gspbc-after-b49cc5c` | 023b8524a92af414259a811e4e7ebf37e41430d24e360ae155826be4a509cf3d |

Both were built with `cargo build --release -p vfio-user-nvgpu`, `CARGO_TARGET_DIR=target/bc`.

## The VM

vm5 ran the 610 guest image. Its state directory is new: a copy of vm3's 610 overlay (nvidia.ko
built in the guest) and a new cloud-init seed for 192.168.105.26 on tap caraxes5.

## The GPU configuration was refused

`env-vm5.sh` holds the GPU configuration: a 1 GiB profile at slot 6, token pool, hybrid
reactor, no BAR1 area. Before each attempt, neither vm5 nor the v1 bench VM ran.

- **Before (`before-slot6-refused/`)** and **after (`after-slot6/`)**: the host RM refused
  the reservation `[0x29800000, 0x69800000)` with NV_ERR_NO_MEMORY. The largest free region
  was 2052 to 3072 MiB higher up, while vm0 and vm2-615 ran on the card. The device failed
  closed and the guest never started.
- No gate ran, and no host Xid occurred in either window.

## Pure emulation (`env-vm5-emu.sh`)

This is the same VM without a host RM client, a reservation or host execution.

- **Before (`before-emu/`)** and **after (`after-emu/`)**:
  - nvml gate: fail, because the forwarded telemetry is absent without a host.
  - cuda gate, the eight scored tests without torch: fail, because cuInit needs the host
    forwards.
- **The comparison (`compare-emu.md`, made by `compare.py`).**
  - Every control answer (id, status, label) is identical before and after.
  - There is no new refusal.
  - The gate rows are unchanged.
  - Neither nvidia-smi nor cuInit reaches a B or C row in this mode.
- No host Xid, no kernel log line of the device.

## Q-B1 sweep (`qb1-sweep/`)

- Command: `cargo run --release -p vfio-user-nvgpu --example qb1-sweep`, at 19:08:41.
- Method: the 30 F1/F2 getters, each once, with a zeroed buffer, on the subdevice of the
  device's own unprivileged host RM client, host driver 615.71.09.
- Record: `sweep.tsv` lists every control with its status and answer. Process names and
  PIDs are counted, never printed.
- Kernel log from 19:08 on (`host-kernel.txt`): no entries.
