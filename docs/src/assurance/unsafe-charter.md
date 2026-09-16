# Unsafe Boundary Charter

**ADR:** ADR-v0.6-004, RFC 060
**Gate:** `cargo xtask test-all --no-qemu` tier 2 (unsafe audit)

---

## Policy

Every use of `unsafe` in Fjell OS must be justified with a `// SAFETY:`
comment immediately above the unsafe site. The comment must state:

1. What invariant the surrounding code upholds that makes the unsafe valid.
2. Who is responsible for maintaining that invariant (caller, kernel,
   capability system, or hardware contract).

A `// SAFETY: category=<tag>` prefix categorises the site for the audit
tool. See the permitted categories below.

## Enforcement

The CI job `ci-unsafe-audit` (and `cargo xtask test-all` tier 2) runs:

```sh
cargo run -p fjell-unsafe-audit -- --workspace . --check
```

A PR that introduces an unsafe site without a `SAFETY:` comment is blocked.
The gate passes only when the missing-annotation count is zero.

## Permitted categories

| Category | Location | Justification |
|----------|----------|---------------|
| `kernel-global-mutable` | `fjell-kernel` | Single-hart kernel; no concurrent mutation |
| `csr-asm` | `fjell-kernel/arch/` | RISC-V hardware requires CSR instructions; gated on `target_arch = "riscv64"` |
| `mmio-access` | `fjell-kernel/` | MMIO writes annotated with `MMIO-ORDER:`; single hart |
| `dma-memory` | driver crates | DMA regions pinned and exclusively owned per descriptor slot |
| `page-table` | `fjell-kernel/mm/` | Physical addresses validated by the frame allocator before use |
| `user-copy` | `fjell-kernel/mm/` | Addresses checked against task VMA map before copy |
| `ipc-buffer` | service `main.rs` | Buffers are capability-gated; sizes bounded by ABI constants |
| `test-intentional-fault` | `fjell-neg-test` | Intentional invalid operations for negative-test coverage |

## Prohibited patterns

- Raw pointer arithmetic on user-supplied addresses without bounds checking.
- `unsafe impl Send/Sync` on types that are genuinely not thread-safe.
- Unchecked index operations on ring buffers.
- Transmutes between types with different invariants without explicit comment.
- Silent discarding of `SAFETY:` obligation by delegating to an inner `unsafe`
  block without re-stating the invariant at the outer site.

## Current state

Run the audit to get the live count:

```sh
cargo run -p fjell-unsafe-audit -- --workspace . --check
```

The gate passes with 0 missing annotations. The site count grows with
the codebase; the zero-missing invariant is the enforced constraint,
not the total count.

*Audit baseline at v0.20.2: 0 missing SAFETY comments.*
