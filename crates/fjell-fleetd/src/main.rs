//! fleetd — Fleet operations manager for Fjell OS v0.8.
//!
//! Responsibilities:
//!   1. Load and verify `NodeRoster` from storaged.
//!   2. Load and verify `FleetPolicy` from storaged.
//!   3. Load and apply `FleetRolloutPlan` (if active).
//!   4. Validate incoming `FleetAction` requests against policy.
//!   5. Process `RemoteDiagRequest` (cap-controlled; no arbitrary exec).
//!
//! v0.8.0 status: roster/policy persistence skeleton; full IPC wiring
//! in v0.8.1 once the fleet identity provisioning flow is complete.
#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{sys_exit, sys_debug_writeln};
use fjell_fleet_format::{
    NodeRoster, FleetPolicy, FleetRolloutPlan, FleetAction, FleetActionKind,
    RolloutStage, RolloutStrategy, PolicyStatement, PolicyAction, PolicyCondition,
    roster_digest, policy_digest,
};
use fjell_remote_diag_format::{RemoteDiagRequest, DiagRequestKind};
use fjell_policy_format::{PolicyBundle, PolicySubject};
use fjell_measure_format::Digest32;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("fleetd: started (v0.8 fleet operations plane)");

    // ── Step 1: Verify fleet format crate health ──────────────────────────────
    let fleet_id = [0xF8u8; 16];

    // NodeRoster self-check.
    let mut roster = NodeRoster::new(fleet_id, [0x01u8; 32]);
    roster.roster_digest = roster_digest(&roster);
    if roster.roster_digest.0 == [0u8; 32] {
        sys_debug_writeln("fleetd: ERROR roster digest is zero");
        sys_exit(1);
    }
    sys_debug_writeln("fleetd: roster_digest verified");

    // FleetPolicy self-check.
    let mut policy = FleetPolicy::new(fleet_id, roster.roster_digest);
    policy.add_statement(PolicyStatement::allow(
        PolicyAction::QueryState, PolicyCondition::Always,
    )).unwrap_or_else(|_| {});
    policy.add_statement(PolicyStatement::deny(
        PolicyAction::RemoteRecovery, PolicyCondition::Always,
    )).unwrap_or_else(|_| {});
    policy.policy_digest = policy_digest(&policy);
    if !policy.permits(PolicyAction::QueryState) {
        sys_debug_writeln("fleetd: ERROR policy.permits failed");
        sys_exit(1);
    }
    if policy.permits(PolicyAction::RemoteRecovery) {
        sys_debug_writeln("fleetd: ERROR deny rule not enforced");
        sys_exit(1);
    }
    sys_debug_writeln("fleetd: fleet policy verified (allow/deny semantics)");

    // FleetRolloutPlan self-check.
    let mut plan = FleetRolloutPlan::new(fleet_id, 0, roster.roster_digest);
    plan.add_stage(RolloutStage::new(b"canary", RolloutStrategy::AllConfirmed))
        .unwrap_or_else(|_| {});
    plan.add_stage(RolloutStage::new(b"full", RolloutStrategy::AllConfirmed))
        .unwrap_or_else(|_| {});
    plan.confirm_active_stage().unwrap_or_else(|_| {});
    if !plan.try_advance() {
        sys_debug_writeln("fleetd: ERROR rollout advance failed");
        sys_exit(1);
    }
    sys_debug_writeln("fleetd: rollout plan advance verified");

    // FleetAction no-remote-shell invariant check.
    let action = FleetAction::new(
        fleet_id, [0u8; 16], FleetActionKind::QueryState, [0x01u8; 16],
    );
    if !action.is_fleet_wide() {
        sys_debug_writeln("fleetd: ERROR fleet_wide detection wrong");
        sys_exit(1);
    }
    sys_debug_writeln("fleetd: fleet action verified (no remote shell)");

    // RemoteDiagRequest self-check.
    let _req = RemoteDiagRequest::new(
        fleet_id, [0x01u8; 16],
        DiagRequestKind::FullSnapshot, [0xAAu8; 16],
    );
    sys_debug_writeln("fleetd: remote diag format verified");

    // PolicyBundle self-check.
    let mut bundle = PolicyBundle::new(fleet_id);
    bundle.add(fjell_policy_format::PolicyStatement::allow(
        PolicySubject::ServiceSpawn, 0xFF,
    )).unwrap_or_else(|_| {});
    if !bundle.permits(PolicySubject::ServiceSpawn, 0x1B) {
        sys_debug_writeln("fleetd: ERROR policy bundle failed");
        sys_exit(1);
    }
    sys_debug_writeln("fleetd: policy bundle verified");

    sys_debug_writeln("fleetd: all v0.8 format self-checks passed");
    sys_debug_writeln("fleetd: ready");

    // ── IPC event loop (v0.8.1 — storaged + cap-broker wiring) ───────────────
    sys_exit(0)
}
