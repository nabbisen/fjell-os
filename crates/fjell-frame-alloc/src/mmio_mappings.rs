//! The kernel's record of the **device pages it has mapped into a task** (RFC-0.49-002 R1).
//!
//! Until 0.49 `sys_mmio_map` mapped device pages into a task's address space and **kept no record that it had**:
//! `MmioRegionTable` describes the board's regions, not what a task mapped, so nothing could take a mapping back and
//! `MmioUnmap = 91` was a syscall the ABI declared and the kernel could not perform. This is the shape
//! `DmaRegionTable` already had -- owner, user VA, physical address, page count -- in a crate a gate can run.
//!
//! **No mapping without a record and no record without a mapping.** The kernel *reserves* an entry before it writes a PTE
//! and *releases* it if the mapping fails, so a full table refuses the map before anything is changed, and a failed map
//! leaves nothing behind.
//!
//! **The table is bounded and must be given back.** A task that maps and then exits or faults must release its entries
//! ([`MmioMappingTable::release_task`]), or a restart loop fills the table -- E-110's shape inside the new structure.

use fjell_abi::task::TaskId;

/// Capacity: **64**. *Re-measured after the first boot with 16 failed* (the `net-absent` and `store` profiles went red): the RFC
/// counted call sites, not calls. `driver-virtio-net` and `storaged` each **scan all eight virtio transports and map every one**
/// (`for bus in 0..8 { sys_mmio_map(.., bus * 0x1000, 0x1000) }`, never unmapping a transport that is not theirs), and
/// `driver-uart` maps one: **up to 17 live mappings in an ordinary boot**, before `neg-test`'s scenario adds its own. 64 leaves
/// room for a few more drivers; the table is ~40 bytes an entry, 2.5 KiB. (Having those drivers unmap the transports they do not
/// use is now possible and is a follow-up, not this line.)
pub const MAX_MMIO_MAPPINGS: usize = 64;

/// One live mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmioMapping {
    /// The task whose page table holds it -- the **real** `TaskId`, generation included.
    pub owner: TaskId,
    /// The `MmioRegionTable` entry it was mapped from. An unmap must present a capability for **this** region.
    pub region_id: u32,
    /// First virtual address, in the owner's device window.
    pub user_va: usize,
    /// First physical address (a device page, **never RAM** and never returned to the frame allocator).
    pub pa: usize,
    /// Consecutive 4 KiB pages.
    pub page_count: u16,
}

/// Why a reservation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmioMapError {
    /// All [`MAX_MMIO_MAPPINGS`] entries are in use.
    Full,
    /// `(owner, user_va)` is already recorded: a second record for one address would let either unmap the other's.
    Duplicate,
}

/// The table. `None` is a free entry.
#[derive(Debug)]
pub struct MmioMappingTable {
    entries: [Option<MmioMapping>; MAX_MMIO_MAPPINGS],
}

impl MmioMappingTable {
    pub const fn new() -> Self {
        MmioMappingTable {
            entries: [None; MAX_MMIO_MAPPINGS],
        }
    }

    /// Reserve an entry for a mapping the caller is about to make. Returns its index, to [`release`](Self::release) if the
    /// map then fails.
    pub fn reserve(&mut self, m: MmioMapping) -> Result<usize, MmioMapError> {
        if self
            .entries
            .iter()
            .flatten()
            .any(|e| e.owner == m.owner && e.user_va == m.user_va)
        {
            return Err(MmioMapError::Duplicate);
        }
        let slot = self
            .entries
            .iter()
            .position(Option::is_none)
            .ok_or(MmioMapError::Full)?;
        self.entries[slot] = Some(m);
        Ok(slot)
    }

    /// The mapping `owner` made at exactly `user_va` -- the **first** page -- and its entry index. An interior address is not
    /// a base and finds nothing: unmapping is whole-mapping only.
    pub fn find(&self, owner: TaskId, user_va: usize) -> Option<(usize, MmioMapping)> {
        self.entries.iter().enumerate().find_map(|(i, e)| {
            e.filter(|m| m.owner == owner && m.user_va == user_va)
                .map(|m| (i, m))
        })
    }

    /// Drop entry `index`. Returns whether there was one.
    pub fn release(&mut self, index: usize) -> bool {
        self.entries.get_mut(index).and_then(Option::take).is_some()
    }

