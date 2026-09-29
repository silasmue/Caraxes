// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! Driver 615.71.09: the display control vocabulary (docs/caraxes/display-615.md §3).
//!
//! The 610.57.04 vocabulary (`ver/v610/display.rs`) with the items that differ overridden
//! here. Between the two trees only three display control headers changed
//! (`sdk/ctrl/ctrl0073/ctrl0073dp.h`, `ctrl0073specific.h`, `ctrl0073system.h`; the NV5070,
//! NVC370 and NVC372 control headers are identical, and of the display classes only method
//! and capability defines were added that NVKMS already defined itself). The table
//! `display-layouts.txt` is the gcc measurement of the 615.71.09 headers for the structs
//! the 610 table lists plus the new and changed ones
//! (caraxes/runs/display-615-2026-09-24/abi/gen.py); the test below checks that every
//! number the 610 table and this one share is either equal or overridden here.
//!
//! What changed for the controls the device answers:
//! - `NV0073_CTRL_SYSTEM_GET_ACTIVE_PARAMS` grew from 16 to 20 bytes: `staleSorIndex`
//!   appended, with the output flag `GOP_STALE_HEAD_NEEDS_CLEANUP` (bit 1 of `flags`)
//!   that makes it valid (ctrl0073system.h:638-670, :676-678);
//! - `NV0073_CTRL_CMD_DP_GET_CAPS_PARAMS` gained `bIsInternalDpTunnelingSupported` at 33,
//!   in what was padding before `DSC` (ctrl0073dp.h:1743): the size (64) and every other
//!   offset stay;
//! - `NV0073_CTRL_CMD_DP_SET_WAR_FLAGS` is new (ctrl0073dp.h:3734-3773); the DP library
//!   sends it on every plug after the OUI handshake and on every unplug
//!   (src/common/displayport/src/dp_connectorimpl.cpp:8048, :8475, dp_evoadapter.cpp:355-367).
//!
//! Unchanged for the device, listed in the test: `INIT_BRIGHTC_STATE_LOAD` (a flag inserted
//! before the backlight data, same size; the device only acknowledges it),
//! `SPECIFIC_DISPLAY_BRIGHTNESS_LTM` (five fields appended; refused, no panel) and
//! `SPECIFIC_HDCP_KEY_DECRYPTION` (new; refused, no HDCP, ver/v615/dmodel.rs).

pub use super::super::v610::display::*;

/// `NV0073_CTRL_SYSTEM_GET_ACTIVE_PARAMS` (20): `staleSorIndex` at 16 (ctrl0073system.h:669).
pub const GET_ACTIVE_SIZE: usize = 20;
pub const GET_ACTIVE_STALE_SOR_INDEX: Option<usize> = Some(16);
/// `NV0073_CTRL_SYSTEM_GET_ACTIVE_FLAGS_GOP_STALE_HEAD_NEEDS_CLEANUP` (1:1), an output.
pub const GET_ACTIVE_FLAGS_GOP_STALE_HEAD: u32 = 1 << 1;
/// `NV0073_CTRL_CMD_DP_GET_CAPS_PARAMS.bIsInternalDpTunnelingSupported` (ctrl0073dp.h:1743).
pub const DP_CAPS_INTERNAL_DP_TUNNELING: Option<usize> = Some(33);
/// `NV0073_CTRL_CMD_DP_SET_WAR_FLAGS` (ctrl0073dp.h:3773): `{subDeviceInstance, displayId,
/// warId, bEnable}`, 16 bytes; the offsets are the 610 module's `WAR_FLAGS_*`.
pub const CMD_DP_SET_WAR_FLAGS: Option<u32> = Some(0x73_1391);
pub const WAR_FLAGS_SIZE: usize = 16;

#[cfg(test)]
mod tests {
    use super::*;

    const MEASURED_615: &str = include_str!("display-layouts.txt");
    const MEASURED_610: &str = include_str!("../v610/display-layouts.txt");

