<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# One device binary for every guest ABI (2026-09-28)

Scope: what chose the guest ABI at compile time in `crates/vfio-user-nvgpu`; which layouts follow the host driver and which follow the guest; how one binary now serves nova-core (570), the 615 Linux guest and the 617 Windows guest (615 model) and picks the ABI from what the guest shows at its GSP boot. Branch `oneabi-2026-09-28` (from dev 7f1143be). Background: [guest-driver-versions.md §6](../vfio/guest-driver-versions.md) (the earlier plan), [cutover-615.md](../history/caraxes-cutover-615-2026-09-24.md) (host side), decision D14.

## Facts

### What was compile time

- The crate feature `v615` (implies `v610`) made `ver::active` an alias of `ver::v615`, else of `ver::v610` [crates/vfio-user-nvgpu/Cargo.toml, src/ver/mod.rs at 7f1143be].
- 35 files imported numbers through `ver::active`; 45 of those numbers differ between 610 and 615, exactly the items `ver/v615.rs` overrides over its `pub use super::v610::*` [src/ver/v615.rs, src/ver/v615/display.rs]:
  - `DRIVER_VERSION`, `GSP_TRANSPORT`, the element header (16 or 24 bytes) and `MAX_RPC_SIZE`, the four RMARGS queue offsets;
  - `CTRL_0A36` (chip info in 610, fabric-probe sync in 615), `FIFO_GET_DEVICE_INFO_TABLE` (3212 or 3340 bytes), `GET_PCIE_P2P_CAPS` offsets;
  - the KGR floorsweeping element (1336 or 1356 bytes) and the SM-order element (nine or ten u16 per SM);
  - the TSG allocation (20 or 28 bytes, `internalFlags` in 615) through `RmAbi`;
  - five NV0073 display items (`GET_ACTIVE` 16 or 20 bytes, `DP_SET_WAR_FLAGS` only in 615).
- The generated control tables (`surface`, `ctrl_layouts`, `dmodel`) were already compiled for both ABIs; `Abi::active()` picked one by `active::DRIVER_VERSION`.
- The chip data resolved its category E and A tables (`chipctl.rs`) against `Abi::active()` when the chip loaded [src/chip/mod.rs].
- `check_guest_abi` refused 615 in a default build and 570/595 in a `v615` build (the "guest ABI 570 needs a 610 build" of the night gate) [caraxes/runs/night-2026-09-28/README.md "Stability gate"].
- Three test gates on the feature: `kgr.rs` (`by_abi`), `boot.rs` (the nova test only without `v615`), `chip/boot_fsp/tests.rs` (GB206 bootstrap ignored with `v615`).

### What was already run time

- The CPU-GSP transport: `BootModel.transport` with `with_transport`; message queue v0, v2 and the GMCAPI init are compiled into every build, and the tests drove the v2 path in the default build [src/boot.rs, src/msgq.rs, src/msgq/v2.rs, src/gmcapi.rs].
- The pre-610 guests: `CARAXES_GUEST_ABI=570|595` switched the element frame, the link events (570: GSP_RUN_CPU_SEQUENCER first), the static info layout and the registry-key warnings [src/ver/mod.rs `older_guest`].
- The bindings: the crate enabled `nvrm-sys` features `v610` and `v615` in every build [crates/vfio-user-nvgpu/Cargo.toml].

### Host side and guest side

| Axis | Chosen from | What it fixes | Where |
|---|---|---|---|
| Host | the running host driver (`/proc/driver/nvidia/version`) or `CARAXES_HOST_ABI`, at start | TSG alloc 20/28, `GPU_GET_PES_INFO` 56/76, public `GR_GET_GLOBAL_SM_ORDER` 9240/10264, GSS legacy sizes, `FB_FLUSH_GPU_CACHE` flag bits, `CE_GET_LCE_SHIM_INFO` privilege, `PERF_BOOST` flags | src/hostabi.rs |
| Guest | the guest's GSP boot (this change); was the build | transport, element frame, RMARGS, static info (struct or NVKV), the 45 items above, the generated tables, the chip's E/A tables | src/ver, src/boot.rs |

- The host side was already correct for this 615.71.09 host in both builds: `hostabi::select` picks `HOST_615` from the running driver whatever the guest ABI [src/hostabi.rs `select`, `current`].
- So the default build did not work "by accident": its host calls used 615 layouts, and only its guest side was 610 (right for nova's and a 610 guest's controls).
- The two meet in two places, both translated field by field for all four combinations: the PES map (the shorter of the two maps is copied) and the SM order (`translate_sm_order`) [src/kgr.rs `floorsweeping`, `global_sm_order`].
- `CARAXES_DRIVER` never reached the device: it selects the guest image and userspace in the launcher [tools/lib/driver.sh].

### What changed

