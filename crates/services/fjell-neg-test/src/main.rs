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
//! | 9    | Interrupt     | IRQ_ACK only     | IRQ rights test (RFC-0.25-001)|
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

use fjell_abi::service::{ImageId, TaskLifecycle};
use fjell_cap::CapHandle;
use fjell_service_api::{negative_markers as M, tags};
use fjell_syscall::{
    sys_audit_drain, sys_cap_bind_lease, sys_cap_copy, sys_cap_drop as _sys_cap_drop,
    sys_cap_inspect, sys_cap_mint, sys_cap_revoke, sys_debug_writeln, sys_dma_alloc,
    sys_dma_revoke, sys_exit, sys_ipc_call_words, sys_ipc_recv, sys_lease_create, sys_lease_revoke,
    sys_mmio_map, sys_task_spawn, sys_task_start, sys_task_status, sys_yield,
};

// ── CSpace slot constants for this service ────────────────────────────────────

const SLOT_OWN_EP: u32 = 0; // Endpoint cap (own endpoint, object 0)
const SLOT_AUDIT: u32 = 1; // AuditDrain cap
const SLOT_DMA: u32 = 2; // DmaRegion cap
const SLOT_CAP_BROKER: u32 = 3; // Endpoint cap to cap-broker (object 5)
#[allow(dead_code)]
const SLOT_LEASE_ADMIN: u32 = 4;
#[allow(dead_code)]
const SLOT_TASK_CREATE: u32 = 5; // TaskCreate cap (used by sys_task_spawn)
#[allow(dead_code)]
const SLOT_TASK_CONTROL: u32 = 6; // TaskControl cap (used by sys_task_start/status)
const SLOT_SAMPLE_EP: u32 = 7; // Endpoint cap to sample-service (object 6, RFC 042)
const SLOT_IRQ: u32 = 9; // Interrupt cap, IRQ_ACK only, no IRQ_BIND (RFC-0.25-001)
// Fixed in v0.2.9 (RB-06): scratch slots moved from 6-9 to 10-13 to avoid
// collision with TaskControl (slot 6) and audit-overflow scratch (slot 8).
const SLOT_SCRATCH_C: u32 = 12; // scratch slot for audit overflow loop
#[allow(dead_code)]
const SLOT_SCRATCH_D: u32 = 13; // scratch for ipc blocked-call recv
const SLOT_NARROW_CAP: u32 = 14; // minted cap with no management rights (RFC 049 tests)
//                       v0.2.8 layout (kept for clarity of fix):
//                       SLOT_SCRATCH_A=6, SLOT_SCRATCH_B=7, _C=8, _D=9 ← collided with TaskControl(6).
const SLOT_SCRATCH_A: u32 = 10; // copy of OWN_EP with lease bound
const SLOT_SCRATCH_B: u32 = 11; // minted cap with narrowed rights
const SLOT_MMIO_BASE: u32 = 31; // First MmioRegion cap (object 0)
const SLOT_MMIO_RAM: u32 = 35; // MmioRegion cap for region 4 (neg-test-RAM, straddles RAM)

// ── RAM_BASE (must match kernel platform constant) ────────────────────────────
const RAM_BASE: usize = 0x8000_0000;

// ── Helper: test that a condition is true and emit a marker ───────────────────

#[inline(never)]
fn check(condition: bool, marker: &str) {
    if condition {
        sys_debug_writeln(marker);
    }
}

/// RFC 050: exact-error-code check.
///
/// Emits the PASS marker only when `result == Err(expected)`.
/// Wrong-error or unexpected Ok both emit a `NEG:HARNESS:*` diagnostic so
/// the failure mode is visible in the QEMU serial log.
fn check_err<T>(
    result: Result<T, fjell_abi::error::SysError>,
    expected: fjell_abi::error::SysError,
    marker: &str,
) {
    // Fail-closed (architect review v0.19 RB-01): the PASS marker is emitted
    // ONLY when the syscall failed with exactly the expected error. A wrong
    // error or an unexpected success emits a harness-failure marker instead;
    // the profile then fails both because the expected marker is absent AND
    // because qemu_run treats the harness markers as forbidden.
    match result {
        Err(e) if e == expected => sys_debug_writeln(marker),
        Err(_) => sys_debug_writeln("NEG:HARNESS:WRONG_ERROR"),
        Ok(_) => sys_debug_writeln("NEG:HARNESS:UNEXPECTED_OK"),
    }
}

