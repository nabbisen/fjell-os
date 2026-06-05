//! A/B upgrade and boot-control block types for Fjell OS M6.
#![no_std]

pub mod release_metadata;
pub mod rollback_record;

pub use release_metadata::{ReleaseMetadata, Provenance,
    RELEASE_METADATA_VERSION, RELEASE_METADATA_DOMAIN};
pub use rollback_record::{
    RollbackRecord, AdvanceSource, RollbackCheckResult,
    ROLLBACK_RECORD_VERSION, ROLLBACK_RECORD_DOMAIN,
    check_rollback, advance_min_counter,
};

// ── CRC32 (ISO 3309 / Castagnoli) — no lookup table, no_std safe ─────────────

/// Compute CRC32 over `data`.  Uses the standard 0xEDB88320 polynomial.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

pub const BOOT_CTL_MAGIC: [u8; 8] = *b"FJBOOT\0\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotId { A = 0, B = 1 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SlotState { Empty = 0, Bootable = 1, Candidate = 2, Confirmed = 3, Failed = 4 }

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotInfo {
    pub state:            SlotState,
    pub image_generation: u64,
    pub confirmed:        u8,
    pub tries_allowed:    u8,
    pub remaining_tries:  u8,
}

impl SlotInfo {
    pub const fn empty() -> Self {
        SlotInfo { state: SlotState::Empty, image_generation: 0,
                   confirmed: 0, tries_allowed: 3, remaining_tries: 3 }
    }
    pub const fn bootable(image_gen: u64) -> Self {
        SlotInfo { state: SlotState::Bootable, image_generation: image_gen,
                   confirmed: 1, tries_allowed: 3, remaining_tries: 3 }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BootControlBlock {
    pub magic:                [u8; 8],
    pub version:              u16,
    pub generation:           u64,
    pub active_slot:          u8,   // SlotId
    pub last_confirmed_slot:  u8,
    pub candidate_slot:       u8,   // 0xFF = none
    pub slot_a:               SlotInfo,
    pub slot_b:               SlotInfo,
    pub crc32:                u32,
}

pub const NO_CANDIDATE: u8 = 0xFF;

impl BootControlBlock {
    pub fn new(image_gen: u64) -> Self {
        BootControlBlock {
            magic: BOOT_CTL_MAGIC, version: 1, generation: image_gen,
            active_slot: SlotId::A as u8,
            last_confirmed_slot: SlotId::A as u8,
            candidate_slot: NO_CANDIDATE,
            slot_a: SlotInfo::bootable(image_gen),
            slot_b: SlotInfo::empty(),
            crc32: 0,
        }
    }

    /// Compute and store CRC32 (RFC 008).  Call before writing to disk.
    pub fn seal(&mut self) {
        self.crc32 = 0;
        let bytes = unsafe { core::slice::from_raw_parts(
            self as *const _ as *const u8, core::mem::size_of::<Self>()) };
        self.crc32 = crc32(bytes);
    }

    /// Returns true if magic is correct AND CRC32 matches (RFC 008).
    pub fn is_valid(&self) -> bool {
        if self.magic != BOOT_CTL_MAGIC { return false; }
        let mut copy = *self;
        copy.crc32 = 0;
        let bytes = unsafe { core::slice::from_raw_parts(
            &copy as *const _ as *const u8, core::mem::size_of::<Self>()) };
        crc32(bytes) == self.crc32
    }
}


// ── RFC 023: BCB mirror selection ────────────────────────────────────────────

/// Result of selecting between two `BootControlBlock` mirror copies.
///
/// Both mirrors carry the same data when written atomically, but power-loss
/// can leave one mirror corrupted.  Selection prefers the higher generation
/// with a valid CRC.
#[derive(Debug, PartialEq)]
pub enum BcbMirrorSelection<'a> {
    /// Mirror A was selected (B invalid or lower generation).
    SelectedA(&'a BootControlBlock),
    /// Mirror B was selected (A invalid or lower generation).
    SelectedB(&'a BootControlBlock),
    /// Both mirrors are valid and have the same generation; A chosen as tie-breaker.
    BothValidSameGeneration(&'a BootControlBlock),
    /// Neither mirror has a valid magic + CRC; the disk may be uninitialised.
    NoneValid,
}

/// Select the authoritative `BootControlBlock` mirror.
///
/// Selection rules (RFC 023, per architect decision):
/// 1. A invalid / B invalid → `NoneValid`
/// 2. Only A valid → `SelectedA`
/// 3. Only B valid → `SelectedB`
/// 4. Both valid, generation A > B → `SelectedA`
/// 5. Both valid, generation B > A → `SelectedB`
/// 6. Both valid, generation A == B → `BothValidSameGeneration(&A)` (tie-breaker: A)
pub fn select_bcb_mirror<'a>(
    a: &'a BootControlBlock,
    b: &'a BootControlBlock,
) -> BcbMirrorSelection<'a> {
    match (a.is_valid(), b.is_valid()) {
        (false, false) => BcbMirrorSelection::NoneValid,
        (true,  false) => BcbMirrorSelection::SelectedA(a),
        (false, true ) => BcbMirrorSelection::SelectedB(b),
        (true,  true ) => {
            use core::cmp::Ordering;
            match a.generation.cmp(&b.generation) {
                Ordering::Greater => BcbMirrorSelection::SelectedA(a),
                Ordering::Less    => BcbMirrorSelection::SelectedB(b),
                Ordering::Equal   => BcbMirrorSelection::BothValidSameGeneration(a),
            }
        }
    }
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpgradeState {
    Created, Verified, Staging, Staged, CandidateSet, Confirmed, Aborted, Failed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_control_block_initial_slot_b_is_empty() {
        let bcb = BootControlBlock::new(1);
        assert_eq!(bcb.slot_b.state, SlotState::Empty,
            "slot B must start as Empty; it has no staged image");
        assert_eq!(bcb.slot_b.image_generation, 0);
        assert_eq!(bcb.slot_b.confirmed, 0);
    }

    #[test]
    fn boot_control_block_initial_slot_a_is_bootable() {
        let bcb = BootControlBlock::new(42);
        assert_eq!(bcb.slot_a.state, SlotState::Bootable);
        assert_eq!(bcb.slot_a.image_generation, 42);
        assert_eq!(bcb.slot_a.confirmed, 1);
    }

    #[test]
    fn boot_control_block_is_valid() {
        let mut bcb = BootControlBlock::new(1);
        bcb.seal();  // is_valid() now checks CRC32 (RFC 008)
        assert!(bcb.is_valid(), "sealed BCB must pass is_valid");
    }
}

    #[test]
    fn bcb_seal_produces_valid_crc() {
        let mut bcb = BootControlBlock::new(1);
        bcb.seal();
        assert!(bcb.is_valid(), "sealed BCB must pass is_valid");
    }

    #[test]
    fn bcb_corrupt_byte_fails_crc() {
        let mut bcb = BootControlBlock::new(1);
        bcb.seal();
        bcb.version ^= 0xFF;  // corrupt one byte
        assert!(!bcb.is_valid(), "corrupted BCB must fail is_valid");
    }

    // ── RFC 023: mirror selection tests ──────────────────────────────────────

    #[test]
    fn select_bcb_mirror_none_valid_when_both_corrupt() {
        let bcb = BootControlBlock::new(1); // unsealed — magic ok but CRC=0
        let mut bad = BootControlBlock::new(1);
        bad.magic = [0u8; 8]; // corrupt magic
        bad.seal();
        let r = select_bcb_mirror(&bcb, &bad);
        assert!(matches!(r, BcbMirrorSelection::NoneValid));
    }

    #[test]
    fn select_bcb_mirror_prefers_only_valid() {
        let mut a = BootControlBlock::new(1); a.seal();
        let mut b = BootControlBlock::new(1); b.seal();
        b.magic = [0u8; 8]; // corrupt B
        assert!(matches!(select_bcb_mirror(&a, &b), BcbMirrorSelection::SelectedA(_)));
        assert!(matches!(select_bcb_mirror(&b, &a), BcbMirrorSelection::SelectedB(_)));
    }

    #[test]
    fn select_bcb_mirror_higher_generation_wins() {
        let mut a = BootControlBlock::new(2); a.seal();
        let mut b = BootControlBlock::new(5); b.seal();
        assert!(matches!(select_bcb_mirror(&a, &b), BcbMirrorSelection::SelectedB(_)));
        assert!(matches!(select_bcb_mirror(&b, &a), BcbMirrorSelection::SelectedA(_)));
    }

    #[test]
    fn select_bcb_mirror_equal_generation_selects_a() {
        let mut a = BootControlBlock::new(3); a.seal();
        let mut b = BootControlBlock::new(3); b.seal();
        assert!(matches!(select_bcb_mirror(&a, &b), BcbMirrorSelection::BothValidSameGeneration(_)));
    }

#[cfg(test)]
mod tests_v03;
