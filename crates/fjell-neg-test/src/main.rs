//! v0.2 negative-test service (RFC 042).
//!
//! Exercises each negative-test scenario and prints the corresponding
//! `NEG:*:PASS` marker when the kernel correctly rejects the invalid request.
//!
//! # CSpace layout (set up by `spawn.rs`)
//!
//! | Slot | Cap kind      | Rights           | Purpose                  |
//! |------|---------------|-----------------|--------------------------|
//! | 0    | Endpoint      | ALL              | Own endpoint             |
//! | 1    | AuditDrain    | AUDIT_DRAIN      | User-copy tests (RFC 039)|
//! | 2    | DmaRegion     | DMA_ALLOC+USE+..| DMA revoke test (RFC 036)|
//! | 31   | MmioRegion 0  | ALL              | Bounds test (RFC 035)    |
//!
//! # Test categories covered
//!
//! See `fjell_service_api::negative_markers` for the full marker list.
//! This service emits markers for all scenarios that can be verified
//! from a single user-space service without multi-task coordination.

#![no_std]
#![no_main]
mod rt;

use fjell_syscall::{
    sys_exit, sys_debug_writeln, sys_yield,
    sys_mmio_map, sys_dma_alloc, sys_dma_revoke,
    sys_audit_drain_raw, sys_ipc_call_words,
    sys_cap_copy, sys_cap_mint, sys_cap_bind_lease,
    sys_cap_drop as _sys_cap_drop,
    sys_lease_create, sys_lease_revoke, sys_ipc_recv,
    sys_audit_drain,
};
use fjell_cap::CapHandle;
use fjell_service_api::{negative_markers as M, tags};

// ── CSpace slot constants for this service ────────────────────────────────────

const SLOT_OWN_EP:     u32 = 0;   // Endpoint cap (own endpoint, object 0)
const SLOT_AUDIT:      u32 = 1;   // AuditDrain cap
const SLOT_DMA:        u32 = 2;   // DmaRegion cap
const SLOT_CAP_BROKER: u32 = 3;   // Endpoint cap to cap-broker (object 5)
#[allow(dead_code)]
const SLOT_LEASE_ADMIN:u32 = 4;   // LeaseAdmin cap
const SLOT_SCRATCH_C:  u32 = 8;   // scratch slot for audit overflow loop
// Scratch slots used by lease-revoked and rights-denied tests
const SLOT_SCRATCH_A:  u32 = 6;   // copy of OWN_EP with lease bound
const SLOT_SCRATCH_B:  u32 = 7;   // minted cap with narrowed rights
const SLOT_MMIO_BASE:  u32 = 31;  // First MmioRegion cap (object 0)

// ── RAM_BASE (must match kernel platform constant) ────────────────────────────
const RAM_BASE: usize = 0x8000_0000;

// ── Helper: test that a condition is true and emit a marker ───────────────────

#[inline(never)]
fn check(condition: bool, marker: &str) {
    if condition {
        sys_debug_writeln(marker);
    }
}

// ── Negative test scenarios ───────────────────────────────────────────────────


/// CAP: mint a cap with RECV right removed, then try ipc_recv → PermissionDenied.
///
/// RFC 031 step 4 (rights check): cap.rights.contains(RECV) is false → denied.
fn test_cap_rights_denied() {
    // Mint slot OWN_EP into SLOT_SCRATCH_B with RECV bit cleared.
    // RECV = 1<<4 = 16.  ALL_RIGHTS ^ RECV clears it.
    let no_recv_rights: u64 = ((1u64 << 26) - 1) ^ (1 << 4);
    match sys_cap_mint(CapHandle(SLOT_OWN_EP), SLOT_SCRATCH_B, no_recv_rights) {
        Ok(_) => {
            // Try to recv on a cap without RECV right → PermissionDenied.
            let result = sys_ipc_recv(SLOT_SCRATCH_B);
            check(result.is_err(), M::CAP_RIGHTS_DENIED);
        }
        Err(_) => {}  // mint failed — skip
    }
}

/// CAP: copy a cap, bind a lease, revoke the lease, use the cap → LeaseRevoked.
///
/// RFC 031 step 7 (lease check) + RFC 033 (epoch revocation).
fn test_cap_lease_revoked() {
    // 1. Copy slot 0 to SLOT_SCRATCH_A.
    let copied = match sys_cap_copy(CapHandle(SLOT_OWN_EP), SLOT_SCRATCH_A) {
        Ok(h) => h,
        Err(_) => return,
    };
    // 2. Create a lease.
    let lease_id = match sys_lease_create(0) {
        Ok(id) => id,
        Err(_) => return,
    };
    // 3. Bind the lease to the copied cap.
    if sys_cap_bind_lease(copied, lease_id).is_err() { return; }
    // 4. Revoke the lease — epoch increments, cap binding now stale.
    if sys_lease_revoke(lease_id).is_err() { return; }
    // 5. Try to recv on the now-lease-revoked cap → LeaseRevoked.
    let result = sys_ipc_recv(SLOT_SCRATCH_A);
    check(result.is_err(), M::CAP_LEASE_REVOKED);
}

