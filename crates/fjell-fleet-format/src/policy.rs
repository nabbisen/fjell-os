//! Fleet policy wire format (RFC v0.8-002).
//!
//! A `FleetPolicy` is the signed set of rules governing what operations are
//! permitted across the fleet. It is verified before any `FleetAction` is
//! executed.

use fjell_measure_format::Digest32;

pub const FLEET_POLICY_SCHEMA_VERSION: u16 = 1;
pub const MAX_POLICY_STATEMENTS: usize = 32;

/// What semantic action a policy statement covers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PolicyAction {
    /// Replace a trust provider (requires SignedPolicyAuth in Enforcing mode).
    ReplaceProvider    = 0x01,
    /// Remove a trust provider.
    RemoveProvider     = 0x02,
    /// Initiate a fleet-wide upgrade rollout.
    InitiateRollout    = 0x03,
    /// Revoke a fleet member.
    RevokeMember       = 0x04,
    /// Accept a snapshot import from a fleet member.
    AcceptSnapshot     = 0x05,
    /// Send remote diagnostics request.
    RemoteDiag         = 0x06,
    /// Initiate remote recovery.
    RemoteRecovery     = 0x07,
}

impl PolicyAction {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::ReplaceProvider),
            0x02 => Some(Self::RemoveProvider),
            0x03 => Some(Self::InitiateRollout),
            0x04 => Some(Self::RevokeMember),
            0x05 => Some(Self::AcceptSnapshot),
            0x06 => Some(Self::RemoteDiag),
            0x07 => Some(Self::RemoteRecovery),
            _    => None,
        }
    }
}

/// Condition under which a policy statement applies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PolicyCondition {
    /// Always applies.
    Always              = 0x00,
    /// Only during a scheduled maintenance window.
    MaintenanceWindow   = 0x01,
    /// Only when the local health check passes.
    HealthyNode         = 0x02,
    /// Only when the rollout stage allows it.
    RolloutStagePermits = 0x03,
}

/// One policy statement: Action × Condition × Allow/Deny.
#[derive(Clone, Copy, Debug)]
pub struct PolicyStatement {
    pub action:    PolicyAction,
    pub condition: PolicyCondition,
    /// If false, this is a deny rule.
    pub allow:     bool,
    /// Audit semantic intent tag emitted when this rule fires.
    pub audit_tag: u16,
}

impl PolicyStatement {
    pub const fn allow(action: PolicyAction, condition: PolicyCondition) -> Self {
        Self { action, condition, allow: true,  audit_tag: 0 }
    }
    pub const fn deny(action: PolicyAction, condition: PolicyCondition) -> Self {
        Self { action, condition, allow: false, audit_tag: 0 }
    }
}

/// Fleet-wide governance policy.
#[derive(Clone, Debug)]
pub struct FleetPolicy {
    pub schema_version:    u16,
    pub fleet_id:          [u8; 16],
    pub policy_generation: u32,
    /// Digest of the current `NodeRoster` this policy applies to.
    pub roster_digest:     Digest32,
    /// Canonical digest (computed by `policy_digest`).
    pub policy_digest:     Digest32,
    pub statement_count:   u16,
    pub statements:        [Option<PolicyStatement>; MAX_POLICY_STATEMENTS],
}

impl FleetPolicy {
    pub fn new(fleet_id: [u8; 16], roster_digest: Digest32) -> Self {
        Self {
            schema_version:    FLEET_POLICY_SCHEMA_VERSION,
            fleet_id,
            policy_generation: 1,
            roster_digest,
            policy_digest:     Digest32([0u8; 32]),
            statement_count:   0,
            statements:        [const { None }; MAX_POLICY_STATEMENTS],
        }
    }

    /// Add a policy statement.
    pub fn add_statement(&mut self, s: PolicyStatement) -> Result<(), PolicyError> {
        if self.statement_count as usize >= MAX_POLICY_STATEMENTS {
            return Err(PolicyError::CapacityExhausted);
        }
        self.statements[self.statement_count as usize] = Some(s);
        self.statement_count += 1;
        Ok(())
    }

    /// Evaluate: is `action` permitted under current conditions?
    ///
    /// Returns the first matching statement (statements are evaluated in
    /// insertion order). Denies by default if no statement matches.
    pub fn permits(&self, action: PolicyAction) -> bool {
        for stmt in self.statements[..self.statement_count as usize].iter() {
            if let Some(s) = stmt {
                if s.action == action {
                    return s.allow;
                }
            }
        }
        false // default deny
    }

    /// Strict evaluation with condition check.
    pub fn permits_under(&self, action: PolicyAction, cond: PolicyCondition) -> bool {
        for stmt in self.statements[..self.statement_count as usize].iter() {
            if let Some(s) = stmt {
                if s.action == action
                    && (s.condition == PolicyCondition::Always || s.condition == cond)
                {
                    return s.allow;
                }
            }
        }
        false
    }
}

/// Typed error for policy operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PolicyError {
    CapacityExhausted = 0x01,
    ActionNotFound    = 0x02,
    DigestMismatch    = 0x03,
}