    /// Drop every entry `owner` holds -- the death path's call. Returns how many. Matches the **whole** `TaskId`: a previous
    /// occupant of the slot is a different task.
    pub fn release_task(&mut self, owner: TaskId) -> usize {
        let mut n = 0;
        for e in self.entries.iter_mut() {
            if e.is_some_and(|m| m.owner == owner) {
                *e = None;
                n += 1;
            }
        }
        n
    }

    /// Entries in use.
    pub fn used(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    /// Every live entry (for the kernel's own log and for tests).
    pub fn iter(&self) -> impl Iterator<Item = &MmioMapping> {
        self.entries.iter().flatten()
    }
}

impl Default for MmioMappingTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(owner: TaskId, va: usize) -> MmioMapping {
        MmioMapping {
            owner,
            region_id: 3,
            user_va: va,
            pa: 0x1000_1000,
            page_count: 1,
        }
    }
    const A: TaskId = TaskId::new(5, 0);
    const B: TaskId = TaskId::new(6, 0);

    #[test]
    fn a_reservation_is_found_by_owner_and_exact_base_only() {
        let mut t = MmioMappingTable::new();
        let i = t
            .reserve(MmioMapping {
                page_count: 4,
                ..m(A, 0x7000_0000)
            })
            .unwrap();
        assert_eq!(t.find(A, 0x7000_0000).map(|(k, _)| k), Some(i));
        assert!(
            t.find(A, 0x7000_1000).is_none(),
            "an interior page is not a base"
        );
        assert!(
            t.find(B, 0x7000_0000).is_none(),
            "another task's address is not mine"
        );
        assert!(
            t.find(TaskId::new(5, 1), 0x7000_0000).is_none(),
            "nor is the next occupant of my slot"
        );
    }

    #[test]
    fn a_full_table_refuses_before_anything_is_changed_and_a_release_makes_room() {
        let mut t = MmioMappingTable::new();
        for k in 0..MAX_MMIO_MAPPINGS {
            t.reserve(m(A, 0x7000_0000 + k * 0x1000)).unwrap();
        }
        assert_eq!(t.reserve(m(B, 0x7000_0000)), Err(MmioMapError::Full));
        assert_eq!(t.used(), MAX_MMIO_MAPPINGS);
        let (i, _) = t.find(A, 0x7000_0000).unwrap();
        assert!(t.release(i));
        assert!(!t.release(i), "releasing twice finds nothing");
        assert!(t.reserve(m(B, 0x7000_0000)).is_ok());
    }

    #[test]
    fn one_address_has_one_record() {
        let mut t = MmioMappingTable::new();
        t.reserve(m(A, 0x7000_0000)).unwrap();
        assert_eq!(t.reserve(m(A, 0x7000_0000)), Err(MmioMapError::Duplicate));
        // the same address in another task's window is another mapping
        assert!(t.reserve(m(B, 0x7000_0000)).is_ok());
    }

    #[test]
    fn a_dead_tasks_entries_are_released_and_only_its_own() {
        let mut t = MmioMappingTable::new();
        t.reserve(m(A, 0x7000_0000)).unwrap();
        t.reserve(m(A, 0x7000_1000)).unwrap();
        t.reserve(m(B, 0x7000_0000)).unwrap();
        t.reserve(m(TaskId::new(5, 1), 0x7000_2000)).unwrap(); // the NEXT occupant of A's slot
        assert_eq!(t.release_task(A), 2);
        assert_eq!(t.used(), 2);
        assert!(t.find(B, 0x7000_0000).is_some());
        assert!(
            t.find(TaskId::new(5, 1), 0x7000_2000).is_some(),
            "the whole TaskId is matched, generation included"
        );
        assert_eq!(t.release_task(A), 0);
    }

    #[test]
    fn a_restart_loop_does_not_fill_the_table() {
        // the E-110 shape: a task maps, dies, a new occupant maps, dies ... forever
        let mut t = MmioMappingTable::new();
        for generation in 0..(MAX_MMIO_MAPPINGS as u16 * 4) {
            let me = TaskId::new(5, generation);
            t.reserve(m(me, 0x7000_0000)).unwrap();
            assert_eq!(t.release_task(me), 1);
        }
        assert_eq!(t.used(), 0);
    }
}
