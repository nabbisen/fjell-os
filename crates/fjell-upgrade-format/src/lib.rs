//! A/B upgrade and boot-control block types for Fjell OS M6.
#![no_std]

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
    pub fn new(generation: u64) -> Self {
        BootControlBlock {
            magic: BOOT_CTL_MAGIC, version: 1, generation,
            active_slot: SlotId::A as u8,
            last_confirmed_slot: SlotId::A as u8,
            candidate_slot: NO_CANDIDATE,
            slot_a: SlotInfo::bootable(generation),
            slot_b: SlotInfo::empty(),
            crc32: 0,
        }
    }
    pub fn is_valid(&self) -> bool { self.magic == BOOT_CTL_MAGIC }
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
        let bcb = BootControlBlock::new(1);
        assert!(bcb.is_valid(), "freshly constructed BCB must have valid magic");
    }
}
