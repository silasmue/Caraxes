# Windows sessions of 2026-09-30 (C93-C122)

The Windows agent's session tools for the night; the results are in
docs/research/windows-findings.md, steps 22 and 23.

- `session.sh TAG BINARY STEPS...`: one Windows session under `vmlock.sh windows`: start, the
  steps, always a stop. The steps: etwclean, arm, restart, heaven, superposition, stages, reboot,
  growc, repair, offlinefix, diag, sleep=N and more. A host-Xid watchdog stops the session on
  any NVRM line of the device's pid.
- `session2.sh`: session.sh plus these steps:
  - ids, driverhash, apphash: the integrity checks of C108;
  - gpuclients, quiet: stop Steam and Edge for this session only (C110, C111);
  - present-bg=N, present-status=N, edge-bg: a second GPU client beside a benchmark (C112);
  - clients: the reconnects after a reboot (C119);
  - events: WER and display events;
  - cacheclear.
  It also keeps the device's exit profile with the run.
- `session3.sh`: session2.sh with the flags branch's minimal device environment (C120).
- `watch-hang.py TAG [GAP_S]`: during a benchmark step, a window-flip gap requests two live state
  dumps through `<state dir>/device-ctl` (C114, C115).
- `chain-*.sh`: the order of the night's sessions as they were queued.
