<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Glossary

Every term the overview and the wiki use, one line each. Project names first, then the
driver's vocabulary, then the device's.

## Project

- **Leandro**: the project; one consumer NVIDIA GPU shared among Linux VMs on a host that keeps using it.
- **VirtIO Leandro (v1)**: the API-forwarding architecture: `virtio_nvrm.ko` in the guest, `vhost-user-nvrm` on the host, RM ioctls replayed on the host driver.
- **VFIO Leandro (v2, code name Caraxes)**: the synthetic-device architecture: a stock guest driver on a vfio-user PCI device, `vfio-user-nvgpu` on the host in the GSP-RM role.
- **Caraxes**: the code name of VFIO Leandro; also the name of the device's launcher (`tools/caraxes-vm.sh`), environment variables (`CARAXES_*`) and record directories (`caraxes/`).
- **Leandro-Test**: the optional private repository with the probes, VM provisioning and hardware gates both architectures use.
- **Decision Dn**: a numbered design decision in [caraxes/decisions.md](../caraxes/decisions.md) (D1 GSP-client mode, D11 VRAM slice, D12 privilege, D13 DMA module, D14 three axes, D15 DisplayPort, D16 comptags, D17 shadow tables, D18 sparse registration, D19 615 baseline, D20 ABI snapshot).
- **Idea Pn**: a recorded, unplanned idea in the same document (P13 the two-path decision, P18 SR-IOV middleman, P21 BAR1 window, P23 GSP surface, P24 poll-mode GSP).
- **Gate**: a binary pass-or-fail test run of one area (`nvml`, `cuda`, `gpu`, `vdisplay`, `display`) with v1's verdict rules.
- **Profile (VRAM)**: the guest framebuffer size a VM is given (`4G`, `1333M`, or a vGPU-style `4Q` resolved to a size).
- **Personality**: the board the guest is told it has (`RTX 2070`); with the transport and the profile it forms the guest-visible name.
- **Costume**: a presented chip or board identity that differs from the executing chip.
- **Costume ladder**: the series of costumes measured for how far each can go (TU104, ids-only 5060 Ti, GA106, AD106, GH100, GB206).
- **Lab kit**: the self-contained pack that captures a card the project has no host for.
- **Capture**: read-only evidence taken from a real card; feeds the chip and board tables.
- **Census**: one driver-API operation at a time with markers, giving traps per operation.
- **Profile (device)**: `CARAXES_PROFILE=1`, per-row counts and device time for every trapped access, RPC, control and host call.
- **Coverage map**: workload windows joined with the device profile and LLVM function coverage.
- **Research note**: a short evidence note under [docs/research](../research/README.md), 615 lines cited.

## NVIDIA driver

