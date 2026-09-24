<!-- SPDX-License-Identifier: MIT -->
<!-- Written 2026-09-24 (subagent report, read-only analysis). Caraxes HEAD b7ec987, Leandro HEAD e2f1aec,
     Leandro-Test refactor-thesis 3edea73. Evidence: git log/diff/ls-files/ls-tree, cargo metadata --no-deps,
     grep. Nothing was built, run or modified; this file is the only write. -->
# Repository split: Caraxes (v2) and Leandro (v1)

The operator's decision (2026-09-24): driver 615.71.09 becomes the target ABI for both, a full
refactor is allowed, and everything that is not Caraxes moves cleanly into Leandro. This is the
plan for that split. It is based on the Caraxes tree at `b7ec987` (63 commits on top of the fork
point `e2f1aec`, which is also the HEAD of `../Leandro`, 177 commits, origin
`github.com/UniStuttgart-IKR/Leandro`).

**Recommendation in one line:** option B. The shared crates stay in Leandro, where they
originated. Caraxes depends on them by a git dependency pinned to a Leandro commit. The 615
switch happens once in Leandro, and Caraxes takes it by moving its pin.

## 0. Corrections to the brief

The brief lists some paths that the fork does not track. The table below gives what the trees
actually contain.

| Named in the brief | What the tree holds |
|---|---|
| `probe/`, `scripts/`, `jobs/`, `graphify-out/` | None of them is tracked at `e2f1aec` or at `b7ec987` (`git ls-tree --name-only e2f1aec`). Leandro commit `e4134de` ("separate hardware test tooling") moved the probes and rig scripts to `../Leandro-Test` (`probe/`, `scripts/`, `tests/`, `deploy/`, `lea`). The directories in the Caraxes checkout are leftovers of the older layout. `probe/` (4.3 MB, only `bin/`, `traces/` and `__pycache__` are left) is ignored by `.gitignore:19-23`. `jobs/` (44 KB, Slurm fleet jobs) is ignored by `.gitignore:39`. `graphify-out/` (113 MB, a generated index) is ignored by `.gitignore:34`. |
| `crates/vhost-user-input` | Not a crate here. `0c2e048` replaced it with upstream `vhost-device-input` (`nix/packages/vhost-device-input.nix`, `vendor/vhost-device` untracked). |
| `tools/build.sh`, `tools/check.sh` | Present. `build.sh` has not changed since the fork. `check.sh` changed once (`f2e7be8`). |
| "the probes under probe/" as shared | They live in `../Leandro-Test/probe` (C, Python, matrix scripts). Caraxes reads them from there (`tools/caraxes-vm.sh:287-300`, `tools/caraxes-gate.sh:164`) and never copies them into its own tree. They are already in the third repository, and the split leaves them where they are. |
| `.claude/CLAUDE.md` Repowise index | Untracked (`/.claude/` ignored). It describes "Leandro" at `a5389d5`, which is before the fork, including paths that no longer exist (`crates/vhost-user-input`, `probe/`, `scripts/`). It is stale in both checkouts (see section 5). |

## 1. Inventory

The class is the result of the split: **v1** goes to Leandro only, **C** stays in Caraxes only,
and **S** is shared, meaning Leandro owns it and Caraxes consumes it.

### 1.1 Crates (`cargo metadata --no-deps`, workspace `Cargo.toml:6-10`)

| Crate | Class | Path deps (from metadata) | Who uses it, with evidence |
|---|---|---|---|
| `nvrm-sys` | S | none | Leandro: everything. Caraxes: `nvrm_sys::v610` in `vfio-user-nvgpu` (`src/ver/v610.rs:17`, `src/kgr.rs:571,678`, `src/ctrl.rs:1067-1076`, `src/nvml.rs:377,389`), and `assert_driver_version` through `nvrm-client` (`crates/nvrm-client/src/lib.rs:39`). Features `v580 v595 v610 v615`, default `v610` (`crates/nvrm-sys/Cargo.toml:18-28`, generated from `DRIVER_VERSION` by `crates/xtask/src/abi/emit.rs:113,332`). No `build.rs`, so a git dependency builds without `vendor/`. |
| `nvrm-abi` | S | nvrm-sys, nvrm-wire | Caraxes: `nvrm_abi::naming` (5 uses), `nvgpu` (5), `sys`, `status_name`, `check_status`. The `naming` module (P17) is new since the fork. |
| `nvrm-wire` | S (transitive) | none | v1 wire protocol. Caraxes reaches it only through `nvrm-abi`'s dependency. |
| `nvrm-client` | S | nvrm-abi, nvrm-sys | Caraxes: `nvrm_client::mem` (4 uses). `crates/vfio-user-nvgpu/Cargo.toml:24` uses a raw `path = "../nvrm-client"` rather than a workspace dependency. `vgpuprofile` is named in the device's help (`crates/vfio-user-nvgpu/src/main.rs:58`). |
| `nvrm-trace` | S (binary only) | nvrm-abi, nvrm-sys | cdylib `libnvrm_trace.so`. Caraxes ships it into the guest for failed-test traces (`tools/caraxes-vm.sh:327-332`, `tools/caraxes-guest-tests.sh:39`). No Rust dependency. |
| `xtask` | S (Leandro-only tool) | none | Generates `nvrm-sys` (`cargo xtask abi`, alias in `.cargo/config.toml`). The `abi` step of `tools/check.sh:365-378` runs it. |
| `vhost-user-nvrm` | v1 | nvrm-abi, nvrm-sys, nvrm-wire | The v1 backend. Caraxes does not depend on it: `vfio-user-nvgpu` has no path dependency on it, despite the wording of D10 in `docs/caraxes/README.md:157`. Caraxes *runs* it in the P13 benchmark (`caraxes/tools/bench-v1vm.sh:22`, built "from this tree"). |
| `vfio-user-nvgpu` | C | dp-sink, nvrm-abi, nvrm-client, nvrm-sys | The Caraxes device. Features `default = ["v610"]`, `v610 = ["nvrm-sys/v610"]` (`Cargo.toml:42-43`). |
| `dp-sink` | C | none | DisplayPort sink, moved into `crates/` by `37582db`. |
| `crates/vhost-user-nvrm/fuzz` | v1 | own lockfile | A separate workspace, excluded from the nix fileset (`nix/packages/leandro.nix:25`). |

### 1.2 Top-level paths

Sizes are of tracked files (`git ls-files | xargs stat`) unless stated otherwise.

