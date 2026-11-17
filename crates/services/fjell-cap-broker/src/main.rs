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
//! (deny → allow → default deny).  Grants are lease-bound (BROKER-004); the lease is
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
//! # BROKER-004 (RFC-0.45-002 R5)
//!
//! **Every capability the broker installs is bound to a lease the broker owns, so that
//! revoking the lease kills the capability.** The broker creates one lease per granted
//! request (`LEASE_CREATE`, slot 11), the kernel binds the installed capability to it
//! inside `sys_cap_install`, and the real lease id is recorded in the `DelegationRecord`.
//! The comment that used to cite this rule defined it nowhere and discarded the lease.
//!
//! # IPC protocol
//!
//! ```text
//! CAP_REQUEST (label=0x020, nwords=3):
//!   w0 = resource_class   (ResourceClass discriminant, u32)
//!   w1 = unused
//!   w2 = requested_rights (CapRights bits, u64 low 32)
//!   The requester is the kernel-attested sender ImageId (RFC 055), never the payload.
//! Reply: CAP_DENIED, or
//!   CAP_GRANTED | handle << 16 | lease << 48 | requester_tid << 56
//!   (the installed capability's handle in the *requester's* CSpace, the lease it is
//!   bound to, and the requester's task index -- all of which the requester may need).
//!
//! CAP_REVOKE (label=0x023): w0 = lease id. Honoured only for a lease in the broker's own
//!   delegation table, and only at the request of that record's grantee or of `init`
//!   (RFC-0.45-002 R7). Reply: the lease's new epoch, or usize::MAX if refused.
//!
//! BOOTSTRAP_COMPLETE (label=0x100):
//!   no payload
//! Reply: 0 (ok) or -1 (already enforcing).
//! ```

#![no_std]
#![no_main]
mod rt;

use fjell_abi::lease::LeaseId;
use fjell_cap::CapHandle;
use fjell_service_api::tags;
use fjell_syscall::{
    sys_cap_install, sys_debug_writeln, sys_exit, sys_ipc_recv_msg, sys_ipc_reply,
    sys_lease_create, sys_lease_revoke, sys_yield,
};

// ── The broker's own capabilities (installed by the kernel at spawn) ─────────
//
// Declared as constants so `capability-topology` reads them against `spawn.rs`'s installs.
// (A runtime install is not seen by that instrument -- see `fjell-consistency-check`'s
// `object_zero` note; the audit trail is what gates those.)

/// `CapInstall` capability: `CAP_INSTALL` plus the envelope of rights an install may confer.
const CAP_INSTALL_CAP: CapHandle = CapHandle(10);
/// `LeaseAdmin` capability: `LEASE_CREATE | LEASE_REVOKE` (not `LEASE_BIND`).
const LEASE_ADMIN_CAP: CapHandle = CapHandle(11);

/// `CAP_REVOKE` request label (RFC 040 §2.5).
const CAP_REVOKE_TAG: usize = 0x023;

// ── CapRights constants (v0.2 u64 bit layout) ───────────────────────────────
//
// Must match fjell_cap::CapRights bit layout.
// See crates/fjell-cap/src/rights.rs.

const RIGHT_SEND: u64 = 1 << 3;
const RIGHT_RECV: u64 = 1 << 4;
const RIGHT_CALL: u64 = 1 << 5;
const RIGHT_REPLY: u64 = 1 << 6;
const RIGHT_COPY: u64 = 1 << 7;
const RIGHT_MINT: u64 = 1 << 8;
const RIGHT_INSPECT: u64 = 1 << 10;
#[allow(dead_code)] // defined for completeness; used when service extraction lands
const RIGHT_DROP: u64 = 1 << 11;
const RIGHT_TASK_CREATE: u64 = 1 << 12;
const RIGHT_TASK_START: u64 = 1 << 13;
const RIGHT_TASK_STATUS: u64 = 1 << 14;
const RIGHT_TASK_KILL: u64 = 1 << 15;
const RIGHT_LEASE_CREATE: u64 = 1 << 16;
const RIGHT_LEASE_REVOKE: u64 = 1 << 17;
const RIGHT_MMIO_MAP: u64 = 1 << 19;
const RIGHT_DMA_ALLOC: u64 = 1 << 20;
const RIGHT_DMA_USE: u64 = 1 << 21;
const RIGHT_DMA_REVOKE: u64 = 1 << 22;
const RIGHT_AUDIT_DRAIN: u64 = 1 << 23;

