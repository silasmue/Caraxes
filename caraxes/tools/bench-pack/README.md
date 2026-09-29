<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Render benchmark pack

A prepared, offline-ready set of GPU render benchmarks for the Caraxes synthetic RTX 2070
(TU106) device: Blender Open Data's benchmark-launcher-cli, and Unigine's Heaven 4.0 and
Superposition 1.1 (free Basic editions). Targets: the Linux guest (Ubuntu 24.04, GNOME,
NVIDIA 615) and the Windows 11 guest (NVIDIA 617), with a native-host run as the reference
point. This pack only *prepares and documents* the benchmarks; nothing here starts a VM or
touches the host GPU by itself (`run-native.sh` is the one script that does — see its header).

All downloads are anonymous, plain HTTPS GETs: no account, no login, no telemetry opt-in, no
identifying header. Nothing here ever sends the operator's e-mail, name or hostname anywhere.

## Contents

| Path | What |
|---|---|
| `fetch-and-seed.sh` | Downloads everything below, verifies checksums, pre-extracts the Blender launcher's cache. Idempotent. |
| `lib-common.sh` | Shared result-line writer and log parsers for `run-linux.sh` / `run-native.sh`. |
| `run-linux.sh` | Host-side: drives the Linux guest over SSH. |
| `run-windows.ps1` | Guest-side: runs inside the Windows 11 guest's signed-in session. |
| `run-native.sh` | Host-side: the same benchmarks run natively, as the reference number. Touches the GPU for real — see its header. |

The downloaded payload itself (~4.5 GB) lives outside git, under `$BENCH_PACK_DIR`
(default `/mnt/vmstore/caraxes/bench-pack/`, see `fetch-and-seed.sh`):

```
bench-pack/
  downloads/           the archives fetch-and-seed.sh fetched (see CHECKSUMS.sha256)
  cache-seed/linux/    pre-extracted Blender launcher cache for the Linux guest
  cache-seed/windows/  same, for the Windows guest
  logs/                fetch-and-seed.sh's own logs
  CHECKSUMS.sha256
```

## 1. Blender Open Data benchmark-launcher-cli

**What it measures:** Cycles path-tracing render throughput (samples/minute) on three
official reference scenes — monster (1024x1024, 256 spp), junkshop (2000x1000, 240 spp) and
classroom (1920x1080, 300 spp) — the same scenes opendata.blender.org's public leaderboard
uses, so results are directly comparable to published device pages.

**What's downloaded** (all fetched 2026-09-29, URLs/checksums from the official
`https://opendata.blender.org/benchmarks/metadata/` endpoint — a small, anonymous JSON GET,
same one the CLI itself performs on every run):

| File | Size | sha256 |
|---|---|---|
| `benchmark-launcher-cli-3.3.0-linux.tar.gz` | 7.8 MB | see note below |
| `benchmark-launcher-cli-3.3.0-windows.zip` | 7.8 MB | see note below |
| `blender-5.2.0-linux-x64.tar.xz` | 367 MB | `96f6c181a30f4950607839dc84d42a354b250d8a0231b098b59b7bc69c351c48` (matches metadata) |
| `blender-5.2.0-windows-x64.zip` | 387 MB | `2d184b626c001692c362291911293b6a297179d618d95e9e9192c3a80318adc4` (matches metadata) |
| `blender-benchmark-script-3.1.2.tar.gz` | 17 KB | `2881a9f160a24ccd4282e43b861bd727e33562a4a230401343b590d10e890272` (matches metadata) |
| `monster.tar.bz2` | 73 MB | `8ff812a02f720433de1403dab447be9541eecca71cc919224a7790d017c1b2ce` (matches metadata) |
| `junkshop.tar.bz2` | 337 MB | `f5515a21a1337b908212d8d76733530df83014464a24c585ab7b96000da0cdce` (matches metadata) |
| `classroom.tar.bz2` | 66 MB | `936f0a528f03eba291c67ea87743a0ebde58938fb87471b262721b89b46b875d` (matches metadata) |

