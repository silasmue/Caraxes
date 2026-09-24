<!-- SPDX-License-Identifier: MIT -->
# Blackwell lab capture: operator checklist

This is the checklist for one capture run on a lab machine with a Blackwell GeForce card
(for example an RTX 5060 Ti, GB206) and the NVIDIA driver installed. The run is done by
the operator, unattended by us. What comes back is filed under
`caraxes/reference/<chip>/` (for example `caraxes/reference/gb206/`), and a Blackwell
identity is built from it later, per decision D14 ([README.md](README.md)) and the
Blackwell checklist of [versatility.md](versatility.md).

The kit is one script, [caraxes/tools/lab-capture.sh](../../caraxes/tools/lab-capture.sh).
Part 1 needs only bash, coreutils, pciutils and nvidia-smi (python3 is used when present).
Part 2 is optional: when rustc and this repository are on the machine it builds three
small RM query tools and runs them. Everything is read-only towards the card and the
driver, with one opt-in exception (`--rom`, below).

## 1. Prerequisites

- Linux on the lab machine, with the card in it and the NVIDIA driver loaded:
  `nvidia-smi` lists the card and `/proc/driver/nvidia/version` exists. Blackwell runs
  only on the open kernel modules; whatever version is installed is fine. Do not install,
  upgrade or downgrade a driver for this. (If the machine is being set up anyway,
  610.57.04, the version this repository is pinned to, gives the most verified part 2;
  see section 5.)
- The script. For part 1 alone, copying `caraxes/tools/lab-capture.sh` onto the machine
  is enough. For part 2, a checkout (or a copy) of this repository; `vendor/` is not
  needed, `caraxes/tools/` is.
- Optional for part 2: `rustc` (any recent stable; no cargo, no crates, no network) and a
  C compiler (`cc`) for the ABI check.
- Optional: `sudo` for the operator account, for the items in section 3. The script only
  uses sudo non-interactively (`sudo -n`); it never asks for a password.
- About 10 MB of free disk space where the output goes. The kernel log is streamed and
  never stored whole.

## 2. The commands, in order

```sh
# 0. go to the repository (or wherever the script was copied)
cd Caraxes

# 1. check that the driver is up and note the version
nvidia-smi
cat /proc/driver/nvidia/version

# 2. cache sudo credentials in this terminal (asks for the password once);
#    skip this step when no sudo is available, the run then records the
#    sudo-only items as reduced or skipped
sudo -v

# 3. the capture, with the PCI ROM read (section 3 explains --rom)
caraxes/tools/lab-capture.sh --rom -o ~/caraxes-lab

# 3b. without sudo, or without the ROM step:
#     caraxes/tools/lab-capture.sh --no-sudo -o ~/caraxes-lab
#     caraxes/tools/lab-capture.sh -o ~/caraxes-lab

# 4. check the end of the console output: it must say
#    "tarball verified: N files, all sums match"
ls ~/caraxes-lab
cat ~/caraxes-lab/lab-capture-*/MANIFEST.txt
```

Options: `--bdf 0000:01:00.0` restricts the run to one GPU when the machine has several
(default: every NVIDIA display controller); `--no-rm-tools` skips part 2; `--ogkm DIR`
points part 2 at an open-gpu-kernel-modules tree of the running driver version for
`gsp-abi-sizes` (section 5); `--walk-rom FILE` only runs the ROM check on a file.

Expected runtime: one to two minutes; part 2 adds about 20 to 60 seconds for the three
rustc builds. A machine with a very large kernel journal takes longer (the self-test host
read a 930 MB journal in about a minute). Expected output: under 1 MB of files, a
tarball of about 100 KB, plus the ROM (0.1 to 2 MB) with `--rom`.

## 3. What needs sudo, and why

