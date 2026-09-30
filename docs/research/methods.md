<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Methods

Scope: how Caraxes is tested and measured: gates, benchmark harness, census, profile, coverage map and control sweep, lab kit, emu-boot check, ABI drift check, and the recipe for a new workload.

## Facts

### Gates

- `tools/caraxes-gate.sh` runs five gates, `nvml`, `cuda`, `gpu`, `vdisplay` and `display`, one per invocation with `VM=<vm>` [tools/caraxes-gate.sh:773-777; docs/vfio/testing.md §3].
- A gate needs the VM up, nvidia.ko loaded (`tools/caraxes-vm.sh probe`), the test material synced (`sync-tests`) and the device trace readable, and a missing precondition is a skip, never a fail [docs/vfio/testing.md §3].
- `cuda` runs one ssh call per test, so each test gets its own window of the device's RPC trace; the scored tests are cuda-init, cuda-context, cuda-core, cuda-jit, cuda-launch, cuda-managed, cuda-hostreg, cuda-torch, cuda-gdb, rlprobe, convburn and robustness [docs/vfio/testing.md §3 cuda].
- A failed `cuda` test is re-run once under the v1 tracer (`LD_PRELOAD=libnvrm_trace.so`), and its row carries the failing CUDA call, the last non-NV_OK RM ioctl and every device answer in the window that was not NV_OK [docs/vfio/testing.md §3 cuda].
- `gpu` runs v1's `gate_gpu` stages a guest can carry (smi masked diff, memory, kernel, torch bit-identical to a native reference, robustness, own, encode) [docs/vfio/testing.md §3 gpu].
- `vdisplay` and `display` need `CARAXES_DISPLAY=1`, and their flips and compositor stages pass within 60 +- 0.5 Hz [docs/vfio/testing.md §3 vdisplay, display].
- `nvml` compares the guest's `nvidia-smi -q` with two host `nvidia-smi -q` readings over 11 checks (name, table row, total, usage, four numbers, temperature, pstate family, host name differs) [docs/vfio/nvml.md §The gate; tools/caraxes-gate.sh:89].
- Each gate run writes `caraxes/runs/gate-<name>-<date>[-N]/` with `verdict.json`, `transcript.log`, `table.txt`, the guest logs and the device trace windows [docs/vfio/testing.md §5].
- The v1 path on the 615 image is gated by `caraxes/tools/v1-gates.sh OUTDIR smi|cuda|all` with the mask of Leandro-Test's gpu gate [caraxes/tools/v1-gates.sh:1-15; run: caraxes/runs/matrix-2026-09-25/v1].

### v1 verdict rules (kept)

- One JSON line on stdout (`{"gate":...,"result":"pass|fail|skip",...}`), human text on stderr [docs/vfio/testing.md §4 rule 1].
- Exit 0 pass, 1 fail, 2 skip, and skip only for a missing precondition, since a stage that could not measure fails [docs/vfio/testing.md §4 rule 2].
- Gates are binary: any failed stage fails the gate, and expected failures live in the unscored suites [docs/vfio/testing.md §4 rule 3].
- A gate that dies still reports a fail through its EXIT trap (`g_atexit`) [docs/vfio/testing.md §4 rule 4; tools/caraxes-gate.sh:187].
- Stages compare against the native run where v1 did, and a probe that exits 0 without a `CRITERION:` line fails [docs/vfio/testing.md §4 rules 5-6].

### Benchmark harness

- `caraxes/tools/bench-run.sh series` interleaves the targets inside every rep (rep loop outermost), restarts v2 per rep with `V2_RESTART=1`, and times inside the target, never across ssh [docs/vfio/benchmark.md §2; caraxes/tools/bench-run.sh:25-47].
- Each run records GPU state, running VMs, device and VMM CPU and RSS, and a run is marked concurrent when another VM's device process used more than 20 % of a CPU [docs/vfio/benchmark.md §2].
- `bench-summary.py` gives `median [min-max] n=runs` and the ratio to native, and `bench-hostwatch.sh` queries `nvidia-smi` every 5 s and stops the device when the host answers slower than `SLOW=5` s [docs/vfio/benchmark.md §4.1; docs/vfio/workload-gate.md §1].

