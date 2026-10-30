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
    ipc_sender_tid, sys_cap_bind_lease, sys_cap_copy, sys_cap_drop as _sys_cap_drop,
    sys_cap_inspect, sys_cap_mint, sys_cap_revoke, sys_debug_write, sys_debug_writeln,
    sys_dma_alloc, sys_dma_revoke, sys_exit, sys_ipc_call_words, sys_ipc_recv, sys_ipc_recv_msg,
    sys_lease_create, sys_lease_revoke, sys_mmio_map, sys_reboot, sys_task_spawn, sys_task_start,
    sys_task_status, sys_yield,
};

// ── CSpace slot constants for this service ────────────────────────────────────

const SLOT_OWN_EP: u32 = 0; // Endpoint cap: neg-test's scratch endpoint (NEG_TEST_OWN_EP_OBJECT, RFC-0.41-002)
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
const SLOT_REBOOT: u32 = 16; // Reboot cap, full rights (RFC-0.33-001 D15)
const SLOT_REBOOT_NARROW: u32 = 17; // scratch: the Reboot cap minted down to no REBOOT right
const SLOT_IRQ: u32 = 9; // Interrupt cap, IRQ_ACK only, no IRQ_BIND (RFC-0.25-001)
// Fixed in v0.2.9 (RB-06): scratch slots moved from 6-9 to 10-13 to avoid
// collision with TaskControl (slot 6) and audit-overflow scratch (slot 8).
#[allow(dead_code)]
const SLOT_SCRATCH_D: u32 = 13; // scratch for ipc blocked-call recv
const SLOT_NARROW_CAP: u32 = 14; // minted cap with no management rights (RFC 049 tests)
//                       v0.2.8 layout (kept for clarity of fix):
//                       SLOT_SCRATCH_A=6, SLOT_SCRATCH_B=7, _C=8, _D=9 ← collided with TaskControl(6).
const SLOT_SCRATCH_A: u32 = 10; // copy of OWN_EP with lease bound
const SLOT_SCRATCH_B: u32 = 11; // minted cap with narrowed rights
const SLOT_MMIO_BASE: u32 = 31; // First MmioRegion cap (object 0)
const SLOT_MMIO_RAM: u32 = 35; // MmioRegion cap for region 4 (neg-test-RAM, straddles RAM)
const SLOT_BOOTCTL_EP: u32 = 18; // CALL-only cap to bootctl (RFC-0.35-001 R7)
const SLOT_VERIFYD_EP: u32 = 19; // CALL-only cap to verifyd (RFC-0.35-001 R7)

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

/// CAP: revocation survives a rebind attempt (RFC-0.38-002 R3/R4, E-088).
///
/// A task whose lease has been revoked must not be able to bind its capability
/// back to that lease and recover the authority revocation took away. The
/// scenario creates a lease, binds a copied capability, uses it, revokes the
/// lease, and then tries to bind the same capability to the same (now
/// `Revoked`) lease. Two things are asserted, and the second is the property:
///
/// 1. the bind is refused with exactly `LeaseRevoked`;
/// 2. the capability **still fails** the lease check afterwards. A refusal that
///    left a usable binding in place would satisfy (1) and not (2).
///
/// The capability is exercised with `sys_cap_inspect`, which runs the lease
/// check (RFC 015) without blocking or touching an endpoint queue.
fn test_cap_rebind_after_revoke() {
    use fjell_abi::error::SysError;
    let copied = match sys_cap_copy(CapHandle(SLOT_OWN_EP), SLOT_SCRATCH_A) {
        Ok(h) => h,
        Err(_) => {
            setup_failed("rebind_after_revoke: copy");
            return;
        }
    };
    let lease_id = match sys_lease_create(SLOT_LEASE_ADMIN, 0) {
        Ok(id) => id,
        Err(_) => {
            setup_failed("rebind_after_revoke: lease_create");
            let _ = _sys_cap_drop(copied);
            return;
        }
    };
    if sys_cap_bind_lease(copied, lease_id).is_err() {
        setup_failed("rebind_after_revoke: bind");
        let _ = _sys_cap_drop(copied);
        return;
    }
    // Control: while the lease is live the capability is usable. Without this a
    // later failure could just be a capability that never worked.
    if sys_cap_inspect(copied).is_err() {
        setup_failed("rebind_after_revoke: control inspect while the lease is live");
        let _ = _sys_cap_drop(copied);
        return;
    }
    if sys_lease_revoke(SLOT_LEASE_ADMIN, lease_id).is_err() {
        setup_failed("rebind_after_revoke: revoke");
        let _ = _sys_cap_drop(copied);
        return;
    }
    if !matches!(sys_cap_inspect(copied), Err(SysError::LeaseRevoked)) {
        setup_failed("rebind_after_revoke: capability not revoked by the revoke");
        let _ = _sys_cap_drop(copied);
        return;
    }
    // The attempt: bind the same capability to the same, revoked, lease. The
    // capability is already bound, so since RFC-0.38-004 this is refused as a
    // second bind (`BadState`) — that check comes before the lease's state.
    check_err(
        sys_cap_bind_lease(copied, lease_id),
        SysError::BadState,
        M::CAP_REBIND_AFTER_REVOKE_REFUSED,
    );
    // The property: the capability is still unusable.
    check_err(
        sys_cap_inspect(copied),
        SysError::LeaseRevoked,
        M::CAP_REVOKED_STAYS_REVOKED,
    );
    let _ = _sys_cap_drop(copied);

    // RFC-0.38-002 R3, which the set-once check now shadows for a bound
    // capability: an UNBOUND capability bound to the revoked lease is refused
    // with `LeaseRevoked`.
    let fresh = match sys_cap_copy(CapHandle(SLOT_OWN_EP), SLOT_SCRATCH_A) {
        Ok(h) => h,
        Err(_) => {
            setup_failed("bind_to_revoked_lease: copy");
            return;
        }
    };
    check_err(
        sys_cap_bind_lease(fresh, lease_id),
        SysError::LeaseRevoked,
        M::CAP_BIND_TO_REVOKED_LEASE_REFUSED,
    );
    let _ = _sys_cap_drop(fresh);
}

