# Developer Handoff — RFC-0.27-001

**Governing RFC:** [RFC-0.27-001](../../done/RFC-0.27-001-document-self-verification.md)
**Milestone:** 0.27
**Status:** inherited from the governing RFC (Implemented, 0.27.0)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. The demonstration is sitting in the tree, and you can destroy it

**E-014, E-015, E-016 and E-017 all read `0.25 candidate`. 0.25 shipped on
2026-08-16 and 0.26 on 2026-08-27.** That is four errata pointing at a milestone
that has passed, on real committed data, right now.

**That is S1's demonstration, and normalising the column first destroys it
permanently.**

Build S1. Run it. Capture it failing on those four. *Then* fix them.

This is the third line running where a live failure was available for free and
the instruction was not to spend it — the `asm-instruction` tag in
RFC-0.24-002, the red tree in RFC-0.26-001, and now this.

## 0.1 What the review will ask of each subcheck

**"Show it failing."** Not "show the tree is green." Every one of S1–S5 gets a
deliberately broken input and an observed failure, per RFC-v0.22-001. A subcheck
that has only ever been seen passing has no evidence it works.

## 0.2 Design decisions settled — do not re-open

1. **S1 before S2** (§0).
2. **Subchecks of `fjell-consistency-check`**, not a new binary. Gate 12 already
   runs four of exactly this kind.
3. **S3/S4/S5 are stated as properties.** The mechanism is yours — but say what
   you chose and why, especially for S5.
4. **D4 is a requirement, not a nicety.** `ROADMAP.md` and the audit close-out
   must *cite* the backlog, not restate it. Skipping it leaves the triplication
   that caused this line.

---

## 1. S1 — the one that matters

Three properties, in the RFC. The second is the point: **no erratum may name a
milestone that has already shipped.**

**Open design question, deliberately not decided:** how does the check learn
which milestones shipped? `git tag`, `docs/release/records/*.md`, or
`CHANGELOG.md` headings.

My inclination is the release records — self-contained, and it avoids making an
instrument depend on git state, which would behave differently in a shallow
clone or an exported tarball. **But state your choice and your reason.** If you
pick git tags, say what happens in a checkout without them.

## 2. S2 — re-disposition honestly

Four values, four shapes, none parseable:

```
0.25 candidate (recorded, not fixed)
RFC after v0.23.0 (recorded, not fixed)
0.27 candidate, with E-016 (recorded, not fixed)
v0.21.3-001 (v0.22 disposition)
```

Normalise to an RFC id or a bare milestone; the commentary moves into the entry
body.

**`unscheduled` is a legitimate value.** E-014, E-015 and E-017 have drifted
through two releases; if nobody intends to schedule them for 0.27, say
`unscheduled` rather than writing `0.27` to make the check pass. **Writing a
milestone you do not mean is the same defect in a new coat.**

## 3. S4 and S5 — where this line will try to grow

The audit recorded **13 broken links**. That number is from 2026-08-03 and will
have moved.

**Fix what is mechanical. Record what is not.** A broken link that needs a
decision about where a document should live is a finding, not a task for this
line. A remaining broken link that is *recorded* is an acceptable outcome; an
unrecorded one is not.

For S5, do not attempt "verify every number in every document" — it is not
buildable. Start with `rfcs/README.md`'s file counts, which drifted 162 → 166
and were corrected by hand. **Say which counts you excluded and why.**

## 4. Prohibited shortcuts

- Do not normalise the tracking column before S1 is demonstrated failing on it.
- Do not write a milestone into an erratum to make a check pass. Use
  `unscheduled`.
- Do not skip D4 because it is documentation.
- Do not chase every broken link. Fix, or record.
- Do not build a general document linter. Five named checks, each with a defect
  on the record.
- Do not run `cargo fmt --all --check` in your head.

## 5. Required evidence

1. **S1 failing on E-014/015/016/017 as they stand**, captured before S2.
2. Each of S1–S5 failing on deliberately broken input, and passing after.
3. Your answer to §1's design question, with the reason.
4. Tracking column normalised; E-014/E-015/E-017 re-dispositioned; **no erratum
   naming a shipped milestone**.
5. **E-016 and E-023 → `CLOSED`**, `ERRATA.md` and `v1-limitations.md` in the
   **same commit**.
6. `ROADMAP.md` and the close-out citing the backlog; the 0.27 list in exactly
   one place.
7. Gate 12 reporting the new subchecks; `syscall-surface` still **35/29/6**.
8. `release-rehearsal` green; `test-all` 21/21; `fmt` clean.

## 6. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Which counts and links you excluded from S5/S4, and why.** That is where
  this line's honesty lives — an excluded case that is recorded is fine, one
  that is quietly dropped is not.
- Your answer to the shipped-milestone design question.
- Anything you found while reading that is not in this RFC. Every line in this
  milestone series has turned up at least one, and the last three were found
  that way rather than by any check.
