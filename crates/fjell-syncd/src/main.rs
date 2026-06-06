//! syncd — Offline-first snapshot sync daemon (RFC v0.7-004).
//!
//! Responsibilities:
//!   1. Accept incoming signed `SnapshotEnvelope` from peer nodes.
//!   2. Verify source identity via identityd policy.
//!   3. Apply records to storaged; track conflict domains.
//!   4. Manage the outbound sync queue (persisted, replay-safe).
#![no_std]
#![no_main]
mod rt;
use fjell_syscall::{sys_exit, sys_debug_writeln};
#[allow(unused_imports)] // stub: full import pipeline lands in v0.7.x patch
use fjell_snapshot_format::{
    SnapshotEnvelope, SnapshotRecord, ConflictDomain,
    SnapshotImportOutcome, SnapshotImportError,
    snapshot_digest, SNAPSHOT_ENVELOPE_V2,
};
#[allow(unused_imports)] // stub: NodeId used by identity verification in v0.7.x
use fjell_identity_format::NodeId;
use fjell_measure_format::Digest32;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("syncd: started (v0.7 snapshot sync)");

    // Stub: create and verify a v2 envelope (self-consistency check).
    let mut env = SnapshotEnvelope::new_v2(
        Digest32([0u8; 32]),
        0,
        [0u8; 16],
    );
    let _ = env.push_record(SnapshotRecord {
        domain:   ConflictDomain::LocallyConfirmed,
        kind:     0x0001,
        seq:      1,
        body:     [0u8; 64],
        body_len: 0,
    });
    env.snapshot_digest = snapshot_digest(&env);

    if env.snapshot_digest.0 == [0u8; 32] {
        sys_debug_writeln("syncd: ERROR digest is zero");
        sys_exit(1);
    }

    sys_debug_writeln("syncd: envelope self-check passed");
    sys_debug_writeln("syncd: ready");
    sys_exit(0)
}