| Item | Without sudo | With sudo | Why |
|---|---|---|---|
| PCI config space (`lspci -xxxx`) | 64 bytes, recorded as `reduced` | all 4096 bytes | The kernel shows unprivileged readers only the standard header; the capability chain and every extended capability (resizable BAR, L1 PM substates, the 16 and 32 GT/s physical-layer capabilities, the NVIDIA vendor capability) sit above it and are what the synthetic config space is built from. |
| `lspci -vvv` decoding | `reduced`: capabilities show `<access denied>` | complete | Same reason. |
| PCI option ROM (`--rom`) | `skipped` | `rom.bin` plus `rom-walk.txt` | The sysfs `rom` file is root-only. Reading it needs `echo 1 > rom` (the kernel sets the enable bit of the card's expansion ROM BAR, config offset 0x30, and maps the ROM), a read, and `echo 0 > rom`. This is the only write of the whole kit, it is opt-in, and it is undone right after the read (also on an interrupted run). It is the same step that captured `caraxes/reference/rtx2070/rom.bin` on 2026-09-23 ([quickstart.md](quickstart.md)). |
| Kernel log | read as user when the journal or `dmesg` allows it | `sudo dmesg` as a fallback | Some distributions restrict `dmesg` to root. |

Why the VBIOS matters: the synthetic identity presents a ROM BAR and a VBIOS version,
and the Turing device builds its whole VBIOS from the captured PCI images
(`caraxes/tools/synth-vbios.py`, [vbios-synthesis.md](vbios-synthesis.md)). For
Blackwell it is the source of the board's PCI images, the BIT tables and the VBIOS
version the guest sees; section 7 says exactly which parts are needed. Right after the
read, the script walks the image chain of the file (read-only, on the file) and prints
a verdict: signatures, image count, type and size per image, the BIT tokens, the VBIOS
version, and whether an FWSEC (code type 0xE0) image is present. The layout it checks is
the one documented by nova-core: https://docs.kernel.org/gpu/nova/core/vbios.html. A
sysfs read ends at the PCI "last image" flag, so a complete dump shows the x86 and EFI
images and, on Turing, no FWSEC image; the FWSEC image follows only in NVIDIA's own chain,
reachable through the BAR0 PROM window, which this kit does not read. On the RTX 2070
this looks like:

```text
image 0 @0x000000: sig 55AA, PCIR, vendor 10de device 1f02, code type 0x00 x86 PC-AT (base VBIOS), PCI length 60416 bytes, PCI last=False, NPDE rev 0x101, subimage 60416 bytes, NVIDIA last=False
image 1 @0x00ec00: sig 55AA, PCIR, vendor 10de device 1f02, code type 0x03 EFI (GOP driver), PCI length 69632 bytes, PCI last=True, NPDE rev 0x100, subimage 69632 bytes, NVIDIA last=False
VBIOS version from BIT BIOSDATA: 90.06.3d.00.76
BIT FALCON_DATA v2: FalconUcodeTablePtr 0xed24 -> in the NVIDIA extension image, which this dump does not contain
VERDICT: PCI image chain complete (the last-image flag is inside the dump); FWSEC (0xE0) image absent (expected for a sysfs read: ...)
```

"PCI image chain complete" is what the operator should see. `INCOMPLETE` or an empty
dump means the read failed; the capture is still valid without the ROM, say so when
bringing it back.

## 4. What to bring back, and where it goes

Bring back two files from the output directory: `lab-capture-<chip>-<host>-<stamp>.tar.gz`
and its `.sha256`. The tarball holds:

| Path | Contents |
|---|---|
| `MANIFEST.txt` | One row per item: `ok`, `reduced`, `skipped` or `failed`, with the reason |
| `SHA256SUMS` | Checksums of every file in the capture |
| `capture.log` | The console output |
| `host/` | uname, os-release, kernel command line, boot time, CPU, loaded modules, `modinfo` of the NVIDIA modules (licence, version, firmware names), every readable file under `/proc/driver/nvidia/` (version, params, registry, per-GPU information), module parameters, the firmware files of the running driver version under `/lib/firmware/nvidia/<version>/` with size and sha256, the linux-firmware files for the chip (`/lib/firmware/nvidia/<chip>/`, used by nova-core and nouveau), the GSP firmware family in use, the filtered kernel log (NVRM, nvidia, Xid, GSP, DRM, IOMMU, PCI lines of the card; first line for the time base; a summary of the most frequent message kinds), `lspci -Dnn`, the PCI tree, the IOMMU groups |
| `gpu-<bdf>/` | `lspci -vvv` of the GPU, of all functions of the slot and of the upstream port; the config space; BAR sizes and resizable-BAR sizes; the sysfs attribute files of the device (link speed and width, AER counters, power state, reset methods, MSI vector count); `/proc/driver/nvidia/gpus/<bdf>/information`; `nvidia-smi -q` as text and XML; `nvidia-smi --query-gpu` with every field the installed nvidia-smi knows, as CSV and one field per line; `rom.bin` and `rom-walk.txt` with `--rom`; `display/` with the DRM connectors of the card, their status and modes, and each EDID as binary, hex and (if installed) `edid-decode` |
| `rm-tools/` | Part 2: `abi-check.txt`, `lab-rm-query.txt` (class list, architecture and implementation, PCI ids as the RM sees them, name strings), `fifo-host-query.txt` (engine list, MMU fault ids, partner lists, PCE masks, CE caps, FIFO info, constructed falcons with register bases), `host-facts.txt` (BAR info, FB info, FB regions, GR masks, CE caps, interrupt table, notification vectors, subtree map; the read-only `facts` mode only), `gsp-abi-sizes.md` when a matching source tree is available |

Privacy: usernames, the home directory and e-mail addresses are replaced in every text
file; the board serial number is redacted in the `nvidia-smi` outputs. Kept: the
hostname, the GPU UUID (it matches captures across runs) and the EDIDs of the attached
monitors (they carry the monitor's serial number; unplug monitors before the run if that
is not wanted).

At home, file it like this:

```sh
sha256sum -c lab-capture-<...>.tar.gz.sha256
tar -xzf lab-capture-<...>.tar.gz
(cd lab-capture-<...> && sha256sum -c SHA256SUMS)
mkdir -p caraxes/reference/<chip>
cp -a lab-capture-<...>/. caraxes/reference/<chip>/
```

`<chip>` is the lowercase code name the script found (`gb206` for an RTX 5060 Ti); it is
also in the directory name. If a second board with the same chip is captured later, use
`<chip>-<board>`. `rom.bin` stays out of git, as `caraxes/reference/rtx2070/rom.bin` does:
add `/caraxes/reference/<chip>/rom.bin` to `.gitignore` in the same change that files the
capture.

## 5. Part 2 and the driver version

The three tools of part 2 (`caraxes/tools/lab-rm-query.rs`, `fifo-host-query.rs`,
`host-facts.rs`) build with rustc alone. They do **not** use `crates/nvrm-sys`: they carry
the 610.57.04 layouts of every structure and control they use as literal offsets, and
they issue only RM getter controls on a private RM client (RM_ALLOC of a root client,
device and subdevice, RM_CONTROL, RM_FREE). So no generated ABI module is needed first,
for any driver version; what changes with the version is whether their layouts still
hold. `caraxes/tools/lab-abi-check.sh` answers that: it compiles a fingerprint of every
structure size, field offset, control id and constant the tools use
(`caraxes/tools/lab-abi-check.c`) against the SDK headers of a version and compares it
with the recorded 610.57.04 fingerprint (`caraxes/tools/lab-abi-610.57.04.txt`). The
script runs it before the tools and writes the verdict into every tool output.

Result for every version whose headers are on the development host
(`target/abi-headers/<v>/`, 2026-09-24):

| Version | Differences from 610.57.04 | Effect on the tool output |
|---|---|---|
| 610.43.02, 610.43.03, 610.57.04 | none | fully verified |
| 615.71.09 | `GPU_GET_PES_INFO_PARAMS` 56 -> 76 bytes, `FIFO_GET_DEVICE_INFO_TABLE_PARAMS` 3212 -> 3340 bytes | the RM refuses both controls (status `NV_ERR_INVALID_PARAM_STRUCT`; it checks the parameter size, `rs_resource.c:134-143`); host-facts loses the per-GPC PES line, fifo-host-query prints only the refused status it printed anyway; the rest is valid |
| 595.91.07, 595.99.02 | `FB_REGION_INFO` has no `regionTag` (same size) | the `tag=` value of the FB regions is padding, ignore it; the rest is valid |
| 580.167.08 to 590.48.01 | as 595, plus `GPU_GET_NAME_STRING_PARAMS` 68 -> 132 bytes | as 595; lab-rm-query retries the name string with the older size, so nothing else is lost |
| 570.211.01, 575.64.05 | as 580, plus `FB_GET_INFO_V2_PARAMS` 1028 -> 460 bytes, `FB_GET_FB_REGION_INFO_PARAMS` 904 -> 776 bytes, no `MC_GET_INTR_CATEGORY_SUBTREE_MAP` | host-facts loses FB info, FB regions and the subtree map (refused); nvidia-smi still has the FB sizes; the rest is valid |

A size mismatch is refused by the RM, never misread, and every control is a getter on
the tool's own client, so running the tools on an unverified version is harmless; the
output is marked `ABI UNVERIFIED` and the differences are listed in `abi-check.txt`.

For a version not in this table, the lab run reports `no SDK headers`. Either run the
check at home once the version is known (the tool outputs of the lab run remain usable
where the fingerprint matches), or fetch the headers first; this needs network access
to GitHub and sends nothing but the request:

```sh
v=$(grep -oE '[0-9]+\.[0-9]+\.[0-9]+' /proc/driver/nvidia/version | head -1)
git clone --quiet --filter=blob:none --no-checkout --depth 1 --branch "$v" \
    https://github.com/NVIDIA/open-gpu-kernel-modules.git "target/abi-headers/$v"
git -C "target/abi-headers/$v" sparse-checkout set --no-cone src/common/sdk/nvidia/inc
git -C "target/abi-headers/$v" checkout --quiet
caraxes/tools/lab-abi-check.sh "$v"
```

(`tools/build.sh vendor-abi <v>` leaves the same checkout in `target/abi-headers/<v>/`,
but also writes `vendor/nvidia-rm-headers/<v>/` and needs clang; it is the step for
adding a version to `crates/nvrm-sys`, which the lab run does not need.)

`gsp-abi-sizes` is different: it reads nothing from the GPU; it compiles the GSP
message-queue and boot structures of a source tree and prints their sizes. It describes
a driver version, not a chip. The script runs it only when the running version is
610.57.04 and `vendor/open-gpu-kernel-modules` is present, or when `--ogkm DIR` names a
full open-gpu-kernel-modules checkout of the running version
(`git clone --depth 1 --branch <v> https://github.com/NVIDIA/open-gpu-kernel-modules`);
otherwise it is recorded as `skipped`, and it can be run at home against that tag with
the same result. `gsp-abi-sizes.c` is written against 610.57.04; on a much older or newer
tree it may need adjusting before it compiles, and the script then records `failed`
with the compiler output.

When the lab version later becomes a device target, that is a separate step: one entry in
`crates/nvrm-sys/abi.toml`, `tools/build.sh vendor-abi <v>` and `cargo xtask abi`
([versatility.md](versatility.md), "Adding a driver version"). The first Blackwell device
does not need it, since 610.57.04 has the GB10x/GB20x HALs.

## 6. What NOT to do

- No driver changes: no install, upgrade, downgrade or rebuild; no `modprobe`, `rmmod`
  or module parameter change; no nouveau or nova-core binding; no vfio-pci binding.
- Nothing that resets or reconfigures the card: no `nvidia-smi -r`, `-pm`, `-pl`, `-ac`,
  `-lgc`, `-e`, `-c`, `--gom`, `-mig`; no writes to `reset`, `remove`, `rescan`,
  `driver_override`, `power/control`, `d3cold_allowed` or `resourceN_resize` in sysfs;
  no suspend or hibernate during the run.
- No register access of any kind: no `setpci` writes, no `devmem`, no `mmap` or read of
  `resourceN`, no `nvflash` or other VBIOS tools (their "read" goes through privileged
  register paths), no `nvidia-bug-report.sh` (it is broad and collects personal data).
- No other Caraxes tools on that machine: not `host-facts reserve`, `host-exec`,
  `host-kick`, `sysmem-iova` or the bench scripts. The kit runs `host-facts facts` only.
- Do not edit the capture by hand; if something looks wrong, rerun the script and bring
  both runs back.

## 7. What we will do with each item

The Blackwell identity work is listed in [other-architectures.md](other-architectures.md)
(§1, §2, §5 and the assessment) and turned into a checklist in
[versatility.md](versatility.md) ("Blackwell (GB10x, GB20x): checklist"). Each captured
item feeds one part of it.

**Boot model** (GSP-FMC instead of FWSEC and Booter; §1, §2).
- `lab-rm-query.txt` `MC_GET_ARCH_INFO` gives architecture (0x1B0 for GB20x),
  implementation and revision, which is what `NV_PMC_BOOT_0` and `NV_PMC_BOOT_42` encode
  and how the guest RM picks the chip's HALs; the config space and `lspci` give the PCI
  device id and revision that must agree with it. The capture reads no register; the
  boot registers are composed from these values, as for TU106.
- `host/gsp-firmware.txt`, `firmware-files.txt`, `modinfo-nvidia.txt`,
  `proc-driver-nvidia/`: which firmware package the driver loads for the chip
  (`gsp_ga10x.bin` and LIBOS3 for GB20x in 610.57.04, §5), its version and hash; the
  guest must find the same file. The linux-firmware `<chip>/gsp/` files (for example
  `fmc`, `bootloader`, `gsp` images of 570.144 for gb206) are what nova-core boots with,
  relevant if the nova-core path of [nova.md](nova.md) is extended to Blackwell.
- `kernel-log.txt`: the driver's own boot messages for the card (GSP boot, FSP, WPR,
  errors) and their timing, as a reference sequence for the new `FirmwareBoot`
  (`src/chip/hopper.rs` in versatility.md).
