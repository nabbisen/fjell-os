# Developer Handoff — RFC-0.37-002

**Governing RFC:** [RFC-0.37-002](../../done/RFC-0.37-002-instruments-that-cannot-be-bought.md)
**Milestone:** 0.37
**Status:** inherited from the governing RFC (Implemented, 0.37.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Can exit criterion 2's "0 warning lines" be bought by a crate-level `allow`?**
Today it can, and behind one of them sits a complete IPC event loop the compiler
knows is dead. When every surviving suppression states its reason at the narrowest
scope that works, and the criterion reports how many survive, this line is done.

**Not the measure:** that the allows are gone. Some are right. The defect is the
**unstated blanket**, not the blanket.

## 1. Order

1. **R1** — re-derive the inventory with a control, and for each of the three
   `unreachable_code` suppressors, **delete the allow and build** to see whether it
   suppresses anything. My reading: `netd` yes, `secure-transportd` and
   `fjell-driver-virtio-net` no. Confirm or refute.
2. **R2** — remove every allow that suppresses nothing, each demonstrated.
3. **R3** — every survivor states its reason, narrowest scope.
   `fjell-measuredd`'s existing one-line comment is the shape to copy.
4. **R4** — E-084: `netd`'s code says what the service does. **A decision with a
   reason, not the shortest diff.**
5. **R6** — the failing case for R5's counter, before R5 is believed.
6. **R5** — criterion 2 reports the surviving count. **A figure in the record, not
   a gate that fails.**
7. **R7** — E-082 and E-084 closed.
8. **R8** — gates, as the required set.

## 2. Traps specific to this line

1. **Do not delete `netd`'s loop just to quiet a lint.** It is a complete
   implementation — receive, `LinkUp`, pre-allocating a session for
   `secure-transportd`. R4 wants the decision stated; if it is kept for a future
   line, put it where a reader cannot mistake it for live code.
2. **Do not turn criterion 2 into a gate that fails on a reasoned allow** (R5). A
   cut blocked by a legitimate suppression would be a worse instrument than the one
   we have.
3. **A counter first seen reporting zero has tested nothing.** R6 is ordered before
   R5 is trusted, for the same reason RFC-0.35-003's subcheck had to fail on a
   fixture first.
4. **`fjell-fleetd` is not yours.** It is the same class as E-084 and it belongs to
   RFC-0.37-001. If both lines are in flight, do not touch it.
5. **If `netd`'s source changes, a prebuilt moves.** Rebuild then re-record the
   baseline in the same commit.

## 3. What to put in the review request

- R1's inventory and, per `unreachable_code` suppressor, the build with the allow
  deleted — the evidence for "suppresses nothing".
- Every removal, and every survivor with its stated reason and scope.
- R4's decision for `netd` and the reason behind it.
- R6's failing case, then R5's figure, and the sentence the release record will be
  able to state.
- E-082 and E-084 closed; `fjell-fleetd` visibly untouched.
- Each gate by its own exit status, as the required set.

Hand back one path: your review request. Do not tag anything.
