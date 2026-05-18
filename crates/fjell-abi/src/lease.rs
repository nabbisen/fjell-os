//! Lease-based capability delegation types.
//!
//! A `LeaseId` is an opaque handle to a kernel-managed lease object.  The
//! kernel tracks a monotonic `epoch` per lease; capabilities bound to a
//! lease carry the epoch at which they were issued.  When the lease is
//! revoked (`LeaseRevoke` syscall), the epoch is incremented and all
//! previously issued bound capabilities fail future checks.

/// Opaque lease identifier.
///
/// Packed into a `u32` for ABI efficiency: upper 16 bits = generation
/// (prevents handle reuse), lower 16 bits = slot index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct LeaseId(pub u32);

impl LeaseId {
    pub const INVALID: LeaseId = LeaseId(u32::MAX);

    #[inline]
    pub fn new(index: u16, generation: u16) -> Self {
        LeaseId(((generation as u32) << 16) | (index as u32))
    }

    #[inline]
    pub fn index(self) -> u16 { (self.0 & 0xFFFF) as u16 }

    #[inline]
    pub fn generation(self) -> u16 { (self.0 >> 16) as u16 }

    #[inline]
    pub fn is_valid(self) -> bool { self != Self::INVALID }
}

/// Packed lease epoch checked during capability validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct LeaseEpoch(pub u32);
