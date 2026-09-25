// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! The parts of the 615 boot exchange beyond the queues and `GSP_INIT`'s static
//! configuration (docs/caraxes/gsp-plan/boot-exchange.md §3-5): the registry keys the
//! device honours (B3), the unstable-interface opt-in (B7), the `GSP_SUSPEND` flags and
//! the resume arguments (B5).
//!
//! Paths are relative to the open-gpu-kernel-modules tree at tag 615.71.09.

use crate::gmcapi::RegValue;
use crate::guestmem::GuestMemory;

// ---------------------------------------------------------------------------------
// B3: registry keys (the REGKEY_* keys of GSP_INIT, kernel_gsp.c:4893-4958).

/// What the registry changes in the device. Every key is traced; these are honoured:
///
/// - `RmLogonRC` (`NV_REG_STR_RM_DO_LOG_RC_EVENTS`, default 1, "0 --> Skip Logging",
///   src/nvidia/interface/nvrm_registry.h:1658-1662): with 0 the device sends an RC
///   without the Xid line (`RC_TRIGGERED` alone, no `OS_ERROR_LOG`; events.rs), the
///   policy RM applies to its own Xid output (`bLogEvents`, kernel_rc.c:311-315);
///   GSP-RM's use of the key is not in the open tree (inferred).
/// - `RMSplitVasMgmtServerClientRm` (default: split VA management on): with 0 the kernel
///   sends `MAP_MEMORY_DMA`/`UNMAP_MEMORY_DMA` (gpu_registry.c:170-181,
///   virtual_mem.c:462-472), which the device refuses (gsprpc.rs); the init notes it.
/// - `EnableGpuFirmwareLogs` (default on): with 0 the device writes no GSP-RM log lines
///   (B6, `LOGRM`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegistryPolicy {
    pub log_on_rc: bool,
    pub split_vas: bool,
    pub firmware_logs: bool,
}

impl Default for RegistryPolicy {
    fn default() -> Self {
        RegistryPolicy {
            log_on_rc: true,
            split_vas: true,
            firmware_logs: true,
        }
    }
}

/// The policy of a registry, and one note per honoured key it sets.
pub fn registry_policy(entries: &[(String, RegValue)]) -> (RegistryPolicy, Vec<String>) {
    let mut p = RegistryPolicy::default();
    let mut notes = Vec::new();
    for (name, value) in entries {
        let RegValue::U32(v) = value else { continue };
        let on = *v != 0;
        match name.as_str() {
            "RmLogonRC" => {
                p.log_on_rc = on;
                notes.push(format!(
                    "registry RmLogonRC = {v}: honoured, an RC {} the Xid line (OS_ERROR_LOG)",
                    if on { "carries" } else { "goes without" }
                ));
            }
            "RMSplitVasMgmtServerClientRm" => {
                p.split_vas = on;
                notes.push(format!(
                    "registry RMSplitVasMgmtServerClientRm = {v}: honoured, {}",
                    if on {
                        "split VA management (the device's model)"
                    } else {
                        "WARNING the kernel will send MAP/UNMAP_MEMORY_DMA, which the device refuses"
                    }
                ));
            }
            "EnableGpuFirmwareLogs" => {
                p.firmware_logs = on;
                notes.push(format!(
                    "registry EnableGpuFirmwareLogs = {v}: honoured, GSP-RM log lines {}",
                    if on { "written" } else { "not written" }
                ));
            }
            _ => {}
        }
    }
    (p, notes)
}

