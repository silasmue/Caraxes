// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
//! The guest DMA table (decision D13, docs/caraxes/sysmem-iova.md §4,
//! docs/caraxes/memory-model.md): which host IOVA range each piece of guest RAM
//! has. The device builds it when it registers guest RAM with the host RM, the
//! guest module `caraxes_dma_range` installs it as the GPU's `dma_range_map`, and
//! from then on every sysmem address the guest hands the device is an IOVA, which
//! the device translates back to a guest physical address for its own accesses
//! ([`crate::guestmem::GuestMemory::translate`]).
//!
//! The published format is the module's parameter, one line:
//! `cpu_start:dma_start:size[,...]`, hex without `0x`, sorted by guest physical
//! address (guest-module/caraxes_dma_range/README.md, "Parameters").

use std::fmt;

/// One entry: `size` bytes of guest RAM at guest physical `gpa` are reached by the
/// GPU at `iova`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DmaRange {
    pub gpa: u64,
    pub iova: u64,
    pub size: u64,
}

/// The module's limits (caraxes_dma_range.c `CDR_MAX_RANGES`, README "Validation").
pub const MAX_RANGES: usize = 64;
const SIZE_GRANULE: u64 = 2 << 20;
/// Where [`DmaTable::with_alias`] puts the guest's swiotlb pool: 960 GiB, below the
/// 40-bit limit of the flush buffer and above anything the host maps (its 32-bit IOVAs
/// sit below 4 GiB, its 64-bit ones just below 2^47), so a stray access faults.
pub const LOW_ALIAS_IOVA: u64 = 0xF0_0000_0000;
const PAGE: u64 = 4096;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DmaTable {
    /// Sorted by `gpa`.
    ranges: Vec<DmaRange>,
}

impl DmaTable {
    /// A validated table: the rules the guest module enforces, so a table the device
    /// publishes is one the module accepts.
    pub fn new(ranges: Vec<DmaRange>) -> Result<Self, String> {
        Self::with_granule(ranges, SIZE_GRANULE)
    }

    /// Exact page-sized mappings from a GPA-speaking guest (QEMU/Windows).
    /// This table is internal; the Linux guest module still requires `new`'s
    /// 2 MiB size granule. In particular, never round a VGA/MMIO hole into RAM.
    pub fn for_gpa_guest(ranges: Vec<DmaRange>) -> Result<Self, String> {
        Self::with_granule(ranges, PAGE)
    }

    fn with_granule(mut ranges: Vec<DmaRange>, granule: u64) -> Result<Self, String> {
        if ranges.len() > MAX_RANGES {
            return Err(format!(
                "{} ranges, the module takes at most {MAX_RANGES}",
                ranges.len()
            ));
        }
        for r in &ranges {
            if r.size == 0 || r.size % granule != 0 {
                return Err(format!("DMA range {r:?}: size is not a non-zero multiple of {granule:#x}"));
            }
            if r.gpa % PAGE != 0 || r.iova % PAGE != 0 {
                return Err(format!("DMA range {r:?}: start not page aligned"));
            }
            if r.gpa.checked_add(r.size).is_none() || r.iova.checked_add(r.size).is_none() {
                return Err(format!("DMA range {r:?}: wraps around the address space"));
            }
        }
        ranges.sort_by_key(|r| r.gpa);
        for w in ranges.windows(2) {
            if w[0].gpa + w[0].size > w[1].gpa {
                return Err(format!(
                    "{} and {} overlap in guest physical space",
                    w[0], w[1]
                ));
            }
        }
        let mut by_iova = ranges.clone();
        by_iova.sort_by_key(|r| r.iova);
        for w in by_iova.windows(2) {
            if w[0].iova + w[0].size > w[1].iova {
                return Err(format!("{} and {} overlap in IOVA space", w[0], w[1]));
            }
        }
        Ok(DmaTable { ranges })
    }

