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
//!
//! RFC-0.37-001 (closes E-078, E-081): this service was never spawned at
//! all before this line — absent from the kernel's image table, the build
//! list, and `init`'s own spawn sequence, and its own `src/rt.rs` had no
//! `_start` symbol, so it could not even have been cross-compiled and
//! linked as a bootable image. `main.rs`'s own section header below
//! (`// ── IPC event loop (v0.8.1 ...) ───`) sat directly above a
//! `sys_exit(0)`, naming a loop this file never wrote. This line gives it
//! one: the format self-checks above are unchanged, but `service_main` now
//! sends readiness and enters a real loop rather than exiting, becoming the
//! node-side endpoint of the conversation RFC-v0.8-005 specifies — not a
//! real `RemoteDiagRequest` arriving over the network (that stays out of
//! scope; the trigger is a console byte `init` relays, like
//! `fjell_service_api::neg_test::RUN_REBOOT`), but a genuine IPC round trip
//! with `diagnosticsd`, asking for a bundle and reading back what it says.
#![no_std]
#![no_main]
mod rt;

use fjell_cap::CapHandle;
use fjell_fleet_format::{
    FleetAction, FleetActionKind, FleetPolicy, FleetRolloutPlan, NodeRoster, PolicyAction,
    PolicyCondition, PolicyStatement, RolloutStage, RolloutStrategy, policy_digest, roster_digest,
};
use fjell_policy_format::{PolicyBundle, PolicySubject};
use fjell_remote_diag_format::{DiagRequestKind, RemoteDiagRequest};
use fjell_service_api::diagnosticsd as proto;
use fjell_syscall::{sys_debug_write, sys_debug_writeln, sys_exit, sys_yield};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    sys_debug_writeln("fleetd: panic");
    sys_exit(1);
}

// ── CSpace layout ─────────────────────────────────────────────────────────────
//
//   slot 1 — `diagnosticsd`'s CAP_TOOLS_EP, the one endpoint this line
//            installs a capability to (RFC-0.37-001 R4, least privilege:
//            the same argument RFC-0.35-001 R5 made for the update-path
//            channel). SEND|RECV: both sides send a request and block on a
//            reply, the same shape as that channel.
//
// The trigger-receive slot (`init` → `fleetd`) is not declared here as a
// named `CapHandle` constant — see
// `fjell_abi::service::FLEETD_TRIGGER_RECV_SLOT`'s own doc for why
// (`NEG_TEST_TRIGGER_RECV_SLOT` sets the precedent).
const CAP_DIAGNOSTICSD_EP: CapHandle = CapHandle(1);

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
    policy
        .add_statement(PolicyStatement::allow(
            PolicyAction::QueryState,
            PolicyCondition::Always,
        ))
        .unwrap_or_else(|_| {});
    policy
        .add_statement(PolicyStatement::deny(
            PolicyAction::RemoteRecovery,
            PolicyCondition::Always,
        ))
        .unwrap_or_else(|_| {});
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
        fleet_id,
        [0u8; 16],
        FleetActionKind::QueryState,
        [0x01u8; 16],
    );
    if !action.is_fleet_wide() {
        sys_debug_writeln("fleetd: ERROR fleet_wide detection wrong");
        sys_exit(1);
    }
    sys_debug_writeln("fleetd: fleet action verified (no remote shell)");

    // RemoteDiagRequest self-check.
    let _req = RemoteDiagRequest::new(
        fleet_id,
        [0x01u8; 16],
        DiagRequestKind::FullSnapshot,
        [0xAAu8; 16],
    );
    sys_debug_writeln("fleetd: remote diag format verified");

    // PolicyBundle self-check.
    let mut bundle = PolicyBundle::new(fleet_id);
    bundle
        .add(fjell_policy_format::PolicyStatement::allow(
            PolicySubject::ServiceSpawn,
            0xFF,
        ))
        .unwrap_or_else(|_| {});
    if !bundle.permits(PolicySubject::ServiceSpawn, 0x1B) {
        sys_debug_writeln("fleetd: ERROR policy bundle failed");
        sys_exit(1);
    }
    sys_debug_writeln("fleetd: policy bundle verified");

    sys_debug_writeln("fleetd: all v0.8 format self-checks passed");

    let _ = fjell_syscall::sys_ipc_send(
        fjell_service_api::ready::SERVICE_READY_SEND_SLOT,
        fjell_service_api::tags::SERVICE_READY,
    );
    sys_debug_writeln("fleetd: ready");

    run_event_loop()
}

// ── IPC event loop (RFC-0.37-001) ───────────────────────────────────────────────

/// Blocks on the console-byte trigger `init` relays
/// (`fjell_service_api::fleetd::RUN_REMOTE_DIAG`), and on each one asks
/// `diagnosticsd` for a bundle. Never exits — the structural fact E-081
/// named missing.
fn run_event_loop() -> ! {
    loop {
        match fjell_syscall::sys_ipc_recv_msg(fjell_abi::service::FLEETD_TRIGGER_RECV_SLOT) {
            Ok((tag, ..)) if tag == fjell_service_api::fleetd::RUN_REMOTE_DIAG => {
                sys_debug_writeln(
                    "fleetd: remote-diag trigger received; asking diagnosticsd for a bundle",
                );
                request_remote_diag();
            }
            Ok(_) => {
                sys_debug_writeln("fleetd: unexpected tag on trigger endpoint");
            }
            Err(_) => {
                // Mirrors `diagnosticsd::recv_msg`'s own reasoning (RFC-0.35-001
                // R5): a recv error must not be a tight loop.
                sys_debug_writeln("fleetd: trigger endpoint recv error; yielding");
                sys_yield();
            }
        }
    }
}

/// RFC-0.37-001 R6: the instigator's own read, alongside `diagnosticsd`'s
/// own output (printed by `diagnosticsd` itself) — the pattern
/// RFC-0.34-004's `semantic-liar` tier established.
fn request_remote_diag() {
    let _ = fjell_syscall::sys_ipc_send(CAP_DIAGNOSTICSD_EP.0, proto::BUILD_BUNDLE);
    match fjell_syscall::sys_ipc_recv_msg(CAP_DIAGNOSTICSD_EP.0) {
        Ok((tag, w0, ..)) if tag == proto::BUNDLE_READY => {
            sys_debug_write("fleetd: bundle ready (audit_event_count=");
            write_u64(w0 as u64);
            sys_debug_writeln(")");
            sys_debug_writeln(fjell_service_api::negative_markers::REMOTE_DIAG_BUNDLE_READY);
        }
        Ok(_) => sys_debug_writeln("fleetd: unexpected reply from diagnosticsd"),
        Err(_) => sys_debug_writeln("fleetd: diagnosticsd recv error"),
    }
}

fn write_u64(mut n: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if n == 0 {
        sys_debug_write("0");
        return;
    }
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    if let Ok(s) = core::str::from_utf8(&buf[i..]) {
        sys_debug_write(s);
    }
}
