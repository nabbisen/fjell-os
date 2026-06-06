//! Node identity policy (RFC v0.7-001 §6.2).

use fjell_measure_format::Digest32;

/// Controls which remote nodes are accepted as snapshot sources.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum TrustMode {
    /// Only nodes with the same `trust_profile_tag` as self.
    SameFamily = 1,
    /// Any node in the fleet roster (pinned_roster must be set).
    Fleet      = 2,
    /// Accept any node with a valid signature (open federation).
    Open       = 3,
}

impl TrustMode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::SameFamily),
            2 => Some(Self::Fleet),
            3 => Some(Self::Open),
            _ => None,
        }
    }
}

/// Opaque reference to a fleet roster (resolved by the trust provider; v0.8).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RosterRef(pub Digest32);

/// Declarative snapshot-acceptance policy bound to this node.
#[derive(Clone, Copy, Debug)]
pub struct NodeIdentityPolicy {
    pub mode:             TrustMode,
    pub allowed_profiles: [u8; 4],   // trust_profile_tag whitelist (0 = any)
    pub allowed_count:    u8,
    /// Set in Fleet mode; reserved for v0.8 otherwise.
    pub pinned_roster:    Option<RosterRef>,
    pub policy_digest:    Digest32,
}

impl NodeIdentityPolicy {
    /// Default single-node policy (SameFamily, no roster, no profile filter).
    pub fn same_family_default(profile_tag: u8) -> Self {
        Self {
            mode:             TrustMode::SameFamily,
            allowed_profiles: [profile_tag, 0, 0, 0],
            allowed_count:    1,
            pinned_roster:    None,
            policy_digest:    Digest32([0u8; 32]),
        }
    }

    /// Check whether a remote node with `profile_tag` is permitted.
    pub fn permits(&self, profile_tag: u8) -> bool {
        match self.mode {
            TrustMode::Open  => true,
            TrustMode::Fleet => self.pinned_roster.is_some(),
            TrustMode::SameFamily => {
                if self.allowed_count == 0 { return true; }
                self.allowed_profiles[..self.allowed_count as usize]
                    .contains(&profile_tag)
            }
        }
    }
}
