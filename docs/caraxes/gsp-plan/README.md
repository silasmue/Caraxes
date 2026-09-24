<!-- SPDX-License-Identifier: MIT -->
<!-- SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de> -->
<!-- SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR -->
# GSP surface: implementation plans

One plan document per part of [gsp-surface.md](../gsp-surface.md) (P23). Each document takes
its part item by item against the 610.57.04 and 615.71.09 trees, so that implementation agents
work from the plan rather than from run logs. Each plan agent adds its own row.

| Plan | Covers | Contents |
|---|---|---|
| [b-host-forwardable.md](b-host-forwardable.md) | category B, 70 controls | per control: class, params and size in both trees (no layout differences), route on the real card, what the host answer is, the isolation filter, known clients, a one-line recipe; six groups (26 forward as-is, 4 forward filtered, 11 device-owned, 12 moved to C, 13 refused, 4 never sent here), the table-driven form with three worked examples, the order, open questions with measurements; machine-readable rows in [b-host-forwardable.toml](b-host-forwardable.toml) |
| [c-guest-state.md](c-guest-state.md) | category C, 31 controls (29 in 610) plus 12 from B | per control: route, the guest state the answer derives from, where it lives in the device (module and struct), what is missing, the answer and a test recipe; seven groups, the attribute record and table form, the missing pieces by module, the order, open questions with measurements |
