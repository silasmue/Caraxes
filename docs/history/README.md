<!-- SPDX-License-Identifier: MIT -->
# Historical documents

- Dated snapshots preserve investigations, briefs, status logs and old commands.
- They describe the source and environment at the recorded date, not current behaviour.
- Raw evidence stays unchanged. A correction goes into the current document that
  supersedes the snapshot.
- Snapshots keep the names of their date: `caraxes-*` files and the code name in them
  belong to Leandro VFIO ([naming](../naming.md)).

## Leandro VirtIO (v1)

| File | Contents |
|---|---|
| [open-questions-2026-09-18.md](open-questions-2026-09-18.md) | The numbered investigations as of 2026-09-18; current: [open-questions.md](../virtio/open-questions.md) |
| [testing-2026-09-18.md](testing-2026-09-18.md) | Test coverage and hardware evidence as of 2026-09-18; current: [testing.md](../virtio/testing.md) |
| [llm-2026-09-18.md](llm-2026-09-18.md) | Maintenance notes as of 2026-09-18; current: [llm.md](../virtio/llm.md) |

## Leandro VFIO (v2)

| File | Contents |
|---|---|
| [caraxes-record-2026-09-28.md](caraxes-record-2026-09-28.md) | The working record of 2026-09-23 to 2026-09-28: goal, findings, the decisions table, backlog, ideas P1 to P24 and the status log; current: [docs/vfio](../vfio/README.md), [decisions](../vfio/decisions.md) |
| [caraxes-kickoff-2026-09-23.md](caraxes-kickoff-2026-09-23.md) | The brief for the first implementation sessions: rules, reading order, experiment E0 |
| [caraxes-experiments-2026-09-29.md](caraxes-experiments-2026-09-29.md) | The experiment ladder E0 to E5 and its per-run log, with the expectation and outcome of each |
| [caraxes-briefing-display-nova-2026-09-23.md](caraxes-briefing-display-nova-2026-09-23.md) | The operator's side assignment: DisplayPort sink, display adapter inventory, nova-core study |
| [caraxes-briefing-naming-2026-09-23.md](caraxes-briefing-naming-2026-09-23.md) | The naming and configuration brief behind `Leandro <Transport> <Personality>-<Profile>`; current: [naming.md](../naming.md) |
| [caraxes-cutover-615-2026-09-24.md](caraxes-cutover-615-2026-09-24.md) | The checklist for the host driver upgrade from 610.57.04 to 615.71.09 |
| [caraxes-environment-2026-09-23.md](caraxes-environment-2026-09-23.md) | Host, driver, tooling and vendor checkouts at the start; the reference dumps of the RTX 2070 |
| [caraxes-display-d0-2026-09-23.md](caraxes-display-d0-2026-09-23.md) | Display experiment D0: the stock modeset module headless on the device |
| [gsp-abi-610.md](../vfio/gsp-abi-610.md) | The GSP message ABI of driver 610.57.04, compiled; kept in vfio/ because the device embeds it (`ver/v610.rs`); current: [gsp-abi-615.md](../vfio/gsp-abi-615.md) |
| [caraxes-gsp-rpc-protocol-610.md](caraxes-gsp-rpc-protocol-610.md) | The 610.57.04 CPU-GSP protocol from source; current: [gsp-transport-615.md](../vfio/gsp-transport-615.md) |
| [caraxes-phase5-rpcs-610.md](caraxes-phase5-rpcs-610.md) | The 98 RPCs of a 610 boot up to `RmInitAdapter succeeded`, with an answer source each |
| [caraxes-repo-split-2026-09-24.md](caraxes-repo-split-2026-09-24.md) | The plan for separating v1 and v2 into two repositories; not taken, both live here |
| [caraxes-roadmap-2026-09-27.md](caraxes-roadmap-2026-09-27.md) | Done, in progress, next and later as of 2026-09-24 with annotations to 2026-09-27; current: [roadmap.md](../roadmap.md), [leandro.md](../leandro.md) chapter 12 |
| [caraxes-open-questions-2026-09-23.md](caraxes-open-questions-2026-09-23.md) | Questions 1 to 33 of the bring-up; the open ones live in the research notes and [known-issues.md](../vfio/known-issues.md) |
