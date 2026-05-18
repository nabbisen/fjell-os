# RFC 050: Specific-error-code negative tests

**RFC ID:** 050
**Also known as:** RFC-v0.2-016
**Status:** Proposed
**Target version:** v0.2.10
**Phase:** Capability/syscall enforcement closure
**Closes review items:** H-06 (and follow-up to RB-05, RB-06)
**Depends on:** RFC 042 (negative test expansion)

## Problem

`crates/fjell-neg-test/src/main.rs` checks failure-mode tests by examining
only `Result::is_err()`:

```rust
let result = sys_ipc_recv(SLOT_SCRATCH_B);
check(result.is_err(), M::CAP_RIGHTS_DENIED);
```

This means a test can emit its marker for the **wrong failure path**.  The
v0.2.8 review concretely identified this risk:

- `RB-05` showed that `auditd`/`neg-test` were granted AuditDrain with
  `CapRights::RECV` instead of `AUDIT_DRAIN`.  As a result,
  `sys_audit_drain` was failing at the *capability check* on entry, not at
  user-pointer validation.  Both `NEG:USER_COPY:NULL_REJECTED:PASS` and
  `NEG:USER_COPY:KERNEL_ADDR_REJECTED:PASS` may have passed because of the
  wrong reason (`PermissionDenied` rather than `InvalidAddress`).
- `RB-06` showed that the slot-collision could cause cap_copy/cap_drop to
  fail at slot-management level, not at the lease-revocation path the test
  intended to exercise.

The audit failed to catch this because `is_err()` is satisfied by both
cases.

The reviewer's recommendation (H-06): assert exact error codes per marker.

## Proposed fix

Introduce a typed helper alongside the existing `check()`:

```rust
/// Boolean check — emits marker if `cond` is true.  Kept for non-Result
/// scenarios (DMA zeroize byte-check, audit-drop-count, etc.).
fn check(cond: bool, marker: &str);

/// Result check — emits marker if `result == Err(expected)`.  Failure
/// emits a diagnostic suffix so log inspection identifies the wrong
/// failure path.  RFC 050.
fn check_err<T, E: PartialEq + core::fmt::Debug>(
    result: Result<T, E>,
    expected: E,
    marker: &str,
);
```

`check_err` semantics:

| Observed | Expected | Output |
|----------|----------|--------|
| `Err(e)` where `e == expected` | `expected` | emit marker (PASS) |
| `Err(e)` where `e != expected` | `expected` | emit `NEG:HARNESS:WRONG_ERROR:<marker>` (diagnostic, distinct from PASS) |
| `Ok(_)` | `expected` | emit `NEG:HARNESS:UNEXPECTED_OK:<marker>` (diagnostic) |

The diagnostic markers are distinct strings (`NEG:HARNESS:*`) so
`qemu-log-check` does not accept them as the original PASS.  This means a
test silently passing for the wrong reason becomes visible as a harness
diagnostic.

### Marker-to-error-code mapping

