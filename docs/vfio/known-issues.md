<!-- SPDX-License-Identifier: MIT -->
# Known issues

Open defects and limits of Leandro VFIO as of 2026-09-30, with the evidence for each. A
fixed issue leaves this page; its record stays in the research note it names. The closed
issues at the end are listed because their symptoms are easy to mistake for an open one.

## Open

| # | Issue | Evidence | State |
|---|---|---|---|
| 1 | Windows, DX12: 3DMark Steel Nomad in exclusive fullscreen fails with "Exclusive full screen mode is not available" (`dx12_swapchain.cpp:238`). In windowed mode the host logs Xid 31 (GCC read at VA 0, FAULT_PTE) on the device's pid, then a TDR follows | [windows-findings.md](../research/windows-findings.md), step 23, KNOWN BUG #2 | open: whether the shadow missed a mapping or the guest faults on its own; a native run would tell |
| 2 | Linux, Shadow of the Tomb Raider: once on the 2026-09-30 binary the frame showed "neon" colours with intact geometry (surfaces read in the wrong format); with the evening's binary 3 of 6 runs gave no result (a hang at loading, a crash at start); the game alone takes 3.4 GiB of the 4 GiB guest VRAM beside the desktop. The gate run on the merged binary averaged 34.7 fps (2026-09-29: 12.1 to 15.4) with no Xid; its screenshots came after the benchmark, so the colour check had no frame to judge | [sottr-invgate-2026-09-29.md](../research/sottr-invgate-2026-09-29.md); the A/B runs A to F under `/mnt/vmstore/caraxes/runs-2026-09-29/sottr-ab` (only A, E and F have an Average FPS); the guest's `nvidia-smi` compute-app reading of 3401 of 4096 MiB in the reference run of 2026-09-30; `caraxes/runs/gate-2026-09-30/frame-check.sh` (the colour check that catches the broken frame) | bisect pending; the gate carries the rendering check |
| 3 | Windows, Sunshine with NVENC: 31 to 70 ms of host processing latency against 12 to 14 ms with x264 on a static desktop at 21 fps; a 60 fps measurement is owed. The KMD sends the Windows-only FIFO control 0x20801111 twice per frame in NVENC mode, which the device only acknowledges | `caraxes/runs/codec-2026-09-30/NOTE.md`, `matrix.md` | cause open |
| 4 | Two VMs at once can exhaust host VRAM: a shadow map is refused with NO_MEMORY, the leaf stays unmapped (fail closed), the guest's channel faults (host Xid 31, then a guest TDR or RC). The shadow VA space's page tables are host VRAM outside the VM's slice. The gate runs one GPU VM at a time | `caraxes/runs/codec-2026-09-30/matrix.md` (Linux B l2 beside the Windows VM), [host-resources.md](host-resources.md) | at most one VM until the shadow tables are budgeted inside the slice and a failed map cannot leave a runnable channel |
| 5 | Host disk full: on 2026-09-29 a failed virtio-blk write damaged the Windows guest's NTFS; autochk and `sfc /scannow` repaired it. The Windows kit refuses a start below 30 GiB free and the trace stops below 10 GiB | [windows-findings.md](../research/windows-findings.md), step 22 | keep 30 GiB free on the VM store |
| 6 | CUDA managed memory (`cudaMallocManaged`) ends in a host Xid 31: the guest's nvidia-uvm expects replayable faults, which the device does not forward; `cuda-managed` and `cuda-gdb` are the two open tests of the cuda gate | [uvm-faults-2026-09-28.md](../research/uvm-faults-2026-09-28.md), [uvm-guest-ram-2026-09-28.md](../research/uvm-guest-ram-2026-09-28.md) | design open: fault forwarding, or managed memory pinned in guest RAM |
| 7 | Traps: 3D workloads still exit 3200 to 8600 times per second (Windows Heaven, Linux glmark2 and a Moonlight stream), 14 to 24 % of a vCPU at 35 to 40 us per round trip; the largest sources are Windows' interrupt-tree stores and Linux' MMU invalidates and flush reads | [trap-census-2026-09-30.md](../research/trap-census-2026-09-30.md) | the elimination plan is ranked there; expected about 250 traps/s (Windows Heaven) and 190/s (Linux) |
| 8 | nouveau with NVK (vm10): vkmark's cube scene under Weston ends in `vk::DeviceLostError` (nouveau: "job timeout, channel 7 killed") and the device logs 16 BAR1 pool remap refusals ("barrier failed: chid busy"). The lf16 gate of 2026-09-29 passed with vkmark 4790 and no errors; final1 and every later binary fail | `caraxes/runs/gate-2026-09-30/stack/RESULT.md` | regression in 1bdf9089..424a43e8 (72 commits under `crates/`); the next binary to test is train0-991e3d86 (lf24) |

## Limits by design, for now

- One VRAM slice per VM, fixed at start; no QoS for GPU time, channel count or RM work
  between VMs ([performance-shape](../wiki/performance-shape.md)).
- Execution only for Turing identities on the RTX 2070; other chips boot in pure emulation
  ([identity-and-costume](../wiki/identity-and-costume.md)).
- Every isolation run used a stock driver; no adversarial guest has been tested
  ([isolation-rules](../wiki/isolation-rules.md)).
- TU106 has no AV1 encoder or decoder, natively too; both fail cleanly in the guest.

## Closed recently

- Guest reboots under Cloud Hypervisor killed the VM: CH drops the vfio-user client and
  connects a new one about 0.7 s later, and the device exited on the first disconnect. The
  device now keeps its socket, resets to power-on state for the next client and releases the
  old client's memory and IRQ eventfds; validated in C119 and the gate's Windows leg
  ([windows-findings.md](../research/windows-findings.md), step 23).
- Windows crashes and hangs beside a second GPU client (Steam, Edge), KNOWN BUG #1: WDDM
  migrates a busy channel to a 65536-entry GPFIFO and a new USERD through a deferred
  `NV2080_CTRL_CMD_FIFO_UPDATE_CHANNEL_INFO` (V2 layout), which the device refused. Fixed
  on 2026-09-30: the host RM migrates the host channel in its own stream; 11 of 11 runs
  passed, every migration switched at the old ring's end
  ([windows-findings.md](../research/windows-findings.md), step 23;
  [channel-migration-and-deferred-api](../wiki/channel-migration-and-deferred-api.md)).
- Windows VSync-timeout TDRs: the flip notifier read FINISHED at the KMD's check; the model
  now reports NOT_BEGUN until the latch, BEGUN at the latch and FINISHED when displaced
  ([windows-findings.md](../research/windows-findings.md), step 16).
- Linux SotTR stuck on its loading screen: MMU invalidates held for other roots' shadow
  work; fixed by the scoped hold and the urgent-first worker
  ([sottr-invgate-2026-09-29.md](../research/sottr-invgate-2026-09-29.md)).