/// CAP: a lease binding is set once (RFC-0.38-004 R2-R4, E-090).
///
/// A `LeaseAdmin` holder must not be able to move a capability off the lease
/// that governs it, or revoking that lease stops being reliable. The scenario
/// binds a copied capability to lease L1, then tries to bind it again — to a
/// different `Active` lease L2, and to L1 itself — and asserts both are refused
/// with `BadState`. Then the requirement: **revoke L1 and assert the capability
/// dies.** A refusal that left the binding corrupted would satisfy the first two
/// and fail the third. L2 is still `Active` throughout, so a bind that had
/// succeeded would have moved the capability somewhere L1's revocation cannot
/// reach.
fn test_cap_binding_is_set_once() {
    use fjell_abi::error::SysError;
    let copied = match sys_cap_copy(CapHandle(SLOT_OWN_EP), SLOT_SCRATCH_A) {
        Ok(h) => h,
        Err(_) => {
            setup_failed("set_once: copy");
            return;
        }
    };
    let (l1, l2) = match (
        sys_lease_create(SLOT_LEASE_ADMIN, 0),
        sys_lease_create(SLOT_LEASE_ADMIN, 0),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        _ => {
            setup_failed("set_once: lease_create");
            let _ = _sys_cap_drop(copied);
            return;
        }
    };
    if sys_cap_bind_lease(copied, l1).is_err() || sys_cap_inspect(copied).is_err() {
        setup_failed("set_once: first bind, or the capability does not work while L1 is live");
        let _ = _sys_cap_drop(copied);
        return;
    }
    // Same lease first, the move to L2 LAST: against a kernel that lets a bind
    // replace a binding, the capability must end up on L2 — if the order were
    // reversed the same-lease bind would move it back to L1 and the property
    // step below would pass against the very defect it is there to catch.
    check_err(
        sys_cap_bind_lease(copied, l1),
        SysError::BadState,
        M::CAP_SAME_LEASE_REBIND_REFUSED,
    );
    check_err(
        sys_cap_bind_lease(copied, l2),
        SysError::BadState,
        M::CAP_SECOND_BIND_REFUSED,
    );
    // The property: L1 still governs the capability.
    if sys_lease_revoke(SLOT_LEASE_ADMIN, l1).is_err() {
        setup_failed("set_once: revoke L1");
        let _ = _sys_cap_drop(copied);
        return;
    }
    check_err(
        sys_cap_inspect(copied),
        SysError::LeaseRevoked,
        M::CAP_ORIGINAL_LEASE_STILL_KILLS,
    );
    let _ = _sys_cap_drop(copied);
}