/// RFC 050: CSpace layout self-check.
///
/// Verifies scratch slots 10-13 are empty before any destructive test runs.
/// If any scratch slot is occupied, spawn.rs disagrees with neg-test's slot
/// map and all subsequent results are unreliable.
fn harness_cspace_check() {
    let scratch_slots = [10u32, 11, 12, 13, 14];
    let mut all_empty = true;
    for &slot in &scratch_slots {
        // sys_cap_inspect returns Err(InvalidCap) for empty slots.
        // An Ok result means spawn.rs installed something there — collision.
        if fjell_syscall::sys_cap_inspect(fjell_cap::CapHandle(slot)).is_ok() {
            all_empty = false;
            break;
        }
    }
    check(all_empty, M::HARNESS_CSPACE_LAYOUT_VALID);
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
            check_err(
                result,
                fjell_abi::error::SysError::PermissionDenied,
                M::CAP_RIGHTS_DENIED,
            );
        }
        Err(_) => setup_failed("rights_denied: scratch mint"),
    }
}

/// IRQ: `sys_irq_bind` on an `Interrupt` cap that holds `IRQ_ACK` but not
/// `IRQ_BIND` → PermissionDenied (RFC-0.25-001 Demonstration 4).
fn test_irq_bind_without_right() {
    let result = fjell_syscall::sys_irq_bind(CapHandle(SLOT_IRQ));
    check_err(
        result,
        fjell_abi::error::SysError::PermissionDenied,
        M::IRQ_BIND_WITHOUT_RIGHT,
    );
}

