//! Property-test harness (RFC v0.6-001) — 10 properties × 1000 cases.
//!
//! Run with: cargo test -p fjell-proptest
//! Regressions in: crates/fjell-proptest/regressions/

use fjell_proptest::{
    generators::arb_op_sequence,
    model::{ModelState, CapId, TaskId, LeaseId, CapKind, CapRights},
    ops::{execute, Op},
    properties,
};
use proptest::prelude::*;

/// Master seed documented in RFC v0.6-001 §7.4; passed via PROPTEST_SEED env in CI.
// const MASTER_SEED: u64 = 0x46656C6C;  // suppress until xtask wires it up
const CASES: u32 = 1_000;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: CASES,
        failure_persistence: Some(Box::new(
            proptest::test_runner::FileFailurePersistence::WithSource("regressions")
        )),
        ..ProptestConfig::default()
    })]

    // P1 — generation alias
    #[test]
    fn p1_no_generation_alias(ops in arb_op_sequence()) {
        let mut s = ModelState::new();
        for op in &ops { execute(&mut s, op); }
        for id in (0..12).map(CapId) {
            properties::p1_no_generation_alias(&s, id).map_err(|e| TestCaseError::fail(e))?;
        }
    }

    // P2 — revoke invalidates
    #[test]
    fn p2_revoke_invalidates(ops in arb_op_sequence(), cap_id in (0u32..12).prop_map(CapId)) {
        let mut s = ModelState::new();
        // Register cap so there's something to revoke.
        execute(&mut s, &Op::CapRegister {
            id: cap_id, kind: CapKind::Endpoint,
            rights: CapRights::SEND, lease: LeaseId(0),
        });
        for op in &ops { execute(&mut s, op); }
        properties::p2_revoke_invalidates(&mut s, cap_id).map_err(|e| TestCaseError::fail(e))?;
    }

    // P3 — delegate subrights subset
    #[test]
    fn p3_delegate_subrights(
        src_rights in (0u64..=0xFF).prop_map(CapRights),
        attempt    in (0u64..=0xFF).prop_map(CapRights),
    ) {
        let mut s = ModelState::new();
        let src = CapId(0);
        execute(&mut s, &Op::CapRegister {
            id: src, kind: CapKind::Endpoint,
            rights: src_rights, lease: LeaseId(0),
        });
        properties::p3_delegate_subrights(&mut s, src, CapId(1), attempt)
            .map_err(|e| TestCaseError::fail(e))?;
    }

    // P4 — lease expiry revokes caps
    #[test]
    fn p4_lease_expiry_revokes_caps(ops in arb_op_sequence()) {
        let mut s = ModelState::new();
        for op in &ops { execute(&mut s, op); }
        properties::p4_lease_expiry_revokes_caps(&mut s, LeaseId(0))
            .map_err(|e| TestCaseError::fail(e))?;
    }

    // P5 — task fault revokes leases
    #[test]
    fn p5_task_fault_revokes_leases(ops in arb_op_sequence()) {
        let mut s = ModelState::new();
        for op in &ops { execute(&mut s, op); }
        properties::p5_task_fault_revokes_leases(&mut s, TaskId(0))
            .map_err(|e| TestCaseError::fail(e))?;
    }

    // P6 — send requires SEND right
    #[test]
    fn p6_send_requires_right(
        rights  in (0u64..=0xFF).prop_map(CapRights),
    ) {
        let mut s = ModelState::new();
        execute(&mut s, &Op::CapRegister {
            id: CapId(0), kind: CapKind::Endpoint,
            rights, lease: LeaseId(0),
        });
        properties::p6_send_requires_right(&mut s, CapId(0))
            .map_err(|e| TestCaseError::fail(e))?;
    }

    // P7 — recv no panic
    #[test]
    fn p7_recv_no_panic(ops in arb_op_sequence()) {
        let mut s = ModelState::new();
        execute(&mut s, &Op::CapRegister {
            id: CapId(0), kind: CapKind::Endpoint,
            rights: CapRights::RECV, lease: LeaseId(0),
        });
        for op in &ops { execute(&mut s, op); }
        properties::p7_recv_no_panic(&mut s, TaskId(0), CapId(0))
            .map_err(|e| TestCaseError::fail(e))?;
    }

    // P8 — in-flight message dropped after revoke
    #[test]
    fn p8_inflight_dropped_after_revoke(ops in arb_op_sequence()) {
        let mut s = ModelState::new();
        execute(&mut s, &Op::CapRegister {
            id: CapId(0), kind: CapKind::Endpoint,
            rights: CapRights::SEND.intersect(CapRights::ALL),
            lease: LeaseId(0),
        });
        for op in &ops { execute(&mut s, op); }
        properties::p8_inflight_dropped_after_revoke(&mut s, CapId(0))
            .map_err(|e| TestCaseError::fail(e))?;
    }

    // P9 — generation monotonic
    #[test]
    fn p9_generation_monotonic(ops in arb_op_sequence()) {
        let mut s = ModelState::new();
        for op in &ops { execute(&mut s, op); }
        for id in (0..12).map(CapId) {
            properties::p9_generation_monotonic(&s, id).map_err(|e| TestCaseError::fail(e))?;
        }
    }

    // P10 — cap table capacity respected
    #[test]
    fn p10_no_capacity_underrun(ops in arb_op_sequence()) {
        let mut s = ModelState::new();
        for op in &ops { execute(&mut s, op); }
        properties::p10_no_capacity_underrun(&s).map_err(|e| TestCaseError::fail(e))?;
    }
}
