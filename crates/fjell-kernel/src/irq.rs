//! IRQ bind table — RFC-0.25-001 R4.
//!
//! Maps a PLIC source number to the task bound to it via `sys_irq_bind`, and
//! tracks a one-slot pending flag per source so an interrupt that arrives
//! between a task deciding to block and actually blocking is not lost (D3 /
//! handoff §0.1): the interrupt handler marks the source pending if the
//! bound task is not (yet) `Blocked` on it; `sys_irq_wait` checks the
//! pending flag before blocking, exactly as `sys_ipc_try_recv` checks an
//! endpoint's queue before a real `ipc_recv` blocks.

use fjell_abi::task::TaskId;

/// Highest PLIC source number this table tracks. QEMU `virt` wires UART0 at
/// IRQ 10; generous headroom for future sources without resizing.
pub const MAX_IRQ: usize = 64;

pub struct IrqTable {
    bound: [Option<TaskId>; MAX_IRQ],
    pending: [bool; MAX_IRQ],
}

impl IrqTable {
    pub const fn new() -> Self {
        IrqTable {
            bound: [None; MAX_IRQ],
            pending: [false; MAX_IRQ],
        }
    }

    /// Record `task` as the owner of `irq`. Clears any stale pending flag
    /// from a previous owner.
    pub fn bind(&mut self, irq: u32, task: TaskId) -> bool {
        let idx = irq as usize;
        if idx == 0 || idx >= MAX_IRQ {
            return false;
        }
        self.bound[idx] = Some(task);
        self.pending[idx] = false;
        true
    }

    /// Release every interrupt line `who` owns, clearing its pending flag (RFC-0.49-001 D4). There is no `sys_irq_unbind`, so
    /// until a task is destroyed a dead driver owns its line for ever; after a reuse, the line would be delivered to
    /// whatever task next occupies that slot. Returns how many lines were released.
    pub fn release_task(&mut self, who: TaskId) -> usize {
        let mut n = 0;
        for i in 0..MAX_IRQ {
            if self.bound[i] == Some(who) {
                self.bound[i] = None;
                self.pending[i] = false;
                n += 1;
            }
        }
        n
    }

    /// The task currently bound to `irq`, if any.
    pub fn bound_task(&self, irq: u32) -> Option<TaskId> {
        self.bound.get(irq as usize).copied().flatten()
    }

    /// Mark `irq` pending (bound task exists but was not blocked waiting
    /// when the interrupt fired).
    pub fn set_pending(&mut self, irq: u32) {
        if let Some(p) = self.pending.get_mut(irq as usize) {
            *p = true;
        }
    }

    /// Take and clear the pending flag for `irq`.
    pub fn take_pending(&mut self, irq: u32) -> bool {
        match self.pending.get_mut(irq as usize) {
            Some(p) => core::mem::take(p),
            None => false,
        }
    }
}