- **Open kernel modules (OGKM)**: NVIDIA's open-source kernel driver tree; the 615.71.09 tree is the reference of every cited line.
- **RM (Resource Manager)**: NVIDIA's driver core; the kernel RM runs in `nvidia.ko`, the physical RM as GSP-RM firmware.
- **GSP**: the GPU System Processor, a RISC-V core on Turing and later; **GSP-RM** is the physical RM running on it.
- **GSP client (`PF_KERNEL_ONLY`)**: the kernel RM variant that boots GSP-RM and sends it every physical operation.
- **vGPU-VF variant**: the kernel RM variant of an SR-IOV vGPU guest; dead on Turing.
- **HAL**: the per-chip implementation set the RM selects from the boot registers.
- **`NV_PMC_BOOT_0/1/42`**: the three BAR0 registers that name the chip and the runtime variant.
- **Registry**: the RM's key-value configuration (`NVreg_RegistryDwords`), sent to GSP-RM at boot in 615.
- **Client, device, subdevice**: the top of the RM object tree (`NV01_ROOT`, `NV01_DEVICE_0`, `NV20_SUBDEVICE_0`).
- **Control**: a command on an RM object, a 32-bit id plus a parameter struct.
- **Routed control**: a control with `RMCTRL_FLAGS_ROUTE_TO_PHYSICAL`, forwarded to GSP-RM.
- **Internal control**: a control with `RMCTRL_FLAGS_INTERNAL`, sent only by the kernel RM itself.
- **GSS legacy, BINAPI**: control ids outside the export tables that `nvidia-smi` sends for perf, clocks and thermal data.
- **Class**: the type of an RM object (`0xC46F` a Turing GPFIFO channel); the **class database** is the per-chip list of allocatable classes.
- **Engine**: GR (graphics and compute), CE (copy), NVDEC, NVENC, NVJPG, OFA, SEC2; named by the engine mask and the FIFO device-info table.
- **Channel**: a GPFIFO ring plus USERD and an instance block; the unit of GPU work submission.
- **TSG**: a time-slice group of channels sharing a context and a runlist slot.
- **Runlist**: the per-engine list of channels the hardware schedules.
- **GPFIFO**: the ring of pushbuffer pointers of a channel; **pushbuffer**: the method stream.
- **USERD**: the per-channel page holding GPPut and GPGet.
- **GPPut, GPGet**: the write and read pointers of the GPFIFO.
- **Doorbell**: the usermode register (`NVC361_NOTIFY_CHANNEL_PENDING`) a submission writes the work-submit token to.
- **Work-submit token**: `runlist << 16 | ChID`, the value rung on the doorbell.
- **Method, `SET_OBJECT`**: a pushbuffer command; `SET_OBJECT` binds a class to a subchannel.
- **QMD**: the queue meta data of a compute launch.
- **GMMU**: the GPU MMU; **v2** on Turing through Ada, **v3** (5 levels, 57-bit VA) on Hopper and later.
- **PDE, PTE, PDB**: page directory entry, page table entry, page directory base.
- **Page kind**: the memory format of a page (pitch, block-linear, depth kinds); some kinds are compressible.
- **Comptag line, comptag store**: the compression tag index in a PTE and the card-wide store it indexes.
- **Aperture**: where an entry points: vidmem, coherent or non-coherent sysmem.
- **UVM**: unified virtual memory, `nvidia-uvm`; managed memory with GPU faults.
- **RC (robust channel)**: the RM's channel error recovery; **`RC_TRIGGERED`** its event.
- **Xid**: an NVIDIA error report in the kernel log with a numeric class (13 class mismatch, 31 MMU fault, 32 PBDMA error, 38, 45, 109, 143, 154).
- **PBDMA**: the pushbuffer DMA engine that fetches methods; **PBENTRY** its stale-fetch error.
- **FECS**: the front-end context switch microcontroller of GR.
- **BAR0, BAR1, BAR2**: the register window, the CPU window onto VRAM, the RM's page-table aperture (PCI BAR3 on Turing).
- **PRAMIN**: the 1 MiB BAR0 window onto VRAM at `NV_PBUS_BAR0_WINDOW`.
- **`NV_PCFG`**: the BAR0 mirror of the PCI configuration space at 0x88000.
- **XVE**: the PCIe interface block whose registers the mirror exposes.
- **PROM window**: BAR0 0x300000, the VBIOS image chain.
- **VBIOS, FWSEC, FRTS, WPR2**: the video BIOS; its firmware-security image; the firmware runtime services region it sets up; the write-protected region GSP-RM lives in.
- **Booter**: the SEC2 firmware that loads and unloads GSP-RM on Turing through Ada.
- **FSP, Chain of Trust (COT)**: the firmware security processor and the boot protocol that replaces FWSEC and Booter from Hopper on.
- **Falcon**: NVIDIA's microcontroller block (GSP, SEC2); `IMEM`, `DMEM`, `BOOTVEC`, `CPUCTL`, mailboxes.
- **libos arguments, RMARGS**: the boot arguments handed to GSP-RM through the falcon mailboxes.
- **Message queue (msgq)**: the command and status rings between the kernel RM and GSP-RM; **v2** in 615 with pointers in registers.
- **Element**: one message on a ring, MCTP framed.
- **MCTP, NVDM**: the framing standard and NVIDIA's message types inside it.
- **RPC**: a message type on the command queue (`GSP_RM_CONTROL`, `GSP_RM_ALLOC`, `ALLOC_MEMORY`, ...).
- **Event**: a message GSP-RM sends unasked on the status queue (`POST_EVENT`, `RC_TRIGGERED`).
- **GMCAPI**: the 615 command layer beside RPC (`GSP_INIT`, `GSP_SUSPEND`, `GSP_RESUME_DONE`).
- **NVKV**: the key-value word stream of GMCAPI payloads.
- **Static configuration (`GspStaticConfigInfo`)**: the chip and board facts GSP-RM sends once at boot.
- **`NV_ABI_STABLE`**: the tag NVIDIA puts on the transport and boot structs from 615 on.
- **Unstable interface**: the RM RPC layer above the stable transport, opted into by the kernel.
- **nvkms, `nvidia-modeset`, `nvidia-drm`**: the display stack: mode setting, KMS.
- **NV0073 controls**: the display-common controls (DP AUX, EDID, hotplug).
- **DP SST, AUX, DPCD, link training, EDID**: DisplayPort single-stream transport, its side channel, its capability registers, the link bring-up, the monitor's descriptor.
- **Head, window, SOR**: a display pipeline, a plane, a serial output resource.
- **Vblank, flip, `AWAKEN`, `LAST_DATA`**: the frame boundary, a scanout buffer change, and the display-channel interrupts that report them.
- **Displayless class (`NVA083_GRID_DISPLAYLESS`)**: the class v1 uses for a display with no hardware head.
- **NVML, `nvidia-smi`**: the management library and its tool.
- **NVENC, NVDEC**: the video encoder and decoder engines.
- **SM, compute capability**: the streaming multiprocessor and its architecture version (7.5 Turing, 8.6 GA10x, 8.9 Ada, 9.0 Hopper, 12.0 GB20x).
- **Feature family**: a group of controls belonging to one feature a chip may lack (NVLink, MIG, CC, FRL, DP 2.x).
- **MIG, CC, NVLink, C2C, HSHUB, ECC**: multi-instance GPU, confidential computing, the GPU interconnect, chip-to-chip, the high-speed hub, error-correcting memory.
- **vGPU, SR-IOV, VF, PF**: NVIDIA's virtualisation product, PCIe virtual functions, a virtual and the physical function.
- **ReBAR (resizable BAR)**: the PCIe capability that lets BAR1 cover the whole VRAM.
- **nova-core**: the upstream Linux Rust driver for NVIDIA GPUs; a possible future guest.

