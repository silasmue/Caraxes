<!-- SPDX-License-Identifier: MIT -->
# PROM window of the host RTX 2070 (2026-09-25)

`prom-window-2026-09-25.bin`: 1048576 bytes, BAR0 0x300000-0x3fffff (`NV_PROM_DATA`), little-endian dwords.
sha256 `759b8ea7f8404f00a9d3a53e4d5db8c5e9b37a314850963c7c9fa1bbacd5f4c2`.

The dump is the card's firmware and is not in the repository (`.gitignore`). Read your own card's window as described under How and put it at this path: `run44.py` passes the chain from 0x28600 (`prom-window-2026-09-25-from-55aa.bin`) to the device, and six unit tests of `vfio-user-nvgpu` read the window and do not run without it.

## How

- Host card TU106, 0000:2d:00.0, driver 615.71.09; read-only, as root, with the user's go-ahead.
- `caraxes/tools/hostreg-read.c`: RM `NV2080_CTRL_CMD_GPU_EXEC_REG_OPS`, `READ_32`, type GLOBAL, 64 ops per call (as the register capture `bar0-registers-2026-09-25.txt`).
- Reads (end exclusive): 0x300000-0x300010 (probe); 0x300000-0x380000 (0.84 s); 0x380000-0x380010, 0x3c0000-0x3c0010, 0x3ffff0-0x400000 (probe: data, so the window continues); 0x380000-0x400000 (0.78 s); 0x300000-0x380000 again (identical).
- 262144 + 131072 + 28 dword reads, no RM error, no refused offset.
- Sysfs ROM (`/sys/bus/pci/devices/0000:2d:00.0/rom`, enable, read, disable): 130048 bytes, identical to `rom.bin`.

## Layout

- The window does not start with 0x55AA: offset 0 holds an IFR header (`NVGI`), so the RM takes `s_romImgFindPciHeader_TU102` (vbios-synthesis.md, row 2b) on the real card.
- Two identical 480 KiB copies: 0x00000-0x78000 and 0x78000-0xf0000 (byte-identical, both start with `NVGI`).
- 0xf0000-0x100000: a separate tail area, 0xff except 0xf9e00-0xff95d (a `VN`/`NPDS` block, type 0x70, 0x5c00, last).
- `rom.bin` (= sysfs ROM) equals 0x28600-0x48200 (the PCI-visible part); the first 130048 bytes of the window are not `rom.bin`.

## Image chain (first copy; second copy at +0x78000)

| Offset | Length | Sig | Header | Device | Code type | Last flag |
|---|---|---|---|---|---|---|
| 0x28600 | 0xec00 | 55AA | PCIR + NPDE | 0x1f02 | 0x00 legacy (PCI-AT) | no |
| 0x37200 | 0x11000 | 55AA | PCIR + NPDE | 0x1f02 | 0x03 EFI | PCIR yes, NPDE no |
| 0x48200 | 0xb400 | 564E | NPDS + NPDE | 0x1f00 | 0xe0 NVIDIA ext. (FWSEC #1) | no |
| 0x53600 | 0x22400 | 564E | NPDS + NPDE | 0x1f00 | 0xe0 NVIDIA ext. (FWSEC #2) | no |
| 0x75a00 | 0x1c00 | 564E | NPDS | 0x0000 | 0x70 | yes (PCIR) |

- Chain end 0x77600; 0xff padding 0x77600-0x78000 (and 0xef600-0xf9e00 after the second copy).
- Other 0xff runs: 0x3000-0x4000 and 0x7b000-0x7c000 (inside the pre-ROM IFR area), short fills before 0x48200 and 0x77600.

## Tables

- BIT at 0x287b0 (ROM offset 0x1b0), v1.00, 17 tokens; `I` at ROM 0x283, all its pointers inside the legacy image (script table 0x466a).
- `p` token (ROM 0x3b7): falcon data pointer 0xed24. Rule (nova-core, and the research note's 0x1fd24): pointer minus the legacy length, from the first 0xE0 image = 0x48324 (ROM offset 0x1fd24).
- Falcon ucode table at 0x48324: v1, header 6, entry 6, 16 entries (9 empty); descriptors resolve the same way (ptr - 0xec00 + 0x48200):

| App id | Target | Descriptor | Desc | Ucode extent |
|---|---|---|---|---|
| 0x01 | 1 | 0x4d954 (E0 #1) | v1, 0x5ac0 | |
| 0x82 | 1 | 0x5841c (E0 #2) | v1, 0x2790 | |
| 0x83 | 1 | 0x5abdc (E0 #2) | v1, 0x1cc0 | |
| 0x45 FWSEC debug | 7 | 0x5c8cc (E0 #2) | v2, 0x9c20 | 0x5c908-0x66528 |
| 0x85 FWSEC prod | 7 | 0x66528 (E0 #2) | v2, 0x9c20 | 0x66564-0x70184 |
| 0x89 | 5 | 0x70184 (E0 #2) | v2, 0x2c34 | 0x701c0-0x72df4 |
| 0x08 | 1 | 0x72df4 (E0 #2) | v1, 0x2af8 | |

- Everything lies inside the dump: the extension images, the ucode table and FWSEC are all in the first 480 KiB copy.
