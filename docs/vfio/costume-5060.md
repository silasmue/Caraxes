<!-- SPDX-License-Identifier: MIT -->
# The 5060 costume: an RTX 5060 Ti on the outside, the TU106 on the inside

The operator's side project (2026-09-24): a Leandro VFIO VM "that emulates a 5060 but
deliberately tells the truth about compute capability and so on, i.e. what Turing can
do", to try a half 5060 and work on it. The device presents the RTX 5060 Ti's PCI
identity from the lab capture (`caraxes/reference/gb206/`, [blackwell-gb206.md](blackwell-gb206.md))
and its product name through the naming model ([identity-profile.md](identity-profile.md)),
while everything the guest driver and libcuda derive a capability from stays the TU106 of
the house and runs on it (host RM, host execution, VRAM reservation).

## 1. How it is selected

- Board data `rtx5060ti-on-tu106` (`crates/vfio-user-nvgpu/boards/rtx5060ti-on-tu106.toml`,
  built in, `--board-data rtx5060ti-on-tu106`). A board file rather than a `--costume`
  switch: the board axis (D14, [versatility.md](versatility.md)) is where PCI identity,
  BARs, ROM, VBIOS version and personality live already, and a board file keeps the chip
  (`chip = "tu106"`) and therefore every host path unchanged. The only code added is a
  generic board field, `ids_from`: a capture whose identity words (vendor, device,
  revision, class, subsystem; `config::ID_BYTES`, `crates/vfio-user-nvgpu/src/config.rs:107`)
  are laid over the board's config space (`overlay_ids` :110, `for_board_with_ids` :118).
- Launcher: `CARAXES_COSTUME=rtx5060ti` (only with `CARAXES_CHIP=tu106`) adds
  `--board-data rtx5060ti-on-tu106` in `tools/caraxes-vm.sh`; `up` passes `LEA_ROOT` to the
  device so `ids_from` resolves. `tools/caraxes-vm.sh name` then prints the costume's name,
  and the nvml gate takes its expectation from there.
- With a 1 GiB profile the guest name is `Leandro VFIO RTX 5060 Ti-1G`: platform, transport
  word, personality `RTX 5060 Ti` (the capture's "NVIDIA GeForce RTX 5060 Ti" without the
  `NVIDIA `/`GeForce ` prefixes, `crates/nvrm-abi/src/naming.rs:102-108`) and the profile
  size. A "half 5060 Ti" is a profile question (`CARAXES_FB_MIB`), the name follows it.

## 2. Costume and truth

