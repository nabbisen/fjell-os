# RFC-v0.7.4-002: MMIO/DMA Mapping Failure Handling and User-Copy Documentation

## Status

Draft (closes review findings **W-H-01, C-H-05, W-H-07, C-M-08**)

## Target Version

`v0.7.4`

## Summary

Fix the overflow-sensitive size alignment in `sys_mmio_map`; require
every `remap_page` call site to check the result and roll back partial
mappings on failure; align `copy_to_user` documentation with what is
actually validated (page-table only, not a separate VMA map) OR
implement true VMA-map validation.  This RFC closes a class of "looks
correct but isn't" kernel-boundary bugs.

## Motivation

Whole-project review §H-01 and crates review §H-05 documented:

```text
sys_mmio_map page-aligns size with:
  let size_bytes = (tf.gpr[10 + 2] + 0xFFF) & !0xFFF;

This can overflow before masking.

Mapping calls ignore the result:
  let _ = crate::mm::page_table::remap_page(...);

The variable `mapped` is set but not used for success/failure semantics.
```

Whole-project review §H-07 and crates review §M-08:

```text
copy_to_user_bytes performs arithmetic checks and page table checks.
It does not appear to consult a separate VMA map, although comments
say "validated against task VMA map."
```

These are documentation-vs-implementation divergences with safety
consequences: a caller (or auditor) reads the comment and reasons that
VMA validation guards the operation; the implementation actually
trusts whatever the page table says.  When the page table itself has
errors (e.g., from RFC-v0.7.4-001's pre-fix DMA bug), this trust is
misplaced.

## Goals

```text
- sys_mmio_map uses checked_add for size alignment; overflow returns
  InvalidArgument.
- Every remap_page / map_page call site checks the Result.
- Partial mapping failures trigger total rollback.
- copy_to_user documentation either:
    (a) accurately describes page-table validation only, OR
    (b) is backed by real VMA-map validation.
- Tests cover partially-mapped user ranges and page-boundary crossings.
```

## Non-Goals

```text
- No new VMA representation type (option (a) is the default; option
  (b) is a follow-up RFC if the project decides to add VMA tracking).
- No new MMIO discovery mechanism; this RFC only fixes the existing
  paths.
- No change to the syscall ABI.
```

## External Design

### `sys_mmio_map` size handling

```rust
fn sys_mmio_map(tf: &mut TrapFrame) -> SyscallResult {
    let raw_size = tf.gpr[REG_A2] as usize;

    // Checked addition before the mask.
    let size_bytes = raw_size
        .checked_add(PAGE_SIZE - 1)
        .ok_or(SyscallError::InvalidArgument)?
        & !(PAGE_SIZE - 1);

    if size_bytes == 0 || size_bytes > MAX_MMIO_REGION_BYTES {
        return Err(SyscallError::InvalidArgument);
    }
    // ... rest of implementation
}
```

`MAX_MMIO_REGION_BYTES = 64 * 1024 * 1024` (64 MiB) by default.

### `sys_mmio_map` mapping loop

```rust
let mut mapped_pages: heapless::Vec<UserVa, 1024> = heapless::Vec::new();

for offset in (0..size_bytes).step_by(PAGE_SIZE) {
    let pa = phys_base + offset;
    let va = user_base + offset;
    match map_page(address_space, va, pa, PtePerms::DEV_MMIO) {
        Ok(_) => mapped_pages.push(va).expect("vec capacity"),
        Err(e) => {
            // Total rollback.
            for &mapped_va in mapped_pages.iter() {
                let _ = unmap_page(address_space, mapped_va);
            }
            sfence_vma_all();
            audit_emit(AUDIT_MMIO_MAP_PARTIAL_ROLLBACK);
            return Err(e.into());
        }
    }
}

sfence_vma_all();
Ok(SyscallReturn::mmio_mapped(user_base, size_bytes))
```

`remap_page` is replaced by `map_page` (with unmapped-precondition
assertion).  Pre-existing mappings at the target VA are an error.

### `copy_to_user` documentation alignment

The current `copy_to_user_bytes` validates:

```text
- Arithmetic: src+len does not overflow user-address-space bounds.
- Page table: each touched page has PTE_W set in the target AS.
```

It does NOT validate against a separate VMA-region map.  v0.7.4-002
takes **option (a)**: update the docs to say so.

```rust
/// Copy `len` bytes from kernel `src` to user `dst` in `task`.
///
/// VALIDATION:
/// - The user range `[dst, dst+len)` must not overflow.
/// - Every page in the range must have PTE_W set in `task`'s
///   address space.  Pages without PTE_W or without a mapping
///   produce `MmError::CopyToUserFailed`.
///
/// NOTE:
/// This function consults the page table directly.  It does NOT
/// validate against a separate VMA-region map; Fjell OS v0.7 does
/// not maintain such a map.  If you need region-level validation
/// (e.g., "this address belongs to a writable data segment, not a
/// stack guard"), the caller is responsible for the lookup.
pub fn copy_to_user_bytes(...) -> Result<(), MmError> { ... }
```

This is the honest, accurate documentation.  Option (b) — adding a
VMA map — is the kind of change that affects the entire kernel
memory subsystem and is deferred to a dedicated v0.8 RFC.

### Test coverage

```text
- sys_mmio_map(size = usize::MAX) → InvalidArgument
- sys_mmio_map(size = 0)          → InvalidArgument
- sys_mmio_map(size > MAX_MMIO_REGION_BYTES) → InvalidArgument
- sys_mmio_map with a synthetic frame allocator that fails on the
  3rd page → 2 pages rolled back, total rollback observed via
  page-table inspection
- copy_to_user(dst spans two pages, second page unmapped) →
  CopyToUserFailed
- copy_to_user(dst = usize::MAX - 1, len = 4) → CopyToUserFailed
```

## Data Model

No new types.

## Internal Design

### `map_page` vs `remap_page` discipline

Going forward:

- `map_page(va, pa, perms)`: asserts the VA is currently unmapped,
  returns `Err(AlreadyMapped)` if it is.  Used for new allocations.
- `remap_page(va, pa, perms)`: explicitly replaces an existing
  mapping.  Documented when this is intended; `unsafe` if the caller
  cannot prove it is correct.

A workspace-wide sweep replaces `remap_page` with `map_page` wherever
new device-VMA allocation happens.

### Rollback budget

The mapping loop uses a small `heapless::Vec` to track which pages
were mapped.  Capacity is `MAX_MMIO_REGION_BYTES / PAGE_SIZE` = 16384.
This requires a `heapless::Vec<UserVa, 16384>` on the kernel stack
(or `heap`), which is 64 KiB.  For v0.7.4 we cap individual MMIO
maps at 1024 pages (4 MiB) and bound the vector accordingly.  Larger
regions must use multiple `sys_mmio_map` calls.

### Audit events

```text
AUDIT_MMIO_MAP_PARTIAL_ROLLBACK      = 0x0501
AUDIT_MMIO_MAP_OVERFLOW              = 0x0502
AUDIT_COPY_TO_USER_PTE_REJECTED      = 0x0503
```

## Security Design

### Pre-RFC failure modes

1. `sys_mmio_map(size = usize::MAX)` → size_bytes wraps to 0 →
   sys_mmio_map returns success with zero pages mapped → caller
   thinks they have a valid region but cannot access anything.  Not
   directly exploitable, but a denial-of-service.
2. `remap_page` fails partway through a multi-page map → caller
   receives success → reads from unmapped middle pages → page fault
   delivered to user but the syscall return claimed success.  Caller
   logic may proceed assuming the region is valid.
3. `copy_to_user` documentation says VMA-validated → reviewer
   accepts a code path that writes to a region whose VMA is
   compromised (no actual VMA map exists) → false security analysis.

All three are closed by this RFC.

### Documentation accuracy as a security property

The reviewed code paths are not actively exploitable today, but the
documentation gap is exploitable in the threat-model sense: an
auditor reasons under false premises.  Fixing the docs is a security
deliverable.

## Memory / Resource Design

- Per-mapping rollback vector: 1024 × 8 B = 8 KiB on the kernel stack
  (or moved to a per-task scratch area).
- `MAX_MMIO_REGION_BYTES` cap of 4 MiB; larger requests must be
  split.

## Compatibility and Migration

- `sys_mmio_map` return semantics tighten: callers that previously
  succeeded with `size = usize::MAX` now receive `InvalidArgument`.
  No legitimate caller does this.
- `copy_to_user_bytes` behaviour is unchanged; only documentation
  changes.

## Test Strategy

```text
- sys_mmio_map_overflow_returns_invalid_argument
- sys_mmio_map_zero_size_returns_invalid_argument
- sys_mmio_map_above_max_returns_invalid_argument
- sys_mmio_map_partial_failure_rolls_back_fully
- sys_mmio_map_double_map_at_same_va_returns_already_mapped
- copy_to_user_overflow_address_returns_error
- copy_to_user_unmapped_page_returns_error
- copy_to_user_partial_page_writable_returns_error
```

## Acceptance Criteria

```text
- All 8 acceptance tests pass.
- No call site in fjell-kernel ignores remap_page or map_page result.
- copy_to_user_bytes docs reference the actual validation, not VMA.
- AUDIT_MMIO_MAP_PARTIAL_ROLLBACK is observed in QEMU smoke when
  a synthetic failure is injected.
- ADR-v0.7.4-002 filed.
```

## Documentation Requirements

```text
- docs/src/reference/mmio-syscalls.md updated for new error cases.
- docs/src/reference/user-copy.md created (or refreshed) with the
  honest validation semantics.
- UNSAFE_CHARTER.md gains an "MMIO mapping" category with the
  rollback invariant.
- ADR-v0.7.4-002 documents the option (a) decision and references
  a future option (b) (true VMA map) as a v0.8+ consideration.
```

## Open Questions

```text
1. Should sys_mmio_map accept a maximum-region-bytes argument from
   userspace, or is a global cap sufficient? Proposal: global cap
   for v0.7.4; per-call cap if v0.8 service profiles need it.

2. True VMA map: when? Proposal: scope-out for v0.8.X; if the kernel
   acquires a region-based memory model anyway (e.g., for swap or
   memory-typed devices), VMA validation rides along.

3. Should map_page be the default and remap_page be a less-safe
   counterpart with an unsafe marker? Proposal: not unsafe (it is
   not memory-unsafe in the Rust sense), but document the discipline
   clearly.
```

## Release Gate

```text
- 8 acceptance tests in CI
- No release path uses `let _ = remap_page(...)`
- ADR-v0.7.4-002 accepted
```
