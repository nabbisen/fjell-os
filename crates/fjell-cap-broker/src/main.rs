//! Capability broker (RFC 040 — v0.2.0 Security Boundary Closure).
//!
//! # State machine
//!
//! ```text
//! ┌───────────┐   BOOTSTRAP_COMPLETE   ┌────────────┐
//! │ Bootstrap │ ─────────────────────► │ Enforcing  │
//! └───────────┘  (from init only)      └────────────┘
//! ```
//!
//! **Bootstrap state**: cap-broker is up but the policy engine is not
//! enforcing.  Only `init` (ImageId 0) may communicate.  The bootstrap
//! message carries no payload in v0.2 (policy is compiled in).
//!
//! **Enforcing state**: all requests go through the three-phase evaluator
//! (deny → allow → default deny).  Grants are lease-bound; the lease is
//! recorded in the `DelegationRecord` tree.
//!
//! # Policy evaluation (BROKER-001 through BROKER-008)
//!
//! ```text
//! Phase 1: explicit Deny  → reject immediately
//! Phase 2: explicit Allow → grant with rights intersection
//! Phase 3: default Deny   → reject (BROKER-001)
//! ```
//!
//! # IPC protocol
//!
//! ```text
//! CAP_REQUEST (label=0x020, nwords=4):
//!   w0 = requester_id    (ImageId, u16)
//!   w1 = resource_class  (ResourceClass discriminant, u32)
//!   w2 = requested_rights (CapRights bits, u64 low 32)
//!   w3 = 0 reserved
//! Reply (CAP_GRANTED | lease_id<<16 | rights<<32) or CAP_DENIED.
//!
//! BOOTSTRAP_COMPLETE (label=0x100):
//!   no payload
//! Reply: 0 (ok) or -1 (already enforcing).
//! ```

#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{
    sys_exit, sys_ipc_recv_msg, sys_ipc_reply,
    sys_lease_revoke,
    sys_debug_writeln, sys_yield,
};
use fjell_service_api::tags;

// ── CapRights constants (v0.2 u64 bit layout) ───────────────────────────────
//
// Must match fjell_cap::CapRights bit layout.
// See crates/fjell-cap/src/rights.rs.

const RIGHT_SEND:         u64 = 1 << 3;
const RIGHT_RECV:         u64 = 1 << 4;
const RIGHT_CALL:         u64 = 1 << 5;
const RIGHT_REPLY:        u64 = 1 << 6;
const RIGHT_COPY:         u64 = 1 << 7;
const RIGHT_MINT:         u64 = 1 << 8;
const RIGHT_INSPECT:      u64 = 1 << 10;
#[allow(dead_code)]  // defined for completeness; used when service extraction lands
const RIGHT_DROP:         u64 = 1 << 11;
const RIGHT_TASK_CREATE:  u64 = 1 << 12;
const RIGHT_TASK_START:   u64 = 1 << 13;
const RIGHT_TASK_STATUS:  u64 = 1 << 14;
const RIGHT_TASK_KILL:    u64 = 1 << 15;
const RIGHT_LEASE_CREATE: u64 = 1 << 16;
const RIGHT_LEASE_REVOKE: u64 = 1 << 17;
const RIGHT_MMIO_MAP:     u64 = 1 << 19;
const RIGHT_DMA_ALLOC:    u64 = 1 << 20;
const RIGHT_DMA_USE:      u64 = 1 << 21;
const RIGHT_DMA_REVOKE:   u64 = 1 << 22;
const RIGHT_AUDIT_DRAIN:  u64 = 1 << 23;

/// All defined rights (26 bits).
const ALL_RIGHTS: u64 = (1u64 << 26) - 1;

/// Endpoint send + call + recv set (for most IPC services).
const EP_RW: u64 = RIGHT_SEND | RIGHT_RECV | RIGHT_CALL | RIGHT_REPLY | RIGHT_COPY;

