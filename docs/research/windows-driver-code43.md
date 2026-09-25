<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# Windows driver and Code 43

Scope: what the registers the Windows driver reads before Code 43 are, whether devinit is involved, the ranked causes with what is already ruled out, and the next device experiments. Runs: `caraxes/runs/win11-2026-09-25` (README "Day one", "Results", "Lead"; `night-2026-09-25/*-summary.txt`).

## Facts

### The order of the driver's accesses

- One attempt is: identification (BOOT_42/1/0, PTOP, PBUS, the config mirror, fuses, PDISP), the seven AUX writes, a second round of fuses and 0x1373a0, `0x110abc`, `0x1478`, `0x140`, the Device Control write, `0x1400`, then the VBIOS through the PROM window, then nothing but a teardown: memory decode off (`W cfg 0x4 = 0x1`) 7 s later [run: T4 trace `gpu-trace.log` of night run C, timestamps 289327109 to 296369163].
- The VBIOS read comes *after* the register sequence, not before: header, PCIR at 0x170, NPDE at 0x180-0x198, image 0 (0x0-0xebfc) twice, then the whole 512 KiB window 0x0-0x7fffc; the 100 `NV_PMC_BOOT_0` reads are interleaved with this scan [same trace; 161307 PROM reads per attempt].
- The GeForce runs have the same PROM pattern: A has 327943 trace lines with 5329 non-PROM, i.e. 2 × 161307 PROM reads, exactly the T4 count per attempt [night-2026-09-25/A-extcfg-mirror-summary.txt].
- Every Windows run served a PROM image of 130048 bytes (x86 image + EFI image, or the x86 image alone with `rom-legacy-only.bin`); beyond it the window answers `0xffffffff` [A-extcfg-mirror-gpu.log line 5; gpu-up.sh:37-40].
- The capture lacks the NVIDIA extension image (PCI code type 0xE0) that the chain announces: the BIT `p` token points the falcon ucode table at 0xed24 + expansion-ROM offset 0x11000 = 0x1fd24, past the end [docs/caraxes/vbios-synthesis.md "Summary"; own decode of the BIT in `rom.bin`].
- The Linux RM failed on exactly this image in E0 (`s_locateExpansionRoms`, `NV_ERR_INVALID_DATA`); Linux runs since then use `synth-vbios.bin`, which appends a 0x600-byte 0xE0 image [docs/caraxes/vbios-synthesis.md "Background"; docs/caraxes/experiments.md "E0 result"].
- The one register value in the sequence that differs from the real card: `0x110abc` answers 0 (GSP falcon model), the card has `0x20` [caraxes/reference/rtx2070/bar0-registers-2026-09-25.txt; A-extcfg-mirror-bar0-head.txt]. All other 34 offsets match the capture or the model's true value.

### Register map (TU106)

