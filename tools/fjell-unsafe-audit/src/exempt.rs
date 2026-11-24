//! The sites whose `category=` the analysis does not support, **exempted by name with a reason** (RFC-0.48-002 R4).
//!
//! A site whose true justification cannot be determined is exempted here, never given a plausible sentence: a guessed comment is
//! false in a new way. **The list's length is the sweep's progress.** Every entry is one of:
//!
//! * **`Seeded { until }`** -- awaiting a sweep. It is *not* a finding settled: the check **refuses it once the workspace version is
//!   past `until`**, so a seeded entry cannot become permanent by being forgotten, and the PASS line counts seeded and permanent
//!   entries separately so the list cannot be read as settled while it is mostly seeded.
//! * **`Permanent`** -- a site that is correctly tagged and that the lexical rule cannot see (it must say why).
//!
//! An entry for a site that is supported, or gone, **fails as stale**: the list is only as long as it is true. An entry with no
//! reason fails. A site with *no comment* cannot be exempted at all.

use crate::audit::{ExemptKind, Exemption};

pub fn exemptions() -> Vec<Exemption> {
    vec![
        Exemption {
            file: "fjell-kernel/src/task/context.rs",
            scope: "context_switch",
            n: 3,
            kind: ExemptKind::Permanent,
            reason: "the host-build stub of `context_switch`: its body is `unimplemented!`, so there is no operation to judge. It is `unsafe fn` only to match the \
                     riscv64 signature, and it never executes.",
        },
        Exemption {
            file: "fjell-kernel/src/trap/entry.rs",
            scope: "supervisor_trap_entry",
            n: 3,
            kind: ExemptKind::Permanent,
            reason: "the host-build stub of the trap entry: its body is `unimplemented!`, so there is no operation to judge; `unsafe` only to match the riscv64 signature.",
        },
        Exemption {
            file: "fjell-syscall/src/lib.rs",
            scope: "sys_audit_drain_ptr",
            n: 1,
            kind: ExemptKind::Permanent,
            reason: "an `unsafe fn` whose body only forwards a caller-supplied address to a *safe* ecall wrapper: the obligation (that the address is valid, or is \
                     deliberately invalid for a negative test) is the caller's, and no operation in the body can be matched to a category.",
        },
        Exemption {
            file: "fjell-syscall/src/lib.rs",
            scope: "sys_audit_drain_raw",
            n: 1,
            kind: ExemptKind::Permanent,
            reason: "as `sys_audit_drain_ptr`: forwards a caller-supplied raw address to the kernel; the check is the kernel's `UserPtr`, the obligation the caller's.",
        },
    ]
}