- `nvidia-smi-q` (GSP Firmware Version, Driver Model, Addressing Mode, Confidential
  Compute fields): the static facts the GSP answers at boot.

**GMMU and memory** (5-level format from Hopper on; versatility.md "GMMU format",
"Engine tables").
- `host-facts.txt` H3 and H4: RAM size, heap, FB tax, reserved regions, BAR1 size, the
  FB region table; the source of `GspStaticConfigInfo` FB facts and of the memory
  profile (D6, D11).
- `bars.txt`, config space: BAR0, BAR1 and BAR2/3 sizes and the resizable-BAR sizes the
  card offers; the device's BAR layout and ROM BAR size (`boards/<board>.toml`).
- `nvidia-smi-q` (FB and BAR1 memory, memory type GDDR7) for the `ver` RAM-type table.
- The page-table format itself comes from the source (`mmu/arch/hopper`,
  `mmu/arch/blackwell`), not from a capture; the capture gives the sizes it must cover.

**Classes and engines** (channel, copy, 3D, compute, video classes; the FIFO device-info
table; [fifo-device-info.md](fifo-device-info.md)).
- `lab-rm-query.txt` class list: the exact class set of the card (Blackwell host, CE,
  3D, compute, NVENC/NVDEC/NVJPG/OFA classes, display classes), for `chip.toml` and the
  method subset check of `channel.rs`.