| Path | Tracked | Class | Evidence |
|---|---|---|---|
| `.cargo/config.toml` | yes | v1 (S tool) | Only the `xtask` alias. |
| `.clang-format`, `.editorconfig`, `.shellcheckrc` | yes | both (copy) | Style for C and shell, which both repositories keep. |
| `.coderabbit.yaml` | yes | v1 | Path instructions for `crates/**`, `crates/xtask/**` and `.github/workflows/**` (lines 20-47). |
| `.github/workflows/check.yml` | yes | v1 | Runs `tools/build.sh vendor`, then `tools/check.sh`, plus the c-kernel job for `guest-module/nvrm_nodes` and `virtio_nvrm` only. The Caraxes remote is self-hosted over SSH and has no GitHub Actions. |
| `.gitignore` | yes | split | Lines 78-90 are the Caraxes additions (`66682ae`, `22f3609`, `6551c27`). The rest is v1. |
| `CH_VERSION` (`v53.0`) | yes | both (copy) | `tools/build.sh ch` and the flake (`flake.nix:22`). Caraxes needs it for its own CH build. |
| `DRIVER_VERSION` (`610.57.04`) | yes | both, must agree | Leandro: it drives the `nvrm-sys` default and the flake. Caraxes: `tools/caraxes-vm.sh:323` copies it into the guest payload. See risk R6. |
| `Cargo.toml`, `Cargo.lock` | yes | split | The members, the `vhost` 0.17 bump (v1) and `[patch.crates-io] vfio_user` (Caraxes, `Cargo.toml:37-44`). |
| `README.md`, `DEVELOPMENT.md` | yes | v1 | Unchanged since the fork. Caraxes needs its own README. |
| `LICENSE`, `LICENSES/`, `LICENSES.md` | yes | both (copy) | `LICENSES.md` names neither `vendor/vfio-user-rs` (Apache-2.0/BSD-3-Clause) nor `patches/linux` (a GPL-2.0 kernel patch). Caraxes has to add both. |
| `rust-toolchain.toml` | yes | both (copy) | Pinned 1.89.0. |
| `flake.nix`, `flake.lock`, `nix/` (15 files) | yes | v1 | Every output is v1: `leandro`, `vhost-user-nvrm`, `cloud-hypervisor`, the guest modules and images, and the host and guest debs. Nothing in it builds `vfio-user-nvgpu`. `../Leandro-Test` consumes `leandro.lib.mkGuestImage` and the packages. |
| `packaging/` (14) | yes | v1 | Guest DKMS deb (`nvrm_nodes`, `virtio_nvrm`) and host deb (`leandro-backend@.service`). |
| `tests/tools/` (3) | yes | v1 | `edid-verify.c` and `vdisp-frame.c`, used by `tools/ci/check-c.sh host-tools` and `nix/packages/test-tools.nix`. |
| `matrix/` (9 files, 824 KB; 240 MB on disk) | 9 reports tracked, `traces/` 239 MB ignored (`.gitignore:72-73`) | v1 | The 610.57.04 coverage matrix. One Caraxes doc cites it (`docs/caraxes/phase5-rpcs.md:340`). |
| `guest-module/nvrm_nodes`, `guest-module/virtio_nvrm` | yes | v1 | The v1 guest side. `tools/ci/check-c.sh` tests and the c-kernel CI job. |
| `guest-module/caraxes_dma_range` (6 files) | yes | C | D13 guest aid (`66682ae`). The licence rule `guest-module/* => GPL-2.0-only` (`tools/check.sh:311`) applies to it. |
| `guest-module/LICENSE` | yes | both (copy) | GPL text for both module sets. |
| `patches/0001-0003`, `patches/REVIEW-vhost-user.md` | yes | v1 | The generic vhost-user SHMEM series, rewritten in `c8a6f29`. |
| `patches/0004-acpi-dma-host-bridge.patch`, `patches/ACPI-DMA.md`, `patches/linux/` | yes | C | "Caraxes only" (`patches/README.md:21`). v1 does not use `_DMA`: its guest memory goes through the SHMEM window. |
| `patches/0005-vfio-user-region-io-fds.patch`, `patches/vfio-user-rs/` | yes | C | Needs `../vfio-user-rs` beside the CH tree (`patches/0005:121-123`). `patches/0006` and `vfio-user-rs/0002` are in flight in worktree `agent-abcf6071de2ca99bb`. |
| `patches/README.md` | yes | split | v1 sections plus the 0004/0005 rows and sections (`fe2a09a`, `66682ae`). |
| `tools/build.sh` | yes | v1 (S) | `vendor`, `vendor-abi`, `ch` (the glob `patches/[0-9][0-9][0-9][0-9]-*.patch`, `tools/build.sh:153`, so it applies 0001-0005 today) and `cargo`. |
| `tools/check.sh`, `tools/ci/` | yes | v1, with a Caraxes copy needed | Steps listed at `tools/check.sh:400-417`. |
| `tools/caraxes-vm.sh`, `caraxes-gate.sh`, `caraxes-guest-tests.sh` | yes | C | The launcher, the gates and the guest test runner. |
| `docs/` except `docs/caraxes` (235 files, 2.0 MB incl. `history/`, `img/`, `measurements/`) | yes | v1 | `docs/README.md:34` links `caraxes/` (added by `66682ae`). |
| `docs/caraxes/` (58 files, 1.7 MB, plus untracked `mapping.md` in flight) | yes | C | Contains `briefing-naming.md`, the P17 spec that `crates/nvrm-abi/src/naming.rs:5` cites. |
| `caraxes/runs/` (2103 files, 523 MB tracked; 1.1 GB on disk) | partly: `*.trace`, `device/` and `transcript.log` are ignored (`.gitignore:86-89`) | C, except `vhost-user-check-2026-09-23/` (14 files, 424 KB), which is v1 evidence for patches 0001-0003 (`patches/README.md:600-642`) | Excluded from the licence check (`tools/check.sh:319`). |
| `caraxes/reference/` (56 files, 354 KB) | yes, `rom.bin` ignored | C | TU106 and GB206 captures. |
| `caraxes/tools/` (26) | yes | C | `bench-v1vm.sh` and `bench-run.sh` drive v1 as a comparison (section 2.3). |
| `vendor/nvidia-rm-headers` (414 files, 8.7 MB) | yes | S (Leandro) | Input to `cargo xtask abi`. Caraxes reads it in `caraxes/tools/lab-abi-check.sh:44`. |
| `vendor/vfio-user-rs` (10 files, 147 KB) | yes | C | `[patch.crates-io]` and the CH path dependency. |
| `vendor/open-gpu-kernel-modules` (332 MB, 610.57.04) | no (`.gitignore:3`) | both, as separate checkouts | Leandro: `check.sh` `class_sizes`/`kapi_abi` and guest NVKMS. Caraxes: `caraxes/tools/gsp-abi-sizes.sh:49`, `tools/caraxes-gate.sh:165`, the source citations in `vfio-user-nvgpu`. `vendor/open-gpu-kernel-modules-615` is already ignored (`.gitignore:82`). |
| `vendor/cloud-hypervisor` (847 MB) | no | both, as separate checkouts | Caraxes' checkout carries 0001-0004 as working-tree changes. 0005 is not applied in the main checkout. |
| `vendor/hostvenv` (4.6 GB) | no | both today | Copies exist in `../Leandro` and in Caraxes. `../Leandro-Test` has none, yet `tools/caraxes-gate.sh:411` defaults to it. |
| `vendor/vhost-device` | no | v1 | Optional upstream input backend. |
| `vendor/vfio`, `vendor/libvfio-user`, `vendor/qemu-vfio-user` | no | C | Reference checkouts (`.gitignore:79-81`). |
| `target` → `/mnt/vmstore/leandro/build-refactor-thesis/target` | symlink, ignored | **shared by both checkouts** | Both `../Leandro/target` and `Caraxes/target` point at the same directory (risk R3). |
| `result` → nix store `leandro-host-deb-0.1+610.57.04` | symlink, ignored | v1 | |
| `local.env` | ignored | local | Sets `LEA_CH=/mnt/vmstore/leandro/cloud-hypervisor/bin/cloud-hypervisor`, a binary built 2026-08-18, before the rewritten series and without 0005. Also sets `LEA_BASE_IMAGE` (the baked 610.57.04 image). |

