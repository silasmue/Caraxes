// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The RTX 2070 as the unit tests see it: the TU106 chip, the built-in board data
//! and the captured config space.

use crate::board::Board;
use crate::chip::{self, Chip};
use crate::config::{parse_dump, ConfigSpace};
use crate::rpc::StaticIdentity;

const DUMP: &str = include_str!("../../../caraxes/reference/rtx2070/config-space.txt");

pub struct Fixture {
    pub chip: &'static dyn Chip,
    pub board: Board,
}

pub fn rtx2070() -> Fixture {
    Fixture {
        chip: chip::by_name("tu106").unwrap(),
        board: Board::load("rtx2070").unwrap(),
    }
}

impl Fixture {
    pub fn config(&self) -> ConfigSpace {
        ConfigSpace::from_dump(
            parse_dump(DUMP).unwrap(),
            self.board.bars,
            self.board.rom_bar_size,
        )
        .unwrap()
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