/// All defined rights (26 bits).
const ALL_RIGHTS: u64 = (1u64 << 26) - 1;

/// Endpoint send + call + recv set (for most IPC services).
const EP_RW: u64 = RIGHT_SEND | RIGHT_RECV | RIGHT_CALL | RIGHT_REPLY | RIGHT_COPY;

/// Task management rights bundle.
const TASK_MGMT: u64 = RIGHT_TASK_CREATE | RIGHT_TASK_START | RIGHT_TASK_STATUS | RIGHT_TASK_KILL;

// ── Resource classes ─────────────────────────────────────────────────────────

/// The kind of kernel object / authority being requested.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ResourceClass {
    Any = 0,
    Endpoint = 1,
    TaskControl = 2,
    AuditDrain = 3,
    MmioRegion = 4,
    DmaRegion = 5,
    Config = 6,
    Semantic = 7,
    // ── v0.4 additions ──────────────────────────────────────────────────────
    /// virtio-net device capability (RFC v0.4-001).
    NetDevice = 8,
    /// `secure-transportd` session slot (RFC v0.4-003).
    SxtSession = 9,
    /// `diagnosticsd` bundle construction authority (RFC v0.4-005).
    DiagBundle = 10,
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
            10 => Self::DiagBundle,
            _ => Self::Any,
        }
    }
}

// ── Policy types ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PolicyKind {
    Allow,
    Deny,
}

/// `0xFFFF` in `requester` or `resource` means "match any".
pub struct PolicyRule {
    pub requester: u16, // ImageId or WILDCARD
    pub resource: u16,  // ResourceClass or WILDCARD
    pub kind: PolicyKind,
    /// Rights mask (v0.2 u64 layout). Meaningful only for Allow.
    pub rights: u64,
    /// The kernel object an `Allow` rule confers (RFC-0.45-002 R6): the `Endpoint` object id
    /// the broker installs a capability to. **`None` means the rule confers nothing the broker
    /// can install** -- the request is refused (`CAP_DENIED`) and says so. Only `Endpoint` is
    /// installable today (`fjell_cap::install::INSTALLABLE_KINDS`), and a rule gets an object
    /// only together with a consumer that needs it; every rule below but one is `None`.
    pub object: Option<u32>,
}

const WILDCARD: u16 = 0xFFFF;

// ── ImageId constants ─────────────────────────────────────────────────────────
// Must match fjell_abi::service::ImageId values.
const INIT: u16 = 0;
const CONFIGD: u16 = 1;
const CAP_BROKER: u16 = 2;
const AUDITD: u16 = 3;
const SVC_MANAGER: u16 = 4;
const DEVMGR: u16 = 8;
const VIRTIO_DRIVER: u16 = 9;
const STORAGED: u16 = 10;
#[allow(dead_code)] // defined for completeness; used when service extraction lands
const BOOTCTL: u16 = 11;
#[allow(dead_code)] // defined for completeness; used when service extraction lands
const UPGRADED: u16 = 12;
#[allow(dead_code)] // defined for completeness; used when service extraction lands
const VERIFYD: u16 = 14;
const SEMANTIC_STREAM: u16 = 6;
const PROXY_TEXT: u16 = 7;
/// RFC 042: dedicated negative-test service (ImageId 20).
const NEG_TEST: u16 = 20;
// ── v0.4 services (RFC v0.4-001 through v0.4-005) ───────────────────────────
const NETD: u16 = 21; // RFC v0.4-002 packet/session router
const VIRTIO_NET: u16 = 22; // RFC v0.4-001 virtio-net driver
const SECURE_TRANSPORTD: u16 = 23; // RFC v0.4-003 TLS channel service
const ATTESTD: u16 = 13; // RFC v0.3-004; extended by v0.4-005
const DIAGNOSTICSD: u16 = 24; // RFC v0.4-005 diagnostic bundle builder