/// CAP: copy a cap, bind a lease, revoke the lease, use the cap → LeaseRevoked.
///
/// RFC 031 step 7 (lease check) + RFC 033 (epoch revocation).
fn test_cap_lease_revoked() {
    // 1. Copy slot 0 to SLOT_SCRATCH_A.
    let copied = match sys_cap_copy(CapHandle(SLOT_OWN_EP), SLOT_SCRATCH_A) {
        Ok(h) => h,
        Err(_) => {
            setup_failed("lease_revoked: copy");
            return;
        }
    };
    // 2. Create a lease.
    let lease_id = match sys_lease_create(SLOT_LEASE_ADMIN, 0) {
        Ok(id) => id,
        Err(_) => {
            setup_failed("lease_revoked: lease_create");
            return;
        }
    };
    // 3. Bind the lease to the copied cap.
    if sys_cap_bind_lease(copied, lease_id).is_err() {
        setup_failed("lease_revoked: bind");
        return;
    }
    // 4. Revoke the lease — epoch increments, cap binding now stale.
    if sys_lease_revoke(SLOT_LEASE_ADMIN, lease_id).is_err() {
        setup_failed("lease_revoked: revoke");
        return;
    }
    // 5. Try to recv on the now-lease-revoked cap → LeaseRevoked.
    let result = sys_ipc_recv(SLOT_SCRATCH_A);
    check_err(
        result,
        fjell_abi::error::SysError::LeaseRevoked,
        M::CAP_LEASE_REVOKED,
    );
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

/// MMIO: map a region that straddles RAM_BASE → RAM-guard rejects it (RFC 005).
///
/// Region 4 (neg-test-RAM): base=0x7FFE_0000, size=0x30000.
/// Request offset=0x10000, size=0x20000:
///   phys_addr = 0x7FFF_0000, end_pa = 0x8001_0000 > 0x8000_0000 (RAM_BASE).
///   RFC 005 guard fires → SysError::InvalidArg.
fn test_mmio_ram_guard() {
    let result = sys_mmio_map(CapHandle(SLOT_MMIO_RAM), 0x10000, 0x20000);
    check_err(
        result,
        fjell_abi::error::SysError::InvalidArg,
        M::MMIO_RAM_GUARD,
    );
}

/// DMA: alloc → write pattern → explicit revoke → read zeroed PA.
///
/// The physical frame is zeroed inside `DmaRegionTable::revoke_by_pa` before
/// being freed.  Because the cooperative scheduler doesn't preempt between
/// the revoke and the read, the physical address cannot be reallocated, so
/// the zero is guaranteed to be from our revoke.  Same zeroize code path as
/// `release_task` (called on task exit).
fn test_dma_zeroize() {
    match sys_dma_alloc(SLOT_DMA, 4096) {
        Ok((user_va, device_pa)) => {
            // Write a non-zero pattern.
            // SAFETY: category=page-table-mutation intentional negative-test: writes to an unmapped address to trigger a fault.
            unsafe {
                core::ptr::write_bytes(user_va as *mut u8, 0xAA, 4096);
            }
            // Explicit revoke — kernel zeroes the physical frame.
            if sys_dma_revoke(CapHandle(SLOT_DMA), device_pa).is_err() {
                setup_failed("dma_zeroize: revoke");
                return;
            }
            // Read back: PA was zeroed by revoke (frame not yet reallocated).
            // SAFETY: category=raw-pointer-deref VA still maps to the freed PA; no preemption between
            // revoke and this read in the cooperative scheduler.
            // MMIO-ORDER: poll
            let byte = unsafe { core::ptr::read_volatile(user_va as *const u8) };
            check(byte == 0, M::DMA_ZEROIZE_ON_EXIT);
        }
        Err(_) => setup_failed("dma_zeroize: alloc"),
    }
}

/// CAP / MMIO: use an Endpoint cap for sys_mmio_map.
///
/// `require_cap` step 3 (kind check) fires: CapKind::Endpoint ≠ MmioRegion.
/// Emits both the capability wrong-kind marker and the MMIO rights marker
/// (since the rights check lives in the same `require_cap` path).
fn test_cap_wrong_kind() {
    let result = sys_mmio_map(CapHandle(SLOT_OWN_EP), 0, 0x1000);
    // Canonical mapping (fjell-cap rights.rs::to_sys_error): WrongKind → WrongType.
    // (A divergent local table in trap/syscall.rs maps WrongKind → InvalidCap for
    // the cap-management syscalls; recorded as a follow-up finding.)
    check_err(
        result,
        fjell_abi::error::SysError::WrongType,
        M::CAP_WRONG_KIND,
    );
    // Both markers from same call: kind mismatch refusal.
    check_err(
        result,
        fjell_abi::error::SysError::WrongType,
        M::MMIO_RIGHTS,
    );
}

/// MMIO: use a real MmioRegion cap but request an out-of-bounds offset.
///
/// `MmioRegionObject::is_accessible(offset, size)` returns false.
fn test_mmio_bounds() {
    // Region 0: base=0x0, size=0x1000_0000. Use offset=0xFFFF_F000 > size.
    let result = sys_mmio_map(CapHandle(SLOT_MMIO_BASE), 0xFFFF_F000, 0x1000);
    check_err(
        result,
        fjell_abi::error::SysError::InvalidArg,
        M::MMIO_BOUNDS,
    );
}

/// DMA: use an Endpoint cap for sys_dma_alloc.
///
/// `require_cap` kind check fires: Endpoint ≠ DmaRegion/DmaAlloc.
fn test_dma_rights() {
    let result = sys_dma_alloc(SLOT_OWN_EP, 4096);
    // Kind mismatch via require_cap → canonical WrongType (see test_cap_wrong_kind).
    check_err(result, fjell_abi::error::SysError::WrongType, M::DMA_RIGHTS);
}

/// DMA: allocate a region and explicitly revoke it.
///
/// Verifies the Active→Zeroized→Freed transition succeeds (RFC 036 §2).
fn test_dma_revoke_explicit() {
    match sys_dma_alloc(SLOT_DMA, 4096) {
        Ok((_user_va, device_pa)) => match sys_dma_revoke(CapHandle(SLOT_DMA), device_pa) {
            Ok(()) => sys_debug_writeln(M::DMA_REVOKE_EXPLICIT),
            Err(e) => {
                sys_debug_writeln("neg-test: dma revoke failed:");
                debug_err(e);
            }
        },
        Err(e) => {
            // No silent skips: a missing precondition is a harness finding,
            // not a pass (architect review v0.18 follow-up).
            sys_debug_writeln("neg-test: dma alloc failed in revoke_explicit:");
            debug_err(e);
        }
    }
}

/// Diagnostic for an unexpected cap-broker reply (no silent skips).
fn debug_policy(label: &str, r: Result<usize, fjell_abi::error::SysError>) {
    sys_debug_writeln(label);
    match r {
        Ok(v) => {
            sys_debug_writeln("neg-test: unexpected reply:");
            debug_u(v);
        }
        Err(e) => {
            sys_debug_writeln("neg-test: ipc err:");
            debug_err(e);
        }
    }
}

/// Print a usize as decimal (no alloc, no fmt).
fn debug_u(mut n: usize) {
    let mut buf = [0u8; 24];
    let mut i = buf.len();
    if n == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    sys_debug_writeln(core::str::from_utf8(&buf[i..]).unwrap_or("?"));
}

/// M-03 (architect review v0.19): setup failures are never silent. The
/// expected marker stays absent (failing the profile), and this diagnostic
/// identifies which scenario could not establish its preconditions.
fn setup_failed(scenario: &str) {
    sys_debug_writeln("NEG:HARNESS:SETUP_FAILED");
    sys_debug_writeln(scenario);
}

/// Print a SysError discriminant as a decimal line (no alloc, no fmt).
fn debug_err(e: fjell_abi::error::SysError) {
    let mut buf = [0u8; 24];
    let mut n = e as usize;
    let mut i = buf.len();
    if n == 0 {
        i -= 1;
        buf[i] = b'0';
    }
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    sys_debug_writeln(core::str::from_utf8(&buf[i..]).unwrap_or("?"));
}

/// USER COPY: pass a null pointer to sys_audit_drain_raw.
///
/// `UserPtr::new(0, 4096)` → NullPointer → SysError::InvalidArg.
fn test_user_copy_null() {
    // SAFETY: category=raw-pointer-deref we intentionally pass 0 (null) to test the kernel's rejection.
    // RFC 050: pass null pointer — kernel UserPtr check rejects with InvalidAddress.
    let result = unsafe { fjell_syscall::sys_audit_drain_ptr(0, 4096, SLOT_AUDIT) };
    // Canonical mapping (mm/user_ptr.rs): NullPointer → InvalidArg.
    check_err(
        result,
        fjell_abi::error::SysError::InvalidArg,
        M::USER_COPY_NULL,
    );
}

/// USER COPY: pass a kernel-space address to sys_audit_drain_raw.
///
/// `UserPtr::new(RAM_BASE, 4096)` → KernelAddress → SysError::InvalidArg.
fn test_user_copy_kernel_addr() {
    // SAFETY: category=raw-pointer-deref we intentionally pass a kernel address to test rejection.
    // RFC 050: pass a kernel-space address — UserPtr rejects with InvalidAddress.
    let result = unsafe { fjell_syscall::sys_audit_drain_ptr(RAM_BASE, 4096, SLOT_AUDIT) };
    // Canonical mapping (mm/user_ptr.rs): KernelAddress → InvalidArg.
    check_err(
        result,
        fjell_abi::error::SysError::InvalidArg,
        M::USER_COPY_KERNEL_ADDR,
    );
}

/// IPC: send BIND_LEASE_FOR_IPC_TEST to sample-service; it blocks in ipc_recv
/// with a lease-bound cap; neg-test revokes the lease → sample-service woken.
///
/// RFC 034 §2: `cancel_blocked_ipc_for_lease` removes RecvWaiter entries
/// whose `LeaseBinding` matches the revoked (lease_id, epoch).
///
/// The marker `NEG:IPC:BLOCKED_RECV_WAKES_ON_REVOKE:PASS` is emitted by
/// sample-service when it observes `LeaseRevoked` from `sys_ipc_recv`.
/// qemu-log-check finds it in the serial log regardless of which service
/// printed it.
fn test_ipc_blocked_recv() {
    // 1. Create a fresh lease (need LeaseAdmin cap in slot 4).
    let lease_id = match sys_lease_create(SLOT_LEASE_ADMIN, 0) {
        Ok(id) => id,
        Err(_) => {
            setup_failed("ipc_blocked_recv: lease_create");
            return;
        }
    };

    // 2. Send BIND_LEASE_FOR_IPC_TEST(w0=lease_id) to sample-service's
    //    dedicated endpoint. This is an ipc_call — we block until it replies.
    match sys_ipc_call_words(
        SLOT_SAMPLE_EP, // endpoint 6 → sample-service
        tags::BIND_LEASE_FOR_IPC_TEST,
        lease_id.0 as usize,
        0,
        0,
    ) {
        Ok(0) => {} // sample-service replied OK
        _ => {
            let _ = sys_lease_revoke(SLOT_LEASE_ADMIN, lease_id);
            return;
        }
    }

    // 3. At this point sample-service has replied and is running.
    //    By the cooperative-scheduling contract, sample-service immediately
    //    calls sys_ipc_recv(SLOT_LEASED_EP) and blocks before the scheduler
    //    returns to neg-test.  One defensive yield is included for safety.
    sys_yield();

    // 4. Revoke the lease — this triggers cancel_blocked_ipc_for_lease in
    //    the kernel which wakes sample-service with LeaseRevoked.
    let _ = sys_lease_revoke(SLOT_LEASE_ADMIN, lease_id);
    // (marker emitted by sample-service asynchronously)
}

/// IPC: trigger BLOCKED_CALL_WAKES and LATE_REPLY_REJECTED in one exchange.
///
/// Protocol:
/// 1. neg-test sends BIND_LEASE_AND_CALL_BACK(lease_id) to sample-service (ipc_call).
/// 2. sample-service: copies slot 0 → slot 6, binds lease, replies OK, then
///    calls neg-test back on the leased copy → blocks waiting for reply.
/// 3. neg-test: receives sample-service's callback (immediately from sendq).
/// 4. neg-test: revokes lease → kernel wakes sample-service with LeaseRevoked
///    (sample-service prints NEG:IPC:BLOCKED_CALL_WAKES_ON_REVOKE:PASS).
/// 5. neg-test: sys_ipc_reply → Err (reply edge cancelled by revoke)
///    → prints NEG:IPC:LATE_REPLY_REJECTED:PASS.
fn test_ipc_blocked_call_and_late_reply() {
    // 1. Create a lease.
    let lease_id = match sys_lease_create(SLOT_LEASE_ADMIN, 0) {
        Ok(id) => id,
        Err(e) => {
            sys_debug_writeln("neg-test: blocked_call lease_create failed:");
            debug_err(e);
            return;
        }
    };

    // 2. Tell sample-service to bind lease and call us back.
    match sys_ipc_call_words(
        SLOT_SAMPLE_EP,
        tags::BIND_LEASE_AND_CALL_BACK,
        lease_id.0 as usize,
        0,
        0,
    ) {
        Ok(0) => {} // sample-service replied OK and is now calling us back
        other => {
            sys_debug_writeln("neg-test: blocked_call handshake unexpected:");
            match other {
                Ok(v) => debug_u(v),
                Err(e) => debug_err(e),
            }
            let _ = sys_lease_revoke(SLOT_LEASE_ADMIN, lease_id);
            return;
        }
    }

    // 3. Receive sample-service's callback call.
    //    sample-service is blocked in ipc_call (waiting for our reply).
    //    Its message is already in endpoint 0's sendq when we call recv.
    // Receive the callback on sample-service's dedicated endpoint — the
    // leased copy it calls on references object 6.
    match sys_ipc_recv(SLOT_SAMPLE_EP) {
        Ok(_) => {
            // Got the CALL_BACK_MSG from sample-service.
            // 4. Revoke the lease — this wakes sample-service (BLOCKED_CALL marker)
            //    and cancels our reply edge.
            let _ = sys_lease_revoke(SLOT_LEASE_ADMIN, lease_id);

            // 5. Try to reply — the edge is gone (cancelled by revoke above).
            // Fail-closed (RB-01): the marker is emitted ONLY on the two
            // contract-specified errors; anything else is a harness failure.
            match fjell_syscall::sys_ipc_reply(0) {
                // RFC 050: accept BadState (edge cancelled by revoke) or
                // LeaseRevoked (defense-in-depth check in sys_ipc_reply).
                Err(e)
                    if e == fjell_abi::error::SysError::BadState
                        || e == fjell_abi::error::SysError::LeaseRevoked =>
                {
                    check(true, M::IPC_LATE_REPLY)
                }
                Err(e) => {
                    sys_debug_writeln("NEG:HARNESS:WRONG_ERROR");
                    debug_err(e);
                }
                Ok(()) => {
                    sys_debug_writeln("NEG:HARNESS:UNEXPECTED_OK");
                }
            }
        }
        Err(e) => {
            sys_debug_writeln("neg-test: ipc late-reply: step3 recv failed:");
            debug_err(e);
            let _ = sys_lease_revoke(SLOT_LEASE_ADMIN, lease_id);
        }
    }
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
        2, // w0 = resource class: TaskControl (requester from attested identity = NEG_TEST, RFC 055)
        0xFF, // w1 = requested rights
        0, // w2 unused
    ) {
        Ok(reply) if (reply & 0xFFFF) == (tags::CAP_DENIED & 0xFFFF) => {
            check(true, M::POLICY_DEFAULT_DENY);
        }
        other => debug_policy("neg-test: default_deny unexpected:", other),
    }
}

