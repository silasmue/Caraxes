<!-- SPDX-License-Identifier: MIT -->
# Xid 31 in cuda-hostreg (vm1, 2026-09-24): record

The analysis is in [docs/caraxes/memory-model.md](../../../docs/caraxes/memory-model.md) §2,
"The alias in the device".

**Cause.** hostregprobe's pinned buffer owned guest pages in the 2 MiB rounding of the
swiotlb alias. The guest gave them IOVAs 0xf0_xxxx_x000. The device's shadow filter
refused those leaves, the shadow had no PTE, and CE2 faulted (FAULT_PTE). This is a
page-placement effect, not a race and not the B and C work.

**Fix.** Commit "vfio-user-nvgpu: follow the guest's swiotlb alias". The binary is
`/mnt/vmstore/caraxes/vfio-user-nvgpu-xid31-9f3831c`, sha256 `331f6e6b...3cee`.

## Setup

Both modes use the environment of `caraxes/runs/gsp-bc-2026-09-24/gpu-vm1.sh`:
- vm1, 1 GiB profile at slot 4;
- the 610 guest;
- a token pool of 64 with the hybrid reactor;
- no BAR1 area;
- one VM start per mode, and vm1 down after each.

The other VMs on the card at the time were vm2, vm3 and vm5-615, plus the desktop. The
first `before` attempt at 23:2x was refused cleanly: 1449 MiB of host VRAM free, below the
1 GiB profile plus the 1 GiB headroom.

The guest's swiotlb pool was `[0xba2b7000, 0xbe2b7000)` in every run, the same as in the
failing run. The alias covered `[0xba200000, 0xbe400000)`, so the margins hold 512 pages.

## Runs

`run.sh before|after`. Each loop iteration runs:
- `caraxes/tools/hostreg-margin`, which registers every 2 MiB chunk that holds margin pages
  and round-trips it;
- hostregprobe.

| Run | Device | Device pid | Result | Host Xid lines |
|---|---|---|---|---|
| gsp-bc gpu-vm1, 19:33-19:44 (the case) | main 7621cfb | 508533 | cuda-hostreg 719 | `19:38:37 Xid 31, pid=508533, CE2 HUBCLIENT_CE0 faulted @ 0x100_0bb0f000, FAULT_PTE ACCESS_TYPE_VIRT_READ` |
| gsp-bc gpu-vm1-rerun, 23:0x | main 7621cfb | 1041334 | cuda-hostreg pass | none |
| `before/`, 23:28-23:32 | main 7621cfb | 1131576 | hostreg-margin 0 of 3: iterations 1 and 2 got 511 and 340 margin pages, and HtoD returned 719 with `bad=2088960` (the case's numbers); iteration 3 found no free margin page. hostregprobe 3 of 3 | `23:32:08 Xid 31, pid=1131576, CE2 HUBCLIENT_CE0 faulted @ 0x100_0bd3b000, FAULT_PTE ACCESS_TYPE_VIRT_READ`; `23:32:25 Xid 31, pid=1131576, same, @ 0x100_0bc33000` |
| `after/`, 23:33-23:47 | fix, 9f3831c | 1151433 | hostreg-margin 20 of 20 (256-512 margin pages in 4-11 chunks per iteration, every chunk correct), hostregprobe 20 of 20, full cuda gate 10 of 11: every stage except cuda-managed, which failed with the 801 of every gate that day | none |

Every Xid line is on the device pid of its own run.

In `after/`, the device logged `dma table: guest aliases from .../dma-ranges.guest: GPA
0xba200000..0xbe400000 -> IOVA 0xf000000000..0xf004200000` (`device-dma.txt`). The host
watch had one ALERT in each mode: a single slow `nvidia-smi` sample (2.4 s and 6.2 s)
while the VM started.

## Files

- `run.sh`: the script.
- `gsp-bc-refused.txt`: the 24 refusals of the failing window, from a device trace that is
  not committed.
- `<mode>/`:
  - `setup.txt`, `device-pid.txt`, `host-xid.txt`, `hostwatch.txt`;
  - `loop.txt` and `loop/`: every iteration's output;
  - `guest-dma.txt`: the guest's pool and table;
  - `device-dma.txt`, `device-summary.txt`;
  - `after/gate-cuda.txt` and `after/gate-cuda-run/`, without the device traces.
