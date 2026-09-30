<!-- SPDX-License-Identifier: MIT -->
# Guest support

One synthetic device and one binary serve four different driver stacks, the stock ones
unmodified. Every row ran on the same synthetic TU106 (RTX 2070) and the same device binary in
the [four-guest gate of 2026-09-30][G], unless a cell names another record. Numbers are in
[performance](../performance.md); how the device tells the drivers apart is in
[guest drivers and ABIs](../wiki/guest-drivers-and-abis.md).

| Guest driver | Probe, bind | Unbind, rebind | Display, desktop | Vulkan, GL, D3D | CUDA | NVENC, NVDEC | Streaming | Reboot, reset |
|---|---|---|---|---|---|---|---|---|
| NVIDIA open 615.71.09 (Linux) | pass ([gate][G]) | not tested | GNOME on Wayland ([gate][G]); display gate 11 of 11 ([display-e][DG]) | Vulkan and GL on Wayland and X11 ([gate][G]); SotTR frame fault open ([issue 2][KI]) | cuda gate 10 of 12 ([testing][T]); managed memory and cuda-gdb open ([issue 6][KI]); PyTorch, llama.cpp ([workloads][W]) | encode pass; H.264, HEVC, VP9 decode bit-exact ([codecs][C]) | Sunshine to Moonlight over HEVC ([gate][G]) | not tested |
| NVIDIA 617.14 (Windows 11) | pass ([gate][G]) | not tested | WDDM desktop on the device's scanout ([gate][G]) | D3D11 stages 12 of 12, Heaven, Superposition ([gate][G]); DX12 3DMark fails ([issue 1][KI]) | llama.cpp, PyTorch ([findings][WF] P29 to P31); CUDA in the codec probes ([codecs][C]) | encode runs; H.264, HEVC, VP9 decode bit-exact ([codecs][C]) | Sunshine with NVENC or x264 ([codecs][C]); NVENC latency open ([issue 3][KI]) | guest reboot pass ([gate][G]); TDR recovery after an FLR ([findings][WF] step 13) |
| nova-core (Linux 7.2.7, GSP-RM 570.144) | 5 probes pass ([nova log][N]) | 3 unbind and bind cycles, a module reload ([nova log][N]) | n/a | n/a | n/a | n/a | n/a | not tested |
| nouveau (r570, GSP-RM 570.144) with Mesa NVK | pass ([nouveau log][V]) | unbind pass ([nouveau log][V]); rebind not tested | headless Weston ([nouveau log][V]); no scanout | Vulkan, GL through zink, glmark2 ([nouveau log][V]); vkmark cube loses the device ([issue 8][KI]) | n/a | not tested | not tested | not tested |

- n/a: the driver has no such part. nova-core boots the GSP and stops; it has no channel,
  display or video code. nouveau has no CUDA.
- No scanout for nouveau: the device reports the display fused off, so nouveau drops its
  display before any display call ([open-render][OR]).

[G]: ../../caraxes/runs/gate-2026-09-30/stack/RESULT.md
[N]: ../../caraxes/runs/gate-2026-09-30/stack/nova/guest-nova.txt
[V]: ../../caraxes/runs/gate-2026-09-30/stack/vm10/guest.txt
[C]: ../../caraxes/runs/codec-2026-09-30/matrix.md
[DG]: display-e.md
[T]: testing.md
[W]: workload-gate.md
[KI]: known-issues.md#open
[WF]: ../research/windows-findings.md
[OR]: ../research/open-render-2026-09-29.md
