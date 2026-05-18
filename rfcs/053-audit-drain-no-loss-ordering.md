# RFC 053: Audit drain no-loss ordering

**RFC ID:** 053
**Also known as:** RFC-v0.2-019
**Status:** Proposed
**Target version:** v0.2.11
**Phase:** MMIO/DMA/audit hardening
**Closes review item:** RB-10
**Depends on:** RFC 039 (Safe user copy)

## Problem

`crates/fjell-kernel/src/trap/syscall.rs:409-414` drains audit records
into a kernel buffer, then copies them to the user buffer one-by-one:

```rust
let (n_drained, n_dropped) = AUDIT.drain_into(&mut kbuf[..max_records]);
// ... lines 421-444 ...
for i in 0..n_drained {
    if copy_to_user_bytes(&kbuf[i].as_bytes(), user_ptr + i * RECORD_SIZE).is_err() {
        break;
    }
    n_copied += 1;
}
tf.gpr[REG_A1] = n_copied;
```

The ring buffer's records are consumed by `drain_into` (cursor advanced)
*before* the copy loop runs.  If `copy_to_user_bytes` fails partway —
because the user buffer ends in a kernel page, was unmapped between calls,
or is shorter than claimed — the records that didn't make it to userspace
are **lost**.  They are no longer in the ring (cursor moved past them) and
they did not reach the user.

The syscall returns `Ok(n_copied)` so the caller (auditd) believes only
`n_copied` records existed.  But `n_drained > n_copied` means
`n_drained - n_copied` records vanished without trace — including the
audit record describing *why* the drain partially failed.

This is exactly the kind of bug that an audit subsystem must not have:
a malicious or buggy auditd, by handing in a partially-invalid buffer, can
arrange for the kernel to silently destroy audit records.

## Proposed fix

Replace consume-then-copy with **peek-copy-advance**.  The ring buffer
exposes a `peek_at(index)` method that returns a record without advancing
the cursor.  The syscall:

```rust
pub fn sys_audit_drain(tf, ...) {
    // ... validate cap (RFC 054), validate user buffer header ...

    let n_available = AUDIT.len();
    let max_records = (user_buf.len() / RECORD_SIZE).min(n_available);
    let mut n_copied = 0;

    for i in 0..max_records {
        // 1. Peek the record without consuming.
        let record = match AUDIT.peek_at(i) {
            Some(r) => r,
            None    => break,
        };
        // 2. Copy to user.
        let dst = user_buf.add(n_copied * RECORD_SIZE);
        if copy_to_user_bytes(&record.as_bytes(), dst).is_err() {
            break;  // stop on first failure — no records consumed yet
        }
        n_copied += 1;
    }

    // 3. Advance the ring cursor by exactly n_copied.
    let n_dropped = AUDIT.advance(n_copied);

    tf.gpr[REG_A0] = SysError::Ok as isize as usize;
    tf.gpr[REG_A1] = n_copied;
    tf.gpr[REG_A2] = n_dropped;  // records that overflowed since last drain
}
```

### Ring buffer API additions

`crates/fjell-kernel/src/audit/ring.rs`:

```rust
impl AuditRing {
    /// Peek at record at offset `i` from the head, without advancing.
    /// Returns None if i >= len.
    pub fn peek_at(&self, i: usize) -> Option<AuditRecord>;

    /// Advance the head cursor by `n` records.  Returns the number of
    /// dropped records *since the last advance call* (which was either
    /// the previous drain or zero at startup).
    pub fn advance(&mut self, n: usize) -> u32;

    /// Existing method retained for kernel-internal uses.  Not used by
    /// sys_audit_drain after RFC 053.
    pub fn drain_into(&mut self, buf: &mut [AuditRecord]) -> (usize, u32);
}
```

`advance(n)` is what makes the ring forget; `peek_at(i)` is read-only.
The existing `drain_into` is retained for callers that don't need
no-loss semantics (none currently; kept for tests).

### `n_dropped` semantics clarification

