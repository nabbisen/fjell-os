//! RISC-V trap cause decoding.
//!
//! `scause` bit 63 is the interrupt bit; bits [62:0] are the cause code.

/// Decoded trap kind passed to the Rust dispatch function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapKind {
    /// `ecall` from U-mode (cause = 8).
    UserEcall,
    /// Supervisor timer interrupt (interrupt bit set, cause = 5).
    SupervisorTimer,
    /// Instruction page fault (cause = 12).
    InstructionPageFault,
    /// Load page fault (cause = 13).
    LoadPageFault,
    /// Store/AMO page fault (cause = 15).
    StorePageFault,
    /// Illegal instruction (cause = 2).
    IllegalInstruction,
    /// Any other cause — logged and ignored in user mode; panic in kernel mode.
    Other(usize),
}

/// Decode raw `scause` value into a `TrapKind`.
pub fn decode_trap(scause: usize) -> TrapKind {
    let interrupt = scause >> 63;
    let code = scause & !(1 << 63);

    if interrupt == 1 {
        match code {
            5 => TrapKind::SupervisorTimer,
            _ => TrapKind::Other(scause),
        }
    } else {
        match code {
            2 => TrapKind::IllegalInstruction,
            8 => TrapKind::UserEcall,
            12 => TrapKind::InstructionPageFault,
            13 => TrapKind::LoadPageFault,
            15 => TrapKind::StorePageFault,
            _ => TrapKind::Other(scause),
        }
    }
}
