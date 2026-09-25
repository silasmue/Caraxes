// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The `ecc` and `row_remap` families (docs/caraxes/gsp-plan/e-families.md §3.18, §3.19;
//! docs/caraxes/limits-mig-ecc-vgpu.md §2): the ECC state a data-center board shows, from
//! the headers of the emulated driver version. Paths are relative to the 610.57.04 tree.
//!
//! All controls here are ROUTE_TO_PHYSICAL with no kernel implementation on a GSP client
//! (src/nvidia/generated/g_subdevice_nvoc.c; `rmresControl_Prologue_IMPL`,
//! src/nvidia/src/kernel/rmapi/resource.c:254-297), so what the model writes is what the
//! guest's caller reads.
//!
//! * **Mode.** nvidia-smi's "Current" is derived from `GPU_QUERY_ECC_STATUS`
//!   `units[].enabled`; its "Pending" is `GPU_QUERY_ECC_CONFIGURATION`
//!   `currentConfiguration`, "the current ECC setting ... stored in non-volatile memory"
//!   (ctrl2080gpu.h:1315-1329). `GPU_SET_ECC_CONFIGURATION` stores `newConfiguration`,
//!   which "will take effect with the next GPU reset" (ctrl2080gpu.h:1349-1358): the
//!   pending half of [`super::Mode`], applied at the next reset or power-on.
//! * **Units.** The open headers name one unit, `NV2080_CTRL_GPU_ECC_UNIT_GSP` 0x1D, and the
//!   count 0x29 (ctrl2080gpu.h:1122-1125); the chip file lists the units it marks supported.
//!   UVM calls ECC on when every supported unit is enabled and at least one is supported
//!   (src/nvidia/src/kernel/rmapi/nv_gpu_ops.c:1358-1377).
//! * **Counters.** All zero: volatile (`ECC_GET_VOLATILE_COUNTS`, ctrl2080ecc.h:104-110),
//!   aggregate (`ECC_GET_CLIENT_EXPOSED_COUNTERS`, ctrl2080ecc.h:47-82) and per unit (the
//!   `dbe`/`sbe` counts of the status). A non-zero `dbe` makes UVM treat the channel as
//!   faulted (nv_gpu_ops.c:8008-8027), so zero is also the only safe answer.
//! * **Resets of the counters** (`GPU_RESET_ECC_ERROR_STATUS`, `nvidia-smi -p`): NV_OK, the
//!   counters are zero already.
//! * **Injection** (`NV208F_CTRL_CMD_FB_ECC_SET_KILL_PTR`, `_SET_WRITE_KILL`): refused; a
//!   model has no DRAM to corrupt. `ECC_INJECT_ERROR` and `ECC_INJECTION_SUPPORTED` stay with
//!   the absent `rm_test` family (chipctl.rs).
//! * **Row remapper** (`row_remap`): `FB_GET_REMAPPED_ROWS` entryCount 0, flags 0 (no remap,
//!   nothing pending, ctrl2080fb.h:1960-1974), `FB_GET_ROW_REMAPPER_HISTOGRAM` all banks in
//!   the "no remapped rows" class (ctrl2080fb.h:2361-2371), `FB_GET_REMAPPED_BANKS` and
//!   `FB_GET_OFFLINED_PAGES` empty, the NV208F clears NV_OK.

use serde::Deserialize;

use super::{id, off, reply, rd32, size, wr32, wr8, Families};
use crate::chip::ChipData;
use crate::ctrl::Answer;
use crate::ver::active::{NV_ERR_INVALID_ARGUMENT, NV_ERR_NOT_SUPPORTED, NV_OK};

/// `[ecc]` of chip.toml: the ECC facts of the board the chip stands for.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EccData {
    /// `NV2080_CTRL_GPU_ECC_UNIT_*` indices the status marks supported.
    pub units: Vec<u32>,
    /// The factory default (`defaultConfiguration`) and the mode of a fresh card.
    pub default_enabled: bool,
    /// Banks the row-remapper histogram counts, all in the "no remapped rows" class.
    pub remap_banks: u32,
    /// The InfoROM "ECC" object's version and subversion (`GPU_GET_INFOROM_OBJECT_VERSION`):
    /// NVML reports ECC unsupported when the object is missing (2026-09-25,
    /// caraxes/runs/limits-2026-09-25/ecc/gh100-1).
    pub inforom_ecc_object: [u8; 2],
    /// Where the facts come from (D9 label of the answers).
    pub label: String,
}

