//! A/B upgrade and boot-control block types for Fjell OS M6.
#![no_std]

pub mod release_metadata;
pub mod rollback_record;

pub use release_metadata::{
    Provenance, RELEASE_METADATA_DOMAIN, RELEASE_METADATA_VERSION, ReleaseMetadata,
};
pub use rollback_record::{
    AdvanceSource, ROLLBACK_RECORD_DOMAIN, ROLLBACK_RECORD_VERSION, RollbackCheckResult,
    RollbackRecord, advance_min_counter, check_rollback,
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

/// On-disk version of `BootControlBlock`.
///
/// 2 (RFC-0.32-002): the CRC is computed over an explicit field serialisation
/// rather than over the struct's memory, so the checksum bytes differ from
/// version 1's for an otherwise identical block.
pub const BOOT_CONTROL_VERSION: u16 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotId {
    A = 0,
    B = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SlotState {
    Empty = 0,
    Bootable = 1,
    Candidate = 2,
    Confirmed = 3,
    Failed = 4,
}

impl SlotState {
    /// The byte this state is written as. The match is exhaustive, so a new
    /// state is a compile error here rather than a value with no encoding.
    pub const fn as_u8(self) -> u8 {
        match self {
            SlotState::Empty => 0,
            SlotState::Bootable => 1,
            SlotState::Candidate => 2,
            SlotState::Confirmed => 3,
            SlotState::Failed => 4,
        }
    }
}

/// Width of `SlotInfo`'s checksum serialisation.
const SLOT_INFO_CHECKSUM_BYTES: usize = 1 + 8 + 1 + 1 + 1;

/// Width of `BootControlBlock`'s checksum serialisation: every field except
/// `crc32`, summed by hand so a field added without a line in
/// `checksum_input` fails the `debug_assert_eq!` there.
const BCB_CHECKSUM_BYTES: usize = 8 + 2 + 8 + 1 + 1 + 1 + SLOT_INFO_CHECKSUM_BYTES * 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotInfo {
    pub state: SlotState,
    pub image_generation: u64,
    pub confirmed: u8,
    pub tries_allowed: u8,
    pub remaining_tries: u8,
}

impl SlotInfo {
    pub const fn empty() -> Self {
        SlotInfo {
            state: SlotState::Empty,
            image_generation: 0,
            confirmed: 0,
            tries_allowed: 3,
            remaining_tries: 3,
        }
    }
    /// This slot's fields, named, little-endian, in declaration order.
    fn checksum_input(&self) -> [u8; SLOT_INFO_CHECKSUM_BYTES] {
        let mut out = [0u8; SLOT_INFO_CHECKSUM_BYTES];
        out[0] = self.state.as_u8();
        out[1..9].copy_from_slice(&self.image_generation.to_le_bytes());
        out[9] = self.confirmed;
        out[10] = self.tries_allowed;
        out[11] = self.remaining_tries;
        out
    }

    pub const fn bootable(image_gen: u64) -> Self {
        SlotInfo {
            state: SlotState::Bootable,
            image_generation: image_gen,
            confirmed: 1,
            tries_allowed: 3,
            remaining_tries: 3,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BootControlBlock {
    pub magic: [u8; 8],
    pub version: u16,
    pub generation: u64,
    pub active_slot: u8, // SlotId
    pub last_confirmed_slot: u8,
    pub candidate_slot: u8, // 0xFF = none
    pub slot_a: SlotInfo,
    pub slot_b: SlotInfo,
    pub crc32: u32,
}

pub const NO_CANDIDATE: u8 = 0xFF;

impl BootControlBlock {
    pub fn new(image_gen: u64) -> Self {
        BootControlBlock {
            magic: BOOT_CTL_MAGIC,
            version: BOOT_CONTROL_VERSION,
            generation: image_gen,
            active_slot: SlotId::A as u8,
            last_confirmed_slot: SlotId::A as u8,
            candidate_slot: NO_CANDIDATE,
            slot_a: SlotInfo::bootable(image_gen),
            slot_b: SlotInfo::empty(),
            crc32: 0,
        }
    }

    /// The bytes the CRC is computed over: each field, named, little-endian,
    /// in declaration order, with `crc32` itself excluded.
    ///
    /// This is the serialisation, not a view of the struct's memory. It
    /// contains no padding — `SlotInfo` alone carries twelve bytes of it — so
    /// it does not depend on the layout the compiler chose, and `seal` and
    /// `is_valid` cannot disagree about what it holds.
    fn checksum_input(&self) -> [u8; BCB_CHECKSUM_BYTES] {
        let mut out = [0u8; BCB_CHECKSUM_BYTES];
        let mut at = 0usize;
        {
            let mut put = |bytes: &[u8]| {
                out[at..at + bytes.len()].copy_from_slice(bytes);
                at += bytes.len();
            };
            put(&self.magic);
            put(&self.version.to_le_bytes());
            put(&self.generation.to_le_bytes());
            put(&[
                self.active_slot,
                self.last_confirmed_slot,
                self.candidate_slot,
            ]);
            put(&self.slot_a.checksum_input());
            put(&self.slot_b.checksum_input());
        }
        debug_assert_eq!(at, BCB_CHECKSUM_BYTES);
        out
    }

    /// Compute and store CRC32 (RFC 008).  Call before writing to disk.
    pub fn seal(&mut self) {
        self.crc32 = crc32(&self.checksum_input());
    }

    /// Returns true if magic is correct AND CRC32 matches (RFC 008).
    pub fn is_valid(&self) -> bool {
        self.magic == BOOT_CTL_MAGIC && crc32(&self.checksum_input()) == self.crc32
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
        (true, false) => BcbMirrorSelection::SelectedA(a),
        (false, true) => BcbMirrorSelection::SelectedB(b),
        (true, true) => {
            use core::cmp::Ordering;
            match a.generation.cmp(&b.generation) {
                Ordering::Greater => BcbMirrorSelection::SelectedA(a),
                Ordering::Less => BcbMirrorSelection::SelectedB(b),
                Ordering::Equal => BcbMirrorSelection::BothValidSameGeneration(a),
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpgradeState {
    Created,
    Verified,
    Staging,
    Staged,
    CandidateSet,
    Confirmed,
    Aborted,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_control_block_initial_slot_b_is_empty() {
        let bcb = BootControlBlock::new(1);
        assert_eq!(
            bcb.slot_b.state,
            SlotState::Empty,
            "slot B must start as Empty; it has no staged image"
        );
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
        bcb.seal(); // is_valid() now checks CRC32 (RFC 008)
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
    bcb.version ^= 0xFF; // corrupt one byte
    assert!(!bcb.is_valid(), "corrupted BCB must fail is_valid");
}

/// §C, first test, widened: every field in the serialisation is flipped in
/// turn, so a field left out of `checksum_input` is a failure here rather
/// than a silent hole. `version` above is one of these; the others, including
/// both slots, were unchecked before RFC-0.32-002.
#[test]
fn every_flipped_field_fails_the_crc() {
    let base = {
        let mut bcb = BootControlBlock::new(1);
        bcb.seal();
        bcb
    };
    assert!(base.is_valid());

    let flips: [fn(&mut BootControlBlock); 9] = [
        |b| b.version ^= 0xFF,
        |b| b.generation ^= 0xFF,
        |b| b.active_slot ^= 0xFF,
        |b| b.last_confirmed_slot ^= 0xFF,
        |b| b.candidate_slot ^= 0xFF,
        |b| b.slot_a.state = SlotState::Failed,
        |b| b.slot_a.image_generation ^= 0xFF,
        |b| b.slot_b.remaining_tries ^= 0xFF,
        |b| b.slot_b.tries_allowed ^= 0xFF,
    ];
    for (i, flip) in flips.iter().enumerate() {
        let mut bcb = base;
        flip(&mut bcb);
        assert!(!bcb.is_valid(), "flip {i} was not caught by the CRC");
    }

    // The magic is rejected by its own check, not by the CRC.
    let mut bcb = base;
    bcb.magic = [0u8; 8];
    assert!(!bcb.is_valid());
}

/// §C, second test: a block sealed by this code validates under this code,
/// and carries the version the bytes belong to.
#[test]
fn bcb_sealed_by_new_code_validates_and_states_version_2() {
    let mut bcb = BootControlBlock::new(9);
    bcb.seal();
    assert!(bcb.is_valid());
    assert_eq!(bcb.version, BOOT_CONTROL_VERSION);
    assert_eq!(BOOT_CONTROL_VERSION, 2);
}

/// §C, third test: the old defect's exact shape. `is_valid` used to checksum
/// `let mut copy = *self` — a struct copy, which is not required to reproduce
/// the padding bytes `seal` saw, so a structurally identical block could fail
/// its own CRC. A copy is now indistinguishable from its original, because
/// neither one's padding is read.
#[test]
fn a_copy_of_a_sealed_block_validates() {
    let mut bcb = BootControlBlock::new(11);
    bcb.seal();
    let copy = bcb;
    assert!(copy.is_valid());
    assert_eq!(copy.checksum_input(), bcb.checksum_input());
    assert_eq!(crc32(&copy.checksum_input()), bcb.crc32);
}

/// The checksum input carries no padding: it is the sum of the named fields,
/// well under the 88 bytes the struct occupies.
#[test]
fn bcb_checksum_input_is_smaller_than_the_struct() {
    assert_eq!(BCB_CHECKSUM_BYTES, 45);
    assert_eq!(core::mem::size_of::<BootControlBlock>(), 88);
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
    let mut a = BootControlBlock::new(1);
    a.seal();
    let mut b = BootControlBlock::new(1);
    b.seal();
    b.magic = [0u8; 8]; // corrupt B
    assert!(matches!(
        select_bcb_mirror(&a, &b),
        BcbMirrorSelection::SelectedA(_)
    ));
    assert!(matches!(
        select_bcb_mirror(&b, &a),
        BcbMirrorSelection::SelectedB(_)
    ));
}

#[test]
fn select_bcb_mirror_higher_generation_wins() {
    let mut a = BootControlBlock::new(2);
    a.seal();
    let mut b = BootControlBlock::new(5);
    b.seal();
    assert!(matches!(
        select_bcb_mirror(&a, &b),
        BcbMirrorSelection::SelectedB(_)
    ));
    assert!(matches!(
        select_bcb_mirror(&b, &a),
        BcbMirrorSelection::SelectedA(_)
    ));
}

#[test]
fn select_bcb_mirror_equal_generation_selects_a() {
    let mut a = BootControlBlock::new(3);
    a.seal();
    let mut b = BootControlBlock::new(3);
    b.seal();
    assert!(matches!(
        select_bcb_mirror(&a, &b),
        BcbMirrorSelection::BothValidSameGeneration(_)
    ));
}

#[cfg(test)]
mod tests_v03;