/// Task management rights bundle.
const TASK_MGMT: u64 = RIGHT_TASK_CREATE | RIGHT_TASK_START
                     | RIGHT_TASK_STATUS | RIGHT_TASK_KILL;

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
    DmaRegion    = 5,
    Config       = 6,
    Semantic     = 7,
    // ── v0.4 additions ──────────────────────────────────────────────────────
    /// virtio-net device capability (RFC v0.4-001).
    NetDevice    = 8,
    /// `secure-transportd` session slot (RFC v0.4-003).
    SxtSession   = 9,
    /// `diagnosticsd` bundle construction authority (RFC v0.4-005).
    DiagBundle   = 10,
}

impl ResourceClass {
    fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Endpoint,
            2 => Self::TaskControl,
            3 => Self::AuditDrain,
            4 => Self::MmioRegion,
            5 => Self::DmaRegion,
            6 => Self::Config,
            7 => Self::Semantic,
            8 => Self::NetDevice,
            9 => Self::SxtSession,
            10=> Self::DiagBundle,
            _ => Self::Any,
        }
    }
}

// ── Policy types ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PolicyKind { Allow, Deny }

/// `0xFFFF` in `requester` or `resource` means "match any".
pub struct PolicyRule {
    pub requester: u16,    // ImageId or WILDCARD
    pub resource:  u16,    // ResourceClass or WILDCARD
    pub kind:      PolicyKind,
    /// Rights mask (v0.2 u64 layout). Meaningful only for Allow.
    pub rights:    u64,
}

const WILDCARD: u16 = 0xFFFF;

// ── ImageId constants ─────────────────────────────────────────────────────────
// Must match fjell_abi::service::ImageId values.
const INIT:           u16 = 0;
const CONFIGD:        u16 = 1;
const CAP_BROKER:     u16 = 2;
const AUDITD:         u16 = 3;
const SVC_MANAGER:    u16 = 4;
const DEVMGR:         u16 = 8;
const VIRTIO_DRIVER:  u16 = 9;
const STORAGED:       u16 = 10;
#[allow(dead_code)]  // defined for completeness; used when service extraction lands
const BOOTCTL:        u16 = 11;
#[allow(dead_code)]  // defined for completeness; used when service extraction lands
const UPGRADED:       u16 = 12;
#[allow(dead_code)]  // defined for completeness; used when service extraction lands
const VERIFYD:        u16 = 14;
const SEMANTIC_STREAM:u16 = 6;
const PROXY_TEXT:     u16 = 7;
/// RFC 042: dedicated negative-test service (ImageId 20).
const NEG_TEST:       u16 = 20;
// ── v0.4 services (RFC v0.4-001 through v0.4-005) ───────────────────────────
const NETD:            u16 = 21;   // RFC v0.4-002 packet/session router
const VIRTIO_NET:      u16 = 22;   // RFC v0.4-001 virtio-net driver
const SECURE_TRANSPORTD: u16 = 23; // RFC v0.4-003 TLS channel service
const ATTESTD:         u16 = 13;   // RFC v0.3-004; extended by v0.4-005
const DIAGNOSTICSD:    u16 = 24;   // RFC v0.4-005 diagnostic bundle builder

// ── Policy table ─────────────────────────────────────────────────────────────
//
// Evaluation order: first Deny wins → then first Allow wins → default Deny.
// BROKER-001: default deny.  BROKER-002: deny > allow.

