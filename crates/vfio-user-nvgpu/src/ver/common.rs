// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The vocabulary every guest ABI shares: the 610.57.04 module ([`crate::ver::v610`])
//! without the items that differ between the guest ABIs the device serves.
//!
//! The items that differ are exactly the ones `v615.rs` overrides (and the three of
//! `v615/display.rs` generic code reads). Generic code reads them at run time from
//! [`crate::ver::layouts`], the table of the guest ABI in force ([`crate::ver::guest`]).
//! Each one is shadowed here by a private item of the same name, so `common::FS_PARAMS`
//! is a compile error (E0603, private constant) instead of the 610 number silently
//! answering a 615 guest. The test `ver::abi_tests::the_shared_vocabulary_hides_every_615_override` keeps
//! the list below equal to the overrides of `v615.rs` and `v615/display.rs`.

pub use super::v610::*;

/// Private stand-ins that shadow the glob import: read these through `ver::layouts()`
/// (or, for a transport-specific number, the transport's own module: `ver::v610`,
/// `msgq::v2`).
macro_rules! per_guest {
    ($($n:ident),* $(,)?) => {
        $(
            #[allow(dead_code, hidden_glob_reexports)]
            const $n: () = ();
        )*
    };
}

per_guest!(
    // The guest's driver and transport: `layouts().driver`, `layouts().transport`.
    DRIVER_VERSION,
    GSP_TRANSPORT,
    // Transport numbers: `ver::element_layout()`, `ver::v610::MQ_*` (message queue v0),
    // `msgq::v2` (v2).
    ELEMENT_HEADER_SIZE,
    MAX_RPC_SIZE,
    MQ_SHARED_MEM_PA,
    MQ_PAGE_TABLE_ENTRY_COUNT,
    MQ_CMD_QUEUE_OFFSET,
    MQ_STAT_QUEUE_OFFSET,
    // `layouts().ctrl_0a36`.
    CTRL_0A36,
    // `layouts().devinfo`.
    DEVINFO_PARAMS_SIZE,
    DEVINFO_NUM_ENTRIES,
    DEVINFO_B_MORE,
    DEVINFO_ENTRIES,
    DEVINFO_ENTRY_SIZE,
    DEVINFO_ENGINE_DATA,
    DEVINFO_PBDMA_IDS,
    DEVINFO_PBDMA_FAULT_IDS,
    DEVINFO_NUM_PBDMAS,
    DEVINFO_ENGINE_NAME,
    // `layouts().p2p_*`.
    P2P_READ_CAPS_STATUS,
    P2P_WRITE_CAPS_STATUS,
    // `layouts().fs`.
    FS_ELEM,
    FS_PARAMS,
    FS_GPC_MASK,
    FS_TPC_MASK,
    FS_TPC_COUNT,
    FS_PHYS_GPC_MASK,
    FS_TPC_TO_PES_MAP,
    FS_TPC_TO_PES_MAP_LEN,
    FS_NUM_PES_PER_GPC,
    FS_ZCULL_MASK,
    FS_PHYS_GFX_GPC_MASK,
    FS_NUM_GFX_TPC,
    // `layouts().sm_order`, `layouts().sm_order_params`.
    SM_ORDER_GUEST,
    SM_ENTRY,
    SM_ORDER_ELEM,
    SM_ORDER_NUM_SM,
    SM_ORDER_NUM_TPC,
    SM_ORDER_PARAMS,
);

/// The guest's allocation parameter layouts: `layouts().tsg_size` and
/// `layouts().tsg_internal_flags` for the TSG (20 bytes in 610, 28 in 615); the channel
/// allocation is the same in both (`nvrm_sys::V610`, hostabi.rs asserts it).
#[allow(dead_code, hidden_glob_reexports)]
type RmAbi = ();

/// The display control vocabulary every guest ABI shares (`v610/display.rs`), without the
/// items `v615/display.rs` overrides: `layouts().display`.
pub mod display {
    pub use super::super::v610::display::*;

    per_guest!(
        GET_ACTIVE_SIZE,
        GET_ACTIVE_STALE_SOR_INDEX,
        GET_ACTIVE_FLAGS_GOP_STALE_HEAD,
        DP_CAPS_INTERNAL_DP_TUNNELING,
        CMD_DP_SET_WAR_FLAGS,
    );
}
