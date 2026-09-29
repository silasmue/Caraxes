// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The RPC functions a GSP client sends only on some paths
//! (docs/caraxes/gsp-plan/rpc-functions.md §4.2, §4.3): `IDLE_CHANNELS`,
//! `GPU_EXEC_REG_OPS`, `DUMP_PROTOBUF_COMPONENT`, `ECC_NOTIFIER_WRITE_ACK`,
//! `MAP_MEMORY_DMA`, `UNMAP_MEMORY_DMA`, and `UNLOADING_GUEST_DRIVER` of older clients.
//!
//! Paths are relative to the open-gpu-kernel-modules tree at tag 615.71.09; the
//! structures are the same in 610.57.04 (rpc-functions.md §1). What the boot model
//! needs from its own state (the channel registry, the chip's identity registers) comes
//! in as closures, so these answers never reach a host register or host object.

// RPC function numbers (src/nvidia/inc/kernel/vgpu/rpc_global_enums.h:24-25, 32, 57, 60,
// 88, 212 of 615; the same in 610).
pub const FN_MAP_MEMORY_DMA: u32 = 14;
pub const FN_UNMAP_MEMORY_DMA: u32 = 15;
pub const FN_IDLE_CHANNELS: u32 = 22;
pub const FN_UNLOADING_GUEST_DRIVER: u32 = 47;
pub const FN_GPU_EXEC_REG_OPS: u32 = 50;
pub const FN_DUMP_PROTOBUF_COMPONENT: u32 = 78;
pub const FN_ECC_NOTIFIER_WRITE_ACK: u32 = 202;

pub fn function_name(f: u32) -> Option<&'static str> {
    Some(match f {
        FN_MAP_MEMORY_DMA => "MAP_MEMORY_DMA",
        FN_UNMAP_MEMORY_DMA => "UNMAP_MEMORY_DMA",
        FN_IDLE_CHANNELS => "IDLE_CHANNELS",
        FN_UNLOADING_GUEST_DRIVER => "UNLOADING_GUEST_DRIVER",
        FN_GPU_EXEC_REG_OPS => "GPU_EXEC_REG_OPS",
        FN_DUMP_PROTOBUF_COMPONENT => "DUMP_PROTOBUF_COMPONENT",
        FN_ECC_NOTIFIER_WRITE_ACK => "ECC_NOTIFIER_WRITE_ACK",
        _ => return None,
    })
}

use crate::gspmem::{NV_ERR_INVALID_ARGUMENT, NV_ERR_INVALID_OBJECT_HANDLE, NV_ERR_NOT_SUPPORTED, NV_OK};
/// `NV_ERR_TIMEOUT` (nvstatuscodes.h:130).
pub const NV_ERR_TIMEOUT: u32 = 0x65;

fn u32_at(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4)
        .map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

fn put32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

// ---------------------------------------------------------------------------------
// IDLE_CHANNELS: rpc_idle_channels_v03_00 {flags, timeout, nchannels,
// idle_channel_list_v03_00 {phClient, phDevice, phChannel}[]}
// (src/nvidia/generated/g_rpc-structures.h:207-213, g_sdk-structures.h:1403-1408).

pub const IDLE_FLAGS: usize = 0;
pub const IDLE_TIMEOUT: usize = 4;
pub const IDLE_NCHANNELS: usize = 8;
pub const IDLE_LIST: usize = 12;
pub const IDLE_ENTRY: usize = 12;

/// The longest the device waits for mirrored channels to drain, whatever the request's
/// timeout: the wait holds the service thread (the software executor cannot run in
/// the meantime, and needs not: it runs to completion at every doorbell).
pub const IDLE_WAIT_CAP: std::time::Duration = std::time::Duration::from_millis(50);

/// One channel's state as the registry and its USERD show it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelIdle {
    pub chid: u32,
    pub mirrored: bool,
    pub gp_get: u32,
    pub gp_put: u32,
}

