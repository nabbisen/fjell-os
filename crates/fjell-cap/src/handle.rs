//! Generation-tagged capability handle.
//!
//! A `CapHandle` is a 32-bit value: upper 16 bits = generation counter,
//! lower 16 bits = slot index.  When a slot is recycled, the generation is
//! bumped so that old handles become detectably invalid.
//!
//! Invariant CAP-C: a `CapHandle` whose generation does not match the slot's
//! current generation must never grant access to the new occupant.

/// An opaque capability handle passed through the syscall ABI.
///
/// `CapHandle(u32::MAX)` is the canonical null / invalid handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct CapHandle(pub u32);

impl CapHandle {
    /// The null handle (never valid).
    pub const NULL: Self = CapHandle(u32::MAX);

    /// Construct a handle from slot index and generation.
    #[inline]
    pub const fn new(slot: u16, generation: u16) -> Self {
        CapHandle(((generation as u32) << 16) | (slot as u32))
    }

    /// Slot index.
    #[inline]
    pub fn slot(self) -> u16 {
        (self.0 & 0xFFFF) as u16
    }

    /// Generation counter.
    #[inline]
    pub fn generation(self) -> u16 {
        (self.0 >> 16) as u16
    }

    /// Is this the null handle?
    #[inline]
    pub fn is_null(self) -> bool {
        self == Self::NULL
    }
}
