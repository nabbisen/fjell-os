//! Embedded static user task images for M2/M3 smoke testing.
//!
//! All tasks use RISC-V RV64I machine code.  No ELF loader in M3.
//!
//! # M3 scenario
//!   user_task_a (client): ipc_call → exit(0)
//!   user_task_b (server): ipc_recv → ipc_reply → exit(0)
//!
//! The shared endpoint capability is installed by the kernel (slot 0).
//! CapHandle(slot=0, gen=0) encodes as u32 = 0, so a0 = 0 is the handle.

#![allow(dead_code)]

/// Entry point virtual address for all embedded user tasks.
pub const USER_ENTRY_VA:  usize = 0x0001_0000;
/// User stack top virtual address.
pub const USER_STACK_TOP: usize = 0x7FFF_F000;
/// Virtual address where user text is loaded.
pub const USER_TEXT_VA:   usize = USER_ENTRY_VA;

// ── RISC-V instruction encoding helpers ──────────────────────────────────────
//
// addi rd, rs, imm  (I-type):  imm[11:0] | rs | 000 | rd | 0010011
// ecall             (I-type):  000000000000 | 00000 | 000 | 00000 | 1110011
// wfi               (I-type):  0001000 00101 00000 000 00000 1110011
//
// Syscall numbers used:
//   0  = Yield
//   1  = Exit
//   22 = IpcCall
//   21 = IpcRecv
//   23 = IpcReply
//
// Encoding: li a7, N  = addi a7, x0, N
//   addi: imm[11:0] | rs1[4:0] | funct3[2:0] | rd[4:0] | opcode[6:0]
//   rd=a7=x17=10001b, rs1=x0, funct3=000, opcode=0010011
//
// li a0, N  = addi a0, x0, N   (a0=x10=01010b)
// li a1, N  = addi a1, x0, N   (a1=x11=01011b)
//
// Encoding (little-endian u32):
//   addi a7, x0, N  =  (N << 20) | (0 << 15) | (0 << 12) | (17 << 7) | 0x13
//                    =  (N << 20) | 0x00000893
//   addi a0, x0, N  =  (N << 20) | 0x00000513
//   addi a1, x0, N  =  (N << 20) | 0x00000593

fn li_a7(n: u32) -> [u8; 4] { ((n << 20) | 0x00000893).to_le_bytes() }
fn li_a0(n: u32) -> [u8; 4] { ((n << 20) | 0x00000513).to_le_bytes() }
fn li_a1(n: u32) -> [u8; 4] { ((n << 20) | 0x00000593).to_le_bytes() }
const ECALL: [u8; 4] = [0x73, 0x00, 0x00, 0x00];
const WFI:   [u8; 4] = [0x73, 0x00, 0x50, 0x10];

// ── user_task_a: client ───────────────────────────────────────────────────────
//
//   li a0, 0        # ep_handle = CapHandle(0) (slot 0, gen 0)
//   li a1, 1        # tag: label=1
//   li a7, 22       # IpcCall
//   ecall
//   li a0, 0        # exit code = 0
//   li a7, 1        # Exit
//   ecall
//   wfi
pub static USER_TASK_A: &[u8] = &[
        // li a0, 0
        0x13, 0x05, 0x00, 0x00,
        // li a1, 1 (tag: label=1, words=0, caps=0)
        0x93, 0x05, 0x10, 0x00,
        // li a7, 22  (IpcCall) — 22<<20 = 0x1600000; |0x893 = 0x01600893
        0x93, 0x08, 0x60, 0x01,
        // ecall
        0x73, 0x00, 0x00, 0x00,
        // li a0, 0
        0x13, 0x05, 0x00, 0x00,
        // li a7, 1  (Exit)
        0x93, 0x08, 0x10, 0x00,
        // ecall
        0x73, 0x00, 0x00, 0x00,
        // wfi
        0x73, 0x00, 0x50, 0x10,
];

// ── user_task_c: denied ───────────────────────────────────────────────────────
//
// Same bytecode as user_task_a, but this task has NO capability installed.
// The ipc_call will fail with SlotEmpty / InvalidCap, then the task exits(0).
pub static USER_TASK_C: &[u8] = &[
    // li a0, 0  (ep_handle = CapHandle(0) — no cap at slot 0)
    0x13, 0x05, 0x00, 0x00,
    // li a1, 1  (tag)
    0x93, 0x05, 0x10, 0x00,
    // li a7, 22  (IpcCall)
    0x93, 0x08, 0x60, 0x01,
    // ecall  (returns error in a0; task ignores it and exits)
    0x73, 0x00, 0x00, 0x00,
    // li a0, 0  (exit code = 0)
    0x13, 0x05, 0x00, 0x00,
    // li a7, 1  (Exit)
    0x93, 0x08, 0x10, 0x00,
    // ecall
    0x73, 0x00, 0x00, 0x00,
    // wfi
    0x73, 0x00, 0x50, 0x10,
];

// ── user_task_b: server ───────────────────────────────────────────────────────
//
//   li a0, 0        # ep_handle = CapHandle(0)
//   li a7, 21       # IpcRecv
//   ecall           # recv — blocks until client calls
//   li a0, 0        # reply tag
//   li a7, 23       # IpcReply
//   ecall           # reply — wakes client
//   li a0, 0        # exit code
//   li a7, 1        # Exit
//   ecall
//   wfi

pub static USER_TASK_B: &[u8] = &[
    // li a0, 0  (ep_handle = slot 0)
    0x13, 0x05, 0x00, 0x00,
    // li a7, 21  (IpcRecv) — 21<<20 = 0x1500000; |0x893 = 0x01500893
    0x93, 0x08, 0x50, 0x01,
    // ecall
    0x73, 0x00, 0x00, 0x00,
    // li a0, 0  (reply tag)
    0x13, 0x05, 0x00, 0x00,
    // li a7, 23  (IpcReply) — 23<<20 = 0x1700000; |0x893 = 0x01700893
    0x93, 0x08, 0x70, 0x01,
    // ecall
    0x73, 0x00, 0x00, 0x00,
    // li a0, 0  (exit code)
    0x13, 0x05, 0x00, 0x00,
    // li a7, 1  (Exit)
    0x93, 0x08, 0x10, 0x00,
    // ecall
    0x73, 0x00, 0x00, 0x00,
    // wfi
    0x73, 0x00, 0x50, 0x10,
];
