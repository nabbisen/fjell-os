# RFC-0.38-004 R1–R8: a lease binding is set once

**Governing RFC:** [../done/RFC-0.38-004-a-lease-binding-is-set-once.md](../done/RFC-0.38-004-a-lease-binding-is-set-once.md)
**Handoff:** [../handoffs/RFC-0.38-004-a-lease-binding-is-set-once/implementation-handoff.md](../handoffs/RFC-0.38-004-a-lease-binding-is-set-once/implementation-handoff.md)

Written against base `ea0e9a8`, in the handoff's order: R1, the tier red (R3/R4), the
refusal (R2), green, R6's check, R5, then R7 and R8.

---

## R1 — the premise holds, with one qualification about my own tests

**Conclusion: no live caller binds an already-bound capability. The refusal is a
no-op for the shipped tree.**

| Claim | Found (`/usr/bin/grep -a`) |
|---|---|
| Only writer of `cap.lease` | `crates/fjell-kernel/src/cap/syscall.rs` (`cap.lease = Some(binding)`) — the one match for `\.lease\s*=[^=]` across `crates/` and `tools/` |
| Other `lease: Some(..)` | `fjell-ipc/src/endpoint.rs:128` (`RecvWaiter`) and `fjell-ipc/src/reply.rs:46` (`ReplyEdge`) — different structs, never through this syscall; `fjell-cap/src/enforcement.rs:313` — inside `#[cfg(test)]` (line 149 opens the test module), built with `install_raw` |
| Bootstrap is unbound | `CSpace::install` sets `lease: None` (`cspace.rs:140`); `spawn.rs` has 31 `lease: None` and `main.rs` 17, and **no other value** in either (control: both counts are non-zero) |
| `copy` inherits | `let cap = …clone();` then only `new_cap.parent = None` (`cspace.rs` `copy`) |
| Live callers | `fjell-sample-service:214`, `:278`, `fjell-neg-test:176` — each `sys_cap_copy` of slot 0 (`SLOT_OWN_EP`, unbound) into a scratch slot, then one bind, then drop. Control: the same search finds the wrapper definition and every other site below |

**The qualification.** `fjell-neg-test` now has more callers than the RFC counts,
and three of them *deliberately* attempt a bind on an already-bound capability: the
rebind in `test_cap_rebind_after_revoke` and the two unprivileged binds in
`test_cap_bind_lease_without_authority` — all added by RFC-0.38-002 as negative tests
that assert refusal. They are not callers that depend on a rebind succeeding, so the
premise (nothing *needs* the looseness) holds and I did not escalate. But one of them
changes: see R2. (The two unprivileged binds still get `PermissionDenied`, because
authority is checked first — which is the ordering trap in the handoff, asserted by
tiers that already existed.)

## R2 — the refusal

`BadState`. I considered the existing errors: `SlotOccupied` is about a *destination
CSpace slot* being taken (it is what `cap_copy` returns), which is not this;
`PermissionDenied` would be false, the caller has the authority; `InvalidArg` blames the
arguments, which are fine; `LeaseExpired` is about a lease. `BadState` — used elsewhere
for "no reply edge exists" and "already queued" — says the object's current state
forbids the operation, and that is exactly what a bound capability is. A new `SysError`
would be an ABI addition and I did not judge that any existing one was dishonest.

Order inside `sys_cap_bind_lease`: **authority** (RFC-0.38-002) → **target** (occupied
slot via the generation-checked lookup, else `InvalidCap`; not already bound, else
`BadState`) → **lease** (`Active`, else `LeaseRevoked`; no such lease `InvalidCap`) →
write. Authority stays first, so an unprivileged caller learns nothing about whether
the target was bound. The target is checked before the lease so the answer to "is this
capability already bound" does not depend on the lease's state. Identical lease and
epoch are refused too; there is no exception.

One consequence the RFC does not mention: 0.38-002's tier rebound a *bound* capability
to its *revoked* lease and expected `LeaseRevoked` (R3 of that line). With the target
checked first that attempt is now `BadState`. So `NEG:CAP:REBIND_AFTER_REVOKE_REFUSED`
now expects `BadState`, and 0.38-002's `LeaseRevoked`-on-a-revoked-lease refusal is kept
by a new marker, `NEG:CAP:BIND_TO_REVOKED_LEASE_REFUSED`, which binds a fresh, *unbound*
copy to the revoked lease. Nothing 0.38-002 asserted is lost.

## R3 / R4 — the tier, red first, and a flaw in my first draft of it

`test_cap_binding_is_set_once` (neg-test, `capability` profile): bind a copied
capability to L1; control (it works while L1 is live); try to bind it to L1 again
(`SAME_LEASE_REBIND_REFUSED`) and to a second `Active` lease L2 (`SECOND_BIND_REFUSED`),
each asserted to be exactly `BadState`; **revoke L1 and assert the capability dies**
(`ORIGINAL_LEASE_STILL_KILLS`, exactly `LeaseRevoked`). L2 stays `Active` throughout.

