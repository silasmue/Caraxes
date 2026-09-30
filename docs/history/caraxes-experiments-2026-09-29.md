<!-- SPDX-License-Identifier: MIT -->
# Experiment ladder

Rules: one observable failure at a time; every experiment states what it expects to
see and what would falsify the current understanding; record the guest kernel log
and the device's access log with the commit of both trees.

## Safety envelope

The physical GPU is reached only through the host RM API as an ordinary unprivileged
client, the same way `vhost-user-nvrm` reaches it today. None of E0–E4 touches the
host driver, the host GSP, or the card's registers. The exceptions are the two
reference-capture experiments R1 and R3, which need the card unbound from the host
driver and therefore take the desktop down; they are opt-in and need scheduling with
the operator.

Passthrough is off the table for now (operator, 2026-09-23): the agent session itself
runs in VSCode on the desktop this card drives, so R1/R3 would kill the session that
runs them. Before any passthrough: run the session over SSH in tmux from another machine,
log the Wayland session out, and script the unbind/rebind with a timeout and a recovery
path; write that plan into this file first. That plan is R4 (2026-09-29, below, with
`caraxes/tools/passthrough/`), for a weekend run by the operator; nothing of it has run.

Risks by activity:

| Activity | Risk to host | Notes |
|---|---|---|
| Synthetic device in QEMU (E0–E3) | none | software only; the "GPU" is a Rust process |
| Forwarding guest RM RPCs to the host RM (E4+) | same as Leandro today: a misbehaving guest can trigger XIDs, channel resets or VRAM exhaustion; hostile-guest isolation is not established (`docs/SECURITY.md`) | keep the guest trusted; keep `LEA_ADMIN_PRIV` unset so the backend has no `CAP_SYS_ADMIN` |
| Reading host BAR0 registers from userspace | blocked by `CONFIG_IO_STRICT_DEVMEM` while `nvidia` owns the BAR (EPERM/EINVAL) | do not work around it with a kernel module; use R1/R2/R3 |
| Passing the real card to a VM (R1, R3, R4) | the desktop stops; if the guest driver or the VM crashes mid-init, the card may need a function-level reset or a host reboot before `nvidia` rebinds cleanly; FWSEC/Booter run on real hardware exactly as during a normal host boot, so no additional firmware risk | do it from an SSH session, with the Wayland session logged out |
| Writing to GSP/SEC2 falcon registers or WPR on the host | could hang GSP-RM (Xid 119/120), which needs a reboot | never done by this plan |

## E0: identity-only device (the smallest experiment)

- Build: a Rust binary on `vendor/vfio/vfio-user` (`Server` + `ServerBackend`) that
  serves: config space from `caraxes/reference/rtx2070/config-space.txt` (BARs/ROM
  BAR/command writable per PCI rules; PM, MSI, PCIe capabilities kept; extended
  capabilities dropped), BAR0 16 MiB trapped with `BOOT_0/1/42` answered and every
  access logged (`offset, size, value, direction, monotonic time`), BAR1 256 MiB and
  BAR3 32 MiB trapped and logged, BAR5 I/O, ROM region from `rom.bin`, IRQ info
  INTx 1 / MSI 1 / MSI-X 0, reset = no-op. No DMA use.
- Identity: the profile from the same environment Leandro uses (`LEA_VGPU_TYPE=4Q`
  with the launcher's `LEA_VGPU_PROFILE_MIB`/`LEA_VGPU_FB_MIB`, or `LEA_VRAM_PROFILE_MIB`);
  E0 only needs the PCI part, later experiments feed `fb_length` and the name into the
  FB-size register and the static info ([identity-profile.md](../caraxes/identity-profile.md)).
- Run: Cloud Hypervisor (the pinned v53 build) with `--memory size=4G,shared=on
  --user-device socket=/run/caraxes/gpu.sock` and no `--platform iommu`, the Ubuntu
  610.57.04 guest with the open module built in-guest from the vendored tree and
  `gsp_tu10x.bin` installed; `modprobe nvidia NVreg_GpuInitOnProbe=1`. QEMU
  (`-object memory-backend-memfd,share=on`, `-device vfio-user-pci,socket=…,x-no-posted-writes=on`)
  is the fallback client if Cloud Hypervisor's client misbehaves; both speak the same
  protocol to the same server.
- Expect: `lspci` in the guest shows the card; `nvidia.ko` binds; the access log shows
  BOOT_1, BOOT_0, BOOT_42 reads at 0x4, 0x0, 0xA00 (three times), then BAR0 fully mapped,
  then reads at 0x300000… (PROM) and failure "RmInitAdapter failed" with the VBIOS
  error path. Falsifiers: probe refusal (wrong class/IDs/BARs), no PROM reads (then the
  order in [init-map.md](../caraxes/init-map.md) is wrong), a read at an address not in
  [registers-turing.md](../caraxes/registers-turing.md) (then the map is incomplete; add it).
- Also record: which config-space bytes QEMU and the guest write (BAR programming,
  command, MSI), to fix the config model.

### E0 result (2026-09-23): expectation met

Setup: `crates/vfio-user-nvgpu` (release build), `tools/caraxes-vm.sh up` with Cloud
Hypervisor v53.0 as the vfio-user client (`--user-device`, `--memory size=4G,shared=on`),
Ubuntu guest 6.8.0-136-generic with `nvidia.ko` 610.57.04 built in-guest from the vendored
tree, `gsp_tu10x.bin` installed, `modprobe nvidia` with `NVreg_GpuInitOnProbe=1
NVreg_ResmanDebugLevel=0`. Evidence: `caraxes/runs/e0-2026-09-23/` (device trace of the
load window, guest kernel log, versions).

Observed, in order:

1. The guest enumerates `00:05.0 VGA compatible controller: NVIDIA Corporation TU106
   [GeForce RTX 2070] [10de:1f02] (rev a1)`, subsystem `1458:4011`, "PCIe Legacy Endpoint",
   BAR0 16 MiB, BAR1 256 MiB 64-bit prefetchable, BAR3 32 MiB, BAR5 128 B I/O, expansion
   ROM 512 KiB. Cloud Hypervisor walked the capability chain, sized the BARs, installed an
   INTx eventfd and sent two `DMA_MAP`s with fds (0–3 GiB and 4–5 GiB of guest RAM).
2. `nvidia.ko` binds: `NVRM: probing 0x10de 0x1f02, class 0x30000`, `enabling device
   (0000 -> 0003)`, `resizable BAR disabled by regkey`, `Failed to evaluate _PR3 object`,
   `pciPbiReadUuid: Could not acquire pciPbi mutex`, `Starting device`, `RmInitAdapter`,
   `RmSetupRegisters`, `Successfully mapped framebuffer and registers`.
3. The load window contains 50 BAR0 reads, 3 BAR0 writes, 450 config reads, 22 config
   writes and 12 `set_irqs`. BAR0 sequence:
   - `BOOT_1`, `BOOT_0`, `BOOT_42` read in three rounds (probe, start, RmInitAdapter),
     as predicted by [init-map.md](../caraxes/init-map.md); with INTx masked/unmasked around each
     round (eight `set_irqs` INTx `DATA_NONE|ACTION_MASK`), then INTx off and MSI on
     (index 1, eventfd) before RmInitAdapter.
   - Config mirror `NV_PCFG` at 0x88000: reads of 0x7C (PCIe device capabilities),
     0xBD8/0xBE0/0xBF4/0xBFC (extended space, zero here), 0x84 (link capabilities);
     later read-modify-write of 0x80 (device control 0x10291F to 0x10211F: clears bit 11,
     Enable No Snoop).
   - `NV_PFB_NISO_FLUSH_SYSMEM_ADDR` 0x100C10 read, then write 0x228370 (dropped in E0).
   - 0x1478 = `NV_PBUS_SW_SCRATCH(30)` read (the reset-required/drain flags; 0 is fine).
     This offset was missing from the map and is now in [registers-turing.md](../caraxes/registers-turing.md).
   - `NV_PMC_INTR_EN(0)` 0x140 read; `NV_FUSE_STATUS_OPT_DISPLAY` 0x21C04 read.
   - PROM walk (`kernel_gsp_vbios_tu102.c:296-404`): 0x300000 signature, 0x300018 PCIR
     pointer, PCIR at 0x300170 (`PCIR`, length 0x1C, image length 0x76 blocks, code type
     and last-image byte), NPDE at 0x300190 (`NPDE`, revision 0x101, subimage length
     0x76, not last); second image at 0x30EC00 (PCIR at 0x30EC1C: 0x88 blocks, code
     type 3 = EFI, PCI last-image bit set; NPDE at 0x30EC40 says not last); third image
     expected at 0x31FC00: PCIR pointer read returns 0xFFFF (erased flash beyond our
     130048-byte image), signature read at 0x32FBFF invalid, `s_locateExpansionRoms`
     returns 0x25 (`NV_ERR_INVALID_DATA`).
4. Guest log: `kgspExtractVbiosFromRom_TU102: failed to locate expansion ROMs: 0x25`,
   `kgspInitRm_IMPL: failed to extract VBIOS image from ROM: 0x25`, `RmInitAdapter:
   Cannot initialize GSP firmware RM`, `RmInitAdapter failed! (0x62:0x25:2119)`, MSI
   released and INTx re-armed, no `/dev/nvidia*`.

Falsifiers: none triggered. Probe was not refused, PROM reads happened where the init
map says, and exactly one BAR0 offset (0x1478) was outside the register map.

Consequences:

- E1 needs the third ROM image: the NVIDIA extension image (FWSEC) that follows the EFI
  image at 0x1FC00, which the NPDE of the EFI image announces and which the sysfs ROM
  read cannot return (open question Q2). It only has to parse; nothing in it executes here.
- The config mirror is read in the extended space (0xBD8–0xBFC, near the Resizable BAR
  capability); zeros were accepted. Keep zeros until a reason appears.
- The NISO flush address write and the device-control write must be latched from E2 on
  (`kbusVerifyBar2`/flush paths read them back).
- Time: the whole probe-to-failure path took 150 ms of guest time; no timeouts were hit.

## E1: PROM window with a full VBIOS

- Source of the image, in order of preference while passthrough is not possible:
  a synthetic image built by `caraxes/tools/synth-vbios.py` from the captured PCI chain
  plus a constructed third image that satisfies the driver's parsers (specification in
  [vbios-synthesis.md](../caraxes/vbios-synthesis.md)); later the real image from R1 for fidelity.
- Add: BAR0 0x300000–0x3FFFFF serving the image (`--vbios <file>` on the device); fuse
  register 0x2174C answered consistently with the image's signature selection.
- Expect: `kgspExtractVbiosFromRom` and `kgspParseFwsecUcodeFromVbiosImg` succeed
  (visible as the CPU proceeding to the GFW wait: polling 0x110100 for ~2 s, then reading
  0x118128/0x118234), then `NV_ERR_NOT_READY`.

### E1 result (2026-09-23): expectation met, with one correction

Setup: as E0, plus `--vbios caraxes/reference/rtx2070/synth-vbios.bin` (131584 bytes,
generated by `caraxes/tools/synth-vbios.py` from the captured PCI chain; specification in
[vbios-synthesis.md](../caraxes/vbios-synthesis.md)); `tools/caraxes-vm.sh probe e1-synth-vbios`.
Evidence: `caraxes/runs/e1-2026-09-23/`.

Observed:

1. The PROM walk accepted the third image: the driver read the whole image through the
   window, 32930 dword reads from 0x300000 to 0x3201FC (it copies the full `biosSize`),
   then read `NV_FUSE_OPT_SECURE_GSP_DEBUG_DIS` 0x2174C three times (value 0 here), and
   printed the version parsed from the BIT: `VBIOS version 90.06.3D.00.76`.
2. It then entered `kgspWaitForGfwBootOk_TU102`: one read of 0x118128 (progress 0x0) and
   72754 polls of GSP `CPUCTL` 0x110100 over 3.0 s, then
   `gpuWaitForGfwBootComplete_TU102: GSP failed to halt with GFW_BOOT: (progress 0x0)`,
   `failed to wait for GFW boot complete: 0x65`, `RmInitAdapter failed! (0x62:0x65:2119)`.
3. Everything before the PROM walk was identical to E0 (same 22 non-PROM BAR0 accesses;
   the NISO flush write is now latched).

Correction to the prediction: the failure is `NV_ERR_TIMEOUT` (0x65) from the GFW wait,
not `NV_ERR_NOT_READY`; the wait is the first thing after the VBIOS parse and the
booter-image build, so the parse itself produced no visible register traffic beyond the
three fuse reads.

Consequences:

- The synthetic VBIOS is sufficient for the parse (decision D8 holds). Whether the FWSEC
  descriptor it carries also satisfies `kgspExecuteFwsecFrts` is E2's question: the CPU
  will load the generic bootloader into the GSP falcon and expect WPR2 registers afterwards.
- The fuse read 0x2174C returned 0 (bit 0 = 0 means debug mode allowed). Answer 1
  (production) from E2 on, as [vbios-synthesis.md](../caraxes/vbios-synthesis.md) recommends.
- E2 starts with the GFW values (`CPUCTL` HALTED, 0x118128 bit 0, 0x118234 = 0xFF); the
  generic falcon block model exists in `crates/vfio-user-nvgpu/src/falcon.rs` (unit-tested,
  not yet wired into BAR0).

## E2: falcon state machines and boot status registers

- Add: GSP and SEC2 falcon register blocks (reset via `FALCON_ENGINE`, `DMACTL` scrub
  bits, `HWCFG`, `HWCFG2` RISC-V bit, IMEM/DMEM ports with auto-increment and tag
  writes captured into buffers, `BOOTVEC`, `CPUCTL` start → HALTED after a delay,
  `MAILBOX0/1` latches, `RM`, `OS`, `DEBUGINFO` = 0), GFW scratch (0x118128 bit 0,
  0x118234 = 0xFF), `PTIMER` writes accepted, FECS readout 0x409660,
  `LOCAL_MEMORY_RANGE` 0x100CE0 (choose 8 GiB to match the card), display fuse 0x21C04
  (display disabled at first), `WPR2_ADDR_LO/HI` (0 until the GSP falcon "halts" after
  the FWSEC command, then LO = frtsOffset >> 12 taken from the FRTS command found in the
  DMEM image), `VBIOS_SCRATCH(0xE)` = 0, `RISCV_STATUS` active after the second reset,
  `QUEUE_HEAD(0)` doorbell latch.
- Expect: the CPU writes MAILBOX0/1 (libos args PA), then SEC2 MAILBOX0/1 (WPR meta
  PA), then two doorbells; then `GspStatusQueueInit` times out after 4 s. The device
  log must show the two PAs, and the DMA_MAP calls must have delivered fds covering
  them (assert this in the device).

### E2 result (2026-09-23): expectation met

Setup: as E1, plus `--boot-model --fb-mib 4096` (`CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=4096`
for the launcher). The model lives in `crates/vfio-user-nvgpu/src/boot.rs` (registers) and
`falcon.rs` (the two falcon blocks); guest RAM is mapped from the `DMA_MAP` fds
(`guestmem.rs`). Evidence: `caraxes/runs/e2-2026-09-23/` (the automatic load at guest
boot, and probe `e2-flr-2` after a function-level reset).

Observed, in the order of the distinct registers after the PROM walk (probe `e2-flr-2`):

1. Fuse 0x2174C (now 1), GSP `CPUCTL` 0x110100 = HALTED, PLM 0x118128 = 1, GFW progress
   0x118234 = 0xFF: the GFW wait passes at once.
2. `NV_PTIMER_TIME_PRIV_LEVEL_MASK` 0x9430 read, then `PTIMER_TIME_1/0` written. With 0
   in the mask the driver asserts (`timer_gv100.c:56-80`, seen in the automatic load);
   0xFF cures it.
3. FECS mailbox 0x409660, `WPR2_ADDR_HI` = 0, `LOCAL_MEMORY_RANGE` = 0x4A (4 GiB),
   display fuse = 1 (no display), GSP `HWCFG2` bit 10.
4. GSP falcon: `ENGINE` reset, `DMACTL`, `RM` = BOOT_0, FBIF `CTL`/`TRANSCFG(4)`,
   `DMEMC(0)` = AINCW then 21 `DMEMD` words (the bootloader descriptor), `HWCFG` read
   (0x20100: IMEM 0x100 and DMEM 0x100 blocks, our choice), `IMEMC(0)` = block 0xFE with
   AINCW, `IMEMT` 0xFD/0xFE and 128 `IMEMD` words, `BOOTVEC` = 0xFD00, `CPUCTL` = start.
   The model halts at once and reads the FRTS command from the FWSEC data buffer in guest
   RAM (descriptor `dataDmaBase` 0x115817000 in the second run): `frtsRegionOffset4K`
   0xFFE00 → WPR2 lo 0xFFE000, hi 0xFFEFF0. The CPU then reads `VBIOS_SCRATCH(0xE)` 0x1438
   (0) and `WPR2_ADDR_LO` and accepts them.
5. GSP reset again (into RISC-V), `MAILBOX0/1` = libos args PA (0x12AB61000).
6. SEC2 falcon: reset, `RM`, mailboxes read, FBIF, 0x100 + 0x8400 bytes of IMEM with tags
   0..0x84, 0x6200 bytes of DMEM, `BOOTVEC` 0, `MAILBOX0/1` = WPR meta PA (0x12AB60000),
   start; the model answers MAILBOX0 = 0 and marks the RISC-V core active.
7. Two writes of 0 to `NV_PGSP_QUEUE_HEAD(0)` (the two async RPCs), `FALCON_OS` write,
   `RISCV_STATUS` 0x111240 read = active, then 134791 polls of `DEBUGINFO` 0x110094
   (the health check inside `GspStatusQueueInit`) over 4 s:

   ```
   NVRM: GPU0 GspStatusQueueInit: msgqRxLink failed: -7, nvStatus 0x00000065, retries: 134791
   NVRM: GPU0 _kgspBootGspRm: unexpected WPR2 already up, cannot proceed with booting GSP
   NVRM: GPU 0000:00:05.0: RmInitAdapter failed! (0x62:0x40:2119)
   ```

   The second message is the driver's own boot retry (`kernel_gsp.c:4805-4811`): it
   refuses because WPR2 is up, exactly as real hardware would after a failed boot.
8. After the failure the kernel reads `NV_PFB_FBPA(i)_ECC_DED_COUNT` (16 units, 2 each)
   and `NV_PLTCG_LTC(i)_LTS(j)_L2_CACHE_ECC_UNCORRECTED_ERR_COUNT` (32) once
   (`kern_mem_sys_tu102.c:97-181`), then tears down.

Both PAs the device needs for E3 (libos args, WPR meta) arrive through the mailboxes and
lie inside the mapped guest RAM (the trace says "mapped" for both).

Repeatability: the first load happens automatically at guest boot (udev matches the PCI
id). A second `modprobe` without a reset fails in the GFW wait because the model's GSP
falcon is no longer halted, the same state the driver reports as "may need to be reset".
`tools/caraxes-vm.sh probe` therefore triggers a PCI function-level reset first; the device
sees the FLR bit in the PCIe device control register (also through the config mirror) and
rebuilds the model. Two consecutive probes then produce the same trace.

Corrections to the prediction: none in substance; the timer PLM register (0x9430) and the
ECC counter reads were not in the map and are now.

## E3: the message queue and the init handshake

- Add: mmap guest RAM from DMA_MAP fds; on the first doorbell, read RMARGS through
  MAILBOX0/1 → libos args → `sharedMemPhysAddr` → page table; validate the CPU's
  command-queue header; write the status-queue header; consume the two async RPCs
  (log them decoded); send `GSP_INIT_DONE` (seq 0, `rpc_init_done_v17_00`); answer
  `SET_GUEST_SYSTEM_INFO` (echo 0x2E/0x0D) and `GET_GSP_STATIC_INFO` with a
  `GspStaticConfigInfo` filled from the profile: `gpuNameString` = `guest_card_name`
  ("Leandro RTX 2070-4G"), `fb_length` and one FB region from `fb_length`, `gidInfo`
  from the per-VM UUID, plus SKU, engine caps, internal handles, BAR PDE bases.
  Success criterion beyond init: `nvidia-smi` in the guest prints the Leandro name and
  the profile's memory size.
- Expect: `kgspInitRm` returns NV_OK ("RmInitAdapter" continues into `RmInitNvHal`);
  then the first `GSP_RM_ALLOC`/`GSP_RM_CONTROL` appear in the log and fail, which
  gives the ordered list of RPCs and the direct register accesses of phase 5. Verify
  the struct sizes (`GspStaticConfigInfo`, `GspSystemInfo`) by compiling them with
  the vendored headers first (open-questions Q4).

### E3 result (2026-09-23): expectation met

Setup: as E2; the boot model now owns the GSP side of the message queues
(`crates/vfio-user-nvgpu/src/msgq.rs`, `rpc.rs`), with the struct layouts from
[gsp-abi-610.md](caraxes-gsp-abi-610.md). Evidence: `caraxes/runs/e3-2026-09-23/`
(probe `e3-queues`).

Observed, from the device's boot events:

1. After Booter the model parses the WPR meta the guest wrote: `fbSize 0x100000000,
   gspFwRsvdStart 0xf7600000, nonWprHeap 0xf7600000+0x100000, wprStart 0xf7700000,
   frtsOffset 0xffe00000, wprEnd 0xfff00000` (the layout [fb-model.md](../caraxes/fb-model.md) derived).
2. First doorbell: libos args `LOGINIT`, `LOGRM`, `RMARGS` found; RMARGS gives the
   129-page queue buffer, command queue at 0x1000, status queue at 0x41000. The CPU's
   command header is as predicted (63 slots, `SWAP_RX`). The model writes its status
   header and sends `GSP_INIT_DONE` (element seq 0). The first command is
   `GSP_SET_SYSTEM_INFO` (984 bytes: BAR addresses, PCI ids, config mirror at 0x88000,
   maxUserVa, host page size), then `SET_REGISTRY` (40 entries).