/// D9 labels.
pub const MODE: &str = "guest (ECC mode of this VM: current and pending, InfoROM analogue)";
pub const COUNTERS: &str = "synthesized (ECC counters: no errors in a model)";
pub const INJECT: &str = "refused (ECC error injection: a model has no DRAM to corrupt)";
pub const REMAP: &str = "synthesized (row remapper: empty tables, nothing pending)";

const STATUS: &str = "NV2080_CTRL_GPU_QUERY_ECC_STATUS_PARAMS";
const CONFIG: &str = "NV2080_CTRL_GPU_QUERY_ECC_CONFIGURATION_PARAMS";
const SET: &str = "NV2080_CTRL_GPU_SET_ECC_CONFIGURATION_PARAMS";
const HISTOGRAM: &str = "NV2080_CTRL_FB_GET_ROW_REMAPPER_HISTOGRAM_PARAMS";

/// `NV2080_CTRL_GPU_ECC_UNIT_COUNT` (ctrl2080gpu.h:1125): the size of `units[]`.
fn unit_count() -> usize {
    crate::ver::Abi::active()
        .layout(STATUS)
        .and_then(|l| l.fields.iter().find(|f| f.path == "units[].enabled"))
        .map_or(0, |f| f.dims[0].1)
}

fn is(cmd: u32, name: &str) -> bool {
    id(name) == Some(cmd)
}

