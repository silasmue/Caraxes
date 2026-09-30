<!-- SPDX-License-Identifier: MIT -->
# Open questions

Numbered; keep numbers stable. Each names the experiment or source that resolves it.

1. **Reading the real card's BAR0 from the host.** `/dev/mem` gives EPERM and
   `/sys/.../resource0` EINVAL while `nvidia` owns the BAR (`CONFIG_STRICT_DEVMEM=y`,
   `CONFIG_IO_STRICT_DEVMEM=y`). Resolve with R2 (RM arch-info control) and R1/R3
   (passthrough capture), not with a kernel module.
2. **Full VBIOS image.** The sysfs ROM read returns only the two PCI-chained images
   (x86 + EFI, 127 KiB). The FWSEC extension image the driver needs is beyond the PCI
   "last image" flag and only reachable through the PROM window. R1.
3. **Real `NV_PMC_BOOT_42`, `HWCFG` IMEM size, and the other status-register values.**
   Predictions are in [registers-turing.md](../caraxes/registers-turing.md); R3 gives ground truth.
4. **Sizes of `GspStaticConfigInfo` and `GspSystemInfo`.** The CPU copies
   `sizeof(GspStaticConfigInfo)` from the reply regardless of the reply length and
   requires both to fit in one message (≤ 65488 bytes). Compile the vendored headers
   with the same flags as `crates/nvrm-sys/wrapper.h` to get the sizes and offsets before E3.
5. **Which FB size to advertise, and how FB accesses are backed later.** Phase 4.7
   reads `LOCAL_MEMORY_RANGE`; the guest will later build BAR2 page tables and instance
   blocks in "its" FB (see kernel-rm-hw-surface.md). Options: fake FB in host RAM (fine for
   milestone 1), a host VIDMEM allocation mapped through BAR1 (real memory but guest FB
   PAs ≠ real PAs), or an identity-mapped reserved range (guest FB PAs = real PAs). Decide
   after E3 shows what the guest writes.
6. **FRTS offset for the WPR2 registers.** The device must answer `WPR2_ADDR_LO` with
   `frtsOffset >> 12`. The value is inside the FWSEC command the CPU writes to GSP DMEM
   (`FWSECLIC_FRTS_CMD.frtsRegionDesc.frtsRegionOffset4K`) and can also be derived from
   the WPR meta (`frtsOffset = gspFwWprEnd − 1 MiB`) once SEC2 MAILBOX0/1 deliver its PA.
   Decoding DMEM is more honest; confirm the DMEM layout in E2.
7. **`SWAP_RX` convention of the real GSP status-queue header** and whether the GSP
   zero-pads elements to 8 bytes before checksumming (the CPU accepts either as long as
   the checksum over the padded length is 0). R3 shows the real header.
8. **The ordered list of phase-5 RPCs and direct register accesses** on a first init.
   E3 gives the RPC list from our own device; R3 gives the real register list.
9. **Internal controls.** The guest kernel RM issues `NV2080_CTRL_CMD_INTERNAL_*`
   controls that an unprivileged host RM client cannot forward. Which ones occur before
   `RmInitAdapter succeeded!`, and which can be synthesized from public host controls?
   E3/E4.
10. **Handle namespaces.** Guest CPU-RM chooses its own `hClient` values; the host RM
    chooses root handles for Leandro (`crates/nvrm-client/src/lib.rs:55-60`). Whether
    `NV01_ROOT_CLIENT` accepts a caller-chosen `hObjectNew` decides if a translation
    table is needed. Test with `nvrm-client` on the host (no GPU risk).
11. **FINN-serialized controls.** The guest serializes a small set of embedded-pointer
    controls (GET_CLASSLIST, GET_ENGINES, …). Does the host ioctl path accept
    `NVOS54_FLAGS_FINN_SERIALIZED` from userspace, or must the device deserialize?
    Leandro refuses both today (`request_shape.rs:164-176`). Test on the host.
