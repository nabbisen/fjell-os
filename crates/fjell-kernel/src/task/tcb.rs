//! Task Control Block types.

use super::id::TaskId; // = fjell_abi::task::TaskId (re-export)
use crate::mm::vspace::AddressSpaceId;

// ── Register indices ──────────────────────────────────────────────────────────

/// `a0` register index in `TrapFrame::gpr`.
pub const REG_A0: usize = 10;
/// `a1` register index.
pub const REG_A1: usize = 11;
pub const REG_A2: usize = 12;
pub const REG_A3: usize = 13;
#[allow(dead_code)]
pub const REG_A4: usize = 14;
#[allow(dead_code)]
pub const REG_A5: usize = 15;
#[allow(dead_code)]
pub const REG_A6: usize = 16;
/// `a7` register index (syscall number).
pub const REG_A7: usize = 17;

// ── TrapFrame ─────────────────────────────────────────────────────────────────

/// Full register save area for trap entry / `sret` return.
///
/// Laid out in memory exactly as the assembly trap-entry stub expects.
/// `gpr[0]` is x0 (always zero, not saved), `gpr[2]` is sp, etc.
#[repr(C)]
pub struct TrapFrame {
    /// General-purpose registers x0–x31.
    pub gpr: [usize; 32],
    pub sstatus: usize,
    pub sepc: usize,
    pub scause: usize,
    pub stval: usize,
}

impl TrapFrame {
    pub const fn zero() -> Self {
        TrapFrame {
            gpr: [0; 32],
            sstatus: 0,
            sepc: 0,
            scause: 0,
            stval: 0,
        }
    }
}

// ── KernelContext ─────────────────────────────────────────────────────────────

/// Callee-saved register state for kernel-to-kernel context switches.
///
/// Only `ra`, `sp`, and `s0`–`s11` need to be saved/restored; caller-saved
/// registers are discarded at the switch boundary.
#[repr(C)]
pub struct KernelContext {
    pub ra: usize,
    pub sp: usize,
    pub s: [usize; 12], // s0..s11
}

impl KernelContext {
    pub const fn zero() -> Self {
        KernelContext {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }
}

// ── Fault information ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultCause {
    InstructionPageFault,
    LoadPageFault,
    StorePageFault,
    IllegalInstruction,
    UnknownSyscall,
    KernelRejectedReturn,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaultInfo {
    pub cause: FaultCause,
    pub sepc: usize,
    pub stval: usize,
}

// ── TaskState ─────────────────────────────────────────────────────────────────

/// Lifecycle state of a task.
///
/// Terminal states: `Faulted` and `Exited` — the scheduler never re-enqueues
/// a task in either state (invariant TASK-005).
#[derive(Debug, PartialEq)]
pub enum TaskState {
    Empty,
    Created,
    Runnable,
    Running,
    Blocked(BlockReason),
    Faulted(FaultInfo),
    Exited(i32),
}

/// Reason a task is blocked.
#[derive(Debug, PartialEq)]
pub enum BlockReason {
    Yield,
    Sleep,
    /// Placeholder for M3 IPC blocking.
    ReservedForIpc,
    /// Blocked in `sys_irq_wait` on the given PLIC source (RFC-0.25-001).
    Irq(u32),
    /// Blocked in `sys_audit_wait` until the audit ring is non-empty
    /// (RFC-0.41-001). No payload: there is one ring, so nothing to discriminate.
    AuditRing,
}

// ── TaskAccounting ────────────────────────────────────────────────────────────

/// Maximum consecutive timer preemptions before `TaskQuantumExceeded` is
/// emitted in the audit ring (RFC 037).
///
/// A task that calls `sys_yield` resets its counter.  A task that consumes
/// `QUANTUM_VIOLATION_THRESHOLD` consecutive timer quanta without yielding
/// is considered "spinning" and is audited.
pub const QUANTUM_VIOLATION_THRESHOLD: u32 = 3;

#[derive(Default, Clone, Copy, Debug)]
pub struct TaskAccounting {
    pub run_count: u64,
    pub total_ticks: u64,
    pub last_scheduled_tick: u64,
    /// Consecutive timer preemptions without a voluntary `sys_yield` (RFC 037).
    ///
    /// Incremented on every timer preemption; reset to 0 on voluntary yield or
    /// block.  Emits `TaskQuantumExceeded` audit event when ≥
    /// `QUANTUM_VIOLATION_THRESHOLD`.
    pub quantum_violations: u32,
}

// ── Task ─────────────────────────────────────────────────────────────────────

/// Task Control Block.
pub struct Task {
    pub id: TaskId,
    pub priority: u8,
    pub state: TaskState,
    pub address_space: AddressSpaceId,
    /// Sv39 satp PFN for this task's root page table.
    pub satp_root_pfn: usize,
    pub kernel_context: KernelContext,
    pub trap_frame: TrapFrame,
    pub kernel_stack_top: usize,
    pub user_stack_top: usize,
    pub accounting: TaskAccounting,
    /// RFC 051: device VMA bump allocator.
    pub dev_vma_next: usize,
    /// RFC 055: kernel-attested image id (set by spawn.rs; used in IPC identity).
    pub image_id: fjell_abi::service::ImageId,
    /// RFC-0.48-001 R1/R4: the lease every capability this task was granted at boot is bound to. Created by `spawn()` on
    /// the **spawner's** behalf (so the spawner may revoke it, with no exception right), and **revoked when this task
    /// exits or faults** -- without that, a lease owned by the spawner would outlive its subject and every service restart
    /// would consume a slot for ever. `None` only for `init`, which is built directly in `main.rs` and is excepted by name.
    pub boot_lease: Option<fjell_abi::lease::LeaseId>,
    /// RFC-0.49-001 D4: frames `spawn()` took for this task (user pages, kernel stack, **page tables**). Destroy frees all
    /// but the page tables, and prints the difference, so the deferred leak is a number in the log and not a claim.
    pub frames_taken: u32,
}

impl Task {
    pub fn new(
        id: TaskId,
        priority: u8,
        address_space: AddressSpaceId,
        kernel_stack_top: usize,
        user_stack_top: usize,
    ) -> Self {
        use crate::platform::qemu_virt::DEVICE_VMA_BASE;
        Task {
            id,
            priority,
            state: TaskState::Created,
            address_space,
            satp_root_pfn: 0,
            kernel_context: KernelContext::zero(),
            trap_frame: TrapFrame::zero(),
            kernel_stack_top,
            user_stack_top,
            accounting: TaskAccounting::default(),
            dev_vma_next: DEVICE_VMA_BASE,
            image_id: fjell_abi::service::ImageId(0xFFFF), // set by spawn
            boot_lease: None,                              // set by spawn
            frames_taken: 0,                               // set by spawn
        }
    }
}

// ── TaskTable ─────────────────────────────────────────────────────────────────

/// Maximum number of concurrent tasks (including idle).
pub const MAX_TASKS: usize = 40;

struct TaskSlot {
    generation: u16,
    task: Option<Task>,
}

/// Fixed-capacity task table.
///
/// No heap allocation; slots are reused with generation bumping.
pub struct TaskTable {
    slots: [TaskSlot; MAX_TASKS],
}

impl TaskTable {
    pub fn new() -> Self {
        // Build the array without requiring Copy/Clone on Task.
        let slots = core::array::from_fn(|_| TaskSlot {
            generation: 0,
            task: None,
        });
        TaskTable { slots }
    }