Branches: the Caraxes repository still carries Leandro's branches `lib-matrix`, `refactor-thesis`,
`slim`, `vram` and `vram-grid`, which came with the fork. It also has five
`worktree-agent-*` branches. Only `worktree-agent-a5d3416bda3c346f5` is ahead of main (6 display
commits), and none of them touches a v1 or shared path. Two agent worktrees have uncommitted
changes (`agent-abcf6071de2ca99bb`: 0006 and vfio-user-rs 0002). The main checkout has
uncommitted `vfio-user-nvgpu` 615 work (`src/ver/mod.rs`, +82 lines).

## 2. Port-back diff (e2f1aec..b7ec987, v1 and shared paths)

The command was `git diff --stat e2f1aec..HEAD` excluding the Caraxes-only paths: 46 files,
+13804/-850. Most of that total is Caraxes-only material in shared directories (patches
0004/0005, `vendor/vfio-user-rs`, the `caraxes-*` tools). The per-path commit ids come from
`git log e2f1aec..HEAD -- <path>`.

### 2.1 Per path

| Path | Commits | What changed | Leandro needs it? | How |
|---|---|---|---|---|
| `patches/0001-0003` | `c8a6f29` | Series rewritten to the vhost-user specification: SHMEM at bit 22, `GET_SHMEM_CONFIG`, a frontend on `vhost` 0.17 (+1232/-338) | **Yes.** The backend in `678a4e4` speaks only this protocol, and a 0.16 peer and a 0.17 peer never negotiate SHMEM (`patches/README.md`, "Crate versions") | Pure move |
| `patches/REVIEW-vhost-user.md` | `72d9da2` | Review of the old series against the spec | Yes | Pure move |
| `patches/README.md` | `31ba74c`, `79b7d0b` (v1). `66682ae`, `fe2a09a` (0004/0005) | The spec tables, the manual VM check, and the 0004/0005 rows and sections | Yes, the state after `79b7d0b` | Adapt: port only `31ba74c` and `79b7d0b`, which predate the 0004/0005 edits. Then relink `:642` from `../caraxes/runs/vhost-user-check-2026-09-23/` to the new Leandro location |
| `caraxes/runs/vhost-user-check-2026-09-23/` (14 files) | `79b7d0b` | Evidence of the manual VM check of 0001-0003 | Yes (v1 evidence) | Adapt: port together with `79b7d0b`, then `git mv` to `docs/measurements/vhost-user-check-2026-09-23/`. The files already carry SPDX headers, so Leandro's licence check passes |
| `nix/packages/cloud-hypervisor.nix` | `c8a6f29` | `cargoPatches` in place of `patches` (0001 changes CH's `Cargo.lock`), new `cargoHash` | **Yes**, or `nix build .#cloud-hypervisor` stays on the old series | Pure move. The hash was computed when `patches/` held only 0001-0003, so it is valid again in Leandro and wrong in Caraxes today (risk R2) |
| `Cargo.toml`, `Cargo.lock`, `crates/vhost-user-nvrm/fuzz/Cargo.lock` | `678a4e4` (v1). `66682ae`, `37582db`, `fe2a09a` (Caraxes) | `vhost` 0.17, `vhost-user-backend` 0.23, `virtio-queue` 0.18, `vm-memory` 0.18 | Yes, `678a4e4` only | Pure move, path-limited. The members and `[patch.crates-io]` stay in Caraxes |
| `crates/vhost-user-nvrm/src/nvrm.rs`, `host_pool.rs`, `Cargo.toml` (dev-dep `vhost-user-frontend`) | `678a4e4` | Speaks the specified SHMEM protocol. `GuestMemoryBackend` import for vm-memory 0.18 | Yes | Pure move |
| `crates/vhost-user-nvrm/src/vram.rs` | `59455f0`, `6233ab5` (via merge `718a311`) | The guest card name from `VirtualGpuSpec`, and the transport-word default | Yes, with the naming module | Pure move |
| `crates/vhost-user-nvrm/README.md` | `59455f0`, `d025a08` | `LEA_GPU_NAME_FORMAT` documented | Yes | `59455f0` pure. `d025a08` path-limited to this file |
| `crates/nvrm-abi/src/naming.rs`, `lib.rs` (`pub mod naming`), `vgpu.rs` (doc link) | `176b53e`, `d45bf4f`, `6233ab5` | P17: one source of guest-visible names. The default is `Leandro <Transport> <Personality>-<Profile>`, and `LEA_GPU_NAME_FORMAT=legacy` gives the old name | **Yes.** Caraxes uses it (option B needs it in Leandro), and v1's names change with it | Adapt: `naming.rs:5` cites `docs/caraxes/briefing-naming.md`, which Leandro will not have. Carry the spec into `docs/NAMING.md` (Leandro) and point the comment there |
| `crates/nvrm-abi/src/lib.rs` (`NvDevice::try_clone`) | `0a78d41` | A second FD for the same RM client | Yes (additive, Caraxes uses it) | Path-limited pure move. The rest of `0a78d41` is Caraxes |
| `guest-module/virtio_nvrm/virtio_nvrm.c` | `678a4e4` | Comment on shmid 1 and `GET_SHMEM_CONFIG` | Yes | Pure move |
| `docs/ARCHITECTURE.md` | `678a4e4` | "Shared mappings" follows the spec | Yes | Pure move |
| `docs/README.md` | `66682ae` | Row linking `caraxes/` | No | Leave it out. Optionally add a sentence without a link: "v2 (Caraxes) is developed in a separate repository" |
| `tools/check.sh` | `f2e7be8` | Licence check skips `caraxes/runs/` | No | Stays in Caraxes' own `check.sh` |
| `.gitignore` | `66682ae`, `22f3609`, `6551c27` | Caraxes ignore rules | No | Stays in Caraxes |
| `nvrm-sys` (`v580`/`v595`/`v610`/`v615`, `abi.toml`, `versions.toml`, `manifests/`) | none | Unchanged since `e2f1aec`: `v615.rs` (615.71.09, commit `61dcc93`) was already generated in Leandro (`5dc0ac7`) | Nothing to port | |
| `tools/build.sh` (`vendor-abi`), `vendor/nvidia-rm-headers` | none | Unchanged | Nothing to port | |
| `crates/nvrm-client` (incl. `vgpuprofile`), `nvrm-wire`, `nvrm-trace`, `xtask` | none | Unchanged | Nothing to port | |
| `crates/nvrm-abi/src/vgpu.rs` (profile code) | `176b53e` | One doc line only | Comes with the naming series | |
| `flake.nix`, `nix/` (other), `packaging/`, `tests/`, `matrix/`, `guest-module/nvrm_nodes` | none | Unchanged | Nothing to port | |
| probes, `scripts/` | n/a | Not in this tree. The only changes are in `../Leandro-Test` (below) | | |

### 2.2 The Leandro-Test companion

- It has no remote. `refactor-thesis` is 2 commits ahead of its `main`: `e046e04` ("Own the
  hardware acceptance harness and pin a tested core baseline") and `3edea73` ("Accept the guest
  GPU name in either format"). `3edea73` is the counterpart of `6233ab5`. Without it the v1 gpu
  gate compares against the legacy name and fails, loudly, once Leandro carries the P17 default.
- Its flake pins Leandro by GitHub rev `e4134de`
  (`../Leandro-Test/flake.nix:13`), two commits before `e2f1aec`. It takes `DRIVER_VERSION`,
  `mkGuestImage`, `cloud-hypervisor`, `host-deb` and `guest-deb` from that pin
  (`flake.nix:28-86`). The 615 guest images therefore follow the Leandro pin automatically.
- Its working tree has 9 modified files (`README.md`, `flake.nix`, `local.env.example`,
  `nix/agent-settings.nix`, `nix/host.nix`, `nix/rig.nix`, `scripts/build.sh`,
  `scripts/lib/rig.sh`, `scripts/test.sh`) and an untracked `probe/python/input_device.py`. The
  operator must commit or park these before the lock is bumped.

### 2.3 Caraxes files that drive v1 (they stay in Caraxes and need adaptation)

| File | Coupling today | After the split |
|---|---|---|
| `caraxes/tools/bench-v1vm.sh:21-25` | `CH=$ROOT/vendor/cloud-hypervisor/...`, `NVRM=$ROOT/target/bench/release/vhost-user-nvrm` "built from this tree". `modules` copies `guest-module/{nvrm_nodes,virtio_nvrm}` from this tree. The image paths are hard-coded to `/mnt/vmstore/leandro/vm/...610.57.04...qcow2` | `LEANDRO=${LEANDRO:-$ROOT/../Leandro}` for the binary and the guest modules. `CH` defaults to Leandro's CH, or to Caraxes' CH if that one includes 0001-0003. `BASE`/`IMG` become variables (the 615 image has a new name) |
| `caraxes/tools/bench-run.sh:34,36` | `LT=../Leandro-Test`, `HOSTPY=../Leandro/vendor/hostvenv` | Unchanged (the paths exist). Document them |
| `tools/caraxes-vm.sh:58-59` | `LEA_VM_DIR`, `LEA_CH` default to `/mnt/vmstore/leandro/...` | `CARAXES_CH` defaults to Caraxes' own CH build (with 0005), then falls back |
| `tools/caraxes-vm.sh:327-332` | `tracer=$LEA_ROOT/target/release/libnvrm_trace.so`. If it is missing the script prints a note and continues | After the split `nvrm-trace` is not a Caraxes member, so the default path goes stale. Use `CARAXES_TRACER`, defaulting to `../Leandro/target/release/libnvrm_trace.so` (built at the same pin) |
| `tools/caraxes-gate.sh:411` | `LEA_HOSTVENV` defaults to `$LEA_TEST/vendor/hostvenv`, which does not exist | Default to `../Leandro/vendor/hostvenv`, as `bench-run.sh` already does |
| `caraxes/tools/lab-abi-check.sh:44` | `$root/vendor/nvidia-rm-headers` | Resolve it through the pinned git checkout (section 4, step C2) |

## 3. The shared-crate question

| | (A) Shared crates stay in Caraxes; Leandro depends on Caraxes by git | (B) Shared crates live in Leandro; Caraxes depends on Leandro by git, pinned | (C) A third repository (shared crates, `vendor/nvidia-rm-headers`, xtask; probes stay in Leandro-Test); both depend on it |
|---|---|---|---|
| Building | Leandro's build needs the Caraxes remote. That remote is private and SSH-only, so cargo needs credentials and clones the whole Caraxes history (97 MB `.git`, 523 MB of tracked runs). | Leandro is unchanged: path dependencies, offline-capable. Caraxes clones a 7.5 MB public repository once into `~/.cargo/git` and builds offline afterwards. | Both repositories fetch a third one. Leandro loses its self-contained build. |
| CI: `tools/check.sh` | The `abi`, `nix-sparse`, `kapi-abi` and `class-sizes` steps move to Caraxes. Leandro's check loses its ABI guard. | Leandro's check stays as at `e2f1aec` (all 17 steps). Caraxes gets a smaller check (cargo, licence, shell, markers, and the new cross-repo checks). | Three check scripts. ABI steps in the third repository, and integration checks in both. |
| CI: flake and GitHub Actions | `nix build .#leandro` needs `cargoLock.outputHashes` for a private git source and fails in the sandbox without credentials. GitHub Actions cannot fetch it. | No change to the flake. It is fixed by the split (risk R2). Caraxes has no flake and needs none. | Leandro's `leandro.nix` needs `outputHashes` for the public third repository. That works, but every bump changes the hash. |
| 615 refactor | Done in Caraxes. Leandro bumps its pin, then Leandro-Test bumps its pin. | Done once in Leandro (`DRIVER_VERSION`, `cargo xtask abi`, defaults, flake hashes). Caraxes moves its pin (one line) and does its own device-side `ver/v615`. | Done in the third repository, then pin bumps in Leandro, in Leandro-Test (through Leandro) and in Caraxes. |
| Thesis | The thesis repository (public, Leandro) no longer contains its core crates. Their history and provenance move into a private repository. | The thesis repository keeps its full history and core. Caraxes-motivated changes (naming, `try_clone`) become ordinary reviewed Leandro commits. | The core leaves the thesis repository. History is preserved only as a `git subtree split` copy with new commit ids. |
| Day-to-day cost for Caraxes | Low: edits in place. | A shared-crate change needs a Leandro commit, a push and a pin bump. Local iteration uses a `--config patch` override (section 4). | The same as B, plus a third CI. |
| Work to set up | Moderate: Leandro needs rewiring. | Smallest: Leandro just gains the port-back. Caraxes switches three dependencies and deletes the copies. | Largest: a new repository, subtree splits, rewiring of both. |

**Recommendation: (B).** Leandro is where `nvrm-sys`, `nvrm-abi`, `nvrm-client`, `nvrm-wire`,
`nvrm-trace` and `xtask` were written, measured and published. B leaves that history, the
thesis and the public build untouched. The flake and `check.sh` in Leandro keep working without
change. The recursive `patches/` glob that breaks the nix CH build today (risk R2) is fixed by
the split itself. Caraxes' use of the shared crates is narrow and one-way (naming, `nvgpu`,
`sys`, status helpers, `mem`, `try_clone`, and the trace library as a binary), so a pinned git
dependency costs one line per bump and turns every host-ABI move into an explicit, reviewable
Caraxes commit. The 615 switch then happens once, in Leandro, where the ABI generator and its
check live. Tonight's host upgrade becomes a pin bump in Caraxes and a lock bump in
Leandro-Test instead of three coordinated edits. Options A and C both make the public thesis
repository depend on another repository, and A makes it depend on a private one.

## 4. Mechanics for option B

Rules followed throughout:

- No history is rewritten in either repository. Every move is an ordinary commit.
- No `git filter-repo`. `git subtree split` is not needed for B.
- Caraxes keeps its full history, including the 177 Leandro commits before the fork.
- Each step leaves both repositories building. `Lx` marks a Leandro step and `Cx` a Caraxes step.

### Step 0: freeze and mark (30 min, operator plus one agent)

1. Land or park the in-flight work:
   - Caraxes main has 10 modified and several untracked files (615 `ver/mod.rs`, the mapping
     runs, `docs/caraxes/mapping.md`).
   - Worktrees `agent-abcf6071de2ca99bb` (0006) and `agent-ab0481c855c331602` have uncommitted
     changes, and `worktree-agent-a5d3416bda3c346f5` is 6 commits ahead.
   - `../Leandro-Test` has 9 dirty files.
2. Tag the start points (a tag does not rewrite anything):
   `git -C ../Leandro tag v1-fork-point e2f1aec`
   `git tag pre-split-2026-09-24` (in Caraxes)
3. Give the two checkouts separate cargo target directories (risk R3). For example, point
   Caraxes' `target` at `/mnt/vmstore/caraxes/target`, or set `CARGO_TARGET_DIR` in Caraxes'
   `local.env`.

### Step L1: Leandro receives the port-back (2.5 h)

Create a branch in `../Leandro` from `e2f1aec`. Take the Caraxes commits as **path-limited
patches**, so that no Caraxes object enters the Leandro object store and authorship, dates and
messages survive:

```sh
cd ../Caraxes
P=$(mktemp -d)
n=0
port() { n=$((n+1)); git format-patch -1 --start-number $n -o "$P" "$@"; }
port 72d9da2 -- patches/REVIEW-vhost-user.md
port c8a6f29 -- patches/0001-generic-vhost-user-shmem.patch patches/0002-generic-vhost-user-device-features.patch patches/0003-generic-vhost-user-refused-request.patch nix/packages/cloud-hypervisor.nix
port 678a4e4 -- Cargo.toml Cargo.lock crates/vhost-user-nvrm docs/ARCHITECTURE.md guest-module/virtio_nvrm
port 31ba74c -- patches/README.md
port 79b7d0b -- patches/README.md caraxes/runs/vhost-user-check-2026-09-23
port 176b53e -- crates/nvrm-abi
port 59455f0 -- crates/vhost-user-nvrm
port d45bf4f -- crates/nvrm-abi
port 6233ab5 -- crates/nvrm-abi crates/vhost-user-nvrm
port d025a08 -- crates/vhost-user-nvrm/README.md
port 0a78d41 -- crates/nvrm-abi/src/lib.rs
cd ../Leandro
git switch -c port/caraxes-2026-09 e2f1aec
git am -3 "$P"/*.patch
```

The order is chronological (`git log --reverse`). The per-path commit lists in section 2.1 show
that no other commit touched these paths after the fork, so the series applies in order onto
`e2f1aec`.

Then add three Leandro-authored commits:

- (a) `git mv caraxes/runs/vhost-user-check-2026-09-23 docs/measurements/vhost-user-check-2026-09-23`
  and relink `patches/README.md:642`.
- (b) The P17 naming spec goes into `docs/NAMING.md` (condensed from
  `docs/caraxes/briefing-naming.md`), and `crates/nvrm-abi/src/naming.rs:5` points there.
- (c) A merge commit, or a note, that lists the Caraxes-to-Leandro commit id mapping.

Verification (all offline, no GPU):

- `git diff ../Caraxes-main -- crates/nvrm-* crates/vhost-user-nvrm crates/xtask` must show only
  the naming comment. Run the diff as `git diff --no-index` between the two checkouts' directories.
- `tools/check.sh` must pass.
- `nix build .#cloud-hypervisor .#leandro` must succeed. This proves that `cargoHash` matches
  0001-0003.
- `nix flake check` must succeed.

Merge into Leandro `main` and push to GitHub. That commit is **L610**.

**Step L1 done (2026-09-24, 12:50):** branch `port/caraxes-2026-09` in `../Leandro`, 14 commits on
e2f1aec, local only; crate diff against Caraxes shows only the naming comment; Leandro's
`tools/check.sh` 17 of 17, `nix build .#cloud-hypervisor .#leandro` and `nix flake check` pass;
the mapping is in Leandro's `docs/measurements/portback-2026-09-24.md`. Corrections to the
recipe above, found while running it: `git am -3 --keep-cr` (plain `git am` strips the CRLF
endings of the four hypervisor logs in the evidence run); `docs/NAMING.md` already exists in
Leandro, the naming section was added to it; the README link is line 360 there, not 642;
check.sh's C scratch steps write under `$LEA_ROOT/target` regardless of `CARGO_TARGET_DIR`
(risk R3 is only partly covered by the variable); the Leandro checkout has no git identity, every
commit needs the `-c` flags; the three path-limited commits keep their original messages, which
name Caraxes content that is not in them (the mapping file says so). The merge into Leandro
`main` and the push are the operator's (option B confirmed 2026-09-24; Caraxes pushes nothing
to Leandro).