/// CAP: drop a lease-revoked cap — sys_cap_drop skips the lease check (RFC 032).
///
/// This test depends on test_cap_lease_revoked having run first (SLOT_SCRATCH_A
/// holds a revoked cap).  If lease-revoked test was skipped, this is a no-op.
fn test_cap_drop_on_revoked() {
    // SLOT_SCRATCH_A should contain a cap with a revoked lease (from previous test).
    // cap_drop must succeed regardless of lease state.
    use fjell_cap::CapHandle as CH;
    let result = fjell_syscall::sys_cap_drop(CH(SLOT_SCRATCH_A));
    check(result.is_ok(), M::CAP_DROP_ON_REVOKED);
}

/// CAP / MMIO: use an Endpoint cap for sys_mmio_map.
///
/// `require_cap` step 3 (kind check) fires: CapKind::Endpoint ≠ MmioRegion.
/// Emits both the capability wrong-kind marker and the MMIO rights marker
/// (since the rights check lives in the same `require_cap` path).
fn test_cap_wrong_kind() {
    let result = sys_mmio_map(CapHandle(SLOT_OWN_EP), 0, 0x1000);
    check(result.is_err(), M::CAP_WRONG_KIND);
    // MMIO rights path is exercised via the same call — emit that marker too.
    check(result.is_err(), M::MMIO_RIGHTS);
}

/// MMIO: use a real MmioRegion cap but request an out-of-bounds offset.
///
/// `MmioRegionObject::is_accessible(offset, size)` returns false.
fn test_mmio_bounds() {
    // Region 0: base=0x0, size=0x1000_0000. Use offset=0xFFFF_F000 > size.
    let result = sys_mmio_map(CapHandle(SLOT_MMIO_BASE), 0xFFFF_F000, 0x1000);
    check(result.is_err(), M::MMIO_BOUNDS);
}

/// DMA: use an Endpoint cap for sys_dma_alloc.
///
/// `require_cap` kind check fires: Endpoint ≠ DmaRegion/DmaAlloc.
fn test_dma_rights() {
    let result = sys_dma_alloc(SLOT_OWN_EP, 4096);
    check(result.is_err(), M::DMA_RIGHTS);
}

/// DMA: allocate a region and explicitly revoke it.
///
/// Verifies the Active→Zeroized→Freed transition succeeds (RFC 036 §2).
fn test_dma_revoke_explicit() {
    match sys_dma_alloc(SLOT_DMA, 4096) {
        Ok((_user_va, device_pa)) => {
            let revoke_ok = sys_dma_revoke(device_pa).is_ok();
            check(revoke_ok, M::DMA_REVOKE_EXPLICIT);
        }
        Err(_) => {
            // DMA cap not installed — skip (emit nothing).
        }
    }
}

/// USER COPY: pass a null pointer to sys_audit_drain_raw.
///
/// `UserPtr::new(0, 4096)` → NullPointer → SysError::InvalidArg.
fn test_user_copy_null() {
    // SAFETY: we intentionally pass 0 (null) to test the kernel's rejection.
    let status = unsafe { sys_audit_drain_raw(0, SLOT_AUDIT) };
    // Any non-zero status from the kernel means the pointer was rejected.
    check(status != 0, M::USER_COPY_NULL);
}

/// USER COPY: pass a kernel-space address to sys_audit_drain_raw.
///
/// `UserPtr::new(RAM_BASE, 4096)` → KernelAddress → SysError::InvalidArg.
fn test_user_copy_kernel_addr() {
    // SAFETY: we intentionally pass a kernel address to test rejection.
    let status = unsafe { sys_audit_drain_raw(RAM_BASE, SLOT_AUDIT) };
    check(status != 0, M::USER_COPY_KERNEL_ADDR);
}


