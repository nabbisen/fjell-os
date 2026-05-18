//! A/B upgrade and boot-control block types for Fjell OS M6.
#![no_std]

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
#[derive(Clone, Copy)]
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
#[derive(Clone, Copy)]
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
