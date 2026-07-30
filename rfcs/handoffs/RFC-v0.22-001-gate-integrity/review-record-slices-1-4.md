# Review Record — RFC-v0.22-001, Slices 1–4, and disposition of two findings

**Reviewer:** architect
**Commits reviewed:** `4cc9e55` (1), `b275035` (2), `deac327` (3), `4f155c5` (4), `a3c4a56` (record)
**Date:** 2026-07-31

## Outcome

**Approved.** All five scope items implemented and every required failure
demonstration verified by me, not accepted on assertion.

Both findings are **dispositioned by the architect in §4 and §5**, and the
corrections applied. `cargo xtask release-rehearsal` now reports **ALL
MECHANICAL GATES PASS** across twelve gates. The line can close.

Marking the release record BLOCKED rather than writing an accepted-risk
statement was the correct call, and stopping to escalate rather than guessing at
either finding was exactly right.

## 1. Verified independently

| Claim | Method | Result |
|---|---|---|
| Failure demonstrations exist **and pass** | `cargo test -p fjell-consistency-check`, `-p fjell-tools callsite` | **Confirmed** — 26 + 12 tests, including all named failure cases |
| Gate 11 is genuine scoping, not `contains` | read `find_function_body` | **Confirmed** — identifier-boundary match on `fn <name>`, then brace-matching. `does_not_match_substring_function_names` covers the boundary case |
| Gate 12 wired, subchecks real | `cargo run -p fjell-consistency-check -- --all` | **Confirmed** — 4 subchecks, all PASS after §4/§5 |
| `expected.toml` is the explicit set, not a count | read it | **Confirmed** — 35/26 plus the 9 names |
| Finding 1 is real | read RFC-v0.17-001 Status vs `v1-limitations.md` | **Confirmed** — and worse than reported, see §4 |
| Finding 2 is real | read ERRATA E-012 and Gate 7's rule | **Confirmed** |

**One verification I could not complete.** I attempted to reproduce the live
Gate 11 demonstration independently — mutating `is_subset_of` inside `mint` in
an isolated worktree — and the command was declined. I verified by reading the
implementation and running the unit tests instead. The submitted live
demonstration is therefore *corroborated but not independently reproduced*.
Recording that rather than implying full verification.

## 2. What is genuinely good here

**The paren-depth choice in `join_wrapped_declaration` is right, and the
reasoning given is right.** Counting angle brackets would desync on `->`, and
this is the failure mode the handoff predicted would ship half-working. It
didn't.

**Slice 3's self-limiting disclosures are the correct instinct** — nested block
comments and raw strings are named as known limitations of a deliberately
textual scanner rather than left to be discovered. Same for leaving char
literals and lifetimes alone with a stated rationale (no search token is a
single character, so no false positive is possible). That is how a heuristic
should be documented.

**The eleven-gates sweep drew the right line**: one live forward-looking claim
updated; three frozen bundles given correction notes; the RFC's and handoff's
own historical narrative left alone. That is the live-documents-vs-dated-
submissions rule applied without being told.

**Not fixing Findings 1 and 2** is the single most important thing in this
submission. Both were within trivial reach. Fixing either would have destroyed
the evidence that the new gates caught something real.

## 3. Judgement calls accepted

- Folding scope item 5 into Slice 4 rather than a fifth commit.
- Routing Gate 12 through `fjell-tools`' existing subcommand indirection, matching Gates 4/5.
- Keeping BCB-CALLSITE-001 file-level (it is inherently cross-file) while running it over stripped text — semantics unchanged, rigour improved.

## 4. Disposition — Finding 1: RFC-v0.17-001 status

**Confirmed, and staler than reported.** The Status read *"Accepted (design
options — requires architect decision)"*. It was wrong in **two** ways:

1. The architect decision it says is *required* was recorded **2026-06-04**.
2. That ruling's v1.0-tier deliverable — TOFU behind `--allow-tofu-provision` —
   **shipped in v0.20.0**.

So the RFC sat in `done/` announcing it was waiting for a decision that had been
made and shipped two release lines earlier.

**Corrected to `Implemented (v0.20.0)`**, with an explicit deferred note for the
untouched tiers (factory station v1.1, hardware-anchored v2+) per RFC 000's
partial-implementation rule. Applied by me — RFC content is architect authority,
and this needed a judgement about what the RFC actually delivered, not a
mechanical edit.

**This is the finding that justifies the whole line.** A status field lied for
two release lines, through multiple reviews including mine, and was caught the
first time a gate looked. That is precisely the class RFC-v0.22-001 exists to
close, and it was closed by the gate rather than by a person.

## 5. Disposition — Finding 2: E-012 and Gate 7

**Gate 7 does not change.** The implementer asked whether it should distinguish
v0-relevant from v1.0-only errata. It should not — that is weakening a check so
a violation disappears, which this RFC prohibits, and a release-scope axis on
the errata register would create a fresh hole for things to hide in.

**The classification was wrong instead.** This register defines OPEN as live,
unresolved drift and ACCEPTED as a documented, deliberate limitation. Not
investigating E-012 was a **deliberate owner decision** (2026-07-30, cutting the
v1.0 checklist audit because v1.0 is not in view). Deliberate deferral is
ACCEPTED semantics — identical grounds to E-004, which is ACCEPTED because the
hardware profile was knowingly never booted.

**Reclassified ACCEPTED**, with the deferral attributed and dated, and recorded
in `v1-limitations.md` under operational notes as Gate 12's `errata-limitations`
rule requires.

**The honesty test, stated explicitly because this move looks like the thing I
banned.** The question is not "does this make the gate pass" but "would I make
this classification if no gate existed?" Yes — E-012 is known, deliberately
deferred, and disclosed, which is what ACCEPTED means here. If the answer had
been no, the correct action would have been to leave Gate 7 red and let the
owner decide whether to accept the risk. Anyone reaching for this precedent must
apply that test first.

## 6. New finding — ACCEPTED is an unguarded escape hatch

Working through §5 exposed a real weakness in Gate 7's design, and it is the
same class this line is about.

Gate 7 enforces `0 OPEN`. ACCEPTED has **no gate of its own** beyond appearing
in a limitations document. So the reliable way to pass Gate 7 is always to move
an item OPEN → ACCEPTED. The bucket is unbounded, has no owner acknowledgement,
and has no review-by date. Nothing stops it becoming where inconvenient truths
go to be technically disclosed.

Today it holds three items and each is genuinely deliberate. The structural
pressure is what concerns me, not the current contents.

Candidate remedies for v0.23 — **not** decided here: require an explicit owner
acknowledgement per ACCEPTED item; or a review-by milestone that fails the gate
once passed. **Tracked as a v0.23 candidate.**

## 7. Answer to the open question (§5 of the request)

**Proceed.** Both findings are dispositioned and both gates are green — I ran the
full rehearsal after applying the corrections: twelve gates, `ALL MECHANICAL
GATES PASS`, Gate 9 manual and correctly unsigned.

Remaining for you:

1. Bump `Cargo.toml` to `0.22.0` and add the dated `[0.22.0]` CHANGELOG entry.
2. Re-run the full exit-criteria sweep **at your final commit** and rewrite
   `docs/release/records/0.22.0.md` — remove the BLOCKED marker, record the
   twelve-gate table, and note both findings under what this release resolved.
   Do not reuse my output above; the tree has changed.
3. Submit for review and owner tag approval. **Do not tag.**

You were right that the version bump was not yours to assume. It is mechanical
once the release is decided, and it now is — the tag remains the owner's.
