<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# The lab kit: capture and validation on a rented cloud GPU or the workplace card

[e-families.md](e-families.md) plans the feature families from source. The kit is how the
source work is checked against real cards the operator has no hardware for.

**What the kit is.** One pack a person runs by hand, unattended, on a machine with an
NVIDIA GPU:
- a rented cloud GPU VM, or
- the workplace Blackwell card (over the next days; no agent there).

It leaves one redacted tarball to carry home. The agent never runs it and never touches the
network. The operator rents the machine, runs the kit, and brings the tarball back.

**What goes in and what comes out.**
- The pack is made at home by `caraxes/tools/lab-kit.sh pack`.
- It holds:
  - the kit script;
  - `caraxes/tools/lab-capture.sh`, the capture of the 5060 Ti
    (`caraxes/reference/gb206/`);
  - the RM query tools, statically linked;
  - the sweep binary and its control list;
  - the LD_PRELOAD tracer of the native traces;
  - the CUDA, OpenCL and Vulkan probes;
  - their sums.
- The lab machine needs a stock Linux with the NVIDIA driver, bash, python3, pciutils and
  `nvidia-smi`. Nothing else is installed there.

Sections 1 to 5 are for the operator. Sections 6 to 9 say what the tarball feeds at home,
what a capture cannot give, and which cloud part covers which chip.

## 1. Before leaving home: build the pack

```sh
cd ~/git/Caraxes
caraxes/tools/lab-kit.sh pack --ogkm /mnt/vmstore/caraxes/ogkm-615.71.09
# -> target/lab-kit/caraxes-lab-kit-615.71.09.tar.gz and its .sha256
```

**The tree.** `--ogkm` names the open-gpu-kernel-modules tree of the driver the lab
machine runs. For 615.71.09, the Leandro vendor tree works too
(`/home/silas/git/Leandro/vendor/open-gpu-kernel-modules`, `version.mk:1`).

**What `pack` builds** (`caraxes/tools/lab-kit.sh`, function `do_pack`):
- **The sweep list.** `caraxes/tools/sweep-list.py`: every getter of categories A, B and
  E, plus the six static-info getters a new chip's boot model needs, with the parameter size
  measured by the compiler against the tree's SDK headers. 164 controls on 615.71.09.
- **The four rustc-only tools.** `lab-rm-query`, `fifo-host-query`, `host-facts` and
  `lab-sweep`, built with `-C target-feature=+crt-static`, so they need no glibc on the lab
  machine.
- **The tracer.** `libnvrm_trace.so` from Leandro's `crates/nvrm-trace`, with the ABI
  feature of the driver's major version (default `v615`,
  `/home/silas/git/Leandro/crates/nvrm-trace/Cargo.toml:22`). The build is
  `cargo build --offline`, so it needs the crates cached and fetches nothing.
- **The probes.** `nvprobe`, `managedprobe`, `hostregprobe`, `oclprobe` and `vkalloc` from
  Leandro-Test's `probe/c`, compiled with `-std=gnu11`. That keeps their glibc need at 2.34
  (the C23 symbol versions of a default GCC 15 build need 2.38). Plus the checked-in
  `kernels.ptx`: sm_75, which the driver JIT-compiles forward on every newer part.
- **The pack's own files.** `REQUIRES.txt`, the glibc each dynamic file needs (tracer and
  probes 2.34, measured 2026-09-24), `KIT.txt` and `SHA256SUMS`.

**The pack of 2026-09-24.** The dry run on the development host took 10 s: 6.9 MB, 164
sweep rows, all probes built. glibc 2.34 means Ubuntu 22.04 or later, Debian 12, RHEL 9. On an
older lab machine, the kit's preflight sets the tracer and the probes aside, and the other
steps still run.

Copy the pack tarball and its `.sha256` to the lab machine: USB stick, `scp` from home, or
the cloud provider's upload. Nothing else is needed there.

## 2. On the lab machine: the commands, in order

