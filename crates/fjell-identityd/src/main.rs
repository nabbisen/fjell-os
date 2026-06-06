//! identityd — Node identity lifecycle manager (RFC v0.7-001).
//!
//! Responsibilities:
//!   1. Generate `NodeIdentity` on first boot; persist to storaged.
//!   2. Expose identity for attestd signing and snapshot-sync peer verification.
//!   3. Enforce `NodeIdentityPolicy` on incoming snapshot import requests.
#![no_std]
#![no_main]
mod rt;
use fjell_syscall::{sys_exit, sys_debug_writeln};
#[allow(unused_imports)] // stub: identity_digest used when storaged IPC wires up
use fjell_identity_format::{NodeIdentity, NodeId, NodeAlias, AttestationPubkey,
                              NodeIdentityPolicy, identity_digest};
use fjell_measure_format::Digest32;

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("identityd: started (v0.7 node identity)");

    // Stub: in production, load persisted identity from storaged.
    // v0.7.0-alpha: generate a deterministic test identity.
    // Stub: full IPC persistence lands in v0.7.x patch
    let _identity = NodeIdentity::new(
        NodeId([0x01u8; 16]),
        NodeAlias(*b"qemu-virt-0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"),
        0,
        1,
        0x01,
        AttestationPubkey([0u8; 32]),
        Digest32([0u8; 32]),
        Digest32([0u8; 32]),
    );
    // identity.identity_digest stamped when storaged IPC wires up

    let policy = NodeIdentityPolicy::same_family_default(0x01);

    sys_debug_writeln("identityd: identity initialised");
    sys_debug_writeln("identityd: policy: SameFamily");

    // Verify the policy permits our own profile tag (self-consistency check).
    if !policy.permits(0x01) {
        sys_debug_writeln("identityd: ERROR policy rejects own profile");
        sys_exit(1);
    }

    sys_debug_writeln("identityd: ready");
    sys_exit(0)
}