| Marker | Expected `SysError` |
|--------|--------------------|
| `NEG:CAP:WRONG_KIND_REJECTED` | `PermissionDenied` |
| `NEG:CAP:RIGHTS_DENIED` | `PermissionDenied` |
| `NEG:CAP:LEASE_REVOKED` | `LeaseRevoked` |
| `NEG:CAP:DROP_ON_REVOKED` | (success — `Ok`; this remains a `check()` not `check_err()`) |
| `NEG:CAP:COPY_WITHOUT_RIGHT_REJECTED` (RFC 049) | `PermissionDenied` |
| `NEG:CAP:MINT_WITHOUT_RIGHT_REJECTED` (RFC 049) | `PermissionDenied` |
| `NEG:CAP:REVOKE_WITHOUT_RIGHT_REJECTED` (RFC 049) | `PermissionDenied` |
| `NEG:CAP:INSPECT_WITHOUT_RIGHT_REJECTED` (RFC 049) | `PermissionDenied` |
| `NEG:CAP:WRONG_SCOPE_REJECTED` (RFC 048) | `PermissionDenied` |
| `NEG:CAP:STALE_GENERATION_REJECTED` (RFC 048) | `InvalidCap` |
| `NEG:MMIO:RIGHTS_CHECK` | `PermissionDenied` |
| `NEG:MMIO:BOUNDS_REJECTED` | `InvalidArg` |
| `NEG:MMIO:RAM_GUARD_REJECTS` | `InvalidArg` |
| `NEG:MMIO:ALREADY_MAPPED_REJECTED` (RFC 051) | `InvalidArg` (or new `AlreadyMapped`) |
| `NEG:DMA:RIGHTS_CHECK` | `PermissionDenied` |
| `NEG:DMA:REVOKE_EXPLICIT` | (success — remains `check()`) |
| `NEG:DMA:ZEROIZE_ON_EXIT` | (byte == 0 — remains `check()`) |
| `NEG:DMA:REGION_TABLE_FULL_ROLLBACK` (RFC 052) | `ResourceExhausted` |
| `NEG:USER_COPY:NULL_REJECTED` | `InvalidAddress` |
| `NEG:USER_COPY:KERNEL_ADDR_REJECTED` | `InvalidAddress` |
| `NEG:POLICY:DEFAULT_DENY` | (reply tag `CAP_DENIED` — already specific) |
| `NEG:POLICY:BOOTSTRAP_GUARD` | (reply tag — already specific) |
| `NEG:POLICY:DENY_PRIORITY` | (reply tag — already specific) |
| `NEG:AUDIT:EVIDENCE_GAP_DETECTED` | (n_dropped > 0 — remains `check()`) |
| `NEG:IPC:BLOCKED_RECV_WAKES_ON_REVOKE` | `LeaseRevoked` |
| `NEG:IPC:BLOCKED_CALL_WAKES_ON_REVOKE` | `LeaseRevoked` |
| `NEG:IPC:LATE_REPLY_REJECTED` | `BadState` (or `LeaseRevoked` — implementation defines) |
| `NEG:SVC:START_TIMEOUT_DETECTED` | (lifecycle byte still alive — remains `check()`) |
| `NEG:SVC:FAULT_DETECTED` | (lifecycle == Faulted — remains `check()`) |

For the IPC `LATE_REPLY` test the exact error returned by `sys_ipc_reply`
depends on whether the revoke-cancel path took the reply edge before or
after the reply syscall.  RFC 050 requires the test to accept **either**
`BadState` or `LeaseRevoked` — both are correct outcomes per RFC 034 §3.

### Harness self-check

Add `NEG:HARNESS:CSpace_LAYOUT_VALID:PASS` (review recommendation):

```rust
fn harness_csspace_check() {
    // Verify expected scratch slots are empty before destructive tests run.
    // Failure of any inspect means spawn.rs disagrees with neg-test's slot map.
    let scratch_slots = [10u32, 11, 12, 13];
    let mut all_empty = true;
    for slot in scratch_slots {
        // sys_cap_inspect returns Err(InvalidCap) for empty slot.
        if sys_cap_inspect(CapHandle(slot)).is_ok() { all_empty = false; break; }
    }
    check(all_empty, M::HARNESS_CSPACE_LAYOUT_VALID);
}
```

This runs first in `service_main`, before any destructive test.  If
spawn.rs ever installs a cap into one of the scratch slots, this self-check
fails visibly rather than letting downstream tests pass for the wrong
reason.

## Rationale

**Why distinct diagnostic markers, not panic on mismatch?**  A user-space
panic on assert mismatch would lose the rest of the test run.  Emitting a
distinct `NEG:HARNESS:WRONG_ERROR:<marker>` lets every test run to
completion and produce a clear log signature: any line beginning with
`NEG:HARNESS:WRONG_ERROR:` flags the broken test.

**Why include `UNEXPECTED_OK`?**  An `Ok` result for a negative test is
the most dangerous failure mode — it indicates the security check was
silently disabled.  Diagnostic markers make it visible.

**Why not change the existing `M::*` marker constants?**  Existing PASS
strings are stable and qemu-log-check matches them exactly.  Diagnostic
markers are *additional* strings with a distinct prefix; they do not
collide.

