//! Thin user-space wrappers around Fjell OS syscalls.
//!
//! Each function corresponds to one `ecall` instruction.  The calling
//! convention follows the Fjell ABI: syscall number in `a7`, arguments in
//! `a0`–`a5`, status returned in `a0`.

#![no_std]

// Syscall wrappers will be added in M2/M3 alongside the kernel implementations.
