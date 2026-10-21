# RFC-0.38-004: A lease binding is set once

**Status:** Accepted (0.38) — accepted 2026-10-22
**Milestone:** 0.38
**Tracks.** **E-090** — `sys_cap_bind_lease` replaces an existing `LeaseBinding`
unconditionally, so the holder of a capability can move it off the lease that governs
it before anyone revokes that lease. The architecture's only cascading-revocation
mechanism therefore has an opt-out available to the party being revoked.
**Touches** *(indicative)*: `crates/fjell-kernel/src/cap/syscall.rs`,
`crates/fjell-cap/src/cspace.rs` *(doc comment)*,
`rfcs/done/RFC-v0.7.4-003-capability-authority-hardening.md` *(dated correction)*, `tests/qemu/profiles/`,
`crates/services/fjell-neg-test/`, `rfcs/ERRATA.md`,
`docs/src/releasing/v1-limitations.md`.
**Relates to:** **RFC-0.38-002**, which gave this syscall the authority check its own
doc comment had always promised — the same defect class one privilege level down, at
**E-088**; **RFC-v0.7.4-003**, whose `parent = None` decision rests on the promise this
line makes true; and **E-091**, the first-bind half of E-090, which this line
deliberately leaves open.

## What is wrong

`sys_cap_bind_lease` writes `cap.lease = Some(binding)` whatever was there before. A
task holding `CapKind::LeaseAdmin` + `CapRights::LEASE_CREATE` can therefore take any
capability in its own CSpace and move it from the lease that governs it to any other
`Active` lease.

That matters because of what `RFC-v0.7.4-003` decided. `CSpace::copy` sets
`parent = None` — kernel `cap_revoke` deliberately does **not** cascade across copies —
and the ADR justifies that choice on a promise, quoted from the doc comment above
`copy`:

> Copies are **independent authorities**. Kernel revoke only clears the single slot
> being revoked. **Recursive revocation is done via lease revocation**, not cap-parent
> links. Any copied cap that shares the same `LeaseBinding` as the original is
> automatically invalidated when that lease is revoked.

**That promise is only as strong as the binding's immutability.** With a mutable
binding, "revoke lease L invalidates everything bound to L" is not a guarantee but a
default, and the party whose authority L governs can opt out of it at any time before
the revocation lands. The alternative mechanism — parent links — was removed on the
strength of the promise, so there is nothing behind it.

This is the same defect class as **E-088**, one privilege level up. E-088 was the
unprivileged case: no authority check at all. RFC-0.38-002 closed it. This is the
privileged case: revocation still unenforced at the binding step, now for the three
tasks holding `LeaseAdmin` (`sample-service` slot 1, `neg-test` slot 4, `init` slot 30).

### Why immutability rather than the scope check RFC-v0.7.4-003 specified

RFC-v0.7.4-003 specified a `LEASE_BIND` right and a check that "scope must cover the
target lease"; neither was ever built (that is E-090's filing, and the unbuilt-right
half is now **E-091**). A scope check **shrinks this hole without closing it**: a holder
whose scope covers two leases can still move a capability between them and escape a
revocation of either.

Immutability is one rule, needs no new rights bit, no scope representation and no ABI
addition, and closes the hole completely. Where the two differ, the simpler mechanism
that fully closes it wins.

### Why this costs nothing today

Established by reading before this line was scoped, and to be re-derived as R1:

- **Every capability installed at bootstrap carries `lease: None`** —
  `CSpace::install` (`crates/fjell-cap/src/cspace.rs:140`), documented at
  `crates/fjell-cap/src/slot.rs:14` ("Bootstrap capabilities carry `lease: None`").
- **`CSpace::copy` clones the capability and clears only `parent`**, so a copy
  inherits its source's binding. In the one case where immutability would forbid
  rebinding — the source is already bound — the copy is *already* governed by that
  lease, so the documented "bind both source and copy" work is unnecessary there.
- **All three live call sites bind a copy of an unbound capability**:
  `fjell-sample-service:214` and `:278`, `fjell-neg-test:176`, each
  `sys_cap_copy` from an unbound bootstrap slot, then bind, then drop.
- The only other producers of a bound capability are the kernel's own IPC reply edges
  (`fjell-ipc/src/endpoint.rs:128`, `reply.rs:46`), which never pass through this
  syscall, and a `#[cfg(test)]` fixture that uses `install_raw` precisely because
  `install` cannot produce one.

So **no existing caller rebinds**, and the refusal is a no-op for the shipped tree.
That is the argument for doing it now rather than once something depends on the
looseness.

## Requirements

**R1 — Re-derive, with controls.** Each of the four bullets above. Then **sweep for
every writer of `cap.lease`** across the tree, not just this syscall, and for every
caller of `sys_cap_bind_lease`. `/usr/bin/grep -a` — plain `grep` skips NUL-containing
files here.

**If any live caller binds an already-bound capability, stop and escalate.** The whole
cost argument rests on there being none; if the tree disagrees with me, I want to hear
it before the refusal lands, not after a tier goes red.

**R2 — Refuse a second binding, before the write.** `sys_cap_bind_lease` refuses when
the target capability already has `cap.lease.is_some()`, with a distinct error, decided
before anything is written.

