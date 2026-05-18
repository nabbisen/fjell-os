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

#[inline]
fn ecall1(nr: usize, a0: usize) -> usize {
    ecall2(nr, a0, 0, 0, 0).0
}

#[inline]
fn ecall0(nr: usize) -> usize {
    ecall2(nr, 0, 0, 0, 0).0
}

#[inline]
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
pub fn sys_ipc_reply(reply_tag: usize) -> Result<(), SysError> {
    to_result(ecall1(SyscallNumber::IpcReply as usize, reply_tag)).map(|_| ())
}

// ── M4 task syscalls ──────────────────────────────────────────────────────────

/// Spawn a task from the embedded image identified by `image_id`.
/// Returns `(task_handle_raw, task_control_cap_slot)` on success.
#[inline]
pub fn sys_task_spawn(image_id: ImageId) -> Result<(usize, usize), SysError> {
    let (r0, r1) = ecall2(SyscallNumber::TaskSpawn as usize,
                           image_id.0 as usize, 0, 0, 0);
    to_result(r0).map(|handle| (handle, r1))
}

/// Start a spawned task (transition to Runnable).
/// `entry_pc` and `stack_top` may be 0 to use the image's default entry.
#[inline]
pub fn sys_task_start(task_handle: usize, entry_pc: usize, stack_top: usize)
    -> Result<(), SysError>
{
    to_result(ecall2(SyscallNumber::TaskStart as usize,
                     task_handle, entry_pc, stack_top, 0).0)
        .map(|_| ())
}

/// Query a task's lifecycle state.  Returns raw `TaskLifecycle` byte.
#[inline]
pub fn sys_task_status(task_handle: usize) -> Result<u8, SysError> {
    to_result(ecall1(SyscallNumber::TaskStatus as usize, task_handle))
        .map(|v| v as u8)
}

// ── M4 lease syscalls ─────────────────────────────────────────────────────────

/// Create a new lease; returns the `LeaseId`.
#[inline]
pub fn sys_lease_create(flags: u32) -> Result<LeaseId, SysError> {
    to_result(ecall1(SyscallNumber::LeaseCreate as usize, flags as usize))
        .map(|v| LeaseId(v as u32))
}

/// Revoke a lease; returns the new epoch.
#[inline]
pub fn sys_lease_revoke(lease_id: LeaseId) -> Result<LeaseEpoch, SysError> {
    to_result(ecall1(SyscallNumber::LeaseRevoke as usize, lease_id.0 as usize))
        .map(|v| LeaseEpoch(v as u32))
}

/// Inspect a lease; returns its current epoch.
#[inline]
pub fn sys_lease_inspect(lease_id: LeaseId) -> Result<LeaseEpoch, SysError> {
    to_result(ecall1(SyscallNumber::LeaseInspect as usize, lease_id.0 as usize))
        .map(|v| LeaseEpoch(v as u32))
}

// ── M4 audit syscalls ─────────────────────────────────────────────────────────

/// Drain up to `buf.len()` bytes of serialized audit records into `buf`.
/// Returns the number of bytes written.
#[inline]
pub fn sys_audit_drain(buf: &mut [u8]) -> Result<usize, SysError> {
    to_result(ecall2(SyscallNumber::AuditDrain as usize,
                     buf.as_mut_ptr() as usize, buf.len(), 0, 0).0)
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