### Census

- `caraxes/tools/census-cu.c` runs one driver-API operation at a time and brackets every repetition with two writes to `NV_PBUS_SW_SCRATCH(30)` (BAR0 0x1478), value `0xce000000 | op << 12 | rep << 1 | end`, outside the timed interval [caraxes/tools/census-cu.c:5-24; docs/vfio/optimization-backlog.md §1].
- The markers need root in the guest (`CENSUS_MARK=<resource0>`), repetitions are 30 ms apart (`CENSUS_GAP_MS`), and natively the same binary only times [caraxes/tools/census-cu.c:18-24].
- `caraxes/tools/census-table.py` splits the device trace at the markers and gives per operation the traps by class and by BAR, page and register, the traps an existing window would take, the GSP RPCs, the shadow plans and the doorbells and MSIs [caraxes/tools/census-table.py:5-20; docs/vfio/optimization-backlog.md §1].
- `summarize.py` subtracts the background measured in 100 ms idle windows, and `workload-rates.py` counts the same classes over whole rlprobe, PPO and short-auto processes [docs/vfio/optimization-backlog.md §1; run: caraxes/runs/trap-census-2026-09-25/summarize.py].
- A census number counts only with 0 host Xid on the device's pid and the other VMs' activity recorded in `census-others.txt` [docs/vfio/optimization-backlog.md §6; run: caraxes/runs/trap-census-2026-09-25].

### Profile mode

- `CARAXES_PROFILE=1` aggregates in memory per row a count, device wall time, the largest time, a status histogram and the answer's label, for trapped accesses, RPCs, controls, allocs, events, host RM ioctls, shadow call kinds and window hits [docs/vfio/coverage-map.md §1.1; crates/vfio-user-nvgpu/src/profile.rs:1-43].
- Rows exist per device lifetime and per guest RM client, from its `NV01_ROOT` allocation to its free [docs/vfio/coverage-map.md §1.1 Scopes].
- SIGUSR2 dumps `<prefix>.<n>.tsv` and exit dumps `<prefix>.exit.tsv`, with the prefix from `CARAXES_PROFILE_OUT`, and a workload's profile is the dump after it minus the dump before it [docs/vfio/coverage-map.md §1.1 Dumps; crates/vfio-user-nvgpu/src/profile.rs:28].
- A hook costs 84 ns per trapped access, 26 ns per control, 38 ns per window hit and 0.3 ns with the profile off, and short-auto with it on stayed within noise [docs/vfio/coverage-map.md §1.3; run: caraxes/runs/coverage-2026-09-25/overhead-on-2].
- `caraxes/tools/profile-rank.py DUMP --base DUMP` ranks rows by count times mean latency and joins them with `controls.tsv`, `classes.tsv`, `rpc.tsv`, `events.tsv`, rpc-cost.py's access class and the handler module; `--seen-in`, `--clients` and `--scope` narrow it [docs/vfio/coverage-map.md §1.2].

### Coverage map and guest control sweep

- The map ran 29 workload windows on vm2 (610 guest, windows on) with the device built with `-C instrument-coverage`, so LLVM function coverage sits beside the profile [docs/vfio/coverage-map.md §2.1; run: caraxes/runs/coverage-2026-09-25/run.sh].
- `coverage-map.py` joins the per-workload dumps into `combined-coverage-map.md` and `combined-coverage.json`, and `llvm-cov.sh` gives per-function coverage [docs/vfio/coverage-map.md §2.2-2.3; run: caraxes/runs/coverage-2026-09-25].
- `caraxes/tools/ctlsweep-gen.py` generates the sweep table from `controls.tsv`: GSP-routed getters a user-space client may call on client, device, subdevice, DISPLAY_COMMON, SUBDEVICE_DIAG, I2C or ZBC_CLEAR, optionally only untouched ids (`--only`) and priv rows (`--priv`) [caraxes/tools/ctlsweep-gen.py:5-21].
- `caraxes/tools/ctlsweep.c` issues each control once in the guest through the stock RM ioctl path with zeroed parameters of the control's size, after the `rmdirect.c` setup (version handshake, root client 0x41, device, subdevice) [caraxes/tools/ctlsweep.c:5-18].
- The map plus sweep reached 208 of 829 GSP-routed controls and all 70 host-forwarded ones, and 931 of 1464 device functions ran under a workload [docs/vfio/coverage-map.md Summary, §2.3; run: caraxes/runs/coverage-2026-09-25/combined-coverage.json].
- The sweep issued 119 of 172 generated controls, 111 reached the device, and 21 user-callable getters stayed unanswered [docs/vfio/coverage-map.md §3; run: caraxes/runs/coverage-2026-09-25/sweep-reach.md].