    /// Insert a task and return its `TaskId`.
    ///
    /// **The task's own `id` is set to the slot's real `(index, generation)`** (RFC-0.49-001 R3): a TCB whose `id` disagrees
    /// with the slot it sits in names a task that does not exist, which is the failure reuse makes reachable.
    pub fn insert(&mut self, mut task: Task) -> Result<TaskId, TaskError> {
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.task.is_none() {
                let id = TaskId::new(i as u16, slot.generation);
                task.id = id;
                slot.task = Some(task);
                return Ok(id);
            }
        }
        Err(TaskError::TableFull)
    }

    /// The real id of the task occupying slot `index` -- the slot's *current* generation -- or `None` if it is empty.
    ///
    /// **This is the replacement for `TaskId::new(index, 0)`** (RFC-0.49-001 D3). A syscall handler knows its caller as an
    /// index; an owner comparison needs the whole id, so that a new occupant of the slot cannot equal an owner stored for its
    /// predecessor.
    pub fn id_at(&self, index: u16) -> Option<TaskId> {
        let slot = self.slots.get(index as usize)?;
        slot.task
            .as_ref()
            .map(|_| TaskId::new(index, slot.generation))
    }

    /// The task in slot `index`, whatever its generation. **For references the kernel itself owns and purges when the
    /// task is destroyed** (endpoint queue entries, reply edges, ...): those carry an index, and destroying a slot removes
    /// every one of them, so an index in a live queue always names the current occupant. Anything that came from outside
    /// the kernel carries a full `TaskId` and goes through [`TaskTable::get`].
    pub fn get_by_index(&self, index: u16) -> Option<&Task> {
        self.slots.get(index as usize)?.task.as_ref()
    }

    /// Mutable form of [`TaskTable::get_by_index`].
    pub fn get_by_index_mut(&mut self, index: u16) -> Option<&mut Task> {
        self.slots.get_mut(index as usize)?.task.as_mut()
    }

    /// Get an immutable reference to a task by `TaskId`.
    pub fn get(&self, id: TaskId) -> Option<&Task> {
        let slot = self.slots.get(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        slot.task.as_ref()
    }

    /// Get a mutable reference to a task by `TaskId`.
    pub fn get_mut(&mut self, id: TaskId) -> Option<&mut Task> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        slot.task.as_mut()
    }

    /// Remove a task, bumping the slot generation to invalidate stale handles.
    pub fn remove(&mut self, id: TaskId) -> Option<Task> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        let task = slot.task.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        Some(task)
    }

    /// Iterate over every occupied slot (RFC-v0.23-002: milestone markers
    /// scan by `image_id` rather than trusting a fixed table index).
    pub fn iter(&self) -> impl Iterator<Item = &Task> {
        self.slots.iter().filter_map(|s| s.task.as_ref())
    }
}

/// Errors from `TaskTable` operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskError {
    TableFull,
    InvalidTaskId,
    GenerationMismatch,
    InvalidState,
    NoRunnableTask,
}

impl TaskTable {
    /// The `(index, generation)` the next [`TaskTable::insert`] will assign -- the first empty slot and **its current
    /// generation**, which `remove` has already bumped if the slot was used before. A spawner must build the TCB's id, the
    /// address-space id and every frame-owner tag from this, not from a literal generation (RFC-0.49-001 R3).
    pub fn next_free(&self) -> Option<(u16, u16)> {
        self.slots
            .iter()
            .enumerate()
            .find(|(_, s)| s.task.is_none())
            .map(|(i, s)| (i as u16, s.generation))
    }
}
