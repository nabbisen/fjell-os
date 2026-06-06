//! Node roster — the signed set of fleet members (RFC v0.8-001).
//!
//! A `NodeRoster` is the authoritative list of nodes that belong to a fleet.
//! It is signed by the fleet policy anchor and verified by every node before
//! accepting snapshot imports from peers.

use fjell_measure_format::Digest32;
use fjell_identity_format::NodeId;

pub const FLEET_SCHEMA_VERSION: u16 = 1;
pub const MAX_ROSTER_ENTRIES:  usize = 64;
/// StoreRecordKind for a persisted `NodeRoster`.
pub const STORE_RECORD_KIND_ROSTER: u16 = 0x0040;

/// Opaque reference to a fleet roster (hash of the canonical encoding).
///
/// Re-exported here to provide a concrete implementation alongside
/// the placeholder used in v0.7.x.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RosterRef(pub Digest32);

impl RosterRef {
    pub fn from_digest(d: Digest32) -> Self { Self(d) }
}

/// Trust profile tag (matches `NodeIdentity::trust_profile_tag`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TrustProfileTag(pub u8);

/// One member in the fleet roster.
#[derive(Clone, Copy, Debug, Default)]
pub struct RosterEntry {
    /// The node's canonical identity digest.
    pub identity_digest:   Digest32,
    /// The node's `NodeId`.
    pub node_id:           NodeId,
    /// Trust profile tag for this member.
    pub trust_profile_tag: TrustProfileTag,
    /// Whether this entry is still active (not revoked).
    pub active:            bool,
    /// Generation at which this entry was added (for conflict resolution).
    pub generation:        u32,
}

impl RosterEntry {
    pub const fn empty() -> Self {
        Self {
            identity_digest:   Digest32([0u8; 32]),
            node_id:           NodeId([0u8; 16]),
            trust_profile_tag: TrustProfileTag(0),
            active:            false,
            generation:        0,
        }
    }
}

/// The fleet node roster.
///
/// This is the v0.8 implementation of the `RosterRef` target deferred in
/// v0.7.x. The `fjell-identity-format::RosterRef` digest identifies a
/// specific `NodeRoster` record stored in storaged.
#[derive(Clone, Debug)]
pub struct NodeRoster {
    pub schema_version: u16,
    pub fleet_id:       [u8; 16],
    pub generation:     u32,
    /// Ed25519 public key of the fleet policy anchor (signs the roster).
    pub anchor_pubkey:  [u8; 32],
    /// Canonical digest of this roster (computed by `roster_digest`).
    pub roster_digest:  Digest32,
    pub entry_count:    u16,
    pub entries:        [RosterEntry; MAX_ROSTER_ENTRIES],
}

impl NodeRoster {
    pub fn new(fleet_id: [u8; 16], anchor_pubkey: [u8; 32]) -> Self {
        Self {
            schema_version: FLEET_SCHEMA_VERSION,
            fleet_id,
            generation: 1,
            anchor_pubkey,
            roster_digest: Digest32([0u8; 32]),
            entry_count: 0,
            entries: [const { RosterEntry::empty() }; MAX_ROSTER_ENTRIES],
        }
    }

    /// Add a member to the roster.
    /// Returns `Err(())` if the roster is at capacity or the node is already present.
    pub fn add_member(&mut self, entry: RosterEntry) -> Result<(), RosterError> {
        if self.entry_count as usize >= MAX_ROSTER_ENTRIES {
            return Err(RosterError::CapacityExhausted);
        }
        // Reject duplicate identity_digest.
        if self.entries[..self.entry_count as usize]
            .iter()
            .any(|e| e.identity_digest.0 == entry.identity_digest.0 && e.active)
        {
            return Err(RosterError::DuplicateMember);
        }
        self.entries[self.entry_count as usize] = entry;
        self.entry_count += 1;
        Ok(())
    }

    /// Check if a node with the given identity_digest is an active member.
    pub fn is_member(&self, identity_digest: &Digest32) -> bool {
        self.entries[..self.entry_count as usize]
            .iter()
            .any(|e| e.active && e.identity_digest.0 == identity_digest.0)
    }

    /// Revoke a member by identity_digest. Returns true if the member was found.
    pub fn revoke_member(&mut self, identity_digest: &Digest32) -> bool {
        for e in self.entries[..self.entry_count as usize].iter_mut() {
            if e.identity_digest.0 == identity_digest.0 {
                e.active = false;
                return true;
            }
        }
        false
    }

    /// Active member count.
    pub fn active_count(&self) -> usize {
        self.entries[..self.entry_count as usize]
            .iter()
            .filter(|e| e.active)
            .count()
    }

    /// Returns the `RosterRef` (digest-based handle) for this roster.
    pub fn as_roster_ref(&self) -> RosterRef {
        RosterRef(self.roster_digest)
    }
}

/// Typed error for roster operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum RosterError {
    CapacityExhausted = 0x01,
    DuplicateMember   = 0x02,
    MemberNotFound    = 0x03,
    DigestMismatch    = 0x04,
    InvalidSchema     = 0x05,
}