### Step T1: Leandro-Test follows (45 min, needs the operator's network steps)

1. Commit the dirty tree.
2. Merge `refactor-thesis` into `main`. It carries `3edea73`, which the P17 default needs.
3. Bump the flake input: set `leandro.url` to `github:UniStuttgart-IKR/Leandro/<L610>`, then run
   `nix flake lock --update-input leandro`. This fetches from GitHub, so the operator runs it.
4. Run `nix flake check`.
5. Leandro-Test keeps having no remote. Record the bump in its README.

### Step C1: Caraxes consumes the shared crates from Leandro (1 h)

Caraxes' `Cargo.toml` after this step:

```toml
[workspace]
resolver = "2"
members = ["crates/vfio-user-nvgpu", "crates/dp-sink"]

[workspace.dependencies]
libc = "0.2"
anyhow = "1"
thiserror = "2"
# The shared RM crates, pinned to a Leandro commit (docs/caraxes/repo-split.md).
nvrm-sys    = { git = "https://github.com/UniStuttgart-IKR/Leandro", rev = "<L610>" }
nvrm-abi    = { git = "https://github.com/UniStuttgart-IKR/Leandro", rev = "<L610>" }
nvrm-client = { git = "https://github.com/UniStuttgart-IKR/Leandro", rev = "<L610>" }

[patch.crates-io]
vfio_user = { path = "vendor/vfio-user-rs" }
```

