# Developer Handoff — RFC-0.40-002

**Governing RFC:** [RFC-0.40-002](../../done/RFC-0.40-002-a-revocation-is-audited-under-a-code-the-decoder-knows.md)
**Milestone:** 0.40 · **Independent of 0.40-001 and 0.40-003** — no ordering constraint
**Status:** inherited from the governing RFC (Implemented, 0.40.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a design
conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The measure of this line

**Does a lease revocation appear in the audit trail as a revocation?** Today every one of
them decodes as `Internal`, because the kernel emits 40 and the published decoder knows 16.
When `auditd`'s decoded output names it, and a tier asserts that, this line is done.

**Not the measure:** that the two numbers now match. The property is what a consumer
reads.

## 0.1 This is a small line and the architect has measured its edges

The RFC carries a table of the wholesale enum comparison: **22 variants each, exactly one
mismatched value** (`LeaseRevoked`), **one kernel-only orphan** (`CapDenied = 41`, emitted
by nothing), and `Internal = 255` correctly userspace-only. So you are not opening an
audit; you are closing a known disagreement and disposing of a dead variant.

**Re-derive it anyway (R1)**, by a method that would find the mismatch without being told
to look for it. If my table is wrong, that is the most useful thing you could report.

## 0.2 The direction, and what would change my recommendation

R2 offers two directions and recommends **(a) the kernel emits 16**, because `AuditKind` is
the published ABI and the kernel is the implementation — and because (b) leaves two values
meaning one event with the published one still never emitted.

I checked what constrains it: **no committed evidence file or expected-markers artefact
contains the wire value**, and `auditd:51` is the only decode site. So (a) breaks nothing I
can find.

**If you find something that depends on 40, take (b) and say what.** That has happened in
four consecutive lines — a figure, a step order, a test that depended on the defect — so it
is a live possibility, not a politeness.

## 1. Order

1. **R1**, the wholesale comparison with its control.
2. **R4's tier, demonstrated failing** — today the revocation decodes as `Internal`.
3. **R2**, the direction, stated.
4. **R3**, `CapDenied` deleted.
5. **Re-run R4 green.**
6. **R5**, confirming the snapshot does not move, then **R6** closing E-092 and **R7** the
   gates.

## 2. Traps specific to this line

1. **Taking (b) because it is one line.** Its size is not a reason. The reason would have
   to be something that depends on the value 40.
2. **Asserting the raw `kind` word in the tier.** It passes under (a) and fails under (b)
   while telling a consumer nothing. **Assert the decoded name.**
3. **Deleting `CapDenied` and adding an emitter.** Two changes under one name; the second is
   new behaviour and out of scope. If you think capability denials should be audited, **say
   so in the review request** and leave the variant deleted.
4. **Confusing this with E-091's `AUDIT_LEASE_BIND_REJECTED`.** That code was specified by
   `RFC-v0.7.4-003` and never built, and it belongs to E-091. Not this line.
5. **An ABI snapshot that moves.** Neither direction changes an enum's shape. If
   `abi-snapshot --verify` reports a change, **stop** — something moved that neither
   direction should move.
6. **Rebuild before re-recording the repro baseline**, and commit the `.bin`s with it.

## 3. What to put in the review request

- **R1's comparison**, the method, and whether my table held.
- **The direction you took and why** — and if (b), what depends on 40.
- **What you did with `CapDenied`**, and whether you think a capability-denial event should
  exist (as a proposal, not a placeholder).
- **Both R4 transcripts**, the pre-change `Internal` and the post-change name.
- **`abi-snapshot --verify`'s output**, confirming nothing moved.
- **Exact commands and exit statuses** for every gate in R7.
- **Any place my RFC is wrong.**
