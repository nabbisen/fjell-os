# RFC-0.41-005 R1–R7: a gate catches prose that still asserts a closed erratum

**Governing RFC:** [../accepted/RFC-0.41-005-a-gate-catches-prose-that-still-asserts-a-closed-erratum.md](../accepted/RFC-0.41-005-a-gate-catches-prose-that-still-asserts-a-closed-erratum.md)
**Handoff:** [../handoffs/RFC-0.41-005-a-gate-catches-prose-that-still-asserts-a-closed-erratum/implementation-handoff.md](../handoffs/RFC-0.41-005-a-gate-catches-prose-that-still-asserts-a-closed-erratum/implementation-handoff.md)

Written against base `610006a`. The subcheck is `closed-erratum-prose`, module
`tools/fjell-consistency-check/src/closed_erratum_prose.rs`. **It catches both instances, and it does so by a rule
narrower than the RFC's R2 asks for — please read "The rule, and where R2 could not be followed" first.**

## The two instances (step 1)

`git show 8ba6762` gives them, before and after:

* **`docs/src/releasing/v1-readiness.md:48`** — *"the record it produces is constants (**E-094**, ACCEPTED, limitations
  row 13): `AttestationRecordV2::dev` supplies every digest as a literal …"*. A closed erratum named beside the word
  `ACCEPTED`.
* **`crates/services/fjell-diagnosticsd/src/main.rs:36`** — *"(the signed attestation record is independent constants,
  E-094; `last_attestation_available` stays `false`)"*. A present-tense claim whose only grounds is the closed erratum,
  inside a parenthesis whose other half is true.

## The legitimate set (step 2, R4) — enumerated and counted

In the scope below, **683 references to 89 closed errata in 515 files** (686 / 89 / 516 once the new module, which cites
errata in its own tests, is in the tree). By the form they take:

| Form | Count |
|---|---|
| a closure word, or `CLOSED`, in the reference's segment (`closes E-NNN`, `closed 2026-10-29 by RFC-… (E-NNN)`, `fixed`, `retired`, `no longer`, `used to`, `since`, the ERRATA-style `(CLOSED)` rows) | 342 |
| an RFC or decision reference in the segment — provenance, `(RFC-0.33-003, E-NNN)`, `RFC-0.38-001 D2 / E-NNN:` | 93 |
| neither: a bare provenance citation `(E-NNN)`, history (`E-NNN was …`), an analogy (`the E-NNN shape`) | 248 |

By area: `crates/` 288, `docs/src/` 266, `tools/` 129. **None is flagged and the exception list is empty.**

## The rule, and where R2 could not be followed

R2 says a reference to a closed erratum must sit within a closure form or be a listed exception. **That rule is
wrong for this tree**, exactly as the handoff's trap 4 predicts: 248 of 683 references carry neither a closure word
nor an RFC reference (bare provenance citations are the habit of the codebase), so requiring a closure form would
need 248 exemptions. I did not widen a list; I re-chose the rule, narrow, on the known shapes — which is the RFC's own
advice ("prefer a narrow rule that catches the known shape over a broad one that catches more and annoys more").

* **A — a live status attached to a closed erratum.** The segment contains a live status word (`ACCEPTED`, `OPEN`,
  `unscheduled`, `IN PROGRESS`, `still open`, `remain(s) open`, `stay(s) open`), and of the status words in the
  segment (live ones and `closed`/`CLOSED`) the **nearest to this id is a live one**; words struck through with `~~`
  are history. Catches instance 1.
* **B — a present-tense claim whose grounds are the closed erratum.** The id closes its segment as an appositive (a
  comma before it, then `;` `)` `.` or the end) after a present-tense assertion verb, and the segment has no closure
  word, no RFC/decision reference and no past-tense marker. Catches instance 2.

A **segment** is the smallest enclosing parenthesis (≤ 400 bytes) or else the sentence/table cell; consecutive comment
or quote lines are joined first, so a wrapped `//` comment is read as one sentence. The rule is written into the
module's own doc comment, with the accepted forms and the scope.

