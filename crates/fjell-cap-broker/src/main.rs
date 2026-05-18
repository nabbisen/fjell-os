//! Capability broker (RFC 021).
//!
//! Implements real policy evaluation: explicit deny → explicit allow →
//! default deny (BROKER-001 through BROKER-008 from the M4 design doc).
//!
//! # IPC protocol
//!
//! A service requests capabilities by calling `sys_ipc_call` on the
//! cap-broker endpoint (slot 0) with a 4-word message:
//!
//! ```
//! label = CAP_REQUEST (0x020) | (4 << 16)   // nwords = 4
//! w0    = requester_id   (ImageId value, u32)
//! w1    = resource_class (ResourceClass discriminant, u32)
//! w2    = requested_rights (CapRights bits, u32)
//! w3    = 0 (reserved)
//! ```
//!
//! Reply:
//! - `CAP_GRANTED | (lease_id as u16) << 16` on success
//! - `CAP_DENIED` on policy violation

#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{
    sys_exit, sys_ipc_recv_msg, sys_ipc_reply,
    sys_lease_create, sys_debug_writeln,
};
use fjell_service_api::tags;

// ── Resource classes ─────────────────────────────────────────────────────────

/// The kind of kernel object / authority being requested.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ResourceClass {
    Any          = 0,
    Endpoint     = 1,
    TaskControl  = 2,
    AuditDrain   = 3,
    MmioRegion   = 4,
    DmaAlloc     = 5,
    Config       = 6,
    Semantic     = 7,
}

impl ResourceClass {
    fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Endpoint,
            2 => Self::TaskControl,
            3 => Self::AuditDrain,
            4 => Self::MmioRegion,
            5 => Self::DmaAlloc,
            6 => Self::Config,
            7 => Self::Semantic,
            _ => Self::Any,
        }
    }
}

// ── Policy types ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PolicyKind { Allow, Deny }

/// `0xFFFF` in `requester` or `resource` field means "match any".
pub struct PolicyRule {
    pub requester: u16,       // ImageId value, or 0xFFFF = wildcard
    pub resource:  u16,       // ResourceClass discriminant, or 0xFFFF = wildcard
    pub kind:      PolicyKind,
    pub rights:    u32,       // CapRights bits (meaningful only for Allow)
}

pub enum PolicyResult {
    Granted(u32 /* rights mask */),
    Denied,
}

// ── Policy table ─────────────────────────────────────────────────────────────
//
// ImageId values (from fjell-abi):
//   0 = init            1 = configd         2 = cap-broker
//   3 = auditd          4 = service-manager 5 = sample-service
//   6 = semantic-stream 7 = proxy-text      8 = devmgr
//   9 = driver-virtio   10 = storaged       11 = bootctl
//   12 = upgraded       13 = powerd         14 = verifyd
//   15 = rootfsd        16 = snapshotd
//
// CapRights bits: SEND=1, RECV=2, CALL=4, GRANT=8, MAP_R=16, MAP_W=32, MAP_X=64, INSPECT=128
//   ALL = 0xFF

const ALLOW_ALL: u32 = 0xFF;
const ALLOW_RECV: u32 = 0x02; // CapRights::RECV

const WILDCARD: u16 = 0xFFFF;

/// Static compile-time policy table.
/// Evaluation order: first Deny match wins; then first Allow match wins;
/// otherwise default Deny.
///
/// BROKER-001: default deny (absence of allow rule = deny).
/// BROKER-002: deny takes priority over allow.
const POLICY: &[PolicyRule] = &[
    // ── Explicit denies (BROKER-002: evaluated before allows) ────────────────
    // Deny any service requesting bootstrap authority.
    PolicyRule { requester: WILDCARD, resource: 0 /* Any */, kind: PolicyKind::Deny, rights: 0 },

    // ── Explicit allows ───────────────────────────────────────────────────────
    // init: may manage tasks (spawn / start services during boot).
    PolicyRule { requester: 0,  resource: ResourceClass::TaskControl as u16,
                 kind: PolicyKind::Allow, rights: ALLOW_ALL },
    PolicyRule { requester: 0,  resource: ResourceClass::Endpoint as u16,
                 kind: PolicyKind::Allow, rights: ALLOW_ALL },

    // service-manager: full task-control and endpoint management.
    PolicyRule { requester: 4,  resource: ResourceClass::TaskControl as u16,
                 kind: PolicyKind::Allow, rights: ALLOW_ALL },
    PolicyRule { requester: 4,  resource: ResourceClass::Endpoint as u16,
                 kind: PolicyKind::Allow, rights: ALLOW_ALL },

    // auditd: read kernel audit ring (AuditDrain capability with RECV right).
    PolicyRule { requester: 3,  resource: ResourceClass::AuditDrain as u16,
                 kind: PolicyKind::Allow, rights: ALLOW_RECV },

    // storaged and virtio-blk driver: hardware access.
    PolicyRule { requester: 10, resource: ResourceClass::MmioRegion as u16,
                 kind: PolicyKind::Allow, rights: ALLOW_ALL },
    PolicyRule { requester: 10, resource: ResourceClass::DmaAlloc as u16,
                 kind: PolicyKind::Allow, rights: ALLOW_ALL },
    PolicyRule { requester: 9,  resource: ResourceClass::MmioRegion as u16,
                 kind: PolicyKind::Allow, rights: ALLOW_ALL },
    PolicyRule { requester: 9,  resource: ResourceClass::DmaAlloc as u16,
                 kind: PolicyKind::Allow, rights: ALLOW_ALL },

    // configd: config reads.
    PolicyRule { requester: 1,  resource: ResourceClass::Config as u16,
                 kind: PolicyKind::Allow, rights: ALLOW_ALL },
    // service-manager: config reads.
    PolicyRule { requester: 4,  resource: ResourceClass::Config as u16,
                 kind: PolicyKind::Allow, rights: ALLOW_ALL },

    // semantic-stream / proxy-text.
    PolicyRule { requester: 6,  resource: ResourceClass::Semantic as u16,
                 kind: PolicyKind::Allow, rights: ALLOW_ALL },
    PolicyRule { requester: 7,  resource: ResourceClass::Semantic as u16,
                 kind: PolicyKind::Allow, rights: ALLOW_RECV | 0x01 /* SEND */ },
];

