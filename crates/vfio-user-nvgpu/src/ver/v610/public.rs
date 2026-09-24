// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The public controls of the GSP plan (docs/caraxes/gsp-plan/b-host-forwardable.md and
//! c-guest-state.md) that are not a plain forward: forwards whose answer is filtered
//! before it reaches the guest, answers the device owns, controls on the guest's own
//! channels and TSGs, and refusals. One row per control, the handler is `crate::public`.
//! Plain forwards stay in the `FORWARD` list of the version module (nvml.rs).
//!
//! Every parameter struct named here has the same `sizeof` and field offsets in
//! 610.57.04 and 615.71.09 (measured with `cc -std=gnu11 -DNV_LINUX` against each tree's
//! SDK headers, 2026-09-24; b-host-forwardable.md §2 found the same for all 70 B rows),
//! so the 615 module takes this one unchanged. Where `nvrm-sys` binds a struct, the
//! tests pin the numbers to the binding.

/// How the device answers one control of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// Forward to the host subdevice, then filter the answer before the guest sees it.
    Filtered(Filter),
}

/// The output filters of forwarded controls (b-host-forwardable.md §3 F2 and §6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    /// `FB_GET_OFFLINED_PAGES`: keep the entries whose pages lie in the reservation.
    OfflinedPages,
    /// `FB_GET_REMAPPED_ROWS`: DRAM rows cannot be placed in the reservation; no entry
    /// reaches the guest, the card-level flags do.
    RemappedRows,
}

/// One control of the table.
#[derive(Clone, Copy, Debug)]
pub struct PublicControl {
    pub id: u32,
    pub name: &'static str,
    /// `sizeof` of the params struct; 0 where the rule does not read the params.
    pub size: usize,
    pub rule: Rule,
}

pub const PUBLIC: &[PublicControl] = &[
    // Forwarded as-is until 2026-09-24 (the FORWARD list): the host answer carries host FB
    // page numbers, possibly outside the reservation (b-host-forwardable.md §6).
    PublicControl {
        id: 0x2080_1322,
        name: "FB_GET_OFFLINED_PAGES",
        size: OFFLINED_PAGES_SIZE,
        rule: Rule::Filtered(Filter::OfflinedPages),
    },
    PublicControl {
        id: 0x2080_1344,
        name: "FB_GET_REMAPPED_ROWS",
        size: REMAPPED_ROWS_SIZE,
        rule: Rule::Filtered(Filter::RemappedRows),
    },
];

/// The row of `id`, if the table has one.
pub fn public_control(id: u32) -> Option<&'static PublicControl> {
    PUBLIC.iter().find(|c| c.id == id)
}

// ---------------------------------------------------------------------------------
// Layouts (sdk/ctrl/ctrl2080/ctrl2080fb.h; line numbers of 615.71.09).

/// `NV2080_CTRL_FB_GET_OFFLINED_PAGES_PARAMS` (ctrl2080fb.h:1083-1088): `offlined[64]` of
/// `NV2080_CTRL_FB_OFFLINED_ADDRESS_INFO` (:999-1006: `pageAddressWithEccOn` u64 @0,
/// `pageAddressWithEccOff` u64 @8, `rbcAddress` @16, `source` @20, `status` @24,
/// `timestamp` @28; 32 bytes), `validEntries` @2048, `bRetirementPending` @2052,
/// `retirementPending` @2053; 2056 bytes. The addresses are 4 KiB page numbers
/// (mem_mgr_gm107.c:1711-1716 shifts them by `RM_PAGE_SHIFT`).
pub const OFFLINED_PAGES_SIZE: usize = 2056;
pub const OFFLINED_ENTRY_SIZE: usize = 32;
pub const OFFLINED_MAX: usize = 64;
pub const OFFLINED_ECC_ON: usize = 0;
pub const OFFLINED_ECC_OFF: usize = 8;
pub const OFFLINED_STATUS: usize = 24;
pub const OFFLINED_VALID_ENTRIES: usize = 2048;
pub const OFFLINED_B_RETIREMENT_PENDING: usize = 2052;
pub const OFFLINED_RETIREMENT_PENDING: usize = 2053;
/// `NV2080_CTRL_FB_OFFLINED_PAGES_INVALID_ADDRESS` (ctrl2080fb.h:960).
pub const OFFLINED_INVALID_ADDRESS: u64 = u64::MAX;
/// `NV2080_CTRL_FB_OFFLINED_PAGES_STATUS_PENDING_RETIREMENT` (ctrl2080fb.h:1016).
pub const OFFLINED_STATUS_PENDING_RETIREMENT: u32 = 1;
/// `RM_PAGE_SHIFT`: the page numbers count 4 KiB pages.
pub const RM_PAGE_SHIFT: u32 = 12;