**Why keep `check()` for some tests?**  Several markers don't correspond
to a single `SysError` — they observe an effect (dropped count > 0, byte
== 0, task lifecycle == Faulted, reply tag == CAP_DENIED).  Forcing those
through `check_err` would distort the test.

## Impact

### Crates affected

| Crate | Change |
|-------|--------|
| `fjell-neg-test` | Add `check_err` helper; rewrite test call sites that use boolean `is_err()` |
| `fjell-syscall` | Add `sys_cap_inspect` wrapper if not present (used by harness check) |
| `fjell-service-api` | Add `negative_markers::HARNESS_CSPACE_LAYOUT_VALID`, `HARNESS_WRONG_ERROR_PREFIX`, `HARNESS_UNEXPECTED_OK_PREFIX` |
| `crates/fjell-tools/src/qemu_log_check.rs` | (no change — diagnostics use a different prefix and do not satisfy expected_markers) |

### Test profiles

No profile TOML changes for diagnostic markers — they are not in
`expected_markers`.  The harness layout marker is added to a new profile
`tests/qemu/profiles/harness.toml`, or appended to `capability.toml` since
that runs first.

### Backward compatibility

Test-only change.  No production code paths affected.

## Test plan

### Host (unit tests in `fjell-neg-test`)

Not applicable — neg-test is a no_std service crate; its tests run in
QEMU.

### QEMU

After landing RFC 048-050:

```bash
cargo xtask qemu-negative capability    # 4 + 2 (RFC 048) + 4 (RFC 049) = 10 markers
cargo xtask qemu-negative mmio          # 3 markers (no change)
cargo xtask qemu-negative dma           # 3 markers (no change)
cargo xtask qemu-negative user-copy     # 2 markers (now exact-error-checked)
cargo xtask qemu-negative policy        # 3 markers (already specific)
cargo xtask qemu-negative audit         # 1 marker (no change)
cargo xtask qemu-negative ipc           # 3 markers (now exact-error-checked)
cargo xtask qemu-negative svc           # 2 markers (no change)
cargo xtask qemu-negative harness       # 1 marker (CSpace layout self-check)
```

A successful run produces only the expected markers.  A regression
introduces lines starting with `NEG:HARNESS:WRONG_ERROR:` or
`NEG:HARNESS:UNEXPECTED_OK:` in the QEMU serial log — caught by either
manual inspection or an additional CI grep step (optional).

## Implementation notes

- `check_err` will need `core::fmt::Debug` on `SysError` — already
  derived in `fjell-abi`.
- The diagnostic emission can reuse `sys_debug_writeln` with a string
  built via `fjell-service-api::negative_markers::format_wrong_error`
  (a small helper).  Avoid `format!` — neg-test is no_std.  A
  `&'static str` per marker mapped via match is the simplest path:

  ```rust
  fn diag_wrong(marker: &str) {
      sys_debug_writeln("NEG:HARNESS:WRONG_ERROR");  // single line, marker echoed
      sys_debug_writeln(marker);
  }
  ```

- Add `sys_cap_inspect(handle) -> Result<(CapKind, CapRights), SysError>`
  to fjell-syscall if missing.  Today's kernel `sys_cap_inspect`
  serializes a record into a user buffer; the harness check only needs
  "does this slot have a cap" → a lightweight no-buffer variant
  (or accept the existing buffer-based call and discard the result).

- The harness self-check must be the *first* call in `service_main`.
  If it fails, neg-test should still attempt subsequent tests so the
  whole picture is visible.

## Open questions

- Should `check_err` include the *actual* error in the diagnostic
  output?  Recommendation: yes if no_std `Debug` printing is cheap;
  otherwise emit just the marker name and let the developer inspect
  the test source.  Defer to implementation.

- Should there be a separate `NEG:HARNESS:DIAG:OK:<marker>` for tests
  that emit `check_err` and got the expected error?  Recommendation:
  no — the existing `M::*` PASS line already communicates success.
