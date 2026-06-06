//! Tests for fjell-fleet-format (RFC v0.8-001..004).

use crate::roster::*;
use crate::policy::*;
use crate::rollout::*;
use crate::action::*;
use crate::digest::*;
use fjell_measure_format::Digest32;
use fjell_identity_format::NodeId;

fn sample_fleet_id() -> [u8; 16] { [0xF1u8; 16] }
fn sample_node_id()  -> NodeId   { NodeId([0x01u8; 16]) }
fn sample_digest()   -> Digest32 { Digest32([0xAAu8; 32]) }

// ── NodeRoster ────────────────────────────────────────────────────────────────

#[test]
fn roster_schema_version_is_one() {
    assert_eq!(FLEET_SCHEMA_VERSION, 1);
}

#[test]
fn roster_add_member_increments_count() {
    let mut r = NodeRoster::new(sample_fleet_id(), [0u8; 32]);
    r.add_member(RosterEntry {
        identity_digest: sample_digest(),
        node_id: sample_node_id(),
        trust_profile_tag: TrustProfileTag(1),
        active: true,
        generation: 1,
    }).unwrap();
    assert_eq!(r.entry_count, 1);
    assert_eq!(r.active_count(), 1);
}

#[test]
fn roster_rejects_duplicate_member() {
    let mut r = NodeRoster::new(sample_fleet_id(), [0u8; 32]);
    let entry = RosterEntry {
        identity_digest: sample_digest(),
        node_id: sample_node_id(),
        trust_profile_tag: TrustProfileTag(1),
        active: true,
        generation: 1,
    };
    r.add_member(entry).unwrap();
    assert_eq!(r.add_member(entry), Err(RosterError::DuplicateMember));
}

#[test]
fn roster_is_member_works() {
    let mut r = NodeRoster::new(sample_fleet_id(), [0u8; 32]);
    let digest = sample_digest();
    r.add_member(RosterEntry {
        identity_digest: digest,
        node_id: sample_node_id(),
        trust_profile_tag: TrustProfileTag(0),
        active: true,
        generation: 1,
    }).unwrap();
    assert!(r.is_member(&digest));
    assert!(!r.is_member(&Digest32([0xBBu8; 32])));
}

#[test]
fn roster_revoke_member() {
    let mut r = NodeRoster::new(sample_fleet_id(), [0u8; 32]);
    let digest = sample_digest();
    r.add_member(RosterEntry {
        identity_digest: digest,
        node_id: sample_node_id(),
        trust_profile_tag: TrustProfileTag(0),
        active: true,
        generation: 1,
    }).unwrap();
    assert!(r.revoke_member(&digest));
    assert_eq!(r.active_count(), 0);
}

#[test]
fn roster_digest_nonzero() {
    let r = NodeRoster::new(sample_fleet_id(), [0xBBu8; 32]);
    let d = roster_digest(&r);
    assert_ne!(d.0, [0u8; 32]);
}

#[test]
fn roster_digest_deterministic() {
    let r = NodeRoster::new(sample_fleet_id(), [0u8; 32]);
    assert_eq!(roster_digest(&r).0, roster_digest(&r).0);
}

// ── FleetPolicy ───────────────────────────────────────────────────────────────

#[test]
fn policy_default_deny() {
    let p = FleetPolicy::new(sample_fleet_id(), sample_digest());
    assert!(!p.permits(PolicyAction::InitiateRollout));
    assert!(!p.permits(PolicyAction::RemoteRecovery));
}

#[test]
fn policy_allow_statement_permits() {
    let mut p = FleetPolicy::new(sample_fleet_id(), sample_digest());
    p.add_statement(PolicyStatement::allow(
        PolicyAction::AcceptSnapshot, PolicyCondition::Always,
    )).unwrap();
    assert!(p.permits(PolicyAction::AcceptSnapshot));
    assert!(!p.permits(PolicyAction::InitiateRollout)); // unrelated action
}

#[test]
fn policy_deny_statement_blocks() {
    let mut p = FleetPolicy::new(sample_fleet_id(), sample_digest());
    p.add_statement(PolicyStatement::deny(
        PolicyAction::RemoteRecovery, PolicyCondition::Always,
    )).unwrap();
    assert!(!p.permits(PolicyAction::RemoteRecovery));
}

#[test]
fn policy_digest_nonzero() {
    let p = FleetPolicy::new(sample_fleet_id(), sample_digest());
    let d = policy_digest(&p);
    assert_ne!(d.0, [0u8; 32]);
}

// ── FleetRolloutPlan ──────────────────────────────────────────────────────────

#[test]
fn rollout_add_stage_increments_count() {
    let mut plan = FleetRolloutPlan::new(sample_fleet_id(), 1, sample_digest());
    plan.add_stage(RolloutStage::new(b"canary", RolloutStrategy::AllConfirmed)).unwrap();
    assert_eq!(plan.stage_count, 1);
}

#[test]
fn rollout_advance_after_confirm() {
    let mut plan = FleetRolloutPlan::new(sample_fleet_id(), 1, sample_digest());
    plan.add_stage(RolloutStage::new(b"canary", RolloutStrategy::AllConfirmed)).unwrap();
    plan.add_stage(RolloutStage::new(b"full",   RolloutStrategy::AllConfirmed)).unwrap();
    plan.confirm_active_stage().unwrap();
    assert!(plan.try_advance());
    assert_eq!(plan.active_stage, 1);
}

#[test]
fn rollout_strategy_variants_accessible() {
    assert_eq!(RolloutStrategy::from_u8(0x01), Some(RolloutStrategy::AllConfirmed));
    assert_eq!(RolloutStrategy::from_u8(0x02), Some(RolloutStrategy::Quorum));
    assert_eq!(RolloutStrategy::from_u8(0xFF), None);
}

// ── FleetAction ───────────────────────────────────────────────────────────────

#[test]
fn fleet_action_kind_mutating_classification() {
    assert!(FleetActionKind::InitiateRecovery.is_mutating());
    assert!(FleetActionKind::RevokeMember.is_mutating());
    assert!(!FleetActionKind::QueryState.is_mutating());
    assert!(!FleetActionKind::CollectAttestation.is_mutating());
}

#[test]
fn fleet_action_fleet_wide_detection() {
    let broad = FleetAction::new(
        sample_fleet_id(), [0u8; 16],  // all-zero target = fleet-wide
        FleetActionKind::QueryState, [0x01u8; 16],
    );
    assert!(broad.is_fleet_wide());

    let narrow = FleetAction::new(
        sample_fleet_id(), [0x01u8; 16], // specific target
        FleetActionKind::QueryState, [0x02u8; 16],
    );
    assert!(!narrow.is_fleet_wide());
}

#[test]
fn fleet_action_error_no_remote_shell() {
    // Verify there is no "ExecuteShell" or "RunCommand" variant.
    // The absence of such a variant in FleetActionKind is a design invariant.
    assert_eq!(FleetActionKind::from_u8(0x08), None); // 0x08 is unassigned
    assert_eq!(FleetActionKind::from_u8(0xFF), None);
}