3. The CPU links the status queue at once (no 4 s wait), accepts `GSP_INIT_DONE`, sends
   `SET_GUEST_SYSTEM_INFO` (vgx 0x2e.0xd, driver "610.57.04"); the echoed reply with
   `rpc_result_private` cleared is accepted. Then `GET_GSP_STATIC_INFO` (sequence 1); the
   1600-byte reply carries the labelled identity: name `Leandro RTX 2070-4G`, short name
   `TU106`, `fb_length` 4 GiB, two FB regions from the WPR meta, engine caps for the
   TU106 engines, PDE bases in the non-WPR heap, VBIOS/subsystem ids from the card,
   synthesized GID and internal handles (all labels are in the trace, decision D9).
4. `kgspInitRm` continues: the next request is the dedicated RPC 228
   (`INIT_GSP_TRACE_CRASH_BUFFER`, row 1 of [phase5-rpcs.md](caraxes-phase5-rpcs-610.md)); the
   device answers `NV_ERR_NOT_SUPPORTED` and the guest stops:

   ```
   NVRM: GPU0 nvAssertOkFailedNoLog: Assertion failed: Call not supported [NV_ERR_NOT_SUPPORTED] (0x00000056) returned from kgspInitGspTraceCrashBuffer(pGpu, pKernelGsp) @ kernel_gsp.c:5167
   NVRM: GPU 0000:00:05.0: RmInitAdapter failed! (0x62:0x56:2119)
   ```

   Time from `RmInitAdapter` to the failure: 1.9 s (no timeouts left in the path).

Falsifiers: none triggered. Open question 7 (`SWAP_RX`) is settled: a status header with
the flag set and the read pointers in the swapped places works. Question 26 is settled
too: the queues answer on the first attempt, so the boot retry never runs.

Not yet visible: `nvidia-smi` cannot run before `/dev/nvidia0` exists, so the name is
confirmed only in the reply bytes, not from userspace. That is E4's success check.

## Scope order after E3

`nvidia.ko` and `nvidia-uvm` first (`/dev/nvidiactl`, `/dev/nvidia0`, `/dev/nvidia-uvm`);
`nvidia-modeset`/`nvidia-drm` later. They sit on top of the same RM objects, and
Leandro v1 already runs them over the forwarded RM path, so nothing in them is
expected to need device-level work beyond what the RM path needs. Do not load them
in E0–E4.

## E4: answer the phase-5 RPCs from the host RM

- Add: a translator from `rpc_gsp_rm_alloc/control` to host RM calls, initially only
  the calls that reach `RmInitAdapter succeeded!`. Internal controls are answered by
  querying the host with the equivalent public controls where they exist, or
  synthesized; see [leandro-reuse.md](../caraxes/leandro-reuse.md) §4 for the mismatch list.
- The ordered inventory with an answer source per RPC is [phase5-rpcs.md](caraxes-phase5-rpcs-610.md):
  98 RPCs in the baseline (display absent, `RmRcWatchdog=0`, `RmDisableGlobalCeUtils=1`,
  `RMDisableScrubOnFree=1`): 4 forwardable as-is, 12 with translation, 65 synthesized,
  17 faked with care. The hard ones: `UPDATE_BAR_PDE` with the BAR2/PRAMIN aliasing
  ([fb-model.md](../caraxes/fb-model.md)), the FIFO device-info table, the interrupt table (an MSI
  contract), and the golden channel plus the Bug 4208224 WAR (channel allocations with
  guest FB addresses that can only be faked).
- Method: a dispatcher table in the device keyed by function and control id, one
  handler per row, each answer labelled with its source (D9); the host-forwarded ones go
  through Leandro's backend as an unprivileged RM client. Probe after every few rows and
  record the guest's next stop.
- Expect: milestone 1 (`RmInitAdapter succeeded!`, `/dev/nvidia0` usable,
  `nvidia-smi` in the guest reports the card).

### E4 progress (2026-09-23)

Setup: `--boot-model --host-rm` (`CARAXES_HOST_RM=1`): the device opens the host RM as an
unprivileged client (decision D10, `hostrm.rs`) and answers phase-5 requests through the
dispatcher in `ctrl.rs`, each answer labelled with its source (D9). Guest registry keys
`RmDisableGlobalCeUtils=1;RMDisableScrubOnFree=1;RmRcWatchdog=0` are set through
`NVreg_RegistryDwords` in the guest's `/etc/modprobe.d/caraxes.conf`. Evidence per
iteration in `caraxes/runs/e4-2026-09-23/`.

- **Iteration 1.** Rows 1-13 answered (chip info from the config space, allow-all
  register map, constructed falcon info forwarded from the host card, no NVLink, memsys
  config, BAR info, BIF, RUSD); the empty FIFO device-info table removed KernelGraphics
  and the guest kernel dereferenced the NULL `KernelGraphics` in
  `memmgrCalcReservedFbSpaceHal_GM107` (oops). Lesson: GR0 must be present in the table.
- **Iteration 2.** A provisional TU106 table (GR0, CE0-2, NVDEC0, NVENC0, SEC2, SW; the
  engine descriptors are NVOC class id << 8 | instance) plus the fake framebuffer
  ([fb-model.md](../caraxes/fb-model.md): `fb.rs`, PRAMIN window, `NV_PBUS_BAR0_WINDOW`, the
  MMU-invalidate registers with TRIGGER reading back 0, BAR3/BAR1 translated by walking
  the guest's page tables in the fake FB). The driver passed `kbusVerifyBar2` (33989
  PRAMIN accesses, BAR3 accesses landing in the FB where the kernel expects them),
  created the global VA space, allocated its KernelGraphics static-info client and asked
  the KGR static-info controls (rows 39-52, not yet answered), then failed in the FIFO
  post-scheduling callbacks:

  ```
  NVRM: GPU0 nvAssertFailedNoLog: Assertion failed: 0 @ kernel_fifo.c:3108
  NVRM: GPU0 RmInitAdapter: RmInitNvDevice failed, bailing out of RmInitAdapter
  NVRM: GPU0 kgspExecuteBooterUnloadIfNeeded_TU102: failed to execute Booter Unload: WPR2 is still up
  ```

  Two consequences implemented right away: the second SEC2 run (Booter Unload) now takes
  WPR2 down and stops the RISC-V core; the KGR rows are being implemented as `kgr.rs`
  (host `GR_GET_*` controls with translation, gpcMask 0 as a deliberate milestone-1
  shortcut that skips the golden channel). After the failed init the guest logged
  `BUG: Bad page map in process (udev-worker)`; whether the device's writes into guest
  RAM (status queue, sysmem-targeted BAR accesses) or the driver's failure path caused it
  is open (Q28).

### Milestone 1 reached (2026-09-23)

- **Iteration 3.** `kgr.rs` (KGR static-info rows 35, 39-52: host `GR_GET_*` controls
  with translation, gpcMask 0 as a deliberate shortcut that skips the golden channel and
  the Bug 4208224 WAR), the 12-entry TU106 FIFO device-info table from
  [fifo-device-info.md](../caraxes/fifo-device-info.md) (engines, MMU fault ids and PBDMA counts read
  from the host card with unprivileged controls; runlists and PBDMA ids synthesized; the
  provisional table's MC field had been an out-of-bounds array index in the guest and GR's
  fault id 0 had claimed fault ids 0..63), split RPCs (the 73760-byte SM-order control
  arrives as a first record plus continuation records and is answered in the same shape,
  `rpc.c:2058-2244`), the interrupt tree with MSI (`intr.rs`: the environment self-test
  triggers vector 129 and receives the MSI), Booter Unload taking WPR2 down, and the
  remaining synthesized rows (30-33, 79, 80, 96-98).

  ```
  NVRM: GPU 0000:00:05.0: RmInitAdapter
  NVRM: GPU 0000:00:05.0: RmInitAdapter succeeded!
  ```

  2.2 s from `RmInitAdapter` to success, at guest boot, on the stock 610.57.04 module with
  no guest patch. `/proc/driver/nvidia/gpus/*/information` shows the synthesized UUID and the
  VBIOS version from the synthetic image. Then, in the guest:

  ```
  $ nvidia-smi
  | NVIDIA-SMI 610.57.04              KMD Version: 610.57.04     CUDA UMD Version: 13.3     |
  |   0  Leandro RTX 2070-4G            Off |   00000000:00:05.0 N/A |                  N/A |
  | N/A   N/A  N/A             N/A  /  N/A  |       0MiB /   4096MiB |     N/A      Default |
  ```

  The identity comes from the profile at the device level (decision D6); no reply was
  rewritten. `nvidia-smi` sent 27 RPCs; the ones still unanswered (all tolerated) are
  `0x20800a4c` (7 times), `0x2080852e`, `0x20808513`, `0x20802068`, `0x20800102`, which is
  why the temperature, power, fan and utilization columns read N/A. Evidence:
  `caraxes/runs/e4-2026-09-23/milestone1-*.txt`.

Open after milestone 1: the N/A columns (forward the perf/thermal controls to the host),
`/dev/nvidia-uvm` and the first CUDA context (E5), the golden channel and real GPU work
(needs the identity-mapped FB, [fb-model.md](../caraxes/fb-model.md) option C), and Q28.

### nvidia-smi complete (2026-09-23)

- **Expected:** after forwarding the telemetry controls to the host card, `nvidia-smi` and
  `nvidia-smi -q` in the guest show numbers where milestone 1 showed N/A, while the
  identity stays device-owned.
- **Implemented:** `nvml.rs` answers every control the two commands send (D9 labels).
  Public ROUTE_TO_PHYSICAL controls and 30 GSS-legacy thermal/clock/perf/pmgr commands
  (exactly the host's own `nvidia-smi -q` set) are forwarded to the host card.
  `THERMAL_SYSTEM_EXECUTE_V2_PHYSICAL` goes to the host as the public 0x20800513, and
  0x2080852e is identified as an output-only GSS-legacy thermal query. The SMC mode
  (unsupported), NVLink platform info and InfoROM BBX/diag answers are synthesized.
  `GPU_GET_INFO_V2` indices 0x23/0x24 (the chip-unique GPU PDI) come from the device
  GID, and `GET_CHIP_DETAILS` gets the emulated PCI id.
- **Supporting fixes:** the dispatcher keys objects by (hClient, hObject). The host
  client passes NULL for zero-size params: with a non-NULL pointer,
  `GPU_QUERY_INFOROM_ECC_SUPPORT` came back 0x1f and nvidia-smi showed ERR!.
  GSS-legacy replies go back with rmctrlFlags 0 so the guest does not cache them.
- **Iterations:** three VM boots with `CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=4096
  CARAXES_HOST_RM=1`.
  - Iteration 1: three unanswered controls (BINAPI 0x20810110/0x20810111, diag
    0x208f1105), plus the 0x1f above.
  - Iteration 2: none unanswered.
  - Iteration 3: the final build, with the gate.

  Before the change, the running milestone-1 build had 24 unique unanswered controls
  under `nvidia-smi -q` and 147 N/A lines. After it, it had 0 and 81 (the host's own
  `nvidia-smi -q` had 80 at the same time).
- **Observed** (commits: this tree `e2f1aec` plus the uncommitted crate, vendor `e4a5faa`):

  ```
  |   0  Leandro RTX 2070-4G            Off |   00000000:00:05.0 Off |                  N/A |
  | 30%   50C    P8             29W /  175W |       0MiB /   4096MiB |     19%      Default |
  ```

  `nvidia-smi` sent 33 controls (28 host, 5 synthesized). `nvidia-smi -q` sent 98
  controls (91 host, 1 profile, 6 synthesized). No control was unanswered. The guest
  journal shows "RmInitAdapter succeeded!" and no assertion.
- **Left different from the host:** PCIe "Host Max" is N/A, because it comes from the
  guest's root port and no RM control is involved. Virtualization Mode reads
  Pass-Through, which the guest kernel detects itself.
- **Gate:** `tools/caraxes-gate.sh nvml` gave the verdict `pass` (exit 0), with 11 of
  11 checks. Guest: 50 C, P8, 29.67 W, 18 %, 4096 MiB total with 311 reserved + 0
  used + 3786 free. Host `nvidia-smi -q` around the run: 50 C, P8, 29.78 W.
- **Details:** control list, labels, the gate and the D9 minimal set are in
  [nvml.md](../caraxes/nvml.md). Forwarded utilization, power and counters describe the whole
  card: the guest sees the host desktop's load. That is the first thing the D9 cut
  has to address.

### E5a: UVM registration (2026-09-23)

- **Expected** ([cuda-path.md](../caraxes/cuda-path.md) §6, row E5a): `nvidia-uvm` loads in the
  guest, and `UVM_REGISTER_GPU` succeeds on the host-RAM framebuffer. The device
  executes the copy-engine pushes that registration waits for. No work reaches the
  physical GPU.
- **Implemented** (details, RPC table and labels in [uvm.md](../caraxes/uvm.md)):
  - `channel.rs`: the channel emulation. It records the guest's VA spaces, TSGs and
    channels from `GSP_RM_ALLOC` and from the VAS-root controls. A doorbell write at BAR0
    0xBB0090 runs the channel's GPFIFO from GPGet to GPPut. The device decodes the
    pushbuffer and executes the host methods and the `TURING_DMA_COPY_A` subset UVM
    uses (semaphore release, copy, constant memset, by virtual or physical address)
    against the fake FB and guest RAM, then writes GPGet back to USERD.
  - `uvm.rs`: answers for the UVM controls (fault and access-counter buffer
    registration, CE caps from the host, decompression LCE, VAS root, schedule), and
    the fault-buffer and access-counter pointer registers. 0xB83110 reads 256 entries.
    The usermode timer counts from the device clock.
  - `fb.rs`: the GMMU walk now follows page tables in guest RAM as well.
  - `msgq.rs`: the parameters of `GSP_RM_ALLOC` are read from the whole queue slot.
- **Iterations:** the milestone-1 build on the already running VM, then five VM boots
  with `CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=4096 CARAXES_HOST_RM=1`. The client is
  `crates/vfio-user-nvgpu/guest/uvm-register.c`, compiled in the guest.
  - Iteration 0 (milestone-1 build):
    - Without `/dev/nvidia0` held open, registration returns 0x1b before any RPC
      (`nv_gpu_ops.c:7671-7674`).
    - With it held open, `INTERNAL_GMMU_REGISTER_FAULT_BUFFER` 0x20800a9b is
      unanswered, and registration returns 0x56.
  - Iteration 1: the channel parameters read as zeros. A `GSP_RM_ALLOC` counts only its
    fixed part in the RPC length (`rpc.c:11002-11025`), so every allocation had been
    echoed with zeroed parameters since E4. The first doorbell named an unrecorded
    ChID, and UVM waited on its push without timeout.
  - Iteration 2: all 7 pushes ran (6 `channel_init`, 1 page-tree init), and
    `DMA_SET_PAGE_DIRECTORY` followed. Registration then failed with 0x3 in
    `uvm_devmem_init`: `request_free_mem_region` returned -34, because the guest's
    PCI host-bridge window claims all physical address space above RAM. The same
    boot with `UVM_INITIALIZE` flags 1, or with `uvm_disable_hmm=1`, returned 0.
  - Iteration 3: the pointer-register model and the usermode timer. `nvidia-smi` and
    `tools/caraxes-gate.sh nvml` (verdict pass, 11 of 11) confirm milestone 1.
  - Iteration 4: the KGR change below.
  - Iteration 5: the final binary, through the new `tools/caraxes-vm.sh uvm <label>
    [flags]`. Both variants return 0.
- **Observed** (commits: this tree `e2f1aec` plus the uncommitted crate, vendor
  `e4a5faa`):

  ```
  UVM_INITIALIZE flags 0x1: ioctl 0 rmStatus 0x0
  UVM_REGISTER_GPU: ioctl 0 rmStatus 0x0 numaEnabled 0 numaNodeId -1
  UVM_UNREGISTER_GPU: ioctl 0 rmStatus 0x0
  ```

  Device trace, first `channel_init` push, and the page-tree push:

  ```
  doorbell 1: token 0x10001 chid 1 runlist 1
  chid 1 gp 0: pushbuffer va 0x120000000 10 dwords
  host SET_OBJECT subch 0 class 0xc5b5
  host SET_OBJECT subch 5 class 0xc076
  ce semaphore release va 0x121000000 = 0x1 [guest]
  chid 5 gp 1: pushbuffer va 0x120600000 26 dwords
  ce memcopy 32 bytes -> Fb(2097152) from va 0x120600004 [guest]
  ce semaphore release va 0x121000010 = 0x2 [guest]
  ctrl 0x801813: set page directory: vaspace 0xcaf00005 root Some(Fb(2097152)) entries 4 flags 0x8 [guest]
  ```

  Registration sends 28 allocations, 37 controls and (on unregister) 28 frees, and
  none is unanswered. The labels are:
  - host: ECC status, the CE caps and the PCE masks;
  - guest: the buffers, VA spaces, TSGs, channels and the page directory;
  - synthesized: decompression LCE, schedule, SW and CE objects.

  With default flags and HMM on, the result is still 0x3 (`uvm_gpu.c:2929`). That is
  the guest platform, not the device ([uvm.md](../caraxes/uvm.md), "HMM device-private memory").
- **Temporary aid:** `uvm_disable_hmm=1` (a module parameter, not a driver patch) or
  `UVM_INITIALIZE` flags 1 in the client. Plan to remove it: give the VM a physical
  address range outside the PCI window, or confirm in the `cuda-core` trace which
  flags CUDA passes.
- **Also changed:** `kgr.rs` no longer forwards
  `INTERNAL_STATIC_KGR_GET_SM_ISSUE_THROTTLE_CTRL` 0x20800b05 to the host. The host's
  static info has no throttle control on TU106, so the public 0x2080123d made the host
  driver log the assertion failure "pSmIssueThrottleCtrl != NULL"
  (`kernel_graphics.c:3425`) once per guest load. The device now answers NOT_SUPPORTED
  itself, which the guest tolerates (phase5-rpcs.md row 48, `kernel_graphics.c:1463-1466`).
  Iteration 4 shows the new answer.
- **Evidence:** `caraxes/runs/e5-2026-09-23/iter0-baseline.txt` … `iter5-launcher.txt`.
- **Blocks E5b** (list in [uvm.md](../caraxes/uvm.md)):
  - the guest's VRAM layout starts at FB 0 (kernel VAS root at 0x0, UVM's page tree
    at 0x200000);
  - the guest physical sysmem addresses in the pushbuffer and semaphore PTEs;
  - guest ChIDs and runlists that differ from the host's;
  - synchronous execution inside the trapped doorbell;
  - no non-stall interrupts.

## D14 refactor: chip, board and driver-version axes (2026-09-23)

- **Goal (decision D14, "Planned refactor after E5a" in [README.md](caraxes-record-2026-09-28.md)):**
  keep the TU106/610.57.04 device working exactly as before while every chip-, board-
  or driver-version-specific fact moves to a home where another chip or version can
  be added without touching the generic paths. Layout and checklists:
  [versatility.md](../caraxes/versatility.md).
