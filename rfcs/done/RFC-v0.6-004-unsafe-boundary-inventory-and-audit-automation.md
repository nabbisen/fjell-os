# RFC-v0.6-004: Unsafe Boundary Inventory and Audit Automation

**Status.** Implemented (v0.6.0)

## Status

Draft (revised, supersedes pack v0.6-004 draft)

## Target Version

`v0.6.0`.

## Phase

Verification, Fuzzing, and Property Testing — Epic D (Unsafe Audit).

## Related Work

- v0.2 RFC 037 — DMA region safety (the principal source of `unsafe`).
- v0.5 RFC 003 — arch boundary cleanup (largest legitimate `unsafe`
  surface).
- v0.6 RFCs 001/002/003 — companion verification work.

---

## 1. Summary

Inventory every `unsafe` block in the workspace, classify by purpose,
attach a justifying ADR or RFC reference to each, and add a CI gate that
rejects new `unsafe` without a matching tag in the commit body.

Output: a machine-checked **Unsafe Inventory** document and an
`UNSAFE_CHARTER.md` that defines the policy for adding new `unsafe`.

---

## 2. Motivation

`unsafe` is the right tool in a kernel, but it is *also* where defects
hide. Today Fjell has dozens of `unsafe` blocks; the rationale for each is
in code comments at best. Centralising the inventory:

- helps new contributors understand the safety story;
- makes regressions visible (a sudden new `unsafe` shows up in CI);
- supports audit work for v0.7+ fleet operations and v1.0 cert claims.

---

## 3. Goals

```text
- Inventory file under docs/src/verification/unsafe-inventory.md, auto-
  generated, listing every `unsafe` block by (file:line:length) with a
  classification tag.
- A small set of classification tags (AsmTrampoline, MmioAccess,
  DmaAccess, AtomicPtr, FFI, RawCast, TraitImplSafetyDoc).
- CI gate: `cargo run -p fjell-unsafe-audit` produces the same inventory
  bit-for-bit as the checked-in file; any drift fails CI.
- An UNSAFE_CHARTER.md defining when `unsafe` is acceptable, what comments
  it must carry, and how to retire blocks.
```

## 4. Non-Goals

```text
- No removal of legitimate `unsafe` from arch / drivers.
- No formal-verification of `unsafe` blocks. The goal is *visibility*,
  not proof.
- No machine-readable safety contract beyond the classification tag.
```

---

## 5. External Design

### 5.1 Tool: `fjell-unsafe-audit`

A host-only binary in `tools/fjell-unsafe-audit/`. Scans the workspace,
emits Markdown:

```text
# Unsafe Inventory

(auto-generated; do not edit. Regenerate with `fjell-unsafe-audit`.)

## crates/fjell-arch-riscv64/src/trap.rs

| Lines  | Tag             | Justification                                  |
|--------|-----------------|------------------------------------------------|
| 42-58  | AsmTrampoline   | RISC-V trap vector entry (see v0.5 RFC 003)    |
| 92-95  | AtomicPtr       | Hart-local pointer install                     |

## crates/fjell-cap/src/table.rs

| Lines  | Tag           | Justification                                |
|--------|---------------|----------------------------------------------|
| 187-191| RawCast       | Index-to-slot pointer; bounds checked above  |

...
```

### 5.2 Classification tags

```text
AsmTrampoline      — inline asm or hand-written assembly entry/exit.
MmioAccess         — read/write to MMIO; requires MmioRegion cap upstream.
DmaAccess          — read/write to DMA-shared memory; quarantine semantics
                     in v0.2 RFC 037.
AtomicPtr          — load/store of a pointer that must be atomic; replaces
                     a logical safe abstraction we don't have yet.
FFI                — external symbol or libc call.
RawCast            — pointer arithmetic or transmute justified by
                     surrounding bounds check.
TraitImplSafetyDoc — implementing an unsafe trait (Send/Sync) where the
                     contract is documented in the type's doc comment.
```

Any `unsafe` block must have a `// SAFETY:` comment whose first word is
one of the tags. The audit tool reads the tag from the comment.

### 5.3 UNSAFE_CHARTER.md