```sh
# 0. unpack and check the pack
sha256sum -c caraxes-lab-kit-615.71.09.tar.gz.sha256
tar -xzf caraxes-lab-kit-615.71.09.tar.gz
cd caraxes-lab-kit-615.71.09

# 1. note the driver (the kit records it too; 615.71.09 is the version the pack was made for)
cat /proc/driver/nvidia/version
nvidia-smi -L

# 2a. the run, as a normal user (everything read-only, no sudo)
./lab-kit.sh run ~/lab-kit-out

# 2b. or with the three sudo items (see section 3); cache sudo first, the kit never asks
sudo -v
./lab-kit.sh run ~/lab-kit-out --rom --priv

# 3. check the result (section 4), then carry home the two files it names:
#    ~/lab-kit-out/lab-kit-<chip>-<stamp>.tar.gz and its .sha256
./lab-kit.sh verify ~/lab-kit-out/lab-kit-*.tar.gz
```

**Machines with more than one GPU.**
- `--device N` picks the RM device instance for the sweep and the traces. It follows
  the order of `nvidia-smi -L`.
- `--bdf 0000:xx:00.0` limits the capture to one card.
- On an HGX node, run the kit once per GPU, with a different OUTDIR each time.

**Other options** (the usage header of the script):
- `--no-traces` skips the probes.
- `--prom` reads the VBIOS through the BAR0 PROM window. That needs sudo, and a BAR no
  driver holds; `docs/caraxes/lab-blackwell.md` §3 has the details.
- `--keep-edid` keeps the monitor serial numbers in the EDIDs.

**Progress.** The kit prints one line per step, `[n/9] step ... status (seconds)`, and
writes everything to `run.log` inside the result.

## 3. What needs root, and why (keep it to these)

| Item | Without root | With root | Why root |
|---|---|---|---|
| PCI config space and `lspci -vvv` capabilities | first 64 bytes, capabilities `<access denied>` | all 4096 bytes | the kernel shows unprivileged readers only the standard header (`docs/caraxes/lab-blackwell.md` §3) |
| `--rom`: the PCI option ROM | skipped | `rom.bin` and the image chain walk | the sysfs `rom` file is root-only. The kernel sets the ROM BAR enable bit for the read and clears it after. This is the one write of the kit. |
| `--priv`: the privileged getters of the sweep | recorded as NV_ERR_INSUFFICIENT_PERMISSIONS | answered | 37 of the 164 rows of the 615 list are `PRIVILEGED` in the export table, among them `DP_GET_CAPS`, which the DP 2.x plan needs. The pass issues only those rows, only getters (`caraxes/tools/lab-sweep.rs`, `--priv-only`). |
| kernel log | read if the distribution allows `dmesg` to users | `sudo dmesg` | some distributions restrict `dmesg` |

Everything else runs as the normal user.

**What the kit never does.** It never loads, unloads or configures a module, never resets
the card, never writes a register, never sets a MIG, ECC, clock or power mode, and never
calls the network. `docs/caraxes/lab-blackwell.md` §6 lists the forbidden actions. They hold
for the kit on both kinds of machine.

## 4. What each step does and how long it takes