- **What changed in `crates/vfio-user-nvgpu`:**
  - `src/chip/`: trait `Chip` (tables as `ChipData`, firmware boot as `FirmwareBoot`
    with the events `WprMeta` and `CmdQueueDoorbell`), the Turing firmware-boot model
    `turing.rs` (the former FWSEC/Booter/falcon/WPR2 half of `boot.rs`) and `tu106.rs`.
    The TU106 tables are data: `chips/tu106/{chip,regmap,device-info,interrupts,boot}.toml`,
    loaded with `include_str!` and parsed once; driver enums in them are names that the
    version module resolves. `--chip` (default: the board's chip, `tu106`).
  - `src/board.rs` and `boards/rtx2070.toml`: BAR layout, ROM BAR size, VBIOS version,
    board name, chip and capture paths. The PCI ids reach the dispatcher, the static
    info and the selftest from the config-space capture instead of literals; the PM,
    MSI and PCIe capabilities are found by walking the capture's chain. New flags
    `--board-data` and `--vbios-version`.
  - `src/ver/`: feature `v610` (default, `nvrm-sys/v610`), module `v610.rs` with the
    RPC numbers, message layouts, classes, control ids, control layouts (computed from
    `nvrm_sys::v610` where typed), driver enums and the `nvidia-smi` lists. Tests check
    the GSP offsets against the measured tables of [gsp-abi-610.md](caraxes-gsp-abi-610.md) (a)
    and pin the typed layouts to the numbers the handlers used.
  - `boot.rs` is the generic boot model (kernel-RM surface at the chip's offsets,
    queues, RPC service); `bar0.rs`, `ctrl.rs`, `kgr.rs`, `nvml.rs`, `uvm.rs`,
    `channel.rs`, `intr.rs`, `msgq.rs`, `rpc.rs`, `config.rs`, `device.rs`,
    `selftest.rs` and `main.rs` take the chip, the board or the version module.
  - One new trace note exists: a guest whose `SET_GUEST_SYSTEM_INFO` names another
    driver than 610.57.04 is noted. It does not fire for the 610.57.04 guest.
- **Expected:** no guest-visible change: the same boot event sequence, the same answer
  and label for every RPC, `RmInitAdapter succeeded!`, the `nvml` gate and the UVM
  registration as in E5a iteration 5.
- **Observed (expectation met):**
  - `cargo test -p vfio-user-nvgpu`: 38 unit tests (29 before; new ones for the chip
    tables, the Turing boot, the board and the version layouts) and the 2 protocol
    tests pass; `cargo fmt`, the kickoff
    clippy line with and without `--all-features`, and `tools/check.sh` (17 of 17) pass.
  - VM (`CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=4096 CARAXES_HOST_RM=1 tools/caraxes-vm.sh
    down; up; wait 150`): `RmInitAdapter succeeded!` at 5.57 s; the device log reads
    `TU106 (BOOT_0 0x166000a1, driver 610.57.04)`.
  - `tools/caraxes-gate.sh nvml`: pass, 11 of 11 checks (name `Leandro RTX 2070-4G`,
    4096 MiB, temperature 51 C against the host's 51, P5 on both).
  - `tools/caraxes-vm.sh uvm verify 1`: `UVM_REGISTER_GPU` rmStatus 0x0 for
    `GPU-9bd7b93d-3d23-c074-6704-984ef33ab7a7`, `uvm-register rc=0`, 7 pushes, no
    unanswered request: the same as `caraxes/runs/e5-2026-09-23/iter5-launcher.txt`.
  - Event sequence: the 325 `E boot:` events of the fresh boot (up to `RmInitAdapter`)
    equal the first 325 of the trace of the pre-refactor binary (the E5a VM session that
    was running when the refactor started) once guest physical addresses, which differ
    per boot, are masked: same RPCs in the same order, same controls, statuses, labels,
    reply lengths and static-info fields. Over the whole sessions (boot, `nvidia-smi`,
    `nvidia-smi -q`, UVM registration) the 200 distinct kinds of answer (control or
    allocation, status, label; reply function, result, length) are the same set.
- **Evidence:** `target/caraxes/d14-refactor/` (git-ignored): both event lists, both
  answer-kind lists, the UVM run (`uvm-verify/`). Trees: Caraxes `e2f1aec` plus the
  uncommitted working tree; `vendor/open-gpu-kernel-modules` 610.57.04 (`e4a5faa`).
- **Host GPU reads** in this run: the device's unprivileged host RM client (D10) for
  the forwarded controls, and the gate's read-only `nvidia-smi -q` on the host; no
  privileged access, no register access.

### Guest DMA-range module (2026-09-23)

The temporary guest aid of D13 ([sysmem-iova.md](../caraxes/sysmem-iova.md) §5) exists:
`guest-module/caraxes_dma_range/` (module, test driver, README). Expectation: with the
module's table installed before `nvidia` binds, every DMA address the guest driver hands to
the device equals guest physical address + the table's offset, and the stock driver still
initializes with an identity table.

- **Result: as expected.** Tested in vm1 (kernel 6.8, driver 610.57.04):
  - (a) 13 invalid loads refused with the expected reason each (missing table, overlaps,
    size not a 2 MiB multiple, unaligned start, coverage gap at 4 GiB, 65 entries, wrong
    BDF, driver bound with and without `force=1`).
  - (b) Identity table in four 1 GiB entries: `RmInitAdapter succeeded!`, `nvidia-smi`
    works, `live` equals `table`, `rmmod` refused while `nvidia` is bound. After
    `rmmod nvidia_uvm nvidia; modprobe nvidia` the map was re-installed by the bus
    notifier before the probe and the driver initialized again (installs=2).
  - (c) Offset tables without a driver (test driver maps pages, never touches the device):
    +5 GiB, 64-bit top-down below 2^47, +8 GiB+4 KiB and a negative offset all mapped to
    GPA + offset with `dma_to_phys` inverting them.
- **Rules learned:** the table must cover every System RAM page, otherwise the 6.8 kernel's
  coverage check (`check_ram_in_range_map`) can loop forever; the driver core frees the
  map on unbind and probe failure, so each install is a fresh copy; the PCI function-level
  reset before a re-probe is still required (an attempt without it failed with "no
  initialized WPR2 found", with and without the map).
- **Evidence:** `caraxes/runs/dma-range-2026-09-23/` (`commands.txt`, `a-refusals.txt`,
  `b-identity-nvidia.txt`, `c-offsets.txt`).
- **Host GPU reads:** none beyond the device's unprivileged host RM client (D10) during the
  guest's normal initialization.
- **Next:** E5b part 1 produces the table from the registered guest RAM and loads the
  module from the launcher before `modprobe nvidia`.

### Test-suite integration and the first CUDA gate (2026-09-23)

`tools/caraxes-vm.sh sync-tests` puts `../Leandro-Test`'s guest-side material (probes, kernels,
matrix, suites, guest scripts, the v1 gate library) and the v1 tracer into the guest;
`tools/caraxes-gate.sh cuda|gpu|vdisplay|display` run the v1 gates against the device with the
v1 verdict logic ([testing.md](../caraxes/testing.md)). Expectation: nvml passes, CUDA fails, and the
failure names the first thing the device does not answer.

- **Result: as expected.** nvml 11/11; cuda 0/12 (11 fail, cuda-gdb skipped); gpu fails in
  every CUDA stage; vdisplay/display "not applicable yet" (exit 2).
- **The wall is one control.** Every CUDA program dies in `cuInit` (error 100) because the
  device answers `NV2080_CTRL_CMD_BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS` (0x2080182a) with
  NOT_SUPPORTED: it is ROUTE_TO_PHYSICAL, so it always reaches the device, and the
  dispatcher has no row for it. The guest's first 48 RM calls match the v1 native trace
  call for call. Predicted next from the native sequence: `GRMGR_GET_GR_FS_INFO`
  (0x20803801) and `BUS_GET_C2C_INFO` (0x2080182b), then `UVM_REGISTER_GPU`.
- **CUDA needs `uvm_disable_hmm=1`:** libcuda calls `UVM_INITIALIZE` with flags 0, so the
  module option is required (E5a's open point settled). vm0 lacked it; added.
- **Guest image facts:** torch 2.13.0 with CUDA 13.0 wheels and `cuda.h` in the venv, no
  nvcc/cuda-gdb, no ffmpeg/cupy/cudf, no network.
- **Evidence:** `caraxes/runs/gate-{cuda,cuda-2,gpu,vdisplay,display}-2026-09-23*/`; the
  ordered table of what the CUDA path sends is testing.md §8.
- **Host GPU reads:** read-only `nvidia-smi` in the smi stage and the nvml gate.

### E5b part 1 (2026-09-23)

The memory model of [memory-model.md](../caraxes/memory-model.md), on vm2. Pushes are still executed
by the device.

**Expectation.** Guest FB and sysmem addresses become the final ones and the device keeps
working:
- guest FB addresses lie inside a fixed host VRAM reservation;
- guest DMA addresses are host IOVAs of registered guest RAM, which the guest module
  `caraxes_dma_range` installs;
- the device translates its own accesses back;
- everything is freed at exit;
- a reservation the host cannot afford is refused.

**What changed.**
- `crates/vfio-user-nvgpu`:
  - `fb.rs`: `FbLayout`, the `FbStore` backing interface, out-of-reservation accounting.
  - `hostmem.rs`: reservation, 1 GiB OS descriptors, teardown.
  - `dmamap.rs`: the table and its module format.
  - `guestmem.rs`: `translate`.
  - `privilege.rs`: CAP_SYS_ADMIN scoping.
  - `device.rs`: registration at DMA_MAP.
  - `main.rs`: `--reserve-vram`, `--register-guest-ram`, `--identity-dma-table`,
    `--dma-ranges`, the SIGTERM thread.
  - `rpc.rs` and `boot.rs`: the region table and BAR roots from the layout.
  - `ver/v610.rs`: host-memory ids and layouts.
- `tools/caraxes-vm.sh`: `CARAXES_RESERVE_VRAM`, `CARAXES_DMA=register|identity`, `dma`
  (also run by `probe` and `uvm`), and the `setcap` copy of the binary.
- **vm2 created** as vm1 was:
  - overlay, seed `caraxes-vm2` 192.168.102.22, tap `caraxes2` 192.168.102.1/24;
  - 610.57.04 built in the guest from `ogkm-610.57.04.tar.zst`; only `nvidia.ko` and
    `nvidia-uvm.ko` installed to `updates/`; `gsp_tu10x.bin` copied (sha256 d157e3b7...);
  - `/etc/modprobe.d/caraxes.conf` with the three registry keys, `uvm_disable_hmm=1` and
    `blacklist nvidia`;
  - `caraxes_dma_range` built in the guest.

**Observed (expectation met).** All steps ran on vm2.

(a) **Reservation held.**
- Device log: `reserved host VRAM [0xe9000000, 0x1e9000000) (4096 MiB), handle 0x80000003,
  attr 0x11800000 attr2 0x100005, contiguous, in 0 ms`.
- The rule's inputs: HEAP_FREE 6520 MiB, nvidia-smi free 6521 MiB.
- Host nvidia-smi used went from 1257 to 5364 MiB (+4107).

(b) **Boot and nvml gate with the reservation.**
- Boot: `RmInitAdapter succeeded!`.
- WPR meta: `fbSize 0x200000000 gspFwRsvdStart 0x1f7600000`, then `fb layout: guest WPR
  [0x1f7600000, 0x200000000) above the reservation [0xe9000000, 0x1e9000000): ok`.
- Static info: regions usable `[0xe9000000, 0x1e8e00000)`, reserved
  `[0x1e8e00000, 0x200000000)`; `bar1PdeBase/bar2PdeBase` `0x1e8e01000/0x1e8e00000`.
- `tools/caraxes-gate.sh nvml`: pass, 11 of 11. The guest shows `0MiB / 4096MiB`.
- UVM registration: rmStatus 0x0. The VAS roots are at 0xe9000000 and 0xe9200000.
- BAR walk targets: 0x1e8da0000..0x1e8e01018.
- Exit summary: `accesses outside it: 0 reads, 0 writes`.

(c) **DMA table active, non-identity.**
- Table: `0:40000000:40000000,40000000:7fffc0000000:40000000,80000000:7fff80000000:40000000,100000000:7fff40000000:40000000`.
  The host GPU went to 64-bit IOVAs after the first chunk, which the operator allowed.
- Registration: 0.7-1.5 s per GiB, 4 privileged windows, uid 1000 with
  `permitted 0x200000`.
- The guest module loaded the table: `map=ours ranges=4`.
- The guest then handed out IOVAs:
  - `fwsec: data buffer PA 0x7fff6020d000`;
  - `wpr meta PA 0x7fff66218000`;
  - fault buffer `pte[0] 0x7fff6dafc000`;
  - pushbuffer and semaphore targets `Sys(0x7fff...)`.
- Boot: `RmInitAdapter succeeded!`.
- nvml gate: pass.
- `tools/caraxes-vm.sh uvm verify 1`: `UVM_REGISTER_GPU ... rmStatus 0x0`,
  `uvm-register rc=0`, 7 pushes executed by the device through the translation.
- Exit summary: `0 DMA address(es) outside the table refused`, 0 FB accesses outside the
  reservation.

(d) **Cleanup at exit** (`down`).
- The device logged, in order:
  - `freed 4 guest RAM descriptor(s), 4096 MiB unpinned`;
  - `freed the VRAM reservation`;
  - `host RM client ... freed`.
- Host `nr_foll_pin_acquired - released`: 3249 before, 1049825 while held, 3249 after.
- Host VRAM used: 1273 before, 5420 while held, 1323 after. The +50 MiB is host desktop
  drift; 4097 MiB were released.
- `MemAvailable`: 12.2 GiB before, 9.7 GiB while held, 14.0 GiB after.
- SIGTERM (standalone device with a 2560 MiB reservation): `signal 15: freeing host memory
  and exiting`, reservation freed, exit status 143.

(e) **Refusal.** `CARAXES_FB_MIB=6144`:
- The device refused: `reserving 6144 MiB would leave the host 302 MiB of VRAM, below the
  1024 MiB headroom (free 6446 MiB = min(HEAP_FREE, nvidia-smi); shortfall 722 MiB)`.
- It freed its client and exited before creating the socket. `up` reported the error; no
  VM started.
- Host VRAM was unchanged.

**Checks.**
- `cargo test -p vfio-user-nvgpu`: 46 unit tests and 2 protocol tests.
- `cargo fmt` and the kickoff clippy line, with and without `--all-features`.
- `tools/check.sh`: 17 of 17.

**Evidence.** `caraxes/runs/e5b-2026-09-23/`:
- `a-*`: device log, host before and while held;
- `b-*`: gate JSON, guest dmesg, trace excerpt, UVM log, exit summary;
- `c-*`: table, module load, gate, UVM, trace excerpt;
- `d-*`: host before, held and after, device exit, SIGTERM;
- `e-refusal.txt`.

Trees: Caraxes `e2f1aec` plus the working tree; `vendor/open-gpu-kernel-modules`
610.57.04 (`e4a5faa`).

**Host GPU and host state touched.**
- RM API only, from the device's unprivileged clients: the reservation, OS descriptors,
  `FB_GET_FB_REGION_INFO`, `FB_GET_INFO_V2`, and the forwarded controls.
- Privileged: four `GET_SURFACE_PHYS_ATTR` windows.
- Read-only `nvidia-smi` queries.
- Root actions: `sudo -n setcap` on the binary copy, and the tap `caraxes2`.
- The host GPU switched to 64-bit IOVAs, per device, until reboot.
- No register access, no mapping of the card.

(c2) **After the host's 64-bit switch.**
- **Failure.** A restart of vm2 registered all four chunks near 2^47 (GPA 0 →
  0x7fff80000000). The guest then failed early:
  - `swiotlb addr 0x00007ffefc2b7000+4096 overflow (mask ffffffffff)`;
  - `osInitNvMapping failed`;
  - `RmInitAdapter failed! (0x22:0x59:894)`.
- **Cause.** The guest's sysmem flush buffer must have a DMA address below 2^40
  (`kern_mem_sys_gm107.c:227-340`). The first run had worked only because GPA 0 still got
  a 32-bit IOVA.
- **Fix.** The launcher aliases the guest's swiotlb pool (`[0xba2b7000, 0xbe2b7000)`,
  rounded to 2 MiB) at IOVA 0xf000000000 in the guest's copy of the table (memory-model.md
  §2). The table is `0:7fff80000000:40000000,40000000:7fff40000000:40000000,80000000:7ffec0000000:3a200000,ba200000:f000000000:4200000,be400000:7ffefe400000:1c00000,100000000:7ffe80000000:40000000`.
- **Result.**
  - Boot: `RmInitAdapter succeeded!`, with `NV_PFB_NISO_FLUSH_SYSMEM_ADDR` =
    0xf0038b70.
  - nvml gate: pass.
  - `uvm verify-alias 1`: rmStatus 0x0, rc=0.

**cuInit rows.**
- Added to the host-forwarded `FORWARD` list:
  - `BUS_GET_PCIE_SUPPORTED_GPU_ATOMICS` 0x2080182a (the host answers 0x0 in the trace);
  - `BUS_GET_C2C_INFO` 0x2080182b;
  - `GRMGR_GET_GR_FS_INFO` 0x20803801.

  All three are NON_PRIVILEGED.
- The INTERNAL `CE_GET_ALL_PHYSICAL_CAPS` 0x20802a0b, which cuInit asked for next, is
  answered by the host's public `CE_GET_ALL_CAPS` 0x20802a0a, which has the same params
  type.
- **After these rows (vm2, python ctypes against `/opt/nvrm/lib/libcuda.so.1`).**
  - `cuInit` still returns 100 (CUDA_ERROR_NO_DEVICE). The guest's libcuda is
    610.43.03.
  - The device's remaining refusals in that window are:
    - `INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO` 0x20800aa2 (unanswered);
    - 0x20808159 (unanswered);
    - `GPU_QUERY_ECC_STATUS` (host NOT_SUPPORTED, as on the real card).
  - Evidence: `c3-cuinit.txt`, `c3-cuinit-trace.txt`.

**Not done.**
- PROMOTE_CTX buffers are not recorded.
- The table is not re-read after a guest reboot. A new VM start re-registers.
- `--identity-dma-table` was not run on vm2. The module's identity path was tested in vm1.

### E5c: CUDA initialization (2026-09-23)

Rows for the CUDA path on vm2 (E5b part 1 memory model: VRAM reservation, registered guest
RAM, DMA table with the swiotlb alias), following [cuda-lookahead.md](../caraxes/cuda-lookahead.md)
sets 1 and 2. Pushes are still executed by the device's software channel; nothing runs on
the physical GPU. Evidence: `caraxes/runs/e5c-2026-09-23/` (`c0`-`c5`, one directory per
device build, gate layout of testing.md §5).

**Expectation.** cuInit, cuCtxCreate and cuMemAlloc/cuMemcpy (nvprobe stages 0-2) pass with
host-forwarded, synthesized and guest-derived rows and the software copy engine; the first
kernel launch is the execution wall.

**libcuda finding.** The guest loads `/opt/nvrm/lib/libcuda.so.1` (ldconfig; the torch venv
has no libcuda of its own), a link to `libcuda.so.610.57.04`, byte-identical to the host's
`/usr/lib/libcuda.so.610.57.04` (sha256 9c7e7e51...). The 610.43.03 files beside it are stale
and unused; E5b's reading was wrong. The version check passes (the guest RM calls continue
past it; no API-mismatch message in the kernel log).

**Rows added** (`crates/vfio-user-nvgpu`; each with its label and a unit test):

| Id | Name | Answer | Label | Why |
|---|---|---|---|---|
| 0x20800aa2 | INTERNAL_STATIC_GRMGR_GET_SKYLINE_INFO | zeros, validEntries 0 (`cuda.rs`) | synthesized (no MIG) | GPU_GET_SKYLINE_INFO in cuInit; native public answer is all zero |
| 0x20808159, 0x20808162 | GSS legacy GPU 0x59 (332 B), 0x62 (1 B) | forwarded opaquely (`LEGACY`) | host | cuInit returned 100 on 0x59 (guest tracer: last failed RM call) |
| 0x20809001, 0x2080a026, 0x2080a084 | GSS legacy telemetry (torch) | forwarded opaquely (`LEGACY`) | host | native cuda-torch trace, NV_OK |
| 0x20800a26 | STATIC_KGR_GET_FLOORSWEEPING_MASKS (row 41) | assembled from host GR_GET_GPC_MASK, GR_GET_TPC_MASK, GR_GET_NUM_TPCS_FOR_GPC, GR_GET_ZCULL_MASK, GPU_GET_PES_INFO, GR_GET_GFX_GPC_AND_TPC_INFO (`kgr.rs` `floorsweeping`) | host | the milestone-1 zero made the compute alloc fail (`numGpcs > 0 @ kernel_graphics_object.c:192`, cuCtxCreate 2); now gpcMask 0x7, 3 GPCs of 6 TPCs |
| 0x2080012b | GPU_PROMOTE_CTX | echo; entries recorded per (hChanClient, hObject), PA and VA merged per buffer id (`channel.rs` `record_promote_ctx`) | guest | golden/WAR channels at module load, compute objects, UVM bind; PRIVILEGED, not forwardable |
| 0x802046 | KGR_INIT_BUG4208224_WAR | NV_OK | synthesized | module load, now that gpcMask is non-zero |
| 0x20800a9a, 0x802009, 0x802004 | PERF_BOOST_SET_2X, PERF_CUDA_LIMIT_SET_CONTROL, PERF_CUDA_LIMIT_DISABLE | NV_OK | synthesized (nothing acted on) | cuCtxCreate / teardown; not forwarded so the host card's clocks stay untouched |
| 0x83de0309, 0x20801210, 0xa06c0103 | DEBUG_SET_EXCEPTION_MASK, GR_SET_CTXSW_PREEMPTION_MODE, SET_TIMESLICE | NV_OK | synthesized | cuCtxCreate; part 2 maps them to host objects |
| 0xa06c0105, 0xa06f0112, 0x2080012c | PREEMPT, STOP_CHANNEL, GPU_EVICT_CTX | NV_OK | synthesized | teardown |
| 0x20801702 | MC_SERVICE_INTERRUPTS | NV_OK | synthesized (no interrupt pending) | libcuda polls it while a launch does not complete (never in native traces) |
| DUP_OBJECT (fn 21) | - | VAS alias recorded (`Registry::record_dup`, `Dispatcher::dup`) | guest | UVM duplicates CUDA's VAS and sets the page directory on its own handle |
| alloc 0x9067 | FERMI_CONTEXT_SHARE_A | hVASpace recorded; channels resolve their VAS through hContextShare | guest | GR TSG and GR channels carry hVASpace 0 |

Channel emulator (`channel.rs`): per-subchannel class dispatch (`compute` class added to
`chips/tu106/chip.toml`); a `TURING_COMPUTE_A` stub runs report semaphores (release, one or
four words, reductions), inline-to-memory with its semaphore, and refuses launches
(`SEND_PCAS_*` and the last `LOAD_INLINE_QMD_DATA`), logging the QMD. Other compute methods
are logged and skipped (context init: 0x248 SET_CWD_REF_COUNTER, 0x1698, 0x155c-0x157c, ...).

**Observed.**