12. **Interrupt delivery.** `osVerifySystemEnvironment` expects a test interrupt.
    Which register write triggers it on Turing and which vector the guest expects
    (from `INTR_GET_KERNEL_TABLE`) are in kernel-rm-hw-surface.md; MSI through vfio-user
    eventfds is the mechanism.
13. **Guest kernel choice.** Ubuntu 6.8 (610.57.04 image exists) versus NixOS 6.12
    (recipe exists). The open module compiles on both; pick Ubuntu for E0 because the
    userspace already matches.
14. **Resolved: Cloud Hypervisor is the client** (`--user-device socket=…`,
    `--memory shared=on`, no vIOMMU). Its client reads BAR sizes from region info and
    the ROM from region 6, supports INTx/MSI/MSI-X, and maps guest memory with fds.
    QEMU stays for the passthrough capture R3 only. See vfio-user.md.
15. **Whether `NVreg_GpuInitOnProbe=1` changes the failure reporting** (probe fails
    instead of open failing). Check in E0; both are fine for logging.
16. **Resolved.** `rm_wait_for_bar_firewall` is a no-op for device ids outside the
    Blackwell ranges 0x2900–0x29FF / 0x3180–0x327F (`gpu_mgr.c:4976-5002`).
17. **FB model for state init.** `kbusVerifyBar2_GM107` writes through PRAMIN
    (`NV_PBUS_BAR0_WINDOW` 0x1700 + 0x700000 window) and through BAR2 (PCI BAR3) and
    requires both to alias the same FB bytes, with BAR2 translated by GMMU page tables
    rooted at GSP's `bar2PdeBase` merged with the kernel's PDE3[0]. So milestone 1 needs
    a fake FB in host RAM plus a GMMU v2 page-table walker for BAR2 (and BAR1 for USERD).
    See kernel-rm-hw-surface.md (KernelBus). Which page-table format
    (`kgmmuFmtGet(VERSION_DEFAULT)`) and how the device learns PDE3[0] (`UPDATE_BAR_PDE`
    RPC) are still to be written down.
18. **Config-space mirror.** The RM reads PCI config through BAR0 at 0x88000 (`NV_PCFG`),
    with MSI/MSI-X enable bits at the fixed offsets 0x68/0xC8 (`gm107/dev_nv_xve.h`,
    `tu102/dev_nv_xve.h`). The emulated config space must keep its MSI capability at
    0x68 and mirror live enable bits into BAR0. Whether other capability offsets matter
    is open (kernel-rm-hw-surface.md, open question 2).
19. **Self-tests that need real GPU work.** `memmgrTestCeUtils` (CE memset/copy) and
    `krcWatchdogInit` (a GR channel with a 2D object) run during state load /
    `RmInitAdapter` and fail init on error. For milestone 1 the module parameters
    `RmDisableGlobalCeUtils=1` and `RmRcWatchdog=0` (via `NVreg_RegistryDwords`) skip
    them; later they become the first real forwarded GPU work.
20. **MSI-X vector selection on TU106** and the 6-entry VF MSI-X table at BAR0+0xB90000
    (`dev_vm.h:222-223`): the real card offers MSI only, so start with MSI.
21. **Encoder capacity under GSP.** `GET_ENCODER_CAPACITY` is kernel-served
    (`subdevice_ctrl_gpu_kernel.c:974`); find which field it reads so the device can
    set the profile's percentage without a rewrite.
22. **Exporting the profile decision.** `vram::profile_from_env`/`decide` are private to
    the vhost-user backend; the Caraxes device needs the same decision. Move `decide`
    next to the catalogue in `nvrm-abi::vgpu` or make it `pub`.

