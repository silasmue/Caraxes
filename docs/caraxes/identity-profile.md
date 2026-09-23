<!-- SPDX-License-Identifier: MIT -->
# Presenting the card as "Leandro RTX 2070-4G": where each identity element comes from

Goal for the first rewrite: the guest sees a Leandro-branded card whose framebuffer
size is set by the operator through the existing profile policy (`LEA_VGPU_TYPE=4Q`,
`LEA_VRAM_PROFILE_MIB`, `LEA_VRAM_LIMIT_MIB`), with the same names Leandro v1 already
uses. Under Caraxes the identity is a property of the synthetic device, so it holds
before any guest software runs and does not depend on rewriting RM replies.

## The policy Leandro already has (reused unchanged)

| Piece | Where | What it decides |
|---|---|---|
| Policy selection | `crates/vhost-user-nvrm/src/vram.rs:57-83` (`Policy::{Off,Accounting,Reserved,Grid}`), `decide` :187-322, `profile_from_env` :328-362 | From `LEA_VRAM_LIMIT_MIB`, `LEA_VRAM_PROFILE_MIB` + `LEA_VRAM_RESERVE_MIB`, or `LEA_VGPU_TYPE` + `LEA_VGPU_PROFILE_MIB` + `LEA_VGPU_FB_MIB` (+ `LEA_VGPU_ENCODER_CAP`) |
| `Profile` | `vram.rs:88-104`: `size` (what the VM costs the card), `reservation` (overhead allowance), `fb_length` (guest-visible framebuffer), `vgpu_type` (label), `encoder_capacity` | The three numbers the device needs |
| Catalogue | `crates/nvrm-abi/src/vgpu.rs` (`Catalogue::derive` :155-189, `profile_for` :210-235, `resolve` :245-263): vGPU-style arithmetic from total FB, usable FB, VMMU segment size and measured overhead; names `<board>-<n>Q` for whole-GiB shares, `<board>-<size>` otherwise | `RTX2070-4Q` means 4 GiB share, fb_length rounded down to whole segments after subtracting overhead |
| Launcher | `crates/nvrm-client/src/bin/vgpuprofile.rs`: asks the host RM for `GET_VMMU_SEGMENT_SIZE` and `FB_GET_INFO_V2`, derives the catalogue, prints `vgpu_type/vgpu_profile_mib/vgpu_fb_mib/…` for the backend environment | Host-side, unchanged |
| Guest card name | `vram.rs:1222-1249` `guest_card_name`: `Leandro ` + board without the `NVIDIA `/`GeForce ` prefixes + `-<n>G` or `-<n>M` from `fb_length`; must fit `NV2080_GPU_MAX_NAME_STRING_LENGTH` = 64 | `Leandro RTX 2070-4G` for a 4 GiB fb_length; the `Q` label is the launcher's name, the guest sees the size |
| Encoder share | `vgpu.rs:120-125` | percentage of the card, reported to the guest |

Leandro v1 applies this by rewriting selected control replies after the fact
(`rewrite_fb_info`, `rewrite_gpu_name`, PID lists, encoder capacity; `vram.rs:1152-1280`,
`session.rs:2125-2202`) and by charging a ledger at allocation time.

## Where the same numbers go under Caraxes

The guest kernel RM in GSP-client mode takes its identity from three device-level
sources, all of which the synthetic device provides. Routing flags below come from
`src/nvidia/generated/g_subdevice_nvoc.c` (bit 0x40 = `ROUTE_TO_PHYSICAL`).

