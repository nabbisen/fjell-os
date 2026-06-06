# RFC 039: Safe user copy and real audit drain

**RFC ID:** 039  
**Also known as:** RFC-v0.2-009  
**Status:** Implemented (v0.2.0)
**Target version:** v0.2.0  
**Phase:** Phase 6 — User Copy and Audit Drain  
**Related epics:** E (Persistent Evidence Hardening)

## Problem

### Safe user copy

`copy_to_user` and `copy_from_user` are the highest-risk
primitives in the kernel.  At v0.1.0:

- Some paths copy through raw pointers without checking the user
  range.
- No code path performs page-table-walk validation per page.
- No code path explicitly rejects kernel addresses, unmapped pages,
  or page-boundary crossings.
- No test corpus exercises invalid pointer shapes.

A single missed check here defeats every other security boundary.

### Real audit drain

RFC 020 introduced a real audit drain in user space.  At v0.1.0 the
kernel-to-user transfer still uses ad-hoc records.  The drain
needs:

- A fixed binary record format.
- A cursor model.
- Visibility into dropped count when the ring overflows.

## Proposed fix

### Safe user copy

```rust
pub fn copy_to_user(
    task:     TaskId,
    dst_user: UserPtr,
    src:      &[u8],
) -> Result<(), UserCopyError>;
```

Required validation, in order:

```
1. user pointer non-null
2. target range inside the canonical user address range
3. target range inside the target task’s VMA
4. for each page in the range:
   - page table walk succeeds
   - PTE_U is present
   - PTE_W is present  (for write; PTE_R for read)
5. no kernel address ever accepted
6. no length overflow / wraparound
7. page boundary crossing handled explicitly (page-by-page copy)
```

For v0.2, page-crossing requests **may** initially be rejected with
`UserCopyError::CrossPage` if and only if this behaviour is
documented in `docs/abi/v0.1-inventory.md` (RFC 028) and exercised
by a negative test.  The preferred design is page-by-page bounded
copy.

`copy_from_user` follows the same shape with `PTE_R` instead of
`PTE_W`.

### Fuzz / property-test requirement

The invalid-pointer test corpus must cover at least:

```
- null pointer
- kernel address
- unmapped page
- non-writable page
- page boundary crossing
- length overflow
- address wraparound
- partially valid range
```

These are property-tested on the host (the page-table walker is
pure Rust); they are exercised on QEMU through negative tests.

### Real audit drain

Fixed binary kernel-side record:

```rust
pub struct AuditRecord {
    pub seq:     u64,
    pub tick:    u64,
    pub kind:    AuditKind,
    pub subject: u64,
    pub object:  u64,
    pub result:  i32,
}
```

`auditd` reads the binary records, converts to JSON Lines, and emits
semantic stream events for security-sensitive kinds.

### Cursor model

Initial: a **global drain cursor**.  Per-subscriber cursors are
deferred (no current subscriber needs them).

### Dropped-count reporting

The drain syscall returns the count of records dropped since the
last drain (kernel ring overflow).  `auditd` exposes this through a
state node so the dropped-count is visible without requiring a
crash.

## Rationale

Splitting safe-user-copy and audit-drain into one RFC is a
deliberate scope choice: the audit drain *uses* the user-copy
primitive, and verifying both at once is cheaper than verifying
each in isolation.  The audit drain is also the most security-
relevant `copy_to_user` consumer; getting it right exercises the
hardest cases.

Page-by-page bounded copy is the most defensible default — it
matches Linux’s `copy_to_user`/`copy_from_user` semantics that have
withstood decades of fuzzing.

## Impact

- Crates: `fjell-kernel` (new MM helper, audit module update),
  `fjell-mm`, `fjell-audit-format` (binary `AuditRecord`),
  `fjell-auditd` (binary→JSON converter, dropped-count state node),
  `fjell-abi` (drain syscall signature).
- Backward compatibility: changes the audit-drain syscall ABI.

## Test plan

### Host property tests (no QEMU)
- `copy_to_user` rejects every invalid-pointer-shape case above.
- `copy_to_user` succeeds for valid in-VMA writable ranges.

### QEMU negative tests
- `NEG:USER_COPY:NULL_REJECTED:PASS`
- `NEG:USER_COPY:KERNEL_ADDR_REJECTED:PASS`
- `NEG:USER_COPY:UNMAPPED_PAGE_REJECTED:PASS`
- `NEG:USER_COPY:NON_WRITABLE_REJECTED:PASS`
- `NEG:USER_COPY:LENGTH_OVERFLOW_REJECTED:PASS`
- `NEG:AUDIT:DRAIN_WITHOUT_CAP_REJECTED:PASS`
- `NEG:AUDIT:DROPPED_COUNT_VISIBLE:PASS`

### Acceptance gates
- `auditd` drains actual kernel records (no inline placeholder).
- `copy_to_user` is bounded and VMA-checked at every entry.
- Dropped-count is queryable.

## Implementation notes

- Out of scope: encrypting the audit channel, multi-subscriber
  audit fan-out, audit retention policies (handled by storaged).
- The page-walker must take and release the target task’s VMA lock
  per page; long copies must not hold it across yields.
- The audit binary record size is fixed at 40 bytes
  (8+8+1+padding+8+8+4+padding); the exact layout must be locked in
  `fjell-audit-format` and recorded in the ABI inventory (RFC 028).
