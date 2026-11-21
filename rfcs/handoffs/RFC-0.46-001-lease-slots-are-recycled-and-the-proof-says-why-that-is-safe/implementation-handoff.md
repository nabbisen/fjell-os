# Developer Handoff — RFC-0.46-001

**Governing RFC:** [RFC-0.46-001](../../done/RFC-0.46-001-lease-slots-are-recycled-and-the-proof-says-why-that-is-safe.md)
**Milestone:** 0.46 · second line — after 0.46-003
**Status:** inherited from the governing RFC (Implemented, 0.46.0)
**Audience:** implementation model

---

## 1. The premise the architect got wrong, corrected before you build on it

The 0.46 scope said recycling closes **E-110**. **It does not close the measured half.** The 0.45
demonstration looped *request → **drop** the capability*, and **dropping a capability does not touch
its lease**: `sys_cap_drop` takes only the `CapTable`, has no lease-table access, and
`fjell_cap::enforcement::cap_drop` carries the comment *"Note: no lease check"*. All 26 leases stayed
`Active`, so a recycler of `Revoked` slots reclaims **none of them**.

The 0.45-002 request described that loop as *"the grant is dropped **and its lease revoked**"*. It was
not, and the architect carried the clause into the scope without checking the mechanism behind a
measurement he was quoting. **So this line needs a bound as well as a recycler.**

## 2. Three requirements, and what each one closes

| | What | Where | Closes |
|---|---|---|---|
| **R-R** | a `Revoked` slot is reclaimed, generation bumped | kernel, `LeaseTable::create` | the grant-use-revoke cycle running out of slots |
| **R-Q** | an owner holds at most `MAX_LEASES_PER_OWNER` `Active` leases | kernel, `LeaseTable::create` | **one owner exhausting the table for everyone else** |
| **R-G** | the broker grants a requester at most N outstanding | `cap-broker` | fairness among the broker's own clients |

**R-Q is what closes E-110's measured half**, and it is a kernel bound, which is this milestone's
theme. **R-G is service policy and defence in depth — never the only bound**, on the same footing as
`revoke_authorised` after 0.46-003.

**State the property, not the number.** What matters is that **no single owner can deny the table to
everyone else**; a reader who sees only `16` will argue about 16.

## 3. The constraint that outranks the feature

`verification/verus/lease/lease_epoch.rs` models `revoke`, `check_active` and retire-before-wrap and
**does not model slot reuse at all**. The safety argument is *"epochs are monotone within a slot that
is never reused"* — and recycling deletes the second half of that sentence.

So the model gains a `Revoked → Empty → Active` transition and the obligation that a recycled slot
cannot be mistaken for its previous incarnation. **Gate 10 must come back green on the new obligation,
not on the old one re-verified** — a proof that passes without having been told about reuse is the
finding, not the result. **Show it non-vacuous:** drop the generation clause and the file must fail.

## 4. Two answers, not one

`create` must distinguish **the table being full** from **this owner being at quota**. `SysError`
already has the precedent: `TaskTableFull = -35` exists *"Distinct from `NoMemory`"* by
**RFC-0.34-002 D1**. Conflating them tells a caller nothing about whether to back off or whether the
system is in trouble. This is an ABI addition — enumerate it where the knowledge is and regenerate
the snapshot in this line.

## 5. Not in this line, and say so in the RFC

**The task table's generation.** `TaskTable::remove` has no callers, so task generations are always 0
and **26 sites across 7 files** construct `TaskId::new(x, 0)` correctly *because* of that. You are
bumping a generation in a **neighbouring** table. Say the task table is untouched, so a future reader
does not take "generations are live now" as true of both.

**Do not raise `MAX_LEASES`.** Moving a number is not fixing a family.

## 6. One thing the architect did not foresee

Reclamation **must be lazy — on allocation pressure, not in `revoke`**. A capability bound to a lease
that has just been revoked must still answer `LeaseRevoked`, and an eager reclaim turns that into a
race with the next `create`. That is a correctness argument, and it is better than the
allocation-pressure reasoning the scope offered.