**Refuse even when the lease id and epoch are identical.** An idempotent-rebind
exception would be tempting and is rejected here: "a binding is set once" is a rule a
caller cannot misread, whereas "set once, unless it happens to match" is a rule whose
edge a caller has to reason about. **State which error you return and why** — reuse an
existing `SysError` if one says this honestly rather than adding one.

**R3 — The tier asserts the property, not the error code.** Bind a capability to a
lease; attempt a second bind to a *different* `Active` lease and see it refused; then
**revoke the first lease and show the capability dies**. That last step is the
requirement: it proves the original binding still governs, which is the property. An
assertion that the syscall returned an error would pass against an implementation that
refuses the call and corrupts the binding anyway.

This is the lesson E-088 taught twice over — once in my own wrong reading of it, once
in RFC-0.38-002's R4. Do not restate it; apply it.

**R4 — Demonstrated failing.** The R3 tier must be **red against the pre-fix kernel**,
where the second bind succeeds and replaces the binding, and green after. Both
transcripts in the review request.

**R5 — Make the architecture's promise true in prose.** Two documents currently
overstate it and one understates what is needed:

- `CSpace::copy`'s doc comment instructs "Bind both source and copy to the same lease
  via `sys_cap_bind_lease`". Correct it: when the source is already bound, the copy
  inherits the binding and nothing further is needed; when it is not, each is bound
  once. Under this line the original instruction would be refused in exactly the case
  it was written for.
- `RFC-v0.7.4-003`'s cascading-revocation promise can now be stated without
  qualification. It is a published decision record, so **add a dated correction
  alongside it rather than rewriting it** — the treatment E-028 describes and the one
  RFC-0.38-002 received.

**R6 — The audit record should say what happened, and this is an escalation point.**
A bind is currently recorded as `AuditKindInternal::CapMint`. A bind is not a mint, and
an audit trail that misdescribes a security-relevant event is the same class of defect
as the rest of this line.

**Check first whether the audit kind is ABI.** If `AuditKindInternal`'s discriminants
or the `sys_audit_drain` record format are enumerated by `abi-snapshot` or relied on by
a consumer outside the kernel, adding a kind is an ABI addition — **escalate rather
than deciding it**. If it is kernel-internal, give the bind its own kind. If it is ABI,
say so and leave the mislabel; it is not worth an ABI change inside this line.

**R7 — E-090 closes in this line's commit. E-091 does not, and must not blur.**
E-090 is narrowed to the rebind half (dated note on the entry, split per the E-072 →
E-074 precedent so a closed entry carries no live residual). **E-091 — which lease a
holder may bind to at all, and the unbuilt `LEASE_BIND` right and scope check — stays
ACCEPTED and unscheduled.** Do not close it, do not implement toward it, and do not let
`v1-limitations.md` imply this line addressed it.

**R8 — Gates by exit status.** `cargo xtask release-rehearsal`,
`consistency-check --all`, `readiness-check`, `abi-snapshot --verify`,
`fjell-repro-check`, `test-all` to completion, `callsite-audit`, `cargo fmt --check`.
A kernel change moves the prebuilts: rebuild **before** re-recording the repro
baseline and commit the binaries with it. One `test-all` at a time.

### Non-goals

- **`LEASE_BIND`, the scope check, and the first-bind question** — all E-091. This line
  narrows what a bind may *replace*, not what it may *target*.
- **Making `cap_copy` strip the binding.** Binding inheritance is load-bearing for
  cascading revocation; removing it would break the mechanism this line exists to
  secure.
- **An unbind operation.** If a capability must leave its lease, the existing answer is
  to drop it and re-copy from an unbound source — explicit and auditable. Adding
  `cap_unbind_lease` would reintroduce exactly the escape this line removes, with a
  franker name.
- **Lease, task, endpoint and frame lifetimes.** Still the register's.
- **Any addition to the kernel's ABI surface** — a syscall, a syscall number, a
  `BootInfo` field, a `CapRights` bit. Escalate. (Marker and tag constants in
  `fjell-service-api` are not that; regenerate the snapshot in this line so the cut
  sees `Added: 0`.)

## Correction, 2026-10-22 (architect)

This RFC cited **`ADR-v0.7.4-003`**, at a path under `docs/src/adr/`. **No such file
exists.** The decision record is
`rfcs/done/RFC-v0.7.4-003-capability-authority-hardening.md`, and the name `ADR-v0.7.4-003`
survives only inside that RFC's own text and in a `cspace.rs` doc comment that pointed
at it — which is where I took it from, without opening it. Corrected above. The
implementer caught it, fixed the code comment, and recorded the naming in the RFC
itself.

## Risks

- **The idempotent-rebind exception.** It will look harmless and it is the one thing
  that would put the edge case back. R2 forbids it deliberately; if you believe a live
  caller needs it, that is an escalation, not a local judgement.
- **A tier that asserts the refusal instead of the invariant.** R3's third step is the
  whole test.
- **The audit kind turning out to be ABI.** R6 says check before changing, and leaving
  the mislabel is an acceptable outcome of that check. Do not spend an ABI addition on
  it inside this line.
- **Scope bleed into E-091.** The two halves read as one topic and are not. R7 is the
  boundary.