| Offset | Name / block | Value served | Meaning | Source |
|---|---|---|---|---|
| 0x1373a0 | NV_PTRIM / PCLOCK block (0x137000-0x137fff), unnamed | 0xa | not public; read 10x in rounds with the fuses | [envytools rnndb/nv_mmio.xml:180](https://github.com/envytools/envytools/blob/master/rnndb/nv_mmio.xml); no match in 615 headers, nouveau, envytools |
| 0x21200, 0x21434, 0x21640 | NV_FUSE (0x21000-0x21fff), unnamed | 0, 1, 1 | SKU/option fuses, names not public | [envytools rnndb/bus/pfuse.xml](https://github.com/envytools/envytools/blob/master/rnndb/bus/pfuse.xml); 615 `tu102/dev_fuse.h` names only 0x2174c and 0x21378 |
| 0x214d8, 0x2156c | NV_FUSE, unnamed | 0x1f02, 0x1f42 | values equal PCI device ids: 0x1f02 RTX 2070, 0x1f42 RTX 2060 SUPER; read only by the data-centre driver | [615 tree: src/nvidia/generated/g_nv_name_released.h:226, 332]; night run C |
| 0x21c04 | NV_FUSE_STATUS_OPT_DISPLAY | 0 (T4: 1) | bit 0 display disabled; with 1 the driver skips all PDISP reads | [nova-core regs.rs](https://github.com/torvalds/linux/blob/master/drivers/gpu/nova-core/regs.rs); night run C |
| 0x22410 | NV_PTOP block, unnamed | 0 | not public | - |
| 0x610018 | NV_PDISP, unnamed on GV100+ | 0x04000000 | on G80-era chips a display clock counter (envytools `nvatiming`); constant here | [envytools nva/nvatiming.c:561](https://github.com/envytools/envytools/blob/master/nva/nvatiming.c) |
| 0x610068 | NV_PDISP pin capabilities | 0x432 | nouveau copies it to 0x640008 at display init | [nouveau disp/gv100.c:1143](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/nouveau/nvkm/engine/disp/gv100.c) |
| 0x610074 | NV_PDISP capability counts | 0x800404 | heads 3:0 = 4, SORs 11:8 = 4, windows 25:20 = 8 | nouveau disp/gv100.c:239, 325, 551 |
| 0x625480 | NV_PDISP FE block, unnamed | 0x4000 | not public; neighbour 0x6254e8 is the display ownership register | [nouveau disp/tu102.c:189](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/nouveau/nvkm/engine/disp/tu102.c) |
| 0x1400 | NV_PBUS_SW_SCRATCH(0) = NV_PBUS_VBIOS_SCRATCH(0) | 5 | VBIOS/devinit scratch, bit meanings not public on TU; FWSEC uses indices 0x0e and 0x15 | [615 tree: src/common/inc/swref/published/turing/tu102/dev_bus.h:27-28]; [615 tree: src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_frts_tu102.c:497] |
| 0x1478 | NV_PBUS_SW_SCRATCH(30) | 0 | bit 0 `GPU_RESET_REQUIRED`, bit 1 `DRAIN_AND_RESET_RECOMMENDED`; 0 = healthy | [615 tree: src/nvidia/generated/g_gpu_nvoc.h:914]; [615 tree: src/common/inc/swref/published/pascal/gp100/dev_bus_addendum.h:31]; [615 tree: src/nvidia/src/kernel/gpu/arch/pascal/kern_gpu_gp100.c:343] |
| 0x140 | NV_PMC_INTR_EN(0) | 0 | interrupts off | - |
| 0x110abc | NV_PGSP HWCFG (EMEM size, bits 8:0), by analogy | **0 (card: 0x20)** | SOE (same falcon IP) has `NV_SOE_HWCFG` 0x0abc with `EMEM_SIZE` 8:0 directly before `EMEMC` 0x0ac0; GSP's `NV_PGSP_EMEMC(i)` is at 0x110ac0 | [615 tree: src/common/inc/swref/published/nvswitch/lr10/dev_soe_ip.h:87]; [615 tree: src/common/inc/swref/published/turing/tu102/dev_gsp.h:50] |
| 0xd96c + n·0x50, n = 0..6 | DP AUX channel n, `TX_TIMEOUT` (block base 0xd930) | written 0xc80 | the card's reset value is 0xc80, so the writes are an idempotent init; no power-down or reset bit | layout: [envytools rnndb/io/pnvio.xml:468-490](https://github.com/envytools/envytools/blob/master/rnndb/io/pnvio.xml) (G94 base 0xe4c0: CTRL +0x24, STAT +0x28, TX_TIMEOUT +0x3c); GM200+ base 0xd930 from [nouveau i2c/auxgm200.c](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/nouveau/nvkm/subdev/i2c/auxgm200.c) (CTRL 0xd954, STAT 0xd958) and [i2c/gm200.c:32](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/nouveau/nvkm/subdev/i2c/gm200.c) (0xd968 = +0x38); capture has 0xd96c = 0xc80 |

- The T4 costume's fused ids (0x1f02, 0x1f42) disagree with its config id 0x1eb8; public sources show no Windows check of this: the Linux RM takes the HAL from BOOT_0/42 only [docs/research/identity-and-costume.md], and GSP-RM checks the PCI id only against its product-name table (`RMDevidCheckIgnore`) [nouveau gsp/rm/r535/gsp.c:585](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/nouveau/nvkm/subdev/gsp/rm/r535/gsp.c). The T4 run continued after reading them to the same end, so the mismatch is not the branch that stops it (it may still be a second gate later).

### Devinit

- On Turing the host does not run devinit: the GPU's own firmware (GFW) does it at reset; nouveau's TU102 `post` only waits for `0x118128` bit 0 and `0x118234 & 0xff == 0xff` [nouveau devinit/tu102.c:75-76](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/nouveau/nvkm/subdev/devinit/tu102.c); nova-core names them `NV_PGC6_AON_SECURE_SCRATCH_GROUP_05_PRIV_LEVEL_MASK` and `..._GROUP_05_0_GFW_BOOT` ("0xff means completed") [nova-core regs.rs](https://github.com/torvalds/linux/blob/master/drivers/gpu/nova-core/regs.rs).
- The x86 option ROM or the EFI GOP is not what POSTs a Turing card; "no GOP ran" is therefore not a devinit gap for this chip. Pre-Turing nouveau detects an un-POSTed card by VGA CRTC regs or `0x2240c` bit 1 [nouveau devinit/nv50.c:112, gf100.c:101]; the capture has `0x2240c` = 2 (set).
- The Windows driver read neither `0x118128`/`0x118234` nor `0x2240c`: it stops before the point where a failed GFW boot would show.
- A failed GFW boot looks from outside like a timeout on `0x118234` (nouveau `-ETIMEDOUT`); the RM reports it as an init failure; nothing like it is in the trace.
- The sequence does not match init-script evaluation: none of 0x1373a0, 0x21434, 0x21200, 0x21640, 0x22410, 0x6100xx, 0x625480, 0x1478, 0x110abc, 0xd96c occurs as a 32-bit literal anywhere in `rom.bin`; only 0x1400 (6x), 0x2240c (2x), 0x118234 (2x), 0x21c04 (1x), 0x140 (2x) do [own search of the image].
- envytools `nvbios` (built without root in the scratchpad) does not parse Turing's BIT `I` v1 (length 0x24): "Failed to parse BIT table 'I' at 0x0283 version 1"; also BIT `B`, `C`, `S`, `U`, `u`, `V`, `x`. Decoded by hand with nouveau's layout: script table 0x466a (one script at 0x7544), condition table 0x466e, PMU opcode tables 0x462a (0x776 bytes), boot scripts 0x751c (0x6b2b bytes); these feed the PMU DEVINIT ucode, not the host [nouveau bios/init.c:398-405](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/nouveau/nvkm/subdev/bios/init.c); [devinit/gm200.c:141-170](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/nouveau/nvkm/subdev/devinit/gm200.c).

### Public knowledge on Code 43 in VMs

- R465 and later drop the hypervisor check; GeForce passthrough is supported (one VM, no SR-IOV on GeForce) [NVIDIA KB 5173](https://nvidia.custhelp.com/app/answers/detail/a_id/5173/~/geforce-gpu-passthrough-for-windows-virtual-machine-(beta)); the old `kvm hidden`/`hv_vendor_id` edits are no longer needed [mathiashueber.com](https://mathiashueber.com/fighting-error-43-nvidia-gpu-virtual-machine/).
- Remaining reported causes since 2021: a broken or host-tainted VBIOS (romfile fixes, VBIOS patchers), the host still owning the card, missing GOP for boot display under OVMF (display only) [Proxmox forum](https://forum.proxmox.com/threads/nvidia-gpu-passthrough-code-43.59827/); on laptops missing ACPI `_ROM`/`_DSM` (Optimus); these are forum reports, not NVIDIA documentation.
- ACPI `_DSM` (NBCI, NVHG, NVOP, MXM) matters for Optimus/MXM laptops; a desktop RTX 2070 does not need it (rumour level for desktops, no public source that it gates Code 43).
- Resizable BAR, MSI, link state, Hyper-V enlightenments: no public source ties them to Code 43 on R465+; MSI was enabled by Windows here before the driver started (`0x6a = 0x81`) [T4 trace 289317636].
- GSP on Windows: `EnableGpuFirmware` (DWORD, 1 = on, 2 = default policy) under the display class key is a community finding, not NVIDIA documentation; `nvidia-smi -q` "GSP Firmware Version" shows whether it is used [guru3D thread](https://forums.guru3d.com/threads/enable-gsp-firmware-on-windows.455714/). NVIDIA documents GSP default-on for Turing+ only for Linux/FreeBSD [README gsp.html](https://download.nvidia.com/XFree86/Linux-x86_64/570.169/README/gsp.html).
- TCC versus WDDM: no public source lists different device requirements; the same sequence with the T4 driver (minus PDISP, plus the two id fuses) says both share the early RM path.

### Where a failure is logged

- The Linux RM names its stages: `RmInitAdapter failed! (stage:status:line)` with stage 0x23 pre-init, 0x30 VBIOS, 0x60-0x62 firmware [615 tree: src/nvidia/arch/nvalloc/unix/src/osinit.c:99-126, 2379]; the Windows RM is the same code base but writes no such text.
- Windows side: Device Manager problem code 43 with `DEVPKEY_Device_ProblemStatus` (0 here); the `nvlddmkm` System-log source logs runtime TDRs (event 13/14/153), not init failures [Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/2282086/how-to-fix-event-id-153-nvlddmkm).
- The WDDM start is `DxgkDdiStartDevice`; its NTSTATUS is visible in an ETW trace of `Microsoft-Windows-DxgKrnl` (WPR `GPU` profile); `logman query providers | findstr /i nv` lists NVIDIA's own ETW providers in the guest [Microsoft ETW guide](https://learn.microsoft.com/en-us/troubleshoot/windows-server/system-management-components/event-tracing-for-windows-simplified).

## Ranked causes

| # | Cause | Evidence | State |
|---|---|---|---|
| 1 | Incomplete VBIOS in the PROM window: no 0xE0 extension image (falcon ucode table, FWSEC, and whatever else a real flash holds), 0xff beyond 0x1fc00 | the driver scans the whole 512 KiB window right before it gives up; the BIT points past the end; the Linux RM failed on the same image (E0) | open, strongest |
| 2 | `0x110abc` = 0 instead of 0x20 (GSP EMEM size) | only value in the sequence that differs from the card; read just before the scratch reads and the VBIOS scan | open, cheap to test |
| 3 | Missing GFW-boot state (`0x118128`, `0x118234`) | never read by the Windows driver | unlikely |
| 4 | T4 costume: fused id 0x1f02 against config id 0x1eb8 | driver continues past the reads | not the first gate |
| - | Hypervisor check | removed in R465 | ruled out |
| - | Expansion ROM BAR off | Results step 1 | ruled out |
| - | No GOP / card not POSTed by firmware | GFW POSTs Turing; step 3 | ruled out |
| - | Root bus against root port, std VGA, class 03 00 against 03 02 | Results steps 3-4, Day one | ruled out |
| - | Extended config space zeros | night runs A, B | ruled out |
| - | GeForce against data-centre driver | night run C, Day one | ruled out |
| - | A wrong BAR0 value among the identification registers | capture rounds 1-3 (except `0x110abc`) | ruled out |

## Consequence for Caraxes (experiments, in order)

1. **Serve a VBIOS with the extension image**: `--vbios caraxes/reference/rtx2070/synth-vbios.bin` (keep `rom-legacy-only.bin` on the ROM BAR). Success: PROM reads above 0x1fc00 return data, and new BAR0 offsets follow the scan (`0x118128`/`0x118234`, `NV_PGSP_FALCON_MAILBOX0` 0x110040, the falcon reset/DMEM ports), or a different stop point. The synthetic FWSEC is a stub: enough for a GSP path the device emulates, not for a CPU-RM path that runs real ucode.
2. **Answer `0x110abc` = 0x20** (let the capture win for unmodelled falcon offsets, or model HWCFG). Success: the sequence after `0x1400` changes. Run it together with 1 and alone, to know which one gated.
3. **The real flash image**: a full PROM-window dump of the host card (1 MiB at BAR0 0x300000, read-only), only with the operator's go-ahead (host GPU rule). Serve it through `--vbios`. Success as in 1, plus the driver's own ucode tables (DEVINIT, PMU, FWSEC) are present.
4. **Make the guest say why**: WPR with the `GPU` profile around `pnputil /restart-device`, read `DxgkDdiStartDevice`'s status; `logman query providers` for the NVIDIA provider; `nvidia-smi -q` for the GSP firmware line once the driver starts.
5. **Costume hygiene for the T4**: serve 0x214d8 = 0x1eb8 and BOOT_42 consistent with TU104 before drawing conclusions from the data-centre driver.

## Open

- The names of 0x1373a0, 0x21200, 0x21434, 0x21640, 0x2156c and 0x625480 on TU10x; not in any public header found.
- Whether 617.14 runs GSP-RM or CPU-RM on a Turing GeForce by default; step 4 answers it.
- What exactly the driver looks for in the 512 KiB scan (IFR header, ROM directory, 0xE0 images).