    /// The identity table over guest RAM regions `(gpa, size)`: IOVA = GPA. For a run
    /// without host registration that still exercises the guest module and the
    /// device's translation path.
    pub fn identity(regions: &[(u64, u64)]) -> Result<Self, String> {
        Self::new(
            regions
                .iter()
                .map(|&(gpa, size)| DmaRange {
                    gpa,
                    iova: gpa,
                    size,
                })
                .collect(),
        )
    }

    /// Parse the module-parameter format (`0x` optional).
    pub fn parse(s: &str) -> Result<Self, String> {
        let hex = |f: &str| {
            let f = f.trim();
            let f = f.strip_prefix("0x").unwrap_or(f);
            u64::from_str_radix(f, 16).map_err(|e| format!("{f:?}: {e}"))
        };
        let mut ranges = Vec::new();
        for tok in s.trim().split(',').filter(|t| !t.trim().is_empty()) {
            let f: Vec<&str> = tok.split(':').collect();
            if f.len() != 3 {
                return Err(format!("{tok:?}: want cpu_start:dma_start:size"));
            }
            ranges.push(DmaRange {
                gpa: hex(f[0])?,
                iova: hex(f[1])?,
                size: hex(f[2])?,
            });
        }
        Self::new(ranges)
    }

    pub fn ranges(&self) -> &[DmaRange] {
        &self.ranges
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn is_identity(&self) -> bool {
        self.ranges.iter().all(|r| r.gpa == r.iova)
    }

    /// The guest physical address of `len` bytes at DMA address `iova`, when they lie
    /// in one entry. A range that leaves its entry is refused: the neighbouring IOVA
    /// belongs to another chunk or to nothing.
    pub fn iova_to_gpa(&self, iova: u64, len: u64) -> Option<u64> {
        self.ranges
            .iter()
            .find(|r| iova >= r.iova && iova - r.iova < r.size && len <= r.size - (iova - r.iova))
            .map(|r| r.gpa + (iova - r.iova))
    }

    /// The DMA address the guest computes for guest physical `gpa` once the table is
    /// installed (the module's `translate_phys_to_dma`).
    pub fn gpa_to_iova(&self, gpa: u64) -> Option<u64> {
        self.ranges
            .iter()
            .find(|r| gpa >= r.gpa && gpa - r.gpa < r.size)
            .map(|r| r.iova + (gpa - r.gpa))
    }

    /// Whether the table covers `[gpa, gpa + size)` completely.
    pub fn covers_gpa(&self, gpa: u64, size: u64) -> bool {
        let mut at = gpa;
        let Some(end) = gpa.checked_add(size) else { return false };
        while at < end {
            match self
                .ranges
                .iter()
                .find(|r| at >= r.gpa && at - r.gpa < r.size)
            {
                Some(r) => at = r.gpa + r.size,
                None => return false,
            }
        }
        true
    }

    /// The table with guest RAM `[gpa, gpa + size)` (rounded out to 2 MiB) given the
    /// DMA addresses at `alias_iova` instead of its registered IOVAs.
    ///
    /// Why: a GSP-client guest allocates its sysmem flush buffer with a temporary 40-bit
    /// DMA mask and refuses to initialize when its DMA address is at or above 2^40
    /// (`kmemsysInitFlushSysmemBuffer_GM107`, kern_mem_sys_gm107.c:227-340); the page
    /// is bounced through swiotlb. Once the host GPU runs on 64-bit IOVAs every registered
    /// chunk lies near 2^47, so the guest's swiotlb pool gets an alias below 2^40. The
    /// physical GPU never uses the guest's flush buffer (the host owns the flush
    /// register); `alias_iova` should lie where the host IOMMU domain maps nothing
    /// (between 4 GiB and 2^40), so that a stray access faults. See
    /// docs/caraxes/memory-model.md §2.
    pub fn with_alias(&self, gpa: u64, size: u64, alias_iova: u64) -> Result<Self, String> {
        let start = gpa & !(SIZE_GRANULE - 1);
        let end = (gpa + size).div_ceil(SIZE_GRANULE) * SIZE_GRANULE;
        let Some(i) = self
            .ranges
            .iter()
            .position(|r| start >= r.gpa && end <= r.gpa + r.size)
        else {
            return Err(format!(
                "alias [{start:#x}, {end:#x}) does not lie in one entry"
            ));
        };
        let r = self.ranges[i];
        let mut ranges: Vec<DmaRange> = self.ranges.clone();
        ranges.remove(i);
        if start > r.gpa {
            ranges.push(DmaRange {
                gpa: r.gpa,
                iova: r.iova,
                size: start - r.gpa,
            });
        }
        ranges.push(DmaRange {
            gpa: start,
            iova: alias_iova,
            size: end - start,
        });
        if end < r.gpa + r.size {
            ranges.push(DmaRange {
                gpa: end,
                iova: r.iova + (end - r.gpa),
                size: r.gpa + r.size - end,
            });
        }
        Self::new(ranges)
    }

    /// The entries of `guest` (the table the guest module installed, `<dma-ranges>.guest`
    /// of the launcher) that give guest RAM other DMA addresses than this table does: the
    /// swiotlb alias of [`Self::with_alias`], which also covers up to 2 MiB of ordinary
    /// guest RAM around the pool (the rounding). The guest hands the device those
    /// addresses for any buffer that lands there (docs/caraxes/memory-model.md §2, "The
    /// alias in the device", the Xid 31 of 2026-09-24).
    ///
    /// Refused unless `guest` describes exactly this table's guest RAM, every entry either
    /// translates as this table does or lies wholly outside this table's IOVAs, and every
    /// alias covers registered guest RAM only. So an alias can only ever name this VM's own
    /// pages, and no address this table accepts changes meaning.
    pub fn aliases_of(&self, guest: &DmaTable) -> Result<Vec<DmaRange>, String> {
        for r in &guest.ranges {
            if !self.covers_gpa(r.gpa, r.size) {
                return Err(format!("{r}: guest RAM the device did not register"));
            }
        }
        for r in &self.ranges {
            if !guest.covers_gpa(r.gpa, r.size) {
                return Err(format!("{r}: registered guest RAM the guest table leaves out"));
            }
        }
        let mut aliases = Vec::new();
        for r in &guest.ranges {
            if self.iova_to_gpa(r.iova, r.size) == Some(r.gpa) {
                continue;
            }
            if let Some(o) = self
                .ranges
                .iter()
                .find(|d| r.iova < d.iova + d.size && d.iova < r.iova + r.size)
            {
                return Err(format!("{r}: overlaps the device's {o} with another meaning"));
            }
            aliases.push(*r);
        }
        Ok(aliases)
    }

    /// The `ranges=` value of `caraxes_dma_range`.
    pub fn module_param(&self) -> String {
        self.ranges
            .iter()
            .map(|r| format!("{:x}:{:x}:{:x}", r.gpa, r.iova, r.size))
            .collect::<Vec<_>>()
            .join(",")
    }
}

impl fmt::Display for DmaRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GPA {:#x}..{:#x} -> IOVA {:#x}..{:#x}",
            self.gpa,
            self.gpa + self.size,
            self.iova,
            self.iova + self.size
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpa_fragments_keep_holes_and_distinct_host_offsets() {
        let ranges = vec![
            DmaRange { gpa: 0, iova: 0x10_0000_0000, size: 0xa0000 },
            DmaRange { gpa: 0x100000, iova: 0x20_0000_0000, size: 0x7bf00000 },
        ];
        assert!(DmaTable::new(ranges.clone()).is_err());
        let host = DmaTable::for_gpa_guest(ranges.clone()).unwrap();
        let guest = DmaTable::for_gpa_guest(ranges.iter().map(|r|
            DmaRange { gpa: r.gpa, iova: r.gpa, size: r.size }).collect()).unwrap();
        assert_eq!(host.aliases_of(&guest).unwrap().len(), 2);
        assert_eq!(host.gpa_to_iova(0x9ffff), Some(0x10_0009_ffff));
        assert_eq!(host.gpa_to_iova(0xa0000), None);
        assert_eq!(host.gpa_to_iova(0xfffff), None);
        assert_eq!(host.gpa_to_iova(0x100000), Some(0x20_0000_0000));
        assert!(!host.covers_gpa(0x9f000, 0x2000));
        assert_eq!(host.iova_to_gpa(0x10_0009_f000, 0x2000), None);
        assert!(!host.covers_gpa(u64::MAX - 0x1000, 0x2000));
        assert!(DmaTable::for_gpa_guest(vec![DmaRange { gpa: 0, iova: 0, size: 1 }]).is_err());
        assert!(DmaTable::for_gpa_guest(vec![DmaRange { gpa: u64::MAX - 0xfff, iova: 0, size: 0x1000 }]).is_err());
    }