| Test | Before (c0) | After (c1-c5) |
|---|---|---|
| cuda-init (nvprobe 0) | fail, cuInit 100 (0x20808159 unanswered) | **pass** (c1 on) |
| cuda-context (nvprobe 1) | fail (cuInit) | **pass** (c2 on; c1 failed `cuCtxCreate: 2`, compute alloc 0x1a) |
| cuda-core (nvprobe 2, cuMemAlloc + H2D/D2H compare) | fail (cuInit) | **pass** (c2, c3, c5): 5 CE virtual copies, 90 CE memsets, 67 compute I2M in the window, all through the translation |
| cuda-hostreg (pinned roundtrips) | fail (cuInit) | **pass** (c3) |
| cuda-jit (nvprobe 3) | fail (cuInit) | fail: launch refused, see the wall |
| cuda-launch (nvprobe 4) | fail (cuInit) | fail: timeout (launch, blocking sync) |
| cuda-managed | fail (cuInit) | fail: `AllocManaged a: 801 operation not supported`, no device refusal in the window (guest UVM side, not analysed) |
| cuda-torch | fail (cuInit) | fail: stage 0 and 1 ok, `H2D/D2H round trip broken` (the copy waits on a kernel's semaphore) |
| rlprobe, convburn | fail (cuInit) | fail: timeout (kernels) |
| robustness | fail | fail: `0 of 3 runs correct (WRONG at 1: 0.000000 != 3.000000)`: the vecAdd result buffer is copied back, the kernel never ran |
| cuda-gdb | skip | skip (no cuda-gdb) |

Module load with the host floorsweeping masks: golden channel and Bug 4208224 WAR channel
allocate, PROMOTE_CTX (9 and 8 entries, MAIN at FB 0x1e8c52000 inside the reservation) and
`TURING_A` go through, 0x802046 answered; `RmInitAdapter succeeded!`.

**The execution wall** (c5, `device/cuda-jit.trace`): the GR channel (chid 2) uploads the
vecAdd SASS (I2M 256 bytes to va 0x10005284a00) and the parameters (I2M 352 + 28 + 32 bytes
to va 0x10004220000...), then launches with an inline QMD:

```
compute KERNEL LAUNCH NOT EXECUTED (needs the physical GPU) [execution wall]: QMD at va 0x204208000; grid 16x1x1; release0 va 0x203c3ff7c payload 0x6 (enable bit 1) is NOT written; raw [...]
host semaphore acquire va 0x203c3ff7c op 3: 0x5 vs 0x6, channel waits
```

The D2H copy on a CE channel (chid 14, runlist 2) acquires exactly that semaphore, so it
waits; libcuda then polls `MC_SERVICE_INTERRUPTS` once per second. What part 2 must provide
at this point:
- a host channel (TSG with GR context, compute object) for the guest GR channel, with the
  guest's GPFIFO/USERD (vidmem, in the reservation) and the guest VAS root; the promoted
  context buffers are recorded per channel in `Registry::promoted`;
- the push itself: `SET_INLINE_QMD_ADDRESS_A/B` 0x318/0x31c and 64 `LOAD_INLINE_QMD_DATA`
  0x320-0x41c; the QMD names the program (I2M'd before), the constant buffers and the
  release semaphore (dwords 23-25), which the GPU writes at grid completion;
- for blocking sync (cuda-launch): non-stall interrupts for GR0 (and CE2-4 for their
  channels) delivered to the guest (cuda-lookahead.md §4.8), not modelled;
- the CE channel that follows must see the semaphore the SMs wrote, so the channels of one
  context must run on the same engine side (all on the host, or a CE acquire satisfied from
  host memory).

**Not done.** Non-stall interrupt vectors (only needed with launches); POST_EVENT for compute
events; the cuda-managed 801 (UVM managed allocation, guest side). Suites were not run to the
end (the full gate run c3 was stopped during the unscored suites to keep the schedule; its
per-test results are `c3-full-gate/results.txt`).

**Launcher fix.** `probe` and `dma` unload `nvidia_uvm` before `nvidia` and stop with an error
when either refuses (a loaded `nvidia_uvm` made `probe` continue with the old driver).
`tools/caraxes-gate.sh` takes `CARAXES_GATE_RUNS` (the directory for its run directories).

**Host GPU and host state touched.** RM API only, from the device's unprivileged clients:
the forwarded controls above (GR masks, PES info, GSS legacy 0x59/0x62), plus the existing
reservation and registration (4 privileged `GET_SURFACE_PHYS_ATTR` windows per VM start, six
starts). Host files read: `/usr/lib/libcuda.so.610.57.04` (hash). No register access.

### E5b part 2: execution on the physical GPU (2026-09-23)

Design and the mirror-object table are in [execution.md](../caraxes/execution.md). On vm2 with
`CARAXES_HOST_EXEC=1` (needs `--reserve-vram --register-guest-ram --host-rm`); host channel
execution behind `CARAXES_MIRROR_CHANNELS=1`. Evidence: `caraxes/runs/e5b2-2026-09-23/`.

**What was built** (`crates/vfio-user-nvgpu`): `fb.rs` `VramStore` (option C: the FB on the
reserved VRAM through LRU BAR1 windows, behind a no-op write filter); `mirror.rs` (the mirror
of every guest VA space / TSG / channel / CE object, the doorbell forwarding, the host OS
events, the TLB-invalidate forwarding, teardown); `intr.rs` `IntrShared` (a host event thread
raises guest vectors + MSI); `hostmem.rs` (raw RM alloc/control/free, BAR1 windows, the
doorbell map, OS events); `channel.rs`, `ctrl.rs`, `boot.rs`, `device.rs`, `main.rs` wiring;
`chips/tu106/interrupts.toml` GR0/CE2-CE4 non-stall vectors. `check.sh` clean; 53 unit tests.

**Verification (expectation: real VRAM and the physical CEs, with the launch as the wall).**

(a) **VRAM store.** Boot `RmInitAdapter succeeded!`, nvml gate **pass** (11/11), `uvm verify 1`
`rmStatus 0x0` — all on the reserved VRAM, no Xid (`a-*`). The FB store held 2 BAR1 windows for
the whole session; the exit summary counts every access.

(b) **Core CUDA on real VRAM.** With host channels off (the clean default), cuda-init,
cuda-context, cuda-core and cuda-hostreg **pass** on the VRAM store, no Xid (`b*`, boot8).

(c) **CE channels on the physical GPU.** With `CARAXES_MIRROR_CHANNELS=1`, CUDA's copy
channels (guest chids 10-21) build, schedule and take their first doorbell on the physical
copy engines with **zero Xid** (`c-wcflush-*`). The unblocking finding: BAR1 is write-combined,
so the device must flush its windows (`sfence` + read-back) before the host doorbell, or the
PBDMA fetches stale VRAM (**PBENTRY**, Xid 32 intr0 `0x00040000`). cuda-core then times out on
the **first CE `LAUNCH_DMA`** not completing (host-exec.md open item 3), with no Xid.

(d..e) not reached: the kernel launch (cuda-jit) and the full gate wait on the two walls below.

(f) **Teardown.** `down` freed all 24 mirror channels (0 live), the descriptors (4096 MiB
unpinned, `foll_pin` balanced), the reservation and the client; host `nvidia-smi` VRAM
returned to its prior value (`f-final-teardown.txt`).

(g) **Attempt table.** Every Xid was contained to the device process (its pid), on our own
mirror channels, cause identified, host desktop unaffected. One hypothesis, one change, one
test per row; "RC" is the host RC notifier event the device receives (`exceptType` 0x20 = Xid
32 PBDMA, 0x26 = Xid 38). Evidence paths are under `caraxes/runs/e5b2-2026-09-23/`.

| Attempt (time) | Hypothesis | Change | Result | Exception | Evidence |
|---|---|---|---|---|---|
| a1 (18:01) | a host mirror (externally-owned VAS + `DMA_SET_PAGE_DIRECTORY` to the guest root, TSG, channel, CE object, schedule, doorbell) runs nvidia-uvm's first push | first mirror build, during `uvm verify 1` | the push never ran: the host RM clears the USERD it is given, so the guest's GPPut 1 became 0; `host-kick` of the token twice did not help | none | `a1-mirror-first-attempt.txt`, `a1-teardown.txt` |
| a2 (18:04) | restoring the guest's GPGet/GPPut after the host alloc lets the push run | USERD restore after the host channel alloc; 6 UVM CE channels mirrored (host chids 0x32-0x37) | each first push faulted; UVM: `NV_ERR_RC_ERROR`, guest Xid 154 | Xid 32 `intr 00800000` (bit 23 DEVICE: `SET_OBJECT(GP100_UVM_SW)` on subchannel 5, a class the host refused to the mirror with 0x1b `INSUFFICIENT_PERMISSIONS`), RC 0x20 x6 | `a2-host-xid.txt`, `a2-mirror-trace-excerpt.txt`, `a2-guest-uvm.txt`, `a2-teardown.txt` |
| verify (18:06) | channels carrying `GP100_UVM_SW` cannot run on a host PBDMA; keep them in software | `Mirror::eligible` refuses UVM-SW channels | `uvm verify 1` passes on real VRAM | none | `a-nvml-gate.*`, `uvm-verify*` |
| b1 (18:11-18:31) | CUDA's channels (GR + CE) can be mirrored the same way | cuda-init/context/core/hostreg with mirrored CUDA channels | a GR mirror was refused (`GPFIFO_SCHEDULE` `NV_ERR_INVALID_STATE` 0x40: `kchannelIsSchedulable_TU102` on an externally-owned VAS without a bound context) and the refusal left the host RM's USERD clear in place, so the software fallback saw GPPut 0 and the context hung; cuda-init passed; 48 refusals, 0 RC | none | `b1-README.txt`, `b1-teardown.txt`, `b1-gate-cuda-gpput-bug/` |
| b2 (18:37-18:45) | a replayed (shared-management) VAS built from the guest's own `FERMI_VASPACE_A` parameters and `COPY_SERVER_RESERVED_PDES` (root/PD2/PD1/PD0) makes GR schedulable and the host RM binds its golden context (c1) | VAS replay; GR channels 2-9 on host chids 59-66, runlist 0 | GR schedules and takes the doorbell; the first push executes on the SMs and faults: the host-built context lives in the host kernel's own tables, unreachable through the guest root (exec-formats.md §4.4); cuCtxCreate 719 | Xid 38 (`0000c597`) x9, RC 0x26 x9 | `b2-host-xid.txt`, `b2-mirror-trace-excerpt.txt`, `b2-gate-cuda-context-xid38/`, `gate-cuda-2026-09-23-2/` |
| b2b (18:47) | narrowing the replayed VAS's RM-internal range keeps the host's context buffers off the guest's VAs | RM-internal range = the guest's server-reserved range | the host `TURING_COMPUTE_A` alloc fails `NV_ERR_NO_MEMORY` (0x51) on every GR channel; the channel is scheduled with the CE object only; cuCtxCreate 719 | Xid 32 x8 (see run 10 for the decode), RC 0x20 | `b2b-gate-cuda-xid32/` |
| c-pbentry (18:55) | CE channels alone (GR in software) run on the physical copy engines | mirror only CE channels (COPY2-4 TSGs) | every CE mirror RCs at its first doorbell: the PBDMA fetched stale VRAM | Xid 32 `intr0 00040000` (bit 18 PBENTRY) on host chids 55/56 = guest 10/11, pid 3670875; RC 0x20 | `c-pbentry-evidence.txt`, `gate-cuda-2026-09-23/` |
| software (19:08) | with host channels off, everything runs on the VRAM store | `CARAXES_MIRROR_CHANNELS` unset | cuda-init, cuda-context, cuda-core, cuda-hostreg **pass** on real VRAM | none | `gate-cuda-2026-09-23-3/` |
| c-wcflush (19:16) | BAR1 is write-combined: the GPFIFO/USERD/page tables the device wrote had not drained when the host doorbell rang | `Window::flush` (`sfence` + read-back) before every host doorbell and TLB invalidate | the 12 CE channels build, schedule and take their first doorbell with **zero Xid**; the first CE `LAUNCH_DMA` (the channel-init semaphore release, `SET_OBJECT` + a four-word CE semaphore release) never completes; exactly one doorbell per channel, cuda-core times out at 90 s | none, no RC | `c-wcflush-ce-firstlaunch.txt`, `gate-cuda-2026-09-23-5/`, `f-final-teardown.txt` (24 mirrors freed, 0 live) |
| c-comptag (19:36) | the stall is a compressible KIND / COMPTAGLINE in the guest's vidmem PTEs driving the L2 comptag path into the COMPR-NONE reservation (memory-model.md §6, the D16 hazard) | `supportCompressed = 0` in the static-info FB regions; the walker audits every vidmem PTE kind | software-mode cuda-context and cuda-core pass; every vidmem PTE walked is kind 0x00/0x06 with COMPTAGLINE 0 (175423 + 59648): the comptag hold is satisfied by evidence | none | `c-comptag-verified.txt`, `gate-cuda-2026-09-23-6/` |
| c-mirror-ce (19:43) | with compression off the CE first launch completes | CE mirror on, compression off | cuda-core still times out (120 s): 24 doorbells, one per CE channel, then the stall; zero Xid, no host effect; the stall is independent of compression | none | `c-mirror-ce-comprfix.txt`, `gate-cuda-2026-09-23-7/` |
| kbytes (19:56) | record the exact kernel bytes and QMD of cuda-jit for a host-side replay | software mode, `CARAXES_DUMP_KERNEL=1` | the I2M uploads (SASS, constant and parameter buffers) and the 64-dword QMD captured; cuda-jit itself times out (the launch is refused in software) | none | `kernel-bytes/kernel-bytes.txt`, `gate-cuda-2026-09-23-8/` |
| run 9 (20:03) | cuda-jit with the CE channels on the host and GR in software | `CARAXES_MIRROR_CHANNELS=1`; a USERD read-back right after the host doorbell | the 12 CE channels each take one doorbell; USERD immediately after the doorbell reads GPGet 0 / GPPut 1 on all of them; cuCtxCreate never returns, cuda-jit times out at 90 s | none, no RC | `gate-cuda-2026-09-23-9/` |
| run 10 (20:12) | c2: the replayed VAS plus a privileged `GPU_PROMOTE_CTX` on the host GR channel with the guest's recorded entries | `CARAXES_MIRROR_GR=1` (`mirror.rs` `promote_gr`), VAS replay with the guest's parameters + `VA_INTERNAL_LIMIT`, 3 levels handed over | the replay is NV_OK, but the host `TURING_COMPUTE_A` alloc is refused `NV_ERR_NO_MEMORY` (0x51) on all 8 GR channels (the same refusal as b2b), so `promote_gr` never ran; each GR channel was scheduled on runlist 0 with only its CE object and its first doorbell RC-ed; cuCtxCreate 719, twice (the guest retried: host chids 0x3b-0x42, then 0x37-0x3d). **Second finding:** at the retry the CE channels' USERD pages still held the physical GPU's GPGet 1 from the first attempt (`the guest's GPGet 1 / GPPut 1 restored`), i.e. the physical PBDMA *had* consumed the first batch's entry after the immediate read-back saw 0, and the fake GSP does not clear a USERD at channel alloc as the RM does | Xid 32 `intr0 00000000 intr1 80000000` x2 per channel (PBDMA INTR_1 bit 31 = CTXNOTVALID, `nouveau/engine/fifo/gv100.c` `gv100_runq_intr_1_ctxnotvalid`: the channel has no valid engine context, there being no compute object), RC 0x20 x13; all pid 4075062 | `gate-cuda-2026-09-23-10/` (`device/cuda-jit.trace` lines 6240-6473, 26182-26242; `guest/cuda-jit.log`), host journal 20:12:53-20:13:48 |

| d-ce-diag (20:40) | with the CE channels on the host, decode the first push and poll the mirror USERD after the doorbell | `fb.rs` `walk_in` (raw PTE), `ctrl.rs` read-only push diagnostic (GPGet poll, entry/pushbuffer decode, semaphore read-back), `boot.rs` clears the guest USERD at channel alloc as GSP-RM does | nvprobe 1 (cuCtxCreate) run: the physical PBDMA fetched each CE channel's own GPFIFO entry (GPGet 1 == GPPut 1 in ~24 us) and the read-back showed the guest's four-word CE init semaphore in guest sysmem holding payload 0x1 + a GPU timestamp — guest-authored CE work executed end to end through the guest's page tables; cuCtxCreate still timed out (context needs GR, still in software) | none, no RC | `d-ce-diag/` |
| d-ce-token (20:48) | after the first submission the guest reads the *host* work-submit token back from its notifier page and rings that, so the device must map the host token to the guest channel | `mirror.rs` `chid_of_host_token`, `ctrl.rs` doorbell routes a host token to its guest channel | the guest's later doorbells (`token 0x8003a chid 58`) now reach the mirror; 88 CE copies ran (H2D, src sysmem -> dst VRAM, payloads landing), then a fresh copy dst 0x100_05200000 faulted **FAULT_PDE** (the guest had just mapped that 2 MiB block and its register invalidate never reached the physical TLB) | Xid 31 MMU Fault ENGINE CE2 (contained) | `d-ce-token/`, host journal 20:49:31 |
| d-ce-inval (21:03) | replay the guest's register TLB invalidate to the physical GPU with a device-owned channel issuing `MEM_OP MMU_TLB_INVALIDATE` of the guest PDB | `invalidate.rs` (a device host channel in the reservation carve-out) | the first `MEM_OP` executed (412 us), but the method is refused on a user-level channel: the channel RC-ed with PBDMA `METHOD` (a privileged method on a non-privileged channel) and every later push went unacknowledged | Xid 32 `intr 00200000` (bit 21 METHOD) on the invalidator's own channel | `d-ce-inval/`, host journal 21:04:31 |
| d-ce-inval2 (21:16) | allocate the invalidator channel under CAP_SYS_ADMIN so its AUTH_LEVEL is ADMIN and the privileged `MEM_OP` is accepted | `hostmem.rs` `alloc_object_admin`; the invalidator channel allocated in one CAP_SYS_ADMIN window | the `MEM_OP MMU_TLB_INVALIDATE` now runs in 22-40 us and **FAULT_PDE is gone**; the wall moved back to the CE first launch: each CE channel's first `LAUNCH_DMA` (a four-word semaphore release with FLUSH, transfer 0) on a freshly scheduled host channel stalls and the watchdog RCs it (host-exec.md open item 3), before any invalidator push fired | Xid 32 `intr1 00000008` HCE (chid 10) and `intr0 00040000` PBENTRY (chid 11) on the CE init pushes (contained) | `d-ce-inval2/`, host journal 21:17:28 |

| d-ce-ab (21:29) | A/B: invalidator on, nvprobe 1 (same stage as the clean d-ce-diag) to isolate the invalidator | invalidator default-on | **0 RC, 0 Xid**, 101 CE semaphores all landed (no first-launch stall); cuCtxCreate timed out only because the per-doorbell 20 ms poll diagnostic serialized the datapath | none | `d-ce-ab/` |
| d-core-fast (21:34) | gate the slow per-doorbell diagnostic and run cuda-core with the CE channels on the physical GPU | `ctrl.rs` `mirror_diag` (`CARAXES_MIRROR_DIAG=1` opt-in), diagnostic off by default | **cuda-core (nvprobe 2) PASSES**: stage 0 (cuInit), stage 1 (context), stage 2 (memory written/read/compared) all ok on the physical copy engines, rc=0, **0 RC, 0 Xid**. The CE first-launch stall does not occur; the earlier CE RCs (d-ce-inval2) were the diagnostic's 20 ms-per-doorbell poll disturbing the datapath, not the CE launch | none | `d-core-fast/` |

| d-gr-c2 (21:38) | c2 with the corrected replayed VAS (guest params unchanged) and the promote of the guest's recorded entries onto the host GR channel, before scheduling | `mirror.rs` `promote_gr` before `GPFIFO_SCHEDULE`; VAS replay with the guest's `FERMI_VASPACE_A` parameters unchanged (no `VA_INTERNAL_LIMIT`), 3 levels handed over | the `GPU_PROMOTE_CTX(admin)` of the guest's MAIN (pa 0x1e8512000 init) + PATCH (pa 0x1e8aab000 init) + FECS_EVENT/PRIV_ACCESS_MAP VAs is **accepted (NV_OK)** and the GR channel schedules on runlist 0, but its first compute push still faults: FECS loads the host kernel's own GR context (allocated at `TURING_COMPUTE_A` on the replayed VAS, in the host kernel's tables), which the guest root does not map — the exec-formats.md §4.5 catch-22. Promoting to the 2nd..8th channel then failed at the ioctl (the same guest buffers cannot be promoted onto a second host channel). cuCtxCreate 719. Nothing green regressed: the CE channels ran clean, only GR RC'd | Xid 38 (`0000c597`) on the GR channel (contained); RC 0x26 | `d-gr-c2/`, host journal 21:39:19 |

| d-gr-shadow (22:03) | c1s: mirror the guest's set-A into a normal host-managed VAS via fixed-VA `RM_MAP_MEMORY_DMA` (D17 route 4 stage 1). First probe: single [0,1TB) `NV50_MEMORY_VIRTUAL` at base 0, map guest leaves at their VAs | `fb.rs` `enumerate_leaves` (walk the guest root for all present PTEs), `shadow.rs` probe, `hostmem.rs` `map_dma_fixed`/`alloc_virtual` | the guest root has 4096 present leaves (vidmem 4K 243/64K 8/2M 2; sysmem 4K 3842; 0 compressible — D16 clean). All 13 sampled maps refused `NV_ERR_INVALID_ARGUMENT` | none | `d-gr-shadow/` |
| d-gr-shadow2 (22:11) | A/B the hDma: NV50_MEMORY_VIRTUAL vs the VAS handle | log the NVOS46 detail | hDma = VAS handle → `NV_ERR_INVALID_OBJECT_HANDLE` (0x33: a bare VAS is not a valid hDma); hDma = NV50_MEMORY_VIRTUAL → `NV_ERR_INVALID_ARGUMENT` (a `DMA_OFFSET_FIXED` map only lands inside the VA range the virtual object reserved, so a single base-0 object was wrong) | none | `d-gr-shadow2/` |
| d-gr-shadow3 (22:16) | one FIXED_ADDRESS `NV50_MEMORY_VIRTUAL` per run at the guest VA (page-size attr = the guest level), then map into it at the same VA | `hostmem.rs` `alloc_virtual_fixed` | **the mapping mechanism works: 9/16 sampled leaves mapped NV_OK — vidmem AND sysmem, 4K/64K/2M all NV_OK** (the reservation's 2 MiB HUGE attr did not block 4K/64K sub-maps). The 7 refusals (0x1a) were only where the probe allocated a *separate* 4 KiB virtual object at a non-64 KiB-aligned VA; the fix is one virtual object per aligned contiguous run (coordinator's guidance). Set-A mapping into a host-managed shadow VAS is therefore viable; the remaining c1s work is run-grouping, the GR channel on the shadow VAS, and re-sync at each guest MMU_INVALIDATE, watching for a VA collision with the host RM's own context buffers | none | `d-gr-shadow3/` |

| d-gr-shadow4 (22:29) | full set-A sync into the shadow VAS with set-B excluded (`shadow.rs` `ShadowVas`, run-grouped fixed-VA maps) | one FIXED_ADDRESS virtual object per contiguous same-page-size run | **7947 set-A leaves mapped NV_OK into 3 runs, 0 failed, 0 collisions** — the guest's set-A is highly contiguous and mirrors cleanly into a host-managed VAS | none | `d-gr-shadow4/` |
| d-gr-launch..launch3 (22:34-23:00) | put the GR channel + TSG on the shadow VAS, alloc the compute object (host builds set B), schedule, forward the doorbell; resync at each GR doorbell | `mirror.rs` `gr_shadow_doorbell`; enlarge the shadow VAS to 1<<47; cumulative set-B exclusion | the GR channel builds, maps 15372 set-A leaves (0 fail, 0 collision after the fixes), allocs the compute object and **schedules with NO Xid 38** — the c1 context wall is broken. Collisions found and fixed: 3 runs at VA ~1 TB (beyond the old 1<<40 vaSize) and 3 at the set-B context VAs (0x120xxx) that chids 3-9 did not exclude. Launch still hung (rc=124) | none | `d-gr-launch3/` |
| d-gr-launch4 (23:00) | one-shot USERD read after the GR doorbell to see if the push was fetched | `gr_shadow_doorbell` GPGet/GPPut read | GPGet 0 / **GPPut 0** — the host RM cleared the channel's USERD at creation and the GR path never restored the guest's GPPut, so the PBDMA saw no work (the same USERD-clear the CE path already handles) | none | `d-gr-launch4/` |
| d-gr-launch5 (22:58) | restore the guest's GPGet/GPPut after the host clears the USERD | `gr_shadow_doorbell` USERD restore | **the GR PBDMA fetched and ran the guest's first push (GPGet 1 / GPPut 1) — the guest's compute work executed on the physical GPU with no Xid 38 and no hang.** It then MMU-faulted: Xid 31 FAULT_PDE, ENGINE GRAPHICS HUBCLIENT_FE, VA 0x100_04e00000 (a set-A I2M upload target) — a set-A page not mirrored in the shadow VAS | Xid 31 (contained) at 0x10004e00000 | `d-gr-launch5/`, host journal 22:58:58 |
| d-gr-launch6 (23:04) | resync the shadow VAS at every guest MMU_INVALIDATE (not just at GR doorbells) | `boot.rs` MmuInvalidate -> `ctrl.mirror_shadow_resync` -> `ShadowVas::sync` | the resync now maps set-A pages the guest adds during the launch (512+ new maps), but "map-failed" counts grow (3, 513, 2293) and the **same VA 0x10004e00000 still faults**: a VA remapped to a new PA needs unmap-before-remap (no NVOS47 unmap yet), and a coalesced run that grows outgrows its fixed virtual object. The GR push still executes; this is a set-A mirror-completeness bug, not a context or scheduling one | Xid 31 (contained) at 0x10004e00000 | `d-gr-launch6/`, host journal 23:04:49 |

| d-gr-launch7 (23:11) | apply the three research-doc fixes: uncap the guest-root walk (1<<20, log truncation + refused-leaf count), give the shadow GR channel the guest's error notifier (`cp.hObjectError`, so a fault returns 719 not a 30 s hang), resync the shadow VAS before ringing (not after) | `shadow.rs` WALK_MAX + refused counter, `mirror.rs` `gr_shadow_doorbell` error notifier + resync-first | the guest's first GR push executes (GPGet 1), the c5b5 copy object allocates on the GR channel (no GRCE refusal), **no Xid, no RC** — but the launch hangs (rc=137 timeout). The dynamic set-A mirror then breaks: as the guest maps more set-A between pushes, a coalesced run grows past its fixed `NV50_MEMORY_VIRTUAL` object and remapped VAs are not unmapped first, so `map-failed` explodes (3 -> 8214 -> 119976). The remaining walls are robust dynamic set-A mirroring (fixed-granularity per-region virtual objects + unmap-before-remap, NVOS47) and the first-push completion (the report semaphore the guest waits on) | none (contained: 0 Xid, 0 RC) | `d-gr-launch7/` |

| d-gr-mirror1 (2026-09-23 23:42) | the launch hang of d-gr-launch7 is the set-A mirror, not the context: a diffed mirror with fixed-granularity virtual objects, unmap-before-remap and one host TSG per guest TSG lets cuCtxCreate and the vecAdd launch complete | `shadow.rs` rewritten: one `NV50_MEMORY_VIRTUAL` per 2 MiB region and page class (2 MiB, or 4K/64K under `PAGE_SIZE_DEFAULT`), never grown, freed when empty or on a class change; per-leaf objects (`ALIGNMENT_FORCE`) where a region touches set B or is refused; VA -> (target, size) map diffed each resync, `NV_ESC_RM_UNMAP_MEMORY_DMA` (NVOS47, `hostmem.rs` `unmap_dma`) before a remap; filter needs the whole leaf inside the reservation / a registered range, each refusal logged once with VA, PA, aperture, reason. `mirror.rs`: the shadow GR channels of a guest TSG share one host TSG (`self.tsgs` keyed by the guest TSG). `hostmem.rs` `Window::read` qword reads and `fb.rs` `enumerate_leaves` block reads (the byte-wise uncached BAR1 reads made one walk ~250 ms, now ~30 ms) | **nvprobe 3 passes on the physical GPU: "stage 3 ok (kernel, result correct)"** — the first CUDA kernel launched by the stock guest driver ran on the RTX 2070. 114 GR doorbells, chids 3-9 joined host TSG 0x80000066; 54 syncs, 19375 leaves mirrored in 87 region + 7 leaf objects, 0 map-failed, 2 unmaps; the filter refused 1 leaf (va 0x100_04c00000 -> FB 0x2000 kind 0xf, outside the reservation, never mapped, not needed). A first attempt in the same directory failed cuInit 100 only because the guest had no `/dev/nvidia*` nodes | none: 0 Xid, 0 RC, 0 `MC_SERVICE_INTERRUPTS` | `e5b2-2026-09-24/d-gr-mirror1/` |
| d-core-mirror1 (23:45) | the CE path is unaffected | nvprobe 2, same device instance | pass (stages 0-2) | none | `e5b2-2026-09-24/d-core-mirror1/` |
| d-launch4-1 (23:46) | 100 spin launches run | nvprobe 4, same instance | **pass, 68.449 ms/launch**: every GR doorbell and every guest MMU_INVALIDATE resyncs with a full ~30-40 ms walk (326 doorbells, 166 invalidates) | none | `e5b2-2026-09-24/d-launch4-1/` |
| d-launch4-2 (23:50) | skip the walk when no page-table page changed | `fb.rs` watches the FB table pages of the last walk (`enumerate_leaves_watched`, `note_write` on the single FB write path; BAR1 is fully trapped, so every guest PTE write is seen; a table in sysmem disables the skip); `CARAXES_SHADOW_ALWAYS_RESYNC=1` restores the full walk | **pass, 18.221 ms/launch** (the spin kernel is 30M cycles, about its own run time); 47 walks instead of ~490 | none | `e5b2-2026-09-24/d-launch4-2/` |
| gate cuda (23:54) | the cuda gate's kernel stages run on the physical GPU | `tools/caraxes-gate.sh cuda cuda-jit cuda-launch cuda-torch rlprobe`, fresh `sync-tests` | **4/4 pass**: cuda-jit, cuda-launch (blocking sync, 100 launches), **cuda-torch (torchprobe stage 5, cuDNN convolution verified)**, **rlprobe (mean10=56.9)** | none: 0 host Xid | `e5b2-2026-09-24/gate-cuda-2026-09-23/` (the gate names its directory by the date it was started with) |
| gate gpu (2026-09-24 00:00-00:15) | the v1 gpu gate on the same build | `tools/caraxes-gate.sh gpu` | FAIL, 1 of 7 stages passes (memory); the six failure reasons: smi fail (masked diff of 2 lines, the device's own name "Leandro RTX 2070-4G" against the host card, `smi-unmasked.txt`), gpu-memory **pass**, gpu-kernel fail (`managedprobe` `cuMemAllocManaged` 801 operation not supported: the UVM managed-memory phase), gpu-torch **pass** on the guest side (mean10=56.9) but the gate fails it for lack of a native reference (`caraxes/reference/rtx2070/torch-native.txt` missing; `CARAXES_GATE_NATIVE=1` would run it on the host card), gpu-robustness fail (0 of 3 runs correct: two nvprobe 3 in parallel), gpu-own fail (holder did not start, pid 4531), gpu-encode fail (no ffmpeg in the baked image) | **17 x Xid 31** (00:08:54-00:12:59), all pid 789512 = the device, all on its own shadow GR channels (host chids 0x3b, 0x3f-0x46, 0x59-0x60), `HOST0 HUBCLIENT_HOST FAULT_PDE VIRT_READ` at the second context's GPFIFO VAs (0x2_00200000 + 0x3000 k, and 0x0_03a00000): the shadow VAS was one per device, so a second CUDA context's GR channels ran on a VAS that mirrored the first context's root (both use the same VAs); host desktop unaffected | `e5b2-2026-09-24/gate-gpu-2026-09-23/` (`host-xid.txt`, `table.txt`) |
| d-gr-perroot1 + gate cuda (00:18-00:35) | one shadow VAS per guest root fixes the parallel contexts | `mirror.rs` `shadows: BTreeMap<guest root, ShadowSpace>` (the VAS, its set-B exclusions, its guest chids and TSGs; freed when the last of both is gone), the doorbell resyncs its channel's root, `MMU_INVALIDATE` resyncs the invalidated PDB (all on ALL_PDB); `fb.rs` page-table watch per root | nvprobe 3 pass; gate cuda cuda-core, **robustness** ("2 in parallel ok, next run after SIGKILL ok"), cuda-launch **pass**; teardown: 140 mirror channels created and freed, 0 live, 1773 doorbells, 0 host RC events, 4096 MiB unpinned, reservation freed | none: 0 Xid | `e5b2-2026-09-24/d-gr-perroot1/`, `e5b2-2026-09-24/gate-cuda-2026-09-24/` |
| d-uflush (2026-09-24 01:30-04:34) | the ~68 s `cuCtxCreate` after a few CUDA processes (bench `v2-hang`: guest RM `kmemsysDoCacheOp_GM107` timeouts on reg 0x70010 and 0x70004, 30 s each, twice per context) is the BAR0 latch reading back `PENDING_BUSY` of the L2 cache-operation registers | `boot.rs` owns `NV_UFLUSH_*` 0x70000-0x70010 and the Turing VF-window invalidates 0xB80F00/0xB80F04 (chip.toml `l2_*`): PENDING/OUTSTANDING read back clear; with host execution FLUSH_DIRTY and SYSMEM_INVALIDATE are first honoured on the physical GPU by `NV2080_CTRL_CMD_FB_FLUSH_GPU_CACHE` (NON_PRIVILEGED, the device's subdevice; flags 0x15 / 0x19); unit tests `bar0::l2_cache_operations_complete`, `ctrl::flush_gpu_cache_parameters` | **pass** on vm2 (slot 0, fixed build): the v2-hang sequence (nvprobe 1/3, bench-cu init, 10 processes) takes 8.8-10.2 s per process instead of ~69 s, `ctxcreate_ms` 5263-5849; 24 FLUSH_DIRTY + 24 SYSMEM_INVALIDATE, each polled once and read complete; 0 `kmemsysDoCacheOp` timeouts in the guest; nvprobe 2 ok, nvprobe 3 correct x3, nvprobe 4 blocking 18.870 ms/launch; no host flush failure logged | none | `e5b2-2026-09-24/d-uflush/` |

**c1s status (2026-09-23, the GR launch frontier).** The shadow-VAS route (D17 stage 1)
broke the Xid-38 context wall: a GR channel on a host-managed VAS, with the host RM's own GR
context (set B) and the guest's set-A mirrored in by fixed-VA `RM_MAP_MEMORY_DMA`, schedules
and **executes the guest's first compute push on the physical GPU with no Xid and no RC**
(d-gr-launch5..7). What remains for the first landed launch: (1) robust dynamic set-A
mirroring — the sync must allocate virtual objects at a fixed granularity that does not grow
(per 2 MiB-aligned region + page size) and unmap-before-remap a VA whose PA changed
(NVOS47), instead of the current grow-and-retry that makes `map-failed` explode; (2) the
first-push completion — the guest waits (sched=auto) on the push's report-semaphore value in
memory, so confirm that semaphore VA is mirrored and the GR write lands where the guest reads
it; (3) then the shared-context fix (one host TSG per guest TSG, all the guest group's GR
channels inside it) so a launch on any of the guest's channels 2-9 sees the context the first
push built. All behind `CARAXES_MIRROR_GR_SHADOW=1`; the default and the CE path are unaffected.

**c1s result (2026-09-23, 23:40-23:59; evidence under `caraxes/runs/e5b2-2026-09-24/`).** The three walls named above are closed. (1) The set-A mirror is
a diff with fixed 2 MiB region objects and unmap-before-remap; (2) the first push and every
later push complete (the report semaphores land where the guest reads them: libcuda
proceeded through cuCtxCreate, the vecAdd launch and 100 spin launches without a stuck
wait); (3) all shadow GR channels of a guest TSG share one host TSG and so one GR context.
The runs above put CUDA kernels, cuDNN convolutions (torch) and rlprobe on the physical GPU
with 0 Xid and 0 RC. The gpu gate then found a fourth wall: the single shadow VAS served every
guest root, so a second CUDA context faulted (17 contained Xid 31, all on the device's own
channels); with one shadow VAS per guest root the robustness stage (two contexts in parallel,
SIGKILL, rerun) passes with 0 Xid. `CARAXES_MIRROR_DIAG` was off in all of these runs. Open
gpu-gate items for the next package: gpu-kernel (`cuMemAllocManaged` 801, the UVM managed
phase), gpu-own (the holder process did not start; not re-run after the per-root fix), gpu-encode
(no ffmpeg in the guest image), gpu-torch (guest passes, the gate needs a native reference
file or `CARAXES_GATE_NATIVE=1`), gpu-robustness (fixed and passed in the cuda gate, not
re-run inside the gpu gate), smi (the masked diff is the device's own name).

**Cause of the run 10 / b2b `NV_ERR_NO_MEMORY` (read from the source, not yet tested):**
`NV_VASPACE_ALLOCATION_FLAGS_VA_INTERNAL_LIMIT` sets `VASPACE_FLAGS_RESTRICTED_RM_INTERNAL_VALIMITS`
and the RM-internal range to exactly `[4 GiB, 4.5 GiB)` (`vaspace_api.c:370-375`, `:642-644`);
on a GSP client the same range is the server-reserved one, which `gvaspaceReserveSplitVaSpace_IMPL`
takes out of the client's allocator (`gpu_vaspace.c:410-420`, `:329-333`), so the host kernel
has **no** internal VA left for the context buffers it maps at `TURING_COMPUTE_A` allocation.
Without the flag the internal range is the whole VAS (`gpu_vaspace.c:1119-1128`, the `else`
branch), which is b2's configuration, where the compute object allocated fine. The c2 replay
therefore needs the guest's parameters unchanged (b2) with 3 levels handed over, and the
promote after the compute object.

**TLB invalidation of the guest PDB (settled 2026-09-23).** `NV0080_CTRL_CMD_DMA_INVALIDATE_TLB`
on a mirror VA space is a no-op for the guest's page tables: `gvaspaceInvalidateTlb_IMPL`
invalidates the RM walker's own root, and an externally-owned VAS has no walker root (only
`pExternalPDB`, read by `gvaspaceGetPageDirBase` alone), so the guest's register
`NV_VIRTUAL_FUNCTION_PRIV_MMU_INVALIDATE` was never replayed to the physical GPU (FAULT_PDE,
d-ce-token). The device now keeps a small **privileged** host channel of its own
(`invalidate.rs`, a COPY2 channel in the reservation carve-out, allocated under CAP_SYS_ADMIN
so its AUTH_LEVEL is ADMIN — a GSP-client channel's privilege is the allocating client's, so
the privileged `MEM_OP MMU_TLB_INVALIDATE` method is accepted only there) and pushes a
`MEM_OP MMU_TLB_INVALIDATE` of the guest PDB (all levels, GPC, ack globally, sysmembar) for
each guest register invalidate; it then polls a host semaphore so the invalidate is
acknowledged before the next doorbell. With it, fresh guest mappings resolve on the physical
GPU (`d-ce-inval2`). `CARAXES_MEMOP_INVALIDATE=0` falls back to the no-op `DMA_INVALIDATE_TLB`.

**CE first-launch stall (still open).** With the TLB fixed, the remaining CE wall is
host-exec.md open item 3: the first CE `LAUNCH_DMA` on a freshly scheduled host channel does
not complete and the watchdog RCs it (a contained Xid 32, HCE or PBENTRY, on the init push).
host-exec absorbed this with a warm-up push on a channel it controlled; the mirror needs the
same, issued before the guest's first doorbell.

**The two walls** (execution.md §6): the CE first-launch stall (a warm-up push, unresolved),
and the GR context on a host channel (c1 faults, c2 unestablished). GR and UVM channels run in
software on the VRAM store meanwhile; the kernel launch is refused there as in E5c.

**Host GPU and host state touched.** RM API only, from the device's clients: the mirror
objects (VA spaces, TSGs, channels, CE objects, `VOLTA_USERMODE_A`, OS events), the two
privileged controls (`DMA_SET_PAGE_DIRECTORY`, `VASPACE_COPY_SERVER_RESERVED_PDES`) inside the
scoped CAP_SYS_ADMIN window, the reservation, the registration. No register access, no host
module change, no passthrough. `caraxes/tools/host-kick.rs` (a doorbell diagnostic) touches
only the usermode doorbell of its own throwaway channel.

## 615 transport (2026-09-24)

- **Goal (decision D19, the 615 baseline):** a 615.71.09 guest boots against the device:
  message queue v2 with the pointers in registers, MCTP-framed elements, `GMCAPI_CMD_GSP_INIT`
  with NVKV system info and static info, the 615 boot arguments, and the 615 control layouts
  where they differ ([gsp-transport-615.md](../caraxes/gsp-transport-615.md)). The 610 path stays as it
  was. Target: `RmInitAdapter succeeded!` with the 615 guest, and `nvidia-smi` answering.
- **Starting point:** the 610 device stops the 615 guest at `GspStatusQueueInit: msgqRxLink
  failed: -7` ([guest-driver-versions.md](../caraxes/guest-driver-versions.md) §5.1). The pre-boot
  `QUEUE_HEAD(0) = 0` counts as a doorbell, and the 615 `GSP_ARGUMENTS_CACHED` is read with the
  610 offsets.
- **What changed in `crates/vfio-user-nvgpu`:** `msgq/v2.rs` (behind `QueueState`), `gmcapi.rs`,
  `ver/v615.rs` (feature `v615`, which implies `v610`; the host side keeps the 610 layouts) and
  additive hooks in `boot.rs` (link at Booter-Load, the register pointers, the `GSP_INIT` and
  `GSP_SUSPEND` arms). `rpc::static_info_nvkv` encodes the static info. `kgr.rs` widens the SM
  order. `main.rs` gained `--guest-abi` (fail closed), and `tools/caraxes-vm.sh` gained
  `CARAXES_GUEST_ABI=615`. The chip data gained the three pointer registers. The file list is
  in gsp-transport-615.md §10.
- **Expected:** the status header lands before the guest's 4 s link window closes, `GSP_INIT`
  is answered with static info the kernel's key handler accepts, and the phase-5 RPCs follow
  unchanged in their RPC headers. A falsifier would be a key refused by
  `_kgspStaticInfoKeyHandler` (`GSP_INIT` failing with 0x1f), a control layout mismatch showing
  up as an assert or a failed `RmInitAdapter`, or the queues wedging on the pointer semantics.
- **Unit tests:** `cargo test -p vfio-user-nvgpu`: 119 library tests and the 2 protocol tests
  pass. That is the 102 existing ones unchanged and 17 new ones: `msgq::v2` 7,
  `gmcapi` 6, `rpc::tests::static_info_as_nvkv_keys`, `boot::tests::boot_615_link_and_gsp_init_from_the_captured_guest`,
  `boot::tests::a_610_device_names_a_615_guest` and `ver::abi_tests`. The 615 transport tests
  run in the default build. With `--features v615` the three `ver::v615` tests pass as well
  (Appendix A of gsp-transport-615.md, the versioned layouts, the reused id); 111 of 122 pass
  there. The 11 that fail are pinned to 610 numbers by construction: the six v0 queue tests
  (their element header is 24 bytes in that build), four kgr layout pins and mocks, and the
  `GSP_GET_FEATURES` version string in `ctrl::tests::answers_have_the_request_size`. The clippy line
  (with and without `--all-features`) and `cargo doc` are clean for the crate.
- **Run** (vm3, pure emulation, no reservation, no host execution; the 615.71.09 overlay
  `vm3/g615` of guest-driver-versions.md §7):
  `cargo build --release -p vfio-user-nvgpu --features v615 --target-dir target/v615`, then
  `VM=vm3/g615 CARAXES_TAP=caraxes3 CARAXES_IP=192.168.103.23 CARAXES_MAC=02:00:00:00:00:23
  CARAXES_HOST_RM=1 CARAXES_BOOT_MODEL=1 CARAXES_GUEST_ABI=615 tools/caraxes-vm.sh up; wait`.
  The guest image loads nvidia at boot (nvidia-persistenced), so the first boot is the first
  probe. For a reload: stop nvidia-persistenced, `rmmod nvidia_modeset`, then
  `tools/caraxes-vm.sh probe <label>`. Evidence is in `caraxes/runs/transport-615-2026-09-24/`.
- **Observed (target reached on the first run):**

  | Run | Result | Evidence |
  |---|---|---|
  | boot | pre-boot doorbell ignored (`queues: not linked yet`); linked at Booter-Load (`rmargs (615): size 184 flags 0x2 ... pages 129`, `queues linked (v2)`); `GMCAPI_GSP_INIT` (2368 bytes, 142 keys, 44 registry entries, driver "615.71.09") answered with 424 bytes of NVKV static info; 162 requests up to and including `nvidia-smi`; guest `RmInitAdapter succeeded!` 19.6 s after `RmInitAdapter` (the known PROM read of the VBIOS, as in 610) | `boot/guest.txt`, `boot/device-events.txt`, `boot/gpu.log` |
  | `nvidia-smi` | answers: `Leandro VFIO RTX 2070-4G`, 4096 MiB, KMD 615.71.09, CUDA UMD 13.4, UUID `GPU-9bd7b93d-3d23-c074-6704-984ef33ab7a7` (the 610 one); power reads `ERR!` | `boot/guest.txt` (`nvidia-smi -q`) |
  | reload a (probe `t615-reload`, device before the suspend fix) | `GMCAPI_GSP_SUSPEND` received; the guest then polled MAILBOX0 for 6 s for the processor-suspended bit, then Booter-Unload; function-level reset; relinked at the next Booter-Load; `RmInitAdapter succeeded!` | `reload-a/` |
  | reload b (probe `t615-reload-b`, MAILBOX0 bit 31 set on `GSP_SUSPEND`) | the unload continues at once (GSP reset 0.2 ms after `GSP_SUSPEND`); `RmInitAdapter` to `succeeded!` in 2.9 s; `nvidia-smi` and `nvidia-smi -q` answer | `reload-b/` |

- **The walls after init, all non-fatal:**
  1. `ALLOC_MEMORY` (RPC 4) from the new `goeimport` path is answered `NV_ERR_NOT_SUPPORTED`,
     and the guest logs `_goeimportSetupTransport: goeimport: step 3a failed (0x56)` and goes on.
  2. Power `ERR!` in `nvidia-smi`: the GSS legacy controls forwarded opaquely carry 615 sizes
     that the 610 host refuses with 0x1f (`pmgr 0x18`: 20512 bytes against the host's 20000;
     `thermal 0x36`: 24532 against 25552). Unanswered and new: `FB_GET_REMAPPED_BANKS`
     0x20801361 and perf 0x2080A0F1.
  3. The guest never sent 0x20800A36 (the id 610 used for chip info), and no RPC was rejected
     by the v2 framing checks.
- **Host GPU and host state touched:** RM API only, from the device's unprivileged client (the
  forwarded controls of `CARAXES_HOST_RM=1`). No reservation, no registration, no host
  execution, no register access, no module change. vm3 was stopped after the runs.

## Reference captures on the real card (opt-in, desktop off)

- **R1: full VBIOS through the PROM window.** Boot a VM with the real card via
  `vfio-pci` and `x-no-mmap=on`, trace `vfio_region_read` (QEMU `-trace`), and let the
  guest driver initialize; reconstruct the bytes read from 0x300000… This also
  captures the entire register sequence on real hardware (R3). Alternative without
  a VM: unbind `nvidia`, bind `vfio-pci`, and read `/dev/vfio/<group>` region 0 at
  0x300000 from userspace.
- **R2: BOOT_0/1/42 values.** Same capture, or an RM control
  (`NV2080_CTRL_CMD_GPU_GET_ARCH_INFO`) through `crates/nvrm-client`, which returns
  architecture/implementation/revision without touching registers (does not need the
  desktop off).
- **R3: complete MMIO trace of a real init.** As R1; the trace is the ground truth
  for the emulator's register model and orders the phase-5 accesses. It is the most
  valuable single artifact for the differential approach.

## R4: real-card reference capture under passthrough (weekend)

Planned 2026-09-29, not run. Opt-in, desktop off, every step run by the operator. Scripts:
`caraxes/tools/passthrough/`. For R4 this replaces sections 2-4 of
[passthrough-procedure.md](../caraxes/passthrough-procedure.md).

**Why.**
- On our device, Windows 11 with the stock 617.14 driver gets `TdrPayloadVSyncTimeout` about 5 s
  after the desktop starts flipping (docs/research/windows-findings.md, CH steps 10-15). R4
  records what the real TU106 answers, and when, with the same disk image.
- The display audit (`/mnt/vmstore/caraxes/ref/display-audit-2026-09-29.md`) names the
  suspects: no latch deadline before loadv, BEGUN written too early, GET := PUT at the kick.
  R4 measures them on the card (M1, M2 in R4.4).
- Secondary: the Linux 615 guest (NVKMS flips). By-product: R1/R3 (PROM reads, the real init
  sequence) from the pass B boot.

**Files** (`caraxes/tools/passthrough/`):

| File | Does | Runs as |
|---|---|---|
| `pt-host.sh` | `status`, `check`, `to-vfio`, `restore`, `reset flr\|bus` | root (`status`/`check` read-only, also as user) |
| `pt-vm.sh` | `prepare`, `preflight`, `cmdline`, `start`, `trace on\|off`, `mark`, `status`, `stop` | operator |
| `pt-guest.py` | Windows guest over SSH: `wait`, `pnp`, `mode`, `clock`, `etw`, `etw-boot`, `dwm-restart`, `task`, `bench` | operator |
| `pt-probe.py` | `capture`: reads the display registers through QEMU's vfio fd; `readout`: M1/M2 | root / operator |
| `qemutrace2gpu.py` | QEMU vfio trace to the device's `gpu-trace.log` format | operator |
| `trace-A.events`, `trace-B.events` | QEMU trace event lists of pass A and B | - |
| `etw-boot.ps1`, `set-mode.ps1` | guest: DxgKrnl as an AutoLogger; display mode | guest admin / signed-in user |
| `test_qemutrace2gpu.py`, `test_pt_probe.py` | unit tests on synthetic data (13 + 7) | - |

**Verified on this host (2026-09-29; no VM with the card started):**
- QEMU 11.1.1 `-device vfio-pci,help`: `x-no-mmap`, `x-no-kvm-msi`, `x-no-kvm-msix`,
  `x-no-kvm-intx`, `x-no-kvm-ioeventfd`, `x-no-vfio-ioeventfd`, `x-no-geforce-quirks`, `rombar`,
  `romfile`, `multifunction`. `x-no-mmap` covers every region of the device: stock QEMU cannot
  trap BAR0 alone.
- `-trace help`: `vfio_region_read` "(%s:region%d+0x%lx, %d) = 0x%lx", `vfio_region_write`
  "(%s:region%d+0x%lx, 0x%lx, %d)", `vfio_msi_interrupt`, `vfio_pci_read_config`,
  `vfio_pci_write_config`, `vfio_quirk_generic_mirror_read/write`,
  `vfio_quirk_nvidia_bar0_msi_ack`, `vfio_pci_reset`, `vfio_pci_reset_flr`,
  `vfio_pci_hot_reset(_result)`, `vfio_ioeventfd_init`, `vfio_rom_read`.
- The log backend writes `<event> <fmt>` (installed `tracetool/backend/log.py`);
  `-msg timestamp=on` prefixes host UTC with microseconds ("2026-09-29T16:35:10.596804Z ");
  `-D FILE` takes it; QMP `trace-event-set-state` switches events at run time without `-trace`.
  Checked on a device-less `-M none` QEMU; the full pass B command minus card, disk and TPM
  parsed on a paused 256 MiB q35 (`-S`), with the events of `trace-B.events` enabled.
- The NVIDIA quirk keeps BAR0 0x88000-0x88fff trapped even with mmap
  (`vfio-nvidia-bar0-88000-mirror-quirk`); the 0x1800 mirror exists only with `x-vga`.
- QMP `human-monitor-command "xp"` costs ~350 us per read, too slow for scan lines; so
  `pt-probe.py` borrows QEMU's vfio device fd (`pidfd_getfd`, root under yama ptrace_scope 1)
  and `pread`s BAR0 itself (tested on a child's file: ~0.6 us per call plus the card's read).
- Sysfs, read-only: `2d:00.0` 10de:1f02 `nvidia`, reset_method "flr bus" (DevCap FLR+; "bus"
  dates from before the siblings appeared, a bus reset needs all four); `.1` 10de:10f9
  `snd_hda_intel` (ALSA card0); `.2` 10de:1ada `xhci_hcd` (D3hot); `.3` 10de:1adb `nvidia-gpu`
  (D3hot, `ucsi_ccg` on it); `.1`-`.3` have no reset method. Group 28 is exactly these four.
  Port `00:03.1`: reset_method `pm` (its `reset` resets the port, not the card; section 4 of
  passthrough-procedure.md is wrong there), `reset_subordinate` present (the secondary bus reset
  of the four). BARs 16 MiB / 256 MiB 64-bit / 32 MiB / I/O 128 B / ROM 512 KiB.
- `vtcon1` "(M) frame buffer device" bound (nvidia-drm fbdev); `nvidia_drm` modeset=Y fbdev=Y;
  `/usr/lib/modprobe.d/nvidia-utils.conf`: softdep nvidia post nvidia-uvm nvidia-drm;
  `/etc/modprobe.d` empty. DRM nodes card1, renderD128.
- No display manager: Hyprland starts from the tty1 login (`start-hyprland`, session 2).
  **sshd is disabled and nothing listens on :22.** vfio modules not loaded. memlock limit of
  `silas` 8 MiB (soft and hard). `kernel.sysrq=16`. Monitors: DP-1 VG27A and DP-3 MG279, both
  2560x1440@144, both offer 1920x1080@60. `/mnt/vmstore` 89 GB free.

### R4.1 Preconditions and safety

Before the weekend (operator):
- sshd: `sudo systemctl enable --now sshd`; log in from the second machine with a key; there
  `tmux new -s r4`, detach (`C-b d`), `tmux attach -t r4`. Without this, no R4.
- Unplug DP-3 (MG279) from the card: one head, as on our device.
- The guest's update pause (`quiet.ps1 -Apply`, 7 days) must cover the weekend: renew it in
  the last Caraxes Windows run before R4 (`run44.py pack quiet.ps1 -Apply`).
- Optional input for the console: a USB keyboard on the card's USB-C port (function .2 goes
  to the guest), or Moonlight to the guest's Sunshine.

On the day, from SSH in tmux, in order:
```sh
tmux new -s r4
P=/mnt/vmstore/caraxes/wt-passthrough/caraxes/tools/passthrough   # in every tmux window
python3 /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build/run44.py stop   # if a Windows run is up
/mnt/vmstore/caraxes/night-2026-09-28/desktop-b-night.sh down                        # Linux B, if up
pgrep -a '^(vfio-user-nvgp|vhost-user-nvr|cloud-hypervis|qemu-system-x86)'          # must print nothing
loginctl list-sessions                        # the tty1 wayland session (2 today)
loginctl terminate-session 2                  # ends Hyprland, VSCode and the agent session
systemctl --user stop pipewire.socket pipewire-pulse.socket pipewire.service pipewire-pulse.service wireplumber.service
sudo $P/pt-host.sh check                      # must end with "check: ok"
```
- All Caraxes and Leandro VMs down: Windows (`run44.py stop`), every `tools/caraxes-vm.sh` VM
  (`down` with its env), v1 VMs. `check` blocks on any `vfio-user-nvgpu`, `vhost-user-nvrm`,
  `cloud-hypervisor`, `qemu-system-x86_64`, `crosvm` or `sunshine` process.
- Nothing may hold the card: `check` lists every open file on `/dev/nvidia*`,
  `/dev/dri/card1`, `/dev/dri/renderD128`, `/dev/snd/*C0*` and `/dev/vfio/28` (with sudo: all
  users). By hand: `sudo fuser -v /dev/nvidia* /dev/dri/card1 /dev/dri/renderD128 /dev/snd/controlC0`.
  Also blocking: a graphical session, a compositor, an active display manager or
  `nvidia-persistenced`, a group other than the four functions.
- A second tmux window watches the host: `sudo dmesg -w | grep -E --line-buffered 'vfio|AER|Xid|NVRM|2d:00|AMD-Vi'`.
- RAM (31 GiB host): with the desktop and all VMs down about 25 GiB are available. One VM at a
  time: 8 GiB guest RAM that VFIO pins (not swappable) plus ~0.5 GiB QEMU. `pt-vm.sh` refuses
  below guest + 6 GiB available. Traces go to `/mnt/vmstore/caraxes/r4` (never `/tmp`, a
  16 GB tmpfs); the converter streams (~40 MB RSS, ~100k lines/s). No builds during R4.
- Stop rules: an AER or Xid flood on the host, MemAvailable under 4 GiB, a STUCK step. Then:
  `$P/pt-vm.sh stop`, `sudo $P/pt-host.sh restore`, and the reboot path if needed.

### R4.2 Host switch

```sh
sudo $P/pt-host.sh to-vfio                                        # root
sudo prlimit --pid $$ --memlock=unlimited:unlimited && ulimit -l  # root, once per tmux shell: prints unlimited
```
- `to-vfio` runs `check` first (nothing changes on a block), then prints and does:
  1. fbcon off the frame buffer (`vtcon1/bind` = 0, remembered in `/run/caraxes-pt`);
  2. `modprobe -r nvidia_drm nvidia_modeset nvidia_uvm nvidia`, one by one with the refcount;
  3. `.1`, `.2`, `.3` unbound from `snd_hda_intel`, `xhci_hcd`, `nvidia-gpu`;
  4. `modprobe vfio-pci disable_idle_d3=1` (the functions stay in D0 while unused);
  5. `driver_override` = vfio-pci and `drivers_probe` for all four;
  6. `/dev/vfio/28` to the operator (QEMU runs as `silas`), then `status`.
- Idempotent: steps already done print "ok (already)". Each kernel step runs in the background
  with a timeout (30 s, module loads 120 s); a step still running is STUCK: the script prints
  its kernel stack and the reboot path and stops. Root runs log to `/var/log/caraxes-pt-host.log`.
- Back to the host (root): `sudo $P/pt-host.sh restore`: refuses while a VM holds
  `/dev/vfio/28`; vfio-pci off and overrides cleared; `.1`-`.3` probed back; `modprobe nvidia`
  and `nvidia_drm`; the kernel log since the start is scanned for `RmInitAdapter failed`, Xid,
  "fallen off the bus", GSP errors; fbcon rebound. Then, at the console: log in on tty1,
  `start-hyprland`; as `silas`: `systemctl --user start pipewire.socket pipewire-pulse.socket wireplumber.service`.
- Failure paths:
  - `check` blocks: nothing changed; fix the named item.
  - A module stays in use: `to-vfio` stops; `sudo $P/pt-host.sh restore` reloads what went.
  - `restore` reports INCOMPLETE (not on `nvidia`, or an error in the log): once
    `sudo $P/pt-host.sh to-vfio && sudo $P/pt-host.sh reset bus && sudo $P/pt-host.sh restore`
    (`reset bus` = `reset_subordinate` of 00:03.1; `reset flr` = FLR of `.0` only).
  - Reboot fallback (STUCK, or restore failed twice): `sudo systemctl reboot`; after 3 min
    without a reboot: `sudo sh -c 'echo 1 > /proc/sys/kernel/sysrq; echo s > /proc/sysrq-trigger; sleep 5; echo u > /proc/sysrq-trigger; sleep 5; echo b > /proc/sysrq-trigger'`;
    no SSH: the reset button. Afterwards: tty1 login, `start-hyprland`, recreate the taps
    caraxes6/caraxes7 (the first GPA launch after a reboot fails once).

### R4.3 Pass A: native baseline (Windows, full speed)

- VM (`$P/pt-vm.sh cmdline win A`): QEMU q35 + OVMF secboot + swtpm, as the maintenance boot of
  `run-qemu-gpu.sh`. The kit's `win11.qcow2` under an overlay (`r4/win/r4.qcow2`; the base
  stays as the Caraxes runs left it, and the overlay is refused once the base changes), copies
  of `OVMF_VARS` and the TPM state. User net plus the tap caraxes7 with the CH runs' MAC (SSH
  192.168.107.2). No emulated VGA, `-display none`: the card drives the monitor. All four
  functions behind one `pcie-root-port`, `multifunction=on`, ROM BAR on (OVMF runs the card's
  GOP). If QEMU cannot read the ROM (no picture before Windows, or a ROM error in `qemu.log`):
  `PT_ROMFILE=/home/silas/git/Caraxes/caraxes/reference/rtx2070/rom.bin $P/pt-vm.sh start win A A1b`
  (this card's ROM BAR capture, legacy and EFI images).
- Captures. ETW is `tools/windows/dxgk-etw.ps1` of wt-win (DxgKrnl, all keywords, level 0xff,
  the C61-C77 capture), so the CSVs compare; `etw-boot.ps1` is the same as an AutoLogger.
  Every ETW start and stop takes a guest-host clock pair (`clock.txt`, +-rtt/2, ~1 ms).
```sh
$P/pt-vm.sh prepare win
$P/pt-vm.sh start win A A1
$P/pt-guest.py wait 900            # first boot with the card: PnP installs 617.14 for the new instance
$P/pt-guest.py pnp                 # all 10DE functions OK; a pending restart: ssh Restart-Computer -Force, sleep 60, wait 900
$P/pt-guest.py mode 1920 1080 60   # the raster of our EDID (CEA, 148.5 MHz)
$P/pt-guest.py etw-boot arm && $P/pt-guest.py ssh Restart-Computer -Force
sleep 60 && $P/pt-guest.py wait 900 && sleep 90 && $P/pt-guest.py etw-boot collect A1-boot   # boot to desktop + 90 s
$P/pt-guest.py etw start && sleep 60 && $P/pt-guest.py etw stop A2-idle
$P/pt-guest.py etw start && $P/pt-guest.py dwm-restart && sleep 30 && $P/pt-guest.py etw stop A3-dwm
$P/pt-guest.py etw start && $P/pt-guest.py task present-probe.ps1 -Frames 600 --user --wait 300 && $P/pt-guest.py etw stop A4-present
$P/pt-guest.py bench               # smoke, d3d12-probe, cuda-probe, present-probe -> bench-*.txt
```
- M1/M2 at full speed (R4.4) in pass A too: in a second window
  `sudo python3 $P/pt-probe.py capture --seconds 60 -o /mnt/vmstore/caraxes/r4/runs/A1/probe-A.tsv`
  while the first runs `$P/pt-guest.py dwm-restart` and the present-probe task.
- Benchmarks (native baseline for the +-10 % target): D3D11 gate and D3D12 fence round trips
  (`smoke`, `d3d12-probe`), CUDA launch cost and copy bandwidth (`cuda-probe`), present rate
  and frame statistics windowed and exclusive fullscreen at 60 Hz (`present-probe`). Vulkan:
  `vulkaninfo --summary` in `smoke` only (no Vulkan renderer in the pack). SotTR: only if it is
  installed in the image (it is not today: no Steam); then its built-in benchmark at 1920x1080
  from the console, with an ETW capture around it.
- Host-visible numbers: `hostwatch.csv` in the run folder, 1 s: `vfio-msi` interrupts of
  2d:00.0 per second (`/proc/interrupts`), QEMU CPU %, RSS, MemAvailable, trace size.
  `$P/pt-vm.sh status` prints the last sample.
- End: `$P/pt-vm.sh stop` (guest `Stop-Computer`, then ACPI, then QMP quit; host kernel lines
  of the run in `host-kernel.log`).
- Control (optional, high value): the same disk under Cloud Hypervisor with the card,
  `$P/pt-vm.sh start win-ch A A5-ch`, then A2-A3. The failing runs' hypervisor with the real
  card: a TDR here points at the hypervisor, not at our device. `pt-probe.py` works on its
  vfio fds too.

### R4.4 Pass B: the register dialogue

- `.0` gets `x-no-mmap=on` (every BAR access traps to QEMU and is traced with its value),
  `x-no-kvm-msi=on` (each MSI passes QEMU: `vfio_msi_interrupt` with a timestamp),
  `x-no-kvm-ioeventfd=on` (otherwise the quirk turns repeated MSI re-arm writes to 0x88704,
  `NV_XVE_CYA_2`, into a KVM ioeventfd and they leave the trace). Events: `trace-B.events`.
  The card and trace arguments (the whole line: `$P/pt-vm.sh cmdline win B`):
```sh
-device pcie-root-port,id=rp1,bus=pcie.0,chassis=1,slot=1
-device vfio-pci,host=0000:2d:00.0,bus=rp1,addr=00.0,multifunction=on,x-no-mmap=on,x-no-kvm-msi=on,x-no-kvm-ioeventfd=on
-device vfio-pci,host=0000:2d:00.1,bus=rp1,addr=00.1 -device vfio-pci,host=0000:2d:00.2,bus=rp1,addr=00.2 -device vfio-pci,host=0000:2d:00.3,bus=rp1,addr=00.3
-msg timestamp=on -D /mnt/vmstore/caraxes/r4/runs/B1/qemu-trace.log -trace events=$P/trace-B.events
```
- Size: the trace can only be filtered by time at the source. B1 traces from boot (R1/R3: GOP,
  driver init, first flips); B2 boots untraced (`PT_TRACE_FROM=qmp`) and switches the
  per-access events on for short windows only. Offsets are filtered afterwards by the
  converter (presets: `display` 0x610000-0x6fffff, `ptimer` 0x9000-0x9fff, `usermode`
  0x810000-0x81ffff, `intr` 0xb80000-0xbbffff, `cfgmirror` 0x88000-0x88fff, `flip` = all
  five; `--range 9000-6dffff` is C73's `CARAXES_TRACE_ACCESSES=bar0:0x9000-0x6e0000`). The
  watcher switches the per-access events off below 10 GiB free. Plan 1-5 GB for B1; compress
  after the run (`zstd -T2 --rm`, the converter reads `.zst`).
- Expected slowdown: every BAR access is a VM exit plus a `pread`/`pwrite` on the vfio fd plus
  one log line, ~10-30 us (native: ~1 us per read, writes posted; our device: 23-27 us per
  trap, c0/c1). MSIs gain ~10-50 us. BAR1 and BAR3 trap too: CPU access to VRAM through BAR1
  drops to ~1 MB/s (cursor shapes, CPU uploads); GPU work is unaffected. Boot to desktop: a
  few minutes. The KMD's PTIMER reads trap as on our device (~85 us each in the audit), so
  its flip DDIs get long and its UPDATEs land late, as in C68-C73: pass B puts the real card
  into the timing our device fails in. A flip stream without TDR here means the latch and
  notifier semantics, not the trap latency, cause the TDR.
```sh
$P/pt-vm.sh start win B B1
$P/pt-guest.py wait 1800
$P/pt-vm.sh mark desktop && sleep 120 && $P/pt-vm.sh trace off
$P/pt-vm.sh stop
PT_TRACE_FROM=qmp $P/pt-vm.sh start win B B2
$P/pt-guest.py wait 1800
$P/pt-guest.py etw start
$P/pt-vm.sh trace on && $P/pt-guest.py dwm-restart && sleep 20 && $P/pt-vm.sh trace off
$P/pt-guest.py etw stop B2-dwm
$P/pt-vm.sh stop
```
- Converter (unit-tested on a synthetic QEMU 11.1 log: format byte for byte, mirror merge,
  filters, markers, time base, MSI rate):
```sh
RUN=/mnt/vmstore/caraxes/r4/runs/B2
python3 $P/qemutrace2gpu.py $RUN/qemu-trace.log --preset flip --markers $RUN/markers.txt --names /mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/C73-lf12/gpu-trace.log -o $RUN/gpu-trace-flip.log
python3 $P/qemutrace2gpu.py $RUN/qemu-trace.log --range 9000-6dffff -o $RUN/gpu-trace-c73range.log
python3 $P/qemutrace2gpu.py /mnt/vmstore/caraxes/r4/runs/B1/qemu-trace.log --preset prom --preset pmc --no-events -o /mnt/vmstore/caraxes/r4/runs/B1/gpu-trace-prom.log
python3 -m unittest discover -s $P -p 'test_*.py'     # 20 tests, synthetic data only
```

**M1 and M2 (display audit, 2026-09-29).** The Windows KMD gets its scan line by RPC
(`SYSTEM_GET_SCANLINE`, answered inside GSP), so no BAR trace shows it. `pt-probe.py capture`
reads, in a loop of a few us per register, `RG_DPCA(h)` 0x616330+0x800h (LINE_CNT 15:0, FRM_CNT
31:16), `EVT_STAT_HEAD_TIMING(h)` 0x611800+4h (LAST_DATA bit 1, VBLANK bit 2) and window w's
`PUT`/`GET` 0x690000+0x1000w / +4 (`--set full` adds window-immediate, core PUT/GET and
`RM_INTR_STAT_HEAD_TIMING`), and logs every change with the line and frame of that moment. Read
only; the reads bypass QEMU (not trapped, not in the pass B log). Run it in A1 and B2 while
flips happen (`dwm-restart`, `present-probe`), 60 s each:
```sh
sudo python3 $P/pt-probe.py capture --seconds 60 -o /mnt/vmstore/caraxes/r4/runs/B2/probe-B.tsv
python3 $P/pt-probe.py readout /mnt/vmstore/caraxes/r4/runs/B2/probe-B.tsv --flips 80
```
- M1a, the latch deadline: per window PUT change, the line of the write and the blank at which
  GET reached PUT; readout prints "latch deadline F: ... -> F in (a, b]" (lines before the
  latch line), the latch line itself (expected BLANK_START+1) and all flips. Pass B's late
  UPDATEs give the late lines; the audit estimates ~85 lines (640 KiB mempool) at 1080p60.
- M1b, LAST_DATA: the line at which `EVT_STAT` LAST_DATA sets (and VBLANK), min/median/max over
  all frames, and how long the ISR takes to clear it. Expectation from the audit: LAST_DATA at
  the fetch end, ~F lines before the latch, not at the blank.
- M2a, GET while an UPDATE is pending: readout lists the GET values seen between the PUT write
  and GET == PUT. The audit expects GET to stop before the UPDATE method (`STG1_STATE_WAIT_FOR_UPD`)
  until the latch; our device sets GET := PUT at the kick. Cross-check with the KMD's own reads
  in the B2 trace: `grep -E ' [RW] bar0 0x0069000[04]' $RUN/gpu-trace-flip.log`.
- M2b, the notifier: BEGUN/FINISHED are DMA writes to guest memory, invisible to BAR traces and
  the probe. The flip-queue log's completion timestamp (the PresentTimestamp the KMD reports
  from the notifier; `FlipQLog complTS` in the audit's decoding) against the VSync interrupt
  (`VSyncISR`) and the probe's latch times (host clock; guest offset from `clock.txt`) says
  whether the card stamps the latch (BEGUN at the promotion) and which flip it retires when.
  Decode the CSVs as the audit did (`/mnt/vmstore/caraxes/ref/etw/flipseq_ms.txt`).
- Probe and trace on one clock: the probe's t=0 is its header's `start`;
  `qemutrace2gpu.py --t0 <that start>` puts the pass B trace on the same origin.

### R4.5 Pass C (optional): the Linux 615 guest

- Guest: overlays on desktop-b's `rootfs.qcow2` and `games.qcow2` (Ubuntu 24.04, 615 open
  driver, GNOME on Wayland as in `kick-smoke.sh`), the Leandro kernel and initrd, `seed.img`,
  tap caraxes6 with desktop-b's MAC; `modprobe.blacklist=caraxes_dma_range` (the device's DMA
  aid is not wanted on real hardware). NVKMS reads window `GET_LINE` 0x690208 itself, so its
  scan line is in the trace.
```sh
$P/pt-vm.sh prepare linux
$P/pt-vm.sh start linux A C1
ssh -i /mnt/vmstore/leandro/vm/id_leandro -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null leandro@192.168.106.26 'nvidia-smi -L; sudo dmesg | grep -E "NVRM|nvidia-drm|Xid" | tail -n 20'
ssh -i /mnt/vmstore/leandro/vm/id_leandro -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null leandro@192.168.106.26 'export DISPLAY=:0 XAUTHORITY=$(ls -t /run/user/1000/.mutter-Xwaylandauth.* | head -1); timeout 60 vkcube > /dev/null 2>&1; echo vkcube $?'
$P/pt-vm.sh stop
PT_TRACE_FROM=qmp $P/pt-vm.sh start linux B C2
$P/pt-vm.sh trace on && ssh -i /mnt/vmstore/leandro/vm/id_leandro -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null leandro@192.168.106.26 'export DISPLAY=:0 XAUTHORITY=$(ls -t /run/user/1000/.mutter-Xwaylandauth.* | head -1); timeout 10 vkcube > /dev/null 2>&1'; $P/pt-vm.sh trace off
$P/pt-vm.sh stop
```
- The probe (`pt-probe.py capture`) during the vkcube run gives M1/M2 for NVKMS.

### R4.6 What to compare

| Item | Real card (R4) | Caraxes (C7x runs) |
|---|---|---|
| Window/core GET and PUT around a flip | probe (M2a); B2 trace 0x690000/4, 0x680000/4 | `gpu-trace.log`, the device's display events |
| ASSY/ARMED readbacks around a flip | B2 trace, core ARMED 0x688000-0x68ffff (`--range 688000-68ffff`) | same range (C73: 0x68a280-0x68a360) |
| Scan line and frame counter | probe `RG_DPCA` (Windows asks GSP; NVKMS reads `GET_LINE` 0x690208) | device clock, `SYSTEM_GET_SCANLINE` answers |
| Latch line, deadline F, LAST_DATA line | probe readout (M1) | engine: latch at the tick, LAST_DATA with it |
| PTIMER | B trace 0x9400/0x9410: reads per second, TIME_1/TIME_0/TIME_1 pattern, value vs host time | same offsets |
| Interrupts | MSIs per second (`hostwatch.csv`; B: `E msi` lines and the converter's MSI rate); the ISR's reads after each MSI: `grep -A12 ' E msi ' $RUN/gpu-trace-flip.log` (0xb81000 tree, 0x611EC0 dispatch, 0x611C00 and 0x611800) | device MSI raises and the tree stores |
| Notifier timing | ETW flip-queue log completion vs `VSyncISR`, the probe's latch times (M2b) | ETW of C68-C77, the device's notifier writes |
| Flip DDI length, present IDs, TDR | ETW A1-A4, B2 | ETW C61-C77 |
| Desktop and benchmarks | `bench-*.txt`, `hostwatch.csv` | Caraxes benchmark runs |

### R4.7 Risks

- Resets: `.0` has FLR; `.1`-`.3` have none, so QEMU hot-resets the slot (all four functions
  are in the VM) and vfio-pci does a bus reset when the last one closes. Turing's GFW boot
  re-runs devinit after a reset, and OVMF runs the GOP again. The USB-C pair (`.2` xHCI, `.3`
  UCSI with `ucsi_ccg`) is the known weak spot: a failed `.3` rebind costs only USB-C PD
  state on the host, never the desktop.
- A host reboot is needed when: a driver step hangs in the kernel (STUCK); the card falls off
  the bus (config reads 0xffffffff, AER fatal); `nvidia` fails `RmInitAdapter` after
  `reset bus` (GSP/WPR2 state a guest left behind); host Xid 79/119/120 after restore.
- Pass B may be too slow for a usable desktop (all BARs trapped). Then keep B to short traced
  windows; a per-BAR no-mmap switch would need a local QEMU change (the operator's; QEMU
  takes no AI-written code).
- The probe reads display registers of a card a guest drives: read-only, status and channel
  pointers only, the same registers the KMD reads; it keeps one host core busy while it runs.
- Windows sees a new display adapter instance: PnP installs 617.14 from the driver store; the
  overlay keeps all of it out of the Caraxes image.
- Time budget:

| Block | Time |
|---|---|
| Preconditions, logout, `check`, `to-vfio` | 30 min |
| Pass A: first boot, mode, A1 (reboot), A2-A4, probe, bench | 75 min |
| A5 Cloud Hypervisor control (optional) | 20 min |
| Pass B: B1 from boot, B2 windows, probe, conversion | 60 min |
| Pass C (optional) | 45 min |
| Stop, `restore`, desktop back | 20 min |
| Total | 3.5-4.5 h; the reboot fallback adds ~15 min |

### R4.8 Open for the operator

- sshd and the second machine (blocker, see R4.1).
- DP-3 unplugged for R4, or two heads in the reference?
- Is the probe (read-only BAR0 reads through QEMU's fd, as root) acceptable?
- Pass C and A5 (Cloud Hypervisor control) this weekend or not; SotTR needs Steam in the image first.

## Guest preparation (once)

1. Copy `vendor/open-gpu-kernel-modules` into the Ubuntu 610.57.04 guest, build
   `kernel-open` against `linux-headers-6.8.0-136-generic`, install `nvidia.ko` (do not
   install `nvidia-modeset`/`nvidia-drm` for E0–E3).
2. Install `/lib/firmware/nvidia/610.57.04/gsp_tu10x.bin` from the host.
3. Blacklist `virtio_nvrm` in that guest instance (it is not present when the device
   is not offered, but keep the images separate to avoid confusion).
4. `NVreg_ResmanDebugLevel=0` in the guest is useful here: it makes the driver print
   the INFO-level path (`RmInitAdapter failed! (0x%x:0x%x:%d)` carries the failing
   line).

### Display phases A and B (2026-09-23/24)

Plan: [display-connector.md](../caraxes/display-connector.md) phases A and B with D15 (DisplayPort SST),
checklist [display-adapter.md](../caraxes/display-adapter.md) §5. Result and tables:
[display-ab.md](../caraxes/display-ab.md); evidence and commands:
`caraxes/runs/display-ab-2026-09-23/`. Tree: branch `worktree-agent-a6f9005032084a629` from
`66682ae`; vendor `open-gpu-kernel-modules` 610.57.04 (`e4a5faa2567f`). VM: vm0, device
`--boot-model --fb-mib 4096 --host-rm` plus `--display` (`CARAXES_DISPLAY=1`).

- **Expected (A):** `RmInitAdapter succeeded!` with KernelDisplay present, the init controls
  A2-A12 answered and labelled, nvml 11/11, UVM registration unchanged.
  **Observed:** as expected. Fuse 0x21C04 read 0 (three times), 0x625F04 read 0 (STATUS invalid,
  layout unchanged: `vgaWorkspace 0xfff00000`), `GET_IP_VERSION` answered 0x04000000, static
  info, brightc, static EDID data, `WRITE_INST_MEM` (FB 0xF75D0000, 64 KiB, as fb-model.md
  §3.2 predicted), `MAP_SHARED_DATA`, the second `NV04_DISPLAY_COMMON`, and nvidia.ko's
  `GET_SUPPORTED`/`OR_GET_INFO`/`GET_I2C_PORTID` (ddcPortId 0). Zero unanswered.
  `SET_IMP_INIT_INFO` never came. The interrupt table carries `MC_ENGINE_IDX_DISP` at
  vector 156.
- **Expected (B, gate D3):** `card0-DP-1` connected, EDID byte-identical, edid-decode clean,
  1080p60, 720p60 and 1080p50 listed, the over-budget mode pruned, no modeset.
  **Observed:** as expected on run b3 (display-ab.md "Outcome"). 85 AUX transactions, HBR3 x4
  trained through `DP_CTRL` and powered down, the EDID ping-pong, 14 `IS_MODE_POSSIBLE`, no
  core `UPDATE`, zero unanswered.
- **Not as expected:**
  - The guest image starts **gdm3**. Its Xorg modeset the new card0 in run b1 (flip timeouts,
    `Lost display notification`, `DP_CTRL` refused with `NotPowered` because the library had
    put the sink in D3). Changed in vm0's overlay: `systemctl disable gdm3` and default target
    `multi-user.target` (not a driver change; revert with `systemctl enable gdm3` and
    `systemctl set-default graphical.target`). The device now wakes the sink to D0 before a
    `DP_CTRL` training.
  - Closing card0 as DRM master commits NVKMS's initial window mapping: one core `UPDATE`
    without raster methods, logged as `Lost display notification`. That wait is phase C's
    notifier.
  - `tools/caraxes-vm.sh down` without a guest `sync` lost the first four-module build and
    its install (all-zero `.ko` files after the restart). The build was redone.
- **D0 baseline** (`gate-nvml-*.json`, `uvm-display-*`): nvml pass 11/11 and UVM
  `rmStatus 0x0` before A (display off), after A, and after B.
- **Guest:** the full four-module build of `~/ogkm/open-gpu-kernel-modules`
  (`make modules -j8 SYSSRC=...`) builds `nvidia-drm.ko` against the 6.8.0-136 headers,
  resolving D0's build failure. It was installed with `modules_install`. `nvidia-modeset` and
  `nvidia-drm` are blacklisted from autoloading in `/etc/modprobe.d/caraxes-display.conf`
  (`options nvidia-drm modeset=1 fbdev=0`). `modprobe video` still comes first.

### Display phases C and D (2026-09-24)

Plan: [display-connector.md](../caraxes/display-connector.md) §4 phases C and D on top of A/B. Result
and tables: [display-cd.md](../caraxes/display-cd.md); evidence and commands:
`caraxes/runs/display-cd-2026-09-24/`, gates `caraxes/runs/gate-{vdisplay,display}-2026-09-24*`.
Tree: branch `worktree-agent-adbfba499c7110889` from `5de9b40`. VM: vm0, device
`--boot-model --fb-mib 4096 --host-rm --display` (no host execution).

- **Expected (C):** the first modeset completes (core notifier FINISHED, no `Lost display
  notification`); flips complete at the tick, 60.0 +- 0.5 Hz; the guest RM services vblank; a
  Wayland compositor's frame callbacks run at the mode's rate.
  **Observed:** as expected. 120 flips at 60.00 Hz (rms 0.18 ms); NVKMS `NOTIFY_VBLANK` 120/120
  at 60.01 Hz; Weston 13 (DRM backend, atomic, pixman) frame callbacks 59.86 Hz at 1080p60 and
  49.87 Hz at 1080p50, refresh reported exactly; mode switch 1080p60 -> 720p60 -> 1080p50 at
  50.00 Hz.
- **Expected (D):** GNOME Wayland with colord, 0 `Page flip failed`; an EDID change as a hotplug;
  the adapted vdisplay and display gates pass the stages that need no GPU execution.
  **Observed:** GNOME 46 on Wayland (Mutter atomic KMS, gbm renderer through Mesa `kms_swrast`),
  colord running, OLUT armed, 0 page-flip failures; EDID-change hotplug through `POST_EVENT`
  (800x600 appears, a drm change uevent), unplug and replug; vdisplay pass, display pass (GPU
  stages listed as not run).
- **Not as expected:**
  - The first AWAKEN rule (an event for every awaken notifier) gave the `nvidia-drm-crtc.h:355`
    WARNING at each Weston start and a `Flip event timeout` at each stop; the device now follows
    nvidia-drm's rule (an event only for a window that scanned out before).
  - `FE_CORE_HEAD_STATE` was decoded for heads 0-2 only and NVKMS drives head 3; fixed.
  - gdm chose X11 twice: the user's AccountsService session was `ubuntu-xorg` (accounts-daemon
    caches it), then gnome-shell on Wayland segfaulted in libgallium under
    `LIBGL_ALWAYS_SOFTWARE=1`; `MESA_LOADER_DRIVER_OVERRIDE=kms_swrast` works.
  - BAR1 CPU access to dumb buffers runs at 0.38 MB/s write, 0.14-0.37 MB/s read (trapped); a
  1080p frame costs 21 s to write. The display gate's first run failed on v1's 45 s modeset
    budget and on the 1080p first-repaint wait (gate budgets, no device change).
  - GNOME frame-callback rates could not be measured (llvmpipe into BAR1); the hardware cursor
    was exercised through the legacy cursor ioctls instead (d14), GNOME had no pointer device.
  - On the final (rebased) build the display gate's compositor stage measured 59.50 Hz once
    (host load average 5.5, fail by 0.01 Hz) and 60.00 Hz on the rerun; vdisplay passed.
  - One vm0 boot fell into emergency mode (`/dev/disk/by-label/BOOT` not found in time under host
    load); the next boot was clean.
  - The host's root filesystem reached 100 % for about a minute at 01:35 (another project's
    process); only this worktree's `target/debug` was deleted.
- **vm0 changes:** offline Ubuntu packages (weston 13, sway 1.9, libdrm-tests, edid-decode,
  wayland-utils, grim, wlr-randr, libwayland-dev, libwayland-bin, wayland-protocols and
  dependencies, from archive.ubuntu.com with plain curl); gdm3 enabled again (default target left
  at `multi-user.target`, started by hand after the display modules), `WaylandEnable=true`,
  `/etc/udev/rules.d/61-gdm.rules -> /dev/null`, AccountsService session `ubuntu-wayland`,
  `/etc/environment` with `__EGL_VENDOR_LIBRARY_FILENAMES=.../50_mesa.json` and
  `MESA_LOADER_DRIVER_OVERRIDE=kms_swrast`, gdm `[debug] Enable=true`; originals as
  `*.caraxes-orig`, undo line printed by `guest/gdm-wayland-setup.sh`. `/dev/nvidia-modeset`,
  `/dev/nvidiactl`, `/dev/nvidia0` made by `guest/display-up.sh`.
- **Host GPU reads:** none beyond the device's nvml forwarding client.

### Display phase E (2026-09-24)

The guest's compositors and clients render on the physical GPU through the shadow GR path; the
write-up is [display-e.md](../caraxes/display-e.md). vm0, slot 1, 2 GiB profile, host execution, `--display`
(configuration: `caraxes/runs/display-f-2026-09-24/env.sh`). Series E (e1-e31) by the first
display agent, evidence `caraxes/runs/display-e-2026-09-24/`; series F (f0-f7) on main `8cca8e6`,
evidence `caraxes/runs/display-f-2026-09-24/`. Every host Xid is on the device's pid.

| Attempt (time) | Hypothesis | Change | Result | Exception | Evidence |
|---|---|---|---|---|---|
| e1-e3 (08:39) | the NVIDIA EGL answers on the device | none | `eglInitialize failed`: phase D's Mesa pin, no `nvidia-drm_gbm.so` | none | `e1-e3` |
| e4-e7 (08:40-08:53) | Weston's GL renderer comes up once the GL driver's controls are answered | `nvidia-egl-setup.sh`; `FB_GET_GPU_CACHE_INFO`, `FIFO_GET_ENGINE_CONTEXT_PROPERTIES` forwarded, channel `BIND` recorded | `EGL_BAD_ACCESS` (e5), then the GL renderer on `Leandro VFIO RTX 2070-2G` | none | `e4-e7` |
| e8-e12 (08:53-09:05) | a GPU clear lands | `eglprobe.c`; `ACTIVE_DISPLAY_DEVICES` reports `numHeads`; `ZCULL_BIND`, BINAPI 0x0107 | Weston GL 60.00 Hz, but the readback is wrong (0-60k of 2.07M pixels) | none | `e8-e12` |
| e13 (09:09) | the GL driver's bare CE channels carry the clear | `CARAXES_MIRROR_BARE_CE=1` | **`GPU-RENDERED-OK`**, the first GPU-rendered frame | none | `e13-eglprobe-bare-ce/` |
| e14-e17 (09:10-09:25) | Weston GL at 1080p | size-0 global context buffers excluded from set A | e14 2 x Xid 31 `PRIV_VIOLATION` at 0x1_20800000; after the fix 56.45 Hz, eglprobe 1080p ok, full-screen damage 52.35 Hz | Xid 31 (e14) | `e14-e17` |
| e18-e19 (09:26-09:32) | GNOME on the NVIDIA EGL | depth/stencil kinds carried as a kind override | e18 28 lines `Xid 13 3D-Z KIND Violation`; e19 clean | Xid 13 (e18) | `e18-e19` |
| e20-e25 (09:38-09:51) | v1's Vulkan stages and KMS capture | `vkprobe-wl.c`, `nvidia-uvm` in `display-up.sh`, block-linear `kmsgrab` | swapchain, present 43.6-44.4 FPS, RT device after `/dev/nvidia-uvm`, fence 0.1-1 ms; capture `0x00808080` | none | `e20-e25`, gates 5-7 |
| e26-e29 (09:54-10:11) | the classic demos | `demos.sh` | Weston vkcube-wayland 33.9, glxgears 54-56, glmark2 1216; GNOME 22.8, 14-21, 31 | none | `e26-e29` |
| e30-e31 (10:23-10:32) | the mapping worker helps the display | merge of `b58cb12` | Weston 59.5 Hz, present 52.4-53.2 FPS, Mutter 47.1 Hz (38.6); the display gate **passes 11/11** (`gate-display-2026-09-24-7`) | none | `e30-e31` |
| f1 (13:15) | the demos on the current main beat e27 | none (main `8cca8e6`) | Weston: vkcube-wayland 44.1, vkcube 37.6, glxgears 57-60, simple-egl 59-60, glmark2 **1804** | none | `f1-demos-weston/` |
| f2 (13:20) | the same in GNOME | none | vkcube-wayland 31.1, vkcube 29.9, glxgears 33-37, simple-egl 22-33, glmark2 34 | 1 Xid 31 `FAULT_PTE` PROP write at 0x1_202c0000 during GNOME's start (display-e.md §9) | `f2-demos-gnome/` |
| f3, f3b, f4 (13:26-13:30) | what still traps per presented frame | full trace, wlframes 14 s, 10 s window (`caraxes/tools/trap-table.py`) | Weston 157 traps per frame at 58.4 Hz, Mutter 662-712 at 30.0-41.5 Hz; 2-3 doorbells, 2-3 GPPut, 0 GPFIFO BAR1 writes, 0 shadow plans; the rest is interrupt servicing and 71 BAR2 PTE writes (Mutter) | none | `f3-traps-mutter/`, `f3b-*`, `f4-*` |
| f5, f5b (13:34) | the doorbell ioeventfd lifts Mutter | patched Cloud Hypervisor `ch-0005-v.sh`, `CARAXES_DOORBELL_IOEVENTFD=1` | doorbell traps 3 -> 0 per frame, callbacks 35.2 / 37.6 Hz (without: 30.0 / 41.5): no measurable effect | none | `f5-*`, `f5b-*` |
| f6 (13:40) | a pointer without patching Cloud Hypervisor | `CARAXES_INPUT=fifo`: vhost-user-input over `--generic-vhost-user device_type=18` | the guest sees `caraxes-vm0-pointer` (mouse and keyboard) on seat0; 180 of 180 host events arrived; the GPU moves to 00:06.0 | none | `f6-pointer/` |
| f7 (13:49) | the stream stage delivers a desktop | Sunshine kms + libx264 as root, Moonlight on the host, bench.sh's summary | valid: 25.85 FPS, host latency avg 101 ms, 0.00 % / 0.35 % drops, 9.8 Mbit/s, 2.09 guest cores; the two runs before failed at pairing (Sunshine's login written to root's default state, fixed) | none | `f7-stream/` |

### P20: parallelism proof (2026-09-24)

Two reserving VMs, vm1 on slot 0 and vm2 on slot 1, each with a 2 GiB profile, run in the green
configuration at the same time on the card that drives the desktop. The write-up is
[parallel.md](../caraxes/parallel.md). Evidence is under `caraxes/runs/parallel-2026-09-24/`. The host logged
0 Xid over the whole window, 06:41 to 08:25. Every host GPU access went through the RM API as the
devices' clients. Privileged controls stayed in the launcher's CAP_SYS_ADMIN windows. No host
module or register was touched.

| Attempt (time) | Hypothesis | Change | Result | Exception | Evidence |
|---|---|---|---|---|---|
| e1 (06:41-06:46) | two 2 GiB slots fit beside the desktop and leave it 1 GiB | vm1 slot 0, vm2 slot 1 | `[0x169000000, 0x1e9000000)` and `[0xe9000000, 0x169000000)`; host used 1148 -> 3195 -> 5243 MiB; nvml 11/11 on both | none | `e1-two-vms/` |
| e2 (06:46-07:20) | nvprobe and the cuda gate run concurrently | both at once | probes pass (28.3/27.2 ms, then about 22 ms, against 18 alone); the scored gate passes except one hostreg roundtrip on vm2; both devices deadlock in `suite-test_async_streams` | no Xid; 732088 refused host BAR1 mappings (06:53:47-06:57:12); guest RCU stalls | `e2-concurrent/` (`hang-*`, `dev-*`, `check.txt`) |
| diagnosis (07:15-07:25) | the server thread waits on the HostMemory lock it already holds | read the thread states and the code paths | the shadow sync holds the lock, and the FB store's window map takes it again; the leak is the missing `RM_UNMAP_MEMORY` in `Window::drop` | none; recovered with kill -9 (fd close), host back to 1152 MiB | `e2-concurrent/` |
| e2b (07:28) | a lock-free mapper and a 2 x 32 MiB budget are enough | first fix build | the device's own 2 MiB scan windows leaked BAR1 (no unmap); the store stalled without a window (writes no longer dropped) | about 920k refused BAR1 mapping lines; vm1 and vm2 stopped | `e2b-aborted-bar1-leak/` |
| e2c (07:42) | `RM_UNMAP_MEMORY` on window drop releases BAR1 | unmap in `Window::drop` | 256 windows mapped and released, 0 refused, 0 asserts | none | `e2c-bar1-unmap/` |
| e2d (07:44-08:07) | the concurrent gate passes on the fixed device | rerun | probes pass (24.7/25.0 ms, then 16.5-18.3 ms, against 18.2-18.5 alone); the scored gate is the single-VM green set on both; then `suite-test_async_streams` (545k mirrored leaves, about 556k RM maps per device) stalls the host RM: `nvidia-smi` timed out at 20 s, the desktop's electron GPU process restarted at 08:04:34, and host IOMMU IO_PAGE_FAULTs at 0x7fff00000000 + 0..0x5000 (07:58:30-08:00:41) | no Xid; host impact, see parallel.md §2.4 and §8 | `e2d-concurrent-fixed/` |
| e3 (08:10-08:18) | VM B never sees VM A's VRAM | a 1 GiB pattern held in A, B scans everything it can allocate, both directions, positive control | 0 foreign records, 1728 MiB and 640 MiB scanned; the control finds 16.7M stale own records; checker PASS on e2 and e3 | none | `e3-isolation/` |
| e5 (08:16) | a third 2 GiB VM is refused by D11 | vm3 slot 2 | refused (would leave 569 MiB, shortfall 455 MiB) before its socket existed; vm1 and vm2 unaffected | none | `e5-refusal/` |
| e4b (08:18-08:21) | host scrub on free covers the reservation | A down while holding the pattern; C reserves slot 0 and scans | before the free, 327680 records in the samples; after the new reservation, 0 records and 0 non-zero dwords (device); 1728 MiB of zeros (guest); the next allocation waits 23-38 ms for the 2 GiB scrub | none | `e4-scrub/` |
| e4a (08:20) | a failed free makes the teardown fail closed | `CARAXES_INJECT_TEARDOWN_FAILURE=1` on vm2 | `FAIL-CLOSED teardown`: descriptors and reservation kept, exit 3; the fd close freed the 2048 MiB | none | `e4-failclosed/` |

### Mapping package (2026-09-24)

This package took the v2 cost the benchmark measured and the two P20 findings (benchmark.md §5
and §7, parallel.md §8). The write-up is [mapping.md](../caraxes/mapping.md). Evidence is under
`caraxes/runs/mapping-2026-09-24/` (`commands.txt`).

Setup:

- vm2 ran alone at slot 0 with a 2 GiB profile. vm0 (the display agent) held slot 1.
- Every host GPU access went through the RM API as the device's client.
- Privileged controls stayed in the launcher's CAP_SYS_ADMIN windows.
- Host `nvidia-smi` latency and the host kernel log were watched during every stress run
  (`bench-hostwatch.sh`, at most 0.12 s).
- 0 Xid came from this package's devices. The Xid 13 lines at 09:26-09:28 belong to vm0's
  device, pid 2061395, class c597.

| Attempt (time) | Hypothesis | Change | Result | Exception | Evidence |
|---|---|---|---|---|---|
| a0 (08:40) | baseline on this configuration | the `718a311` device | 1 MiB alloc/free 18.9/20.8 ms, churn device CPU 80 s, ctx 5.4/3.4 s, short launch 0.457 ms | none | `a0-baseline/` |
| a1 (08:50) | walking only what the guest wrote cuts the mapping cost | cached walk per root, diff per 2 MiB unit (coalescing off) | 1 MiB alloc/free 1.30/1.40 ms, churn device CPU 10.7 s, N=2048 matmul 781 -> 2440 GFLOP/s; each table read still about 0.8 ms of BAR1 | none | `a1-incremental/` |
| a2 (08:55) | a write-through copy of every watched table page removes the reads | patch the copy on every guest write | 1 MiB alloc/free 0.92/0.97 ms, 64 MiB 10.5/7.2 ms, short launch 0.193 ms | none | `a2-writethrough/` |
| a3 (08:58) | coalesced maps cut the RM calls | one map per contiguous run, partial unmaps | ctx destroy 3.6 -> 1.5 s; a context's scattered sysmem leaves (the guest's order-0 page pool) often do not coalesce | none | `a3-coalesce/` |
| p3 (09:00) | find where cuCtxCreate goes | device trace of one `bench-cu init` | 96193 trapped 8-byte BAR2 writes (107 zeroed table pages, about 19.8k PTEs written twice) and about 4M uncached BAR1 reads for the device's BAR2 walk | none | `p3-profile/` |
| a4 (09:04) | a BAR aperture TLB removes the device's walk | TLB flushed by a write to a table page it used | ctx 2.7/1.0 s, short launch 0.131 ms, 0 Xid | none | `a4-bartlb/` |
| b1 (09:06) | the P20 suite no longer hurts the host | `suite-test_async_streams` on a4 | pass (14.8 s); 544750 leaves in 18921 RM maps (556k leaves by 30k calls); teardown 18914 maps in 2.5 s; host nvidia-smi max 0.056 s | none | `b1-async-streams/` |
| c0/c1 (09:10) | where the per-submission cost goes | trace of 100 short launches; guest microbenchmark `bench-trap.c`; trace lines off | 4 traps per submission (GPFIFO lo/hi and GPPut through BAR1, then the doorbell), 23-27 us each; device share 1-2 us | none | `c0-doorbell-study/`, `c1-trace-off/` |
| c2 (09:20) | USERD in sysmem by `RMInstLoc=0x10000` removes the GPPut trap | guest registry key, sysmem USERD for shadow channels | the guest RM never finished RmInitAdapter: last RPC 0x20802a08, then a BOOT_0 poll loop and asserts at alloc_free.c:360; reverted; the device trace was lost to the next `up` | none on the host; vm2 restarted | `c2-userd-sysmem/` |
| d1 (09:38) | IOVA = GPA + one offset and CE on the shadow VAS close the P20 holes | linear registration with a hole filler and restart, CE channels on the shadow VAS | IOVA = GPA + 0x7ffac0000000 after 1 restart; cuda gate green (cuda-managed the known gap), all unscored suites pass; 0 sysmem refusals (128 in b1); 0 IOMMU faults; 400 channels shadowed, 0 externally-owned VAS | none | `d1-isolation/` |
| d2 (09:52) | resync off the server thread | shadow worker, doorbell barrier, streaming BAR1 reads | server-thread time per resync p50 42 us, p99 2.4 ms (was 1.7 ms and 506 ms); cuda gate green; plans waited up to 2 s behind teardowns | none | `d2-worker/` |
| d3 (10:05) | teardowns must not delay plans | teardown stepped in the worker's idle time | quick metrics pass (ctx 4.0/1.7 s on a host with vm0 and vm3 busy); checkpoint `b58cb12`, exercised by the final series | none | `d3-worker2/` |
## Blackwell N gate (2026-09-24, defined, not run)

