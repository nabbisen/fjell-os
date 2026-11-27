//! Safe user-pointer validation (RFC 039 §2.1) -- **moved to `fjell-user-ptr`** (RFC-0.50-001), with its nine tests, so that they run.
//!
//! `copy_to_user` and `copy_from_user` are the highest-risk primitives in the kernel. Everything here is a re-export, so every
//! use site (`user_copy.rs`, `trap/syscall.rs`) is unchanged.

pub use fjell_user_ptr::*;

// The crate owns a copy of the bound (it has no `platform` to read); this ties the copy to the original, so the kernel BUILD fails if
// they ever disagree (RFC-0.50-001 D2). A replicated constant with nothing tying it is the E-125 shape.
const _: () = assert!(
    fjell_user_ptr::USER_ADDR_MAX == crate::platform::qemu_virt::RAM_BASE,
    "fjell-user-ptr's USER_ADDR_MAX must equal the kernel's RAM_BASE"
);