/// `NV2080_CTRL_FB_GET_REMAPPED_ROWS_PARAMS` (ctrl2080fb.h:1973-1977): `entryCount` @0,
/// `flags` @4 (PENDING 0:0, FAILURE 1:1), `entries[512]` of 12-byte
/// `NV2080_CTRL_FB_ROW_REMAP_ENTRY` @8 (`remapRegVal`, `timestamp`, `fbpa`,
/// `sublocation`, `source`, `flags`: a DRAM row, no FB address); 6152 bytes.
pub const REMAPPED_ROWS_SIZE: usize = 6152;
pub const REMAPPED_ROWS_ENTRY_COUNT: usize = 0;
pub const REMAPPED_ROWS_FLAGS: usize = 4;
pub const REMAPPED_ROWS_ENTRIES: usize = 8;

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{offset_of, size_of};

    /// The layouts against the `nvrm-sys` bindings (the same in every version, `stable`).
    #[test]
    fn layouts_match_the_bindings() {
        type P = nvrm_sys::NV2080_CTRL_FB_GET_OFFLINED_PAGES_PARAMS;
        type E = nvrm_sys::NV2080_CTRL_FB_OFFLINED_ADDRESS_INFO;
        assert_eq!(size_of::<P>(), OFFLINED_PAGES_SIZE);
        assert_eq!(size_of::<E>(), OFFLINED_ENTRY_SIZE);
        assert_eq!(offset_of!(P, validEntries), OFFLINED_VALID_ENTRIES);
        assert_eq!(offset_of!(P, bRetirementPending), OFFLINED_B_RETIREMENT_PENDING);
        assert_eq!(offset_of!(P, retirementPending), OFFLINED_RETIREMENT_PENDING);
        assert_eq!(offset_of!(E, pageAddressWithEccOn), OFFLINED_ECC_ON);
        assert_eq!(offset_of!(E, pageAddressWithEccOff), OFFLINED_ECC_OFF);
        assert_eq!(offset_of!(E, status), OFFLINED_STATUS);
        assert_eq!(OFFLINED_MAX * OFFLINED_ENTRY_SIZE, OFFLINED_VALID_ENTRIES);
        // Bound per version (the entry struct differs by name only).
        type R610 = nvrm_sys::v610::NV2080_CTRL_FB_GET_REMAPPED_ROWS_PARAMS;
        type R615 = nvrm_sys::v615::NV2080_CTRL_FB_GET_REMAPPED_ROWS_PARAMS;
        assert_eq!(size_of::<R610>(), REMAPPED_ROWS_SIZE);
        assert_eq!(size_of::<R615>(), REMAPPED_ROWS_SIZE);
        assert_eq!(offset_of!(R610, flags), REMAPPED_ROWS_FLAGS);
        assert_eq!(offset_of!(R615, flags), REMAPPED_ROWS_FLAGS);
        assert_eq!(offset_of!(R610, entries), REMAPPED_ROWS_ENTRIES);
        assert_eq!(offset_of!(R615, entries), REMAPPED_ROWS_ENTRIES);
        assert_eq!(offset_of!(R615, entryCount), REMAPPED_ROWS_ENTRY_COUNT);
    }

    #[test]
    fn one_row_per_id() {
        let mut ids: Vec<u32> = PUBLIC.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PUBLIC.len());
    }
}