- `fifo-host-query.txt`: engine list, MMU fault ids, subcontext support, runqueue
  partners, PCE masks, CE caps, FIFO limits, constructed falcons with their register
  bases; the host-derived columns of `device-info.toml` and the engine caps of
  `chip.toml`. Its partner-list query names the Turing channel class `C46F`; on
  Blackwell that query may be refused, the other columns do not depend on it.
- `host-facts.txt` H5 and H6: GPC/TPC/ZCULL masks, PES info, CE caps: the GR and CE
  properties of `chip.toml`.

**Display IP v05** (NVCA7D core channel on GB20x; [display-path.md](display-path.md),
[display-adapter.md](display-adapter.md)).
- The class list shows which display classes the RM exposes (`0xca7d`, `0xca7e` and
  friends) and therefore which display HAL NVKMS will pick in the guest.
- `display/connectors.txt` and the EDIDs: connector types and count of the physical
  board, modes; a reference for the DP connector model of D15 on v05 (the heads stay
  synthetic).
- `nvidia-smi-q` display fields (Display Attached, Display Active).

**Interrupt table** (`interrupts.toml`).
- `host-facts.txt` H7: `MC_GET_STATIC_INTR_TABLE`, `MC_GET_ENGINE_NOTIFICATION_INTR_VECTORS`,
  `MC_GET_INTR_CATEGORY_SUBTREE_MAP`: the vectors and subtree map the device must present.
