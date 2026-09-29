// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
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

    /// The snapshot only when the GPU moved since `last` (Get and GPGet as last read): any
    /// GPU-owned word moves only with one of them (every consumed method, SET_REFERENCE
    /// among them, advances Get; every fetched entry GPGet). Two reads for an idle channel
    /// instead of six, each an uncached read of a host USERD in vidmem (P37: 9.6 us for the
    /// six). Updates `last`.
    pub fn read_if_moved(
        last: &mut Option<(u32, u32)>,
        mut read: impl FnMut(u64) -> Option<u32>,
    ) -> Option<Self> {
        let (get, gp_get) = (read(GPU_FIELDS[0])?, read(GPU_FIELDS[5])?);
        if *last == Some((get, gp_get)) {
            return None;
        }
        let state = Self::read(|off| match off {
            o if o == GPU_FIELDS[0] => Some(get),
            o if o == GPU_FIELDS[5] => Some(gp_get),
            o => read(o),
        })?;
        *last = Some((get, gp_get));
        Some(state)
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

    /// An idle channel costs two reads and publishes nothing; a moved one is read whole.
    #[test]
    fn an_idle_gpu_snapshot_is_two_reads() {
        let mut host: BTreeMap<u64, u32> = GPU_FIELDS.into_iter().map(|o| (o, 1)).collect();
        let mut last = None;
        let reads = std::cell::Cell::new(0);
        let read = |h: &BTreeMap<u64, u32>, off| {
            reads.set(reads.get() + 1);
            h.get(&off).copied()
        };
        assert!(GpuState::read_if_moved(&mut last, |o| read(&host, o)).is_some());
        assert_eq!(reads.get(), 6);
        assert!(GpuState::read_if_moved(&mut last, |o| read(&host, o)).is_none());
        assert_eq!(reads.get(), 8);
        // GPGet moved: the whole snapshot, with the new values.
        host.insert(0x88, 2);
        host.insert(0x48, 9);
        let s = GpuState::read_if_moved(&mut last, |o| read(&host, o)).unwrap();
        assert_eq!((s.gp_get(), reads.get()), (2, 14));
        let mut guest = BTreeMap::new();
        s.publish(|off, v| {
            guest.insert(off, v);
        });
        assert_eq!(guest[&0x48], 9);
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