Other changes in the same commit:

- `crates/vfio-user-nvgpu/Cargo.toml:24` becomes `nvrm-client.workspace = true`.
- `git rm -r` the shared crates (`nvrm-sys`, `nvrm-abi`, `nvrm-client`, `nvrm-wire`,
  `nvrm-trace`, `xtask`) and `vhost-user-nvrm`, which is v1.
- Remove `.cargo/config.toml` (the xtask alias).
- Regenerate `Cargo.lock` with `cargo generate-lockfile`. This needs network once, so the
  operator runs it.
- Trim `tools/check.sh` in the same commit:
  - Drop `nvrm-genhdr`, `abi`, `nix-sparse`, `class-sizes`, `kapi-abi`, `c-interpreter` and `edid`.
  - Keep `cargo test`, `test --release`, `test --doc`, `doc`, `clippy` (both), `bash -n`,
    `licence`, `dangling-refs` and `no-markers`.
  - Add `--exclude nvrm-sys`, which is no longer a member.

At L610 the build is byte-for-byte the same code as today, which makes this a pure refactor. The
check is `cargo test --locked --workspace`, which runs the device's 97 tests
(`patches/README.md:508`).

For local iteration on shared crates without pushing, use a local override:
`cargo --config 'patch."https://github.com/UniStuttgart-IKR/Leandro".nvrm-abi.path="../Leandro/crates/nvrm-abi"' ...`,
with one entry per crate, wrapped in a small `tools/caraxes-dev-leandro.sh`. It changes
`Cargo.lock`, so the rule is to never commit a lock written under the override (checklist item
K5).