// ── Policy table ─────────────────────────────────────────────────────────────
//
// Evaluation order: first Deny wins → then first Allow wins → default Deny.
// BROKER-001: default deny.  BROKER-002: deny > allow.

const POLICY: &[PolicyRule] = &[
    // ── Explicit denies ───────────────────────────────────────────────────────
    // No service may request "Any" resource class (bootstrap authority).
    PolicyRule {
        requester: WILDCARD,
        resource: ResourceClass::Any as u16,
        kind: PolicyKind::Deny,
        rights: 0,
        object: None,
    },
    // ── Explicit allows ───────────────────────────────────────────────────────
    // init: task management + endpoint routing during boot.
    PolicyRule {
        requester: INIT,
        resource: ResourceClass::TaskControl as u16,
        kind: PolicyKind::Allow,
        rights: TASK_MGMT | RIGHT_LEASE_CREATE | RIGHT_LEASE_REVOKE,
        object: None,
    },
    PolicyRule {
        requester: INIT,
        resource: ResourceClass::Endpoint as u16,
        kind: PolicyKind::Allow,
        rights: EP_RW | RIGHT_MINT,
        object: None,
    },
    // ── RFC 042: neg-test policy rules (demonstrate DENY_PRIORITY) ────────────
    // NEG_TEST is denied from Config — explicit deny takes precedence even when
    // an allow rule for the same requester+resource exists (BROKER-002).
    PolicyRule {
        requester: NEG_TEST,
        resource: ResourceClass::Config as u16,
        kind: PolicyKind::Deny,
        rights: 0,
        object: None,
    },
    // This allow would grant Config access to NEG_TEST, but the deny above
    // is evaluated first (phase 1 > phase 2), so it never fires.
    PolicyRule {
        requester: NEG_TEST,
        resource: ResourceClass::Config as u16,
        kind: PolicyKind::Allow,
        rights: EP_RW,
        object: None,
    },
    // ── RFC-0.45-002: the grant path's demonstration (the only installable rule) ─
    // NEG_TEST may be handed a `SEND`-only capability to **its own scratch endpoint**
    // (`NEG_TEST_OWN_EP_OBJECT`). This is not a convenience kept for a test: the capability
    // is a strict subset of the one `neg-test` already holds on that object (slot 0, with
    // more rights), so the grant confers **no authority it does not have** -- it exists so the
    // first runtime install is exercised on a requester and an object that really exist.
    // Nothing else in the tree can serve as a requester (`SECURE_TRANSPORTD`'s `SxtSession`
    // names no object yet). The `policy` tier asserts this grant, its use and its revocation.
    PolicyRule {
        requester: NEG_TEST,
        resource: ResourceClass::Endpoint as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_SEND,
        object: Some(fjell_abi::service::NEG_TEST_OWN_EP_OBJECT),
    },
    // An *allowed* class with no installable mapping, to show the broker refuses it rather
    // than falling through to an `Endpoint` install (the `_ =>` arm RFC-0.45-002 R6 removed).
    // It exists only so that refusal is reachable from a requester the harness controls.
    //
    // **Which refusal fires: the class->kind `match`, not the object.** `Semantic` (7) has no arm,
    // so the grant path replies `CAP_DENIED` with "resource class has no installable mapping"
    // *before* `object` is read. `object: None` on this rule is therefore unreachable
    // belt-and-braces, not the cause -- and this rule can never confer anything, so the `Allow`
    // and the `rights` below are inert. *Corrected 2026-11-17 at the architect's review: this
    // comment said the rule "confers nothing -- `object: None` -- and is refused by design",
    // naming the wrong mechanism for a correct outcome. The marker it makes reachable
    // (`UNMAPPED_CLASS_REFUSED`) tests the class arm, which is what it should test.*
    PolicyRule {
        requester: NEG_TEST,
        resource: ResourceClass::Semantic as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_SEND,
        object: None,
    },
    // service-manager: task lifecycle + endpoint management.
    PolicyRule {
        requester: SVC_MANAGER,
        resource: ResourceClass::TaskControl as u16,
        kind: PolicyKind::Allow,
        rights: TASK_MGMT,
        object: None,
    },
    PolicyRule {
        requester: SVC_MANAGER,
        resource: ResourceClass::Endpoint as u16,
        kind: PolicyKind::Allow,
        rights: EP_RW,
        object: None,
    },
    PolicyRule {
        requester: SVC_MANAGER,
        resource: ResourceClass::Config as u16,
        kind: PolicyKind::Allow,
        rights: EP_RW,
        object: None,
    },
    // auditd: kernel audit ring drain.
    PolicyRule {
        requester: AUDITD,
        resource: ResourceClass::AuditDrain as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_AUDIT_DRAIN | RIGHT_INSPECT,
        object: None,
    },
    // storaged + virtio-blk driver: hardware access.
    PolicyRule {
        requester: STORAGED,
        resource: ResourceClass::MmioRegion as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_MMIO_MAP | RIGHT_INSPECT,
        object: None,
    },
    PolicyRule {
        requester: STORAGED,
        resource: ResourceClass::DmaRegion as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_DMA_ALLOC | RIGHT_DMA_USE | RIGHT_DMA_REVOKE,
        object: None,
    },
    PolicyRule {
        requester: VIRTIO_DRIVER,
        resource: ResourceClass::MmioRegion as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_MMIO_MAP | RIGHT_INSPECT,
        object: None,
    },
    PolicyRule {
        requester: VIRTIO_DRIVER,
        resource: ResourceClass::DmaRegion as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_DMA_ALLOC | RIGHT_DMA_USE | RIGHT_DMA_REVOKE,
        object: None,
    },
    // configd: config read.
    PolicyRule {
        requester: CONFIGD,
        resource: ResourceClass::Config as u16,
        kind: PolicyKind::Allow,
        rights: EP_RW,
        object: None,
    },
    // devmgr: MMIO enumeration.
    PolicyRule {
        requester: DEVMGR,
        resource: ResourceClass::MmioRegion as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_MMIO_MAP | RIGHT_INSPECT,
        object: None,
    },
    // semantic-stream / proxy-text.
    PolicyRule {
        requester: SEMANTIC_STREAM,
        resource: ResourceClass::Semantic as u16,
        kind: PolicyKind::Allow,
        rights: EP_RW,
        object: None,
    },
    PolicyRule {
        requester: PROXY_TEXT,
        resource: ResourceClass::Semantic as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_SEND | RIGHT_RECV | RIGHT_COPY,
        object: None,
    },
    // ── RFC v0.4-001: virtio-net driver ──────────────────────────────────────
    // The virtio-net driver may map MMIO and DMA regions, and bind/ack IRQs.
    PolicyRule {
        requester: VIRTIO_NET,
        resource: ResourceClass::MmioRegion as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_MMIO_MAP | RIGHT_INSPECT,
        object: None,
    },
    PolicyRule {
        requester: VIRTIO_NET,
        resource: ResourceClass::DmaRegion as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_DMA_ALLOC | RIGHT_DMA_USE | RIGHT_DMA_REVOKE,
        object: None,
    },
    // The driver may send/recv packets on its NetDevice capability.
    PolicyRule {
        requester: VIRTIO_NET,
        resource: ResourceClass::NetDevice as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_SEND | RIGHT_RECV | RIGHT_INSPECT,
        object: None,
    },
    // ── RFC v0.4-002: netd ────────────────────────────────────────────────────
    // netd receives the NetDevice capability and allocates Session caps.
    PolicyRule {
        requester: NETD,
        resource: ResourceClass::NetDevice as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_RECV | RIGHT_INSPECT | RIGHT_MINT,
        object: None,
    },
    PolicyRule {
        requester: NETD,
        resource: ResourceClass::Endpoint as u16,
        kind: PolicyKind::Allow,
        rights: EP_RW | RIGHT_MINT,
        object: None,
    },
    // ── RFC v0.4-003: secure-transportd ──────────────────────────────────────
    // secure-transportd may acquire SxtSession caps and open Endpoint channels.
    PolicyRule {
        requester: SECURE_TRANSPORTD,
        resource: ResourceClass::SxtSession as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_SEND | RIGHT_RECV | RIGHT_MINT,
        object: None,
    },
    PolicyRule {
        requester: SECURE_TRANSPORTD,
        resource: ResourceClass::Endpoint as u16,
        kind: PolicyKind::Allow,
        rights: EP_RW,
        object: None,
    },
    // upgraded may open an SxtSession to request update metadata.
    PolicyRule {
        requester: UPGRADED,
        resource: ResourceClass::SxtSession as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_SEND | RIGHT_RECV,
        object: None,
    },
    // ── RFC v0.4-005: diagnosticsd ────────────────────────────────────────────
    // diagnosticsd is the only service permitted to construct DiagBundle.
    PolicyRule {
        requester: DIAGNOSTICSD,
        resource: ResourceClass::DiagBundle as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_MINT | RIGHT_INSPECT,
        object: None,
    },
    PolicyRule {
        requester: DIAGNOSTICSD,
        resource: ResourceClass::SxtSession as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_SEND | RIGHT_RECV,
        object: None,
    },
    // All other services are denied DiagBundle authority.
    PolicyRule {
        requester: WILDCARD,
        resource: ResourceClass::DiagBundle as u16,
        kind: PolicyKind::Deny,
        rights: 0,
        object: None,
    },
    // attestd: may request an SxtSession for attestation push (v0.4-005 §5.3).
    PolicyRule {
        requester: ATTESTD,
        resource: ResourceClass::SxtSession as u16,
        kind: PolicyKind::Allow,
        rights: RIGHT_SEND | RIGHT_RECV,
        object: None,
    },
];