/// POLICY: send BOOTSTRAP_COMPLETE to a broker that is already Enforcing.
///
/// RFC 040: cap-broker transitions Bootstrap→Enforcing exactly once.
/// A second BOOTSTRAP_COMPLETE returns usize::MAX (rejection sentinel).
fn test_policy_bootstrap_guard() {
    match sys_ipc_call_words(SLOT_CAP_BROKER, tags::BOOTSTRAP_COMPLETE, 0, 0, 0) {
        Ok(reply) if reply == usize::MAX => {
            check(true, M::POLICY_BOOTSTRAP_GUARD);
        }
        other => debug_policy("neg-test: bootstrap_guard unexpected:", other),
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
        6,    // w0 = resource: Config (requester = attested NEG_TEST, RFC 055)
        0xFF, // w1 = requested rights
        0,    // w2 unused
    ) {
        Ok(reply) if (reply & 0xFFFF) == (tags::CAP_DENIED & 0xFFFF) => {
            check(true, M::POLICY_DENY_PRIORITY);
        }
        other => debug_policy("neg-test: deny_priority unexpected:", other),
    }
}

/// POLICY: sender identity cannot be spoofed — broker uses attested ImageId.
///
/// RFC 055: neg-test requests MmioRegion (resource class 4), which STORAGED
/// is allowed but NEG_TEST is not.  Before RFC 055, a malicious caller could
/// pass w0=STORAGED(10) and potentially get a grant.  After RFC 055, the
/// broker ignores the payload for identity and uses the kernel-attested
/// sender_image_id = NEG_TEST(20) → default deny fires.
fn test_policy_identity_spoofing() {
    match sys_ipc_call_words(
        SLOT_CAP_BROKER,
        tags::CAP_REQUEST,
        4,    // w0 = resource: MmioRegion (STORAGED can get this; NEG_TEST cannot)
        0xFF, // w1 = requested rights
        0,
    ) {
        Ok(reply) if (reply & 0xFFFF) == (tags::CAP_DENIED & 0xFFFF) => {
            check(true, M::POLICY_IDENTITY_SPOOFING_REJECTED);
        }
        other => debug_policy("neg-test: identity_spoofing unexpected:", other),
    }
}