| Item | What the guest sees | Costume or truth | Source |
|---|---|---|---|
| Vendor/device, revision | `10de:2d04` rev a1 | costume | GB206 capture through `ids_from` |
| Class | 030000 (VGA) | same on both cards | capture |
| Subsystem | `1043:8a08` (ASUS) | costume | capture; kept as `boards/rtx5060ti.toml` keeps it (identity-profile.md: free as long as `rm_is_legacy_device` does not match; it matches nothing in 610, `osapi.c:3673`) |
| Capability chain | PM 0x60, MSI 0x68, PCI Express 0x78 (LnkCap 8 GT/s x16), no MSI-X, no vendor-specific, no extended space | truth (TU106 layout) | RTX 2070 capture; section 4 says why not the GB206 chain |
| BAR0 | 64 MiB | costume | 5060 Ti's size (`gb206/.../bars.txt:3`); the Turing register map lies below 16 MiB and every access is trapped, the RM takes the size only as an upper bound (`nv-pci.c:1849-1899`, `osinit.c:860`) |
| BAR1 / BAR3 / I/O | 256 MiB / 32 MiB / 128 B | BAR1 as the rtx5060ti board (the card's firmware default is 16 GiB, resizable from 64 MiB); the others equal on both cards | board file |
| Expansion ROM | 512 KiB, the RTX 2070's PCI-visible ROM | truth | see VBIOS |
| Product name (NV2080 GET_NAME_STRING, nvidia-smi, NVML, CUDA, torch) | `Leandro VFIO RTX 5060 Ti-1G` | costume (through the naming model) | `GspStaticConfigInfo.gpuNameString`, `crates/vfio-user-nvgpu/src/rpc.rs:420` |
| /proc/driver/nvidia/gpus/*/information "Model" | `NVIDIA GeForce RTX 5060 Ti` | costume, by the guest's own name table | `g_nv_name_released.h:839` via `rm_get_device_name`, `nv-procfs.c:121-125` |
| Short name | `TU106` | truth | chip data |
| PMC_BOOT_0 / BOOT_42 | 0x166000a1 / 0x166a1000 | truth | `chips/tu106/chip.toml:21-23` |
| BAR0 register model, GSP boot | TU106: FWSEC/Booter side effects, `gsp_tu10x.bin`, no FSP | truth | chip tu106 |
| GMMU | v2 (49-bit) | truth | `chips/tu106/chip.toml:121` |
| Classes, engines | TURING_CHANNEL_GPFIFO_A 0xC46F, TURING_COMPUTE_A 0xC5C0, the TU106 engine list | truth | `chips/tu106/chip.toml:148-151`, engine caps |
| GET_ARCH_INFO, "Product Architecture" | Turing | truth | kernel RM from BOOT_0 (`kernel_mc.c:47-70`) |
| Compute capability, SMs, L2 | 7.5, 36 SMs, 4 MiB (torch) | truth | host GR info of the physical card |
| FB size | 1024 MiB | profile | `CARAXES_FB_MIB` |
| VBIOS | 90.06.3D.00.76, synthesized Turing image | truth | see below |
| GPU UUID | `GPU-a3da8c7c-08c7-ea03-e158-7d60549b910f` | the device's own (seeded by the legacy-format name and the FB size) | `rpc.rs` `synthetic_gid`; no UUID, PDI or serial of the capture is used |
| Chip info (INTERNAL_GPU_GET_CHIP_INFO), GET_CHIP_DETAILS pciDevId | 0x2D0410DE / sub 0x8A081043 / rev a1; register bases TU106 | costume ids, truth bases | `ctrl.rs:1184`, `nvml.rs:220-236` |
| GPU Part Number (nvidia-smi) | `2D04-400-A1` | mixed: the costume's device id with the host TU106's SKU | GET_CHIP_DETAILS: SKU from the host, pciDevId from config space |

The VBIOS stays Turing's, both behind the PROM window and in the expansion ROM BAR. The
TU106 boot path parses the PROM image: `kgspExtractVbiosFromRom_TU102`
(`kernel_gsp_vbios_tu102.c:442`, `s_locateExpansionRoms` :276) walks the images and the
FWSEC ucode that sets up FRTS comes out of it, which the GB206 images do not carry (a GB20x
boots through the FSP, [blackwell-gb206.md](blackwell-gb206.md) 1.4). The parser checks
signatures, image length, code type and the NVIDIA PCI data extension but no vendor or device
id, and the kernel RM never reads the expansion ROM BAR (it disables it through the mirror,
`osinit.c:1333`), so a 5060 Ti ROM in the ROM BAR would be seen by `lspci`/sysfs only. The
reported version is the one of the image the guest parses; `--vbios-version 98.06.1F.40.4A`
would print the 5060 Ti's number over a Turing image and is not used.

## 3. Where the guest could catch the costume

From the 610.57.04 sources (`vendor/open-gpu-kernel-modules`, paths below relative to it)
and from the run. Nothing in the kernel RM compares the PCI device id with the chip from
PMC_BOOT_0/42: the HAL comes from the boot registers alone (`osapi.c:3693-3838`
`rm_is_supported_device`, `halmgrGetHalForGpu` at `osapi.c:3757`), and no devid-to-architecture
mapping exists.

| Place | Keyed on | Verdict |
|---|---|---|
| Probe table `kernel-open/nvidia/nv-pci-table.c:39-58`, `rm_is_supported_pci_device` `osapi.c:3840-3887` | vendor 10de, class 0300/0302, any device | passes |
| `rm_is_legacy_device` `osapi.c:3673`, `rm_is_legacy_arch` :3683 | device id | FALSE for 2d04 |
| `gpumgrWaitForBarFirewall` `gpu_mgr.c:4976-5004` | devid 0x2900-0x29FF, 0x3180-0x327F | not 2d04 |
| `pci_devid_is_self_hosted` `kernel-open/nvidia/detect-self-hosted.h:29-54` | Grace-Hopper/Blackwell ranges | not 2d04 |
| Devid quirk tables (`kernel_gsp_tu102.c:898-899` boost off for 0x20BB/0x14A1, `mem_mgr_gm107.c:1467`, `pci_pbi.c:171-190`, `chipset_pcie.c:1159-1166`, gsync, vGPU, Hopper conf compute) | specific ids | none lists 2d04 or 1f02 |
| `osTestPcieExtendedConfigAccess` `os.c:3536-3593` via `kernel_bif_gm107.c:99-127` | OS config dword 0 against the BAR0 mirror's `NV_XVE_ID` | consistent: the device serves the mirror from the same config space (test `bar0_boot_registers_are_tu106`) |
| `kbifWaitForConfigAccessAfterReset` `kernel_bif.c:2024-2070` | OS config device id against `idInfo.PCIDeviceID` after a bus reset | consistent for the same reason |
| Link registers at fixed XVE offsets 0x84/0x88 (`maxwell/gm107/dev_nv_xve.h:104-105`, `kbifGetBusOptionsAddr_GM107` `kernel_bif_gm107.c:993-1025`) | the Turing config layout | **caught with the whole GB206 config space**: 0x84 is Device Capabilities 2 there (PCI Express at 0x60), nvidia-smi showed "Link Width Max 25x, Current Unknown Error" (`caraxes/runs/costume-2026-09-24/gb206-layout/facts/guest-nvidia-smi.txt:106-108`). Fixed by taking only the ids (`ids_from`); the final run shows Gen3 x16 |
| `GspSystemInfo.PCIDeviceID/PCISubDeviceID/PCIRevisionID` `kernel_gsp.c:4498-4500` | sent to "GSP" | the device is the GSP here and ignores them; the host RM never sees the costume ids |
| /proc "Model" against nvidia-smi "Product Architecture" | name table by devid against BOOT_0 | visible contrast ("NVIDIA GeForce RTX 5060 Ti", "Turing"); nothing compares them |
| nvidia-smi GPU Part Number `2D04-400-A1` | devid plus the host TU106's SKU | a mixed value a careful reader could notice |
| VBIOS 90.06.3D.00.76 | the Turing image | a Turing family number on a 5060 Ti name |
| Compute capability 7.5 (a Blackwell GeForce is 12.0), `lspci` LnkCap 8 GT/s (the 5060 Ti: 32 GT/s) | the silicon | intended: the truth the costume is for |
| No extended capabilities (SR-IOV, resizable BAR, DSN), no MSI-X, BAR1 256 MiB, no audio function 01:00.1 | device model | also true of the RTX 2070 identity; the HDA function is never probed by nvidia (class 0403 is not in the probe table; `nv.c:5770` looks it up by slot) |
| libcuda, NVML internal devid tables | closed source | not visible; observed: cuInit, context, JIT, launches, torch and cuDNN run, NVML answers every identity query |

## 4. The check on the GPU (vm1, 2026-09-24)

vm1 with the 1 GiB profile at reservation slot 4 (`caraxes/runs/doorbell-2026-09-24/env-vm1.sh`:
host RM, host execution, reservation, DMA register), the 610 guest image, Cloud Hypervisor
0001-0006 (`/mnt/vmstore/caraxes/vm1/ch-0006-v.sh`), `CARAXES_DOORBELL_TOKENS=64
CARAXES_DOORBELL_POLL=hybrid`, no BAR1 area, `CARAXES_COSTUME=rtx5060ti`, the device of
this change (`target/costume/release/vfio-user-nvgpu`); host driver 615.71.09; vm0 ran on the
card at the same time. Script and evidence: `caraxes/runs/costume-2026-09-24/run.sh`,
`final/` (commit 96ad5f8), and `gb206-layout/` for the first run with the whole GB206
config space (commit 65cfb3e).

| Check | Final (TU106 layout, 5060 Ti ids) | First run (whole GB206 config space) |
|---|---|---|
| Driver load | RmInitAdapter succeeded; dmesg `NVRM: probing 0x10de 0x2d04`, BAR0 64 MB | same |
| `nvidia-smi --query-gpu=name,compute_cap` | `Leandro VFIO RTX 5060 Ti-1G, 7.5` | same |
| torch 2.13.0+cu130 | name `Leandro VFIO RTX 5060 Ti-1G`, capability (7, 5), 36 SMs, 855 MB, L2 4 MB | same |
| nvidia-smi -q | Product Architecture Turing, Brand GeForce, VBIOS 90.06.3D.00.76, Device Id 0x2D0410DE, Sub System Id 0x8A081043, PCIe Gen max 3, Link Width 16x/16x | Link Width Max 25x, Current Unknown Error |
| `lspci -nn` | `NVIDIA Corporation Device [10de:2d04] (rev a1)`, subsystem ASUSTeK 8a08 (the guest's pci.ids does not know 2d04) | same, capabilities at 0x40/0x48/0x60 |
| nvml gate | name, smi-row, total, usage, temp, pstate, util, temp-matches-host, pstate-family, host-name-differs pass; power-number fails (`'N/A'`, the documented 610-on-615 limitation) | same |
| cuda gate | 10 of 12 (the gate script's count: the declared-unsupported cuda-gdb row is among the 12; documents that count only the 11 scored rows write the same result as 10 of 11, testing.md section 3, cuda) scored pass (init, context, core, jit, launch, hostreg, torch, rlprobe, convburn, robustness), cuda-managed fails (`AllocManaged: 801`, as on the 2070 identity), cuda-gdb declared unsupported; unscored: async_streams OOM at 1 GiB | same |
| short kernel (`short-auto`, ms per launch, 2 reps) | 0.410, 0.318 | 0.136, 0.234 |
| rlprobe (50 episodes, s) | 17.6, 17.9 (mean10 56.9) | 21.3, 21.8 |
| Host Xid | none (`final/host-xid.txt`, host watch 0 Xid lines) | none |

The short-kernel numbers scatter with the concurrent load of the other VMs (the `# MEAS`
lines name it); the costume changes no execution path, only identity bytes.

## 5. Tests

`crates/vfio-user-nvgpu/src/costume.rs`: the PCI ids are the GB206 board's and every other
config byte the RTX 2070's, with LnkCap Gen3 x16 at 0x84; the GB206 layout would give width
25 at 0x84; BARs are the 5060 Ti board's; BAR0 boots Turing and mirrors the costume ids;
the static identity names the 5060 Ti with short name TU106 and VBIOS 0x90063D00; the GSP
static info differs from the RTX 2070's only in the name, the GID and the subsystem ids; the
chip tables are TU106's (GMMU v2, Turing classes, engines, no MIG). `board.rs`
`rtx5060ti_on_tu106_is_the_costume` and `ctrl.rs`
`costume_chip_info_has_the_5060_ti_ids_and_tu106_bases` pin the board file and the chip-info
answer.