Update to Q2 (E0, 2026-09-23): confirmed. The driver follows the NPDE of the EFI image
past the PCI last-image flag and expects a third image at 0x1FC00; our sysfs capture ends
there. Two ways to get it, the operator chooses: R1 (PROM window read with the card
passed through, desktop off), or a full VBIOS dump of a TU106 board from a public VBIOS
database (the driver only parses the image to build the FWSEC command; nothing executes
on our device, so a different TU106 board's image is acceptable for E1/E2, with the board
mismatch recorded). Third path, chosen on 2026-09-23 because passthrough is not possible
while the agent session runs on the desktop (decision D8): synthesize the missing third
image so that the driver's parsers accept it; specification in
[vbios-synthesis.md](../caraxes/vbios-synthesis.md), generator `caraxes/tools/synth-vbios.py`.
23. **Aligning device-side RPC logs with the userspace tracer.** From E3 on, the device
   logs `GSP_RM_ALLOC`/`GSP_RM_CONTROL` RPCs; `crates/nvrm-trace` (LD_PRELOAD) logs the
   guest's ioctls with the same class/control vocabulary (`log.rs` decode over the
   `nvrm-abi` tables). Extend the tracer's decoder into a shared function so both logs
   use one format, and run the tracer on `nvidia-smi` in the guest for the E3 success
   check so the RPC stream and the ioctl stream can be compared line by line.
24. **How stable is the GSP API across architectures and driver versions?** Once the
   driver mainly talks to GSP-RM, the RPC vocabulary and the reply structures are the
   device's capability description as far as the guest is concerned. To establish from
   source: whether the RPC enum and message structs are shared by all architectures
   (with per-architecture files such as `src/nvidia/src/kernel/vgpu/arch/ampere/rpcga102.c`
   only selecting paths), how per-message versions (`rpc_init_done_v17_00`,
   `rpc_set_guest_system_info_v03_00`) are checked, and how tightly `gsp_*.bin` is bound
   to the driver version. Consequence for the emulator: the version-bound parts of the GSP
   side should be generated from the vendored headers per driver version, the way
   `crates/nvrm-sys` does for the ioctl ABI. Survey requested 2026-09-23; result to land in
   [other-architectures.md](../caraxes/other-architectures.md).

Update to Q2 (E1, 2026-09-23): the synthetic image passes the parse (`VBIOS version
90.06.3D.00.76` printed, driver proceeds to the GFW wait). Still open: whether the real
extension image differs in ways that matter later (listed under "Open" in
[vbios-synthesis.md](../caraxes/vbios-synthesis.md)); a real capture stays desirable for fidelity.
Update to Q24 (2026-09-23): answered in [other-architectures.md](../caraxes/other-architectures.md) §6.
The RPC vocabulary, structs and per-message versions are fixed per driver release and shared
by all architectures; the firmware must match the driver version string exactly
(`kernel_gsp.c:5719-5774`). Consequence for the emulator: generate the RPC structs per
vendored driver version like `crates/nvrm-sys`; keep only the boot register sequence
per-architecture.
25. **Fuse register values.** E1 read `NV_FUSE_OPT_SECURE_GSP_DEBUG_DIS` 0x2174C three times
   (we answered 0). The synthetic image's FWSEC application id is accepted either way, but
   the booter images and later checks expect production (bit 0 = 1). Answer 1 from E2 on and
   watch for other fuse offsets in the trace.
Update to Q3 (E2, 2026-09-23): the device reports `HWCFG` = 0x100 IMEM and 0x100 DMEM
blocks; the driver only uses IMEM_SIZE to place the 512-byte bootloader at the top, so any
value large enough works; the real value stays unknown until R3.
26. **First-time success in E3.** After a failed `GspStatusQueueInit` the driver retries
   the boot and refuses because WPR2 is already up (`kernel_gsp.c:4805-4811`); real hardware
   behaves the same. E3 must therefore answer the status queue within 4 s on the first
   attempt, or the model must treat the retry's reset sequence like a function-level reset
   (decide from the E3 trace).
