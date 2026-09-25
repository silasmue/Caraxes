<!-- SPDX-License-Identifier: MIT -->
# The costume ladder: how far the chip identity can lie over a TU106

The operator's side experiment (2026-09-24): "can one fake the chip completely, GA106 or so,
and only tell the truth about compute capability, everything else lies; try around until it
crashes, to test the limits". The anchor of truth is the executing hardware, the RTX 2070's
TU106 in the house: SM version 7.5, 36 SMs in 3 GPCs of 6 TPCs, the GR context the host RM
builds, the classes whose methods the TU106 actually runs. Each rung lies one layer deeper,
and the point of each rung is the first place where the stock guest driver (610.57.04) or
libcuda catches or breaks on the lie. The operator's additions of the same night: a Hopper
rung (GH100) between Ada and Blackwell, and SM-version rungs on the TU106 identity ("you can
also fake the SM version, just limit tests").

Source paths are relative to `vendor/open-gpu-kernel-modules` at 610.57.04 unless they start
with `crates/`, `docs/`, `caraxes/` or `tools/`. Evidence: `caraxes/runs/costume-ladder-2026-09-24/`.

## 1. The mechanism

| Piece | Where | What it does |
|---|---|---|
| Derived chips | `crates/vfio-user-nvgpu/src/chip/derived.rs`, `chips/{tu104,ga106,ad106,gh100}/` | A presented chip is a base chip's tables (`base = "tu106"` or `"gb206"`) with a TOML overlay merged table by table; an optional `boot.toml` overlay changes the base's boot-model data. Everything the overlay does not name is the base chip's truth, so the overlay *is* the list of lies. |
| Ladder boards | `crates/vfio-user-nvgpu/boards/*-on-tu106.toml` | `chip` (the presented chip), `host_chip = "tu106"` (the executing chip: the device then allows the host flags for a chip other than tu106, `src/main.rs`), `[ids]` (the PCI device id and subsystem when no capture of the card exists, applied by `config::for_board_with_ids`), `class_map` (guest class to host class). |
| Class translation | `src/board.rs` `host_class_of`, `src/mirror.rs` | The mirror allocates the host channel, the shadow channel, the engine objects and the invalidator's channel with the TU106 class of the same role; the guest keeps its own class in everything it reads. The trace notes `as host class 0x... (class_map)`. |
| SM version | `src/kgr.rs` `--sm-version`, `CARAXES_SM_VERSION` | Every `INTERNAL_STATIC_KGR_GET_INFO` answer (host-forwarded or emulated) reports that SM version (`NV2080_CTRL_GR_INFO_SM_VERSION_*`, `major << 8 | minor`, ctrl2080gr.h:288-340). The silicon does not change. |
| Emulated GR facts | `chips/<chip>/chip.toml` `[gr] emulated_info` | Pure emulation answers `INTERNAL_STATIC_KGR_GET_INFO` from the chip's capture facts (GPCs, TPCs, SMs per TPC, subcontexts, SM version) instead of NOT_SUPPORTED; the GB206 table moved here from `chip/physical.rs`, and its SM version is now 0x0C00 (12.0, `NV2080_CTRL_GR_INFO_SM_VERSION_12_00`, ctrl2080gr.h:335): the 0x1200 written before decodes as SM 18.0. |
| GA102-family boot | `caraxes/tools/synth-vbios.py --desc v3`, `src/falcon.rs`, `src/chip/turing.rs` | The FWSEC entry as a boot-from-HS descriptor (V3, one zero RSA3K signature for fuse version 0); falcon DMA transfers report idle, the first DMEM transfer locates the FWSEC data image (the FRTS offset is read from the command the kernel wrote there), `NV_PRISCV_RISCV_CPUCTL.ACTIVE_STAT`, and optional `NV_USABLE_FB_SIZE_IN_MB` and GSP-RM/libos heartbeat registers in the Turing boot data. |
| Push diagnostic for GR | `src/ctrl.rs` | `CARAXES_MIRROR_DIAG=1` also decodes the pushbuffer of GR shadow channels (the method stream libcuda submits). |
| Launcher | `tools/caraxes-vm.sh` | `CARAXES_COSTUME=rtx2080|rtx3060|rtx4060ti|h100|rtx5060ti-gb206` (with `CARAXES_CHIP=tu106`, the executing chip; rtx3060 and rtx4060ti take `caraxes/reference/rtx2070/synth-vbios-fwsec-v3.bin`), `CARAXES_SM_VERSION=<m.n>`, `CARAXES_TAP=none` (a VM without a network device). |

## 2. Procedure

- **Pure emulation** (no host RM, no reservation, no GPU): `vm6`, a fresh overlay on the 610
  image (`/mnt/vmstore/leandro/vm/guest-baked-20260801-610.57.04-20260820-105733.qcow2`) with
  the guest modules of vm0, the 610.57.04 `gsp_tu10x.bin` (from vm4's state directory) and
  `gsp_ga10x.bin` (copied out of the stopped vm1 overlay; the host has only 615's), the serial
  getty, gdm and the network wait masked, started without a tap (`CARAXES_TAP=none`; vm3-615
  held caraxes3). `caraxes-check.service` runs `guest-check.sh` at boot (modprobe, /proc
  information, nvidia-smi, the query of name and compute capability, `nvidia-smi -q`) onto the
  serial console and powers off; `emu-check.sh <dir> CARAXES_COSTUME=...` keeps the serial log,
  the device log and the notable trace lines.
- **GPU check**: vm1 with the environment of `caraxes/runs/doorbell-2026-09-24/env-vm1.sh`
  through `gpu-run.sh <dir> full|stepped|small|ident`: host watch at 5 s with KILL on the device
  pidfile (SLOW=1 s for the SM rungs as the operator set it, 5 s for the chip rungs, since
  single samples of 1.7 to 5.2 s occurred on this shared host with no guest GPU work in flight),
  the host kernel log's Xid lines since the start compared against the device pid after every
  step, the VM down after every run. "vm1 free" was read as no process on
  `/mnt/vmstore/caraxes/vm1/ch.sock` or `gpu.sock`: `pgrep -af /mnt/vmstore/caraxes/vm1` also
  matches the Cloud Hypervisor binary in vm1's directory, which vm2, vm3-615 and vm5-615 run.

### 2.1 The baseline wall of pure emulation (TU106 itself)

At this main the stock TU106 does not reach `RmInitAdapter succeeded` in pure emulation, so
every Turing-HAL and Hopper rung is compared against the same wall:

- Without a host, `INTERNAL_STATIC_KGR_GET_INFO` was NOT_SUPPORTED, `pGrInfo` stayed NULL and the
  scrubber's first channel group failed `numMax == numFree && numMax != 0`
  (kernel_channel_group_api.c:914; `baseline` run 1). Answered from chip data now (section 1).
- Past it: `memmgrMemSet(... TRANSFER_FLAGS_PREFER_CE)` times out (mem_mgr.c:445, then
  `memmgrInitCeUtils` mem_mgr.c:508, `RmInitNvDevice: *** Cannot load state into the device`).
  The device trace says why: the kernel's CeUtils VA space (`FERMI_VASPACE_A` 0xc) gets its root
  through `COPY_SERVER_RESERVED_PDES` and is freed right after (`free 0xc class 37105`), the CE
  channel is allocated without a VA space, and the software executor finds no root at the
  doorbell (`chid 2: VAS root unknown, nothing run`, `crates/vfio-user-nvgpu/src/channel.rs:830`).
  With the host RM the same sequence boots (rung 1 on the GPU). Not fixed here: it is a
  pure-emulation limit of the channel registry, the same for every chip.
- GB206 (the Blackwell N gate's configuration, `CARAXES_CHIP=gb206`) no longer reaches
  RmInitAdapter at this main either: `numGpcs > 0` (kernel_graphics_object.c:192) fails
  `krcWatchdogInit`, identically with the main binary 7621cfb
  (`r5-gb206/emu-main-7621cfb/`), so the regression predates the ladder.

## 3. The rungs

| Rung | Lie | Truth kept | Pure emulation, first failure | What it took | GPU check (vm1) | Verdict |
|---|---|---|---|---|---|---|
| 1 TU104 (RTX 2080) | BOOT_0/42 0x164000a1/0x164a1000, devid 10de:1e82, name | everything else (TU106 tables, VBIOS, config layout, classes) | the baseline wall, identical to TU106 | an overlay of 3 values | boots; nvml gate all but power-number; cuda gate 10 of 12 scored; 0 Xid | **works** |
| 2 GA106 (RTX 3060) | Ampere HAL (BOOT 0x176000a1/0x176a1000), devid 10de:2503, the GA106 class database, channel classes C56F/C7B5/C7C0 | TU106 device-info, interrupts, engine caps, memsys, GMMU v2, feature families, SM 7.5 in GR info | 4 walls, all in the boot (below), then the baseline wall | v3 FWSEC VBIOS, 4 boot-model registers, class map | boots; nvml gate all but power-number; nvidia-smi compute_cap **8.6**; cuda-init passes, cuda-jit `cuCtxCreate: 719 unspecified launch failure` with host Xid 32 (CE) and Xid 13 (GR "Class 0xc7c0 Subchannel 0x1 Mismatch") on the device's channels; ladder stopped (section 3.2) | **breaks at SET_OBJECT**: the class id travels in the method stream |
| 3 AD106 (RTX 4060 Ti) | Ada HAL (0x196000a1/0x196a1000), devid 10de:2803, AD106 classes, C9C0 | as rung 2 | rung 2's walls plus the Ada scrubber (`done bit not set`), then the baseline wall | + SCRUBBER_HANDOFF_DONE in the boot data | not run (the ladder stopped at rung 2) | **boots as far as TU106 in emulation** |
| 4 GH100 (H100 PCIe) | Hopper HAL (0x180000a1/0x180a1000) on the GB206 FSP model and GMMU v3, devid 10de:2331 sub 10de:1626, GH100 classes, NVLink/HSHUB/fabric/C2C/MIG/CC present, MIG on | GB206 register map, device info, interrupts | Xid 143 "Error status 0x65 while polling for FSP boot complete" (guest-side; kern_fsp_gh100.c:726-770): Hopper's `NV_THERM_I2CS_SCRATCH` is 0x200BC, GB20x's 0xAD00BC; then CC, HUB PCE mask and CCU complaints, then the baseline wall | a boot overlay (FSP-boot-complete, WPR2 in the FB unit, the GA100 debug fuse) | not run (the ladder stopped at rung 2) | **boots as far as TU106 in emulation**; the present families are asked, none is fatal |
| 5 GB206 over TU106 | the whole GB206 identity (the N gate's) with `host_chip = "tu106"` | none of the chip; only the TU106 executes | the GB206 baseline at this main (`numGpcs > 0`) | the board | not attempted: host execution needs the shadow walker to read GMMU v3 guest tables, and the incremental walker (ptcache.rs) is v2 only (`MMU_FORMATS_UNWALKED`, chip/mod.rs) | **breaks at the page-table format** (by construction) |
| SM 7.0 on TU106 | `--sm-version 7.0` in every KGR info answer | the chip | (host-only rung) | the flag | the device answered 0x0700 (`kgr info: 63 entries from host; SM version 0x0705 reported as 0x0700`), yet `nvidia-smi` compute_cap 7.5 and torch `(7, 5)` | **ignored**: CUDA does not take the SM version from GR info |
| SM 8.6, 12.0 | | | | | not run: the 7.0 rung showed the field does not reach CUDA, and rung 2 showed that an 8.6 answer comes with the Ampere identity (section 3.4); the ladder had stopped | moot |

### 3.1 Rung 1: TU104

`chips/tu104/chip.toml` changes the boot identity and the short name; the class database stays
TU106's because the TU104 lists of the 610 tree (`g_gpu_class_list.c:163-267`) differ from
TU106's (:270-375) only in the NVDEC instance count, not in any class id, and
`gpuChildrenPresent_TU104` equals TU106's (kern_gpu_tu102.c:146-205). What the driver derives
from the chip id beyond the name: the HAL (halmgr from `PMC_BOOT_42`, osapi.c:3757), which for
TU104 and TU106 selects the same TU102 implementations; `/proc` "Model" from the name table by
devid; nvidia-smi "GPU Part Number" `1E82-400-A1` (the costume's devid with the host TU106's
SKU, as in costume-5060.md).

GPU check (`r1-tu104/gpu/`, `r1-tu104/gpu-ident/`, device pids 1277623 and 1368802): RmInitAdapter
succeeded; nvml gate: every check but power-number (`'N/A'`, the documented 610-on-615
limitation); cuda gate: init, context, core, jit, launch, hostreg, torch, rlprobe, convburn,
robustness pass, cuda-managed fails (`AllocManaged: 801`, as on the 2070 identity), cuda-gdb
declared unsupported; `nvidia-smi --query-gpu`: `Leandro VFIO RTX 2080-1G, 7.5, 0x1E8210DE,
90.06.3D.00.76, 1024 MiB`; Product Architecture Turing; lspci `TU104 [GeForce RTX 2080]
[10de:1e82]`; torch `Leandro VFIO RTX 2080-1G`, (7, 5), 36 SMs, 855 MB, L2 4 MB. 0 host Xid.
The first run's host watch saw one 2.5 s host nvidia-smi sample during the unscored suites
(multi-process) and stopped the device, so the identity was read in a second, probe-only run.

### 3.2 Rung 2: GA106

The guest takes the GA102-family HAL from `PMC_BOOT_42`. The walls, in order, each read from the
guest's NVRM lines and fixed in the costume layers (VBIOS, boot model), never in the guest:

1. `Assertion failed: pFlcnUcode->bootType == KGSP_FLCN_UCODE_BOOT_FROM_HS @
   kernel_gsp_falcon_ga102.c:204`, then `kgspExecuteFwsec_TU102: failed to execute FWSEC cmd
   0x15: status 0x56`. The Turing VBIOS carries FWSEC as a V2 (boot with loader) descriptor
   (kernel_gsp_fwsec.c:653-669 picks the fill by version); the GA102 HAL runs HS ucode only
   from BROM (`kgspExecuteHsFalcon_GA102`). Caught by the **VBIOS layer**. Fix:
   `synth-vbios.py --desc v3` (`caraxes/reference/rtx2070/synth-vbios-fwsec-v3.bin`, generated,
   git-ignored like the other images; the oracle checks fwsec.c:866-1000 and frts.c:341-395),
   plus the falcon's DMATRFCMD reading idle (`s_dmaPoll_GA102` would time out otherwise).
2. `kgspExecuteFwsec_TU102: failed to execute FWSEC for FRTS: WPR2 initialized at an unexpected
   location: 0x0003fe00 (expected 0xfffffe00)`: the GA102 memsys HAL reads the FB size from
   `NV_USABLE_FB_SIZE_IN_MB` 0x1183A4 (kern_mem_sys_ga102.c:37-52), which read 0. Fix:
   `usable_fb_size_mb` in `chips/ga106/boot.toml`; the model now also reads the FRTS offset
   from the DMA-loaded FWSEC data (`fwsec: data buffer PA ..., frtsOffset 0x3fe00000 (from the
   FWSEC command in guest RAM)`).
3. `kgspBootstrap_TU102: Failed to boot GSP.` (kernel_gsp_tu102.c:598): after Booter Load the
   GA102 HAL checks `NV_PRISCV_RISCV_CPUCTL.ACTIVE_STAT` (`kflcnIsRiscvActive_GA102`), not the
   Turing RISCV_STATUS. Fix: the falcon model answers it.
4. `_kgspRpcRecvPoll: GSP RM heartbeat timed out` / `LibOS heartbeat timed out`
   (kernel_gsp.c:2296): libos 3 GSP-RM keeps heartbeats in `NV_PGSP_MAILBOX(0/1)`. Fix: the
   heartbeat registers in the boot data, answered with the device's PTIMER ms.
5. Tolerated, not a wall: `NV2080_CTRL_CMD_INTERNAL_CONF_COMPUTE_GET_STATIC_INFO` NOT_SUPPORTED
   (conf_compute.c:560, :455; the `cc` family is absent on the TU106 truth), then the baseline
   wall of section 2.1.

In emulation the guest shows `GA106 [GeForce RTX 3060] [10de:2503]` (lspci) and Model "NVIDIA
GeForce RTX 3060".

The GPU check (`r2-ga106/gpu/`, 2026-09-25 07:57-07:59, device pid 3110668, `gpu-run.sh stepped`
with `CARAXES_MIRROR_DIAG=1`): RmInitAdapter succeeded on the host RM; the nvml gate passes
everything but power-number (name "Leandro VFIO RTX 3060-1G", 1024 MiB); `nvidia-smi
--query-gpu`: `Leandro VFIO RTX 3060-1G, 8.6, 0x250310DE, 90.06.3D.00.76, 1024 MiB`, although
the GR info the device forwarded says SM 7.5: the compute capability CUDA sees follows the chip
identity (section 3.4). cuda-init passes. cuda-jit fails at `cuCtxCreate: 719 unspecified
launch failure`. The mirror created the host objects as mapped (`shadow: chid 5 host object
class 0xc7c0 as host class 0xc5c0 (class_map)`, `0xc7b5 as host class 0xc5b5`), and the push
diagnostic shows what libcuda submits first on each channel:

- the copy channels: `20018000 0000c7b5 ...`, SET_OBJECT on subchannel 4 with AMPERE_DMA_COPY_B;
  the TU106 PBDMA rejects the class against the bound TURING_DMA_COPY_A: host Xid 32 on
  channels 0x2f-0x32, `HCE_DBG1 0000c7b5`;
- the compute channel: `20012000 0000c7c0 20012040 00000000 200120a8 00007886 200120a9 ...`,
  SET_OBJECT on subchannel 1 with AMPERE_COMPUTE_B, then Ampere compute-class state methods; the
  TU106 GR raises Xid 13 `Graphics Exception: Class 0xc7c0 Subchannel 0x1 Mismatch` (ESR
  0x4041b0=0x21c7c0), and the host RM tears down the TSG's channels (Xid 13 on channels
  0x33-0x36, `Class 0000c597`).

So the break comes before any QMD or SASS: the class number is data in the method stream, and
the class map, which only renames host objects, cannot hide it. Past this point the device
would have to rewrite the guest's pushbuffers (every SET_OBJECT, and every Ampere-only method
and QMD layout after it): that is the physical engine executing methods TU106 does not have,
so the rung stops here, as the brief says.

**Stop rule.** 40 host Xid lines in the run; the channel-level lines name the device pid, but
24 of the exception detail lines (the ESR and "Class ... Mismatch" lines of the same Xid 13
events) are printed with the pid of whatever process was current in the interrupt path: the
host watch's own nvidia-smi (pids 3120082, 3121005) or no pid at all
(`r2-ga106/gpu/host-xid.txt`). By the letter of the rule these are Xids outside the device pid,
so `gpu-run.sh` stopped after cuda-jit and the ladder stopped there: no further GPU run (Ada,
Hopper, Blackwell and the SM 8.6 rung stayed in emulation or unrun). No other VM's channel
appears in the lines, the host watch's nvidia-smi stayed at 0.06 s at most, the device went
down with the VM.

### 3.3 Rungs 3 and 4: Ada and Hopper

AD106 needs what GA106 needs plus one Ada step: `kgspExecuteScrubberIfNeeded_AD102:
failed to execute Scrubber: done bit not set` (kernel_gsp_ad102.c:49-90) until the boot data
says the devinit scrubber already ran (`NV_PGC6_BSI_VPR_SECURE_SCRATCH_15` SCRUBBER_HANDOFF
31:29 = DONE, 0x1180FC; invented, marked in `chips/ad106/boot.toml`). Then the CC complaint and
the baseline wall; Model "NVIDIA GeForce RTX 4060 Ti".

GH100 runs the GB206 FSP model. Its first wall is a guest-side Xid 143 in the FSP boot poll:
Hopper keeps `NV_THERM_I2CS_SCRATCH` at 0x200BC (hopper/gh100/dev_therm.h:26), GB20x at
0xAD00BC; the Hopper WPR2 check reads the FB unit's 0x1FA824/8 (`kgspIsWpr2Up_GH100` calls the
TU102 one, kernel_gsp_gh100.c:229) and the debug fuse is the GA100 one (HAL
g_kernel_gsp_nvoc.c:624-627). With `chips/gh100/boot.toml` the FSP Chain-of-Trust, the GSP boot
and the init RPCs pass. Of the families declared present, the guest asks at boot for
Confidential Computing static info (`INTERNAL_CONF_COMPUTE_GET_STATIC_INFO`, asserted twice,
conf_compute.c:560 and :455, continues), the HUB PCE mask (`CE_GET_HUB_PCE_MASK_V2`, kernel_ce.c:890,
continues) and the CCU shared buffer (`kccuShrBufInfoToCcu_IMPL: CCU memdesc map request
failed`, continues); none of NVLink, fabric, C2C or MIG is asked before the baseline wall. So
nothing the Hopper HAL insists on at boot stops it before the point TU106 itself reaches.

### 3.4 The SM-version rungs

`--sm-version 7.0` changed the one field the device owns: every `INTERNAL_STATIC_KGR_GET_INFO`
answer carried SM version 0x0700 (`sm70-tu106/gpu/device-log-first.txt`, the trace line
`kgr info: 63 entries from host; SM version 0x0705 reported as 0x0700`). The guest's
`nvidia-smi --query-gpu=compute_cap` still said 7.5 and torch `(7, 5)`: NVML and libcuda do not
take the compute capability from the GR info's SM version. The converse came from rung 2: with
the GA106 identity and the TU106's true SM 7.5 in GR info, nvidia-smi reported 8.6. So the
compute capability follows the chip identity the guest kernel derives from the boot registers
(the architecture and implementation of `NV2080_CTRL_CMD_MC_GET_ARCH_INFO`, kernel_mc.c:47-70,
and the compute class the chip's class list carries; libcuda and NVML are closed, so which of
the two decides is not measured), and the operator's premise, "fake the chip, tell the truth
about compute capability", cannot hold past the Turing family: an SM-version lie is a chip lie
and a chip lie is an SM-version lie. The run's CUDA probes did not execute (the guest's device nodes were missing in the
small mode; fixed in `gpu-run.sh` since), and the host watch stopped the device at 00:52 on a
5.2 s host nvidia-smi sample while the VM was idle. The run script had died on an edit made
while it ran (bash reads scripts as it goes), so the VM itself stayed up without its device from
00:52 until it was brought down at 07:56. SM 8.6 and 12.0 were not run.

## 4. The general lesson

| Layer | Can it lie over the TU106? | Evidence |
|---|---|---|
| PCI ids, subsystem, name, BAR sizes | yes, freely | costume-5060.md; rung 1 to 4 |
| Config-space layout | no: the Turing RM reads the link registers at fixed XVE offsets | costume-5060.md section 3 |
| Boot identity within a HAL family (TU104 on TU106) | yes | rung 1: nothing in the driver distinguishes |
| Boot identity across families (Ampere, Ada, Hopper) | yes, if the costume also lies consistently where the new HAL looks: the VBIOS (FWSEC descriptor format), the boot model (registers the new HAL reads: usable FB size, RISC-V CPUCTL, heartbeats, the Ada scrubber handoff, Hopper's FSP registers) | rungs 2 to 4 reach the TU106 baseline in emulation |
| Class database and channel classes | yes toward the guest; the host needs a class map to the executing chip's classes | rung 2 GPU check |
| GMMU format | no, not without a v3-to-v2 shadow walker | rung 5 |
| Compute capability | not independently: CUDA derives it from the chip identity (TU106 with SM 7.0 in GR info: 7.5; GA106 with SM 7.5 in GR info: 8.6) | SM 7.0 rung, rung 2 |
| Classes on the executing engines | no: libcuda writes its class id into the method stream (SET_OBJECT), and the TU106 PBDMA and GR reject a foreign class (Xid 32, Xid 13 Class Mismatch) before any QMD | rung 2 GPU check |

In one line: the identity can lie on every layer the guest only reads (ids, names, boot
registers, VBIOS, boot-model side effects, class database), and it stops lying where the guest
*writes* something the silicon interprets: the class ids in the method stream, the page-table
format, and (through the architecture) the compute capability libcuda compiles for.