/// IPC: a call dequeued by `sys_ipc_try_recv` can be replied to
/// (RFC-0.38-003 R4, E-089).
///
/// No service in the tree serves a *call* through the non-blocking receive, so
/// this builds the smallest one. `sample-service` is asked to call us back on
/// its dedicated endpoint (object 6, which we hold `RECV` on through
/// `SLOT_SAMPLE_EP`); we drain that endpoint with `sys_ipc_try_recv` **only**
/// and reply. The reply is asserted by the caller, which prints
/// `NEG:IPC:TRY_RECV_CALL_REPLIED:PASS` only if it receives the label sent here.
/// Where `sys_ipc_try_recv` installs no reply edge, our `sys_ipc_reply` is refused
/// (`NEG:HARNESS:TRY_RECV_REPLY_REFUSED`, with the error) and the caller is never
/// woken, so the marker is absent.
fn test_try_recv_serves_a_call() {
    const POLL_BUDGET: u32 = 20_000;
    match sys_ipc_call_words(SLOT_SAMPLE_EP, tags::TRY_RECV_CALL_TEST, 0, 0, 0) {
        Ok(0) => {}
        _ => {
            setup_failed("try_recv_call: handshake with sample-service");
            return;
        }
    }
    let mut arrived = false;
    for _ in 0..POLL_BUDGET {
        match fjell_syscall::sys_ipc_try_recv(CapHandle(SLOT_SAMPLE_EP)) {
            Ok(tag) => {
                arrived = tag & 0xFFFF == tags::TRY_RECV_CALL_MSG;
                break;
            }
            Err(fjell_abi::error::SysError::WouldBlock) => sys_yield(),
            Err(_) => break,
        }
    }
    if !arrived {
        setup_failed("try_recv_call: the call never arrived through try_recv");
        return;
    }
    if let Err(e) = fjell_syscall::sys_ipc_reply(tags::TRY_RECV_CALL_REPLY) {
        sys_debug_writeln("NEG:HARNESS:TRY_RECV_REPLY_REFUSED");
        debug_err(e);
    }
}

