//! Task Control Block types.

use super::id::TaskId;  // = fjell_abi::task::TaskId (re-export)
use crate::mm::vspace::AddressSpaceId;

// ── Register indices ──────────────────────────────────────────────────────────

/// `a0` register index in `TrapFrame::gpr`.
pub const REG_A0: usize = 10;
/// `a1` register index.
pub const REG_A1: usize = 11;
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
    pub gpr:     [usize; 32],
    pub sstatus: usize,
    pub sepc:    usize,
    pub scause:  usize,
    pub stval:   usize,
}

impl TrapFrame {
    pub const fn zero() -> Self {
        TrapFrame {
            gpr:     [0; 32],
            sstatus: 0,
            sepc:    0,
            scause:  0,
            stval:   0,
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
    pub s:  [usize; 12], // s0..s11
}

impl KernelContext {
    pub const fn zero() -> Self {
        KernelContext { ra: 0, sp: 0, s: [0; 12] }
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
    pub sepc:  usize,
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
}

// ── TaskAccounting ────────────────────────────────────────────────────────────

#[derive(Default, Clone, Copy, Debug)]
pub struct TaskAccounting {
    pub run_count:           u64,
    pub total_ticks:         u64,
    pub last_scheduled_tick: u64,
}

// ── Task ─────────────────────────────────────────────────────────────────────

/// Task Control Block.
pub struct Task {
    pub id:              TaskId,
    pub priority:        u8,
    pub state:           TaskState,
    pub address_space:   AddressSpaceId,
    /// Sv39 satp PFN for this task's root page table.
    /// Written to `satp` on every context switch so the CPU uses the correct
    /// virtual address space.  0 means "use kernel root" (idle task).
    pub satp_root_pfn:   usize,
    pub kernel_context:  KernelContext,
    pub trap_frame:      TrapFrame,
    pub kernel_stack_top: usize,
    pub user_stack_top:  usize,
    pub accounting:      TaskAccounting,
}

impl Task {
    pub fn new(
        id: TaskId,
        priority: u8,
        address_space: AddressSpaceId,
        kernel_stack_top: usize,
        user_stack_top: usize,
    ) -> Self {
        Task {
            id,
            priority,
            state: TaskState::Created,
            address_space,
            satp_root_pfn: 0,   // caller must set this after creating the page table
            kernel_context: KernelContext::zero(),
            trap_frame: TrapFrame::zero(),
            kernel_stack_top,
            user_stack_top,
            accounting: TaskAccounting::default(),
        }
    }
}

// ── TaskTable ─────────────────────────────────────────────────────────────────

/// Maximum number of concurrent tasks (including idle).
pub const MAX_TASKS: usize = 16;

struct TaskSlot {
    generation: u16,
    task:       Option<Task>,
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
        let slots = core::array::from_fn(|_| TaskSlot { generation: 0, task: None });
        TaskTable { slots }
    }

    /// Insert a task and return its `TaskId`.
    pub fn insert(&mut self, task: Task) -> Result<TaskId, TaskError> {
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.task.is_none() {
                let id = TaskId::new(i as u16, slot.generation);
                slot.task = Some(task);
                return Ok(id);
            }
        }
        Err(TaskError::TableFull)
    }

    /// Get an immutable reference to a task by `TaskId`.
    pub fn get(&self, id: TaskId) -> Option<&Task> {
        let slot = self.slots.get(id.index as usize)?;
        if slot.generation != id.generation { return None; }
        slot.task.as_ref()
    }

    /// Get a mutable reference to a task by `TaskId`.
    pub fn get_mut(&mut self, id: TaskId) -> Option<&mut Task> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation { return None; }
        slot.task.as_mut()
    }

    /// Remove a task, bumping the slot generation to invalidate stale handles.
    pub fn remove(&mut self, id: TaskId) -> Option<Task> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation { return None; }
        let task = slot.task.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        Some(task)
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
    /// Return the index of the next free slot (for pre-allocation checks).
    pub fn next_free_index(&self) -> Option<u16> {
        self.slots.iter().enumerate()
            .find(|(_, s)| s.task.is_none())
            .map(|(i, _)| i as u16)
    }
}