| Step | What | Output | Time |
|---|---|---|---|
| 1 preflight | Checks the pack's sums, the tools, the running driver against the pack's, and glibc against `REQUIRES.txt` (a binary that needs a newer glibc is set aside). Collects the identifiers the scrub replaces. | `preflight.txt` (the identifier list stays private and is deleted) | 5 s |
| 2 capture | `lab-capture.sh --no-rm-tools`: host facts, `lspci`, config space, BARs, sysfs, `/proc/driver/nvidia`, `nvidia-smi -q` (text and XML) and `--query-gpu`, firmware files with sums, DRM connectors and EDIDs, options `--rom` and `--prom` | `capture/` | 30 s to 2 min |
| 3 rm-tools | `lab-rm-query` (architecture, implementation, PCI ids, names, class list), `fifo-host-query` (engines, fault ids, PCE masks, CE caps), `host-facts facts` (BAR and FB info, FB regions, GR masks, interrupt table) | `rm-tools/` | 5 s |
| 4 sweep | `lab-sweep`: 164 getters of categories A, B and E, plus the boot getters, each once on the card's own root client, device, subdevice, display common, ZBC, InfoROM and diag objects. The FB, BUS and GPU info lists go one index at a time; the 0073 controls with a display id go once per display of `SYSTEM_GET_SUPPORTED`. | `sweep/sweep-user.txt` (`sweep-priv.txt` with `--priv`) | 5 to 30 s |
| 5 traces | Under the tracer (`LD_PRELOAD`, `LEA_TRACE_FILE`, the way `scripts/ioctl-matrix.sh:317` of Leandro-Test does it): `nvprobe 2`, `3` and `4` (context and memory, PTX JIT, launches), `managedprobe`, `hostregprobe`, `nvidia-smi -q`, `oclprobe`, `vkalloc` | `traces/<probe>.jsonl`, `.tsv`, `.out`, `summary.txt` | 1 to 5 min |
| 6 display | DRM connectors with status and modes, `display_mode` and `display_active`, the 0073 rows of the sweep | `display/display.txt` | 2 s |
| 7 extras | `nvidia-smi mig -lgip`, `-lgipp`, `-lcip` (answers only with MIG mode on), `nvlink --status` and `--capabilities`, `topo -m`, and the ELF section table of the running driver's `gsp_*.bin` (read-only on the file; the boot model's open appVersion, `docs/caraxes/gsp-boot-blackwell.md` §6 item 3) | `extras/` | 10 s |
| 8 scrub | Replaces the identifiers (section 5) in every text file, zeroes the EDID serial fields, then checks every file, binary ones too, for every collected value. It refuses to pack when one is left. | `run.log` line | 5 s |
| 9 pack | Writes `MANIFEST.txt` and `SHA256SUMS`, the tarball, its `.sha256`, and unpacks the tarball again to verify it | `lab-kit-<chip>-<stamp>.tar.gz` | 5 s |

**Total.** Under 10 minutes per GPU on a machine with the driver up. A rented VM needs about
15 minutes more if 615.71.09 has to be installed first (section 7). The target of one hour
per chip leaves room for a second run with `--priv`, and for the operator-only MIG step of
section 7.

**The sweep's answers.**
- Per row: status and the bytes after the call, with trailing zeros cut.
- Rows whose answer can carry an identifier print only the count of non-zero bytes. That
  covers names with GID, UUID, serial, hostname, GUID, PIDs, process, accounting, OEM, ECID,
  FRU, chassis, board info, module id or platform info (`REDACT`, `caraxes/tools/lab-sweep.rs`).
- **Driver mismatch.** A size that does not match the running driver is refused by the RM
  before anything is read. On a driver other than the pack's, some rows are lost, but no
  answer is wrong (`docs/caraxes/lab-blackwell.md` §5, the 615 check of `control.c:446-461`).

## 5. Before carrying the tarball away: the checklist

1. **The console.** The last lines of the run say `verified: N files, all sums match`,
   and name the tarball and its sha256. `./lab-kit.sh verify <tarball>` prints the same
   and the manifest.
2. **The manifest.** `MANIFEST.txt` has one line per step.
   - `ok` or `partial` is fine.
   - `skipped` is fine when the note says why: no display head, traces off, no sudo.
   - `failed` on `capture` or `sweep` is worth a second run. Bring both runs home.
3. **The scrub.** The `scrub` step reads `ok`. On `RESIDUAL` the kit wrote no tarball, and
   `run.log` names the file and the kind of identifier (never the value). The work directory
   is private (mode 700). Leave it on the machine, or delete it. Tell the agent at home which
   kind was left, so the scrub learns it.
