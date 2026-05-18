//! virtio-blk driver skeleton — M6 smoke test.
//!
//! The actual virtio initialization and I/O is handled inline by fjell-init
//! in M6 (since we don't have synchronous service-to-service IPC yet).
//! This binary exists to satisfy the ImageId and shows "driver-virtio-blk
//! started" via fjell-init's spawn label, then exits.
#![no_std]
#![no_main]
mod rt;
use fjell_syscall::sys_exit;
#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! { sys_exit(0) }