- `src/ver/common.rs`: the 610 vocabulary every guest ABI shares; each of the 45 per-guest items is shadowed there by a private stand-in, so reading one through `common` is a compile error, not a silent 610 number for a 615 guest. The test `the_shared_vocabulary_hides_every_615_override` keeps the list equal to the overrides of `v615.rs` and `v615/display.rs`.
- `ver::GuestAbi` (570, 595, 610, 615) with `transport()`, and `GuestLayouts` tables `L610`/`L615` built from each version module by one macro; generic code reads `ver::layouts()` (ctrl.rs, kgr.rs, display/mod.rs, channel.rs).
- `ver::guest()`: the detected ABI, else the pin, else 610; process-wide (one device process serves one guest), a thread-local in unit tests.
- `ver::PerAbi<T>`: the chip's E and A tables are resolved for both ABIs at load and read for the guest in force (`ChipData::features()`, `controls()`).
- `BootModel::detect_guest`, before any queue link (first doorbell or Booter Load), when nothing is pinned:
  - RMARGS (`GSP_ARGUMENTS_CACHED`) opening with the magic "GSP ": 615, message queue v2 and the GMCAPI init (615 Linux, 617 Windows);
  - without it: message queue v0, provisionally 610; at the link, the first command element's frame decides (`ver::element_frame`: the "VRPC" v3.0 header at 0x10 is 610, at 0x30 pre-610, served as 570);
  - RMARGS not found or not mapped: not linked, noted, decided at the next doorbell or Booter Load.
- `--guest-abi` / `CARAXES_GUEST_ABI` stay as an optional pin (570, 595, 610, 615): the transport is then fixed, and a guest that shows another ABI gets a `guest ABI: ERROR` note (fail closed, as before). 595 is pin-only: its frame equals 570's.
- Cargo features `v610`/`v615` are no-ops kept for old command lines; `tools/caraxes-vm.sh` no longer switches to `target/v615`, `tools/emu-boot-check.sh` builds one binary, `tools/lib/driver.sh` no longer exports `CARAXES_GUEST_ABI=615`.
- Size: 51 files, +1.6k/-0.6k lines (with this note and the tests), much of it the mechanical `active` to `common` rename; no change to the RPC core.

### Tests (one build)

- `cargo test -p vfio-user-nvgpu`: 678 passed, 1 ignored (the existing bench), with default and with `--all-features`; clippy clean with and without `--all-features` (tools/check.sh flags).
- Per guest, in one test binary:
  - `boot::nova_570_boot_through_the_queue`: nova's exchange once chosen by the guest (RMARGS without magic, pre-610 frame) and once pinned 570;
  - `boot::a_610_guest_is_chosen_from_its_first_element`: 610 frame, GSP_INIT_DONE alone, registry keys checked, 0x640-byte static info;
  - `boot::the_615_guest_is_chosen_from_its_rmargs`: the captured 615 guest links v2 at Booter Load, 0x20800A36 answered as the fabric-probe echo;
  - `boot::one_binary_serves_nova_610_and_615_in_turn`: 570, 610, 615, 570 in one process;
  - `Guest615::boot` (every 615 queue test) now boots through the detection, not a fixed transport.
- Layout tests run both tables: `kgr` (floorsweeping, SM order, all four guest/host combinations), `display` (GET_ACTIVE, DP_CAPS, WAR_FLAGS), `ctrl` (0x20800A36 for 570, 610, 615), `recorded`, `mirror` (TSG id), `chipctl::chip_tables_follow_the_guest_abi`.

## Gate (no VM was started for this note)

- Binary: `/mnt/vmstore/caraxes/vfio-user-nvgpu-oneabi-d7157f5a`, the release build of d7157f5a, sha256 `5993304b8d7cf75f61ac880912dfbae6b567089998a99033efba2274611d9360`; `<binary>` below.
- Each device log names the choice: `grep 'guest ABI' gpu.log` shows "guest ABI 570 ... framed Pre610", "guest ABI 610 ... framed V610" or "guest ABI 615 ... magic "GSP "" (the trace has the same as notes).
- nova (570), unpinned: `caraxes/runs/nova-2026-09-25/nova-boot.sh <dir> CARAXES_SERVER=<binary> CARAXES_KERNEL=/mnt/vmstore/caraxes/nova-kernel/build/arch/x86/boot/bzImage CARAXES_VBIOS=/mnt/vmstore/caraxes/night-2026-09-28/synth-vbios-nova.bin CARAXES_GUEST_ABI=` (the empty value lifts the script's 570 pin).
- Linux 615 (lean VM, Linux B): the usual runs with `CARAXES_SERVER=<binary>`; nothing else. With dev's `tools/lib/driver.sh` the launcher still pins 615, which the binary accepts; with this branch's it runs unpinned.
- Windows 617: `WIN_GPU_SERVER=<binary>` with the usual `WIN_GPU_*` settings, `CARAXES_GUEST_ABI` unset.

## Consequence for Leandro VFIO

- One binary replaces the `v610`/`v615` pair and the `target/v615` tree; the guest-ABI knob disappears from the default configuration (D14, the weekend goal of a stock driver without module options).
- A new guest version is a version module, a `GuestAbi` variant, a `guest_layouts!` table, its private stand-ins in `common.rs` and, if its transport or frame is new, a rule in `detect_guest` ([versatility.md](../vfio/versatility.md) "The version module", step 4).

## Open

- Exact version detection: the guest's GSP firmware ELF `.fwversion` at `WprMeta` (guest-driver-versions.md §6 (a)) would tell 570 from 595 and name 617; today the generation is read, not the version.
- A 610 guest on a 615 guest-ABI table is never served: 610 and 615 differ in transport, so the RMARGS magic separates them; a future version with the 615 transport but other control layouts needs the `.fwversion` rule above.
- `cargo doc -D warnings` fails on dev independently of this change (compslice.rs `SPA[35:16]`, private links in mirror.rs and shadow.rs).
