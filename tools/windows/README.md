<!-- SPDX-License-Identifier: MIT -->
# Windows test pack

PowerShell scripts for the Windows 11 guest on the synthetic GPU (stock driver 617.14). They run as
SYSTEM through the QEMU guest agent; stock Windows and driver components only (PowerShell 5.1, C# 5).

## What the image has (2026-09-28, read-only inspection of windows-plain/win11.qcow2)

- Windows 11 Pro 25H2 (build 26200), German UI, 46 GB free on C:, RDP enabled (QEMU forwards
  127.0.0.1:3389).
- In System32: nvidia-smi.exe, nvcuda.dll, nvEncodeAPI64.dll, nvcuvid.dll, nvml.dll, vulkan-1.dll,
  vulkaninfo.exe, D3D12, dxdiag; winget (DesktopAppInstaller); d3dcompiler_47 (stock Windows).
- Not there: Steam, 3DMark, ffmpeg, a CUDA toolkit.
- Network: QEMU user networking (SLIRP) gives the guest outbound NAT; no host iptables rule needed.

## Scripts

| Script | Checks |
|---|---|
| `smoke.ps1` | Device Manager name, status and problem code; registry adapter strings; `nvidia-smi -L`, `--query-gpu`, `-q`; `vulkaninfo --summary`; the D3D11 and D3D12 gates; TDR/nvlddmkm events |
| `d3d11-render.ps1` | D3D11 triangle, clear, triangle, each read back and verified |
| `d3d11-benchmark.ps1` | Bounded offscreen fill, ALU, depth and memory cases; GPU timestamp/disjoint queries, separate CPU encode/Flush/wait times, JSONL and readback validation. `-ValidateOnly` compiles and checks helpers without opening a GPU; `-DispatchMiB 1\|16` controls memory dispatch granularity. |
| `d3d12-probe.ps1` | D3D12 device, 100 fence-signal round trips, a compute dispatch of 256 words read back |
| `cuda-probe.ps1` | deviceQuery, PTX JIT vector add (1 Mi floats), launch cost, pinned copy bandwidth |
| `nvenc-probe.ps1` | NVENC through the NVENC API (no ffmpeg), the twin of caraxes/tools/nvenc-probe.c: P1 ULL CBR, the same pattern and ENC line; `-Mode lock` (driver input buffers filled by the CPU, as on Linux) or `-Mode vram` (8 frames in CUDA memory, registered and mapped: the encoder alone) |
| `media-probe.ps1` | NVENC H.264/HEVC 1080p60 encode, NVDEC with `-hwaccel cuda` and `*_cuvid`, frame counts, fps; `-Fetch` downloads ffmpeg (BtbN win64 GPL build) |
| `workload.ps1` | the Linux/native compute comparison: `-Stage install` (llama.cpp b10173 CUDA 13.3, Python 3.12.10, torch 2.13.0+cu130, the models, sha256-checked from the host's loopback server at 10.0.2.2:8765), `info`, `llama -Model 1b\|7b [-Ngl n]`, `torch -Tool transformer\|convnet` |
| `quiet.ps1` | Windows Update state (build, hotfixes, pending servicing); `-Apply` pauses quality and feature updates for `-Days` (7) and takes no drivers from Windows Update, so the stock 617.14 stays |
| `present-probe.ps1` | D3D11 flip-model swap chain on a window: windowed, exclusive fullscreen, windowed; fps and frame statistics per phase; `-Seconds N` windowed only for N s (signed-in user's session: CH-build `run44.py upack`) |
| `soak-media.ps1` | the display soak's media load (signed-in session): Edge plays local 1080p60 H.264 and VP9 clips in loops with a dropped-frame overlay, a YouTube video, a WebGL page on ANGLE's Vulkan backend; logs the GPU engines' utilization and nvidia-smi's decoder use; decode seen and no TDR = PASS |
| `cuda-loop.ps1` | cuda-probe.ps1 round after round for `-Minutes` (the soak's CUDA load) |
| `dxgk-etw.ps1` | `-Action start` a DxgKrnl ETW session (all keywords) into C:\etw\dxgk.etl, `-Action stop` it and summarise the events (names by count, TDR/timeout/reset events) from tracerpt's CSV |
| `process-trace.ps1` | WMI process starts, stops and unsigned exit status; exit-37 self-test before readiness; bounded recording with a stop file |
| `display-mode.ps1` | the modes EnumDisplaySettings lists, a change to `-Width`x`-Height` and back (signed-in user's session) |
| `steam-3dmark.ps1` | `-Install` Steam silently, `-Prefer` NVIDIA GPU for 3DMark, `-Collect` result scores |
| `3dmark-launch.ps1` | Launch installed Steam app 223850 in the signed-in desktop and wait for its 3DMark window; reuse an existing process |
| `showcase.ps1` | the screenshots of docs/vfio/showcase.md: Device Manager, the GPU's driver tab, Task Manager's GPU page, dxdiag's display tab; elevated in the signed-in session (caraxes/runs/win-2026-09-30/showcase-hi.sh) |
| `display-name.ps1` | report the single present PCI NVIDIA RTX 2070; `-Apply` sets its Device Manager FriendlyName to `Leandro VFIO RTX 2070`, with a JSON backup; optional `-DriverDescription` also tests its DriverDesc; `-Restore` restores original values and absence |

Run `display-name.ps1` without switches to report names. Elevated `display-name.ps1 -Apply`
changes FriendlyName through SetupAPI; `-Apply -DriverDescription` additionally changes only
the selected device's software-key `DriverDesc`. The backup defaults to
`C:\ProgramData\Leandro\display-name-backup.json` (`-BackupPath` overrides it); repeated applies
preserve the original FriendlyName. `-Restore` is repeatable and refuses a different device
instance or software key. It restores both tracked names, so to keep FriendlyName after an
unsuccessful DriverDesc experiment, run `-Restore` and then `-Apply` without `-DriverDescription`.
Replacement uses a same-directory `.previous.json` recovery copy, removed only after the new
backup validates; if retained after failure, inspect it before another apply.
The script does not restart the GPU. On the measured Windows 11 / stock 617.14 boot, FriendlyName
alone showed `Leandro VFIO RTX 2070` in fresh Device Manager and Task Manager windows. DXGI remains
unverified. Driver reinstall or device recreation can replace these guest names.

## 3DMark startup

Run the launcher in the signed-in desktop, with a wrapper longer than its
120-second default wait:

```sh
PACK_WAIT_S=180 python caraxes/runs/win11-2026-09-25/run44.py upack 3dmark-launch.ps1
```

Require task exit 0 and the `READY` line, then confirm the requested UI page.
Window presence alone does not establish that its controls have loaded. `-ReadySeconds`
accepts 30–180 seconds. The script checks the current session every 500 ms,
uses a monotonic deadline and requires a window titled exactly `3DMark`.
An existing current-session process is reused. SYSTEM/session 0, a missing
Steam executable and a Steam error window fail explicitly.

## Process exit tracing

Run `process-trace.ps1` as LocalSystem in a dedicated PowerShell 5.1 process during
an owned diagnostic session. Use a fresh output directory and allow the task
wrapper more time than the collector's duration:

```sh
PACK_WAIT_S=660 python caraxes/runs/win11-2026-09-25/run44.py pack process-trace.ps1 \
  -OutputDirectory 'C:\Users\Public\caraxes\process-trace-01' -DurationSeconds 600
```

- Wait for `ready.json`. It proves both WMI subscriptions observed an owned
  `cmd.exe` start and stop with exit status 37, matching its direct exit code.
- `process-events.jsonl` records process names, PIDs, parent PIDs, session IDs,
  event and observation times, and unsigned exit status in decimal and hex.
  Live command-line lookup for 3DMark is best effort; absence is inconclusive.
- Create the output directory's `stop` file to finish early. Accept cleanup only
  with task exit 0, `finished.json` reason `stop_marker` or `deadline`, and empty
  `cleanup_errors`. A VM shutdown is not a clean collector stop.
- Stop names may be truncated and parent PIDs may be zero. Match compatible names
  and the corresponding start/stop instance; PIDs can be reused within one trace.
  WMI event times are provider creation
  times, not exact process birth or exit times; callback order can differ.
- Preserve raw files locally: process metadata can contain private machine data.
  These traces diagnose completion; runs with tracing do not establish a speedup.

## Order (stop at the first failure)

1. `run18.py probe start` / `probe status`: the D3D11 gate alone (P18 acceptance).
2. `run18.py pack smoke.ps1`, then `run18.py pack smoke.ps1 status`.
3. `run18.py pack cuda-probe.ps1`.
4. `run18.py pack media-probe.ps1 -Fetch` (first time; later without `-Fetch`).
5. `run18.py pack steam-3dmark.ps1 -Install`; the user logs in to Steam at the console and installs
   3DMark (or 3DMark Demo, 231350); `-Prefer`; the user runs Night Raid or Fire Strike first, then
   Time Spy; `-Collect`.

- Every script prints one `PASS`/`FAIL` line per check and exits non-zero on a failure.
- Outputs stay in `C:\caraxes` and `C:\Windows\Temp\caraxes-*-out.txt`; copy them into the run folder
  with `run18.py get`.
- Before the 3DMark step: Time Spy renders at 2560x1440 and the guest has 1 GiB of VRAM
  (`WIN_GPU_FB_MIB=1024`); raise it when the host has room.
