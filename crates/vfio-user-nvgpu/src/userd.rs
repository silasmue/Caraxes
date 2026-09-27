// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// SPDX-FileCopyrightText: 2026 Universität Stuttgart, IKR
//! Directional snapshots for a guest USERD and a private host USERD.
//!
//! The channel-class ABI assigns each word to one writer. Keeping the two
//! directions explicit prevents either side from overwriting the other's state.

pub const USERD_BYTES: usize = 512;

/// Words written by the CPU, from `Nvc46fControl` in `clc46f.h`.
pub const CPU_FIELDS: [u64; 3] = [0x40, 0x4c, 0x8c];

/// Words written by the GPU, from `Nvc46fControl` in `clc46f.h`.
pub const GPU_FIELDS: [u64; 6] = [0x44, 0x48, 0x58, 0x5c, 0x60, 0x88];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuState {
    put: u32,
    put_hi: u32,
    gp_put: u32,
}

impl CpuState {
    /// Read every CPU-owned word and reject an invalid GPFIFO producer index.
    pub fn read(entries: u32, mut read: impl FnMut(u64) -> Option<u32>) -> Option<Self> {
        let state = Self {
            put: read(CPU_FIELDS[0])?,
            put_hi: read(CPU_FIELDS[1])?,
            gp_put: read(CPU_FIELDS[2])?,
        };
        (entries != 0 && state.gp_put < entries).then_some(state)
    }

    pub fn gp_put(&self) -> u32 {
        self.gp_put
    }

    /// Publish GPPut last: it is the commit word for newly available entries.
    pub fn publish(&self, mut write: impl FnMut(u64, u32)) {
        write(CPU_FIELDS[1], self.put_hi);
        write(CPU_FIELDS[0], self.put);
        write(CPU_FIELDS[2], self.gp_put);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuState {
    values: [u32; GPU_FIELDS.len()],
}

impl GpuState {
    /// Finish all reads before returning a state that can publish any writes.
    pub fn read(mut read: impl FnMut(u64) -> Option<u32>) -> Option<Self> {
        let mut values = [0; GPU_FIELDS.len()];
        for (i, off) in GPU_FIELDS.into_iter().enumerate() {
            values[i] = read(off)?;
        }
        Some(Self { values })
    }

    pub fn gp_get(&self) -> u32 {
        self.values[GPU_FIELDS.len() - 1]
    }

    /// Publish GPU-owned words only; guest GPPut is never touched.
    pub fn publish(&self, mut write: impl FnMut(u64, u32)) {
        for (off, value) in GPU_FIELDS.into_iter().zip(self.values) {
            write(off, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn field_ownership_is_disjoint_and_inside_userd() {
        for off in CPU_FIELDS.into_iter().chain(GPU_FIELDS) {
            assert_eq!(off % 4, 0);
            assert!((off as usize) < USERD_BYTES);
        }
        assert!(CPU_FIELDS.iter().all(|off| !GPU_FIELDS.contains(off)));
    }

    #[test]
    fn gpput_is_published_last() {
        let state = CpuState::read(8, |off| Some(if off == 0x8c { 3 } else { off as u32 }))
            .expect("valid snapshot");
        let mut writes = Vec::new();
        state.publish(|off, value| writes.push((off, value)));
        assert_eq!(state.gp_put(), 3);
        assert_eq!(writes, [(0x4c, 0x4c), (0x40, 0x40), (0x8c, 3)]);
    }

    #[test]
    fn unreadable_or_malformed_cpu_snapshot_refuses_all_writes() {
        for (missing, entries) in [(Some(0x40), 8), (Some(0x4c), 8), (Some(0x8c), 8), (None, 3)] {
            let read =
                |off| (Some(off) != missing).then_some(if off == 0x8c { 3 } else { off as u32 });
            let mut writes = Vec::new();
            if let Some(state) = CpuState::read(entries, read) {
                state.publish(|off, value| writes.push((off, value)));
            }
            assert!(writes.is_empty(), "missing {missing:?}, entries {entries}");
        }
    }

    #[test]
    fn gpu_publish_does_not_clobber_gpput() {
        let host: BTreeMap<_, _> = GPU_FIELDS
            .into_iter()
            .map(|off| (off, off as u32 + 7))
            .collect();
        let state = GpuState::read(|off| host.get(&off).copied()).expect("readable snapshot");
        let mut guest = BTreeMap::from([(0x8c, 19)]);
        state.publish(|off, value| {
            guest.insert(off, value);
        });
        assert_eq!(guest[&0x8c], 19);
        assert_eq!(state.gp_get(), host[&0x88]);
        assert!(GPU_FIELDS.iter().all(|off| guest[off] == host[off]));
    }

    #[test]
    fn unreadable_gpu_snapshot_refuses_all_writes() {
        let mut writes = Vec::new();
        if let Some(state) = GpuState::read(|off| (off != GPU_FIELDS[3]).then_some(off as u32)) {
            state.publish(|off, value| writes.push((off, value)));
        }
        assert!(writes.is_empty());
    }
}
