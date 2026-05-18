//! Thin user-space wrappers around Fjell OS syscalls.
//!
//! Each function corresponds to one `ecall` instruction.  The calling
//! convention follows the Fjell ABI: syscall number in `a7`, arguments in
//! `a0`–`a5`, status returned in `a0`.
//!
//! # Safety model
//! Every syscall wrapper is `unsafe`-free from the caller's perspective
//! (the kernel validates all inputs).  The inline assembly is `unsafe`
//! internally.
//!
//! # no_std
//! This crate targets both `no_std` bare-metal services and host-side tools.

#![no_std]

use fjell_abi::error::SysError;
use fjell_abi::lease::{LeaseEpoch, LeaseId};
use fjell_abi::service::ImageId;
use fjell_abi::syscall::SyscallNumber;
use fjell_cap::CapHandle;

// ── raw ecall primitive ───────────────────────────────────────────────────────

/// Execute a syscall with up to 4 arguments; returns (a0, a1).
#[inline]
fn ecall2(nr: usize, a0: usize, a1: usize, a2: usize, a3: usize) -> (usize, usize) {
    let r0: usize;
    let r1: usize;
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") nr,
            inlateout("a0") a0 => r0,
            inlateout("a1") a1 => r1,
            in("a2") a2,
            in("a3") a3,
            options(nostack),
        );
    }
    #[cfg(not(target_arch = "riscv64"))]
    { let _ = (nr, a0, a1, a2, a3); r0 = 0; r1 = 0; }
    (r0, r1)
}
/// Execute a syscall returning three registers (a0, a1, a2).
#[inline]
fn ecall3(nr: usize, a0: usize, a1: usize, a2: usize) -> (usize, usize, usize) {
    let r0: usize;
    let r1: usize;
    let r2: usize;
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") nr,
            inlateout("a0") a0 => r0,
            inlateout("a1") a1 => r1,
            inlateout("a2") a2 => r2,
            options(nostack),
        );
    }
    #[cfg(not(target_arch = "riscv64"))]
    { let _ = (nr, a0, a1, a2); r0 = 0; r1 = 0; r2 = 0; }
    (r0, r1, r2)
}


#[inline]
fn ecall1(nr: usize, a0: usize) -> usize {
    ecall2(nr, a0, 0, 0, 0).0
}

#[inline]
fn ecall0(nr: usize) -> usize {
    ecall2(nr, 0, 0, 0, 0).0
}

#[inline]

/// Receive an IPC message and return all five values (label, w0..w3).
///
/// Returns `Ok((label, w0, w1, w2, w3))` on success.
/// Use this variant when the caller needs the full 4-word payload.
pub fn sys_ipc_recv_msg(ep: u32)
    -> Result<(usize, usize, usize, usize, usize), SysError>
{
    let status: usize;
    let label: usize;
    let w0: usize;
    let w1: usize;
    let w2: usize;
    let w3: usize;
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!(
            "li a7, 21", "ecall",
            inlateout("a0") ep as usize => status,
            lateout("a1") label,
            lateout("a2") w0,
            lateout("a3") w1,
            lateout("a4") w2,
            lateout("a5") w3,
            lateout("a7") _,
            options(nostack),
        );
    }
    #[cfg(not(target_arch = "riscv64"))]
    { let _ = ep; status = 0; label = 0; w0 = 0; w1 = 0; w2 = 0; w3 = 0; }
    // Fixed in v0.2.9 (RB-04): actually check the a0 status — previously
    // hardcoded `to_result(0)` silently swallowed LeaseRevoked, BadState, etc.
    to_result(status)?;
    Ok((label & 0xFFFF, w0, w1, w2, w3))
}

fn to_result(raw: usize) -> Result<usize, SysError> {
    let code = raw as isize;
    if code >= 0 { Ok(raw) } else { Err(SysError::from_isize(code)) }
}

// ── M2 syscalls ───────────────────────────────────────────────────────────────

