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
// SAFETY: category=symbol-export `service_main` is this service's entry symbol, tail-called by `_start` in rt.rs; no other item in the image exports the name.
#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_exit(0)
}