**The first draft passed against the defect.** I ordered the bind to L2 before the bind
to L1. Against the unfixed kernel the capability moved to L2 and then *back* to L1, so
revoking L1 killed it and `ORIGINAL_LEASE_STILL_KILLS` printed PASS — the property step
agreeing with the bug. The pre-fix run said so (the other two markers were red but that
one was green). I reordered so the move to L2 is last; then it is red pre-fix. The
comment in the test records why. This is the handoff's trap 1 by another route: the
last step has to be unable to pass for the wrong reason.

| Stage | Result |
|---|---|
| Unfixed kernel, first draft (`r4-004-prefix`) | exit 1; `ORIGINAL_LEASE_STILL_KILLS:PASS` **present** — the flaw |
| Unfixed kernel, corrected tier (`r4-004-prefix2`) | **exit 1**; missing `REBIND_AFTER_REVOKE_REFUSED` (kernel still said `LeaseRevoked`: `NEG:HARNESS:WRONG_ERROR`), `SECOND_BIND_REFUSED`, `SAME_LEASE_REBIND_REFUSED`, **`ORIGINAL_LEASE_STILL_KILLS`**; forbidden `NEG:HARNESS:UNEXPECTED_OK` ×3 — the last is the capability alive after its first lease was revoked |
| Fixed kernel (`r4-004-postfix`) | **exit 0**, 18 markers: `REBIND_AFTER_REVOKE_REFUSED`, `BIND_TO_REVOKED_LEASE_REFUSED`, `SAME_LEASE_REBIND_REFUSED`, `SECOND_BIND_REFUSED`, `ORIGINAL_LEASE_STILL_KILLS`, plus 0.38-002's |

## R6 — the audit kind is ABI; left as it was

What I checked: `AuditKindInternal` is `#[repr(u16)]` and its discriminant is the
`kind` field of the records `sys_audit_drain` writes. User space decodes that `u16`
with `fjell_audit_format::AuditKind::from_u16` (`fjell-auditd` does, for its JSON
Lines output), and unknown values become `AuditKind::Internal`. `AuditKind` is a
published enum in `tests/abi/snapshot.json` (`fjell-audit-format`, sig-hashed with its
members in order) and the persistent log format carries the `u16`. So a distinct kind
for a bind needs a new `AuditKind` variant — an ABI addition. Per R6 I did not make it
and left `AuditKindInternal::CapMint` as the bind's record; the mislabel stands.

Found by the same check, not touched: the kernel's enum defines `LeaseRevoked = 40` and
`CapDenied = 41`, which the decoder does not know (it has `LeaseRevoked = 16`); a record
of either would read `Internal`. I did not check whether the kernel appends them.

## R5 — the two documents

* `CSpace::copy`'s doc comment is corrected outright (it describes behaviour): a copy
  inherits the source's binding, so a bound source needs nothing more; an unbound one
  is bound once each; to move a capability, drop it and copy again — there is no
  unbind.
* **There is no `docs/src/adr/ADR-v0.7.4-003-*.md`.** The "ADR-v0.7.4-003" the code,
  the RFC and E-090 cite is `rfcs/done/RFC-v0.7.4-003-capability-authority-hardening.md`
  (no ADR of that name exists; `docs/src/adr/` has none). The dated correction sits
  beside the paragraph in that RFC, original left as written. It says three things:
  the "recursive primitive" now holds without the qualification; **"every meaningful
  cap MUST have a lease" was never enforced and is not true** (bootstrap capabilities
  are `lease: None`); and the ADR name is that RFC. The second is a finding from R1,
  not something the RFC asked me to say — flagged for review.
* `v1-limitations.md` row 9 (the architect's) says "Being closed by RFC-0.38-004"; now
  it records that it is closed and what stands, and points at row 10 (E-091) as the
  remainder. Row 10 is untouched.

## R7 — E-090 closed, E-091 not

E-090's Summary row is `CLOSED`, with a closure note; E-091 is unchanged (ACCEPTED,
`unscheduled`), row 10 of `v1-limitations.md` is unchanged, nothing was built toward
`LEASE_BIND` or a scope check.

## R8 — gates

See the review request. ABI snapshot 476 → 480: four `negative_markers` string
constants, no kernel surface. `fjell-neg-test.bin` is the only binary that moved (the
kernel is not in the prebuilt set); the baseline was rebuilt-then-recorded, one line.

## Left alone, and noticed

* No unbind, no `cap_copy` change, no lease lifetimes, no `LEASE_BIND`, no scope check.
* The audit record for a bind is still `CapMint`; a refused bind records nothing.
* A holder of `LeaseAdmin` can still bind an *unbound* capability to any `Active` lease
  (E-091's question), and can still `sys_cap_drop` a bound capability and re-copy from an
  unbound source — the route the RFC names for moving a capability. That route needs a
  free slot and an unbound source; it is explicit and visible in the audit ring.