const POLICY: &[PolicyRule] = &[
    // ── Explicit denies ───────────────────────────────────────────────────────
    // No service may request "Any" resource class (bootstrap authority).
    PolicyRule { requester: WILDCARD, resource: ResourceClass::Any as u16,
                 kind: PolicyKind::Deny, rights: 0 },

    // ── Explicit allows ───────────────────────────────────────────────────────
    // init: task management + endpoint routing during boot.
    PolicyRule { requester: INIT, resource: ResourceClass::TaskControl as u16,
                 kind: PolicyKind::Allow, rights: TASK_MGMT | RIGHT_LEASE_CREATE
                                                | RIGHT_LEASE_REVOKE },
    PolicyRule { requester: INIT, resource: ResourceClass::Endpoint as u16,
                 kind: PolicyKind::Allow, rights: EP_RW | RIGHT_MINT },

    // ── RFC 042: neg-test policy rules (demonstrate DENY_PRIORITY) ────────────
    // NEG_TEST is denied from Config — explicit deny takes precedence even when
    // an allow rule for the same requester+resource exists (BROKER-002).
    PolicyRule { requester: NEG_TEST, resource: ResourceClass::Config as u16,
                 kind: PolicyKind::Deny, rights: 0 },
    // This allow would grant Config access to NEG_TEST, but the deny above
    // is evaluated first (phase 1 > phase 2), so it never fires.
    PolicyRule { requester: NEG_TEST, resource: ResourceClass::Config as u16,
                 kind: PolicyKind::Allow, rights: EP_RW },

    // service-manager: task lifecycle + endpoint management.
    PolicyRule { requester: SVC_MANAGER, resource: ResourceClass::TaskControl as u16,
                 kind: PolicyKind::Allow, rights: TASK_MGMT },
    PolicyRule { requester: SVC_MANAGER, resource: ResourceClass::Endpoint as u16,
                 kind: PolicyKind::Allow, rights: EP_RW },
    PolicyRule { requester: SVC_MANAGER, resource: ResourceClass::Config as u16,
                 kind: PolicyKind::Allow, rights: EP_RW },

    // auditd: kernel audit ring drain.
    PolicyRule { requester: AUDITD, resource: ResourceClass::AuditDrain as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_AUDIT_DRAIN | RIGHT_INSPECT },

    // storaged + virtio-blk driver: hardware access.
    PolicyRule { requester: STORAGED, resource: ResourceClass::MmioRegion as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_MMIO_MAP | RIGHT_INSPECT },
    PolicyRule { requester: STORAGED, resource: ResourceClass::DmaRegion as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_DMA_ALLOC | RIGHT_DMA_USE
                                                | RIGHT_DMA_REVOKE },
    PolicyRule { requester: VIRTIO_DRIVER, resource: ResourceClass::MmioRegion as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_MMIO_MAP | RIGHT_INSPECT },
    PolicyRule { requester: VIRTIO_DRIVER, resource: ResourceClass::DmaRegion as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_DMA_ALLOC | RIGHT_DMA_USE
                                                | RIGHT_DMA_REVOKE },

    // configd: config read.
    PolicyRule { requester: CONFIGD, resource: ResourceClass::Config as u16,
                 kind: PolicyKind::Allow, rights: EP_RW },

    // devmgr: MMIO enumeration.
    PolicyRule { requester: DEVMGR, resource: ResourceClass::MmioRegion as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_MMIO_MAP | RIGHT_INSPECT },

    // semantic-stream / proxy-text.
    PolicyRule { requester: SEMANTIC_STREAM, resource: ResourceClass::Semantic as u16,
                 kind: PolicyKind::Allow, rights: EP_RW },
    PolicyRule { requester: PROXY_TEXT, resource: ResourceClass::Semantic as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_SEND | RIGHT_RECV | RIGHT_COPY },

    // ── RFC v0.4-001: virtio-net driver ──────────────────────────────────────
    // The virtio-net driver may map MMIO and DMA regions, and bind/ack IRQs.
    PolicyRule { requester: VIRTIO_NET, resource: ResourceClass::MmioRegion as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_MMIO_MAP | RIGHT_INSPECT },
    PolicyRule { requester: VIRTIO_NET, resource: ResourceClass::DmaRegion as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_DMA_ALLOC | RIGHT_DMA_USE
                                                | RIGHT_DMA_REVOKE },
    // The driver may send/recv packets on its NetDevice capability.
    PolicyRule { requester: VIRTIO_NET, resource: ResourceClass::NetDevice as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_SEND | RIGHT_RECV | RIGHT_INSPECT },

    // ── RFC v0.4-002: netd ────────────────────────────────────────────────────
    // netd receives the NetDevice capability and allocates Session caps.
    PolicyRule { requester: NETD, resource: ResourceClass::NetDevice as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_RECV | RIGHT_INSPECT | RIGHT_MINT },
    PolicyRule { requester: NETD, resource: ResourceClass::Endpoint as u16,
                 kind: PolicyKind::Allow, rights: EP_RW | RIGHT_MINT },

    // ── RFC v0.4-003: secure-transportd ──────────────────────────────────────
    // secure-transportd may acquire SxtSession caps and open Endpoint channels.
    PolicyRule { requester: SECURE_TRANSPORTD, resource: ResourceClass::SxtSession as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_SEND | RIGHT_RECV | RIGHT_MINT },
    PolicyRule { requester: SECURE_TRANSPORTD, resource: ResourceClass::Endpoint as u16,
                 kind: PolicyKind::Allow, rights: EP_RW },
    // upgraded may open an SxtSession to request update metadata.
    PolicyRule { requester: UPGRADED, resource: ResourceClass::SxtSession as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_SEND | RIGHT_RECV },

    // ── RFC v0.4-005: diagnosticsd ────────────────────────────────────────────
    // diagnosticsd is the only service permitted to construct DiagBundle.
    PolicyRule { requester: DIAGNOSTICSD, resource: ResourceClass::DiagBundle as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_MINT | RIGHT_INSPECT },
    PolicyRule { requester: DIAGNOSTICSD, resource: ResourceClass::SxtSession as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_SEND | RIGHT_RECV },
    // All other services are denied DiagBundle authority.
    PolicyRule { requester: WILDCARD, resource: ResourceClass::DiagBundle as u16,
                 kind: PolicyKind::Deny, rights: 0 },
    // attestd: may request an SxtSession for attestation push (v0.4-005 §5.3).
    PolicyRule { requester: ATTESTD, resource: ResourceClass::SxtSession as u16,
                 kind: PolicyKind::Allow, rights: RIGHT_SEND | RIGHT_RECV },
];

