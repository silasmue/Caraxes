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

Copy `kd-bridge.ps1` and `nv-start-failures.kds` to `C:\Temp` in the debugger VM.
The existing `CaraxesKDBridge` scheduled task starts the bridge. It launches KD
with redirected input/output and records output in `C:\Temp\kd-console.log`.
Commands are queued as text files under `C:\Temp\kd-cmd`.

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

Raw transcripts: `windows-kd/debugger/KD3-callback-return.log` and
`KD3-internal-start.log` under the state directory. The latter is an intermediate
snapshot; keep the final transcript with the run before stopping the debugger.

## Continuing the diagnosis

1. Resolve the actual NVIDIA callback and arm a thread-scoped return breakpoint.
2. Trace the internal call that produces `0xC000009A`; compare with historical B4.
3. Change emulator behavior only when the trace identifies a concrete mismatch.
4. Require NVIDIA hardware device creation, verified rendered output and a usable
   desktop before treating Windows as working.

Send KD commands on separate lines. Path commands can interpret semicolons as
path separators. Avoid global allocator breakpoints and broad symbol reloads over
serial; use the local symbol cache and matching debugger-VM system images.
See the [Windows evidence](../../../docs/research/windows-2026-09-27.md).