/// `IDLE_CHANNELS` (sender rpc.c:4488, from kfifoIdleChannelsPerDevice_KERNEL,
/// kernel_idle_channels.c:44): every listed channel must be a channel of this device.
/// `probe` resolves (hClient, hChannel) through the channel registry and reads USERD;
/// unknown channels are `NV_ERR_INVALID_OBJECT_HANDLE` (0x33), whatever else the list
/// holds, and nothing is asked of the host then.
///
/// * A software channel is idle when its USERD GPGet caught up with GPPut (the executor
///   runs a push to completion at its doorbell; a channel still behind is busy at once).
/// * The channels that run on a host mirror go to `host_idle` in one call, with the
///   request's flags and the timeout capped at [`IDLE_WAIT_CAP`] (the call holds the
///   service thread): the host RM's own idle (`NV0080_CTRL_CMD_FIFO_IDLE_CHANNELS` on the
///   device's own host device object, over the mirror's own host channel handles only,
///   mirror.rs `Mirror::idle_channels`). Its `NV_OK` means idle, its `NV_ERR_TIMEOUT`
///   is the guest's `NV_ERR_TIMEOUT`. When the mirror has no host object for them
///   (`None`) or the host refuses the call, the device polls their USERD instead for
///   `min(timeout, IDLE_WAIT_CAP)`.
pub fn idle_channels(
    payload: &[u8],
    mut probe: impl FnMut(u32, u32) -> Option<ChannelIdle>,
    mut host_idle: impl FnMut(&[(u32, u32)], u32, u32) -> Option<(u32, String)>,
) -> (u32, String) {
    let n = u32_at(payload, IDLE_NCHANNELS) as usize;
    if payload.len() < IDLE_LIST + n * IDLE_ENTRY {
        return (NV_ERR_INVALID_ARGUMENT, format!("idle channels: {n} channels in {} bytes", payload.len()));
    }
    let (flags, timeout_us) = (u32_at(payload, IDLE_FLAGS), u32_at(payload, IDLE_TIMEOUT));
    let list: Vec<(u32, u32)> = (0..n)
        .map(|i| {
            let e = IDLE_LIST + i * IDLE_ENTRY;
            (u32_at(payload, e), u32_at(payload, e + 8))
        })
        .collect();
    let limit = std::time::Duration::from_micros(timeout_us as u64).min(IDLE_WAIT_CAP);
    let start = std::time::Instant::now();
    // Resolve everything first: an unknown handle fails the call before any host call.
    let mut mirrored = Vec::new();
    for &(c, h) in &list {
        match probe(c, h) {
            None => {
                return (
                    NV_ERR_INVALID_OBJECT_HANDLE,
                    format!("idle channels flags {flags:#x}: {c:#x}/{h:#x} is no channel of this device"),
                )
            }
            Some(s) if s.mirrored => mirrored.push((c, h)),
            Some(_) => {}
        }
    }
    let mut host_note = String::new();
    let mut host_idle_ok = false;
    if !mirrored.is_empty() {
        let capped = limit.as_micros().min(u32::MAX as u128) as u32;
        match host_idle(&mirrored, flags, capped) {
            Some((NV_OK, note)) => {
                host_idle_ok = true;
                host_note = format!("; host idle: {note}");
            }
            Some((NV_ERR_TIMEOUT, note)) => {
                return (
                    NV_ERR_TIMEOUT,
                    format!(
                        "idle channels flags {flags:#x} timeout {timeout_us} us: host idle of {} mirror channel(s) timed out after {:?} (capped at {capped} us): {note}",
                        mirrored.len(),
                        start.elapsed()
                    ),
                )
            }
            Some((st, note)) => host_note = format!("; host idle refused {st:#x} ({note}), USERD polled instead"),
            None => host_note = "; no host mirror object, USERD polled".into(),
        }
    }
    loop {
        let mut busy = Vec::new();
        for &(c, h) in &list {
            match probe(c, h) {
                None => {
                    return (
                        NV_ERR_INVALID_OBJECT_HANDLE,
                        format!("idle channels flags {flags:#x}: {c:#x}/{h:#x} is no channel of this device"),
                    )
                }
                Some(s) if s.mirrored && host_idle_ok => {}
                Some(s) if s.gp_get != s.gp_put => busy.push(s),
                Some(_) => {}
            }
        }
        let waited = start.elapsed();
        if busy.is_empty() {
            return (
                NV_OK,
                format!(
                    "idle channels flags {flags:#x} timeout {timeout_us} us: {n} channel(s) idle after {waited:?} ({} on the host) [guest]{host_note}",
                    mirrored.len()
                ),
            );
        }
        // Only a mirrored channel can still move while this thread waits.
        if waited >= limit || busy.iter().all(|s| !s.mirrored) {
            let names: Vec<String> = busy
                .iter()
                .map(|s| format!("chid {} GPGet {} GPPut {}{}", s.chid, s.gp_get, s.gp_put, if s.mirrored { " (host mirror)" } else { "" }))
                .collect();
            return (
                NV_ERR_TIMEOUT,
                format!("idle channels flags {flags:#x} timeout {timeout_us} us: not idle after {waited:?}: {}{host_note}", names.join(", ")),
            );
        }
        std::thread::sleep(std::time::Duration::from_micros(200));
    }
}