| What the guest reports | Guest RM source | Device provides | Leandro v1 rewrite that becomes unnecessary |
|---|---|---|---|
| PCI identity (`lspci`, `NV2080_CTRL_CMD_BUS_GET_PCI_INFO`, kernel-served) | config space via BAR0 mirror 0x88000 and real config cycles | vendor `10de`, device `1f02` (TU106 so pci.ids says "TU106 [GeForce RTX 2070]"); subsystem ids free to choose, for example a Leandro-specific subsystem id, as long as `rm_is_legacy_device` does not match | none |
| Chip identity (`GET_ARCH_INFO`, kernel-served) | `NV_PMC_BOOT_0/42` | TU106 values ([registers-turing.md](registers-turing.md)) | none |
| Card name (`GET_NAME_STRING` 0x20800110, `GET_SHORT_NAME_STRING`, both kernel-served, `gpuGetNameString_FWCLIENT`, `gpu_gspclient.c:288-315`) | `GspStaticConfigInfo.gpuNameString[64]`, `gpuShortNameString[64]` (`gsp_static_config.h`) | `guest_card_name(...)` output, e.g. `Leandro RTX 2070-4G`; short name `Leandro RTX 2070` | `rewrite_gpu_name` |
| FB size (`FB_GET_INFO_V2` 0x20801303, kernel-served, `kern_mem_sys_ctrl.c:1018`; `TOTAL_RAM_SIZE` from `Ram.fbTotalMemSizeMb`, `HEAP_SIZE`/`HEAP_FREE` from the guest's own PMA) | `GspStaticConfigInfo.fb_length` and `fbRegionInfoParams` (`mem_mgr_gsp_client.c:59-123`), plus the pre-boot register `NV_PFB_PRI_MMU_LOCAL_MEMORY_RANGE` 0x100CE0 used for the WPR layout | `fb_length` = profile `fb_length` (4 GiB), one usable region `[0, fb_length)` minus whatever reserved region the WPR layout needs at the top; 0x100CE0 encoding the same size | `rewrite_fb_info`, `rewrite_fb_info_list`, VID_HEAP INFO rewrite, and the allocation-time ledger cap for VIDMEM (the guest cannot allocate past its own FB) |
| GPU UUID (`GET_GID_INFO`, kernel-served) | `GspStaticConfigInfo.gidInfo` | Leandro's per-VM UUID (`grid.rs:82-116` `vm_uuid(name_from_socket)`) | the UUID swap in `grid.rs:117-148` for the guest-visible side; the host-side swap remains for forwarded calls |
| Encoder capacity (`GET_ENCODER_CAPACITY` 0x2080016c, kernel-served, `subdevice_ctrl_gpu_kernel.c:974`) | kernel RM state, filled from GSP-side info | check what the kernel implementation reads (open question 21) | `rewrite_encoder_capacity` |
| VMMU segment size (`GET_VMMU_SEGMENT_SIZE` 0x2080017e, **routed to GSP**) | our RPC handler | forward to the host RM (unprivileged control) | none |
| Process lists (`GET_PIDS`/`GET_PID_INFO`, kernel-served) | the guest's own client list | nothing | the VM-only PID roster rewrite (`vram.rs:1047-1107`) |
| Virtualization mode (`rewrite_virtualization_mode`) | `GspStaticConfigInfo.bIsVgx` and `GspSystemInfo.hypervisorType` | choose; a plain "not virtual" identity is the default | opt-in `LEA_VGPU_MEDIATE=mode` |

Everything the guest asks about itself is therefore answered from the device's own
static configuration; nothing has to be patched on the way back from the host.

## What the profile numbers mean on the host side

- `fb_length` is what the guest believes it owns and manages with its own PMA. Every
  guest FB allocation eventually needs real VRAM behind it. Which backing model
  (fake FB in host RAM for milestone 1; a host VIDMEM allocation of `profile.size`
  mapped through BAR1; or an identity-mapped reserved range) is open question 5.
- `size` (`fb_length` + `reservation`) is what the VM costs the card and is the amount
  a host-side admission should hold; `Catalogue::admits` (`vgpu.rs:238-240`) already
  expresses the rule.
- `encoder_capacity` stays a percentage of the physical card.

## Consequences for the plan

1. E0 already presents PCI identity and BOOT registers; E2 adds 0x100CE0 from the
   profile; E3 fills `gpuNameString`, `fb_length`, `fbRegionInfoParams`, `gidInfo` from
   the profile. From E3 on `nvidia-smi` in the guest should print `Leandro RTX 2070-4G`
   with a 4 GiB total, before any real GPU work exists.
2. The backend environment (`LEA_VGPU_TYPE`, `LEA_VGPU_PROFILE_MIB`, `LEA_VGPU_FB_MIB`,
   `LEA_VRAM_*`) and the launcher tool stay as they are; the Caraxes device reads the
   same variables through `vram::profile_from_env` (currently private; export it or
   move `decide` into `nvrm-abi::vgpu`).
3. Keep `guest_card_name` as the single naming rule so v1 and v2 guests are named alike.
