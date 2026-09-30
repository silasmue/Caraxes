<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Contributing a capture

A capture of a card the maintainers do not own is how VFIO Leandro gains a chip, a board or
a feature family. The lab kit is a self-contained pack a person runs by hand on a machine
with an NVIDIA GPU; it reads, redacts, and leaves one tarball to send. It never loads a
module, resets the card, writes a register, changes a mode or calls the network.

## What you need

- A Linux machine with the NVIDIA driver up, bash, python3, pciutils and `nvidia-smi`;
  glibc 2.34 or later for the traced probes (Ubuntu 22.04, Debian 12, RHEL 9 or later),
  otherwise the preflight sets the probes aside and the other steps run.
- The pack for your driver version: `caraxes-lab-kit-<driver>.tar.gz` and its `.sha256`,
  built by the maintainers with `caraxes/tools/lab-kit.sh pack --ogkm <open-gpu-kernel-modules
  tree of that version>`. Ask for the pack of the version `cat /proc/driver/nvidia/version`
  prints; a pack of another version loses some sweep rows and gives no wrong answer, because
  the RM refuses a parameter size that does not match.

## Run

```sh
sha256sum -c caraxes-lab-kit-615.71.09.tar.gz.sha256
tar -xzf caraxes-lab-kit-615.71.09.tar.gz
cd caraxes-lab-kit-615.71.09
cat /proc/driver/nvidia/version; nvidia-smi -L

./lab-kit.sh run ~/lab-kit-out                 # as a normal user, everything read-only
sudo -v; ./lab-kit.sh run ~/lab-kit-out --rom --priv   # or with the root items below
./lab-kit.sh verify ~/lab-kit-out/lab-kit-*.tar.gz
```

- More than one GPU: `--device N` (the order of `nvidia-smi -L`), `--bdf 0000:xx:00.0`; one
  run per GPU with its own output directory.
- `--no-traces` skips the probes; `--keep-edid` keeps monitor serial numbers.
- The kit prints one line per step, `[n/9] step ... status (seconds)`, and writes `run.log`.

## What needs root, and why

| Item | Without root | With root |
|---|---|---|
| PCI configuration space, `lspci -vvv` capabilities | the first 64 bytes | all 4096 bytes |
| `--rom`: the PCI option ROM | skipped | `rom.bin` and the image chain (the kernel sets the ROM enable bit for the read, the kit's one write) |
| `--priv`: the 37 privileged getters of the sweep, among them `DP_GET_CAPS` | recorded as `NV_ERR_INSUFFICIENT_PERMISSIONS` | answered |
| the kernel log | if the distribution allows `dmesg` | `sudo dmesg` |

Everything else runs as the normal user.

## The nine steps and their runtime

| Step | What | Time |
|---|---|---|
| 1 preflight | sums, tools, the running driver against the pack's, glibc; collects the identifiers to redact | 5 s |
| 2 capture | host facts, `lspci`, configuration space, BARs, sysfs, `/proc/driver/nvidia`, `nvidia-smi -q` (text, XML, CSV), firmware files with sums, DRM connectors and EDIDs | 30 s to 2 min |
| 3 rm-tools | architecture, ids, names, class list, engines, fault ids, PCE masks, BAR and FB info, GR masks, the interrupt table | 5 s |
| 4 sweep | 164 getters of categories A, B and E plus the boot getters, once each on the card's own objects | 5 to 30 s |
| 5 traces | CUDA, managed-memory, pinned-memory, OpenCL and Vulkan probes and `nvidia-smi -q` under the `LD_PRELOAD` tracer | 1 to 5 min |
| 6 display | connectors, modes, the display rows of the sweep | 2 s |
| 7 extras | MIG profiles (with MIG on), NVLink status, topology, the section table of the driver's `gsp_*.bin` | 10 s |
| 8 scrub | redaction and the residual check | 5 s |
| 9 pack | manifest, sums, the tarball, unpacked again to verify | 5 s |

Under 10 minutes per GPU with the driver up.

## Redaction

- The scrub replaces host names, user names and home directories, e-mail addresses, the GPU
  UUID, board and chassis serials, the PBI mailbox and DSN bytes of the configuration space,
  MAC and IP addresses, machine and file-system UUIDs, process names and PIDs, monitor serial
  numbers and fabric GUIDs, in text and binary files, and refuses to pack when one collected
  value remains (`RESIDUAL` in `run.log`, the kind named, never the value).
- Sweep rows whose answer could carry an identifier print only their count of non-zero bytes.
- On a rented VM the tarball holds no provider data: no metadata service is read.
- Spot-check with `grep -r` in the unpacked result before sending.

## Before sending

1. The last lines say `verified: N files, all sums match` and name the tarball and its sha256.
2. `MANIFEST.txt` has one line per step: `ok` or `partial` is fine; `skipped` is fine with a
   reason (no display head, traces off, no sudo); `failed` on `capture` or `sweep` is worth a
   second run, and both runs are welcome.
3. The `scrub` line reads `ok`.

Send the tarball and its checksum to the maintainers. Say which driver, which machine kind
(bare metal, cloud instance, workstation) and whether MIG or ECC was on; nothing else about
the machine is needed.

## What happens at home

- `lab-kit.sh verify` checks the sums; `caraxes/tools/lab-ingest.py` repeats an identifier
  check (it refuses to file on a hit), compares the sweep with the chip file's feature
  families (a family named absent that the card answers `NV_OK` is marked `CHECK`) and lists
  every category-A getter the card answers, with its bytes.
- `--file` copies the result to `caraxes/reference/<chip>/lab-kit-<stamp>/` and the traces
  to `matrix/traces/<driver>/<chip>/`.
- The parts feed, by hand: `chips/<chip>/chip.toml` and `boards/<board>.toml` from the
  capture; `device-info.toml` and `interrupts.toml` from the RM tools; `[features]`,
  `controls.toml` rows and the `empty_ok` list from the sweep; the guest-against-native
  comparison from the traces; the display model from the display rows; MIG tables and the
  GSP firmware version from the extras.
- The card gets a row in [captured-cards](captured-cards.md).

## What a capture cannot give

- GSP-RM's internal state and the GSP RPC stream: the tracer sees only the user-space edge,
  and the internal controls and boot RPCs never leave the kernel. Only an instrumented kernel
  module on a rented machine could record them; that is a separate, opt-in work item.
- Display families on data-center parts without a display engine.

## Depth

- [caraxes/gsp-plan/lab-kit.md](../vfio/gsp-plan/lab-kit.md): the kit in full, the
  operator-only steps on a rented VM, which cloud part covers which chip.
- [caraxes/lab-blackwell.md](../vfio/lab-blackwell.md): the checklist of the first capture.
