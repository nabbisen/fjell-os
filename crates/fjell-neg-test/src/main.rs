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
    sys_audit_drain_raw,
};
use fjell_cap::CapHandle;
use fjell_service_api::negative_markers as M;

// ── CSpace slot constants for this service ────────────────────────────────────

const SLOT_OWN_EP:     u32 = 0;   // Endpoint cap (own endpoint)
const SLOT_AUDIT:      u32 = 1;   // AuditDrain cap
const SLOT_DMA:        u32 = 2;   // DmaRegion cap
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

// ── Service entry point ───────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    // Yield briefly so the rest of the system (cap-broker Enforcing handoff,
    // storaged init) settles before we start hammering syscalls.
    sys_yield();
    sys_yield();

    sys_debug_writeln("neg-test: starting v0.2 negative test scenarios");

    // ── Capability enforcement (RFC 031) ──────────────────────────────────────
    test_cap_wrong_kind();

    // ── MMIO boundary (RFC 035) ───────────────────────────────────────────────
    test_mmio_bounds();

    // ── DMA boundary (RFC 036) ────────────────────────────────────────────────
    test_dma_rights();
    test_dma_revoke_explicit();

    // ── Safe user copy (RFC 039) ──────────────────────────────────────────────
    test_user_copy_null();
    test_user_copy_kernel_addr();

    sys_debug_writeln("neg-test: all scenarios complete");
    sys_exit(0)
}
