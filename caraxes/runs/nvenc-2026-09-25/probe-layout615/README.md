<!-- SPDX-License-Identifier: MIT -->
# probe-layout615 and probe-layout615b: no result on vm2-615

- probe-layout615: `cuInit` returned 803 (`CUDA_ERROR_SYSTEM_DRIVER_MISMATCH`).
  - `run.sh` set `LD_LIBRARY_PATH` to the 610 userspace in `/opt/nvrm/lib`, while the guest
    kernel ran 615.
  - Fixed: `run.sh` now takes the selected driver's library directory (tools/lib/driver.sh).
- probe-layout615b: `libnvidia-encode.so.1 not loaded`.
  - The vm2-615 base image has libcuda 615.71.09 but no `libnvidia-encode` and no
    `libnvcuvid` (docs/caraxes/cutover-615.md: the image lacks libnvidia-encode-615).
  - The game VM vm4-615 installs them (`sottr-prepare.sh`), so the 615 NVENC check moves
    there.
- Neither run had a host NVRM line or an Xid.
