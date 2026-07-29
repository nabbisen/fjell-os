//! `PlatformProfile` and `BoardProfile` wire formats for Fjell OS.
//!
//! RFC v0.5-001: externalises the hardware description that was previously
//! hard-coded in `devmgr`.  Both profiles are content-addressable; their
//! SHA-256 digests are bound into the measurement chain and the v0.3
//! attestation record.
#![no_std]

pub mod board;
pub mod digest;
pub mod isa;
pub mod platform;

pub use board::{
    BOARD_PROFILE_VERSION, BoardDevice, BoardProfile, DeviceClass, MAX_BOARD_DEVICES,
    RecoveryDescriptor, RecoveryKind,
};
pub use digest::{board_digest, platform_digest};
pub use platform::{
    ISA_EXT_A, ISA_EXT_C, ISA_EXT_D, ISA_EXT_F, ISA_EXT_I, ISA_EXT_M, ISA_EXT_ZBB, ISA_EXT_ZICSR,
    ISA_EXT_ZIFENCEI, ISA_MANDATORY, IsaExtensions, KernelAbiVersion, MemMap,
    PLATFORM_PROFILE_VERSION, PlatformFamily, PlatformProfile, PlicLayout,
};

#[cfg(test)]
mod tests;