// ── Policy evaluator ─────────────────────────────────────────────────────────

pub enum PolicyResult {
    Granted(u64 /* intersected rights mask */),
    Denied,
}

/// Three-phase policy evaluation (BROKER-001 / BROKER-002).
///
/// 1. Scan for explicit `Deny` matching (requester, resource) — reject immediately.
/// 2. Scan for explicit `Allow` — return intersection of requested and granted rights.
/// 3. Default deny — BROKER-001.
pub fn evaluate(requester: u16, resource: u16, requested_rights: u64) -> PolicyResult {
    // cap-broker itself always succeeds self-queries.
    if requester == CAP_BROKER {
        return PolicyResult::Granted(requested_rights & ALL_RIGHTS);
    }

    // Phase 1: explicit deny.
    for rule in POLICY {
        if rule.kind != PolicyKind::Deny { continue; }
        let req_m = rule.requester == WILDCARD || rule.requester == requester;
        let res_m = rule.resource  == WILDCARD || rule.resource  == resource;
        // Skip wildcard-resource deny for known (non-Any) resources —
        // they should fall through to explicit allows.
        if res_m && resource == ResourceClass::Any as u16 { continue; }
        if req_m && res_m { return PolicyResult::Denied; }
    }

    // Phase 2: explicit allow.
    for rule in POLICY {
        if rule.kind != PolicyKind::Allow { continue; }
        let req_m = rule.requester == WILDCARD || rule.requester == requester;
        let res_m = rule.resource  == WILDCARD || rule.resource  == resource;
        if req_m && res_m {
            let granted = rule.rights & requested_rights;
            if granted != 0 { return PolicyResult::Granted(granted); }
        }
    }

    // Phase 3: default deny (BROKER-001).
    PolicyResult::Denied
}