/// Cooperatively yield the CPU.
#[inline]
pub fn sys_yield() {
    ecall0(SyscallNumber::Yield as usize);
}

/// Exit the current task with the given code.
#[inline]
pub fn sys_exit(code: i32) -> ! {
    ecall1(SyscallNumber::Exit as usize, code as usize);
    loop { sys_yield(); }
}

// ── M3 IPC syscalls ───────────────────────────────────────────────────────────

/// Blocking call to `ep_handle`; returns `(status, reply_tag)`.
#[inline]
pub fn sys_ipc_call(ep_handle: u32, tag: usize) -> Result<usize, SysError> {
    let (r0, _r1) = ecall2(SyscallNumber::IpcCall as usize,
                            ep_handle as usize, tag, 0, 0);
    to_result(r0)
}

/// Block waiting to receive on `ep_handle`; returns `(status, sender_tag)`.
#[inline]
pub fn sys_ipc_recv(ep_handle: u32) -> Result<usize, SysError> {
    let (r0, r1) = ecall2(SyscallNumber::IpcRecv as usize,
                           ep_handle as usize, 0, 0, 0);
    to_result(r0).map(|_| r1)
}

/// Reply to the pending reply edge with the given tag.
#[inline]
/// Send a reply to a pending call.
///
/// ABI: the kernel reads `reply_label` from `a1`; `a0` is the (ignored)
/// ep handle slot.  `ecall1` would put the tag in `a0`, so we use `ecall2`
/// explicitly to place it in `a1`.  Fixed in v0.2.9 — RB-04.
pub fn sys_ipc_reply(reply_tag: usize) -> Result<(), SysError> {
    to_result(ecall2(SyscallNumber::IpcReply as usize, 0, reply_tag, 0, 0).0).map(|_| ())
}

// ── M4 task syscalls ──────────────────────────────────────────────────────────

/// RFC 048: first arg is the `TaskCreate` cap handle; second is the image id.
/// Returns the packed task handle `(index | generation<<16)` on success.
#[inline]
pub fn sys_task_spawn(cap_handle: u32, image_id: ImageId) -> Result<usize, SysError> {
    let (r0, r1) = ecall2(SyscallNumber::TaskSpawn as usize,
                          cap_handle as usize, image_id.0 as usize, 0, 0);
    to_result(r0).map(|_| r1)
}

/// Start a spawned task (transition to Runnable).
/// `entry_pc` and `stack_top` may be 0 to use the image's default entry.
#[inline]
/// RFC 048: first arg is `TaskControl` cap handle.
pub fn sys_task_start(cap_handle: u32, task_handle: usize, entry_pc: usize, stack_top: usize)
    -> Result<(), SysError>
{
    to_result(ecall2(SyscallNumber::TaskStart as usize,
                     cap_handle as usize, task_handle, entry_pc, stack_top).0)
        .map(|_| ())
}

/// RFC 048: first arg is `TaskControl` cap handle; second is the task handle.
#[inline]
pub fn sys_task_status(cap_handle: u32, task_handle: usize) -> Result<u8, SysError> {
    to_result(ecall2(SyscallNumber::TaskStatus as usize,
                     cap_handle as usize, task_handle, 0, 0).0)
        .map(|v| v as u8)
}

// ── M4 lease syscalls ─────────────────────────────────────────────────────────

/// RFC 048: first arg is `LeaseAdmin` cap handle; second is flags.
#[inline]
pub fn sys_lease_create(cap_handle: u32, flags: u32) -> Result<LeaseId, SysError> {
    let (r0, r1) = ecall2(SyscallNumber::LeaseCreate as usize,
                          cap_handle as usize, flags as usize, 0, 0);
    to_result(r0).map(|_| LeaseId(r1 as u32))
}