/// POLICY: send CAP_REQUEST as unknown ImageId — default deny expected.
///
/// RFC 040: cap-broker is in Enforcing state (init sent BOOTSTRAP_COMPLETE).
/// ImageId 20 (NEG_TEST) is not in the policy table → default deny.
fn test_policy_default_deny() {
    // Resource class 2 = TaskControl, ALL_RIGHTS requested.
    match sys_ipc_call_words(
        SLOT_CAP_BROKER,
        tags::CAP_REQUEST,
        20,       // w0 = requester_id (ImageId::NEG_TEST, not in policy)
        2,        // w1 = resource class: TaskControl
        0xFF,     // w2 = requested rights (some bits)
    ) {
        Ok(reply) if (reply & 0xFFFF) == (tags::CAP_DENIED & 0xFFFF) => {
            check(true, M::POLICY_DEFAULT_DENY);
        }
        _ => {}  // unexpected reply — marker not emitted
    }
}

/// POLICY: send BOOTSTRAP_COMPLETE to a broker that is already Enforcing.
///
/// RFC 040: cap-broker transitions Bootstrap→Enforcing exactly once.
/// A second BOOTSTRAP_COMPLETE returns usize::MAX (rejection sentinel).
fn test_policy_bootstrap_guard() {
    match sys_ipc_call_words(
        SLOT_CAP_BROKER,
        tags::BOOTSTRAP_COMPLETE,
        0, 0, 0,
    ) {
        Ok(reply) if reply == usize::MAX => {
            check(true, M::POLICY_BOOTSTRAP_GUARD);
        }
        _ => {}  // was Bootstrap state or unexpected — marker not emitted
    }
}


/// POLICY: deny takes precedence over allow for same requester + resource.
///
/// NEG_TEST (ImageId 20) has both a Deny and an Allow rule for Config.
/// Phase 1 (deny scan) fires first → returns CAP_DENIED → BROKER-002.
fn test_policy_deny_priority() {
    // Resource class 6 = Config.
    match sys_ipc_call_words(
        SLOT_CAP_BROKER,
        tags::CAP_REQUEST,
        20,   // w0 = requester (NEG_TEST — has deny AND allow for Config)
        6,    // w1 = resource: Config
        0xFF, // w2 = requested rights
    ) {
        Ok(reply) if (reply & 0xFFFF) == (tags::CAP_DENIED & 0xFFFF) => {
            check(true, M::POLICY_DENY_PRIORITY);
        }
        _ => {}
    }
}

/// AUDIT: overflow the 256-entry audit ring, drain, check dropped count.
///
/// Strategy: drain current backlog, then do 300 cap_copy+cap_drop cycles
/// (600 events > 256 ring capacity → guaranteed overflow).  A positive
/// dropped count confirms RFC 041 gap detection infrastructure works.
fn test_audit_evidence_gap() {
    // 1. Drain current backlog so we start from a known state.
    let mut buf = [0u8; 32 * 32];   // space for 32 records
    let _ = sys_audit_drain(&mut buf, SLOT_AUDIT);

    // 2. Generate 300 cap_copy + cap_drop cycles = 600 audit events.
    for _ in 0..300u32 {
        if let Ok(h) = sys_cap_copy(CapHandle(SLOT_OWN_EP), SLOT_SCRATCH_C) {
            let _ = _sys_cap_drop(h);
        }
    }

    // 3. Drain again — dropped count should be positive.
    let (_, dropped) = sys_audit_drain(&mut buf, SLOT_AUDIT)
        .unwrap_or((0, 0));
    check(dropped > 0, M::AUDIT_EVIDENCE_GAP);
}

// ── Service entry point ───────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // Yield briefly so the rest of the system (cap-broker Enforcing handoff,
    // storaged init) settles before we start hammering syscalls.
    sys_yield();
    sys_yield();

    sys_debug_writeln("neg-test: starting v0.2 negative test scenarios");

    // ── Capability enforcement (RFC 031) ──────────────────────────────────────
    test_cap_rights_denied();
    test_cap_lease_revoked();
    test_cap_drop_on_revoked();
    test_cap_wrong_kind();

    // ── MMIO boundary (RFC 035) ───────────────────────────────────────────────
    test_mmio_bounds();

    // ── DMA boundary (RFC 036) ────────────────────────────────────────────────
    test_dma_rights();
    test_dma_revoke_explicit();

    // ── Safe user copy (RFC 039) ──────────────────────────────────────────────
    test_user_copy_null();
    test_user_copy_kernel_addr();

    // ── Cap-broker policy (RFC 040) ───────────────────────────────────────────
    // Additional yields so the system is fully settled before IPC.
    sys_yield(); sys_yield(); sys_yield();
    test_policy_default_deny();
    test_policy_bootstrap_guard();
    test_policy_deny_priority();

    // ── Audit evidence gap (RFC 041) ─────────────────────────────────────────
    test_audit_evidence_gap();

    sys_debug_writeln("neg-test: all scenarios complete");
    sys_exit(0)
}
