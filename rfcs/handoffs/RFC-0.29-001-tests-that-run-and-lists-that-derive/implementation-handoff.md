# Developer Handoff — RFC-0.29-001

**Governing RFC:** [RFC-0.29-001](../../accepted/RFC-0.29-001-tests-that-run-and-lists-that-derive.md)
**Milestone:** 0.29 — the first line of the audit-backlog theme
**Status:** inherited from the governing RFC (Accepted, 2026-09-08)
**Audience:** implementation model

This handoff directs execution. It does not redefine the RFC. If you find a
design conflict, **stop and escalate** — do not resolve it in code.

---

## 0. Expect failures, and do not treat them as obstacles

**305 tests that have never run in CI are about to run.** Some will fail. On
this host, in CI, or both.

Every one of those failures is a **finding** — it is a test that has been
asserting something unverified for as long as it has existed, in a crate that
backs a release gate. That is the entire point of the line.

**The instinct to exclude the crate that fails is the instinct that produced
`--lib`.** If you cannot make one pass, report it and leave it failing rather
than narrow the tier around it. A red tier with a named cause is worth more than
a green one that skipped the problem.

## 0.1 Design decisions settled — do not re-open

1. **Derive, do not enumerate** (D1). RFC-0.28-005's template.
2. **The new tier is a new instrument** (D2) — demonstrated failing before it is
   trusted.
3. **Say what each flag reaches** (D3). `--bins` gets the 285, `--tests` gets the
   20, `--all-targets` gets both plus benches and examples. Choose deliberately;
   the original defect is that `--lib` was chosen without checking what it
   excluded.
4. **The stale doc-comment is a symptom** (D4). Do not fix *"× 9 categories"* by
   editing the comment.
5. **`store` and `upgrade` are an explicit decision** (D5), not a side effect.

---

## 1. Order

**R1 → R2 → §6 answered → R3 → R4 → close.**

R1 and R2 first: the tier is the cheap half, and running it tells you how much
of the rest of the milestone is actually failures rather than plumbing. Do not
touch the category lists until you know that.

## 2. R2 — break one of the 36

The demonstration should break a test behind **`SYSCALL-CALLSITE-002`**, because
that is exactly the case the erratum describes: a guard whose only check is a
suite no gate runs. Show the tier red, restore, show it green.

A tier demonstrated only on a trivially-broken test in an unrelated crate proves
the harness works. Breaking one of the 36 proves the *erratum* is closed.

## 3. §6 — the authority question, and my lean is weak

**What is the authority for "the negative-test categories"?** Three shapes. I
lean to 3 (profiles on disk, with an opt-out key recorded in the profile),
because D5's two profiles are real: `store` and `upgrade` have no emitting
scenarios and shape 1 forces them red immediately.

**That lean is weak and shape 1 may simply be right** — a profile that cannot
pass arguably *should* be red until someone writes its scenario, and "not
release-gated" recorded in a `.toml` is one more thing that can go stale.
Argue it.

## 4. R4 — absence and intent look identical

23 of 93 crates are never named in `ci.yml`. **Some of that is deliberate**:
`v1-limitations.md` records that the three drill crates backing Gate 8 run at
rehearsal time by design.

Deriving CI coverage will sweep them in. **A deliberate exclusion must end this
line recorded as one** — in the workflow, or in a file the workflow reads. The
defect is not that 23 crates are excluded; it is that nothing distinguishes
"excluded on purpose" from "forgotten".

## 5. Prohibited shortcuts

- Do not exclude a crate to make the new tier green.
- Do not drop a negative category to keep CI runtime down. That is **E-031**,
  which this project has already shipped once.
- Do not fix the doc-comment instead of the lists.
- Do not take on **E-014**. It is the next milestone.
- Do not touch the kernel, ABI, or any service.
- Do not run `cargo fmt --all --check` in your head, and put it in the evidence
  list.

## 6. Required evidence

1. R1's flag choice, with **what each candidate flag reaches** stated.
2. **The tier demonstrated failing** on a broken test behind
   `SYSCALL-CALLSITE-002` (R2).
3. **Every failure the new tier surfaced**, listed — including any you could not
   fix, left failing, and named.
4. **§6 answered in writing**, with the rejected shapes and D5 decided
   explicitly.
5. The five category lists reduced to one derived answer.
6. CI crate coverage derived, with deliberate exclusions **recorded as
   deliberate**.
7. **E-013 and E-015 `CLOSED`** — or, if an E-015 instance survives, said so and
   left open. Register and `v1-limitations.md` in the same commit.
8. Any CI runtime change, reported.
9. `release-rehearsal` green; `test-all` all tiers; `syscall-surface` 35/29/6;
   `callsite-audit` 5 checks.
10. `cargo fmt --all --check`.

## 7. Review request

Standard format, in `.git-exclude/review-request/`.

Flag for focused review:

- **Every test that failed when first run.** This is what I will read hardest.
  A line that turns on 305 tests and reports zero failures is a line I will
  want to see the tier demonstrated for very carefully.
- Your §6 answer, and the `store`/`upgrade` decision.
- Anything that turned out to be deliberate rather than forgotten in the 23.
- Any count of mine you re-derived and found different. Six consecutive lines
  have corrected one.