**Checksum note (needs no decision, just disclosure):** the metadata endpoint's `checksum`
field for the *launcher* archives themselves (`c52cd088…` linux, `9d01636a…` windows) does
**not** match what `download.blender.org` actually serves at that URL — confirmed by fetching
the metadata twice and re-downloading; both agree with each other and disagree with the file.
Reading the CLI's own Go source (`internal/rpc/download.go`) shows why this is harmless: the
launcher checksum-verifies Blender, the benchmark script and every scene against the metadata
(and refuses/deletes them on mismatch) but never self-verifies its own archive — that
`checksum` field is only used for a client-side "a newer launcher exists" notice. The extracted
binary runs (`--help`, `--version` → `3.3.0`) and matches the CLI documented in the project's
own `launcher/internal/cli/*.go`. Per the "otherwise record the sha256 you computed" rule, the
values we actually verified are in `CHECKSUMS.sha256`:
`a15e062179e7bf5cd20d58081f3c84cf87d56df3f596ffb0a11383638139324a` (linux),
`3f00bb68915800f14ddad6877083544aa25c56aac252b77b161bf7962ee67dde` (windows).

**License/terms:** Blender itself is GPL. The launcher and benchmark scenes are provided by
Blender Open Data for running and (optionally) submitting benchmark results; running locally
without `--submit`/`authenticate` needs no account (opendata.blender.org, About page).

