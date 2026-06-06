//! Node identity policy (RFC v0.7-001 §6.2, RFC-v0.7.2-003).

use fjell_measure_format::Digest32;

/// Controls which remote nodes are accepted as snapshot sources.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum TrustMode {
    /// Only nodes with the same `trust_profile_tag` as self.
    SameFamily = 1,
    /// Any node in the fleet roster (pinned_roster must be set and validated).
    Fleet      = 2,
    /// Accept any node with a valid signature (open federation).
    /// Requires `trust-mode-open` feature flag (RFC-v0.7.2-003).
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

/// Result of a `permits()` call (RFC-v0.7.2-003, replaces bare `bool`).
///
/// The caller is responsible for acting correctly on each variant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decision {
    /// The policy permits this profile.
    Allow,
    /// `TrustMode::Open` — policy permits any profile, but this is insecure.
    /// Requires the `trust-mode-open` feature; otherwise callers MUST reject.
    AllowInsecure,
    /// `TrustMode::Fleet` — roster validation is required before accepting.
    NeedsRosterValidation(RosterRef),
    /// The policy denies this profile.
    Deny,
}

/// Typed policy error (RFC-v0.7.2-003).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PolicyError {
    /// `allowed_count` exceeds the `allowed_profiles` array length.
    AllowedCountOverflow = 0x01,
    /// `TrustMode::Fleet` requires `pinned_roster` to be `Some`.
    FleetWithoutRoster   = 0x02,
}

/// Declarative snapshot-acceptance policy bound to this node.
#[derive(Clone, Copy, Debug)]
pub struct NodeIdentityPolicy {
    pub mode:             TrustMode,
    pub allowed_profiles: [u8; 4],   // trust_profile_tag allowlist (0 = any)
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

    /// Validate the policy for internal consistency (RFC-v0.7.2-003).
    ///
    /// Must be called before trusting `allowed_count` in slice operations.
    pub fn validate(&self) -> Result<(), PolicyError> {
        if self.allowed_count as usize > self.allowed_profiles.len() {
            return Err(PolicyError::AllowedCountOverflow);
        }
        if matches!(self.mode, TrustMode::Fleet) && self.pinned_roster.is_none() {
            return Err(PolicyError::FleetWithoutRoster);
        }
        Ok(())
    }

    /// Return a `Decision` for the given `profile_tag` — never panics
    /// (RFC-v0.7.2-003, closes C-H-02).
    ///
    /// Returns `Decision::Deny` if the policy is invalid (e.g., malformed
    /// `allowed_count`). The caller must match on the returned `Decision`.
    pub fn permits(&self, profile_tag: u8) -> Decision {
        // Validate first; any invalid policy → Deny.
        if self.validate().is_err() {
            return Decision::Deny;
        }
        match self.mode {
            TrustMode::Open => Decision::AllowInsecure,
            TrustMode::Fleet => {
                // Roster ref is guaranteed Some by validate().
                Decision::NeedsRosterValidation(self.pinned_roster.unwrap())
            }
            TrustMode::SameFamily => {
                if self.allowed_count == 0 {
                    return Decision::Allow;
                }
                // Slice is safe: validate() confirmed allowed_count <= 4.
                let n = self.allowed_count as usize;
                if self.allowed_profiles[..n].contains(&profile_tag) {
                    Decision::Allow
                } else {
                    Decision::Deny
                }
            }
        }
    }
}

// ── Fleet roster validation placeholder (RFC-v0.7.2-003, v0.8 target) ─────────

/// Result of a roster membership check.
///
/// v0.7.x: always returns `NotValidated` because roster signature verification
/// requires fleet identity infrastructure not yet implemented.
/// v0.8: will actually verify the roster digest against the keyring.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RosterCheckResult {
    /// Membership confirmed (v0.8 only; not reachable in v0.7.x).
    Confirmed,
    /// Roster validation not yet implemented; treat as Deny.
    NotValidated,
    /// The roster reference does not match any known fleet.
    UnknownRoster,
}

/// Check whether a remote node is a member of the fleet roster.
///
/// v0.7.x stub: always returns `NotValidated`.
/// Callers MUST treat `NotValidated` as a denial (fail-closed).
pub fn check_roster_membership(
    _roster_ref: RosterRef,
    _node_profile_tag: u8,
) -> RosterCheckResult {
    RosterCheckResult::NotValidated
}

#[cfg(test)]
mod fleet_stub_tests {
    use super::*;
    use fjell_measure_format::Digest32;

    #[test]
    fn roster_check_returns_not_validated() {
        let roster = RosterRef(Digest32([0xAAu8; 32]));
        let result = check_roster_membership(roster, 0x01);
        assert_eq!(result, RosterCheckResult::NotValidated);
    }

    #[test]
    fn fleet_mode_decision_requires_roster_validation() {
        let p = NodeIdentityPolicy {
            mode:             TrustMode::Fleet,
            allowed_profiles: [0; 4],
            allowed_count:    0,
            pinned_roster:    Some(RosterRef(Digest32([0xBBu8; 32]))),
            policy_digest:    Digest32([0u8; 32]),
        };
        // Fleet must return NeedsRosterValidation, not Allow
        let d = p.permits(0x01);
        assert!(matches!(d, Decision::NeedsRosterValidation(_)));

        // Callers treating NeedsRosterValidation as a pass-through check roster:
        if let Decision::NeedsRosterValidation(roster_ref) = d {
            let result = check_roster_membership(roster_ref, 0x01);
            // In v0.7.x: always NotValidated = deny
            assert_eq!(result, RosterCheckResult::NotValidated);
        }
    }
}