- `sysfs-attributes.txt` (MSI vector count) and the MSI/MSI-X capabilities in the config
  space: how many vectors the device offers.

**VBIOS** (versatility.md "VBIOS"; [vbios-synthesis.md](vbios-synthesis.md)).
- `rom.bin` and `rom-walk.txt`. Reference for the layout: the nova-core VBIOS
  documentation, https://docs.kernel.org/gpu/nova/core/vbios.html (ROM header and PCIR,
  NPDE, the BIT header and its Falcon data token, the FWSEC image, the IFR header).
- What the Turing identity needs, and what Blackwell does not: for TU106 the kernel RM
  itself reads the whole NVIDIA chain through the BAR0 PROM window, finds the BIT
  FALCON_DATA token, follows it into the 0xE0 extension image and parses the FWSEC
  descriptor to run FRTS (`kernel_gsp.c:4906-4950` calling
  `kgspExtractVbiosFromRom_TU102`), so `synth-vbios.py` keeps the captured x86 and EFI
  images byte for byte and appends a constructed 0xE0 image with a falcon ucode table,
  an FWSEC descriptor and a DMEM mapper. From GH100 on, `kgspExtractVbiosFromRom` is a
  stub that returns `NV_ERR_NOT_SUPPORTED`, which `kernel_gsp.c` treats as "no VBIOS
  image, continue" (other-architectures.md §1): the kernel RM of a GB20x guest never
  parses the ROM; FRTS is set up on the card by FSP (the kernel only tells FSP the sizes,
  `kernel_gsp_gh100.c:323`, `:879`). What the guest RM learns about
  the VBIOS arrives in `GspStaticConfigInfo` (`bVbiosValid`, `vbiosSubVendor`,
  `vbiosSubDevice`, `vbiosRevision`, `vbiosOemRevision`, `gsp_static_config.h:133-137`),
  which the device answers itself.