// ---------------------------------------------------------------------------------
// GPU_EXEC_REG_OPS: rpc_gpu_exec_reg_ops_v12_01 {hClient, hObject,
// gpu_exec_reg_ops_v12_01 {NV2080_CTRL_GPU_EXEC_REG_OPS_PARAMS_v12_01 (48 bytes),
// NV2080_CTRL_GPU_REG_OP_v03_00 operations[]}} (g_rpc-structures.h:342-347,
// g_sdk-structures.h:389-397, 423-436, 1395-1399).

pub const REGOPS_COUNT: usize = 8 + 20;
pub const REGOPS_OPERATIONS: usize = 56;
pub const REGOP_SIZE: usize = 32;
pub const REGOP_OP: usize = 0;
pub const REGOP_TYPE: usize = 1;
pub const REGOP_STATUS: usize = 2;
pub const REGOP_OFFSET: usize = 12;
pub const REGOP_VALUE_HI: usize = 16;
pub const REGOP_VALUE_LO: usize = 20;
/// `NV2080_CTRL_GPU_REG_OP_READ_32` and `_TYPE_GLOBAL`, the statuses
/// (src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080gpu.h:665-690).
pub const REG_OP_READ_32: u8 = 0;
pub const REG_OP_TYPE_GLOBAL: u8 = 0;
pub const REG_OP_STATUS_SUCCESS: u8 = 0x00;
pub const REG_OP_STATUS_NOACCESS: u8 = 0x20;