/// RFC 048: first arg is `LeaseAdmin` cap handle; second is the lease id.
#[inline]
pub fn sys_lease_revoke(cap_handle: u32, lease_id: LeaseId) -> Result<LeaseEpoch, SysError> {
    let (r0, r1) = ecall2(SyscallNumber::LeaseRevoke as usize,
                          cap_handle as usize, lease_id.0 as usize, 0, 0);
    to_result(r0).map(|_| LeaseEpoch(r1 as u32))
}

/// RFC 048: first arg is `LeaseAdmin` cap handle; second is the lease id.
#[inline]
pub fn sys_lease_inspect(cap_handle: u32, lease_id: LeaseId) -> Result<LeaseEpoch, SysError> {
    let (r0, r1) = ecall2(SyscallNumber::LeaseInspect as usize,
                          cap_handle as usize, lease_id.0 as usize, 0, 0);
    to_result(r0).map(|_| LeaseEpoch(r1 as u32))
}

// ── M4 audit syscalls ─────────────────────────────────────────────────────────

/// Drain up to `buf.len()` bytes of serialized audit records into `buf`.
/// Returns the number of bytes written.
#[inline]
/// Drain kernel audit records into `buf`.
///
/// `cap` is the `CapKind::AuditDrain` capability handle (slot 1 for auditd).
///
/// Returns `Ok((n_records, n_dropped))` where:
/// - `n_records` — number of [`fjell_audit_format::AuditRecordBin`] records
///    written into `buf` (each exactly 32 bytes).
/// - `n_dropped` — cumulative records dropped by the kernel due to ring-full.
pub fn sys_audit_drain(
    buf: &mut [u8],
    cap: u32,
) -> Result<(usize, usize), SysError> {
    let (r0, r1, r2) = ecall3(
        SyscallNumber::AuditDrain as usize,
        buf.as_mut_ptr() as usize,
        buf.len(),
        cap as usize,
    );
    to_result(r0)?;
    Ok((r1, r2))
}

// ── Debug write (testing only) ────────────────────────────────────────────────

/// Write a single byte to the kernel UART (smoke-test helper).
#[inline]
pub fn sys_debug_write_byte(b: u8) {
    ecall1(SyscallNumber::DebugWrite as usize, b as usize);
}

/// Write a string slice to the kernel UART (smoke-test helper).
#[inline]
pub fn sys_debug_write(s: &str) {
    for b in s.bytes() { sys_debug_write_byte(b); }
}

/// Write a string followed by '\n'.
#[inline]
pub fn sys_debug_writeln(s: &str) {
    sys_debug_write(s);
    sys_debug_write_byte(b'\n');
}



// ── M6 syscall wrappers ───────────────────────────────────────────────────────

/// `sys_platform_info_get() -> virtio_base_pa`
pub fn sys_platform_info_get() -> Result<usize, SysError> {
    let (a0, a1) = ecall2(SyscallNumber::PlatformInfoGet as usize, 0, 0, 0, 0);
    if a0 != 0 { Err(SysError::from_isize(a0 as isize)) } else { Ok(a1) }
}



/// `sys_dma_alloc(dma_cap, size_bytes) -> (user_va, phys_addr)`
///
/// The caller must pass `dma_cap`, a `CapKind::DmaAlloc` handle (slot 2
/// for storaged / driver-virtio-blk).  RFC 017.
pub fn sys_dma_alloc(dma_cap: u32, size_bytes: usize) -> Result<(usize, usize), SysError> {
    #[allow(unused_variables)]
    let nr = SyscallNumber::DmaAlloc as usize;
    let r0: usize; let r1: usize; let r2: usize;
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") nr,
            inlateout("a0") dma_cap as usize => r0,
            inlateout("a1") size_bytes => r1,
            lateout("a2") r2,
            options(nostack),
        );
    }
    #[cfg(not(target_arch = "riscv64"))]
    { let _ = (dma_cap, size_bytes); r0 = 0; r1 = 0; r2 = 0; }
    to_result(r0)?;
    Ok((r1, r2))
}