A stock 610.57.04 guest on the synthetic GB206 (RTX 5060 Ti identity of
`caraxes/reference/gb206/`), pure emulation: no host RM, no host execution, no reservation,
no host register. Stages N0 to N4, the exact launcher command and the predicted walls are in
[blackwell-gate.md](../caraxes/blackwell-gate.md); the boot path the device emulates is
[gsp-boot-blackwell.md](../caraxes/gsp-boot-blackwell.md). Rows as in the E0 to E5 steps: one wall per
row, read from the guest dmesg and the device trace, fixed from the 610 source.

| Attempt (time) | Hypothesis | Change | Result | Exception | Evidence |
|---|---|---|---|---|---|
| b0 (2026-09-24, morning) | the device side is complete up to the linked GSP queues | GB206 identity (board `rtx5060ti`, chip `gb206`, MSI-X unlinked, BAR1 256 MiB), FSP boot model `src/chip/boot_fsp.rs` | unit tests: config identity and capability chain, client selftest against a running `--board-data rtx5060ti` server, the FSP sequence of gsp-boot-blackwell.md section 2 up to `queues linked` and GSP_INIT_DONE | not run in a VM: vm3 busy | `cargo test -p vfio-user-nvgpu` |
| b1 (12:06-12:12, vm1) | N0 to N2 pass as far as the boot model goes | `CARAXES_CHIP=gb206 CARAXES_BOOT_MODEL=1 CARAXES_FB_MIB=4096`, worktree build, stock Cloud Hypervisor; the guest had only `gsp_tu10x.bin`, so the host's `/lib/firmware/nvidia/610.57.04/gsp_ga10x.bin` (same driver version, sha256 c0156954...) was installed in vm1's rootfs | N0 and N1 pass (10de:2d04 rev a1, 1043:8a08, BAR0 64 MiB, BAR1 256 MiB, BAR3 32 MiB, I/O 128 B, ROM 512 KiB, capabilities PM, MSI, Express only). N2: the FSP sequence runs as gsp-boot-blackwell.md section 2 predicts (CAPS_QUERY, CLOCK_BOOST, COT, ACR layout, target mask and lockdown released, queues linked, init RPCs, GET_GSP_STATIC_INFO, phase-5 controls). Wall: `NV2080_CTRL_CMD_INTERNAL_GET_DEVICE_INFO_TABLE` 0x20800A40 refused 44 times (the GA100+ `gpuConstructDeviceInfoTable_FWCLIENT`, gpu_gspclient.c:224-280), so `kfifoConstructEngineList_KERNEL` builds no engine list and the guest oopses (NULL KernelGraphics) in `memmgrCalcReservedFbSpaceHal_GM107` (mem_mgr_gm107.c:1997-2002, `kgraphicsDoesUcodeSupportPrivAccessMap`). Also: `_kgspIsHeartbeatTimedOut` messages on every RPC (NV_PGSP_MAILBOX(0/1) heartbeats 0x110804/0x110808 read 0; logged, not fatal); unanswered 0x20800A4B (DISPLAY_GET_IP_VERSION), 0x20800AF3 (CONF_COMPUTE_GET_STATIC_INFO); unnamed BAR0 0x590, 0x8A1D58/5C | guest oops in modprobe; no host effect (pure emulation) | `caraxes/runs/blackwell-2026-09-24/n2a/` |
| b2 (12:14-12:19, vm1) | answering the device-info table from the chip data gets past the heap | `src/chip/physical.rs`: DEVICE_INFO2 for GR0, CE0/4/7 (fault ids, instances) plus TMR 0x9000, PBUS 0x1000, HUBMMU 0x880000 PRI bases; a small hook in boot.rs asks it before the dispatcher | the heap and the engine list come up (no oop). Wall: `kbusVerifyBar2_GB202`, "MMUTest BAR2 readback VA = 0x0 returned garbage 0x0" (NV_ERR_MEMORY_ERROR via kbusStateInitLockedKernel, kern_bus_gm107.c:362, 467), `RmInitAdapter failed! (0x24:0x72:1281)`. The device trace shows every BAR3 access "UNMAPPED (level 2)": the guest's BAR2 page tables are GMMU v3 (5 levels), fb.rs walks only v2 (`MMU_FORMATS_UNWALKED`). Next wall is the GMMU v3 walker, which is fb.rs work (outside this package) | driver load fails cleanly; no host effect | `caraxes/runs/blackwell-2026-09-24/n2b/` |
| b3 (12:25-, unit tests, no VM) | the b2 wall is the missing GMMU v3 walker | `src/gmmu_v3.rs` (5-level, 57-bit, GB10X levels, gh100/dev_mmu.h fields), selected in fb.rs by the chip's format (`with_mmu_format`, purely additive: the v2 code paths unchanged); secure timer and heartbeat mailboxes in `boot_fsp.rs`; device-info columns checked against the source | unit tests: v3 and v2 trees with the same mappings give the same `Target`s for 4 KiB, 64 KiB, 2 MiB, sysmem and sparse pages, the same leaf enumeration, the BAR TLB flushes on a v3 table write; the heartbeat check of `_kgspIsHeartbeatTimedOut` stays within 2 ms; next: a VM run when the coordinator frees vm1 | none | `cargo test -p vfio-user-nvgpu` |
| b4 (12:31-12:36, vm1) | with the v3 walker the BAR2 self-test passes and RmInitAdapter completes | b3 binary | `RmInitAdapter succeeded!`. BAR2 root confirmed: the device walks from the static info's `bar2PdeBase` (the ACR layout's `nonWprHeapOffset` 0xF0900000), the guest's v3 tables sit right below it (0xF08C5000-0xF08CF000) and every BAR3 access resolves (0 UNMAPPED, bar TLB 11103 hits / 97 walks). Tolerated on the way: CCU buffer size (kccuGetBufSize_GB100, 0x56), CE_GET_HUB_PCE_MASK_V2, CONF_COMPUTE_GET_STATIC_INFO; unanswered 0x20800A4B, 0x20800AB2, 0x20800AB4, 0x20800AF3, 0x20802A0E. **N3 passes**: `nvidia-smi -q` (the guest's `/opt/nvrm/bin/nvidia-smi`, device nodes made by hand as the `uvm` step does) shows Product Name "Leandro VFIO RTX 5060 Ti-4G", Product Architecture Blackwell, VBIOS 98.06.1F.40.4A, FB 4096 MiB, BAR1 256 MiB, a synthetic UUID and PDI (neither the capture's, checked), no Xid | none | `caraxes/runs/blackwell-2026-09-24/n2c/`, `n3/nvidia-smi-q.txt` |
| b5 (12:36, vm1) | UVM registration allocates the first channels (N4) | `tools/caraxes-vm.sh uvm n4` | UVM_REGISTER_GPU 0x56: `CE_GET_PHYSICAL_CAPS` 0x20802A07 is only forwarded to a host RM on Turing, so queryCopyEngines fails (kernel_ce.c:564) | none | `n4a/` |
| b6 (12:39, vm1) | the capture's CE caps satisfy queryCopyEngines | `physical.rs` answers 0x20802A07 from CE_GET_ALL_CAPS of the capture (host-facts.txt:33-36) | next: `CE_GET_CE_PCE_MASK` 0x20802A02, same cause | none | `n4b/` |
| b7 (12:43, vm1) | the capture's PCE masks satisfy it | `physical.rs` answers 0x20802A02 (fifo-host-query.txt:6-8) | UVM gets to channel creation: TSG allocated, then `kchangrpapiSetLegacyMode` fails `numMax == numFree && numMax != 0` (kernel_channel_group_api.c:914), because the GR info is not answered without a host (`pGrInfo` NULL, `kfifoGetMaxSubcontextFromGr_KERNEL` returns 0, kernel_fifo.c:2760-2775) | none | `n4c/` |
| b8 (12:47-12:51, vm1) | synthesized GR info gets the channel allocated | `physical.rs` answers INTERNAL_STATIC_KGR_GET_INFO 0x20800A2A for gb206: MAX_SUBCONTEXT_COUNT 64, 3 GPCs, 6 TPCs per GPC, 2 SMs per TPC, SM version 0x1200, rest 0 (correction 2026-09-25: the field is `major << 8 | minor`, so 0x1200 decodes as SM 18.0; Blackwell's SM 12.0 is 0x0C00, `NV2080_CTRL_GR_INFO_SM_VERSION_12_00` ctrl2080gr.h:335 in the 615 tree, and `chips/gb206/chip.toml` now carries 0x0C00, [costume-ladder.md](../caraxes/costume-ladder.md) section 1; this row records what the run sent) | **N4 reached**: TSG 0xA06C, `BLACKWELL_CHANNEL_GPFIFO_B` 0xCA6F on chid 1 (engine COPY4) -> status 0, then GP100_UVM_SW 0xC076 and `BLACKWELL_DMA_COPY_B` 0xCAB5 objects and GPFIFO schedule, all status 0. The gate stops here, as defined. Past it, as expected: UVM's first push (the channel_init semaphore release, uvm_channel.c:2546) never completes because the device does not execute Blackwell methods (and no GPFIFO doorbell was seen), so UVM_REGISTER_GPU hangs until the VM is brought down | none; vm1 down 12:51 | `n4d/` |