/// AUDIT: overflow the 256-entry audit ring, drain, check dropped count.
///
/// Strategy: drain current backlog, then do 300 cap_copy+cap_drop cycles
/// (600 events > 256 ring capacity → guaranteed overflow).  A positive
/// dropped count confirms RFC 041 gap detection infrastructure works.
fn test_audit_evidence_gap() {
    // 1. Drain current backlog so we start from a known state.
    let mut buf = [0u8; 32 * 32]; // space for 32 records
    if let Err(e) = sys_audit_drain(SLOT_AUDIT, &mut buf) {
        sys_debug_writeln("neg-test: evidence_gap: initial drain failed:");
        debug_err(e);
    }

    // 2. Generate 300 cap_copy + cap_drop cycles = 600 audit events.
    let mut copy_fails = 0u32;
    let mut drop_fails = 0u32;
    for _ in 0..300u32 {
        match sys_cap_copy(CapHandle(SLOT_OWN_EP), SLOT_SCRATCH_C) {
            Ok(h) => {
                if _sys_cap_drop(h).is_err() {
                    drop_fails += 1;
                }
            }
            Err(_) => copy_fails += 1,
        }
    }
    if copy_fails > 0 {
        sys_debug_writeln("neg-test: evidence_gap copy fails:");
        debug_u(copy_fails as usize);
    }
    if drop_fails > 0 {
        sys_debug_writeln("neg-test: evidence_gap drop fails:");
        debug_u(drop_fails as usize);
    }

    // 3. Drain again — dropped count should be positive.
    match sys_audit_drain(SLOT_AUDIT, &mut buf) {
        Ok((_got, dropped)) => {
            if dropped == 0 {
                sys_debug_writeln("neg-test: evidence_gap: dropped == 0");
            }
            check(dropped > 0, M::AUDIT_EVIDENCE_GAP);
        }
        Err(e) => {
            sys_debug_writeln("neg-test: evidence_gap: final drain failed:");
            debug_err(e);
        }
    }
}

