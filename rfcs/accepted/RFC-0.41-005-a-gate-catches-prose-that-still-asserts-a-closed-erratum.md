# RFC-0.41-005: A gate catches prose that still asserts a closed erratum

**Status:** Accepted — accepted 2026-10-30
**Milestone:** 0.41
**Tracks.** No erratum. This is the **RFC-v0.22-001 shape**: a class of stale-status defect found
by instance, fixed by a subcheck that makes the class unrepresentable. Instances below.
**Touches** *(indicative)*: `tools/fjell-consistency-check/src/` (a new subcheck module,
`main.rs`, `lib.rs`), `docs/src/` (the subcheck list), `rfcs/ERRATA.md` (read only).
**Relates to:** **RFC-v0.22-001**, which created `rfc-status-folder` and `handoff-status` from
exactly this kind of enumerated instance — its Motivation instance 4 was *"a handoff stayed
`Proposed` after its governing RFC moved to `Implemented`"*; **E-094**, **E-092**, **E-100**, the
errata 0.40.0 closed; **RFC-0.40-001**, whose closure the stale prose contradicted.

## What is wrong

**0.40.0 shipped two documents asserting a defect that 0.40.0 itself had fixed.**

Found 2026-10-30 by the architect while scoping 0.41, and corrected in `8ba6762` — but the
correction is the instance, not the class.

| Site | What it asserted after E-094 closed |
|---|---|
| `docs/src/releasing/v1-readiness.md:48` | the attestation record *"is constants (**E-094**, ACCEPTED, limitations row 13)"*, that `AttestationRecordV2::dev` supplies every digest as a literal and asserts the three `*_verified` flags **true**, and that *"`attestd` holds no endpoint to `measuredd`"* |
| `crates/services/fjell-diagnosticsd/src/main.rs:36` | *"the signed attestation record is independent constants, E-094"* |

Every clause of the readiness row was false at the tag: E-094 is **CLOSED**, the three flags read
`false`, and `attestd` holds a `CALL`-only capability to `measuredd`'s endpoint at slot 4 — read
from the spawn arm, not inferred. The `diagnosticsd` comment's *other* half was still true
(`last_attestation_available` does stay `false`), which is exactly what makes this class hard to
spot by eye: the stale clause sits inside a sentence that is otherwise correct.

**`v1-limitations.md` rows 11 and 13 were swept correctly at the cut**, so this is not a careless
cut. It is a sweep with no instrument behind it. The existing checks cannot catch it:

* `errata-limitations` verifies that every **ACCEPTED** erratum is referenced in
  `v1-limitations.md`. A **CLOSED** erratum has no obligation, so nothing looks at prose about it.
* `errata-tracking` counts entries. A count cannot notice a false sentence.
* `doc-links` resolves links. A stale claim is not a broken link.

So a closed erratum's prose can rot indefinitely, and the longer an erratum has been closed the
more confidently a reader will trust the document that still describes it as live. **This is the
claim-versus-reality class the whole 0.40 milestone was about, one level out: the records about
the fix went stale while the fix was correct.**

## Requirements

**R1 — a subcheck that reads CLOSED errata and the prose that mentions them.** For every entry in
`rfcs/ERRATA.md` whose status is `CLOSED`, find the references to its identifier elsewhere in the
tree and require each to be in a form that does not assert the defect is live.

**R2 — the rule must be mechanical, and it must state what it cannot decide.** Natural-language
sentiment is not checkable. The practical rule, taken from the instances: a reference to a CLOSED
erratum must either sit within a **closure form** — "closes E-094", "closed 2026-10-29 by
RFC-0.40-001 (E-094)", "that would be E-094 with extra steps" — or be **listed as a known
exception** with a reason.

Choose the discriminator from the instances in the tree, not from first principles, and **write
down the forms the check accepts**. A check whose rule is undocumented becomes a check nobody can
satisfy deliberately.

**R3 — show the failing case.** Reconstruct the `v1-readiness.md:48` text as it stood before
`8ba6762`, run the check, and watch it **fail with that line named**; then restore. Do the same for
the `diagnosticsd` comment. A check that passes on the fixed tree is not evidence — the tree is
fixed. **This requirement is the point of the line**: the gate exists because the previous
mechanism, a careful human sweep, had no failing case to prove it worked.

**R4 — a control for the forms that must keep passing.** The tree legitimately mentions closed
errata in many places (`crates/fjell-schema/src/registry.rs`, `tests_v2.rs`,
`fjell-attestd/src/main.rs:208`, the `v1-limitations.md` closure rows, and the ERRATA entries
themselves). Enumerate them, confirm the check passes them, and record the count. If the check can
only pass by exempting most of them, the rule in R2 is wrong and should be re-chosen rather than
widened.

**R5 — `rfcs/ERRATA.md` is read, not edited.** The entry file is the source of truth for status;
this check consumes it. Do not add per-erratum annotations to make the check easier — that would
move the burden onto the record the check exists to protect.

**R6 — the subcheck count moves.** `consistency-check --all` reads 20 subchecks today. It becomes
21. Update the places that state the figure, including the subcheck list in the book, and say the
new number in the record. Letting that count drift is the defect one level out again.

**R7 — the exception list, if any, is small and dated.** If R2 needs an exception list, each entry
carries a reason and a date. An exception list that grows without reasons is how a check stops
meaning anything — the pattern `capability-topology`'s single named waiver avoids.

### Non-goals

* **Re-fixing the two instances.** Both were corrected in `8ba6762` before this line was written.
  This line stops the next one.
* **Checking prose about ACCEPTED or OPEN errata.** `errata-limitations` owns the ACCEPTED
  obligation and Gate 7 owns OPEN. This line is about the *closed* state only, which is the one
  with no current instrument.
* **Natural-language verification.** The check matches forms, not meaning. Where it cannot decide,
  it must say so rather than guess — a check that is confidently wrong is worse than one with a
  stated blind spot.
* **Retrospectively correcting 0.40.0.** The tag is immutable and the released record stands. The
  stale sentences are fixed on `main` for 0.41; what shipped, shipped.

## Risks

**A rule chosen from two instances may not generalise.** R4 is the mitigation: if passing the
legitimate references needs a long exception list, the rule is wrong. **Report that outcome rather
than widening the list** — a check with twenty exceptions tells a reader nothing, and arriving
there would be a real finding about the rule, not a failure of the line.

**False positives land on documents people must edit.** A check that fires on correct prose will be
worked around by rephrasing until it stops complaining, which silently destroys its value. Prefer
a narrow rule that catches the known shape over a broad one that catches more and annoys more.

**The check runs over the whole tree**, including `crates/` comments. Keep it to the file types
where the claim matters and say which they are; a check whose scope is implicit cannot be reasoned
about when it next fires.