### Lab kit

- `caraxes/tools/lab-kit.sh pack --ogkm <615.71.09 tree>` builds a tarball for 615.71.09 that a person runs by hand on a rented or workplace GPU with `run`, then `verify` [docs/vfio/gsp-plan/lab-kit.md §1-2].
- The run has nine steps: preflight, capture (`lab-capture.sh`), rm-tools, a sweep of 164 getters (`lab-sweep`), traces under the LD_PRELOAD tracer, display, extras, scrub and pack [docs/vfio/gsp-plan/lab-kit.md §4].
- Only `--rom`, `--priv` and the kernel log may need root, and the kit never loads a module, resets the card, writes a register or calls the network [docs/vfio/gsp-plan/lab-kit.md §3].
- The scrub replaces host name, user, GPU UUID, serials, MACs, IPs and UUIDs, and refuses to pack when one collected value remains [docs/vfio/gsp-plan/lab-kit.md §4-5].
- At home `caraxes/tools/lab-ingest.py` checks sums and identifiers and compares the sweep with `chips/<chip>/chip.toml` `[features]` [docs/vfio/gsp-plan/lab-kit.md §6].
- The capture self-test on the development host ended with 33 items: 30 ok, 2 reduced and 1 skipped without sudo, and a verified 53-file tarball [run: caraxes/runs/lab-selftest-2026-09-24/README.md].

### Emu-boot check

- `tools/emu-boot-check.sh [--driver 610|615] [--chip tu106|gb206]` boots the synthetic GPU in pure emulation (no host RM, no reservation, no GPU, no tap) and exits 0 when RmInitAdapter succeeds and nvidia-smi returns 0, 1 when not, 77 when a prerequisite is missing [tools/emu-boot-check.sh:5-16].
- `--driver 615` uses vm7-615 and the `target/v615` build, prepared by `caraxes/runs/emu-regress-2026-09-25/prepare-vm7.sh` [tools/emu-boot-check.sh:31-36].
- `tools/check.sh` runs it as step `emu-boot` only with `CARAXES_CHECK_EMU_BOOT=1` [tools/check.sh:484-487].
- The guest must carry `RmDisableGlobalCeUtils=1;RMDisableScrubOnFree=1;RmRcWatchdog=0`, since without them the boot fails at the global CeUtils self-test (TU106) or the watchdog's GR object (GB206) [run: caraxes/runs/emu-regress-2026-09-25/README.md; docs/vfio/quickstart.md §2].
- The final 615 boot passed with RmInitAdapter succeeded and "Leandro VFIO RTX 2070-1G", and the negative case without the options failed with the device's registry warnings [run: caraxes/runs/emu-regress-2026-09-25/final; run: caraxes/runs/emu-regress-2026-09-25/check].

### ABI drift check

