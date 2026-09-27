<!-- SPDX-License-Identifier: MIT -->
# Windows kernel debugger checkpoint

Purpose: locate the stock NVIDIA Windows driver's startup failure. The target uses
host GPU execution; the debugger VM has no NVIDIA device. No Windows rendering
success has been established.

## Saved setup

- State: `/mnt/vmstore/caraxes/alpha-2026-09-27/windows-kd/`.
- Both Windows disks are separate qcow2 children of the stopped
  `alpha-2026-09-27/windows/win11.qcow2`. Keep that backing image frozen.
- Debugger: 4 GiB RAM, two vCPUs, Microsoft KD 10.0.28000.2705, QGA control.
- Target: 4 GiB RAM, four vCPUs, 1 GiB reserved VRAM in slot 0, host-backed Caraxes.
- The debugger QEMU listens for the target serial connection on
  `127.0.0.1:4445`. Target BCD enables kernel debugging on COM1 at 115200 baud;
  EMS is disabled to avoid sharing that port.
- The host NVIDIA driver remains bound. Do not launch another slot-0 reservation
  while the target backend runs.

## Console bridge

Copy `kd-bridge.ps1`, `kd-log.ps1` and `nv-start-failures.kds` to `C:\Temp` in the debugger VM.
The existing `CaraxesKDBridge` scheduled task starts the bridge. It launches KD
with redirected input/output and records output in `C:\Temp\kd-console.log`.
Commands are queued as text files under `C:\Temp\kd-cmd`.

- Read logs with `kd-log.ps1 -Lines 80`; use `-Snapshot C:\Temp\kd-snapshot.log`
  before copying a transcript through QGA. The reader and persistent writer share
  file access. The earlier `Add-Content` loop could fail during concurrent reads
  and terminate KD. Thirty reads plus a snapshot passed with KD still running.
- Ordinary lines are KD commands. A consumed file only proves delivery to stdin;
  confirm execution in the KD log.
- `@BREAK` generates a Windows console break event. KD must inherit the bridge's
  private console (`CreateNoWindow=false`). The earlier no-console launch could
  log delivery without interrupting KD.
- `g` resumes the target. While the target is halted, its QGA does not respond;
  control KD through the separate debugger VM's QGA.
- Startup arms the NVIDIA module-load filter and stays stopped. It does not run
  a global symbol reload or automatically resume. Load needed symbols explicitly.
- A live reconnect, command execution, resume and subsequent break have been
  observed. Initial boot attachment alone did not reliably stop the target.

The local host wrappers are `windows-kd/kd-command.py`, `kd-log.sh` and
`run-debuggee.py`. They reference this machine's artifact paths. Start/stop target
runs through `python3 run-debuggee.py`; do not reuse a run name.

## Captured failure: KD3

- NVIDIA module load was caught before driver initialization.
- On Windows build 26100, the public symbol `dxgkrnl!DxgkInitialize` did not resolve.
  `dxgkrnl!DpiDxgkDdiStartDevice` exposes the callback invocation instead.
- Disassembly of this build places the callback dispatch at wrapper `+0x7b` and
  its return at `+0x80`. Verify instructions again for other builds.
- At dispatch, `RAX` is the callback. Filter against the loaded NVIDIA module
  bounds: Microsoft Basic Display callbacks also reach this wrapper.
- NVIDIA callback `nvlddmkm+0x16f21d0` returned `0xC000009A`
  (`STATUS_INSUFFICIENT_RESOURCES`). Both output counts remained zero.
- This confirms the earlier ETW status directly; it does not prove actual RAM or
  VRAM exhaustion. No whitelist failure has been established.
- The callback's indirect wrappers lead to `nvlddmkm+0x192c450`, which propagates
  the return from `+0x1854890`. That function tail-calls `+0x1865570`.
  Internal offsets apply only to the tested NVIDIA 617.14 binary.
- A guest PnP restart hit `+0x186628b`: `+0x185a0a0` returned false, which
  this branch converts into `0xC000009A`. The other literal resource-error
  branches in this function were not the observed origin.
- A second restart hit `+0x185a0c7`, identifying the first helper,
  `+0x18586b0`, as the source of that false result. Its six fatal helper-return
  checks are `+0x18586d8`, `+0x1858720`, `+0x1858845`, `+0x18588df`,
  `+0x1858905` and `+0x1858bd4`. Conditional breakpoints stop only for `AL=0`.
- The third restart stopped at `+0x18588df`, after `+0x185b6f0` returned false.
  The fourth stopped at `+0x185baa4`: its first `+0xdda3d0` call failed for
  index 0. Continuing reached `+0x185bd42` with `SIL=0`, confirming that this
  failure remained fatal.
- PnP restart unloads and reloads NVIDIA at a different address. Catch the new
  module load, resolve its base and re-arm breakpoints before continuing.
  No new host Xid was observed during these restarts.