### Step C2: Caraxes removes the rest of v1 and becomes self-standing (3.5 h)

These are ordinary `git rm` and `git mv` commits, one group each:

1. **v1 paths out:**
   - `guest-module/nvrm_nodes`, `guest-module/virtio_nvrm`
   - `matrix/`, `nix/`, `flake.nix`, `flake.lock`, `packaging/`, `tests/`
   - `docs/*` except `docs/caraxes/`, `.coderabbit.yaml`, `.github/workflows/check.yml`
   - `tools/ci/check-c.sh` (it tests only `virtio_nvrm`), `DEVELOPMENT.md`
   - `patches/0001-0003`, `patches/REVIEW-vhost-user.md`
   - `caraxes/runs/vhost-user-check-2026-09-23`: keep it, or leave a README pointing at Leandro.
     History keeps it either way.
   - `vendor/nvidia-rm-headers`
2. **Keep the `docs/caraxes/` path** rather than promoting it to `docs/`. About 60 internal
   relative links assume it. Write a short root `README.md` that points at
   `docs/caraxes/README.md`.
3. **Patches:** `patches/README.md` keeps the 0004/0005 (and 0006) sections. The 0001-0003
   specification moves out, with a one-line pointer to Leandro's `patches/README.md` at the
   pinned rev.
4. **Build script:** add `tools/caraxes-build.sh` with three commands:
   - `vendor VERSION`: the full NVIDIA tree into `vendor/open-gpu-kernel-modules-<VERSION>`,
     with `vendor/open-gpu-kernel-modules` as a link to the default. Copy the function from
     Leandro's `do_vendor`.
   - `ch`: `CH_VERSION`, then Caraxes' patches 0004-0006. With `CARAXES_CH_WITH_V1=1` it first
     applies `../Leandro/patches/000[1-3]`, which gives one binary for the P13 comparison. The
     script writes `cloud-hypervisor.patches`, a sidecar listing the applied series and its
     sha256 (risk R4).
   - `cargo`.
5. **Headers:** `caraxes/tools/lab-abi-check.sh` stops using `vendor/nvidia-rm-headers` and
   resolves the headers from the pinned checkout instead:
   `$(dirname "$(cargo metadata --format-version 1 | jq -r '.packages[]|select(.name=="nvrm-sys").manifest_path')")/../../vendor/nvidia-rm-headers`.
   This guarantees the headers are the pin's.
6. **Paths:** fix the couplings listed in section 2.3.
7. **Links:** relink the 7 markdown links from `docs/caraxes` into v1 docs (`ARCHITECTURE.md`,
   `TESTING.md` twice, `VIRTIO-UAPI.md` three times, `measurements/sottr-4q`,
   `matrix/PROBES.md`) to `https://github.com/UniStuttgart-IKR/Leandro/blob/<L610>/...`.
   The about 70 plain-text path mentions in historical reports (`leandro-reuse.md` 16,
   `testing.md` 10, `cuda-path.md` 6, and others) stay as they are, with one note in
   `docs/caraxes/README.md` saying that they refer to Leandro at `e2f1aec`.
8. **Licences:** `LICENSES.md` for Caraxes adds `vendor/vfio-user-rs` (Apache-2.0 OR
   BSD-3-Clause, `vendor/vfio-user-rs/LICENSE-*`), `patches/linux/` (GPL-2.0) and
   `guest-module/caraxes_dma_range` (GPL-2.0-only). Copy `LICENSE`, `LICENSES/` and
   `guest-module/LICENSE`.
9. **`.gitignore`:** drop the v1 rules that no longer apply (`/probe/*`, `/matrix/*`,
   `/vendor/vhost-device`) and keep the Caraxes block. Delete the local leftovers `probe/`,
   `jobs/`, `graphify-out/` and `result` from the Caraxes checkout. They are ignored, so this
   is not a commit.
10. **Driver version:** add a test that ties Caraxes' `DRIVER_VERSION` file to
    `nvrm_sys::DRIVER_VERSION` of the pin (risk R6).
11. **Branch refs:** delete the stale branch refs that came from Leandro (`lib-matrix`, `slim`,
    `vram`, `vram-grid`, `refactor-thesis`) with `git branch -D`. That removes refs, not history.

Push Caraxes after each group ("push regularly").

### Step L2: the 615 switch in Leandro (3 h plus the operator's hash fetches)

This happens on Leandro `main` after L610 and follows D19's order for the shared side:

1. `DRIVER_VERSION` becomes `615.71.09`, then run `cargo xtask abi`. This regenerates the
   `nvrm-sys` default through `emit.rs:113`. The version is already in `abi.toml`, so nothing
   new is vendored.
2. Hand-set defaults: `default = ["v615"]` in `nvrm-abi`, `nvrm-client`, `nvrm-trace` and
   `vhost-user-nvrm` (all `Cargo.toml:18-26`). `vhost-user-nvrm` keeps `v610` as an additional
   feature for the transition. `detect()` (`crates/nvrm-sys/src/version.rs:48`) selects at
   start-up, so a binary built with both serves either host driver.
3. `cargo run --bin nvrm-genhdr` regenerates `guest-module/virtio_nvrm/nvrm_wire.h`, whose
   `NVRM_DRIVER_VERSION` is currently `"610.57.04"` (`nvrm_wire.h:21`).
4. `tools/build.sh vendor` fetches `vendor/open-gpu-kernel-modules` at 615, over the network,
   so the operator runs it.