```text
1. Every `unsafe` block requires a `// SAFETY: <Tag>` line.
2. Every `unsafe` impl Send/Sync requires the same line on the impl.
3. Adding a new `unsafe` block requires:
     - the SAFETY line,
     - an ADR or RFC reference if the block introduces a new category,
     - an inventory regeneration in the same PR.
4. Replacing an unsafe block with a safe equivalent is encouraged; the
   PR removing it must regenerate the inventory.
```

---

## 6. Data Model

### 6.1 Audit AST walk

The tool uses `syn` to parse each `.rs` file and find:

- `unsafe { ... }` blocks;
- `unsafe fn`;
- `unsafe impl`;
- `unsafe trait` definitions.

For each, it locates the preceding `// SAFETY:` comment (within 4 lines
above the unsafe token). If missing → fail.

### 6.2 Inventory record

```rust
pub struct UnsafeRecord {
    pub file:    String,
    pub start:   u32,
    pub end:     u32,
    pub kind:    UnsafeKind,    // Block | Fn | Impl | Trait
    pub tag:     SafetyTag,
    pub justification: String,
}
```

---

## 7. Internal Design

### 7.1 Audit flow

```text
1. enumerate crates via cargo metadata.
2. for each Rust file: parse with syn, walk for unsafe nodes.
3. for each node: collect SAFETY comment.
4. if comment missing → record as Violation; fail.
5. build records list, sorted by (file, line).
6. render Markdown.
7. compare to checked-in docs/src/verification/unsafe-inventory.md;
   exit non-zero on diff.
```

### 7.2 Doc-comment scrape

The justification text is the rest of the SAFETY comment (after the tag).
Inventory shows the first 80 chars; full text remains in the source.

### 7.3 CI integration

A new CI job `unsafe-audit` runs:

```bash
cargo run -p fjell-unsafe-audit -- --check
```

Failure types:

- `MissingSafetyComment`;
- `UnknownTag(...)`;
- `InventoryDrift` (file content differs from generator output).

---

## 8. Security Design

### 8.1 What this RFC delivers

```text
- Visible surface of unsafe blocks for any auditor.
- Reproducible "all unsafe is intentional" claim for releases.
- Enforces the rule that unsafe never sneaks in without review.
```

### 8.2 Audit emission

None at runtime.

---

## 9. Memory / Resource Design

Host tool; no runtime cost.

---

## 10. Compatibility and Migration

- Initial inventory checked in alongside the audit tool.
- All existing unsafe blocks classified retroactively in a single
  preparation PR.
- Subsequent PRs maintain the inventory.

---

## 11. Test Strategy

### 11.1 Audit tool tests

```text
- audit_finds_unsafe_blocks
- audit_finds_unsafe_fn
- audit_finds_unsafe_impl_send_sync
- audit_rejects_missing_safety_comment
- audit_rejects_unknown_tag
- audit_idempotent_render
- audit_reports_inventory_drift
```

### 11.2 Workspace acceptance

```text
- Every existing unsafe has a SAFETY comment.
- Inventory regenerates byte-identical to the checked-in version.
- Adding an unsafe block in a test branch without a SAFETY comment fails
  CI.
```

---

## 12. Acceptance Criteria

```text
- tools/fjell-unsafe-audit ships.
- docs/src/verification/unsafe-inventory.md checked in.
- UNSAFE_CHARTER.md checked in at repo root.
- CI job `unsafe-audit` enforced on every PR.
- Every existing unsafe block has a SAFETY comment.
- ADR-v0.6-004 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/verification/v0.6-004-unsafe-audit.md
docs/src/verification/unsafe-inventory.md      — auto-generated
UNSAFE_CHARTER.md                              — repo root
docs/src/adr/v0.6-004-unsafe-charter.md
```

---

## 14. Open Questions

1. **More-than-comment contract** — should SAFETY clauses be machine-
   checked beyond the tag? A future RFC may add structured invariants
   (`// SAFETY: AsmTrampoline; requires: trap-frame layout matches arch
   v0.5 RFC 003`).
2. **Hot rebase** — the inventory is order-stable but file rewrites may
   shuffle line numbers and produce noisy diffs. Tracked; mitigation is
   to regenerate after rebase.

---

## 15. Release Gate (RFC-local)

```text
- Audit tool ships.
- Inventory complete.
- CI enforces.
- UNSAFE_CHARTER published.
- ADR Accepted.
```