/// `sys_ipc_try_recv(ep_handle) -> Ok(tag_packed) | Err(WouldBlock | ...)`
///
/// RFC 019: Non-blocking IPC receive.  Returns `Err(SysError::WouldBlock)` if
/// no message is pending, without blocking the calling task.
pub fn sys_ipc_try_recv(ep: CapHandle) -> Result<usize, SysError> {
    let (r0, r1) = ecall2(SyscallNumber::IpcTryRecv as usize, ep.0 as usize, 0, 0, 0);
    to_result(r0).map(|_| r1)
}

/// `sys_mmio_map(mmio_cap, offset, size) → Ok(user_va) | Err`
///
/// RFC 016: Caller must hold a `CapKind::MmioRegion` capability.
/// `offset + size` is bounds-checked against the region by the kernel.
pub fn sys_mmio_map(mmio_cap: CapHandle, offset: usize, size: usize) -> Result<usize, SysError> {
    let (r0, r1) = ecall2(SyscallNumber::MmioMap as usize,
                          mmio_cap.0 as usize, offset, size, 0);
    to_result(r0).map(|_| r1)
}
/// `sys_cap_drop(cap_handle) -> Ok(()) | Err`
///
/// RFC 032 (v0.2.0): Explicitly drop a capability slot so it can be reused.
///
/// Unlike `sys_cap_delete`, this syscall succeeds even when the capability's
/// lease has been revoked — a task must always be able to release a dead slot.
///
/// # ABI
/// ```text
/// a7 = SyscallNumber::CapDrop (15)
/// a0 = cap_handle (u32)
/// → a0 = SysError (0 = Ok)
/// ```
pub fn sys_cap_drop(cap: CapHandle) -> Result<(), SysError> {
    let r0 = ecall1(SyscallNumber::CapDrop as usize, cap.0 as usize);
    to_result(r0).map(|_| ())
}

/// `sys_dma_revoke(a0=device_pa) -> a0=status`  (RFC 036 explicit revoke)
pub fn sys_dma_revoke(device_pa: usize) -> Result<(), SysError> {
    let r = ecall1(SyscallNumber::DmaRevoke as usize, device_pa);
    to_result(r).map(|_| ())
}

/// Raw audit-drain call for negative testing (RFC 042).
///
/// Unlike `sys_audit_drain`, accepts an arbitrary destination pointer so
/// the negative-test service can verify that `UserPtr::new` rejects invalid
/// addresses (null, kernel-space) before any page-table access.
///
/// Returns the raw kernel status code (`0` = Ok, non-zero = error).
///
/// # Safety
/// Must only be called from the negative-test service for error-path testing.
/// The pointed-to memory is NOT read on error; the caller must not use
/// results from a non-Ok status.
pub unsafe fn sys_audit_drain_raw(ptr: usize, cap: u32) -> usize {
    let (r0, _, _) = ecall3(
        SyscallNumber::AuditDrain as usize,
        ptr,
        4096,
        cap as usize,
    );
    r0
}

/// `sys_ipc_call` with up to 3 data words (RFC 042 / cap-broker protocol).
///
/// The cap-broker protocol passes:
/// - `w0` = requester ImageId
/// - `w1` = resource class discriminant
/// - `w2` = requested rights mask
///
/// Returns `Ok(reply_label)` on success.
/// Passes w0→a2, w1→a3, w2→a4 in the ECALL so the kernel copies them into
/// the `PendingMessage.words` array for the server to read.
pub fn sys_ipc_call_words(
    ep: u32, tag: usize, w0: usize, w1: usize, w2: usize,
) -> Result<usize, SysError> {
    let r0: usize;
    let r1: usize; // reply label
    #[cfg(target_arch = "riscv64")]
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7")          SyscallNumber::IpcCall as usize,
            inlateout("a0")   ep as usize => r0,
            inlateout("a1")   tag => r1,
            in("a2")          w0,
            in("a3")          w1,
            in("a4")          w2,
            options(nostack),
        );
    }
    #[cfg(not(target_arch = "riscv64"))]
    { let _ = (ep, tag, w0, w1, w2); r0 = 0; r1 = 0; }
    to_result(r0)?;
    Ok(r1)
}

