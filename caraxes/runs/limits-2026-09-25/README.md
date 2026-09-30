<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
# Limits runs, 2026-09-25

This directory holds the evidence for [docs/vfio/limits-mig-ecc-vgpu.md](../../../docs/vfio/limits-mig-ecc-vgpu.md).
The runs are pure emulation on vm8: no host RM, no reservation, no GPU, no network device.

## Setup

- `prepare-vm8.sh` builds vm8 from vm7's overlay (the caraxes.conf registry keys come with
  it). It also installs `limits-check.sh` as the guest's boot check and `cuda-probe.c`.
- `run.sh <record dir> <scenario> [NAME=value ...]` does one boot:
  1. It uploads `scenarios/<scenario>.sh` into the guest.
  2. It boots vm8 with `CARAXES_TAP=none CARAXES_BOOT_MODEL=1 CARAXES_PERSIST=1`.
  3. It keeps `serial.txt`, `device-log.txt`, `trace-notable.txt` (refusals and errors) and
     `trace-family.txt` (family lines).
  4. `setup.txt` records the persistent record (`gpu-persist.toml`) before and after the boot.

  `FRESH=1` starts from a card as shipped. Rung: `CARAXES_COSTUME=h100|a100|rtx2070vf`.

## Directories

| Directory | Contents |
|---|---|
| `before/` | the survey before the models |
| `ecc/` | ECC runs |
| `mig/` | MIG runs, in order |

The doc's section 5 lists every run and its outcome.