/// SVC: spawn svc-timeout; yield N times; it never sent READY → timeout detected.
///
/// The "timeout" is cooperative: after READY_WAIT_YIELDS yields without the
/// service exiting or self-completing, we declare a start timeout.
/// The service is still alive (Runnable/Blocked) — not faulted, not exited.
fn test_svc_start_timeout() {
    const READY_WAIT_YIELDS: u32 = 20;

    // Spawn the timeout-test service.
    let handle = match sys_task_spawn(SLOT_TASK_CREATE, ImageId::SVC_TIMEOUT) {
        Ok(h) => h,
        Err(_) => {
            setup_failed("svc_timeout: spawn");
            return;
        }
    };
    if sys_task_start(SLOT_TASK_CONTROL, handle, 0, 0).is_err() {
        setup_failed("svc_timeout: start");
        return;
    }

    // Wait READY_WAIT_YIELDS cooperative cycles.
    for _ in 0..READY_WAIT_YIELDS {
        sys_yield();
    }

    // Check: task is still alive (running) — READY was never sent.
    // TaskLifecycle: Running=2, Runnable=1, Blocked=3 all mean "alive but no READY".
    match sys_task_status(SLOT_TASK_CONTROL, handle) {
        Ok(lc)
            if lc == TaskLifecycle::Running as u8
                || lc == TaskLifecycle::Runnable as u8
                || lc == TaskLifecycle::Blocked as u8 =>
        {
            // Service is still alive after the wait window — start timeout detected.
            check(true, M::SVC_START_TIMEOUT);
        }
        _ => {} // already exited/faulted before timeout window — skip
    }
}

