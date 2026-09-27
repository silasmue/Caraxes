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

## Next diagnostic gate

1. Stop at NVIDIA module load and record its address range.
2. Break at `dxgkrnl!DxgkInitialize` and inspect the driver initialization table.
3. Verify that the StartDevice callback belongs to NVIDIA before breaking there.
4. Record its return status and failure path, then compare with historical B4.

Avoid global allocator breakpoints over the serial transport. PnP code 0 is only
a startup result: acceptance also requires NVIDIA hardware device creation,
verified rendered output and a usable desktop. See the
[Windows evidence](../../../docs/research/windows-2026-09-27.md).
