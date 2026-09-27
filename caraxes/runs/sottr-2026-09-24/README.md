<!-- SPDX-License-Identifier: MIT -->
# Shadow of the Tomb Raider on Caraxes: the operator's run

The stage is described in [docs/caraxes/sottr-stage.md](../../../docs/caraxes/sottr-stage.md).
This folder holds the scripts and, per run, one `session-<time>/` folder with the evidence.

## Before you start

- **vm0 and vm2 must be down.** vm4 takes a 3 GiB reservation at slot 0, the top 3 GiB of the
  host framebuffer. vm0 (2 GiB) and vm2 (2 GiB) have their slots there. `up.sh` refuses while
  vm0, vm2 or vm2-615 runs. vm1 (1 GiB, slot 4) may stay up. The host then keeps
  8192 - 3072 - 1024 (vm1) = 4096 MiB minus its own use, above the 1 GiB floor (D11).
- The launcher of the main checkout needs the `CARAXES_GAMES_DISK` change of this branch
  (`up.sh` checks it).
- vm4 uses the tap `caraxes3` (192.168.103.1 on the host, guest 192.168.103.24, MAC
  02:00:00:00:00:24). vm3 uses `caraxes3` too, so vm3 must be down (`up.sh` checks that the
  tap is free).
- The first start builds the guest driver in vm4 (about 10 minutes, once).
- Steam needs the internet. `up.sh` adds a NAT rule for 192.168.103.0/24 (MASQUERADE out of
  the uplink plus two FORWARD accepts, as v1's `rig.sh net up`); `down.sh` removes them.
  `SOTTR_NAT=0 up.sh` keeps the guest offline.

## Commands

From any directory, in bash:

```
caraxes/runs/sottr-2026-09-24/up.sh       # vm4 up, prepared, GNOME + Steam + Sunshine
caraxes/runs/sottr-2026-09-24/stream.sh   # pair if needed, open the Moonlight window
caraxes/runs/sottr-2026-09-24/down.sh     # Moonlight quit, logs, VM down, NAT removed
```

`stream.sh [WxH] [fps] [kbps]` defaults to 1920x1080, 60 and 20000, as in the display stream
stage. Pairing runs by itself: `moonlight pair 192.168.103.24 --pin 4321`, and the PIN goes to
Sunshine's `/api/pin` with the web login. The web UI is `https://192.168.103.24:47990`, login
`lea` / `leastream`, the same values as in v1's `config.sh`. A Moonlight that is already paired
is not paired again.

## What to click

1. The Moonlight window shows the GNOME desktop of vm4. The Steam installer dialog appears
   ("Steam is proprietary... Install / Cancel"). Its default is **Cancel**, so click **Install**.
   Steam downloads its client and updates, then shows the login window.
2. Log in to Steam. Your account and login state stay in the guest, in
   `/home/leandro/.steam` on vm4's overlay.
3. Library: Shadow of the Tomb Raider should show as installed. The library folder
   `/games/SteamLibrary` is registered in advance. If the game shows as not installed, go to
   Steam > Settings > Storage > Add Drive, choose `/games/SteamLibrary`, and the game appears.
   Do not let Steam "install" the game again: the download would go into vm4's games overlay.
4. The game is the native Feral Linux port (`ShadowOfTheTombRaider.sh`, Vulkan), so
   Properties > Compatibility must stay **off** (no Proton). The library also has Proton and the
   Steam Linux Runtimes (soldier, sniper, 4) from v1. The native game does not need them.
5. Start the game. Under Options > Display choose the resolution, then Graphics > Run
   Benchmark:
   - 1280x720 (fullscreen), preset as in v1's sottr-4q run, one benchmark pass;
   - 1920x1080, the same preset, one benchmark pass.
   Write down, or photograph, the benchmark's result screen: average FPS, min/max, GPU- and
   CPU-bound share. The result files of the Feral port are in the guest under
   `~/.local/share/feral-interactive/Shadow of the Tomb Raider/`.
6. Close the game and Steam (or leave them open), then run `down.sh`.

## What gets recorded (`session-<time>/`)

| File | What |
|---|---|
| `setup.txt` | host driver, sha256 of the device binary and Cloud Hypervisor, git state, all `CARAXES_*` settings |
| `hostwatch.txt` | every 5 s: host `nvidia-smi` time, memory used, host kernel log lines, Xid lines; the device gets SIGTERM if host nvidia-smi takes more than 3 s |
| `host-1hz.csv` | 1 Hz: card memory, utilisation, clocks, temperature, power, encoder/decoder use, host RAM available, the device process's VRAM, device and Cloud Hypervisor CPU seconds (v1's sottr-4q columns) |
| `up.txt`, `device-start.txt`, `prepare.txt`, `probe.txt`, `display-up.txt`, `gnome.txt`, `sunshine-start.txt`, `status.txt` | the start, step by step |
| `pair.log`, `pin-*.txt`, `moonlight-list.txt`, `moonlight.log`, `moonlight.pid`, `stream.txt` | pairing and the Moonlight client; `moonlight.log` ends with Moonlight's session summary (frame rates, host latency, drops) when the stream is quit |
| `sunshine.log`, `sunshine-stop.txt` | Sunshine's log from the guest (encoder, capture) |
| `status-end.txt`, `guest-dmesg.txt`, `guest-journal.txt` | the guest at the end: processes, games disk, kernel log, Steam, gnome-shell and Sunshine journal lines |
| `device-summary.txt`, `device-log-issues.txt`, `device-log-tail.txt` | the device's summary lines (interrupt page, display page, doorbell poll), refused or unanswered calls |
| `host-xid.txt` | host kernel Xid lines since `up.sh` |

The benchmark numbers and Moonlight's on-screen statistics (Ctrl+Alt+Shift+S in the stream)
are yours to note down. Copy them into a `result.md` in the session folder.

`plumbing/` is the test of this set-up in pure emulation, without the host GPU
(`SOTTR_EMULATION=1`), recorded on 2026-09-24.

## The 615 game VM (vm4-615, from 2026-09-25)

The same stage with the 615.71.09 driver as Ubuntu installs it (docs/caraxes/sottr-stage.md
section 8): `up-615.sh`, `stream-615.sh`, `down-615.sh`. Always pair `up-615.sh` with
`down-615.sh`: plain `down.sh` targets vm4. vm4 and vm4-615 share the tap and the reservation,
so only one runs. The first start installs the driver packages with apt through the NAT (a few
minutes, once). Steam needs a login again (a new overlay). Sunshine tries NVENC first and falls
back to libx264; `sunshine-encoder.txt` in the session folder shows which encoder it took.
Evidence goes to `session-615-<time>/`.

`CARAXES_GUEST_DMA_GPA=1 up-615.sh` selects the helper-free stock-driver path. Preparation
migrates an existing overlay's display-module rules on every start, so an old `modprobe615.done`
marker does not retain the `caraxes_dma_range` guards. Without the flag, the legacy helper and
probe path remains unchanged.