    const G: u64 = 1 << 30;

    /// The shape of a 4 GiB Cloud Hypervisor guest after the 64-bit IOVA switch.
    fn table() -> DmaTable {
        DmaTable::new(vec![
            DmaRange {
                gpa: 4 * G,
                iova: 0x7fff_0000_0000,
                size: G,
            },
            DmaRange {
                gpa: 0,
                iova: 0x4000_0000,
                size: G,
            },
            DmaRange {
                gpa: G,
                iova: 0x7ffe_c000_0000,
                size: G,
            },
            DmaRange {
                gpa: 2 * G,
                iova: 0x7ffe_8000_0000,
                size: G,
            },
        ])
        .unwrap()
    }

    #[test]
    fn translates_both_ways_and_refuses_what_it_does_not_cover() {
        let t = table();
        assert_eq!(t.iova_to_gpa(0x4000_1234, 4), Some(0x1234));
        assert_eq!(t.iova_to_gpa(0x7ffe_c000_0010, 8), Some(G + 0x10));
        assert_eq!(t.gpa_to_iova(4 * G + 0x2000), Some(0x7fff_0000_2000));
        // Leaves its entry (the next IOVA is not the next GPA).
        assert_eq!(t.iova_to_gpa(0x7fff_0000_0000 - 4, 8), None);
        // An address the guest could only have if it bypassed the table.
        assert_eq!(t.iova_to_gpa(0x1000, 4), None);
        assert_eq!(t.gpa_to_iova(3 * G), None);
        assert!(t.covers_gpa(0, 3 * G));
        assert!(!t.covers_gpa(0, 4 * G));
        assert!(!t.is_identity());
    }

