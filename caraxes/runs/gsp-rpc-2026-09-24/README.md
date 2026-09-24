<!-- SPDX-License-Identifier: MIT -->
# GSP plan implementation, runs of 2026-09-24

The runs that verify the implementation of the GSP plan (docs/caraxes/gsp-plan/, "Status"
sections). Device commits: 29f3366 (ALLOC_MEMORY and the 615 event import), caf007d (the
conditional RPCs), 4bacf2a (GMCAPI requests, RC events), 4e34617 (class refusal, TSG id),
fc9b537 (boot exchange B2-B5, B7). Binaries: `target/rpc/release/vfio-user-nvgpu` (610 guest
ABI) and `target/rpc615/release/vfio-user-nvgpu` (`--features v615`) of this checkout;
`setup.txt` of each run names the sha256, the commit and the environment.

Scripts: `prepare-vm3-615.sh` (the vm3-615 overlay on the 615 base image, autoload of
nvidia_modeset, nvidia_drm and nvidia_peermem blocked, nvidia-persistenced masked, as vm2-615),
`seed-vm3-615.sh` (its cloud-init seed: vm3's with a new instance id), `run.sh
emu615|gate610|gate615`. `run.sh` refuses to start while a vm3 process runs and brings the VM
down at its end. A first `emu615` attempt stopped before boot because `seed.img` was missing;
its directory was removed, the seed script added.

## emu615: the pure-emulation 615 boot

vm3-615, boot model and host RM for the forwarded controls only (no reservation, no host
execution). Result: `RmInitAdapter succeeded!` (`guest-dmesg-probe.txt`), `nvidia-smi` shows
"Leandro VFIO RTX 2070-4G", driver 615.71.09 (`guest.txt`). The event import completes
(`device-gsp-plan.txt`, `device-events.txt`): the three `ALLOC_MEMORY` of steps 3a-3c
(header 0x1000, records 0x7000, vardata 0x10000 bytes, one contiguous page list each) are
answered NV_OK and registered, step 5b's `NV_EVENT_BUFFER` (256 records of 104 bytes) and step
7's `ENABLE_EVENTS` (KEEP_OLDEST) answer NV_OK, step 8's bind gets bindId 1; the guest logs no
`goeimport` failure. `GSP_INIT` now carries 560 bytes of static keys (was 424) with the B2 keys;
the client opted in to RPCs and events (key value 3); `RmLogonRC` and `EnableGpuFirmwareLogs`
are honoured. No request is unanswered, no class refused; the refused controls of
`device-trace-issues.txt` are the ones of earlier runs (host NOT_SUPPORTED of inforom and
DRAM-encryption queries, kgr rows). Host: 0 Xid lines.

## gate610: the 610 guest on the GPU

vm3 (610.57.04 guest), host execution with mirror channels, 1 GiB profile at reservation
slot 5, token pool (64) and hybrid reactor, no BAR1 area; vm1, vm2 and the game VM ran
concurrently. nvml gate: fail on `power-number` only ("Instantaneous Power Draw 'N/A'"),
every other check passes (name "Leandro VFIO RTX 2070-1G", 1024 MiB, temperature within the
tolerance of the host). cuda gate (367 s): 10 of 11 scored tests pass (init, context, core,
jit, launch, hostreg, torch/cuDNN, rlprobe mean10=56.9, convburn acc=3.041450977e+00,
robustness); cuda-managed fails with 801 (managed memory is not supported, experiments.md);
cuda-gdb skipped. The same configuration before this work
(`caraxes/runs/cutover-615-2026-09-24/v2-610guest/`) failed nvml on `power-number` and cuda
on cuda-managed and robustness: no regression. No class refusal, no RC event, no request
unanswered by the new code. Host: 0 Xid lines (`host-xid.txt`, `hostwatch.txt`).

## gate615: the 615 guest on the GPU

vm3-615 (615.71.09 guest, the prepared overlay), the same configuration with the v615 device.
The boot completes with the event import (three `ALLOC_MEMORY` answered NV_OK, the bind with
bindId 1). nvml gate: **pass** (all checks; power 30.70 W, P5). cuda gate (48 s): every scored
test fails at `cuInit: 100 no CUDA-capable device is detected`, the last failed RM call
`NV2080_CTRL_CMD_CE_GET_LCE_SHIM_INFO -> 0x56` (29 times in the trace, `device-trace-issues.txt`).
That control is the known 615 cuInit gap of cutover-615.md run 4
(`caraxes/runs/cutover-615-2026-09-24/v2-615guest/`, the same result), handled on another
branch (the `CE_GET_LCE_SHIM_INFO` and `QUERY_ECC_STATUS` work), not part of this branch's base
(1be5ac2). Host: 0 Xid lines.