**Known offline limitation:** every `benchmark-launcher-cli` subcommand — `blender list`,
`scenes list`, `devices`, `benchmark` — calls `GetMetaData()` first, which does one HTTPS GET
to `https://opendata.blender.org/benchmarks/metadata/` (~56 KB, anonymous, no auth). This is
not gated by cache state; there is no fully air-gapped mode in the official tool (confirmed
both by reading the source and by the project's own devtalk thread on this exact complaint:
<https://devtalk.blender.org/t/blender-benchmark-cli-wont-run-if-no-access-to-internet-but-all-the-data-is-in-the-right-place/11722>).
What *is* fully cached is the heavy part: `fetch-and-seed.sh` pre-extracts Blender, the
benchmark script and the three scenes into the exact on-disk layout the launcher expects
(content-addressed by sha256, `valid.launcher` flag files and all — verified end to end by
actually running `scenes list` and `devices` against a seeded cache: zero bytes downloaded,
only the one metadata GET). If a guest has outbound HTTPS to opendata.blender.org this is a
non-issue. If a guest is fully air-gapped, skip the launcher and call Blender directly — this
is the exact command line the launcher itself runs internally (from `core/benchmark.go`):

```
blender --background --factory-startup -noaudio --debug-cycles --enable-autoexec \
  --engine CYCLES <cache>/scenes/<sha>/monster/main.blend \
  --python <cache>/benchmark-scripts/<sha>/blender_benchmark_script-3.1.2/main.py \
  --log render -- --device-type CUDA
```
It prints a `Benchmark JSON Data: {...}` line with `samples_per_minute` — same numbers, no
network at all, just no launcher-side JSON aggregation across scenes/devices.

**Commands** (`run-linux.sh` / `run-windows.ps1` do this already; shown for manual use):
```
./benchmark-launcher-cli blender list
./benchmark-launcher-cli scenes list --blender-version 5.2.0
./benchmark-launcher-cli devices --blender-version 5.2.0
./benchmark-launcher-cli benchmark monster junkshop classroom \
    --blender-version 5.2.0 --device-type CUDA --json
```
Device types tested: `CUDA` and `OPTIX` (TU106 supports both) on all three scenes, `CPU` once
on `monster` as a sanity check (`BLENDER_FULL_DEVICES` / `BLENDER_SANITY_*` env vars). Never
pass `--submit` (needs `authenticate`, an OAuth/browser flow — excluded by the no-accounts
rule).

**Expected native ballpark, RTX 2070:** the device's own Blender Open Data page reports a
**median score of 2194** across 937 public submissions (aggregate across scenes/Blender
versions — the page doesn't break it out further without an account/query tool):
<https://opendata.blender.org/devices/NVIDIA%20GeForce%20RTX%202070/>. Treat it as a ballpark,
not a per-scene target — the pack's own numbers will differ by Blender version (5.2.0 here
vs. whatever each submitter used) and driver.

## 2. Unigine Heaven 4.0 / Superposition 1.1 (free Basic editions)

**What they measure:** fixed-camera OpenGL/Direct3D11/9 rendering workload with heavy
tessellation (Heaven) or PBR/volumetrics (Superposition); 100%-GPU-bound stress + an FPS/Score
result. Both are the free **Basic** edition: personal/benchmark use only, no commercial
redistribution of results (benchmark.unigine.com product pages, fetched 2026-09-29).

| File | Size | sha256 (we computed; Unigine publishes none) |
|---|---|---|
| `Unigine_Heaven-4.0.run` | 274 MB | `52db2e5dedd5621d7c2bfa936b482c0a7cc69b30616278ef199513d494d8e397` |
| `Unigine_Heaven-4.0.exe` | 248 MB | `497865a09af205a7d2e8812bfc2dd54870216cc1e95541b63cef89a4433b0e5f` |
| `Unigine_Superposition-1.1.run` | 1.53 GB | `7494e1c73bf59ef2161513d5d5cb9d9bffbd8479929d4976ac53b6955de580f8` |
| `Unigine_Superposition-1.1.exe` | 1.25 GB | `de5dbf4f7ad284b709c5db318dbedc1b025a41e50326595a444ee9cbd4a17182` |

Source: `https://assets.unigine.com/d/<file>`, linked from `benchmark.unigine.com/heaven` and
`/superposition`.

**Automation — the one real decision point in this pack.** Unigine's own Basic-edition manual
(`benchmark.unigine.com/docs/heaven/User_Manual.pdf`, `.../superposition/Superposition_Benchmark_User_Manual.pdf`)
states plainly: *"Command line automation support [only for Advanced and Pro versions]"*. The
free download is meant to be clicked through. What `run-linux.sh`/`run-native.sh`/
`run-windows.ps1` use instead is **not** that licensed feature — it's calling the same free
`.run`/`.exe` payload's raw engine binary directly (`heaven_x64`/`Heaven.exe`,
`superposition`/`Superposition.exe`) with standard Unigine-engine startup flags
(`-data_path`, `-engine_config`, `-system_script`, `-video_app`, `-extern_define`,
`-console_command … -mode 2 -preset 0`). This is exactly the invocation Phoronix Test Suite's
`pts/unigine-heaven` and `pts/unigine-super` test profiles have used against these same free
downloads for years (verified by fetching their `install.sh`/`install_windows.sh`/
`downloads.xml`/`results-definition.xml` from
<https://github.com/phoronix-test-suite/test-profiles> on 2026-09-29 — the download URLs there
matched ours exactly). It reliably produces a non-interactive run and a parseable result
(`FPS:`, `Score:`, `Min FPS:`, `Max FPS:` lines in Heaven's own stdout / Superposition's
`~/.Superposition/automation/log*.txt`), but it is a community-proven technique, not something
Unigine documents or supports for the free edition — flagging that honestly rather than
presenting it as an official CLI. The quality/tessellation `-extern_define` names
(`QUALITY_HIGH`, `TESSELLATION_EXTREME`, …) likewise come from a Phoronix forum thread on
tuning Heaven 4.0, not from Unigine's manual.

Presets implemented (`HEAVEN_PRESETS` / `SUPERPOSITION_PRESETS` env vars):
- Heaven: `basic` (High quality, Normal tessellation) and `extreme` (Ultra, Extreme
  tessellation) — matching the manual's own Basic/Extreme preset names.
