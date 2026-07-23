//! `PlatformProfile` and `BoardProfile` wire formats for Fjell OS.
//!
//! RFC v0.5-001: externalises the hardware description that was previously
//! hard-coded in `devmgr`.  Both profiles are content-addressable; their
//! SHA-256 digests are bound into the measurement chain and the v0.3
//! attestation record.
#![no_std]

pub mod platform;
pub mod board;
pub mod isa;
pub mod digest;

pub use platform::{
    PlatformProfile, PlatformFamily, IsaExtensions, KernelAbiVersion,
    MemMap, PlicLayout, PLATFORM_PROFILE_VERSION,
    ISA_EXT_I, ISA_EXT_M, ISA_EXT_A, ISA_EXT_F, ISA_EXT_D,
    ISA_EXT_C, ISA_EXT_ZBB, ISA_EXT_ZICSR, ISA_EXT_ZIFENCEI,
    ISA_MANDATORY,
};
pub use board::{
    BoardProfile, BoardDevice, DeviceClass, RecoveryDescriptor, RecoveryKind,
    BOARD_PROFILE_VERSION, MAX_BOARD_DEVICES,
};
pub use digest::{platform_digest, board_digest};

#[cfg(test)]
mod tests;