/// `GPU_EXEC_REG_OPS` (sender rpc.c:9394-9470, from subdeviceCtrlCmdGpuExecRegOps_cmn,
/// subdevice_ctrl_gpu_regops.c:224). Never forwarded to the physical GPU: a 32-bit read
/// of a global register that `read` (the device's own register model) answers succeeds
/// with the model's value; every other operation, and every write, is
/// `NV2080_CTRL_GPU_REG_OP_STATUS_NOACCESS` with the call `NV_OK`, which is how RM
/// reports a denied register (the kernel copies regStatus and the values back,
/// rpc.c:9458-9467; the call's status is `rpc_result_private`, :9454).
pub fn exec_reg_ops(payload: &[u8], read: impl Fn(u32) -> Option<u32>) -> (u32, Vec<u8>, String) {
    let count = u32_at(payload, REGOPS_COUNT) as usize;
    if count == 0 || payload.len() < REGOPS_OPERATIONS + count * REGOP_SIZE {
        return (
            NV_ERR_INVALID_ARGUMENT,
            payload.to_vec(),
            format!("reg ops: {count} operation(s) in {} bytes", payload.len()),
        );
    }
    let mut out = payload.to_vec();
    let (mut ok, mut denied) = (0, 0);
    for i in 0..count {
        let o = REGOPS_OPERATIONS + i * REGOP_SIZE;
        let (op, ty, off) = (out[o + REGOP_OP], out[o + REGOP_TYPE], u32_at(&out, o + REGOP_OFFSET));
        let value = (op == REG_OP_READ_32 && ty == REG_OP_TYPE_GLOBAL)
            .then(|| read(off))
            .flatten();
        match value {
            Some(v) => {
                out[o + REGOP_STATUS] = REG_OP_STATUS_SUCCESS;
                put32(&mut out, o + REGOP_VALUE_HI, 0);
                put32(&mut out, o + REGOP_VALUE_LO, v);
                ok += 1;
            }
            None => {
                out[o + REGOP_STATUS] = REG_OP_STATUS_NOACCESS;
                denied += 1;
            }
        }
    }
    (
        NV_OK,
        out,
        format!("reg ops: {count} operation(s), {ok} read from the device's register model, {denied} NOACCESS; nothing reaches the physical GPU [synthesized]"),
    )
}

// ---------------------------------------------------------------------------------
// DUMP_PROTOBUF_COMPONENT: rpc_dump_protobuf_component_v18_12 {u16 component,
// u8 nvDumpType, NvBool countOnly, bugCheckCode, internalCode, bufferSize, blob[]}
// (g_rpc-structures.h:1468-1477).

pub const DUMP_COMPONENT: usize = 0;
pub const DUMP_BUFFER_SIZE: usize = 12;
pub const DUMP_FIXED: usize = 16;

/// `DUMP_PROTOBUF_COMPONENT` (sender rpc.c:10543-10580, for nvidia-bug-report's NVDUMP
/// and RC journal dumps): `NV_OK` with `bufferSize` 0, an empty blob, so the kernel adds
/// nothing (rpc.c:10575-10577). Never the host's dump: it names host clients and
/// processes.
pub fn dump_protobuf_component(payload: &[u8]) -> (u32, Vec<u8>, String) {
    let mut out = payload[..payload.len().min(DUMP_FIXED)].to_vec();
    out.resize(DUMP_FIXED, 0);
    let asked = u32_at(&out, DUMP_BUFFER_SIZE);
    put32(&mut out, DUMP_BUFFER_SIZE, 0);
    let component = u16::from_le_bytes([out[DUMP_COMPONENT], out[DUMP_COMPONENT + 1]]);
    (
        NV_OK,
        out,
        format!("dump protobuf component {component:#x}: empty blob (room for {asked} bytes; the device keeps no GSP-side dump) [synthesized]"),
    )
}