    #[test]
    fn module_parameter_round_trip() {
        let t = table();
        let p = t.module_param();
        assert_eq!(
            p,
            "0:40000000:40000000,40000000:7ffec0000000:40000000,\
             80000000:7ffe80000000:40000000,100000000:7fff00000000:40000000"
        );
        assert_eq!(DmaTable::parse(&p).unwrap(), t);
        assert_eq!(
            DmaTable::parse("0x0:0x0:0xc0000000")
                .unwrap()
                .ranges()
                .len(),
            1
        );
        let id = DmaTable::identity(&[(0, 3 * G), (4 * G, G)]).unwrap();
        assert!(id.is_identity());
        assert_eq!(
            id.module_param(),
            "0:0:c0000000,100000000:100000000:40000000"
        );
    }

    #[test]
    fn swiotlb_alias_splits_one_entry() {
        // The guest's pool [0xba2b7000, 0xbe2b7000) in the [2 GiB, 3 GiB) chunk.
        let t = table()
            .with_alias(0xba2b_7000, 64 << 20, 0xF0_0000_0000)
            .unwrap();
        assert_eq!(t.ranges().len(), 6);
        assert_eq!(t.gpa_to_iova(0xba2b_7000), Some(0xF0_000B_7000));
        assert!(t.gpa_to_iova(0xba2b_7000).unwrap() < 1 << 40);
        assert_eq!(
            t.gpa_to_iova(0xba1f_f000),
            Some(0x7ffe_8000_0000 + 0x3a1f_f000)
        );
        assert_eq!(
            t.gpa_to_iova(0xbe40_0000),
            Some(0x7ffe_8000_0000 + 0x3e40_0000)
        );
        assert!(t.covers_gpa(0, 3 * G));
        assert_eq!(t.iova_to_gpa(0xF0_0000_1000, 4), Some(0xba20_1000));
        assert!(table()
            .with_alias(0xbfff_f000, 8192, 0xF0_0000_0000)
            .is_err());
    }