Raw transcripts: `windows-kd/debugger/KD3-callback-return.log` and
`KD3-internal-start-full.log` under the state directory. `KD3-status-origin.log`
captures the first internal failure branch; `KD3-first-stage.log` includes the
next helper's disassembly. `KD3-init-helper.log` captures the third restart and
the failing routine's disassembly. `KD3-internal-start.log` is an incomplete snapshot
from before the console logging fix. Save a final transcript before stopping KD.

## Fixed-address allocation: controlled diagnostic

The fourth restart captured an `NVOS32_PARAMETERS` allocation inside the guest:

| Field | Value |
| --- | --- |
| Function | `2`, `NVOS32_FUNCTION_ALLOC_SIZE` |
| Status | `0x51`, `NV_ERR_NO_MEMORY` |
| Owner | `0xff080002` |
| Size | `0x38c7f000`, about 908.5 MiB |
| Flags | `0x8010`, fixed address and map not required |
| Requested offset | `0` |
| Usable static FB region | `[0x1a9800000, 0x1e9600000)` |

- Windows segment 2 also has base 0 and size `0x38c7f000`. The accessor
  `+0x1963d00` reads that base from the segment object at `+0x238 + index*0x58`.
- This is a **guest RM** allocation. No NVOS32 ioctl is forwarded to the host.
  The backend trace ends with a successful virtual-memory allocation before
  the failed operation; no failed memory-allocation RPC was observed.
- In the fifth restart, stop before the RM allocation dispatch. Change only the
  copied `NV_MEMORY_ALLOCATION_PARAMS.offset` from 0 to `0x1a9800000`, then stop
  immediately at the caller's return (`+0x156e10`). The allocation returns
  `NV_OK`, with size rounded to `0x38c80000`, alignment `0x10000`, and the
  requested high offset preserved. That range remains inside the reservation.
- This establishes the address mismatch for this allocation. It does not
  establish successful `StartDevice`, desktop rendering, or an unmodified guest
  result. No register or instruction was changed to force a successful status.
- `FB_GET_INFO_V2` base handling also deserves a consistency review, but the
  recorded request in this run contains **none** of base indices `0x11`, `0x13`
  or `0x1e`. Changing those replies alone is not an evidenced fix.

Evidence: `KD3-allocation-failure.log`, `KD3-rm-dispatch.log`, and
`KD3-fixed-offset-ab.log` under `windows-kd/debugger/`. The final transcript marks
the guest-memory edit as `DIAGNOSTIC_ONLY_REBASE_ONE_ALLOCATION`. Continuing the diagnostic run still returned `0xC000009A` from the internal
startup routine. KD3 was then stopped; `KD3-final.log` captures that result.
The single successful allocation did not resolve startup. KD4 boots the stock
driver again without the KD3 request edit.

## Continuing the diagnosis

1. Reconcile Windows segment coordinates with the emulated FB region coordinates.
2. Prefer a consistent guest-visible layout; audit BAR roots, page-table leaves,
   physical copy commands and CPU FB access together before changing the mapping.
3. Reboot into an unmodified guest and reproduce the allocation without KD edits.
4. Require NVIDIA hardware device creation, verified rendered output and a usable
   desktop before treating Windows as working.

Keep the existing identity FB mapping while tracing segment construction. Direct
page-directory adoption, USERD/context pointers and physical-address controls
depend on it; changing only the advertised regions to zero would be incomplete.

The public TU102 `memmgrCalculateHeapOffsetWithGSP` routine is a useful comparison:
it derives the heap start from leading reserved regions only when console or CBC
memory is present. Otherwise it falls back to the heap object's base. Caraxes
still reports `disable_compbit_backing=true`; `CARAXES_WIN_COMPBIT=1` only changes
the separate compbit-store-info reply. Windows console-reservation state is not
yet known. Adding a large reserved prefix is therefore not an established fix:
it also changes reserved-memory accounting and may request an internal allocation
outside the owned VRAM. Do not relax the existing ownership bounds to test it.

Send KD commands on separate lines. Path commands can interpret semicolons as
path separators. Avoid global allocator breakpoints and broad symbol reloads over
serial; use the local symbol cache and matching debugger-VM system images.
See the [Windows evidence](../../../docs/research/windows-2026-09-27.md).


## Native FB-start override: KD4

- Stock 617.14 contains a DWORD setting named `OverrideFbStartKB`.
  Its reader is `nvlddmkm+0x3ce8bc`; the registry-helper return is `+0x3ce8d8`.
  A successful nonzero lookup stores the value at kernel-memory-system `+0x478`.
- KD4 reached this reader during startup. No registry override has been applied.
  The helper reads an internal cache; its exact Windows registry source still
  needs verification. A string in the binary alone does not establish that path.
- The reservation base corresponds to `0x6a6000` KiB; the advertised 1 GiB size
  is only `0x100000` KiB. Public RM consumers subtract the override from memory
  sizes and reserve a prefix in PMA. Applying the start alone risks underflow or
  inconsistent accounting. It is not a validated workaround.
- Continue tracing segment construction and size accounting before changing this
  setting. Preserve reservation ownership and the identity FB mapping.
