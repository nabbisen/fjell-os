# Developer Handoff — RFC-0.47-002

**Governing RFC:** [RFC-0.47-002](../../done/RFC-0.47-002-the-lifecycle-policys-kept-rules-are-written-down.md)
**Milestone:** 0.47 · second line
**Status:** inherited from the governing RFC (Implemented, 0.47.0)
**Audience:** implementation model

---

## 1. Why this line exists

**The rule that this project's entire errata register applies was written in no normative document.**
`RFC-0.25-002`'s decision D1 kept five things from the in-repo policy — `Accepted`,
`Implemented-with-Errata`, `Closed`, the drift/errata rule and the required-sections list — and its own
acceptance checklist required *"All five kept rules present and checked **by name**."*

**They landed, and a rewrite deleted them.** `be8cf6f` (2026-08-15) put the merged RFC 000 at 430 lines
containing the rule, and its commit message records all five *"verified independently by name …
verbatim as a normative blockquote"*. Then `4d8044a` (2026-10-21), whose entire commit message is
**"RFC 000 life cycle policy: revised"** with no body, rewrote the file — drift rule 1 → 0,
`Implemented-with-Errata` 13 → 0, `Closed` 7 → 0, the `Accepted` row and the required-sections list
gone — reinstating the general-purpose shape RFC-0.25-002 had analysed and rejected. **No erratum. It
stood two months.**

**The first diagnosis was wrong and the correction is the lesson.** Both the architect's filing and
this RFC's first draft said D1 "never landed"; the architect found `be8cf6f` by `git show` at the
commits between, and the implementer re-verified it rather than taking it on authority.

## 2. What this line does

RFC 000 carries a `State | Folder | Meaning` table defining six states, with `Draft` and `Closed`
stated **absent and why**. The drift rule is normative **in the definition of `Implemented`** — a
condition on a state's meaning belongs where the term is defined — with the discovery clause the
original lacked: an RFC found to over-claim *after* marking is **re-marked in the same change that
files the entry**.

**The dependency is reversed, not monitored.** `status.rs` no longer holds a vocabulary; it parses the
policy's table, and `rfc-status-folder` takes each folder's allowed set from the `Folder` column. An
instrument can no longer make a word legal that the policy does not define.

## 3. What it found in the instruments themselves

`rfc-status-folder` treated a Status field naming **no defined state** as *"no Status field"* and
skipped it, so `Closed` or a typo passed unseen; and `Implemented-with-Errata` **matched its own
prefix `Implemented`**, so deleting the state would have silently re-read every such RFC as plain
`Implemented`. A state word must now stand alone.

**The rule's first harvest, at the stage-2 review:** four RFCs re-marked `Implemented-with-Errata`
because a live erratum records their normative text over-claiming — `RFC-v0.12-002` (E-004),
`RFC-0.41-002` (E-129), `RFC-0.33-003` (E-073), `RFC-v0.7.4-003` (E-093). **Three predate the
deletion, so the rule was not being applied while it was still written down.**

## 4. What a green `rfc-policy` does not mean

It cannot judge whether an RFC's normative text is satisfied by the code — that is a reviewer's
question, and it is the rule itself. A green means the rule is written down and its bookkeeping holds.
**E-004 shows both halves: the rule was not applied, and no check could have said so.**