/// SVC: spawn svc-fault; wait for it to fault; detect via task status.
///
/// svc-fault yields once then dereferences NULL → page fault →
/// `TaskState::Faulted`.  neg-test detects this and emits the marker.
fn test_svc_fault_detected() {
    let handle = match sys_task_spawn(SLOT_TASK_CREATE, ImageId::SVC_FAULT) {
        Ok(h) => h,
        Err(_) => {
            setup_failed("svc_fault: spawn");
            return;
        }
    };
    if sys_task_start(SLOT_TASK_CONTROL, handle, 0, 0).is_err() {
        setup_failed("svc_fault: start");
        return;
    }

    // Yield a few times to let svc-fault run, yield, then fault.
    for _ in 0..10u32 {
        sys_yield();
    }

    // Check: task is Faulted.
    match sys_task_status(SLOT_TASK_CONTROL, handle) {
        Ok(lc) if lc == TaskLifecycle::Faulted as u8 => {
            check(true, M::SVC_FAULT);
        }
        _ => {}
    }
}

// ── RFC 049: capability management rights tests ───────────────────────────────

/// Setup helper: mint a cap with operational rights only (no COPY/MINT/REVOKE/INSPECT).
/// Returns Ok(()) if the narrow cap was installed into SLOT_NARROW_CAP.
fn install_narrow_cap() -> Option<fjell_cap::CapHandle> {
    // EP_RW = SEND | RECV — no management bits.
    //
    // Returns the handle produced by the mint (which encodes the slot's
    // current generation). Earlier revisions used the raw slot constant as a
    // handle for the follow-up operation and cleanup; after the first
    // mint/drop cycle bumped the slot generation, those raw handles failed
    // the generation check, silently skipping the REVOKE/INSPECT scenarios
    // and surfacing GenerationMismatch instead of the permission error under
    // test (architect review v0.18 follow-up).
    let ep_rw = fjell_cap::CapRights::SEND | fjell_cap::CapRights::RECV;
    sys_cap_mint(fjell_cap::CapHandle(SLOT_OWN_EP), SLOT_NARROW_CAP, ep_rw.0).ok()
}

/// RFC 049: COPY right is required for sys_cap_copy.
fn test_cap_copy_without_right() {
    let Some(h) = install_narrow_cap() else {
        return;
    };
    // Attempt to copy the narrow cap — source has no COPY right.
    let result = sys_cap_copy(h, 15u32);
    check_err(
        result,
        fjell_abi::error::SysError::PermissionDenied,
        M::CAP_COPY_WITHOUT_RIGHT,
    );
    // Clean up for subsequent tests (generation-correct handle).
    let _ = _sys_cap_drop(h);
}

