//! Embedded static user task images for M2 smoke testing.
//!
//! No ELF loader in M2.  Two tiny RISC-V RV64 programs are expressed as
//! raw machine-code byte arrays and mapped directly into user address spaces.
//!
//! # user_task_a
//!   sys_yield → sys_yield → sys_exit(0)
//!   Expected output: "user0: yield", "user0: yield", "user0: exit(0)"
//!
//! # user_task_b
//!   sys_yield → illegal load (NULL dereference) → TaskState::Faulted
//!   Expected output: "user1: yield", "user1: fault(load page fault)"

/// Entry point virtual address for all embedded user tasks.
pub const USER_ENTRY_VA: usize = 0x0001_0000;

/// User stack top virtual address.
pub const USER_STACK_TOP: usize = 0x7FFF_F000;

/// Virtual address range for user text (one page).
pub const USER_TEXT_VA: usize = USER_ENTRY_VA;

// ── Encoded RISC-V 64 instructions ───────────────────────────────────────────
//
// ecall          : 0x0000_0073
// li a7, N       : 0x0N00_0893  (addi a7, zero, N) — works for N < 12
// li a0, 0       : 0x0000_0513  (addi a0, zero, 0)
// wfi            : 0x1050_0073
// ld a0, 0(zero) : 0x0000_3503  — loads from address 0 → page fault
//
// user_task_a:  li a7,0; ecall; li a7,0; ecall; li a0,0; li a7,1; ecall; wfi
// user_task_b:  li a7,0; ecall; ld a0,0(zero); wfi

/// `user_task_a`: yield, yield, exit(0).
///
/// Instruction layout (each instruction is 4 bytes, little-endian u32):
///   li  a7, 0    (SyscallNumber::Yield)
///   ecall
///   li  a7, 0
///   ecall
///   li  a0, 0    (exit code 0)
///   li  a7, 1    (SyscallNumber::Exit)
///   ecall
///   wfi          (safety net — should never reach here)
pub static USER_TASK_A: &[u8] = &[
    // li a7, 0  (addi a7, zero, 0)
    0x93, 0x08, 0x00, 0x00,
    // ecall
    0x73, 0x00, 0x00, 0x00,
    // li a7, 0
    0x93, 0x08, 0x00, 0x00,
    // ecall
    0x73, 0x00, 0x00, 0x00,
    // li a0, 0  (addi a0, zero, 0)
    0x13, 0x05, 0x00, 0x00,
    // li a7, 1  (addi a7, zero, 1)
    0x93, 0x08, 0x10, 0x00,
    // ecall
    0x73, 0x00, 0x00, 0x00,
    // wfi
    0x73, 0x00, 0x50, 0x10,
];

/// `user_task_b`: yield, then fault (load from address 0).
///
///   li  a7, 0    (SyscallNumber::Yield)
///   ecall
///   ld  a0, 0(zero)   — load page fault at address 0
///   wfi               (safety net)
pub static USER_TASK_B: &[u8] = &[
    // li a7, 0
    0x93, 0x08, 0x00, 0x00,
    // ecall
    0x73, 0x00, 0x00, 0x00,
    // ld a0, 0(zero)   0x0000_3503
    0x03, 0x35, 0x00, 0x00,
    // wfi
    0x73, 0x00, 0x50, 0x10,
];
