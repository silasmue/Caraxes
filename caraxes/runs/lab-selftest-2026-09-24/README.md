<!-- SPDX-License-Identifier: MIT -->
# Lab capture kit self-test, 2026-09-24

Self-test of `caraxes/tools/lab-capture.sh` (docs/caraxes/lab-blackwell.md) on the
development host: RTX 2070 (TU106, `0000:2d:00.0`), open kernel modules 610.57.04,
kernel 7.2.4, no sudo.

## Command

```sh
caraxes/tools/lab-capture.sh --no-sudo --rom -o caraxes/runs/lab-selftest-2026-09-24
```

`--no-sudo` makes the script call no sudo at all (not even `sudo -n true`). `--rom` was
given so that the ROM item shows how a sudo-only item is recorded.

## Result

- Exit status 0, wall time 81 s (of which about a minute is streaming the 930 MB kernel
  journal of this host through the filter; see below).
- `lab-capture-tu106-manacor-20260924-075835/MANIFEST.txt`: 33 items, 30 `ok`, 2 `reduced`, 1 `skipped`, 0 `failed`.
  - reduced: `lspci-vvv` (capabilities past 0x40 show `<access denied>` as user) and
    `config-space` (64 bytes instead of 4096).
  - skipped: `rom` (`--rom needs sudo (--no-sudo given)`).
- Tarball `lab-capture-tu106-manacor-20260924-075835.tar.gz` (84 KB, 53 files, sha256 in the `.sha256` beside it); the script
  unpacked it into a temporary directory and checked every entry of `SHA256SUMS`:
  "tarball verified: 53 files, all sums match". Directory size 896 KB.
- Part 2 ran completely: ABI verified (running driver 610.57.04), `lab-rm-query`,
  `fifo-host-query`, `host-facts facts` exit 0, `gsp-abi-sizes` against
  `vendor/open-gpu-kernel-modules`.
- Privacy check: no occurrence of the username or of an e-mail address in any file of
  this directory; serial numbers show `<redacted>`.
- `console.txt` is the console output of the run.

The ROM walk was tested on the existing captures instead (no sudo here):
`--walk-rom caraxes/reference/rtx2070/rom.bin` reports 2 images (x86 60416 B, EFI 69632 B),
PCI chain complete at 0x1fc00, VBIOS 90.06.3d.00.76, FALCON_DATA pointing into the absent
extension image, verdict "chain complete, FWSEC absent"; `--walk-rom
caraxes/reference/rtx2070/synth-vbios.bin` finds the third (0xE0) image and the falcon
ucode table at 0x1fd24 (application id 0x05).

## What was read on this host

Only reads: files under `/proc` (`cmdline`, `modules`, `uptime`, `driver/nvidia/*`),
`/sys/bus/pci/devices/0000:2d:00.0/` attribute files (never `config`, `rom`,
`resourceN`, `vpd`), `/sys/module/nvidia*/parameters`, `/sys/class/drm/card1-*`
(status, modes, cached EDID), `/lib/firmware/nvidia` (hashes), `lspci` as user,
`nvidia-smi -q`, `-q -x` and `--query-gpu`, the kernel journal, and RM getter controls
on a private RM client (MC_GET_ARCH_INFO, BUS_GET_PCI_INFO, GPU_GET_NAME_STRING,
GPU_GET_SHORT_NAME_STRING, GPU_GET_CLASSLIST and the controls of fifo-host-query and
host-facts `facts`, all as in their 2026-09-23 runs). No register access, no module
change, no sudo.

## Incidents and observations

- A first attempt stored the whole kernel journal in the output directory before
  filtering; the journal of this host is 930 MB (8.4 million lines) and the root file
  system had about 300 MB free, so the write hit "No space left on device" for a moment
  before the file was removed. The script now streams the journal through the filter and
  keeps at most 3000 + 2000 matching lines, 500 Xid lines and a frequency summary.
- The frequency summary of this host's journal shows, besides the known debug flood
  (`NVRM: ioctl(...)`, 4.1 million lines), 2.7 million
  `dmaAllocMapping_GM107: can't alloc VA space for mapping` and about 740 000
  `NV_ERR_NO_MEMORY` assertions from `reusemappingdbMap(&pBar1VaInfo->reuseDb, ...)`,
  i.e. BAR1 VA exhaustion reported by the host RM at some point in this boot. Not
  investigated here.