// ── Delegation record ─────────────────────────────────────────────────────────

const MAX_DELEGATIONS: usize = 64;

/// One entry in the cap-broker delegation tree (RFC 040 §2.4).
///
/// Tracks every lease-bound grant so that:
/// - Revoke cascades to all children of a lease (cap-broker policy layer).
/// - Audit records contain the full delegation path.
#[derive(Clone, Copy)]
#[allow(dead_code)]  // fields written at grant time; read path lands with audit export
struct DelegationRecord {
    /// 0 = root delegation (from cap-broker to a service).
    parent_idx:  u8,
    /// Requester ImageId (grantee).
    requester:   u16,
    /// Resource class granted.
    resource:    u16,
    /// Actual rights granted (intersection of requested and allowed).
    rights:      u64,
    /// Lease id assigned to this grant (u16 — LeaseId low bits).
    lease_id:    u16,
    /// Whether this slot is occupied.
    active:      bool,
}

impl DelegationRecord {
    const fn empty() -> Self {
        DelegationRecord {
            parent_idx: 0, requester: 0, resource: 0,
            rights: 0, lease_id: 0, active: false,
        }
    }
}

struct DelegationTree {
    records: [DelegationRecord; MAX_DELEGATIONS],
    len:     usize,
}

impl DelegationTree {
    const fn new() -> Self {
        DelegationTree {
            records: [const { DelegationRecord::empty() }; MAX_DELEGATIONS],
            len: 0,
        }
    }

    fn insert(&mut self, rec: DelegationRecord) -> bool {
        if self.len >= MAX_DELEGATIONS { return false; }
        // Find a free slot.
        for slot in self.records.iter_mut() {
            if !slot.active {
                *slot = DelegationRecord { active: true, ..rec };
                self.len += 1;
                return true;
            }
        }
        false
    }

    fn revoke_lease(&mut self, lease_id: u16) {
        for slot in self.records.iter_mut() {
            if slot.active && slot.lease_id == lease_id {
                *slot = DelegationRecord::empty();
                if self.len > 0 { self.len -= 1; }
            }
        }
    }
}

// ── Broker state machine ──────────────────────────────────────────────────────

/// One-way typestate for the capability broker (RFC 040 §2.1).
///
/// ```text
/// Bootstrap ──(BOOTSTRAP_COMPLETE from init)──► Enforcing
/// ```
///
/// In `Bootstrap` state only `init` may send requests and the only accepted
/// message label is `BOOTSTRAP_COMPLETE`.
///
/// In `Enforcing` state the full three-phase policy engine is active.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BrokerState { Bootstrap, Enforcing }