4. **The redaction checklist**, done by the kit, for the operator to spot-check with
   `grep -r` in the unpacked result:

   | Identifier | Where it would be | What the kit does |
   |---|---|---|
   | host name (short, FQDN) | `MANIFEST.txt` of the capture, `run.log`, `nvidia-smi -q`, `hostname` field of `--query-gpu`, trace dumps | `<host1>`; its ASCII hex in dumps `x...x` |
   | user name, home directory | paths in logs and traces | `<user1>`, `<home1>`; generic cloud accounts (`ubuntu`, `ec2-user`, `azureuser`, ...) are kept, they identify nobody |
   | e-mail addresses | anywhere | `<email>` |
   | GPU UUID | `nvidia-smi -q`, `/proc/driver/nvidia/gpus/*/information`, `--query-gpu`, the trace dumps of GID and UUID controls | `<gpuuuid1>`; its 16 raw bytes and its ASCII in hex dumps `x...x` |
   | board, chassis, product serial | `nvidia-smi` text, XML and CSV, DMI | `<redacted>` (already `lab-capture.sh:618-622`, `:650`) and the collected values |
   | config-space PBI mailbox and DSN | `config-space.txt`, `lspci-vvv.txt` | `xx` (`lab-capture.sh:543`, `:561`) |
   | MAC addresses | `lspci -vvv` of other functions, sysfs, logs | the collected values and any MAC shape: `<mac>` |
   | IP addresses of the machine | logs, `cmdline` | the collected values: `<ip1>` |
   | machine id, file-system UUIDs, DMI product UUID | `cmdline` (`root=UUID=`), logs | the collected values and any UUID shape: `<uuid>` |
   | process names and PIDs | `nvidia-smi -q` Processes, the sweep's accounting and PID rows | `<redacted>`; the sweep prints counts only |
   | monitor serial numbers | EDIDs of attached monitors | the serial field and the serial string descriptor zeroed, the checksum fixed, the dump redone (`--keep-edid` keeps them) |
   | NVLink node host names, fabric GUIDs | the sweep, `nvidia-smi -q` Fabric | the sweep prints counts only; GUID lines `<redacted>` |

5. **The cloud VM's own data.** On a rented VM, the tarball holds no provider account data:
   the kit reads no metadata service and calls no network tool. Instance names appear only as
   the host name, which is replaced.

## 6. At home: what the tarball feeds

```sh
caraxes/tools/lab-kit.sh verify lab-kit-<chip>-<stamp>.tar.gz
python3 caraxes/tools/lab-ingest.py lab-kit-<chip>-<stamp>.tar.gz              # the report only
python3 caraxes/tools/lab-ingest.py lab-kit-<chip>-<stamp>.tar.gz --file       # and file it
```

**What `lab-ingest.py` does** (`caraxes/tools/lab-ingest.py`):
1. Checks the sums.
2. Repeats a generic identifier check. It refuses to file on a hit.
3. Compares the sweep with the chip file:
   - For every feature family: the card's NV_OK and NOT_SUPPORTED counts against
     `chips/<chip>/chip.toml` `[features]`. A family named absent whose getters the card
     answers NV_OK is marked `CHECK`: the feature is there, or it is an `empty_ok` case. A
     family named present that the card refuses is marked too.
   - It lists every category A getter the card answers NV_OK, with its bytes: the candidates
     for `chips/<chip>/controls.toml`.
4. With `--file`, copies the result:
   - the result to `caraxes/reference/<chip>/lab-kit-<stamp>/`;
   - the traces to `matrix/traces/<driver>/<chip>/`, next to the RTX 2070's
     `matrix/traces/610.57.04/` and like them outside git.

A dry run on a synthetic result tarball, with fake statuses and no card, checked the report
and the sum checks (2026-09-24).

**Where each part goes:**

| Part of the result | Feeds | How |
|---|---|---|
| `capture/` | `chips/<chip>/chip.toml` identity, `boards/<board>.toml`, the config space and ROM of the synthetic identity | by hand, as for GB206 (`docs/caraxes/blackwell-gb206.md` §2.1) |
| `rm-tools/` | device-info, engines, PCE masks, interrupt table (`device-info.toml`, `interrupts.toml`) | by hand, as for GB206 |
| `sweep/` | `[features]` of the chip file, `controls.toml` rows (category A), the `empty_ok` list, the B forward table's expectations, and the truth of the families of e-families.md | `lab-ingest.py` report. Turning bytes into named fields needs the layouts of `caraxes/tools/ctrl-layouts.py`; `lab-ingest.py` names this as its next step. |
| `traces/` | the guest-against-native comparison of Leandro-Test's `ioctl-matrix.sh guest` and `verify` | filed under `matrix/traces/<driver>/<chip>/`; the catalog step of `ioctl-matrix.sh` reads that layout |
| `display/`, the 0073 sweep rows | the display model's connector table, `DP_GET_CAPS` for `dp2x` | by hand |
| `extras/` | the MIG profile tables (`chips/<chip>/mig.toml`, e-families.md §3.8), the NVLink tier-0 check, the GSP firmware appVersion | by hand |