    #[test]
    fn the_guest_alias_is_accepted_and_nothing_else() {
        let dev = table();
        let guest = dev
            .with_alias(0xba2b_7000, 64 << 20, 0xF0_0000_0000)
            .unwrap();
        let a = dev.aliases_of(&guest).unwrap();
        assert_eq!(
            a,
            vec![DmaRange {
                gpa: 0xba20_0000,
                iova: 0xF0_0000_0000,
                size: 0x420_0000
            }]
        );
        // The same table as the device's: no alias.
        assert!(dev.aliases_of(&dev).unwrap().is_empty());
        // Guest RAM the device never registered.
        let r = |gpa, iova, size| DmaRange { gpa, iova, size };
        let mut more = guest.ranges().to_vec();
        more.push(r(5 * G, 0xF1_0000_0000, 2 << 20));
        assert!(dev.aliases_of(&DmaTable::new(more).unwrap()).is_err());
        // A registered chunk the guest table leaves out.
        let fewer: Vec<DmaRange> = guest.ranges().iter().copied().filter(|x| x.gpa != 4 * G).collect();
        assert!(dev.aliases_of(&DmaTable::new(fewer).unwrap()).is_err());
        // An entry that gives a device IOVA to other guest pages (swaps two chunks).
        let swapped = DmaTable::new(vec![
            r(0, 0x4000_0000, G),
            r(G, 0x7ffe_8000_0000, G),
            r(2 * G, 0x7ffe_c000_0000, G),
            r(4 * G, 0x7fff_0000_0000, G),
        ])
        .unwrap();
        assert!(dev.aliases_of(&swapped).is_err());
    }

    #[test]
    fn refuses_what_the_module_refuses() {
        let r = |gpa, iova, size| DmaRange { gpa, iova, size };
        assert!(DmaTable::new(vec![r(0, 0, 0x1000)]).is_err()); // not 2 MiB
        assert!(DmaTable::new(vec![r(0x800, 0, 2 << 20)]).is_err()); // unaligned
        assert!(DmaTable::new(vec![r(0, 0, G), r(G / 2, 2 * G, G)]).is_err()); // GPA overlap
        assert!(DmaTable::new(vec![r(0, 0, G), r(G, G / 2, G)]).is_err()); // IOVA overlap
        assert!(DmaTable::new((0..65).map(|i| r(i * G, i * G, G)).collect()).is_err());
        assert!(DmaTable::parse("0:0").is_err());
    }

    /// A guest that speaks GPA (Windows, CARAXES_GUEST_DMA_GPA): the identity of a linearly
    /// registered table is accepted as aliases, one per range; a registration that put some RAM
    /// at IOVAs inside the GPA space would make an identity alias ambiguous and is refused.
    #[test]
    fn identity_guest_table_aliases_a_linear_registration() {
        let off = 0x7ffa_c000_0000u64;
        let t = DmaTable::new(vec![
            DmaRange { gpa: 0, iova: off, size: 2 * G },
            DmaRange { gpa: 4 * G, iova: off + 4 * G, size: 2 * G },
        ])
        .unwrap();
        let ident = DmaTable::new(t.ranges().iter().map(|r| DmaRange { gpa: r.gpa, iova: r.gpa, size: r.size }).collect()).unwrap();
        let a = t.aliases_of(&ident).unwrap();
        assert_eq!(a.len(), 2);
        assert!(a.iter().all(|r| r.iova == r.gpa));
        let low = DmaTable::new(vec![DmaRange { gpa: 0, iova: 0x4000_0000, size: G }, DmaRange { gpa: G, iova: 0, size: G }]).unwrap();
        let ident = DmaTable::new(low.ranges().iter().map(|r| DmaRange { gpa: r.gpa, iova: r.gpa, size: r.size }).collect()).unwrap();
        assert!(low.aliases_of(&ident).is_err());
    }
}
