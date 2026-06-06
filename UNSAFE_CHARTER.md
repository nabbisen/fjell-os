# Fjell OS Unsafe Boundary Charter

**Version:** v0.6.0  
**ADR:** ADR-v0.6-004

## Policy

Every use of `unsafe` in Fjell OS must be justified with a `// SAFETY:` comment
immediately above the unsafe site.  The comment must state:

1. What invariant the surrounding code upholds that makes the unsafe valid.
2. Who is responsible for maintaining that invariant (caller, kernel, capability system).

## Enforcement

The CI job `unsafe-audit` runs `fjell-unsafe-audit --check --root crates/` on every PR.
A PR that introduces an unsafe site without a SAFETY comment is blocked.

## Scope

The tool scans `crates/` for:
- `unsafe { ... }` blocks
- `unsafe fn` declarations
- `unsafe impl` declarations
- `unsafe trait` declarations

## Permitted unsafe categories

| Category | Location | Justification summary |
|----------|----------|-----------------------|
| CSR access | `fjell-arch-riscv64`, `fjell-kernel/arch/` | RISC-V hardware requires CSR instructions; gated on `target_arch = "riscv64"` |
| Page-table walks | `fjell-kernel/mm/page_table.rs` | Physical addresses are validated by the frame allocator before use |
| User-space copy | `fjell-kernel/mm/user_copy.rs` | Addresses are checked against the task VMA map before the copy |
| IPC buffer access | Service `main.rs` files | Buffers are capability-gated; sizes bounded by `MAX_IPC_MSG` |
| DMA ring | `fjell-driver-virtio-net` | DMA regions are pinned and exclusively owned per descriptor slot |
| Negative tests | `fjell-neg-test` | Intentional faults for CI negative-test coverage |

## Prohibited patterns

- Raw pointer arithmetic on user-supplied addresses without bounds checking.
- `unsafe impl Send/Sync` on types that are genuinely not thread-safe.
- Unchecked index operations on ring buffers.
- Transmutes between types with different invariants without explicit comment.

## Audit baseline (v0.6.0)

- 261 unsafe sites total
- 261 with SAFETY comment (100%)
- 0 missing

Run `cargo run -p fjell-unsafe-audit -- --root crates/` for the current inventory.
