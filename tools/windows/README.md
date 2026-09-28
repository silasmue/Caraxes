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
| `d3d12-probe.ps1` | D3D12 device, 100 fence-signal round trips, a compute dispatch of 256 words read back |
| `cuda-probe.ps1` | deviceQuery, PTX JIT vector add (1 Mi floats), launch cost, pinned copy bandwidth |
| `media-probe.ps1` | NVENC H.264/HEVC 1080p60 encode, NVDEC with `-hwaccel cuda` and `*_cuvid`, frame counts, fps; `-Fetch` downloads ffmpeg (BtbN win64 GPL build) |
| `workload.ps1` | the Linux/native compute comparison: `-Stage install` (llama.cpp b10173 CUDA 13.3, Python 3.12.10, torch 2.13.0+cu130, the models, sha256-checked from the host's loopback server at 10.0.2.2:8765), `info`, `llama -Model 1b\|7b [-Ngl n]`, `torch -Tool transformer\|convnet` |
| `quiet.ps1` | Windows Update state (build, hotfixes, pending servicing); `-Apply` pauses quality and feature updates for `-Days` (7) and takes no drivers from Windows Update, so the stock 617.14 stays |
| `steam-3dmark.ps1` | `-Install` Steam silently, `-Prefer` NVIDIA GPU for 3DMark, `-Collect` result scores |

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