**Scope:** Markdown under `docs/src/` except `history/`, `audit/`, `assurance/`; `README.md` and `crates/*/README.md`;
every `.rs` under `crates/` and `tools/`. **Not in scope:** `rfcs/` (dated documents), `releases/`, `CHANGELOG.md`,
`tests/evidence/`, `ROADMAP.md`, `.github/`, `.toml`, and `rfcs/ERRATA.md`, which is read, never edited (R5).

**Stated blind spots** (in the module doc): it matches forms, not meaning. A live claim of a closed defect that does not
name its erratum, that places the id mid-clause instead of as a tail, or that is hedged with a past-tense word, passes.
B in particular is tuned to the shape instance 2 took. Alternatives measured and rejected: closure-form-for-every-
reference (248 exemptions); any bare id beside a present-tense verb (44 hits on the tree, mostly correct analogies).

## R3 — both reconstructions fail, naming the line (step 5)

Run through the real subcheck on the real files. The pre-8ba6762 text was put back with `git show 8ba6762 -- <file> |
git apply -R`, the check run, and the file restored with `git checkout` (clean `git diff` afterwards):

```
(control, fixed tree)    exit 0  closed-erratum-prose: PASS (686 references to 89 closed errata in 516 live files; …)
instance 1, v1-readiness exit 1  closed-erratum-prose: FAIL -- docs/src/releasing/v1-readiness.md:48: a live status is
                                  attached to a closed erratum (E-094): (**E-094**, ACCEPTED, limitations row 13)
instance 2, diagnosticsd exit 1  closed-erratum-prose: FAIL -- crates/services/fjell-diagnosticsd/src/main.rs:36: a
                                  present-tense claim rests on a closed erratum (E-094): (the signed attestation record
                                  is independent constants, E-094; `last_attestation_available` stays `false`)
(restored)               exit 0
```

Unit tests (9, in the tool's own tier; the host-bins tier, not host-lib) cover each rule, the corrected wordings, thirteen legitimate forms, "a status
word attaches to the nearest id" (both orderings), an id inside a longer identifier (`LEASE-CALLSITE-001` is not
`E-001`), a wrapped comment, line reporting, and a register with no closed errata. The fixtures build the id from a
variable so the module does not contain the stale shapes it tests for (it is in its own scope).

## R6 — the count: 20 → 21

`consistency-check --all` prints `PASS (21 subchecks)` and Gate 12 lists 21. The figure is derived from
`ALL_SUBCHECKS` (`main.rs`), which `lib.rs`'s `SUBCHECK_NAMES` must equal (a test in `main.rs` asserts it); both were
updated, and the hand-maintained list in `main.rs`'s header comment gained the entry. **No book page states the count
or lists the subchecks as current**; the places that say "Gate 12's 20th subcheck" (`release-handoff.md`,
`v1-readiness.md`, `v1-limitations.md`) are dated statements about `readiness-summary` and are correct as written.

## Third instances — reported, not fixed

Enumerating the legitimate set turned up **no live document** that the check flags. Widening the scope to the dated
areas I excluded, the same two rules flag **five** references: four in `ROADMAP.md` (milestone narratives written at
their milestone — "33 findings remain open under errata E-013 through E-017", "E-019 (accepted, not yet implemented)",
"E-013 similarly unscheduled" — all CLOSED now) and one in `docs/src/assurance/instrument-audit.md` (a sentence quoting
a test). I excluded them as dated records, not live claims; **if you read `ROADMAP.md` as a live document, those four are
third instances** and the scope should change. I did not touch them.

One more, of my own making and caught by this gate's logic rather than its code: closing E-103 in RFC-0.41-004 would
have left `v1-limitations.md` row 16 describing it as ACCEPTED — exactly rule A's shape — so E-103 stays ACCEPTED with a
progress note (E-102 likewise). The architect closes them and rewrites the rows together.

## Gates

No prebuilt moved (a host tool). See the review request.