## 7. Operator-only steps on a rented VM (not the kit)

A rented VM is the operator's own machine for its lifetime. Some things the kit forbids on
the workplace card are acceptable there. They are the operator's decisions, done by hand,
never by the kit.

**Driver 615.71.09.** Cloud images ship their own driver branch, and the pack was made for
615.71.09. If the image has another version, the operator installs 615.71.09 from NVIDIA's
`.run` file (open kernel modules) before the run, or builds a pack for the image's version
at home (`--ogkm` of that tag; section 1). The kit only records which driver it found.

**MIG profiles** (A100, H100, H200, B200, RTX PRO 6000 Server Edition):

```sh
sudo nvidia-smi -i 0 -mig 1       # needs a GPU reset; on some clouds a VM restart
./lab-kit.sh run ~/lab-kit-out-mig --device 0
```

The extras step then records `-lgip`, `-lgipp` and `-lcip`. Afterwards,
`sudo nvidia-smi -i 0 -mig 0`.

**An instrumented kernel module** (section 8). Only on a rented VM, only if the operator
wants the GSP RPC stream.

## 8. What a capture cannot give

**GSP-RM's internal state.** The WPR layout inside the firmware, the heap, the scheduler,
the PMU and FSP state, the fuses the firmware reads. The kit sees what the RM exports:
controls, `nvidia-smi`, `/proc`, sysfs. Firmware state shows up only through those answers.

**The GSP RPC traffic on the card.** Two reasons:
- **The tracer sees only the user-space edge.** On a passthrough card, or a bare-metal
  one, the stock kernel RM talks to GSP-RM over message queues in memory it shares with the
  firmware. The kernel sets them up and polls them. No ioctl and no file descriptor carries
  them. The LD_PRELOAD tracer sees what user space asks the kernel RM (`NV_ESC_RM_CONTROL`
  and friends; `/home/silas/git/Leandro/crates/nvrm-trace/src/lib.rs:4`). For a routed
  control, the answer in that trace is GSP-RM's answer after the kernel's processing, which
  is what the sweep and the traces are for.
- **The INTERNAL controls and the boot RPCs cannot be reached from user space.** These are
  what the kernel itself sends: `GSP_SET_SYSTEM_INFO`, `GET_GSP_STATIC_INFO`, the INTERNAL
  controls of every family, the event stream. The RM refuses an `RMCTRL_FLAGS_INTERNAL`
  control from a user client, so the sweep leaves them out (`caraxes/tools/sweep-list.py`,
  rule 2).

**What the 615 tree's registry keys can add**, set with `NVreg_RegistryDwords`
(`kernel-open/nvidia/nv-reg.h:218-221`), or as module parameters for the first two:
- `ResmanDebugLevel` (`kernel-open/nvidia/nv-reg.h:163-168`, read at `src/nvidia/arch/nvalloc/unix/src/registry.c:221`)
  and `RmMsg` (`nv-reg.h:278`, `src/nvidia/interface/nvrm_registry.h:573`). These open
  the kernel RM's own `NV_PRINTF` levels and filters. What shows up is the kernel's view:
  which RPC it sends and which status comes back, where the code prints it. Payloads do not
  show up. It is also the flood the host saw with `ResmanDebugLevel=0` (operator memory
  "Host dmesg flood cause").
- The kernel keeps a 128-entry history of RPCs (`RPC_HISTORY_DEPTH`,
  `src/nvidia/inc/kernel/gpu/rpc/objrpc.h:58`; filled at `gpu/gsp/kernel_gsp.c:421-519`).
  The history is printed only when an RPC times out or fails (`kgspLogRpcDebugInfo`,
  `gpu/gsp/kernel_gsp.c:2305-2347`, called at `:2675`, `:2912`, `:2959`). The last 8 entries
  also go into the Xid report (`kgspLogRpcDebugInfoToProtobuf`, `:2204-2252`). No key turns
  it into a stream.
- `RmGspRmProfiling` (`nvrm_registry.h:2497-2499`) turns on GSP-RM's profiling. The result
  stays in the firmware's buffers.
- GSP-RM's own LIBOS log buffers are decoded by the kernel when it dumps them, which it does
  on error paths (`kgspDumpGspLogs`, `gpu/gsp/kernel_gsp.c:2066`, `:3667`).