// ── Policy evaluator ─────────────────────────────────────────────────────────

pub enum PolicyResult {
    /// `rights`: the intersected rights mask; `object`: what the matching rule confers
    /// (`None` = nothing installable, RFC-0.45-002 R6).
    Granted {
        rights: u64,
        object: Option<u32>,
    },
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
        return PolicyResult::Granted {
            rights: requested_rights & ALL_RIGHTS,
            object: None,
        };
    }

    // Phase 1: explicit deny.
    for rule in POLICY {
        if rule.kind != PolicyKind::Deny {
            continue;
        }
        let req_m = rule.requester == WILDCARD || rule.requester == requester;
        let res_m = rule.resource == WILDCARD || rule.resource == resource;
        // Skip wildcard-resource deny for known (non-Any) resources —
        // they should fall through to explicit allows.
        if res_m && resource == ResourceClass::Any as u16 {
            continue;
        }
        if req_m && res_m {
            return PolicyResult::Denied;
        }
    }

    // Phase 2: explicit allow.
    for rule in POLICY {
        if rule.kind != PolicyKind::Allow {
            continue;
        }
        let req_m = rule.requester == WILDCARD || rule.requester == requester;
        let res_m = rule.resource == WILDCARD || rule.resource == resource;
        if req_m && res_m {
            let granted = rule.rights & requested_rights;
            if granted != 0 {
                return PolicyResult::Granted {
                    rights: granted,
                    object: rule.object,
                };
            }
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
#[allow(dead_code)] // fields written at grant time; read path lands with audit export
struct DelegationRecord {
    /// 0 = root delegation (from cap-broker to a service).
    parent_idx: u8,
    /// Requester ImageId (grantee).
    requester: u16,
    /// Resource class granted.
    resource: u16,
    /// Actual rights granted (intersection of requested and allowed).
    rights: u64,
    /// Lease id assigned to this grant (u16 — LeaseId low bits).
    lease_id: u16,
    /// Whether this slot is occupied.
    active: bool,
}

impl DelegationRecord {
    const fn empty() -> Self {
        DelegationRecord {
            parent_idx: 0,
            requester: 0,
            resource: 0,
            rights: 0,
            lease_id: 0,
            active: false,
        }
    }
}

struct DelegationTree {
    records: [DelegationRecord; MAX_DELEGATIONS],
    len: usize,
}

impl DelegationTree {
    const fn new() -> Self {
        DelegationTree {
            records: [const { DelegationRecord::empty() }; MAX_DELEGATIONS],
            len: 0,
        }
    }

    fn insert(&mut self, rec: DelegationRecord) -> bool {
        if self.len >= MAX_DELEGATIONS {
            return false;
        }
        // Find a free slot.
        for slot in self.records.iter_mut() {
            if !slot.active {
                *slot = DelegationRecord {
                    active: true,
                    ..rec
                };
                self.len += 1;
                return true;
            }
        }
        false
    }

    /// Room for one more record. Checked **before** a lease is created or a capability
    /// installed, so a full table refuses the request instead of leaving an installed
    /// capability the broker has no record of.
    fn has_room(&self) -> bool {
        self.records.iter().any(|r| !r.active)
    }

    /// The grantee of the active record for `lease_id`, if the broker granted one
    /// (RFC-0.45-002 R7: a lease the broker did not create is not the broker's to revoke).
    fn grantee_of(&self, lease_id: u16) -> Option<u16> {
        self.records
            .iter()
            .find(|r| r.active && r.lease_id == lease_id)
            .map(|r| r.requester)
    }

    fn revoke_lease(&mut self, lease_id: u16) {
        for slot in self.records.iter_mut() {
            if slot.active && slot.lease_id == lease_id {
                *slot = DelegationRecord::empty();
                if self.len > 0 {
                    self.len -= 1;
                }
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
enum BrokerState {
    Bootstrap,
    Enforcing,
}

// ── Service entry point ───────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("RFC040: cap-broker Bootstrap");

    let mut state = BrokerState::Bootstrap;
    let mut tree = DelegationTree::new();

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
                        if sender_image_id != 0 {
                            // init has ImageId(0)
                            let _ = sys_ipc_reply(usize::MAX);
                            continue;
                        }
                        state = BrokerState::Enforcing;
                        sys_debug_writeln("RFC040: cap-broker Enforcing");
                        let _ = sys_ipc_reply(0); // ok
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
                    let requester_id = fjell_syscall::ipc_sender_image_id(sender_identity);
                    let resource_class = ResourceClass::from_u32(w0 as u32) as u16; // w0 is now resource (was requester)
                    let requested_rights = w2 as u64; // v0.2: u64 rights

                    match evaluate(requester_id, resource_class, requested_rights) {
                        PolicyResult::Granted {
                            rights: granted_rights,
                            object,
                        } => {
                            // R6: class -> kind is an explicit table with **no wildcard
                            // arm**. It used to end `_ => Endpoint`, so a class with no
                            // mapping silently became an Endpoint install. Class 2
                            // (`TaskControl`) stays refused: it would be `ObjectScope::Any`
                            // control over every task, and there is no scoped design for it.
                            let cap_kind = match resource_class {
                                1 => fjell_cap::CapKind::Endpoint as u8,
                                _ => {
                                    sys_debug_writeln(
                                        "cap-broker: refused: resource class has no installable mapping",
                                    );
                                    let _ = sys_ipc_reply(tags::CAP_DENIED);
                                    continue;
                                }
                            };
                            let Some(object_id) = object else {
                                sys_debug_writeln(
                                    "cap-broker: refused: the matching rule confers no installable object",
                                );
                                let _ = sys_ipc_reply(tags::CAP_DENIED);
                                continue;
                            };
                            if !tree.has_room() {
                                sys_debug_writeln("cap-broker: refused: delegation table full");
                                let _ = sys_ipc_reply(tags::CAP_DENIED);
                                continue;
                            }

                            // BROKER-004: the grant is lease-bound. The lease is created by,
                            // and owned by, the broker; the kernel binds the installed
                            // capability to it inside `sys_cap_install`.
                            //
                            // NB the call's own lease binding (the reply edge's) is
                            // irrelevant to this: an unbound call can be served an install.
                            let lease = match sys_lease_create(LEASE_ADMIN_CAP.0, 0) {
                                Ok(l) => l,
                                Err(_) => {
                                    sys_debug_writeln("cap-broker: refused: no lease available");
                                    let _ = sys_ipc_reply(tags::CAP_DENIED);
                                    continue;
                                }
                            };

                            // RFC-0.45-002: the capability is installed into the task being
                            // served -- the caller of this very call. There is no target to
                            // name, so none can be wrong.
                            match sys_cap_install(
                                CAP_INSTALL_CAP,
                                cap_kind,
                                object_id,
                                granted_rights,
                                lease,
                            ) {
                                Ok(new_handle) => {
                                    tree.insert(DelegationRecord {
                                        parent_idx: 0,
                                        requester: requester_id,
                                        resource: resource_class,
                                        rights: granted_rights,
                                        lease_id: lease.0 as u16,
                                        active: true,
                                    });
                                    let requester_tid =
                                        fjell_syscall::ipc_sender_tid(sender_identity) as usize;
                                    let reply = tags::CAP_GRANTED
                                        | ((new_handle.0 as usize) << 16)
                                        | (((lease.0 as usize) & 0xFF) << 48)
                                        | ((requester_tid & 0xFF) << 56);
                                    let _ = sys_ipc_reply(reply);
                                }
                                Err(_) => {
                                    // Nothing was installed: do not leave the lease Active.
                                    let _ = sys_lease_revoke(LEASE_ADMIN_CAP.0, lease);
                                    let _ = sys_ipc_reply(tags::CAP_DENIED);
                                }
                            }
                        }
                        PolicyResult::Denied => {
                            let _ = sys_ipc_reply(tags::CAP_DENIED);
                        }
                    }
                    continue;
                }

                // ── Grant revocation (RFC-0.45-002 R7) ───────────────────────
                if tag == CAP_REVOKE_TAG {
                    // w0 = lease id. **Authorised**: this arm revokes with the broker's own
                    // `LeaseAdmin`, which -- unlike the requester's -- can revoke any lease in
                    // the system. Without these two checks, any service that can call the
                    // broker could revoke any lease: a confused deputy, created the moment the
                    // broker was given the authority (`R5`). So:
                    //  * the lease must be one **this broker granted** (it is in the table);
                    //  * and the sender must be that grant's grantee, or `init`.
                    let lid = w0 as u16;
                    let sender_image_id = fjell_syscall::ipc_sender_image_id(sender_identity);
                    let authorised = fjell_service_api::cap_broker::revoke_authorised(
                        tree.grantee_of(lid),
                        sender_image_id,
                        INIT,
                    );
                    if !authorised {
                        sys_debug_writeln(
                            "cap-broker: CAP_REVOKE refused (not the grantee, or not a granted lease)",
                        );
                        let _ = sys_ipc_reply(usize::MAX);
                        continue;
                    }
                    match sys_lease_revoke(LEASE_ADMIN_CAP.0, LeaseId(lid as u32)) {
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
                let _ = (w0, w1, w2); // suppress unused-var lint
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
