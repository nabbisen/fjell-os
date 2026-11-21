# Developer Handoff — RFC-0.46-003

**Governing RFC:** [RFC-0.46-003](../../done/RFC-0.46-003-the-kernel-bounds-revocation-itself.md)
**Milestone:** 0.46 · first line — before 0.46-001
**Status:** inherited from the governing RFC (Implemented, 0.46.0)
**Audience:** implementation model

---

## 1. Why this line is first

Recycling a lease slot (0.46-001) while anyone with `LeaseAdmin` can revoke a lease they do not own
is worse than either alone: a revoke you did not own could return a slot to the pool under its owner.
**003 narrows the blast radius before 001 widens the mechanism.**

## 2. What the kernel already has, and why the obvious fix is not the fix

`sys_lease_revoke` requires `LeaseAdmin` + `LEASE_REVOKE` **and already validates the capability's
scope against the specific lease** (`ObjectScope::Lease(id)`). The bound exists. **All four
`LeaseAdmin` grants carry `ObjectScope::Any`** — and they have to, because the leases they revoke are
created at runtime and cannot be named in a spawn-time grant.

So **do not narrow the grants' scope**. That path ends at granting a capability naming an id that does
not exist yet. **Ownership is the only workable bound for a dynamically created lease**, and
`lt.owner(id)` already exists — `sys_cap_install` uses it to require that an installer owns the lease
it binds.

## 3. The exception is structural, not a convenience

Two `ipc` scenarios have `neg-test` revoke a lease `sample-service` **owns**, and
`fjell-sample-service`'s own header documents that this works *"checked by the `LeaseAdmin`
capability's scope, not by who created the lease"*. **An owner blocked on its own leased capability
cannot revoke it — it is blocked** — so only a non-owner can wake it. Rewriting the scenarios to exit
the owner would stop testing the revoke path.

Hence one named right, `LEASE_REVOKE_ANY`, in **neither** `ALL_NON_META` nor `ALL_DEFINED`
(the `LEASE_BIND` / `CLOCK_READ` precedent), held by `neg-test` alone. **`init` gets none** — it
revokes no lease in the tree, and a right grows with a consumer, never ahead of one.

**This is not a new authority.** Four images could already revoke any lease; the exception takes four
unbounded holders to one named test image and bounds the other three.

## 4. Rulings that are not open

* **The exception must be counted.** `capability-topology` asserts exactly one holder, by name; a
  second holder and zero holders are both failures. A bound nobody counts is not a bound, which is
  this milestone's whole subject.
* **The grant must be minimal.** A second *complete* `LeaseAdmin` pools authority and defeats
  `test_cap_bind_lease_without_authority`. Carry `LEASE_REVOKE | LEASE_REVOKE_ANY` and nothing else.
* **The refusal is audited** under its own kind, not as a result code on the success kind.
  A revocation's **success** was already audited as `AuditKind::LeaseRevoked = 16` (RFC 031–036's
  block, outside the `0x06xx` numbering) — **do not add a second kind for it.**

## 5. What the demonstration has to show, and the trap in it

With two independent bounds — the kernel's and the broker's `revoke_authorised` — **neither single
mutation reddens `REVOKE_FOREIGN_LEASE_REFUSED`.** Mutating the broker's check off leaves it green
because the kernel refuses; mutating the kernel's off leaves it green because the broker refuses.
**The pair is what reddens it**, and the mutation table must say so rather than report a green it
cannot explain. Showing R7 became defence in depth *is* the demonstration.
