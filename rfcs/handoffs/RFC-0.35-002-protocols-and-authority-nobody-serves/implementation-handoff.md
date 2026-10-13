# Developer Handoff — RFC-0.35-002

**Governing RFC:** [RFC-0.35-002](../../accepted/RFC-0.35-002-protocols-and-authority-nobody-serves.md)
**Milestone:** 0.35
**Status:** inherited from the governing RFC (Accepted, 0.35)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Can a caller tell "no audit records" from "nobody implements this"?** Today it
cannot, and finding out hangs the service. When the shared API describes only what
the system speaks, and no loop in either service re-enters silently on an error,
this line is done.

**Not the measure:** that an audit query works. The ruling is that it is **not**
being built — read the governing RFC's ruling section before you touch anything,
because the temptation to make the comment true by implementing it is the thing
that was explicitly rejected, and for reasons the owner's design bar supplies.

## 0.1 The order that matters

**RFC-0.35-001 comes first.** It establishes that the slots these protocols would
travel through are not installed at all. If you take this line first you will be
retracting protocols while believing the IPC merely goes unanswered, when in fact
it never leaves the sender. Read that RFC's §Summary before R1.

## 1. Order

1. **R1 re-derivation**, with controls, including the reachability question: does
   **anything** in the tree send `BUILD_BUNDLE` or `PUSH` to `diagnosticsd`? My
   answer is no — two sites, the declaration and `diagnosticsd`'s own comparison.
   If you find a sender, stop: the severity changes.
2. **R6 first half** — demonstrate the hang in a scratch build.
3. **R2 / R3** — retract the sub-protocol, and leave the record.
4. **R4** — `transportd` declares only what it uses.
5. **R5** — neither loop spins on absence; state the behaviour you chose.
6. **R6 second half** — show the hang gone.
7. **R7 register and limitations**, one commit, with E-068's consequence corrected.
8. **R8 gates, `test-all`, CI.**

## 2. Traps specific to this line

1. **`AUDIT_RECORD` is a substring.** 19 loose matches across the kernel,
   `fjell-audit-format`, `auditd` and `diagnosticsd`; only **4** are the constant
   (word-boundary probe). Delete by exact match with the control named, or you will
   remove something live. This is the `grep` discipline the register already
   records twice.
2. **Do not install a capability.** Not one. RFC-0.35-001's R5 deliberately
   excludes these endpoints, and installing them here would grant standing
   authority to services that will not use it.
3. **"Unavailable" must be true for the real reason.** A bundle section that says
   the audit data is unavailable because the query is not implemented is honest; a
   placeholder that says it for no stated reason is the defect in new clothes.
4. **Deleting the protocol must not delete the reason.** R3 is not optional
   paperwork — it is the only thing standing between this retraction and its own
   reinvention.
5. **`diagnosticsd`'s readiness send is wrong too** (slot 0, its own shared
   endpoint, under a comment naming service-manager). That is E-072's, in
   RFC-0.35-001. Do not fix it here; do not let it confuse your reading of this
   service's loop.

## 3. What to put in the review request

- R1's probes and controls, and the reachability answer with its evidence.
- The hang, demonstrated, then absent.
- What left `fjell-service-api`, by exact match, with the control.
- The record R3 leaves, and where a reader will find it.
- R4's diff and `:131`'s corrected comment.
- R5: the chosen behaviour for an unexpected tag and for a recv error, in each
  service, and the argument for it.
- E-068 and E-069 closed, with E-068's consequence corrected from "returns nothing,
  silently" to the hang.
- Each gate's own exit status, `test-all`, a CI run id.

Hand back one path: your review request. Do not tag anything.