5. Run `class-sizes` and `kapi-abi` against 615. `nvrm_kapi.h` mirrors a kernel-open header,
   and if 615 moved it, the check says so.
6. Update the flake: `openModulesHash` (`flake.nix:33`) and the sparse-checkout hash in
   `nix/packages/nvidia-headers.nix`. The operator runs `nix store prefetch-file`, which needs
   network. Then run `nix flake check`.
7. `matrix/` stays a 610 record. A 615 matrix is a later measurement.

Call that commit **L615** and push it.

Before tonight, build two sets of host binaries and keep them side by side: L610 and L615.
Caraxes' device must also be built at both pins (C3 below).

**Step L2 prepared (2026-09-24, 15:45):** local branch `port/615` in `../Leandro` on top of the
port-back branch, five commits: the 615 switch made by `cargo xtask abi` (DRIVER_VERSION
615.71.09, default feature v615 in the shared crates and vhost-user-nvrm, `nvrm_wire.h`
regenerated with only the version string changed, so the guest module needs a rebuild and no
source change); the flake hashes for the 615 tag computed offline by the same NAR method that
reproduces the committed 610 hashes (to be confirmed by one `nix store prefetch-file` from
GitHub); packaging with a `@BRANCH@` placeholder for the 615 package names; docs; the record
`docs/measurements/l615-2026-09-24.md`. Passing offline: `tools/check.sh` 17 of 17 against the
615 vendor tree, 393 cargo tests, `cargo xtask abi --check`, `nix flake check --offline`, the
leandro nix build with an impure header override, the two guest modules against the flake's
kernel. Not run: `nix build .#guest-modules` (kernel closure not in the store). Deviations: a
vhost-user-nvrm that serves both drivers needs `--features v610` (the generator owns the
feature block); Leandro's untracked vendor tree is now the 615 clone and the 610 tree sits at
`/mnt/vmstore/leandro/ogkm-610.57.04`, so `check.sh` on L610 reads the 615 tree until L615 is
merged (the record has the one-line `mv` back). Host binaries for tonight:
`/mnt/vmstore/leandro/l615-host-bin-8c0d2b2/` (615-only and dual-ABI vhost-user-nvrm,
`libnvrm_trace.so`). The operator merges L610 then `port/615`, pushes after each, confirms the
hashes, builds the nix packages, runs the verified 615 matrix on the card, and bakes the v1
615 image (step G).

### Step C3: Caraxes at L615 (1 h for the pin; the device's `ver/v615` is separate work)

1. Bump `rev = "<L615>"`, set `DRIVER_VERSION` to 615.71.09, and add
   `v615 = ["nvrm-sys/v615"]` to `vfio-user-nvgpu`.
2. The host side (the `nvrm-client` session and its `assert_driver_version`) now expects 615.
   The guest side still emulates `ver/v610`: the host ABI and the guest ABI are independent
   axes (D14, D19). The tests that pin 610 layouts reference `nvrm_sys::v610` explicitly
   (`src/ctrl.rs:1067`, `src/kgr.rs:571`, `src/nvml.rs:377`) and keep passing, because the
   `v610` feature stays on.
3. `cargo test` then shows every place where guest-facing code uses the default-version
   re-export instead of `v610`. This is the host/guest separation D19 asks for. Fix those in
   this step.
4. Keep this commit on a branch until the window. It is the Caraxes side of the cut-over.

### Step G: guest images and the `_DMA` pieces on 615 (2 h of work plus bake time, before the window)

- **v1:** Leandro-Test bumps its pin to L615, then runs `nix build .#guest-image`. The image
  takes `DRIVER_VERSION` and the guest NVKMS from the pin.
- **Ubuntu baked images** (used by v1 benchmarks and by Caraxes via `LEA_BASE_IMAGE`): bake a
  new image with the 615 userspace and a new name. The script paths in section 2.3 must stop
  hard-coding the 610 file name.
- **`_DMA` pieces:**
  - `caraxes_dma_range` has no NVIDIA-version dependency. The source mentions `nvidia` only in
    comments and messages (`caraxes_dma_range.c:6,516`). It needs a rebuild against the guest
    kernel of the new image, not a rebase.
  - `patches/0004` applies to Cloud Hypervisor `v53.0`, which is unchanged.
  - `patches/linux/0001` applies to the guest kernel (Ubuntu 6.8.0-136), which is also
    unchanged.
  - So nothing in the `_DMA` series is rebased for 615. Run the module's test
    (`caraxes_dma_range_test.ko`) once in the new image.

### Step W: tonight's window (operator; agents do not touch host GPU state)

1. Stop every VM, both v1 and Caraxes.
2. The operator upgrades the host driver to 615.71.09 and reboots.
3. Run `tools/build.sh check-driver` (Leandro). It must report running = target = vendor =
   615.71.09.
4. Install or point to the L615 host binaries: Leandro's `vhost-user-nvrm`, the
   `libnvrm_trace.so` of the pin, and Caraxes' device built at C3.
5. Caraxes: merge the C3 branch, then run `tools/caraxes-gate.sh cuda` on a 610 guest. This
   tests the host side on 615 while the guest side is unchanged.
6. v1: bring up a 615 image, then run the Leandro-Test gates.
7. Rollback: the L610 binaries, the pre-C3 Caraxes commit and the 610 images still exist, and
   the operator downgrades the driver.

The P13 benchmark numbers from before and after the window are not comparable. Record the
driver next to every run.

### Step C4 (after the window)

The device's `ver/v615` (D19 step 1, already in progress), verified with a 615 guest in pure
emulation. Then P23 on the 615 inventory.

### Order and the "both build" invariant

L1 (L610), then T1, C1, C2, L2 (L615), G, C3, W, C4.

- After L1, Caraxes still builds from its in-tree copies.
- After C1, Caraxes builds against L610, the same code as its old in-tree copies.
- L2 does not affect Caraxes until C3 moves the pin.
- Leandro-Test moves only when its lock is bumped.

If time before tonight is short, the minimum is L1, C1, L2, C3 (about 7.5 h). C2 and T1/G for
v1 can follow tomorrow, and v1 stays down until its 615 image exists.

| Step | Estimate | Who |
|---|---|---|
| 0 freeze and tags | 0.5 h | operator plus agent |
| L1 port-back, checks, push | 2.5 h | agent (push: operator) |
| T1 Leandro-Test merge and lock | 0.75 h | operator (network) plus agent |
| C1 git dependencies | 1 h | agent (lockfile fetch: operator) |
| C2 removal, build script, relinks, licences | 3.5 h | agent |
| L2 615 switch in Leandro | 3 h plus hash fetches | agent plus operator |
| G guest images | 2 h plus bake | agent plus operator |
| C3 Caraxes pin to L615 | 1 h | agent |
| W window | about 1 h | operator |

## 5. Risks and the checklist