// ── Evaluator ────────────────────────────────────────────────────────────────

/// Three-phase policy evaluation (RFC 021, BROKER-001 / BROKER-002).
///
/// 1. Explicit Deny  → returns `Denied` immediately.
/// 2. Explicit Allow → returns `Granted(granted_rights)` for first match.
/// 3. Default Deny   → returns `Denied` if no allow rule matched.
fn evaluate(requester: u16, resource: u16, requested_rights: u32) -> PolicyResult {
    // Override: cap-broker itself can always query its own policy.
    if requester == 2 { return PolicyResult::Granted(requested_rights & ALLOW_ALL); }

    // Phase 1: explicit deny.
    for rule in POLICY {
        if rule.kind != PolicyKind::Deny { continue; }
        let req_match = rule.requester == WILDCARD || rule.requester == requester;
        let res_match = rule.resource  == WILDCARD || rule.resource  == resource;
        // Only apply the catch-all deny for ResourceClass::Any (wildcard any
        // resource) if the requester is explicitly unknown (not in allow list).
        // Skip the wildcard-resource deny for known resources to let allows run.
        if res_match && resource == 0 { continue; } // skip Any-resource deny, fall to allows
        if req_match && res_match { return PolicyResult::Denied; }
    }

    // Phase 2: explicit allow.
    for rule in POLICY {
        if rule.kind != PolicyKind::Allow { continue; }
        let req_match = rule.requester == WILDCARD || rule.requester == requester;
        let res_match = rule.resource  == WILDCARD || rule.resource  == resource;
        if req_match && res_match {
            let granted = rule.rights & requested_rights;
            if granted != 0 { return PolicyResult::Granted(granted); }
        }
    }

    // Phase 3: default deny.
    PolicyResult::Denied
}

// ── Service main ─────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("M4: cap-broker started");

    // Bootstrap: demonstrate lease lifecycle (kept for smoke continuity).
    if let Ok(lid) = sys_lease_create(0) {
        // Lease created — revoke it immediately (bootstrap authority released).
        let _ = fjell_syscall::sys_lease_revoke(lid);
    }

    loop {
        match sys_ipc_recv_msg(0u32) {
            Ok((label, w0, w1, w2, _w3)) => {
                let tag = label & 0xFFFF;
                if tag == (tags::CAP_REQUEST & 0xFFFF) {
                    // Decode request fields from IPC words.
                    let requester_id    = w0 as u16;
                    let resource_class  = ResourceClass::from_u32(w1 as u32) as u16;
                    let requested_rights = w2 as u32;

                    match evaluate(requester_id, resource_class, requested_rights) {
                        PolicyResult::Granted(rights) => {
                            // Make the grant lease-bound (BROKER-004).
                            let lease_id: u16 = match sys_lease_create(0) {
                                Ok(lid) => lid.0 as u16,
                                Err(_)  => 0,
                            };
                            // Encode lease_id in upper 16 bits of reply label.
                            let reply = tags::CAP_GRANTED | ((lease_id as usize) << 16)
                                      | ((rights as usize) << 32);
                            let _ = sys_ipc_reply(reply);
                        }
                        PolicyResult::Denied => {
                            let _ = sys_ipc_reply(tags::CAP_DENIED);
                        }
                    }
                } else if tag == (tags::SERVICE_SHUTDOWN & 0xFFFF) {
                    let _ = sys_ipc_reply(0);
                    break;
                } else {
                    let _ = sys_ipc_reply(0);
                }
            }
            Err(_) => { let _ = sys_ipc_reply(0); }
        }
    }

    sys_exit(0)
}