/// The ECC and row-remapper answers, when the chip models the family.
pub fn answer(f: &mut Families, chip: &ChipData, cmd: u32, params: &[u8]) -> Option<Answer> {
    let e = chip.ecc.as_ref()?;
    let n = params.len();
    if super::modelled(chip, "ecc") {
        let mode = f.persist.ecc.unwrap_or_default();
        if is(cmd, "NV2080_CTRL_CMD_GPU_QUERY_ECC_STATUS") {
            let mut b = vec![0u8; n.max(size(STATUS))];
            let stride = off(STATUS, "units[1].enabled") - off(STATUS, "units[0].enabled");
            let mut listed = Vec::new();
            for &u in &e.units {
                if (u as usize) < unit_count() {
                    let o = off(STATUS, "units[0].enabled") + stride * u as usize;
                    wr8(&mut b, o, mode.current as u8);
                    wr8(&mut b, o + off(STATUS, "units[0].scrubComplete") - off(STATUS, "units[0].enabled"), 1);
                    wr8(&mut b, o + off(STATUS, "units[0].supported") - off(STATUS, "units[0].enabled"), 1);
                    listed.push(format!("{u:#x}"));
                }
            }
            // The request's grRouteInfo stays as sent.
            let route = off(STATUS, "grRouteInfo.flags");
            if n > route {
                b[route..n].copy_from_slice(&params[route..n]);
            }
            b.truncate(n);
            return Some(reply(
                NV_OK,
                b,
                MODE,
                format!(
                    "ecc status: units [{}] supported, enabled {}, all counts 0 ({})",
                    listed.join(", "),
                    mode.current,
                    e.label
                ),
            ));
        }
        if is(cmd, "NV2080_CTRL_CMD_GPU_QUERY_ECC_CONFIGURATION") {
            let mut b = vec![0u8; n.max(size(CONFIG))];
            wr32(&mut b, off(CONFIG, "currentConfiguration"), mode.pending as u32);
            wr32(&mut b, off(CONFIG, "defaultConfiguration"), e.default_enabled as u32);
            b.truncate(n);
            return Some(reply(
                NV_OK,
                b,
                MODE,
                format!(
                    "ecc configuration: stored (pending) {}, default {}, in force {}",
                    mode.pending, e.default_enabled, mode.current
                ),
            ));
        }
        if is(cmd, "NV2080_CTRL_CMD_GPU_SET_ECC_CONFIGURATION") {
            let new = rd32(params, off(SET, "newConfiguration"));
            if new > 1 {
                return Some(reply(
                    NV_ERR_INVALID_ARGUMENT,
                    params.to_vec(),
                    MODE,
                    format!("set ecc configuration: newConfiguration {new} is neither DISABLE 0 nor ENABLE 1 (ctrl2080gpu.h:1367-1368)"),
                ));
            }
            let m = f.persist.ecc.get_or_insert(super::Mode::new(e.default_enabled));
            m.pending = new == 1;
            let m = *m;
            f.save();
            return Some(reply(
                NV_OK,
                params.to_vec(),
                MODE,
                format!(
                    "set ecc configuration: pending {} (in force {} until the next GPU reset)",
                    m.pending, m.current
                ),
            ));
        }
        if is(cmd, "NV2080_CTRL_CMD_GPU_RESET_ECC_ERROR_STATUS") {
            return Some(reply(
                NV_OK,
                params.to_vec(),
                COUNTERS,
                format!(
                    "reset ecc error status: statuses {:#x}, the counters are 0 already",
                    rd32(params, 0)
                ),
            ));
        }
        // The InfoROM's ECC object ("ECC", ctrl2080gpu.h:1803-1815); other objects go on to
        // the host forward of nvml.rs.
        if is(cmd, "NV2080_CTRL_CMD_GPU_GET_INFOROM_OBJECT_VERSION")
            && params.get(..3) == Some(&b"ECC"[..])
        {
            let st = "NV2080_CTRL_GPU_GET_INFOROM_OBJECT_VERSION_PARAMS";
            let mut b = params.to_vec();
            let [ver, sub] = e.inforom_ecc_object;
            wr8(&mut b, off(st, "version"), ver);
            wr8(&mut b, off(st, "subversion"), sub);
            return Some(reply(
                NV_OK,
                b,
                MODE,
                format!("inforom object ECC: version {ver}.{sub} ({})", e.label),
            ));
        }
        if is(cmd, "NV2080_CTRL_CMD_GPU_QUERY_INFOROM_ECC_SUPPORT") {
            return Some(reply(
                NV_OK,
                params.to_vec(),
                MODE,
                "inforom ecc support: supported (the mode is kept in the persist record)".into(),
            ));
        }
        for name in [
            "NV2080_CTRL_CMD_ECC_GET_CLIENT_EXPOSED_COUNTERS",
            "NV2080_CTRL_CMD_ECC_GET_VOLATILE_COUNTS",
            "NV2080_CTRL_CMD_ECC_GET_SRAM_UNIQUE_UNCORR_COUNTS",
            "NV2080_CTRL_CMD_ECC_GET_REPAIR_STATUS",
            "NV2080_CTRL_CMD_ECC_GET_UNREPAIRABLE_MEMORY_FLAG",
        ] {
            if is(cmd, name) {
                return Some(reply(
                    NV_OK,
                    vec![0u8; n],
                    COUNTERS,
                    format!("{name}: all zero"),
                ));
            }
        }
        for name in [
            "NV208F_CTRL_CMD_FB_ECC_SET_KILL_PTR",
            "NV208F_CTRL_CMD_FB_ECC_SET_WRITE_KILL",
        ] {
            if is(cmd, name) {
                return Some(reply(
                    NV_ERR_NOT_SUPPORTED,
                    params.to_vec(),
                    INJECT,
                    format!("{name}: refused"),
                ));
            }
        }
    }
    if super::modelled(chip, "row_remap") {
        if is(cmd, "NV2080_CTRL_CMD_FB_GET_ROW_REMAPPER_HISTOGRAM") {
            let mut b = vec![0u8; n.max(size(HISTOGRAM))];
            wr32(&mut b, off(HISTOGRAM, "histogram[0]"), e.remap_banks);
            b.truncate(n);
            return Some(reply(
                NV_OK,
                b,
                REMAP,
                format!("row remapper histogram: {} banks without remapped rows", e.remap_banks),
            ));
        }
        for name in [
            "NV2080_CTRL_CMD_FB_GET_REMAPPED_ROWS",
            "NV2080_CTRL_CMD_FB_GET_REMAPPED_BANKS",
            "NV2080_CTRL_CMD_FB_GET_OFFLINED_PAGES",
            "NV2080_CTRL_CMD_FB_GET_DYNAMIC_OFFLINED_PAGES",
        ] {
            if is(cmd, name) {
                return Some(reply(NV_OK, vec![0u8; n], REMAP, format!("{name}: empty")));
            }
        }
        for name in [
            "NV208F_CTRL_CMD_FB_CLEAR_REMAPPED_ROWS",
            "NV208F_CTRL_CMD_FB_CLEAR_REMAPPED_BANKS",
        ] {
            if is(cmd, name) {
                return Some(reply(
                    NV_OK,
                    params.to_vec(),
                    REMAP,
                    format!("{name}: nothing to clear"),
                ));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctrl::{Dispatcher, INTERNAL_CLIENT, INTERNAL_SUBDEVICE};
    use crate::ver::active as v;

    fn chip(name: &str) -> &'static ChipData {
        crate::chip::by_name(name).unwrap().data()
    }

    fn dispatcher(name: &str) -> Dispatcher {
        let f = crate::fixture::rtx2070();
        Dispatcher::new(chip(name), &f.identity(4 << 30), None)
    }

    fn ctl(d: &mut Dispatcher, name: &str, params: &[u8]) -> Answer {
        d.control(INTERNAL_CLIENT, INTERNAL_SUBDEVICE, id(name).unwrap(), params)
    }

    fn status(d: &mut Dispatcher) -> Vec<u8> {
        let a = ctl(d, "NV2080_CTRL_CMD_GPU_QUERY_ECC_STATUS", &vec![0u8; size(STATUS)]);
        assert_eq!((a.status, a.label), (NV_OK, MODE));
        a.params
    }

    fn unit(b: &[u8], u: usize, field: &str) -> u8 {
        b[off(STATUS, &format!("units[{u}].{field}"))]
    }

    /// The GH100 rung: ECC on at power-on, the GSP unit supported and enabled, every other
    /// unit neither, every count zero.
    #[test]
    fn gh100_reports_ecc_on_with_zero_counts() {
        let mut d = dispatcher("gh100");
        let b = status(&mut d);
        for u in 0..unit_count() {
            let on = (u == 0x1D) as u8;
            assert_eq!(unit(&b, u, "supported"), on, "unit {u:#x}");
            assert_eq!(unit(&b, u, "enabled"), on, "unit {u:#x}");
            for c in ["dbe.count", "sbe.count"] {
                let o = off(STATUS, &format!("units[{u}].{c}"));
                assert_eq!(&b[o..o + 8], &[0u8; 8], "unit {u:#x} {c}");
            }
        }
        let a = ctl(&mut d, "NV2080_CTRL_CMD_GPU_QUERY_ECC_CONFIGURATION", &[0u8; 8]);
        assert_eq!(a.status, NV_OK);
        assert_eq!((rd32(&a.params, 0), rd32(&a.params, 4)), (1, 1));
        let a = ctl(&mut d, "NV2080_CTRL_CMD_GPU_QUERY_INFOROM_ECC_SUPPORT", &[]);
        assert_eq!(a.status, NV_OK);
        for name in [
            "NV2080_CTRL_CMD_ECC_GET_CLIENT_EXPOSED_COUNTERS",
            "NV2080_CTRL_CMD_ECC_GET_VOLATILE_COUNTS",
            "NV2080_CTRL_CMD_ECC_GET_REPAIR_STATUS",
        ] {
            let a = ctl(&mut d, name, &[0xAAu8; 96]);
            assert_eq!((a.status, a.label), (NV_OK, COUNTERS), "{name}");
            assert!(a.params.iter().all(|&x| x == 0), "{name}");
        }
        // The memsys static config carries the mode in force (bEnabledEccFBPA).
        let a = d.control(
            INTERNAL_CLIENT,
            INTERNAL_SUBDEVICE,
            v::CMD_INTERNAL_MEMSYS_GET_STATIC_CONFIG,
            &[0u8; 40],
        );
        let o = off("NV2080_CTRL_INTERNAL_MEMSYS_GET_STATIC_CONFIG_PARAMS", "bEnabledEccFBPA");
        assert_eq!(a.params[o], 1);
    }

    /// `-e 0`: the stored (pending) setting changes at once, the mode in force at the next
    /// GPU reset; an invalid setting is refused.
    #[test]
    fn set_is_pending_until_a_reset() {
        let mut d = dispatcher("gh100");
        let a = ctl(&mut d, "NV2080_CTRL_CMD_GPU_SET_ECC_CONFIGURATION", &0u32.to_le_bytes());
        assert_eq!(a.status, NV_OK);
        let a = ctl(&mut d, "NV2080_CTRL_CMD_GPU_QUERY_ECC_CONFIGURATION", &[0u8; 8]);
        assert_eq!((rd32(&a.params, 0), rd32(&a.params, 4)), (0, 1));
        assert_eq!(unit(&status(&mut d), 0x1D, "enabled"), 1);
        let a = ctl(&mut d, "NV2080_CTRL_CMD_GPU_SET_ECC_CONFIGURATION", &2u32.to_le_bytes());
        assert_eq!(a.status, NV_ERR_INVALID_ARGUMENT);
        // The reset (BootModel::reset makes a fresh dispatcher that takes the record over).
        let mut fresh = dispatcher("gh100");
        fresh.families.after_reset(&d.families);
        let b = status(&mut fresh);
        assert_eq!((unit(&b, 0x1D, "supported"), unit(&b, 0x1D, "enabled")), (1, 0));
        let a = ctl(&mut fresh, "NV2080_CTRL_CMD_GPU_QUERY_ECC_CONFIGURATION", &[0u8; 8]);
        assert_eq!(rd32(&a.params, 0), 0);
        assert!(!fresh.families.ecc_enabled());
    }

    /// Injection is refused, the row remapper's tables are empty.
    #[test]
    fn injection_refused_and_remapper_empty() {
        let mut d = dispatcher("gh100");
        let a = ctl(&mut d, "NV208F_CTRL_CMD_FB_ECC_SET_KILL_PTR", &[0u8; 16]);
        assert_eq!((a.status, a.label), (NV_ERR_NOT_SUPPORTED, INJECT));
        let rows = "NV2080_CTRL_FB_GET_REMAPPED_ROWS_PARAMS";
        let mut req = vec![0xEEu8; size(rows)];
        wr32(&mut req, off(rows, "entryCount"), 7);
        let a = ctl(&mut d, "NV2080_CTRL_CMD_FB_GET_REMAPPED_ROWS", &req);
        assert_eq!((a.status, a.label), (NV_OK, REMAP));
        assert_eq!(rd32(&a.params, off(rows, "entryCount")), 0);
        assert_eq!(a.params[off(rows, "flags")], 0);
        let a = ctl(&mut d, "NV2080_CTRL_CMD_FB_GET_ROW_REMAPPER_HISTOGRAM", &[0xFFu8; 20]);
        assert_eq!((a.status, &a.params[..]), (NV_OK, &[0u8; 20][..]));
    }

    /// TU106 has neither family: the status stays the host forward (without a host, the
    /// nvml fallback), the setter the chipctl refusal.
    #[test]
    fn tu106_is_untouched() {
        let mut d = dispatcher("tu106");
        let a = ctl(&mut d, "NV2080_CTRL_CMD_GPU_QUERY_ECC_STATUS", &vec![0u8; size(STATUS)]);
        assert_eq!(a.status, NV_ERR_NOT_SUPPORTED);
        assert_ne!(a.label, MODE);
        let a = ctl(&mut d, "NV2080_CTRL_CMD_GPU_SET_ECC_CONFIGURATION", &[0u8; 4]);
        assert_eq!((a.status, a.label), (NV_ERR_NOT_SUPPORTED, crate::chipctl::REFUSED));
        assert!(!d.families.ecc_enabled());
    }
}
