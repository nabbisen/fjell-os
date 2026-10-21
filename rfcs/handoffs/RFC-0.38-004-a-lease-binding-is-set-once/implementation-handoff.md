# Developer Handoff — RFC-0.38-004

**Governing RFC:** [RFC-0.38-004](../../accepted/RFC-0.38-004-a-lease-binding-is-set-once.md)
**Milestone:** 0.38
**Status:** inherited from the governing RFC (Accepted, 0.38)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Can a capability escape the lease that governs it?** Today a `LeaseAdmin` holder can
move any capability in its own CSpace to any other `Active` lease, so revoking the
first lease no longer reliably kills it. When a second bind is refused **and the
original lease still kills the capability afterwards**, this line is done.

**Not the measure:** that the second bind returns an error. See R3's third step.

## 0.1 This should be a no-op for the shipped tree, and R1 is how you find out

The cost argument for this line is that nothing rebinds today: bootstrap capabilities
carry `lease: None`, `CSpace::copy` inherits the source's binding, and all three live
call sites bind a copy of an unbound capability. I established that by reading before
scoping, and the RFC names each site.

**So R1 is not a formality here — it is the premise.** If a fourth caller exists, or
any live caller binds an already-bound capability, the refusal stops being free and the
line needs rethinking. **Escalate before the refusal lands**, not after a tier goes red.

## 0.2 Two places where the obvious move is the wrong one

1. **Do not allow an idempotent rebind.** Refusing a second bind to the *same* lease
   and epoch will feel pointlessly strict. It is deliberate: "a binding is set once" is
   a rule nobody misreads; "set once, unless it matches" has an edge every caller must
   reason about, and the owner's standard for this project is an API that cannot be
   misunderstood. If you think a caller needs the exception, escalate.
2. **Do not add `cap_unbind_lease`**, and do not make `cap_copy` strip the binding. The
   first reintroduces the escape this line removes under a franker name; the second
   breaks the inheritance that makes cascading revocation work at all. Both are listed
   as non-goals for this reason.

## 1. Order

1. **R1 completely**, including the sweep for every writer of `cap.lease` and every
   caller of the syscall. This is the premise check (§0.1).
2. **R3's tier, demonstrated failing (R4), before R2.** Against the current kernel the
   second bind succeeds and replaces the binding — capture that. It is the evidence
   E-090 was real and it cannot be reconstructed after the refusal exists.
3. **R2, the refusal.** It goes before the write, with the authority check that
   RFC-0.38-002 added still first — authority, then target state, then the lease's
   state, then the write.
4. **Re-run R3 green.**
5. **R6, the audit kind — check whether it is ABI before touching it.** Leaving the
   mislabel is an acceptable outcome of that check.
6. **R5, the two documents.** `cspace.rs`'s doc comment corrected; `RFC-v0.7.4-003`
   gets a dated correction beside its text, not a rewrite.
7. **R7**, closing E-090 only, and **R8**, the gates.

## 2. Traps specific to this line

1. **Asserting the refusal instead of the invariant.** R3's third step — revoke the
   first lease, show the capability dies — is the test. Without it the tier passes
   against an implementation that refuses the call and corrupts the binding anyway.
   This is the third time in this milestone that this exact distinction has decided a
   requirement; it is the house standard, not a stylistic preference.
2. **E-091 is not yours.** E-090 is narrowed to the rebind half; the unbuilt
   `LEASE_BIND` right, the scope check and "which lease may a holder bind to at all"
   are E-091, ACCEPTED and unscheduled. Do not close it, implement toward it, or let
   `v1-limitations.md` imply this line touched it. The two halves read as one topic and
   are not.
3. **Ordering inside the handler.** Authority (RFC-0.38-002) stays the first gate, so
   an unprivileged caller still gets `PermissionDenied` rather than learning whether
   the target was already bound. Do not let the new check leak that.
4. **`/usr/bin/grep -a`, not `grep`.** Plain `grep` skips NUL-containing files here, and
   a blind "0 sites" has closed an erratum wrongly in this project before. Control every
   absence probe against a file you know contains the string — R1's sweep especially.
5. **The published-record distinction.** `cspace.rs`'s doc comment describes current
   behaviour, so correct it outright. `RFC-v0.7.4-003` is a decision record, so the
   correction sits beside it and the original stays. Getting these the wrong way round
   either erases history or leaves a live lie.
6. **Rebuild before re-recording the repro baseline**, and commit the `.bin`s in the
   same commit.

## 3. What to put in the review request

Per the roles document §7, and specifically for this line:

- **R1's premise check as a conclusion with its command and control**: every writer of
  `cap.lease`, every caller of the syscall, and whether any live caller binds an
  already-bound capability. State it even — especially — if it confirms what I claimed.
- **Which error R2 returns and why**, and whether you reused an existing `SysError` or
  judged that none said it honestly.
- **Both R3/R4 transcripts**, pre-fix red and post-fix green, with the third step
  (revoke kills the capability) visible in the green one.
- **R6's ABI determination**: what you checked, what you concluded, and what you did.
- **Confirmation that E-091 is untouched.**
- **Exact commands and exit statuses** for every gate in R8.
- **Any place my RFC is wrong.** The last two lines in this milestone each found one —
  a false mechanism in my E-088 reasoning, and a cause I had reduced to "possibly" —
  and both are better documents for it. The tree wins.