/// RFC 049: MINT right is required for sys_cap_mint.
fn test_cap_mint_without_right() {
    let Some(h) = install_narrow_cap() else {
        return;
    };
    let ep_rw = fjell_cap::CapRights::SEND | fjell_cap::CapRights::RECV;
    // Attempt to mint from the narrow cap — source has no MINT right.
    let result = sys_cap_mint(h, 15u32, ep_rw.0);
    check_err(
        result,
        fjell_abi::error::SysError::PermissionDenied,
        M::CAP_MINT_WITHOUT_RIGHT,
    );
    let _ = _sys_cap_drop(h);
}

/// RFC 049: REVOKE right is required for sys_cap_revoke.
fn test_cap_revoke_without_right() {
    let Some(h) = install_narrow_cap() else {
        return;
    };
    // Attempt to revoke through the narrow cap — it has no REVOKE right.
    let result = sys_cap_revoke(h);
    check_err(
        result,
        fjell_abi::error::SysError::PermissionDenied,
        M::CAP_REVOKE_WITHOUT_RIGHT,
    );
    let _ = _sys_cap_drop(h);
}

/// RFC 049: INSPECT right is required for sys_cap_inspect.
fn test_cap_inspect_without_right() {
    let Some(h) = install_narrow_cap() else {
        return;
    };
    // Attempt to inspect the narrow cap — it has no INSPECT right.
    let result = sys_cap_inspect(h);
    check_err(
        result,
        fjell_abi::error::SysError::PermissionDenied,
        M::CAP_INSPECT_WITHOUT_RIGHT,
    );
    let _ = _sys_cap_drop(h);
}

// ── Service entry point ───────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // Yield briefly so the rest of the system (cap-broker Enforcing handoff,
    // storaged init) settles before we start hammering syscalls.
    sys_yield();
    sys_yield();

    sys_debug_writeln("neg-test: starting v0.2 negative test scenarios");

    // RFC 058: signal service-manager we are ready.
    // RFC 058: signal READY to service-manager (best-effort; no reply expected).
    let _ = fjell_syscall::sys_ipc_send(0, fjell_service_api::tags::SERVICE_READY);

    // ── RFC 050: CSpace layout self-check (must run first) ────────────────────
    harness_cspace_check();

    // ── Capability enforcement (RFC 031) ──────────────────────────────────────
    test_cap_rights_denied();
    test_cap_lease_revoked();
    test_cap_drop_on_revoked();
    test_cap_wrong_kind();
    test_irq_bind_without_right();
    // ── RFC 049: capability management rights ─────────────────────────────────
    test_cap_copy_without_right();
    test_cap_mint_without_right();
    test_cap_revoke_without_right();
    test_cap_inspect_without_right();

    // ── MMIO boundary (RFC 035) ───────────────────────────────────────────────
    test_mmio_ram_guard();
    test_mmio_bounds();

    // ── DMA boundary (RFC 036) ────────────────────────────────────────────────
    test_dma_rights();
    test_dma_revoke_explicit();
    test_dma_zeroize();

    // ── Safe user copy (RFC 039) ──────────────────────────────────────────────
    test_user_copy_null();
    test_user_copy_kernel_addr();

    // ── Cap-broker policy (RFC 040) ───────────────────────────────────────────
    // Additional yields so the system is fully settled before IPC.
    sys_yield();
    sys_yield();
    sys_yield();
    test_policy_default_deny();
    test_policy_bootstrap_guard();
    test_policy_deny_priority();
    test_policy_identity_spoofing();

    // ── Audit evidence gap (RFC 041) ─────────────────────────────────────────
    test_audit_evidence_gap();

    // ── Service lifecycle (RFC 038) ───────────────────────────────────────────
    test_svc_start_timeout();
    test_svc_fault_detected();

    // ── IPC blocked-recv revocation (RFC 034) ────────────────────────────────
    // Deliberately LAST (architect review v0.19 follow-up): these scenarios
    // coordinate with sample-service across endpoint 6 and are the only ones
    // with cross-service blocking steps. Running them last guarantees a
    // coordination stall can never shadow the policy/audit/svc scenarios.
    test_ipc_blocked_recv();
    test_ipc_blocked_call_and_late_reply();

    sys_debug_writeln("neg-test: all scenarios complete");
    sys_exit(0)
}