/// `MAP_MEMORY_DMA` and `UNMAP_MEMORY_DMA` (senders rpc.c:4350, 4379): only without
/// split VA management, which a GSP client enables unless the registry key
/// `RMSplitVasMgmtServerClientRm` is 0 (gpu_registry.c:170-181, virtual_mem.c:462-472).
/// The device relies on the kernel owning the page tables: refused.
pub fn map_memory_dma(function: u32) -> (u32, String) {
    (
        NV_ERR_NOT_SUPPORTED,
        format!(
            "{}: refused: the device serves split VA management only (the guest set RMSplitVasMgmtServerClientRm=0?) [synthesized]",
            function_name(function).unwrap_or("?")
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle_request(timeout: u32, list: &[(u32, u32)]) -> Vec<u8> {
        let mut p = vec![0u8; IDLE_LIST + IDLE_ENTRY * list.len()];
        put32(&mut p, IDLE_TIMEOUT, timeout);
        put32(&mut p, IDLE_NCHANNELS, list.len() as u32);
        for (i, (c, h)) in list.iter().enumerate() {
            put32(&mut p, IDLE_LIST + i * IDLE_ENTRY, *c);
            put32(&mut p, IDLE_LIST + i * IDLE_ENTRY + 8, *h);
        }
        p
    }

    /// No host mirror object for anything: the host is never asked.
    fn no_host(_: &[(u32, u32)], _: u32, _: u32) -> Option<(u32, String)> {
        None
    }

    #[test]
    fn idle_channels_follows_userd() {
        let idle = |_: u32, h: u32| Some(ChannelIdle { chid: h, mirrored: false, gp_get: 3, gp_put: 3 });
        assert_eq!(idle_channels(&idle_request(1000, &[(1, 2), (1, 3)]), idle, no_host).0, NV_OK);
        let unknown = |_: u32, _: u32| None;
        assert_eq!(idle_channels(&idle_request(1000, &[(1, 2)]), unknown, no_host).0, NV_ERR_INVALID_OBJECT_HANDLE);
        // A software channel that did not drain stays busy: no wait.
        let stuck = |_: u32, h: u32| Some(ChannelIdle { chid: h, mirrored: false, gp_get: 2, gp_put: 3 });
        let t = std::time::Instant::now();
        assert_eq!(idle_channels(&idle_request(10_000_000, &[(1, 2)]), stuck, no_host).0, NV_ERR_TIMEOUT);
        assert!(t.elapsed() < IDLE_WAIT_CAP);
        // A mirrored channel without a host object is asked again until it drains.
        let mut calls = 0;
        let draining = |_: u32, h: u32| {
            calls += 1;
            Some(ChannelIdle { chid: h, mirrored: true, gp_get: calls.min(3), gp_put: 3 })
        };
        let (st, note) = idle_channels(&idle_request(10_000_000, &[(1, 2)]), draining, no_host);
        assert_eq!(st, NV_OK, "{note}");
        assert_eq!(idle_channels(&idle_request(8, &[]), idle, no_host).0, NV_OK);
        assert_eq!(idle_channels(&idle_request(8, &[(1, 2)])[..16], idle, no_host).0, NV_ERR_INVALID_ARGUMENT);
    }

    /// Item 4 of the GSP plan round 2: the mirrored channels of the list go to the host
    /// idle in one call (only those, with the flags and the capped timeout); the host's
    /// NV_OK makes them idle whatever their USERD says, its NV_ERR_TIMEOUT is the guest's,
    /// any other host status falls back to polling; an unknown channel keeps 0x33 and the
    /// host is not asked.
    #[test]
    fn idle_channels_asks_the_host_for_mirror_channels() {
        // chid 2 software (idle), chid 3 mirrored and still busy in USERD.
        let probe = |_: u32, h: u32| match h {
            2 => Some(ChannelIdle { chid: 2, mirrored: false, gp_get: 5, gp_put: 5 }),
            3 => Some(ChannelIdle { chid: 3, mirrored: true, gp_get: 1, gp_put: 4 }),
            _ => None,
        };
        let mut req = idle_request(10_000_000, &[(1, 2), (1, 3)]);
        put32(&mut req, IDLE_FLAGS, 0x42);
        let mut asked = Vec::new();
        let (st, note) = idle_channels(&req, probe, |l: &[(u32, u32)], f, t| {
            asked.push((l.to_vec(), f, t));
            Some((NV_OK, "host NV_OK".into()))
        });
        assert_eq!(st, NV_OK, "{note}");
        assert_eq!(asked, [(vec![(1, 3)], 0x42, IDLE_WAIT_CAP.as_micros() as u32)]);
        // A short request keeps its own timeout; the host's timeout is the guest's.
        let mut asked = Vec::new();
        let (st, _) = idle_channels(&idle_request(700, &[(1, 3)]), probe, |_: &[(u32, u32)], _, t| {
            asked.push(t);
            Some((NV_ERR_TIMEOUT, "host timeout".into()))
        });
        assert_eq!((st, asked), (NV_ERR_TIMEOUT, vec![700]));
        // A refused host call: USERD polled; chid 3 never drains, so the cap ends it.
        let (st, note) = idle_channels(&idle_request(2000, &[(1, 3)]), probe, |_: &[(u32, u32)], _, _| {
            Some((NV_ERR_NOT_SUPPORTED, "refused".into()))
        });
        assert_eq!(st, NV_ERR_TIMEOUT);
        assert!(note.contains("USERD polled"), "{note}");
        // An unknown channel anywhere in the list: 0x33, no host call.
        let mut called = false;
        let (st, _) = idle_channels(&idle_request(1000, &[(1, 3), (1, 9)]), probe, |_: &[(u32, u32)], _, _| {
            called = true;
            Some((NV_OK, String::new()))
        });
        assert_eq!(st, NV_ERR_INVALID_OBJECT_HANDLE);
        assert!(!called);
        // Only software channels: the host is not asked.
        let (st, _) = idle_channels(&idle_request(1000, &[(1, 2)]), probe, |_: &[(u32, u32)], _, _| {
            panic!("no mirrored channel in the list")
        });
        assert_eq!(st, NV_OK);
    }

    #[test]
    fn reg_ops_answer_from_the_model_only() {
        let mut p = vec![0u8; REGOPS_OPERATIONS + 3 * REGOP_SIZE];
        put32(&mut p, REGOPS_COUNT, 3);
        // A read of BOOT_0, a read of a register the model does not answer, a write.
        put32(&mut p, REGOPS_OPERATIONS + REGOP_OFFSET, 0x0);
        put32(&mut p, REGOPS_OPERATIONS + REGOP_SIZE + REGOP_OFFSET, 0x0040_0100);
        p[REGOPS_OPERATIONS + 2 * REGOP_SIZE + REGOP_OP] = 1;
        put32(&mut p, REGOPS_OPERATIONS + 2 * REGOP_SIZE + REGOP_OFFSET, 0x0);
        let model = |off: u32| (off == 0).then_some(0x1660_00A1);
        let (st, out, note) = exec_reg_ops(&p, model);
        assert_eq!(st, NV_OK, "{note}");
        assert_eq!(out[REGOPS_OPERATIONS + REGOP_STATUS], REG_OP_STATUS_SUCCESS);
        assert_eq!(u32_at(&out, REGOPS_OPERATIONS + REGOP_VALUE_LO), 0x1660_00A1);
        assert_eq!(out[REGOPS_OPERATIONS + REGOP_SIZE + REGOP_STATUS], REG_OP_STATUS_NOACCESS);
        assert_eq!(out[REGOPS_OPERATIONS + 2 * REGOP_SIZE + REGOP_STATUS], REG_OP_STATUS_NOACCESS);
        put32(&mut p, REGOPS_COUNT, 0);
        assert_eq!(exec_reg_ops(&p, model).0, NV_ERR_INVALID_ARGUMENT);
    }

    #[test]
    fn dump_is_empty_and_map_dma_refused() {
        let mut p = vec![0u8; DUMP_FIXED];
        put32(&mut p, DUMP_BUFFER_SIZE, 0xFFD0);
        let (st, out, _) = dump_protobuf_component(&p);
        assert_eq!((st, out.len(), u32_at(&out, DUMP_BUFFER_SIZE)), (NV_OK, DUMP_FIXED, 0));
        assert_eq!(map_memory_dma(FN_MAP_MEMORY_DMA).0, NV_ERR_NOT_SUPPORTED);
        assert_eq!(function_name(FN_ECC_NOTIFIER_WRITE_ACK), Some("ECC_NOTIFIER_WRITE_ACK"));
    }
}