## The device and the host

- **vfio-user**: the protocol by which a user-space process serves a PCI device to a VMM over a socket.
- **vhost-user**: the protocol by which a user-space process serves a virtio device to a VMM.
- **Cloud Hypervisor**: the VMM both architectures run on, with the patches in `patches/`.
- **Region, sparse mmap area**: a vfio-user BAR and the part of it a VMM maps as memory instead of trapping.
- **Trap**: a guest access that exits to the VMM and travels to the device over the socket; 23 to 35 us.
- **ioeventfd, datamatch**: a KVM mechanism that turns a guest write at an address (and value) into an eventfd signal without leaving the kernel.
- **Doorbell token pool**: the device's own work-submit tokens, one datamatch ioeventfd each.
- **Reactor**: the device thread that polls the eventfds and the memory pages (`spin`, `hybrid`).
- **Interrupt page, display page**: BAR0 pages offered as memory so interrupt servicing does not trap.
- **BAR1 pool**: a private piece of the VRAM slice exported as a dma-buf behind guest BAR1 `[0, pool)`.
- **BAR2 memfd**: region 3 as a sealed memfd, synced at the RM's flush points.
- **Window (shared-memory)**: any of the pages or areas a guest accesses without a trap.
- **Sync point**: a trapped access at which the device reconciles a window with VRAM (flush read, invalidate, bind, GSP doorbell).
- **VRAM slice, reservation, slot**: the fixed range of host VRAM a VM owns, and its position n profile sizes below the top.
- **Carve-out**: the top 2 MiB of the slice, device-owned, holding the BAR roots.
- **Registered guest RAM, OS descriptor**: guest memory pinned and mapped for the GPU through `NV01_MEMORY_SYSTEM_OS_DESCRIPTOR`.
- **GPA, IOVA, HVA**: guest physical address, the address the GPU uses through the IOMMU, host virtual address.
- **`caraxes_dma_range`**: the temporary guest module that makes guest DMA addresses equal to host IOVAs (D13).
- **swiotlb alias**: the alias IOVA below 2^40 for the guest's bounce-buffer pool.
- **Shadow, walk cache, `resolve_leaf`**: the device's validated copy of the guest's tables, its per-root cache, and the admission function.
- **Set A, set B**: the GR context pages the host mirrors into the shadow, and the device-owned ranges a leaf may not name.
- **Gather descriptor, region object, region pool**: one OS descriptor over scattered pages placed back to back; the 2 MiB host mapping unit; the free list of emptied units.
- **Mirror (channel, TSG, VA space)**: the host RM object the device creates for a guest object.
- **Shadow VA space**: the host VA space a mirror runs on, with filtered leaves only.
- **Invalidator**: the privileged device channel that issues the TLB invalidate of a guest PDB.
- **Untrapped-GPPut hold**: applying an invalidate before the guest continues when its USERD does not trap.
- **Fail closed**: refuse and keep state rather than guess; teardown stops at the first failure.
- **Contained Xid**: a host Xid on the device's own pid and mirrors, with the host desktop unaffected.
- **Host watch**: the sampler that judges the host desktop by `nvidia-smi` response time during a run.
- **Emu-boot, pure emulation**: the synthetic GPU without a host RM, reservation or GPU.
- **Version module (`ver/v615`)**: the generated per-driver-version tables of the device.
- **Chip table, board file**: `chips/<chip>/*.toml` and `boards/*.toml`, the two data axes of the identity.
- **ABI snapshot, delta, drift check**: the recorded 615 GSP ABI, a diff against it, the `check.sh` step.
- **Transport class**: ZT-PAGE, ZT-QUEUE, HOST, TRAP, REFUSE; a routed control's potential to leave the trapped path.
- **Category A to E**: where a routed control's answer comes from (chip data, host, guest state, device model, absent feature).
- **Provenance label (D9)**: the source recorded with every answer the device gives.
- **`hostabi.rs`**: the device's hand-kept layouts of the controls it sends to the host RM.
- **`nvrm-sys`, `nvrm-abi`, `nvrm-client`, `nvrm-wire`, `nvrm-trace`**: the shared crates: generated bindings, translation tables, the RM client, v1's wire schema, the `LD_PRELOAD` tracer.
- **`dp-sink`**: the DisplayPort sink library behind the emulated connector.