/// The guest registry keys the device's model depends on: the module options of
/// docs/caraxes/quickstart.md section 2 step 3, which every Caraxes guest carries in
/// `/etc/modprobe.d/caraxes.conf`. A guest without them fails `RmInitAdapter` in pure
/// emulation, and the failure looked like a device regression on 2026-09-24
/// (docs/caraxes/blackwell-gate.md, "Pure emulation regressions of 2026-09-24"). One
/// warning per key that is absent or has another value; paths in the 610.57.04 tree.
///
/// - `RmDisableGlobalCeUtils` = 1 (`NV_REG_STR_DISABLE_GLOBAL_CE_UTILS_YES`,
///   interface/nvrm_registry.h:975-977, read at mem_mgr.c:331-335). Without it the kernel
///   creates its global CeUtils and tests it with a CE memset (mem_mgr.c:445), whose
///   channel the device does not execute (no VA space root, channel.rs): the memset times
///   out and `memmgrInitCeUtils` fails (mem_mgr.c:508), `RmInitAdapter` 0x25:0x65.
/// - `RmRcWatchdog` = 0 (`NV_REG_STR_RM_RC_WATCHDOG_DISABLE`, nvrm_registry.h:1584-1587,
///   default enabled; read at kernel_rc.c:198-203). Without it `krcWatchdogInit` allocates
///   a GR object, which needs a GPC in the floorsweeping masks
///   (kernel_graphics_object.c:190-192); pure emulation answers the masks with zero.
/// - `RMDisableScrubOnFree` nonzero (nvrm_registry.h:926, mem_mgr.c:204-208). Without it
///   freed video memory is scrubbed through the copy engine.
pub fn missing_guest_keys(entries: &[(String, RegValue)]) -> Vec<String> {
    let get = |k: &str| {
        entries.iter().find_map(|(n, v)| match v {
            RegValue::U32(x) if n == k => Some(*x),
            _ => None,
        })
    };
    let mut out = Vec::new();
    let mut want = |key: &str, ok: bool, have: Option<u32>, need: &str, why: &str| {
        if !ok {
            let have = have.map_or("absent".to_string(), |x| format!("{x}"));
            out.push(format!(
                "WARNING guest registry {key} is {have}, the device needs {need} (docs/caraxes/quickstart.md section 2): {why}"
            ));
        }
    };
    let ce = get("RmDisableGlobalCeUtils");
    want(
        "RmDisableGlobalCeUtils",
        ce == Some(1),
        ce,
        "1",
        "the kernel's CeUtils memset will time out (mem_mgr.c:445) and RmInitAdapter fail",
    );
    let wd = get("RmRcWatchdog");
    want(
        "RmRcWatchdog",
        wd == Some(0),
        wd,
        "0",
        "krcWatchdogInit allocates a GR object, which needs a GPC in the floorsweeping masks (kernel_graphics_object.c:192), and pure emulation answers them with zero",
    );
    let sc = get("RMDisableScrubOnFree");
    want(
        "RMDisableScrubOnFree",
        sc.is_some_and(|x| x != 0),
        sc,
        "nonzero",
        "freed video memory will be scrubbed through the copy engine",
    );
    out
}

// ---------------------------------------------------------------------------------
// B7: NVGMC_SI_UNSTABLE_INTERFACE_SUPPORT (interface/gmcapi/gmcapi_system_info.h:200-203),
// RPC 0:0 and EVENTS 1:1; the 615 kernel sets both (kernel_gsp.c:5203-5206).

/// What the client opted in to: RM RPCs, RM events. A client without the key opted in
/// to neither: it speaks `GSP_INIT` and the other GMCAPI commands only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unstable {
    pub rpc: bool,
    pub events: bool,
}

impl Unstable {
    pub fn from_key(v: Option<u32>) -> Self {
        let v = v.unwrap_or(0);
        Unstable {
            rpc: v & 1 != 0,
            events: v & 2 != 0,
        }
    }
}

// ---------------------------------------------------------------------------------
// B5: GSP_SUSPEND (interface/gmcapi/gmcapi_gsp.h:39-56) and the resume arguments.

pub const SUSPEND_PM_TRANSITION: u64 = 1 << 0;
pub const SUSPEND_GC6_ENTERING: u64 = 1 << 1;
pub const SUSPEND_HIBERNATE: u64 = 1 << 2;
pub const SUSPEND_FAST: u64 = 1 << 3;
pub const SUSPEND_FORCE: u64 = 1 << 4;

/// `GmcApiGspSuspendParams.flags`; 0 for a short payload.
pub fn suspend_flags(payload: &[u8]) -> u64 {
    payload
        .get(..8)
        .map_or(0, |b| u64::from_le_bytes(b.try_into().unwrap()))
}

pub fn describe_suspend(flags: u64) -> String {
    let names: Vec<&str> = [
        (SUSPEND_PM_TRANSITION, "PM_TRANSITION"),
        (SUSPEND_GC6_ENTERING, "GC6_ENTERING"),
        (SUSPEND_HIBERNATE, "HIBERNATE"),
        (SUSPEND_FAST, "FAST"),
        (SUSPEND_FORCE, "FORCE"),
    ]
    .iter()
    .filter(|(b, _)| flags & b != 0)
    .map(|&(_, n)| n)
    .collect();
    if names.is_empty() {
        format!("{flags:#x} (a regular unload: the next boot starts from construction)")
    } else {
        format!(
            "{flags:#x} {}{}",
            names.join("|"),
            if flags & SUSPEND_PM_TRANSITION != 0 {
                " (a PM transition: the device keeps every object and the queues for the resume)"
            } else {
                ""
            }
        )
    }
}