Update to Q4 (2026-09-23): answered by [gsp-abi-610.md](caraxes-gsp-abi-610.md): `GspStaticConfigInfo`
1600 bytes, `GspSystemInfo` 952, headers 16 + 32, `GSP_ARGUMENTS_CACHED` 152; the largest
RPC is 65520 bytes.
Update to Q7 (E3, 2026-09-23): settled. The status header carries `SWAP_RX`; the CPU
reads the device's command-queue read pointer at status queue + 0x20 and writes its own
read pointer at command queue + 0x20; the link succeeded at once.
Update to Q26 (E3, 2026-09-23): settled; the queues answer within the first attempt.
27. **The two reads at 0x110804 and 0x110808** after a failed init (E3 trace) are not
   named yet; they read 0 and nothing depended on them so far.
28. **`BUG: Bad page map in process (udev-worker)` after a failed init (E4 iteration 2).**
   Either the driver's failure path or a device write into guest RAM at a wrong address
   (status queue pages, sysmem-targeted BAR accesses through the page-table walk). Check
   whether any BAR access resolved to a sysmem PTE and compare the addresses with the
   DMA maps; it did not recur in the trace as a device-side error.
29. **Diverging guest and host driver versions.** The device speaks the guest driver's GSP
   RPC ABI (today 610.57.04 structures) and the host driver's ioctl ABI (today the same
   version). When they differ, the forwarded controls need per-version parameter layouts on
   both sides: `crates/nvrm-sys` already carries 580/595/610/615 for the host side, and the
   D14 refactor gives the guest side a version module. Leandro v1 already ran a 610.43 guest
   userspace against a 610.57 host. To settle by experiment once the refactor lands: a guest
   with 615.71.09 modules against this 610.57.04 host.
30. **Is the guest DMA-range module permanent?** No: it exists because x86 Linux ignores
   ACPI `_DMA` and Cloud Hypervisor emits none. Removal routes: upstream `_DMA` support on
   x86 plus a Cloud Hypervisor patch, or a vIOMMU in the VM whose IOVA space the device
   maps 1:1 onto the host IOVAs (Cloud Hypervisor's vIOMMU does not cover vfio-user devices
   today). It covers every DMA address the guest kernel computes (RM sysmem, UVM's
   `dma_map_page`, coherent allocations), so CPU-written and GPU-written page-table entries
   alike; the one exception, UVM's page-table root, stays in vidmem.

31. **The 40-bit flush-buffer alias.** The guest needs a DMA address below 2^40 for its
   flush buffer and fails `RmInitAdapter` (0x22:0x59:894) otherwise; after the host's switch
   to 64-bit IOVAs every chunk lies near 2^47, so the launcher aliases the guest's swiotlb pool
   at IOVA 0xf000000000 in the guest table only ([memory-model.md](../caraxes/memory-model.md)). The
   physical GPU never uses the guest's flush buffer (the host RM programs its own), but up to
   2 MiB of ordinary guest RAM shares the alias. Resolve: confirm no GPU access ever carries an
   alias address (an IOMMU fault on the host would show it), and find the clean fix (register
   the first chunk before the 64-bit switch after a host reboot, or a host-side IOVA below
   2^40 for that one page).
32. **Instance-memory location keys.** Route 4 of [dma-routes.md](../caraxes/dma-routes.md) wants page
   tables and instance memory in VRAM so the device sees every write. Verify in
   `vendor/open-gpu-kernel-modules` the exact registry keys (`RmInstLoc*` family) and which of
   them the GSP-client kernel RM honours, plus `uvm_page_table_location=vid` for UVM.
33. **Synthetic heads on display-less silicon (P15).** The device declares heads and
   connectors itself and nothing scans out physically, so any host card could present a
   display, provided the emulated chip identity has a display HAL in the guest driver and the
   same SM architecture as the physical card. Check whether the driver's chip configuration
   allows a display for identities that have none in hardware (GA100, GH100: is the
   KernelDisplay object present when the fuse and static info claim heads?), or whether such
   hosts need a sibling identity with the same SM version.
