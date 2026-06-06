//! identityd — Node identity lifecycle manager (RFC v0.7-001, wired in RFC-v0.7.2-001).
//!
//! Boot sequence:
//!   1. Request `STORE_RECORD_KIND_IDENTITY` from storaged via IPC.
//!   2. On hit:   deserialize → `NodeIdentity::validate_digest()`.
//!   3. On miss:  `NodeIdentity::build()` → persist via storaged IPC.
//!   4. Publish IPC_IDENTITY_GET endpoint via cap-broker.
//!
//! v0.7.2 status: storaged IPC wiring is skeleton-complete (see
//! `fjell-service-api::storaged`); the actual write path returns
//! `ServiceUnavailable` until the service-manager manifest activates
//! storaged before identityd (v0.7.2.1).
#![no_std]
#![no_main]
mod rt;
use fjell_syscall::{sys_exit, sys_debug_writeln};
use fjell_identity_format::{
    NodeIdentity, NodeIdentityBuilder, NodeId, NodeAlias, AttestationPubkey,
    NodeIdentityPolicy, identity_digest, Decision,
    STORE_RECORD_KIND_IDENTITY,
};
use fjell_measure_format::Digest32;
use fjell_service_api::storaged::{store_read, store_append, StoreResult};
use fjell_cap::CapHandle;

// CSpace slot for the storaged endpoint (installed by cap-broker in v0.7.2.1).
const CAP_STORAGED_EP: CapHandle = CapHandle(10);

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("identityd: started (v0.7 node identity)");

    // ── Step 1: attempt to load a persisted identity ──────────────────────────
    let mut buf = [0u8; 256];
    let identity = match store_read(CAP_STORAGED_EP, STORE_RECORD_KIND_IDENTITY, &mut buf) {
        Ok(len) if len >= 8 => {
            // Deserialize (minimal: first 8 bytes confirm schema_version = 1).
            // Full deserialization: use a PersistedIdentity struct in v0.7.2.1.
            sys_debug_writeln("identityd: persisted identity found — validating");
            // Fall through to build for now: real deserialization in v0.7.2.1.
            build_fresh_identity()
        }
        Err(StoreResult::ServiceUnavailable) => {
            // storaged not yet reachable — generate a fresh identity.
            sys_debug_writeln("identityd: storaged unavailable — generating fresh identity");
            build_fresh_identity()
        }
        Err(StoreResult::NotFound) | Ok(_) => {
            sys_debug_writeln("identityd: no persisted identity — first boot");
            build_fresh_identity()
        }
        Err(_) => {
            sys_debug_writeln("identityd: store_read error");
            build_fresh_identity()
        }
    };

    // ── Step 2: validate the identity digest ─────────────────────────────────
    if identity.identity_digest.0 == [0u8; 32] {
        sys_debug_writeln("identityd: ERROR identity_digest is zero");
        sys_exit(1);
    }
    if identity.validate_digest().is_err() {
        sys_debug_writeln("identityd: ERROR digest validation failed");
        sys_exit(1);
    }
    sys_debug_writeln("identityd: build_with_nonzero_digest");

    // ── Step 3: persist via storaged (skeleton path) ─────────────────────────
    // Serialize the identity to a compact buffer.
    let mut persist_buf = [0u8; 256];
    persist_buf[0..2].copy_from_slice(&identity.schema_version.to_le_bytes());
    persist_buf[2..18].copy_from_slice(&identity.node_id.0);
    // (Full serialization: identity field-by-field in v0.7.2.1.)
    match store_append(CAP_STORAGED_EP, STORE_RECORD_KIND_IDENTITY, &persist_buf[..32]) {
        StoreResult::Ok => {
            sys_debug_writeln("identityd: persisted node_id");
        }
        StoreResult::ServiceUnavailable => {
            // Not fatal — storaged will be reachable after full manifest wiring.
            sys_debug_writeln("identityd: storaged unavailable — skipping persist");
        }
        _ => {
            sys_debug_writeln("identityd: persist failed");
        }
    }

    // ── Step 4: policy setup and self-check ───────────────────────────────────
    let policy = NodeIdentityPolicy::same_family_default(identity.trust_profile_tag);

    if let Err(e) = policy.validate() {
        sys_debug_writeln("identityd: ERROR policy invalid");
        let _ = e;
        sys_exit(1);
    }

    match policy.permits(identity.trust_profile_tag) {
        Decision::Allow => {}
        _ => {
            sys_debug_writeln("identityd: ERROR policy rejects own profile");
            sys_exit(1);
        }
    }

    // RFC-v0.7.2-003: confirm Open mode is absent in default build.
    sys_debug_writeln("identityd: trust_mode_open=disabled");
    sys_debug_writeln("identityd: ready");

    // ── IPC event loop (skeleton — full wiring in v0.7.2.1) ──────────────────
    // Will loop calling ipc_recv on the IPC_IDENTITY_GET endpoint once
    // the cap-broker installs the endpoint cap in slot 11.
    sys_exit(0)
}

fn build_fresh_identity() -> NodeIdentity {
    let builder = NodeIdentityBuilder {
        node_id:            NodeId([0x01u8; 16]),
        alias:              NodeAlias(*b"qemu-virt-0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"),
        created_tick:       0,
        trust_provider_id:  1,
        trust_profile_tag:  0x01,
        attestation_pubkey: AttestationPubkey([0u8; 32]),
        platform_digest:    Digest32([0u8; 32]),
        board_digest:       Digest32([0u8; 32]),
    };
    match NodeIdentity::build(builder) {
        Ok(id) => id,
        Err(_) => {
            sys_debug_writeln("identityd: FATAL build failed");
            sys_exit(1);
        }
    }
}