/// `GSP_ARGUMENTS_FLAG_IN_PM_TRANSITION` (src/nvidia/inc/kernel/gpu/gsp/gsp_init_args.h:67)
/// and `srInitArguments` {oldLevel, flags} after the queue arguments (:50-55, 75), set
/// for a resume (kernel_gsp.c:6192-6197).
pub const ARGS_FLAG_IN_PM_TRANSITION: u64 = 0x01;
pub const ARGS_SR_OLD_LEVEL: u64 = crate::msgq::v2::ARGS_MQ + 64;
pub const ARGS_SR_FLAGS: u64 = ARGS_SR_OLD_LEVEL + 4;

/// The resume facts of the RMARGS at `pa`: (IN_PM_TRANSITION, oldLevel, flags).
pub fn resume_args(mem: &GuestMemory, rmargs_pa: u64) -> Option<(bool, u32, u32)> {
    let flags = mem.read_u64(rmargs_pa + crate::msgq::v2::ARGS_FLAGS)?;
    Some((
        flags & ARGS_FLAG_IN_PM_TRANSITION != 0,
        mem.read_u32(rmargs_pa + ARGS_SR_OLD_LEVEL)?,
        mem.read_u32(rmargs_pa + ARGS_SR_FLAGS)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_keys_that_change_the_device() {
        let (p, notes) = registry_policy(&[]);
        assert_eq!((p, notes.len()), (RegistryPolicy::default(), 0));
        let (p, notes) = registry_policy(&[
            ("RmLogonRC".into(), RegValue::U32(0)),
            ("RmRcWatchdog".into(), RegValue::U32(0)),
            ("RMSplitVasMgmtServerClientRm".into(), RegValue::U32(0)),
            ("EnableGpuFirmwareLogs".into(), RegValue::U32(2)),
        ]);
        assert!(!p.log_on_rc && !p.split_vas && p.firmware_logs);
        assert_eq!(notes.len(), 3, "RmRcWatchdog is traced only: {notes:?}");
    }

    /// The pure-emulation boots of 2026-09-24 that failed at mem_mgr.c:445 and
    /// kernel_graphics_object.c:192 ran a guest without the three keys; the quickstart's
    /// registry string gives no warning (docs/caraxes/blackwell-gate.md section 5).
    #[test]
    fn guest_keys_the_device_needs() {
        let w = missing_guest_keys(&[]);
        assert_eq!(w.len(), 3, "{w:?}");
        assert!(w[0].contains("RmDisableGlobalCeUtils is absent") && w[0].contains("mem_mgr.c:445"));
        assert!(w[1].contains("RmRcWatchdog is absent") && w[1].contains("kernel_graphics_object.c:192"));
        let quickstart: Vec<(String, RegValue)> = vec![
            ("RmDisableGlobalCeUtils".into(), RegValue::U32(1)),
            ("RMDisableScrubOnFree".into(), RegValue::U32(1)),
            ("RmRcWatchdog".into(), RegValue::U32(0)),
            ("ResmanDebugLevel".into(), RegValue::U32(0)),
        ];
        assert!(missing_guest_keys(&quickstart).is_empty());
        let mut on = quickstart.clone();
        on[2].1 = RegValue::U32(1);
        let w = missing_guest_keys(&on);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].starts_with("WARNING guest registry RmRcWatchdog is 1, the device needs 0"));
        // A string value is not the DWORD the kernel reads.
        on[2].1 = RegValue::String("0".into());
        assert_eq!(missing_guest_keys(&on).len(), 1);
    }

    #[test]
    fn opt_in_and_suspend_flags() {
        assert_eq!(Unstable::from_key(Some(3)), Unstable { rpc: true, events: true });
        assert_eq!(Unstable::from_key(None), Unstable { rpc: false, events: false });
        assert_eq!(Unstable::from_key(Some(1)), Unstable { rpc: true, events: false });
        assert_eq!(suspend_flags(&5u64.to_le_bytes()), 5);
        assert!(describe_suspend(5).contains("PM_TRANSITION|HIBERNATE"));
        assert!(describe_suspend(0).contains("regular unload"));
        assert_eq!(ARGS_SR_OLD_LEVEL, 80);
    }
}