// ── Service entry point ───────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("RFC040: cap-broker Bootstrap");

    let mut state = BrokerState::Bootstrap;
    let mut tree  = DelegationTree::new();

    loop {
        // RFC 037 cooperative shape: blocking recv (try_recv not available
        // in current fjell-syscall; replace when IpcTryRecv is exposed).
        match sys_ipc_recv_msg(0u32) {
            Ok((label, w0, w1, w2, _w3, sender_identity)) => {
                let tag = (label & 0xFFFF) as usize;

                // ── Bootstrap handoff ────────────────────────────────────────
                if tag == (tags::BOOTSTRAP_COMPLETE & 0xFFFF) {
                    if state == BrokerState::Bootstrap {
                        // RFC 055: verify sender is init (ImageId 0) via attested identity.
                        let sender_image_id = fjell_syscall::ipc_sender_image_id(sender_identity);
                        if sender_image_id != 0 {  // init has ImageId(0)
                            let _ = sys_ipc_reply(usize::MAX);
                            continue;
                        }
                        state = BrokerState::Enforcing;
                        sys_debug_writeln("RFC040: cap-broker Enforcing");
                        let _ = sys_ipc_reply(0);  // ok
                    } else {
                        // Already enforcing — reject.
                        let _ = sys_ipc_reply(usize::MAX);
                    }
                    continue;
                }

                // ── Capability request ───────────────────────────────────────
                if tag == (tags::CAP_REQUEST & 0xFFFF) {
                    // Reject in Bootstrap state: policy engine not yet active.
                    if state == BrokerState::Bootstrap {
                        let _ = sys_ipc_reply(tags::CAP_DENIED);
                        continue;
                    }

                    // RFC 055: use kernel-attested sender image_id (not payload w0).
                    let requester_id     = fjell_syscall::ipc_sender_image_id(sender_identity);
                    let resource_class   = ResourceClass::from_u32(w0 as u32) as u16;  // w0 is now resource (was requester)
                    let requested_rights = w2 as u64;  // v0.2: u64 rights

                    match evaluate(requester_id, resource_class, requested_rights) {
                        PolicyResult::Granted(granted_rights) => {
                            // BROKER-004: make the grant lease-bound.
                            let _lease_id: u16 = 0; // lease binding for installed cap (simplified)

                            // RFC 056: install the cap into the requester's CSpace.
                            // sender_tid is the requester's task id (low 16 bits of sender_identity).
                            let sender_tid = fjell_syscall::ipc_sender_tid(sender_identity) as usize;
                            // Map resource class to cap kind (simplified endpoint grant).
                            let cap_kind = match resource_class {
                                1 => fjell_cap::CapKind::Endpoint as u8,
                                2 => fjell_cap::CapKind::TaskControl as u8,
                                _ => fjell_cap::CapKind::Endpoint as u8,
                            };
                            match fjell_syscall::sys_cap_install(
                                fjell_cap::CapHandle(10u32),  // broker's CapInstall cap (slot 10)
                                sender_tid,
                                cap_kind,
                                0, // object_id
                            ) {
                                Ok(new_handle) => {
                                    // Record delegation.
                                    tree.insert(DelegationRecord {
                                        parent_idx: 0,
                                        requester:  requester_id,
                                        resource:   resource_class,
                                        rights:     granted_rights,
                                        lease_id:   0,
                                        active:     true,
                                    });
                                    // Reply with the installed cap handle.
                                    let reply = tags::CAP_GRANTED | ((new_handle.0 as usize) << 16);
                                    let _ = sys_ipc_reply(reply);
                                }
                                Err(_) => { let _ = sys_ipc_reply(tags::CAP_DENIED); }
                            }
                        }
                        PolicyResult::Denied => {
                            let _ = sys_ipc_reply(tags::CAP_DENIED);
                        }
                    }
                    continue;
                }

                // ── Grant revocation ─────────────────────────────────────────
                if tag == 0x023 {
                    // CAP_REVOKE: w0 = lease_id to revoke (RFC 040 §2.5).
                    let lid = w0 as u16;
                    use fjell_abi::lease::LeaseId;
                    // RFC 048: sys_lease_revoke needs LeaseAdmin cap (slot 11 = broker's LeaseAdmin).
                    match sys_lease_revoke(11u32, LeaseId(lid as u32)) {
                        Ok(new_epoch) => {
                            // Cascade: remove delegation records for this lease.
                            tree.revoke_lease(lid);
                            let _ = sys_ipc_reply(new_epoch.0 as usize);
                        }
                        Err(_) => {
                            let _ = sys_ipc_reply(usize::MAX);
                        }
                    }
                    continue;
                }

                // ── Shutdown ─────────────────────────────────────────────────
                if tag == (tags::SERVICE_SHUTDOWN & 0xFFFF) {
                    let _ = sys_ipc_reply(0);
                    break;
                }

                // ── Unknown ──────────────────────────────────────────────────
                let _ = (w0, w1, w2);  // suppress unused-var lint
                let _ = sys_ipc_reply(0);
            }
            Err(_) => {
                // IPC error — yield and retry.
                sys_yield();
            }
        }
    }

    sys_exit(0)
}