    fn table(text: &str) -> Vec<(&str, usize)> {
        text.lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let (k, v) = l.rsplit_once(' ')?;
                Some((k, v.parse().ok()?))
            })
            .collect()
    }

    fn measured(key: &str) -> usize {
        table(MEASURED_615)
            .into_iter()
            .find_map(|(k, v)| (k == key).then_some(v))
            .unwrap_or_else(|| panic!("{key} not measured for 615"))
    }

    /// Every size and offset the 610 table measured is the same in 615, except the ones
    /// this module overrides or the device does not read (listed with the reason).
    #[test]
    fn display_layouts_differ_only_where_overridden() {
        let t615 = table(MEASURED_615);
        let mut differ: Vec<&str> = table(MEASURED_610)
            .into_iter()
            .filter(|(k, v)| t615.iter().find(|(k2, _)| k2 == k).map(|(_, v2)| v2) != Some(v))
            .map(|(k, _)| k)
            .collect();
        differ.sort();
        assert_eq!(
            differ,
            [
                // Overridden here (GET_ACTIVE_SIZE).
                "NV0073_CTRL_SYSTEM_GET_ACTIVE_PARAMS sizeof",
                // Acknowledged without reading the parameters (display/mod.rs).
                "NV2080_CTRL_INTERNAL_INIT_BRIGHTC_STATE_LOAD_PARAMS.backLightData",
                "NV2080_CTRL_INTERNAL_INIT_BRIGHTC_STATE_LOAD_PARAMS.backLightDataSize",
            ]
        );
        assert_eq!(
            measured("NV2080_CTRL_INTERNAL_INIT_BRIGHTC_STATE_LOAD_PARAMS sizeof"),
            BRIGHTC_STATE_LOAD_SIZE
        );
    }

    #[test]
    fn the_615_display_fields_match_the_headers() {
        let s = |n: &str| measured(&format!("{n} sizeof"));
        let f = |n: &str, field: &str| measured(&format!("{n}.{field}"));
        let ga = "NV0073_CTRL_SYSTEM_GET_ACTIVE_PARAMS";
        assert_eq!(s(ga), GET_ACTIVE_SIZE);
        assert_eq!(f(ga, "head"), GET_ACTIVE_HEAD);
        assert_eq!(f(ga, "flags"), GET_ACTIVE_FLAGS);
        assert_eq!(f(ga, "displayId"), GET_ACTIVE_DISPLAY_ID);
        assert_eq!(Some(f(ga, "staleSorIndex")), GET_ACTIVE_STALE_SOR_INDEX);
        let caps = "NV0073_CTRL_CMD_DP_GET_CAPS_PARAMS";
        assert_eq!(s(caps), DP_CAPS_SIZE);
        assert_eq!(f(caps, "DSC"), DP_CAPS_DSC_SUPPORTED);
        assert_eq!(
            Some(f(caps, "bIsInternalDpTunnelingSupported")),
            DP_CAPS_INTERNAL_DP_TUNNELING
        );
        let w = "NV0073_CTRL_CMD_DP_SET_WAR_FLAGS_PARAMS";
        assert_eq!(s(w), WAR_FLAGS_SIZE);
        assert_eq!(f(w, "displayId"), WAR_FLAGS_DISPLAY_ID);
        assert_eq!(f(w, "warId"), WAR_FLAGS_WAR_ID);
        assert_eq!(f(w, "bEnable"), WAR_FLAGS_ENABLE);
        // The id and size agree with the generated D table (ver/v615/dmodel.rs).
        let r = crate::ver::dmodel615::D_RULES
            .iter()
            .find(|r| r.name == "NV0073_CTRL_CMD_DP_SET_WAR_FLAGS")
            .unwrap();
        assert_eq!(
            (Some(r.id), r.size as usize),
            (CMD_DP_SET_WAR_FLAGS, WAR_FLAGS_SIZE)
        );
        // The two new controls the device refuses keep their measured sizes.
        assert_eq!(s("NV0073_CTRL_SPECIFIC_HDCP_KEY_DECRYPTION_PARAMS"), 8);
        assert_eq!(s("NV0073_CTRL_SPECIFIC_DISPLAY_BRIGHTNESS_LTM_PARAMS"), 72);
    }
}