None of these gives the RPC payloads. The only full record would be a kernel module built
from the open 615 tree with a print or a ring buffer in the message queue send and receive
path (`src/nvidia/src/kernel/gpu/gsp/message_queue_cpu.c`). That is a driver change: never
on the workplace card, and not part of the kit. On a rented VM it is possible as a separate,
opt-in step, if the operator decides the RPC stream of an H100 or B200 boot is worth it. A
patch and a decoder for it would be their own work item (about 3 days). The Caraxes device
logs the same stream for the synthetic chips already, so the comparison would be direct.

**Display on data-center parts.** GA100, GH100 and GB100 have no display engine
(e-families.md §2). A cloud VM with such a part has no head, and `frl`, `dp2x` and
`display_later` can only be validated on the workplace Blackwell card (the home rig has only the TU106, which has none of these families).

## 9. Which cloud GPU covers which chip

| Chip | Cloud part | What it validates (e-families.md) |
|---|---|---|
| TU104 | Tesla T4 | the closest Turing sibling of TU106: same generation, but the NVLink HAL property is on for TU104 and off for TU106 (`g_kernel_nvlink_nvoc.c:241-243`). The TU106 answers of chipctl stay the reference; the T4 shows what a Turing chip with the NVLink object answers. |
| GA102 | A10G, A10 | ampere_later, ga10x_pcie_ro, nvlink tier 0, APM static info (`cc`), ECC on a GDDR pro board |
| GA100 | A100 | mig, ecc and row_remap on HBM, nvlink tier 0 (PCIe) or tier 1 (SXM, HGX), hshub, fabric |
| AD104 | L4 | the cheapest start; nvlink tier 0, ECC, frl only as the HAL (no head) |
| AD102 | L40S | as L4, bigger |
| GH100 | H100, H200 | Hopper CUDA; cc static info and ccu, spdm refused, mig, nvlink tier 0 or 1, hshub, fabric; c2c on GH200 |
| GB100 | B200, GB200 | Blackwell data-center CUDA; ccu, blackwell (HES, DRAM encryption), mig, nvlink and NVLE; c2c on GB200 |
| GB202 | RTX PRO 6000 Blackwell Server Edition (`README.md:977`), where a provider offers it | the closest GB20x in a cloud: same display-less server board, the GB20x HAL with MIG (GB202 is in `PDB_PROP_GPU_MIG_SUPPORTED`, GB206 is not) |
| GB206 | none; the workplace card (the source of the GB206 capture, [blackwell-gb206.md](../blackwell-gb206.md)); there is no Blackwell card at home | everything GB20x-consumer, the display families |

**Consumer RTX boards** (the 2070, the 5060 Ti) are not offered by the usual GPU clouds. The
operator's list of rentable parts is the data-center parts above. For the GB20x consumer
view, the workplace card is the source (there is no Blackwell card at home; the GB206 capture came from the workplace card, [blackwell-gb206.md](../blackwell-gb206.md)). The T4 is the closest Turing part
a cloud offers.

**Verifying the chip.** The chip column is the operator's mapping. The kit checks it on the
instance with `MC_GET_ARCH_INFO` in `rm-tools/lab-rm-query.txt`, as the GB206 capture did
(`caraxes/reference/gb206/rm-tools/lab-rm-query.txt:3`). The device ids come from the 615
tree's `README.md` (e-families.md §2).

**A useful order of rentals**, following e-families.md §6:
1. L4: the cheapest; it proves the kit and gives the AD10x tables.
2. H100: Hopper CUDA and the tier-0 families.
3. B200: Blackwell data-center.
4. A100 with MIG on: the MIG tables.
5. An HGX node: the multi-GPU tier, only when that tier starts.

**Without any cloud.**
- The workplace Blackwell card gives the GB20x consumer view with display.
- There is no Blackwell card at home (corrected 2026-09-25: an earlier version named a home
  5060 Ti; the GB206 capture came from the workplace card, [blackwell-gb206.md](../blackwell-gb206.md)).
- The RTX 2070 stays the TU106 reference. Its native traces are in `matrix/traces/610.57.04/`.