// ── Cap manipulation wrappers (RFC 042) ───────────────────────────────────────

/// `sys_cap_copy(src, dst_slot) → Ok(new_handle)` (SyscallNumber::CapCopy = 10)
///
/// Copies the capability in `src` into `dst_slot`, returning the new handle.
pub fn sys_cap_copy(src: CapHandle, dst_slot: u32) -> Result<CapHandle, SysError> {
    let (r0, r1) = ecall2(
        SyscallNumber::CapCopy as usize,
        src.0 as usize, dst_slot as usize, 0, 0,
    );
    to_result(r0)?;
    Ok(CapHandle(r1 as u32))
}

/// `sys_cap_mint(src, dst_slot, rights) → Ok(new_handle)` (SyscallNumber::CapMint = 11)
///
/// Mints a derived capability into `dst_slot` with rights narrowed to
/// `src.rights & rights`.  Used to create caps with fewer rights for
/// rights-denied testing.
pub fn sys_cap_mint(src: CapHandle, dst_slot: u32, rights: u64) -> Result<CapHandle, SysError> {
    let (r0, r1) = ecall2(
        SyscallNumber::CapMint as usize,
        src.0 as usize, dst_slot as usize, rights as usize, 0,
    );
    to_result(r0)?;
    Ok(CapHandle(r1 as u32))
}

/// `sys_cap_revoke(cap) → Ok(())` — RFC 049: requires REVOKE right.
///
/// Revokes the capability subtree rooted at `cap`.  Fails with
/// `PermissionDenied` if the source cap lacks `CapRights::REVOKE`.
pub fn sys_cap_revoke(cap: CapHandle) -> Result<(), SysError> {
    to_result(ecall2(SyscallNumber::CapRevoke as usize, cap.0 as usize, 0, 0, 0).0)
        .map(|_| ())
}

/// `sys_cap_inspect(cap) → Ok((kind, rights, badge))` — RFC 049: requires INSPECT right.
///
/// Fails with `PermissionDenied` if the source cap lacks `CapRights::INSPECT`.
pub fn sys_cap_inspect(cap: CapHandle) -> Result<(usize, u64, u64), SysError> {
    let (r0, kind) = ecall2(SyscallNumber::CapInspect as usize, cap.0 as usize, 0, 0, 0);
    to_result(r0)?;
    // rights and badge returned in a2/a3; ecall2 only returns a0/a1.
    // Use inline asm to read them.
    #[cfg(target_arch = "riscv64")]
    let (rights, badge): (usize, usize) = unsafe {
        let r: usize;
        let b: usize;
        core::arch::asm!(
            "li a7, 13", "ecall",  // SyscallNumber::CapInspect
            inlateout("a0") cap.0 as usize => _,
            lateout("a1") _,
            lateout("a2") r,
            lateout("a3") b,
            lateout("a7") _,
            options(nostack),
        );
        (r, b)
    };
    #[cfg(not(target_arch = "riscv64"))]
    let (rights, badge) = (0usize, 0usize);
    let _ = kind;
    Ok((kind, rights as u64, badge as u64))
}


///
/// Binds a lease to an existing capability in the caller's CSpace.
/// After binding, `require_cap` step 7 verifies the lease is still active
/// before allowing the cap to be used.
///
/// Requires: caller must hold a `LeaseAdmin` capability with `LEASE_CREATE`.
pub fn sys_cap_bind_lease(
    cap:      CapHandle,
    lease_id: fjell_abi::lease::LeaseId,
) -> Result<(), SysError> {
    let r = ecall2(
        SyscallNumber::CapBindLease as usize,
        cap.0 as usize, lease_id.0 as usize, 0, 0,
    );
    to_result(r.0).map(|_| ())
}
