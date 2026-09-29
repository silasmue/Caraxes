// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The RTX 2070 as the unit tests see it: the TU106 chip, the built-in board data
//! and the captured config space; the RTX 5060 Ti (GB206) the same way.

use crate::board::Board;
use crate::chip::{self, Chip};
use crate::config::ConfigSpace;
use crate::rpc::StaticIdentity;

const DUMP: &str = include_str!("../../../caraxes/reference/rtx2070/config-space.txt");
pub const DUMP_GB206: &str =
    include_str!("../../../caraxes/reference/gb206/gpu-0000_01_00.0/config-space.txt");

pub struct Fixture {
    pub chip: &'static dyn Chip,
    pub board: Board,
    dump: &'static str,
    /// The capture of the board's `ids_from`, when it has one.
    ids: Option<&'static str>,
}

pub fn rtx2070() -> Fixture {
    Fixture {
        chip: chip::by_name("tu106").unwrap(),
        board: Board::load("rtx2070").unwrap(),
        dump: DUMP,
        ids: None,
    }
}

pub fn rtx5060ti() -> Fixture {
    Fixture {
        chip: chip::by_name("gb206").unwrap(),
        board: Board::load("rtx5060ti").unwrap(),
        dump: DUMP_GB206,
        ids: None,
    }
}

/// The costume (docs/caraxes/costume-5060.md): the TU106 chip and config layout under the
/// RTX 5060 Ti's PCI ids (board data `rtx5060ti-on-tu106`, ids from the GB206 capture).
pub fn rtx5060ti_on_tu106() -> Fixture {
    Fixture {
        chip: chip::by_name("tu106").unwrap(),
        board: Board::load("rtx5060ti-on-tu106").unwrap(),
        dump: DUMP,
        ids: Some(DUMP_GB206),
    }
}

impl Fixture {
    pub fn config(&self) -> ConfigSpace {
        crate::config::for_board_with_ids(self.dump, self.ids, &self.board).unwrap()
    }

    /// The identity `main` builds for `fb_bytes` ("Leandro VFIO RTX 2070-<size>").
    pub fn identity(&self, fb_bytes: u64) -> StaticIdentity {
        StaticIdentity::new(
            self.chip.data(),
            &self.board,
            self.config().ids(),
            &self.board.name,
            fb_bytes,
        )
    }
}
