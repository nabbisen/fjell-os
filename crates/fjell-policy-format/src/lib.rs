//! Policy statement and bundle wire formats (RFC v0.8-006).
//!
//! `PolicyBundle` is a signed set of `PolicyStatement`s that governs which
//! capabilities may be installed and which fleet actions are permitted.
//! It is verified by cap-broker on startup and reloaded on policy update.
#![no_std]

use fjell_measure_format::Digest32;

pub const POLICY_BUNDLE_SCHEMA_VERSION: u16 = 1;
pub const MAX_BUNDLE_STATEMENTS: usize = 128;

/// The subject a policy statement addresses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PolicySubject {
    /// Capability installation (which kinds are permitted).
    CapInstall      = 0x01,
    /// Capability delegation (which rights may be narrowed).
    CapDelegate     = 0x02,
    /// Fleet action execution.
    FleetAction     = 0x03,
    /// Service spawn permission.
    ServiceSpawn    = 0x04,
    /// Trust-provider registration.
    TrustProvider   = 0x05,
}

impl PolicySubject {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::CapInstall),
            0x02 => Some(Self::CapDelegate),
            0x03 => Some(Self::FleetAction),
            0x04 => Some(Self::ServiceSpawn),
            0x05 => Some(Self::TrustProvider),
            _    => None,
        }
    }
}

/// Effect of a policy statement.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PolicyEffect {
    Allow = 0x01,
    Deny  = 0x02,
}

/// One policy rule in the bundle.
#[derive(Clone, Copy, Debug)]
pub struct PolicyStatement {
    pub subject:        PolicySubject,
    /// Object discriminant (e.g., CapKind u8, FleetActionKind u8).
    pub object_tag:     u8,
    pub effect:         PolicyEffect,
    /// Semantic audit intent tag emitted when this rule fires.
    pub audit_tag:      u16,
}

impl PolicyStatement {
    pub const fn allow(subject: PolicySubject, object_tag: u8) -> Self {
        Self { subject, object_tag, effect: PolicyEffect::Allow, audit_tag: 0 }
    }
    pub const fn deny(subject: PolicySubject, object_tag: u8) -> Self {
        Self { subject, object_tag, effect: PolicyEffect::Deny, audit_tag: 0 }
    }
}

/// A signed collection of policy statements.
#[derive(Clone, Debug)]
pub struct PolicyBundle {
    pub schema_version:   u16,
    /// Domain this bundle applies to (e.g. fleet_id or service_id).
    pub domain_id:        [u8; 16],
    pub generation:       u32,
    /// Canonical digest of this bundle (signed by the policy anchor).
    pub bundle_digest:    Digest32,
    /// Ed25519 signature by the policy anchor.
    pub signature:        [u8; 64],
    pub statement_count:  u16,
    pub statements:       [Option<PolicyStatement>; MAX_BUNDLE_STATEMENTS],
}

impl PolicyBundle {
    pub fn new(domain_id: [u8; 16]) -> Self {
        Self {
            schema_version:  POLICY_BUNDLE_SCHEMA_VERSION,
            domain_id,
            generation:      1,
            bundle_digest:   Digest32([0u8; 32]),
            signature:       [0u8; 64],
            statement_count: 0,
            statements:      [const { None }; MAX_BUNDLE_STATEMENTS],
        }
    }

    /// Add a policy statement.
    pub fn add(&mut self, s: PolicyStatement) -> Result<(), BundleError> {
        if self.statement_count as usize >= MAX_BUNDLE_STATEMENTS {
            return Err(BundleError::CapacityExhausted);
        }
        self.statements[self.statement_count as usize] = Some(s);
        self.statement_count += 1;
        Ok(())
    }

    /// Evaluate: is `subject × object_tag` permitted?
    /// First matching rule wins; default deny.
    pub fn permits(&self, subject: PolicySubject, object_tag: u8) -> bool {
        for stmt in self.statements[..self.statement_count as usize].iter() {
            if let Some(s) = stmt {
                if s.subject == subject && (s.object_tag == object_tag || s.object_tag == 0xFF) {
                    return s.effect == PolicyEffect::Allow;
                }
            }
        }
        false // default deny
    }
}

/// Typed error for bundle operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum BundleError {
    CapacityExhausted = 0x01,
    DigestMismatch    = 0x02,
    SignatureInvalid  = 0x03,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_default_deny() {
        let b = PolicyBundle::new([0u8; 16]);
        assert!(!b.permits(PolicySubject::CapInstall, 0x05));
    }

    #[test]
    fn bundle_allow_statement_permits() {
        let mut b = PolicyBundle::new([0u8; 16]);
        b.add(PolicyStatement::allow(PolicySubject::CapInstall, 0x05)).unwrap();
        assert!(b.permits(PolicySubject::CapInstall, 0x05));
        assert!(!b.permits(PolicySubject::CapInstall, 0x06));
    }

    #[test]
    fn bundle_wildcard_object_tag() {
        let mut b = PolicyBundle::new([0u8; 16]);
        // object_tag = 0xFF means wildcard
        b.add(PolicyStatement::allow(PolicySubject::ServiceSpawn, 0xFF)).unwrap();
        assert!(b.permits(PolicySubject::ServiceSpawn, 0x01));
        assert!(b.permits(PolicySubject::ServiceSpawn, 0xFF));
    }

    #[test]
    fn bundle_deny_overrides() {
        let mut b = PolicyBundle::new([0u8; 16]);
        b.add(PolicyStatement::deny(PolicySubject::FleetAction, 0x02)).unwrap();
        assert!(!b.permits(PolicySubject::FleetAction, 0x02));
    }

    #[test]
    fn policy_subject_roundtrip() {
        for (v, expected) in [
            (0x01u8, PolicySubject::CapInstall),
            (0x03,   PolicySubject::FleetAction),
            (0x05,   PolicySubject::TrustProvider),
        ] {
            assert_eq!(PolicySubject::from_u8(v).unwrap(), expected);
        }
        assert_eq!(PolicySubject::from_u8(0x00), None);
    }
}
