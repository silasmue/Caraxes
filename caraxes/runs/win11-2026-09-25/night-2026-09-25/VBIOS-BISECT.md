<!-- SPDX-License-Identifier: MIT -->
# VBIOS bisect: which flash content the 617.14 driver needs (2026-09-25)

- Setup: run L of `night-win2.sh` (GeForce identity, `vfio-user-nvgpu-v615-extcfg-9df39581`, `rom-legacy-only.bin` ROM BAR), `WIN_MEM=4G`, one boot per variant, `NIGHT_PROM_DUMP=<variant>`.
- Variants built from `prom-window-2026-09-25-from-55aa.bin` (0xd7a00 bytes), same length, 0xff padding; kept in `/mnt/vmstore/caraxes/win11/vbios-variants/` (not in the repo).
- Results: `<variant>-L-extcfg-promdump-summary.txt` here; traces `*-trace.log.gz` stay local (not committed). The original run L moved to `L-original/`.
- "Stopped at": the last first-seen BAR0 register before the driver clears memory decode (`W cfg 0x04 = 0x1`), which precedes the PnP re-enumeration.

| Variant | Contents | Distinct regs | Falcon writes | PROM read up to | Stopped at |
|---|---|---|---|---|---|
| W-full-cut-repeat | unmodified from-55aa image (control) | 554 | 345 | 0x4effc (chain end) | `R 0x88088` (config mirror), then teardown `0xdc68 ... 0x160`; `gsp start 1: halted` + FWSEC data buffer, later `gsp start 2: halted` |
| S-no-0x70 | legacy + EFI + E0 #1 + E0 #2; 0x70 image dropped, E0 #2 marked last (NPDS + NPDE) | 554 | 345 | 0x4d3fc (new chain end) | same as W, same events |
| T-first-fwsec-only | legacy + EFI + E0 #1; E0 #1 marked last | 37 | 0 | 0x2affc (new chain end) | `R 0x22408`; no falcon event |
| U-fwsec-bodies-zeroed | full chain, both E0 images keep NPDS/NPDE, everything after the NPDE zeroed (includes the ucode table at 0x1fd24) | 36 | 0 | 0x7fffc (512 KiB scan) | `R 0x22408`; no falcon event |
| V-rom-plus-ucode-table | rom.bin (EFI last) + only the ucode table (0x1fd24, 0x66 bytes) and the 7 descriptor headers (v1 0x30, v2 0x3c) in 0xff space | 36 | 0 | 0x7fffc (512 KiB scan) | `R 0x22408`; no falcon event |

- W reproduces the first run L exactly (554 / 345), so a boot is deterministic enough for a single run per variant.
- T/U/V end at the old Code 43 loop (35-register baseline plus `0x22408`, T also `0x1100`), no GSP/PMU programming.
- U and V change image checksums (E0 bytes zeroed or missing); S and T keep all image sums at 0 (the two last-flag bits add 0x100).

## Conclusion

- Required: the second 0xE0 image (0x2b000, 0x22400 bytes), reachable through the image chain. It holds six of the seven ucode descriptors and the ucode itself, among them FWSEC prod (app 0x85) and app 0x89; T, which has the table but not its targets, fails.
- Required: real ucode bytes behind the table. Headers alone (U) or the table and descriptors alone (V) fail the same way as the plain `rom.bin`.
- Not required: the type 0x70 image and everything after the chain (second flash copy, tail area): S equals W.
- Open: whether the first 0xE0 image is needed beyond carrying the ucode table (the table sits at its start, app 0x01 descriptor inside it). Next bisect step: E0 #1 with only header + table kept, E0 #2 intact; then per-app descriptors (0x85 alone).
- The driver locates the 0xE0 images by walking the chain: with the table present but EFI marked last (V) it scans the first 512 KiB and gives up.
