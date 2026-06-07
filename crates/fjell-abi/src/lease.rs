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

// ── Pure lease-decision logic (RFC-v0.17-003) ──────────────────────────────────
//
// These are the pure predicates the kernel lease table implements
// (crates/fjell-kernel/src/lease/mod.rs). They are extracted here, in a
// host-testable crate, so the Verus model
// (verification/verus/lease/lease_epoch.rs) and the kernel share one
// source of truth that ordinary `cargo test` can exercise.
//
// LEASE-VERUS-001: a binding is usable iff the lease is active and the
//                  current epoch equals the epoch recorded at issue.
// LEASE-VERUS-002: revoke increments the epoch.

/// Is a capability bound at `epoch_at_issue` usable against a lease that is
/// `active` with `current_epoch`?  Mirrors `LeaseTable::check_active`.
#[inline]
pub fn lease_usable(active: bool, current_epoch: u32, epoch_at_issue: u32) -> bool {
    active && current_epoch == epoch_at_issue
}

/// The epoch after a revoke.  Mirrors `LeaseTable::revoke`'s
/// `slot.epoch = slot.epoch.wrapping_add(1)`.
///
/// Note: the kernel uses `wrapping_add` on `u32`; the Verus model uses
/// unbounded `nat` and proves strict monotonicity. Wraparound would require
/// 2^32 revocations of a single lease; see RFC-v0.17-003 conformance note.
#[inline]
pub fn lease_revoke_epoch(current_epoch: u32) -> u32 {
    current_epoch.wrapping_add(1)
}