- Superposition: `1080p_medium` and `1080p_high`, per the task's ask — these map to
  `-shaders_quality`/`-textures_quality` 1/1 and 2/2 respectively, 1920x1080. (The GUI's
  "1080p Extreme" preset changes more than quality/texture flags; Unigine doesn't document its
  exact flag set outside the interactive GUI and the paid CLI, so it isn't reproduced here.)

**Score/FPS without interaction:** both binaries write their results to a place the scripts
already capture — Heaven prints them to its own stdout (redirected to the run's log file);
Superposition writes `~/.Superposition/automation/log*.txt` (`%USERPROFILE%\Superposition\automation\` on
Windows) when run with `-mode 2` (benchmark, not interactive). `lib-common.sh`'s
`parse_heaven_log` / `parse_superposition_log` (and the PowerShell `Get-LogField`) grep exactly
those field names.

**GNOME/Wayland note (Linux):** Heaven/Superposition are X11 OpenGL apps; under GNOME on
Wayland they render through XWayland, which needs the already-logged-in session's `DISPLAY`
(and usually `XAUTHORITY`) — there's nothing to render into otherwise. `run-linux.sh`
auto-discovers both via `loginctl show-session … -p Display/User`; override with
`LINUX_DISPLAY`/`LINUX_XAUTHORITY` if that guest's session manager differs.

**Windows note:** `run-windows.ps1` must run inside the signed-in session (a scheduled task
with an *interactive* principal, not a service/batch principal — the same class of session the
Blackwell scripts and Sunshine host use). Use the existing `run44.py upack` helper to deploy
and trigger it; this pack doesn't attempt to create that scheduled task itself since that
tooling lives outside it.

**Expected native ballpark, RTX 2070** (public numbers, both need the same caveat: settings
weren't independently reproducible from the source page, so treat as rough orientation only):
- Heaven, default/Extreme-ish settings: **~5852 score, ~232 FPS** avg, ASUS ROG STRIX RTX 2070
  OC review — <https://www.vortez.net/articles_pages/asus_rog_strix_rtx_2070_oc_review,15.html>
  (page 15; the OC card should sit a little above a stock 2070).
- Superposition, 1080p Extreme: **~9085** points, a community-submitted result (not an
  official database) — referenced from an eGPU.io Superposition benchmark thread found via web
  search; treat this one as a very rough sanity range, not a citation-grade figure.

## 3. Result format

Every run appends one tab-separated line per test to `$OUT_DIR/results.tsv` and echoes it:

```
RESULT  ts=<iso8601>  host=<linux-guest|windows-guest|native-host-…>  benchmark=<blender|heaven|superposition>
        preset=<scene-or-quality-preset>  device_api=<CPU|CUDA|OPTIX|OpenGL|Direct3D11>
        score=<value|NA>  fps_avg=<value|NA>  fps_min=<value|NA>  duration_s=<n>  log=<path>
```
Raw logs (Blender's `--json` output + `-v 2` debug log, Heaven's stdout, Superposition's
automation log) are kept next to it under `$OUT_DIR/logs/`, so anything the one-line summary
drops is still on disk.

## 4. Preparing and running

```
# 1) fetch + verify + pre-seed the offline cache (host, ~4.5 GB, ~2 min on this link)
caraxes/tools/bench-pack/fetch-and-seed.sh all

# 2) Linux guest (from the host, over SSH)
LINUX_GUEST_HOST=caraxes@<linux-guest-ip> caraxes/tools/bench-pack/run-linux.sh

# 3) Windows guest (deployed into the guest's signed-in session via run44.py upack,
#    payload staged at C:\bench-pack per run-windows.ps1's header)
powershell -File run-windows.ps1

# 4) native reference (on THIS host, touches the GPU for real — see run-native.sh's header;
#    only run with the user's explicit go-ahead for that session)
caraxes/tools/bench-pack/run-native.sh
```

## Open decisions for the operator

1. **Guest staging path.** `run-linux.sh` rsyncs the CLI/Heaven/Superposition archives and the
   seeded Blender cache to the guest over SSH (`GUEST_BENCH_DIR`, default `~/bench-pack`); if
   the guest already has `/mnt/vmstore/caraxes` shared in some other way, staging can skip the
   copy — set `GUEST_BENCH_DIR` to that mount instead.
2. **Windows staging.** `run-windows.ps1` assumes the same payload already sits at
   `C:\bench-pack` (parameter `-BenchPackDir`); how it gets there is `run44.py upack`'s job, not
   this script's.
3. **Heaven's Windows installer silence.** Superposition's Windows install is confirmed silent
   with `/SILENT` (Unigine's own Phoronix-profile install script uses it); Heaven's isn't
   independently confirmed the same way — if it pops an install wizard instead of installing
   quietly, that's a one-time interactive click-through in the guest, then it's unattended from
   there on.
4. **Scope/size.** Superposition alone is ~2.9 GB of the ~4.5 GB pack. If that's more than
   wanted for this pass, `fetch-and-seed.sh download` can be edited to skip it — Heaven +
   Blender alone is under 1.4 GB.
