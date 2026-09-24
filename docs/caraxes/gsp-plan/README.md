<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# The GSP-surface implementation plan (P23)

This folder holds the plans that implementation agents work from, one document per part of
[gsp-surface.md](../gsp-surface.md). Each covers one category or one part of the RPC, event,
class and boot exchange, for driver 615.71.09 (D19).

| Document | Contents |
|---|---|
| [d-device-model.md](d-device-model.md) | Category D, the 308 controls that need device modelling. They are sorted into 17 subsystem groups. For each group: the controls with their 610 and 615 parameter sizes, the state a model needs, what the closed GSP-RM does (inferred), callers, truth source, isolation risk, design, estimate and rank. The document also covers the finding that no CUDA or Wayland workload has reached D so far, the answer rules (REFUSE, DENY, ACK, MODEL, MIRROR, TABLE), the 126 controls that belong to A, C or E, a roadmap by client, and 14 open questions with the measurement for each. |
| [d-controls.tsv](d-controls.tsv) | The per-control data of d-device-model.md, one row per control: id, sizes, header lines, answer, rank, client, caller, truth source, risk and reclassification. |
