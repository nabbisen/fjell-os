#![no_std]
#![doc = include_str!("../README.md")]

/// The stable ABI surface, re-exported from [`fjell_abi`].
///
/// This is the only Fjell crate published so far beyond this one. It carries
/// the syscall numbers, capability kinds and rights, service image identifiers,
/// and boot-control types shared between the kernel and every user-space
/// service.
pub use fjell_abi as abi;