- `caraxes/tools/gsp-abi.py` has `snapshot`, `diff` and `render`, and a snapshot is TSVs (controls, rpc, events, classes, boot, layouts, constants, enums, bitfields) plus a `MANIFEST` [docs/vfio/gsp-abi-615.md §1; caraxes/abi/gsp-615.71.09/MANIFEST].
- Sizes are compiler `sizeof`, and a layout hash of clang's record layout catches a moved field at equal size [docs/vfio/gsp-abi-615.md §1].
- `diff` exits 1 when a struct the device implements changed size or field layout [docs/vfio/gsp-abi-615.md §1].
- `tools/check.sh` step `gsp-abi` checks that `gsp-abi-615-tables.md` is exactly the snapshot's rendering and runs `diff --strict` against a found 615.71.09 tree, skipping only the drift half without a tree [tools/check.sh:407-424, :481; docs/vfio/gsp-abi-615.md §8].
- The check on 2026-09-25 found 0 driver-side and 0 device-side changes; the per-driver workflow is in [abi-baseline.md](abi-baseline.md) [docs/vfio/gsp-abi-615.md §6, §8].
- 610 delta: `emu-boot-check.sh` and the `emu-boot` step default to the 610 guest (vm7), and the lab-capture self-test ran against the host's 610.57.04 driver [tools/emu-boot-check.sh:21, :31-33; tools/check.sh:485; run: caraxes/runs/lab-selftest-2026-09-24/README.md].

### How a new workload is measured

1. Add it as a `bench-run.sh` metric that prints one `# NAME key=value ...` line for `bench-summary.py`, and stage it into `target/bench/bin` and the guest's `~/bench` [docs/vfio/workload-gate.md §1-2].
2. Start the VM in the configuration under test plus `CARAXES_PROFILE=1 CARAXES_PROFILE_OUT=<run>/profile/dev`, with the host watch running [docs/vfio/coverage-map.md §5; docs/vfio/workload-gate.md §1].
3. `kill -USR2 $(cat $W/gpu.pid)` before the workload, then run it native and v2 interleaved with `V2_RESTART=1`, 3 reps, then `kill -USR2` again [docs/vfio/coverage-map.md §5; docs/vfio/workload-gate.md §1].
4. Rank with `profile-rank.py dev.2.tsv --base dev.1.tsv --top 30`, add `--seen-in <gate dumps>` for controls no gate sends and `--clients` for the process [docs/vfio/coverage-map.md §5].
5. Read ratio v2 / native, device CPU against wall time, and `step_p90_ms` against `step_med_ms` for device stalls [docs/vfio/workload-gate.md §4].
6. For untouched surface compare with `combined-coverage.json` and extend `ctlsweep-gen.py --only`; for unreached code rebuild with `-C instrument-coverage` and run `llvm-cov.sh report` [docs/vfio/coverage-map.md §5].
7. Keep the dumps, `marks.tsv` and the host Xid lines of the device's pid, and count numbers only with 0 host Xid on that pid [docs/vfio/coverage-map.md §5; docs/vfio/optimization-backlog.md §6].

## Consequence for Caraxes

- A green gate means what a green v1 gate meant, so a 615 result is comparable with v1's history row by row.
- Trap counts come from the census, device time outside traps from the profile, so both are needed before an optimisation is ranked (see [performance-facts.md](performance-facts.md)).
- The emu-boot and gsp-abi steps guard boot and ABI without the GPU, so they belong in every `tools/check.sh` run before a host session.
- The lab kit is the only route to chips the host lacks, and its output feeds chip files by hand (see [blackwell-and-hopper.md](blackwell-and-hopper.md)).

## Open

- The v1 aggregator's cross-check of each gate's JSON against its exit code is not reproduced; gates run one by one [docs/vfio/testing.md §4 rule 7].
- The `gpu` gate's torch stage has no native reference, because the host venv `LEA_HOSTVENV` does not exist [docs/vfio/testing.md §3 gpu].
- The sweep leaves setters, display getters (vm2 has no display engine) and controls on channel, profiler or debugger objects untouched [docs/vfio/coverage-map.md §3].
- cuda-gdb, nvcc, NVENC/NVDEC tools, clinfo and an OpenCL ICD are not in the guest image, so the map did not run them [docs/vfio/coverage-map.md §2.1].
- 141 device functions run under no workload and no test, among them `Invalidator`, `dma_unmap`, `reset` and the mirror's query and scheduling paths [docs/vfio/coverage-map.md §2.3].
- No lab-machine run of the kit is recorded, only the pack's dry run on the development host, the capture self-test and an ingest dry run on a synthetic tarball [docs/vfio/gsp-plan/lab-kit.md §1, §6; run: caraxes/runs/lab-selftest-2026-09-24/README.md].
