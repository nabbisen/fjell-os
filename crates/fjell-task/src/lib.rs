//! The task control block, the task table, the ready queue and the milestone queries, extracted from the kernel (RFC-0.50-001).
//!
//! These were `crates/fjell-kernel/src/task/tcb.rs`, the `ReadyQueue` half of `task/scheduler.rs`, and two queries in
//! `trap/dispatch.rs`. They moved because a test in a bare-metal crate is never compiled (E-134): eleven of the 27 tests the
//! kernel carried (the 8 milestone queries and the 3 ready-queue tests) were over this logic, **and the code below is the
//! slot-reuse mechanism RFC-0.49-001 built** (`TaskTable::next_free`, `id_at`, the generation bump in `remove`), which had no
//! test anywhere. The kernel re-exports the modules (`task::tcb`, `task::scheduler`), so its own paths are unchanged.
//!
//! **What stays in the kernel:** `Scheduler` (its `set_current` tells the audit ring who has the CPU), the exit **latch** (a
//! process-global static, which in a library shared by parallel host tests would leak one test's state into another -- so the
//! queries here are the table *scan* only, and the kernel's wrapper adds the latch), `check_smoke_pass`, and everything that
//! touches a CSR.
#![no_std]

pub mod milestone;
pub mod ready_queue;
pub mod tcb;

/// Unique identifier for an address space. (Moved with the TCB, which carries one; `mm::vspace` re-exports it.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressSpaceId(pub u16);

/// Base of the per-task device VMA window (RFC 051): `0x7000_0000..0x8000_0000`.
///
/// **Replicated, and tied to the original at compile time** (RFC-0.50-001 D2): the kernel asserts
/// `DEVICE_VMA_BASE == platform::qemu_virt::DEVICE_VMA_BASE`, so the kernel build fails if they ever disagree.
pub const DEVICE_VMA_BASE: usize = 0x7000_0000;
