//! Task Control Block types and the task table -- **moved to `fjell-task`** (RFC-0.50-001), so that the slot-reuse mechanism
//! RFC-0.49-001 built (and its tests) run on the host. Everything is re-exported: `crate::task::tcb::X` paths are unchanged.

pub use fjell_task::tcb::*;

// The crate owns a copy of the device-VMA base (it has no `platform` to read); this ties the copy to the original at build time.
const _: () = assert!(
    fjell_task::DEVICE_VMA_BASE == crate::platform::qemu_virt::DEVICE_VMA_BASE,
    "fjell-task's DEVICE_VMA_BASE must equal the kernel's"
);