/// CAP: `sys_cap_bind_lease` needs `LeaseAdmin` with `LEASE_CREATE`
/// (RFC-0.38-002 R2/R5, E-088) — and a revoked capability cannot be revived by
/// a task without it.
///
/// The authority-recovery path is a bind to a *different*, still-`Active`
/// lease: re-stamping a capability against its own revoked lease revives
/// nothing (`check_active` requires the slot to be `Active`, and a revoked slot
/// never is again — see `test_cap_rebind_after_revoke`). So this scenario:
///
/// 1. binds a copied capability to lease L1 and revokes L1 — the capability is
///    now dead (asserted, not assumed);
/// 2. creates L2, which stays `Active`;
/// 3. gives up this task's `LeaseAdmin` in two steps — first down to a
///    `LeaseAdmin` capability **without** `LEASE_CREATE`, then to none — and at
///    each step tries to bind the dead capability to L2 and is refused with
///    `PermissionDenied`, so the right is checked and not only the kind;
/// 4. asserts the capability is **still** dead (the property), not merely that
///    the calls returned errors.
///
/// Before the fix both binds succeed and the capability is usable again. Because
/// this task drops its own `LeaseAdmin`, the scenario must run after every
/// scenario that creates or revokes leases.
fn test_cap_bind_lease_without_authority() {
    use fjell_abi::error::SysError;
    let copied = match sys_cap_copy(CapHandle(SLOT_OWN_EP), SLOT_SCRATCH_A) {
        Ok(h) => h,
        Err(_) => {
            setup_failed("bind_lease_authority: copy");
            return;
        }
    };
    let (l1, l2) = match (
        sys_lease_create(SLOT_LEASE_ADMIN, 0),
        sys_lease_create(SLOT_LEASE_ADMIN, 0),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        _ => {
            setup_failed("bind_lease_authority: lease_create");
            let _ = _sys_cap_drop(copied);
            return;
        }
    };
    if sys_cap_bind_lease(copied, l1).is_err() || sys_lease_revoke(SLOT_LEASE_ADMIN, l1).is_err() {
        setup_failed("bind_lease_authority: bind and revoke L1");
        let _ = _sys_cap_drop(copied);
        return;
    }
    if !matches!(sys_cap_inspect(copied), Err(SysError::LeaseRevoked)) {
        setup_failed("bind_lease_authority: capability not dead after L1 was revoked");
        let _ = _sys_cap_drop(copied);
        return;
    }
    // A LeaseAdmin capability that lacks LEASE_CREATE. SLOT_NARROW_CAP is empty
    // here: every earlier user of it drops it again.
    let without_create =
        fjell_cap::CapRights::ALL_NON_META.0 & !fjell_cap::CapRights::LEASE_CREATE.0;
    let narrow = match sys_cap_mint(CapHandle(SLOT_LEASE_ADMIN), SLOT_NARROW_CAP, without_create) {
        Ok(h) => h,
        Err(_) => {
            setup_failed("bind_lease_authority: mint LeaseAdmin without LEASE_CREATE");
            let _ = _sys_cap_drop(copied);
            return;
        }
    };
    // Give up the full authority. Slot 4 was never dropped, so generation 0.
    if _sys_cap_drop(CapHandle(SLOT_LEASE_ADMIN)).is_err() {
        setup_failed("bind_lease_authority: drop LeaseAdmin");
        let _ = _sys_cap_drop(narrow);
        let _ = _sys_cap_drop(copied);
        return;
    }
    check_err(
        sys_cap_bind_lease(copied, l2),
        SysError::PermissionDenied,
        M::CAP_BIND_LEASE_WITHOUT_RIGHT,
    );
    let _ = _sys_cap_drop(narrow);
    check_err(
        sys_cap_bind_lease(copied, l2),
        SysError::PermissionDenied,
        M::CAP_BIND_LEASE_WITHOUT_AUTHORITY,
    );
    // The property: neither attempt revived the capability.
    check_err(
        sys_cap_inspect(copied),
        SysError::LeaseRevoked,
        M::CAP_REVOKED_NOT_REVIVED_BY_UNPRIVILEGED_BIND,
    );
    let _ = _sys_cap_drop(copied);
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

/// DMA: alloc → write pattern → explicit revoke → the revoke succeeded.
///
/// RFC-0.39-004: this used to read the frame back through the same virtual
/// address and expect zeros. That read worked only because `sys_dma_revoke` left
/// the mapping in place (E-099): the task was reading a frame it had given back.
/// With the mapping removed the read faults -- which is exactly what
/// `test_dma_revoked_va_faults` now asserts -- so the zeroing can no longer be
/// observed from here, and this scenario stops pretending it can.
///
/// What `Ok` from `sys_dma_revoke` now **means**, and the kernel checks before
/// returning it: the user mapping was removed and proved to be the one the region
/// records, the frame was zeroed **and read back as zero** (`free_dma_frame`), and
/// only then returned to the allocator. A revoke that could not do all of that
/// returns an error, so `Ok` is the evidence. The marker keeps its name; its basis
/// moved from a read through a stale mapping to a verified kernel invariant.
fn test_dma_zeroize() {
    match sys_dma_alloc(SLOT_DMA, 4096) {
        Ok((user_va, device_pa)) => {
            // Write a non-zero pattern, so there is something to zero.
            // SAFETY: category=raw-pointer-deref the VA is this task's own live DMA mapping at this point.
            unsafe {
                core::ptr::write_bytes(user_va as *mut u8, 0xAA, 4096);
            }
            match sys_dma_revoke(CapHandle(SLOT_DMA), device_pa) {
                Ok(()) => sys_debug_writeln(M::DMA_ZEROIZE_ON_EXIT),
                Err(_) => setup_failed("dma_zeroize: revoke"),
            }
        }
        Err(_) => setup_failed("dma_zeroize: alloc"),
    }
}

/// DMA (RFC-0.39-004 R4, E-099): after `sys_dma_revoke`, the revoking task must
/// no longer be able to reach the frame. The assertion is **the mapping's
/// absence**: a read of the revoked VA must *fault*.
///
/// What this deliberately does not check is that the read returns zeros. The
/// revoke zeroes the frame before freeing it, so a read that still works returns
/// innocent-looking zeros -- exactly the shape that let the hole stand from
/// v0.8.x: the read succeeds and the data looks harmless. Nor does it try to
/// observe another task's data in the frame: arranging a particular later owner
/// is allocation-order dependent, and a tier built on that would be flaky.
///
/// The fault ends this task, which is why this is a triggered scenario that runs
/// after all the others and only in the profile that injects the trigger. The
/// evidence is the kernel's `[task#N neg-test]: fault(LoadPageFault)` line.
fn test_dma_revoked_va_faults() {
    match sys_dma_alloc(SLOT_DMA, 4096) {
        Ok((user_va, device_pa)) => {
            // SAFETY: category=raw-pointer-deref the VA is this task's own live DMA mapping at this point.
            unsafe {
                core::ptr::write_bytes(user_va as *mut u8, 0x5A, 4096);
            }
            if sys_dma_revoke(CapHandle(SLOT_DMA), device_pa).is_err() {
                setup_failed("dma_revoked_va: revoke");
                return;
            }
            // The address, in decimal, so the kernel's fault line (`stval=`, hex) can
            // be compared with the address this task is about to read.
            sys_debug_write("neg-test: reading revoked dma va=");
            debug_dec(user_va as u64);
            sys_debug_writeln("");
            sys_debug_writeln(M::DMA_REVOKED_VA_READ_ATTEMPTED);
            // SAFETY: category=raw-pointer-deref intentional negative test: this VA was just revoked, so
            // the read is expected to take a LoadPageFault and end this task.
            // MMIO-ORDER: poll
            let byte = unsafe { core::ptr::read_volatile(user_va as *const u8) };
            // Reached only if the read did not fault: the mapping survived the revoke.
            let _ = byte;
            sys_debug_writeln("NEG:HARNESS:UNEXPECTED_OK");
        }
        Err(_) => setup_failed("dma_revoked_va: alloc"),
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
/// Print `n` in decimal (no allocation).
fn debug_dec(mut n: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    if let Ok(s) = core::str::from_utf8(&buf[i..]) {
        sys_debug_write(s);
    }
}

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

/// Bound on the `sys_task_status` poll in `test_ipc_blocked_recv` (D3: the
/// poll must be bounded, and must fail with a marker on exhaustion rather
/// than falling through to the revoke).
const BLOCKED_RECV_POLL_BOUND: u32 = 500;

/// IPC: send BIND_LEASE_FOR_IPC_TEST to sample-service; it blocks in ipc_recv
/// with a lease-bound cap; neg-test revokes the lease → sample-service woken.
///
/// RFC 034 §2: `cancel_blocked_ipc_for_lease` removes RecvWaiter entries
/// whose `LeaseBinding` matches the revoked (lease_id, epoch).
///
/// RFC-0.28-003: the revoke used to be issued after one defensive
/// `sys_yield()`, on the assumption that sample-service would already be
/// blocked in `sys_ipc_recv` by then — a "cooperative-scheduling contract"
/// RFC-0.26-001 removed the priority asymmetry that made true (see the
/// governing RFC's summary). The wait is now a bounded poll of
/// `sys_task_status`, addressed via a kernel-attested identity exchange
/// (§4 of the answer document) rather than assumed timing.
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

    // 3. sample-service has replied and is about to send us a one-way
    //    IPC_TEST_ABOUT_TO_BLOCK announcement on this same dedicated object
    //    (6) before it blocks on the leased cap. Receive it — this is the
    //    only way we learn sample-service's real TaskId (the attested
    //    sender identity, not anything self-reported — §4 of the answer
    //    document). A sender-mismatch or wrong-tag reply here is a harness
    //    failure, not the thing under test: fail closed rather than
    //    guessing a TaskId and polling garbage.
    let sample_tid = match sys_ipc_recv_msg(SLOT_SAMPLE_EP) {
        Ok((label, _w0, _w1, _w2, _w3, sender)) if label == tags::IPC_TEST_ABOUT_TO_BLOCK => {
            ipc_sender_tid(sender)
        }
        _ => {
            sys_debug_writeln("NEG:HARNESS:BLOCKED_RECV_IDENTITY_EXCHANGE_FAILED");
            let _ = sys_lease_revoke(SLOT_LEASE_ADMIN, lease_id);
            return;
        }
    };

    // 4. Poll sys_task_status until sample-service reads Blocked, bounded
    //    (D3): an unbounded poll turns a failing test into a hang, the
    //    failure mode this project diagnoses worst. sample-service's own
    //    generation is 0 for the run's duration (checked, not assumed —
    //    §4 of the answer document): it is spawned once at boot and never
    //    removed, faulted, or respawned by anything this profile runs.
    let sample_task_handle = sample_tid as usize;
    let mut blocked = false;
    for _ in 0..BLOCKED_RECV_POLL_BOUND {
        if let Ok(lc) = sys_task_status(SLOT_TASK_CONTROL, sample_task_handle) {
            if lc == TaskLifecycle::Blocked as u8 {
                blocked = true;
                break;
            }
        }
        sys_yield();
    }
    if !blocked {
        sys_debug_writeln("NEG:HARNESS:BLOCKED_RECV_POLL_EXHAUSTED");
        let _ = sys_lease_revoke(SLOT_LEASE_ADMIN, lease_id);
        return;
    }

    // 5. Revoke the lease — this triggers cancel_blocked_ipc_for_lease in
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

// ── Audit trail (RFC-0.41-001) ────────────────────────────────────────────────
//
// These three tests do not read the trail. They **cause** things, print what they
// caused, and the `audit` profile checks that `auditd` -- the one consumer that
// renders the trail, and now a live one -- printed them. RFC-0.40-002 and RFC 041
// asserted inside this service (draining the ring itself) because `auditd` could see
// nothing after boot; that was a workaround for a blind consumer, and it also competes
// with a live one for the same ring, so it is gone. See `tests/qemu/profiles/audit.toml`.

/// First unknown-syscall number used as a probe record. The kernel appends one audit
/// record per unknown syscall whose `arg0` is the number issued -- a record a test can
/// cause at will, with a value it chooses, that no other task produces.
const PROBE_BASE: usize = 0x4155_0000;

/// Issue the probe `nr` and print which one it was.
fn audit_probe(nr: usize) {
    fjell_syscall::sys_probe_unknown(nr);
}

/// Let the rest of the system finish starting, and `auditd` print everything it queued.
fn test_audit_settle() {
    for _ in 0..30_000 {
        sys_yield();
    }
    sys_debug_writeln("neg-test: audit tests begin (the system has settled)");
}

/// R10 / E-102: a revocation raised **after boot** must appear in `auditd`'s own
/// output, as a revocation. This only causes it: create a lease, revoke it, and say
/// which one.
fn test_audit_revocation_reaches_the_trail() {
    let lease_id = match sys_lease_create(SLOT_LEASE_ADMIN, 0) {
        Ok(id) => id,
        Err(_) => {
            setup_failed("audit_revocation: lease_create");
            return;
        }
    };
    if sys_lease_revoke(SLOT_LEASE_ADMIN, lease_id).is_err() {
        setup_failed("audit_revocation: revoke");
        return;
    }
    sys_debug_write("neg-test: audit: revoked lease arg0=");
    debug_dec(lease_id.0 as u64);
    sys_debug_writeln("");
}

/// R4: the lost wakeup, really run.
///
/// `auditd` drains until the kernel says empty and then calls `sys_audit_wait`. The
/// hazard is a record appended in the window between the two: after the drain that
/// found the ring empty, before the wait. That window is one scheduling turn wide --
/// every trap ends in a scheduling decision, so when `auditd`'s empty drain returns
/// the other runnable task gets exactly one turn before `auditd` reaches its wait.
///
/// So this test sweeps for it. Each round issues a probe `A`, which wakes `auditd`
/// and starts its cycle (drain, print the record in about a dozen traps, drain again,
/// find the ring empty, wait); then waits `k` non-appending traps (a yield appends
/// nothing: `TaskSwitch` is not recorded); then issues a probe `B`. As `k` grows from 0,
/// `B` lands first before `auditd` has printed `A` (an ordinary non-empty drain), then,
/// for some `k`, in the one-turn window between the empty drain and the wait, then
/// after `auditd` has parked (an ordinary wake). The profile requires every probe's
/// record to have been printed **before the round's `settled` line**: a `B` that fell
/// in the window and was slept through would not be missing for good (the next append
/// flushes it) -- it would be *late*, and late is what is asserted against.
///
/// Whether a round really entered the window is not taken on trust: see the review
/// request for the instrumented run that counted it, and the run with the guard removed
/// that went red.
fn test_audit_wake_is_not_lost() {
    const ROUNDS: usize = 120;
    for k in 0..ROUNDS {
        let a = PROBE_BASE + 2 * k;
        let b = a + 1;
        audit_probe(a);
        for _ in 0..k {
            sys_yield();
        }
        audit_probe(b);
        for nr in [a, b] {
            sys_debug_write("neg-test: audit probe nr=");
            debug_dec(nr as u64);
            sys_debug_writeln("");
        }
        // Let `auditd` run: it prints a record in about a dozen traps.
        for _ in 0..400 {
            sys_yield();
        }
        sys_debug_writeln("neg-test: audit settled");
    }
}

/// R6: a loss is reported, with the right count.
///
/// Issue a burst of probe records far larger than the ring and say how many and from
/// where. The ring drops the newest when full and `auditd`'s printing is slower than
/// this loop appends, so most of the burst overflows. The profile requires the trail
/// to **account** for it: probes printed + records the gap notices declare lost ==
/// the burst, and at least one lost.
fn test_audit_overflow_is_reported() {
    const BURST: usize = 600;
    const BURST_BASE: usize = 0x4156_0000;
    sys_debug_writeln("neg-test: audit burst begins");
    for i in 0..BURST {
        audit_probe(BURST_BASE + i);
    }
    sys_debug_write("neg-test: audit burst: count=");
    debug_dec(BURST as u64);
    sys_debug_write(" base=");
    debug_dec(BURST_BASE as u64);
    sys_debug_writeln("");
    // Let the trail catch up before the next tests add records of their own.
    for _ in 0..6000 {
        sys_yield();
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

    // Yield until svc-fault has run, yielded, written its console lines and faulted.
    // This was a fixed ten yields, which held only while svc-fault did nothing between
    // its own yield and its fault: RFC-0.34-002's console scenario (a 160+-byte line and
    // a partial one, written a byte per ecall) made it miss. Poll for the state
    // instead, bounded, so a task that never faults still fails closed.
    let mut faulted = false;
    for _ in 0..200u32 {
        sys_yield();
        if matches!(
            sys_task_status(SLOT_TASK_CONTROL, handle),
            Ok(lc) if lc == TaskLifecycle::Faulted as u8
        ) {
            faulted = true;
            break;
        }
    }

    // Check: task is Faulted.
    if faulted {
        check(true, M::SVC_FAULT);
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

// ── RFC-0.33-001 D15: the reset mechanism, exercised on its own ──────────────
//
// `bootctl`'s health decision cannot reach a reset in this deployment (one
// image, no slot switching — D14), so the mechanism it would use — the
// `PlatformReboot` dispatch arm, its capability check, the MMIO write — is
// exercised here instead, and the two are not joined. Two halves:
//
// * **without the right** — a `Reboot` capability minted down to lack `REBOOT`
//   is refused, exactly, with `PermissionDenied`. Marker: refused, observed.
// * **with the right** — the machine resets. **No marker says so.** The harness
//   passes the run only if QEMU itself reports `SHUTDOWN` with
//   `reason: "guest-reset"` and exited by itself (`expect_shutdown`).
//
// Neither runs unless asked. Every profile boots this same image, and a reset in
// all of them would end them all, so the scenario waits for a message on
// `neg-test`'s own endpoint that `init` sends when a console byte (`R`) is
// injected (RFC-0.33-001 D17). **A trigger for a reset must not survive the
// reset**: the byte is injected once, by an external agent, and is absent on the
// next boot. The previous trigger, a virtio entropy device on the bus, survived
// it — a machine with one would reset, boot and reset again, and the tier hid
// that only because the harness passes `-no-reboot`.

fn test_reboot() {
    // Without the right: mint the full capability down to one with no REBOOT.
    match sys_cap_mint(
        CapHandle(SLOT_REBOOT),
        SLOT_REBOOT_NARROW,
        fjell_cap::CapRights::SEND.0 as u64,
    ) {
        Ok(narrow) => check_err(
            sys_reboot(narrow, 0),
            fjell_abi::error::SysError::PermissionDenied,
            M::REBOOT_WITHOUT_RIGHT,
        ),
        Err(_) => sys_debug_writeln("NEG:HARNESS:REBOOT_NARROW_MINT_FAILED"),
    }
    // With the right. This line states an intention, not an outcome.
    sys_debug_writeln("neg-test: calling sys_reboot with the Reboot right");
    let _ = sys_reboot(CapHandle(SLOT_REBOOT), 0);
    // Reaching here means the reset did not happen: fail closed.
    sys_debug_writeln("NEG:HARNESS:REBOOT_RETURNED");
}

// ── RFC-0.35-001 R7: the update path's rejection scenarios ───────────────────
//
// Both `verifyd`'s `VERIFY` and `bootctl`'s new `BOOT_PROBE_ACTIVE_SLOT_WRITE`
// were unreachable before this line (E-072's class — a capability topology
// nothing compared). `neg-test` is the first real caller of either, over the
// CALL-only capabilities `spawn.rs` installs at `SLOT_VERIFYD_EP`/
// `SLOT_BOOTCTL_EP`.

/// `verifyd`'s `VERIFY_FAIL` arm, exercised for the first time. Two inputs,
/// both correctly refused: an absent signature (`sig_lo = 0`) and a
/// present-but-wrong one. The model has one compare, not two branches, so
/// the distinction is in what is sent, not in which code path runs.
fn test_upgrade_verify_rejections() {
    // `digest_lo` does not decide which arm this hits — `verify_dev`
    // compares the whole (digest, sig) pair against the one dev anchor, and
    // every `sig_lo` tried here is wrong for it regardless.
    const DIGEST_LO: usize = 0xAAAA_AAAA;

    // Unsigned: sig_lo = 0, the value nothing legitimately signs with.
    match sys_ipc_call_words(SLOT_VERIFYD_EP, tags::VERIFY, DIGEST_LO, 0, 0) {
        Ok(reply_tag) => check(
            reply_tag & 0xFFFF == tags::VERIFY_FAIL,
            M::UPGRADE_UNSIGNED_RELEASE_REJECTED,
        ),
        Err(_) => sys_debug_writeln("NEG:HARNESS:UPGRADE_VERIFY_UNSIGNED_CALL_FAILED"),
    }

    // Invalid: a non-zero sig_lo that does not verify against the dev anchor.
    match sys_ipc_call_words(SLOT_VERIFYD_EP, tags::VERIFY, DIGEST_LO, 0xDEAD_BEEF, 0) {
        Ok(reply_tag) => check(
            reply_tag & 0xFFFF == tags::VERIFY_FAIL,
            M::UPGRADE_INVALID_SIGNATURE_REJECTED,
        ),
        Err(_) => sys_debug_writeln("NEG:HARNESS:UPGRADE_VERIFY_INVALID_CALL_FAILED"),
    }
}

/// `bootctl`'s `BOOT_PROBE_ACTIVE_SLOT_WRITE`: a candidate-set into the
/// active slot must be refused (ADR-0009) — the receiver decides this from
/// its own `active()`, never from anything this call sends.
fn test_upgrade_active_slot_write_rejected() {
    match sys_ipc_call_words(SLOT_BOOTCTL_EP, tags::BOOT_PROBE_ACTIVE_SLOT_WRITE, 0, 0, 0) {
        Ok(reply_tag) => check(
            reply_tag & 0xFFFF == (tags::BOOT_ACTIVE_SLOT_WRITE_REJECTED & 0xFFFF),
            M::UPGRADE_ACTIVE_SLOT_WRITE_REJECTED,
        ),
        Err(_) => sys_debug_writeln("NEG:HARNESS:UPGRADE_ACTIVE_SLOT_CALL_FAILED"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // Yield briefly so the rest of the system (cap-broker Enforcing handoff,
    // storaged init) settles before we start hammering syscalls.
    sys_yield();
    sys_yield();

    sys_debug_writeln("neg-test: starting v0.2 negative test scenarios");

    // RFC 058: signal service-manager we are ready. Also NEG:SVC:
    // UNAUTHORIZED_READY_REJECTED's own trigger — service-manager records
    // this send with the kernel-attested sender identity (NEG_TEST=20),
    // which is what the marker checks.
    //
    // RFC-0.28-001: was slot 0 (this task's own identity endpoint,
    // defaulting to the contested shared object 0 — also defaulted-to by
    // auditd and bootctl, see rfcs/answers/
    // RFC-0.28-001-readiness-topology-answer.md §3). Every service now
    // gets an unconditional slot dedicated to reaching service-manager
    // instead.
    let _ = fjell_syscall::sys_ipc_send(
        fjell_service_api::ready::SERVICE_READY_SEND_SLOT,
        fjell_service_api::tags::SERVICE_READY,
    );

    // ── RFC 050: CSpace layout self-check (must run first) ────────────────────
    harness_cspace_check();

    // ── Capability enforcement (RFC 031) ──────────────────────────────────────
    test_cap_rights_denied();
    test_cap_lease_revoked();
    test_cap_drop_on_revoked();
    test_cap_rebind_after_revoke();
    test_cap_binding_is_set_once();
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

    // ── Service lifecycle (RFC 038) ───────────────────────────────────────────
    test_svc_start_timeout();
    test_svc_fault_detected();

    // ── The update path's rejection scenarios (RFC-0.35-001 R7) ──────────────
    test_upgrade_verify_rejections();
    test_upgrade_active_slot_write_rejected();

    // ── IPC blocked-recv revocation (RFC 034) ────────────────────────────────
    // Deliberately LAST (architect review v0.19 follow-up): these scenarios
    // coordinate with sample-service across endpoint 6 and are the only ones
    // with cross-service blocking steps. Running them last guarantees a
    // coordination stall can never shadow the policy/audit/svc scenarios.
    test_ipc_blocked_recv();
    test_ipc_blocked_call_and_late_reply();
    // RFC-0.38-003 R4: the one call in the tree served through try_recv.
    test_try_recv_serves_a_call();

    // ── Audit trail (RFC 041, RFC-0.41-001) ──────────────────────────────────
    // Late, and after a settle: these assert what `auditd` printed, and `auditd` prints
    // a record in about a dozen traps while every runnable task takes its turn between
    // them -- so while the rest of the system is still starting, a probe is queued
    // behind hundreds of boot records and "printed before the next line" and "every
    // lost record is counted" measure the boot, not the wake. The settle runs boot out.
    // Before `test_cap_bind_lease_without_authority`, which drops LeaseAdmin.
    test_audit_settle();
    test_audit_revocation_reaches_the_trail();
    test_audit_wake_is_not_lost();
    test_audit_overflow_is_reported();

    // RFC-0.38-002 R5: deliberately after every scenario that needs LeaseAdmin;
    // this one drops this task's own LeaseAdmin capability.
    test_cap_bind_lease_without_authority();

    sys_debug_writeln("neg-test: all scenarios complete");

    // RFC-0.33-001 D17: wait for `init`'s message on this task's own endpoint,
    // sent when the console byte `R` is injected. In every profile that injects
    // nothing (all but `reboot`) this task simply stays parked, as several
    // services do; nothing waits on its exit. Only `init` holds a capability
    // that can send here, and the capability check on `sys_reboot` is still
    // this task's own.
    if let Ok((tag, ..)) = sys_ipc_recv_msg(fjell_abi::service::NEG_TEST_TRIGGER_RECV_SLOT) {
        if tag & 0xFFFF == fjell_service_api::neg_test::RUN_REBOOT {
            test_reboot();
        }
        if tag & 0xFFFF == fjell_service_api::neg_test::RUN_DMA_REVOKED_READ {
            test_dma_revoked_va_faults();
        }
    }
    sys_exit(0)
}