In the v0.2.8 ABI `n_dropped` was returned alongside `n_drained` and
represented overflows since the last drain.  RFC 053 preserves the
ABI: `advance(n)` reads and clears the dropped counter.  The semantics
are unchanged from auditd's perspective — `n_dropped > 0` still
indicates evidence-gap, used by `NEG:AUDIT:EVIDENCE_GAP_DETECTED:PASS`.

## Rationale

**Why peek-copy-advance and not validate-then-drain?**  The reviewer's
Option A (validate the entire buffer before draining) would require
walking the page table for every page of the user buffer, twice — once
to validate, once during copy.  Option B (peek-copy-advance) achieves
no-loss with only the normal per-page validation.

**Why not record-by-record (Option C)?**  Option C would call
`drain_one` then `copy_one` per record — same effect but each iteration
incurs the ring-buffer locking overhead.  Peek is cheap; advance
once is cheaper.

**Why does the cursor not advance per-record?**  Single advance keeps
the ring's head pointer consistent in a single update.  If the kernel
were preempted between record-peeks (it isn't, in a single-hart
cooperative kernel), the ring would remain in a valid state.

**Why preserve `drain_into`?**  Kernel-internal callers (currently none,
but plausible future uses for snapshot generation) may legitimately
want to consume-without-copy.  Removing the method now would force a
re-add later.

## Impact

### Crates affected

| Crate | Change |
|-------|--------|
| `fjell-kernel/audit/ring.rs` | Add `peek_at`, `advance`; refactor existing `drain_into` to call them |
| `fjell-kernel/trap/syscall.rs` | Rewrite `sys_audit_drain` flow (above) |
| `fjell-syscall` | (no ABI change — return values are the same) |

### Backward compatibility

The syscall's user-visible ABI is unchanged.  Failure modes change:
previously a partial buffer caused records to be lost; now the kernel
returns the same `n_copied` but the *uncopied records remain in the
ring* for the next drain.  This is a strict improvement.

### Audit trail

No new audit events.  The dropped-counter semantics are unchanged.

## Test plan

### Host (unit tests in `fjell-kernel::audit::ring`)

1. `peek_at(0)` on empty ring → `None`
2. `peek_at(0)` returns the same record on repeated calls (cursor doesn't move)
3. `peek_at(n_records)` → `None` (out of range)
4. `advance(0)` is a no-op
5. `advance(n)` clears records from head; subsequent `peek_at(0)` returns
   what was at index `n`
6. Drop-count is preserved across multiple `advance(0)` calls (cleared
   only when a successful drain happens)
7. After `peek_at(0); peek_at(1); advance(2)`, the next `peek_at(0)`
   returns the third record

### Host (integration test in `fjell-kernel::audit`)

8. Simulate partial-copy failure: peek 5 records, "fail" on the 3rd copy,
   advance(2).  Verify records 3, 4, 5 remain in the ring (peek_at(0)
   returns record-3-equivalent).

### QEMU (existing markers, recharacterized)

9. `NEG:AUDIT:EVIDENCE_GAP_DETECTED:PASS` — unchanged behavior; verify
   `n_dropped > 0` after ring overflow.
10. (No new marker needed.  The no-loss invariant is verified by host
    test #8 — QEMU cannot easily synthesize a partial-page-mapped user
    buffer.)

## Implementation notes

- `peek_at` returns `Option<AuditRecord>` by value (records are
  `Copy`-able 32-byte structs in current layout).  No reference
  juggling.
- `AuditRing::head` and `len` remain `Cell<usize>` (single-hart kernel,
  no atomics needed).
- The user-buffer length-check happens once at syscall entry — the
  per-record copy in the loop uses bounded offsets.

## Open questions

- Should `sys_audit_drain` return the new `n_remaining` (records still
  in the ring after this drain)?  Useful for auditd's pacing.
  Recommendation: yes, in `a3`; but defer to a follow-up RFC to keep
  this one narrow.  v0.2.11 unblocks the no-loss property; pacing is
  optimization.
