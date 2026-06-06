//! `proptest` strategy generators for Op sequences (RFC v0.6-001 §7.2).

use proptest::prelude::*;
use crate::model::*;
use crate::ops::Op;

// ── Leaf generators ───────────────────────────────────────────────────────────

fn arb_cap_id() -> impl Strategy<Value = CapId> {
    (0u32..12).prop_map(CapId)
}
fn arb_task_id() -> impl Strategy<Value = TaskId> {
    (0u16..4).prop_map(TaskId)
}
fn arb_lease_id() -> impl Strategy<Value = LeaseId> {
    (0u32..4).prop_map(LeaseId)
}
fn arb_rights() -> impl Strategy<Value = CapRights> {
    (0u64..=0xFF).prop_map(CapRights)
}
fn arb_kind() -> impl Strategy<Value = CapKind> {
    prop_oneof![
        Just(CapKind::Endpoint),
        Just(CapKind::TaskControl),
        Just(CapKind::MmioRegion),
        Just(CapKind::NetDevice),
    ]
}

// ── Op generator ─────────────────────────────────────────────────────────────

pub fn arb_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        // CapRegister — 35 %
        35 => (arb_cap_id(), arb_kind(), arb_rights(), arb_lease_id())
            .prop_map(|(id, kind, rights, lease)| Op::CapRegister { id, kind, rights, lease }),
        // CapDelegate — 15 %
        15 => (arb_cap_id(), arb_cap_id(), arb_task_id(), arb_rights())
            .prop_map(|(src, new_id, dst_task, sub_rights)| Op::CapDelegate { src, new_id, dst_task, sub_rights }),
        // CapRevoke — 10 %
        10 => arb_cap_id().prop_map(|id| Op::CapRevoke { id }),
        // CapReplace — 5 %
        5  => (arb_cap_id(), arb_kind(), arb_rights())
            .prop_map(|(id, new_kind, new_rights)| Op::CapReplace { id, new_kind, new_rights }),
        // IpcSend — 10 %
        10 => (arb_task_id(), arb_cap_id(), 0u16..=255, any::<u64>())
            .prop_map(|(from, cap_id, tag, payload)| Op::IpcSend { from, cap_id, tag, payload }),
        // IpcRecv — 10 %
        10 => (arb_task_id(), arb_cap_id())
            .prop_map(|(task, endpoint)| Op::IpcRecv { task, endpoint }),
        // LeaseExpire — 5 %
        5  => arb_lease_id().prop_map(|lease_id| Op::LeaseExpire { lease_id }),
        // LeaseRenew — 3 %
        3  => arb_lease_id().prop_map(|lease_id| Op::LeaseRenew { lease_id }),
        // TaskFault — 5 %
        5  => arb_task_id().prop_map(|task| Op::TaskFault { task }),
        // Tick — 2 %
        2  => Just(Op::Tick),
    ]
}

/// A sequence of 1–64 operations, weighted per RFC v0.6-001 §7.2.
pub fn arb_op_sequence() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(arb_op(), 1..=64)
}
