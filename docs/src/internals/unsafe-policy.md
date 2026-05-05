# `unsafe` Policy

Every `unsafe` block in Fjell OS must carry a structured `// SAFETY:` comment
that answers four questions:

```rust
// SAFETY:
// - Why this unsafe operation is necessary.
// - Which invariant must hold for it to be sound.
// - Who guarantees that invariant.
// - What would break if the invariant were violated.
```

## Permitted uses of `unsafe`

| Category | Location |
|---|---|
| CSR read/write (`csrr`, `csrw`) | `fjell-arch/src/riscv64/csr.rs` |
| `satp` write + `sfence.vma` | `fjell-arch/src/riscv64/satp.rs` |
| `stvec` write | `fjell-kernel/src/trap/entry.rs` |
| Trap-entry assembly (`global_asm!`) | `fjell-kernel/src/boot.rs`, `trap/entry.rs` |
| Context-switch assembly | `fjell-kernel/src/task/context.rs` |
| Page-table MMIO / raw pointer access | `fjell-kernel/src/mm/page_table.rs` |
| Volatile UART MMIO | `fjell-kernel/src/uart.rs` |
| `static mut UART` (M1 temporary) | `fjell-kernel/src/console.rs` |

## Explicitly forbidden

- Dereferencing user-supplied pointers without bounds/capability checking.
- Using `unsafe` to bypass the capability model "for convenience".
- `static mut` outside the narrow early-console bootstrap (replace with a
  spinlock-protected wrapper in M2+).
- Transmuting values across privilege/ABI boundaries without explicit layout
  guarantees (`#[repr(C)]`).

## Review checklist

Before merging a PR that introduces or modifies `unsafe`:

- [ ] Every `unsafe` block has a `// SAFETY:` comment answering all four questions.
- [ ] The minimal surface is used (e.g. prefer `read_volatile` over a raw
      pointer cast when only a read is needed).
- [ ] The owning module exports no `unsafe` API to callers outside its
      permission boundary.
- [ ] `miri` or Clippy `#[deny(unsafe_op_in_unsafe_fn)]` checks are satisfied
      for any host-testable logic.