| # | Risk | Breaks silently because | Caught by |
|---|---|---|---|
| R1 | **The shared CH binary loses or gains patches.** `tools/build.sh ch` globs `patches/[0-9]*-*.patch` (`:153`), so today one checkout applies 0001-0005. After the split, Leandro's CH has 0001-0003 and Caraxes' has 0004-0006. The Caraxes launcher's default `LEA_CH` (`local.env`, `tools/caraxes-vm.sh:59`) is an August build with neither the rewritten series nor 0005. | The device falls back cleanly when 0005 is missing: "a server that refuses keeps every BAR trapped" (`patches/README.md:20`). The doorbell ioeventfd and P21 measurements just get slower, with no error. | K3, and the `cloud-hypervisor.patches` sidecar (C2 item 4) checked by the launcher |
| R2 | **The nix CH build in Caraxes is already wrong.** `nix/packages/cloud-hypervisor.nix:27-28` feeds `listFilesRecursive patchDir` (all of `patches/`, including `linux/0001` and `vfio-user-rs/0001`) into `cargoPatches`, against a hash computed for 0001-0003. `nix/packages/leandro.nix:21-25` also omits `vendor/vfio-user-rs`, which `Cargo.toml:44` patches in. | `nix flake check` only evaluates drvPaths, so neither failure shows until someone runs `nix build`. | K2 in Leandro after L1 |
| R3 | **One cargo target directory for both repositories.** Both `target` links resolve to `/mnt/vmstore/leandro/build-refactor-thesis/target`. The last build wins for `target/release/libnvrm_trace.so`, which `tools/caraxes-vm.sh:327` copies into guests, and for `vhost-user-nvrm`. | A Caraxes guest can receive a tracer built for another driver version or another pin. | Step 0 item 3, K7 |
| R4 | Hard-coded sibling paths: `../Leandro/vendor/hostvenv` (`bench-run.sh:36`), `$LEA_TEST/vendor/hostvenv` (`caraxes-gate.sh:411`, which does not exist today), `target/bench/release/vhost-user-nvrm` "from this tree" (`bench-v1vm.sh:22`), `/mnt/vmstore/leandro/vm/guest-baked-...610.57.04...` (`bench-v1vm.sh:24`, `local.env`) | The scripts print a note, or use the old 610 image | K6 |
| R5 | Docs links across the split: 7 markdown links from `docs/caraxes` into v1 docs, `patches/README.md:642` into `caraxes/runs`, `docs/README.md:34` into `docs/caraxes`, and `naming.rs:5` as plain text | `check.sh` has no link checker. `cargo doc -D warnings` checks rustdoc links only. | K4 |
| R6 | `DRIVER_VERSION` disagreement: Caraxes' file (guest payload) and `nvrm_sys::DRIVER_VERSION` of the pin (host ABI) can differ after a pin bump | The guest payload is labelled with one version while the host speaks another | The C2 item 10 test, K5 |
| R7 | The licence check: the `guest-module/* => GPL-2.0-only` rule (`tools/check.sh:311`) and the `caraxes/runs/` exemption (`:319`) must survive in Caraxes' copy. Leandro must not carry the exemption. `LICENSES.md` omits vfio-user-rs and `patches/linux` | A missing rule passes, and a missing listing is not checked at all | K1 in both, plus a manual read of `LICENSES.md` |
| R8 | The P17 default reaches v1 without `3edea73` in the Leandro-Test ref that runs the gates | This one is loud (the gpu gate fails), but only on hardware | T1 before the first v1 gate |
| R9 | In-flight agent work: `patches/0006`, `vfio-user-rs/0002`, the display branch (6 commits) and the 615 `ver/mod.rs`. A later merge of a branch that edited a shared crate would hit a modify/delete conflict, and resolving it by keeping the deleted file would re-add a dead copy. | A dead copy of a crate that is not a member builds nothing and is never checked | K8 |
| R10 | The Repowise index (`.claude/CLAUDE.md`, `.repowise/`) describes Leandro at `a5389d5`, with paths absent from both trees. After the split it describes neither repository. | Agents that "trust the index" read a map of files that do not exist | Re-index both checkouts after C2, and fix the "Leandro" name in Caraxes' copy |
| R11 | A local `--config patch` override writes a path source into `Cargo.lock` | A commit then pins a local path that other clones cannot resolve | K5 |

The three largest are R1, R2 and R3: each changes behaviour or build output without an error
message.

### Checklist (run after each step; all offline unless noted, none of it touches the GPU)

```sh
# K1  Both repositories: the full software check.
(cd ../Leandro && tools/check.sh) && tools/check.sh
# K2  Leandro: nix builds, not only evaluation (R2).
(cd ../Leandro && nix build .#cloud-hypervisor .#leandro .#guest-modules && nix flake check)
# K3  CH patch series per repository (R1): what applies and what the binary carries.
(cd ../Leandro && ls patches/[0-9]*-*.patch)      # expect 0001-0003 only
ls patches/[0-9]*-*.patch                          # expect 0004-0006 only
cat "$CARAXES_CH_DIR/cloud-hypervisor.patches"     # sidecar from tools/caraxes-build.sh ch
git -C vendor/cloud-hypervisor apply --check ../../patches/0004-*.patch ../../patches/0005-*.patch  # on a clean v53.0 tree
# K4  Relative markdown links that do not resolve (R5), in both repositories.
git ls-files '*.md' | while read -r f; do
  grep -oE '\]\([^)#: ]+' "$f" | sed 's/^](//' | while read -r l; do
    [ -e "$(dirname "$f")/$l" ] || echo "$f -> $l"; done; done
git grep -nE 'docs/caraxes|caraxes/runs' -- ':!docs/caraxes' ':!caraxes'  # in Leandro: expect nothing
# K5  The pin, the lock and the driver version agree (R6, R11).
git diff --exit-code Cargo.lock
grep -c 'source = "path+' Cargo.lock                # expect 0 (vfio_user is a [patch], not a path source)
cargo metadata --format-version 1 | jq -r '.packages[]|select(.name=="nvrm-sys")|.id'  # shows the rev
cargo test -p vfio-user-nvgpu driver_version       # the C2 item 10 test
# K6  Sibling and absolute paths in Caraxes scripts (R4).
git grep -nE '\.\./Leandro|/mnt/vmstore/leandro|target/(bench/)?release/(vhost-user-nvrm|libnvrm_trace)' -- tools caraxes/tools
# K7  Separate target directories (R3).
readlink -f target ../Leandro/target                # must differ
# K8  No shared crate re-added by a merge (R9).
git ls-files crates | cut -d/ -f2 | sort -u         # expect dp-sink, vfio-user-nvgpu
git -C ../Leandro ls-files crates | cut -d/ -f2 | sort -u
# K9  Nothing of Caraxes in Leandro, nothing of v1 left in Caraxes.
git -C ../Leandro grep -liE 'vfio-user-nvgpu|caraxes' -- ':!crates/nvrm-abi/src/naming.rs' ':!docs/NAMING.md'
git ls-files | grep -E '^(nix|packaging|matrix|tests)/|^guest-module/(nvrm_nodes|virtio_nvrm)/|^flake\.'
```