- So the Blackwell identity needs from the chain: the PCI images as captured (image 0,
  x86, if the board still has one, and the EFI GOP image), served unchanged on the ROM
  BAR for guest firmware and tools that read the option ROM; the BIT BIOSDATA version
  (`VBIOS version from BIT BIOSDATA` in the walk, the same value as `Video BIOS` in
  `proc-information.txt`) for `vbiosRevision`/`vbiosOemRevision` and for nvidia-smi's
  VBIOS version; the PCIR subsystem ids for `vbiosSubVendor`/`vbiosSubDevice`. No 0xE0
  extension image, no falcon ucode table, no FWSEC descriptor and no PROM window model
  are needed, which is why the kit does not try to read past the PCI chain. If the walk
  shows an IFR header (`NVGI`) or a chain that does not end on the PCI last-image flag,
  that is new for a sysfs read and worth a look before building the ROM BAR contents.

## 8. Self-test on the development host

The script was run on the RTX 2070 host (TU106, driver 610.57.04) without sudo
(`--no-sudo --rom`) on 2026-09-24. The record is in
`caraxes/runs/lab-selftest-2026-09-24/` (`README.md` there has the result). The ROM
walk was checked separately on `caraxes/reference/rtx2070/rom.bin` (verdict: chain
complete, no FWSEC image, as documented in [environment.md](environment.md)) and on
`synth-vbios.bin` (FWSEC image present, falcon ucode table found at 0x1fd24).

## Sources

- [other-architectures.md](other-architectures.md) §1, §2, §5, assessment; [versatility.md](versatility.md), Blackwell checklist.
- nova-core VBIOS documentation: https://docs.kernel.org/gpu/nova/core/vbios.html
- `vendor/open-gpu-kernel-modules` (610.57.04): `src/nvidia/src/kernel/gpu/gsp/kernel_gsp.c:4906-4950`, `src/nvidia/inc/kernel/gpu/gsp/gsp_static_config.h:133-137`, `src/nvidia/src/libraries/resserv/src/rs_resource.c:134-143`, `src/nvidia/arch/nvalloc/common/inc/nv-firmware.h:85-119`, `src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080mc.h:61-153`.
- SDK headers of 570.211.01 to 615.71.09 in `target/abi-headers/` for the table of section 5.
