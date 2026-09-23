<!-- SPDX-License-Identifier: MIT -->
# nvidia-smi on the synthetic device: controls, sources, gate

Written 2026-09-23. After milestone 1, `nvidia-smi` in the guest printed the Leandro name
and 4096 MiB, but temperature, power, fan, performance state and utilization read N/A
because five controls went unanswered. The module
`crates/vfio-user-nvgpu/src/nvml.rs` now answers every control that `nvidia-smi` and
`nvidia-smi -q` make the guest kernel send. The dispatcher calls it after the KGR
hook in `Dispatcher::control` (`ctrl.rs`). Each answer carries a provenance label
(decision D9), which the device trace prints as `E boot: ctrl <cmd>: ... [label]`.

Result on 2026-09-23: no control is unanswered for either command. The guest
`nvidia-smi -q` has the same N/A fields as the host's own `nvidia-smi -q`, except
"PCIe Generation, Host Max" (see [Left N/A](#left-na)). The gate
`tools/caraxes-gate.sh nvml` passes.

## How the controls arrive

Section 2 of [phase5-rpcs.md](phase5-rpcs.md) describes the routes by which a control
reaches GSP. The device answers each route as follows:

| Route | Handles | What the device does |
|---|---|---|
| ROUTE_TO_PHYSICAL public controls | guest user client (`0xc1d0xxxx`/`0xa55axxxx`) | It sends the same command to the host object of the same class, with no change |
| GSS legacy (bit 0x8000, categories THERMAL 0x85, CLK 0x90, PERF 0xa0, PMGR 0xa6) | guest user client | It forwards the command opaquely to the host subdevice. Only the 30 commands in the host's own `nvidia-smi -q` trace (`matrix/traces/610.57.04/nvml.tsv`) are forwarded, so the host card never receives a command that a read-only query tool would not send |
| Kernel-served controls that issue their own RPC (K1, K2, K5) | guest kernel internal client (`0xca7a0001`) or guest user | The answer is translated, synthesized, or forwarded as the public equivalent |
| NV2081_BINAPI commands | guest user BINAPI object | It forwards the command to a BINAPI object that the device's host client allocates on first use (`hostrm.rs`) |

The dispatcher now keys objects by (hClient, hObject), because RM handles are
per client and the guest's clients reuse numbers. When a client is freed, its
objects are forgotten with it. The class that the guest allocated decides the host
object: device, subdevice or BINAPI.

The two fixes that made forwarding work:
- **NULL params for zero-size controls.** `GPU_QUERY_INFOROM_ECC_SUPPORT` has no
  params. With a non-NULL pointer and size 0, the host RM returned
  `NV_ERR_INVALID_ARGUMENT` (0x1f), and nvidia-smi showed ERR! in the ECC column.
  `hostrm.rs` now passes NULL.
- **GSS-legacy replies go back with rmctrlFlags 0.** Otherwise the guest kernel
  caches the reply (`rpc.c:10902-10907`), and telemetry would freeze (`boot.rs`).

## Control list with sources

These controls were observed in the final run: `nvidia-smi` then `nvidia-smi -q`, on
the stock 610.57.04 guest driver.
- `nvidia-smi` sent 33 controls (20 unique) and 5 allocations. Labels: 28 host, 5 synthesized.
- `nvidia-smi -q` sent 98 controls (55 unique) and 5 allocations. Labels: 91 host, 1 profile, 6 synthesized.

The "Host status" column gives what the host card returned. `0x56`/`0x57` are the
host card's own NOT_SUPPORTED and INVALID_STATE answers, which the host's nvidia-smi
also receives.

### Forwarded to the host (label `host (...)`)

| Control | Name | Host status | nvidia-smi field it feeds |
|---|---|---|---|
| 0x00800294 | GPU_GET_BRAND_CAPS (device) | 0 | Product Brand |
| 0x2080012f | GPU_QUERY_ECC_STATUS | 0x56 | ECC errors (N/A, as on the host) |
| 0x2080014b | GPU_GET_INFOROM_OBJECT_VERSION | 0 / 0x57 | InfoROM versions |
| 0x20800156 | GPU_GET_INFOROM_IMAGE_VERSION | 0 | InfoROM image version |
| 0x20800157 | GPU_QUERY_INFOROM_ECC_SUPPORT (no params) | 0x56 | ECC mode (N/A, as on the host) |
| 0x20801322 | FB_GET_OFFLINED_PAGES | 0x56 | Retired pages |
| 0x20801344 | FB_GET_REMAPPED_ROWS | 0x56 | Remapped rows |
| 0x20801357 | FB_QUERY_DRAM_ENCRYPTION_INFOROM_SUPPORT | 0x56 | DRAM encryption mode |
| 0x20801813 | BUS_GET_PEX_COUNTERS | 0 | Replays since reset |
| 0x20801819 | BUS_GET_PEX_UTIL_COUNTERS | 0 | Tx/Rx throughput |
| 0x20801823 | BUS_GET_INFO_V2 (K4, PCIE_GEN_INFO) | 0 | PCIe generation max/current |
| 0x20801829 | BUS_GET_PCIE_REQ_ATOMICS_CAPS | 0 | Atomic caps |
| 0x20801830 | BUS_GET_PCIE_CPL_ATOMICS_CAPS | 0 | Atomic caps |
| 0x20802068 | PERF_GET_CURRENT_PSTATE | 0 | Perf / Performance State |
| 0x20802087 | PERF_GET_VID_ENG_PERFMON_SAMPLE | 0 | Encoder/decoder utilization |
| 0x20808513 | THERMAL_SYSTEM_EXECUTE_V2_PHYSICAL (K5); sent to the host as the public 0x20800513, which has the same struct (message id 0x13) | 0 | Temp (opcodes 0x100, 0x101, 0x1500 seen) |
| 0x2080852a, 852c, 852e, 852f, 8536, 853a, 8542, 8546 | GSS-legacy thermal, unnamed in the open headers | 0 (8546: 5 of 13 times 0x56) | Temperature thresholds, fan speed, thermal event reasons |
| 0x20809004, 9009, 9019, 9038, 9064 | GSS-legacy clock | 0 (9038: 0x56) | Clocks, max clocks |
| 0x2080a014, a060, a06e, a079, a080, a081, a097, a0a4, a0a7, a0a8, a0f2 | GSS-legacy perf | 0 (a0f2: 0x56) | Utilization, clocks event reasons and counters, pstate details |
| 0x2080a612, a613, a618, a61a, a61f, a63c | GSS-legacy pmgr | 0 (a63c: 0x56) | Power draw, power limits |
| 0x20810108, 0x20810110, 0x20810111 | NV2081_BINAPI commands, unnamed | 0, 0x56, 0x56 | nothing visible (0x0110/0x0111 are not in the host trace; the guest adds them) |

Identifying 0x2080852e: THERMAL_LEGACY_NON_PRIVILEGED (0x85), message 0x2e. The
open headers do not name it. The request is all zeros, so it is output only. The host
fills 1432 bytes, which is the size of THERMAL_SYSTEM_EXECUTE_V2, but in a different
layout: the header words are 1, 1, 0, 1, then small constants including 96 and 5000.
It is one of the five controls the milestone-1 run left unanswered, and after
forwarding, the fan and temperature columns read numbers. Which field it feeds by
itself has not been isolated.

### Mixed: host data with device-owned fields

| Control | Label | What is device-owned |
|---|---|---|
| 0x20800102 GPU_GET_INFO_V2 (K1) | `host (GPU_GET_INFO_V2, marked indices)` and/or `profile (chip-unique indices 0x23/0x24 from the device GID)` | Indices marked with bit 31 are cleared. Index 0x11 is asked from the host. 0x23/0x24 are chip-unique on the host (0xbe1523a4, 0x79f23fe1, printed as "GPU PDI"), so they come from the device GID: the guest shows `GPU PDI 0x74c0233d3db9d79b` |
| 0x208001a4 GPU_GET_CHIP_DETAILS | `host (GET_CHIP_DETAILS: SKU, major, minor) + card (pciDevId from config space)` | pciDevId is set to the emulated function's device id |
| 0x20801303 FB_GET_INFO_V2 | `host (FB_GET_INFO_V2) rewritten to the profile size` | RAM, TOTAL_RAM, HEAP, MAPPABLE_HEAP, HEAP_FREE and USABLE_RAM are set to the profile's `fb_length`, following the v1 `rewrite_fb_info` rule. It was not observed: the guest kernel serves FB_GET_INFO_V2 itself (`kern_mem_sys_ctrl.c:1018`). The rewrite is a safety net and has unit tests |

### Synthesized (label `synthesized (...)`)

| Control | Answer | Why |
|---|---|---|
| 0x20800a4c INTERNAL_GPU_GET_SMC_MODE (K2), 7 calls | NV_OK, smcMode 0 (UNSUPPORTED) | An internal control, not forwardable, and TU106 has no MIG |
| 0x20803083 NVLINK_GET_PLATFORM_INFO | NV_OK, zeros | The device has no NVLink. The host card also answers zeros |
| 0x90e70102, 0x90e70113 InfoROM BBX_GET_TIME_DATA / BBX_GET_LAST_FLUSH_TIME | 0x56 | The host card answers 0x56 (no BBX). The device has no InfoROM object on the host |
| 0x208f1105 NV208F GPU_VERIFY_INFOROM (subdevice diag class) | 0x56 | The diag class is not opened on the host, and GeForce boards have no InfoROM checksum to verify |

The rows that were already in `ctrl.rs` and that nvidia-smi also reaches keep their
labels. Examples are `GSP_GET_FEATURES` (firmware string "610.57.04") and
`BIOS_GET_INFO_V2` (the synthetic VBIOS version, 90.06.3D.00.76).

### Never forwarded: identity (decision D6)

| Field | Source |
|---|---|
| Product Name `Leandro RTX 2070-4G` | `GspStaticConfigInfo.gpuNameString` (profile) |
| FB total 4096 MiB, reserved/used/free | static info `fb_length` and the guest's own PMA. The FB_GET_INFO rewrite covers the forwarded case |
| GPU UUID `GPU-9bd7b93d-...` | static info `gidInfo` (profile) |
| GPU PDI | GET_INFO_V2 0x23/0x24 from the GID (above) |
| PCI ids, bus id, Board ID 0x5 | the emulated config space and the guest's bus numbering |
| VBIOS version | the synthetic VBIOS |

## After the fix

Guest `nvidia-smi` (final run, 2026-09-23):

```
|   0  Leandro RTX 2070-4G            Off |   00000000:00:05.0 Off |                  N/A |
| 30%   50C    P8             29W /  175W |       0MiB /   4096MiB |     19%      Default |
|                                         |                        |                  N/A |
```

Guest `nvidia-smi -q` excerpt:

```
    Product Name                                       : Leandro RTX 2070-4G
    GPU UUID                                           : GPU-9bd7b93d-3d23-c074-6704-984ef33ab7a7
    GPU PDI                                            : 0x74c0233d3db9d79b
    Fan Speed                                          : 32 %
    Performance State                                  : P8
    FB Memory Usage
        Total                                          : 4096 MiB
        Reserved                                       : 311 MiB
        Used                                           : 0 MiB
        Free                                           : 3786 MiB
    Utilization
        GPU                                            : 18 %
        Memory                                         : 24 %
    Temperature
        GPU Current Temp                               : 50 C
        GPU Shutdown Temp                              : 94 C
        GPU Slowdown Temp                              : 91 C
    GPU Power Readings
        Instantaneous Power Draw                       : 29.46 W
            Current Power Limit                        : 175.00 W
    Clocks
        Graphics                                       : 420 MHz
        Memory                                         : 405 MHz
    Max Clocks
        Graphics                                       : 2100 MHz
        Memory                                         : 7001 MHz
```

Before the change, the same `nvidia-smi -q` had 147 N/A lines and 24 unique
unanswered controls. After it, there are 81 N/A lines. At the same time the host's
own `nvidia-smi -q` had 80, plus its process list.

<a id="left-na"></a>
## Left N/A, and other differences from the host

Field by field, the guest and the host `nvidia-smi -q` differ in N/A-ness in one field:
- **PCIe Generation, Host Max: N/A** (the host shows 4). This is the upstream port's
  capability, which the guest reads from its own PCI topology (the Cloud Hypervisor
  root complex). No RM control is involved, so the device cannot answer it.

These differences are not N/A, and they are correct for a VM:
- **Virtualization Mode: Pass-Through** (host: None). The guest kernel serves
  `GPU_GET_VIRTUALIZATION_MODE` itself from its hypervisor detection. Changing it
  would require lying at the static-info level (`bIsVgx`, `hypervisorType`). No reason
  to do that yet.
- **Display Attached/Active: No/Disabled.** The guest has no display.
- **Processes: None**, and used memory 0 MiB. The guest sees only its own clients.
- Every other N/A in the guest (serial and part number, ECC, MIG, C2C, fabric,
  memory temperature, average power, module power, and so on) is N/A on the host card too.

## The gate

`tools/caraxes-gate.sh nvml` follows the gate conventions in
[../TESTING.md](../TESTING.md): one JSON verdict on stdout, diagnostics on stderr, and
exit 0 pass, 1 fail, 2 skip. It skips when the launcher, the host `nvidia-smi`, the
guest (ssh) or the guest's `/opt/nvrm/bin/nvidia-smi` is missing. It neither builds
nor starts anything. The checks:

| Check | Pass condition |
|---|---|
| `name` | `-q` Product Name equals the name the device presents with the launcher's settings (`tools/caraxes-vm.sh name`, `Leandro RTX 2070-4G` by default; `CARAXES_GATE_NAME` overrides) |
| `smi-row` | The plain `nvidia-smi` table shows the name and `4096MiB` (`CARAXES_GATE_FB_MIB`) |
| `total` | FB total is 4096 MiB |
| `usage` | reserved + used + free equals total within 2 MiB (rounding), and used is at most total |
| `temp-number`, `pstate-number`, `power-number`, `util-number` | Numbers, not N/A |
| `temp-matches-host` | The guest temperature lies within the host's `nvidia-smi -q` readings before and after the guest run, widened by `CARAXES_GATE_TEMP_TOL` (4 degrees) |
| `pstate-family` | The guest pstate's group (P0-P2, P3-P7, P8 and up) equals the host's group before or after |
| `host-name-differs` | The host name differs from the guest name: the identity was not forwarded |

The host-side reading is the host's own `nvidia-smi -q`, a read-only NVML query
that any user can run. The gate takes it twice per run.

Verdict of the final run, 2026-09-23 (checks abbreviated):

```
{"gate":"nvml","verdict":"pass","reason":"identity device-owned, telemetry forwarded and consistent with the host",
 "guest":{"name":"Leandro RTX 2070-4G","total_mib":"4096","reserved_mib":"311","used_mib":"0","free_mib":"3786",
          "temp":"50 C","pstate":"P8","power":"29.67 W","util":"18 %","fan":"32 %"},
 "host":{"name":"NVIDIA GeForce RTX 2070","temp_before":"50","temp_after":"50","pstate_before":"P8","pstate_after":"P8","power":"29.78 W"},
 "checks":[... 11 checks, all ok ...]}
```

The gate was also checked against failure: with `CARAXES_GATE_FB_MIB=2048` it fails
(exit 1, `total` and `smi-row`), and with an unreachable guest it skips (exit 2).

## The minimal set (D9 cut-down), from the labels above

Everything forwarded today reports the **whole physical card**, not the VM. For
example, the guest's 18-19 % GPU utilization is the host desktop's load, and the
"SW Power Capping" counter (44670628216 us) is the host card's uptime history. A
per-VM isolation cut would keep these values:

| Keep as is | Why |
|---|---|
| Identity (name, FB size, UUID, PDI, PCI ids, VBIOS version) | Already device-owned |
| Temperature (THERMAL_SYSTEM_EXECUTE_V2 opcodes 0x100/0x101/0x1500 only) | Shared physical fact, no workload information at 1-degree resolution. Filter the opcodes rather than forward the instruction list |
| Performance state | Coarse. Needed for a meaningful Perf column |
| Static limits: shutdown/slowdown temperatures, power limits, max clocks, brand, PCIe link capability | Constant per board. Capture them once and serve them as constants, so the corresponding legacy controls stop being forwarded |
| NOT_SUPPORTED statuses (ECC, InfoROM, retired pages, remapped rows, DRAM encryption, BBX, diag, SMC) | Constants for a GeForce TU106 |

| Replace | With |
|---|---|
| GPU/memory utilization, encoder/decoder samples (PERF_GET_VID_ENG_PERFMON_SAMPLE, perf legacy) | Per-VM accounting (the VM's own channels), or 0 |
| Power draw, fan speed, current clocks | Either forward them as shared physical facts (an operator choice), or quantize them |
| PEX counters and throughput, clocks-event-reason counters | 0, or the VM's own counts. These reveal host activity |
| BINAPI 0x0108/0x0110/0x0111 | The captured constant replies (0x0108 is all zeros on the host; 0x0110/0x0111 return 0x56) |

This cut is open. The mapping from each unnamed GSS-legacy command to the
nvidia-smi field it feeds has not been measured. One experiment would give it:
answer one command at a time with NOT_SUPPORTED and diff `nvidia-smi -q`.

## Future: test the synthesized answers, then extend both tools

Requested by the operator on 2026-09-23. The six synthesized answers above (SMC mode,
NVLink platform info, the two InfoROM BBX queries, the InfoROM verification, and the
device-owned chip-unique indices of `GPU_GET_INFO_V2`) are not failures: they are the
answers the host card itself gives or values that must stay device-owned. They should
still be covered:

1. Add a `synthesized` check to `tools/caraxes-gate.sh nvml`: issue the same six controls
   on the host through `nvrm-client` and assert that the guest's synthesized status or value
   matches the host's answer (0x56 for the BBX and verification queries, zeros for NVLink,
   MIG unsupported), so a driver update that starts answering them is noticed.
2. Extend the tool coverage in both directions: the gate should run more of `nvidia-smi`'s
   sub-commands (`--query-gpu` with every field, `-q -d CLOCK,POWER,TEMPERATURE,PERFORMANCE`,
   `topo -m`, `nvlink -s`) and diff against the host, and the device's `nvml.rs` should
   answer whatever new controls that reveals, with the same labels.
3. When the minimal set (decision D9) is cut, keep these checks so the cut is measured.

