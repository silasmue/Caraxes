## The sweep

172 controls in the table, 119 issued (53 without their object: NV04_DISPLAY_COMMON is refused on vm2, which has no display engine).

| path | provenance | guest status | controls |
|---|---|---|---|
| answered by the guest kernel RM | guest-derived | 0x57 | 1 |
| answered by the guest kernel RM | refused | 0x1f | 4 |
| answered by the guest kernel RM | synthesized | 0 | 1 |
| answered by the guest kernel RM | synthesized | 0x56 | 1 |
| answered by the guest kernel RM | unhandled | 0x21 | 1 |
| reached the device | guest-derived | 0 | 4 |
| reached the device | guest-derived | 0x1f | 1 |
| reached the device | guest-derived | 0x33 | 2 |
| reached the device | host-forwarded | 0 | 14 |
| reached the device | host-forwarded | 0x1f | 3 |
| reached the device | host-forwarded | 0x56 | 12 |
| reached the device | host-forwarded | 0xffff | 1 |
| reached the device | refused | 0x1b | 1 |
| reached the device | refused | 0x56 | 40 |
| reached the device | synthesized | 0 | 12 |
| reached the device | unhandled | 0x56 | 21 |

Reached the device: 111 of 119; the device's answers there by label: refused (feature absent on this chip) 28, host (forwarded as-is, public control) 27, unanswered 21, refused (feature absent on this board) 4, guest (no channel of this device) 2, host (forwarded, filtered: scrub range clamped to the VM's reservation) 1, refused (MIG absent on this chip) 1, chip (no FIPS mode on TU106: bFipsEnabled 0) 1, chip (first async CE is CE2, chips/tu106/device-info.toml) 1, board (the synthesized VBIOS carries the captured EFI image: present, not running; version not parsed) 1, synthesized (the synthesized VBIOS has no NBSI: NOT_FOUND, retSize 0) 1, refused (host scheduler log: every tenant's scheduling) 1, guest (a guest ChID only when one of this VM's own mirrors is resident; never another tenant's) 1, synthesized (no compression, D16: numCompTags 0) 1, refused (isolation) 1, host (forwarded, filtered: dynamically offlined pages inside the VM's reservation) 1, chip (the engine non-stall vectors of chips/tu106/interrupts.toml) 1, chip (the static interrupts of chips/tu106/interrupts.toml) 1, chip (the subtree map of chips/tu106/interrupts.toml) 1, refused (raw PHY register read) 1, refused (eye measurement: acts on the physical link) 1, refused (Tegra only) 1, guest (the device's RC journal for this guest, rc.rs) 1, synthesized (the device's RC policy: recovery enabled) 1, guest (the device's RC journal, rc.rs: no such record) 1, host (forwarded as-is; NON_PRIVILEGED on the 615 host) 1, chip (no decompression LCE on Turing, chip.toml [ce]) 1, synthesized (NOT_SUPPORTED: no ECC on this board, as the host answers the ECC getters, Q-B1) 1, synthesized (the device's own GSP heap view: no RM heap, all 0; the host GSP is never asked, decision D9/Q-B6) 1, refused (no dielets on this chip) 1, guest (only the graphics engine has a context here) 1, refused (diag class, not opened on the host) 1, synthesized (no I2C ports, as I2C_GET_PORT_INFO answers) 1, guest (ZBC table of this VM: the chip defaults read once from the host, then this guest's entries) 1

## Device-answered controls no workload and no sweep reached

| provenance | untouched | user space, non-priv | user space as root (priv) | object the sweep lacks | guest kernel only (internal, kernel) |
|---|---|---|---|---|---|
| synthesized | 42 | 15 | 12 | 7 | 8 |
| host-forwarded | 0 | 0 | 0 | 0 | 0 |
| guest-derived | 59 | 11 | 7 | 18 | 23 |
| echo | 48 | 5 | 17 | 8 | 18 |
| refused | 427 | 51 | 96 | 73 | 207 |

By the handler (controls.tsv answered_by), largest groups first:

| provenance | answered_by | reach | controls |
|---|---|---|---|
| refused | ctrl:E family nvlink | kernel | 51 |
| refused | ctrl:D table | priv | 46 |
| refused | ctrl:D table | object | 34 |
| refused | ctrl:D table | kernel | 28 |
| refused | ctrl:D table | non-priv | 24 |
| refused | ctrl:E family vgpu | kernel | 18 |
| refused | public:public table | object | 17 |
| guest-derived | public:public table | object | 15 |
| echo | display | kernel | 15 |
| refused | ctrl:E family notebook_power | kernel | 15 |
| refused | ctrl:E family nvlink_sli | kernel | 13 |
| synthesized | display | non-priv | 12 |
| refused | ctrl:E family nvlink | priv | 12 |
| synthesized | display | priv | 10 |
| echo | display | priv | 10 |
| guest-derived | ctrl:D table | kernel | 9 |
| refused | ctrl:E family legacy_uncalled | kernel | 9 |
| refused | ctrl:E family gsync | kernel | 9 |
| refused | ctrl:E family mig | kernel | 9 |
| refused | ctrl:E family rm_test | non-priv | 8 |
| refused | ctrl:E family cc | kernel | 8 |
| guest-derived | ctrl:D table | non-priv | 7 |
| echo | ctrl:D table | priv | 7 |
| refused | public:public table | non-priv | 7 |
| synthesized | display | kernel | 6 |
